use std::sync::mpsc;

use corsair_common::config::{AppConfig, RgbConfig};

use crate::dto::{DeviceTree, SystemSnapshot};

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
    SetManualDuty {
        hub_serial: String,
        channels: Vec<u8>,
        duty: u8,
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
