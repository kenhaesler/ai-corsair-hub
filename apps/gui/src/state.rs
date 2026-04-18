use std::sync::mpsc;

use corsair_common::config::{AppConfig, RgbConfig};

use crate::dto::{DeviceTree, ManualDutyResult, SystemSnapshot};

/// Commands sent from Tauri async command handlers to the hardware thread.
pub enum HwCommand {
    GetSnapshot {
        reply: tokio::sync::oneshot::Sender<Result<SystemSnapshot, String>>,
    },
    GetDevices {
        reply: tokio::sync::oneshot::Sender<Result<DeviceTree, String>>,
    },
    GetConfig {
        reply: tokio::sync::oneshot::Sender<Result<AppConfig, String>>,
    },
    UpdateConfig {
        config: AppConfig,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    ApplyPreset {
        preset: String,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Legacy V1 duty path — kept for one release for the unmigrated
    /// Svelte frontend. PR3 migrates callers to
    /// [`HwCommand::SetManualDutyByDeviceId`].
    SetManualDuty {
        hub_serial: String,
        channels: Vec<u8>,
        duty: u8,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// V2 duty path: caller supplies device_ids, the hardware thread
    /// resolves each to `(hub_serial, channel)` via the live registry,
    /// buckets per hub, and issues one `set_speeds` per hub. The reply
    /// reports which ids were applied and which were unresolvable so the
    /// UI can surface orphans to the user without hiding the failure.
    SetManualDutyByDeviceId {
        device_ids: Vec<String>,
        duty: u8,
        reply: tokio::sync::oneshot::Sender<Result<ManualDutyResult, String>>,
    },
    /// Persist a friendly name for a specific device_id. Empty `name`
    /// clears the entry (reverts to the system-generated display name).
    RenameDevice {
        device_id: String,
        name: String,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetRgbConfig {
        config: RgbConfig,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetRgbEnabled {
        enabled: bool,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    Shutdown,
}

/// Shared state managed by Tauri, accessible from command handlers.
pub struct AppState {
    pub hw_sender: mpsc::Sender<HwCommand>,
}
