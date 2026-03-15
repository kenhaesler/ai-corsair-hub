use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};

use corsair_common::config::{AppConfig, FanGroupConfig, FanMode, TempSourceConfig};
use corsair_common::CorsairDevice;
use corsair_hid::{DeviceScanner, FanSpeed, HubInfo, IcueLinkHub, LinkDeviceType};
use corsair_sensors::cpu::CpuSensor;
use corsair_sensors::gpu::GpuSensor;
use corsair_sensors::psu::{PsuReading, PsuSensor};
use corsair_sensors::TemperatureSource;

use crate::acoustic::AcousticFilter;
use crate::curve::FanCurve;
use crate::pid::PidController;

const FAILSAFE_DUTY: f64 = 70.0;
const EMERGENCY_DUTY: f64 = 100.0;
const CRITICAL_CPU_TEMP: f64 = 95.0;
const CRITICAL_GPU_TEMP: f64 = 90.0;
const SENSOR_STALE_TIMEOUT: Duration = Duration::from_secs(10);
const MIN_FAN_DUTY: f64 = 20.0;
const MIN_PUMP_DUTY: f64 = 50.0;

/// Result of one control cycle — sent to the frontend as a SystemSnapshot.
pub struct CycleResult {
    pub readings: HashMap<String, f64>,
    pub group_duties: Vec<GroupDutyReport>,
    pub emergency: bool,
    pub any_stale: bool,
    pub fan_speeds: Vec<(String, Vec<FanSpeed>)>,
    pub hub_health: Vec<HubHealthReport>,
}

/// Health status of a single hub after a control cycle.
pub struct HubHealthReport {
    pub serial: String,
    pub healthy: bool,
    pub consecutive_failures: u32,
}

/// Duty report for a single fan group after one cycle.
pub struct GroupDutyReport {
    pub name: String,
    pub hub_serial: String,
    pub channels: Vec<(u8, u8)>, // (channel, duty_u8)
}

/// RGB frame data for sending to hardware (no dependency on corsair-rgb crate).
pub struct RgbFrameRef<'a> {
    pub hub_serial: &'a str,
    pub channel: u8,
    pub leds: &'a [[u8; 3]],
}

pub struct ControlLoop {
    config: AppConfig,
    sensors: HashMap<String, Box<dyn TemperatureSource>>,
    sensor_state: HashMap<String, SensorState>,
    hubs: HashMap<String, HubConnection>,
    groups: Vec<FanGroup>,
    shutdown: Arc<AtomicBool>,
}

struct SensorState {
    last_value: f64,
    last_success: Instant,
    is_stale: bool,
}

struct HubConnection {
    hub: IcueLinkHub,
    #[allow(dead_code)]
    serial: String,
    healthy: bool,
    consecutive_failures: u32,
    last_recovery_attempt: Instant,
    pump_channels: Vec<u8>,
    info: HubInfo,
    color_logged: bool,
}

struct FanGroup {
    name: String,
    channels: Vec<u8>,
    hub_serial: String,
    controller: FanController,
    acoustic: Option<AcousticFilter>,
    #[allow(dead_code)]
    last_duty: f64,
}

enum FanController {
    Fixed(f64),
    Curve {
        curve: FanCurve,
        source: TempSourceConfig,
    },
    Pid {
        pid: PidController,
        source: TempSourceConfig,
    },
}

