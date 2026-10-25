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
