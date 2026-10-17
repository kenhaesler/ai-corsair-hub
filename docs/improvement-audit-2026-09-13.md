# Improvement audit — 13 September 2026

The highest-value next work is cooling reliability: scheduling, bounded sensor reads, shutdown, and controller behavior. The repository already includes substantial GUI, RGB, identity migration, and recovery functionality; AGENTS.md's Phase 0 description is obsolete.

Scope: local source review and automated checks. No live fan/USB commands were issued, no application configuration was changed, and no GitHub issues were opened. Findings below are source-confirmed unless explicitly described as hardware behavior requiring verification.

## Validation

- `cargo test --workspace --quiet`: 132 tests passed.
- `npm run check`: 0 errors, 40 warnings in 5 files.
- `npm run build`: passed.
- A temporary standalone Rust probe imported the repository's actual acoustic filter. It reproduced both the scheduling time-step mismatch and suppression of changing controller requests at steady temperature. The scheduling probe simulated 30 calls; it did not measure live USB throughput.
- Existing tests do not cover the GUI command/configuration lifecycle or scheduling integration sufficiently to detect the issues below.

## Bugs and reliability issues

### 1. P1 — RGB changes fan-control cadence and acoustic ramp speed

Evidence: `apps/gui/src/hardware_thread.rs:398,655`; `crates/fancontrol/src/control_loop.rs:465`.

Every outer-loop iteration runs a full control tick. Enabling RGB changes the sleep target to the smaller of the fan poll interval and RGB interval, but the acoustic filter still receives the configured fan interval as elapsed time. With 1000 ms polling and 30 FPS, sufficiently fast I/O permits roughly 30 fan ticks per second, each claiming a full second elapsed. A 3%/second ramp then advances 90 percentage points over 30 calls. Actual speed depends on I/O latency. Sensor/PSU requests and snapshot events also run unnecessarily often; sample-count-based history covers less time. Conversely, slow sensor I/O blocks RGB.

Fix: schedule fan, telemetry, and RGB work with separate monotonic deadlines, use measured elapsed time for control, prioritize cooling, and bound command draining. Return cached snapshots from GetSnapshot: its current implementation runs another control tick (`hardware_thread.rs:766`).

Acceptance: switching RGB between disabled/30/60 FPS must not alter fan ramp speed or configured sensor polling; slow RGB writes must not starve cooling.

### 2. P1 — CPU HTTP reads can block the cooling worker indefinitely

Evidence: `crates/sensors/src/cpu.rs:113,246`; `crates/fancontrol/src/control_loop.rs:1143`.

The CPU backend uses bare `ureq::get(...).call()` and body reads without a deadline. The locked ureq 3.2.0 source defaults global, response, and body timeouts to None. A local server that accepts a connection but stops responding can block the hardware worker before the five-second stale-sensor check runs. PSU/LHM helper reads already use an explicit timeout, but the CPU backend does not.

Fix: a shared LHM client with a bounded total request/body deadline, ideally one cached tree per telemetry poll. For stronger isolation, keep sensor acquisition outside the cooling scheduler and use timestamped readings.

Acceptance: a mock HTTP server that accepts but never responds must cause a timely sensor-failure transition while cooling commands and shutdown remain responsive.

### 3. P1 — Quit does not wait for hardware control to be restored

Evidence: `apps/gui/src/lib.rs:96`; `apps/gui/src/hardware_thread.rs:50,382`.

The tray Quit handler sends Shutdown and immediately calls `app.exit(0)`. The worker is detached; neither a completion acknowledgment nor a join is awaited. The worker's bounded hardware-mode restoration therefore races process termination. The eventual fan behavior depends on firmware fallback and needs hardware verification; the application itself does not guarantee restoration.

Fix: coordinate all exit paths, await restoration acknowledgment with an overall deadline, then exit. Preserve/report restoration failures.

Acceptance: a delayed mock hardware-mode transition completes before application exit, and a permanently blocked transition reaches a bounded exit with an explicit error.

### 4. P1 — Temperature hysteresis suppresses PID corrections

Evidence: `crates/fancontrol/src/acoustic.rs:31`; `crates/fancontrol/src/control_loop.rs:1237`.

The same acoustic filter gates both curve and PID requests on a temperature change exceeding hysteresis. A PID integral can change its requested duty while temperature remains steady, but the filter ignores that new duty. The probe requested 25% and then 80% at the same 65°C; filtered duty stayed at 25%. PID's first update also returns its minimum, which can become the latched target until temperature moves enough.

Fix: use temperature hysteresis for curve evaluation and apply only output slew limiting/deadband to PID requests. Preserve the emergency bypass.

Acceptance: sustained temperature above target must increase applied PID duty even when the input temperature is numerically constant.

### 5. P2 — Lighting changes leave configuration copies inconsistent

Evidence: `apps/gui/src/hardware_thread.rs:209,234,302,324`; `apps/gui/ui/src/components/lighting/Lighting.svelte:29`.

SetRgbConfig/SetRgbEnabled update the worker-local configuration and disk, but GetConfig returns `control_loop.config()`, which retains the old RGB state. Reopening Lighting can reload old settings. A subsequent full settings save can persist that stale RGB state over the user's edits. Conversely, UpdateConfig updates the control loop but does not rebuild the RGB renderer or refresh its enable/FPS variables, so full-config changes can diverge from live lighting. Several RGB/preset save failures also log a warning and return success.

Fix: one authoritative configuration, section-level updates or revision checks, coherent application to all consumers, and propagated persistence errors.

Acceptance: edit lighting, leave/reenter the tab, edit fan settings, and restart; the same lighting settings must survive each step. Injected disk failure must be reported.

### 6. P1 — Stale-sensor fallback can lower an already high cooling duty

