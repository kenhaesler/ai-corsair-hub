use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tauri::Emitter;
use tracing::{error, info, warn};

use corsair_common::config::AppConfig;
use corsair_common::CorsairDevice;
use corsair_fancontrol::control_loop::{self, ControlLoop};
use corsair_hid::{CorsairPsu, DeviceScanner, IcueLinkHub};

use crate::dto::{DeviceTree, SystemSnapshot};
use crate::state::HwCommand;

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

    // Initialize device scanner (stays alive for device queries)
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

    let mut poll_interval = Duration::from_millis(config.general.poll_interval_ms);
    info!(
        interval_ms = config.general.poll_interval_ms,
        "Hardware thread running"
    );

    // Cache for device tree (rebuilt on demand, not every cycle)
    let mut _cached_device_tree: Option<DeviceTree> = None;

    // Open PSU once and keep the handle alive
    let psu_handle = open_psu(&scanner);
    if psu_handle.is_some() {
        info!("PSU opened and initialized");
    }

    loop {
        let cycle_start = Instant::now();

        // Process all pending commands (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(cmd) => match cmd {
                    HwCommand::GetSnapshot { reply } => {
                        // Will be sent via event below, but also reply directly
                        let result = build_snapshot(&mut control_loop, psu_handle.as_ref());
                        let _ = reply.send(result);
                        continue; // Don't tick, just respond
                    }
                    HwCommand::GetDevices { reply } => {
                        let tree = build_device_tree_from_loop(&control_loop, &scanner);
                        _cached_device_tree = Some(tree.clone());
                        let _ = reply.send(Ok(tree));
                    }
                    HwCommand::GetConfig { reply } => {
                        let _ = reply.send(Ok(control_loop.config().clone()));
                    }
                    HwCommand::UpdateConfig {
                        config: new_config,
                        reply,
                    } => {
                        // Validate first
                        if let Err(e) = control_loop::validate_config(&new_config) {
                            let _ = reply.send(Err(format!("Invalid config: {}", e)));
                            continue;
                        }
                        // Persist to disk
                        if let Err(e) = save_config_to_disk(&new_config) {
                            let _ = reply.send(Err(format!("Failed to save config: {}", e)));
                            continue;
                        }
                        // Apply live
                        match control_loop.update_config(new_config.clone()) {
                            Ok(()) => {
                                config = new_config;
                                poll_interval =
                                    Duration::from_millis(config.general.poll_interval_ms);
                                let _ = reply.send(Ok(()));
                            }
                            Err(e) => {
                                let _ = reply
                                    .send(Err(format!("Failed to apply config: {}", e)));
                            }
                        }
                    }
                    HwCommand::ApplyPreset { preset, reply } => {
                        let result = apply_preset(&preset, &mut config, &mut control_loop);
                        let ok = result.is_ok();
                        let _ = reply.send(result);
                        if ok {
                            poll_interval =
                                Duration::from_millis(config.general.poll_interval_ms);
                        }
                    }
                    HwCommand::SetManualDuty {
                        hub_serial,
                        channels,
                        duty,
                        reply,
                    } => {
                        let result = control_loop
                            .set_manual_duty(&hub_serial, &channels, duty)
                            .map_err(|e| format!("Failed to set duty: {}", e));
                        let _ = reply.send(result);
                    }
                    HwCommand::Shutdown => {
                        info!("Hardware thread received shutdown command");
                        control_loop.shutdown_hardware();
                        return Ok(());
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    info!("Hardware command channel closed — shutting down");
                    control_loop.shutdown_hardware();
                    return Ok(());
                }
            }
        }

        // Run one control cycle
        let cycle_result = control_loop.tick();

        // Read PSU status from persistent handle
        let psu_status = psu_handle.as_ref().and_then(|psu| {
            psu.read_all().ok().and_then(validate_psu_status)
        });

        // Build and emit snapshot
        let snapshot = SystemSnapshot::from_cycle(
            &cycle_result,
            &cycle_result.fan_speeds,
            psu_status.as_ref(),
        );
        let _ = app.emit("sensor-update", &snapshot);

        // Sleep for remaining poll interval
        let elapsed = cycle_start.elapsed();
        if elapsed < poll_interval {
            std::thread::sleep(poll_interval - elapsed);
        }
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
            Ok(HwCommand::UpdateConfig { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::ApplyPreset { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::SetManualDuty { reply, .. }) => {
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
            Ok(HwCommand::UpdateConfig { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::ApplyPreset { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::SetManualDuty { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
        }
    }
}

fn build_snapshot(
    control_loop: &mut ControlLoop,
    psu_handle: Option<&CorsairPsu>,
) -> Result<SystemSnapshot, String> {
    let cycle_result = control_loop.tick();
    let psu_status = psu_handle.and_then(|psu| {
        psu.read_all().ok().and_then(validate_psu_status)
    });
    Ok(SystemSnapshot::from_cycle(
        &cycle_result,
        &cycle_result.fan_speeds,
        psu_status.as_ref(),
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
    // Temperature: -10 to 150°C is physically plausible
    if s.temp_vrm < -10.0 || s.temp_vrm > 150.0 { return None; }
    if s.temp_case < -10.0 || s.temp_case > 150.0 { return None; }
    // Voltage: AC input 80-300V, rails 0-15V
    if s.input_voltage < 0.0 || s.input_voltage > 300.0 { return None; }
    if s.rail_12v.voltage < 0.0 || s.rail_12v.voltage > 15.0 { return None; }
    if s.rail_5v.voltage < 0.0 || s.rail_5v.voltage > 7.0 { return None; }
    if s.rail_3v3.voltage < 0.0 || s.rail_3v3.voltage > 5.0 { return None; }
    // Power: 0-2000W total
    if s.total_power < 0.0 || s.total_power > 2000.0 { return None; }
    Some(s)
}

fn apply_preset(
    preset: &str,
    config: &mut AppConfig,
    control_loop: &mut ControlLoop,
) -> Result<(), String> {
    use corsair_common::config::{CurvePoint, FanMode, TempSourceConfig};

    // Pick best available temp source
    let available = control_loop.available_sensors();
    let temp_source = if available.contains(&"cpu".to_string())
        && available.contains(&"gpu".to_string())
    {
        // Both available: 60% CPU, 40% GPU — water loop cools both
        TempSourceConfig {
            sensors: vec!["cpu".to_string(), "gpu".to_string()],
            weights: vec![0.6, 0.4],
        }
    } else if available.contains(&"cpu".to_string()) {
        TempSourceConfig {
            sensors: vec!["cpu".to_string()],
            weights: vec![1.0],
        }
    } else if available.contains(&"gpu".to_string()) {
        TempSourceConfig {
            sensors: vec!["gpu".to_string()],
            weights: vec![1.0],
        }
    } else {
        // No sensors — fall back to fixed duty
        for group in &mut config.fan_groups {
            group.mode = match preset {
                "silent" => FanMode::Fixed { duty_percent: 25.0 },
                "balanced" => FanMode::Fixed { duty_percent: 50.0 },
                "performance" => FanMode::Fixed { duty_percent: 80.0 },
                other => return Err(format!("Unknown preset: {}", other)),
            };
        }
        return control_loop
            .update_config(config.clone())
            .map_err(|e| format!("Failed to apply preset: {}", e));
    };

    // Intelligent curves optimized for custom water cooling
    // (3x 420mm radiators, high thermal mass, shared CPU+GPU loop)
    let mode = match preset {
        "silent" => FanMode::Curve {
            // Prioritize silence — rely on radiator thermal mass for short bursts
            // Fans stay near-silent until water loop really heats up
            points: vec![
                CurvePoint { temp: 30.0, duty: 20.0 },
                CurvePoint { temp: 50.0, duty: 30.0 },
                CurvePoint { temp: 65.0, duty: 50.0 },
                CurvePoint { temp: 80.0, duty: 80.0 },
                CurvePoint { temp: 90.0, duty: 100.0 },
            ],
            hysteresis: 5.0, // wide band prevents fan hunting
            ramp_rate: 3.0,  // slow ramp — inaudible transitions
            temp_source: temp_source.clone(),
        },
        "balanced" => FanMode::Curve {
            // Good tradeoff: cool enough for sustained gaming, quiet at idle
            points: vec![
                CurvePoint { temp: 30.0, duty: 25.0 },
                CurvePoint { temp: 45.0, duty: 35.0 },
                CurvePoint { temp: 60.0, duty: 55.0 },
                CurvePoint { temp: 75.0, duty: 80.0 },
                CurvePoint { temp: 85.0, duty: 100.0 },
            ],
            hysteresis: 3.0,
            ramp_rate: 5.0,
            temp_source: temp_source.clone(),
        },
        "performance" => FanMode::Curve {
            // Aggressive cooling: fans respond quickly, keeps temps low
            // Good for sustained workloads, rendering, stress tests
            points: vec![
                CurvePoint { temp: 30.0, duty: 30.0 },
                CurvePoint { temp: 40.0, duty: 50.0 },
                CurvePoint { temp: 55.0, duty: 70.0 },
                CurvePoint { temp: 70.0, duty: 90.0 },
                CurvePoint { temp: 80.0, duty: 100.0 },
            ],
            hysteresis: 2.0, // tight band — quick response
            ramp_rate: 10.0, // fast ramp — don't let temps climb
            temp_source: temp_source.clone(),
        },
        other => return Err(format!("Unknown preset: {}", other)),
    };

    for group in &mut config.fan_groups {
        group.mode = mode.clone();
    }

    // Persist to disk so it survives restart
    if let Err(e) = save_config_to_disk(config) {
        warn!("Failed to save preset config: {}", e);
    }

    control_loop
        .update_config(config.clone())
        .map_err(|e| format!("Failed to apply preset: {}", e))
}

fn save_config_to_disk(config: &AppConfig) -> Result<()> {
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    let config_path = crate::config_path();
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create config dir: {}", e))?;
    }
    std::fs::write(&config_path, toml_str)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", config_path.display(), e))?;
    info!("Config saved to {}", config_path.display());
    Ok(())
}
