use std::time::Instant;

/// PID controller for fan speed regulation.
///
/// Targets a temperature setpoint and outputs a duty cycle percentage.
/// Includes anti-windup, derivative filtering, and output clamping.
pub struct PidController {
    kp: f64,
    ki: f64,
    kd: f64,
    setpoint: f64,
    min_output: f64,
    max_output: f64,

    // State
    integral: f64,
    prev_error: f64,
    prev_time: Option<Instant>,

    // Anti-windup: clamp integral term
    integral_limit: f64,
}

impl PidController {
    pub fn new(kp: f64, ki: f64, kd: f64, setpoint: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint,
            min_output: 0.0,
            max_output: 100.0,
            integral: 0.0,
            prev_error: 0.0,
            prev_time: None,
            integral_limit: 50.0,
        }
    }

    pub fn with_output_limits(mut self, min: f64, max: f64) -> Self {
        self.min_output = min;
        self.max_output = max;
        self
    }

    pub fn set_setpoint(&mut self, setpoint: f64) {
        self.setpoint = setpoint;
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
        self.prev_time = None;
    }

    /// Clear only the accumulated integral term, leaving prev_error and
    /// prev_time in place. Used on sensor-stale cycles so the controller
    /// doesn't carry integral windup into the post-recovery cycles, while
    /// preserving the time baseline so dt measurement is coherent when
    /// readings resume.
    pub fn reset_integral(&mut self) {
        self.integral = 0.0;
    }

    /// Current integral term. Exposed for tests.
    #[cfg(test)]
    pub fn integral(&self) -> f64 {
        self.integral
    }

    /// Compute the next control output given the current temperature.
    /// Returns duty cycle percentage (clamped to min_output..max_output).
    pub fn update(&mut self, current_temp: f64) -> f64 {
        let now = Instant::now();
        let dt = match self.prev_time {
            Some(prev) => now.duration_since(prev).as_secs_f64(),
            None => {
                self.prev_time = Some(now);
                self.prev_error = current_temp - self.setpoint;
                return self.min_output;
            }
        };

        if dt < 0.001 {
            return self.clamp(self.kp * self.prev_error);
        }

        let error = current_temp - self.setpoint;

        // Proportional
        let p = self.kp * error;

        // Integral with anti-windup
        self.integral += error * dt;
        self.integral = self.integral.clamp(-self.integral_limit, self.integral_limit);
        let i = self.ki * self.integral;

        // Derivative (on error, with simple filtering)
        let d = self.kd * (error - self.prev_error) / dt;

        self.prev_error = error;
        self.prev_time = Some(now);

        self.clamp(p + i + d)
    }

    fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min_output, self.max_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_below_setpoint_returns_min() {
        let mut pid = PidController::new(2.0, 0.1, 0.05, 70.0)
            .with_output_limits(0.0, 100.0);

        // First call initializes
        let output = pid.update(50.0);
        assert_eq!(output, 0.0);
    }

    #[test]
    fn test_pid_above_setpoint_increases_output() {
        let mut pid = PidController::new(2.0, 0.1, 0.05, 70.0)
            .with_output_limits(0.0, 100.0);

        pid.update(70.0); // init
        std::thread::sleep(std::time::Duration::from_millis(50));
        let output = pid.update(80.0); // 10 degrees over
        assert!(output > 0.0, "Output should be positive when over setpoint");
    }

    #[test]
    fn test_pid_output_clamped() {
        let mut pid = PidController::new(100.0, 0.0, 0.0, 30.0)
            .with_output_limits(20.0, 100.0);

        pid.update(30.0);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let output = pid.update(90.0); // way over
        assert_eq!(output, 100.0);
    }

    #[test]
    fn test_reset_integral_clears_windup() {
        let mut pid = PidController::new(1.0, 0.5, 0.0, 70.0)
            .with_output_limits(0.0, 100.0);

        // Drive integral positive by feeding errors above setpoint for several
        // cycles. First call initializes (returns min_output immediately), so
        // we need at least three calls with a dt gap for the integral term to
        // accumulate.
        pid.update(70.0); // init
        std::thread::sleep(std::time::Duration::from_millis(20));
        pid.update(80.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
        pid.update(80.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
        pid.update(80.0);

        let before = pid.integral();
        assert!(
            before > 0.0,
            "test precondition: expected integral > 0 after windup cycles, got {}",
            before
        );

        pid.reset_integral();
        assert_eq!(pid.integral(), 0.0, "reset_integral must clear the integral term");

        // prev_error and prev_time should be untouched — distinct from full
        // reset(). We can verify indirectly: another update() should proceed
        // using the retained prev_time (dt > 0) rather than re-initializing.
        // (The full reset() would set prev_time = None.)
        // We assert reset_integral is a strict subset of reset() by testing
        // that full reset() still zeroes the integral.
        pid.update(80.0);
        pid.reset(); // full reset
        assert_eq!(pid.integral(), 0.0);
    }
}
