# Gap analysis of the hub fix implementation plan

Reviewed on 4 October 2026 against local commit `c32a0bd`, `docs/fix-implementation-plan.md`, and the current source. The checkout is still at the plan's original implementation baseline. The plan and initial audit remain untracked repository documents.

**Conclusion:** the plan covers all nine original findings and identifies the right RGB invariants, but several failure paths and acceptance decisions remain incomplete. The most significant gaps concern loss of LED-count evidence during reconnect, topology validation before indexing, migration changing override precedence, and config persistence that can delete the working file. Address these before treating the relevant PRs as ready for implementation and release.

This review adds twelve actionable gaps. “P1” means resolve before implementing or accepting the affected reliability change; “P2” means an execution or verification prerequisite. These are planning gaps, not a claim that every described hardware failure has been reproduced. No application settings, hardware state, or production source were changed. The original plan is preserved; the amendments below are proposals.

## What the plan already covers well

- Stable device IDs, per-hub channel order, full-device black padding, firmware-zero fallback, explicit override precedence, and the historical channel-15 80-LED case are explicit contracts.
- Byte comparisons, single HID ownership, physical comparison, and rollback are required. This is stronger than relying only on unit tests or a visual preview.
- Sensor timeouts, coordinated exit, PID filtering, fallback policy, separate schedules, coherent config, controller-state preservation and discovery all have assigned work.
- Protocol rewrites and new product features are kept outside the fix sequence.

The gaps below qualify those strengths. They do not justify replacing the working RGB protocol or broadly rewriting the application.

## Coverage of the original audit

| Original issue | Existing plan coverage | Remaining gaps in this review |
|---|---|---|
| 1 RGB changes control cadence | PR 4 | G09 blocking-work budget and command semantics; G12 sequencing |
| 2 CPU HTTP requests can hang | PR 1, then PR 4 | G08 source validity; G10 startup waiting outside HTTP deadlines |
| 3 Quit races restoration | PR 2 | G10 partial initialization and lifecycle ownership |
| 4 Hysteresis suppresses PID | PR 3 | G07 shared command arbitration; G08 explicit state and timing rules |
| 5 Configuration copies disagree | PR 5 | G03 migration precedence; G06 file preservation; G11 preset intent |
| 6 Stale fallback can reduce duty | PR 3 | G07 manual bypass; G08 source and duty semantics; G11 missing-source presets |
| 7 Incomplete discovery and recovery | PR 6 | G01 uncertain counts; G02 incomplete identity data; G04 partial RGB writes; G10 cleanup |
| 8 Metadata edits reset control | PR 5 | G08 requested versus acknowledged duty; G12 validation ordering |
| 9 Effective LED metadata and renderer diverge | PR 7 | G01 to G05 and the PR 6 publication gate in G12 |

## RGB gaps

### G01 Preserve count confidence across reconnect

**P1, PRs 0, 6 and 7.** The plan preserves count precedence and the 80-LED case, but does not distinguish a successfully reported zero from a failed LED-table query. `IcueLinkHub::initialize` replaces a failed query with an empty map; adapter defaults are 21 LEDs (`crates/hid/src/icue_link.rs:184,730`). A reconnect can therefore produce internally consistent metadata for 21 LEDs even though the previously verified physical chain still has 80. Atomic publication alone does not make that layout correct.

**Amendment:** retain query status and provenance independently of effective counts. Define when a verified previous count may be reused and when RGB on the affected hub must remain suspended pending reliable enumeration. Do not reuse old locations blindly after a possible physical topology change. Keep cooling and independently healthy hubs operating.

**Acceptance:** exercise verified 80 → failed query, non-OK query, successful zero table, successful partial table, and genuine topology change as separate cases. A failed query must not silently emit a 21-LED replacement frame. A later verified result must restore the expected offsets. Preserve the successful-path protocol and explicit override semantics.

### G02 Validate complete topology before building indexes or migrating

**P1, PRs 0, 6 and 7.** The plan says to reject ambiguity, but does not specify where it can still be detected. The device parser stops at a truncated entry or ID and returns the prefix as success (`crates/hid/src/icue_link.rs:854,868,883`). `DeviceRegistry::rebuild` overwrites duplicate IDs/locations while constructing maps (`crates/common/src/identity.rs:90`). Startup migration then consumes the constructed registry (`apps/gui/src/hardware_thread.rs:118`). Checking the final index is too late to discover evidence already discarded.

