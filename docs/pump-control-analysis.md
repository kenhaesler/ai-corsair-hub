# Pump Control Feasibility Analysis

**Date**: 2026-03-15
**Analyzer**: brahma-analyzer
**Subject**: Can ai-corsair-hub control the EK-Loop D5 G3 pump?

---

## Executive Summary

The ai-corsair-hub codebase has **strong support for iCUE LINK-connected pumps** but
**no support for motherboard-connected pumps** like the EK D5 G3 on a PWM fan header.
Adding motherboard pump control is technically feasible via LibreHardwareMonitor (which
the project already depends on) but carries significant safety risks and requires
substantial architectural changes.

**Feasibility Rating: MEDIUM** -- possible but requires careful implementation with
robust safety guarantees.

---

## Part 1: Current Codebase Coverage

### 1.1 iCUE LINK Pump Support (Already Implemented)

The codebase already handles LINK-connected pumps (XD5, XD6) comprehensively:

**File: `crates/hid/src/icue_link.rs`**
- `LinkDeviceType` enum includes `PumpXd5` (0x0C) and `PumpXd6` (0x19) -- lines 71, 77
- `is_pump()` helper method returns true for both pump types -- line 133
- `set_speeds()` accepts any channel including pump channels -- line 318
- Doc comment explicitly notes: "Minimum 20% for fans, 50% for pumps (caller must enforce pump minimum)" -- line 317

**File: `crates/fancontrol/src/control_loop.rs`**
- On hub initialization, pump channels are auto-detected via `is_pump()` -- lines 239-244
- `pump_channels: Vec<u8>` is stored per hub connection -- line 66
- During tick(), per-channel minimum duty enforcement checks `is_pump` -- lines 338-348
- `MIN_PUMP_DUTY` constant is 50% (vs 20% for fans) -- line 28
- Emergency override forces pumps to 100% -- lines 320-325
- Sensor stale fallback forces pumps to 70% (FAILSAFE_DUTY) -- lines 326-330

**Verdict**: If the D5 pump were connected to an iCUE LINK hub, it would work today
with proper minimum duty enforcement. But it is NOT connected to a LINK hub.

### 1.2 Motherboard Fan/Pump Header Support (NOT Implemented)

The following components have ZERO awareness of motherboard fan/pump headers:

| Component | Current State | Gap |
|-----------|--------------|-----|
| `config.rs` FanGroupConfig | Requires `hub_serial` (iCUE LINK only) | No motherboard device type |
| `control_loop.rs` build() | Only opens iCUE LINK hubs via DeviceScanner | No SuperIO/LHM fan control path |
| `control_loop.rs` tick() | Only sends commands via `hub.set_speeds()` | No motherboard PWM write path |
| `hardware_thread.rs` | Only manages hubs and PSU | No motherboard device handles |
| `dto.rs` DeviceTree | Only contains hubs and PSU | No motherboard fan header entries |
| `sensors/` crate | Read-only temperature sources | No fan/pump speed write capability |

**Verdict**: The entire output path (duty -> hardware) is hardcoded to iCUE LINK hubs.
There is no abstraction layer for "fan/pump output device."

---

## Part 2: Motherboard Fan Header Control -- Interface Research

### 2.1 The MSI MEG X870E Godlike Super I/O Chip

The MSI MEG X870E Godlike uses the **Nuvoton NCT6799D** Super I/O chip (or a close
variant in the NCT67xx family). This chip provides:

- Up to 7 fan speed monitoring inputs (tachometer)
- Up to 7 fan control outputs (PWM and DC voltage mode)
- Temperature monitoring from multiple sources
- The W_PUMP+ header is typically wired to one of these fan control outputs

### 2.2 Can LibreHardwareMonitor Read AND Write Fan Headers?

**Reading: YES** -- LibreHardwareMonitor can read fan RPM and current duty from the
NCT6799D via its Super I/O driver. This data is available via both the HTTP API
(already used by this project at `http://127.0.0.1:8085/data.json`) and WMI.

**Writing: PARTIALLY** -- This is the critical question:

1. **LibreHardwareMonitor's internal API** has `ISuperIO.SetControl(index, value)` which
   writes PWM values directly to Super I/O registers. However:
   - This is a C# internal API, not exposed via HTTP or WMI
   - The HTTP webserver at port 8085 is **read-only** (GET /data.json)
   - The WMI namespace is **read-only** (sensor queries only)

