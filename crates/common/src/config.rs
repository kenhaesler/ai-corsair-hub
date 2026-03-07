use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub fan_groups: Vec<FanGroupConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub poll_interval_ms: u64,
    pub log_level: String,
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
            },
            fan_groups: vec![],
        }
    }
}
