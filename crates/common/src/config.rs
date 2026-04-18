use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub fan_groups: Vec<FanGroupConfig>,
    #[serde(default)]
    pub rgb: RgbConfig,
    /// Per-device overrides for hub enumeration quirks. See [`DeviceOverride`].
    #[serde(default)]
    pub device_overrides: Vec<DeviceOverride>,
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
    pub channels: Vec<u8>,
    pub hub_serial: Option<String>,
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
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: vec![],
            rgb: RgbConfig::default(),
            device_overrides: vec![],
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbDeviceRef {
    pub hub_serial: String,
    pub channel: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbPreset {
    pub name: String,
    pub zones: Vec<RgbZoneConfig>,
}

// Re-export the types from corsair-rgb that config needs
pub use corsair_rgb::{BlendMode, EffectConfig, FlowConfig, FlowDirection, Rgb as RgbColor};
