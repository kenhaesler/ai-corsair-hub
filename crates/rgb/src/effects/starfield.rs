use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

/// Random sparkles that fade in and out.
pub struct StarfieldEffect {
    density: f32,
    speed: f32,
    /// Deterministic seed per LED — generated from density.
    seeds: Vec<(f64, f64)>, // (birth_offset, lifetime)
}

impl StarfieldEffect {
    pub fn new(density: f32, speed: f32) -> Self {
        // Pre-generate seeds for up to 512 LEDs
        let seeds: Vec<(f64, f64)> = (0..512)
            .map(|i| {
                // Simple hash for deterministic pseudo-random
                let h = ((i as u64).wrapping_mul(2654435761) ^ 0xDEADBEEF) as f64 / u64::MAX as f64;
                let lifetime = 0.5 + h * 1.5; // 0.5–2.0 seconds
                let birth = h * 3.0; // staggered start
                (birth, lifetime)
            })
            .collect();

        Self { density, speed, seeds }
    }
}

impl RgbEffect for StarfieldEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        let cycle_period = 3.0 / self.speed as f64;

        positions
            .iter()
            .enumerate()
            .map(|(i, _pos)| {
                let (birth, lifetime) = self.seeds.get(i).copied().unwrap_or((0.0, 1.0));
                let lifetime = lifetime / self.speed as f64;

                // Repeat the sparkle cycle
                let t_in_cycle = (elapsed - birth).rem_euclid(cycle_period);

                if t_in_cycle > lifetime {
                    return Rgb::BLACK;
                }

                // Use density to determine if this LED is active
                let hash = ((i as u64).wrapping_mul(6364136223846793005) >> 32) as f64
                    / u32::MAX as f64;
                if hash > self.density as f64 {
                    return Rgb::BLACK;
                }

                // Fade curve: quick rise, slow fall
                let progress = t_in_cycle / lifetime;
                let brightness = if progress < 0.2 {
                    (progress / 0.2) as f32
                } else {
                    (1.0 - (progress - 0.2) / 0.8) as f32
                };

                Rgb::WHITE.dim(brightness.clamp(0.0, 1.0))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Starfield"
    }
}
