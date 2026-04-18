use serde::{Deserialize, Serialize};

/// Schema version constants.
///
/// V1 predates the identity refactor: fan groups and RGB zones are keyed by
/// `(hub_serial, channel)`. Channels are assigned by hub firmware based on
/// daisy-chain position and renumber whenever the physical chain changes.
///
/// V2 introduces device_id-first identity: groups/zones/overrides reference
/// the 26-hex `device_id` burned into each device at manufacturing. Channels
/// become an internal runtime detail, resolved at the USB boundary via
/// [`crate::identity::DeviceRegistry`].
pub const SCHEMA_VERSION_V1: u8 = 1;
pub const SCHEMA_VERSION_V2: u8 = 2;

/// Default schema version for configs written before the `schema_version`
/// field existed. Serde deserializes missing fields to this default.
fn default_schema_version() -> u8 {
    SCHEMA_VERSION_V1
}

/// The application configuration. Serves as both V1 and V2 during the
/// migration window.
///
/// ## Design: single struct, optional V2 fields (not a version-tagged enum)
///
/// We support both schema versions through a single `AppConfig` type with V2
/// fields wrapped in `Option` and/or `#[serde(default)]`. Reasoning:
///
/// 1. The control loop consumes one struct shape. A tagged enum would force a
///    match at every use-site or a conversion pass at load. The field-level
///    approach gives us a single type the runtime can reason about.
/// 2. Backward compatibility: a V1 file with no `schema_version` field
///    deserializes cleanly, with V2 fields defaulted.
/// 3. Forward compatibility: a V2 file can add `schema_version = 2`, populate
///    V2 fields, and the V1 fields can be left empty or kept populated during
///    the dual-write period. After the one-release deprecation window, V1
///    fields are deleted in a follow-up PR.
///
/// The cost is a somewhat-wider struct during the transition. The benefit is
/// that step 4 can add behavior incrementally without refactoring the type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Schema version. Missing (V1 configs written before this field existed)
    /// means V1. V2 configs set `schema_version = 2`.
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    pub general: GeneralConfig,
    #[serde(default)]
    pub fan_groups: Vec<FanGroupConfig>,
    #[serde(default)]
    pub rgb: RgbConfig,
    /// V1 per-device overrides for hub enumeration quirks, keyed by
    /// (hub_serial, channel). See [`DeviceOverride`]. Retained through the
    /// transition so V1 configs keep working.
    #[serde(default)]
    pub device_overrides: Vec<DeviceOverride>,
    /// V2 per-device metadata, keyed by device_id. See [`v2::DeviceEntry`].
    /// Empty on V1 configs.
    #[serde(default)]
    pub devices: Vec<v2::DeviceEntry>,
}

/// Manual override for a specific (hub, channel) device. Use when hub
/// enumeration misclassifies a device (e.g. LS350 strip reported as QX Fan)
/// and the wrong LED count corrupts the chain.
///
/// Example config snippet:
/// ```toml
/// [[device_overrides]]
/// hub_serial = "8B44BF040D45AA58B07DC6BC9E70E7EC"
/// channel = 15
/// led_count = 21
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceOverride {
    pub hub_serial: String,
    pub channel: u8,
    pub led_count: u16,
}

