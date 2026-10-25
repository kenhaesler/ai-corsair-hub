# How iCUE 5 detects devices, and what ai-corsair-hub should copy

Date: 2026-10-25 · Target: iCUE 5.42.70 (`C:\Program Files\Corsair\Corsair iCUE5 Software`)
Method: static reverse engineering with [REA](https://github.com/morluto/rea) 6.7.0 + Ghidra 12.1.4
(native modules) and REA managed inspection + ilspycmd (.NET `clink`). Nothing was executed from
the iCUE install and nothing was sent to USB devices. Five parallel analyses; their full,
evidence-cited reports are in [`research/icue-device-detection/`](research/icue-device-detection/):

| Report | Scope |
|---|---|
| [A-enumeration](research/icue-device-detection/A-enumeration.md) | HidEnumerator, UsbDeviceListing, DeviceListing, WmiDeviceListing, DeviceListerService |
| [B-manifests](research/icue-device-detection/B-manifests.md) | `devices/*.rcc` manifests, ManifestRegistry/Builder, UniqueDeviceIds |
| [C-lifecycle-link](research/icue-device-detection/C-lifecycle-link.md) | DeviceManager, Bragi/LINK sub-device discovery, type-id table |
| [D-clink-service](research/icue-device-detection/D-clink-service.md) | .NET Corsair Link stack (PSU path), Device Control Service |
| [E-orchestration](research/icue-device-detection/E-orchestration.md) | Plugin loading, provider aggregation, rescan triggers, sleep/resume |

Claims below are tagged **[O]** observed in the binary/data, **[I]** inference. See the reports for addresses
and REA evidence ids.

## 1. The iCUE detection pipeline

```
 OS PnP ──► HidEnumerator (SetupAPI walk + hidden window RegisterDeviceNotification)
             │  arrival queued → wait CMP_WaitNoPendingInstallEvents → check CM_Get_DevNode_Status → open
             ▼
 DeviceListingProvider plugins (Bragi/USB, WMI, LLAccess-DRAM, BT, audio)  — report *ids only*
             │  settle timer per device, snapshot diff, notify only on change ("Same devices")
             ▼
 DeviceLister (aggregate, errors attributed per provider, VID/PID filter)  [may run in CorsairDeviceListerService.exe]
             ▼
 DeviceManager: DeviceKey {Usb vid/pid | CorsairDeviceId} → manifest (.cuebmf JSON) → device object
             ▼
 Per-device controllers; LINK hub: LinkEcosystemSubdevicesController → StartSubdeviceDetection,
 hub notifications → re-read connected-subdevice bitmap; 30 s ping, missed ping → sub-device removed
```

Key mechanisms:

1. **Enumeration is SetupAPI, not hidapi** [O, A-F1]. `SetupDiGetClassDevsW(GUID, DIGCF_PRESENT|DIGCF_DEVICEINTERFACE)`,
   then `CM_Get_Parent` to record the composite parent per interface. It also records the enumerator name
   and accepts only `usb|hid|hdaudio|pci` (BLE rejected). Identity per interface is
   `{path, transport, instanceId, parentId, vid, pid}`. The serial is read later from the open handle.
2. **Hotplug is event-driven with a PnP-quiescence debounce** [O, A-F3]:
   - A hidden window registers `RegisterDeviceNotificationW(DBT_DEVTYP_DEVICEINTERFACE)`.
   - On arrival it queues the path, and a worker thread loops `CMP_WaitNoPendingInstallEvents(1000)` until
     driver install is idle, then drains the queue.
   - Before `CreateFile` it rejects devnodes whose `CM_Get_DevNode_Status` reports a problem.
   - Every opened handle gets a `DBT_DEVTYP_HANDLE` registration: QUERYREMOVE closes the handle at once,
     QUERYREMOVEFAILED reopens it, and REMOVECOMPLETE drops it.
   - The listing layer adds a restartable settle QTimer plus a snapshot diff [O, A-F3, E-2.3].
3. **Device-to-model matching is data-driven** [O, B]:
   - Manifests are JSON (`*.cuebmf` inside Qt `.rcc`) carrying `DeviceKeys`. The first `DeviceKey` equality match wins.
   - Missing properties come from a fallback manifest.
   - LINK children are matched by `CorsairDeviceId` (sub-device type), not by USB id.
4. **Identity is persistent** [O, B-2.4, C-2.4]:
   - Each device gets a random GUID, persisted and keyed by (USB key, serial, else PairingId).
   - LINK children are persisted per hub as `DetectedSubdevices = {type → [13-byte ids in chain order]}`.
5. **LINK sub-devices are hub-driven** [O strings / I flow, C-2.2]:
   - Detection starts with `StartSubdeviceDetection`.
   - Hub notifications trigger a re-read of the connected-subdevice bitmap and info.
   - A per-sub-device ping runs at `RequiredPingInterval = 30000` ms; a missed countdown removes that sub-device.
   - An IO error on the hub removes the main sub-device, and with it all children.
   - Hub limits: 2 ports × 12 = 24 channels, 476 LEDs max.
6. **The HX1500i is not handled by native iCUE**: it goes through the legacy .NET Corsair Link 4 service (`clink`) [O, D]:
   - Detection: WMI `Win32_PnPEntity WHERE DeviceID LIKE 'hid%'` plus a PID table, with `RegisterDeviceNotification`
     and a 250 ms throttle before re-enumerating.
   - Handles are opened shared (`FILE_SHARE_READ|WRITE`) per command, with 500 ms read/write timeouts and 5 open retries.
   - Devices that fail are ignored for 2 minutes.
   - Products divide devices between them through a registry PID partition (`HKLM\SOFTWARE\Corsair\Corsair Utility Engine\CLinkPids`).
7. **What iCUE has that we don't need**: the lister service + Qt RemoteObjects IPC, the package-manager
   download stages, and the Device Control Service. The DCS is a DRAM/SMBus lighting arbiter, not a HID broker [O, D-2.6].
   These exist for a large catalogue and SDK clients; a fixed-hardware app gets nothing from them.

## 2. Gaps in ai-corsair-hub (verified against the code)

| # | Gap | Where |
|---|---|---|
| G1 | No hotplug: `DeviceScanner::refresh()` runs only inside `RealHubBackend::reopen()` after a failure | `apps/gui/src/hardware_thread.rs:903`, `crates/hid/src/discovery.rs:18` |
| G2 | No settle/PnP-idle wait or devnode problem check before opening | `discovery.rs` |
| G3 | Identity is `pid:serial` with an `"unknown"` serial fallback, so two serial-less devices collide; no parent/container id and no persistent id | `discovery.rs:38` |
| G4 | No resume/session handling; recovery waits for a write to fail | hardware thread |
| G5 | LINK channel sanity cap is 20; iCUE's hub supports 24 | `crates/hid/src/icue_link.rs:615` |
| G6 | Sub-device changes (fan added/removed) are only seen at `initialize()`; a dead sub-device keeps being written | `icue_link.rs`, `control_loop.rs` |
| G7 | Small PID table in code, no transport filter (a BLE/BT HID with VID 1B1C would pass) | `crates/common/src/types.rs` |
| G8 | HID write has no timeout watchdog (read is 500 ms) | `crates/hid` |
| G9 | LS350 default of 21 LEDs; iCUE decodes strips from the adapter's LED total (40 per LS350, 80 = 2 strips) | `icue_link.rs:185`, `crates/rgb/src/layout.rs:17` |
| G10 | LINK type byte decoded as one byte; iCUE ids go up to 0x81A (variants) and 0x0A is "Fan" in iCUE vs our `GpuBlock` | `icue_link.rs:83-107` |

## 3. Recommended design (prioritised)

**P0: Hotplug + debounce + diff** (fixes G1, G2; fits the hotplug slot "PR 6-7" in `fix-implementation-plan.md`)
- New `crates/hid/src/hotplug.rs`:
  - Register `CM_Register_Notification` (`windows` crate, `CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE`, HID class GUID from
    `HidD_GetHidGuid`). The callback only forwards `{Arrived|Removed, path}` into a channel. This needs no window;
    iCUE's message-window approach is equivalent.
  - Settle with a restartable ~750 ms timer (tunable; iCUE's constant was not recovered). Before opening, poll
    `CM_Locate_DevNodeW` + `CM_Get_DevNode_Status` until there is no problem code and `DN_STARTED` is set, with a 5 s cap.
    This is the documented replacement for iCUE's `CMP_WaitNoPendingInstallEvents`.
  - Then run `refresh()`, build a `BTreeMap<DeviceKey, DeviceInfo>`, diff it against the previous snapshot, and send
    `HwCommand::DevicesChanged{added, removed}` to the hardware thread. If nothing changed, do nothing.
- Per-handle removal: `CM_Register_Notification(CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE)` on each opened hub/PSU handle.
  On QUERYREMOVE drop the handle; on REMOVECOMPLETE mark the device gone. The hub then enters the existing recovery path.

**P0: Resume handling** (G4). `RegisterSuspendResumeNotification` or `WM_POWERBROADCAST`:
- On suspend, quiesce writes.
- On `PBT_APMRESUMEAUTOMATIC`, mark hubs stale, wait for the hotplug settle, then reopen and re-`initialize` (including
  the colour-pipeline SLEEP→re-init recovery documented in G04).

**P1: Identity** (G3)
- Use `DeviceKey { vid, pid, serial: Option<String>, parent_instance_id }`. Use the serial when present; otherwise fall
  back to the parent devnode instance id (`CM_Get_Parent` + `CM_Get_Device_IDW`). Group interfaces by parent.
- Persist a generated UUID per key in config, mirroring `UniqueDeviceIds::cachedDeviceId`.
- LINK children: key by `(hub serial, type, 13-byte device_id)`, not channel index. Persist the last-seen list per hub
  so the UI can say "fan X missing".

**P1: LINK robustness** (G5, G6)
- Raise the channel cap to 24 (2 × `MaxSubdevicesOnSide`).
- When idle, re-enumerate sub-devices every 30 s (iCUE's ping interval) and diff by stable key.
- Mark a sub-device absent after N cycles without status/RPM while the hub is healthy, and stop writing to it.
- The hub's own change notification (`0x3b`) and the ping opcode are still unknown. They need a USB capture before we
  can be event-driven at the hub level.

**P1: Data-driven device table** (G7)
- Use `static DEVICE_DEFS: &[DeviceDef { vid, pid, proto, data_interface, caps }]` with only 0x0C3F and 0x1C1F active,
  plus the PSU PID family from D-2.3 for names.
- Accept only USB-enumerated devices.
- Unknown PIDs become `Unsupported`: logged, never opened.

**P2: Defensive IO** (G8): bounded write timeout (deadline thread → drop handle and reopen); up to 5 open retries;
2-minute ignore window for a path that fails init; a flood guard that quarantines a device re-arriving more than N
times in T (iCUE DataMiningUsb).

**P2: Co-existence**: at startup, read the state of the `CorsairService` / `CorsairDeviceListerService` services and of
iCUE.exe (read-only). If they are running, warn in the UI that the hardware is shared. All four Corsair services are
Disabled on this PC today.

**P3, needs hardware evidence first** (RGB addressing is protected):
- G9: LS350 sizing from the adapter total.
- G10: is the "model" byte the high byte of `corsairDeviceId`? Log the raw bytes for all 10 QX fans and the adapter first.
- PSU framing: clink uses `[0, addr<<1, cmd]` with a 0xFE handshake before every command, ours uses liquidctl's
  `[0, len, cmd]`. Ours works on this HX1500i, so don't change it.

## 4. Open questions (need capture or hardware)

- Bragi opcodes for `StartSubdeviceDetection`, `GetConnectedSubdevicesBitmap`, ping, and notification id `0x3b`.
- Ping miss count before removal; whether there is a bitmap poll period.
- Which interface GUIDs HidEnumerator registers (HID class assumed).
- Hub fallback on host loss: `LimitedSoftwareMode` / `SupportsSelfOperatedModeOnly` are the leads.
- iCUE.dll (37 MB) timed out in Ghidra at 330 s. Rerun with `REA_GHIDRA_STARTUP_TIMEOUT_MS=1800000` if aggregator
  priority between providers matters.

## 5. Reproducing

```powershell
npm install --global rea-agents            # 6.7.0
winget install Microsoft.OpenJDK.21        # Ghidra 12.1.4 extracted to C:\tools\ghidra_12.1.4_PUBLIC
$env:GHIDRA_INSTALL_DIR="C:\tools\ghidra_12.1.4_PUBLIC"; $env:JAVA_HOME="C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
rea doctor --provider ghidra --json
rea search "<iCUE>\modules\UsbDeviceListing.dll" "VID" --provider ghidra --json
```