**Amendment:** validate raw enumeration completeness, identities and locations before index construction, migration, or generation publication. Include stale metadata from a disconnected hub when evaluating a device that appears on another hub. Preserve legitimate empty-channel entries. Any necessary parser correction is a small, separately justified exception to the protected-code rule, with byte-equivalence tests for valid responses.

**Acceptance:** truncated final entry/ID, duplicate ID across hubs, duplicate location, and cross-hub moves with stale old metadata cannot publish a guessed mapping or persist a migration. Independent valid hubs continue operating. No malformed input is accepted merely because parsing produced some devices.

### G03 Resolve the migration and override precedence contradiction

**P1, PRs 0 and 5, before startup migration is trusted.** The sender prefers the V2 count over the V1 override (`crates/fancontrol/src/control_loop.rs:1046`). Migration instead replaces an existing V2 entry's count with the V1 override (`crates/common/src/config_migration.rs:179`). For a mixed V1 document with V2 count 80 and legacy count 21, the effective wire length can change during migration. “Preserve migration behavior” and “preserve V2 precedence” cannot both hold for that input.

**Amendment:** explicitly preserve the existing effective V2 count, or defer a conflicting migration with a visible diagnostic. Prefer preserving effective behavior; never silently choose the legacy count. Isolate this correction from other identity work, without adding a new schema version.

**Acceptance:** full RGB-byte equality before migration, after migration and after restart for mixed overrides, explicit zero, device names and unresolved devices. Deferred/failed migration preserves the original file. Add an expected-failure regression for the current conflict rather than changing a baseline fixture to accept the regression.

### G04 Specify recovery from an incomplete color transaction

**P1, PRs 0, 4 and 6.** The plan tests successful chunk ordering and calls for timeout/backoff handling. It does not define the endpoint state after a failed chunk. `write_color` exits immediately on transfer error (`crates/hid/src/icue_link.rs:476`); the open flag remains true, and the next `set_rgb` skips reopening (`:486`). Whether the next first-chunk command resynchronizes the firmware is not established here.

**Amendment:** define a partial-frame state and an evidence-based retry/resynchronization policy. Until a recovery sequence is verified, suspend RGB for that hub and surface the failure instead of repeatedly assuming a valid endpoint. Backoff alone is not state recovery. Keep normal successful bytes and ordering unchanged.

**Acceptance:** scripted errors at endpoint open, first/middle/final chunks, and delayed/stale responses. Stop remaining continuation chunks on failure; service cooling; prove the next accepted frame is fully delimited and correct. Any new reset/reopen sequence requires controlled hardware evidence. Include malformed responses and write-length handling in the transport test contract; do not infer correctness solely from an `Ok` result.

### G05 Deliver a marker diagnostic that uses production addressing

**P2, PR 0, required before hardware acceptance.** The plan correctly says identification alone is insufficient, but does not assign implementation of the missing diagnostic. Identification recolors only frames generated by existing zones (`apps/gui/src/hardware_thread.rs:559,576`). The LED-walk utility sizes neighbors by device defaults, sizes its target from a CLI maximum, then calls the hub directly (`apps/rgb-test/src/main.rs:233,250,263`). That utility is useful for exploration but cannot prove the production config/registry/effective-count path remains correct.

**Amendment:** assign a bounded diagnostic mode through the existing hardware owner, stable IDs, effective metadata and production sender. Export an expected slot manifest: hub, device ID, channel, count/source, start/end offset. Diagnostic frames must cover unassigned devices too and must not save temporary effects to config.

**Acceptance:** distinct per-LED markers at every device boundary and both strip ends reach the expected physical LEDs; diagnostic exit/timeout restores prior desired state. Compare the same manifest and marker inputs between baseline and candidate. The baseline executable's identity and physical result must be recorded, not inferred from a source comment.

## Cooling and lifecycle gaps

### G06 Make file preservation and invalid startup config explicit

