use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tauri::Emitter;
use tracing::{error, info, warn};

use corsair_common::config::AppConfig;
use corsair_common::config_migration::{
    try_migrate_file, MigrationError, MigrationOutcome,
};
use corsair_common::identity::DeviceRegistry;
use corsair_common::CorsairDevice;
use corsair_fancontrol::control_loop::{self, ControlLoop};
use corsair_hid::{port_power_factor, CorsairPsu, DeviceScanner, IcueLinkHub, LinkDeviceType};
use corsair_rgb::effect::EffectContext;
use corsair_rgb::layout::LedLayout;
use corsair_rgb::renderer::{DeviceConfig, RgbRenderer, ZoneConfig};

use crate::dto::{CpuMetricsDto, DeviceTree, GpuMetricsDto, RgbFrameDto, SystemSnapshot};
use crate::state::{ChannelIdentity, HwCommand};
use crate::worker_env::{
    emit, ConfigStore, DiskConfigStore, HubBackend, SystemClock, TauriEventSink, Telemetry,
    WorkerEnv,
};

/// Drop the cached PSU reading after this many consecutive failed reads.
const PSU_STALE_LIMIT: u32 = 10;

/// Distinct high-contrast colours used by the Channel Identify
/// diagnostic. Indexed modulo by an arbitrary `(hub_index, channel)`
/// slot so each physical device in the case shows a unique, easily-
/// describable hue. Pairs with [`identify_color_name`].
const IDENTIFY_PALETTE: &[([u8; 3], &str)] = &[
    ([255, 0, 0], "red"),
    ([255, 128, 0], "orange"),
    ([255, 255, 0], "yellow"),
    ([0, 255, 0], "green"),
    ([0, 255, 255], "cyan"),
    ([0, 128, 255], "blue"),
    ([128, 0, 255], "purple"),
    ([255, 0, 255], "magenta"),
    ([255, 0, 128], "pink"),
    ([255, 255, 255], "white"),
    ([128, 128, 0], "olive"),
    ([0, 128, 128], "teal"),
    ([128, 0, 0], "dark red"),
    ([0, 128, 0], "dark green"),
    ([0, 0, 128], "dark blue"),
    ([255, 128, 128], "pastel red"),
];

/// Spawn the dedicated hardware I/O thread. Returns a channel sender for commands.
///
/// All !Send hardware handles (HidDevice, WMI COM objects, NVML) live on this thread.
/// The Tauri async runtime communicates via mpsc channel + oneshot replies.
pub fn spawn(config: AppConfig, app: tauri::AppHandle) -> mpsc::Sender<HwCommand> {
    let (tx, rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("hardware-io".into())
        .spawn(move || {
            if let Err(e) = run_loop(config, rx, app) {
                error!("Hardware thread exited with error: {}", e);
            }
        })
        .expect("failed to spawn hardware-io thread");

    tx
}

fn run_loop(
    mut config: AppConfig,
    rx: mpsc::Receiver<HwCommand>,
    app: tauri::AppHandle,
) -> Result<()> {
    info!("Hardware thread starting");

    // Ensure LibreHardwareMonitor is running for accurate CPU temps
    let lhm_ready = corsair_sensors::lhm::ensure_lhm_running(
        config.general.lhm_exe_path.as_deref(),
    );
    if !lhm_ready {
        warn!("LibreHardwareMonitor not available — CPU temps will use fallback sensors");
    }

    // Initialize device scanner (stays alive for device queries and recovery)
    let scanner = match DeviceScanner::new() {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Failed to initialize HID API: {}", e);
            error!("{}", msg);
            let _ = app.emit("hw-error", &msg);
            // Run in degraded mode — can't do anything with hardware
            run_degraded(rx);
            return Ok(());
        }
    };

    // Build control loop
    let mut control_loop = match ControlLoop::build(config.clone(), Default::default(), &scanner) {
        Ok(cl) => cl,
        Err(e) => {
            let msg = format!("Failed to build control loop: {}", e);
            warn!("{}", msg);
            let _ = app.emit("hw-error", &msg);
            // Run in discovery-only mode
            run_discovery_only(rx, &scanner, &app);
            return Ok(());
        }
    };

    // V1→V2 config migration. Runs AFTER the control loop is built (so the
    // registry is populated with this boot's enumeration) and BEFORE the
    // main loop starts cycling (so the new config is authoritative for the
    // very first tick).
    //
    // All-or-nothing semantics: if any device referenced by the V1 config
    // isn't currently enumerated, migration aborts and we continue on the
    // V1 config for this boot. Migration retries on the next startup.
    //
    // On success we reload the written file from disk and apply it via
    // update_config() so the control loop sees the V2 shape without a
    // disk round-trip through save_config_to_disk.
    run_config_migration(&mut config, &mut control_loop, &app);

    let poll_interval = Duration::from_millis(config.general.poll_interval_ms);

    // RGB renderer — time-sliced alongside fan control
    let mut rgb_renderer = RgbRenderer::new();
    let rgb_enabled = config.rgb.enabled;
    let rgb_interval = Duration::from_millis(if config.rgb.fps > 0 {
        1000 / config.rgb.fps as u64
    } else {
        33
    });
    let last_rgb_tick = Instant::now();
    let last_temp: Option<f64> = None;
    let last_temp_time: Option<Instant> = None;
    apply_rgb_config(
        &mut rgb_renderer,
        &config,
        &control_loop.device_type_map(),
        control_loop.registry(),
    );

    info!(
        interval_ms = config.general.poll_interval_ms,
        "Hardware thread running"
    );

    // Check if LHM already has the PSU — avoids USB HID contention
    let lhm_has_psu = corsair_sensors::lhm::read_psu_from_lhm().is_some();
    if lhm_has_psu {
        info!("PSU data available via LibreHardwareMonitor — using LHM (no USB contention)");
    }

    // Only open our own HID handle if LHM doesn't have the PSU
    let psu_handle = if lhm_has_psu {
        None
    } else {
        let h = open_psu(&scanner);
        if h.is_some() {
            info!("PSU opened via direct HID");
        }
        h
    };

    // Cache last good PSU reading so transient failures don't flicker the UI
    let last_good_psu: Option<corsair_hid::PsuStatus> = None;
    let psu_fail_count: u32 = 0;

    // Channel-identify diagnostic state. When `Some((deadline, map))`,
    // the RGB tick overrides normal rendering with per-device distinct
    // colours from `map` (keyed by device_id). Automatically clears when
    // `Instant::now() >= deadline`, at which point the next tick
    // resumes the configured effect rendering.
    let rgb_identify: Option<(Instant, std::collections::HashMap<String, [u8; 3]>)> =
        None;

    // Separate NVML handle for extended GPU metrics (util, power, clock,
    // VRAM). The control loop already owns a GpuSensor for temperature
    // fan control — we keep an independent one here so those two concerns
    // don't leak into the fancontrol crate's API. NVML is refcounted by
    // the driver, so two handles in one process is legit.
    let gpu_metrics_sensor: Option<corsair_sensors::GpuSensor> =
        match corsair_sensors::GpuSensor::new() {
            Ok(g) => {
                info!("GPU metrics sensor ready for dashboard");
                Some(g)
            }
            Err(e) => {
                info!(error = %e, "GPU metrics sensor unavailable (non-NVIDIA machine or driver missing)");
                None
            }
        };

    let mut worker = Worker {
        config,
        control_loop,
        poll_interval,
        rgb_renderer,
        rgb_enabled,
        rgb_interval,
        last_rgb_tick,
        last_temp,
        last_temp_time,
        last_good_psu,
        psu_fail_count,
        rgb_identify,
        env: WorkerEnv {
            events: Box::new(TauriEventSink::new(app)),
            clock: Box::new(SystemClock),
            store: Box::new(DiskConfigStore),
            telemetry: Box::new(RealTelemetry {
                psu_handle,
                gpu_metrics_sensor,
            }),
            hubs: Box::new(RealHubBackend { scanner }),
        },
    };

    while worker.step(&rx) == StepOutcome::Continue {}
    Ok(())
}

