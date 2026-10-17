//! Hardware-free integration harness for the GUI hardware worker.
//!
//! Drives the real `hardware_thread::Worker::step` loop with a fake clock,
//! recording mock hubs, scripted temperature sensors, scripted telemetry
//! (PSU / GPU / CPU metrics), an in-memory config store and a recording event
//! sink. No Tauri window, USB device or LibreHardwareMonitor is involved.
//!
//! Known limit: `ControlLoop` keeps its own `Instant`s (sensor staleness,
//! recovery backoff, PID/acoustic timing uses `poll_interval_ms`, not wall
//! time), and `RgbRenderer` reads `Instant::now()` for effect time. The fake
//! clock therefore governs the worker loop only; tests must not depend on
//! those internal timers.

#![allow(dead_code)]

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use corsair_common::config::{AppConfig, FanGroupConfig, FanMode, GeneralConfig, RgbConfig};
use corsair_common::Temperature;
use corsair_fancontrol::control_loop::{ControlLoop, HubParts};
use corsair_hid::{
    FanSpeed, FirmwareVersion, HubInfo, IcueLinkTransport, LinkDevice, LinkDeviceType, PsuStatus,
};
use corsair_rgb::renderer::RgbRenderer;
use corsair_sensors::TemperatureSource;

use crate::dto::{CpuMetricsDto, DeviceTree, GpuMetricsDto};
use crate::hardware_thread::{apply_rgb_config, StepOutcome, Worker};
use crate::state::HwCommand;
use crate::worker_env::{
    Clock, ConfigStore, DiagnosticsStore, EventSink, HubBackend, Telemetry, WorkerEnv,
};

// --- Fake clock ---------------------------------------------------------

struct ClockInner {
    base: Instant,
    offset: Mutex<Duration>,
    sleeps: Mutex<Vec<Duration>>,
}

/// Virtual monotonic clock. `sleep` advances it instantly and is recorded.
#[derive(Clone)]
pub struct FakeClock(Arc<ClockInner>);

impl FakeClock {
    pub fn new() -> Self {
        Self(Arc::new(ClockInner {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
            sleeps: Mutex::new(Vec::new()),
        }))
    }
    /// Virtual time elapsed since the clock was created.
    pub fn elapsed(&self) -> Duration {
        *self.0.offset.lock().unwrap()
    }
    /// Jump forward without recording a sleep.
    pub fn advance(&self, d: Duration) {
        *self.0.offset.lock().unwrap() += d;
    }
    pub fn sleeps(&self) -> Vec<Duration> {
        self.0.sleeps.lock().unwrap().clone()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.0.base + *self.0.offset.lock().unwrap()
    }
    fn sleep(&self, d: Duration) {
        self.0.sleeps.lock().unwrap().push(d);
        *self.0.offset.lock().unwrap() += d;
    }
    /// Virtual wall clock: 1_700_000_000 s since the Unix epoch
    /// (2023-11-14 22:13:20 UTC) plus the virtual elapsed time.
    fn wall_time(&self) -> std::time::SystemTime {
        std::time::UNIX_EPOCH
            + Duration::from_secs(1_700_000_000)
            + *self.0.offset.lock().unwrap()
    }
}

// --- Recording hub ------------------------------------------------------

/// Mock `IcueLinkTransport` that records every call it receives.
pub struct RecordingHub {
    pub speeds: Mutex<Vec<Vec<(u8, u8)>>>,
    pub rgb: Mutex<Vec<Vec<(u8, Vec<[u8; 3]>)>>>,
    pub hardware_mode_calls: AtomicU32,
    pub fail_set_speeds: AtomicBool,
}

impl RecordingHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            speeds: Mutex::new(Vec::new()),
            rgb: Mutex::new(Vec::new()),
            hardware_mode_calls: AtomicU32::new(0),
            fail_set_speeds: AtomicBool::new(false),
        })
    }
    pub fn set_fail(&self, fail: bool) {
        self.fail_set_speeds.store(fail, Ordering::SeqCst);
    }
    pub fn speed_calls(&self) -> Vec<Vec<(u8, u8)>> {
        self.speeds.lock().unwrap().clone()
    }
    pub fn rgb_calls(&self) -> Vec<Vec<(u8, Vec<[u8; 3]>)>> {
        self.rgb.lock().unwrap().clone()
    }
    pub fn hardware_mode(&self) -> u32 {
        self.hardware_mode_calls.load(Ordering::SeqCst)
    }
}

