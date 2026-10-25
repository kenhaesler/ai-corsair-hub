use std::collections::HashSet;

use serde::Serialize;

use corsair_common::catalog::{self, SupportLevel};
use corsair_common::identity::DeviceRegistry;
use corsair_fancontrol::CycleResult;
use corsair_hid::icue_link::{LinkDevice, LinkDeviceType, SubdeviceDiff, SubdeviceMove};
use corsair_hid::topology::{DetectedDevice, DeviceKey, Snapshot, TopologyDiff};
use corsair_hid::{DeviceGroup, FanSpeed, HubInfo, PsuStatus};
use corsair_rgb::RgbFrame;

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
    pub hub_health: Vec<HubHealth>,
    /// Extended GPU telemetry (util %, power W, clock MHz, VRAM). Present
    /// only when NVML is available — on AMD/Intel systems this is `None`
    /// and the dashboard GPU card hides itself. Populated alongside the
    /// existing `gpu` entry in `temperatures`; the two can co-exist since
    /// temperature is a sensor-fusion input and these metrics are pure
    /// display. Added in v0.4.
    pub gpu_metrics: Option<GpuMetricsDto>,
    /// Extended CPU telemetry (load %, package power, max core clock)
    /// sourced from LibreHardwareMonitor's HTTP tree. `None` when LHM
    /// isn't running or doesn't have a `/amdcpu` / `/intelcpu` node.
    /// Added in v0.4.
    pub cpu_metrics: Option<CpuMetricsDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuMetricsDto {
    pub utilization_pct: u32,
    pub power_watts: f64,
    pub clock_mhz: u32,
    pub memory_used_mb: u64,
    /// Temperature in °C — duplicated from `temperatures` so the card can
    /// render without cross-referencing. The temperatures vec is the
    /// authoritative source for fan-control logic.
    pub temp_celsius: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuMetricsDto {
    pub load_pct: f64,
    pub power_watts: f64,
    pub clock_mhz: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubHealth {
    pub serial: String,
    pub healthy: bool,
    pub consecutive_failures: u32,
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
    /// Stable device identity (26-hex string burned in at manufacturing).
    /// Empty string when the device isn't in the hub's enumerated list at
    /// this moment (should not happen for fans that returned an RPM, but
    /// the construction path is defensive).
    pub device_id: String,
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
    /// Every Corsair USB device from the latest topology snapshot, sorted by
    /// (support, name, key). Diagnostics only; empty until the first snapshot.
    pub detected: Vec<DetectedDeviceDto>,
    /// Bluetooth / problem devnodes filtered out of `detected`.
    pub excluded_count: u32,
}

/// Why a detected device is present but not (yet) usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Ok,
    /// Devnode not started yet.
    NotReady,
    /// Re-arrived too often (flood guard).
    Quarantined,
    /// Recent init failure (failure-ignore window).
    #[allow(dead_code)] // no producer yet: recovery paths do not record failures (fix plan PR 6+7)
    Backoff,
}

impl DeviceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NotReady => "not_ready",
            Self::Quarantined => "quarantined",
            Self::Backoff => "backoff",
        }
    }

    /// Precedence when several conditions hold: quarantine, then backoff, then readiness.
    pub fn resolve(quarantined: bool, backoff: bool, all_ready: bool) -> Self {
        if quarantined {
            Self::Quarantined
        } else if backoff {
            Self::Backoff
        } else if !all_ready {
            Self::NotReady
        } else {
            Self::Ok
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectedDeviceDto {
    /// Stable identity string: "VVVV:PPPP:<serial>" or "VVVV:PPPP:usb:<usb instance id>"
    /// (hex upper-case; "VVVV:PPPP:unresolved" when neither is known).
    pub key: String,
    pub vid: u16,
    pub pid: u16,
    /// Catalog name, else "Unknown Corsair device (PID 0xPPPP)".
    pub name: String,
    /// Catalog family as a string, e.g. "IcueLinkHub", "HidPsu", "Unknown".
    pub family: String,
    /// "supported" | "detect_only" | "unknown" (not in catalog).
    pub support: String,
    pub serial: Option<String>,
    pub interface_count: u32,
    pub bootloader: bool,
    /// "ok" | "not_ready" | "quarantined" | "backoff"
    pub status: String,
}

impl DetectedDeviceDto {
    pub fn from_detected(dev: &DetectedDevice, status: DeviceStatus) -> Self {
        let key = &dev.key;
        let def = catalog::lookup(key.vid, key.pid);
        let key_str = match (&key.serial, &key.usb_instance_id) {
            (Some(serial), _) => format!("{:04X}:{:04X}:{}", key.vid, key.pid, serial),
            (None, Some(usb)) => format!("{:04X}:{:04X}:usb:{}", key.vid, key.pid, usb),
            (None, None) => format!("{:04X}:{:04X}:unresolved", key.vid, key.pid),
        };
        Self {
            key: key_str,
            vid: key.vid,
            pid: key.pid,
            name: def.map_or_else(
                || format!("Unknown Corsair device (PID 0x{:04X})", key.pid),
                |d| d.name.to_string(),
            ),
            family: def.map_or_else(|| "Unknown".to_string(), |d| format!("{:?}", d.family)),
            support: match def.map(|d| d.support) {
                Some(SupportLevel::Supported) => "supported",
                Some(SupportLevel::DetectOnly) => "detect_only",
                None => "unknown",
            }
            .to_string(),
            serial: key.serial.clone(),
            interface_count: dev.interfaces.len() as u32,
            bootloader: def.is_some_and(|d| d.is_bootloader),
            status: status.as_str().to_string(),
        }
    }

    /// Supported devices first, then detect-only, then unknown.
    fn support_rank(&self) -> u8 {
        match self.support.as_str() {
            "supported" => 0,
            "detect_only" => 1,
            _ => 2,
        }
    }
}

/// All devices in `snapshot` as DTOs, sorted by (support, name, key).
/// `status_of` supplies each device's status.
pub fn detected_devices(
    snapshot: &Snapshot,
    mut status_of: impl FnMut(&DetectedDevice) -> DeviceStatus,
) -> Vec<DetectedDeviceDto> {
    let mut out: Vec<DetectedDeviceDto> = snapshot
        .devices
        .values()
        .map(|d| DetectedDeviceDto::from_detected(d, status_of(d)))
        .collect();
    out.sort_by(|a, b| {
        (a.support_rank(), &a.name, &a.key).cmp(&(b.support_rank(), &b.name, &b.key))
    });
    out
}

/// Devices with at least one interface whose devnode is not ready (`is_ready` takes the
/// interface path).
pub fn devices_not_ready(
    snapshot: &Snapshot,
    is_ready: impl Fn(&str) -> bool,
) -> HashSet<DeviceKey> {
    snapshot
        .devices
        .values()
        .filter(|d| d.interfaces.iter().any(|i| !is_ready(&i.path)))
        .map(|d| d.key.clone())
        .collect()
}

/// Re-check the keys in `not_ready` against `snapshot` and drop those whose interfaces are
/// all ready now (or that left the snapshot). Cheap per-interface devnode queries only.
pub fn recheck_not_ready(
    snapshot: &Snapshot,
    not_ready: &mut HashSet<DeviceKey>,
    is_ready: impl Fn(&str) -> bool,
) {
    not_ready.retain(|key| {
        snapshot
            .devices
            .get(key)
            .is_some_and(|d| d.interfaces.iter().any(|i| !is_ready(&i.path)))
    });
}

/// What one detection poll produced, to be emitted by the caller as Tauri events
/// (`device-topology`, `link-subdevices-changed`).
#[derive(Debug, Default)]
pub struct DetectionOutput {
    pub topology: Vec<TopologyEventDto>,
    pub link: Vec<LinkSubdevicesChangedDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopologyEventDto {
    /// "added" | "removed" | "changed" | "suspending" | "resumed"
    pub kind: String,
    pub device: Option<DetectedDeviceDto>,
    pub timestamp_ms: u64,
}

impl TopologyEventDto {
    /// A suspend/resume marker with no device.
    pub fn power(kind: &str, timestamp_ms: u64) -> Self {
        Self {
            kind: kind.to_string(),
            device: None,
            timestamp_ms,
        }
    }
}

/// One event per added / removed / changed device, in that order. Removed devices are
/// reported with `DeviceStatus::Ok`; `status_of` applies to added and changed ones.
pub fn topology_events(
    diff: &TopologyDiff,
    mut status_of: impl FnMut(&DetectedDevice) -> DeviceStatus,
    timestamp_ms: u64,
) -> Vec<TopologyEventDto> {
    let event = |kind: &str, dev: &DetectedDevice, status: DeviceStatus| TopologyEventDto {
        kind: kind.to_string(),
        device: Some(DetectedDeviceDto::from_detected(dev, status)),
        timestamp_ms,
    };
    let mut out = Vec::new();
    for dev in &diff.added {
        out.push(event("added", dev, status_of(dev)));
    }
    for dev in &diff.removed {
        out.push(event("removed", dev, DeviceStatus::Ok));
    }
    for ch in &diff.changed {
        out.push(event("changed", &ch.new, status_of(&ch.new)));
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct SubdeviceMoveDto {
    pub device_id: String,
    pub from_channel: u8,
    pub to_channel: u8,
}

impl From<&SubdeviceMove> for SubdeviceMoveDto {
    fn from(m: &SubdeviceMove) -> Self {
        Self {
            device_id: m.device_id.clone(),
            from_channel: m.from_channel,
            to_channel: m.to_channel,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkSubdevicesChangedDto {
    pub hub_serial: String,
    /// device_ids
    pub added: Vec<String>,
    /// device_ids
    pub removed: Vec<String>,
    pub moved: Vec<SubdeviceMoveDto>,
    pub timestamp_ms: u64,
}

impl LinkSubdevicesChangedDto {
    /// `None` when the diff has no added, removed or moved devices.
    pub fn from_diff(hub_serial: &str, diff: &SubdeviceDiff, timestamp_ms: u64) -> Option<Self> {
        if diff.added.is_empty() && diff.removed.is_empty() && diff.moved.is_empty() {
            return None;
        }
        Some(Self {
            hub_serial: hub_serial.to_string(),
            added: diff.added.iter().map(|d| d.device_id.clone()).collect(),
            removed: diff.removed.iter().map(|d| d.device_id.clone()).collect(),
            moved: diff.moved.iter().map(SubdeviceMoveDto::from).collect(),
            timestamp_ms,
        })
    }
}

/// Result of `set_manual_duty_by_device_id`. The UI uses `unresolved` to
/// show which fans were not driven (e.g. because the device is currently
/// unplugged) without hiding the partial-success case as a blanket error.
#[derive(Debug, Clone, Serialize)]
pub struct ManualDutyResult {
    /// Device ids that were resolved and whose hub was commanded.
    pub applied: Vec<String>,
    /// Device ids that did not resolve via the current registry. Caller
    /// surfaces these as a warning — the devices are not enumerated at
    /// this moment (unplugged, moved, or renamed-out).
    pub unresolved: Vec<String>,
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
    /// LS350 strips decoded from a LINK Adapter's effective LED total
    /// (display only). `None` for every other device type and for adapter
    /// totals not in the iCUE manifest table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strips: Option<Vec<StripSegmentDto>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StripSegmentDto {
    pub index: u8,
    pub led_start: u16,
    pub led_count: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsuDeviceInfo {
    pub serial: String,
    pub model: String,
}

/// Compact RGB frame for IPC — emitted as Tauri event at 30 FPS.
///
/// `device_id` is the stable identity. `hub_serial` + `channel` are the
/// currently-resolved runtime location, included for the duration of the
/// V1→V2 UI transition: the Svelte preview still keys by channel (PR3 will
/// migrate it). The backend resolves `(hub_serial, channel)` from the
/// registry at emit time so the DTO is always self-consistent — if the
/// device can't be resolved now, the location fields are empty.
#[derive(Debug, Clone, Serialize)]
pub struct RgbFrameDto {
    pub hub_serial: String,
    pub channel: u8,
    /// Stable device identity. Always populated by the backend post-Step 5.
    pub device_id: String,
    pub leds: Vec<[u8; 3]>,
}

impl RgbFrameDto {
    /// Build a DTO from a renderer frame, resolving `(hub_serial, channel)`
    /// from the current registry. If the device_id is not in the registry
    /// (orphan — e.g. fan unplugged between frame render and emit), we emit
    /// empty location strings. The frontend treats empty fields as "unknown
    /// location" and continues to render by device_id once PR3 lands.
    pub fn from_frame(frame: &RgbFrame, registry: &DeviceRegistry) -> Self {
        let (hub_serial, channel) = registry
            .channel_for(&frame.device_id)
            .unwrap_or_else(|| (String::new(), 0));
        Self {
            hub_serial,
            channel,
            device_id: frame.device_id.clone(),
            leds: frame.leds.iter().map(|c| [c.r, c.g, c.b]).collect(),
        }
    }
}

// --- Conversion helpers ---

impl SystemSnapshot {
    /// Build a snapshot from a CycleResult plus optional fan RPM and PSU data.
    ///
    /// `device_id_map` provides the `(hub_serial, channel) → device_id`
    /// lookup used to populate the new `FanReading.device_id` field. Built
    /// from `ControlLoop::hub_snapshots()` at the call site — we accept a
    /// borrowed map so the caller controls ownership of the strings. When the
    /// map is missing an entry (e.g. a hub that failed enumeration), the
    /// device_id is emitted as the empty string; the frontend treats an
    /// empty string as "unknown" and falls back to legacy (hub, channel)
    /// display.
    pub fn from_cycle(
        result: &CycleResult,
        fan_speeds: &[(String, Vec<FanSpeed>)],
        psu_status: Option<&PsuStatus>,
        device_id_map: &std::collections::HashMap<(String, u8), String>,
        gpu_metrics: Option<GpuMetricsDto>,
        cpu_metrics: Option<CpuMetricsDto>,
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
                let device_id = device_id_map
                    .get(&(serial.clone(), speed.channel))
                    .cloned()
                    .unwrap_or_default();
                fans.push(FanReading {
                    hub_serial: serial.clone(),
                    channel: speed.channel,
                    device_id,
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

        let hub_health: Vec<HubHealth> = result
            .hub_health
            .iter()
            .map(|h| HubHealth {
                serial: h.serial.clone(),
                healthy: h.healthy,
                consecutive_failures: h.consecutive_failures,
            })
            .collect();

        SystemSnapshot {
            timestamp_ms,
            temperatures,
            fans,
            psu,
            group_duties,
            emergency: result.emergency,
            any_stale: result.any_stale,
            hub_health,
            gpu_metrics,
            cpu_metrics,
        }
    }
}

/// Strip segments for a LINK Adapter, from the same effective LED count the
/// control loop registers (hub 0x1d table, else the type default).
fn adapter_strips(info: &HubInfo, dev: &LinkDevice) -> Option<Vec<StripSegmentDto>> {
    if dev.device_type != LinkDeviceType::LinkAdapter {
        return None;
    }
    let effective = info
        .led_counts
        .get(&dev.channel)
        .copied()
        .filter(|&c| c > 0)
        .unwrap_or_else(|| dev.device_type.led_count());
    catalog::decode_adapter_strips(effective).map(|segs| {
        segs.into_iter()
            .map(|s| StripSegmentDto {
                index: s.index,
                led_start: s.led_start,
                led_count: s.led_count,
            })
            .collect()
    })
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
                            strips: adapter_strips(info, d),
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

        DeviceTree {
            hubs,
            psu,
            detected: Vec::new(),
            excluded_count: 0,
        }
    }

    /// Attach the detection diagnostics built from the latest topology snapshot.
    pub fn with_detection(mut self, detected: Vec<DetectedDeviceDto>, excluded_count: u32) -> Self {
        self.detected = detected;
        self.excluded_count = excluded_count;
        self
    }
}

#[cfg(test)]
mod detection_tests {
    use super::*;
    use corsair_common::CORSAIR_VID;
    use corsair_hid::icue_link::{diff_subdevices, FirmwareVersion};
    use corsair_hid::topology::{diff, DevnodeContext, RawInterface};

    fn tree_for(devices: Vec<LinkDevice>, leds: &[(u8, u16)]) -> DeviceTree {
        let info = HubInfo {
            firmware: FirmwareVersion { major: 1, minor: 0, patch: 0 },
            devices,
            led_counts: leds.iter().copied().collect(),
        };
        DeviceTree::from_discovery(&[], &[("HUB".to_string(), info, Vec::new())], None)
    }

    fn link_dev(channel: u8, device_type: LinkDeviceType) -> LinkDevice {
        LinkDevice { channel, device_type, model: 0, device_id: format!("dev{channel}") }
    }

    #[test]
    fn adapter_with_80_leds_gets_two_40_led_strips_and_fan_gets_none() {
        let tree = tree_for(
            vec![
                link_dev(1, LinkDeviceType::LinkAdapter),
                link_dev(2, LinkDeviceType::QxFan),
            ],
            &[(1, 80), (2, 34)],
        );
        let devs = &tree.hubs[0].devices;
        let strips = devs[0].strips.as_ref().unwrap();
        assert_eq!(
            strips,
            &vec![
                StripSegmentDto { index: 0, led_start: 0, led_count: 40 },
                StripSegmentDto { index: 1, led_start: 40, led_count: 40 },
            ]
        );
        assert!(devs[1].strips.is_none());
    }

    #[test]
    fn adapter_with_undecodable_total_gets_no_strips() {
        let tree = tree_for(vec![link_dev(1, LinkDeviceType::LinkAdapter)], &[(1, 77)]);
        assert!(tree.hubs[0].devices[0].strips.is_none());
    }

    const HUB: u16 = 0x0C3F;
    const PSU: u16 = 0x1C1F;
    const DRAM: u16 = 0x0006; // catalogued, detect-only
    const UNKNOWN: u16 = 0xFFFE; // not in catalog

    fn rec(
        pid: u16,
        serial: Option<&str>,
        iface: i32,
        usb: Option<&str>,
    ) -> (RawInterface, DevnodeContext) {
        (
            RawInterface {
                vid: CORSAIR_VID,
                pid,
                serial: serial.map(String::from),
                interface_number: iface,
                path: format!("hid-path-{pid:04x}-{iface}"),
                usage_page: 0xFF42,
                usage: 1,
            },
            DevnodeContext {
                usb_instance_id: usb.map(String::from),
                ..Default::default()
            },
        )
    }

    fn fixture() -> Snapshot {
        Snapshot::from_records([
            rec(UNKNOWN, None, 0, Some("USB-INSTANCE-1")),
            rec(DRAM, Some("DRAM1"), 0, None),
            rec(HUB, Some("HUB-B"), 0, None),
            rec(HUB, Some("HUB-B"), 1, None),
            rec(HUB, Some("HUB-A"), 0, None),
            rec(PSU, Some("PSU1"), 0, None),
        ])
    }

    #[test]
    fn dto_names_family_and_support_from_catalog() {
        let list = detected_devices(&fixture(), |_| DeviceStatus::Ok);
        let hub = list.iter().find(|d| d.key == "1B1C:0C3F:HUB-A").unwrap();
        assert_eq!(hub.name, catalog::lookup(CORSAIR_VID, HUB).unwrap().name);
        assert_eq!(hub.family, "IcueLinkHub");
        assert_eq!(hub.support, "supported");
        assert_eq!(hub.serial.as_deref(), Some("HUB-A"));
        assert!(!hub.bootloader);
        assert_eq!(hub.status, "ok");

        let psu = list.iter().find(|d| d.pid == PSU).unwrap();
        assert_eq!(psu.family, "HidPsu");

        let dram = list.iter().find(|d| d.pid == DRAM).unwrap();
        assert_eq!(dram.support, "detect_only");
    }

    #[test]
    fn dto_unknown_device_gets_placeholder_name_and_usb_key() {
        let list = detected_devices(&fixture(), |_| DeviceStatus::Ok);
        let u = list.iter().find(|d| d.pid == UNKNOWN).unwrap();
        assert_eq!(u.name, "Unknown Corsair device (PID 0xFFFE)");
        assert_eq!(u.family, "Unknown");
        assert_eq!(u.support, "unknown");
        assert_eq!(u.serial, None);
        assert_eq!(u.key, "1B1C:FFFE:usb:USB-INSTANCE-1");
    }

    #[test]
    fn dto_counts_interfaces_per_device() {
        let list = detected_devices(&fixture(), |_| DeviceStatus::Ok);
        let b = list.iter().find(|d| d.key == "1B1C:0C3F:HUB-B").unwrap();
        assert_eq!(b.interface_count, 2);
    }

    #[test]
    fn dto_sorted_by_support_then_name_then_key() {
        let list = detected_devices(&fixture(), |_| DeviceStatus::Ok);
        let supports: Vec<&str> = list.iter().map(|d| d.support.as_str()).collect();
        assert_eq!(
            supports,
            ["supported", "supported", "supported", "detect_only", "unknown"]
        );
        // Within the supported group: (name, key) ascending.
        let supported: Vec<(&str, &str)> = list[..3]
            .iter()
            .map(|d| (d.name.as_str(), d.key.as_str()))
            .collect();
        let mut sorted = supported.clone();
        sorted.sort();
        assert_eq!(supported, sorted);
        // The two hubs share a name, so the key breaks the tie.
        let hubs: Vec<&str> = list
            .iter()
            .filter(|d| d.pid == HUB)
            .map(|d| d.key.as_str())
            .collect();
        assert_eq!(hubs, ["1B1C:0C3F:HUB-A", "1B1C:0C3F:HUB-B"]);
    }

    #[test]
    fn recheck_not_ready_clears_devices_that_became_ready() {
        let snap = fixture();
        let mut not_ready = devices_not_ready(&snap, |_| false);
        assert_eq!(not_ready.len(), 5);
        // DRAM becomes ready; every other device stays not ready.
        recheck_not_ready(&snap, &mut not_ready, |p| p.contains("0006"));
        assert_eq!(not_ready.len(), 4);
        assert!(!not_ready.iter().any(|k| k.pid == DRAM));
        // A key that left the snapshot is dropped too.
        let smaller = Snapshot::from_records([rec(HUB, Some("HUB-A"), 0, None)]);
        recheck_not_ready(&smaller, &mut not_ready, |_| false);
        assert_eq!(not_ready.len(), 1);
        assert!(not_ready.iter().all(|k| k.pid == HUB));
    }

    #[test]
    fn status_precedence_and_strings() {
        assert_eq!(DeviceStatus::resolve(false, false, true), DeviceStatus::Ok);
        assert_eq!(DeviceStatus::resolve(false, false, false), DeviceStatus::NotReady);
        assert_eq!(DeviceStatus::resolve(false, true, false), DeviceStatus::Backoff);
        assert_eq!(DeviceStatus::resolve(true, true, false), DeviceStatus::Quarantined);
        let strings: Vec<&str> = [
            DeviceStatus::Ok,
            DeviceStatus::NotReady,
            DeviceStatus::Quarantined,
            DeviceStatus::Backoff,
        ]
        .iter()
        .map(|s| s.as_str())
        .collect();
        assert_eq!(strings, ["ok", "not_ready", "quarantined", "backoff"]);
    }

    #[test]
    fn status_of_is_applied_per_device() {
        let list = detected_devices(&fixture(), |d| {
            if d.key.pid == DRAM {
                DeviceStatus::NotReady
            } else {
                DeviceStatus::Ok
            }
        });
        assert_eq!(list.iter().filter(|d| d.status == "not_ready").count(), 1);
    }

    #[test]
    fn excluded_count_comes_from_snapshot() {
        let mut snap = fixture();
        snap.excluded.push(corsair_hid::topology::Excluded {
            path: "bt".into(),
            reason: corsair_hid::topology::ExcludeReason::Bluetooth,
        });
        let tree = DeviceTree::from_discovery(&[], &[], None).with_detection(
            detected_devices(&snap, |_| DeviceStatus::Ok),
            snap.excluded.len() as u32,
        );
        assert_eq!(tree.excluded_count, 1);
        assert_eq!(tree.detected.len(), 5);
    }

    #[test]
    fn topology_events_one_per_device_with_kinds() {
        let old = fixture();
        let new = Snapshot::from_records([
            rec(HUB, Some("HUB-A"), 0, None),
            rec(HUB, Some("HUB-A"), 1, None), // changed (extra interface)
            rec(UNKNOWN, None, 0, Some("USB-INSTANCE-1")),
            rec(DRAM, Some("DRAM1"), 0, None),
            rec(PSU, Some("PSU1"), 0, None),
            rec(0x0A34, Some("NEW1"), 0, None), // added
                                                // HUB-B removed
        ]);
        let d = diff(&old, &new);
        let events = topology_events(&d, |_| DeviceStatus::Ok, 42);
        let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["added", "removed", "changed"]);
        assert!(events.iter().all(|e| e.timestamp_ms == 42 && e.device.is_some()));
        assert_eq!(events[0].device.as_ref().unwrap().key, "1B1C:0A34:NEW1");
        assert_eq!(events[1].device.as_ref().unwrap().key, "1B1C:0C3F:HUB-B");
        assert_eq!(events[2].device.as_ref().unwrap().interface_count, 2);
    }

    #[test]
    fn not_ready_devices_come_from_interface_readiness() {
        let snap = fixture();
        // HUB-B's second interface is not ready; everything else is.
        let bad = snap.devices.values().find(|d| d.key.serial.as_deref() == Some("HUB-B")).unwrap();
        let bad_path = bad.interfaces[1].path.clone();
        let set = devices_not_ready(&snap, |p| p != bad_path);
        assert_eq!(set.len(), 1);
        assert!(set.contains(&bad.key));
        assert!(devices_not_ready(&snap, |_| true).is_empty());
    }

    #[test]
    fn identical_snapshots_emit_no_events() {
        let d = diff(&fixture(), &fixture());
        assert!(topology_events(&d, |_| DeviceStatus::Ok, 0).is_empty());
    }

    #[test]
    fn power_event_has_no_device() {
        let e = TopologyEventDto::power("resumed", 7);
        assert_eq!(e.kind, "resumed");
        assert!(e.device.is_none());
    }

    fn link(channel: u8, id: &str) -> LinkDevice {
        LinkDevice {
            channel,
            device_type: LinkDeviceType::from_byte(0x01),
            model: 0,
            device_id: id.to_string(),
        }
    }

    #[test]
    fn link_dto_none_when_unchanged_some_when_different() {
        let a = vec![link(1, "A"), link(2, "B")];
        assert!(LinkSubdevicesChangedDto::from_diff("H", &diff_subdevices(&a, &a), 1).is_none());

        let b = vec![link(1, "A"), link(3, "B"), link(4, "C")];
        let dto = LinkSubdevicesChangedDto::from_diff("H", &diff_subdevices(&a, &b), 9).unwrap();
        assert_eq!(dto.hub_serial, "H");
        assert_eq!(dto.added, vec!["C"]);
        assert!(dto.removed.is_empty());
        assert_eq!(dto.moved.len(), 1);
        assert_eq!(
            (
                dto.moved[0].device_id.as_str(),
                dto.moved[0].from_channel,
                dto.moved[0].to_channel
            ),
            ("B", 2, 3)
        );
        assert_eq!(dto.timestamp_ms, 9);
    }
}