/// Result of one [`Worker::step`] iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepOutcome {
    Continue,
    Exit,
}

/// State carried by the hardware loop after startup. `run_loop` builds one
/// and calls [`Worker::step`] until it returns [`StepOutcome::Exit`]; the
/// integration harness builds one over fake dependencies.
pub(crate) struct Worker {
    pub(crate) config: AppConfig,
    pub(crate) control_loop: ControlLoop,
    pub(crate) poll_interval: Duration,
    pub(crate) rgb_renderer: RgbRenderer,
    pub(crate) rgb_enabled: bool,
    pub(crate) rgb_interval: Duration,
    pub(crate) last_rgb_tick: Instant,
    pub(crate) last_temp: Option<f64>,
    pub(crate) last_temp_time: Option<Instant>,
    /// Cache last good PSU reading so transient failures don't flicker the UI
    pub(crate) last_good_psu: Option<corsair_hid::PsuStatus>,
    pub(crate) psu_fail_count: u32,
    /// Channel-identify diagnostic state. When `Some((deadline, map))`,
    /// the RGB tick overrides normal rendering with per-device distinct
    /// colours from `map` (keyed by device_id). Automatically clears when
    /// `clock.now() >= deadline`, at which point the next tick
    /// resumes the configured effect rendering.
    pub(crate) rgb_identify: Option<(Instant, std::collections::HashMap<String, [u8; 3]>)>,
    pub(crate) env: WorkerEnv,
}

