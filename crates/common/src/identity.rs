//! Stable device identity resolver.
//!
//! iCUE LINK devices have a 26-hex-char `device_id` burned in at manufacturing
//! (e.g. `"0100224A020369939600000D6E"`). The channel number assigned by the
//! hub firmware, in contrast, depends on daisy-chain position — reshuffle the
//! chain and channel assignments change. Any config that persists
//! `(hub_serial, channel)` bindings therefore breaks silently after a topology
//! change.
//!
//! `DeviceRegistry` maps `device_id → (hub_serial, channel)` for the current
//! runtime, rebuilt every time hub enumeration changes. Callers persist
//! device_ids only; the registry is the runtime bridge back to the channel
//! number the wire protocol speaks.
//!
//! ## Why this lives in `corsair-common` and takes erased inputs
//!
//! `DeviceRegistry::rebuild` must not import `LinkDevice`/`HubInfo` from
//! `corsair-hid` — that would create a dependency cycle (`hid` already
//! depends on `common`). Instead `rebuild` takes a lower-level iterator of
//! `(hub_serial, device_id, channel, device_type_byte, led_count)` tuples
//! and stores only the type-erased byte. Consumers in `fancontrol`/`hid` can
//! recover `LinkDeviceType` via `LinkDeviceType::from_byte(device_type_byte)`.

use std::collections::HashMap;

/// Current physical location of a device on the iCUE LINK bus.
///
/// `device_type_byte` is the raw protocol byte (see `LinkDeviceType::from_byte`
/// in `corsair-hid`). `common` cannot depend on `hid`, so the byte is stored
/// type-erased and callers decode it themselves.
#[derive(Debug, Clone)]
pub struct DeviceLocation {
    pub hub_serial: String,
    pub channel: u8,
    pub device_type_byte: u8,
    pub led_count: u16,
}

/// Bidirectional index between stable `device_id` and current `(hub_serial,
/// channel)`. Rebuilt at every hub enumeration.
#[derive(Debug, Default, Clone)]
pub struct DeviceRegistry {
    /// device_id → current location.
    by_id: HashMap<String, DeviceLocation>,
    /// (hub_serial, channel) → device_id.
    by_location: HashMap<(String, u8), String>,
}

/// A single enumerated device as observed from a hub. Input to
/// `DeviceRegistry::rebuild`.
///
/// Kept minimal and `Copy`-able-ish (strings borrowed) so callers in
/// `corsair-hid` / `corsair-fancontrol` can build an iterator over their
/// `HubInfo.devices` without allocating.
#[derive(Debug, Clone, Copy)]
pub struct DeviceEnumEntry<'a> {
    pub hub_serial: &'a str,
    pub device_id: &'a str,
    pub channel: u8,
    pub device_type_byte: u8,
    pub led_count: u16,
}

impl DeviceRegistry {
    /// Empty registry. Useful as a placeholder before hubs are enumerated.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild both indexes from an iterator of enumerated devices.
    ///
    /// Takes an iterator of `DeviceEnumEntry` so the input source (hub info,
    /// mock data, config-sourced metadata) stays flexible. Duplicate
    /// device_ids in the input are a protocol anomaly — last-wins semantics
    /// here; the `config_migration` path detects and refuses migration when
    /// that happens.
    pub fn rebuild<'a, I>(entries: I) -> Self
    where
        I: IntoIterator<Item = DeviceEnumEntry<'a>>,
    {
        let mut by_id = HashMap::new();
        let mut by_location = HashMap::new();
        for e in entries {
            let loc = DeviceLocation {
                hub_serial: e.hub_serial.to_string(),
                channel: e.channel,
                device_type_byte: e.device_type_byte,
                led_count: e.led_count,
            };
            by_id.insert(e.device_id.to_string(), loc);
            by_location.insert(
                (e.hub_serial.to_string(), e.channel),
                e.device_id.to_string(),
            );
        }
        Self {
            by_id,
            by_location,
        }
    }

    /// Look up a device's current location.
    pub fn resolve(&self, device_id: &str) -> Option<&DeviceLocation> {
        self.by_id.get(device_id)
    }

