# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Windows 11 desktop app: a Tauri 2 shell rendering a Svelte 5 UI in the system WebView2 (Chromium).
It has a frameless window with its own titlebar. The redesign targets a 1440×900 window (minimum 1000×700).

## Users

The primary users are PC enthusiasts who own Corsair iCUE LINK hardware and want to leave iCUE behind.
Typically that means a custom-loop or high-end air/AIO build. The project is open source and headed for a
public release. Ken (the author) is the first user but not the design target. Nothing in the product may
assume one specific rig.

They use it in three situations:
- **Glancing while gaming or working:** the window sits on a second monitor or is opened briefly to check
  temperatures, fan behaviour and power draw. It must read in a second.
- **Tuning sessions:** deliberate sessions shaping curves, PID, acoustics or lighting, often under a stress
  test while watching the effect live.
- **Showing off the build:** lighting and live visuals are part of the enjoyment, and the app may be shown
  to others.

## Product Purpose

A lightweight replacement for Corsair iCUE, covering smart fan control, thermal/acoustic optimisation, live
sensor monitoring and RGB lighting for iCUE LINK systems. It exists because iCUE is heavy (~500 MB install,
~200 MB RAM), carries telemetry, and offers only basic fan curves.

Success means users trust it to run their cooling all day and understand what their system is doing at a
glance. Using it should feel smooth and enjoyable, and it should cost almost nothing in system resources.

## Positioning

- A target-temperature controller (PID with sensor fusion and an acoustic filter), not just static curves.
- A near-zero footprint (~5 MB RAM, <0.1% CPU for the control loop).
- The first open-source Rust implementation of the iCUE LINK protocol.

iCUE cannot truthfully claim lightness or this depth of fan control. Other open-source tools do not support
the LINK System Hub.

## Operating Context

- Runs continuously alongside games and heavy workloads. The UI must never cost meaningful CPU/GPU or
  stutter the user's foreground work.
- Sensor sources:
  - CPU temperature via LibreHardwareMonitor's HTTP API;
  - GPU temperature via NVML;
  - PSU telemetry over USB HID.
- Live values update about once per second.
- Users tune while watching the result physically: fan noise and LED output on the real hardware.

## Capabilities and Constraints

- **Fan control:**
  - fixed, curve (draggable editor) and PID modes per fan group;
  - acoustic filter (asymmetric ramps, hysteresis, minimum duty);
  - Silent/Balanced/Performance presets;
  - emergency 100% override;
  - weighted multi-sensor temperature sources.
- **Monitoring:**
  - CPU/GPU/PSU temperatures;
  - per-fan RPM and duty;
  - PSU power, rails, VRM/case temperatures and fan;
  - 60-second history;
  - hub health.
- **Devices:**
  - hub info (serial, firmware) and the device tree;
  - device identity and renaming;
  - hotplug and topology detection (diagnostics);
  - LED diagnostic modes (markers, ruler, walk).
- **Lighting:**
  - 14 effects and multi-layer compositing with blend modes;
  - zones;
  - sensor-reactive effects (temperature map, thermal pulse, duty meter);
  - presets;
  - live fan-ring and strip preview.
- **Settings:** poll interval, log level, LHM path, updater.
- **Startup config recovery:** a banner explains a preserved invalid config.
- **Hardware scope:** the iCUE LINK ecosystem (LINK System Hub and LINK devices: fans, strips, pumps, AIOs)
  plus Corsair HID PSUs. Older Commander Pro/Core/Lighting Node are out of scope.
- **General-app rule:** derive only what is detectable (device types, LED counts, strip segmentation, chain
  order). Placement and names are user settings with neutral defaults (e.g. "Strip 1 · 40 LEDs").
- **Feature parity:** a redesign must never drop a capability or a piece of information. The authoritative
  checklist is `prototypes/INVENTORY.md` on branch `proto/base`.
- **Undecided:** tray/background service mode (the Windows service is a stub), installer and auto-update
  rollout (Phase 6), and pump/coolant sensor support.

## Brand Commitments

- Name: "Corsair Hub" in the titlebar; "ai-corsair-hub" is the repo name. It is not affiliated with Corsair.
- Ken's binding feedback (from rejected prototype rounds):
  - Quality reference: Apple's own apps, "more polished, modern and flawless". Polish over spectacle.
  - The product must feel premium and calm, not gamer-garish.
  - Each datum appears once per screen; use progressive disclosure, not repetition.
  - Horizontal bars, not arc/radial gauges, for temperatures and fan speeds.
  - 3D or heavy effects only if genuinely high-end; cheap 3D is worse than none.
  - Motion is smooth, physical and interruptible, never at the cost of system resources.

## Evidence on Hand

- `docs/ai-corsair-hub.png`: a screenshot of the current app.
- `docs/reference/*.avif` (untracked) and `prototypes/reference/*.png` on `proto/base`: product photos of
  a real build (Lian Li O11D EVO XL, custom loop).
- `prototypes/BRIEF.md` and `prototypes/INVENTORY.md` on `proto/base`: the round-2 brief and the parity
  checklist.
- Round-2 prototypes on local `kenhaesler/proto2-*` branches. Ken's pick is **health** (c6f255f): "way
  more polished". He liked pro's style but found it redundant.
- Footprint figures (~5 MB RAM, <0.1% CPU) come from the README.
- There are no user testimonials, download numbers or benchmarks against iCUE beyond the README figures.
  Do not fabricate any.

## Product Principles

1. **Glanceable truth first.** The current thermal and acoustic state must be readable in about a second.
   Detail is revealed on demand.
2. **Smooth and light at once.** Every animation earns its cost. The UI pauses when hidden and never
   competes with the user's game or workload.
3. **Tuning you can feel.** Changes show their effect live and immediately, so users connect a control to
   what they hear and see.
4. **Works on any LINK build.** Detect what's detectable, let users name the rest, and never assume one rig.
5. **Never lose capability.** Simplify presentation, not power: every feature stays reachable.

## Accessibility & Inclusion

- Every live number and status must be readable without relying on colour alone.
- Keyboard-operable controls with visible focus.
- `prefers-reduced-motion` is honoured with an intentional, still-informative alternative.
- AA text contrast.