impl Worker {
    /// One outer iteration of the hardware loop: drain pending commands,
    /// run one control cycle, emit the snapshot, render RGB if due, then
    /// sleep out the remainder of the cycle.
    pub(crate) fn step(&mut self, rx: &mpsc::Receiver<HwCommand>) -> StepOutcome {
        let cycle_start = self.env.clock.now();

        // Process all pending commands (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(cmd) => match cmd {
                    HwCommand::GetSnapshot { reply } => {
                        // Will be sent via event below, but also reply directly
                        let result = build_snapshot(
                            &mut self.control_loop,
                            &*self.env.telemetry,
                            &self.last_good_psu,
                        );
                        let _ = reply.send(result);
                        continue; // Don't tick, just respond
                    }
                    HwCommand::GetDevices { reply } => {
                        let tree = self.env.hubs.device_tree(&self.control_loop);
                        let _ = reply.send(Ok(tree));
                    }
                    HwCommand::GetConfig { reply } => {
                        let _ = reply.send(Ok(self.control_loop.config().clone()));
                    }
                    HwCommand::ListSensors { reply } => {
                        let _ = reply.send(Ok(self.control_loop.available_sensors()));
                    }
                    HwCommand::UpdateConfig {
                        config: mut new_config,
                        reply,
                    } => {
                        // Validate first
                        if let Err(e) = control_loop::validate_config(&new_config) {
                            let _ = reply.send(Err(format!("Invalid config: {}", e)));
                            continue;
                        }
                        // Reconcile preset_applied: if a group's mode has
                        // drifted from its named preset's canonical values,
                        // clear the tag so the UI indicator goes to
                        // Modified / Off on next read.
                        reconcile_preset_applied(&mut new_config);
                        // Persist to disk
                        if let Err(e) = self.env.store.save(&new_config) {
                            let _ = reply.send(Err(format!("Failed to save config: {}", e)));
                            continue;
                        }
                        // Apply live
                        match self.control_loop.update_config(new_config.clone()) {
                            Ok(()) => {
                                self.config = new_config;
                                self.poll_interval =
                                    Duration::from_millis(self.config.general.poll_interval_ms);
                                let _ = reply.send(Ok(()));
                            }
                            Err(e) => {
                                let _ = reply
                                    .send(Err(format!("Failed to apply config: {}", e)));
                            }
                        }
                    }
                    HwCommand::ApplyPreset {
                        preset,
                        group_name,
                        reply,
                    } => {
                        let result = apply_preset(
                            &preset,
                            group_name.as_deref(),
                            &mut self.config,
                            &mut self.control_loop,
                            &*self.env.store,
                        );
                        let ok = result.is_ok();
                        let _ = reply.send(result);
                        if ok {
                            self.poll_interval =
                                Duration::from_millis(self.config.general.poll_interval_ms);
                        }
                    }
                    HwCommand::SetManualDuty {
                        hub_serial,
                        channels,
                        duty,
                        reply,
                    } => {
                        let result = self.control_loop
                            .set_manual_duty(&hub_serial, &channels, duty)
                            .map_err(|e| format!("Failed to set duty: {}", e));
                        let _ = reply.send(result);
                    }
                    HwCommand::SetManualDutyByDeviceId {
                        device_ids,
                        duty,
                        reply,
                    } => {
                        let result = apply_manual_duty_by_device_id(
                            &self.control_loop,
                            &device_ids,
                            duty,
                        );
                        let _ = reply.send(result);
                    }
                    HwCommand::RenameDevice {
                        device_id,
                        name,
                        reply,
                    } => {
                        let result = apply_rename_device(
                            &mut self.config,
                            &mut self.control_loop,
                            &device_id,
                            &name,
                            &*self.env.store,
                        );
                        let _ = reply.send(result);
                    }
                    HwCommand::SetRgbConfig {
                        config: rgb_config,
                        reply,
                    } => {
                        self.config.rgb = rgb_config;
                        self.rgb_enabled = self.config.rgb.enabled;
                        self.rgb_interval = Duration::from_millis(if self.config.rgb.fps > 0 {
                            1000 / self.config.rgb.fps as u64
                        } else {
                            33
                        });
                        apply_rgb_config(
                            &mut self.rgb_renderer,
                            &self.config,
                            &self.control_loop.device_type_map(),
                            self.control_loop.registry(),
                        );
                        if let Err(e) = self.env.store.save(&self.config) {
                            warn!("Failed to save RGB config: {}", e);
                        }
                        let _ = reply.send(Ok(()));
                    }
                    HwCommand::SetRgbEnabled { enabled, reply } => {
                        self.rgb_enabled = enabled;
                        self.config.rgb.enabled = enabled;
                        if let Err(e) = self.env.store.save(&self.config) {
                            warn!("Failed to save RGB enabled state: {}", e);
                        }
                        let _ = reply.send(Ok(()));
                    }
                    HwCommand::RgbIdentifyChannels {
                        duration_secs,
                        reply,
                    } => {
                        // Build a deterministic (hub_index, channel) →
                        // palette-index mapping so every enumerated device
                        // receives a unique colour. Key the runtime
                        // override by device_id so the colour follows the
                        // device even if the registry re-resolves
                        // mid-test. Also return the mapping so the UI
                        // can render a legend.
                        let hub_snaps = self.control_loop.hub_snapshots();
                        let mut identity: Vec<ChannelIdentity> = Vec::new();
                        let mut id_to_rgb: std::collections::HashMap<String, [u8; 3]> =
                            std::collections::HashMap::new();
                        // Keep hubs in enumeration order so the
                        // colour indices are stable across runs.
                        for (hub_idx, (serial, info, _speeds)) in hub_snaps.iter().enumerate() {
                            for dev in &info.devices {
                                // Offset by hub index × 8 so two
                                // different hubs cycle through the
                                // palette starting at different
                                // colours — the same ch1 on two hubs
                                // shouldn't both be red.
                                let slot = (hub_idx * 8 + (dev.channel as usize))
                                    % IDENTIFY_PALETTE.len();
                                let (rgb, name) = IDENTIFY_PALETTE[slot];
                                id_to_rgb.insert(dev.device_id.clone(), rgb);
                                identity.push(ChannelIdentity {
                                    hub_serial: serial.clone(),
                                    channel: dev.channel,
                                    device_id: dev.device_id.clone(),
                                    device_type: dev.device_type.name().to_string(),
                                    color_name: name.to_string(),
                                    rgb,
                                });
                            }
                        }
                        let clamped = duration_secs.clamp(3, 120);
                        self.rgb_identify = Some((
                            self.env.clock.now() + Duration::from_secs(clamped),
                            id_to_rgb,
                        ));
                        info!(
                            duration_secs = clamped,
                            devices = identity.len(),
                            "RGB channel-identify mode active"
                        );
                        let _ = reply.send(Ok(identity));
                    }
                    HwCommand::Shutdown => {
                        info!("Hardware thread received shutdown command");
                        self.control_loop.shutdown_hardware();
                        return StepOutcome::Exit;
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    info!("Hardware command channel closed — shutting down");
                    self.control_loop.shutdown_hardware();
                    return StepOutcome::Exit;
                }
            }
        }

        // Run one control cycle
        let cycle_result = self.control_loop.tick();

        // Emit hub-lost for any hub that just became unhealthy (first failure)
        for report in &cycle_result.hub_health {
            if report.consecutive_failures == 1 {
                warn!(serial = report.serial.as_str(), "Hub lost — will attempt recovery");
                emit(&*self.env.events, "hub-lost", &report.serial);
            }
        }

        // Attempt recovery for hubs with sustained failures
        let needing_recovery = self.control_loop.hubs_needing_recovery();
        for serial in &needing_recovery {
            info!(serial = serial.as_str(), "Attempting hub recovery");
            let recovered = self.env.hubs.reopen(serial);

            match recovered {
                Ok((hub, info)) => {
                    info!(serial = serial.as_str(), "Hub recovery succeeded");
                    self.control_loop.replace_hub_transport(serial, hub, info);
                    emit(&*self.env.events, "hub-recovered", &serial);
                }
                Err(e) => {
                    warn!(serial = serial.as_str(), error = %e, "Hub recovery failed");
                    self.control_loop.mark_recovery_attempted(serial);
                }
            }
        }

        // Read PSU status: LHM first (no USB contention), then direct HID fallback
        let psu_status = self
            .env
            .telemetry
            .read_psu_lhm()
            .and_then(validate_psu_status)
            .or_else(|| {
                self.env.telemetry.read_psu_hid().and_then(|read| {
                    match read {
                        Ok(raw) => validate_psu_status(raw),
                        Err(e) => {
                            if self.psu_fail_count <= 3 || self.psu_fail_count % 10 == 0 {
                                warn!("PSU HID read error (fail #{}): {}", self.psu_fail_count + 1, e);
                            }
                            None
                        }
                    }
                })
            });

        // Update cache or fall back to last good reading
        let psu_status = match psu_status {
            Some(valid) => {
                if self.psu_fail_count > 0 {
                    info!("PSU read recovered after {} failures", self.psu_fail_count);
                }
                self.psu_fail_count = 0;
                self.last_good_psu = Some(valid.clone());
                Some(valid)
            }
            None => {
                self.psu_fail_count += 1;
                if self.psu_fail_count <= PSU_STALE_LIMIT {
                    self.last_good_psu.clone()
                } else {
                    None
                }
            }
        };

        // Build and emit snapshot. Pass a fresh device_id map so the DTO
        // carries stable identity alongside (hub, channel) during the V1→V2
        // transition.
        let device_id_map = self.control_loop.device_id_map();
        let gpu_metrics = self.env.telemetry.read_gpu_metrics();
        // CPU metrics come from LHM — same HTTP tree the PSU path
        // already touches. Only populated when LHM is running; on
        // fallback backends (root\WMI, CIMV2) this stays `None` and
        // the dashboard hides the CPU extras strip.
        let cpu_metrics = self.env.telemetry.read_cpu_metrics();
        let snapshot = SystemSnapshot::from_cycle(
            &cycle_result,
            &cycle_result.fan_speeds,
            psu_status.as_ref(),
            &device_id_map,
            gpu_metrics,
            cpu_metrics,
        );
        emit(&*self.env.events, "sensor-update", &snapshot);

        // RGB render tick (time-sliced at configured FPS)
        if self.rgb_enabled && self.env.clock.now().saturating_duration_since(self.last_rgb_tick) >= self.rgb_interval {
            // Build EffectContext from latest sensor data
            let primary_temp = snapshot
                .temperatures
                .first()
                .map(|t| t.celsius);

            // Compute temp delta (°C/sec)
            let temp_delta = primary_temp.and_then(|current| {
                let now = self.env.clock.now();
                let delta = self.last_temp.map(|prev| {
                    let dt = self.last_temp_time
                        .map(|t| now.duration_since(t).as_secs_f64())
                        .unwrap_or(1.0)
                        .max(0.01);
                    (current - prev) / dt
                });
                self.last_temp = Some(current);
                self.last_temp_time = Some(now);
                delta
            });

            let avg_duty = if snapshot.group_duties.is_empty() {
                None
            } else {
                Some(
                    snapshot.group_duties.iter().map(|g| g.duty_percent as f64).sum::<f64>()
                        / snapshot.group_duties.len() as f64,
                )
            };

            let effect_ctx = EffectContext {
                temperature: primary_temp,
                temp_delta,
                duty_percent: avg_duty,
                gpu_utilization: snapshot
                    .gpu_metrics
                    .as_ref()
                    .map(|g| g.utilization_pct as f64),
                all_temps: snapshot
                    .temperatures
                    .iter()
                    .map(|t| (t.source.clone(), t.celsius))
                    .collect(),
            };

            let mut frames = self.rgb_renderer.tick(&effect_ctx);

            // Channel-identify override: if active and not expired,
            // replace every LED colour in every frame with the
            // device's assigned identify colour. This bypasses the
            // effect output but still uses the real renderer's frame
            // shapes (device_id, led count) so the hub buffer layout
            // is identical to production rendering — the only
            // variable is colour-per-device.
            let expired = self.rgb_identify
                .as_ref()
                .map(|(deadline, _)| self.env.clock.now() >= *deadline)
                .unwrap_or(false);
            if expired {
                info!("RGB channel-identify mode expired — resuming normal rendering");
                self.rgb_identify = None;
            }
            if let Some((_, id_to_rgb)) = &self.rgb_identify {
                for frame in frames.iter_mut() {
                    let rgb = id_to_rgb
                        .get(&frame.device_id)
                        .copied()
                        .unwrap_or([32, 32, 32]); // unknown device → dim grey
                    for led in frame.leds.iter_mut() {
                        led.r = rgb[0];
                        led.g = rgb[1];
                        led.b = rgb[2];
                    }
                }
            }

            // Resolve (hub_serial, channel) via the current registry so the
            // DTO carries a self-consistent snapshot for the preview UI
            // during the V1→V2 transition.
            let registry_snapshot = self.control_loop.registry();
            let frame_dtos: Vec<RgbFrameDto> = frames
                .iter()
                .map(|f| RgbFrameDto::from_frame(f, registry_snapshot))
                .collect();
            emit(&*self.env.events, "rgb-frame", &frame_dtos);

            // Send to hardware if enabled
            if self.config.rgb.hardware_output && !frames.is_empty() {
                let hw_start = self.env.clock.now();

                // Compute port power factor from total LEDs across all frames
                let total_leds: u16 = frames.iter().map(|f| f.leds.len() as u16).sum();
                let power_factor = port_power_factor(total_leds);

                // Convert frames to hardware format with power protection applied
                let hw_leds: Vec<Vec<[u8; 3]>> = frames
                    .iter()
                    .map(|f| {
                        f.leds
                            .iter()
                            .map(|c| {
                                if power_factor < 1.0 {
                                    [
                                        (c.r as f32 * power_factor).round() as u8,
                                        (c.g as f32 * power_factor).round() as u8,
                                        (c.b as f32 * power_factor).round() as u8,
                                    ]
                                } else {
                                    [c.r, c.g, c.b]
                                }
                            })
                            .collect()
                    })
                    .collect();

                // Post-Step-5: send_rgb_frames takes (device_id, leds) only.
                // The renderer already keys frames by device_id; the control
                // loop resolves to (hub_serial, channel) via the registry
                // immediately before the wire write.
                let frame_refs: Vec<(String, &[[u8; 3]])> = frames
                    .iter()
                    .zip(hw_leds.iter())
                    .map(|(f, leds)| (f.device_id.clone(), leds.as_slice()))
                    .collect();

                let sent = self.control_loop.send_rgb_frames(&frame_refs);
                let hw_elapsed = self.env.clock.now().saturating_duration_since(hw_start);
                if hw_elapsed > Duration::from_millis(25) {
                    warn!(
                        elapsed_ms = hw_elapsed.as_millis(),
                        sent,
                        "RGB hardware write slow"
                    );
                }
            }

            self.last_rgb_tick = self.env.clock.now();
        }

        // Sleep for remaining poll interval (account for RGB timing)
        let elapsed = self.env.clock.now().saturating_duration_since(cycle_start);
        let sleep_target = if self.rgb_enabled {
            self.poll_interval.min(self.rgb_interval)
        } else {
            self.poll_interval
        };
        if elapsed < sleep_target {
            self.env.clock.sleep(sleep_target - elapsed);
        }
        StepOutcome::Continue
    }
}

