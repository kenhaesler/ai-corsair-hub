//! Device monitor: the event source and settle logic for topology diagnostics.
//!
//! [`DeviceMonitor`] owns a [`HotplugWatcher`] and turns its raw events into a small,
//! non-blocking signal stream for the hardware thread. It never enumerates devices itself:
//! the caller reacts to [`MonitorSignal::RescanNow`] by running its existing scanner, so
//! there is exactly one `HidApi` in the process.
//!
//! Settling: any arrival or removal (re)starts a quiet timer ([`SETTLE_QUIET`]); the rescan
//! is signalled once the bus has been quiet that long, so a burst of interface events from one
//! plug-in costs one enumeration. A resume schedules a forced rescan [`RESUME_RESCAN_DELAY`]
//! later. If the watcher cannot start, the monitor degrades to a rescan every
//! [`DEGRADED_POLL`].

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use tracing::{debug, warn};

use crate::hotplug::{HotplugEvent, HotplugWatcher, Readiness};

/// Quiet period after the last arrival/removal before a rescan is signalled.
pub const SETTLE_QUIET: Duration = Duration::from_millis(750);
/// Delay between a resume and the forced rescan (devices re-enumerate after wake).
pub const RESUME_RESCAN_DELAY: Duration = Duration::from_secs(2);
/// Rescan period when the hotplug watcher is unavailable.
pub const DEGRADED_POLL: Duration = Duration::from_secs(10);

/// Why a rescan was signalled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RescanReason {
    /// Arrivals/removals went quiet.
    Settled,
    /// Forced rescan after a resume.
    Resume,
    /// Periodic poll because the watcher failed to start.
    DegradedPoll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSignal {
    RescanNow { reason: RescanReason },
    Suspending,
    Resumed,
}

/// Pure settle logic with an injectable clock; see the module docs for the rules.
#[derive(Debug, Clone)]
pub struct SettleTimer {
    quiet: Duration,
    resume_delay: Duration,
    settle_deadline: Option<Instant>,
    resume_deadline: Option<Instant>,
}

impl Default for SettleTimer {
    fn default() -> Self {
        Self::new(SETTLE_QUIET, RESUME_RESCAN_DELAY)
    }
}

impl SettleTimer {
    pub fn new(quiet: Duration, resume_delay: Duration) -> Self {
        Self {
            quiet,
            resume_delay,
            settle_deadline: None,
            resume_deadline: None,
        }
    }

    /// An interface arrived or was removed: (re)start the quiet timer.
    pub fn note_topology_event(&mut self, now: Instant) {
        self.settle_deadline = Some(now + self.quiet);
    }

    /// The system resumed: schedule the forced rescan.
    pub fn note_resume(&mut self, now: Instant) {
        self.resume_deadline = Some(now + self.resume_delay);
    }

    /// The system is suspending: a pending post-resume rescan no longer applies.
    pub fn note_suspend(&mut self) {
        self.resume_deadline = None;
    }

    /// `Some(reason)` once a deadline has passed (clearing every due deadline, so one rescan
    /// covers both), otherwise `None`. The resume reason wins when both are due.
    pub fn poll(&mut self, now: Instant) -> Option<RescanReason> {
        let settled = self.settle_deadline.is_some_and(|d| now >= d);
        let resumed = self.resume_deadline.is_some_and(|d| now >= d);
        if resumed {
            self.settle_deadline = None;
            self.resume_deadline = None;
            Some(RescanReason::Resume)
        } else if settled {
            self.settle_deadline = None;
            Some(RescanReason::Settled)
        } else {
            None
        }
    }
}

/// Fixed-period rescan used when the watcher is unavailable.
#[derive(Debug, Clone)]
struct DegradedPoller {
    period: Duration,
    next: Instant,
}

impl DegradedPoller {
    fn new(period: Duration, now: Instant) -> Self {
        Self {
            period,
            next: now + period,
        }
    }

    fn poll(&mut self, now: Instant) -> bool {
        if now >= self.next {
            self.next = now + self.period;
            true
        } else {
            false
        }
    }
}

enum Source {
    Events(Receiver<HotplugEvent>),
    Degraded(DegradedPoller),
}

pub struct DeviceMonitor {
    source: Source,
    timer: SettleTimer,
    /// Keeps the notifier thread alive; dropped (and stopped) with the monitor.
    _watcher: Option<HotplugWatcher>,
}

/// Readiness of the devnode behind a hidapi path, mapped to the hotplug vocabulary.
#[cfg(windows)]
fn devnode_readiness(path: &str) -> Readiness {
    match crate::devnode::readiness(path) {
        crate::devnode::Readiness::Ready => Readiness::Ready,
        other => Readiness::NotReady(format!("{other:?}")),
    }
}

#[cfg(not(windows))]
fn devnode_readiness(_path: &str) -> Readiness {
    Readiness::Ready
}

