use anyhow::{Context, Result};
use corsair_common::Temperature;
use serde::Deserialize;
use tracing::{debug, info, warn};
use wmi::{FilterValue, WMIConnection};

use crate::TemperatureSource;

// --- WMI structs ---

#[derive(Deserialize)]
#[serde(rename = "MSAcpi_ThermalZoneTemperature")]
#[allow(non_snake_case)]
struct ThermalZone {
    CurrentTemperature: u32,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_PerfFormattedData_Counters_ThermalZoneInformation")]
#[allow(non_snake_case)]
struct ThermalZoneInfo {
    Temperature: u32,
}

#[derive(Deserialize)]
#[serde(rename = "Sensor")]
#[allow(non_snake_case)]
struct LhmWmiSensor {
    Name: String,
    Value: f32,
    Parent: String,
}

// --- LHM HTTP JSON structs ---

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct LhmJsonNode {
    #[serde(default)]
    Text: String,
    #[serde(default)]
    Value: String,
    #[serde(default)]
    SensorId: String,
    #[serde(default)]
    Type: String,
    #[serde(default)]
    Children: Vec<LhmJsonNode>,
}

/// Which backend provides CPU temperature.
enum CpuBackend {
    AcpiWmi { wmi_con: WMIConnection },
    Cimv2 { wmi_con: WMIConnection },
    LhmWmi {
        wmi_con: WMIConnection,
        sensor_name: String,
    },
    LhmHttp {
        url: String,
        sensor_id: String,
    },
}

/// CPU temperature sensor.
///
/// Tries backends in order:
/// 1. LibreHardwareMonitor HTTP API (most reliable on AMD Zen)
/// 2. LibreHardwareMonitor/OpenHardwareMonitor WMI
/// 3. root\WMI MSAcpi_ThermalZoneTemperature
/// 4. root\CIMV2 ThermalZoneInformation
pub struct CpuSensor {
    name: String,
    backend: CpuBackend,
}

impl CpuSensor {
    pub fn new() -> Result<Self> {
        // 1. LHM HTTP — works with winget/portable installs
        if let Ok(sensor) = Self::try_lhm_http("http://127.0.0.1:8085") {
            return Ok(sensor);
        }
        // 2. LHM/OHM WMI
        if let Ok(sensor) = Self::try_lhm_wmi("root\\LibreHardwareMonitor") {
            return Ok(sensor);
        }
        if let Ok(sensor) = Self::try_lhm_wmi("root\\OpenHardwareMonitor") {
            return Ok(sensor);
        }
        // 3. Native ACPI WMI
        match WMIConnection::with_namespace_path("root\\WMI") {
            Ok(wmi_con) => {
                match wmi_con.query::<ThermalZone>() {
                    Ok(zones) if !zones.is_empty() => {
                        let temp = kelvin_tenths_to_celsius(zones[0].CurrentTemperature);
                        info!(temp_c = temp, "CPU sensor initialized via root\\WMI");
                        return Ok(Self {
                            name: "CPU".to_string(),
                            backend: CpuBackend::AcpiWmi { wmi_con },
                        });
                    }
                    _ => warn!("root\\WMI thermal zones empty, trying CIMV2 fallback"),
                }
            }
            Err(e) => warn!(error = %e, "root\\WMI unavailable"),
        }
        // 4. CIMV2 fallback
        Self::try_cimv2()
    }

