use anyhow::Result;
use corsair_common::CorsairDevice;
use corsair_hid::discovery::DeviceScanner;
use corsair_hid::{CorsairPsu, IcueLinkHub};
use corsair_sensors::cpu::CpuSensor;
use corsair_sensors::gpu::GpuSensor;
use corsair_sensors::TemperatureSource;
use tracing::warn;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("corsair=debug".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    println!();
    println!("  ai-corsair-hub :: USB Device Scanner");
    println!("  ====================================");
    println!();

    // --- System Temperatures ---
    print_system_temps();

    // --- USB Device Scan ---
    let scanner = DeviceScanner::new()?;
    let groups = scanner.scan_grouped();

    if groups.is_empty() {
        warn!("No Corsair devices found!");
        println!("  No Corsair USB devices detected.");
        println!("  Make sure your devices are connected and powered on.");
        println!();
        return Ok(());
    }

    println!("  Found {} Corsair device(s):", groups.len());
    println!();

    for group in &groups {
        let icon = if group.device_type.supports_fan_control() {
            "[FAN]"
        } else {
            "[MON]"
        };

        println!(
            "  {} {} (VID: 0x{:04X}, PID: 0x{:04X})",
            icon,
            group.device_type.name(),
            group.vid,
            group.pid
        );
        println!("      Serial: {}", group.serial);

        // Probe iCUE LINK Hubs via the real protocol
        if group.device_type == CorsairDevice::IcueLinkHub {
            probe_icue_link_hub(&scanner, group);
        }

        // Probe HX1500i PSU
        if group.device_type == CorsairDevice::Hx1500i {
            probe_psu(&scanner, group);
        }

        println!();
    }

    println!("  ---");
    println!("  Tip: Run with RUST_LOG=trace for detailed protocol output.");
    println!("  Tip: Close iCUE before scanning to avoid device access conflicts.");
    println!();

    Ok(())
}

fn print_system_temps() {
    println!("  System Temperatures:");

    // CPU temperature
    match CpuSensor::new() {
        Ok(cpu) => match cpu.read() {
            Ok(temp) => println!("      CPU Tctl:  {:.1}\u{00B0}C", temp.celsius),
            Err(e) => println!("      CPU Tctl:  error \u{2014} {}", e),
        },
        Err(e) => println!("      CPU Tctl:  unavailable \u{2014} {}", e),
    }

    // GPU temperature + metrics
    match GpuSensor::new() {
        Ok(gpu) => match gpu.read_metrics() {
            Ok(m) => {
                println!(
                    "      GPU Core:  {:.1}\u{00B0}C  ({:.0}W, {} MHz, {}%)",
                    m.temp_celsius, m.power_watts, m.clock_mhz, m.utilization_pct
                );
            }
            Err(e) => println!("      GPU Core:  error \u{2014} {}", e),
        },
        Err(e) => println!("      GPU Core:  unavailable \u{2014} {}", e),
    }

    println!();
}

fn probe_icue_link_hub(scanner: &DeviceScanner, group: &corsair_hid::discovery::DeviceGroup) {
    // Open only MI_00 (data interface)
    let device = match scanner.open_device(
        group.pid,
        &group.serial,
        IcueLinkHub::data_interface(),
    ) {
        Ok(d) => d,
        Err(e) => {
            println!("      Cannot open data interface: {} (is iCUE running?)", e);
            return;
        }
    };

    let mut hub = IcueLinkHub::new(device, group.serial.clone());

    // Initialize: firmware + software mode + enumerate
    let info = match hub.initialize() {
        Ok(info) => info,
        Err(e) => {
            println!("      Initialization failed: {}", e);
            return;
        }
    };

    println!("      Firmware: {}", info.firmware);
    println!("      Connected devices ({}):", info.devices.len());
    for dev in &info.devices {
        let type_label = match &dev.device_type {
            corsair_hid::LinkDeviceType::Unknown(b) => format!("Unknown (0x{:02X})", b),
            other => other.name().to_string(),
        };
        println!(
            "        CH{}: {} \u{2014} ID: \"{}\"",
            dev.channel, type_label, dev.device_id
        );
    }

    // Read fan speeds
    match hub.get_speeds() {
        Ok(speeds) if !speeds.is_empty() => {
            println!("      Fan speeds:");
            for s in &speeds {
                println!("        CH{}: {} RPM", s.channel, s.rpm);
            }
        }
        Ok(_) => println!("      Fan speeds: (none reported)"),
        Err(e) => println!("      Fan speeds: error \u{2014} {}", e),
    }

    // Read temperatures
    match hub.get_temperatures() {
        Ok(temps) if !temps.is_empty() => {
            println!("      Temperatures:");
            for t in &temps {
                println!("        CH{}: {:.1}\u{00B0}C", t.channel, t.temp_celsius);
            }
        }
        Ok(_) => println!("      Temperatures: (none reported)"),
        Err(e) => println!("      Temperatures: error \u{2014} {}", e),
    }

    // Return to hardware mode
    if let Err(e) = hub.enter_hardware_mode() {
        println!("      Warning: failed to restore hardware mode: {}", e);
    }
}

fn probe_psu(scanner: &DeviceScanner, group: &corsair_hid::discovery::DeviceGroup) {
    let device = match scanner.open_device(
        group.pid,
        &group.serial,
        CorsairPsu::data_interface(),
    ) {
        Ok(d) => d,
        Err(e) => {
            println!("      Cannot open PSU: {} (is iCUE running?)", e);
            return;
        }
    };

    let psu = CorsairPsu::new(device, group.serial.clone());

    if let Err(e) = psu.initialize() {
        println!("      PSU init failed: {}", e);
        return;
    }

    match psu.read_all() {
        Ok(status) => {
            println!("      VRM Temp:    {:.1}\u{00B0}C", status.temp_vrm);
            println!("      Case Temp:   {:.1}\u{00B0}C", status.temp_case);
            let fan_label = if status.fan_rpm == 0 {
                "0 RPM (idle)".to_string()
            } else {
                format!("{} RPM", status.fan_rpm)
            };
            println!("      Fan:         {}", fan_label);
            println!("      AC Input:    {:.1}V", status.input_voltage);
            println!(
                "      12V Rail:    {:.2}V / {:.1}A / {:.1}W",
                status.rail_12v.voltage, status.rail_12v.current, status.rail_12v.power
            );
            println!(
                "       5V Rail:    {:.2}V / {:.1}A / {:.1}W",
                status.rail_5v.voltage, status.rail_5v.current, status.rail_5v.power
            );
            println!(
                "      3.3V Rail:   {:.2}V / {:.1}A / {:.1}W",
                status.rail_3v3.voltage, status.rail_3v3.current, status.rail_3v3.power
            );
            println!("      Total Power: {:.1}W", status.total_power);
        }
        Err(e) => {
            println!("      Failed to read PSU status: {}", e);
        }
    }
}
