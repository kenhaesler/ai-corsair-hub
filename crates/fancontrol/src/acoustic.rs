/// Acoustic filter: prevents rapid fan speed changes.
/// Applied as post-processing after PID/curve output.
pub struct AcousticFilter {
    current_duty: f64,
    ramp_up_rate: f64,   // max %/sec increase
    ramp_down_rate: f64,  // max %/sec decrease
    hysteresis: f64,      // °C band to ignore
    last_trigger_temp: f64,
}

impl AcousticFilter {
    pub fn new(ramp_up: f64, ramp_down: f64, hysteresis: f64) -> Self {
        Self {
            current_duty: 0.0,
            ramp_up_rate: ramp_up,
            ramp_down_rate: ramp_down,
            hysteresis,
            last_trigger_temp: 0.0,
        }
    }

    /// Apply ramp rate limiting. Returns the filtered duty.
    pub fn filter(&mut self, target_duty: f64, dt_secs: f64) -> f64 {
        let diff = target_duty - self.current_duty;

        if diff > 0.0 {
            // Ramping up
            let max_change = self.ramp_up_rate * dt_secs;
            self.current_duty += diff.min(max_change);
        } else if diff < 0.0 {
            // Ramping down
            let max_change = self.ramp_down_rate * dt_secs;
            self.current_duty += diff.max(-max_change);
        }

        self.current_duty
    }

    /// Check if temp change exceeds hysteresis band from last trigger.
    /// Returns true if we should recompute duty.
    pub fn should_update(&mut self, current_temp: f64) -> bool {
        if (current_temp - self.last_trigger_temp).abs() >= self.hysteresis {
            self.last_trigger_temp = current_temp;
            true
        } else {
            false
        }
    }

    /// Bypass filter (emergency). Sets current_duty immediately.
    pub fn override_duty(&mut self, duty: f64) {
        self.current_duty = duty;
    }

    /// Get the current filtered duty.
    pub fn current_duty(&self) -> f64 {
        self.current_duty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ramp_up_limiting() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        // Try to jump from 0 to 50 in 1 second — limited to 5%/s
        let result = filter.filter(50.0, 1.0);
        assert!((result - 5.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_ramp_down_limiting() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        filter.override_duty(50.0);
        // Try to drop from 50 to 0 in 1 second — limited to 2%/s
        let result = filter.filter(0.0, 1.0);
        assert!((result - 48.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_hysteresis_band() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        // First trigger at 50°C
        assert!(filter.should_update(50.0));
        // Within 3°C band — should NOT trigger
        assert!(!filter.should_update(51.0));
        assert!(!filter.should_update(52.0));
        // Exceeds band — should trigger
        assert!(filter.should_update(53.0));
    }

    #[test]
    fn test_emergency_override() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        filter.override_duty(100.0);
        assert!((filter.current_duty() - 100.0).abs() < 0.01);
    }
}
