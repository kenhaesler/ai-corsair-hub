//! Auto-detect and launch LibreHardwareMonitor before sensor initialization.
//!
//! LHM's HTTP webserver at `http://127.0.0.1:8085/data.json` provides accurate
//! CPU temps on AMD Zen 5. Without it, the app falls back to unreliable
//! Windows ACPI/CIMV2 thermal zones.
//!
//! Also provides PSU telemetry when LHM has the Corsair PSU plugin active,
//! avoiding USB HID contention with our direct reads.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use corsair_hid::{PsuStatus, RailReading};
use serde::Deserialize;
use tracing::{debug, info, warn};

const LHM_URL: &str = "http://127.0.0.1:8085/data.json";
const HTTP_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

/// Ensure LibreHardwareMonitor is running with its webserver enabled.
///
/// Returns `true` if the LHM HTTP endpoint is responding, `false` otherwise.
/// On failure, the caller should fall back to WMI/ACPI sensors.
pub fn ensure_lhm_running(custom_exe_path: Option<&str>) -> bool {
    // Fast path: already running (~2ms)
    if is_webserver_responding() {
        info!("LibreHardwareMonitor webserver confirmed available");
        return true;
    }

    // Try to find and launch it
    let exe_path = match find_lhm_executable(custom_exe_path) {
        Some(path) => path,
        None => {
            warn!(
                "LibreHardwareMonitor not found. \
                 Install via: winget install LibreHardwareMonitor.LibreHardwareMonitor"
            );
            return false;
        }
    };

    launch_and_wait(&exe_path)
}

/// Check if the LHM HTTP webserver is responding.
fn is_webserver_responding() -> bool {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    match agent.get(LHM_URL).call() {
        Ok(resp) => resp.status() == 200,
        Err(_) => false,
    }
}

/// Search for the LHM executable in known locations.
fn find_lhm_executable(custom_path: Option<&str>) -> Option<PathBuf> {
    // 1. User-configured path
    if let Some(path) = custom_path {
        let p = PathBuf::from(path);
        if p.exists() {
            info!("Using configured LHM path: {}", p.display());
            return Some(p);
        }
        warn!("Configured LHM path not found: {}", path);
    }

    // 2. Standard install path (MSI)
    let standard = PathBuf::from(r"C:\Program Files\LibreHardwareMonitor\LibreHardwareMonitor.exe");
    if standard.exists() {
        info!("Found LHM at standard path: {}", standard.display());
        return Some(standard);
    }

    // 3. Winget per-user install path
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let winget_dir = PathBuf::from(&local_appdata)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages");
        if let Ok(entries) = std::fs::read_dir(&winget_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("LibreHardwareMonitor.LibreHardwareMonitor") {
                    let exe = entry.path().join("LibreHardwareMonitor.exe");
                    if exe.exists() {
                        info!("Found LHM at winget path: {}", exe.display());
                        return Some(exe);
                    }
                }
            }
        }
    }

    None
}

/// Launch LHM and poll until its webserver responds (or timeout).
///
/// LHM requires admin privileges to access hardware sensors, so we use
/// PowerShell Start-Process -Verb RunAs to trigger UAC elevation.
fn launch_and_wait(exe_path: &PathBuf) -> bool {
    info!("Launching LibreHardwareMonitor (elevated): {}", exe_path.display());

    // Use PowerShell to launch with UAC elevation (runas verb)
    let exe_str = exe_path.to_string_lossy();
    match std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -Verb RunAs -WindowStyle Minimized",
                exe_str
            ),
        ])
        .spawn()
    {
        Ok(mut child) => {
            // Wait for the PowerShell process to finish (UAC prompt)
            let _ = child.wait();
        }
        Err(e) => {
            warn!("Failed to launch LibreHardwareMonitor: {}", e);
            return false;
        }
    }

    // Poll for webserver readiness
    let start = Instant::now();
    while start.elapsed() < STARTUP_TIMEOUT {
        std::thread::sleep(POLL_INTERVAL);
        if is_webserver_responding() {
            info!(
                "LibreHardwareMonitor webserver confirmed available (startup took {:.1}s)",
                start.elapsed().as_secs_f64()
            );
            return true;
        }
    }

    warn!(
        "LibreHardwareMonitor launched but webserver did not respond within {}s. \
         Is the webserver enabled in LHM settings?",
        STARTUP_TIMEOUT.as_secs()
    );
    false
}