impl DeviceMonitor {
    /// Start the hotplug watcher. On failure, log a warning and fall back to periodic
    /// rescans; never fails.
    pub fn new(now: Instant) -> Self {
        let (tx, rx) = mpsc::channel();
        match HotplugWatcher::start(tx, Box::new(devnode_readiness)) {
            Ok(watcher) => Self {
                source: Source::Events(rx),
                timer: SettleTimer::default(),
                _watcher: Some(watcher),
            },
            Err(e) => {
                warn!(
                    error = %e,
                    "Hotplug watcher unavailable; rescanning every {}s",
                    DEGRADED_POLL.as_secs()
                );
                Self::degraded(now)
            }
        }
    }

    fn degraded(now: Instant) -> Self {
        Self {
            source: Source::Degraded(DegradedPoller::new(DEGRADED_POLL, now)),
            timer: SettleTimer::default(),
            _watcher: None,
        }
    }

    /// Monitor over an arbitrary event channel (no watcher thread), for tests.
    pub fn from_receiver(rx: Receiver<HotplugEvent>, timer: SettleTimer) -> Self {
        Self {
            source: Source::Events(rx),
            timer,
            _watcher: None,
        }
    }

    /// Whether the monitor fell back to periodic polling.
    pub fn is_degraded(&self) -> bool {
        matches!(self.source, Source::Degraded(_))
    }

    /// Drain pending events and report what the caller should do. Never blocks.
    /// Events are timestamped with `now`, the time they were observed here.
    pub fn poll(&mut self, now: Instant) -> Vec<MonitorSignal> {
        let mut signals = Vec::new();
        match &mut self.source {
            Source::Degraded(poller) => {
                if poller.poll(now) {
                    signals.push(MonitorSignal::RescanNow {
                        reason: RescanReason::DegradedPoll,
                    });
                }
            }
            Source::Events(rx) => {
                loop {
                    match rx.try_recv() {
                        Ok(event) => apply_event(&mut self.timer, &event, now, &mut signals),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            warn!("Hotplug event channel closed; rescanning every {}s", DEGRADED_POLL.as_secs());
                            self.source = Source::Degraded(DegradedPoller::new(DEGRADED_POLL, now));
                            break;
                        }
                    }
                }
                if let Some(reason) = self.timer.poll(now) {
                    signals.push(MonitorSignal::RescanNow { reason });
                }
            }
        }
        signals
    }
}

