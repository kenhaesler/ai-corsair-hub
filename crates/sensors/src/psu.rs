use anyhow::Result;
use corsair_common::Temperature;
use corsair_hid::CorsairPsu;

use crate::TemperatureSource;

/// Which PSU temperature reading to return.
#[derive(Debug, Clone, Copy)]
pub enum PsuReading {
    Vrm,
    Case,
}

/// PSU temperature sensor — wraps a `CorsairPsu` connection.
pub struct PsuSensor {
    name: String,
    psu: CorsairPsu,
    reading: PsuReading,
}

impl PsuSensor {
    pub fn new(psu: CorsairPsu, reading: PsuReading) -> Self {
        let name = match reading {
            PsuReading::Vrm => "PSU VRM".to_string(),
            PsuReading::Case => "PSU Case".to_string(),
        };
        Self { name, psu, reading }
    }
}

impl TemperatureSource for PsuSensor {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> Result<Temperature> {
        let celsius = match self.reading {
            PsuReading::Vrm => self.psu.read_temp_vrm()?,
            PsuReading::Case => self.psu.read_temp_case()?,
        };
        Ok(Temperature { celsius })
    }
}
