pub mod cpu;
pub mod gpu;
pub mod lhm;
pub mod lhm_registry;
pub mod lhm_temps;
pub mod psu;

pub use gpu::{GpuMetrics, GpuSensor};
pub use lhm::CpuMetrics;
pub use lhm_registry::{LhmCache, LhmKind, LhmSensor};
pub use lhm_temps::{discover_lhm_temp_sources, LhmTempSource, LHM_TEMP_SPECS};

use corsair_common::Temperature;

/// A source of temperature readings.
///
/// Note: Not required to be Send/Sync. Sensors use platform handles
/// (WMI COM objects, HID devices) that are inherently single-threaded.
/// The fan control loop will own sensors on a single thread.
pub trait TemperatureSource {
    fn name(&self) -> &str;
    fn read(&self) -> anyhow::Result<Temperature>;
}