fn apply_event(
    timer: &mut SettleTimer,
    event: &HotplugEvent,
    now: Instant,
    signals: &mut Vec<MonitorSignal>,
) {
    match event {
        HotplugEvent::Arrived { path, ready } => {
            debug!(path = path.as_str(), ready = ?ready, "Hotplug: device arrived");
            timer.note_topology_event(now);
        }
        HotplugEvent::Removed { path } => {
            debug!(path = path.as_str(), "Hotplug: device removed");
            timer.note_topology_event(now);
        }
        HotplugEvent::Suspending => {
            timer.note_suspend();
            signals.push(MonitorSignal::Suspending);
        }
        HotplugEvent::Resumed => {
            timer.note_resume(now);
            signals.push(MonitorSignal::Resumed);
        }
        // Per-handle watches are not used by the monitor.
        HotplugEvent::QueryRemove { .. }
        | HotplugEvent::QueryRemoveFailed { .. }
        | HotplugEvent::RemoveComplete { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    fn timer() -> SettleTimer {
        SettleTimer::new(MS(750), MS(2000))
    }

    #[test]
    fn no_spurious_rescan_without_events() {
        let t0 = Instant::now();
        let mut t = timer();
        assert_eq!(t.poll(t0), None);
        assert_eq!(t.poll(t0 + Duration::from_secs(3600)), None);
    }

    #[test]
    fn rescan_fires_after_quiet_period_once() {
        let t0 = Instant::now();
        let mut t = timer();
        t.note_topology_event(t0);
        assert_eq!(t.poll(t0 + MS(749)), None);
        assert_eq!(t.poll(t0 + MS(750)), Some(RescanReason::Settled));
        assert_eq!(t.poll(t0 + MS(5000)), None, "fires once per burst");
    }

    #[test]
    fn burst_coalesces_into_one_rescan() {
        let t0 = Instant::now();
        let mut t = timer();
        for i in 0..10 {
            t.note_topology_event(t0 + MS(i * 50));
            assert_eq!(t.poll(t0 + MS(i * 50)), None);
        }
        // Last event at +450 ms -> due at +1200 ms.
        assert_eq!(t.poll(t0 + MS(1199)), None);
        assert_eq!(t.poll(t0 + MS(1200)), Some(RescanReason::Settled));
        assert_eq!(t.poll(t0 + MS(1300)), None);
    }

    #[test]
    fn new_event_restarts_the_quiet_timer() {
        let t0 = Instant::now();
        let mut t = timer();
        t.note_topology_event(t0);
        t.note_topology_event(t0 + MS(700));
        assert_eq!(t.poll(t0 + MS(750)), None, "original deadline superseded");
        assert_eq!(t.poll(t0 + MS(1449)), None);
        assert_eq!(t.poll(t0 + MS(1450)), Some(RescanReason::Settled));
    }

    #[test]
    fn resume_forces_rescan_after_delay() {
        let t0 = Instant::now();
        let mut t = timer();
        t.note_resume(t0);
        assert_eq!(t.poll(t0 + MS(1999)), None);
        assert_eq!(t.poll(t0 + MS(2000)), Some(RescanReason::Resume));
        assert_eq!(t.poll(t0 + MS(2001)), None);
    }

    #[test]
    fn settle_during_resume_wait_does_not_cancel_forced_rescan() {
        let t0 = Instant::now();
        let mut t = timer();
        t.note_resume(t0);
        t.note_topology_event(t0 + MS(100));
        assert_eq!(t.poll(t0 + MS(900)), Some(RescanReason::Settled));
        assert_eq!(t.poll(t0 + MS(2000)), Some(RescanReason::Resume));
    }

    #[test]
    fn simultaneous_deadlines_collapse_to_one_rescan() {
        let t0 = Instant::now();
        let mut t = timer();
        t.note_resume(t0);
        t.note_topology_event(t0 + MS(1500));
        assert_eq!(t.poll(t0 + MS(3000)), Some(RescanReason::Resume));
        assert_eq!(t.poll(t0 + MS(3001)), None);
    }

    #[test]
    fn suspend_cancels_pending_resume_rescan() {
        let t0 = Instant::now();
        let mut t = timer();
        t.note_resume(t0);
        t.note_suspend();
        assert_eq!(t.poll(t0 + MS(5000)), None);
    }

    #[test]
    fn degraded_poller_fires_every_period() {
        let t0 = Instant::now();
        let mut p = DegradedPoller::new(Duration::from_secs(10), t0);
        assert!(!p.poll(t0 + Duration::from_secs(9)));
        assert!(p.poll(t0 + Duration::from_secs(10)));
        assert!(!p.poll(t0 + Duration::from_secs(11)));
        assert!(p.poll(t0 + Duration::from_secs(20)));
    }

    #[test]
    fn monitor_maps_events_to_signals() {
        let t0 = Instant::now();
        let (tx, rx) = mpsc::channel();
        let mut m = DeviceMonitor::from_receiver(rx, timer());

        assert!(m.poll(t0).is_empty());

        tx.send(HotplugEvent::Arrived {
            path: "a".into(),
            ready: Readiness::Ready,
        })
        .unwrap();
        tx.send(HotplugEvent::Removed { path: "b".into() }).unwrap();
        tx.send(HotplugEvent::Suspending).unwrap();
        assert_eq!(m.poll(t0), vec![MonitorSignal::Suspending]);

        assert!(m.poll(t0 + MS(749)).is_empty());
        assert_eq!(
            m.poll(t0 + MS(750)),
            vec![MonitorSignal::RescanNow {
                reason: RescanReason::Settled
            }]
        );

        tx.send(HotplugEvent::Resumed).unwrap();
        assert_eq!(m.poll(t0 + MS(1000)), vec![MonitorSignal::Resumed]);
        assert_eq!(
            m.poll(t0 + MS(3000)),
            vec![MonitorSignal::RescanNow {
                reason: RescanReason::Resume
            }]
        );
    }

    #[test]
    fn handle_events_are_ignored() {
        use crate::hotplug::WatchToken;
        let t0 = Instant::now();
        let (tx, rx) = mpsc::channel();
        let mut m = DeviceMonitor::from_receiver(rx, timer());
        tx.send(HotplugEvent::QueryRemove {
            token: WatchToken(1),
        })
        .unwrap();
        tx.send(HotplugEvent::RemoveComplete {
            token: WatchToken(1),
        })
        .unwrap();
        assert!(m.poll(t0).is_empty());
        assert!(m.poll(t0 + Duration::from_secs(60)).is_empty());
    }

    #[test]
    fn closed_channel_degrades_instead_of_panicking() {
        let t0 = Instant::now();
        let (tx, rx) = mpsc::channel::<HotplugEvent>();
        let mut m = DeviceMonitor::from_receiver(rx, timer());
        drop(tx);
        assert!(m.poll(t0).is_empty());
        assert!(m.is_degraded());
        assert_eq!(
            m.poll(t0 + DEGRADED_POLL),
            vec![MonitorSignal::RescanNow {
                reason: RescanReason::DegradedPoll
            }]
        );
    }

    #[test]
    fn degraded_monitor_polls_without_watcher() {
        let t0 = Instant::now();
        let mut m = DeviceMonitor::degraded(t0);
        assert!(m.is_degraded());
        assert!(m.poll(t0 + Duration::from_secs(5)).is_empty());
        assert_eq!(m.poll(t0 + Duration::from_secs(10)).len(), 1);
    }
}
