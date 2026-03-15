use crate::effect::{EffectContext, RgbEffect};
use crate::Rgb;

pub struct TemperatureMapEffect {
    pub gradient: Vec<(f64, Rgb)>,
    pub glow_on_spike: bool,
}

impl RgbEffect for TemperatureMapEffect {
    fn render(&self, positions: &[f32], elapsed: f64, ctx: &EffectContext) -> Vec<Rgb> {
        let temp = ctx.temperature.unwrap_or(40.0);
        let mut color = gradient_lookup(&self.gradient, temp);

        // Flash on rapid temp change (derivative spike)
        if self.glow_on_spike {
            if let Some(delta) = ctx.temp_delta {
                if delta > 2.0 {
                    // Quick temp spike — pulse white overlay
                    let intensity = ((delta - 2.0) / 5.0).min(1.0);
                    let pulse = ((elapsed * 4.0).sin().abs() * intensity) as f32;
                    color = Rgb::lerp(color, Rgb::WHITE, pulse * 0.5);
                }
            }
        }

        vec![color; positions.len()]
    }

    fn name(&self) -> &'static str {
        "Temperature Map"
    }
}

/// Multi-stop gradient lookup.
fn gradient_lookup(stops: &[(f64, Rgb)], value: f64) -> Rgb {
    if stops.is_empty() {
        return Rgb::BLACK;
    }
    if stops.len() == 1 {
        return stops[0].1;
    }

    // Clamp to gradient range
    if value <= stops[0].0 {
        return stops[0].1;
    }
    if value >= stops[stops.len() - 1].0 {
        return stops[stops.len() - 1].1;
    }

    // Find the two stops that bracket the value
    for i in 0..stops.len() - 1 {
        let (t0, c0) = stops[i];
        let (t1, c1) = stops[i + 1];
        if value >= t0 && value <= t1 {
            let t = ((value - t0) / (t1 - t0)) as f32;
            return Rgb::lerp(c0, c1, t);
        }
    }

    stops[stops.len() - 1].1
}