**P1, before PR 5; also affects PR 8.** PR 5 relies on atomic persistence. However, `write_atomic` removes the destination after `PermissionDenied`, retries rename, then removes the temporary file if that retry fails (`crates/common/src/atomic_write.rs:58`). This creates a file-loss window despite the helper's preservation claim. At startup, invalid config falls through to a legacy file or defaults (`apps/gui/src/lib.rs:273`); defaults have no fan groups (`crates/common/src/config.rs:246`). Stricter validation can therefore turn a previously loadable config into a silent change of operating configuration, including lost RGB overrides.

**Amendment:** remove the delete-first fallback; failure must leave the original bytes intact. Define first install separately from corrupt/invalid existing config. Preserve invalid files and show a recovery state; use a known-good backup only under an explicit, documented selection rule. Never automatically overwrite the failed original with defaults. Define crash behavior between durable save and in-memory commit and identify the committed revision on restart.

**Acceptance:** injected failure before replacement, after the first rename failure, and at the second failure; the original remains readable and unchanged. Test sharing violations, interrupted save, invalid primary plus valid legacy config, and no valid config. Verify LED overrides and fan groups survive every recoverable path. Current tests do not prove this guarantee.

### G07 Route manual commands through the same safety policy

**P1, PR 3, before the scheduling change.** Emergency and device minimums are applied inside `tick`, but `set_manual_duty` writes the requested value directly (`crates/fancontrol/src/control_loop.rs:776`). Both manual IPC commands accept zero (`apps/gui/src/commands.rs:134,164`), and the ID-based path reaches the same direct write (`apps/gui/src/hardware_thread.rs:1124`). The proposed emergency latch can therefore be overridden between scheduled ticks. Decoupling cooling from fast RGB ticks may lengthen that interval.

**Amendment:** define one output arbiter for scheduled, manual, preset, startup, and recovery commands. Specify precedence among emergency, degraded-sensor policy, device minimums and user intent. Specify whether manual duty persists, expires, or lasts one cycle. Cover ungrouped devices and deletion of the last group: define who controls those channels after ownership changes. Whole-hub hardware-mode restoration also closes the color endpoint (`crates/hid/src/icue_link.rs:426`), so it cannot be the unexamined response to removing one cooling group while other fans or RGB still use that hub.

**Acceptance:** neither manual API can reduce a latched emergency or send a pump below its defined floor; unknown/non-speed-capable targets fail explicitly; removing one group or the final cooling group preserves the defined cooling and RGB ownership policy; manual timeout, partial multi-hub failure and preset interaction are deterministic. Verify the actual transport commands, not just computed group duty.

### G08 Finish the sensor and controller state contract

**P1, PRs 1 and 3.** The plan says to define required sources, recovery thresholds/durations and last applied duty, but leaves those choices open. Config currently contains only sensor names and weights (`crates/common/src/config.rs:179`), and `group.last_duty` is updated before the write succeeds (`crates/fancontrol/src/control_loop.rs:577,596`). In addition, the WMI backend substitutes the first temperature sensor if the selected sensor disappears (`crates/sensors/src/cpu.rs:267`). A plausible reading from another component can consequently refresh CPU health and could satisfy a future emergency-clear rule.

**Amendment:** write a state table covering initializing, fresh, stale, absent, invalid, emergency-latched and recovering. Define source identity/provenance, the timestamp used for freshness, source replacement rules, required-source semantics without an unnecessary schema change, and sustained recovery criteria. A reasonable starting rule is that every configured control source is required; label this as a proposed policy until adopted. Track requested and last acknowledged duty separately per device, including manual changes and different pump/fan floors: a group spanning two hubs can have partial write success. A successful write acknowledgment does not measure actual physical PWM/RPM.

**Acceptance:** selected CPU sensor disappears while a cooler non-CPU sensor remains; repeated cached results age rather than becoming fresh again; one of several emergency sources recovers while another remains hot/missing; hub writes fail; config changes during emergency; resume produces a long clock gap. Each transition must have an expected output, health state and recovery condition. Add concrete timeout/recovery values before accepting PR 3, supported by the intended hardware behavior.

### G09 Budget all work on the hardware owner

