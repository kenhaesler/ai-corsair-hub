//! Temperature sources derived from the LHM registry: motherboard, chipset,
//! VRM, memory, CPU CCD average, coolant thermistor and GPU memory junction.
//!
//! Each source is a *rule*, not a pinned sensor id: it re-selects the
//! matching sensors on every read, so a thermistor plugged in later, or a
//! board whose driver names things slightly differently, works without any
//! configuration. Ids are the stable strings fan groups reference in their
//! `temp_source.sensors` (e.g. `"board_system"`, `"water_inline"`).

use std::sync::Arc;

use anyhow::{Context, anyhow};
use corsair_common::Temperature;

use crate::TemperatureSource;
use crate::lhm_registry::{LhmCache, LhmKind, LhmSensor, is_connected_thermistor};

/// How several matching sensors collapse into one reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reduce {
    /// Hottest of the matches (memory modules).
    Max,
    /// Mean of the matches (CCDs when LHM gives no average).
    Mean,
    /// First match in tree order.
    First,
}

/// One derived source: its id, a human label, the selection rule and the reduction.
pub struct LhmTempSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub select: fn(&LhmSensor) -> bool,
    pub reduce: Reduce,
}

fn is_temp(s: &LhmSensor) -> bool {
    s.kind == LhmKind::Temperature
}
fn on_superio(s: &LhmSensor) -> bool {
    s.hardware_id.starts_with("/lpc/")
}
fn is_gpu(s: &LhmSensor) -> bool {
    s.hardware_id.starts_with("/gpu-")
}

fn sel_board_system(s: &LhmSensor) -> bool {
    is_temp(s)
        && on_superio(s)
        && (s.name == "System" || s.name == "Motherboard" || s.name == "System Temperature")
}
fn sel_chipset(s: &LhmSensor) -> bool {
    is_temp(s) && on_superio(s) && s.name.starts_with("Chipset")
}
fn sel_vrm(s: &LhmSensor) -> bool {
    is_temp(s) && on_superio(s) && s.name.starts_with("VRM")
}
fn sel_dimm(s: &LhmSensor) -> bool {
    is_temp(s)
        && (s.hardware_id.starts_with("/memory/")
            || s.hardware_id.starts_with("/ram/")
            || s.name.starts_with("DIMM"))
        && !s.name.contains("Limit")
        && !s.name.contains("Resolution")
}
fn sel_ccd_average(s: &LhmSensor) -> bool {
    is_temp(s) && s.name == "CCDs Average (Tdie)"
}
fn sel_ccd_any(s: &LhmSensor) -> bool {
    // "CCD1 (Tdie)", "CCD2 (Tdie)" but not the Max/Average aggregates
    is_temp(s)
        && s.name.starts_with("CCD")
        && s.name.ends_with("(Tdie)")
        && !s.name.starts_with("CCDs")
}
fn sel_water(s: &LhmSensor) -> bool {
    is_temp(s)
        && on_superio(s)
        && (s.name.contains("Thermistor")
            || s.name.contains("T_Sensor")
            || s.name.contains("Water"))
        && is_connected_thermistor(s.value)
}
fn sel_gpu_memory(s: &LhmSensor) -> bool {
    is_temp(s) && is_gpu(s) && s.name == "GPU Memory Junction"
}
fn sel_gpu_hotspot(s: &LhmSensor) -> bool {
    is_temp(s) && is_gpu(s) && s.name == "GPU Hot Spot"
}

/// The derived sources, in the order they are offered to the user.
pub const LHM_TEMP_SPECS: &[LhmTempSpec] = &[
    LhmTempSpec {
        id: "board_system",
        label: "Board",
        select: sel_board_system,
        reduce: Reduce::First,
    },
    LhmTempSpec {
        id: "chipset",
        label: "Chipset",
        select: sel_chipset,
        reduce: Reduce::First,
    },
    LhmTempSpec {
        id: "vrm",
        label: "VRM",
        select: sel_vrm,
        reduce: Reduce::Max,
    },
    LhmTempSpec {
        id: "dimm",
        label: "Memory",
        select: sel_dimm,
        reduce: Reduce::Max,
    },
    LhmTempSpec {
        id: "cpu_ccd",
        label: "CPU cores",
        select: sel_ccd_average,
        reduce: Reduce::First,
    },
    LhmTempSpec {
        id: "water_inline",
        label: "Coolant",
        select: sel_water,
        reduce: Reduce::First,
    },
    LhmTempSpec {
        id: "gpu_memory",
        label: "GPU memory",
        select: sel_gpu_memory,
        reduce: Reduce::First,
    },
];

/// Evaluate a spec against a sensor list. `None` when nothing matches.
pub fn evaluate(spec: &LhmTempSpec, sensors: &[LhmSensor]) -> Option<f64> {
    let mut matches: Vec<f64> = sensors
        .iter()
        .filter(|s| (spec.select)(s))
        .map(|s| s.value)
        .collect();
    // CPU cores: fall back to the mean of the individual CCDs when the driver
    // offers no "CCDs Average"; GPU memory: fall back to the hot spot.
    if matches.is_empty() {
        let fallback: Option<fn(&LhmSensor) -> bool> = match spec.id {
            "cpu_ccd" => Some(sel_ccd_any),
            "gpu_memory" => Some(sel_gpu_hotspot),
            _ => None,
        };
        if let Some(f) = fallback {
            matches = sensors.iter().filter(|s| f(s)).map(|s| s.value).collect();
            if spec.id == "cpu_ccd" && !matches.is_empty() {
                return Some(matches.iter().sum::<f64>() / matches.len() as f64);
            }
        }
    }
    if matches.is_empty() {
        return None;
    }
    Some(match spec.reduce {
        Reduce::Max => matches.iter().cloned().fold(f64::MIN, f64::max),
        Reduce::Mean => matches.iter().sum::<f64>() / matches.len() as f64,
        Reduce::First => matches[0],
    })
}

