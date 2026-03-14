use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use corsair_fancontrol::control_loop::{self, ControlLoop};
use corsair_hid::DeviceScanner;

fn main() -> Result<()> {
    // 1. Parse --config <path> from args (default: "config.toml")
    let config_path = std::env::args()
        .skip_while(|a| a != "--config")
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    // 2. Load config
    let config = control_loop::load_config(Path::new(&config_path))?;

    // 3. Set up tracing from config.general.log_level or RUST_LOG
    let default_filter = format!("corsair={}", config.general.log_level);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&default_filter)),
        )
        .init();

    // 4. Print banner
    println!("ai-corsair-hub v0.1.0 — Fan Control Service");
    println!(
        "  Config: {} ({} fan groups)",
        config_path,
        config.fan_groups.len()
    );
    println!("  Press Ctrl+C to stop");
    println!();

    // 5. Set up shutdown signal
    let shutdown = Arc::new(AtomicBool::new(false));
    let s = shutdown.clone();
    ctrlc::set_handler(move || {
        println!("\n  Shutting down...");
        s.store(true, Ordering::Relaxed);
    })?;

    // 6. Build and run control loop
    let scanner = DeviceScanner::new()?;
    let mut control = ControlLoop::build(config, shutdown, &scanner)?;
    control.run()?;

    println!("  Shutdown complete. Hardware mode restored.");
    Ok(())
}
