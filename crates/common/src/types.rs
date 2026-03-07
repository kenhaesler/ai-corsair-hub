use serde::{Deserialize, Serialize};

pub const CORSAIR_VID: u16 = 0x1B1C;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CorsairDevice {
    IcueLinkHub,
    CommanderPro,
    CommanderCore,
    CommanderCoreXt,
    CommanderSt,
    LightingNodePro,
    Hx1500i,
    Unknown(u16),
}

impl CorsairDevice {
    pub fn from_pid(pid: u16) -> Self {
        match pid {
            0x0C3F => Self::IcueLinkHub,
            0x0C10 => Self::CommanderPro,
            0x0C1C => Self::CommanderCore,
            0x0C2A => Self::CommanderCoreXt,
            0x0C32 => Self::CommanderSt,
            0x0C0B => Self::LightingNodePro,
            0x1C1F => Self::Hx1500i,
            other => Self::Unknown(other),
        }
    }

    pub fn pid(&self) -> u16 {
        match self {
            Self::IcueLinkHub => 0x0C3F,
            Self::CommanderPro => 0x0C10,
            Self::CommanderCore => 0x0C1C,
            Self::CommanderCoreXt => 0x0C2A,
            Self::CommanderSt => 0x0C32,
            Self::LightingNodePro => 0x0C0B,
            Self::Hx1500i => 0x1C1F,
            Self::Unknown(pid) => *pid,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::IcueLinkHub => "iCUE LINK System Hub",
            Self::CommanderPro => "Commander Pro",
            Self::CommanderCore => "Commander Core",
            Self::CommanderCoreXt => "Commander Core XT",
            Self::CommanderSt => "Commander ST",
            Self::LightingNodePro => "Lighting Node Pro",
            Self::Hx1500i => "HX1500i PSU",
            Self::Unknown(_) => "Unknown Corsair Device",
        }
    }

    pub fn supports_fan_control(&self) -> bool {
        matches!(
            self,
            Self::IcueLinkHub
                | Self::CommanderPro
                | Self::CommanderCore
                | Self::CommanderCoreXt
                | Self::CommanderSt
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_type: CorsairDevice,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
    pub path: String,
    pub interface_number: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Temperature {
    pub celsius: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FanReading {
    pub channel: u8,
    pub rpm: u16,
    pub duty_percent: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FanTarget {
    pub channel: u8,
    pub duty_percent: f64,
}