Evidence: `crates/fancontrol/src/control_loop.rs:474,503`.

Any initialized stale sensor forces every group to exactly 70%, including fixed-duty groups and groups unrelated to that sensor. A pump explicitly running at 100% drops to 70% if an unrelated sensor goes stale. After a critical CPU reading is discarded as stale, the emergency condition can clear and similarly reduce cooling despite no confirmed temperature recovery.

Fix: distinguish required and optional sensors per group, preserve an appropriate previous/configured duty floor, and latch thermal emergencies until valid readings demonstrate recovery. Make the policy explicit rather than treating 70% as universally sufficient.

Acceptance: losing an unrelated sensor cannot reduce a 100% fixed pump; a critical-to-missing sensor transition cannot clear emergency cooling without a defined recovery condition.

### 7. P2 — Recovery only covers a subset of missing-hardware cases

Evidence: `crates/fancontrol/src/control_loop.rs:174,198,303,640,796`; `apps/gui/src/hardware_thread.rs:97,712`.

V1 startup aborts ControlLoop construction if a referenced hub cannot open. Discovery-only mode does not attempt to rebuild control. V2 opens currently enumerated hubs; a hub absent at startup never enters the recovery set. Runtime recovery considers only known hubs marked unhealthy by fan writes: speed-read errors merely log warnings, so a hub with no fan commands can remain reported healthy despite failing reads. CPU/GPU initialization failures likewise do not schedule reinitialization.

Fix: independent per-device initialization, periodic discovery/reconciliation, sensor retries, and health based on relevant I/O rather than only fan writes. Continue controlling available hubs during partial failure.

Acceptance: start with one of two hubs absent, reconnect it, and regain control without restarting; repeat with a sensor unavailable at startup and a lighting-only hub.

### 8. P2 — Live non-control edits reset controller history

Evidence: `crates/fancontrol/src/control_loop.rs:689`; `apps/gui/src/hardware_thread.rs:1199`; `crates/fancontrol/src/acoustic.rs:13`.

Every update_config rebuilds every fan group, including its PID and acoustic state. Renaming a device invokes this path. An unrelated rename or settings save can therefore reset an actively cooling curve/PID group to its startup ramp/minimum behavior.

Fix: preserve state for unchanged group definitions; transfer current duty when changing a controller deliberately. Metadata/RGB changes should not rebuild fan controllers.

Acceptance: renaming a device while fans run at high duty must not change the next commanded duty.

### 9. P2 — Hub replacement leaves effective LED metadata and rendering inconsistent

Evidence: `crates/fancontrol/src/control_loop.rs:813`; `apps/gui/src/hardware_thread.rs:430,1011`.

replace_hub installs fresh enumeration info without reapplying configured LED-count overrides to that cached info. Important qualification from the implementation-plan review: send_rgb_frames separately consults the current V2/V1 configuration overrides, so this does **not** establish that reconnect loses wire-level override lengths. The confirmed issue is disagreement between effective metadata, renderer layouts, and wire sizing. Recovery does not call apply_rgb_config, so renderer layouts remain based on previous topology/counts; devices skipped as unresolved during configuration are not added just because the registry changes. Removing an override during update_config also leaves the old count in cached hub info.

Fix: rebuild effective hardware metadata from raw enumeration plus current overrides, then reconcile renderer targets/layouts after topology or override changes.

Acceptance: configured strip lengths survive reconnect; removing an override restores the detected count; a previously absent target starts rendering after discovery.

## Features worth building next

1. **Coolant-based radiator control.** Expose real coolant and ambient sources with stable IDs, units, freshness, and provenance; support coolant target or coolant-minus-ambient control. Keep CPU/GPU emergency thresholds as independent guards. `water_inline` is currently a preset placeholder, and hub temperature reads are only called by the scanner. Identify what each physical probe measures before labeling it coolant.
2. **Pump/fan stall detection.** Compare sustained commanded duty with measured RPM, allow spin-up grace, distinguish missing telemetry from zero RPM, and report the affected device. Add pump-specific recovery rules.
3. **Windows Service with one hardware owner.** The service app is currently a console loop, not an SCM-registered service. Move hardware ownership to a boot-time service and let the GUI communicate over local IPC; avoid competing handles between GUI/service/diagnostic tools. Include sleep/resume reconciliation and watchdog reporting.
4. **Explainable cooling status.** Show why each group is at its present duty: curve/PID, ramping, minimum floor, manual setting, stale source, or emergency. Display reading age and sensor backend, plus a time-based history and exportable diagnostic bundle.
5. **Acoustic calibration.** Offer a deliberate calibration workflow for minimum stable fan duty, RPM response, and user-marked resonance bands. Treat pumps separately. Record results per stable device ID.

## Engineering backlog and suggested order

- First: bounded CPU reads, coordinated shutdown, independent scheduling, PID filtering, and conservative failure policy.
- Second: authoritative config/state preservation and complete discovery/recovery with RGB reconciliation.
- Third: coolant sources, stall detection, and service ownership.
- Add Windows PR CI for Rust tests, frontend checks/build, and the lifecycle/fault-injection tests described above. Current workflow files are release and manual release-dryrun workflows; they do not provide PR test coverage.
- Tighten config validation: finite numerical values, positive ramp rates, valid hysteresis/PID limits, RGB bounds, and exclusive fan ownership across groups. Current validation does not comprehensively enforce these invariants.
- Resolve the 40 frontend warnings, especially inaccessible labels and pointer-only editor interactions; unused CSS can follow.
- Update AGENTS.md, architecture/README phase claims, and version labels to describe the current implementation. Separate measured resource usage from resource targets.

This audit does not establish firmware behavior during app crashes, actual pump-safe operating ranges, or the identity/accuracy of physical temperature probes. Those need controlled hardware validation after the software regressions are covered.
