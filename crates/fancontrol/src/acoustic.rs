/// Acoustic filter: prevents rapid fan speed changes.
/// Applied as post-processing after PID/curve output.
pub struct AcousticFilter {
    current_duty: f64,
    target_duty: f64,
    ramp_up_rate: f64,   // max %/sec increase
    ramp_down_rate: f64,  // max %/sec decrease
    hysteresis: f64,      // °C band to ignore
    last_trigger_temp: f64,
}

impl AcousticFilter {
    pub fn new(ramp_up: f64, ramp_down: f64, hysteresis: f64) -> Self {
        Self {
            current_duty: 0.0,
            target_duty: 0.0,
            ramp_up_rate: ramp_up,
            ramp_down_rate: ramp_down,
            hysteresis,
            last_trigger_temp: 0.0,
        }
    }

    /// Unified update: conditionally re-evaluate target from curve output,
    /// then always ramp current_duty toward target_duty.
    ///
    /// - If temp changed by >= hysteresis from last trigger, update target_duty
    ///   from `raw_duty` (the curve/PID output).
    /// - Always ramp current_duty toward target_duty regardless of hysteresis.
    pub fn update(&mut self, raw_duty: f64, temp: f64, dt_secs: f64) -> f64 {
        // Re-evaluate target if temp moved enough
        if (temp - self.last_trigger_temp).abs() >= self.hysteresis {
            self.last_trigger_temp = temp;
            self.target_duty = raw_duty;
        }

        // Always ramp toward target
        let diff = self.target_duty - self.current_duty;
        if diff > 0.0 {
            let max_change = self.ramp_up_rate * dt_secs;
            self.current_duty += diff.min(max_change);
        } else if diff < 0.0 {
            let max_change = self.ramp_down_rate * dt_secs;
            self.current_duty += diff.max(-max_change);
        }

        self.current_duty
    }

    /// Bypass filter (emergency). Sets both current and target duty immediately.
    pub fn override_duty(&mut self, duty: f64) {
        self.current_duty = duty;
        self.target_duty = duty;
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
        // Temp jump triggers target update, then ramp is limited to 5%/s
        let result = filter.update(50.0, 55.0, 1.0);
        assert!((result - 5.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_ramp_down_limiting() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        filter.override_duty(50.0);
        // Temp jump triggers target update to 0%, ramp down limited to 2%/s
        let result = filter.update(0.0, 30.0, 1.0);
        assert!((result - 48.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_hysteresis_updates_target_only_on_temp_change() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        // First call: temp jumps from 0→50, updates target to 40%
        filter.update(40.0, 50.0, 1.0);
        // Second call: temp stable at 51 (within 3°C band), raw_duty=60 is ignored
        // but ramping toward 40% continues
        let result = filter.update(60.0, 51.0, 1.0);
        // Should be 5+5=10 (two ticks of ramp-up toward target 40)
        assert!((result - 10.0).abs() < 0.01, "got {}", result);
    }

    #[test]
    fn test_continues_ramping_when_temp_stable() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        // Tick 1: temp jumps 0→55, sets target to 48%
        let r1 = filter.update(48.0, 55.0, 1.0);
        assert!((r1 - 5.0).abs() < 0.01, "tick1: {}", r1);
        // Tick 2: temp stable at 55 — should keep ramping toward 48%
        let r2 = filter.update(48.0, 55.0, 1.0);
        assert!((r2 - 10.0).abs() < 0.01, "tick2: {}", r2);
        // Tick 3: still ramping
        let r3 = filter.update(48.0, 55.0, 1.0);
        assert!((r3 - 15.0).abs() < 0.01, "tick3: {}", r3);
    }

    #[test]
    fn test_emergency_override() {
        let mut filter = AcousticFilter::new(5.0, 2.0, 3.0);
        filter.override_duty(100.0);
        assert!((filter.current_duty() - 100.0).abs() < 0.01);
    }
}
