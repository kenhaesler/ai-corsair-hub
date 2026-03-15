use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

/// Rain droplets moving along positions, spawning and despawning.
pub struct RainEffect {
    color: Rgb,
    speed: f32,
    density: f32,
    /// Pre-computed droplet parameters: (start_position, birth_time_offset).
    droplets: Vec<(f64, f64)>,
}

impl RainEffect {
    pub fn new(color: Rgb, speed: f32, density: f32) -> Self {
        let num_droplets = (density * 20.0).max(3.0) as usize;
        let droplets: Vec<(f64, f64)> = (0..num_droplets)
            .map(|i| {
                let h = ((i as u64).wrapping_mul(2654435761) ^ 0xCAFEBABE) as f64
                    / u64::MAX as f64;
                let start = h; // 0..1 starting position
                let birth = h * 2.0; // staggered birth
                (start, birth)
            })
            .collect();

        Self {
            color,
            speed,
            density: density.clamp(0.1, 1.0),
            droplets,
        }
    }
}

impl RgbEffect for RainEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        let mut result = vec![Rgb::BLACK; positions.len()];
        let cycle = 2.0 / self.speed as f64;

        for &(start_pos, birth) in &self.droplets {
            // Droplet moves from start_pos downward (wrapping)
            let t = (elapsed - birth).rem_euclid(cycle) / cycle;
            let droplet_pos = (start_pos + t).fract();

            // Light up nearby LEDs with distance-based falloff
            for (i, &led_pos) in positions.iter().enumerate() {
                let dist = (led_pos as f64 - droplet_pos).abs();
                let dist = dist.min(1.0 - dist); // wrap-around distance

                let falloff = 0.05; // How wide the droplet appears
                if dist < falloff {
                    let brightness = ((1.0 - dist / falloff) * self.density as f64) as f32;
                    let color = self.color.dim(brightness.clamp(0.0, 1.0));
                    // Additive blend with existing
                    result[i] = Rgb::new(
                        result[i].r.saturating_add(color.r),
                        result[i].g.saturating_add(color.g),
                        result[i].b.saturating_add(color.b),
                    );
                }
            }
        }

        result
    }

    fn name(&self) -> &'static str {
        "Rain"
    }
}