    /// Convenience: `(hub_serial, channel)` pair for sending to the wire.
    /// Owns the returned strings; cheap to clone in control-loop hot paths.
    pub fn channel_for(&self, device_id: &str) -> Option<(String, u8)> {
        self.by_id
            .get(device_id)
            .map(|loc| (loc.hub_serial.clone(), loc.channel))
    }

    /// Reverse lookup: what device is currently at `(hub_serial, channel)`?
    pub fn device_id_at(&self, hub_serial: &str, channel: u8) -> Option<&str> {
        self.by_location
            .get(&(hub_serial.to_string(), channel))
            .map(String::as_str)
    }

    /// Iterate all `device_id → location` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &DeviceLocation)> {
        self.by_id.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of devices currently indexed.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry holds no devices.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// All known device_ids (iteration order unspecified).
    pub fn device_ids(&self) -> impl Iterator<Item = &str> {
        self.by_id.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(
        hub: &'a str,
        id: &'a str,
        channel: u8,
        type_byte: u8,
        leds: u16,
    ) -> DeviceEnumEntry<'a> {
        DeviceEnumEntry {
            hub_serial: hub,
            device_id: id,
            channel,
            device_type_byte: type_byte,
            led_count: leds,
        }
    }

    #[test]
    fn rebuild_populates_both_indexes() {
        let reg = DeviceRegistry::rebuild([
            entry("HUB_A", "0100AAA", 1, 0x01, 34),
            entry("HUB_A", "0100BBB", 2, 0x05, 21),
        ]);

        // Forward: device_id → location
        let a = reg.resolve("0100AAA").expect("A should resolve");
        assert_eq!(a.hub_serial, "HUB_A");
        assert_eq!(a.channel, 1);
        assert_eq!(a.device_type_byte, 0x01);
        assert_eq!(a.led_count, 34);

        // channel_for helper returns owned tuple
        let (hub, ch) = reg.channel_for("0100BBB").expect("B should resolve");
        assert_eq!(hub, "HUB_A");
        assert_eq!(ch, 2);

        // Reverse: (hub, channel) → device_id
        assert_eq!(reg.device_id_at("HUB_A", 1), Some("0100AAA"));
        assert_eq!(reg.device_id_at("HUB_A", 2), Some("0100BBB"));

        assert_eq!(reg.len(), 2);
        assert!(!reg.is_empty());
    }

    #[test]
    fn resolve_missing_returns_none() {
        let reg = DeviceRegistry::rebuild([entry("HUB_A", "0100AAA", 1, 0x01, 34)]);

        assert!(reg.resolve("0100ZZZ").is_none());
        assert!(reg.channel_for("0100ZZZ").is_none());
        assert!(reg.device_id_at("HUB_A", 99).is_none());
        assert!(reg.device_id_at("HUB_MISSING", 1).is_none());
    }

    #[test]
    fn cross_hub_devices_indexed_correctly() {
        // Two hubs, each with channel=1 — the (hub, channel) key must
        // disambiguate correctly so a lookup on HUB_A/1 never returns
        // HUB_B/1's device_id and vice versa.
        let reg = DeviceRegistry::rebuild([
            entry("HUB_A", "ID_ON_A", 1, 0x01, 34),
            entry("HUB_B", "ID_ON_B", 1, 0x01, 34),
            entry("HUB_A", "ID_ON_A_CH2", 2, 0x01, 34),
        ]);

        assert_eq!(reg.device_id_at("HUB_A", 1), Some("ID_ON_A"));
        assert_eq!(reg.device_id_at("HUB_B", 1), Some("ID_ON_B"));
        assert_eq!(reg.device_id_at("HUB_A", 2), Some("ID_ON_A_CH2"));

        let a = reg.resolve("ID_ON_A").unwrap();
        let b = reg.resolve("ID_ON_B").unwrap();
        assert_eq!(a.hub_serial, "HUB_A");
        assert_eq!(b.hub_serial, "HUB_B");
        assert_eq!(a.channel, 1);
        assert_eq!(b.channel, 1);

        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn empty_registry_behaves() {
        let reg = DeviceRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.resolve("anything").is_none());
        assert!(reg.channel_for("anything").is_none());
        assert_eq!(reg.iter().count(), 0);
        assert_eq!(reg.device_ids().count(), 0);
    }
}