impl ControlLoop {
    /// Build from config. Discovers hubs, initializes sensors.
    /// The caller provides a DeviceScanner so it can be reused for device queries.
    pub fn build(
        config: AppConfig,
        shutdown: Arc<AtomicBool>,
        scanner: &DeviceScanner,
    ) -> Result<Self> {
        let mut sensors: HashMap<String, Box<dyn TemperatureSource>> = HashMap::new();
        let mut hubs: HashMap<String, HubConnection> = HashMap::new();

        let mut needed_serials = std::collections::HashSet::new();

        for group in &config.fan_groups {
            let serial = group
                .hub_serial
                .as_ref()
                .context(format!("Fan group '{}' missing hub_serial", group.name))?;
            needed_serials.insert(serial.clone());
        }

        // Always try to initialize all known sensors — makes them available
        // for presets and live config changes, not just what's in the current config.
        match CpuSensor::new() {
            Ok(cpu) => {
                info!("CPU sensor available: {}", cpu.name());
                sensors.insert("cpu".to_string(), Box::new(cpu));
            }
            Err(e) => warn!("CPU sensor unavailable: {}", e),
        }
        match GpuSensor::new() {
            Ok(gpu) => {
                info!("GPU sensor available: {}", gpu.name());
                sensors.insert("gpu".to_string(), Box::new(gpu));
            }
            Err(e) => warn!("GPU sensor unavailable: {}", e),
        }

        // Also try PSU sensors if referenced by any fan group
        let mut needed_sensors = std::collections::HashSet::new();
        for group in &config.fan_groups {
            match &group.mode {
                FanMode::Fixed { .. } => {}
                FanMode::Curve { temp_source, .. } | FanMode::Pid { temp_source, .. } => {
                    for s in &temp_source.sensors {
                        if s.starts_with("psu_") {
                            needed_sensors.insert(s.clone());
                        }
                    }
                }
            }
        }

        for sensor_name in &needed_sensors {
            match sensor_name.as_str() {
                "psu_vrm" | "psu_case" => {
                    // Only open PSU once for both readings
                    if !sensors.contains_key("psu_vrm") && !sensors.contains_key("psu_case") {
                        let psu_device = scanner
                            .open_device(
                                CorsairDevice::Hx1500i.pid(),
                                "", // Will match first PSU found
                                corsair_hid::CorsairPsu::data_interface(),
                            )
                            .or_else(|_| {
                                // Try scanning for any PSU
                                let groups = scanner.scan_grouped();
                                for g in &groups {
                                    if g.device_type == CorsairDevice::Hx1500i {
                                        return scanner.open_device(
                                            g.pid,
                                            &g.serial,
                                            corsair_hid::CorsairPsu::data_interface(),
                                        );
                                    }
                                }
                                bail!("No Corsair PSU found")
                            })
                            .context("Failed to open PSU device")?;

                        let serial = "psu".to_string();
                        let psu = corsair_hid::CorsairPsu::new(psu_device, serial);
                        psu.initialize().context("Failed to initialize PSU")?;

                        // We need to create separate PsuSensor wrappers.
                        // But PsuSensor takes ownership of CorsairPsu which isn't Clone.
                        // For now, only support one PSU reading at a time based on what's needed.
                        if needed_sensors.contains("psu_vrm") {
                            // Re-open for VRM
                            let groups = scanner.scan_grouped();
                            let psu_group = groups
                                .iter()
                                .find(|g| g.device_type == CorsairDevice::Hx1500i)
                                .context("No Corsair PSU found for psu_vrm")?;
                            let dev = scanner.open_device(
                                psu_group.pid,
                                &psu_group.serial,
                                corsair_hid::CorsairPsu::data_interface(),
                            )?;
                            let psu_vrm =
                                corsair_hid::CorsairPsu::new(dev, psu_group.serial.clone());
                            psu_vrm.initialize()?;
                            sensors.insert(
                                "psu_vrm".to_string(),
                                Box::new(PsuSensor::new(psu_vrm, PsuReading::Vrm)),
                            );
                        }
                        if needed_sensors.contains("psu_case") {
                            let groups = scanner.scan_grouped();
                            let psu_group = groups
                                .iter()
                                .find(|g| g.device_type == CorsairDevice::Hx1500i)
                                .context("No Corsair PSU found for psu_case")?;
                            let dev = scanner.open_device(
                                psu_group.pid,
                                &psu_group.serial,
                                corsair_hid::CorsairPsu::data_interface(),
                            )?;
                            let psu_case =
                                corsair_hid::CorsairPsu::new(dev, psu_group.serial.clone());
                            psu_case.initialize()?;
                            sensors.insert(
                                "psu_case".to_string(),
                                Box::new(PsuSensor::new(psu_case, PsuReading::Case)),
                            );
                        }
                    }
                }
                other => bail!("Unknown sensor type: '{}'", other),
            }
        }

        // Discover and initialize hubs
        for serial in &needed_serials {
            let hid_device = scanner
                .open_device(
                    CorsairDevice::IcueLinkHub.pid(),
                    serial,
                    IcueLinkHub::data_interface(),
                )
                .with_context(|| format!("Hub serial '{}' not found on USB bus", serial))?;

            let hub = IcueLinkHub::new(hid_device, serial.clone());
            let hub_info = hub
                .initialize()
                .with_context(|| format!("Failed to initialize hub '{}'", serial))?;

            let pump_channels: Vec<u8> = hub_info
                .devices
                .iter()
                .filter(|d| d.device_type.is_pump())
                .map(|d| d.channel)
                .collect();

            info!(
                serial = serial.as_str(),
                firmware = %hub_info.firmware,
                devices = hub_info.devices.len(),
                pumps = pump_channels.len(),
                "Hub initialized"
            );
            for dev in &hub_info.devices {
                let hub_leds = hub_info.led_counts.get(&dev.channel).copied();
                let effective = hub_leds.unwrap_or_else(|| dev.device_type.led_count());
                info!(
                    serial = serial.as_str(),
                    channel = dev.channel,
                    device_type = dev.device_type.name(),
                    type_leds = dev.device_type.led_count(),
                    hub_leds = ?hub_leds,
                    effective_leds = effective,
                    "  Device"
                );
            }

            hubs.insert(
                serial.clone(),
                HubConnection {
                    hub,
                    serial: serial.clone(),
                    healthy: true,
                    consecutive_failures: 0,
                    last_recovery_attempt: Instant::now(),
                    pump_channels,
                    info: hub_info,
                    color_logged: false,
                },
            );
        }

        // Build fan groups
        let mut groups = Vec::new();
        for group_cfg in &config.fan_groups {
            let group = build_fan_group(group_cfg, &sensors)?;
            groups.push(group);
        }

        if groups.is_empty() {
            warn!("No fan groups configured — control loop will run idle");
        }

        let sensor_state = sensors
            .keys()
            .map(|k| {
                (
                    k.clone(),
                    SensorState {
                        last_value: 0.0,
                        last_success: Instant::now(),
                        is_stale: true, // no reading yet
                    },
                )
            })
            .collect();

        Ok(Self {
            config,
            sensors,
            sensor_state,
            hubs,
            groups,
            shutdown,
        })
    }