**P1, PR 4 and its prerequisites.** PR 4 moves sensor work and prioritizes cooling, but the budget focuses on RGB transfers. Current config saves perform synchronous write/fsync from the hardware command handler (`apps/gui/src/hardware_thread.rs:230,1391`; `crates/common/src/atomic_write.rs:47`). GetDevices and identify can read speeds synchronously through `hub_snapshots` (`crates/fancontrol/src/control_loop.rs:722`). Recovery initialization, PSU polling and speed-write retries also occupy the owner. Bounding the number of commands drained does not bound one slow command. The queue itself is an unbounded `mpsc::channel` (`hardware_thread.rs:51`).

**Amendment:** inventory every blocking operation, not just rendering. Separate prepared persistence from hardware scheduling with ordered commit acknowledgments; cache read-only device snapshots; define a bounded/coalesced command queue and completion semantics for superseded requests. Account for one PSU handle/provider per device and its transaction sequence when moving sensor ownership. Define what happens when an operation exceeds the budget: a timed-out caller must not imply that a mutating command was canceled if it later commits.

**Acceptance:** a deliberately blocked file save, command flood, device-tab polling, PSU timeout and hub recovery cannot create unbounded backlog or silently starve healthy cooling. Publish numerical limits for supported topology, normal control lateness, failure-mode lateness, queue capacity and shutdown duration. Measurements must include non-RGB failures. If an OS I/O call cannot be bounded, document the limit and choose explicit isolation/fallback rather than claiming timer-based protection.

### G10 Include startup and partial initialization in ownership cleanup

**P1, PR 2 and PR 6.** The exit plan handles a running worker, but the worker calls LHM startup before it processes commands (`apps/gui/src/hardware_thread.rs:73`). `launch_and_wait` blocks on the child before starting its 15-second readiness timer (`crates/sensors/src/lhm.rs:124,133`). HID initialization enters software mode before enumeration can fail (`crates/hid/src/icue_link.rs:727,742`). Later build errors can also abandon already initialized hubs. No general automatic hardware-restoration destructor was found in these paths.

**Amendment:** define acquisition/initialization/active/restoring/closed states. Every successful software-mode acquisition must acquire a cleanup responsibility even if a later step fails. Make startup cancellable independently of ordinary command draining. Cover startup failure, partial startup, recovery failure, normal exit, updater/restart and a worker panic. Keep forced-process-termination limitations explicit. If a restoration worker times out and remains alive, prevent a second live owner from taking over that same handle/device in-process.

**Acceptance:** quit while waiting for LHM/UAC; enumeration fails after entering software mode; first hub initializes and second fails; recovery initialization fails; shutdown races recovery. Record an acknowledged restoration attempt or a visible bounded failure for every acquired hub, with no overlapping owner. This requires lifecycle fault injection, not only a delayed successful quit test.

### G11 Preserve preset intent when sensors are temporarily absent

**P1, PRs 3, 5 and 6.** Reinitializing sensors does not necessarily restore the user's intended control mode. `apply_preset` removes unavailable sources and, when none are available, persists a fixed duty derived from the middle curve point (`apps/gui/src/hardware_thread.rs:916,944,981`). A later recovered CPU/GPU source will not automatically return that group to its intended curve or weighting. The plan's missing-source policy cannot operate on a source that was deleted from desired configuration.

**Amendment:** distinguish durable preset/control intent from temporary runtime fallback. Define behavior for permanently unsupported sensors versus transiently unavailable ones. Preserve the intended curve and required-source contract when applying a preset during an outage; report the degraded runtime state separately. Avoid using a preset tag alone as the durable definition, since users can modify the underlying mode.

**Acceptance:** apply a curve preset with all sources absent; regain one source, then all sources; compare desired config, effective mode, weights and fallback across restart. Recovery must follow the stated policy without silently remaining fixed. Genuine fixed-duty groups stay fixed.

## Execution gap

### G12 Revise dependencies and make acceptance gates executable

**P2, affects the whole sequence.** The plan ends with a strict PR 0–9 order, but essential config validation is in PR 8, after scheduling and config publication start consuming those values. PR 6 stages generations for PR 7 and explicitly cannot ship without it, conflicting with an uncomplicated “revert the individual PR” expectation. PR 4 also combines a scheduler change, new sensor ownership, HTTP caching, queue behavior, snapshot behavior and history retention, making failures harder to attribute.