2. **LibreHardwareMonitor does NOT expose a fan control API** over any remote interface.
   You would need to either:
   - Link against LibreHardwareMonitorLib.dll directly (C# interop from Rust)
   - Use direct Super I/O register access from Rust

### 2.3 Direct Super I/O Access (Most Viable Path)

The NCT6799D registers are accessible via port I/O on Windows:

```
Base address: typically 0x2E/0x2F or 0x4E/0x4F (ISA-style port I/O)
Fan control registers: bank-specific, e.g.:
  - Bank 0, offset 0x09: Fan Control Output Value (PWM duty 0-255)
  - Bank 0, offset 0x04: Fan Control Mode (manual/auto/SmartFan)
```

**Approaches**:

| Approach | Complexity | Risk | Admin Required |
|----------|-----------|------|----------------|
| Direct port I/O via `inpoutx64.dll` | Medium | High (raw register writes) | Yes |
| WinRing0 driver (used by LHM, HWiNFO) | Medium | Medium (kernel driver) | Yes |
| LibreHardwareMonitor C# interop | High | Low (proven safe) | Yes (LHM needs admin) |
| MSI SDK / MSI Center API | Low (if exists) | Low | Depends |
| ACPI/WMI vendor methods | Low | Low | Yes |

### 2.4 MSI WMI/ACPI Interface

MSI motherboards sometimes expose WMI methods for fan control:
- WMI namespace: `root\WMI` with class `MSI_ACPIMethod` or similar
- The MSI Center software uses proprietary EC (Embedded Controller) commands
- These are undocumented and vary by motherboard model
- Risk: MSI may change the interface between BIOS updates

### 2.5 Recommended Approach: WinRing0 + NCT6799D Register Access

The most reliable path mirrors what LibreHardwareMonitor does internally:

1. Use the **WinRing0** kernel driver (or equivalent like `inpoutx64`) for port I/O
2. Detect the NCT6799D at its ISA base address
3. Read the current fan control mode for the W_PUMP+ channel
4. Switch to **manual PWM mode** and write duty values
5. On shutdown, restore the original fan control mode (auto/SmartFan)

This is exactly how OpenHardwareMonitor, FanControl (by Rem0o), and HWiNFO control
motherboard fans on Windows.

**Reference implementation**: The open-source **FanControl** project by Rem0o
(`github.com/Rem0o/FanControl.Releases`) wraps LibreHardwareMonitorLib for this purpose
and has proven safety across hundreds of motherboard models.

---

## Part 3: Gap Analysis -- What Would Need to Change

### 3.1 New Crate: `crates/superio/` or extend `crates/sensors/`

A new module for motherboard Super I/O interaction:

```rust
// New trait: FanOutput (abstraction over iCUE LINK and motherboard)
pub trait FanOutput {
    fn set_duty(&self, channel: u8, duty_percent: u8) -> Result<()>;
    fn get_rpm(&self, channel: u8) -> Result<u16>;
    fn restore_auto(&self) -> Result<()>; // critical for safety
}
```

### 3.2 Config Changes (`crates/common/src/config.rs`)

Currently `FanGroupConfig` requires `hub_serial`. Need a device abstraction:

```rust
// Current (only supports iCUE LINK hubs):
pub struct FanGroupConfig {
    pub hub_serial: Option<String>,  // <-- tied to iCUE LINK
    ...
}

// Proposed (supports multiple device types):
pub enum FanOutputDevice {
    IcueLinkHub { serial: String },
    Motherboard { header: String },  // e.g. "W_PUMP+", "CPU_FAN", "SYS_FAN1"
}

pub struct FanGroupConfig {
    pub device: FanOutputDevice,  // replaces hub_serial
    ...
}
```

### 3.3 Control Loop Changes (`crates/fancontrol/src/control_loop.rs`)

The control loop needs a polymorphic output path:

| Current | Required Change |
|---------|----------------|
| `hubs: HashMap<String, HubConnection>` | Add `mobo_outputs: HashMap<String, MoboFanOutput>` |
| `hub.set_speeds(commands)` in tick() | Route to correct output device based on group config |
| `hub_conn.hub.get_speeds()` for RPM readback | Add motherboard RPM readback via LHM or SuperIO |
| `shutdown_hardware()` restores hub to hardware mode | Must also restore motherboard auto fan control |
| Pump minimum duty check uses `pump_channels` from hub | Need a way to mark motherboard channels as pump |

### 3.4 Hardware Thread Changes (`apps/gui/src/hardware_thread.rs`)

- Initialize motherboard fan output handles alongside hub handles
- Include motherboard pump RPM in SystemSnapshot
- Handle motherboard-specific errors (driver not loaded, access denied)

### 3.5 GUI Changes

- DeviceTree needs a motherboard section showing fan headers
- Dashboard needs a pump RPM readout (currently only shows iCUE LINK fans)
- Settings need to allow selecting motherboard headers as output targets
- FanGroupCard needs to support non-hub output devices

### 3.6 Estimated Effort

| Component | Effort | Risk |
|-----------|--------|------|
| SuperIO driver integration (WinRing0) | 2-3 days | High -- kernel driver, ring-0 access |
| NCT6799D register map + fan channel detection | 1-2 days | Medium -- hardware-specific |
| Config schema migration | 0.5 day | Low |
| Control loop output abstraction | 1 day | Medium |
| Hardware thread integration | 0.5 day | Low |
| GUI changes (device tree, dashboard) | 1 day | Low |
| Safety system (failsafes, watchdog) | 1-2 days | Critical |
| Testing (hardware-in-the-loop) | 1-2 days | -- |
| **Total** | **8-13 days** | -- |

---

## Part 4: Risk Assessment -- Pump Safety

### 4.1 Critical Safety Concern: Pump Stall = System Damage

A water cooling pump is NOT a fan. If a fan stops, temperatures rise slowly and
thermal throttling protects the CPU/GPU. If a **pump stops**:

- Coolant circulation halts immediately
- CPU and GPU temperatures spike within **seconds** (not minutes)
- Thermal throttling may not react fast enough if coolant is stagnant
- Risk of **hardware damage** to CPU, GPU, or waterblocks

**The D5 G3 pump must NEVER be set to 0% duty or allowed to stall.**

### 4.2 Minimum Speed Requirements

| Device | Absolute Minimum | Safe Minimum | Recommended Range |
|--------|-----------------|-------------|-------------------|
| EK D5 G3 pump | ~800 RPM (PWM 30%) | ~1200 RPM (PWM 50%) | 50-100% PWM |
| QX140 fan | ~300 RPM (PWM 20%) | ~400 RPM (PWM 25%) | 20-100% PWM |
| QX120 fan | ~350 RPM (PWM 20%) | ~450 RPM (PWM 25%) | 20-100% PWM |

The existing `MIN_PUMP_DUTY = 50.0` constant is appropriate and should apply to
the motherboard pump channel as well.

### 4.3 Required Safety Systems

**Tier 1: Software Failsafes (Already Partially Implemented)**
- [x] Minimum duty floor (50% for pumps) -- in control_loop.rs
- [x] Emergency override to 100% on critical temps -- in control_loop.rs
- [x] Sensor stale timeout triggers failsafe duty (70%) -- in control_loop.rs
- [ ] **NEW: Pump RPM monitoring with stall detection**
- [ ] **NEW: Watchdog timer -- if software crashes, restore auto fan control**

**Tier 2: Hardware Restore on Exit**
- [x] `shutdown_hardware()` restores iCUE LINK hubs to hardware mode -- in control_loop.rs
- [ ] **NEW: Restore motherboard to SmartFan/auto mode on exit**
- [ ] **NEW: Handle SIGTERM/SIGKILL/crash -- must restore even on abnormal exit**
- [ ] **NEW: Windows Service recovery action should restore auto mode**

**Tier 3: Pump-Specific Protection**
- [ ] **NEW: Pump stall detection** -- if RPM drops below threshold despite non-zero duty,
  trigger emergency (all fans 100%) AND log critical warning
- [ ] **NEW: Pump RPM cross-validation** -- compare commanded duty vs actual RPM,
  detect disconnect or malfunction
- [ ] **NEW: Never allow pump duty below MIN_PUMP_DUTY through ANY code path**
  (including direct SetManualDuty commands from GUI)
- [ ] **NEW: Startup safety check** -- verify pump is spinning before entering
  software control mode

### 4.4 Crash Recovery Problem

The most dangerous scenario: the ai-corsair-hub process crashes or is killed while
the motherboard is in manual PWM mode. If the last commanded duty was low (or the
register defaults to 0), the pump could stall.

**Mitigation strategies**:

1. **Hardware watchdog**: Some Super I/O chips have a watchdog timer that reverts to
   auto mode if not refreshed. The NCT6799D has watchdog capability but it is typically
   not wired for fan control revert.

2. **BIOS fallback**: Set the BIOS W_PUMP+ header to "100% duty" as the default. Then
   software control overrides to a quieter speed. If software dies, BIOS takes over
   at 100% (safe but loud).

3. **Separate watchdog process**: A tiny helper process that monitors the main process
   and restores auto mode if it exits unexpectedly. This is what FanControl does.

4. **Periodic register refresh**: Write the duty value every poll cycle. If the process
   freezes, the Super I/O may (depending on configuration) revert to its BIOS-configured
   behavior after a timeout.

**Recommended**: Option 2 (BIOS set to 100%) + Option 3 (watchdog process) together
provide defense in depth.

---

## Part 5: Recommended Approach

### Phase A: Read-Only Monitoring (Low Risk, High Value)

Add motherboard pump monitoring without any control:

1. Extend the LHM HTTP reader (`crates/sensors/src/lhm.rs`) to also extract fan RPM
   from motherboard fan headers (the data is already in LHM's data.json)
2. Add a "Pump RPM" readout to the GUI dashboard
3. This requires NO Super I/O write access and is completely safe

**Effort**: 1-2 days. **Risk**: None.

### Phase B: Controlled Pump Speed (Medium Risk, Medium Value)

Add actual PWM control of the W_PUMP+ header:

1. Integrate WinRing0 or equivalent port I/O driver
2. Implement NCT6799D Super I/O fan control for the specific header
3. Add all safety systems from Section 4.3
4. Implement crash-safe watchdog
5. Refactor config and control loop for output device abstraction

**Effort**: 8-13 days. **Risk**: Significant (hardware damage if buggy).

### Phase C: Alternative -- Use BIOS/MSI Center for Pump

The simplest and safest approach may be:

1. Set the W_PUMP+ header in BIOS to a fixed high speed (80-100%)
2. Let ai-corsair-hub control only the fans via iCUE LINK
3. The pump runs at a constant safe speed; the fans do the dynamic cooling

**Effort**: 0 days. **Risk**: None. **Tradeoff**: Slightly louder pump, but D5 pumps
at 80%+ are generally inaudible compared to 10 fans.

---

## Conclusion

| Approach | Effort | Risk | Noise Impact | Recommended |
|----------|--------|------|-------------|-------------|
| A: Read-only monitoring | 1-2 days | None | None | YES -- do this first |
| B: Full PWM control | 8-13 days | Significant | Best (dynamic speed) | Maybe later |
| C: Fixed BIOS speed | 0 days | None | Slight (constant pump) | YES -- pragmatic default |

**Immediate recommendation**: Implement Phase A (monitor pump RPM via LHM) and use
Phase C (BIOS fixed speed) for now. This gives you visibility into pump status with
zero risk. Phase B can be revisited if the pump noise at fixed speed is unacceptable,
but given 3x 420mm radiators providing massive cooling capacity, the fans are the
primary noise source, not the pump.

---

## Appendix: Key File Locations

| File | Relevance |
|------|-----------|
| `crates/hid/src/icue_link.rs` | iCUE LINK pump support (PumpXd5, PumpXd6, is_pump) |
| `crates/fancontrol/src/control_loop.rs` | Pump minimum duty enforcement, emergency override |
| `crates/common/src/config.rs` | FanGroupConfig -- needs FanOutputDevice abstraction |
| `crates/sensors/src/lhm.rs` | LHM HTTP reader -- extend for motherboard fan RPM |
| `apps/gui/src/hardware_thread.rs` | Hardware I/O loop -- needs motherboard output path |
| `apps/gui/src/dto.rs` | SystemSnapshot/DeviceTree -- needs pump/mobo data |
| `docs/architecture.md` | Architecture -- already lists "Pump (D5 G3 via motherboard)" |
