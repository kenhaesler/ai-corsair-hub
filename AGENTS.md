# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Goal

**ai-corsair-hub** is a lightweight replacement for Corsair's iCUE software, focused on
smart fan control and thermal/acoustic optimization for a high-end custom water-cooled PC.

## Build & Test Commands

```bash
# Set PATH (required in new shell sessions)
export PATH="$USERPROFILE/.cargo/bin:/c/Program Files/nodejs:$PATH"

# Toolchains are pinned by rust-toolchain.toml and .nvmrc
rustup show active-toolchain
node --version

# CI-equivalent backend checks
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets

# Frontend checks
npm --prefix apps/gui ci
npm --prefix apps/gui/ui ci
npm --prefix apps/gui/ui run check -- --fail-on-warnings
npm --prefix apps/gui/ui test
npm --prefix apps/gui/ui run build

# Verify release metadata
powershell -File scripts/check-version.ps1

# Run USB device scanner
cargo run --bin corsair-scanner

# Run scanner with debug logging
RUST_LOG=corsair_hid=debug cargo run --bin corsair-scanner

# Run scanner with trace-level protocol output
RUST_LOG=trace cargo run --bin corsair-scanner

# Run RGB protocol test on connected hardware
RUST_LOG=corsair_hid=trace cargo run --bin corsair-rgb-test
```

## Project Structure

Rust workspace with five library crates and four application crates:

- `crates/common` — Shared types and configuration
- `crates/hid` — USB HID discovery and iCUE LINK/PSU protocols
- `crates/sensors` — CPU via LHM, GPU via NVML, and PSU telemetry
- `crates/fancontrol` — PID, curves, acoustic filtering, and the control loop
- `crates/rgb` — Effects, compositing, layouts, and rendering
- `apps/gui` — Tauri 2.0 + Svelte 5 desktop application
- `apps/scanner` — USB device scanner and protocol probe
- `apps/rgb-test` — Hardware RGB protocol validation tool
- `apps/service` — Windows SCM fan-control service with console mode

## Architecture

See `docs/architecture.md` for full architecture, protocol details, and roadmap.

## Hardware Context

The target system has:
- 2x Corsair iCUE LINK System Hubs (VID 0x1B1C, PID 0x0C3F) — data and color endpoints implemented
- 1x Corsair HX1500i PSU (VID 0x1B1C, PID 0x1C1F) — protocol documented via liquidctl
- 10 fans (9x QX140 + 1x QX120) connected via iCUE LINK daisy chain
- 2x iCUE LINK LS350 Aurora strips
- Full custom water loop with 3x 420mm radiators

## Key Technical Details

- Corsair VID: `0x1B1C`
- iCUE LINK Hub: PID `0x0C3F`, MI_00 = data interface, responses are 512 bytes
- Data endpoint `0x0D 0x01` handles fan, temperature, and device operations
- Color endpoint `0x0D 0x00` handles RGB output
- The implementation was ported from OpenLinkHub and is documented in `docs/rgb-protocol.md`
- HX1500i PSU: PID `0x1C1F`, protocol is in liquidctl's `corsair_hid_psu` driver

## Current Phase

**Phases 0–5 are implemented. Phase 6 (Polish & Community) is active.** NSIS
packaging, signed updater artifacts, updater integration, and Windows Service SCM
lifecycle are implemented. The GUI and service are alternative direct hardware owners;
a named mutex prevents concurrent access. GUI/service IPC and automatic SCM failure
recovery are still pending.

## Tech Stack

- **Backend**: Rust (2024 edition, tokio, hidapi, serde, tracing)
- **Frontend**: Tauri 2.0 + Svelte 5
- **Platform**: Windows 11 only
- **License**: Apache 2.0
