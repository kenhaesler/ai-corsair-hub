# Thermal Profile, Fan Curves and Temperature Management Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement the software tasks (Phase B/C) task-by-task. Steps use checkbox (`- [ ]`) syntax. Phase A is Ken's hands (BIOS, UI, hardware).

**Goal:** Quieter idle, protection for the parts the loop does not cool (DIMMs, chipset) and for the pump itself, and the control inputs a high-thermal-mass custom loop needs: board temperature, coolant temperature, pump telemetry, power feed-forward.

**Architecture:** (1) Configuration-only changes to the four fan groups and the BIOS pump curve, deployable today. (2) A single typed LibreHardwareMonitor sensor registry in `crates/sensors` that turns sensors LHM already publishes (board, DIMM, CCD, thermistor, GPU memory junction, pump rpm/duty) into fan-group temperature sources and pump telemetry, plus a pump watchdog in the control loop. (3) Control quality: power feed-forward for curve mode and presets re-tuned for this class of loop.

**Tech Stack:** Rust 2024 (`crates/sensors`, `crates/fancontrol`, `crates/common`, `apps/gui`), LHM HTTP JSON at `http://127.0.0.1:8085/data.json` (`crates/sensors/src/lhm.rs:17`), Tauri 2 commands, Svelte 5 UI. The Sensor Mix UI already lists whatever `list_available_sensors` returns, so new sources appear with no UI plumbing beyond labels.

**Spec:** This document; it argues from the measurements in Context.

## Context

### Measurements (LHM dump 2026-11-08, ~1 h 45 into a light-load session)

| Part | Now | Session min / max | Reading |
|---|---|---|---|
| CPU Tctl/Tdie | 69.5 °C | 43.5 / 75.4 | The only hot part in the loop. CCDs Average (Tdie) 64.5, max 78.1. |
| GPU core | 34.9 °C | 25.0 / 35.7 | Near ambient; memory junction 44 °C. |
| DIMM #1 / #3 | 56.8 / 57.5 °C | 29.3 / 57.5 | Above the modules' own 55 °C "Thermal Sensor High Limit". |
| Chipset / System / PCIe x1 | 60 / 56 / 55.5 °C | max 61 | Board ambient is warm for ten fans. VRM MOS 31 °C. |
| PSU case / VRM | 56 / 55 °C | 31 / 56 | HX1500i fan 0 rpm (zero-RPM at ~22 % load). |
| Pump (D5 G3 on PUMP_FAN) | 4428 rpm at 89 % | 3380–4761 rpm, 69–95 % | BIOS curve drives it; a D5 at 4400 rpm is audible. Not on a LINK hub (`pumps=0` on both). |
| Power | CPU 133 W, GPU 81 W; PSU 334 W in / 314 W out | PSU max 432 W | ~94 % efficiency. |
| Thermistor header | −40 °C | — | Nothing connected; T_SENSOR is free. |
| Other board fan headers | 0 rpm | — | Unused. |

### Current fan configuration (`%APPDATA%\corsair-hub\config.toml`)

Four groups, all identical: Balanced (30→25 %, 45→35 %, 60→55 %, 75→80 %, 85→100 %, hysteresis 3, ramp 5/3, mix CPU 0.6 / GPU 0.4).

| Config name | Physical fans (verified LED map, memory `physical-fan-led-map`) |
|---|---|
| `top_exhaust` | rear (22DE/1), side top (22DE/13), side middle (22DE/14) |
| `rear_exhaust` | side bottom (22DE/16) |
| `side_intake` | top radiator ×3 (8B44/2,3,1) |
| `bottom_intake` | bottom radiator ×3 (8B44/13,14,15) |

Names do not match hardware. Radiators per the MIFCOM order: 2× EK P420M (44 mm) + 1× S420 (30 mm); which is top/bottom/side is to be confirmed while renaming.

### Diagnosis

