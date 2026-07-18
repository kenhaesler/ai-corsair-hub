use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

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
    Shutdown {
        reply: mpsc::SyncSender<Result<(), String>>,
    },
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
    hw_join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    pub startup_config_error: Option<String>,
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub(crate) struct ShutdownHandle {
    hw_sender: mpsc::Sender<HwCommand>,
    hw_join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl AppState {
    pub fn new(
        hw_sender: mpsc::Sender<HwCommand>,
        hw_join: std::thread::JoinHandle<()>,
        startup_config_error: Option<String>,
    ) -> Self {
        Self {
            hw_sender,
            hw_join: Arc::new(Mutex::new(Some(hw_join))),
            startup_config_error,
        }
    }

    pub fn shutdown_and_join(&self) -> Result<(), String> {
        self.shutdown_handle().shutdown_and_join()
    }

    pub(crate) fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            hw_sender: self.hw_sender.clone(),
            hw_join: Arc::clone(&self.hw_join),
        }
    }
}

impl ShutdownHandle {
    pub(crate) fn shutdown_and_join(&self) -> Result<(), String> {
        self.shutdown_and_join_with_timeout(SHUTDOWN_TIMEOUT)
    }

    fn shutdown_and_join_with_timeout(&self, timeout: Duration) -> Result<(), String> {
        let Some(handle) = self
            .hw_join
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
        else {
            return Ok(());
        };

        let deadline = Instant::now() + timeout;
        let (reply, ack) = mpsc::sync_channel(0);
        let acknowledgement = if self.hw_sender.send(HwCommand::Shutdown { reply }).is_ok() {
            ack.recv_timeout(remaining(deadline))
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => {
                        "Timed out waiting for hardware shutdown acknowledgement".to_string()
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        "Hardware thread stopped without acknowledging shutdown".to_string()
                    }
                })?
        } else {
            Ok(())
        };

        let joined = join_before_deadline(handle, deadline);
        acknowledgement?;
        joined
    }
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn join_before_deadline(handle: thread::JoinHandle<()>, deadline: Instant) -> Result<(), String> {
    while !handle.is_finished() {
        let remaining = remaining(deadline);
        if remaining.is_zero() {
            return Err("Timed out waiting for hardware thread to stop".to_string());
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }

    handle
        .join()
        .map_err(|_| "Hardware thread panicked during shutdown".to_string())
}

impl Drop for AppState {
    fn drop(&mut self) {
        let _ = self.shutdown_and_join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_waits_for_acknowledgement_and_joins() {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let HwCommand::Shutdown { reply } = receiver.recv().unwrap() else {
                panic!("unexpected command");
            };
            reply.send(Ok(())).unwrap();
        });
        let state = AppState::new(sender, worker, None);

        assert!(state.shutdown_and_join().is_ok());
        assert!(state.shutdown_and_join().is_ok());
    }

    #[test]
    fn shutdown_timeout_does_not_block_on_a_stuck_worker() {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let HwCommand::Shutdown { reply: _reply } = receiver.recv().unwrap() else {
                panic!("unexpected command");
            };
            thread::sleep(Duration::from_millis(200));
        });
        let state = AppState::new(sender, worker, None);
        let started = Instant::now();

        let error = state
            .shutdown_handle()
            .shutdown_and_join_with_timeout(Duration::from_millis(20))
            .unwrap_err();

        assert!(error.contains("Timed out"));
        assert!(started.elapsed() < Duration::from_millis(150));
    }
}