/// Production telemetry: LHM HTTP reads, the optional direct PSU HID handle
/// and the dashboard's NVML metrics handle.
struct RealTelemetry {
    psu_handle: Option<CorsairPsu>,
    gpu_metrics_sensor: Option<corsair_sensors::GpuSensor>,
}

impl Telemetry for RealTelemetry {
    fn read_psu_lhm(&self) -> Option<corsair_hid::PsuStatus> {
        corsair_sensors::lhm::read_psu_from_lhm()
    }

    fn read_psu_hid(&self) -> Option<Result<corsair_hid::PsuStatus>> {
        self.psu_handle.as_ref().map(|psu| psu.read_all())
    }

    fn read_gpu_metrics(&self) -> Option<GpuMetricsDto> {
        self.gpu_metrics_sensor
            .as_ref()
            .and_then(|s| s.read_metrics().ok())
            .map(|m| GpuMetricsDto {
                utilization_pct: m.utilization_pct,
                power_watts: m.power_watts,
                clock_mhz: m.clock_mhz,
                memory_used_mb: m.memory_used_mb,
                temp_celsius: m.temp_celsius,
            })
    }

    fn read_cpu_metrics(&self) -> Option<CpuMetricsDto> {
        corsair_sensors::lhm::read_cpu_metrics_from_lhm().map(|m| CpuMetricsDto {
            load_pct: m.load_pct,
            power_watts: m.power_watts,
            clock_mhz: m.clock_mhz,
        })
    }
}

/// Production hub backend: the scanner kept alive from startup.
struct RealHubBackend {
    scanner: DeviceScanner,
}

impl HubBackend for RealHubBackend {
    fn reopen(
        &mut self,
        serial: &str,
    ) -> Result<(std::sync::Arc<dyn corsair_hid::IcueLinkTransport>, corsair_hid::HubInfo)> {
        let serial = serial.to_string();
        let scanner = &mut self.scanner;
        scanner
            .refresh()
            .and_then(|()| {
                scanner.open_device(
                    CorsairDevice::IcueLinkHub.pid(),
                    &serial,
                    IcueLinkHub::data_interface(),
                )
            })
            .and_then(|dev| {
                let hub = IcueLinkHub::new(dev, serial.clone());
                let info = hub.initialize()?;
                Ok((std::sync::Arc::new(hub) as std::sync::Arc<dyn corsair_hid::IcueLinkTransport>, info))
            })
    }

    fn device_tree(&self, control_loop: &ControlLoop) -> DeviceTree {
        build_device_tree_from_loop(control_loop, &self.scanner)
    }
}

