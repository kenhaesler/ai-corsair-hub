use std::sync::mpsc;

use corsair_common::config::{AppConfig, RgbConfig};

use crate::diagnostics::LedMarkerReport;
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
    /// Return the names of sensors the running control loop currently
    /// has initialized. Powers the per-group sensor-weighting chips
    /// introduced in v0.4 — the UI only shows sensors that are actually
    /// available rather than a speculative hardcoded list.
    ListSensors {
        reply: tokio::sync::oneshot::Sender<Result<Vec<String>, String>>,
    },
    UpdateConfig {
        config: AppConfig,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    ApplyPreset {
        preset: String,
        /// `None` applies the preset to every fan group (pre-v0.3
        /// global behavior). `Some(name)` applies only to the named
        /// group — used by the v0.3 per-group preset rail so the user
        /// can set "Competitive FPS" on radiator fans while keeping
        /// chassis fans on "Quiet Productivity".
        group_name: Option<String>,
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
    /// Debug / identification: assign a unique colour to every enumerated
    /// device on every hub for `duration_secs` seconds, then resume the
    /// normal zone rendering. Colours are deterministic (see the palette
    /// in hardware_thread) so the user can correlate "which physical
    /// fan is ch1 of hub E7EC?" by eye.
    ///
    /// The reply returns the planned (hub_serial, channel, [r,g,b], label)
    /// mapping so the UI can show the user what to look for.
    RgbIdentifyChannels {
        duration_secs: u64,
        reply: tokio::sync::oneshot::Sender<Result<Vec<ChannelIdentity>, String>>,
    },
    /// G05 LED marker diagnostic: for `duration_secs` (clamped 5..=300) every
    /// LED slot on every hub shows white (first LED), green (second, when the
    /// slot has 3+ LEDs), red (last LED) and its Identify palette colour
    /// elsewhere, through the production conversion and send path. Requires
    /// RGB enabled with hardware output. Never touches the saved config.
    /// The reply is the slot manifest report (also exported as JSON).
    /// `mode` selects the pattern (markers, black-background markers, ruler
    /// or a walking LED on one slot); see `diagnostics::MarkerMode`.
    RgbMarkerDiagnostic {
        duration_secs: u64,
        mode: crate::diagnostics::MarkerMode,
        reply: tokio::sync::oneshot::Sender<Result<LedMarkerReport, String>>,
    },
    Shutdown,
}

/// One row in the channel-identify mapping returned by
/// [`HwCommand::RgbIdentifyChannels`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChannelIdentity {
    pub hub_serial: String,
    pub channel: u8,
    pub device_id: String,
    pub device_type: String,
    pub color_name: String,
    pub rgb: [u8; 3],
}

/// Shared state managed by Tauri, accessible from command handlers.
pub struct AppState {
    pub hw_sender: mpsc::Sender<HwCommand>,
    /// Set when the on-disk config could not be used at startup.
    pub startup_issue: Option<crate::startup_config::StartupConfigIssue>,
}
