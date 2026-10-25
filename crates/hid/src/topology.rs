//! Pure device topology: snapshots of what is plugged in, diffs between snapshots, and the
//! flood / failure-backoff policies the hotplug loop applies. No Windows or HID calls here.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use corsair_common::{CORSAIR_VID, CorsairDevice};

use crate::devnode::{DevnodeInfo, DevnodeStatus};

/// How long a path that failed init is skipped (clink `UpdateIgnoreTimeout`, D-clink-service.md 2.2B).
pub const FAILURE_IGNORE: Duration = Duration::from_secs(120);

/// A device's identity across replug and port moves.
///
/// Rule: the serial when present (not empty, not `unknown`), otherwise the composite USB
/// device instance id. This mirrors iCUE's `UniqueDeviceIds` `isSameDevice` (B-manifests.md 2.4):
/// both serials non-empty compare serials, otherwise fall back to a secondary id. A serial
/// survives moving the device to another port; the instance id (which embeds the port) is
/// only a fallback so serial-less devices with different parents never collapse into one.
///
/// `Eq`/`Ord`/`Hash` follow that rule, so `usb_instance_id` is ignored when a serial exists.
#[derive(Debug, Clone)]
pub struct DeviceKey {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub usb_instance_id: Option<String>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Identity<'a> {
    Serial(&'a str),
    Usb(&'a str),
    /// Neither available; serial-less devices of one PID can only be told apart as a group.
    Unresolved,
}

/// `None` for empty / `unknown` serials, otherwise the trimmed serial.
pub fn normalize_serial(serial: Option<&str>) -> Option<String> {
    let s = serial?.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(s.to_string())
    }
}

impl DeviceKey {
    pub fn new(vid: u16, pid: u16, serial: Option<&str>, usb_instance_id: Option<String>) -> Self {
        Self {
            vid,
            pid,
            serial: normalize_serial(serial),
            usb_instance_id,
        }
    }

    fn identity(&self) -> (u16, u16, Identity<'_>) {
        let id = match (&self.serial, &self.usb_instance_id) {
            (Some(s), _) => Identity::Serial(s),
            (None, Some(u)) => Identity::Usb(u),
            (None, None) => Identity::Unresolved,
        };
        (self.vid, self.pid, id)
    }
}

impl PartialEq for DeviceKey {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}
impl Eq for DeviceKey {}
impl Hash for DeviceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity().hash(state);
    }
}
impl PartialOrd for DeviceKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DeviceKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.identity().cmp(&other.identity())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Usb,
    Bluetooth,
    /// Could not be determined (no devnode information).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedInterface {
    pub interface_number: i32,
    pub path: String,
    pub usage_page: u16,
    pub usage: u16,
    pub instance_id: Option<String>,
    pub transport: Transport,
}

