use serde::{Deserialize, Serialize};

/// Physical LED arrangement for a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LedLayout {
    FanRing { led_count: u16 },
    LinearStrip { led_count: u16 },
}

impl LedLayout {
    /// QX140/QX120 fan — 34 LEDs in a circle.
    pub fn qx_fan() -> Self {
        LedLayout::FanRing { led_count: 34 }
    }

    /// LS350 Aurora strip — 21 LEDs (placeholder, TBD from enumeration).
    pub fn ls350() -> Self {
        LedLayout::LinearStrip { led_count: 21 }
    }

    pub fn led_count(&self) -> u16 {
        match self {
            LedLayout::FanRing { led_count } | LedLayout::LinearStrip { led_count } => *led_count,
        }
    }

    /// Normalized 1D positions (0.0..1.0) for spatial effects.
    /// Fan rings: angular position around the circle.
    /// Linear strips: position along the strip.
    pub fn positions(&self) -> Vec<f32> {
        let count = self.led_count() as usize;
        if count == 0 {
            return vec![];
        }
        (0..count).map(|i| i as f32 / count as f32).collect()
    }

    /// Normalized 2D positions (x, y) for 2D noise effects.
    /// Fan rings: (cos, sin) on unit circle.
    /// Linear strips: (position, 0.0).
    pub fn positions_2d(&self) -> Vec<(f32, f32)> {
        let count = self.led_count() as usize;
        if count == 0 {
            return vec![];
        }
        match self {
            LedLayout::FanRing { .. } => (0..count)
                .map(|i| {
                    let angle = 2.0 * std::f32::consts::PI * i as f32 / count as f32;
                    (angle.cos() * 0.5 + 0.5, angle.sin() * 0.5 + 0.5)
                })
                .collect(),
            LedLayout::LinearStrip { .. } => (0..count)
                .map(|i| (i as f32 / count as f32, 0.5))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qx_fan_has_34_leds() {
        let layout = LedLayout::qx_fan();
        assert_eq!(layout.led_count(), 34);
        assert_eq!(layout.positions().len(), 34);
        assert_eq!(layout.positions_2d().len(), 34);
    }

    #[test]
    fn ls350_has_21_leds() {
        let layout = LedLayout::ls350();
        assert_eq!(layout.led_count(), 21);
    }

    #[test]
    fn positions_normalized() {
        let positions = LedLayout::qx_fan().positions();
        for &p in &positions {
            assert!(p >= 0.0 && p < 1.0, "Position {p} out of range");
        }
    }

    #[test]
    fn positions_2d_fan_on_circle() {
        let positions = LedLayout::qx_fan().positions_2d();
        for &(x, y) in &positions {
            // Should be within [0, 1]
            assert!(x >= 0.0 && x <= 1.0);
            assert!(y >= 0.0 && y <= 1.0);
        }
    }
}
