//! RGB protocol validation tool for iCUE LINK System Hubs.
//!
//! Cycles fans through solid red → green → blue, then restores hardware mode.
//!
//! Usage: RUST_LOG=corsair_hid=trace cargo run --bin corsair-rgb-test

use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{error, info};

use corsair_common::CorsairDevice;
use corsair_hid::{DeviceScanner, IcueLinkHub};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,corsair_hid=debug".into()),
        )
        .init();

    info!("iCUE LINK RGB Protocol Test");
    info!("===========================");

    let scanner = DeviceScanner::new().context("Failed to initialize HID API")?;
    let groups = scanner.scan_grouped();

    let hub_groups: Vec<_> = groups
        .iter()
        .filter(|g| g.device_type == CorsairDevice::IcueLinkHub)
        .collect();

    if hub_groups.is_empty() {
        bail!("No iCUE LINK System Hubs found on USB bus");
    }

    info!("Found {} hub(s)", hub_groups.len());

    // Use first hub for testing
    let group = hub_groups[0];
    info!("Testing hub: serial={}", group.serial);

    let device = scanner
        .open_device(group.pid, &group.serial, IcueLinkHub::data_interface())
        .context("Failed to open hub")?;

    let mut hub = IcueLinkHub::new(device, group.serial.clone());
    let hub_info = hub.initialize().context("Failed to initialize hub")?;

    info!("Firmware: {}", hub_info.firmware);
    info!("Devices: {}", hub_info.devices.len());

    // Count total LEDs and build channel list
    let mut total_leds: u16 = 0;
    let mut rgb_channels: Vec<(u8, u16)> = Vec::new(); // (channel, led_count)
    for dev in &hub_info.devices {
        let leds = dev.device_type.led_count();
        info!(
            "  CH{}: {} (type={:?}, leds={})",
            dev.channel,
            dev.device_type.name(),
            dev.device_type,
            leds
        );
        if leds > 0 {
            rgb_channels.push((dev.channel, leds));
            total_leds += leds;
        }
    }

    if rgb_channels.is_empty() {
        info!("No RGB-capable devices found on this hub");
        hub.enter_hardware_mode()?;
        return Ok(());
    }

    info!(
        "Total RGB LEDs: {} across {} device(s)",
        total_leds,
        rgb_channels.len()
    );

    // Test sequence: solid colors
    let colors: &[(&str, [u8; 3])] = &[
        ("RED", [255, 0, 0]),
        ("GREEN", [0, 255, 0]),
        ("BLUE", [0, 0, 255]),
    ];

    info!("Opening color endpoint...");
    hub.open_color_endpoint()
        .context("Failed to open color endpoint")?;

    for (name, color) in colors {
        info!("Setting all LEDs to {}...", name);

        let channel_leds: Vec<(u8, Vec<[u8; 3]>)> = rgb_channels
            .iter()
            .map(|&(ch, count)| (ch, vec![*color; count as usize]))
            .collect();

        let refs: Vec<(u8, &[[u8; 3]])> = channel_leds
            .iter()
            .map(|(ch, leds)| (*ch, leds.as_slice()))
            .collect();

        match hub.set_rgb(&refs) {
            Ok(()) => info!("  {} sent successfully", name),
            Err(e) => error!("  {} failed: {}", name, e),
        }

        thread::sleep(Duration::from_secs(3));
    }

    info!("Closing color endpoint and restoring hardware mode...");
    hub.close_color_endpoint()
        .context("Failed to close color endpoint")?;
    hub.enter_hardware_mode()
        .context("Failed to restore hardware mode")?;

    info!("Test complete — LEDs should revert to firmware default");
    Ok(())
}