impl IcueLinkTransport for RecordingHub {
    fn set_speeds(&self, targets: &[(u8, u8)]) -> Result<()> {
        // Record the attempt even when failing: the worker still issued it.
        self.speeds.lock().unwrap().push(targets.to_vec());
        if self.fail_set_speeds.load(Ordering::SeqCst) {
            bail!("mock set_speeds failure");
        }
        Ok(())
    }
    fn get_speeds(&self) -> Result<Vec<FanSpeed>> {
        Ok(Vec::new())
    }
    fn set_rgb(&self, channel_leds: &[(u8, &[[u8; 3]])]) -> Result<()> {
        self.rgb
            .lock()
            .unwrap()
            .push(channel_leds.iter().map(|(c, l)| (*c, l.to_vec())).collect());
        Ok(())
    }
    fn enter_hardware_mode(&self) -> Result<()> {
        self.hardware_mode_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

// --- Fake sensor --------------------------------------------------------

/// Scripted temperature source. Keep the `Rc<Cell<f64>>` to change the value.
pub struct FakeSensor {
    name: String,
    temp: Rc<Cell<f64>>,
}

impl TemperatureSource for FakeSensor {
    fn name(&self) -> &str {
        &self.name
    }
    fn read(&self) -> Result<Temperature> {
        Ok(Temperature {
            celsius: self.temp.get(),
        })
    }
}

// --- Recording event sink ----------------------------------------------

#[derive(Clone, Default)]
pub struct RecordingSink(Arc<Mutex<Vec<(String, String)>>>);

impl RecordingSink {
    pub fn all(&self) -> Vec<(String, String)> {
        self.0.lock().unwrap().clone()
    }
    /// Raw JSON payloads of every emission of `event`, in order.
    pub fn payloads(&self, event: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _)| e == event)
            .map(|(_, p)| p.clone())
            .collect()
    }
    pub fn count(&self, event: &str) -> usize {
        self.payloads(event).len()
    }
}

impl EventSink for RecordingSink {
    fn emit_json(&self, event: &str, payload_json: String) {
        self.0
            .lock()
            .unwrap()
            .push((event.to_string(), payload_json));
    }
}

// --- Fake config store --------------------------------------------------

#[derive(Clone, Default)]
pub struct FakeStore {
    saves: Arc<Mutex<Vec<AppConfig>>>,
    attempts: Arc<AtomicU32>,
    fail: Arc<AtomicBool>,
}

impl FakeStore {
    pub fn set_fail(&self, fail: bool) {
        self.fail.store(fail, Ordering::SeqCst);
    }
    /// Successful saves, in order.
    pub fn saves(&self) -> Vec<AppConfig> {
        self.saves.lock().unwrap().clone()
    }
    /// Total `save` calls, successful or not.
    pub fn attempts(&self) -> u32 {
        self.attempts.load(Ordering::SeqCst)
    }
}

impl ConfigStore for FakeStore {
    fn save(&self, cfg: &AppConfig) -> Result<()> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            bail!("mock store failure");
        }
        self.saves.lock().unwrap().push(cfg.clone());
        Ok(())
    }
}

// --- Fake diagnostics store ---------------------------------------------

/// Records diagnostic exports instead of touching the filesystem.
#[derive(Clone, Default)]
pub struct FakeDiagnostics {
    files: Arc<Mutex<Vec<(String, String)>>>,
    fail: Arc<AtomicBool>,
}

impl FakeDiagnostics {
    pub fn set_fail(&self, fail: bool) {
        self.fail.store(fail, Ordering::SeqCst);
    }
    /// `(file_name, contents)` of every successful write, in order.
    pub fn files(&self) -> Vec<(String, String)> {
        self.files.lock().unwrap().clone()
    }
}

impl DiagnosticsStore for FakeDiagnostics {
    fn write_file(&self, file_name: &str, contents: &str) -> Result<std::path::PathBuf> {
        if self.fail.load(Ordering::SeqCst) {
            bail!("mock diagnostics failure");
        }
        self.files
            .lock()
            .unwrap()
            .push((file_name.to_string(), contents.to_string()));
        Ok(std::path::PathBuf::from("fake-diagnostics").join(file_name))
    }
}

// --- Fake telemetry -----------------------------------------------------

#[derive(Default)]
pub struct TelemetryScript {
    pub psu_lhm: Option<PsuStatus>,
    /// `None` = no HID handle; `Some(Err(msg))` = read failure.
    pub psu_hid: Option<std::result::Result<PsuStatus, String>>,
    pub gpu: Option<GpuMetricsDto>,
    pub cpu: Option<CpuMetricsDto>,
}

#[derive(Clone, Default)]
pub struct FakeTelemetry(pub Arc<Mutex<TelemetryScript>>);

