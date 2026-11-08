//! Pump stall watchdog.
//!
//! A stalled or dead pump cooks a water-cooled CPU and GPU within a minute
//! while the fans spin happily. The control loop cannot drive a motherboard
//! pump, but it can see its rpm (and LINK pumps' rpm) and raise the same
//! emergency override it uses for critical temperatures.
//!
//! Rules (see the plan's Review Focus):
//! - the watchdog arms the first time any pump reads at or above the
//!   threshold, so the zeros LHM reports during boot and resume never alarm;
//! - once armed, it trips after `TRIP_TICKS` consecutive ticks in which every
//!   pump reads below the threshold, and clears after `CLEAR_TICKS` good
//!   ticks;
//! - no pumps at all means nothing to watch: never armed, never tripped;
//! - a threshold of 0 disables it.

pub const DEFAULT_PUMP_MIN_RPM: f64 = 800.0;
const TRIP_TICKS: u8 = 5;
const CLEAR_TICKS: u8 = 3;

#[derive(Debug, Clone)]
pub struct PumpWatchdog {
    min_rpm: f64,
    armed: bool,
    bad_ticks: u8,
    good_ticks: u8,
    stalled: bool,
}

impl PumpWatchdog {
    pub fn new(min_rpm: f64) -> Self {
        Self {
            min_rpm: min_rpm.max(0.0),
            armed: false,
            bad_ticks: 0,
            good_ticks: 0,
            stalled: false,
        }
    }

    pub fn min_rpm(&self) -> f64 {
        self.min_rpm
    }

    /// Change the threshold live. Re-arms from scratch so a new, higher
    /// threshold cannot trip on the same stale evidence.
    pub fn set_min_rpm(&mut self, min_rpm: f64) {
        if (min_rpm - self.min_rpm).abs() > f64::EPSILON {
            *self = Self::new(min_rpm);
        }
    }

    pub fn stalled(&self) -> bool {
        self.stalled
    }

    /// Feed one tick's pump readings (rpm of every pump we can see).
    /// Returns the stall state after this tick.
    pub fn tick(&mut self, pump_rpms: &[f64]) -> bool {
        if self.min_rpm <= 0.0 || pump_rpms.is_empty() {
            self.stalled = false;
            return false;
        }
        let any_ok = pump_rpms.iter().any(|&r| r >= self.min_rpm);
        if !self.armed {
            if any_ok {
                self.armed = true;
            }
            return false;
        }
        if any_ok {
            self.bad_ticks = 0;
            if self.stalled {
                self.good_ticks += 1;
                if self.good_ticks >= CLEAR_TICKS {
                    self.stalled = false;
                    self.good_ticks = 0;
                }
            }
        } else {
            self.good_ticks = 0;
            self.bad_ticks = self.bad_ticks.saturating_add(1);
            if self.bad_ticks >= TRIP_TICKS {
                self.stalled = true;
            }
        }
        self.stalled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_alarm_before_armed() {
        let mut w = PumpWatchdog::new(800.0);
        for _ in 0..20 {
            assert!(!w.tick(&[0.0]));
        }
    }

    #[test]
    fn alarms_after_five_bad_ticks_once_armed() {
        let mut w = PumpWatchdog::new(800.0);
        assert!(!w.tick(&[4428.0]));
        for _ in 0..4 {
            assert!(!w.tick(&[0.0]));
        }
        assert!(w.tick(&[0.0]), "fifth consecutive bad tick trips");
    }

    #[test]
    fn a_single_good_tick_resets_the_count() {
        let mut w = PumpWatchdog::new(800.0);
        w.tick(&[3000.0]);
        for _ in 0..4 {
            w.tick(&[0.0]);
        }
        w.tick(&[3000.0]);
        for _ in 0..4 {
            assert!(!w.tick(&[0.0]));
        }
    }

    #[test]
    fn recovers_after_three_good_ticks() {
        let mut w = PumpWatchdog::new(800.0);
        w.tick(&[3000.0]);
        for _ in 0..5 {
            w.tick(&[0.0]);
        }
        assert!(w.stalled());
        assert!(w.tick(&[3000.0]));
        assert!(w.tick(&[3000.0]));
        assert!(!w.tick(&[3000.0]), "third good tick clears");
    }

    #[test]
    fn missing_pump_sensor_never_alarms() {
        let mut w = PumpWatchdog::new(800.0);
        for _ in 0..20 {
            assert!(!w.tick(&[]));
        }
        // and a pump that vanishes after arming does not trip either
        w.tick(&[3000.0]);
        for _ in 0..20 {
            assert!(!w.tick(&[]));
        }
    }

    #[test]
    fn zero_threshold_disables() {
        let mut w = PumpWatchdog::new(0.0);
        w.tick(&[3000.0]);
        for _ in 0..20 {
            assert!(!w.tick(&[0.0]));
        }
    }

    #[test]
    fn one_healthy_pump_of_two_is_enough() {
        let mut w = PumpWatchdog::new(800.0);
        w.tick(&[3000.0, 2800.0]);
        for _ in 0..10 {
            assert!(!w.tick(&[0.0, 2800.0]));
        }
    }

    #[test]
    fn changing_threshold_rearms() {
        let mut w = PumpWatchdog::new(800.0);
        w.tick(&[3000.0]);
        w.set_min_rpm(6000.0);
        for _ in 0..20 {
            assert!(!w.tick(&[3000.0]), "never saw >= 6000, so not armed");
        }
    }
}
