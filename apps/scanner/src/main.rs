use anyhow::Result;
use corsair_hid::discovery::DeviceScanner;
use corsair_hid::icue_link::IcueLinkHub;
use corsair_common::CorsairDevice;
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
        println!(
            "      Interfaces: {}",
            group
                .interfaces
                .iter()
                .map(|i| format!("MI_{:02}", i.number))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Probe iCUE LINK Hubs
        if group.device_type == CorsairDevice::IcueLinkHub {
            println!("      Status: Attempting probe...");

            // Try each interface
            for iface in &group.interfaces {
                match scanner.open_device(group.pid, &group.serial, iface.number) {
                    Ok(device) => {
                        let hub = IcueLinkHub::new(device, group.serial.clone());
                        match hub.probe() {
                            Ok(result) => {
                                println!("      Manufacturer: {}", result.manufacturer);
                                println!("      Product: {}", result.product);
                                println!(
                                    "      Probe responses (MI_{:02}): {}",
                                    iface.number,
                                    result.responses.len()
                                );
                                for resp in &result.responses {
                                    let hex: String = resp
                                        .response
                                        .iter()
                                        .take(32)
                                        .map(|b| format!("{:02X}", b))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    let suffix = if resp.response.len() > 32 {
                                        "..."
                                    } else {
                                        ""
                                    };
                                    println!(
                                        "        [{}] size={} -> ({} bytes) {}{}",
                                        resp.probe_name,
                                        resp.report_size,
                                        resp.response.len(),
                                        hex,
                                        suffix
                                    );
                                }
                            }
                            Err(e) => {
                                println!(
                                    "      Probe failed (MI_{:02}): {}",
                                    iface.number, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        println!(
                            "      Cannot open MI_{:02}: {} (is iCUE running?)",
                            iface.number, e
                        );
                    }
                }
            }
        }

        println!();
    }

    println!("  ---");
    println!("  Tip: Run with RUST_LOG=trace for detailed protocol output.");
    println!("  Tip: Close iCUE before scanning to avoid device access conflicts.");
    println!();

    Ok(())
}