    fn try_lhm_http(base_url: &str) -> Result<Self> {
        let url = format!("{}/data.json", base_url);
        let body: String = ureq::get(&url)
            .call()
            .context("LHM HTTP not available")?
            .body_mut()
            .read_to_string()
            .context("Failed to read LHM response")?;

        let root: LhmJsonNode =
            serde_json::from_str(&body).context("Failed to parse LHM JSON")?;

        // Walk the tree to find CPU temperature sensors
        let mut temps = Vec::new();
        collect_cpu_temps(&root, &mut temps);

        // Pick best sensor: Tctl/Tdie > CPU Package > any amdcpu temp
        let best_names = ["Core (Tctl/Tdie)", "CPU Package", "Tctl/Tdie"];
        let sensor = best_names
            .iter()
            .find_map(|&name| temps.iter().find(|(n, _, _)| n == name))
            .or_else(|| {
                temps
                    .iter()
                    .find(|(_, id, _)| id.contains("amdcpu") || id.contains("intelcpu"))
            });

        let (name, sensor_id, temp) = match sensor {
            Some(s) => s,
            None => anyhow::bail!("No CPU temp sensor found in LHM HTTP response"),
        };

        if *temp < -10.0 || *temp > 150.0 {
            anyhow::bail!("LHM CPU temp out of range: {}°C", temp);
        }

        info!(
            temp_c = temp,
            sensor = name.as_str(),
            "CPU sensor initialized via LibreHardwareMonitor HTTP"
        );

        Ok(Self {
            name: "CPU".to_string(),
            backend: CpuBackend::LhmHttp {
                url: url.clone(),
                sensor_id: sensor_id.clone(),
            },
        })
    }

    fn try_lhm_wmi(namespace: &str) -> Result<Self> {
        let wmi_con = WMIConnection::with_namespace_path(namespace)
            .with_context(|| format!("{} namespace not available", namespace))?;

        let mut filters = std::collections::HashMap::new();
        filters.insert("SensorType".to_string(), FilterValue::Str("Temperature"));
        let sensors: Vec<LhmWmiSensor> = wmi_con
            .filtered_query(&filters)
            .context("Failed to query LHM WMI sensors")?;

        if sensors.is_empty() {
            anyhow::bail!("No temperature sensors in {}", namespace);
        }

        let best_names = ["Core (Tctl/Tdie)", "CPU Package", "Tctl/Tdie"];
        let sensor = best_names
            .iter()
            .find_map(|&target| sensors.iter().find(|s| s.Name == target))
            .or_else(|| {
                sensors
                    .iter()
                    .find(|s| s.Parent.contains("cpu") || s.Parent.contains("amdcpu"))
            });

        let sensor = match sensor {
            Some(s) => s,
            None => anyhow::bail!("No CPU temp sensor in {}", namespace),
        };

        let temp = sensor.Value as f64;
        if temp < -10.0 || temp > 150.0 {
            anyhow::bail!("LHM WMI CPU temp out of range: {}°C", temp);
        }

        let source = if namespace.contains("Libre") {
            "LibreHardwareMonitor"
        } else {
            "OpenHardwareMonitor"
        };
        info!(
            temp_c = temp,
            sensor = sensor.Name.as_str(),
            "CPU sensor initialized via {} WMI",
            source
        );

        Ok(Self {
            name: "CPU".to_string(),
            backend: CpuBackend::LhmWmi {
                wmi_con,
                sensor_name: sensor.Name.clone(),
            },
        })
    }

