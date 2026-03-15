use crate::effect::{EffectContext, RgbEffect};
use crate::noise::fbm;
use crate::Rgb;

pub struct FireEffect {
    pub intensity: f32,
    pub speed: f32,
}

impl RgbEffect for FireEffect {
    fn render(&self, positions: &[f32], elapsed: f64, _ctx: &EffectContext) -> Vec<Rgb> {
        positions
            .iter()
            .map(|&pos| {
                // fbm noise → orange/yellow/red palette lookup
                let n = fbm(
                    pos as f64 * 3.0,
                    elapsed * self.speed as f64,
                    4,
                );
                // Map noise [-1, 1] to heat [0, 1]
                let heat = ((n + 1.0) * 0.5 * self.intensity as f64).clamp(0.0, 1.0);

                // Fire palette: black → red → orange → yellow → white
                fire_palette(heat as f32)
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Fire"
    }
}

fn fire_palette(t: f32) -> Rgb {
    // 0.0 = black, 0.25 = dark red, 0.5 = orange, 0.75 = yellow, 1.0 = white
    if t < 0.25 {
        let s = t / 0.25;
        Rgb::lerp(Rgb::new(0, 0, 0), Rgb::new(180, 30, 0), s)
    } else if t < 0.5 {
        let s = (t - 0.25) / 0.25;
        Rgb::lerp(Rgb::new(180, 30, 0), Rgb::new(255, 120, 0), s)
    } else if t < 0.75 {
        let s = (t - 0.5) / 0.25;
        Rgb::lerp(Rgb::new(255, 120, 0), Rgb::new(255, 220, 50), s)
    } else {
        let s = (t - 0.75) / 0.25;
        Rgb::lerp(Rgb::new(255, 220, 50), Rgb::new(255, 255, 200), s)
    }
}
