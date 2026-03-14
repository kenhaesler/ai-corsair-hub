use serde::Serialize;

use corsair_fancontrol::CycleResult;
use corsair_hid::{DeviceGroup, FanSpeed, HubInfo, PsuStatus};

/// Complete system state snapshot — emitted to frontend every poll cycle.
#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub timestamp_ms: u64,
    pub temperatures: Vec<TempReading>,
    pub fans: Vec<FanReading>,
    pub psu: Option<PsuSnapshot>,
    pub group_duties: Vec<GroupDuty>,
    pub emergency: bool,
    pub any_stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TempReading {
    pub source: String,
    pub celsius: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FanReading {
    pub hub_serial: String,
    pub channel: u8,
    pub rpm: u16,
    pub duty_percent: u8,
    pub group_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupDuty {
    pub name: String,
    pub duty_percent: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsuSnapshot {
    pub temp_vrm: f64,
    pub temp_case: f64,
    pub fan_rpm: u16,
    pub input_voltage: f64,
    pub rails: Vec<RailSnapshot>,
    pub total_power: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RailSnapshot {
    pub name: String,
    pub voltage: f64,
    pub current: f64,
    pub power: f64,
}

/// Device tree — response to get_devices command.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceTree {
    pub hubs: Vec<HubSnapshot>,
    pub psu: Option<PsuDeviceInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubSnapshot {
    pub serial: String,
    pub firmware: String,
    pub devices: Vec<HubDeviceEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubDeviceEntry {
    pub channel: u8,
    pub device_type: String,
    pub model: u8,
    pub device_id: String,
    pub rpm: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsuDeviceInfo {
    pub serial: String,
    pub model: String,
}

// --- Conversion helpers ---

impl SystemSnapshot {
    /// Build a snapshot from a CycleResult plus optional fan RPM and PSU data.
    pub fn from_cycle(
        result: &CycleResult,
        fan_speeds: &[(String, Vec<FanSpeed>)],
        psu_status: Option<&PsuStatus>,
    ) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut temperatures: Vec<TempReading> = result
            .readings
            .iter()
            .map(|(source, &celsius)| TempReading {
                source: source.clone(),
                celsius,
            })
            .collect();
        temperatures.sort_by(|a, b| a.source.cmp(&b.source));

        // Build duty lookup from group duties
        let mut channel_duty: std::collections::HashMap<(String, u8), (u8, String)> =
            std::collections::HashMap::new();
        for gd in &result.group_duties {
            for &(ch, duty) in &gd.channels {
                channel_duty.insert(
                    (gd.hub_serial.clone(), ch),
                    (duty, gd.name.clone()),
                );
            }
        }

        let mut fans = Vec::new();
        for (serial, speeds) in fan_speeds {
            for speed in speeds {
                let (duty, group) = channel_duty
                    .get(&(serial.clone(), speed.channel))
                    .map(|(d, g)| (*d, Some(g.clone())))
                    .unwrap_or((0, None));
                fans.push(FanReading {
                    hub_serial: serial.clone(),
                    channel: speed.channel,
                    rpm: speed.rpm,
                    duty_percent: duty,
                    group_name: group,
                });
            }
        }

        let group_duties: Vec<GroupDuty> = result
            .group_duties
            .iter()
            .map(|gd| {
                let avg_duty = if gd.channels.is_empty() {
                    0
                } else {
                    (gd.channels.iter().map(|(_, d)| *d as u32).sum::<u32>()
                        / gd.channels.len() as u32) as u8
                };
                GroupDuty {
                    name: gd.name.clone(),
                    duty_percent: avg_duty,
                }
            })
            .collect();

        let psu = psu_status.map(|s| PsuSnapshot {
            temp_vrm: s.temp_vrm,
            temp_case: s.temp_case,
            fan_rpm: s.fan_rpm,
            input_voltage: s.input_voltage,
            rails: vec![
                RailSnapshot {
                    name: "12V".into(),
                    voltage: s.rail_12v.voltage,
                    current: s.rail_12v.current,
                    power: s.rail_12v.power,
                },
                RailSnapshot {
                    name: "5V".into(),
                    voltage: s.rail_5v.voltage,
                    current: s.rail_5v.current,
                    power: s.rail_5v.power,
                },
                RailSnapshot {
                    name: "3.3V".into(),
                    voltage: s.rail_3v3.voltage,
                    current: s.rail_3v3.current,
                    power: s.rail_3v3.power,
                },
            ],
            total_power: s.total_power,
        });

        SystemSnapshot {
            timestamp_ms,
            temperatures,
            fans,
            psu,
            group_duties,
            emergency: result.emergency,
            any_stale: result.any_stale,
        }
    }
}

impl DeviceTree {
    pub fn from_discovery(
        hub_groups: &[DeviceGroup],
        hub_infos: &[(String, HubInfo, Vec<FanSpeed>)],
        psu_group: Option<&DeviceGroup>,
    ) -> Self {
        let hubs = hub_infos
            .iter()
            .map(|(serial, info, speeds)| {
                let devices = info
                    .devices
                    .iter()
                    .map(|d| {
                        let rpm = speeds
                            .iter()
                            .find(|s| s.channel == d.channel)
                            .map(|s| s.rpm);
                        HubDeviceEntry {
                            channel: d.channel,
                            device_type: d.device_type.name().to_string(),
                            model: d.model,
                            device_id: d.device_id.clone(),
                            rpm,
                        }
                    })
                    .collect();
                HubSnapshot {
                    serial: serial.clone(),
                    firmware: info.firmware.to_string(),
                    devices,
                }
            })
            .collect();

        let psu = psu_group.map(|g| PsuDeviceInfo {
            serial: g.serial.clone(),
            model: g.device_type.name().to_string(),
        });

        let _ = hub_groups; // used for future enrichment

        DeviceTree { hubs, psu }
    }
}