    /// Execute one control cycle: poll sensors -> compute -> send to hubs.
    /// Returns a CycleResult for the frontend/logging.
    pub fn tick(&mut self) -> CycleResult {
        let dt_secs = self.config.general.poll_interval_ms as f64 / 1000.0;

        // 1. Poll all sensors
        let readings = self.poll_sensors();

        // 2. Check emergency override
        let emergency = self.check_emergency(&readings);

        // 3. Check sensor staleness
        let any_stale = self.sensor_state.values().any(|s| s.is_stale);

        // 4. Compute per-hub command batches
        let mut hub_commands: HashMap<String, Vec<(u8, u8)>> = HashMap::new();
        let mut group_duties = Vec::new();

        for group in &mut self.groups {
            let duty = if emergency {
                // Emergency: bypass everything
                if let Some(ref mut acoustic) = group.acoustic {
                    acoustic.override_duty(EMERGENCY_DUTY);
                }
                EMERGENCY_DUTY
            } else if any_stale {
                // Sensor stale: failsafe
                if let Some(ref mut acoustic) = group.acoustic {
                    acoustic.override_duty(FAILSAFE_DUTY);
                }
                FAILSAFE_DUTY
            } else {
                // Normal operation
                compute_group_duty(group, &readings, dt_secs)
            };

            // Enforce per-channel minimums and convert to u8
            let hub = self.hubs.get(&group.hub_serial);
            let pump_channels = hub.map(|h| &h.pump_channels);

            let commands = hub_commands.entry(group.hub_serial.clone()).or_default();
            let mut group_channels = Vec::new();
            for &ch in &group.channels {
                let is_pump = pump_channels
                    .map(|pcs| pcs.contains(&ch))
                    .unwrap_or(false);
                let min = if is_pump { MIN_PUMP_DUTY } else { MIN_FAN_DUTY };
                let final_duty = duty.max(min).round().clamp(0.0, 100.0) as u8;
                commands.push((ch, final_duty));
                group_channels.push((ch, final_duty));
            }

            group.last_duty = duty;

            group_duties.push(GroupDutyReport {
                name: group.name.clone(),
                hub_serial: group.hub_serial.clone(),
                channels: group_channels,
            });
        }

        // 5. Send commands to hubs
        for (serial, commands) in &hub_commands {
            if let Some(hub_conn) = self.hubs.get_mut(serial) {
                match hub_conn.hub.set_speeds(commands) {
                    Ok(()) => {
                        if hub_conn.consecutive_failures > 0 {
                            info!(
                                serial = serial.as_str(),
                                prev_failures = hub_conn.consecutive_failures,
                                "Hub recovered — set_speeds succeeded"
                            );
                        }
                        hub_conn.healthy = true;
                        hub_conn.consecutive_failures = 0;
                    }
                    Err(e) => {
                        hub_conn.consecutive_failures += 1;
                        hub_conn.healthy = false;
                        error!(
                            serial = serial.as_str(),
                            consecutive = hub_conn.consecutive_failures,
                            error = %e,
                            "Failed to set fan speeds"
                        );
                    }
                }
            }
        }

        // 6. Read back fan speeds from all hubs (always attempt — serves as keepalive)
        let mut fan_speeds = Vec::new();
        for (serial, hub_conn) in &self.hubs {
            match hub_conn.hub.get_speeds() {
                Ok(speeds) => fan_speeds.push((serial.clone(), speeds)),
                Err(e) => {
                    warn!(serial = serial.as_str(), error = %e, "Failed to read fan speeds");
                }
            }
        }

        // 7. Build hub health reports
        let hub_health: Vec<HubHealthReport> = self
            .hubs
            .iter()
            .map(|(serial, conn)| HubHealthReport {
                serial: serial.clone(),
                healthy: conn.healthy,
                consecutive_failures: conn.consecutive_failures,
            })
            .collect();

        CycleResult {
            readings,
            group_duties,
            emergency,
            any_stale,
            fan_speeds,
            hub_health,
        }
    }

