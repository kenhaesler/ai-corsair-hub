//! Power feed-forward for curve mode.
//!
//! In a water loop with three big radiators the coolant warms tens of
//! seconds after the chips start pulling power, so a temperature-only curve
//! reacts late and then overshoots. Package power is the leading indicator:
//! when CPU + GPU watts rise well above their recent average, the curve is
//! evaluated as if the temperature were a few degrees higher, so fans start
//! early and settle as the average catches up. The offset is never negative
//! and never larger than `MAX_OFFSET_C`, and it decays on its own because it
//! is measured against a moving average of the same signal.

/// Largest temperature offset feed-forward may add, in °C.
pub const MAX_OFFSET_C: f64 = 15.0;
/// The power rise that one unit of gain maps to one degree: k is "°C per 100 W".
const WATTS_PER_UNIT: f64 = 100.0;

/// Exponential moving average of power with a time constant, fed per tick.
#[derive(Debug, Clone)]
pub struct PowerAverage {
    tau_s: f64,
    mean: Option<f64>,
}

impl PowerAverage {
    /// `window_s` is the time constant; after ~3× that the average has caught up.
    pub fn new(window_s: f64) -> Self {
        Self {
            tau_s: window_s.max(1.0),
            mean: None,
        }
    }

    /// Push one reading taken `dt_s` after the previous one; returns the new mean.
    pub fn push(&mut self, watts: f64, dt_s: f64) -> f64 {
        let m = match self.mean {
            None => watts,
            Some(prev) => {
                let a = (dt_s.max(0.0) / self.tau_s).min(1.0);
                prev + a * (watts - prev)
            }
        };
        self.mean = Some(m);
        m
    }

    pub fn mean(&self) -> Option<f64> {
        self.mean
    }
}

/// Temperature offset (°C) for a power rise of `p_now - p_avg` watts at gain
/// `k_per_100w`. Zero when power is at or below its average or when k is 0.
pub fn feed_forward_offset(p_now: f64, p_avg: f64, k_per_100w: f64) -> f64 {
    if !(p_now.is_finite() && p_avg.is_finite() && k_per_100w.is_finite()) || k_per_100w <= 0.0 {
        return 0.0;
    }
    ((p_now - p_avg) / WATTS_PER_UNIT * k_per_100w).clamp(0.0, MAX_OFFSET_C)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_offset_when_power_steady() {
        assert_eq!(feed_forward_offset(300.0, 300.0, 5.0), 0.0);
    }

    #[test]
    fn offset_scales_with_rise() {
        assert_eq!(feed_forward_offset(450.0, 300.0, 5.0), 7.5);
    }

    #[test]
    fn never_negative() {
        assert_eq!(feed_forward_offset(200.0, 300.0, 5.0), 0.0);
    }

    #[test]
    fn clamped() {
        assert_eq!(feed_forward_offset(2000.0, 300.0, 5.0), MAX_OFFSET_C);
    }

    #[test]
    fn zero_gain_is_off() {
        assert_eq!(feed_forward_offset(900.0, 300.0, 0.0), 0.0);
    }

    #[test]
    fn average_decays_toward_new_level() {
        let mut avg = PowerAverage::new(60.0);
        for _ in 0..60 {
            avg.push(300.0, 1.0);
        }
        let mut last = 0.0;
        for _ in 0..180 {
            last = avg.push(450.0, 1.0);
        }
        assert!(
            last > 440.0,
            "after 3 time constants the mean should be near 450, got {last}"
        );
        // and the offset has therefore collapsed
        assert!(feed_forward_offset(450.0, last, 5.0) < 0.6);
    }

    #[test]
    fn first_push_seeds_the_mean() {
        let mut avg = PowerAverage::new(60.0);
        assert_eq!(avg.push(333.0, 1.0), 333.0);
        assert_eq!(feed_forward_offset(333.0, avg.mean().unwrap(), 5.0), 0.0);
    }
}