#[derive(Debug, Clone)]
pub struct DetectedDevice {
    pub key: DeviceKey,
    pub device: CorsairDevice,
    /// Sorted by (interface number, path).
    pub interfaces: Vec<DetectedInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExcludeReason {
    Bluetooth,
    DevnodeProblem(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excluded {
    pub path: String,
    pub reason: ExcludeReason,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub devices: BTreeMap<DeviceKey, DetectedDevice>,
    pub excluded: Vec<Excluded>,
}

/// The fields of a hidapi `DeviceInfo` that enumeration needs.
#[derive(Debug, Clone)]
pub struct RawInterface {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub interface_number: i32,
    pub path: String,
    pub usage_page: u16,
    pub usage: u16,
}

/// Devnode facts for one interface; all `None` when the lookup failed or is unavailable.
#[derive(Debug, Clone, Default)]
pub struct DevnodeContext {
    pub instance_id: Option<String>,
    pub info: Option<DevnodeInfo>,
    /// Composite USB device that owns the interface.
    pub usb_instance_id: Option<String>,
}

fn is_bluetooth(path: &str, ctx: &DevnodeContext) -> bool {
    let hit = |s: &str| {
        let u = s.to_ascii_uppercase();
        u.contains("BTHENUM") || u.contains("BTHLEDEVICE") || u.contains("{00001124-")
    };
    hit(path)
        || ctx.instance_id.as_deref().is_some_and(hit)
        || ctx.info.as_ref().is_some_and(|i| {
            i.enumerator.as_deref().is_some_and(hit)
                || i.parent_instance_id.as_deref().is_some_and(hit)
        })
}

impl Snapshot {
    /// Build a snapshot from enumerated interfaces plus their devnode context.
    /// Non-Corsair VIDs are dropped silently; Bluetooth and problem devnodes are recorded
    /// in `excluded`.
    pub fn from_records(records: impl IntoIterator<Item = (RawInterface, DevnodeContext)>) -> Self {
        let mut snap = Snapshot::default();

        for (raw, ctx) in records {
            if raw.vid != CORSAIR_VID {
                continue;
            }
            if is_bluetooth(&raw.path, &ctx) {
                snap.excluded.push(Excluded {
                    path: raw.path,
                    reason: ExcludeReason::Bluetooth,
                });
                continue;
            }
            if let Some(DevnodeStatus::Problem(code)) = ctx.info.as_ref().map(|i| i.status) {
                snap.excluded.push(Excluded {
                    path: raw.path,
                    reason: ExcludeReason::DevnodeProblem(code),
                });
                continue;
            }

            let transport = if ctx.usb_instance_id.is_some() {
                Transport::Usb
            } else {
                Transport::Unknown
            };
            let key = DeviceKey::new(
                raw.vid,
                raw.pid,
                raw.serial.as_deref(),
                ctx.usb_instance_id.clone(),
            );
            let iface = DetectedInterface {
                interface_number: raw.interface_number,
                path: raw.path,
                usage_page: raw.usage_page,
                usage: raw.usage,
                instance_id: ctx.instance_id,
                transport,
            };
            snap.devices
                .entry(key.clone())
                .or_insert_with(|| DetectedDevice {
                    key,
                    device: CorsairDevice::from_pid(raw.pid),
                    interfaces: Vec::new(),
                })
                .interfaces
                .push(iface);
        }

        for dev in snap.devices.values_mut() {
            dev.interfaces
                .sort_by(|a, b| (a.interface_number, &a.path).cmp(&(b.interface_number, &b.path)));
        }
        snap
    }
}

#[derive(Debug, Clone)]
pub struct ChangedDevice {
    pub old: DetectedDevice,
    pub new: DetectedDevice,
}

#[derive(Debug, Clone, Default)]
pub struct TopologyDiff {
    pub added: Vec<DetectedDevice>,
    pub removed: Vec<DetectedDevice>,
    pub changed: Vec<ChangedDevice>,
}

impl TopologyDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

fn iface_set(d: &DetectedDevice) -> Vec<(i32, &str)> {
    d.interfaces
        .iter()
        .map(|i| (i.interface_number, i.path.as_str()))
        .collect()
}

/// Compare two snapshots. `changed` = same key, different interface numbers or paths.
pub fn diff(old: &Snapshot, new: &Snapshot) -> TopologyDiff {
    let mut out = TopologyDiff::default();

    for (key, n) in &new.devices {
        match old.devices.get(key) {
            None => out.added.push(n.clone()),
            Some(o) if iface_set(o) != iface_set(n) => out.changed.push(ChangedDevice {
                old: o.clone(),
                new: n.clone(),
            }),
            Some(_) => {}
        }
    }
    for (key, o) in &old.devices {
        if !new.devices.contains_key(key) {
            out.removed.push(o.clone());
        }
    }
    out
}

/// Quarantines a device that re-arrives more than `max` times within `window` (a flapping
/// cable or a driver reset loop). Rejected arrivals still count, so a device that keeps
/// flapping stays quarantined until it has been quiet for a full window.
#[derive(Debug)]
pub struct FloodGuard {
    max: usize,
    window: Duration,
    arrivals: HashMap<DeviceKey, VecDeque<Instant>>,
}

impl FloodGuard {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            arrivals: HashMap::new(),
        }
    }

    fn prune(&mut self, key: &DeviceKey, now: Instant) -> &mut VecDeque<Instant> {
        let window = self.window;
        let q = self.arrivals.entry(key.clone()).or_default();
        while q
            .front()
            .is_some_and(|&t| now.saturating_duration_since(t) >= window)
        {
            q.pop_front();
        }
        q
    }

    /// Record an arrival. Returns `true` when it may be processed, `false` when quarantined.
    pub fn record_arrival(&mut self, key: &DeviceKey, now: Instant) -> bool {
        let max = self.max;
        let q = self.prune(key, now);
        q.push_back(now);
        q.len() <= max
    }

    pub fn is_quarantined(&mut self, key: &DeviceKey, now: Instant) -> bool {
        let max = self.max;
        self.prune(key, now).len() > max
    }
}

/// Paths whose init failed are ignored for [`FAILURE_IGNORE`] (default) to stop thrashing a
/// device that another application owns.
#[derive(Debug)]
pub struct FailureBackoff {
    ignore_for: Duration,
    until: HashMap<String, Instant>,
}

impl Default for FailureBackoff {
    fn default() -> Self {
        Self::new(FAILURE_IGNORE)
    }
}

impl FailureBackoff {
    pub fn new(ignore_for: Duration) -> Self {
        Self {
            ignore_for,
            until: HashMap::new(),
        }
    }

