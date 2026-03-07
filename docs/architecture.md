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
| RGB (later) | iCUE LINK LS350 strips, Dominator Titanium | iCUE LINK Hub |

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
│   │   └── config.rs          # AppConfig, FanGroupConfig, FanMode, CurvePoint
│   ├── hid/                   # USB HID communication layer
│   │   ├── discovery.rs       # DeviceScanner: enumerate & group Corsair devices
│   │   └── icue_link.rs       # IcueLinkHub: raw protocol, probe, send/receive
│   ├── sensors/               # Temperature source abstractions
│   │   ├── cpu.rs             # CpuSensor (WMI/LHWM) [stub]
│   │   └── gpu.rs             # GpuSensor (NVML) [stub]
│   └── fancontrol/            # Fan control algorithms
│       └── pid.rs             # PID controller with anti-windup (tested)
├── apps/
│   ├── scanner/               # CLI: USB device scanner + protocol probe
│   └── service/               # Windows Service daemon [stub]
├── ui/                        # Svelte 5 frontend [future]
├── docs/
│   └── architecture.md        # This file
└── scripts/
    └── scan_usb.ps1           # PowerShell USB device enumerator
```

## Protocol Status

### iCUE LINK System Hub (PID 0x0C3F) — REVERSE ENGINEERING IN PROGRESS

**What we know:**
- MI_00 is the bidirectional data interface
- MI_01 is read-only (writes fail with "Incorrect function")
- All responses are exactly 512 bytes
- Device echoes the command byte in response[0]
- response[1] = 0x0F for all commands tested (likely "not initialized")
- Protocol is related to Commander Core but distinct

**Probe results (2026-03-07):**
```
TX: [0x00, 0x01, ...pad...]  →  RX: [0x01, 0x0F, 0x00, ...]  (512 bytes)
TX: [0x00, 0x05, ...pad...]  →  RX: [0x05, 0x0F, 0x00, ...]  (512 bytes)
TX: [0x00, 0x04, 0x01, 0x00] →  RX: [0x04, 0x0F, 0x00, ...]  (512 bytes)
```

**Hypothesis:** The 0x0F status means the device hasn't received a proper
handshake/init sequence. The Commander Core uses an open→configure→query flow.
The iCUE LINK Hub likely has a similar but different initialization.

**Next step:** Capture iCUE's USB traffic with USBPcap + Wireshark to discover
the initialization handshake.

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

### Phase 1: Protocol Reverse Engineering
- [ ] Install USBPcap on the system
- [ ] Capture iCUE's USB traffic with Wireshark (filter: usb.idVendor == 0x1B1C)
- [ ] Decode initialization handshake for iCUE LINK Hub
- [ ] Implement open/init sequence in Rust
- [ ] Discover and implement: read fan RPM
- [ ] Discover and implement: set fan duty cycle
- [ ] Discover and implement: read connected device count
- [ ] Discover and implement: read temperature sensors (if hub has any)

### Phase 2: Sensor Integration
- [ ] CPU temperature via WMI or LibreHardwareMonitor
- [ ] GPU temperature via NVML (nvidia-ml-sys crate)
- [ ] HX1500i PSU monitoring (power, temp, fan)
- [ ] Motherboard sensors (VRM, chipset) via WMI
- [ ] Water temperature (when user adds inline sensor)

### Phase 3: Fan Control Service
- [ ] Wire PID controller to real sensor inputs
- [ ] Wire fan output to iCUE LINK Hub set-duty commands
- [ ] Implement fan groups with per-group PID
- [ ] Implement acoustic filter (ramp rates, hysteresis, zero-RPM)
- [ ] TOML config file loading/watching
- [ ] Windows Service registration and lifecycle
- [ ] System tray integration (temperature display)
- [ ] Auto-start on boot

### Phase 4: Tauri UI
- [ ] Scaffold Tauri 2.0 + Svelte 5 project
- [ ] Dashboard: real-time temp/fan/power graphs
- [ ] Fan curve editor (draggable points on a graph)
- [ ] PID tuning interface
- [ ] Fan group configuration
- [ ] Device info panel (serial, firmware, connected fans)
- [ ] System tray with quick presets (Silent/Balanced/Performance)
- [ ] Dark theme, modern design

### Phase 5: RGB Control (Future)
- [ ] Reverse-engineer RGB commands from iCUE LINK protocol
- [ ] Static color control for QX fans
- [ ] Static color for LS350 Aurora strips
- [ ] Basic effects (breathing, color cycle)
- [ ] Per-fan color configuration in UI

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
| Protocol approach | Reverse-engineer from scratch | iCUE LINK not in any OSS tool, we're first |
| RGB | Deferred | Focus on thermal first, RGB adds complexity |

## Development Environment

- **OS**: Windows 11 Pro (10.0.26200)
- **Rust**: 1.94.0 (2024 edition)
- **Node.js**: v24.14.0 (for Tauri UI)
- **Build tools**: MSVC Build Tools 2022
- **IDE**: Claude Code CLI
