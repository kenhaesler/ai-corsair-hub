use serde::{Deserialize, Serialize};

use crate::Rgb;

/// Trait for RGB effects. Each effect renders LED colors based on positions and time.
pub trait RgbEffect: Send + Sync {
    /// Render colors for each LED. `positions` are normalized 0.0–1.0.
    /// `elapsed` is seconds since start. `ctx` carries live system state.
    fn render(&self, positions: &[f32], elapsed: f64, ctx: &EffectContext) -> Vec<Rgb>;

    /// Human-readable name for the effect.
    fn name(&self) -> &'static str;
}

/// Live system state passed to effects for reactive behavior.
#[derive(Debug, Clone, Default)]
pub struct EffectContext {
    /// Primary temperature source (°C).
    pub temperature: Option<f64>,
    /// Rate of change (°C/sec) — for spike detection.
    pub temp_delta: Option<f64>,
    /// Current fan duty (0–100).
    pub duty_percent: Option<f64>,
    /// All available sensors: (name, celsius).
    pub all_temps: Vec<(String, f64)>,
}

/// Serializable effect configuration — stored in config.toml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum EffectConfig {
    // === Solid & Basic ===
    Static {
        color: Rgb,
    },
    Breathing {
        color: Rgb,
        speed: f32,
    },

    // === Color Motion ===
    ColorCycle {
        speed: f32,
        saturation: f32,
    },
    RainbowWave {
        speed: f32,
        wavelength: f32,
    },
    SpectrumShift {
        speed: f32,
    },

    // === Procedural / Organic (noise-driven) ===
    Fire {
        intensity: f32,
        speed: f32,
    },
    Aurora {
        speed: f32,
        color_spread: f32,
    },
    Candle {
        color: Rgb,
        flicker_speed: f32,
    },
    Starfield {
        density: f32,
        speed: f32,
    },
    Rain {
        color: Rgb,
        speed: f32,
        density: f32,
    },

    // === Sensor-Reactive ===
    TemperatureMap {
        /// (temp_celsius, color) gradient stops
        gradient: Vec<(f64, Rgb)>,
        /// Flash on rapid temp change
        glow_on_spike: bool,
    },
    ThermalPulse {
        cold_color: Rgb,
        hot_color: Rgb,
        min_temp: f64,
        max_temp: f64,
    },
    DutyMeter {
        low_color: Rgb,
        high_color: Rgb,
    },

    // === User-defined ===
    Gradient {
        colors: Vec<Rgb>,
        speed: f32,
    },
}

impl EffectConfig {
    /// Create the runtime effect from this config.
    pub fn create_effect(&self) -> Box<dyn RgbEffect> {
        use crate::effects::*;

        match self.clone() {
            EffectConfig::Static { color } => Box::new(static_color::StaticEffect { color }),
            EffectConfig::Breathing { color, speed } => {
                Box::new(breathing::BreathingEffect { color, speed })
            }
            EffectConfig::ColorCycle { speed, saturation } => {
                Box::new(color_cycle::ColorCycleEffect { speed, saturation })
            }
            EffectConfig::RainbowWave { speed, wavelength } => {
                Box::new(rainbow_wave::RainbowWaveEffect { speed, wavelength })
            }
            EffectConfig::SpectrumShift { speed } => {
                Box::new(spectrum_shift::SpectrumShiftEffect { speed })
            }
            EffectConfig::Fire { intensity, speed } => {
                Box::new(fire::FireEffect { intensity, speed })
            }
            EffectConfig::Aurora { speed, color_spread } => {
                Box::new(aurora::AuroraEffect { speed, color_spread })
            }
            EffectConfig::Candle {
                color,
                flicker_speed,
            } => Box::new(candle::CandleEffect {
                color,
                flicker_speed,
            }),
            EffectConfig::Starfield { density, speed } => {
                Box::new(starfield::StarfieldEffect::new(density, speed))
            }
            EffectConfig::Rain {
                color,
                speed,
                density,
            } => Box::new(rain::RainEffect::new(color, speed, density)),
            EffectConfig::TemperatureMap {
                gradient,
                glow_on_spike,
            } => Box::new(temperature_map::TemperatureMapEffect {
                gradient,
                glow_on_spike,
            }),
            EffectConfig::ThermalPulse {
                cold_color,
                hot_color,
                min_temp,
                max_temp,
            } => Box::new(thermal_pulse::ThermalPulseEffect {
                cold_color,
                hot_color,
                min_temp,
                max_temp,
            }),
            EffectConfig::DutyMeter {
                low_color,
                high_color,
            } => Box::new(duty_meter::DutyMeterEffect {
                low_color,
                high_color,
            }),
            EffectConfig::Gradient { colors, speed } => {
                Box::new(gradient::GradientEffect { colors, speed })
            }
        }
    }
}
