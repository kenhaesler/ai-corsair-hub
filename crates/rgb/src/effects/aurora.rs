use crate::effect::{EffectContext, RgbEffect};
use crate::noise::noise_2d;
use crate::Rgb;

pub struct AuroraEffect {
    pub speed: f32,
    pub color_spread: f32,
}

impl RgbEffect for AuroraEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        positions
            .iter()
            .map(|&pos| {
                let n = noise_2d(
                    pos as f64 * 2.0 * self.color_spread as f64,
                    elapsed * self.speed as f64,
                );
                // Map noise to aurora palette: green → cyan → blue → purple → magenta
                aurora_palette(((n + 1.0) * 0.5) as f32)
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Aurora"
    }
}

fn aurora_palette(t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    if t < 0.25 {
        let s = t / 0.25;
        Rgb::lerp(Rgb::new(0, 180, 60), Rgb::new(0, 220, 180), s)
    } else if t < 0.5 {
        let s = (t - 0.25) / 0.25;
        Rgb::lerp(Rgb::new(0, 220, 180), Rgb::new(30, 100, 255), s)
    } else if t < 0.75 {
        let s = (t - 0.5) / 0.25;
        Rgb::lerp(Rgb::new(30, 100, 255), Rgb::new(130, 50, 220), s)
    } else {
        let s = (t - 0.75) / 0.25;
        Rgb::lerp(Rgb::new(130, 50, 220), Rgb::new(200, 50, 180), s)
    }
}
