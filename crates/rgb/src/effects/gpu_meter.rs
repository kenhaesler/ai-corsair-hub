use crate::Rgb;
use crate::effect::{EffectContext, RgbEffect};

/// GPU utilization → fill level + color.
///
/// Mirrors `DutyMeterEffect` but reads `ctx.gpu_utilization` (0–100 %)
/// instead of fan duty. Fills LEDs proportionally; the colour lerps from
/// `low_color` to `high_color` with utilisation. Inactive LEDs are dimmed
/// to 5 % so the meter is still legible on a black case but not glaring
/// at idle.
pub struct GpuMeterEffect {
    pub low_color: Rgb,
    pub high_color: Rgb,
}

impl RgbEffect for GpuMeterEffect {
    fn render(&self, positions: &[f32], _elapsed: f64, ctx: &EffectContext) -> Vec<Rgb> {
        let util = ctx.gpu_utilization.unwrap_or(0.0) / 100.0;
        let fill_level = util.clamp(0.0, 1.0) as f32;
        let active_color = Rgb::lerp(self.low_color, self.high_color, fill_level);

        positions
            .iter()
            .map(|&pos| {
                if pos <= fill_level {
                    active_color
                } else {
                    active_color.dim(0.05)
                }
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "GPU Meter"
    }
}