1. **The loop over-serves the GPU and under-serves the board.** All groups follow CPU 0.6 / GPU 0.4; with the GPU at 35 °C the mix sits ~5 °C under Tctl, fans idle at 35–40 %, and nothing reacts to DIMM or chipset temperature. 57 °C on DDR5-6400 is a stability risk.
2. **Tctl drives everything.** Tctl spikes on single-core bursts that never reach the coolant; LHM's "CCDs Average (Tdie)" is 5 °C lower and smoother, a better proxy for heat entering the water.
3. **The pump is the loudest idle component and unmonitored.** The BIOS runs it 69–95 %. A stall would cook CPU and GPU in a minute; our failsafes (`FAILSAFE_DUTY`, `check_emergency`) only touch fans and only watch `cpu`/`gpu`.
4. **No coolant temperature.** `water_inline` is only a preset placeholder (`hardware_thread.rs:1655`), never produced by any reader. The header for it is free.
5. **Group names mislead tuning.**

### Codebase map (from exploration, read-only)

- Sensor ids are hard-coded keys inserted in `ControlLoop::build` (`crates/fancontrol/src/control_loop.rs:198-302`): `cpu`, `gpu`, lazily `psu_vrm`/`psu_case`; unknown `psu_*` bails at :300. `available_sensors()` at :697 returns the map keys. GUI: `HwCommand::ListSensors` (`apps/gui/src/hardware_thread.rs:460`) → `commands::list_available_sensors` (`apps/gui/src/commands.rs:56`).
- `TemperatureSource` trait: `crates/sensors/src/lib.rs:16-19` (`name()`, `read() -> Result<Temperature>`), `Temperature { celsius }` at `crates/common/src/types.rs:88`.
- LHM is walked three separate ways: `cpu.rs` (`LhmJsonNode` :38, `collect_cpu_temps` :315, `find_sensor_value` :327, `parse_lhm_value` :340, name priority `["Core (Tctl/Tdie)","CPU Package","Tctl/Tdie"]` :151), `lhm.rs` (`LhmNode` with `HardwareId` :174, `read_psu_from_lhm` :195, `read_cpu_metrics_from_lhm` :271, helpers :306-373). Each `read()` refetches the whole tree.
- Control loop tick :477-680: `poll_sensors` :1251 (stale after `SENSOR_STALE_TIMEOUT` 5 s), `check_emergency` :1282 (`CRITICAL_CPU_TEMP` 95, `CRITICAL_GPU_TEMP` 90), `compute_weighted_temp` :1371 (missing ids silently dropped, renormalised), `compute_group_duty` :1328, `AcousticFilter` (`crates/fancontrol/src/acoustic.rs:30-54`), pump floor `MIN_PUMP_DUTY` 50 applied at :555-583 via `pump_channels` from `device_type.is_pump()` :342.
- Config: `FanMode` (`crates/common/src/config.rs:129-162`, Curve has `points, hysteresis, ramp_rate, ramp_down_rate: Option<f64>, temp_source`), `TempSourceConfig` :170 (free-form ids), `FanPresetDef` :186 (`cpu_weight, gpu_weight, water_weight`). Validation: `config_validation.rs:55-138`, `control_loop.rs::validate_config` :1681, `validate_temp_source` :1806 (no check that ids are known).
- GPU: NVML only, core temperature only (`crates/sensors/src/gpu.rs:56-110`); memory junction/hotspot not read anywhere.
- Presets: `apps/gui/assets/presets/fans.toml` embedded via `include_str!` (`apps/gui/src/presets.rs:21`); `hardware_thread.rs::apply_preset` :1631-1726 builds the sensor list from `[("cpu",cpu_weight),("gpu",gpu_weight),("water_inline",water_weight)]` filtered by `available_sensors()`.
- GUI labels: `apps/gui/ui/src/lib/format.ts:28` (`sourceLabel`), `apps/gui/ui/src/screens/fans/fanModes.ts:117` (`sensorLabel`), `apps/gui/ui/src/screens/dashboard/overview.ts` (`srcShort`/`srcLong`, `pathSources` icon by prefix).
- LINK hubs can read device temperatures: `IcueLinkHub::get_temperatures()` (`crates/hid/src/icue_link.rs:856`), not exposed as a source.
- Prior art: `docs/pump-control-analysis.md` (motherboard pump write path, safety tiers), `docs/improvement-audit-2026-09-13.md:109` (coolant control proposal).

## Global Constraints

