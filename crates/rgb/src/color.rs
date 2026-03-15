use serde::{Deserialize, Serialize};

/// Wire-format RGB color (0–255 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// HSV color for effect computation (h: 0–360, s/v: 0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
    pub const WHITE: Rgb = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert from HSV (h: 0–360, s: 0–1, v: 0–1).
    pub fn from_hsv(hsv: Hsv) -> Self {
        let h = ((hsv.h % 360.0) + 360.0) % 360.0;
        let s = hsv.s.clamp(0.0, 1.0);
        let v = hsv.v.clamp(0.0, 1.0);

        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = match h as u32 {
            0..60 => (c, x, 0.0),
            60..120 => (x, c, 0.0),
            120..180 => (0.0, c, x),
            180..240 => (0.0, x, c),
            240..300 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Rgb {
            r: ((r1 + m) * 255.0).round() as u8,
            g: ((g1 + m) * 255.0).round() as u8,
            b: ((b1 + m) * 255.0).round() as u8,
        }
    }

    /// Linear interpolation between two colors.
    pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb {
            r: (a.r as f32 + (b.r as f32 - a.r as f32) * t).round() as u8,
            g: (a.g as f32 + (b.g as f32 - a.g as f32) * t).round() as u8,
            b: (a.b as f32 + (b.b as f32 - a.b as f32) * t).round() as u8,
        }
    }

    /// Scale brightness (0.0–1.0).
    pub fn dim(self, brightness: f32) -> Rgb {
        let b = brightness.clamp(0.0, 1.0);
        Rgb {
            r: (self.r as f32 * b).round() as u8,
            g: (self.g as f32 * b).round() as u8,
            b: (self.b as f32 * b).round() as u8,
        }
    }

    fn to_f32(self) -> (f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        )
    }

    fn from_f32(r: f32, g: f32, b: f32) -> Rgb {
        Rgb {
            r: (r.clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (g.clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (b.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }
}

impl Hsv {
    pub fn new(h: f32, s: f32, v: f32) -> Self {
        Self { h, s, v }
    }

    pub fn from_rgb(rgb: Rgb) -> Self {
        let (r, g, b) = rgb.to_f32();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let h = if delta < 1e-6 {
            0.0
        } else if (max - r).abs() < 1e-6 {
            60.0 * (((g - b) / delta) % 6.0)
        } else if (max - g).abs() < 1e-6 {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        let s = if max < 1e-6 { 0.0 } else { delta / max };
        let v = max;

        Hsv {
            h: ((h % 360.0) + 360.0) % 360.0,
            s,
            v,
        }
    }
}

/// Blend modes for compositing layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Add,
    Multiply,
    Screen,
    Overlay,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Normal
    }
}

impl BlendMode {
    /// Apply blend mode: composites `layer` onto `base` with given opacity.
    pub fn apply(self, base: Rgb, layer: Rgb, opacity: f32) -> Rgb {
        let opacity = opacity.clamp(0.0, 1.0);
        let (br, bg, bb) = base.to_f32();
        let (lr, lg, lb) = layer.to_f32();

        let (rr, rg, rb) = match self {
            BlendMode::Normal => (lr, lg, lb),
            BlendMode::Add => ((br + lr).min(1.0), (bg + lg).min(1.0), (bb + lb).min(1.0)),
            BlendMode::Multiply => (br * lr, bg * lg, bb * lb),
            BlendMode::Screen => {
                (1.0 - (1.0 - br) * (1.0 - lr), 1.0 - (1.0 - bg) * (1.0 - lg), 1.0 - (1.0 - bb) * (1.0 - lb))
            }
            BlendMode::Overlay => {
                fn overlay_ch(base: f32, layer: f32) -> f32 {
                    if base < 0.5 {
                        2.0 * base * layer
                    } else {
                        1.0 - 2.0 * (1.0 - base) * (1.0 - layer)
                    }
                }
                (overlay_ch(br, lr), overlay_ch(bg, lg), overlay_ch(bb, lb))
            }
        };

        // Mix blended result with base using opacity
        Rgb::from_f32(
            br + (rr - br) * opacity,
            bg + (rg - bg) * opacity,
            bb + (rb - bb) * opacity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_rgb_round_trip() {
        let colors = [
            Rgb::new(255, 0, 0),
            Rgb::new(0, 255, 0),
            Rgb::new(0, 0, 255),
            Rgb::new(128, 64, 192),
            Rgb::new(255, 255, 255),
            Rgb::new(0, 0, 0),
        ];
        for c in colors {
            let hsv = Hsv::from_rgb(c);
            let back = Rgb::from_hsv(hsv);
            // Allow ±1 rounding error per channel
            assert!((c.r as i16 - back.r as i16).abs() <= 1, "R mismatch for {c:?}: got {back:?}");
            assert!((c.g as i16 - back.g as i16).abs() <= 1, "G mismatch for {c:?}: got {back:?}");
            assert!((c.b as i16 - back.b as i16).abs() <= 1, "B mismatch for {c:?}: got {back:?}");
        }
    }

    #[test]
    fn lerp_endpoints() {
        let a = Rgb::new(0, 0, 0);
        let b = Rgb::new(255, 255, 255);
        assert_eq!(Rgb::lerp(a, b, 0.0), a);
        assert_eq!(Rgb::lerp(a, b, 1.0), b);
        let mid = Rgb::lerp(a, b, 0.5);
        assert!((mid.r as i16 - 128).abs() <= 1);
    }

    #[test]
    fn blend_normal_full_opacity() {
        let base = Rgb::new(100, 100, 100);
        let layer = Rgb::new(200, 50, 150);
        let result = BlendMode::Normal.apply(base, layer, 1.0);
        assert_eq!(result, layer);
    }

    #[test]
    fn blend_normal_zero_opacity() {
        let base = Rgb::new(100, 100, 100);
        let layer = Rgb::new(200, 50, 150);
        let result = BlendMode::Normal.apply(base, layer, 0.0);
        assert_eq!(result, base);
    }

    #[test]
    fn blend_add_clamps() {
        let base = Rgb::new(200, 200, 200);
        let layer = Rgb::new(200, 200, 200);
        let result = BlendMode::Add.apply(base, layer, 1.0);
        assert_eq!(result, Rgb::new(255, 255, 255));
    }

    #[test]
    fn blend_multiply() {
        let base = Rgb::new(255, 128, 0);
        let layer = Rgb::new(128, 255, 128);
        let result = BlendMode::Multiply.apply(base, layer, 1.0);
        // 255*128/255 ≈ 128, 128*255/255 ≈ 128, 0*128/255 = 0
        assert!((result.r as i16 - 128).abs() <= 1);
        assert!((result.g as i16 - 128).abs() <= 1);
        assert_eq!(result.b, 0);
    }

    #[test]
    fn blend_screen() {
        let base = Rgb::new(128, 128, 128);
        let layer = Rgb::new(128, 128, 128);
        let result = BlendMode::Screen.apply(base, layer, 1.0);
        // screen(0.5, 0.5) = 1 - 0.5*0.5 = 0.75 → 191
        assert!((result.r as i16 - 191).abs() <= 1);
    }

    #[test]
    fn blend_overlay() {
        // Base < 0.5: 2*base*layer
        let result = BlendMode::Overlay.apply(Rgb::new(64, 64, 64), Rgb::new(128, 128, 128), 1.0);
        // 2 * (64/255) * (128/255) ≈ 0.251 → 64
        assert!((result.r as i16 - 64).abs() <= 2);
    }

    #[test]
    fn dim_zero_is_black() {
        assert_eq!(Rgb::new(255, 128, 64).dim(0.0), Rgb::BLACK);
    }

    #[test]
    fn dim_one_is_identity() {
        let c = Rgb::new(200, 100, 50);
        assert_eq!(c.dim(1.0), c);
    }
}
