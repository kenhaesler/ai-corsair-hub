use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};

use corsair_common::config::{AppConfig, FanGroupConfig, FanMode, TempSourceConfig};
use corsair_common::identity::{DeviceEnumEntry, DeviceRegistry};
use corsair_common::CorsairDevice;
use corsair_hid::{DeviceScanner, FanSpeed, HubInfo, IcueLinkHub, IcueLinkTransport, LinkDeviceType};
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
/// B3: sensor reading is considered stale after this duration of no success.
/// Tightened from 10s to 5s so the failsafe path engages sooner when LHM or
/// NVML stop producing readings (prevents the controller from running on
/// arithmetic on an old frozen value for the full 10s window).
const SENSOR_STALE_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_FAN_DUTY: f64 = 20.0;
const MIN_PUMP_DUTY: f64 = 50.0;
/// B5: bounded wall-clock timeout for `enter_hardware_mode()` during shutdown.
/// If a hub is wedged (unplugged mid-operation, firmware hang) we abandon the
/// call rather than blocking the UI thread forever. 5s is well above the
/// normal ~50ms success path but below any user-visible "app won't quit" point.
const SHUTDOWN_HARDWARE_MODE_TIMEOUT: Duration = Duration::from_secs(5);
/// B2: exponential backoff base / cap for proactive hub recovery. First retry
/// after a healthy→failed transition: ~10s; doubles each attempt up to 160s,
/// then clamped to RECOVERY_BACKOFF_MAX.
const RECOVERY_BACKOFF_BASE: Duration = Duration::from_secs(10);
const RECOVERY_BACKOFF_MAX: Duration = Duration::from_secs(120);

/// B2: compute the minimum cooldown between recovery attempts as a function
/// of how many attempts have already been made. Pure function, exposed for
/// testing.
fn recovery_backoff(attempts: u32) -> Duration {
    // Cap the exponent at 4 (2^4 = 16x) so we don't overflow for pathological
    // hub states. 10s * 16 = 160s, then clamped to RECOVERY_BACKOFF_MAX (120s).
    let exp = attempts.min(4);
    let scaled = RECOVERY_BACKOFF_BASE
        .checked_mul(1u32 << exp)
        .unwrap_or(RECOVERY_BACKOFF_MAX);
    std::cmp::min(scaled, RECOVERY_BACKOFF_MAX)
}

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

pub struct ControlLoop {
    config: AppConfig,
    sensors: HashMap<String, Box<dyn TemperatureSource>>,
    sensor_state: HashMap<String, SensorState>,
    hubs: HashMap<String, HubConnection>,
    groups: Vec<FanGroup>,
    /// Current device_id ↔ (hub_serial, channel) index. Rebuilt at every
    /// hub init and every successful hub recovery. Used by the dual-key
    /// path in `set_speeds` bucketing and `send_rgb_frames` — a fan_group
    /// with non-empty `device_ids` resolves through the registry;
    /// otherwise the legacy channel path runs.
    registry: DeviceRegistry,
    /// Orphan device_ids already warned about this process lifetime. Step 5
    /// invariant: `send_rgb_frames` receives frames keyed by device_id only;
    /// a device_id not in the registry means the config references a device
    /// that isn't enumerated this boot (unplugged, moved elsewhere). We log
    /// once per boot per device_id — repeating every tick at 30 FPS would
    /// flood the log. The set is cleared on `update_config` and on
    /// `replace_hub` so a device that reappears gets a fresh warning budget
    /// if it disappears again later.
    logged_orphan_rgb_ids: HashSet<String>,
    shutdown: Arc<AtomicBool>,
}

struct SensorState {
    last_value: f64,
    last_success: Instant,
    is_stale: bool,
}

struct HubConnection {
    /// Concrete transport behind a trait object so tests can inject a mock hub
    /// without a real USB device. `Arc` makes it cheap to clone into a worker
    /// thread (used by shutdown_hardware's bounded-timeout path).
    hub: Arc<dyn IcueLinkTransport>,
    #[allow(dead_code)]
    serial: String,
    healthy: bool,
    consecutive_failures: u32,
    last_recovery_attempt: Instant,
    /// B2: number of recovery attempts made for this hub since it last went
    /// healthy. Drives the exponential backoff in `recovery_backoff()`.
    /// Reset to 0 when `set_speeds` succeeds.
    recovery_attempts: u32,
    pump_channels: Vec<u8>,
    info: HubInfo,
    color_logged: bool,
}

struct FanGroup {
    name: String,
    /// V1 identity: channels on `hub_serial`. Authoritative when
    /// `device_ids` is empty (V1 config).
    channels: Vec<u8>,
    /// V1 identity: hub serial the channels belong to. Ignored when
    /// `device_ids` is non-empty.
    hub_serial: String,
    /// V2 identity: stable device_ids. Authoritative when non-empty.
    /// Resolved to (hub_serial, channel) at each cycle via the
    /// ControlLoop's registry.
    device_ids: Vec<String>,
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

        // V2 configs don't have hub_serial on fan groups. In that case we
        // can't pre-compute which hubs to open from the config alone — we
        // open every currently-enumerated iCUE LINK hub instead. V1 configs
        // opt in to the per-group hub_serial path below.
        for group in &config.fan_groups {
            if let Some(serial) = group.hub_serial.as_ref() {
                needed_serials.insert(serial.clone());
            }
        }
        if config.is_v2() {
            // V2: enumerate all iCUE LINK hubs on the bus so the registry
            // can resolve any device_id the config references.
            for group in scanner.scan_grouped() {
                if group.device_type == CorsairDevice::IcueLinkHub {
                    needed_serials.insert(group.serial.clone());
                }
            }
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
            let mut hub_info = hub
                .initialize()
                .with_context(|| format!("Failed to initialize hub '{}'", serial))?;

            // Apply user-configured device overrides. These take precedence over
            // both the hub's 0x1d LED-count table and the device-type defaults,
            // so users can fix misenumeration without waiting for a code change.
            //
            // V1 overrides are keyed by (hub_serial, channel). V2 per-device
            // entries are keyed by device_id and applied below, after we've
            // seen the hub's enumeration. V2 wins when both reference the
            // same physical device (which shouldn't happen post-migration).
            let mut overridden_channels: Vec<u8> = Vec::new();
            for ov in &config.device_overrides {
                if ov.hub_serial == *serial {
                    hub_info.led_counts.insert(ov.channel, ov.led_count);
                    overridden_channels.push(ov.channel);
                }
            }
            if !overridden_channels.is_empty() {
                info!(
                    serial = serial.as_str(),
                    channels = ?overridden_channels,
                    "Applied config device_overrides"
                );
            }

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
                let overridden = overridden_channels.contains(&dev.channel);
                info!(
                    serial = serial.as_str(),
                    channel = dev.channel,
                    device_type = dev.device_type.name(),
                    model = format!("0x{:02X}", dev.model),
                    device_id = dev.device_id.as_str(),
                    type_leds = dev.device_type.led_count(),
                    hub_leds = ?hub_leds,
                    effective_leds = effective,
                    overridden,
                    "  Device"
                );
            }

