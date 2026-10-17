use tauri::State;

use corsair_common::config::{AppConfig, FanPresetDef, RgbConfig};
use corsair_fancontrol::control_loop;

use crate::dto::{DeviceTree, ManualDutyResult, SystemSnapshot};
use crate::state::{AppState, ChannelIdentity, HwCommand};

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<SystemSnapshot, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::GetSnapshot { reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

#[tauri::command]
pub async fn get_devices(state: State<'_, AppState>) -> Result<DeviceTree, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::GetDevices { reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::GetConfig { reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// Return the set of sensor names the running control loop currently
/// has initialized (e.g. `cpu`, `gpu`, `psu_vrm`, `psu_case`). Powers
/// the per-group sensor-weighting UI — the picker only shows sensors
/// that actually exist on this machine right now.
#[tauri::command]
pub async fn list_available_sensors(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::ListSensors { reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::UpdateConfig { config, reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// Apply a curated fan preset globally (every configured group).
/// Retained for back-compat with saved UI state and the tray menu;
/// the v0.3 UI uses `apply_preset_to_group` for per-group control.
#[tauri::command]
pub async fn apply_preset(
    state: State<'_, AppState>,
    preset: String,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::ApplyPreset {
            preset,
            group_name: None,
            reply: tx,
        })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// Apply a curated fan preset to a specific group, or globally if
/// `group_name` is `None`. Powers the v0.3 preset rail + group
/// selector — the user can run "Competitive FPS" on radiators while
/// chassis fans stay on "Quiet Productivity".
#[tauri::command]
pub async fn apply_preset_to_group(
    state: State<'_, AppState>,
    preset: String,
    group_name: Option<String>,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::ApplyPreset {
            preset,
            group_name,
            reply: tx,
        })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// Return the full curated fan-preset library. Read-only, no
/// hardware-thread round-trip needed — the library is a compile-time
/// asset.
#[tauri::command]
pub fn get_fan_presets() -> Vec<FanPresetDef> {
    crate::presets::all().to_vec()
}

/// Legacy V1 duty command — still used by the unmigrated Svelte frontend.
/// Kept for one release so we don't break the UI in the middle of the
/// device-identity refactor; PR3 migrates callers to
/// [`set_manual_duty_by_device_id`] and a future release deletes this.
#[deprecated(
    since = "0.1.2",
    note = "Use set_manual_duty_by_device_id; channel-based addressing is fragile across topology changes."
)]
#[tauri::command]
pub async fn set_manual_duty(
    state: State<'_, AppState>,
    hub_serial: String,
    channels: Vec<u8>,
    duty: u8,
) -> Result<(), String> {
    if duty > 100 {
        return Err("duty must be 0-100".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::SetManualDuty {
            hub_serial,
            channels,
            duty,
            reply: tx,
        })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// V2 duty command: caller supplies a list of `device_id` strings. The
/// hardware thread resolves each to its current (hub_serial, channel)
/// pair, buckets by hub, and issues one `set_speeds` per hub. The
/// returned [`ManualDutyResult`] distinguishes applied vs unresolved ids
/// so the UI can surface orphans without hiding partial success as a
/// blanket error.
#[tauri::command]
pub async fn set_manual_duty_by_device_id(
    state: State<'_, AppState>,
    device_ids: Vec<String>,
    duty: u8,
) -> Result<ManualDutyResult, String> {
    if duty > 100 {
        return Err("duty must be 0-100".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::SetManualDutyByDeviceId {
            device_ids,
            duty,
            reply: tx,
        })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// Rename (or clear the custom name of) a specific device by its stable
/// device_id. Empty `name` clears the entry, reverting the UI to the
/// system-generated fallback label. Persisted atomically via the shared
/// config write path; the control loop is notified live so a subsequent
/// get_config returns the new name without a round-trip through disk.
#[tauri::command]
pub async fn rename_device(
    state: State<'_, AppState>,
    device_id: String,
    name: String,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::RenameDevice {
            device_id,
            name,
            reply: tx,
        })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

#[tauri::command]
pub fn validate_config(config: AppConfig) -> Result<(), String> {
    control_loop::validate_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_rgb_config(
    state: State<'_, AppState>,
    config: RgbConfig,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::SetRgbConfig { config, reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

#[tauri::command]
pub async fn set_rgb_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::SetRgbEnabled { enabled, reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

/// Debug / identification: paint every enumerated device a unique
/// colour for `duration_secs` seconds, then resume normal rendering.
/// The returned mapping lets the UI render a legend so the user can
/// correlate physical positions in their rig with channel numbers
/// the firmware actually assigned. No config-label guessing.
#[tauri::command]
pub async fn rgb_identify_channels(
    state: State<'_, AppState>,
    duration_secs: u64,
) -> Result<Vec<ChannelIdentity>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::RgbIdentifyChannels {
            duration_secs,
            reply: tx,
        })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}