impl AppConfig {
    /// Return the LED count override for (hub_serial, channel) if any.
    pub fn led_count_override(&self, hub_serial: &str, channel: u8) -> Option<u16> {
        self.device_overrides
            .iter()
            .find(|o| o.hub_serial == hub_serial && o.channel == channel)
            .map(|o| o.led_count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub poll_interval_ms: u64,
    pub log_level: String,
    /// Optional path to LibreHardwareMonitor.exe for non-standard/portable installs.
    /// If `None`, auto-detects at the standard Program Files location.
    #[serde(default)]
    pub lhm_exe_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanGroupConfig {
    pub name: String,
    /// V1 identity: channels on `hub_serial`. Empty when the config is V2
    /// (see `device_ids`).
    #[serde(default)]
    pub channels: Vec<u8>,
    /// V1 identity: hub_serial that owns `channels`. Optional because a V2
    /// config omits it entirely (device_ids resolve across hubs via the
    /// runtime registry).
    #[serde(default)]
    pub hub_serial: Option<String>,
    /// V2 identity: stable device_ids. Empty on a V1 config; populated on a
    /// V2 config. When non-empty this is authoritative and `channels` /
    /// `hub_serial` are ignored by the control loop.
    #[serde(default)]
    pub device_ids: Vec<String>,
    pub mode: FanMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FanMode {
    #[serde(rename = "fixed")]
    Fixed { duty_percent: f64 },

    #[serde(rename = "curve")]
    Curve {
        points: Vec<CurvePoint>,
        hysteresis: f64,
        ramp_rate: f64,
        temp_source: TempSourceConfig,
    },

    #[serde(rename = "pid")]
    Pid {
        target_temp: f64,
        kp: f64,
        ki: f64,
        kd: f64,
        min_duty: f64,
        max_duty: f64,
        temp_source: TempSourceConfig,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub temp: f64,
    pub duty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempSourceConfig {
    pub sensors: Vec<String>,
    pub weights: Vec<f64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: vec![],
            rgb: RgbConfig::default(),
            device_overrides: vec![],
            devices: vec![],
        }
    }
}

impl AppConfig {
    /// Whether this config is in the V2 schema (device_id-first).
    pub fn is_v2(&self) -> bool {
        self.schema_version >= SCHEMA_VERSION_V2
    }

    /// Lookup V2 per-device entry by device_id.
    pub fn device_entry(&self, device_id: &str) -> Option<&v2::DeviceEntry> {
        self.devices.iter().find(|d| d.device_id == device_id)
    }

    /// V2 LED-count override lookup by device_id. Returns None when the
    /// device isn't listed or has no led_count override.
    pub fn led_count_override_by_id(&self, device_id: &str) -> Option<u16> {
        self.device_entry(device_id).and_then(|d| d.led_count)
    }
}

// --- RGB Configuration ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbConfig {
    pub enabled: bool,
    /// Master brightness (0–100).
    pub brightness: u8,
    /// Frames per second (30 or 60).
    pub fps: u8,
    /// If true, send RGB data to hardware. If false, preview-only.
    pub hardware_output: bool,
    pub zones: Vec<RgbZoneConfig>,
    #[serde(default)]
    pub presets: Vec<RgbPreset>,
}

impl Default for RgbConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            brightness: 80,
            fps: 30,
            hardware_output: false,
            zones: vec![],
            presets: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbZoneConfig {
    pub name: String,
    pub devices: Vec<RgbDeviceRef>,
    pub layers: Vec<LayerConfig>,
    pub brightness: u8,
    #[serde(default)]
    pub flow: Option<FlowConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    pub effect: EffectConfig,
    pub blend_mode: BlendMode,
    pub opacity: f32,
    pub enabled: bool,
}

/// V1 zone device reference. Carries both (hub_serial, channel) for the
/// legacy identity path and an optional `device_id` populated on V2 reads
/// (so a single `RgbZoneConfig.devices` array can hold V1 or V2 entries
/// during the transition).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbDeviceRef {
    #[serde(default)]
    pub hub_serial: String,
    #[serde(default)]
    pub channel: u8,
    /// V2 identity. Empty string on a V1 entry; populated on V2.
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbPreset {
    pub name: String,
    pub zones: Vec<RgbZoneConfig>,
}

// Re-export the types from corsair-rgb that config needs
pub use corsair_rgb::{BlendMode, EffectConfig, FlowConfig, FlowDirection, Rgb as RgbColor};

/// Schema V2 native types.
///
/// These mirror the V1 types but use `device_id` as the stable identity. The
/// V1→V2 migration (see `corsair_common::config_migration`) produces these
/// and the control loop consumes them through the dual-key path added in
/// Step 4.
///
/// During the transition `AppConfig` carries the V2 collections alongside the
/// V1 fields. Serde's `#[serde(default)]` makes each side optional so a
/// purely-V1 file still parses and a migrated V2 file still parses.
pub mod v2 {
    use super::FanMode;
    use serde::{Deserialize, Serialize};

    /// V2 fan group: membership keyed by device_id only. No hub_serial (the
    /// runtime registry resolves each device_id to its current hub).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FanGroupConfigV2 {
        pub name: String,
        pub device_ids: Vec<String>,
        pub mode: FanMode,
    }

    /// V2 zone device reference: device_id only.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RgbDeviceRefV2 {
        pub device_id: String,
    }

    /// Per-device metadata row, keyed by device_id. Both `name` and
    /// `led_count` are optional — a device not listed here uses its type
    /// default.
    ///
    /// This replaces V1's `DeviceOverride` (which was keyed by
    /// (hub_serial, channel)) — a device moved between hubs keeps its
    /// override because it follows the device_id.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct DeviceEntry {
        pub device_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub led_count: Option<u16>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V1 config without `schema_version` field parses as V1.
    #[test]
    fn v1_without_version_defaults_to_v1() {
        let toml = r#"
[general]
poll_interval_ms = 1000
log_level = "info"

[[fan_groups]]
name = "top_exhaust"
channels = [1, 2, 3]
hub_serial = "HUB_A"

[fan_groups.mode]
type = "fixed"
duty_percent = 50
"#;
        let cfg: AppConfig = toml::from_str(toml).expect("V1 parses");
        assert_eq!(cfg.schema_version, SCHEMA_VERSION_V1);
        assert!(!cfg.is_v2());
        assert_eq!(cfg.fan_groups.len(), 1);
        assert_eq!(cfg.fan_groups[0].channels, vec![1, 2, 3]);
        assert_eq!(cfg.fan_groups[0].hub_serial.as_deref(), Some("HUB_A"));
        // V2 fields default to empty
        assert!(cfg.fan_groups[0].device_ids.is_empty());
        assert!(cfg.devices.is_empty());
    }

    /// Explicit `schema_version = 1` is also recognized as V1.
    #[test]
    fn v1_with_explicit_version_parses() {
        let toml = r#"
schema_version = 1

[general]
poll_interval_ms = 1000
log_level = "info"
"#;
        let cfg: AppConfig = toml::from_str(toml).expect("V1 with explicit version parses");
        assert_eq!(cfg.schema_version, SCHEMA_VERSION_V1);
        assert!(!cfg.is_v2());
    }

    /// V2 config with `schema_version = 2` plus device_ids round-trips.
    #[test]
    fn v2_parses_and_round_trips() {
        let toml = r#"
schema_version = 2

[general]
poll_interval_ms = 1000
log_level = "info"

[[devices]]
device_id = "0100282F8203582BFB0002BCD1"
name = "Top Front Left"

[[devices]]
device_id = "0100095912036BF32B00004508"
name = "Aurora Strip Top"
led_count = 42

[[fan_groups]]
name = "top_exhaust"
device_ids = [
    "0100282F8203582BFB0002BCD1",
    "010012D852036E72B700000FE9",
]

[fan_groups.mode]
type = "fixed"
duty_percent = 60
"#;
        let cfg: AppConfig = toml::from_str(toml).expect("V2 parses");
        assert_eq!(cfg.schema_version, SCHEMA_VERSION_V2);
        assert!(cfg.is_v2());
        assert_eq!(cfg.devices.len(), 2);
        assert_eq!(cfg.devices[0].device_id, "0100282F8203582BFB0002BCD1");
        assert_eq!(cfg.devices[0].name.as_deref(), Some("Top Front Left"));
        assert_eq!(cfg.devices[0].led_count, None);
        assert_eq!(cfg.devices[1].led_count, Some(42));
        assert_eq!(cfg.fan_groups[0].device_ids.len(), 2);
        assert!(cfg.fan_groups[0].channels.is_empty());
        assert!(cfg.fan_groups[0].hub_serial.is_none());

        // Round-trip: serialize then reparse and compare semantic equality.
        let serialized = toml::to_string(&cfg).expect("serializes");
        let cfg2: AppConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(cfg2.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(cfg2.devices, cfg.devices);
        assert_eq!(cfg2.fan_groups[0].device_ids, cfg.fan_groups[0].device_ids);
    }

    /// Default AppConfig is schema V1.
    #[test]
    fn default_config_is_v1() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION_V1);
        assert!(!cfg.is_v2());
        assert!(cfg.devices.is_empty());
    }

