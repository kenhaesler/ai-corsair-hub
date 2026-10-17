use std::time::Duration;

use anyhow::{Context, Result};
use corsair_common::Temperature;
use serde::Deserialize;
use tracing::{debug, info, warn};
use wmi::{FilterValue, WMIConnection};

use crate::{TemperatureSource, lhm};

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
        timeout: Duration,
    },
}

/// Plausible CPU temperature range (°C); readings outside it are rejected.
const CPU_TEMP_RANGE_C: std::ops::RangeInclusive<f64> = -10.0..=150.0;

/// False for NaN/inf (which `str::parse::<f64>` accepts) and out-of-range values.
fn is_plausible_cpu_temp(t: f64) -> bool {
    t.is_finite() && CPU_TEMP_RANGE_C.contains(&t)
}

/// One bounded LHM HTTP read. Any failure (timeout, bad JSON, missing sensor,
/// implausible value) is an error; never a stale value.
fn read_lhm_http(url: &str, sensor_id: &str, timeout: Duration) -> Result<f64> {
    let body = lhm::bounded_get_string(url, timeout)?;
    let root: LhmJsonNode = serde_json::from_str(&body).context("Failed to parse LHM JSON")?;
    let temp =
        find_sensor_value(&root, sensor_id).context("CPU temp sensor not found in LHM response")?;
    if !is_plausible_cpu_temp(temp) {
        anyhow::bail!("LHM CPU temp implausible: {}°C", temp);
    }
    Ok(temp)
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
        Self::try_lhm_http_with(base_url, lhm::HTTP_TIMEOUT)
    }

    fn try_lhm_http_with(base_url: &str, timeout: Duration) -> Result<Self> {
        let url = format!("{}/data.json", base_url);
        let body = lhm::bounded_get_string(&url, timeout).context("LHM HTTP not available")?;

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

        if !is_plausible_cpu_temp(*temp) {
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
                timeout,
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
            CpuBackend::LhmHttp {
                url,
                sensor_id,
                timeout,
            } => Ok(Temperature {
                celsius: read_lhm_http(url, sensor_id, *timeout)?,
            }),
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

    // --- bounded LHM HTTP reads, against fake local servers ---

    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Instant;

    const DEADLINE: Duration = Duration::from_millis(250);
    /// Allowed overshoot past the deadline.
    const SLACK: Duration = Duration::from_secs(1);
    /// How long stalled fake-server connections linger before being dropped, so
    /// detached threads never outlive the test process for long.
    const STALL: Duration = Duration::from_secs(5);

    enum Fake {
        /// Accept, read the request, never answer.
        NoHeaders,
        /// Send headers promising more body than is ever sent, then stall.
        StallMidBody,
        /// Answer 200 with this body.
        Ok(String),
    }

    /// Serve `fake` on an ephemeral port for any number of connections (detached
    /// threads). Returns the base URL.
    fn spawn_fake(fake: Fake) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fake = std::sync::Arc::new(fake);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let fake = fake.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(STALL));
                    let mut req = Vec::new();
                    let mut buf = [0u8; 512];
                    while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => req.extend_from_slice(&buf[..n]),
                        }
                    }
                    match &*fake {
                        Fake::NoHeaders => std::thread::sleep(STALL),
                        Fake::StallMidBody => {
                            let _ = stream.write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                                  Content-Length: 100000\r\n\r\n{\"Text\":\"Sen",
                            );
                            let _ = stream.flush();
                            std::thread::sleep(STALL);
                        }
                        Fake::Ok(body) => {
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                                 Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = stream.flush();
                        }
                    }
                });
            }
        });
        base
    }

    fn sensor_json(value: &str) -> String {
        format!(
            r#"{{"Text":"Sensor","Children":[{{"Text":"PC","Children":[
                {{"Text":"CPU","Children":[{{"Text":"Temperatures","Children":[
                    {{"Text":"Core (Tctl/Tdie)","SensorId":"/amdcpu/0/temperature/2",
                      "Type":"Temperature","Value":"{value}","Children":[]}}]}}]}}]}}]}}"#
        )
    }

    fn assert_err_within_deadline(url: &str) {
        let start = Instant::now();
        let res = read_lhm_http(url, "/amdcpu/0/temperature/2", DEADLINE);
        let elapsed = start.elapsed();
        assert!(res.is_err(), "expected Err, got {:?}", res.ok());
        assert!(elapsed < DEADLINE + SLACK, "took {:?}", elapsed);
    }

    #[test]
    fn test_lhm_read_times_out_when_no_headers() {
        let base = spawn_fake(Fake::NoHeaders);
        assert_err_within_deadline(&format!("{base}/data.json"));
    }

    #[test]
    fn test_lhm_read_times_out_mid_body() {
        let base = spawn_fake(Fake::StallMidBody);
        assert_err_within_deadline(&format!("{base}/data.json"));
    }

    #[test]
    fn test_lhm_read_rejects_malformed_json() {
        let base = spawn_fake(Fake::Ok("{not json".to_string()));
        assert!(read_lhm_http(&format!("{base}/data.json"), "x", DEADLINE).is_err());
    }

    #[test]
    fn test_lhm_read_rejects_missing_sensor() {
        let base = spawn_fake(Fake::Ok(sensor_json("50 °C")));
        let res = read_lhm_http(
            &format!("{base}/data.json"),
            "/nope",
            Duration::from_secs(2),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_lhm_read_rejects_non_finite_and_out_of_range() {
        for bad in ["NaN °C", "inf °C", "-inf °C", "200 °C", "-20 °C"] {
            let base = spawn_fake(Fake::Ok(sensor_json(bad)));
            let res = read_lhm_http(
                &format!("{base}/data.json"),
                "/amdcpu/0/temperature/2",
                Duration::from_secs(2),
            );
            assert!(res.is_err(), "{bad} should be rejected, got {:?}", res.ok());
        }
    }

    #[test]
    fn test_lhm_read_recovers_after_timeout() {
        let stalled = spawn_fake(Fake::NoHeaders);
        assert_err_within_deadline(&format!("{stalled}/data.json"));

        let healthy = spawn_fake(Fake::Ok(sensor_json("61,5 °C")));
        let t = read_lhm_http(
            &format!("{healthy}/data.json"),
            "/amdcpu/0/temperature/2",
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(t, 61.5);
    }

    #[test]
    fn test_try_lhm_http_times_out_on_stalled_server() {
        for fake in [Fake::NoHeaders, Fake::StallMidBody] {
            let base = spawn_fake(fake);
            let start = Instant::now();
            let res = CpuSensor::try_lhm_http_with(&base, DEADLINE);
            assert!(res.is_err());
            let elapsed = start.elapsed();
            assert!(elapsed < DEADLINE + SLACK, "took {:?}", elapsed);
        }
    }

    #[test]
    fn test_try_lhm_http_healthy_sample_selects_tctl_and_reads_value() {
        // Other sensors come first so selection must not just take the first temp.
        let body = r#"{"Text":"Sensor","Children":[{"Text":"PC","Children":[
            {"Text":"NVIDIA GeForce RTX 5090","Children":[{"Text":"Temperatures","Children":[
                {"Text":"GPU Core","SensorId":"/gpu-nvidia/0/temperature/0",
                 "Type":"Temperature","Value":"45,0 °C","Children":[]}]},
              {"Text":"Fans","Children":[
                {"Text":"GPU Fan","SensorId":"/gpu-nvidia/0/fan/1",
                 "Type":"Fan","Value":"1200 RPM","Children":[]}]}]},
            {"Text":"AMD Ryzen 9 9950X3D","Children":[{"Text":"Temperatures","Children":[
                {"Text":"CCD1 (Tdie)","SensorId":"/amdcpu/0/temperature/3",
                 "Type":"Temperature","Value":"52,1 °C","Children":[]},
                {"Text":"Core (Tctl/Tdie)","SensorId":"/amdcpu/0/temperature/2",
                 "Type":"Temperature","Value":"59,6 °C","Children":[]}]}]}]}]}"#;
        let base = spawn_fake(Fake::Ok(body.to_string()));

        let sensor = CpuSensor::try_lhm_http_with(&base, Duration::from_secs(2))
            .unwrap_or_else(|e| panic!("healthy sample rejected: {e:#}"));
        match &sensor.backend {
            CpuBackend::LhmHttp { sensor_id, .. } => {
                assert_eq!(sensor_id, "/amdcpu/0/temperature/2");
            }
            _ => panic!("expected LhmHttp backend"),
        }
        assert_eq!(sensor.read().unwrap().celsius, 59.6);
    }
}
