use crate::effect::{EffectContext, RgbEffect};
use crate::{Hsv, Rgb};

pub struct SpectrumShiftEffect {
    pub speed: f32,
}

impl RgbEffect for SpectrumShiftEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        positions
            .iter()
            .map(|&pos| {
                let hue = ((pos + elapsed as f32 * self.speed) * 360.0) % 360.0;
                let hue = ((hue % 360.0) + 360.0) % 360.0;
                Rgb::from_hsv(Hsv::new(hue, 1.0, 1.0))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Spectrum Shift"
    }
}
