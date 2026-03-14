use anyhow::{Context, Result};
use corsair_common::Temperature;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use tracing::debug;

use crate::TemperatureSource;

/// Extended GPU metrics beyond just temperature.
#[derive(Debug, Clone)]
pub struct GpuMetrics {
    pub temp_celsius: f64,
    pub power_watts: f64,
    pub clock_mhz: u32,
    pub memory_used_mb: u64,
    pub utilization_pct: u32,
}

/// GPU temperature sensor using NVIDIA NVML.
///
/// Initializes NVML once and caches the device index. Each read is a direct
/// driver call via shared memory (~0.01ms).
pub struct GpuSensor {
    name: String,
    nvml: Nvml,
    device_index: u32,
}

impl GpuSensor {
    pub fn new() -> Result<Self> {
        Self::with_index(0)
    }

    pub fn with_index(device_index: u32) -> Result<Self> {
        let nvml = Nvml::init().context(
            "Failed to initialize NVML. Ensure NVIDIA drivers are installed \
             and nvml.dll is accessible.",
        )?;

        // Verify the device exists
        let device = nvml
            .device_by_index(device_index)
            .context("Failed to get GPU device by index")?;

        let gpu_name = device.name().unwrap_or_else(|_| "NVIDIA GPU".to_string());
        debug!(gpu = gpu_name.as_str(), index = device_index, "GPU sensor initialized");

        Ok(Self {
            name: format!("GPU {}", gpu_name),
            nvml,
            device_index,
        })
    }

    /// Read extended GPU metrics (temp, power, clock, memory, utilization).
    pub fn read_metrics(&self) -> Result<GpuMetrics> {
        let device = self
            .nvml
            .device_by_index(self.device_index)
            .context("Failed to get GPU device")?;

        let temp = device
            .temperature(TemperatureSensor::Gpu)
            .context("Failed to read GPU temperature")?;

        // Power usage is in milliwatts
        let power_mw = device.power_usage().unwrap_or(0);

        // Graphics clock in MHz
        let clock = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
            .unwrap_or(0);

        // Memory info
        let mem = device.memory_info();
        let memory_used_mb = mem.as_ref().map(|m| m.used / 1_048_576).unwrap_or(0);

        // GPU utilization percentage
        let util = device.utilization_rates();
        let utilization_pct = util.as_ref().map(|u| u.gpu).unwrap_or(0);

        Ok(GpuMetrics {
            temp_celsius: temp as f64,
            power_watts: power_mw as f64 / 1000.0,
            clock_mhz: clock,
            memory_used_mb,
            utilization_pct,
        })
    }
}

impl TemperatureSource for GpuSensor {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> Result<Temperature> {
        let device = self
            .nvml
            .device_by_index(self.device_index)
            .context("Failed to get GPU device")?;

        let temp = device
            .temperature(TemperatureSensor::Gpu)
            .context("Failed to read GPU temperature")?;

        Ok(Temperature {
            celsius: temp as f64,
        })
    }
}
