use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

pub struct StaticEffect {
    pub color: Rgb,
}

impl RgbEffect for StaticEffect {
    fn render(&self, positions: &[f32], _elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        vec![self.color; positions.len()]
    }

    fn name(&self) -> &'static str {
        "Static"
    }
}
