use crate::effect::{EffectContext, RgbEffect};
use crate::noise::noise_1d;
use crate::Rgb;

pub struct CandleEffect {
    pub color: Rgb,
    pub flicker_speed: f32,
}

impl RgbEffect for CandleEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        positions
            .iter()
            .enumerate()
            .map(|(i, _pos)| {
                // Each LED gets slightly different noise offset for organic variation
                let n = noise_1d(elapsed * self.flicker_speed as f64 + i as f64 * 0.3);
                // Map noise [-1,1] to brightness [0.3, 1.0] — candle never fully goes out
                let brightness = (0.65 + 0.35 * n as f32).clamp(0.3, 1.0);
                self.color.dim(brightness)
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Candle"
    }
}
