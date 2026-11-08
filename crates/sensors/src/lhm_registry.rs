//! One typed view of LibreHardwareMonitor's `data.json` tree.
//!
//! The other LHM readers in this crate each walk the raw tree for one purpose
//! (CPU temperature, CPU metrics, PSU). This module parses the tree once into
//! a flat list of typed sensors, each tagged with the hardware node it belongs
//! to, so new sources (board temperatures, memory, pump headers, a coolant
//! thermistor) can be selected by `kind` + `name` + hardware id prefix without
//! hard-coding any `SensorId`. `LhmCache` lets every source that reads in the
//! same tick share one HTTP round trip.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::lhm::{bounded_get_string, HTTP_TIMEOUT, LHM_URL};

/// The sensor classes LHM reports (its `Type` field), collapsed to what we use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LhmKind {
    Temperature,
    Fan,
    Control,
    Power,
    Load,
    Voltage,
    Other,
}

impl LhmKind {
    fn from_type(t: &str) -> Self {
        match t {
            "Temperature" => Self::Temperature,
            "Fan" => Self::Fan,
            "Control" => Self::Control,
            "Power" => Self::Power,
            "Load" => Self::Load,
            "Voltage" => Self::Voltage,
            _ => Self::Other,
        }
    }
}

/// One leaf sensor of the LHM tree with its owning hardware node.
#[derive(Debug, Clone, PartialEq)]
pub struct LhmSensor {
    /// LHM's stable sensor path, e.g. `/lpc/nct6687dr/0/fan/1`.
    pub sensor_id: String,
    /// Nearest ancestor hardware id, e.g. `/lpc/nct6687dr/0`, `/ram/0`, `/gpu-nvidia/0`.
    pub hardware_id: String,
    /// Nearest ancestor hardware display name, e.g. `Nuvoton NCT6687D-R`.
    pub hardware_name: String,
    /// The sensor's display name, e.g. `Pump Fan #1`, `DIMM #1`, `Thermistor Sensor`.
    pub name: String,
    pub kind: LhmKind,
    pub value: f64,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct Node {
    #[serde(default)]
    Text: String,
    #[serde(default)]
    Value: String,
    #[serde(default)]
    SensorId: String,
    #[serde(default)]
    Type: String,
    #[serde(default)]
    HardwareId: String,
    #[serde(default)]
    Children: Vec<Node>,
}

/// Parse an LHM value string such as `"59.6 °C"`, `"59,6 °C"`, `"4428 RPM"` or `"89.0 %"`.
pub fn parse_lhm_value(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.replace(',', ".").parse().ok()
}

/// Parse the whole `data.json` body into a flat sensor list. Pure; no I/O.
pub fn parse_lhm_tree(json: &str) -> anyhow::Result<Vec<LhmSensor>> {
    let root: Node = serde_json::from_str(json)?;
    let mut out = Vec::new();
    walk(&root, "", "", &mut out);
    Ok(out)
}

fn walk(node: &Node, hw_id: &str, hw_name: &str, out: &mut Vec<LhmSensor>) {
    let (hw_id, hw_name) = if node.HardwareId.is_empty() {
        (hw_id, hw_name)
    } else {
        (node.HardwareId.as_str(), node.Text.as_str())
    };
    if node.Children.is_empty() && !node.SensorId.is_empty() {
        if let Some(value) = parse_lhm_value(&node.Value) {
            out.push(LhmSensor {
                sensor_id: node.SensorId.clone(),
                hardware_id: hw_id.to_string(),
                hardware_name: hw_name.to_string(),
                name: node.Text.clone(),
                kind: LhmKind::from_type(&node.Type),
                value,
            });
        }
        return;
    }
    for child in &node.Children {
        walk(child, hw_id, hw_name, out);
    }
}

/// GET the live tree and parse it.
pub fn fetch_lhm_sensors() -> anyhow::Result<Vec<LhmSensor>> {
    let body = bounded_get_string(LHM_URL, HTTP_TIMEOUT)?;
    parse_lhm_tree(&body)
}

/// A motherboard thermistor header with nothing attached reads a sentinel
/// (−40 °C on Nuvoton, 0 °C on some ITE chips). Anything outside the range a
/// coolant or air probe can report is "not connected".
pub fn is_connected_thermistor(value_c: f64) -> bool {
    value_c.is_finite() && value_c > -30.0 && value_c != 0.0 && value_c <= 120.0
}

/// Shares one fetch of the LHM tree between every reader in a tick.
///
/// `sensors()` returns the cached list while it is younger than `ttl`, and
/// refetches otherwise. Readers hold an `Arc<LhmCache>`.
pub struct LhmCache {
    ttl: Duration,
    state: Mutex<Option<(Instant, Arc<Vec<LhmSensor>>)>>,
}

impl LhmCache {
    pub fn new(ttl: Duration) -> Self {
        Self { ttl, state: Mutex::new(None) }
    }

