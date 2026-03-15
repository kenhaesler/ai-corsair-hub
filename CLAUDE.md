# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Goal

**ai-corsair-hub** is a lightweight replacement for Corsair's iCUE software, focused on
smart fan control and thermal/acoustic optimization for a high-end custom water-cooled PC.

## Build & Test Commands

```bash
# Set PATH (required in new shell sessions)
export PATH="$USERPROFILE/.cargo/bin:/c/Program Files/nodejs:$PATH"

# Build entire workspace
cargo build

# Run tests
cargo test

# Run USB device scanner
cargo run --bin corsair-scanner

# Run scanner with debug logging
RUST_LOG=corsair_hid=debug cargo run --bin corsair-scanner

# Run scanner with trace-level protocol output
RUST_LOG=trace cargo run --bin corsair-scanner

# Run RGB protocol test (cycles red/green/blue on hardware)
RUST_LOG=corsair_hid=trace cargo run --bin corsair-rgb-test
```

## Project Structure

Rust workspace with 7 crates:

- `crates/common` — Shared types (CorsairDevice, config, Temperature, FanReading, RgbConfig)
- `crates/hid` — USB HID layer (device discovery, iCUE LINK data + color endpoint protocol)
- `crates/sensors` — Temperature sources (CPU via LHM, GPU via NVML, PSU via HID/LHM)
- `crates/fancontrol` — PID controller, fan curves, acoustic filter, control loop, RGB frame routing
- `crates/rgb` — RGB effect engine (effects, layers, blending, renderer, layouts)
- `apps/gui` — Tauri 2.0 + Svelte 5 desktop app (dashboard, fans, lighting, settings)
- `apps/scanner` — CLI tool: scans USB devices, probes iCUE LINK hubs
- `apps/rgb-test` — CLI tool: RGB protocol validation (cycles red/green/blue on hardware)
- `apps/service` — Windows Service daemon [stub]

## Architecture

See `docs/architecture.md` for full architecture, protocol details, and roadmap.

## Hardware Context

The target system has:
- 2x Corsair iCUE LINK System Hubs (VID 0x1B1C, PID 0x0C3F) — protocol fully implemented
- 1x Corsair HX1500i PSU (VID 0x1B1C, PID 0x1C1F) — protocol documented via liquidctl
- 10 fans (9x QX140 + 1x QX120) connected via iCUE LINK daisy chain (34 LEDs each)
- 2x iCUE LINK LS350 Aurora strips
- Full custom water loop with 3x 420mm radiators

## Key Technical Details

- Corsair VID: `0x1B1C`
- iCUE LINK Hub: PID `0x0C3F`, MI_00 = data interface, responses are 512 bytes
- Data endpoint (`0x0D 0x01`): fan/temp/device operations
- Color endpoint (`0x0D 0x00`): RGB LED control (separate from data EP)
- Protocol sourced from OpenLinkHub (github.com/jurkovic-nikola/OpenLinkHub)
- HX1500i PSU: PID `0x1C1F`, protocol is in liquidctl's `corsair_hid_psu` driver
- See `docs/rgb-protocol.md` for RGB byte-level protocol reference

## Current Phase

**Phases 0–5 complete.** Fan control, sensor integration, GUI, and RGB are all working.
Next: Phase 6 (Polish & Community) — installer, auto-update, documentation.

## Tech Stack

- **Backend**: Rust (2024 edition, tokio, hidapi, serde, tracing)
- **Frontend**: Tauri 2.0 + Svelte 5
- **Platform**: Windows 11 only
- **License**: Apache 2.0
