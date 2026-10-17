# Fix implementation plan — preserve working RGB addressing

Prepared 13 September 2026 against commit `c32a0bd` and the local improvement audit. This is a plan; production fixes and the new regression tests have not been implemented. No hardware commands are needed to prepare it.

**Amended 17 October 2026** with the twelve gaps (G01–G12) from `docs/plan-gap-analysis-2026-10-04.md`. The PR descriptions below are unchanged; the binding additions and the revised execution order are in [Amendments from the gap analysis](#amendments-from-the-gap-analysis) at the end. Where an amendment and a PR description disagree, the amendment wins.

## Outcome and scope

Fix all nine audited reliability issues while retaining the working device-to-LED mapping. Deliver small, dependency-ordered PRs with explicit tests and rollback points. Cover config validation, PR CI, frontend warnings, and documentation as supporting work. New coolant control, stall detection, calibration, and Windows Service functionality remain a subsequent feature phase, rather than prerequisites for these fixes.

The critical constraint is that an unrelated cooling/configuration fix must not change which physical LEDs receive an RGB frame. Passing Rust tests alone cannot establish that the physical mapping still works: retain a verified baseline and require hardware comparison before releasing changes that affect RGB orchestration or topology.

## RGB contract to preserve

The source is authoritative where older protocol documentation or comments disagree. Relevant boundaries:

| Boundary | Current implementation | Required invariant |
|---|---|---|
| Persisted identity | `crates/common/src/identity.rs`, `config_migration.rs` | Keep stable device IDs and V1 compatibility; never substitute enumeration order, labels, or GUI indexes for identity. No schema migration is needed for this fix train. |
| Renderer targets | `apps/gui/src/hardware_thread.rs::apply_rgb_config`, `crates/rgb/src/renderer.rs` | Frames remain keyed by device ID. Resolve V1 references through the registry; retain unresolved config references. Preserve zone/effect/flow ordering. |
| Wire routing | `crates/fancontrol/src/control_loop.rs::send_rgb_frames` | Resolve each ID to its current hub/channel, separate hubs, then include every enumerated LED-bearing device on each hub being written. |
| Slot lengths | Same function and `IcueLinkHub::initialize` | Preserve precedence: explicit V2 override, explicit V1 override, accepted nonzero firmware count, then device-type default. Firmware zero is a fallback marker, not proof of a zero-length slot. Explicit config zero retains its existing meaning. |
| Missing frame on a written hub | `send_rgb_frames` | Black-fill the device's entire slot. Pad short frames and truncate long frames to the effective count. Never concatenate only the lit devices: that shifts later devices. |
| Flat buffer | `crates/hid/src/icue_link.rs::set_rgb` | Ascending physical channel order within each hub; three bytes per LED in R,G,B order. Channel gaps do not create imaginary device slots. |
| Framing and lifecycle | `write_color`, `build_rgb_payload`, endpoint methods | Preserve length/header bytes, 508-byte chunking, first/continuation commands, packet sizes, endpoint sequence, initialization order and retries. Complete a frame transaction before another operation on that hub. |
| Brightness | Worker RGB conversion and `port_power_factor` | Preserve current scaling, rounding, and power-factor behavior during this train. Any power-factor redesign needs separate evidence and tests. |

**Mandatory historical case:** `IcueLinkHub::initialize` records an empirically validated topology with channel 15 reporting 80 addressable positions for the adapter/strip chain and other channels reporting zero. Keep that nonzero entry and apply defaults on the other channels. Do not replace 80 with the 21-LED layout placeholder, reject the whole table because some entries are zero, or infer lengths from product names.

**Protected production code:** HID color encoding/flattening, LED-table interpretation, device-ID parsing/migration, and RGB effect/layout math. Changes there require a separately justified diff and byte-level equivalence evidence; do not include cleanup or abstraction rewrites in the cooling fixes. Test-only instrumentation may observe existing paths. If a test seam needs a production refactor, isolate it and prove before/after equivalence first.

One qualification to audit issue 9: the wire sender already checks config overrides directly after reconnect. Fix inconsistent metadata/layout reconstruction without removing that protection or assuming every reconnect currently shifts wire offsets.

## PR 0 — Record the baseline and establish regression gates

Dependencies: none. Establish before any production fix.

- Record the full baseline commit, executable checksum, firmware versions, device IDs, hub/channel map, detected/effective LED counts and their source, and a backup of the working configuration. Keep machine-specific evidence local unless deliberately sanitized for the repository. Keep a usable baseline executable and matching config for rollback.
- Add fixture data from the working setup where available. Mark synthetic fixtures as synthetic; the documented channel-15 case is not a substitute for checking the current machine's actual topology.
- Extend the existing RecordingHub test double to capture owned **RGB values**, not only `(channel, length)` summaries. Fixtures should assert expected per-device slots and per-hub concatenated bytes, with independent expected values rather than calling the same production builder to compute expectations.
- Exercise actual HID packet construction through a minimal recording I/O seam, including call order and response scripting. Existing tests that only call `.chunks(508)` do not prove the real sender uses the right continuation command.
- Add Windows PR CI: `cargo test --workspace`, frontend `npm ci`, `npm run check`, and `npm run build`. Ensure Rust tests fail the job independently of the frontend commands. No scanner/RGB-test program or physical device access in CI.

Required fixtures and assertions:

1. Two hubs with overlapping channel numbers; shuffled input order must not mix their frames.
2. Mixed QX fans, adapter/strip chain, a zero-LED device, and channel gaps; assert every downstream byte offset.
3. Channel 15 = 80 and zeros elsewhere, including the final LED and neighboring device boundary. Partial firmware tables retain valid entries.
4. Only one device has a frame: other enumerated LED devices on that written hub occupy black slots. Record separately the current behavior when an entire hub has no frames; do not silently invent a blackout transaction in this train.
5. Short/long frames, V1/V2 count precedence, explicit zero, nonzero firmware counts, missing counts, and orphan IDs.
6. Deterministic static and per-LED marker frames with different R/G/B components. Compare exact bytes after brightness/power scaling. Freeze time/random inputs when testing effects.
7. Actual packet headers, declared length, first/continuation commands, and ordering. Use valid RGB lengths immediately below/above 508-byte boundaries and large multichunk frames; test exact arbitrary chunk boundaries at the packet layer where appropriate.
8. Endpoint opens once, frame chunks stay contiguous, fan traffic resumes after the frame, and shutdown restores hardware mode in the existing command order.

Gate: all tests pass on the unmodified production baseline. Keep expected fixtures under review; changing an expected offset/color/length requires explaining the intended behavior change, never merely refreshing snapshots until green.

## PR 1 — Bound CPU sensor reads (audit 2)

Dependencies: PR 0. RGB exposure: low; sensor latency changes still affect the shared worker.

Files: `crates/sensors/src/cpu.rs`, `lhm.rs`, sensor tests.

- Use the existing bounded LHM HTTP policy for CPU startup and recurring reads, covering response and body consumption with a total deadline.
- Return errors on timeout/malformed/non-finite readings. Do not retain a successful timestamp for failed data.
- Keep this first patch narrow. Consolidating the CPU/PSU/metrics JSON fetch into one timestamped tree can follow in PR 4; avoid unrelated sensor backend changes.

Tests: server accepts and stalls before headers; stalls during body; sends malformed data; succeeds after a timeout. Verify the request returns by the configured deadline plus test tolerance and existing valid readings parse unchanged. RGB golden fixtures remain identical.

## PR 2 — Coordinate application shutdown (audit 3)

Dependencies: PR 1 and PR 0. RGB exposure: lifecycle, requiring physical quit/restart comparison.

Files: `apps/gui/src/lib.rs`, `state.rs`, `hardware_thread.rs`; reuse `ControlLoop::shutdown_hardware`.

- Retain worker completion state and expose a shutdown acknowledgment. Request shutdown once, stop accepting mutations, let current bounded I/O finish, restore hardware mode, then exit.
- Use an asynchronous exit coordinator for explicit application exit paths. Closing the window must continue hiding to tray. Handle repeated requests idempotently and inspect updater/restart paths as part of the exit audit.
- Preserve the existing restoration command sequence. Set/document an overall shutdown deadline accounting for the number of hubs and current per-hub timeout; report failed/timed-out restoration before exit.
- Wake promptly while idle; do not join synchronously on the UI thread. Forced process termination cannot provide a software restoration guarantee and must not be reported as covered.

Tests: delayed successful restoration occurs before exit, one wedged hub does not prevent other restoration attempts, repeated quit, disconnected worker, and window close. Physical gate: quit/relaunch and check both cooling ownership and the unchanged LED mapping.

## PR 3 — Correct PID filtering and stale-sensor policy (audits 4, 6)

Dependencies: PR 0. RGB exposure: low; do not edit rendering or hardware addressing.

Files: `crates/fancontrol/src/acoustic.rs`, `pid.rs`, `control_loop.rs`.

- Separate curve temperature hysteresis from duty slew limiting. PID's changing output must reach the slew limiter even at constant temperature.
- Make controller elapsed time injectable for deterministic tests; PR 4 supplies actual monotonic elapsed time. Handle first tick, stale intervals, and resume explicitly, resetting stale PID timing as well as integral state so recovery does not integrate an entire outage.
- Define sensor health for configured sources, including sources absent at startup. A partial weighted average must not silently count as full sensor health.
- For a temperature-controlled group losing a required source, use at least the existing failsafe floor, its previous applied duty, and its device minimum. Preserve higher fixed settings; an unrelated optional telemetry failure must not slow a pump.
- Latch an emergency triggered by a critical source until valid readings from that source remain below a documented recovery threshold for a documented duration. Keep emergency output at 100%; missing readings never qualify as recovery. Keep initial critical thresholds unchanged and avoid claiming universal hardware-safe pump values.

Tests: constant above-target temperature increases PID duty; curve hysteresis still suppresses temperature noise; ramp up/down limits hold; stale recovery avoids a derivative/integral spike; fixed 100% pump survives unrelated sensor loss; critical-to-missing stays at 100%; missing startup source and partial-source failure use the defined policy.

## PR 4 — Decouple scheduling while retaining one HID owner (audit 1)

Dependencies: PRs 1–3. RGB exposure: high due to transaction timing; protected encoders remain untouched.

Files: `apps/gui/src/hardware_thread.rs`, `crates/fancontrol/src/control_loop.rs`, sensor acquisition/cache, snapshot store.

- Schedule cooling, telemetry, RGB, and discovery with separate monotonic deadlines. Enabling RGB must not change fan tick frequency. Feed measured elapsed time to controller/filter updates; skip missed RGB frames instead of sending a catch-up burst.
- Cache the last completed snapshot. GetSnapshot returns cached data or an explicit initializing state; reading the dashboard must never command fans.
- Bound command processing per iteration, coalesce superseded preview/config requests where semantics permit, and prioritize shutdown and due cooling work. Use a wakeable timed wait.
- Acquire LHM/WMI/NVML telemetry outside the HID scheduler, creating thread-affine sensor handles on their owning thread. Share immutable, timestamped results; deduplicate HTTP tree fetches. Keep PSU HID, fan HID, and RGB HID transactions on the single hardware owner.
- Keep each existing complete RGB frame write atomic relative to other commands on its hub. Do not introduce concurrent HID handles, interleave fan writes between color chunks, or change initialization sleeps to reach a target FPS.
- Establish a measured transaction budget for the maximum supported topology. A slow synchronous HID transaction cannot be preempted safely: on overrun/failure, drop pending RGB work and back off RGB attempts to preserve cooling opportunities. State the measured worst-case cooling delay; do not claim hard real-time guarantees from a timer alone.
- Preserve the currently verified effective RGB rate as the initial release default. The protocol doc says both “20ms+” and “30–60 FPS,” which conflict at 60 FPS. Test 60 FPS scheduler requests in simulation, but do not raise actual hardware rate until measurements validate it.
- Store history by timestamps and retention duration, not by assuming one snapshot per second.

Tests: fake-clock matrix for RGB disabled/30/60, variable fan intervals, sensor delays, RGB delays/failures, command floods and clock gaps/resume. A 3%/second ramp advances by elapsed time independently of FPS. Verify identical RGB bytes for identical frame inputs, no chunk interleaving, bounded queue growth, no catch-up bursts, and cooling service after an RGB timeout.

Physical gate: compare measured RGB/fan cadence and USB errors against baseline, static colors, per-device identification, strip endpoint markers and a moving effect. Reduce achieved RGB rate if necessary; preserve addressing and cooling priority.

## PR 5 — Make config updates coherent and preserve controller state (audits 5, 8)

Dependencies: PRs 3–4. RGB exposure: renderer inputs/configuration.

Files: worker commands, `control_loop.rs::update_config`, config APIs/stores and Lighting/settings callers.

- Make the worker's validated configuration the authoritative committed state. GetConfig, persistence, controller configuration and renderer inputs must represent the same revision.
- Prepare and validate the candidate plus derived in-memory state before saving. Persist atomically, then commit the prepared state. Report persistence failure; do not partially mutate config and return success. Hardware availability is separate from successfully saving the desired config.
- Use section-specific updates and revision checks for remaining full-config saves so stale fan/settings tabs cannot overwrite lighting. Revision metadata can remain runtime/API state; retain existing TOML schema and V1/V2 migration behavior.
- Diff group definitions and preserve unchanged PID/acoustic state. For deliberate controller changes, seed from the last applied duty with explicit timing/integral handling. Names and RGB-only edits must not reset cooling.
- Rebuild RGB state only when RGB inputs change. Preserve zone order, device IDs, flow and layout rules. Do not initialize endpoints again just because a name or fan curve changed.
- Clearing a device's friendly name must clear only the name, preserving an LED override. The current rename helper deletes the whole entry on empty name; update the test that currently codifies that behavior. This small additional fix directly protects RGB configuration.
- Return committed state to callers and display save conflicts/failures. Define/save-before-live behavior consistently for presets, rename and RGB toggles.

Tests: lighting edit → tab reopen → fan edit → reload/restart retains exact config and RGB bytes; stale save rejected; disk failure leaves committed state unchanged; rename/name-clear preserves count and next fan duty; full config RGB edits update the renderer; no-op edits do not restart effect/controller state.

## PR 6 — Reconcile missing hubs and sensor recovery (audit 7)

Dependencies: PRs 4–5. RGB exposure: topology/lifecycle; split from effective-layout changes in PR 7.

Files: control-loop construction, discovery/recovery worker, sensor owner.

- Build independently per hub and keep healthy hubs running when another is missing or cannot initialize. Retain desired config for absent devices.
- Periodically discover unknown/reconnected hubs and retry failed sensor initialization with bounded backoff. Do not repeatedly reinitialize healthy hubs.
- Account for fan writes, speed reads, and RGB failures in health reporting. Distinguish RGB-only degradation from loss of cooling control; one transient color error should not trigger an uncontrolled reinitialization loop.
- Keep current HID initialization/LED-table retry sequence intact. On successful enumeration, stage a topology generation for PR 7 to reconcile. Publish diagnostics for absent/ambiguous IDs; do not guess identity or remap a missing device to a neighboring channel.
- Apply these semantics to both GUI use and the console `ControlLoop::run` path. Cover resume as a reconciliation trigger without introducing a Windows Service in this train.

Tests: start with one of two hubs absent; reconnect it; sensor absent on startup becomes available; read-only/lighting-only hub failures are visible; healthy hub is not reinitialized; repeated failures back off; pending shutdown wins over a retry; console path receives the same recovery behavior.

Gate: topology changes remain behind the release gate until PR 7 and its physical comparison pass. Do not ship automatic rediscovery with stale renderer topology.

## PR 7 — Reconcile RGB metadata/layouts without changing the wire contract (audit 9)

Dependencies: PRs 0, 5–6. Highest RGB risk; keep this diff independently revertible.

Files: `HubConnection` metadata and update/replace logic; `device_type_map`; `apply_rgb_config` orchestration.

- Keep raw accepted enumeration metadata separate from effective override-applied metadata. Derive effective counts with the existing precedence; removing an override falls back to the original firmware/default value rather than a previously overwritten cache.
- For each successful topology/config generation, prepare a self-consistent registry, effective metadata and renderer target set. Publish between complete transactions; never send a frame prepared against a mismatched topology generation.
- Reconcile orphan targets when they become available; preserve desired config. Rebuild only affected layouts/targets and preserve animation state where feasible.
- Leave `send_rgb_frames`' explicit override checks, full-device black padding and existing HID sort/flatten/framing unchanged unless equivalence tests establish a necessary change. An offset changes only when the effective physical topology/count intentionally changes, with an expected before/after fixture.
- Reject ambiguous topology from RGB publication rather than writing to a guessed location. Retain diagnostics and keep independently healthy cooling running.

Tests: reconnect same topology produces exactly baseline bytes; V1/V2 overrides survive reconnect; removing override uses raw baseline count; channel-15 80-length table survives; orphan rejoins; shuffled enumeration and device move across hubs resolve correctly; no frame uses old layout/new registry or vice versa; unassigned slots stay black.

Physical gate: static marker pattern on every device, both ends of each strip/adapter chain, unaffected neighbors and both hubs. Repeat after restart and a controlled reconnect, then a dynamic effect. Exact expected mapping must match the saved baseline or an explicitly documented physical topology change.

## PR 8 — Validation and user-visible failures

Dependencies: PRs 3, 5, 7. Supporting audit backlog.

- Validate finite temperatures/weights/gains/duties, sensible PID limits, nonnegative hysteresis, positive ramp rates, bounded RGB settings and count/payload limits before persistence or allocation.
- Reject conflicting fan ownership, including mixed V1/V2 references resolved to the same current device, while preserving unresolved IDs for reconnection. Do not forbid existing RGB zone compositions under a fan-ownership rule.
- Preserve historical valid config fixtures, including the 80-LED adapter case and configured overrides. Do not silently clamp or replace old configs; give a precise validation error and retain the last working state.
- Show command/persistence/sensor failures and reading ages consistently. Make clear whether a displayed duty is requested/applied or unavailable due to I/O failure.

Tests: non-finite TOML values, zero/negative rates, invalid limits, duplicate fan targets, excessive payload bounds, offline IDs, valid historical configs, and all errors surfaced without overwriting working config. RGB baselines unchanged for valid inputs.

## PR 9 — Warning cleanup, documentation and release evidence

Dependencies: prior fixes. Supporting audit backlog.

- Resolve the current frontend check warnings, prioritizing labels and keyboard operation. Restrict this PR to behavior/accessibility fixes and unused CSS; visual redesign would obscure regression review.
- Update AGENTS.md/README/architecture to match implemented features and the retained console-service limitation. Correct the protocol documentation's zero-count semantics and record verified RGB cadence without modifying protocol code to fit prose.
- Document shutdown/recovery/failsafe behavior, deadlines, logs, known firmware limitations, configuration compatibility and rollback.

Gate: `cargo test --workspace`, frontend clean install/check/build, no newly introduced warnings in earlier PRs and zero current Svelte warnings by this PR, completed hardware matrix below, and a reviewed release evidence record. Prior audit totals (132 Rust tests; 40 Svelte warnings) are historical baseline numbers, not results for future changes.

## Hardware regression and rollback gate

Run only during implementation/release validation, with one hardware-controlling application active. A scanner or RGB test must not compete with the running GUI/service. This planning task does not run those tools.

| Scenario | Required observation |
|---|---|
| Baseline and candidate at same config/topology | Same physical device IDs, effective counts, first/last LED markers, full strip coverage, neighbor boundaries, brightness and colors. |
| RGB on/off and verified FPS settings | Fan cadence/ramp stays correct; no new flicker, shifts or USB error pattern. Preserve existing RGB-off semantics unless changed in a separate specified fix. |
| Fan edit, rename/name-clear, preset and lighting save | No addressing change, preserved overrides, settings survive reopening/restart; unaffected controllers continue smoothly. |
| Quit/relaunch | Restoration completes or reports its bounded failure; addressing remains correct after initialization. |
| Hub unavailable at startup / controlled reconnect / resume | Available cooling continues, reappearing device resolves by ID, unaffected hub retains mapping, counts and layouts reconcile together. |
| Sensor unavailable/stale | Defined cooling fallback/emergency latch; no unrelated pump slowdown; RGB worker remains responsive within measured I/O limits. |
| Mixed operation soak | At least 30 minutes on representative effects and cooling modes; record frame/control timing, I/O failures, topology events and config revision. This is regression evidence, not a universal reliability guarantee. |

Do not unplug a running pump or disturb cooling power to simulate USB loss. Prefer injected transport failures; reserve physical data-link tests for a controlled setup that retains cooling. Record topology and relevant logs before/after each hardware gate. The existing identify command alone is insufficient: pair it with deterministic per-LED markers and compare the returned ID/channel legend rather than assuming palette positions are stable.

**Stop/revert criteria:** unexpected channel offset, strip tail dark, color sent to the wrong hub/device, changed effective count without a deliberate topology/config change, new sustained flicker, or failure to maintain the cooling schedule within the measured transaction budget. Revert the individual candidate PR/build and restore its matching saved config; preserve logs and fixtures for diagnosis. Do not patch around a failure by guessing LED counts or bulk-regenerating expected outputs.

## Completion checklist

- Audits 1–9 have the tests and behavior specified in PRs 1–7; supporting validation/CI/documentation is covered by PRs 0, 8–9.
- All protected RGB invariants have fixed expected-byte tests; recording tests cover actual bytes/command order, not just lengths.
- Working historical LED-table behavior, configuration overrides and V1/V2 identity compatibility remain covered.
- Timing/lifecycle/topology PRs have baseline-versus-candidate physical evidence before release. Unperformed hardware checks remain explicitly pending.
- No protocol rewrite, RGB power redesign, schema migration, new platform, or feature-phase work is hidden inside the fixes.
- Each PR records affected audit IDs, validation results, RGB exposure, required hardware gate status and rollback commit/config compatibility.

Original implementation order: 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9. **Superseded** by the amended order below; each step must still leave a buildable, testable revision.

## Amendments from the gap analysis

Source: `docs/plan-gap-analysis-2026-10-04.md`. Each gap's full rationale and acceptance criteria live there; this section assigns them to PRs and records the decisions the plan now adopts.

### Gap assignment

| Gap | Priority | Folded into | Adopted requirement (summary) |
|---|---|---|---|
| G01 Count confidence across reconnect | P1 | PR 0, 6, 7 | Keep LED-table query status/provenance separate from effective counts. A failed query never silently produces a default-length (21-LED) frame; suspend RGB on that hub until a verified count exists. |
| G02 Validate topology before indexing | P1 | PR 0, 6, 7 | Validate raw enumeration (truncation, duplicate IDs/locations, cross-hub moves) before registry build, migration or publication. Any parser change is a separately justified protected-code diff with byte-equivalence tests. |
| G03 Migration vs override precedence | P1 | Early RGB prerequisite (before PR 5) | Migration preserves the currently effective V2 count, or defers with a visible diagnostic. Never silently pick the legacy V1 count. Add an expected-failure regression for the current conflict first. |
| G04 Incomplete colour transaction | P1 | PR 0, 4, 6 | On a failed chunk, stop the remaining chunks and suspend RGB for that hub until a hardware-verified resync sequence exists. Backoff alone is not recovery. |
| G05 Production-path marker diagnostic | P2 | PR 0 | Diagnostic mode through the real hardware owner/registry/sender with an exported slot manifest; required before any hardware acceptance. |
| G06 File preservation / invalid startup config | P1 | Early config prerequisite | `write_atomic` must never delete the target (**done 17 Oct 2026**: delete-first fallback replaced by a bounded rename retry, with regression tests). Still open: invalid existing config must be preserved and surfaced, never silently replaced by defaults. |
| G07 Manual commands through safety policy | P1 | PR 3 | One output arbiter for scheduled, manual, preset, startup and recovery writes. No manual API can undercut an emergency latch or a pump floor. |
| G08 Sensor/controller state contract | P1 | PR 1, 3 | Written state table (initializing, fresh, stale, absent, invalid, emergency-latched, recovering) with concrete thresholds. Track requested vs acknowledged duty per device. No silent substitution of a different WMI sensor. |
| G09 Budget all hardware-owner work | P1 | PR 4 and prerequisites | Inventory every blocking operation (saves, snapshots, PSU, recovery), bounded/coalesced command queue, numeric timing budgets. |
| G10 Startup / partial-init cleanup | P1 | PR 2, 6 | Every successful software-mode entry acquires a restoration responsibility, including on later startup failure; startup cancellable; no second owner of a hub. |
| G11 Preset intent with absent sensors | P1 | PR 3, 5, 6 | Separate durable preset/control intent from runtime fallback; a preset applied during an outage must recover to its curve when sources return. |
| G12 Dependencies and executable gates | P2 | Whole sequence | Revised order below; PR 0 owns a hardware-free integration harness (fake clock, recorded HID, fake sensors, injectable storage). |

### Decisions adopted

| Decision | Adopted direction |
|---|---|
| Count evidence becomes uncertain | Suspend affected RGB unless verified-unchanged topology justifies reusing counts; never downgrade to defaults silently. |
| Mixed override migration conflict | Preserve the effective V2 override, or visibly defer migration. |
| Partial-frame resynchronization | Only a hardware-verified reset/restart sequence; otherwise suspend that hub's RGB. |
| Safety precedence and manual lifetime | One arbiter; emergency cannot be overridden by direct IPC writes. |
| Required sources and emergency recovery | Every configured control source is required (proposed policy until validated on hardware); explicit aging and multi-source transitions. |
| Invalid config and interrupted save | Preserve original file and known-good evidence; separate first install from recovery. |
| Recovery deployment and rollback | PRs 6–7 ship and roll back as one coupled unit, or new recovery stays disabled until both are present. |

### Amended execution order

1. **PR 0** — baseline fixtures, G05 marker harness, integration harness, Windows PR CI. Classify tests as baseline characterization vs bug regressions (regressions may fail on the unfixed baseline).
2. **Config prerequisite** — G06 (atomic write: done; startup recovery policy: open) and the numeric/shape validation from PR 8 that PRs 3–5 consume.
3. **RGB prerequisite** — G02 raw topology criteria and G03 migration conflict test + correction.
4. **Cooling** — PRs 1, 2, 3 with G07, G08, G10.
5. **Scheduling** — PR 4 split into (a) sensor/cache ownership, (b) scheduling/queue/snapshot mechanics, (c) history retention; G09 budgets.
6. **Configuration** — PR 5 with G11.
7. **Recovery + RGB publication** — PRs 6–7 as one coupled release unit, including G01 and G04.
8. **Finish** — remaining PR 8 validation/UI errors, PR 9 warnings/docs, physical comparison and release evidence.

Separately from this fix train, a time-boxed hardware **safety spike** should establish (a) what the iCUE LINK hub does with fan duty when the host stops talking (iCUE ships explicit heartbeat/limited-software-mode logic; this app currently assumes speed reads act as keepalive) and (b) whether a hardware fallback fan setting can be stored on the hub. Its findings feed PR 2/G10 and PR 3; use a USB capture of iCUE plus a controlled process-kill test before any static reverse engineering.