impl Telemetry for FakeTelemetry {
    fn read_psu_lhm(&self) -> Option<PsuStatus> {
        self.0.lock().unwrap().psu_lhm.clone()
    }
    fn read_psu_hid(&self) -> Option<Result<PsuStatus>> {
        self.0
            .lock()
            .unwrap()
            .psu_hid
            .clone()
            .map(|r| r.map_err(|m| anyhow::anyhow!(m)))
    }
    fn read_gpu_metrics(&self) -> Option<GpuMetricsDto> {
        self.0.lock().unwrap().gpu.clone()
    }
    fn read_cpu_metrics(&self) -> Option<CpuMetricsDto> {
        self.0.lock().unwrap().cpu.clone()
    }
}

// --- Fake hub backend (recovery + device tree) --------------------------

#[derive(Default)]
pub struct BackendScript {
    pub reopen_calls: Vec<String>,
    /// Results handed out by successive `reopen` calls. Empty queue = Err.
    pub reopen_results: VecDeque<std::result::Result<(Arc<dyn IcueLinkTransport>, HubInfo), String>>,
}

#[derive(Clone, Default)]
pub struct FakeBackend(pub Arc<Mutex<BackendScript>>);

impl FakeBackend {
    pub fn reopen_calls(&self) -> Vec<String> {
        self.0.lock().unwrap().reopen_calls.clone()
    }
    pub fn push_reopen_ok(&self, hub: Arc<dyn IcueLinkTransport>, info: HubInfo) {
        self.0
            .lock()
            .unwrap()
            .reopen_results
            .push_back(Ok((hub, info)));
    }
    pub fn push_reopen_err(&self, msg: &str) {
        self.0
            .lock()
            .unwrap()
            .reopen_results
            .push_back(Err(msg.to_string()));
    }
}

impl HubBackend for FakeBackend {
    fn reopen(&mut self, serial: &str) -> Result<(Arc<dyn IcueLinkTransport>, HubInfo)> {
        let mut s = self.0.lock().unwrap();
        s.reopen_calls.push(serial.to_string());
        match s.reopen_results.pop_front() {
            Some(Ok(v)) => Ok(v),
            Some(Err(m)) => bail!(m),
            None => bail!("no scripted recovery"),
        }
    }
    fn device_tree(&self, control_loop: &ControlLoop) -> DeviceTree {
        DeviceTree::from_discovery(&[], &control_loop.hub_snapshots(), None)
    }
}

// --- Harness ------------------------------------------------------------

pub struct HubSpec {
    pub serial: &'static str,
    pub devices: Vec<LinkDevice>,
    pub led_counts: HashMap<u8, u16>,
    pub pump_channels: Vec<u8>,
}

pub fn device(channel: u8, id: &str, ty: LinkDeviceType) -> LinkDevice {
    LinkDevice {
        channel,
        device_type: ty,
        model: 0x01,
        device_id: id.to_string(),
    }
}

pub fn hub_info_for(spec: &HubSpec) -> HubInfo {
    HubInfo {
        firmware: FirmwareVersion {
            major: 0,
            minor: 0,
            patch: 0,
        },
        devices: spec.devices.clone(),
        led_counts: spec.led_counts.clone(),
    }
}

/// V1 config with one fixed-duty group on `hub_serial`.
pub fn v1_config(groups: Vec<FanGroupConfig>, rgb: RgbConfig, poll_ms: u64) -> AppConfig {
    AppConfig {
        schema_version: corsair_common::config::SCHEMA_VERSION_V1,
        general: GeneralConfig {
            poll_interval_ms: poll_ms,
            log_level: "info".to_string(),
            lhm_exe_path: None,
        },
        fan_groups: groups,
        rgb,
        device_overrides: Vec::new(),
        devices: Vec::new(),
    }
}

pub fn fixed_group(name: &str, hub: &str, channels: &[u8], duty: f64) -> FanGroupConfig {
    FanGroupConfig {
        name: name.to_string(),
        channels: channels.to_vec(),
        hub_serial: Some(hub.to_string()),
        device_ids: Vec::new(),
        mode: FanMode::Fixed { duty_percent: duty },
        preset_applied: None,
    }
}

pub struct Harness {
    pub worker: Worker,
    tx: Option<mpsc::Sender<HwCommand>>,
    rx: mpsc::Receiver<HwCommand>,
    pub clock: FakeClock,
    pub store: FakeStore,
    pub diagnostics: FakeDiagnostics,
    pub events: RecordingSink,
    pub telemetry: FakeTelemetry,
    pub backend: FakeBackend,
    pub hubs: HashMap<String, Arc<RecordingHub>>,
    pub temps: HashMap<String, Rc<Cell<f64>>>,
}

