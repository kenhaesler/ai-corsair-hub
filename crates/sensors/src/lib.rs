pub mod cpu;
pub mod gpu;
pub mod psu;

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
