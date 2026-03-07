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
```

## Project Structure

Rust workspace with 6 crates:

- `crates/common` — Shared types (CorsairDevice, config, Temperature, FanReading)
- `crates/hid` — USB HID layer (device discovery, iCUE LINK protocol)
- `crates/sensors` — Temperature sources (CPU via WMI, GPU via NVML) [stubs]
- `crates/fancontrol` — PID controller with anti-windup, fan curve logic
- `apps/scanner` — CLI tool: scans USB devices, probes iCUE LINK hubs
- `apps/service` — Windows Service daemon [stub]

## Architecture

See `docs/architecture.md` for full architecture, protocol details, and roadmap.

## Hardware Context

The target system has:
- 2x Corsair iCUE LINK System Hubs (VID 0x1B1C, PID 0x0C3F) — protocol partially discovered
- 1x Corsair HX1500i PSU (VID 0x1B1C, PID 0x1C1F) — protocol documented via liquidctl
- 10 fans (9x QX140 + 1x QX120) connected via iCUE LINK daisy chain
- Full custom water loop with 3x 420mm radiators

## Key Technical Details

- Corsair VID: `0x1B1C`
- iCUE LINK Hub: PID `0x0C3F`, MI_00 = data interface, responses are 512 bytes
- The iCUE LINK protocol is NOT documented anywhere — we are reverse-engineering it
- HX1500i PSU: PID `0x1C1F`, protocol is in liquidctl's `corsair_hid_psu` driver

## Current Phase

**Phase 0 (Foundation)** is complete. Next: Phase 1 (Protocol Reverse Engineering)
using USBPcap + Wireshark to capture iCUE's initialization sequence.

## Tech Stack

- **Backend**: Rust (2024 edition, tokio, hidapi, serde, tracing)
- **Frontend** (planned): Tauri 2.0 + Svelte 5
- **Platform**: Windows 11 only
- **License**: Apache 2.0