// --- LHM PSU reader ---

/// JSON node from LHM's data.json tree.
#[derive(Deserialize)]
#[allow(non_snake_case, dead_code)]
struct LhmNode {
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
    Children: Vec<LhmNode>,
}

/// Read PSU telemetry from LHM's HTTP API.
///
/// Returns `Some(PsuStatus)` if LHM has a Corsair PSU node (`/psu/corsair/*`).
/// This avoids USB HID contention — LHM already has the device open.
pub fn read_psu_from_lhm() -> Option<PsuStatus> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let body: String = agent.get(LHM_URL).call().ok()?.body_mut().read_to_string().ok()?;
    let root: LhmNode = serde_json::from_str(&body).ok()?;

    // Find the PSU hardware node (HardwareId starts with /psu/corsair)
    let psu_node = find_hw_node(&root, "/psu/corsair/")?;
    let prefix = &psu_node.HardwareId; // e.g. "/psu/corsair/0"

    // Extract sensor values by SensorId pattern
    let temp_vrm = find_sensor(&psu_node, &format!("{}/temperature/0", prefix))?;
    let temp_case = find_sensor(&psu_node, &format!("{}/temperature/1", prefix))?;
    let fan_rpm = find_sensor(&psu_node, &format!("{}/fan/", prefix)).unwrap_or(0.0) as u16;
    let input_voltage = find_sensor(&psu_node, &format!("{}/voltage/3", prefix))?;

    let rail_12v = RailReading {
        voltage: find_sensor(&psu_node, &format!("{}/voltage/4", prefix))?,
        current: find_sensor(&psu_node, &format!("{}/current/7", prefix)).unwrap_or(0.0),
        power: find_sensor(&psu_node, &format!("{}/power/10", prefix)).unwrap_or(0.0),
    };
    let rail_5v = RailReading {
        voltage: find_sensor(&psu_node, &format!("{}/voltage/5", prefix))?,
        current: find_sensor(&psu_node, &format!("{}/current/8", prefix)).unwrap_or(0.0),
        power: find_sensor(&psu_node, &format!("{}/power/11", prefix)).unwrap_or(0.0),
    };
    let rail_3v3 = RailReading {
        voltage: find_sensor(&psu_node, &format!("{}/voltage/6", prefix))?,
        current: find_sensor(&psu_node, &format!("{}/current/9", prefix)).unwrap_or(0.0),
        power: find_sensor(&psu_node, &format!("{}/power/12", prefix)).unwrap_or(0.0),
    };

    // Prefer "Total watts" (page 0x96 output-only) over "Total Output" (sum of rails)
    let total_power = find_sensor(&psu_node, &format!("{}/power/13", prefix))
        .or_else(|| find_sensor(&psu_node, &format!("{}/power/14", prefix)))
        .unwrap_or(rail_12v.power + rail_5v.power + rail_3v3.power);

    debug!(
        total_power,
        input_voltage, "PSU data from LHM"
    );

    Some(PsuStatus {
        temp_vrm,
        temp_case,
        fan_rpm,
        input_voltage,
        rail_12v,
        rail_5v,
        rail_3v3,
        total_power,
    })
}

/// Extended CPU metrics pulled from the LHM JSON tree — load %, package
/// power (W), and max core clock (MHz). Only available when LHM is
/// running and exposing `/amdcpu/…` or `/intelcpu/…` nodes.
#[derive(Debug, Clone)]
pub struct CpuMetrics {
    /// Aggregate CPU load percentage (0–100).
    pub load_pct: f64,
    /// Package power draw in watts.
    pub power_watts: f64,
    /// Peak core clock in MHz (max across cores, since at idle many
    /// cores park at a low clock and reporting the min/average would
    /// hide real boost behaviour).
    pub clock_mhz: u32,
}

