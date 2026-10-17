//! Injectable dependencies for the hardware worker loop.
//!
//! `hardware_thread::Worker` talks to the outside world only through the
//! traits in this module, so the integration harness can drive the real loop
//! with a fake clock, mock hubs, scripted telemetry, in-memory config storage
//! and a recording event sink -- no Tauri window, USB device or LHM process.
//! The production implementations of the trivially-wrapping traits live here;
//! the ones that need hardware helpers (`RealTelemetry`, `RealHubBackend`)
//! live next to those helpers in `hardware_thread.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use corsair_common::config::AppConfig;
use corsair_fancontrol::control_loop::ControlLoop;
use corsair_hid::{HubInfo, IcueLinkTransport, PsuStatus};

use crate::dto::{CpuMetricsDto, DeviceTree, GpuMetricsDto};

/// Sink for events pushed to the frontend.
///
/// The payload is passed already serialized (`serde_json::to_string` of the
/// DTO) -- exactly what Tauri's `Emitter::emit` does internally -- so the
/// wire bytes are identical to emitting the struct directly. (Round-tripping
/// through `serde_json::Value` would not be: it widens `f32` to `f64` and
/// reorders object keys.)
pub trait EventSink {
    fn emit_json(&self, event: &str, payload_json: String);
}

/// Serialize `payload` and hand it to the sink. Serialization failure drops
/// the event silently, matching the previous `let _ = app.emit(..)`.
pub fn emit<T: Serialize + ?Sized>(sink: &dyn EventSink, event: &str, payload: &T) {
    if let Ok(json) = serde_json::to_string(payload) {
        sink.emit_json(event, json);
    }
}

/// Production sink: forwards to the Tauri app handle.
pub struct TauriEventSink {
    app: tauri::AppHandle,
}

impl TauriEventSink {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriEventSink {
    fn emit_json(&self, event: &str, payload_json: String) {
        let _ = self.app.emit_str(event, payload_json);
    }
}

/// Monotonic time source plus the loop's only sleep.
pub trait Clock {
    fn now(&self) -> Instant;
    fn sleep(&self, d: Duration);
    /// Wall-clock time, used only to stamp diagnostic exports.
    fn wall_time(&self) -> std::time::SystemTime {
        std::time::SystemTime::now()
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Persistence for the application config.
pub trait ConfigStore {
    fn save(&self, cfg: &AppConfig) -> anyhow::Result<()>;
}

pub struct DiskConfigStore;

impl ConfigStore for DiskConfigStore {
    fn save(&self, cfg: &AppConfig) -> anyhow::Result<()> {
        crate::hardware_thread::save_config_to_disk(cfg)
    }
}

/// Destination for diagnostic exports (e.g. the LED slot manifest).
pub trait DiagnosticsStore {
    /// Write `contents` to `file_name` (a bare file name, no directories)
    /// inside the diagnostics directory and return the full path written.
    fn write_file(&self, file_name: &str, contents: &str) -> anyhow::Result<std::path::PathBuf>;
}

/// Production store: `%APPDATA%\corsair-hub\diagnostics\` (next to
/// `config.toml`, see `config_path()`).
pub struct DiskDiagnosticsStore;

impl DiagnosticsStore for DiskDiagnosticsStore {
    fn write_file(&self, file_name: &str, contents: &str) -> anyhow::Result<std::path::PathBuf> {
        use anyhow::Context;
        if file_name.contains(['/', '\\']) || file_name.contains("..") {
            anyhow::bail!("invalid diagnostics file name: {file_name}");
        }
        let dir = crate::config_path()
            .parent()
            .map(|p| p.join("diagnostics"))
            .unwrap_or_else(|| std::path::PathBuf::from("diagnostics"));
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join(file_name);
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }
}

/// Per-iteration telemetry reads that are not fan-control sensors.
pub trait Telemetry {
    /// Raw (unvalidated) PSU status from LibreHardwareMonitor.
    fn read_psu_lhm(&self) -> Option<PsuStatus>;
    /// Raw PSU status over the direct HID handle. `None` when no handle is
    /// open; `Some(Err)` when a read failed.
    fn read_psu_hid(&self) -> Option<anyhow::Result<PsuStatus>>;
    fn read_gpu_metrics(&self) -> Option<GpuMetricsDto>;
    fn read_cpu_metrics(&self) -> Option<CpuMetricsDto>;
}

/// Hub re-open (recovery) and device-tree discovery.
pub trait HubBackend {
    /// Re-enumerate, re-open and re-initialize the hub with `serial`.
    fn reopen(&mut self, serial: &str) -> anyhow::Result<(Arc<dyn IcueLinkTransport>, HubInfo)>;
    /// Device tree for the `GetDevices` command.
    fn device_tree(&self, control_loop: &ControlLoop) -> DeviceTree;
}

/// Everything the worker loop needs from the outside world.
pub struct WorkerEnv {
    pub events: Box<dyn EventSink>,
    pub clock: Box<dyn Clock>,
    pub store: Box<dyn ConfigStore>,
    pub diagnostics: Box<dyn DiagnosticsStore>,
    pub telemetry: Box<dyn Telemetry>,
    pub hubs: Box<dyn HubBackend>,
}
