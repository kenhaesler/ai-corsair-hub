use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

/// PID-inspired: proportional color + integral saturation ramp + derivative brightness spike.
pub struct ThermalPulseEffect {
    pub cold_color: Rgb,
    pub hot_color: Rgb,
    pub min_temp: f64,
    pub max_temp: f64,
}

impl RgbEffect for ThermalPulseEffect {
    fn render(&self, positions: &[f32], _elapsed: f64, ctx: &EffectContext) -> Vec<Rgb> {
        let temp = ctx.temperature.unwrap_or(self.min_temp);

        // P (proportional): map temp to color blend
        let range = (self.max_temp - self.min_temp).max(1.0);
        let t = ((temp - self.min_temp) / range).clamp(0.0, 1.0) as f32;
        let base_color = Rgb::lerp(self.cold_color, self.hot_color, t);

        // D (derivative): brightness spike on rapid temp change
        let brightness_boost = ctx
            .temp_delta
            .map(|d| (d / 5.0).clamp(0.0, 0.3) as f32)
            .unwrap_or(0.0);

        let final_color = Rgb::lerp(base_color, Rgb::WHITE, brightness_boost);

        vec![final_color; positions.len()]
    }

    fn name(&self) -> &'static str {
        "Thermal Pulse"
    }
}