/// Read extended CPU metrics from LHM's HTTP tree.
///
/// Returns `None` when LHM is unreachable or has no CPU hardware node
/// (fallback path: the dashboard just hides the CPU extras block the
/// same way it does for GPU on a non-NVIDIA machine).
pub fn read_cpu_metrics_from_lhm() -> Option<CpuMetrics> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let body: String = agent.get(LHM_URL).call().ok()?.body_mut().read_to_string().ok()?;
    let root: LhmNode = serde_json::from_str(&body).ok()?;

    // Prefer AMD node (the owner's 9950X3D), fall back to Intel for
    // portability.
    let cpu_node = find_hw_node(&root, "/amdcpu/")
        .or_else(|| find_hw_node(&root, "/intelcpu/"))?;

    // Load %: "CPU Total" is the canonical aggregate; fall back to the
    // first Load sensor so we still show something on an LHM version
    // that renamed the node.
    let load_pct = find_by_type_and_text(cpu_node, "Load", "CPU Total")
        .or_else(|| find_first_by_type(cpu_node, "Load"))
        .unwrap_or(0.0);

    // Package power — the total CPU domain including IOD + uncore. On
    // AMD Zen the node is "Package"; keep "CPU Package" as a fallback
    // for other naming schemes.
    let power_watts = find_by_type_and_text(cpu_node, "Power", "Package")
        .or_else(|| find_by_type_and_text(cpu_node, "Power", "CPU Package"))
        .or_else(|| find_first_by_type(cpu_node, "Power"))
        .unwrap_or(0.0);

    // Max core clock: we want to see boost, not the idle-core minimum.
    let clock_mhz = find_max_core_clock(cpu_node).unwrap_or(0.0) as u32;

    Some(CpuMetrics { load_pct, power_watts, clock_mhz })
}

/// Find the first sensor with both Type and Text matching.
fn find_by_type_and_text(node: &LhmNode, typ: &str, text: &str) -> Option<f64> {
    if node.Type == typ && node.Text == text {
        return parse_lhm_value(&node.Value);
    }
    for child in &node.Children {
        if let Some(v) = find_by_type_and_text(child, typ, text) {
            return Some(v);
        }
    }
    None
}

/// First sensor of a given Type anywhere in the subtree.
fn find_first_by_type(node: &LhmNode, typ: &str) -> Option<f64> {
    if node.Type == typ {
        return parse_lhm_value(&node.Value);
    }
    for child in &node.Children {
        if let Some(v) = find_first_by_type(child, typ) {
            return Some(v);
        }
    }
    None
}

/// Walk a CPU subtree and return the highest "CPU Core #N" clock.
fn find_max_core_clock(node: &LhmNode) -> Option<f64> {
    let mut max: Option<f64> = None;
    walk_cpu_core_clocks(node, &mut max);
    max
}

fn walk_cpu_core_clocks(node: &LhmNode, max: &mut Option<f64>) {
    if node.Type == "Clock" && node.Text.starts_with("CPU Core") {
        if let Some(v) = parse_lhm_value(&node.Value) {
            *max = Some(max.map_or(v, |m: f64| m.max(v)));
        }
    }
    for child in &node.Children {
        walk_cpu_core_clocks(child, max);
    }
}

/// Find a hardware node whose HardwareId starts with the given prefix.
fn find_hw_node<'a>(node: &'a LhmNode, prefix: &str) -> Option<&'a LhmNode> {
    if node.HardwareId.starts_with(prefix) {
        return Some(node);
    }
    for child in &node.Children {
        if let Some(found) = find_hw_node(child, prefix) {
            return Some(found);
        }
    }
    None
}

/// Find a sensor value by exact SensorId or prefix match within a subtree.
fn find_sensor(node: &LhmNode, sensor_id: &str) -> Option<f64> {
    if !node.SensorId.is_empty() && node.SensorId.starts_with(sensor_id) {
        return parse_lhm_value(&node.Value);
    }
    for child in &node.Children {
        if let Some(v) = find_sensor(child, sensor_id) {
            return Some(v);
        }
    }
    None
}

/// Parse LHM value strings like "12.072 V", "50.8 °C", "0 RPM" → f64
fn parse_lhm_value(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.replace(',', ".").parse().ok()
}
