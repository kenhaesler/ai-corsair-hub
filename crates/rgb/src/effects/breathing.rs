use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

pub struct BreathingEffect {
    pub color: Rgb,
    pub speed: f32,
}

impl RgbEffect for BreathingEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        // Sinusoidal brightness: (1 + sin(2π * t * speed)) / 2
        let phase = 2.0 * std::f64::consts::PI * elapsed * self.speed as f64;
        let brightness = ((1.0 + phase.sin()) / 2.0) as f32;
        let color = self.color.dim(brightness);
        vec![color; positions.len()]
    }

    fn name(&self) -> &'static str {
        "Breathing"
    }
}
