use anyhow::Result;
use corsair_common::Temperature;

use crate::TemperatureSource;

pub struct CpuSensor {
    name: String,
}

impl CpuSensor {
    pub fn new() -> Result<Self> {
        // TODO: Initialize WMI or LibreHardwareMonitor connection
        Ok(Self {
            name: "CPU Tctl".to_string(),
        })
    }
}

impl TemperatureSource for CpuSensor {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> Result<Temperature> {
        // TODO: Read actual CPU temperature via WMI/LHWM
        // For now, return a placeholder
        tracing::warn!("CPU sensor not yet implemented, returning dummy value");
        Ok(Temperature { celsius: 45.0 })
    }
}