    pub fn sensors(&self) -> anyhow::Result<Arc<Vec<LhmSensor>>> {
        let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((at, list)) = guard.as_ref() {
            if at.elapsed() < self.ttl {
                return Ok(Arc::clone(list));
            }
        }
        let fresh = Arc::new(fetch_lhm_sensors()?);
        *guard = Some((Instant::now(), Arc::clone(&fresh)));
        Ok(fresh)
    }

    /// Test seam: prime the cache with a parsed list (never expires while `ttl` is huge).
    #[cfg(test)]
    pub(crate) fn with_sensors(list: Vec<LhmSensor>) -> Self {
        Self {
            ttl: Duration::from_secs(u64::MAX / 4),
            state: Mutex::new(Some((Instant::now(), Arc::new(list)))),
        }
    }
}

#[cfg(test)]
pub(crate) const FIXTURE: &str = include_str!("../tests/fixtures/lhm-godlike.json");

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<LhmSensor> {
        parse_lhm_tree(FIXTURE).expect("fixture parses")
    }
    fn find<'a>(list: &'a [LhmSensor], kind: LhmKind, name: &str) -> &'a LhmSensor {
        list.iter()
            .find(|s| s.kind == kind && s.name == name)
            .unwrap_or_else(|| panic!("no {kind:?} sensor named {name:?}"))
    }

    #[test]
    fn parses_pump_fan_and_control_from_fixture() {
        let list = fixture();
        let fan = find(&list, LhmKind::Fan, "Pump Fan #1");
        assert_eq!(fan.value, 4428.0);
        assert!(fan.hardware_name.starts_with("Nuvoton"), "{}", fan.hardware_name);
        assert!(fan.hardware_id.starts_with("/lpc/"), "{}", fan.hardware_id);
        let ctl = find(&list, LhmKind::Control, "Pump Fan");
        assert_eq!(ctl.value, 89.0);
    }

    #[test]
    fn parses_board_and_dimm_temperatures() {
        let list = fixture();
        let t = |name: &str| find(&list, LhmKind::Temperature, name).value;
        assert_eq!(t("System"), 56.0);
        assert_eq!(t("Chipset"), 60.0);
        assert_eq!(t("VRM MOS"), 31.0);
        assert_eq!(t("DIMM #1"), 56.8);
        assert_eq!(t("DIMM #3"), 57.5);
        assert_eq!(t("CCDs Average (Tdie)"), 64.5);
        assert_eq!(t("GPU Memory Junction"), 44.0);
        assert_eq!(t("Thermistor Sensor"), -40.0);
        assert!(find(&list, LhmKind::Temperature, "DIMM #1").hardware_id.starts_with("/memory/"));
    }

    #[test]
    fn parse_value_handles_units_and_comma() {
        assert_eq!(parse_lhm_value("59,6 °C"), Some(59.6));
        assert_eq!(parse_lhm_value("4428 RPM"), Some(4428.0));
        assert_eq!(parse_lhm_value("89.0 %"), Some(89.0));
        assert_eq!(parse_lhm_value(""), None);
        assert_eq!(parse_lhm_value("1:44:52"), None);
    }

    #[test]
    fn thermistor_sentinels_are_not_connected() {
        assert!(!is_connected_thermistor(-40.0));
        assert!(!is_connected_thermistor(0.0));
        assert!(!is_connected_thermistor(130.0));
        assert!(!is_connected_thermistor(f64::NAN));
        assert!(is_connected_thermistor(31.5));
    }

    #[test]
    fn cache_serves_primed_list() {
        let cache = LhmCache::with_sensors(fixture());
        let a = cache.sensors().unwrap();
        let b = cache.sensors().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!a.is_empty());
    }
}
