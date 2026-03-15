use tauri::State;

use corsair_common::config::{AppConfig, RgbConfig};
use corsair_fancontrol::control_loop;

use crate::dto::{DeviceTree, SystemSnapshot};
use crate::state::{AppState, HwCommand};

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

#[tauri::command]
pub async fn apply_preset(
    state: State<'_, AppState>,
    preset: String,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .hw_sender
        .send(HwCommand::ApplyPreset { preset, reply: tx })
        .map_err(|_| "Hardware thread unavailable".to_string())?;
    rx.await
        .map_err(|_| "Hardware thread dropped".to_string())?
}

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
