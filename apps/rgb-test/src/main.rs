//! RGB protocol validation + LED-count diagnostic tool.
//!
//! Two modes:
//!
//! **Cycle** (default, no args): cycles all fans through red → green → blue.
//!     cargo run --bin corsair-rgb-test
//!
//! **Walk** (LED-count diagnostic): walks a single lit LED across positions 0..max
//! on one specific channel, leaves all other devices black. Lets you visually
//! count how many physical LEDs the channel actually has.
//!     cargo run --bin corsair-rgb-test -- walk <hub_suffix> <channel> [max]
//!
//! `hub_suffix` is the last 4 hex chars of the hub serial (e.g. "7AFC", "E7EC"
//! matching the UI labels). `channel` is the iCUE LINK channel (1, 2, 3, 13, ...).
//! `max` defaults to 80 — upper bound on LEDs to probe. Each LED stays lit for
//! 500 ms; the walk takes `max * 0.5` seconds total.
//!
//! IMPORTANT: the main corsair-hub-gui app holds an exclusive HID handle to the
//! hub. Exit the app (tray → Quit) before running this tool.

use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};

use corsair_common::CorsairDevice;
use corsair_hid::{DeviceScanner, IcueLinkHub};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,corsair_hid=debug".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("walk") => run_walk(&args),
        Some("cycle") | None => run_cycle(),
        Some(other) => {
            eprintln!("Unknown mode: {}", other);
            eprintln!("Usage:");
            eprintln!("  corsair-rgb-test                     # cycle all LEDs R/G/B");
            eprintln!("  corsair-rgb-test walk <HUB> <CH> [MAX]  # walk a single LED");
            std::process::exit(1);
        }
    }
}

fn open_hub(hub_suffix_match: Option<&str>) -> Result<(IcueLinkHub, corsair_hid::icue_link::HubInfo)>
{
    let scanner = DeviceScanner::new().context("Failed to initialize HID API")?;
    let groups = scanner.scan_grouped();

    let mut hub_groups: Vec<_> = groups
        .iter()
        .filter(|g| g.device_type == CorsairDevice::IcueLinkHub)
        .collect();

    if hub_groups.is_empty() {
        bail!("No iCUE LINK System Hubs found on USB bus");
    }

    // If a hub suffix is provided, filter to matching hub.
    if let Some(suffix) = hub_suffix_match {
        let suffix_upper = suffix.to_uppercase();
        hub_groups.retain(|g| g.serial.to_uppercase().ends_with(&suffix_upper));
        if hub_groups.is_empty() {
            bail!(
                "No hub matches suffix '{}'. Expected last 4 chars of hub serial.",
                suffix
            );
        }
    }

    let group = hub_groups[0];
    info!(
        "Opening hub: serial={} (suffix {})",
        group.serial,
        &group.serial[group.serial.len().saturating_sub(4)..]
    );

    let device = scanner
        .open_device(group.pid, &group.serial, IcueLinkHub::data_interface())
        .context("Failed to open hub — another process may have it open (exit corsair-hub-gui first)")?;

    let hub = IcueLinkHub::new(device, group.serial.clone());
    let hub_info = hub.initialize().context("Failed to initialize hub")?;
    Ok((hub, hub_info))
}

fn run_cycle() -> Result<()> {
    info!("iCUE LINK RGB Protocol Test — Cycle Mode");
    info!("=========================================");

    let (hub, hub_info) = open_hub(None)?;
    info!("Firmware: {}", hub_info.firmware);
    info!("Devices: {}", hub_info.devices.len());

    let mut total_leds: u16 = 0;
    let mut rgb_channels: Vec<(u8, u16)> = Vec::new();
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

    let colors: &[(&str, [u8; 3])] = &[
        ("RED", [255, 0, 0]),
        ("GREEN", [0, 255, 0]),
        ("BLUE", [0, 0, 255]),
    ];

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
            Ok(()) => info!("  {} sent", name),
            Err(e) => error!("  {} failed: {}", name, e),
        }
        thread::sleep(Duration::from_secs(3));
    }

    hub.close_color_endpoint()
        .context("Failed to close color endpoint")?;
    hub.enter_hardware_mode()
        .context("Failed to restore hardware mode")?;
    info!("Cycle complete");
    Ok(())
}