    /// Current configuration.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Names of sensors that are currently initialized and available.
    pub fn available_sensors(&self) -> Vec<String> {
        self.sensors.keys().cloned().collect()
    }

    /// Replace config and rebuild fan groups (for live config reload).
    pub fn update_config(&mut self, config: AppConfig) -> Result<()> {
        let mut groups = Vec::new();
        for group_cfg in &config.fan_groups {
            let group = build_fan_group(group_cfg, &self.sensors)?;
            groups.push(group);
        }
        self.config = config;
        self.groups = groups;
        Ok(())
    }

    /// Return cached hub info and current fan speeds for the device tree.
    /// Uses existing initialized handles — no competing USB access.
    pub fn hub_snapshots(&self) -> Vec<(String, HubInfo, Vec<FanSpeed>)> {
        let mut result = Vec::new();
        for (serial, hub_conn) in &self.hubs {
            let speeds = if hub_conn.healthy {
                hub_conn.hub.get_speeds().unwrap_or_default()
            } else {
                Vec::new()
            };
            result.push((serial.clone(), hub_conn.info.clone(), speeds));
        }
        result
    }

    /// Return a map of (hub_serial, channel) → (device_type, effective_led_count)
    /// for all enumerated devices across all hubs. Uses the 0x1d LED count table
    /// when available, otherwise falls back to `device_type.led_count()`.
    pub fn device_type_map(&self) -> HashMap<(String, u8), (LinkDeviceType, u16)> {
        let mut map = HashMap::new();
        for (serial, hub_conn) in &self.hubs {
            for dev in &hub_conn.info.devices {
                let effective_leds = hub_conn
                    .info
                    .led_counts
                    .get(&dev.channel)
                    .copied()
                    .filter(|&c| c > 0)
                    .unwrap_or_else(|| dev.device_type.led_count());
                map.insert(
                    (serial.clone(), dev.channel),
                    (dev.device_type.clone(), effective_leds),
                );
            }
        }
        map
    }

    /// Set manual duty on a specific hub using its existing initialized handle.
    pub fn set_manual_duty(&self, hub_serial: &str, channels: &[u8], duty: u8) -> Result<()> {
        let hub_conn = self
            .hubs
            .get(hub_serial)
            .with_context(|| format!("Hub '{}' not managed by control loop", hub_serial))?;
        let targets: Vec<(u8, u8)> = channels.iter().map(|&ch| (ch, duty)).collect();
        hub_conn.hub.set_speeds(&targets)?;
        Ok(())
    }

    /// Return serials of hubs with 5+ consecutive failures and 10s+ since last recovery attempt.
    pub fn hubs_needing_recovery(&self) -> Vec<String> {
        const FAILURE_THRESHOLD: u32 = 5;
        const RECOVERY_COOLDOWN: Duration = Duration::from_secs(10);

        self.hubs
            .iter()
            .filter(|(_, conn)| {
                conn.consecutive_failures >= FAILURE_THRESHOLD
                    && conn.last_recovery_attempt.elapsed() >= RECOVERY_COOLDOWN
            })
            .map(|(serial, _)| serial.clone())
            .collect()
    }

    /// Swap in a freshly initialized hub after successful recovery.
    pub fn replace_hub(&mut self, serial: &str, new_hub: IcueLinkHub, new_info: HubInfo) {
        if let Some(conn) = self.hubs.get_mut(serial) {
            let pump_channels: Vec<u8> = new_info
                .devices
                .iter()
                .filter(|d| d.device_type.is_pump())
                .map(|d| d.channel)
                .collect();

            conn.hub = new_hub;
            conn.info = new_info;
            conn.healthy = true;
            conn.consecutive_failures = 0;
            conn.last_recovery_attempt = Instant::now();
            conn.pump_channels = pump_channels;
            conn.color_logged = false;

            info!(serial, "Hub handle replaced after recovery");
        }
    }