            hubs.insert(
                serial.clone(),
                HubConnection {
                    hub: Arc::new(hub),
                    serial: serial.clone(),
                    healthy: true,
                    consecutive_failures: 0,
                    last_recovery_attempt: Instant::now(),
                    recovery_attempts: 0,
                    pump_channels,
                    info: hub_info,
                    color_logged: false,
                },
            );
        }

        // Build the identity registry from every enumerated device across
        // every hub. This is the runtime bridge between device_id (what
        // persists) and (hub_serial, channel) (what the wire speaks).
        let registry = build_registry(&hubs);

        // Apply V2 per-device led_count overrides now that we have both the
        // registry and mutable HubConnections. V2 takes precedence over the
        // V1 (hub_serial, channel) overrides applied earlier: a user-
        // specified led_count on a device_id wins.
        for entry in &config.devices {
            let Some(override_leds) = entry.led_count else {
                continue;
            };
            let Some(loc) = registry.resolve(&entry.device_id) else {
                warn!(
                    device_id = entry.device_id.as_str(),
                    "V2 device entry references device_id not currently enumerated — \
                     led_count override deferred until device reappears"
                );
                continue;
            };
            if let Some(hub_conn) = hubs.get_mut(&loc.hub_serial) {
                hub_conn.info.led_counts.insert(loc.channel, override_leds);
                info!(
                    device_id = entry.device_id.as_str(),
                    hub_serial = loc.hub_serial.as_str(),
                    channel = loc.channel,
                    led_count = override_leds,
                    "Applied V2 per-device led_count override"
                );
            }
        }

        // Build fan groups. Each group carries both (channels, hub_serial)
        // for the V1 path and `device_ids` for the V2 path; the cycle code
        // picks whichever is non-empty.
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
            registry,
            logged_orphan_rgb_ids: HashSet::new(),
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

        // B3: reset PID integral on any-stale cycle so the controller doesn't
        // carry accumulated windup into the post-recovery cycles. Without this,
        // a long stale period followed by a hot CPU temp on sensor recovery
        // causes a duty spike: the integral accumulated while the sensor was
        // frozen drives output high, then the proportional term piles on too.
        // Resetting is safe because we're entering the failsafe duty path this
        // tick anyway — the PID output isn't used while stale, and on the
        // recovery tick we want to start from clean state.
        if any_stale {
            for group in &mut self.groups {
                if let FanController::Pid { pid, .. } = &mut group.controller {
                    pid.reset_integral();
                }
            }
        }

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

            // Enforce per-channel minimums and convert to u8.
            //
            // Dual-key bucketing: if the group has `device_ids` (V2 path),
            // resolve each id through the registry to its current
            // (hub_serial, channel) and bucket by the resolved hub. For
            // orphans (device_ids not currently enumerated) we skip cleanly
            // — the group simply doesn't drive that fan this cycle; it
            // rejoins once the registry sees the device again. V1 groups
            // (empty device_ids) fall through to the original
            // channel-based path.
            //
            // `group_duties` reports are grouped per (resolved) hub_serial
            // so the snapshot continues to tag RPMs correctly. A V2 group
            // may span multiple hubs; we emit one report per hub the group
            // touched this cycle.
            let mut per_hub_reports: HashMap<String, Vec<(u8, u8)>> = HashMap::new();

            if !group.device_ids.is_empty() {
                // V2 path: resolve via registry.
                for device_id in &group.device_ids {
                    let Some(loc) = self.registry.resolve(device_id) else {
                        // Orphan: config references a device not currently
                        // enumerated. Skip silently each cycle — warning-
                        // once could be added later but would add a
                        // seen-set to the FanGroup.
                        continue;
                    };
                    let hub = self.hubs.get(&loc.hub_serial);
                    let pump_channels = hub.map(|h| &h.pump_channels);
                    let is_pump = pump_channels
                        .map(|pcs| pcs.contains(&loc.channel))
                        .unwrap_or(false);
                    let min = if is_pump { MIN_PUMP_DUTY } else { MIN_FAN_DUTY };
                    let final_duty = duty.max(min).round().clamp(0.0, 100.0) as u8;

                    hub_commands
                        .entry(loc.hub_serial.clone())
                        .or_default()
                        .push((loc.channel, final_duty));
                    per_hub_reports
                        .entry(loc.hub_serial.clone())
                        .or_default()
                        .push((loc.channel, final_duty));
                }
            } else {
                // V1 path: channels on a single hub_serial.
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
                per_hub_reports.insert(group.hub_serial.clone(), group_channels);
            }

            group.last_duty = duty;

            // Emit one GroupDutyReport per hub the group touched. For V1
            // groups that's always a single report on group.hub_serial.
            // For V2 groups spanning multiple hubs, we emit a report per
            // hub — the snapshot builder keys duty by (hub_serial, channel)
            // which makes this unambiguous.
            for (hub_serial, group_channels) in per_hub_reports {
                group_duties.push(GroupDutyReport {
                    name: group.name.clone(),
                    hub_serial,
                    channels: group_channels,
                });
            }
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
                        // B2: recovery complete — reset backoff so the next
                        // failure gets immediate attention, not delayed.
                        hub_conn.recovery_attempts = 0;
                    }
                    Err(e) => {
                        let was_healthy = hub_conn.healthy;
                        hub_conn.consecutive_failures += 1;
                        hub_conn.healthy = false;
                        error!(
                            serial = serial.as_str(),
                            consecutive = hub_conn.consecutive_failures,
                            error = %e,
                            "Failed to set fan speeds"
                        );

                        // B2: on the healthy→failed transition, reset the
                        // recovery_attempts counter so the next
                        // `hubs_needing_recovery()` call schedules an immediate
                        // first attempt (no backoff gate at attempts == 0).
                        // Subsequent attempts go through the exponential
                        // backoff curve; `recovery_attempts` is bumped by
                        // `mark_recovery_attempted()` as attempts are spent.
                        if was_healthy {
                            hub_conn.recovery_attempts = 0;
                        }
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

    /// Current device identity registry. Used by callers that need to
    /// resolve (hub_serial, channel) → device_id at config-expansion time
    /// (notably `apply_rgb_config` in the GUI, which pairs V1
    /// `RgbDeviceRef` entries with their stable device_id before handing
    /// zone configs to the renderer).
    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
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
        // Re-apply V2 per-device led_count overrides. A V1→V2 migration or a
        // preset swap may have changed them. Restore the base hub
        // led_counts from the previous pass is not necessary here because
        // the hub firmware 0x1d table would require re-enumeration to
        // refresh — the user-visible led_counts in HubConnection.info
        // already reflect the prior overrides, and we simply lay the new
        // ones on top.
        for entry in &config.devices {
            if let Some(override_leds) = entry.led_count
                && let Some(loc) = self.registry.resolve(&entry.device_id)
                && let Some(hub_conn) = self.hubs.get_mut(&loc.hub_serial)
            {
                hub_conn.info.led_counts.insert(loc.channel, override_leds);
            }
        }
        self.config = config;
        self.groups = groups;
        // A new config may reference a different set of device_ids —
        // reset the orphan warning dedupe so legitimate new orphans get
        // surfaced in the log rather than silently suppressed by stale
        // entries from the previous config.
        self.logged_orphan_rgb_ids.clear();
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

    /// Return `(hub_serial, channel) → device_id` for every currently-
    /// enumerated device across all hubs. Used by DTO construction to
    /// populate `FanReading.device_id` so the frontend can key on stable
    /// identity instead of channel numbers.
    pub fn device_id_map(&self) -> HashMap<(String, u8), String> {
        let mut map = HashMap::new();
        for (serial, hub_conn) in &self.hubs {
            for dev in &hub_conn.info.devices {
                map.insert(
                    (serial.clone(), dev.channel),
                    dev.device_id.clone(),
                );
            }
        }
        map
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

    /// Return serials of hubs that should be recovery-attempted now.
    ///
    /// B2: a hub qualifies when it is currently unhealthy AND either:
    ///   - No recovery has been attempted yet in this failure episode
    ///     (`recovery_attempts == 0`): schedule immediately. This replaces
    ///     the prior "wait for 5 consecutive failures" heuristic — the first
    ///     failure is now the trigger, which keeps fans responsive to a
    ///     plugged-back-in hub within one control cycle instead of five.
    ///   - At least one attempt has been made AND enough time has elapsed
    ///     per the exponential backoff curve (10s, 20s, 40s, 80s, 120s cap).
    pub fn hubs_needing_recovery(&self) -> Vec<String> {
        self.hubs
            .iter()
            .filter(|(_, conn)| {
                if conn.healthy {
                    return false;
                }
                if conn.recovery_attempts == 0 {
                    return true; // B2: immediate on healthy→failed
                }
                conn.last_recovery_attempt.elapsed() >= recovery_backoff(conn.recovery_attempts)
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

            conn.hub = Arc::new(new_hub);
            conn.info = new_info;
            conn.healthy = true;
            conn.consecutive_failures = 0;
            conn.last_recovery_attempt = Instant::now();
            conn.recovery_attempts = 0;
            conn.pump_channels = pump_channels;
            conn.color_logged = false;

            info!(serial, "Hub handle replaced after recovery");
        }
        // Registry may have shifted if channel assignments changed after
        // re-enumeration (fans added/removed/reordered on the chain).
        // Rebuild so subsequent cycles resolve device_ids correctly.
        self.registry = build_registry(&self.hubs);
        // Drop the orphan warning dedupe so a device that disappears
        // again after a recovery cycle gets a fresh log entry — the
        // user deserves to know when a reconnected device later goes
        // missing again, even if we saw it orphan earlier.
        self.logged_orphan_rgb_ids.clear();
    }

    /// Mark that a recovery attempt was made (even if it failed) to enforce
    /// the exponential backoff between subsequent attempts.
    pub fn mark_recovery_attempted(&mut self, serial: &str) {
        if let Some(conn) = self.hubs.get_mut(serial) {
            conn.last_recovery_attempt = Instant::now();
            // Saturating: u32::MAX attempts is never reached in practice but
            // the saturation keeps the backoff formula total.
            conn.recovery_attempts = conn.recovery_attempts.saturating_add(1);
        }
    }

    /// Restore all hubs to hardware (firmware) control mode.
    ///
    /// B5: each `enter_hardware_mode()` call is wrapped in a bounded-timeout
    /// worker thread so a wedged hub (unplugged mid-operation, firmware hang,
    /// driver stuck) can't block shutdown indefinitely. On timeout we log,
    /// abandon the worker thread (it will finish when the I/O eventually
    /// returns, or when the process exits), and move on to the next hub.
    ///
    /// The hub handle is cloned into the worker thread — cheap because
    /// `IcueLinkHub` is `Arc<Mutex<_>>`-internal after the prep refactor.
    pub fn shutdown_hardware(&mut self) {
        info!("Restoring hardware mode on all hubs");
        for (serial, hub_conn) in &mut self.hubs {
            let hub = Arc::clone(&hub_conn.hub);
            let serial_owned = serial.clone();
            let (tx, rx) = std::sync::mpsc::channel::<Result<()>>();

            let handle = std::thread::Builder::new()
                .name(format!("shutdown-hub-{}", serial_owned))
                .spawn(move || {
                    // Channel send may fail if the receiver already timed out
                    // and dropped the rx — we tolerate that (the main thread
                    // has already moved on).
                    let _ = tx.send(hub.enter_hardware_mode());
                });

            let handle = match handle {
                Ok(h) => h,
                Err(e) => {
                    warn!(
                        serial = serial_owned.as_str(),
                        error = %e,
                        "Failed to spawn shutdown worker thread — skipping hub"
                    );
                    continue;
                }
            };

            match rx.recv_timeout(SHUTDOWN_HARDWARE_MODE_TIMEOUT) {
                Ok(Ok(())) => {
                    info!(serial = serial_owned.as_str(), "hardware mode restored");
                    // Wait for the worker to finish so we don't leak a handle
                    // in the happy path. recv_timeout already succeeded so
                    // the worker is moments from exiting.
                    let _ = handle.join();
                }
                Ok(Err(e)) => {
                    warn!(
                        serial = serial_owned.as_str(),
                        error = %e,
                        "enter_hardware_mode failed (hub may keep last software-mode duty)"
                    );
                    let _ = handle.join();
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    warn!(
                        serial = serial_owned.as_str(),
                        timeout_secs = SHUTDOWN_HARDWARE_MODE_TIMEOUT.as_secs(),
                        "enter_hardware_mode timed out — abandoning worker thread"
                    );
                    // Deliberately do NOT join: the worker is blocked on HID
                    // I/O and joining would reinstate the hang we just
                    // escaped from. The thread will finish when the I/O
                    // eventually returns, or when the process exits.
                    std::mem::forget(handle);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    warn!(
                        serial = serial_owned.as_str(),
                        "Shutdown worker thread panicked before sending result"
                    );
                    let _ = handle.join();
                }
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
    /// ## Input shape: `&[(device_id, leds)]`
    ///
    /// Post-Step-5 (PR2): the caller supplies frames keyed by stable
    /// `device_id` only. The renderer produces these; the legacy
    /// `(hub_serial, channel)` path is gone. This function resolves each
    /// device_id via the runtime registry, bucketizes by hub, sorts by
    /// ascending channel within each hub (wire requirement — the hub's
    /// daisy-chain parser walks the buffer in channel order), and builds a
    /// flat buffer for every enumerated device on the hub. Devices present
    /// on a hub but not referenced in `frames` get black fill. Orphan
    /// device_ids (in the input but not in the registry — e.g. a fan
    /// unplugged since config load) are logged once per boot per device_id
    /// and skipped.
    ///
    /// ## LED count precedence (Step 6)
    ///
    /// For each device we need the actual LED count so the flat buffer
    /// matches what the hub parser expects. Precedence:
    ///   1. V2 per-device override: `config.led_count_override_by_id(device_id)`.
    ///   2. V1 (hub, channel) override: `config.led_count_override(hub, ch)`.
    ///   3. Hub firmware 0x1d table: `hub_info.led_counts`.
    ///   4. Device-type default: `dev.device_type.led_count()`.
    /// V2 and V1 overrides are both pre-applied into `hub_info.led_counts`
    /// at build time, so in practice the 0x1d-table lookup already reflects
    /// them. The explicit checks here guard against timing windows where
    /// `update_config` hasn't refreshed the hub table yet (e.g. a live
    /// config edit that changes an override — the control loop's next RGB
    /// tick still produces a correct buffer before the config-apply path
    /// rewrites `hub_info.led_counts`).
    pub fn send_rgb_frames(&mut self, frames: &[(String, &[[u8; 3]])]) -> usize {
        use std::collections::BTreeMap;

        const BLACK: [u8; 3] = [0, 0, 0];

        // Group frames by hub serial, sort by channel within each hub.
        //
        // Orphan device_ids (not in the registry) are logged once per boot
        // per id via `logged_orphan_rgb_ids`, then skipped. A 30 FPS RGB
        // loop would flood the log otherwise. The dedupe set is reset
        // when `update_config` or `replace_hub` refreshes the registry.
        //
        // We store `(channel, device_id, leds)` keyed by hub so the
        // per-device LED-count lookup below has the device_id on hand for
        // the V2 override precedence check.
        let mut by_hub: HashMap<String, BTreeMap<u8, (String, &[[u8; 3]])>> = HashMap::new();
        let mut new_orphans: Vec<String> = Vec::new();
        for (device_id, leds) in frames {
            let Some(loc) = self.registry.resolve(device_id) else {
                // Orphan — defer the log until after the loop so we can
                // update the seen-set without fighting the immutable
                // borrow on `self.registry` above.
                if !self.logged_orphan_rgb_ids.contains(device_id) {
                    new_orphans.push(device_id.clone());
                }
                continue;
            };
            by_hub
                .entry(loc.hub_serial.clone())
                .or_default()
                .insert(loc.channel, (device_id.clone(), *leds));
        }
        for id in new_orphans {
            warn!(
                device_id = id.as_str(),
                "RGB frame references device_id not currently enumerated — \
                 skipping (further occurrences of this id will be silent this boot)"
            );
            self.logged_orphan_rgb_ids.insert(id);
        }

        let mut sent = 0;
        for (serial, frame_channels) in &by_hub {
            let hub_conn = match self.hubs.get_mut(serial.as_str()) {
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

                let frame_data = frame_channels.get(&dev.channel).map(|(_id, leds)| *leds);

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
                    serial = serial.as_str(),
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
                        serial = serial.as_str(),
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
    // V2 groups don't carry hub_serial on the config. For those we fall
    // back to an empty string — the control-loop bucketing sees
    // `device_ids` non-empty first and takes the registry path, never
    // consulting hub_serial. V1 groups retain the original requirement.
    let hub_serial = if !cfg.device_ids.is_empty() {
        cfg.hub_serial.clone().unwrap_or_default()
    } else {
        cfg.hub_serial
            .clone()
            .context(format!("Fan group '{}' missing hub_serial", cfg.name))?
    };

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
        device_ids: cfg.device_ids.clone(),
        controller,
        acoustic,
        last_duty: 0.0,
    })
}

/// Build the identity registry from current hub state. Called at every
/// hub init and every successful hub recovery so the registry reflects
/// the latest channel assignments.
///
/// We go through the `DeviceEnumEntry` tuple shape because `DeviceRegistry`
/// lives in `corsair-common` and must not depend on `corsair-hid`'s
/// `LinkDevice`/`HubInfo`. The `device_type_byte` is round-tripped:
/// effective LED count here is computed from the hub's 0x1d table with
/// fall-back to the device-type default.
fn build_registry(hubs: &HashMap<String, HubConnection>) -> DeviceRegistry {
    let entries: Vec<_> = hubs
        .iter()
        .flat_map(|(serial, hub_conn)| {
            hub_conn.info.devices.iter().map(move |dev| {
                let effective_leds = hub_conn
                    .info
                    .led_counts
                    .get(&dev.channel)
                    .copied()
                    .filter(|&c| c > 0)
                    .unwrap_or_else(|| dev.device_type.led_count());
                // The device_type_byte is the raw protocol byte —
                // reconstruct it so common can store it without
                // importing hid types. We don't have a direct byte on
                // LinkDeviceType, so use a byte that round-trips via
                // LinkDeviceType::from_byte. See `link_device_type_byte`.
                let type_byte = link_device_type_byte(&dev.device_type);
                (
                    serial.clone(),
                    dev.device_id.clone(),
                    dev.channel,
                    type_byte,
                    effective_leds,
                )
            })
        })
        .collect();

    DeviceRegistry::rebuild(entries.iter().map(|(serial, id, channel, type_byte, leds)| {
        DeviceEnumEntry {
            hub_serial: serial.as_str(),
            device_id: id.as_str(),
            channel: *channel,
            device_type_byte: *type_byte,
            led_count: *leds,
        }
    }))
}

/// Round-trip a `LinkDeviceType` back to its protocol byte. Kept local to
/// this crate because `corsair-common` cannot depend on `corsair-hid`.
/// A future PR that moves `LinkDeviceType` into `corsair-common` will
/// delete this helper.
fn link_device_type_byte(t: &LinkDeviceType) -> u8 {
    use LinkDeviceType::*;
    match t {
        QxFan => 0x01,
        LxFan => 0x02,
        RxMaxRgbFan => 0x03,
        RxMaxFan => 0x04,
        LinkAdapter => 0x05,
        LiquidCooler => 0x07,
        WaterBlock => 0x09,
        GpuBlock => 0x0A,
        Psu => 0x0B,
        PumpXd5 => 0x0C,
        Xg7Block => 0x0D,
        RxRgbFan => 0x0F,
        VrmCooler => 0x10,
        TitanCooler => 0x11,
        RxFan => 0x13,
        PumpXd6 => 0x19,
        CommanderDuo => 0x1B,
        // LsStrip is enumerated via the LINK Adapter; its wire byte
        // isn't a standard device_type entry. Return 0x05 (LINK Adapter)
        // as an approximation — the registry doesn't need to
        // discriminate further; downstream consumers use the led_count.
        LsStrip => 0x05,
        Unknown(b) => *b,
    }
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

        // V1: must have hub_serial AND at least one channel.
        // V2: must have at least one device_id; hub_serial / channels are
        // optional (and expected empty post-migration).
        let has_v2_ids = !group.device_ids.is_empty();
        if !has_v2_ids {
            if group.hub_serial.is_none() {
                bail!(
                    "Fan group '{}' missing hub_serial (and no device_ids)",
                    group.name
                );
            }
            if group.channels.is_empty() {
                bail!(
                    "Fan group '{}' has no channels (and no device_ids)",
                    group.name
                );
            }
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

// --- Test-only helpers ---
//
// These live outside the `tests` module because they are referenced from
// `impl ControlLoop` `#[cfg(test)]` methods that need `pub(crate)` visibility
// for integration-style tests. Kept gated on `cfg(test)` so they have zero
// impact on release builds.

#[cfg(test)]
impl ControlLoop {
    /// Assemble a ControlLoop from pre-built parts for integration tests.
    ///
    /// Bypasses USB discovery, sensor initialization, and hub enumeration so
    /// tests can exercise the control loop against mock transports without
    /// real hardware. Real code path is `build()`.
    fn from_parts_for_test(
        config: AppConfig,
        hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)>,
        groups: Vec<FanGroup>,
        sensor_state: HashMap<String, SensorState>,
    ) -> Self {
        Self::from_parts_for_test_with_devices(
            config,
            hubs,
            HashMap::new(),
            groups,
            sensor_state,
        )
    }

    /// Test-only variant that also supplies enumerated devices per hub so
    /// the identity registry has content. Used by the dual-key path tests
    /// added in Step 4.
    fn from_parts_for_test_with_devices(
        config: AppConfig,
        hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)>,
        // (hub_serial) -> Vec<LinkDevice>
        devices_by_hub: HashMap<String, Vec<corsair_hid::LinkDevice>>,
        groups: Vec<FanGroup>,
        sensor_state: HashMap<String, SensorState>,
    ) -> Self {
        let hub_conns: HashMap<String, HubConnection> = hubs
            .into_iter()
            .map(|(serial, (hub, pump_channels))| {
                let devices = devices_by_hub.get(&serial).cloned().unwrap_or_default();
                (
                    serial.clone(),
                    HubConnection {
                        hub,
                        serial,
                        healthy: true,
                        consecutive_failures: 0,
                        last_recovery_attempt: Instant::now(),
                        recovery_attempts: 0,
                        pump_channels,
                        info: HubInfo {
                            firmware: corsair_hid::FirmwareVersion {
                                major: 0,
                                minor: 0,
                                patch: 0,
                            },
                            devices,
                            led_counts: std::collections::HashMap::new(),
                        },
                        color_logged: false,
                    },
                )
            })
            .collect();

        let registry = build_registry(&hub_conns);

        Self {
            config,
            sensors: HashMap::new(),
            sensor_state,
            hubs: hub_conns,
            groups,
            registry,
            logged_orphan_rgb_ids: HashSet::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corsair_common::config::*;
    use std::sync::atomic::AtomicU32;

    // --- Mock transport helpers ---

    /// Mock IcueLinkHub transport with configurable behavior. Tracks call
    /// counts so tests can assert recovery scheduling fires.
    struct MockHub {
        /// When true, set_speeds returns Err. Default false (healthy).
        fail_set_speeds: std::sync::atomic::AtomicBool,
        set_speeds_calls: AtomicU32,
        /// If > 0, enter_hardware_mode sleeps for this many ms before
        /// returning Ok. Used to exercise the shutdown timeout path.
        enter_hardware_mode_delay_ms: u64,
        enter_hardware_mode_calls: AtomicU32,
    }

    impl MockHub {
        fn new() -> Self {
            Self {
                fail_set_speeds: std::sync::atomic::AtomicBool::new(false),
                set_speeds_calls: AtomicU32::new(0),
                enter_hardware_mode_delay_ms: 0,
                enter_hardware_mode_calls: AtomicU32::new(0),
            }
        }

        fn with_slow_shutdown(ms: u64) -> Self {
            let mut h = Self::new();
            h.enter_hardware_mode_delay_ms = ms;
            h
        }

        fn set_fail(&self, fail: bool) {
            self.fail_set_speeds
                .store(fail, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl IcueLinkTransport for MockHub {
        fn set_speeds(&self, _targets: &[(u8, u8)]) -> Result<()> {
            self.set_speeds_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail_set_speeds.load(std::sync::atomic::Ordering::SeqCst) {
                bail!("mock set_speeds failure");
            }
            Ok(())
        }
        fn get_speeds(&self) -> Result<Vec<FanSpeed>> {
            Ok(Vec::new())
        }
        fn set_rgb(&self, _channel_leds: &[(u8, &[[u8; 3]])]) -> Result<()> {
            Ok(())
        }
        fn enter_hardware_mode(&self) -> Result<()> {
            self.enter_hardware_mode_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.enter_hardware_mode_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(
                    self.enter_hardware_mode_delay_ms,
                ));
            }
            Ok(())
        }
    }

    /// Build a minimal ControlLoop with one fan group pointing at a single
    /// mock hub. The hub has one pump channel (1) and one fan channel (2).
    fn loop_with_mock_hub(
        mock: Arc<MockHub>,
        mode: FanMode,
    ) -> (ControlLoop, Arc<MockHub>) {
        let serial = "TESTHUB0".to_string();
        let mut hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)> =
            HashMap::new();
        hubs.insert(serial.clone(), (mock.clone() as Arc<dyn IcueLinkTransport>, vec![1]));

        let (controller, acoustic) = match mode {
            FanMode::Fixed { duty_percent } => (FanController::Fixed(duty_percent), None),
            FanMode::Curve {
                points,
                hysteresis,
                ramp_rate,
                temp_source,
            } => {
                let curve = FanCurve::new(points).expect("test curve");
                let ramp_down = ramp_rate * 0.4;
                let filter = AcousticFilter::new(ramp_rate, ramp_down, hysteresis);
                (
                    FanController::Curve {
                        curve,
                        source: temp_source,
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
                let pid = PidController::new(kp, ki, kd, target_temp)
                    .with_output_limits(min_duty, max_duty);
                let filter = AcousticFilter::new(10.0, 3.0, 2.0);
                (
                    FanController::Pid {
                        pid,
                        source: temp_source,
                    },
                    Some(filter),
                )
            }
        };

        let group = FanGroup {
            name: "test".to_string(),
            channels: vec![1, 2],
            hub_serial: serial.clone(),
            device_ids: Vec::new(),
            controller,
            acoustic,
            last_duty: 0.0,
        };

        let config = AppConfig {
            schema_version: corsair_common::config::SCHEMA_VERSION_V1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: Vec::new(), // groups supplied separately
            rgb: Default::default(),
            device_overrides: Vec::new(),
            devices: Vec::new(),
        };

        let cl = ControlLoop::from_parts_for_test(config, hubs, vec![group], HashMap::new());
        (cl, mock)
    }

    fn valid_config() -> AppConfig {
        AppConfig {
            schema_version: corsair_common::config::SCHEMA_VERSION_V1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: vec![FanGroupConfig {
                name: "test".to_string(),
                channels: vec![1, 2],
                hub_serial: Some("ABCD1234".to_string()),
                device_ids: Vec::new(),
                mode: FanMode::Fixed { duty_percent: 50.0 },
            }],
            rgb: Default::default(),
            device_overrides: Vec::new(),
            devices: Vec::new(),
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

    // --- B2: recovery_backoff pure-function curve ---

    #[test]
    fn test_recovery_backoff_curve() {
        // attempt 0 -> 10s
        assert_eq!(recovery_backoff(0), Duration::from_secs(10));
        // attempt 1 -> 20s
        assert_eq!(recovery_backoff(1), Duration::from_secs(20));
        // attempt 2 -> 40s
        assert_eq!(recovery_backoff(2), Duration::from_secs(40));
        // attempt 3 -> 80s
        assert_eq!(recovery_backoff(3), Duration::from_secs(80));
        // attempt 4 -> 160s clamped to 120s (RECOVERY_BACKOFF_MAX)
        assert_eq!(recovery_backoff(4), Duration::from_secs(120));
        // attempt 5+ -> clamped
        assert_eq!(recovery_backoff(5), Duration::from_secs(120));
        assert_eq!(recovery_backoff(100), Duration::from_secs(120));
        // No overflow panic at u32::MAX
        assert_eq!(recovery_backoff(u32::MAX), Duration::from_secs(120));
    }

    // --- B2: healthy→failed transition schedules immediate recovery ---

    #[test]
    fn test_healthy_to_failed_transition_schedules_recovery() {
        let mock = Arc::new(MockHub::new());
        let (mut cl, mock) = loop_with_mock_hub(
            mock,
            FanMode::Fixed { duty_percent: 50.0 },
        );

        // Initial state: hub is healthy, no recovery scheduled.
        assert!(cl.hubs_needing_recovery().is_empty());

        // First tick — set_speeds succeeds (healthy), no recovery.
        let _ = cl.tick();
        assert!(cl.hubs_needing_recovery().is_empty());
        assert_eq!(mock.set_speeds_calls.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Simulate hub failure.
        mock.set_fail(true);
        let _ = cl.tick();

        // B2: first failure after healthy schedules IMMEDIATE recovery, not
        // after 5 consecutive failures. Prior behavior required 5 failures +
        // 10s cooldown; this test pins the new contract.
        let needing = cl.hubs_needing_recovery();
        assert_eq!(needing.len(), 1, "expected one hub scheduled for recovery");
        assert_eq!(needing[0], "TESTHUB0");

        // Mark attempted. recovery_attempts goes to 1 → next check must wait
        // for recovery_backoff(1) = 20s before returning the serial again.
        cl.mark_recovery_attempted("TESTHUB0");
        assert!(
            cl.hubs_needing_recovery().is_empty(),
            "after one failed attempt, backoff should gate the next"
        );
    }

    // --- B5: shutdown timeout fires without hanging ---

    #[test]
    fn test_shutdown_hardware_times_out() {
        // Mock hub that blocks in enter_hardware_mode for 10s (longer than
        // the 5s SHUTDOWN_HARDWARE_MODE_TIMEOUT). The test asserts
        // shutdown_hardware returns within ~6s — if the timeout path were
        // broken it would hang for the full 10s.
        let mock = Arc::new(MockHub::with_slow_shutdown(10_000));
        let (mut cl, _mock) = loop_with_mock_hub(
            mock,
            FanMode::Fixed { duty_percent: 50.0 },
        );

        let start = Instant::now();
        cl.shutdown_hardware();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(7),
            "shutdown_hardware took {:?}, expected <7s (timeout is 5s)",
            elapsed
        );
        assert!(
            elapsed >= Duration::from_secs(5),
            "shutdown_hardware took {:?}, expected >=5s (timeout must actually wait)",
            elapsed
        );
    }

    // --- B3: sensor-stale cycle resets PID integral across groups ---

    #[test]
    fn test_sensor_stale_resets_pid_integral() {
        let mock = Arc::new(MockHub::new());
        let pid_mode = FanMode::Pid {
            target_temp: 70.0,
            kp: 2.0,
            ki: 0.5,
            kd: 0.0,
            min_duty: 20.0,
            max_duty: 100.0,
            temp_source: TempSourceConfig {
                sensors: vec!["cpu".to_string()],
                weights: vec![1.0],
            },
        };

        let (mut cl, _mock) = loop_with_mock_hub(mock, pid_mode);

        // Inject a stale-CPU sensor state so `any_stale` is true next tick.
        cl.sensor_state.insert(
            "cpu".to_string(),
            SensorState {
                last_value: 80.0,
                last_success: Instant::now() - Duration::from_secs(30),
                is_stale: true,
            },
        );

        // Pre-load integral with non-zero value so the reset has something
        // to clear.
        if let FanController::Pid { pid, .. } = &mut cl.groups[0].controller {
            // Drive integral up: feed error above setpoint for several cycles.
            // The first update() initializes; subsequent ones accumulate.
            pid.update(70.0);
            std::thread::sleep(Duration::from_millis(20));
            pid.update(90.0);
            std::thread::sleep(Duration::from_millis(20));
            pid.update(90.0);
            std::thread::sleep(Duration::from_millis(20));
            pid.update(90.0);
            assert!(
                pid.integral() > 0.0,
                "test setup: expected non-zero integral before tick, got {}",
                pid.integral()
            );
        } else {
            panic!("test setup: group should be PID");
        }

        // Tick — the any_stale path must reset the integral.
        let _ = cl.tick();

        if let FanController::Pid { pid, .. } = &cl.groups[0].controller {
            assert_eq!(
                pid.integral(),
                0.0,
                "integral should have been reset on any_stale tick"
            );
        } else {
            unreachable!();
        }
    }

    // --- Step 4: dual-key control loop tests ---

    /// Mock hub that records every set_speeds and set_rgb call so tests
    /// can verify which (channel, duty) pairs and (channel, led_count)
    /// buffers were actually dispatched through the dual-key path.
    struct RecordingHub {
        speeds_sent: std::sync::Mutex<Vec<Vec<(u8, u8)>>>,
        /// Each element is one `set_rgb` call; per call we record
        /// `(channel, led_count)` so tests can assert routing and
        /// buffer sizing without holding the raw byte slices alive.
        rgb_sent: std::sync::Mutex<Vec<Vec<(u8, usize)>>>,
    }

    impl RecordingHub {
        fn new() -> Self {
            Self {
                speeds_sent: std::sync::Mutex::new(Vec::new()),
                rgb_sent: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn last_speeds(&self) -> Option<Vec<(u8, u8)>> {
            self.speeds_sent.lock().unwrap().last().cloned()
        }

        fn last_rgb(&self) -> Option<Vec<(u8, usize)>> {
            self.rgb_sent.lock().unwrap().last().cloned()
        }

        fn rgb_call_count(&self) -> usize {
            self.rgb_sent.lock().unwrap().len()
        }
    }

    impl IcueLinkTransport for RecordingHub {
        fn set_speeds(&self, targets: &[(u8, u8)]) -> Result<()> {
            self.speeds_sent.lock().unwrap().push(targets.to_vec());
            Ok(())
        }
        fn get_speeds(&self) -> Result<Vec<FanSpeed>> {
            Ok(Vec::new())
        }
        fn set_rgb(&self, channel_leds: &[(u8, &[[u8; 3]])]) -> Result<()> {
            let summary: Vec<(u8, usize)> = channel_leds
                .iter()
                .map(|(ch, leds)| (*ch, leds.len()))
                .collect();
            self.rgb_sent.lock().unwrap().push(summary);
            Ok(())
        }
        fn enter_hardware_mode(&self) -> Result<()> {
            Ok(())
        }
    }

    fn mk_link_device(channel: u8, device_id: &str) -> corsair_hid::LinkDevice {
        corsair_hid::LinkDevice {
            channel,
            device_type: LinkDeviceType::QxFan,
            model: 0x01,
            device_id: device_id.to_string(),
        }
    }

    fn base_v1_config() -> AppConfig {
        AppConfig {
            schema_version: corsair_common::config::SCHEMA_VERSION_V1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: Vec::new(),
            rgb: Default::default(),
            device_overrides: Vec::new(),
            devices: Vec::new(),
        }
    }

    /// A V2-style FanGroup with device_ids routes its duty through the
    /// registry to the right hub and channel. Two devices on the same hub
    /// at channels 1 and 2, referenced by device_id — after one tick, the
    /// hub's recorded set_speeds call must contain both channels.
    #[test]
    fn device_ids_resolve_via_registry_to_correct_channel() {
        let hub_serial = "HUB_A".to_string();
        let mock: Arc<RecordingHub> = Arc::new(RecordingHub::new());
        let mut hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)> = HashMap::new();
        hubs.insert(
            hub_serial.clone(),
            (mock.clone() as Arc<dyn IcueLinkTransport>, Vec::new()),
        );

        // Two devices on HUB_A at channels 1 and 2
        let mut devices_by_hub: HashMap<String, Vec<corsair_hid::LinkDevice>> = HashMap::new();
        devices_by_hub.insert(
            hub_serial.clone(),
            vec![
                mk_link_device(1, "ID_A1"),
                mk_link_device(2, "ID_A2"),
            ],
        );

        // V2-shaped FanGroup: device_ids populated, no channels/hub_serial
        let group = FanGroup {
            name: "v2_group".to_string(),
            channels: Vec::new(),
            hub_serial: String::new(),
            device_ids: vec!["ID_A1".to_string(), "ID_A2".to_string()],
            controller: FanController::Fixed(55.0),
            acoustic: None,
            last_duty: 0.0,
        };

        let mut cl = ControlLoop::from_parts_for_test_with_devices(
            base_v1_config(),
            hubs,
            devices_by_hub,
            vec![group],
            HashMap::new(),
        );

        // Act
        let _ = cl.tick();

        // Assert: hub received both channels with duty 55 (rounded).
        let sent = mock
            .last_speeds()
            .expect("hub should have received one set_speeds call");
        let mut sorted = sent.clone();
        sorted.sort_by_key(|(ch, _)| *ch);
        assert_eq!(
            sorted,
            vec![(1u8, 55u8), (2u8, 55u8)],
            "V2 group should resolve both device_ids to ch 1 & 2"
        );
    }

    /// When device_ids is empty, the legacy channel path runs — the
    /// hub receives the FanGroup.channels as-is. Regression guard that
    /// the V1 fallback wasn't broken by the V2 branch.
    #[test]
    fn empty_device_ids_falls_through_to_channel_path() {
        let hub_serial = "HUB_A".to_string();
        let mock: Arc<RecordingHub> = Arc::new(RecordingHub::new());
        let mut hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)> = HashMap::new();
        hubs.insert(
            hub_serial.clone(),
            (mock.clone() as Arc<dyn IcueLinkTransport>, Vec::new()),
        );

        // No devices registered in the registry — the channel path must
        // work regardless of what the registry knows (V1 doesn't use it).
        let devices_by_hub: HashMap<String, Vec<corsair_hid::LinkDevice>> = HashMap::new();

        // V1-shaped FanGroup: channels populated, device_ids empty.
        let group = FanGroup {
            name: "v1_group".to_string(),
            channels: vec![7, 8],
            hub_serial: hub_serial.clone(),
            device_ids: Vec::new(),
            controller: FanController::Fixed(40.0),
            acoustic: None,
            last_duty: 0.0,
        };

        let mut cl = ControlLoop::from_parts_for_test_with_devices(
            base_v1_config(),
            hubs,
            devices_by_hub,
            vec![group],
            HashMap::new(),
        );

        let _ = cl.tick();

        // Fan duty 40 is below MIN_FAN_DUTY (20) threshold? No, 40 > 20.
        let sent = mock
            .last_speeds()
            .expect("hub should have received set_speeds");
        let mut sorted = sent.clone();
        sorted.sort_by_key(|(ch, _)| *ch);
        assert_eq!(
            sorted,
            vec![(7u8, 40u8), (8u8, 40u8)],
            "V1 group should keep its literal channels"
        );
    }

    // --- Step 5: RGB pipeline keyed by device_id ---

    /// send_rgb_frames now takes `(device_id, leds)` pairs. Each device_id
    /// must resolve via the registry to its current (hub, channel). Two
    /// devices on different hubs demonstrate that bucketing routes the
    /// right LEDs to the right hub.
    #[test]
    fn send_rgb_frames_routes_v2_device_ids_correctly() {
        let hub_a = "HUB_A".to_string();
        let hub_b = "HUB_B".to_string();
        let mock_a: Arc<RecordingHub> = Arc::new(RecordingHub::new());
        let mock_b: Arc<RecordingHub> = Arc::new(RecordingHub::new());

        let mut hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)> = HashMap::new();
        hubs.insert(
            hub_a.clone(),
            (mock_a.clone() as Arc<dyn IcueLinkTransport>, Vec::new()),
        );
        hubs.insert(
            hub_b.clone(),
            (mock_b.clone() as Arc<dyn IcueLinkTransport>, Vec::new()),
        );

        // ID_A1 on HUB_A ch 1; ID_B2 on HUB_B ch 2. QxFan device type
        // declares 34 LEDs as its default.
        let mut devices_by_hub: HashMap<String, Vec<corsair_hid::LinkDevice>> = HashMap::new();
        devices_by_hub.insert(
            hub_a.clone(),
            vec![mk_link_device(1, "ID_A1")],
        );
        devices_by_hub.insert(
            hub_b.clone(),
            vec![mk_link_device(2, "ID_B2")],
        );

        let mut cl = ControlLoop::from_parts_for_test_with_devices(
            base_v1_config(),
            hubs,
            devices_by_hub,
            Vec::new(),
            HashMap::new(),
        );

        // Build frames keyed by device_id. Each device advertises 34 LEDs
        // (QxFan default), so provide that many.
        let a_leds = vec![[1u8, 2, 3]; 34];
        let b_leds = vec![[4u8, 5, 6]; 34];
        let frames: Vec<(String, &[[u8; 3]])> = vec![
            ("ID_A1".to_string(), a_leds.as_slice()),
            ("ID_B2".to_string(), b_leds.as_slice()),
        ];

        let sent = cl.send_rgb_frames(&frames);

        // Each hub's enumeration lists one device; the flat buffer for
        // each hub is therefore one entry long.
        assert_eq!(sent, 2, "one device per hub should yield sent == 2");

        // HUB_A call should include channel 1 with 34 LEDs (not channel 2).
        let a_call = mock_a.last_rgb().expect("HUB_A should have received set_rgb");
        assert_eq!(
            a_call,
            vec![(1u8, 34usize)],
            "HUB_A should receive ch1 34 LEDs (ID_A1)"
        );
        let b_call = mock_b.last_rgb().expect("HUB_B should have received set_rgb");
        assert_eq!(
            b_call,
            vec![(2u8, 34usize)],
            "HUB_B should receive ch2 34 LEDs (ID_B2)"
        );
    }

    /// Orphan device_ids (in frames but not in registry) are skipped cleanly
    /// and logged only once per boot per id. We verify the behavior by
    /// calling send_rgb_frames twice with the same orphan id — the second
    /// call must still skip without crashing and without redundant log
    /// attempts (the `logged_orphan_rgb_ids` set dedupes the warning path).
    #[test]
    fn send_rgb_frames_skips_orphan_device_id_with_log() {
        let hub_a = "HUB_A".to_string();
        let mock_a: Arc<RecordingHub> = Arc::new(RecordingHub::new());

        let mut hubs: HashMap<String, (Arc<dyn IcueLinkTransport>, Vec<u8>)> = HashMap::new();
        hubs.insert(
            hub_a.clone(),
            (mock_a.clone() as Arc<dyn IcueLinkTransport>, Vec::new()),
        );

        // Only ID_A1 is enumerated; ID_ORPHAN is never seen.
        let mut devices_by_hub: HashMap<String, Vec<corsair_hid::LinkDevice>> = HashMap::new();
        devices_by_hub.insert(hub_a.clone(), vec![mk_link_device(1, "ID_A1")]);

        let mut cl = ControlLoop::from_parts_for_test_with_devices(
            base_v1_config(),
            hubs,
            devices_by_hub,
            Vec::new(),
            HashMap::new(),
        );

        let real_leds = vec![[10u8, 20, 30]; 34];
        let orphan_leds = vec![[99u8, 99, 99]; 34];
        let frames: Vec<(String, &[[u8; 3]])> = vec![
            ("ID_A1".to_string(), real_leds.as_slice()),
            ("ID_ORPHAN".to_string(), orphan_leds.as_slice()),
        ];

        // First call — should warn once on the orphan and successfully
        // route ID_A1 to channel 1.
        let sent1 = cl.send_rgb_frames(&frames);
        assert_eq!(
            sent1, 1,
            "one real device on hub; orphan skipped so sent == 1"
        );
        assert!(
            cl.logged_orphan_rgb_ids.contains("ID_ORPHAN"),
            "orphan id should be recorded in the seen-set after first call"
        );
        let call1 = mock_a
            .last_rgb()
            .expect("HUB_A should have received a set_rgb call");
        assert_eq!(
            call1,
            vec![(1u8, 34usize)],
            "HUB_A should receive only ch1 (ID_A1), not the orphan"
        );

        // Second call — orphan is now in the seen-set; skip silently.
        // The real device still makes it through.
        let sent2 = cl.send_rgb_frames(&frames);
        assert_eq!(sent2, 1, "orphan still skipped on repeat");
        assert_eq!(
            mock_a.rgb_call_count(),
            2,
            "hub should have received two set_rgb calls overall"
        );
    }
}
