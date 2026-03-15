use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

/// Fan duty → fill level + color. LEDs fill proportionally to fan duty.
pub struct DutyMeterEffect {
    pub low_color: Rgb,
    pub high_color: Rgb,
}

impl RgbEffect for DutyMeterEffect {
    fn render(&self, positions: &[f32], _elapsed: f64, ctx: &EffectContext) -> Vec<Rgb> {
        let duty = ctx.duty_percent.unwrap_or(0.0) / 100.0;
        let fill_level = duty.clamp(0.0, 1.0) as f32;
        let active_color = Rgb::lerp(self.low_color, self.high_color, fill_level);

        positions
            .iter()
            .map(|&pos| {
                if pos <= fill_level {
                    active_color
                } else {
                    // Dim inactive LEDs
                    active_color.dim(0.05)
                }
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Duty Meter"
    }
}