    /// Mark that a recovery attempt was made (even if it failed) to enforce cooldown.
    pub fn mark_recovery_attempted(&mut self, serial: &str) {
        if let Some(conn) = self.hubs.get_mut(serial) {
            conn.last_recovery_attempt = Instant::now();
        }
    }

    /// Restore all hubs to hardware control mode.
    pub fn shutdown_hardware(&mut self) {
        info!("Restoring hardware mode on all hubs");
        for (serial, hub_conn) in &mut self.hubs {
            if let Err(e) = hub_conn.hub.enter_hardware_mode() {
                warn!(serial = serial.as_str(), error = %e, "Failed to restore hardware mode");
            }
        }
    }

    /// Send RGB frames to hardware hubs.
    ///
    /// The iCUE LINK protocol requires a flat buffer covering ALL enumerated devices
    /// on each hub, in ascending channel order. The hub parses the buffer using each
    /// device's actual LED count. We must match those counts exactly, or subsequent
    /// devices receive misaligned data.
    ///
    /// This method uses the hub's cached device enumeration to build a correctly-sized
    /// buffer for each hub: truncating/padding frame data to match the actual LED count,
    /// and inserting black for devices without frames.
    pub fn send_rgb_frames(&mut self, frames: &[RgbFrameRef]) -> usize {
        use std::collections::BTreeMap;

        const BLACK: [u8; 3] = [0, 0, 0];

        // Group frames by hub serial, sort by channel within each hub
        let mut by_hub: HashMap<&str, BTreeMap<u8, &[[u8; 3]]>> = HashMap::new();
        for frame in frames {
            by_hub
                .entry(&frame.hub_serial)
                .or_default()
                .insert(frame.channel, &frame.leds);
        }

        let mut sent = 0;
        for (serial, frame_channels) in &by_hub {
            let hub_conn = match self.hubs.get_mut(*serial) {
                Some(c) if c.healthy => c,
                _ => continue,
            };

            // Build LED arrays for ALL enumerated devices, using the hub's actual LED counts.
            // The hub parses the flat buffer sequentially using each device's known LED count.
            // We MUST match those counts exactly. The hub's LED count table (read via 0x1d)
            // is authoritative — it accounts for LINK Adapters with strips, etc.
            let mut device_leds: Vec<(u8, Vec<[u8; 3]>)> = Vec::new();

            for dev in &hub_conn.info.devices {
                // Use hub firmware's LED count (from 0x1d table) if available,
                // otherwise fall back to device type default.
                // The 0x1d table is authoritative for LINK Adapters with connected strips.
                let actual_count = hub_conn
                    .info
                    .led_counts
                    .get(&dev.channel)
                    .copied()
                    .map(|c| c as usize)
                    .filter(|&c| c > 0)
                    .unwrap_or(dev.device_type.led_count() as usize);

                if actual_count == 0 {
                    continue; // truly no LEDs
                }

                let frame_data = frame_channels.get(&dev.channel);

                let leds = if let Some(data) = frame_data {
                    // Truncate or pad to actual LED count
                    let mut buf = Vec::with_capacity(actual_count);
                    for i in 0..actual_count {
                        buf.push(data.get(i).copied().unwrap_or(BLACK));
                    }
                    buf
                } else {
                    // No frame for this device: fill with black
                    vec![BLACK; actual_count]
                };

                device_leds.push((dev.channel, leds));
            }

            let refs: Vec<(u8, &[[u8; 3]])> = device_leds
                .iter()
                .map(|(ch, leds)| (*ch, leds.as_slice()))
                .collect();

            if !hub_conn.color_logged {
                let total_bytes: usize = refs.iter().map(|(_, leds)| leds.len() * 3).sum();
                let channels: Vec<(u8, usize)> = refs.iter().map(|(ch, leds)| (*ch, leds.len())).collect();
                info!(
                    serial = *serial,
                    devices = refs.len(),
                    total_bytes,
                    channels = ?channels,
                    "RGB first write"
                );
                hub_conn.color_logged = true;
            }

            match hub_conn.hub.set_rgb(&refs) {
                Ok(()) => sent += refs.len(),
                Err(e) => {
                    warn!(
                        serial = *serial,
                        error = %e,
                        "RGB write failed (non-fatal)"
                    );
                }
            }
        }
        sent
    }