    /// V2 LED-count override lookup by device_id resolves correctly.
    #[test]
    fn v2_led_count_override_by_id() {
        let toml = r#"
schema_version = 2

[general]
poll_interval_ms = 1000
log_level = "info"

[[devices]]
device_id = "0100095912036BF32B00004508"
led_count = 42
"#;
        let cfg: AppConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.led_count_override_by_id("0100095912036BF32B00004508"),
            Some(42)
        );
        assert_eq!(cfg.led_count_override_by_id("UNKNOWN_ID"), None);
    }

    /// V1 device_overrides still parse alongside V2 devices (dual-support
    /// during migration window).
    #[test]
    fn v1_device_overrides_coexist_with_v2_devices() {
        let toml = r#"
schema_version = 2

[general]
poll_interval_ms = 1000
log_level = "info"

[[device_overrides]]
hub_serial = "HUB_A"
channel = 15
led_count = 21

[[devices]]
device_id = "0100AAA"
led_count = 30
"#;
        let cfg: AppConfig = toml::from_str(toml).expect("coexistence parses");
        assert_eq!(cfg.device_overrides.len(), 1);
        assert_eq!(cfg.led_count_override("HUB_A", 15), Some(21));
        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(cfg.led_count_override_by_id("0100AAA"), Some(30));
    }

    /// V1 config round-trip preserves all fields.
    #[test]
    fn v1_round_trip_preserves_fields() {
        let original = AppConfig {
            schema_version: SCHEMA_VERSION_V1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: vec![FanGroupConfig {
                name: "g1".to_string(),
                channels: vec![1, 2],
                hub_serial: Some("HUB_A".to_string()),
                device_ids: vec![],
                mode: FanMode::Fixed { duty_percent: 50.0 },
            }],
            rgb: RgbConfig::default(),
            device_overrides: vec![DeviceOverride {
                hub_serial: "HUB_A".to_string(),
                channel: 5,
                led_count: 21,
            }],
            devices: vec![],
        };
        let serialized = toml::to_string(&original).expect("serializes");
        let reparsed: AppConfig = toml::from_str(&serialized).expect("reparses");
        assert_eq!(reparsed.schema_version, SCHEMA_VERSION_V1);
        assert_eq!(reparsed.fan_groups.len(), 1);
        assert_eq!(reparsed.fan_groups[0].channels, vec![1, 2]);
        assert_eq!(reparsed.device_overrides.len(), 1);
        assert_eq!(reparsed.device_overrides[0].led_count, 21);
    }
}
