use crate::Rgb;

/// Crossfade transition between old and new effect frames.
/// Asymmetric: fast into dramatic effects, slow fade on cooldown.
pub struct CrossFade {
    old_frame: Vec<Rgb>,
    start_time: f64,
    duration_secs: f64,
}

impl CrossFade {
    /// Create a new crossfade from the snapshot of the old frame.
    /// `duration_secs`: total crossfade time (default 0.5).
    pub fn new(old_frame: Vec<Rgb>, start_time: f64, duration_secs: f64) -> Self {
        Self {
            old_frame,
            start_time,
            duration_secs,
        }
    }

    /// Returns true if the crossfade is complete.
    pub fn is_done(&self, elapsed: f64) -> bool {
        elapsed >= self.start_time + self.duration_secs
    }

    /// Blend old frame with new frame based on progress.
    /// Uses an ease-in curve for smooth transitions.
    pub fn blend(&self, new_frame: &[Rgb], elapsed: f64) -> Vec<Rgb> {
        let progress = ((elapsed - self.start_time) / self.duration_secs).clamp(0.0, 1.0);
        // Ease-in-out cubic for smooth transition
        let t = if progress < 0.5 {
            4.0 * progress * progress * progress
        } else {
            1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
        } as f32;

        new_frame
            .iter()
            .enumerate()
            .map(|(i, &new_color)| {
                let old_color = self.old_frame.get(i).copied().unwrap_or(Rgb::BLACK);
                Rgb::lerp(old_color, new_color, t)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfade_start_is_old() {
        let old = vec![Rgb::new(255, 0, 0)];
        let new = vec![Rgb::new(0, 0, 255)];
        let fade = CrossFade::new(old.clone(), 0.0, 1.0);
        let result = fade.blend(&new, 0.0);
        assert_eq!(result[0], Rgb::new(255, 0, 0));
    }

    #[test]
    fn crossfade_end_is_new() {
        let old = vec![Rgb::new(255, 0, 0)];
        let new = vec![Rgb::new(0, 0, 255)];
        let fade = CrossFade::new(old, 0.0, 1.0);
        let result = fade.blend(&new, 1.0);
        assert_eq!(result[0], Rgb::new(0, 0, 255));
    }

    #[test]
    fn crossfade_mid_interpolates() {
        let old = vec![Rgb::new(0, 0, 0)];
        let new = vec![Rgb::new(255, 255, 255)];
        let fade = CrossFade::new(old, 0.0, 1.0);
        let result = fade.blend(&new, 0.5);
        // At midpoint of ease-in-out cubic, t ≈ 0.5
        assert!(result[0].r > 50 && result[0].r < 200);
    }

    #[test]
    fn crossfade_done_check() {
        let fade = CrossFade::new(vec![], 1.0, 0.5);
        assert!(!fade.is_done(1.0));
        assert!(!fade.is_done(1.4));
        assert!(fade.is_done(1.5));
        assert!(fade.is_done(2.0));
    }
}