    /// Run the control loop until shutdown signal.
    pub fn run(&mut self) -> Result<()> {
        let poll_interval = Duration::from_millis(self.config.general.poll_interval_ms);

        info!(
            interval_ms = self.config.general.poll_interval_ms,
            groups = self.groups.len(),
            hubs = self.hubs.len(),
            sensors = self.sensors.len(),
            "Control loop starting"
        );

        while !self.shutdown.load(Ordering::Relaxed) {
            let cycle_start = Instant::now();

            let result = self.tick();
            self.log_status(&result.readings);

            // Sleep for remaining time
            let elapsed = cycle_start.elapsed();
            if elapsed < poll_interval {
                std::thread::sleep(poll_interval - elapsed);
            }
        }

        // Shutdown: restore hardware mode on all hubs
        self.shutdown_hardware();

        Ok(())
    }

    fn poll_sensors(&mut self) -> HashMap<String, f64> {
        let mut readings = HashMap::new();

        for (name, sensor) in &self.sensors {
            match sensor.read() {
                Ok(temp) => {
                    readings.insert(name.clone(), temp.celsius);
                    if let Some(state) = self.sensor_state.get_mut(name) {
                        state.last_value = temp.celsius;
                        state.last_success = Instant::now();
                        state.is_stale = false;
                    }
                }
                Err(e) => {
                    warn!(sensor = name.as_str(), error = %e, "Sensor read failed");
                    if let Some(state) = self.sensor_state.get_mut(name) {
                        if state.last_success.elapsed() > SENSOR_STALE_TIMEOUT {
                            state.is_stale = true;
                            warn!(sensor = name.as_str(), "Sensor data stale (>10s)");
                        } else {
                            // Use last known good value
                            readings.insert(name.clone(), state.last_value);
                        }
                    }
                }
            }
        }

        readings
    }

    fn check_emergency(&self, readings: &HashMap<String, f64>) -> bool {
        if let Some(&cpu) = readings.get("cpu") {
            if cpu >= CRITICAL_CPU_TEMP {
                error!(temp = cpu, threshold = CRITICAL_CPU_TEMP, "CPU CRITICAL — emergency override");
                return true;
            }
        }
        if let Some(&gpu) = readings.get("gpu") {
            if gpu >= CRITICAL_GPU_TEMP {
                error!(temp = gpu, threshold = CRITICAL_GPU_TEMP, "GPU CRITICAL — emergency override");
                return true;
            }
        }
        false
    }

    fn log_status(&self, readings: &HashMap<String, f64>) {
        let now = chrono_time();
        let mut parts = vec![format!("[{}]", now)];

        if let Some(&cpu) = readings.get("cpu") {
            parts.push(format!("CPU: {:.1}C", cpu));
        }
        if let Some(&gpu) = readings.get("gpu") {
            parts.push(format!("GPU: {:.1}C", gpu));
        }

        parts.push("|".to_string());

        for group in &self.groups {
            let duty = if let Some(ref acoustic) = group.acoustic {
                acoustic.current_duty()
            } else {
                match &group.controller {
                    FanController::Fixed(d) => *d,
                    _ => group.last_duty,
                }
            };
            parts.push(format!("{}: {:.0}%", group.name, duty));
        }

        info!("{}", parts.join("  "));
    }
}

/// Compute the duty for a fan group in normal operation.
fn compute_group_duty(
    group: &mut FanGroup,
    readings: &HashMap<String, f64>,
    dt_secs: f64,
) -> f64 {
    match &mut group.controller {
        FanController::Fixed(duty) => *duty,
        FanController::Curve { curve, source } => {
            let temp = compute_weighted_temp(source, readings);
            match temp {
                Some(t) => {
                    let raw = curve.evaluate(t);
                    apply_acoustic_filter(&mut group.acoustic, raw, t, dt_secs)
                }
                None => FAILSAFE_DUTY,
            }
        }
        FanController::Pid { pid, source } => {
            let temp = compute_weighted_temp(source, readings);
            match temp {
                Some(t) => {
                    let raw = pid.update(t);
                    apply_acoustic_filter(&mut group.acoustic, raw, t, dt_secs)
                }
                None => FAILSAFE_DUTY,
            }
        }
    }
}

fn apply_acoustic_filter(
    acoustic: &mut Option<AcousticFilter>,
    raw_duty: f64,
    temp: f64,
    dt_secs: f64,
) -> f64 {
    match acoustic {
        Some(filter) => filter.update(raw_duty, temp, dt_secs),
        None => raw_duty,
    }
}

