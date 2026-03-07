use anyhow::Result;
use corsair_common::Temperature;

use crate::TemperatureSource;

pub struct GpuSensor {
    name: String,
}

impl GpuSensor {
    pub fn new() -> Result<Self> {
        // TODO: Initialize NVML for GPU temp reading
        Ok(Self {
            name: "GPU Core".to_string(),
        })
    }
}

impl TemperatureSource for GpuSensor {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> Result<Temperature> {
        // TODO: Read actual GPU temperature via NVML
        tracing::warn!("GPU sensor not yet implemented, returning dummy value");
        Ok(Temperature { celsius: 55.0 })
    }
}
