# ai-corsair-hub

A lightweight, open-source replacement for Corsair's iCUE software — focused on smart fan control and thermal/acoustic optimization for custom water-cooled PCs.

## Why?

iCUE is bloated (~500 MB install, ~200 MB RAM), buggy, and runs telemetry you don't need. If all you want is **intelligent fan control** for your Corsair hardware, you shouldn't need half a gigabyte of software to do it.

**ai-corsair-hub** aims to deliver:
- Smart fan control with PID algorithms and acoustic optimization
- Near-zero system impact (~5 MB RAM, <0.1% CPU)
- Modern configuration UI (Tauri + Svelte — opens when you need it, gone when you don't)
- First open-source implementation of the **Corsair iCUE LINK** protocol

## Status

**Phase 0 (Foundation) — Complete**

- Rust workspace with 6 crates
- USB HID device scanner detects all Corsair hardware
- iCUE LINK System Hub (PID `0x0C3F`) identified and probed — first open-source documentation
- PID controller with anti-windup implemented and tested
- Full architecture and 6-phase roadmap documented

**Next: Phase 1 — Protocol Reverse Engineering** (capturing iCUE USB traffic to decode the iCUE LINK handshake)

## Supported Hardware

Currently detected and partially supported:

| Device | PID | Status |
|--------|-----|--------|
| iCUE LINK System Hub | `0x0C3F` | Detected, probed, protocol RE in progress |
| iCUE LINK QX140/QX120 fans | via Hub | Connected through hub daisy chain |
| Corsair HX1500i PSU | `0x1C1F` | Detected, protocol documented (via liquidctl) |

Planned support for other Corsair devices:
- Commander Pro (`0x0C10`)
- Commander Core / Core XT (`0x0C1C` / `0x0C2A`)
- Hydro series AIOs

## Quick Start

### Prerequisites

- Windows 11
- [Rust](https://rustup.rs/) (1.94+)
- MSVC Build Tools 2022 (`winget install Microsoft.VisualStudio.2022.BuildTools`)

### Build & Run

```bash
# Clone
git clone https://github.com/kenhaesler/ai-corsair-hub.git
cd ai-corsair-hub

# Build
cargo build

# Run tests
cargo test

# Scan your Corsair USB devices
cargo run --bin corsair-scanner

# With debug logging
RUST_LOG=corsair_hid=debug cargo run --bin corsair-scanner

# With full protocol trace
RUST_LOG=trace cargo run --bin corsair-scanner
```

### Example Output

```
  ai-corsair-hub :: USB Device Scanner
  ====================================

  Found 3 Corsair device(s):

  [FAN] iCUE LINK System Hub (VID: 0x1B1C, PID: 0x0C3F)
      Serial: 22DE335F6BBA065AA0653243E4BD7AFC
      Interfaces: MI_00, MI_01
      Manufacturer: Corsair
      Product: iCUE LINK System Hub

  [FAN] iCUE LINK System Hub (VID: 0x1B1C, PID: 0x0C3F)
      Serial: 8B44BF040D45AA58B07DC6BC9E70E7EC
      Interfaces: MI_00, MI_01
      Manufacturer: Corsair
      Product: iCUE LINK System Hub

  [MON] HX1500i PSU (VID: 0x1B1C, PID: 0x1C1F)
      Serial:
      Interfaces: MI_00
```

## Project Structure

```
ai-corsair-hub/
├── crates/
│   ├── common/          Shared types, config, device definitions
│   ├── hid/             USB HID discovery + iCUE LINK protocol
│   ├── sensors/         Temperature sources (CPU, GPU, water)
│   └── fancontrol/      PID controller, fan curves, acoustic filter
├── apps/
│   ├── scanner/         CLI: detect and probe Corsair devices
│   └── service/         Windows Service for background fan control
├── docs/
│   └── architecture.md  Full architecture and roadmap
└── scripts/
    └── scan_usb.ps1     PowerShell USB device enumerator
```

## Architecture

See [`docs/architecture.md`](docs/architecture.md) for the full architecture document including:
- Hardware profile and USB device map
- Protocol research and byte-level findings
- Fan control algorithm design (PID + multi-sensor + acoustic optimization)
- Tech stack rationale
- 6-phase implementation roadmap

## Fan Control Design

The fan control algorithm targets **Level 4 (Enthusiast)**:

```
Sensor Fusion → Weighted Temperature → PID Controller → Acoustic Filter → Fan Output
```

Key features:
- **Multi-sensor weighted averaging** — CPU, GPU, water temp with configurable weights
- **PID controller** — targets a temperature setpoint, dynamically adjusts fan speed
- **Acoustic optimization** — asymmetric ramp rates (slow up, slower down), hysteresis band, zero-RPM mode
- **Per-group control** — different fan groups can have different curves/targets
- **Emergency override** — instant 100% if any sensor hits critical temperature

## Protocol Research

### iCUE LINK System Hub (PID 0x0C3F) — Novel Discovery

This project is the **first open-source effort** to document the Corsair iCUE LINK protocol. Key findings so far:

- Two HID interfaces per hub: MI_00 (bidirectional data), MI_01 (read-only)
- All responses are exactly **512 bytes**
- The hub echoes command bytes and returns a `0x0F` status (likely "not initialized")
- Protocol is related to Commander Core but requires a different initialization handshake
- Full RE via USB packet capture is the next step

### Reference Protocols

The project builds on protocol knowledge from:
- [liquidctl](https://github.com/liquidctl/liquidctl) — Commander Core/XT, Commander Pro, HX1500i PSU
- [OpenCorsairLink](https://github.com/audiohacked/OpenCorsairLink) — Commander Pro, H-series AIOs

## Roadmap

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Foundation: Rust workspace, USB scanner, protocol probe | Done |
| 1 | Protocol RE: USBPcap capture, decode iCUE LINK handshake | Next |
| 2 | Sensors: CPU/GPU/PSU temperature integration | Planned |
| 3 | Fan control service: PID + acoustic filter as Windows Service | Planned |
| 4 | UI: Tauri 2.0 + Svelte 5 dashboard with fan curve editor | Planned |
| 5 | RGB: Static colors and basic effects for LINK devices | Future |
| 6 | Polish: Installer, auto-update, community documentation | Future |

## Tech Stack

| Component | Technology | Why |
|-----------|------------|-----|
| Backend | Rust | Near-zero overhead, memory safety, excellent USB HID support |
| USB | hidapi | Cross-platform HID library, works out of the box on Windows |
| UI (planned) | Tauri 2.0 + Svelte 5 | Uses system WebView2 (~3 MB app vs Electron's ~150 MB) |
| Config | TOML | Human-readable, great Rust ecosystem |
| Fan control | Custom PID | Full control over tuning, acoustic optimization |

## Contributing

This is an early-stage project. If you have Corsair iCUE LINK hardware and want to help with protocol reverse engineering, contributions are very welcome.

Especially useful:
- USB packet captures (USBPcap/Wireshark) of iCUE communicating with LINK hubs
- Testing on different Corsair LINK devices
- Protocol analysis and documentation

## License

[Apache 2.0](LICENSE)
