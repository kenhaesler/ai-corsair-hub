use crate::effect::{EffectContext, RgbEffect};
use crate::{Hsv, Rgb};

pub struct ColorCycleEffect {
    pub speed: f32,
    pub saturation: f32,
}

impl RgbEffect for ColorCycleEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        // Uniform hue rotation — all LEDs same color, cycling over time
        let hue = ((elapsed * self.speed as f64 * 60.0) % 360.0) as f32;
        let color = Rgb::from_hsv(Hsv::new(hue, self.saturation, 1.0));
        vec![color; positions.len()]
    }

    fn name(&self) -> &'static str {
        "Color Cycle"
    }
}