    pub fn record_failure(&mut self, path: &str, now: Instant) {
        self.until.insert(path.to_string(), now + self.ignore_for);
    }

    pub fn is_ignored(&self, path: &str, now: Instant) -> bool {
        self.until.get(path).is_some_and(|&t| now < t)
    }

    /// Forget a path (it initialised successfully or vanished).
    pub fn clear(&mut self, path: &str) {
        self.until.remove(path);
    }

    /// Drop expired entries.
    pub fn prune(&mut self, now: Instant) {
        self.until.retain(|_, &mut t| now < t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUB: u16 = 0x0C3F;

    fn raw(pid: u16, serial: Option<&str>, iface: i32, path: &str) -> RawInterface {
        RawInterface {
            vid: CORSAIR_VID,
            pid,
            serial: serial.map(String::from),
            interface_number: iface,
            path: path.into(),
            usage_page: 0xFF42,
            usage: 1,
        }
    }

    fn ctx(usb: Option<&str>, status: DevnodeStatus) -> DevnodeContext {
        DevnodeContext {
            instance_id: Some("HID\\X".into()),
            info: Some(DevnodeInfo {
                status,
                enumerator: Some("HID".into()),
                parent_instance_id: usb.map(String::from),
            }),
            usb_instance_id: usb.map(String::from),
        }
    }

    fn usb_ctx(usb: &str) -> DevnodeContext {
        ctx(Some(usb), DevnodeStatus::Started)
    }

    fn snap(recs: Vec<(RawInterface, DevnodeContext)>) -> Snapshot {
        Snapshot::from_records(recs)
    }

    #[test]
    fn identity_prefers_serial_over_instance() {
        let a = DeviceKey::new(CORSAIR_VID, HUB, Some("ABC"), Some("USB\\A".into()));
        let b = DeviceKey::new(CORSAIR_VID, HUB, Some("ABC"), Some("USB\\B".into()));
        assert_eq!(a, b, "port move keeps identity when serial present");
        assert_eq!(a.cmp(&b), Ordering::Equal);
    }

    #[test]
    fn identity_falls_back_for_empty_and_unknown_serial() {
        for s in [None, Some(""), Some("  "), Some("unknown"), Some("UNKNOWN")] {
            let a = DeviceKey::new(CORSAIR_VID, HUB, s, Some("USB\\A".into()));
            let b = DeviceKey::new(CORSAIR_VID, HUB, s, Some("USB\\B".into()));
            assert_ne!(a, b, "{s:?}");
            assert_eq!(a.serial, None);
        }
    }

    #[test]
    fn different_pid_or_serial_differ() {
        let a = DeviceKey::new(CORSAIR_VID, HUB, Some("A"), None);
        assert_ne!(a, DeviceKey::new(CORSAIR_VID, HUB, Some("B"), None));
        assert_ne!(a, DeviceKey::new(CORSAIR_VID, 0x1C1F, Some("A"), None));
    }

    #[test]
    fn groups_multi_interface_device() {
        let s = snap(vec![
            (raw(HUB, Some("S1"), 1, "p1"), usb_ctx("USB\\A")),
            (raw(HUB, Some("S1"), 0, "p0"), usb_ctx("USB\\A")),
        ]);
        assert_eq!(s.devices.len(), 1);
        let d = s.devices.values().next().unwrap();
        assert_eq!(d.device, CorsairDevice::IcueLinkHub);
        let nums: Vec<_> = d.interfaces.iter().map(|i| i.interface_number).collect();
        assert_eq!(nums, [0, 1]);
        assert_eq!(d.interfaces[0].transport, Transport::Usb);
    }

    #[test]
    fn serialless_devices_with_different_parents_do_not_collide() {
        let s = snap(vec![
            (raw(HUB, None, 0, "a0"), usb_ctx("USB\\A")),
            (raw(HUB, Some("unknown"), 1, "a1"), usb_ctx("USB\\A")),
            (raw(HUB, None, 0, "b0"), usb_ctx("USB\\B")),
        ]);
        assert_eq!(s.devices.len(), 2);
        let counts: Vec<_> = s.devices.values().map(|d| d.interfaces.len()).collect();
        assert_eq!(counts, [2, 1]);
    }

    #[test]
    fn excludes_bluetooth_and_problem_devnodes() {
        let bt_path = r"\\?\hid#{00001124-0000-1000-8000-00805f9b34fb}_vid&1b1c#9&aa#{g}";
        let s = snap(vec![
            (raw(HUB, Some("S1"), 0, "ok"), usb_ctx("USB\\A")),
            (raw(0x1B2D, Some("BT"), -1, bt_path), DevnodeContext::default()),
            (
                raw(HUB, Some("S2"), 0, "bad"),
                ctx(Some("USB\\B"), DevnodeStatus::Problem(10)),
            ),
        ]);
        assert_eq!(s.devices.len(), 1);
        assert_eq!(
            s.excluded,
            vec![
                Excluded { path: bt_path.into(), reason: ExcludeReason::Bluetooth },
                Excluded { path: "bad".into(), reason: ExcludeReason::DevnodeProblem(10) },
            ]
        );
    }

    #[test]
    fn bluetooth_detected_via_enumerator() {
        let c = DevnodeContext {
            info: Some(DevnodeInfo {
                status: DevnodeStatus::Started,
                enumerator: Some("BTHLEDEVICE".into()),
                parent_instance_id: None,
            }),
            ..Default::default()
        };
        let s = snap(vec![(raw(HUB, Some("S"), 0, "plain"), c)]);
        assert!(s.devices.is_empty());
        assert_eq!(s.excluded[0].reason, ExcludeReason::Bluetooth);
    }

    #[test]
    fn non_corsair_vid_is_dropped_silently() {
        let mut r = raw(HUB, Some("S"), 0, "p");
        r.vid = 0x046D;
        let s = snap(vec![(r, usb_ctx("USB\\A"))]);
        assert!(s.devices.is_empty() && s.excluded.is_empty());
    }

    #[test]
    fn missing_devnode_info_is_kept_with_unknown_transport() {
        let s = snap(vec![(raw(HUB, Some("S"), 0, "p"), DevnodeContext::default())]);
        let d = s.devices.values().next().unwrap();
        assert_eq!(d.interfaces[0].transport, Transport::Unknown);
    }

    fn base() -> Snapshot {
        snap(vec![
            (raw(HUB, Some("S1"), 0, "p0"), usb_ctx("USB\\A")),
            (raw(HUB, Some("S1"), 1, "p1"), usb_ctx("USB\\A")),
            (raw(0x1C1F, Some("PSU"), 0, "psu"), usb_ctx("USB\\P")),
        ])
    }

    #[test]
    fn diff_noop() {
        assert!(diff(&base(), &base()).is_empty());
    }

    #[test]
    fn diff_added_and_removed() {
        let old = base();
        let new = snap(vec![
            (raw(HUB, Some("S1"), 0, "p0"), usb_ctx("USB\\A")),
            (raw(HUB, Some("S1"), 1, "p1"), usb_ctx("USB\\A")),
            (raw(HUB, Some("S2"), 0, "q0"), usb_ctx("USB\\C")),
        ]);
        let d = diff(&old, &new);
        assert_eq!(d.added.len(), 1);
        assert_eq!(d.added[0].key.serial.as_deref(), Some("S2"));
        assert_eq!(d.removed.len(), 1);
        assert_eq!(d.removed[0].key.serial.as_deref(), Some("PSU"));
        assert!(d.changed.is_empty());
    }

    #[test]
    fn diff_changed_on_interface_set_or_path() {
        let old = base();
        let fewer = snap(vec![
            (raw(HUB, Some("S1"), 0, "p0"), usb_ctx("USB\\A")),
            (raw(0x1C1F, Some("PSU"), 0, "psu"), usb_ctx("USB\\P")),
        ]);
        assert_eq!(diff(&old, &fewer).changed.len(), 1);

        let repathed = snap(vec![
            (raw(HUB, Some("S1"), 0, "p0"), usb_ctx("USB\\A")),
            (raw(HUB, Some("S1"), 1, "p1-new"), usb_ctx("USB\\A")),
            (raw(0x1C1F, Some("PSU"), 0, "psu"), usb_ctx("USB\\P")),
        ]);
        let d = diff(&old, &repathed);
        assert_eq!(d.changed.len(), 1);
        assert!(d.added.is_empty() && d.removed.is_empty());
    }

    #[test]
    fn diff_port_move_with_serial_is_changed_not_replaced() {
        let old = snap(vec![(raw(HUB, Some("S1"), 0, "old"), usb_ctx("USB\\A"))]);
        let new = snap(vec![(raw(HUB, Some("S1"), 0, "new"), usb_ctx("USB\\Z"))]);
        let d = diff(&old, &new);
        assert_eq!((d.added.len(), d.removed.len(), d.changed.len()), (0, 0, 1));
    }

    fn key(s: &str) -> DeviceKey {
        DeviceKey::new(CORSAIR_VID, HUB, Some(s), None)
    }

    #[test]
    fn flood_guard_quarantines_and_recovers() {
        let t0 = Instant::now();
        let mut g = FloodGuard::new(3, Duration::from_secs(10));
        let k = key("A");
        for i in 0..3 {
            assert!(g.record_arrival(&k, t0 + Duration::from_secs(i)));
        }
        assert!(!g.record_arrival(&k, t0 + Duration::from_secs(3)));
        assert!(g.is_quarantined(&k, t0 + Duration::from_secs(4)));
        // Other devices unaffected.
        assert!(g.record_arrival(&key("B"), t0 + Duration::from_secs(4)));
        // Quiet for a full window: recovers.
        let later = t0 + Duration::from_secs(14);
        assert!(!g.is_quarantined(&k, later));
        assert!(g.record_arrival(&k, later));
    }

    #[test]
    fn flood_guard_arrivals_spread_over_window_are_fine() {
        let t0 = Instant::now();
        let mut g = FloodGuard::new(2, Duration::from_secs(10));
        let k = key("A");
        for i in 0..6 {
            assert!(g.record_arrival(&k, t0 + Duration::from_secs(i * 6)));
        }
    }

    #[test]
    fn backoff_ignores_until_expiry() {
        let t0 = Instant::now();
        let mut b = FailureBackoff::default();
        assert!(!b.is_ignored("p", t0));
        b.record_failure("p", t0);
        assert!(b.is_ignored("p", t0 + Duration::from_secs(119)));
        assert!(!b.is_ignored("p", t0 + Duration::from_secs(120)));
        assert!(!b.is_ignored("other", t0));
    }

    #[test]
    fn backoff_clear_and_prune() {
        let t0 = Instant::now();
        let mut b = FailureBackoff::new(Duration::from_secs(5));
        b.record_failure("a", t0);
        b.record_failure("b", t0);
        b.clear("a");
        assert!(!b.is_ignored("a", t0));
        b.prune(t0 + Duration::from_secs(6));
        assert!(!b.is_ignored("b", t0 + Duration::from_secs(1)));
    }
}