/// Weighted average of sensor readings.
pub fn compute_weighted_temp(
    source: &TempSourceConfig,
    readings: &HashMap<String, f64>,
) -> Option<f64> {
    let mut total_weight = 0.0;
    let mut weighted_sum = 0.0;

    for (i, sensor_name) in source.sensors.iter().enumerate() {
        let weight = source.weights.get(i).copied().unwrap_or(1.0);
        if let Some(&temp) = readings.get(sensor_name) {
            weighted_sum += temp * weight;
            total_weight += weight;
        }
    }

    if total_weight > 0.0 {
        Some(weighted_sum / total_weight)
    } else {
        None
    }
}

fn build_fan_group(
    cfg: &FanGroupConfig,
    sensors: &HashMap<String, Box<dyn TemperatureSource>>,
) -> Result<FanGroup> {
    let hub_serial = cfg
        .hub_serial
        .clone()
        .context(format!("Fan group '{}' missing hub_serial", cfg.name))?;

    let (controller, acoustic) = match &cfg.mode {
        FanMode::Fixed { duty_percent } => (FanController::Fixed(*duty_percent), None),
        FanMode::Curve {
            points,
            hysteresis,
            ramp_rate,
            temp_source,
        } => {
            // Warn about missing sensors but continue — will use failsafe duty
            for s in &temp_source.sensors {
                if !sensors.contains_key(s) {
                    warn!(
                        "Fan group '{}' references unavailable sensor '{}' — will use failsafe duty until sensor comes online",
                        cfg.name, s
                    );
                }
            }
            let curve = FanCurve::new(points.clone())?;
            let ramp_down = ramp_rate * 0.4;
            let filter = AcousticFilter::new(*ramp_rate, ramp_down, *hysteresis);
            (
                FanController::Curve {
                    curve,
                    source: temp_source.clone(),
                },
                Some(filter),
            )
        }
        FanMode::Pid {
            target_temp,
            kp,
            ki,
            kd,
            min_duty,
            max_duty,
            temp_source,
        } => {
            for s in &temp_source.sensors {
                if !sensors.contains_key(s) {
                    warn!(
                        "Fan group '{}' references unavailable sensor '{}' — will use failsafe duty until sensor comes online",
                        cfg.name, s
                    );
                }
            }
            let pid = PidController::new(*kp, *ki, *kd, *target_temp)
                .with_output_limits(*min_duty, *max_duty);
            let filter = AcousticFilter::new(10.0, 3.0, 2.0);
            (
                FanController::Pid {
                    pid,
                    source: temp_source.clone(),
                },
                Some(filter),
            )
        }
    };

    Ok(FanGroup {
        name: cfg.name.clone(),
        channels: cfg.channels.clone(),
        hub_serial,
        controller,
        acoustic,
        last_duty: 0.0,
    })
}

/// Simple HH:MM:SS timestamp (no chrono dependency — use std).
fn chrono_time() -> String {
    // Use elapsed from a reference point. For a proper timestamp we'd need chrono,
    // but for a status line, system time via std is fine.
    let now = std::time::SystemTime::now();
    let since_midnight = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86400;
    let h = since_midnight / 3600;
    let m = (since_midnight % 3600) / 60;
    let s = since_midnight % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

// --- Config loading ---

/// Load and validate an AppConfig from a TOML file.
pub fn load_config(path: &Path) -> Result<AppConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read config: {}", path.display()))?;
    let config: AppConfig =
        toml::from_str(&contents).context("Invalid TOML config")?;
    validate_config(&config)?;
    Ok(config)
}

