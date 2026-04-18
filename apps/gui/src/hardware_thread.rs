use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tauri::Emitter;
use tracing::{error, info, warn};

use corsair_common::config::AppConfig;
use corsair_common::CorsairDevice;
use corsair_fancontrol::control_loop::{self, ControlLoop, RgbFrameRef};
use corsair_hid::{port_power_factor, CorsairPsu, DeviceScanner, IcueLinkHub, LinkDeviceType};
use corsair_rgb::effect::EffectContext;
use corsair_rgb::layout::LedLayout;
use corsair_rgb::renderer::{DeviceConfig, RgbRenderer, ZoneConfig};

use crate::dto::{DeviceTree, RgbFrameDto, SystemSnapshot};
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

    // Ensure LibreHardwareMonitor is running for accurate CPU temps
    let lhm_ready = corsair_sensors::lhm::ensure_lhm_running(
        config.general.lhm_exe_path.as_deref(),
    );
    if !lhm_ready {
        warn!("LibreHardwareMonitor not available — CPU temps will use fallback sensors");
    }

    // Initialize device scanner (stays alive for device queries and recovery)
    let mut scanner = match DeviceScanner::new() {
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

    // RGB renderer — time-sliced alongside fan control
    let mut rgb_renderer = RgbRenderer::new();
    let mut rgb_enabled = config.rgb.enabled;
    let mut rgb_interval = Duration::from_millis(if config.rgb.fps > 0 {
        1000 / config.rgb.fps as u64
    } else {
        33
    });
    let mut last_rgb_tick = Instant::now();
    let mut last_temp: Option<f64> = None;
    let mut last_temp_time: Option<Instant> = None;
    apply_rgb_config(&mut rgb_renderer, &config, &control_loop.device_type_map());

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
    let mut last_good_psu: Option<corsair_hid::PsuStatus> = None;
    let mut psu_fail_count: u32 = 0;
    const PSU_STALE_LIMIT: u32 = 10; // drop after 10 consecutive failures

    loop {
        let cycle_start = Instant::now();

        // Process all pending commands (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(cmd) => match cmd {
                    HwCommand::GetSnapshot { reply } => {
                        // Will be sent via event below, but also reply directly
                        let result = build_snapshot(&mut control_loop, psu_handle.as_ref(), &last_good_psu);
                        let _ = reply.send(result);
                        continue; // Don't tick, just respond
                    }
                    HwCommand::GetDevices { reply } => {
                        let tree = build_device_tree_from_loop(&control_loop, &scanner);
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
                    HwCommand::SetRgbConfig {
                        config: rgb_config,
                        reply,
                    } => {
                        config.rgb = rgb_config;
                        rgb_enabled = config.rgb.enabled;
                        rgb_interval = Duration::from_millis(if config.rgb.fps > 0 {
                            1000 / config.rgb.fps as u64
                        } else {
                            33
                        });
                        apply_rgb_config(&mut rgb_renderer, &config, &control_loop.device_type_map());
                        if let Err(e) = save_config_to_disk(&config) {
                            warn!("Failed to save RGB config: {}", e);
                        }
                        let _ = reply.send(Ok(()));
                    }
                    HwCommand::SetRgbEnabled { enabled, reply } => {
                        rgb_enabled = enabled;
                        config.rgb.enabled = enabled;
                        if let Err(e) = save_config_to_disk(&config) {
                            warn!("Failed to save RGB enabled state: {}", e);
                        }
                        let _ = reply.send(Ok(()));
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

        // Emit hub-lost for any hub that just became unhealthy (first failure)
        for report in &cycle_result.hub_health {
            if report.consecutive_failures == 1 {
                warn!(serial = report.serial.as_str(), "Hub lost — will attempt recovery");
                let _ = app.emit("hub-lost", &report.serial);
            }
        }

        // Attempt recovery for hubs with sustained failures
        let needing_recovery = control_loop.hubs_needing_recovery();
        for serial in &needing_recovery {
            info!(serial = serial.as_str(), "Attempting hub recovery");
            let recovered = scanner
                .refresh()
                .and_then(|()| {
                    scanner.open_device(
                        CorsairDevice::IcueLinkHub.pid(),
                        serial,
                        IcueLinkHub::data_interface(),
                    )
                })
                .and_then(|dev| {
                    let hub = IcueLinkHub::new(dev, serial.clone());
                    let info = hub.initialize()?;
                    Ok((hub, info))
                });

            match recovered {
                Ok((hub, info)) => {
                    info!(serial = serial.as_str(), "Hub recovery succeeded");
                    control_loop.replace_hub(serial, hub, info);
                    let _ = app.emit("hub-recovered", serial);
                }
                Err(e) => {
                    warn!(serial = serial.as_str(), error = %e, "Hub recovery failed");
                    control_loop.mark_recovery_attempted(serial);
                }
            }
        }

        // Read PSU status: LHM first (no USB contention), then direct HID fallback
        let psu_status = corsair_sensors::lhm::read_psu_from_lhm()
            .and_then(validate_psu_status)
            .or_else(|| {
                psu_handle.as_ref().and_then(|psu| {
                    match psu.read_all() {
                        Ok(raw) => validate_psu_status(raw),
                        Err(e) => {
                            if psu_fail_count <= 3 || psu_fail_count % 10 == 0 {
                                warn!("PSU HID read error (fail #{}): {}", psu_fail_count + 1, e);
                            }
                            None
                        }
                    }
                })
            });

        // Update cache or fall back to last good reading
        let psu_status = match psu_status {
            Some(valid) => {
                if psu_fail_count > 0 {
                    info!("PSU read recovered after {} failures", psu_fail_count);
                }
                psu_fail_count = 0;
                last_good_psu = Some(valid.clone());
                Some(valid)
            }
            None => {
                psu_fail_count += 1;
                if psu_fail_count <= PSU_STALE_LIMIT {
                    last_good_psu.clone()
                } else {
                    None
                }
            }
        };

        // Build and emit snapshot
        let snapshot = SystemSnapshot::from_cycle(
            &cycle_result,
            &cycle_result.fan_speeds,
            psu_status.as_ref(),
        );
        let _ = app.emit("sensor-update", &snapshot);

        // RGB render tick (time-sliced at configured FPS)
        if rgb_enabled && last_rgb_tick.elapsed() >= rgb_interval {
            // Build EffectContext from latest sensor data
            let primary_temp = snapshot
                .temperatures
                .first()
                .map(|t| t.celsius);

            // Compute temp delta (°C/sec)
            let temp_delta = primary_temp.and_then(|current| {
                let now = Instant::now();
                let delta = last_temp.map(|prev| {
                    let dt = last_temp_time
                        .map(|t| now.duration_since(t).as_secs_f64())
                        .unwrap_or(1.0)
                        .max(0.01);
                    (current - prev) / dt
                });
                last_temp = Some(current);
                last_temp_time = Some(now);
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
                all_temps: snapshot
                    .temperatures
                    .iter()
                    .map(|t| (t.source.clone(), t.celsius))
                    .collect(),
            };

            let frames = rgb_renderer.tick(&effect_ctx);
            let frame_dtos: Vec<RgbFrameDto> =
                frames.iter().map(RgbFrameDto::from_frame).collect();
            let _ = app.emit("rgb-frame", &frame_dtos);

            // Send to hardware if enabled
            if config.rgb.hardware_output && !frames.is_empty() {
                let hw_start = Instant::now();

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

                let frame_refs: Vec<RgbFrameRef> = frames
                    .iter()
                    .zip(hw_leds.iter())
                    .map(|(f, leds)| RgbFrameRef {
                        hub_serial: &f.hub_serial,
                        channel: f.channel,
                        leds,
                    })
                    .collect();

                let sent = control_loop.send_rgb_frames(&frame_refs);
                let hw_elapsed = hw_start.elapsed();
                if hw_elapsed > Duration::from_millis(25) {
                    warn!(
                        elapsed_ms = hw_elapsed.as_millis(),
                        sent,
                        "RGB hardware write slow"
                    );
                }
            }

            last_rgb_tick = Instant::now();
        }

        // Sleep for remaining poll interval (account for RGB timing)
        let elapsed = cycle_start.elapsed();
        let sleep_target = if rgb_enabled {
            poll_interval.min(rgb_interval)
        } else {
            poll_interval
        };
        if elapsed < sleep_target {
            std::thread::sleep(sleep_target - elapsed);
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
            Ok(HwCommand::SetRgbConfig { reply, .. }) => {
                let _ = reply.send(Err("Hardware unavailable".into()));
            }
            Ok(HwCommand::SetRgbEnabled { reply, .. }) => {
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
            Ok(HwCommand::SetRgbConfig { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
            Ok(HwCommand::SetRgbEnabled { reply, .. }) => {
                let _ = reply.send(Err("Control loop not running".into()));
            }
        }
    }
}

fn build_snapshot(
    control_loop: &mut ControlLoop,
    psu_handle: Option<&CorsairPsu>,
    last_good_psu: &Option<corsair_hid::PsuStatus>,
) -> Result<SystemSnapshot, String> {
    let cycle_result = control_loop.tick();
    let psu_status = corsair_sensors::lhm::read_psu_from_lhm()
        .and_then(validate_psu_status)
        .or_else(|| {
            psu_handle.and_then(|psu| psu.read_all().ok().and_then(validate_psu_status))
        })
        .or_else(|| last_good_psu.clone());
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

/// Convert the RGB config into renderer zone configs and apply.
/// Uses the device type map to assign correct LED layouts (fan ring vs linear strip).
fn apply_rgb_config(
    renderer: &mut RgbRenderer,
    config: &AppConfig,
    device_types: &std::collections::HashMap<(String, u8), (LinkDeviceType, u16)>,
) {
    let zones: Vec<ZoneConfig> = config
        .rgb
        .zones
        .iter()
        .map(|z| {
            let devices = z
                .devices
                .iter()
                .map(|d| {
                    let layout = match device_types.get(&(d.hub_serial.clone(), d.channel)) {
                        Some((LinkDeviceType::LinkAdapter | LinkDeviceType::LsStrip, led_count)) => {
                            LedLayout::LinearStrip { led_count: *led_count }
                        }
                        Some((_, led_count)) if *led_count > 0 => {
                            LedLayout::FanRing { led_count: *led_count }
                        }
                        _ => LedLayout::qx_fan(), // safe fallback
                    };
                    DeviceConfig {
                        hub_serial: d.hub_serial.clone(),
                        channel: d.channel,
                        layout,
                    }
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

fn save_config_to_disk(config: &AppConfig) -> Result<()> {
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