/// Degraded mode: no hardware access at all, just drain commands.
fn run_degraded(rx: mpsc::Receiver<HwCommand>) {
    loop {
        match rx.recv() {
            Ok(HwCommand::Shutdown) | Err(_) => return,
            Ok(HwCommand::GetSnapshot { reply }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::GetDevices { reply }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::GetConfig { reply }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::ListSensors { reply }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::UpdateConfig { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::ApplyPreset { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::SetManualDuty { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::SetManualDutyByDeviceId { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::RenameDevice { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::SetRgbConfig { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::SetRgbEnabled { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::RgbIdentifyChannels { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
        }
    }
}

/// Discovery-only mode: can enumerate devices but no control loop.
fn run_discovery_only(
    rx: mpsc::Receiver<HwCommand>,
    scanner: &DeviceScanner,
    _app: &tauri::AppHandle,
) {
    loop {
        match rx.recv() {
            Ok(HwCommand::Shutdown) | Err(_) => return,
            Ok(HwCommand::GetDevices { reply }) => {
                let tree = build_device_tree(scanner);
                let _ = reply.send(Ok(tree));
            }
            Ok(HwCommand::GetSnapshot { reply }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::GetConfig { reply }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::ListSensors { reply }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::UpdateConfig { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::ApplyPreset { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::SetManualDuty { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::SetManualDutyByDeviceId { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::RenameDevice { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::SetRgbConfig { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::SetRgbEnabled { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::RgbIdentifyChannels { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
        }
    }
}

fn build_snapshot(
    control_loop: &mut ControlLoop,
    telemetry: &dyn Telemetry,
    last_good_psu: &Option<corsair_hid::PsuStatus>,
) -> Result<SystemSnapshot, String> {
    let cycle_result = control_loop.tick();
    let psu_status = telemetry
        .read_psu_lhm()
        .and_then(validate_psu_status)
        .or_else(|| {
            telemetry
                .read_psu_hid()
                .and_then(|read| read.ok().and_then(validate_psu_status))
        })
        .or_else(|| last_good_psu.clone());
    let device_id_map = control_loop.device_id_map();
    // GetSnapshot is a rare on-demand path (used for reply-direct calls
    // from Tauri commands), so we skip the NVML + CPU-metrics reads
    // here rather than carrying another `&GpuSensor` borrow — the
    // frontend's live stream is driven by the main-loop event.
    Ok(SystemSnapshot::from_cycle(
        &cycle_result,
        &cycle_result.fan_speeds,
        psu_status.as_ref(),
        &device_id_map,
        None,
        None,
    ))
}

/// Standalone device tree builder for discovery-only mode (no control loop running).
fn build_device_tree(scanner: &DeviceScanner) -> DeviceTree {
    let groups = scanner.scan_grouped();
    let mut hub_infos = Vec::new();
    let mut psu_group = None;

    for group in &groups {
        match group.device_type {
            CorsairDevice::IcueLinkHub => {
                if let Ok(dev) = scanner.open_device(
                    group.pid,
                    &group.serial,
                    IcueLinkHub::data_interface(),
                ) {
                    let hub = IcueLinkHub::new(dev, group.serial.clone());
                    if let Ok(info) = hub.initialize() {
                        let speeds = hub.get_speeds().unwrap_or_default();
                        hub_infos.push((group.serial.clone(), info, speeds));
                        let _ = hub.enter_hardware_mode();
                    }
                }
            }
            CorsairDevice::Hx1500i => {
                psu_group = Some(group);
            }
            _ => {}
        }
    }

    DeviceTree::from_discovery(&groups, &hub_infos, psu_group)
}

/// Build device tree using the control loop's cached hub info — no competing USB handles.
fn build_device_tree_from_loop(control_loop: &ControlLoop, scanner: &DeviceScanner) -> DeviceTree {
    let groups = scanner.scan_grouped();
    let hub_infos = control_loop.hub_snapshots();
    let psu_group = groups.iter().find(|g| g.device_type == CorsairDevice::Hx1500i);
    DeviceTree::from_discovery(&groups, &hub_infos, psu_group)
}

/// Open and initialize the PSU once. Returns a persistent handle.
fn open_psu(scanner: &DeviceScanner) -> Option<CorsairPsu> {
    let groups = scanner.scan_grouped();
    for group in &groups {
        if group.device_type == CorsairDevice::Hx1500i {
            if let Ok(dev) = scanner.open_device(
                group.pid,
                &group.serial,
                CorsairPsu::data_interface(),
            ) {
                let psu = CorsairPsu::new(dev, group.serial.clone());
                match psu.initialize() {
                    Ok(()) => return Some(psu),
                    Err(e) => {
                        warn!("PSU init failed: {}", e);
                    }
                }
            }
        }
    }
    None
}

/// Sanity-check PSU values — reject obviously wrong readings.
fn validate_psu_status(s: corsair_hid::PsuStatus) -> Option<corsair_hid::PsuStatus> {
    macro_rules! check {
        ($val:expr, $lo:expr, $hi:expr, $name:expr) => {
            if $val < $lo || $val > $hi {
                warn!(
                    "PSU validation failed: {} = {:.2} (expected {}..{})",
                    $name, $val, $lo, $hi
                );
                return None;
            }
        };
    }
    // Temperature: -10 to 150°C is physically plausible
    check!(s.temp_vrm, -10.0, 150.0, "temp_vrm");
    check!(s.temp_case, -10.0, 150.0, "temp_case");
    // Voltage: AC input 80-300V, rails with ATX tolerance
    check!(s.input_voltage, 0.0, 300.0, "input_voltage");
    check!(s.rail_12v.voltage, 0.0, 15.0, "12V rail");
    check!(s.rail_5v.voltage, 0.0, 7.0, "5V rail");
    check!(s.rail_3v3.voltage, 0.0, 5.0, "3.3V rail");
    // Power: 0-2000W total
    check!(s.total_power, 0.0, 2000.0, "total_power");
    Some(s)
}

/// Apply a curated fan preset to one or all fan groups.
///
/// Preset definitions live in the compile-time-embedded
/// `assets/presets/fans.toml` asset and are looked up via
/// `crate::presets::lookup`.
///
/// `group_name` is the per-group filter added in v0.3:
/// - `None` — apply to every group (pre-v0.3 global behavior, still used
///   by the tray menu and the `apply_preset` back-compat command).
/// - `Some(name)` — apply to that group only. Unknown names are a no-op
///   (the call returns `Ok(())`) — the UI should only ever call with a
///   name that came from `get_devices` / `get_config`, so a missing
///   name means the user just deleted the group and the click raced.
///
/// On success every targeted group's `preset_applied` is set to the
/// preset's id — the UI reads this back in `get_config` to render the
/// three-state (Active / Modified / Off) indicator.
fn apply_preset(
    preset: &str,
    group_name: Option<&str>,
    config: &mut AppConfig,
    control_loop: &mut ControlLoop,
    store: &dyn ConfigStore,
) -> Result<(), String> {
    use corsair_common::config::{FanMode, TempSourceConfig};

    let def = crate::presets::lookup(preset)
        .ok_or_else(|| format!("Unknown preset: {}", preset))?;

    let available = control_loop.available_sensors();

    // Build a TempSourceConfig from the preset's per-sensor weights,
    // dropping any sensor not currently available. `water_inline` is a
    // v0.4 placeholder — when it isn't enumerated we gracefully degrade
    // to CPU/GPU weighting (if *those* are also missing we fall through
    // to the fixed-duty fallback below).
    let mut sensors = Vec::new();
    let mut weights = Vec::new();
    for (name, weight) in [
        ("cpu", def.cpu_weight),
        ("gpu", def.gpu_weight),
        ("water_inline", def.water_weight),
    ] {
        if weight > 0.0 && available.contains(&name.to_string()) {
            sensors.push(name.to_string());
            weights.push(weight);
        }
    }

    // water_priority preview: if the user picked it but no water sensor
    // is present, fall back to whatever cpu/gpu split was authored on
    // the *same* preset or, if the preset is water-only, a CPU 0.6 /
    // GPU 0.4 default that matches the pre-v0.3 behavior.
    if sensors.is_empty() {
        if available.contains(&"cpu".to_string())
            && available.contains(&"gpu".to_string())
        {
            sensors = vec!["cpu".to_string(), "gpu".to_string()];
            weights = vec![0.6, 0.4];
        } else if available.contains(&"cpu".to_string()) {
            sensors = vec!["cpu".to_string()];
            weights = vec![1.0];
        } else if available.contains(&"gpu".to_string()) {
            sensors = vec!["gpu".to_string()];
            weights = vec![1.0];
        }
    }

    let mode = if sensors.is_empty() {
        // No sensors available at all — fall back to a fixed-duty that
        // picks a sensible mid-curve duty, not 0. The mid-point of the
        // preset's curve is a reasonable proxy.
        let mid_duty = def
            .points
            .get(def.points.len() / 2)
            .map(|p| p.duty)
            .unwrap_or(50.0);
        FanMode::Fixed { duty_percent: mid_duty }
    } else {
        FanMode::Curve {
            points: def.points.clone(),
            hysteresis: def.hysteresis,
            ramp_rate: def.ramp_up,
            ramp_down_rate: Some(def.ramp_down),
            temp_source: TempSourceConfig { sensors, weights },
        }
    };

    // Apply to the target group(s).
    let mut applied_any = false;
    for group in &mut config.fan_groups {
        if group_name.map_or(true, |n| group.name == n) {
            group.mode = mode.clone();
            group.preset_applied = Some(def.id.clone());
            applied_any = true;
        }
    }

    if !applied_any {
        // Harmless no-op — the UI raced against a delete. Return Ok so
        // the user isn't surprised by an error toast.
        return Ok(());
    }

    // Persist to disk so it survives restart
    if let Err(e) = store.save(config) {
        warn!("Failed to save preset config: {}", e);
    }

    control_loop
        .update_config(config.clone())
        .map_err(|e| format!("Failed to apply preset: {}", e))
}

/// Convert the RGB config into renderer zone configs and apply.
///
/// Post-Step-5 (PR2): every `DeviceConfig` carries a non-empty `device_id`.
/// Source of truth depends on the config schema:
///   - **V2** (`config.is_v2()`): `RgbDeviceRef.device_id` is authoritative —
///     we use it directly. Layout resolution then tries to find the device
///     by device_id in the registry; if the device isn't currently
///     enumerated we emit a warning and skip it (orphan — the config
///     reference is preserved, but this tick's renderer won't include it).
///   - **V1** (`!config.is_v2()`): the `RgbDeviceRef` carries
///     `(hub_serial, channel)` only. We resolve that pair to a device_id
///     via the live registry. If the lookup fails (device not enumerated
///     this boot), we warn and skip — consistent with the V2 path above.
///
/// Skipping is the correct behavior for unresolved references: the
/// renderer's Step 5 invariant is "every DeviceTarget has a device_id", and
/// letting an empty-device_id DeviceTarget through would trip the `expect`
/// in `RgbRenderer::tick`. The user-visible effect of skipping is the same
/// as the pre-refactor "orphan" behavior in the control loop — that zone
/// just doesn't drive the missing fan this tick, and rejoins
/// automatically when the device reappears and the registry is rebuilt.
fn apply_rgb_config(
    renderer: &mut RgbRenderer,
    config: &AppConfig,
    device_types: &std::collections::HashMap<(String, u8), (LinkDeviceType, u16)>,
    registry: &DeviceRegistry,
) {
    let is_v2 = config.is_v2();
    let zones: Vec<ZoneConfig> = config
        .rgb
        .zones
        .iter()
        .map(|z| {
            let devices = z
                .devices
                .iter()
                .filter_map(|d| {
                    // Resolve device_id: V2 uses the config field directly,
                    // V1 resolves (hub_serial, channel) via the registry.
                    let device_id = if is_v2 && !d.device_id.is_empty() {
                        d.device_id.clone()
                    } else {
                        match registry.device_id_at(&d.hub_serial, d.channel) {
                            Some(id) => id.to_string(),
                            None => {
                                warn!(
                                    hub_serial = d.hub_serial.as_str(),
                                    channel = d.channel,
                                    zone = z.name.as_str(),
                                    "RGB zone references device not currently enumerated — \
                                     skipping in renderer (will rejoin on next enumeration)"
                                );
                                return None;
                            }
                        }
                    };

                    // Layout: prefer registry-resolved (hub, channel) for V2,
                    // or the V1 config's literal (hub, channel). Either way we
                    // look up the device_type_map to pick the right LED layout.
                    let (lookup_hub, lookup_ch) = if is_v2 {
                        match registry.channel_for(&device_id) {
                            Some(hc) => hc,
                            None => {
                                // V2 reference to a device not currently
                                // enumerated — skip (same orphan semantics
                                // as above, different source path).
                                warn!(
                                    device_id = device_id.as_str(),
                                    zone = z.name.as_str(),
                                    "V2 RGB zone references device_id not currently \
                                     enumerated — skipping in renderer"
                                );
                                return None;
                            }
                        }
                    } else {
                        (d.hub_serial.clone(), d.channel)
                    };

                    let layout = match device_types.get(&(lookup_hub, lookup_ch)) {
                        Some((LinkDeviceType::LinkAdapter | LinkDeviceType::LsStrip, led_count)) => {
                            LedLayout::LinearStrip { led_count: *led_count }
                        }
                        Some((_, led_count)) if *led_count > 0 => {
                            LedLayout::FanRing { led_count: *led_count }
                        }
                        _ => LedLayout::qx_fan(), // safe fallback
                    };

                    Some(DeviceConfig { device_id, layout })
                })
                .collect();

            let layers = z
                .layers
                .iter()
                .map(|l| corsair_rgb::Layer {
                    effect_config: l.effect.clone(),
                    blend_mode: l.blend_mode,
                    opacity: l.opacity,
                    enabled: l.enabled,
                })
                .collect();

            ZoneConfig {
                name: z.name.clone(),
                devices,
                layers,
                brightness: z.brightness,
                flow: z.flow.clone(),
            }
        })
        .collect();

    renderer.update_config(&zones, config.rgb.brightness as f32 / 100.0);
}

/// Resolve each device_id via the control loop's registry, bucket by hub,
/// and issue one `set_speeds` per hub. Returns a structured result listing
/// which ids were applied vs unresolved so the UI can surface partial-
/// success cases (e.g. a fan was unplugged between enumeration and the
/// user's click).
///
/// ## Why we group per hub
///
/// `ControlLoop::set_manual_duty(hub, channels, duty)` takes a slice of
/// channels per hub — one hub call per group. A V2 fan selection can
/// span multiple hubs, so we bucket before issuing any hub calls and
/// treat the per-hub call as atomic (either all its channels are
/// commanded or none — a mid-hub failure stops further channels on
/// that hub but doesn't roll back earlier hubs). Errors from a
/// per-hub call are propagated as a String error so the command
/// Result captures them.
fn apply_manual_duty_by_device_id(
    control_loop: &ControlLoop,
    device_ids: &[String],
    duty: u8,
) -> Result<crate::dto::ManualDutyResult, String> {
    if duty > 100 {
        return Err("duty must be 0-100".into());
    }
    let registry = control_loop.registry();
    let mut per_hub: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();
    let mut applied: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    for id in device_ids {
        match registry.channel_for(id) {
            Some((hub, ch)) => {
                per_hub.entry(hub).or_default().push(ch);
                applied.push(id.clone());
            }
            None => unresolved.push(id.clone()),
        }
    }
    for (hub, channels) in &per_hub {
        control_loop
            .set_manual_duty(hub, channels, duty)
            .map_err(|e| format!("Failed to set duty on hub '{}': {}", hub, e))?;
    }
    Ok(crate::dto::ManualDutyResult {
        applied,
        unresolved,
    })
}

/// Mutate `config.devices` to set a friendly name for `device_id`:
/// - An empty `name` clears the device entry (revert to system default).
/// - A non-empty `name` upserts — existing entry's `name` is updated;
///   absent entry gets inserted. `led_count` on an existing entry is
///   preserved.
///
/// Pure mutation. The caller is responsible for persisting and for
/// informing the control loop via `update_config`. Exposed as
/// `pub(crate)` for unit testing without Tauri state.
pub(crate) fn rename_device_in_config(
    config: &mut AppConfig,
    device_id: &str,
    name: &str,
) {
    // NB: writing to `config.devices` is safe even on a V1 config —
    // `AppConfig.devices: Vec<v2::DeviceEntry>` has `#[serde(default)]`
    // and serializes with `#[serde(skip_serializing_if = "Option::is_none")]`
    // on optional fields, so a V1 round-trip with one new device entry
    // produces a file that still parses as V1 and also as V2. We do NOT
    // bump `schema_version` here — renaming is orthogonal to schema
    // version; Step 10 (migration wiring) owns that transition.
    if name.is_empty() {
        config.devices.retain(|d| d.device_id != device_id);
        return;
    }
    if let Some(existing) = config.devices.iter_mut().find(|d| d.device_id == device_id) {
        existing.name = Some(name.to_string());
    } else {
        config
            .devices
            .push(corsair_common::config::v2::DeviceEntry {
                device_id: device_id.to_string(),
                name: Some(name.to_string()),
                led_count: None,
            });
    }
}

/// Handle `HwCommand::RenameDevice`: mutate config, persist, notify
/// control loop. Returns a String error if persistence fails; the
/// in-memory mutation is best-effort applied even if write fails, so the
/// UI stays consistent with what it just committed to.
fn apply_rename_device(
    config: &mut AppConfig,
    control_loop: &mut ControlLoop,
    device_id: &str,
    name: &str,
    store: &dyn ConfigStore,
) -> Result<(), String> {
    rename_device_in_config(config, device_id, name);
    if let Err(e) = store.save(config) {
        return Err(format!("Failed to persist renamed device: {}", e));
    }
    // Apply live so a subsequent get_config returns the new name without
    // a round-trip through disk. update_config re-runs its device-entry
    // pass; since we only changed `name`, no led_count rewrite happens.
    if let Err(e) = control_loop.update_config(config.clone()) {
        return Err(format!("Failed to apply rename to control loop: {}", e));
    }
    Ok(())
}

/// Run the V1→V2 migration, if applicable, and reload the control loop's
/// config from the rewritten file on success.
///
/// Error semantics match the plan's soft-fail contract:
/// - `AlreadyMigrated`: info log, nothing else.
/// - `Migrated { resolved_count }`: info log, reload config from disk,
///   apply to the running control loop, emit `config-migrated` event.
/// - `UnresolvedDevice`: warn with the specific (hub, channel), emit
///   `config-migration-skipped` event, continue with current V1 config.
///   Migration retries at next startup.
/// - Any other `MigrationError`: error log, continue with current V1 config.
///
/// This function intentionally swallows all errors — a fan-control daemon
/// must never refuse to start because config migration hit a snag.
fn run_config_migration(
    config: &mut AppConfig,
    control_loop: &mut ControlLoop,
    app: &tauri::AppHandle,
) {
    // If we're already on V2 in memory, skip the file probe entirely —
    // there's no legacy shape to rewrite. (The file-level check still runs
    // below when the in-memory config is V1, even if the on-disk file is
    // already V2, because the migration helper itself is cheap and
    // idempotent on an already-V2 file.)
    if config.is_v2() {
        info!("Config already V2 in memory — migration skipped");
        return;
    }

    let path = crate::config_path();
    if !path.exists() {
        // No file on disk (fresh install, running on defaults). Nothing to
        // migrate — the next save_config_to_disk writes a V1-shaped file
        // that a subsequent build will migrate.
        info!(
            path = %path.display(),
            "Config file not present — nothing to migrate"
        );
        return;
    }

    let registry = control_loop.registry();
    match try_migrate_file(&path, registry) {
        Ok(MigrationOutcome::AlreadyMigrated) => {
            info!("Config already V2 on disk");
        }
        Ok(MigrationOutcome::Migrated { resolved_count }) => {
            info!(
                resolved = resolved_count,
                path = %path.display(),
                "Migrated config to V2 ({} device references resolved)",
                resolved_count
            );
            // Reload and apply. If the reload or apply fails we keep the
            // in-memory V1 config running — the new V2 file is on disk and
            // will be picked up on the next clean boot.
            match control_loop::load_config(&path) {
                Ok(new_config) => {
                    if let Err(e) = control_loop.update_config(new_config.clone()) {
                        warn!(
                            error = %e,
                            "Migration wrote V2 config but live apply failed — \
                             continuing on in-memory V1 until restart"
                        );
                    } else {
                        *config = new_config;
                        let _ = app.emit(
                            "config-migrated",
                            serde_json::json!({ "resolved_count": resolved_count }),
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        path = %path.display(),
                        "Migration wrote V2 config but reload failed — \
                         continuing on in-memory V1 until restart"
                    );
                }
            }
        }
        Err(MigrationError::UnresolvedDevice {
            hub_serial,
            channel,
        }) => {
            warn!(
                hub_serial = hub_serial.as_str(),
                channel,
                "Config migration skipped — device at hub '{}' channel {} not \
                 currently enumerated (will retry on next boot)",
                hub_serial,
                channel
            );
            // Surface to the frontend so a toast/banner can explain why a
            // startup-time migration didn't happen. Payload carries the
            // first offending pair; a more comprehensive list would require
            // refactoring the all-or-nothing migration to accumulate.
            let _ = app.emit(
                "config-migration-skipped",
                serde_json::json!({
                    "hub_serial": hub_serial,
                    "channel": channel,
                    "reason": "device not currently enumerated",
                }),
            );
        }
        Err(e) => {
            error!(
                error = %e,
                "Config migration failed with non-resolvable error — continuing \
                 on V1 config"
            );
        }
    }
}

/// Clear `preset_applied` on every group whose current mode no longer
/// matches the canonical values of the named preset. Called from the
/// `save_config` path so a user drag-edit immediately marks a group as
/// drifted. `apply_preset` then re-sets the tag on the next apply.
///
/// Comparison is structural on the mode variant: for Curve mode we
/// compare points / hysteresis / ramp_up / ramp_down. Temp-source
/// differences alone do NOT drift the preset (the preset's sensor
/// weights get re-resolved at apply-time against the currently-
/// available sensors, so a machine with no GPU resolves a preset's
/// 0.6/0.4 to 1.0 CPU — that's by design, not a drift).
pub(crate) fn reconcile_preset_applied(config: &mut AppConfig) {
    use corsair_common::config::FanMode;

    for group in &mut config.fan_groups {
        let Some(preset_id) = group.preset_applied.as_deref() else {
            continue;
        };
        let Some(def) = crate::presets::lookup(preset_id) else {
            // Unknown preset id (e.g. config from a newer version that
            // shipped a preset removed here). Clear it — the UI would
            // render it as Off anyway.
            group.preset_applied = None;
            continue;
        };

        let matches = match &group.mode {
            FanMode::Curve {
                points,
                hysteresis,
                ramp_rate,
                ramp_down_rate,
                ..
            } => {
                let same_points = points.len() == def.points.len()
                    && points.iter().zip(def.points.iter()).all(|(a, b)| {
                        (a.temp - b.temp).abs() < 0.001
                            && (a.duty - b.duty).abs() < 0.001
                    });
                let same_hyst = (hysteresis - def.hysteresis).abs() < 0.001;
                let same_up = (ramp_rate - def.ramp_up).abs() < 0.001;
                let same_down = ramp_down_rate
                    .map(|d| (d - def.ramp_down).abs() < 0.001)
                    .unwrap_or(false);
                same_points && same_hyst && same_up && same_down
            }
            // Fixed / Pid modes never match a curve-valued preset.
            _ => false,
        };

        if !matches {
            group.preset_applied = None;
        }
    }
}

pub(crate) fn save_config_to_disk(config: &AppConfig) -> Result<()> {
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    let config_path = crate::config_path();
    // atomic_write handles parent-directory creation, write+fsync, and rename.
    // Using it here prevents a truncated/empty config.toml if the process is
    // killed (crash, power loss, Task Manager) mid-save.
    corsair_common::atomic_write::write_atomic(&config_path, toml_str.as_bytes())
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", config_path.display(), e))?;
    info!("Config saved to {}", config_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use corsair_common::config::v2::DeviceEntry;
    use corsair_common::config::{FanMode, GeneralConfig, SCHEMA_VERSION_V1};

    /// Baseline V1 config used by rename tests — no schema_version bump
    /// happens on rename so we pin the starting state explicitly.
    fn empty_v1_config() -> AppConfig {
        AppConfig {
            schema_version: SCHEMA_VERSION_V1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: Vec::new(),
            rgb: Default::default(),
            device_overrides: Vec::new(),
            devices: Vec::new(),
        }
    }

    /// Renaming a device that has no existing entry inserts a new one
    /// with `name` set and `led_count = None` (preserves the invariant
    /// that renaming is orthogonal to LED-count overrides). Schema
    /// version is left untouched — the plan says rename is orthogonal
    /// to schema version.
    #[test]
    fn rename_device_inserts_new_entry() {
        let mut cfg = empty_v1_config();
        rename_device_in_config(&mut cfg, "ID_A1", "Top Front Left");
        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(
            cfg.devices[0],
            DeviceEntry {
                device_id: "ID_A1".to_string(),
                name: Some("Top Front Left".to_string()),
                led_count: None,
            }
        );
        // Schema version NOT bumped by rename — orthogonal concerns.
        assert_eq!(cfg.schema_version, SCHEMA_VERSION_V1);
    }

    /// Renaming an existing entry updates its `name` while preserving
    /// any `led_count` override that was already set for that device.
    #[test]
    fn rename_device_preserves_led_count() {
        let mut cfg = empty_v1_config();
        cfg.devices.push(DeviceEntry {
            device_id: "ID_A1".to_string(),
            name: Some("Old Name".to_string()),
            led_count: Some(30),
        });

        rename_device_in_config(&mut cfg, "ID_A1", "New Name");

        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(cfg.devices[0].name.as_deref(), Some("New Name"));
        assert_eq!(
            cfg.devices[0].led_count,
            Some(30),
            "led_count override must survive a rename"
        );
    }

    /// An empty `name` removes the device entry entirely. If the entry
    /// also carried an led_count, the user loses it — that's the
    /// documented contract (empty name = "revert to defaults, drop
    /// everything"). If that's not what the user wanted, they'd keep a
    /// placeholder name or edit the override separately.
    #[test]
    fn rename_device_empty_name_removes_entry() {
        let mut cfg = empty_v1_config();
        cfg.devices.push(DeviceEntry {
            device_id: "ID_A1".to_string(),
            name: Some("Old Name".to_string()),
            led_count: Some(42),
        });
        cfg.devices.push(DeviceEntry {
            device_id: "ID_B1".to_string(),
            name: Some("Keep Me".to_string()),
            led_count: None,
        });

        rename_device_in_config(&mut cfg, "ID_A1", "");

        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(cfg.devices[0].device_id, "ID_B1");
    }

    /// Renaming a non-existent device_id while asking for an empty name
    /// is a no-op (don't insert an entry just to immediately clear it).
    #[test]
    fn rename_device_empty_name_on_missing_is_noop() {
        let mut cfg = empty_v1_config();
        rename_device_in_config(&mut cfg, "ID_NONE", "");
        assert!(cfg.devices.is_empty());
    }

    // --- reconcile_preset_applied ---

    use corsair_common::config::{CurvePoint, TempSourceConfig};

    /// Silent preset's canonical values, matching fans.toml. Keeping
    /// this literal in the test instead of reading from the library
    /// prevents a silent regression — if someone changes fans.toml and
    /// the reconciler stops recognising Silent, this test goes red.
    fn silent_canonical_curve() -> FanMode {
        FanMode::Curve {
            points: vec![
                CurvePoint { temp: 30.0, duty: 20.0 },
                CurvePoint { temp: 50.0, duty: 30.0 },
                CurvePoint { temp: 65.0, duty: 50.0 },
                CurvePoint { temp: 80.0, duty: 80.0 },
                CurvePoint { temp: 90.0, duty: 100.0 },
            ],
            hysteresis: 5.0,
            ramp_rate: 3.0,
            ramp_down_rate: Some(1.5),
            temp_source: TempSourceConfig {
                sensors: vec!["cpu".into()],
                weights: vec![1.0],
            },
        }
    }

    /// A group tagged with "silent" whose mode exactly matches Silent's
    /// canonical values retains its tag through reconciliation. This
    /// is the "Active" path the UI reads back as a glowing card.
    #[test]
    fn reconcile_keeps_tag_when_values_match() {
        let mut cfg = empty_v1_config();
        cfg.fan_groups.push(corsair_common::config::FanGroupConfig {
            name: "g".into(),
            channels: vec![1],
            hub_serial: Some("H".into()),
            device_ids: Vec::new(),
            mode: silent_canonical_curve(),
            preset_applied: Some("silent".into()),
        });

        super::reconcile_preset_applied(&mut cfg);

        assert_eq!(
            cfg.fan_groups[0].preset_applied.as_deref(),
            Some("silent"),
            "unchanged Silent curve must keep its tag"
        );
    }

    /// A group tagged with "silent" whose curve has been edited is
    /// cleared on reconciliation. This is the drift path that makes
    /// the UI switch to the Modified badge.
    #[test]
    fn reconcile_clears_tag_when_curve_drifts() {
        let mut cfg = empty_v1_config();
        let mut drifted = silent_canonical_curve();
        if let FanMode::Curve { ref mut points, .. } = drifted {
            // Nudge one point's duty by 1%.
            points[0].duty += 1.0;
        }
        cfg.fan_groups.push(corsair_common::config::FanGroupConfig {
            name: "g".into(),
            channels: vec![1],
            hub_serial: Some("H".into()),
            device_ids: Vec::new(),
            mode: drifted,
            preset_applied: Some("silent".into()),
        });

        super::reconcile_preset_applied(&mut cfg);

        assert!(
            cfg.fan_groups[0].preset_applied.is_none(),
            "drifted curve must clear the preset tag"
        );
    }

    /// An unknown preset id gets cleared — a config from a newer app
    /// version whose preset was renamed here shouldn't keep a dangling
    /// tag that never matches anything.
    #[test]
    fn reconcile_clears_unknown_preset_id() {
        let mut cfg = empty_v1_config();
        cfg.fan_groups.push(corsair_common::config::FanGroupConfig {
            name: "g".into(),
            channels: vec![1],
            hub_serial: Some("H".into()),
            device_ids: Vec::new(),
            mode: silent_canonical_curve(),
            preset_applied: Some("nonexistent_preset".into()),
        });

        super::reconcile_preset_applied(&mut cfg);

        assert!(cfg.fan_groups[0].preset_applied.is_none());
    }

    /// V1-shaped config round-trips cleanly with a newly-added V2
    /// device entry. The serde skip_serializing_if attributes on
    /// DeviceEntry.name / led_count keep the output compact and backwards
    /// parseable.
    #[test]
    fn rename_device_roundtrips_through_toml_on_v1_config() {
        let mut cfg = empty_v1_config();
        // Add a V1 fan group so the serialized form is realistic.
        cfg.fan_groups.push(corsair_common::config::FanGroupConfig {
            name: "g1".to_string(),
            channels: vec![1, 2],
            hub_serial: Some("HUB_A".to_string()),
            device_ids: Vec::new(),
            mode: FanMode::Fixed { duty_percent: 50.0 },
            preset_applied: None,
        });

        rename_device_in_config(&mut cfg, "ID_A1", "Top Front");

        let toml_str = toml::to_string(&cfg).expect("serialize");
        let reparsed: AppConfig = toml::from_str(&toml_str).expect("reparse");

        assert_eq!(reparsed.schema_version, SCHEMA_VERSION_V1);
        assert_eq!(reparsed.devices.len(), 1);
        assert_eq!(reparsed.devices[0].device_id, "ID_A1");
        assert_eq!(reparsed.devices[0].name.as_deref(), Some("Top Front"));
        assert_eq!(reparsed.fan_groups.len(), 1);
        assert_eq!(reparsed.fan_groups[0].channels, vec![1, 2]);
    }
}
