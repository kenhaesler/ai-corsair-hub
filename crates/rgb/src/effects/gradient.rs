use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

/// Animated multi-stop gradient with configurable speed.
pub struct GradientEffect {
    pub colors: Vec<Rgb>,
    pub speed: f32,
}

impl RgbEffect for GradientEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        if self.colors.is_empty() {
            return vec![Rgb::BLACK; positions.len()];
        }
        if self.colors.len() == 1 {
            return vec![self.colors[0]; positions.len()];
        }

        let offset = (elapsed * self.speed as f64).fract() as f32;
        let num_stops = self.colors.len();

        positions
            .iter()
            .map(|&pos| {
                // Shift position by time offset (wrapping)
                let shifted = (pos + offset).fract();
                // Map to gradient stops
                let scaled = shifted * (num_stops - 1) as f32;
                let idx = scaled.floor() as usize;
                let frac = scaled - idx as f32;

                if idx >= num_stops - 1 {
                    self.colors[num_stops - 1]
                } else {
                    Rgb::lerp(self.colors[idx], self.colors[idx + 1], frac)
                }
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Gradient"
    }
}
