//! Pump telemetry from motherboard fan headers, via the LHM registry.
//!
//! A pump on a motherboard header (PUMP_FAN / W_PUMP / AIO_PUMP) shows up in
//! LibreHardwareMonitor as a `Fan` sensor whose name contains "Pump", with a
//! sibling `Control` of the same name carrying the duty the board applies.
//! LINK-attached pumps (XD5/XD6) are read by the control loop and merged by
//! the caller; this module only knows the motherboard side.

use std::sync::LazyLock;
use std::time::Duration;

use crate::lhm_registry::{LhmCache, LhmKind, LhmSensor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpSource {
    Motherboard,
    Link,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PumpReading {
    /// LHM sensor id for motherboard pumps, `link:<serial>:<channel>` for LINK pumps.
    pub id: String,
    /// Display name as the source reports it (e.g. `Pump Fan #1`, `XD5 Pump`).
    pub name: String,
    pub rpm: f64,
    /// Duty the controlling side applies, when known.
    pub duty_pct: Option<f64>,
    pub source: PumpSource,
}

fn is_pump_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("pump")
}

/// Strip a trailing ` #n` so `Pump Fan #1` pairs with the control `Pump Fan`.
fn base_name(name: &str) -> &str {
    match name.rfind(" #") {
        Some(i) if name[i + 2..].chars().all(|c| c.is_ascii_digit()) && i + 2 < name.len() => {
            &name[..i]
        }
        _ => name,
    }
}

/// Motherboard pumps in a parsed sensor list. Pure; no I/O.
pub fn motherboard_pumps(sensors: &[LhmSensor]) -> Vec<PumpReading> {
    sensors
        .iter()
        .filter(|s| {
            s.kind == LhmKind::Fan && s.hardware_id.starts_with("/lpc/") && is_pump_name(&s.name)
        })
        .map(|fan| {
            let base = base_name(&fan.name);
            let duty = sensors
                .iter()
                .find(|c| {
                    c.kind == LhmKind::Control
                        && c.hardware_id == fan.hardware_id
                        && (c.name == fan.name || base_name(&c.name) == base)
                })
                .map(|c| c.value);
            PumpReading {
                id: fan.sensor_id.clone(),
                name: fan.name.clone(),
                rpm: fan.rpm_value(),
                duty_pct: duty,
                source: PumpSource::Motherboard,
            }
        })
        .collect()
}

impl LhmSensor {
    fn rpm_value(&self) -> f64 {
        self.value.max(0.0)
    }
}

/// Read motherboard pumps through a shared cache.
pub fn read_motherboard_pumps(cache: &LhmCache) -> Vec<PumpReading> {
    match cache.sensors() {
        Ok(list) => motherboard_pumps(&list),
        Err(_) => Vec::new(),
    }
}

static HW_THREAD_CACHE: LazyLock<LhmCache> =
    LazyLock::new(|| LhmCache::new(Duration::from_millis(400)));

/// Convenience for callers without their own cache (the GUI hardware thread).
pub fn read_motherboard_pumps_now() -> Vec<PumpReading> {
    read_motherboard_pumps(&HW_THREAD_CACHE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lhm_registry::{FIXTURE, parse_lhm_tree};

    #[test]
    fn finds_pump_fan_with_duty() {
        let list = parse_lhm_tree(FIXTURE).unwrap();
        let pumps = motherboard_pumps(&list);
        assert_eq!(pumps.len(), 1, "{pumps:?}");
        let p = &pumps[0];
        assert_eq!(p.name, "Pump Fan #1");
        assert_eq!(p.rpm, 4428.0);
        assert_eq!(p.duty_pct, Some(89.0));
        assert_eq!(p.source, PumpSource::Motherboard);
        assert!(p.id.starts_with("/lpc/"));
    }

    #[test]
    fn ignores_zero_rpm_non_pump_headers() {
        let list = parse_lhm_tree(FIXTURE).unwrap();
        let names: Vec<String> = motherboard_pumps(&list)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert!(
            !names.iter().any(|n| n.starts_with("System Fan")),
            "{names:?}"
        );
        assert!(!names.iter().any(|n| n == "CPU Fan"), "{names:?}");
    }

    #[test]
    fn base_name_strips_index() {
        assert_eq!(base_name("Pump Fan #1"), "Pump Fan");
        assert_eq!(base_name("Pump Fan"), "Pump Fan");
        assert_eq!(base_name("W_PUMP+ #12"), "W_PUMP+");
    }
}