    fn try_cimv2() -> Result<Self> {
        let wmi_con = WMIConnection::with_namespace_path("root\\CIMV2")
            .context("Failed to connect to CIMV2")?;
        let zones: Vec<ThermalZoneInfo> = wmi_con
            .query()
            .context("No CIMV2 thermal data")?;
        if zones.is_empty() {
            anyhow::bail!(
                "No thermal zone sensors found. \
                 Install and run LibreHardwareMonitor for AMD CPU temp support."
            );
        }
        let temp = zones[0].Temperature as f64 - 273.15;
        debug!(temp_c = temp, "CPU sensor initialized via CIMV2 fallback");
        Ok(Self {
            name: "CPU".to_string(),
            backend: CpuBackend::Cimv2 { wmi_con },
        })
    }
}

impl TemperatureSource for CpuSensor {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> Result<Temperature> {
        match &self.backend {
            CpuBackend::LhmHttp { url, sensor_id } => {
                let body: String = ureq::get(url)
                    .call()
                    .context("LHM HTTP request failed")?
                    .body_mut()
                    .read_to_string()
                    .context("Failed to read LHM response")?;
                let root: LhmJsonNode =
                    serde_json::from_str(&body).context("Failed to parse LHM JSON")?;
                let temp = find_sensor_value(&root, sensor_id)
                    .context("CPU temp sensor not found in LHM response")?;
                Ok(Temperature { celsius: temp })
            }
            CpuBackend::LhmWmi {
                wmi_con,
                sensor_name,
            } => {
                let mut filters = std::collections::HashMap::new();
                filters.insert("SensorType".to_string(), FilterValue::Str("Temperature"));
                let sensors: Vec<LhmWmiSensor> = wmi_con
                    .filtered_query(&filters)
                    .context("Failed to read LHM WMI")?;
                let sensor = sensors
                    .iter()
                    .find(|s| s.Name == *sensor_name)
                    .or_else(|| sensors.first())
                    .context("No LHM WMI temperature sensors")?;
                Ok(Temperature {
                    celsius: sensor.Value as f64,
                })
            }
            CpuBackend::AcpiWmi { wmi_con } => {
                let zones: Vec<ThermalZone> =
                    wmi_con.query().context("Failed to read WMI thermal zone")?;
                let zone = zones.first().context("No WMI thermal zone data")?;
                Ok(Temperature {
                    celsius: kelvin_tenths_to_celsius(zone.CurrentTemperature),
                })
            }
            CpuBackend::Cimv2 { wmi_con } => {
                let zones: Vec<ThermalZoneInfo> =
                    wmi_con.query().context("Failed to read CIMV2 thermal zone")?;
                let zone = zones.first().context("No CIMV2 thermal zone data")?;
                Ok(Temperature {
                    celsius: zone.Temperature as f64 - 273.15,
                })
            }
        }
    }
}

/// Walk the LHM JSON tree collecting temperature sensors with CPU-related SensorIds.
fn collect_cpu_temps(node: &LhmJsonNode, results: &mut Vec<(String, String, f64)>) {
    if node.Type == "Temperature" && !node.SensorId.is_empty() {
        if let Some(temp) = parse_lhm_value(&node.Value) {
            results.push((node.Text.clone(), node.SensorId.clone(), temp));
        }
    }
    for child in &node.Children {
        collect_cpu_temps(child, results);
    }
}

/// Find a specific sensor by SensorId in the LHM JSON tree.
fn find_sensor_value(node: &LhmJsonNode, target_id: &str) -> Option<f64> {
    if node.SensorId == target_id {
        return parse_lhm_value(&node.Value);
    }
    for child in &node.Children {
        if let Some(v) = find_sensor_value(child, target_id) {
            return Some(v);
        }
    }
    None
}

/// Parse LHM value string like "59.6 °C" → 59.6
fn parse_lhm_value(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.replace(',', ".").parse().ok()
}

fn kelvin_tenths_to_celsius(tenths_k: u32) -> f64 {
    tenths_k as f64 / 10.0 - 273.15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelvin_to_celsius() {
        let celsius = kelvin_tenths_to_celsius(3182);
        assert!((celsius - 45.05).abs() < 0.01, "got {}", celsius);
    }

    #[test]
    fn test_kelvin_to_celsius_boiling() {
        let celsius = kelvin_tenths_to_celsius(3731);
        assert!((celsius - 99.95).abs() < 0.01, "got {}", celsius);
    }

    #[test]
    fn test_kelvin_to_celsius_freezing() {
        let celsius = kelvin_tenths_to_celsius(2731);
        assert!((celsius - (-0.05)).abs() < 0.01, "got {}", celsius);
    }

    #[test]
    fn test_parse_lhm_value() {
        assert_eq!(parse_lhm_value("59.6 °C"), Some(59.6));
        assert_eq!(parse_lhm_value("59,6 °C"), Some(59.6));
        assert_eq!(parse_lhm_value(""), None);
    }
}