/// Walk a single lit LED across positions 0..max on the target channel.
/// All other channels on the hub get dark frames (so whatever else is on the
/// chain goes black for the duration).
///
/// Colors change every 10 positions to help the observer count:
///   positions  0..9  = RED
///   positions 10..19 = GREEN
///   positions 20..29 = BLUE
///   positions 30..39 = YELLOW
///   positions 40..49 = MAGENTA
///   positions 50..59 = CYAN
///   positions 60..69 = WHITE
///   positions 70..79 = ORANGE
fn run_walk(args: &[String]) -> Result<()> {
    let hub_suffix = args
        .get(2)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing hub suffix. Usage: walk <HUB> <CH> [MAX]"))?;
    let target_channel: u8 = args
        .get(3)
        .ok_or_else(|| anyhow::anyhow!("missing channel. Usage: walk <HUB> <CH> [MAX]"))?
        .parse()
        .context("channel must be a number 1..255")?;
    let max_leds: u16 = args
        .get(4)
        .map(|s| s.parse().context("max must be a number"))
        .transpose()?
        .unwrap_or(80);

    info!("iCUE LINK RGB Walk — LED Count Diagnostic");
    info!("==========================================");
    info!("Target: hub ending in '{}', channel {}", hub_suffix, target_channel);
    info!("Probing up to {} LED positions", max_leds);
    info!(
        "Color legend: 0-9 RED, 10-19 GREEN, 20-29 BLUE, 30-39 YELLOW,"
    );
    info!("              40-49 MAGENTA, 50-59 CYAN, 60-69 WHITE, 70-79 ORANGE");
    info!("");

    let (hub, hub_info) = open_hub(Some(&hub_suffix))?;
    info!("Firmware: {}", hub_info.firmware);
    info!("Devices:");
    for dev in &hub_info.devices {
        info!(
            "  CH{}: {} leds={}",
            dev.channel,
            dev.device_type.name(),
            dev.device_type.led_count()
        );
    }

    // Verify the target channel exists on this hub.
    if !hub_info.devices.iter().any(|d| d.channel == target_channel) {
        warn!(
            "Channel {} not in hub enumeration. Sending anyway — may surface nothing.",
            target_channel
        );
    }

    hub.open_color_endpoint()
        .context("Failed to open color endpoint")?;

    // Build base frames for ALL channels: every other channel gets its
    // enumerated-count worth of BLACK so the chain stays in sync.
    let other_channels: Vec<(u8, u16)> = hub_info
        .devices
        .iter()
        .filter(|d| d.channel != target_channel)
        .map(|d| (d.channel, d.device_type.led_count()))
        .collect();

    info!("");
    info!("Starting walk — count the highest LIT position you observe on CH{}.", target_channel);
    info!("(Each LED stays lit for 500 ms.)");
    info!("");

    for position in 0..max_leds {
        let color = walk_color_for(position);
        let color_name = walk_color_name_for(position);

        // Build LEDs for the target channel: all black except the lit position.
        // We send max_leds worth of data for this channel so the hub has enough
        // slots to light position N even if its enumerated count says less.
        // (If the hub ignores the overflow, positions past the real count will
        // simply not light up — which is exactly what we want to observe.)
        let mut target_leds: Vec<[u8; 3]> = vec![[0, 0, 0]; max_leds as usize];
        target_leds[position as usize] = color;

        let mut channel_leds: Vec<(u8, Vec<[u8; 3]>)> = vec![(target_channel, target_leds)];
        for &(ch, count) in &other_channels {
            channel_leds.push((ch, vec![[0, 0, 0]; count as usize]));
        }

        let refs: Vec<(u8, &[[u8; 3]])> = channel_leds
            .iter()
            .map(|(ch, leds)| (*ch, leds.as_slice()))
            .collect();

        if let Err(e) = hub.set_rgb(&refs) {
            warn!("set_rgb failed at position {}: {}", position, e);
        }

        if position % 5 == 0 {
            info!("  LED {:02}  {}", position, color_name);
        }
        thread::sleep(Duration::from_millis(500));
    }

    info!("");
    info!("Walk complete. Restoring hardware mode.");
    hub.close_color_endpoint()
        .context("Failed to close color endpoint")?;
    hub.enter_hardware_mode()
        .context("Failed to restore hardware mode")?;
    info!("");
    info!("Now tell me: what was the HIGHEST position that actually lit up");
    info!("on channel {}? That's the real LED count minus 1.", target_channel);
    Ok(())
}

fn walk_color_for(position: u16) -> [u8; 3] {
    match position / 10 {
        0 => [255, 0, 0],    // RED
        1 => [0, 255, 0],    // GREEN
        2 => [0, 0, 255],    // BLUE
        3 => [255, 255, 0],  // YELLOW
        4 => [255, 0, 255],  // MAGENTA
        5 => [0, 255, 255],  // CYAN
        6 => [255, 255, 255], // WHITE
        _ => [255, 128, 0],  // ORANGE
    }
}

fn walk_color_name_for(position: u16) -> &'static str {
    match position / 10 {
        0 => "RED",
        1 => "GREEN",
        2 => "BLUE",
        3 => "YELLOW",
        4 => "MAGENTA",
        5 => "CYAN",
        6 => "WHITE",
        _ => "ORANGE",
    }
}
