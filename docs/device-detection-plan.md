# Device detection rebuild: execution plan

Source analysis: [`icue-device-detection.md`](icue-device-detection.md). Decisions (Ken, 2026-10-25):

1. **Detect everything, control known.** A data-driven catalogue names every Corsair USB device and LINK sub-device
   type iCUE knows. Control stays limited to implemented, hardware-testable families (iCUE LINK hub, HX1500i/HID PSU).
   Unsupported devices show as "detected, no driver".
2. **Detection now, addressing gated.** The new detection layer reports topology events and diagnostics only. It does
   not change live fan/RGB addressing. Wiring rediscovery into addressing remains the PR 6+7 coupled unit of
   `fix-implementation-plan.md`.
3. **Stable identity.** LINK children are already keyed by the 13-byte `device_id` (`DeviceRegistry`, V2 config).
   Extend stable identity to USB devices (serial, else parent devnode instance id).
4. **Mirror iCUE's mechanisms** where they make detection robust (Ken: "do it the same way as Corsair, rock solid").

## Invariants (every task)

- No change to bytes on the wire for existing devices. RGB addressing is protected; the existing golden/recording
  tests must pass unchanged. Any parser change needs a byte-equivalence test.
- No new hardware writes. Detection is read-only (enumeration, descriptors, existing read commands).
- No `unwrap`/panic on device data; unknown values become `Unknown(..)` variants.
- `cargo build`, `cargo test --workspace` and `cargo clippy --workspace` stay green with no new warnings.
- Windows-only APIs go behind `#[cfg(windows)]`, with pure logic separated so it is unit-testable without hardware.

## Branching

Integration branch `feat/device-detection` from `main`. Each task is its own branch off it, in its own Orca worktree;
the coordinator merges finished tasks into the integration branch. The main checkout is not touched while
`tauri dev` may be running there.

## Wave 1 (parallel)

| Task | Owns | Result |
|---|---|---|
| T1 catalog | `crates/common/src/catalog.rs` (new), `types.rs` | `DeviceDef` table: all Corsair PIDs (iCUE/clink tables, OpenLinkHub, liquidctl), with family, protocol, data interface, driver support status. `LinkSubdeviceDef` table: u16 corsairDeviceId → category/model (C-2.3). `CorsairDevice::from_pid`/`name`/`pid` API kept compatible. |
| T2 hotplug | `crates/hid/src/hotplug.rs` (new) | iCUE-style notifier thread: hidden window with `RegisterDeviceNotificationW` (HID + USB device interface GUIDs) and `WM_POWERBROADCAST` suspend/resume; arrival queue drained after `CMP_WaitNoPendingInstallEvents` plus a devnode problem check; per-handle `DBT_DEVTYP_HANDLE` registration (query-remove/remove-complete). Events go out on a channel. Clean shutdown. |
| T3 enumeration | `crates/hid/src/discovery.rs`, new `crates/hid/src/topology.rs` | Enriched enumeration: per interface, instance id + parent instance id + enumerator name (CM APIs from the hidapi path). USB-only transport filter, devnode problem filter. `DeviceKey` identity (serial, else parent instance). Pure `Snapshot` + `diff()` → `TopologyDiff`. Flood guard and 2-minute failure-ignore window as pure, tested logic. Existing `scan`/`scan_grouped`/`open_device` keep working. |
| T4 LINK robustness | `crates/hid/src/icue_link.rs` | Channel sanity cap 20 → 24, with a byte-equivalence test showing ≤20-channel tables parse identically. Pure `diff_subdevices(old, new)` keyed by `device_id`. Raw type/model byte captured in `LinkDevice` (diagnostic only, addressing unchanged). |

## Wave 2 (after wave 1 is merged)

| Task | Owns | Result |
|---|---|---|
| T5 monitor | `crates/hid/src/monitor.rs` (new), wiring in `apps/gui/src/hardware_thread.rs` and the console path | `DeviceMonitor`: hotplug events → restartable settle (750 ms) → enumerate → diff → `TopologyEvent`s. Resume triggers a forced rescan. Periodic LINK sub-device re-enumeration every 30 s, only when idle, with a diff. Events are **logged and published to the GUI as diagnostics only**; recovery and addressing stay on the existing paths. |
| T6 GUI | Tauri command/event + devices view | Device list shows all detected Corsair devices with catalogue names and support status, plus topology events (added/removed/missing). Minimal UI, existing design language. |

## Wave 3

- T7 review (opus): adversarial review of the integrated diff against the invariants. Fixes are dispatched back to sonnet workers.
- Hardware checklist for Ken: plug/unplug of a non-cooling Corsair USB device, sleep/resume, start with iCUE services
  running, and confirm unchanged fan/RGB behaviour (marker check per `hardware-baseline.md`).

## Status (2026-10-25)

T1–T6 are merged on `feat/device-detection`. The T7 review found 0 critical, 0 high, 1 medium and 11 low. The medium
and the lows were fixed in T8a/T8b, except two that are deferred to PR 6/7:

- L2: releasing our own hidapi handles on query-remove (`watch_path` is ready but unwired; its registration race is fixed).
- L4: resume rescan after PnP idle. A 10 s hold-back of resume removals is in place.

`cargo test --workspace` passes, there are no clippy warnings on lines this branch changed, and the frontend check
and build are clean. What changed:

- Detection is event-driven and Corsair-only (VID filter): settle (750 ms quiet, 5 s cap), then snapshot, then diff.
- Resume, flood guard and readiness are handled.
- A catalog of 226 USB PIDs and 39 LINK sub-device ids drives naming and support level.
- Identity is the serial, else the composite USB instance id.
- The LED-table cap is 24, with legacy-oracle equivalence tests.
- `get_speeds` rejects stale-mode payloads. It retries once, and an error is only logged, as in OpenLinkHub.
- A 30 s LINK sub-device check reports changes after 2 confirmations.
- The GUI lists all Corsair devices and shows connect/disconnect notices.
- All of it is diagnostics only: fan/RGB addressing, registry, recovery and config are unchanged.

### Hardware acceptance checklist (run before merging to `main`)

Run with only this app controlling the hardware (iCUE closed).

1. **Baseline:** start the GUI. The Devices tab lists both hubs and the HX1500i as Supported, and the log shows
   `Initial Corsair device snapshot`.
2. **RGB/fan regression:** run the G05 LED marker diagnostic and compare it with `docs/hardware-baseline.md`. It must
   match exactly. Fan RPMs must be plausible on every channel.
3. **Periodic LINK check soak:** RGB on, at least 5 minutes (10 check rounds). Search the log for
   `wrong data type` (at most an occasional retry warning) and for the `LINK sub-device` diff lines (none expected).
   Repeat the marker check afterwards.
4. **Hotplug, non-Corsair:** plug and unplug a USB stick or phone. No rescan should be logged (VID filter) and RGB
   must not hitch.
5. **Hotplug, Corsair non-cooling device (if available, e.g. a peripheral):** plug and unplug it. You should see a
   "connected"/"disconnected" notice, a Detected-no-driver badge, and the rescan duration logged (warn if > 50 ms).
6. **Sleep/resume:** suspend for 1 minute and resume. You should see suspending/resumed notices, no false
   "disconnected" for the hubs, and fans and RGB recover through the existing recovery path. Repeat the marker check.
7. **Do not** unplug a hub or the pump during these tests (see the fix plan's hardware gate).
