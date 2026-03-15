# ai-corsair-hub Architecture

## Vision

A lightweight, high-performance replacement for Corsair's iCUE software focused on
smart fan control and thermal/acoustic optimization. Zero bloat, zero telemetry,
full control.

## Target Hardware

| Component | Model | Interface |
|-----------|-------|-----------|
| CPU | AMD Ryzen 9 9950X3D (16-core Zen5) | WMI / LibreHardwareMonitor |
| GPU | NVIDIA GeForce RTX 5090 32GB | NVML |
| PSU | Corsair HX1500i 2025 | USB HID (PID 0x1C1F) |
| Fans | 9x iCUE LINK QX140 + 1x QX120 | iCUE LINK Hub (PID 0x0C3F) |
| Pump | EK-Loop D5 G3 PWM | Motherboard PWM header |
| Radiators | 3x EK 420mm (2x P420M + 1x S420) | Passive |
| Motherboard | MSI MEG X870E Godlike | SuperIO / WMI |
| RGB | iCUE LINK LS350 strips, Dominator Titanium | iCUE LINK Hub (color endpoint) |

## USB Device Map

Two iCUE LINK System Hubs detected:

| Device | VID | PID | Serial | Interfaces |
|--------|-----|-----|--------|------------|
| iCUE LINK Hub 1 | 0x1B1C | 0x0C3F | 22DE335F... | MI_00 (data), MI_01 (read-only) |
| iCUE LINK Hub 2 | 0x1B1C | 0x0C3F | 8B44BF04... | MI_00 (data), MI_01 (read-only) |
| HX1500i PSU | 0x1B1C | 0x1C1F | (empty) | MI_00 |

## Tech Stack

### Backend (Rust)
- **Language**: Rust 2024 edition
- **Async**: Tokio
- **USB**: hidapi crate (via system hid.dll)
- **Config**: TOML (serde)
- **Logging**: tracing + tracing-subscriber
- **Windows**: windows crate for service integration

### Frontend (Tauri 2.0 + Svelte 5)
- **Shell**: Tauri 2.0 (uses system WebView2, no bundled Chromium)
- **UI**: Svelte 5 with runes (compiled reactivity, near-zero runtime)
- **Graphs**: uPlot or D3.js for real-time temperature/fan curves
- **System tray**: Tauri tray plugin for background monitoring

### Resource Budget
| Component | RAM Target | CPU Target |
|-----------|-----------|------------|
| Fan control service | < 5 MB | < 0.1% |
| System tray (idle) | < 5 MB | 0% |
| UI (when open) | < 20 MB | < 1% |
| **Total** | **< 30 MB** | **< 1.1%** |
| iCUE (comparison) | ~200-500 MB | ~1-3% |

## Project Structure

```
ai-corsair-hub/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── common/                # Shared types, config, device definitions
│   │   ├── types.rs           # CorsairDevice, DeviceInfo, Temperature, FanReading
│   │   └── config.rs          # AppConfig, FanGroupConfig, FanMode, RgbConfig
│   ├── hid/                   # USB HID communication layer
│   │   ├── discovery.rs       # DeviceScanner: enumerate & group Corsair devices
│   │   ├── icue_link.rs       # IcueLinkHub: data + color endpoint protocol
│   │   └── corsair_psu.rs     # HX1500i PSU protocol (via liquidctl docs)
│   ├── sensors/               # Temperature source abstractions
│   │   ├── cpu.rs             # CpuSensor (WMI/LibreHardwareMonitor)
│   │   ├── gpu.rs             # GpuSensor (NVML)
│   │   └── lhm.rs             # LibreHardwareMonitor HTTP API bridge
│   ├── fancontrol/            # Fan control algorithms + control loop
│   │   ├── pid.rs             # PID controller with anti-windup
│   │   ├── curve.rs           # Fan curve interpolation
│   │   ├── acoustic.rs        # Acoustic filter (ramp rates, hysteresis)
│   │   └── control_loop.rs    # Main control loop + RGB frame routing
│   └── rgb/                   # RGB effect engine
│       ├── color.rs           # Rgb, Hsv, BlendMode
│       ├── effect.rs          # EffectConfig, EffectContext, RgbEffect trait
│       ├── effects/           # Built-in effects (static, rainbow, breathing, etc.)
│       ├── renderer.rs        # Zone-based multi-layer renderer
│       ├── layout.rs          # LedLayout (FanRing, LinearStrip)
│       └── frame.rs           # RgbFrame output type
├── apps/
│   ├── gui/                   # Tauri 2.0 + Svelte 5 desktop app
│   ├── scanner/               # CLI: USB device scanner + protocol probe
│   ├── rgb-test/              # CLI: RGB protocol validation tool
│   └── service/               # Windows Service daemon [stub]
├── docs/
│   ├── architecture.md        # This file
│   └── rgb-protocol.md        # iCUE LINK RGB protocol reference
└── scripts/
    └── scan_usb.ps1           # PowerShell USB device enumerator
```

## Protocol Status

### iCUE LINK System Hub (PID 0x0C3F) — IMPLEMENTED