/// A fan-group temperature source backed by one spec over the shared cache.
pub struct LhmTempSource {
    spec: &'static LhmTempSpec,
    cache: Arc<LhmCache>,
}

impl LhmTempSource {
    pub fn new(spec: &'static LhmTempSpec, cache: Arc<LhmCache>) -> Self {
        Self { spec, cache }
    }
    pub fn label(&self) -> &'static str {
        self.spec.label
    }
}

impl TemperatureSource for LhmTempSource {
    fn name(&self) -> &str {
        self.spec.id
    }
    fn read(&self) -> anyhow::Result<Temperature> {
        let sensors = self
            .cache
            .sensors()
            .context("LibreHardwareMonitor unavailable")?;
        let celsius = evaluate(self.spec, &sensors)
            .ok_or_else(|| anyhow!("{} sensor not connected", self.spec.label))?;
        Ok(Temperature { celsius })
    }
}

/// Build one source per spec that has a connected match right now.
///
/// Sources absent at startup are simply not offered (the Sensor Mix lists
/// what `available_sensors()` returns); a later `read()` of an offered source
/// whose sensor vanished errors, which the control loop treats as stale and
/// drops from the mix.
pub fn discover_lhm_temp_sources(cache: Arc<LhmCache>) -> Vec<Box<dyn TemperatureSource>> {
    let Ok(sensors) = cache.sensors() else {
        return Vec::new();
    };
    LHM_TEMP_SPECS
        .iter()
        .filter(|spec| evaluate(spec, &sensors).is_some())
        .map(|spec| {
            Box::new(LhmTempSource::new(spec, Arc::clone(&cache))) as Box<dyn TemperatureSource>
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lhm_registry::{FIXTURE, parse_lhm_tree};
    use std::collections::BTreeSet;

    fn fixture() -> Vec<LhmSensor> {
        parse_lhm_tree(FIXTURE).unwrap()
    }
    fn spec(id: &str) -> &'static LhmTempSpec {
        LHM_TEMP_SPECS.iter().find(|s| s.id == id).unwrap()
    }

    #[test]
    fn discovers_expected_ids() {
        let cache = Arc::new(LhmCache::with_sensors(fixture()));
        let ids: BTreeSet<String> = discover_lhm_temp_sources(cache)
            .iter()
            .map(|s| s.name().to_string())
            .collect();
        let want: BTreeSet<String> = [
            "board_system",
            "chipset",
            "vrm",
            "dimm",
            "cpu_ccd",
            "gpu_memory",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            ids, want,
            "water_inline must be absent while the thermistor reads -40"
        );
    }

    #[test]
    fn dimm_reduces_to_max() {
        assert_eq!(evaluate(spec("dimm"), &fixture()), Some(57.5));
    }

    #[test]
    fn ccd_uses_lhm_average() {
        assert_eq!(evaluate(spec("cpu_ccd"), &fixture()), Some(64.5));
    }

    #[test]
    fn ccd_falls_back_to_mean_of_ccds() {
        let list: Vec<LhmSensor> = fixture()
            .into_iter()
            .filter(|s| s.name != "CCDs Average (Tdie)")
            .collect();
        let v = evaluate(spec("cpu_ccd"), &list).unwrap();
        assert!(
            (v - 64.5).abs() < 0.01,
            "mean of CCD1 65.6 and CCD2 63.4, got {v}"
        );
    }

    #[test]
    fn board_values() {
        let list = fixture();
        assert_eq!(evaluate(spec("board_system"), &list), Some(56.0));
        assert_eq!(evaluate(spec("chipset"), &list), Some(60.0));
        assert_eq!(evaluate(spec("vrm"), &list), Some(31.0));
        assert_eq!(evaluate(spec("gpu_memory"), &list), Some(44.0));
    }

    #[test]
    fn water_inline_appears_when_thermistor_connected() {
        let mut list = fixture();
        assert_eq!(evaluate(spec("water_inline"), &list), None);
        for s in list.iter_mut().filter(|s| s.name == "Thermistor Sensor") {
            s.value = 31.2;
        }
        assert_eq!(evaluate(spec("water_inline"), &list), Some(31.2));
        let cache = Arc::new(LhmCache::with_sensors(list));
        let src = LhmTempSource::new(spec("water_inline"), cache);
        assert_eq!(src.name(), "water_inline");
        assert_eq!(src.read().unwrap().celsius, 31.2);
    }

    #[test]
    fn missing_source_reads_as_error_not_zero() {
        let cache = Arc::new(LhmCache::with_sensors(fixture()));
        let src = LhmTempSource::new(spec("water_inline"), cache);
        assert!(src.read().is_err());
    }
}