impl Harness {
    /// Build a worker over `hub_specs` (one `RecordingHub` each) and fake
    /// sensors `(name, initial celsius)`.
    ///
    /// The post-construction setup mirrors `hardware_thread::run_loop` after
    /// its hardware-bound startup: poll interval, RGB renderer + interval and
    /// the initial `apply_rgb_config`.
    pub fn new(config: AppConfig, hub_specs: Vec<HubSpec>, sensors: &[(&str, f64)]) -> Self {
        let clock = FakeClock::new();
        let mut hubs = HashMap::new();
        let mut parts = HashMap::new();
        for spec in &hub_specs {
            let hub = RecordingHub::new();
            hubs.insert(spec.serial.to_string(), hub.clone());
            parts.insert(
                spec.serial.to_string(),
                HubParts {
                    transport: hub as Arc<dyn IcueLinkTransport>,
                    pump_channels: spec.pump_channels.clone(),
                    devices: spec.devices.clone(),
                    led_counts: spec.led_counts.clone(),
                },
            );
        }
        let mut temps = HashMap::new();
        let mut sensor_map: HashMap<String, Box<dyn TemperatureSource>> = HashMap::new();
        for (name, t) in sensors {
            let cell = Rc::new(Cell::new(*t));
            temps.insert(name.to_string(), cell.clone());
            sensor_map.insert(
                name.to_string(),
                Box::new(FakeSensor {
                    name: name.to_string(),
                    temp: cell,
                }),
            );
        }
        let control_loop =
            ControlLoop::from_parts(config.clone(), parts, sensor_map).expect("build control loop");

        let poll_interval = Duration::from_millis(config.general.poll_interval_ms);
        let mut rgb_renderer = RgbRenderer::new();
        let rgb_enabled = config.rgb.enabled;
        let rgb_interval = Duration::from_millis(if config.rgb.fps > 0 {
            1000 / config.rgb.fps as u64
        } else {
            33
        });
        apply_rgb_config(
            &mut rgb_renderer,
            &config,
            &control_loop.device_type_map(),
            control_loop.registry(),
        );

        let store = FakeStore::default();
        let diagnostics = FakeDiagnostics::default();
        let events = RecordingSink::default();
        let telemetry = FakeTelemetry::default();
        let backend = FakeBackend::default();
        let worker = Worker {
            config,
            control_loop,
            poll_interval,
            rgb_renderer,
            rgb_enabled,
            rgb_interval,
            last_rgb_tick: clock.now(),
            last_temp: None,
            last_temp_time: None,
            last_good_psu: None,
            psu_fail_count: 0,
            rgb_identify: None,
            rgb_marker: None,
            env: WorkerEnv {
                events: Box::new(events.clone()),
                clock: Box::new(clock.clone()),
                store: Box::new(store.clone()),
                diagnostics: Box::new(diagnostics.clone()),
                telemetry: Box::new(telemetry.clone()),
                hubs: Box::new(backend.clone()),
            },
        };

        let (tx, rx) = mpsc::channel();
        Self {
            worker,
            tx: Some(tx),
            rx,
            clock,
            store,
            diagnostics,
            events,
            telemetry,
            backend,
            hubs,
            temps,
        }
    }

    pub fn hub(&self, serial: &str) -> &Arc<RecordingHub> {
        &self.hubs[serial]
    }

    pub fn set_temp(&self, sensor: &str, celsius: f64) {
        self.temps[sensor].set(celsius);
    }

    /// Run one outer loop iteration.
    pub fn step(&mut self) -> StepOutcome {
        self.worker.step(&self.rx)
    }

    pub fn steps(&mut self, n: usize) {
        for _ in 0..n {
            assert_eq!(self.step(), StepOutcome::Continue);
        }
    }

    /// Queue a command without stepping.
    pub fn send(&self, cmd: HwCommand) {
        self.tx.as_ref().expect("sender dropped").send(cmd).unwrap();
    }

    /// Send a command with a oneshot reply, run one step, return the reply.
    pub fn call<R>(
        &mut self,
        build: impl FnOnce(tokio::sync::oneshot::Sender<R>) -> HwCommand,
    ) -> (R, StepOutcome) {
        let (rtx, mut rrx) = tokio::sync::oneshot::channel();
        self.send(build(rtx));
        let outcome = self.step();
        let reply = rrx.try_recv().expect("command was not answered in this step");
        (reply, outcome)
    }

    /// Drop the only command sender so the next step sees `Disconnected`.
    pub fn disconnect(&mut self) {
        self.tx = None;
    }
}