**Amendments to the sequence:**

| Stage | Proposed change to the existing plan |
|---|---|
| Before production fixes | PR 0 fixtures plus G05 production marker harness; classify tests as baseline characterization versus bug regressions. Bug regressions need not pass the unfixed baseline. |
| Early config prerequisite | G06 preserve-old-file fix and startup recovery policy; move numeric/shape validation needed by consumers ahead of PRs 3–5. Preserve valid historical fixtures. |
| Early RGB prerequisite | G02 raw topology criteria and G03 migration conflict tests/correction, each narrowly reviewed where protected code changes are necessary. |
| Cooling | PRs 1–3 with G07 output arbitration, G08 state contract and G10 acquisition cleanup. |
| Scheduling | Split PR 4 into sensor/cache ownership, scheduling/queue/snapshot mechanics, and history retention. Each needs its own failure tests and byte-equivalence check. |
| Configuration | PR 5 with persistence prerequisite and G11 durable preset intent. |
| Recovery and RGB publication | Keep PRs 6–7 as a coupled release unit or keep new recovery behavior disabled until both are present. Include G01 and G04 policies. Roll back the coupled unit, not an arbitrary dependency in isolation. |
| Finish | Remaining validation/UI errors, warning cleanup, documentation, physical comparison and release evidence. |

Assign the integration test harness to PR 0: drive the real command/config/scheduler code with a fake clock, recorded HID, fake sensors, injectable storage and an event sink, without requiring hardware or a Tauri window. For cross-subsystem gates, assert command order and published config/topology generations, not only isolated helper results.

Give every acceptance run a baseline/candidate revision, exact test command and exit result, relevant artifact/manifest, hardware topology/firmware when applicable, and explicit pass/fail/pending status. Pin dependency resolution with the lockfiles. Prioritize a clean Windows checkout run so local build artifacts cannot conceal missing fixtures. Branch protection and automatic release eligibility should only be claimed if actually configured; the presence of a workflow file does not prove enforcement.

## Decisions needed before the affected PRs begin

These decisions are work for the implementation design; the gap analysis itself does not require changing hardware or asking the user to pick low-level constants.

| Decision | Recommended direction | Must be settled by |
|---|---|---|
| Count evidence becomes uncertain | Suspend affected RGB unless retention of verified counts is justified by verified unchanged topology; never silently downgrade to defaults | PR 6/7 design |
| Mixed override migration conflict | Preserve current effective V2 override or visibly defer migration | RGB baseline/migration prerequisite |
| Partial-frame resynchronization | Use only a verified reset/restart policy; otherwise suspend that hub's RGB | Before RGB retry changes |
| Safety precedence and manual lifetime | One arbiter; emergency cannot be overridden by direct IPC writes | PR 3 |
| Required sources and emergency recovery | Explicit source identity, aging and multi-source state transitions with chosen thresholds/durations | PR 3 |
| Timing guarantees | Numerical normal/failure budgets derived from supported topology and all blocking work | PR 4 acceptance |
| Invalid config and interrupted save | Preserve original file and known-good evidence; separate initialization from recovery | Config prerequisite |
| Recovery deployment and rollback | Coupled PR 6/7 release or a disabled-until-ready feature path | Before PR 6 merges into a deployable branch |

## Evidence and limits

The review inspected the current plan, config/migration/persistence, hardware worker, sensor backends, HID initialization and RGB transport, device registry, diagnostic utility, shutdown and update entry points. Independent reviews checked RGB and cooling/lifecycle paths. Source references above describe the current checkout, not changes already made.

No new builds or test suites were run for this document-only review. The earlier 132-test result and 40 frontend warnings are historical evidence, not a fresh verification on 4 October. No physical RGB mapping, firmware recovery sequence, timing bound, or pump operating range was revalidated. Those remain implementation/release evidence requirements, with explicit proposed closure tests above.

The analysis is complete as a review artifact. The implementation plan is not yet amended and the listed gaps are not fixed. The next concrete step is to incorporate the prerequisite corrections and state/acceptance decisions into the plan before starting the production fix sequence.