/// Validate config constraints.
pub fn validate_config(config: &AppConfig) -> Result<()> {
    // Poll interval bounds
    if config.general.poll_interval_ms < 100 || config.general.poll_interval_ms > 10000 {
        bail!(
            "poll_interval_ms must be 100..10000, got {}",
            config.general.poll_interval_ms
        );
    }

    let mut group_names = std::collections::HashSet::new();

    for group in &config.fan_groups {
        // No duplicate group names
        if !group_names.insert(&group.name) {
            bail!("Duplicate fan group name: '{}'", group.name);
        }

        // Must have hub_serial
        if group.hub_serial.is_none() {
            bail!("Fan group '{}' missing hub_serial", group.name);
        }

        // Must have at least one channel
        if group.channels.is_empty() {
            bail!("Fan group '{}' has no channels", group.name);
        }

        // Mode-specific validation
        match &group.mode {
            FanMode::Fixed { duty_percent } => {
                if *duty_percent < 0.0 || *duty_percent > 100.0 {
                    bail!(
                        "Fan group '{}': duty_percent must be 0..100, got {}",
                        group.name,
                        duty_percent
                    );
                }
            }
            FanMode::Curve {
                points,
                temp_source,
                ..
            } => {
                if points.len() < 2 {
                    bail!(
                        "Fan group '{}': curve requires at least 2 points, got {}",
                        group.name,
                        points.len()
                    );
                }
                for p in points {
                    if p.duty < 0.0 || p.duty > 100.0 {
                        bail!(
                            "Fan group '{}': curve duty must be 0..100, got {}",
                            group.name,
                            p.duty
                        );
                    }
                }
                validate_temp_source(&group.name, temp_source)?;
            }
            FanMode::Pid {
                kp,
                ki,
                kd,
                min_duty,
                max_duty,
                temp_source,
                ..
            } => {
                if *kp <= 0.0 {
                    bail!("Fan group '{}': kp must be > 0", group.name);
                }
                if *ki < 0.0 {
                    bail!("Fan group '{}': ki must be >= 0", group.name);
                }
                if *kd < 0.0 {
                    bail!("Fan group '{}': kd must be >= 0", group.name);
                }
                if *min_duty >= *max_duty {
                    bail!(
                        "Fan group '{}': min_duty ({}) must be < max_duty ({})",
                        group.name,
                        min_duty,
                        max_duty
                    );
                }
                validate_temp_source(&group.name, temp_source)?;
            }
        }
    }

    Ok(())
}

fn validate_temp_source(group_name: &str, source: &TempSourceConfig) -> Result<()> {
    if source.sensors.is_empty() {
        bail!("Fan group '{}': temp_source has no sensors", group_name);
    }
    if source.sensors.len() != source.weights.len() {
        bail!(
            "Fan group '{}': sensors count ({}) != weights count ({})",
            group_name,
            source.sensors.len(),
            source.weights.len()
        );
    }
    for w in &source.weights {
        if *w <= 0.0 {
            bail!(
                "Fan group '{}': all weights must be > 0, got {}",
                group_name,
                w
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use corsair_common::config::*;

    fn valid_config() -> AppConfig {
        AppConfig {
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: vec![FanGroupConfig {
                name: "test".to_string(),
                channels: vec![1, 2],
                hub_serial: Some("ABCD1234".to_string()),
                mode: FanMode::Fixed { duty_percent: 50.0 },
            }],
            rgb: Default::default(),
        }
    }

    #[test]
    fn test_weighted_temp_calculation() {
        let source = TempSourceConfig {
            sensors: vec!["cpu".to_string(), "gpu".to_string()],
            weights: vec![0.7, 0.3],
        };
        let mut readings = HashMap::new();
        readings.insert("cpu".to_string(), 50.0);
        readings.insert("gpu".to_string(), 60.0);

        let result = compute_weighted_temp(&source, &readings).unwrap();
        // (50*0.7 + 60*0.3) / (0.7+0.3) = (35+18)/1.0 = 53.0
        assert!((result - 53.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_weighted_temp_missing_sensor() {
        let source = TempSourceConfig {
            sensors: vec!["cpu".to_string(), "gpu".to_string()],
            weights: vec![0.7, 0.3],
        };
        let mut readings = HashMap::new();
        readings.insert("cpu".to_string(), 50.0);
        // gpu missing — weight redistributed

        let result = compute_weighted_temp(&source, &readings).unwrap();
        // Only cpu available: 50*0.7 / 0.7 = 50.0
        assert!((result - 50.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_weighted_temp_all_missing() {
        let source = TempSourceConfig {
            sensors: vec!["cpu".to_string(), "gpu".to_string()],
            weights: vec![0.7, 0.3],
        };
        let readings = HashMap::new();

        let result = compute_weighted_temp(&source, &readings);
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_config_valid() {
        let config = valid_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_missing_serial() {
        let mut config = valid_config();
        config.fan_groups[0].hub_serial = None;
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_empty_channels() {
        let mut config = valid_config();
        config.fan_groups[0].channels = vec![];
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_bad_curve() {
        let mut config = valid_config();
        config.fan_groups[0].mode = FanMode::Curve {
            points: vec![CurvePoint {
                temp: 50.0,
                duty: 50.0,
            }],
            hysteresis: 3.0,
            ramp_rate: 5.0,
            temp_source: TempSourceConfig {
                sensors: vec!["cpu".to_string()],
                weights: vec![1.0],
            },
        };
        assert!(validate_config(&config).is_err());
    }
}