- General app, not rig-specific: no hard-coded LHM `SensorId`s, header names or Ken's group names in app code. Match by sensor `Type` + `Text` patterns that LHM uses across boards; expose what exists; the user labels placement.
- Every new sensor is optional: absent → the source is not registered, presets fall back exactly as `water_inline` does today (`apply_preset` :1631).
- Pump safety: keep `MIN_PUMP_DUTY = 50.0` for LINK pumps; motherboard pump remains BIOS-controlled in this plan (read + alarm only). Writing pump duty is out of scope (see `docs/pump-control-analysis.md`).
- Config additions use `#[serde(default)]` optional fields so `schema_version = 2` configs load unchanged; no migration unless a field is required.
- Do not touch `crates/rgb` or RGB frame routing.
- Presets live in `apps/gui/assets/presets/fans.toml`; the `shipped_fan_presets_satisfy_rules` test (`config_validation.rs:~522`) must keep passing.
- UI: new sources need only label entries (format.ts, fanModes.ts, overview.ts); the Sensor Mix, Summary temperature rows and control-path pills already render any id.

## Review Focus

1. **Sensor vanishes at runtime** (LHM restarted, thermistor unplugged → −40 °C): the mix must keep working on the remaining sensors and never drive 0 % or the override. Pinned by Task 1 (`is_connected` sentinel test) and existing `compute_weighted_temp` drop behaviour; Task 5 adds the "no pump sensor → no alarm" test.
2. **Thermistor sentinels** (−40 °C, exactly 0 °C, > 120 °C) are "not connected", never coolant. Task 1 test.
3. **Duplicate nodes** (DIMM #1/#3, CCD1/CCD2, two "D3D Copy"): aggregate to one logical source (DIMM max; CCD via LHM's "CCDs Average (Tdie)"), so the Sensor Mix does not fill with near-duplicates. Task 2 test.
4. **Pump watchdog on boot/resume**: LHM reports 0 rpm for the first seconds and after sleep; alarm only after 5 consecutive bad reads and only once the pump has been seen above the threshold at least once this session. Task 5 tests.
5. **Feed-forward only raises and always decays**: a sustained high-power idle (video decode at 130 W) must not pin fans; the offset is relative to a 60 s power average and clamps at a maximum. Task 6 tests.

---

## Phase A — Configuration today (Ken, no code)

### Task A1: Rename and regroup fans by physical role

- [ ] In Fan Control, rename groups with the pencil / F2 using the verified map: `Top radiator` = 8B44/2,3,1 (today `side_intake`); `Bottom radiator` = 8B44/13,14,15 (today `bottom_intake`); `Side radiator` = 22DE/13,14,16; `Rear exhaust` = 22DE/1. Use Edit devices to move 22DE/16 into the side group and 22DE/1 out of the old `top_exhaust`.
- [ ] Confirm which radiator is the S420 (30 mm) and which two are P420M by eye, and which radiators are intake vs exhaust (tissue test). Add the direction to the name where helpful (`Side radiator · intake`).
- [ ] Save Configuration; Summary → Control path shows four groups with the new names.

### Task A2: One group follows the board, not the cold GPU

Approximation until Task 2 exposes board temperature.

- [ ] `Rear exhaust`: Sensor Mix CPU 1.0; curve 30→30, 50→45, 65→65, 80→100; hysteresis 3, ramp 5/3. Its job is board air, so it must not idle because the GPU is cold.
- [ ] Radiator groups: keep CPU 0.6 / GPU 0.4; idle shape from Silent (30→20, 50→30), steeper above 65 °C (65→60, 80→90, 88→100); hysteresis 4, ramp 4/2.
- [ ] After 30 min of normal use compare LHM DIMM/Chipset/System temps against today's 57 / 60 / 56 °C. Target: DIMMs ≤ 52 °C at idle. If not reached, raise the rear exhaust idle point to 40 %.

### Task A3: Tame the pump in BIOS (safe interim)

- [ ] BIOS → Hardware Monitor → PUMP_FAN (PWM mode): flat 60 % to 60 °C CPU, 100 % at 80 °C.
- [ ] Verify with LHM `Pump Fan #1`: idle 2600–3000 rpm; under a 10 min load the CPU Tctl − GPU core spread grows by no more than ~3 °C versus before.
- [ ] Record the points in `docs/hardware-baseline.md`.

### Task A4: Order and fit the coolant sensor (prerequisite for Task 3's coolant source going live)

- [ ] G1/4 inline 10 kΩ NTC sensor with a 2-pin motherboard lead (Alphacool / Barrow / Bitspower; β 3435 or 3950 both read fine on MSI's T_SENSOR).
- [ ] Fit on the return line before the first radiator (hottest coolant) at the next loop service; plug into T_SENSOR. LHM then shows `Nuvoton NCT6687D-R / Temperatures / Thermistor Sensor` with a real value instead of −40.

---

## Phase B — Read-side software

### Task 1: One LHM sensor registry (`crates/sensors/src/lhm_registry.rs`)

**Files:**
- Create: `crates/sensors/src/lhm_registry.rs`
- Create: `crates/sensors/tests/fixtures/lhm-godlike.json` (trimmed copy of today's dump: the Nuvoton node, AMD CPU temperatures/powers, both DIMM nodes, NVIDIA temps/powers, PSU node; strip the per-core clock/load/VID lists to keep it < 60 KB)
- Modify: `crates/sensors/src/lib.rs` (add `pub mod lhm_registry;`)
- Test: `crates/sensors/src/lhm_registry.rs` (unit tests on the fixture)

**Interfaces:**
- Produces:
  ```rust
  pub enum LhmKind { Temperature, Fan, Control, Power, Load, Voltage, Other }
  pub struct LhmSensor { pub sensor_id: String, pub hardware_id: String, pub hardware_name: String, pub name: String, pub kind: LhmKind, pub value: f64 }
  pub fn parse_lhm_tree(json: &str) -> anyhow::Result<Vec<LhmSensor>>;      // pure
  pub fn fetch_lhm_sensors() -> anyhow::Result<Vec<LhmSensor>>;            // GET via lhm::bounded_get_string, then parse
  pub struct LhmCache { ttl: Duration, .. }  impl LhmCache { pub fn new(ttl: Duration) -> Self; pub fn sensors(&self) -> anyhow::Result<Arc<Vec<LhmSensor>>>; }
  pub fn is_connected_thermistor(value_c: f64) -> bool;  // false for <= -30.0, == 0.0, > 120.0, non-finite
  ```
  `LhmCache::sensors` refetches when the cached tree is older than `ttl` (use 400 ms), so N sources reading in one tick share one HTTP round trip (the tick runs at ≤ 1 Hz; readers today each refetch).

- [ ] **Step 1:** Write failing tests in `lhm_registry.rs`:
  - `parses_pump_fan_and_control_from_fixture`: parse fixture; assert a sensor with `kind == Fan`, `name == "Pump Fan #1"`, `value == 4428.0`, `hardware_name` starts with "Nuvoton"; and a `Control` named `"Pump Fan"` with `value == 89.0`.
  - `parses_board_and_dimm_temperatures`: Temperature sensors named `"System"` 56.0, `"Chipset"` 60.0, `"VRM MOS"` 31.0, `"DIMM #1"` 56.8, `"DIMM #3"` 57.5, `"CCDs Average (Tdie)"` 64.5, `"GPU Memory Junction"` 44.0, `"Thermistor Sensor"` -40.0.
  - `parse_value_handles_units_and_comma`: `"59,6 °C"` → 59.6, `"4428 RPM"` → 4428, `"89.0 %"` → 89.
  - `thermistor_sentinels_are_not_connected`: `is_connected_thermistor` false for -40.0, 0.0, 130.0, NaN; true for 31.5.
- [ ] **Step 2:** Run `cargo test -p corsair-sensors lhm_registry` → FAIL (module missing).
- [ ] **Step 3:** Implement `parse_lhm_tree` (walk `Children`, carry the nearest ancestor with a `HardwareId` as hardware id/name, map `Type` string to `LhmKind`, parse value with the same first-token/comma rule as `cpu.rs::parse_lhm_value`), `fetch_lhm_sensors`, `LhmCache`, `is_connected_thermistor`.
- [ ] **Step 4:** `cargo test -p corsair-sensors lhm_registry` → PASS.
- [ ] **Step 5:** Commit: `feat(sensors): typed LibreHardwareMonitor sensor registry with shared cache`.

### Task 2: Board, DIMM, CCD, coolant and GPU-memory temperature sources

**Files:**
- Create: `crates/sensors/src/lhm_temps.rs`
- Modify: `crates/sensors/src/lib.rs` (`pub mod lhm_temps;` + re-exports)
- Modify: `crates/fancontrol/src/control_loop.rs:198-302` (register discovered sources; keep `cpu`/`gpu`/`psu_*` as they are)
- Modify: `apps/gui/ui/src/lib/format.ts:28`, `apps/gui/ui/src/screens/fans/fanModes.ts:117`, `apps/gui/ui/src/screens/dashboard/overview.ts` (`srcShort`/`srcLong`/icon), `apps/gui/ui/src/lib/format.ts` `sourceColor` (new ids reuse `--s-other` except `water_inline` → `--s-water`)
- Test: `crates/sensors/src/lhm_temps.rs` unit tests; `crates/fancontrol/src/control_loop.rs` existing tests unchanged.

**Interfaces:**
- Consumes: Task 1 `LhmCache`, `LhmSensor`, `is_connected_thermistor`.
- Produces:
  ```rust
  pub struct LhmTempSpec { pub id: &'static str, pub label: &'static str, pub select: fn(&LhmSensor) -> bool, pub reduce: Reduce } // Reduce = Max | Mean | First
  pub const LHM_TEMP_SPECS: &[LhmTempSpec]; // ids: "board_system","chipset","vrm","dimm","cpu_ccd","water_inline","gpu_memory"
  pub fn discover_lhm_temp_sources(cache: Arc<LhmCache>) -> Vec<Box<dyn TemperatureSource>>; // one per spec that matches ≥1 connected sensor now
  pub struct LhmTempSource { .. } impl TemperatureSource for LhmTempSource { fn name(&self)->&str /* the id */; fn read(&self)->Result<Temperature> }
  ```
  Selection rules (Type Temperature; names as LHM emits them across Nuvoton/ITE/Fintek drivers): `board_system`: name == "System" or "Motherboard" under a SuperIO hardware node (hardware_id starts with `/lpc/`); `chipset`: name == "Chipset" under `/lpc/`; `vrm`: name starts with "VRM"; `dimm`: hardware_id starts with `/ram/` or name starts with "DIMM" → Max; `cpu_ccd`: name == "CCDs Average (Tdie)" (fallback Mean over names matching `CCD\d+ \(Tdie\)`); `water_inline`: name contains "Thermistor" or "T_Sensor" or "Water" under `/lpc/` and `is_connected_thermistor(value)`; `gpu_memory`: name == "GPU Memory Junction" or "GPU Hot Spot" under `/gpu-nvidia/` or `/gpu-amd/` (Hot Spot only if Junction absent).
  `read()` re-selects by the same rule each tick (no pinned SensorId) so a hot-plugged thermistor appears without restart, and errors with "not connected" when the selection is empty (the loop then treats it as stale → dropped from the mix, as today).

- [ ] **Step 1:** Failing tests on the fixture: `discovers_expected_ids` (set equality with `{"board_system","chipset","vrm","dimm","cpu_ccd","gpu_memory"}`, i.e. no `water_inline` because −40 °C), `dimm_reduces_to_max` (57.5), `ccd_uses_lhm_average` (64.5), `water_inline_appears_when_thermistor_connected` (mutate fixture value to 31.2 → id present, read 31.2).
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement specs, discovery, source; in `ControlLoop::build` after the `gpu` insert (:205): `for s in discover_lhm_temp_sources(cache.clone()) { sensors.entry(s.name().to_string()).or_insert(s); }` with one `Arc<LhmCache>` constructed in `build` (also hand it to `CpuSensor` later if desired; not required).
- [ ] **Step 4:** `cargo test -p corsair-sensors -p corsair-fancontrol` → PASS; `cargo run --bin corsair-hub-gui` dev: Fan Control → Sensor Mix → "Add" shows Board, Chipset, VRM, Memory, CPU cores, GPU memory.
- [ ] **Step 5:** UI labels: format.ts `SOURCE_LABEL` + fanModes.ts `FRIENDLY` + overview.ts `NAMES/LONG` gain `board_system: "Board"`, `chipset: "Chipset"`, `vrm: "VRM"`, `dimm: "Memory"`, `cpu_ccd: "CPU cores"`, `water_inline: "Coolant"`, `gpu_memory: "GPU memory"`; `pathSources` rank puts `water_inline` after gpu (already by prefix "water"); `sourceColor`: `water_inline` → `WATER_COLOR`, others → `OTHER_COLOR`. `npx svelte-check` clean.
- [ ] **Step 6:** Commit: `feat(sensors): board, memory, CCD, coolant and GPU-memory temperature sources from LHM`.

### Task 3: Pump telemetry in the snapshot and UI

**Files:**
- Create: `crates/sensors/src/pump.rs`
- Modify: `apps/gui/src/dto.rs:15-35` (`SystemSnapshot.pumps: Vec<PumpReading>`), `apps/gui/src/hardware_thread.rs` (fill it next to `cpu_metrics` ~:744), `apps/gui/ui/src/lib/types.ts` (type), `apps/gui/ui/src/screens/dashboard/SpecPanel.svelte` (Speeds tab: one row per pump, scale shared with fans), `apps/gui/ui/src/screens/devices/Devices.svelte` + new `apps/gui/ui/src/screens/devices/PumpCard.svelte` (panel "Pumps": name, source, rpm, duty, min/max this session), `apps/gui/ui/src/preview/fixtures.ts` (mock pump)
- Test: `crates/sensors/src/pump.rs` unit tests on the Task 1 fixture.

**Interfaces:**
- Consumes: Task 1 `LhmCache`.
- Produces:
  ```rust
  pub struct PumpReading { pub id: String /* lhm sensor_id or "link:<serial>:<ch>" */, pub name: String, pub rpm: f64, pub duty_pct: Option<f64>, pub source: PumpSource } pub enum PumpSource { Motherboard, Link }
  pub fn read_motherboard_pumps(cache: &LhmCache) -> Vec<PumpReading>; // Fan sensors whose name contains "pump" (case-insensitive) under /lpc/; duty from the Control sensor with the same name minus the " #n" suffix when present
  ```
  LINK pumps: `hardware_thread` adds `PumpReading { source: Link }` for channels in `pump_channels` from the existing `cycle_result.fan_speeds` (no new HID traffic).
  DTO: `#[serde(default)] pub pumps: Vec<PumpReadingDto>` so old consumers are unaffected.

- [ ] **Step 1:** Failing tests: `finds_pump_fan_with_duty` (fixture → one reading, name "Pump Fan #1", rpm 4428, duty Some(89.0)), `ignores_zero_rpm_non_pump_headers` (no "System Fan #1").
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** PASS.
- [ ] **Step 5:** DTO + hardware thread + UI: Speeds tab row "Pump · 4,428 rpm · 89 % duty" (icon Droplet, scale max ≥ 5000 when a pump exists), Devices "Pumps" panel. Preview fixture: one motherboard pump at ~2800 rpm, 62 %.
- [ ] **Step 6:** `cargo test`, `svelte-check`, screenshot Summary Speeds and Devices at 1440 and 1000. Commit: `feat: pump rpm and duty from the motherboard header on Summary and Devices`.

### Task 4: Coolant goes live (Water Priority no longer a preview)

**Files:**
- Modify: `apps/gui/src/hardware_thread.rs::apply_preset` :1631-1726 (no logic change needed; verify `water_inline` now passes the `available_sensors()` filter), `apps/gui/assets/presets/fans.toml:145-164` (description drops "Preview — "; keep the fallback sentence), `apps/gui/ui/src/screens/dashboard/overview.ts::tempSub` (`'Coolant'` already), `apps/gui/ui/src/screens/fans/PresetChips.svelte` (chip tooltip: when `water_inline` is unavailable, append "needs a coolant sensor").
- Test: `apps/gui/src/hardware_thread.rs` tests (there are preset tests around `apply_preset`): `water_priority_uses_water_when_available` (fake `available_sensors` containing `water_inline` → sensors == ["water_inline","cpu","gpu"] with weights [1.0,0.6,0.4]); existing fallback test stays.

- [ ] Steps: failing test → implement/adjust → PASS → `cargo test -p corsair-hub-gui` → commit `feat(presets): Water Priority uses the coolant sensor when present`.
- [ ] Hardware (Ken, after A4): plug in the sensor; in Fan Control the Sensor Mix "Add" row shows Coolant; apply Water Priority to the radiator groups; keep `Rear exhaust` on Board.

### Task 5: Pump watchdog → emergency override + alert

**Files:**
- Modify: `crates/common/src/config.rs` (`GeneralConfig.pump_min_rpm: Option<u32>` with `#[serde(default)]`; default used = 800), `crates/fancontrol/src/control_loop.rs` (new `PumpWatchdog` struct + `tick(&mut self, pumps: &[PumpReading]) -> WatchdogState`; `check_emergency` ORs `watchdog.stalled`), `apps/gui/src/hardware_thread.rs` (pass pumps into the tick; emit `hw-error`-style event `pump-stall` with the pump name on rising edge, `pump-recovered` on falling edge), `apps/gui/src/dto.rs` (`SystemSnapshot.pump_stall: bool`), `apps/gui/ui/src/screens/dashboard/AlertRows.svelte` (red row "Pump <name> below N rpm. All fans at full speed." with the same style as the emergency row), `apps/gui/ui/src/screens/settings/Settings.svelte` (General: "Pump alarm below … rpm" NumberField 0–3000, 0 = off).
- Test: `crates/fancontrol/src/control_loop.rs` tests.

**Interfaces:**
- Consumes: Task 3 `PumpReading`.
- Produces:
  ```rust
  pub struct PumpWatchdog { min_rpm: f64, bad_ticks: u8, armed: bool, pub stalled: bool }
  impl PumpWatchdog { pub fn new(min_rpm: f64) -> Self; pub fn tick(&mut self, pumps: &[PumpReading]) -> bool /* stalled */ }
  ```
  Rules: `armed` becomes true the first tick any pump reads ≥ `min_rpm` (so boot/resume zeros do not alarm); once armed, `stalled` after 5 consecutive ticks with every pump < `min_rpm`; clears after 3 consecutive good ticks; no pumps present → never armed, never stalled; `min_rpm == 0` disables.

- [ ] **Step 1:** Failing tests: `does_not_alarm_before_armed` (five 0-rpm ticks from boot → false), `alarms_after_five_bad_ticks_once_armed` (4428 then 0×4 → false, 5th → true), `recovers_after_three_good_ticks`, `missing_pump_sensor_never_alarms` (empty slice ×20 → false), `zero_threshold_disables`.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement; wire into `tick()` so `stalled` forces `EMERGENCY_DUTY` through the existing emergency path (pump floor still applies to LINK pumps). **Step 4:** PASS, `cargo test` whole workspace.
- [ ] **Step 5:** GUI event + AlertRows + Settings field; preview fixture gets a `pump_stall` toggle (dev-only) so the banner can be screenshotted.
- [ ] **Step 6:** Commit: `feat(safety): pump watchdog raises the emergency override on a stall`.

---

## Phase C — Control quality

### Task 6: Power feed-forward for curve mode

**Files:**
- Modify: `crates/common/src/config.rs:129-162` (`FanMode::Curve.power_feed_forward: Option<f64>` °C per 100 W, `#[serde(default)]`), `crates/fancontrol/src/control_loop.rs` (`tick` gains `power_watts: Option<f64>`; `compute_group_duty` Curve branch evaluates `curve.evaluate(t + feed_forward_offset(..))`), new pure fn in `crates/fancontrol/src/feedforward.rs`, `apps/gui/src/hardware_thread.rs` (sum `cpu_metrics.power_watts + gpu_metrics.power_watts` when both present, pass to tick), `crates/common/src/config_validation.rs` (0 ≤ k ≤ 20), `apps/gui/ui/src/screens/fans/NoiseBehavior.svelte` (slider "Power anticipation" 0–10 °C per 100 W, hint "Raises the curve early when CPU+GPU power jumps; 0 = off").
- Test: `crates/fancontrol/src/feedforward.rs`.

**Interfaces:**
```rust
pub struct PowerAverage { .. } impl PowerAverage { pub fn new(window_s: f64) -> Self; pub fn push(&mut self, watts: f64, dt_s: f64) -> f64 /* running mean */ }
pub fn feed_forward_offset(p_now: f64, p_avg: f64, k_per_100w: f64) -> f64; // = clamp((p_now - p_avg)/100 * k, 0, 15)
```
Only positive offsets (power above its 60 s average); decays as the average catches up; max +15 °C equivalent.

- [ ] **Step 1:** Failing tests: `no_offset_when_power_steady` (300,300,k=5 → 0), `offset_scales_with_rise` (450,300,k=5 → 7.5), `never_negative` (200,300 → 0), `clamped` (2000,300,k=5 → 15), `average_decays` (push 300 for 60 s then 450 for 60 s → mean ≥ 440).
- [ ] **Step 2:** FAIL. **Step 3:** Implement + wiring; default `None` → behaviour identical to today (regression test: existing curve tests pass untouched). **Step 4:** PASS.
- [ ] **Step 5:** UI slider; `svelte-check`; commit `feat(fancontrol): optional power feed-forward for curve mode`.

### Task 7: Presets for a high-mass loop + a case-airflow preset

**Files:**
- Modify: `apps/gui/assets/presets/fans.toml`, `crates/common/src/config.rs:186-219` (`FanPresetDef.board_weight: f64` default 0 via `#[serde(default)]`, `power_feed_forward: Option<f64>`), `apps/gui/src/hardware_thread.rs::apply_preset` (sensor list adds `("board_system", board_weight)`; sets `power_feed_forward`), `apps/gui/ui/src/lib/types.ts`, `apps/gui/ui/src/screens/dashboard/PresetShelf.svelte` + `screens/fans/PresetChips.svelte` (meta line shows "board" when board_weight > 0).
- Test: `config_validation.rs::shipped_fan_presets_satisfy_rules` (extend to the new fields), `hardware_thread.rs` preset tests.

Preset changes (values to ship; the header comment already states the loop assumptions):
- `silent`: unchanged shape; `power_feed_forward = 2.0`.
- `balanced`: points 30→22, 50→32, 65→55, 78→85, 88→100; hysteresis 4, ramp 4 / 2; cpu 0.5 / gpu 0.5; `power_feed_forward = 4.0`. Rationale: idle quieter (the 45 °C point was driving 35 % at desktop load), steeper where coolant actually warms, equal weights because this loop's GPU is cold at idle and hot in games.
- `gaming`: cpu 0.4 / gpu 0.6 kept; `power_feed_forward = 6.0`; description "GPU-weighted, ramps early on power spikes".
- `performance`, `benchmark`: unchanged shapes; feed-forward 6 / 8.
- `water_priority`: description without "Preview —".
- New `case_airflow`: for exhaust/case groups; `board_weight = 1.0`, cpu 0 / gpu 0; points 35→30, 45→45, 55→65, 65→100; hysteresis 2; ramp 4 / 2; description "For exhaust fans: follows motherboard temperature, keeps memory and chipset cool".

- [ ] Steps: extend tests (new preset passes validation; `apply_preset` with `case_airflow` and `board_system` available → sensors == ["board_system"]; without it → falls back to cpu/gpu as today) → implement → PASS → UI meta → screenshot Quick Presets at 1440/1000 (7 tiles still fit the 3-col grid at 1000) → commit `feat(presets): high-mass-loop tuning, power anticipation, Case airflow preset`.
- [ ] Ken: apply `Case airflow` to `Rear exhaust`, `Balanced` to the radiators; after Task A4, `Water Priority` to the radiators.

## Verification

- `cargo test` (workspace) green; `cd apps/gui/ui && npx svelte-check` 0 errors; `impeccable detect --json apps/gui/ui/src` → `[]`.
- Real app: Fan Control → Sensor Mix "Add" lists Board, Chipset, VRM, Memory, CPU cores, GPU memory (and Coolant after A4). Summary Temperatures tab shows the same once a group uses them (rows come from `snapshot.temperatures`, which the loop fills from registered sources).
- Summary → Speeds shows the pump at its live rpm; Devices shows the Pumps panel.
- Pump alarm drill: Settings → pump alarm threshold 6000 rpm → within 5 s the red "Pump … below 6000 rpm" row appears and all groups report 100 % in the control path; set back to 800 → clears within 3 s. Never stall the pump for real.
- Thermal result after A1–A3 + Task 7: at desktop idle, DIMM ≤ 52 °C, Chipset ≤ 55 °C, radiator fans ≤ 30 %, pump ≤ 3000 rpm; under a 20 min game, CPU Tctl ≤ 85 °C, no emergency events in the log, feed-forward visible as fans rising before Tctl in the History chart.
- Memory/docs: update `docs/hardware-baseline.md` with the new group names, BIOS pump points and the before/after table; update `docs/architecture.md:141-146` sensor-fusion section and `PRODUCT.md` capability list (board/coolant/pump).