Protocol reverse-engineered using [OpenLinkHub](https://github.com/jurkovic-nikola/OpenLinkHub) as reference.

**Data endpoint** (`0x0D 0x01`): fan/temp/device operations
- Software/hardware mode switching
- Device enumeration (daisy chain)
- Fan/pump RPM reading + speed control
- Temperature reading from connected devices

**Color endpoint** (`0x0D 0x00`): RGB LED control
- Separate from data endpoint, opened with mode `0x22`
- Hub-global writes: one flat RGB buffer for ALL devices
- 508-byte chunking with continuation commands
- Port power protection for high LED counts

See [`docs/rgb-protocol.md`](rgb-protocol.md) for detailed byte-level protocol reference.

### HX1500i PSU (PID 0x1C1F) — DOCUMENTED

Protocol is fully documented in liquidctl's `corsair_hid_psu` driver. We can read:
- Input/output power, efficiency
- Voltages: 12V, 5V, 3.3V rails
- Temperatures (internal, VRM)
- Fan RPM and mode

### Reference Protocols (for context)

**Commander Pro (PID 0x0C10):** 64-byte reports
- `[0x04, 0x02, fanId, 0x01, duty]` → set fan PWM
- `[0x04, 0x01, tempId]` → read temperature

**Commander Core (PID 0x0C1C):** 96-byte reports
- `[0x08, cmd, channel, data...]` → command format
- Uses open/close endpoint model with data types

## Fan Control Algorithm

### Level 4: Multi-sensor Weighted PID with Acoustic Optimization

```
Sensor Fusion:
  CPU Tctl, CCD0, CCD1 → weighted avg → T_cpu
  GPU Core, Hotspot    → weighted avg → T_gpu
  Water In/Out (later) → weighted avg → T_water

  T_effective = w1*T_cpu + w2*T_gpu + w3*T_water

PID Controller:
  target: T_target (user-configurable)
  output: duty cycle (0-100%)
  features: anti-windup, derivative filtering, output clamping

Acoustic Filter:
  - Asymmetric ramp rates (5%/sec up, 2%/sec down)
  - Hysteresis band (3°C default)
  - Zero-RPM mode below threshold
  - Emergency override above critical temp
  - Minimum duty floor (prevent fan stall)

Fan Groups (per-group PID):
  - Top exhaust (3x QX140)
  - Side intake (3x QX140)
  - Bottom intake (3x QX140)
  - Rear exhaust (1x QX120)
  - Pump (D5 G3 via motherboard)
```

## Implementation Phases

### Phase 0: Foundation (COMPLETE)
- [x] Rust workspace with 6 crates
- [x] USB HID device discovery (hidapi)
- [x] iCUE LINK Hub detection and identification
- [x] Initial protocol probe (512-byte responses, 0x0F status)
- [x] PID controller with tests
- [x] Config types (TOML-based)
- [x] HX1500i PSU detection

### Phase 1: Protocol Reverse Engineering (COMPLETE)
- [x] Decoded iCUE LINK Hub initialization handshake
- [x] Implemented open/init sequence in Rust
- [x] Read fan RPM, set fan duty cycle
- [x] Read connected device count (device enumeration)
- [x] Read temperature sensors from connected devices

### Phase 2: Sensor Integration (COMPLETE)
- [x] CPU temperature via LibreHardwareMonitor HTTP API
- [x] GPU temperature via NVML
- [x] HX1500i PSU monitoring (power, temp, fan) via HID + LHM
- [ ] Motherboard sensors (VRM, chipset) via WMI
- [ ] Water temperature (when user adds inline sensor)

### Phase 3: Fan Control Service (COMPLETE)
- [x] Wire PID controller to real sensor inputs
- [x] Wire fan output to iCUE LINK Hub set-duty commands
- [x] Implement fan groups with per-group PID
- [x] Implement acoustic filter (ramp rates, hysteresis)
- [x] TOML config file loading + live reload
- [x] Hub health monitoring and automatic recovery
- [ ] Windows Service registration and lifecycle
- [ ] Auto-start on boot

### Phase 4: Tauri UI (COMPLETE)
- [x] Tauri 2.0 + Svelte 5 desktop app
- [x] Dashboard: real-time temp/fan/power monitoring
- [x] Fan curve editor (draggable SVG points)
- [x] Fan group configuration
- [x] Device info panel (serial, firmware, connected fans)
- [x] Preset system (Silent/Balanced/Performance)
- [x] Settings panel
- [x] Dark theme

### Phase 5: RGB Control (COMPLETE)
- [x] Phase 5A: RGB effect engine (effects, layers, blending)
- [x] Phase 5B: Backend integration (renderer, zone configs, IPC)
- [x] Phase 5C: Lighting UI tab (preview, zone config, presets)
- [x] Phase 5D: Hardware protocol (color endpoint, chunking, power protection)
    - Protocol sourced from OpenLinkHub (github.com/jurkovic-nikola/OpenLinkHub)
    - See `docs/rgb-protocol.md` for byte-level reference
    - RGB test binary: `cargo run --bin corsair-rgb-test`

### Phase 6: Polish & Community
- [ ] Installer (WiX or NSIS)
- [ ] Auto-update mechanism
- [ ] Documentation for other Corsair LINK users
- [ ] Publish iCUE LINK protocol documentation
- [ ] Support for additional iCUE LINK devices

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | Near-zero overhead, memory safety, hidapi crate |
| UI framework | Tauri 2.0 + Svelte 5 | WebView2 native, no Chromium, tiny bundle |
| Fan algorithm | PID + acoustic filter | Best thermal performance with noise optimization |
| Architecture | Service + UI separated | Service runs 24/7 at ~3MB, UI only when needed |
| Config format | TOML | Human-readable, good Rust ecosystem support |
| Protocol approach | OpenLinkHub reference + own RE | Protocol sourced from Go OSS project, ported to Rust |
| RGB | Implemented (Phase 5) | Effect engine + hardware output via color endpoint |

## Development Environment

- **OS**: Windows 11 Pro (10.0.26200)
- **Rust**: 1.94.0 (2024 edition)
- **Node.js**: v24.14.0 (for Tauri UI)
- **Build tools**: MSVC Build Tools 2022
- **IDE**: Claude Code CLI
