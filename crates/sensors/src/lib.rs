pub mod cpu;
pub mod gpu;

use corsair_common::Temperature;

pub trait TemperatureSource: Send + Sync {
    fn name(&self) -> &str;
    fn read(&self) -> anyhow::Result<Temperature>;
}
