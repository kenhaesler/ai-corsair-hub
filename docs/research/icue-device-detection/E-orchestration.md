# Task E — iCUE 5.42 discovery orchestration (provider registration, merge, rescan, sleep/resume)

Target install: `C:\Program Files\Corsair\Corsair iCUE5 Software` (v5.42, PDB path `Corsair_iCUE_RJ_v5.42`). Read-only analysis.
Work dir: `...\scratchpad\re\E\`. Tags: **[O]** observed in binary, **[I]** inference, **[U]** unknown.

## Method caveats (read first)
- `rea inspect-pe-imports` (named in BRIEF) does **not exist** in REA 6.7.0. Imports were not extracted from the PE import table; I used raw ASCII/UTF-16 string sweeps (`E\strings.ps1`, outputs `E\str_*.txt`) instead. API names below are therefore "strings present", not proof of import.
- Ghidra **timed out on iCUE.dll** (36.9 MB; `provider_timeout` at 330 s, `E\trace_suspend.json`). All iCUE.dll claims are string-adjacency based **[O strings / I wiring]**. No iCUE.dll xrefs/decompilation.
- Ghidra worked on the small modules (Bragi, LLAccess, DataMiningUsb) via `rea trace/decompile --snapshot`.
- Extra finding outside the target list but central to the question: `CorsairDeviceListerService.exe` (364,696 B) — string-analysed (`E\str_ListerSvc.txt`). Not obviously covered by other workers.

## 1. Summary
- iCUE is a **module/plugin system**, not hard-wired code: every module is a Qt plugin (`qt_plugin_instance`/`qt_plugin_query_metadata_v2`) whose metadata is just `IID="Module.core.cue.corsair.com"`, class `ExternalModule` **[O]** (QTMETADATA blob in Bragi/LLAccess/DataMiningUsb `.qtmetad`, e.g. Bragi file offset 496128). The *real* manifest is a C++ `cue::moddefhlp::ModuleDefinition` that lists **provided interface ids** and **required interface ids** **[O]**.
- Interface ids are reverse-DNS strings (`DeviceListingProvider.dev.cue.corsair.com`, `HidEnumerator.dev.cue.corsair.com`, `DeviceLister.dev.cue.corsair.com`, `DeviceManager.dev.cue.corsair.com`, …). Registration code (Bragi `FUN_180001040`, LLAccess `FUN_180001000`) constructs `cue::core::InterfaceId` from the string, fills a `{provided id, deps[], factory}` record and pushes it into a module-definition vector **[O]**.
- **Dependency graph differs per provider**: Bragi lists `HidEnumerator` as its dependency (dep array ptr/len = `&HidEnumerator id`, 1) **[O]**; LLAccess has an empty dependency array (talks to an SMBus/“LLAccess” driver directly) **[O]**. DataMiningUsb exports `DataMiningUsbEnumerator` and consumes `HidEnumerator` **[O strings]**.
- Detection is **split across processes**: a separate `CorsairDeviceListerService.exe` ("dls.service", `cue::dev::DeviceDetectorImpl`, `cue::dev::lister_svc::IpcServer`) runs the minimal providers (`DeviceLister`, `DeviceListingProvider`, `HidEnumerator`, `BragiDeviceListing`) and publishes `devices`/`devicesReady`/`enumerationCompleted` to iCUE over Qt Remote Objects IPC (`DeviceListerServiceIpc`, methods `startDetection`, `stopDetection`, `setUsbDevicesFilter`) **[O]**. iCUE.dll's `cue.dev.lister_watcher` consumes it ("Start detection through service", "Devices ready received from service") **[O]**.
- **Providers return only ids; they do not build device models.** `cue::dev::DeviceIds` lists (USB vid/pid) are all the lister reports (module description: "Provides USB device ids detection for connected devices") **[O]**. Model matching happens later via the **package manager**: stage1 (detect → search `*-device-listing` / `dev-*` / `devplugin-*` packages), stage2 (install, "Restart DeviceLister", "Packages installed. Load new modules") **[O strings, iCUE.dll]**. Unknown-ids ⇒ "ignoring detected device due to pending installation of required packages for listers".
- **Merge/dedup**: no single global “merge” function was found by decompilation (iCUE.dll not decompiled). Observed dedup at three levels: (a) each provider diffs its own snapshot and emits only on change (Bragi "Same devices" vs "Connected devices changed:"), (b) DataMiningUsb dedups by (path-key, vid, pid) and drops flooding devices, (c) iCUE later keys devices by a composite id string `vid<1b1c>pid<%1>serial<0>index<%2>` **[O string]**. Priority/ordering between providers: **[U]**.
- **Rescan triggers**: hotplug is event-driven via `HidEnumerator` (providers observe `HidEnumeratorObserver`), debounced with per-device settle timers + a QTimer "enumeration checker" **[O]**; also re-enumeration on device-initiated events (`WillReenumerateWithDifferentPid`, `WillReenumerateToApplyPairing`, `WillReenumerateToAdjustPollingRate` — device property names in Bragi) **[O]**; and a **full DeviceLister restart after package installation** ("Restart DeviceLister") **[O]**.
- **Sleep/resume / service restart**: iCUE has a `SessionWatcher.dev.cue.corsair.com` interface producing power-state and user-session transitions; consumers log "Transition from Suspended…" / "User session about to be locked/unlocked" (SDK server) and the device-SDK "Entering working state, because of transition from Suspended state" **[O strings]**. I could **not** prove the lister is stopped/restarted on suspend; the service has "Requesting service shutdown due to inactivity", "Service enumeration requested", "Service connection failed" **[O]**, suggesting watchdog-style reconnect **[I]**.

## 2. Findings with evidence

### 2.1 Module / provider loading and registration
| Claim | Evidence |
|---|---|
| Modules are Qt plugins, uniform metadata | `BragiDeviceListing.dll` file offset 0x79500 QTMETADATA: `Module.core.cue.corsair.com` + `ExternalModule`; same blob in LLAccess (@100864), DataMiningUsb (@132608). `DeviceExtensionsController.dll` strings `qt_plugin_instance`, `qt_plugin_query_metadata_v2` |
| Module class registry | RTTI `.?AVExternalModule@@`, `cue::moddefhlp::ModuleDefinition`, `cue::core::Module/Provider/Service/Interface` in Bragi & LLAccess; error string "Several implementation of the same interface in the same module not yet supported." (LLAccess) |
| Human-readable module description lives in the definition | LLAccess: "Provides ids of detected Corsair DRAM devices." / "LLAccess Device Listing", `cue.mod.llaccess_dev_listing`; Bragi: "Provides USB device ids detection for connected devices" / "Bragi Device Listing", `cue.mod.bragi_dev_listing(.hid_dev/.hid_report/.sub_dev)` |
| Bragi registers provider w/ dependency | `BragiDeviceListing.dll` `FUN_180001040` @0x180001040: builds `InterfaceId("HidEnumerator.dev.cue.corsair.com")` @0x1800761d8 and `InterfaceId("DeviceListingProvider.dev.cue.corsair.com")` @0x1800761e8; record `{+8 ptr=&HidEnumeratorId, +0x10 count=1, +0x28 factory=FUN_18000be70}` pushed to module registry `FUN_180049e90()` |
| LLAccess registers provider with no deps | `LLAccessDeviceListing.dll` `FUN_180001000` @0x180001000: same shape, dep ptr/len = 0, factory `FUN_180001330` |
| Interface IDs seen in iCUE.dll | `DeviceLister`, `DeviceListerObserver`, `EnumerationProvider`, `EnumeratorObserver`, `HidEnumerator`, `AudioEnumerator`, `BluetoothDeviceEnumerator`, `DataMiningUsbEnumerator`, `EmulationEnumerator`, `RouterEnumerator.routers`, `DummyDongleEnumerator.bragi`, `SubdeviceEnumerationController.bragi`, `DeviceManager`, `DeviceConnectionProxy`, `DeviceMapping.legacydev`, `SessionWatcher` (E\str_iCUE.txt, all `.cue.corsair.com`) |
| Provider ids are *not* `DeviceListingProvider.dev.cue.corsair.com` per plugin | That string is the **interface** id several modules *provide*; multiple implementers are keyed by their module, not by distinct ids **[I]** |
| `DeviceExtensionsController.dll` is NOT a discovery component | It provides `DeviceExtensionsControllerManagerProvider.control.dev.cue.corsair.com`; per-device property controller with USB-id tables (`UsbIds`, `BootloaderUsbId`, `SupportedUpdateUsbIds`, `CompatibleToImportUsbIds`, `ExclusionUsbId`, `DemoUsbId`, `HidProto`, `cue::dev::manifest::DeviceEnumerator`) — i.e. it *consumes* manifests post-detection **[O strings]** |
| `PluginDevice.dll` / `SystemDevice.dll` are data-type DLLs only | Exports are QDataStream operators / metaobjects for `cue::dev::plugin::{DeviceInfo,Zone,LedColor,…}` and `cue::dev::system::{SystemDevice,SystemSensor,SystemValue}` (+ enums Processor/Display_Adapter/Mainboard/Memory_Module/Watercooler). No enumeration logic **[O]** |

### 2.2 The lister service split
Evidence: `CorsairDeviceListerService.exe` strings (`E\str_ListerSvc.txt`):
- Service identity: `dls.service`, `dls.ipc_server`, `Starting service:`, `Service already running`, `Requesting service shutdown due to inactivity`, `Error during service startup sequence. Manual restart may be required`, config `/config.ini`, plan `:/init/run_plan.json`.
- IPC: `cue::dev::lister_svc::IpcServer`, `CorsairDevListerServiceAddress`, Qt RemoteObjects (`DeviceListerServiceIpcSimpleSource/Source`, `RemoteObject Signature a1e5e237…`), signals `devicesReady`, `devices` (`QList<cue::dev::DeviceIds>`), methods `startDetection`, `stopDetection`, `setUsbDevicesFilter`; shared-memory address handoff with private namespace + DACL (`shared_memory_helpers`, `Failed to create boundary descriptor`, `Global\`, `_mutex`).
- Detector: `cue::dev::DeviceDetectorImpl`, `cue::dev::DeviceDetector::Mode` = {`Regular`, `Service`}; requires system interfaces `DeviceLister`, `DeviceListingProvider`, `HidEnumerator` (`Missing required system DeviceLister/HidEnumerator`); applies `cue::dev::DeviceListerFilter` (`Setting USB devices filter...`, "Detector is not subscribed, filter will be applied later."); lister events `EnumerationActive`/`EnumerationCompleted`, `Lister state changed:`, `devicesChanged`, `enumerationCompleted`; names `BragiDeviceListing` module explicitly.
- iCUE side (`iCUE.dll`, log category `cue.dev.lister_watcher`): `Setting up detection service`, `Start detection through service`, `Detection through the service has already started`, `Devices ready received from service:`, `Filtered for service devices received from iCUE:`, `Service enumeration requested`, `Service connection failed`, `Errors encountered during devices enumeration:` and `DeviceLister::Error(description: …; provider: …)`. Classes `DeviceListerServiceController`, `DeviceListerWatcher`, `DeviceListerServiceIpcReplica`.
- Purpose **[I]**: let the installer/tray detect which hardware is present *before* the heavyweight UI/plugins load (to know which device packages to download), and keep detection alive without the main app. The "Regular" mode is the in-process fallback.

### 2.3 Provider internals (observed by decompilation)
**Bragi provider** (`BragiDeviceListing.dll`):
- Observes HID enumerator events ("`HidEnumeratorObserver`"), per-device lifecycle: "Trying to attach", "Physical device for … not found. Crate new", "Enqueue instance remove", "Enqueue device release", "active collections. Force release", "when device ready to remove", `BragiPhysicalDevice::Status`.
- **Debounce/settle algorithm** [O, `FUN_18000a410` = `onEnumerationCheckerTimeout`, `FUN_18000a8e0`]: provider keeps a list (`this+0x120`) of pending-device records each with `QElapsedTimer` + int timeout. Checker slot: if any record not expired → log "Enumeration still active. Restart timer", `QTimer::start` (member @+0x100) and return; else post a `QRunnable` (`FUN_180009f00`) on its `QThreadPool` (@+0x170) to do the (blocking) work. `FUN_18000a8e0` (result handler): if any record unexpired → "New device inserted. Enumeration still active", return; if enumeration timer active → "Enumeration triggered again"; else compare new device-id list with cached (`FUN_180002360`, `this+0x158` implicit-shared QList): equal → "Same devices"; different → "Connected devices changed: <list>", swap cache, **notify all observers under mutex** (loop over observer vector @+0x68..+0x70). So the listing is **snapshot-diff + notify-on-change, with settle debouncing**. Timer interval/timeout constants are runtime values; **[U]** exact ms.
- Per-device probing before reporting: reads `PairedSubdevicesIds`/`ConnectedSubdevicesIds` (files in the Bragi device FS) to append VIDs/"CorsairId"s for wireless sub-devices (dongle ⇒ child devices), reads `MID` and cooler-id, starts a **host software session** ("Host session with id … started successfully", "cannot set HostControlled mode", `get initial operating mode`, retry) — i.e. listing involves *talking to the device* (Bragi protocol over HID), not just reading descriptors. "Read protocol variant … USB string timeout" shows it reads a USB string descriptor to pick the protocol variant; "invalid protocol variant" ⇒ device ignored.
- Wireless re-enumeration side effects are explicitly modeled (`WillReenumerateWithDifferentPid`, etc.).

**LLAccess provider** (`LLAccessDeviceListing.dll`): one-shot-looking SMBus/DRAM probe. `FUN_180005090` @0x180005090: calls vtable+0x158 of the LLAccess driver object (member +0x70) to fill an array of ≤8 module records of 0x4c bytes; >8 ⇒ warn "System has more memory modules than max" and clamp to 8; 0 ⇒ "Failed to detect any memory modules". Groups modules by a 64-bit key (likely SMBus controller/slot), logs "Detected DRAM module [%1:%2] %3", skips modules whose PID cannot be determined ("Skipping DRAM module … because of failure to determine its PID"), reports `usb-vid`/`usb-pid`/`part-number-prefix` properties (a *synthetic* VID/PID for RAM; iCUE.dll has `vid<1b1c>pid<000006>serial<0>index<0>` and package id `unidentified-dram`). Platform tables: `Northbridge:`, `Southbridge:`, chipsets ("X370","X470","X399","H170","Z390","SKYLAKE-L9"…), `Slot count`, `MemType=`, `FW ver=`, `BL ver=`, SMBus mutex ("Failed to lock SMBus controller"), CRC check on block reads. Trigger for rescans: **[U]** (no timer/hotplug hook in module; likely on-demand by `DeviceLister`).

**DataMiningUsb** (telemetry-only enumerator, `cue.mod.datastatistics.enum.usb`): consumes `HidEnumerator`, provides `DataMiningUsbEnumerator`; uses SetupAPI-config-manager strings (`CM_Get_Device_Interface_List(_Size)A`, `CM_Locate_DevNodeA`, `CM_Get_Parent`, `CM_Get_DevNode(_Registry)_Property*`, `CreateFileA`, `DeviceIoControl`). Insert policy `FUN_180008d30` @0x180008d30: linear scan of known devices (record stride 0x90) for same key (+0x60 string) + vid + pid → "Device already inserted"; otherwise increments a per-key counter in a hash map (@+0xe0) and if counter > threshold (@+0xe8) → warn "Device is spamming, ignore" else append and notify observers. Other strings: "Ignore vid:", "Request open vid:", "Try reopen after fail", "Device removed while waiting for its subdevices". **Useful as a hot-plug-storm guard pattern**.

### 2.4 Multi-provider merge / dedup / priority
- Observed: each provider is independent, emits `DeviceIds`-like lists on its own observer interface (`DeviceListerObserver`, `EnumeratorObserver`); `DeviceLister.dev.cue.corsair.com` is the aggregator (service requires it; iCUE logs `Device listers received:` as the stage1 package list, `Lister state changed:`, `Errors encountered during devices enumeration: … provider: …`) **[O strings]**.
- The error struct `DeviceLister::Error(description, provider)` shows **provider failures are aggregated, non-fatal, and attributed** **[O]**.
- Inferred pipeline order **[I]**: HID (USB-id) providers (Bragi + legacy `LegacyEnumerator`/`HidEnumerator`/`UsbDeviceListing`/`DeviceListing`/`WmiDeviceListing` per other workers) + LLAccess DRAM + Bluetooth/Audio/Router/Emulation enumerators → aggregated by `DeviceLister` → filtered (`DeviceListerFilter`, `setUsbDevicesFilter`) → `Process detected devices:` (iCUE package-manager stage1) → stage2 install → `DeviceManager` instantiates devices from manifests (other workers).
- Dedup keys: provider-level snapshot equality; DataMiningUsb composite (path, vid, pid); final device identity string `vid<1b1c>pid<%1>serial<0>index<%2>` and `senstype<%1>sensindex<%2>` for sensors **[O strings]**. Merge priority across providers: **[U]**.

### 2.5 What triggers a rescan
| Trigger | Evidence |
|---|---|
| Startup | service `Start device detection`/`startDetection`; iCUE "Setting up detection service" |
| USB hot-plug/unplug | Providers are observers of `HidEnumerator` (Bragi dep; DataMiningUsb); settle timers per inserted device (above). The OS notification mechanism itself lives in HidEnumerator (other worker) **[U here]** |
| Device-initiated re-enumeration | Bragi property names `WillReenumerateWithDifferentPid`/`…ToApplyPairing`/`…ToAdjustPollingRate`; `FirmwareUpdateRequiresReplug`, `BootloaderUsbId` in manifests; "Device removed while waiting for its subdevices" |
| New package installed | stage2 completes → "Restart DeviceLister" → "Packages installed. Load new modules" |
| Explicit | `Service enumeration requested`; `startDetection/stopDetection` IPC; `Cannot start while restart is scheduled` |
| Timers | Only the Bragi enumeration-checker QTimer (debounce) is observed; no periodic polling timer found in the providers **[O: none seen; U for iCUE.dll/HidEnumerator]** |
| Sleep/resume | not proven for lister (see 2.6) |

### 2.6 Sleep/resume, user-session, service restart
- `SessionWatcher.dev.cue.corsair.com` (iCUE.dll) delivers power-state + session transitions. Strings: "Leaving working state, session disconnected." / "Entering working state, session connected." / "Entering working state, because of transition from Suspended state." / "Leaving working state, because of transition to Suspended or Shutdown state." / SDK server: "User session about to be locked. Stopping a listening" / "…unlocked. Starting a listening" / "Transition from Suspended power state. Starting a listening" / "Unknown power state". `SuspendedState`, `isSuspended`, `DirectLightingSuspendController`, "Start prepare WorkingStateState" (save/restore device-order states around transitions) **[O strings]**. This is the SDK/lighting layer; relation to device listing **[U]**.
- Device-side: wireless/sleep modes (`SleepMode_HostControlled_*`, `PowerPerformanceEnableSleep`) are device features, unrelated to PC suspend.
- Lister service robustness **[O/I]**: inactivity-based self-shutdown, `Service connection failed`, `Service already running` (single-instance via named mutex + shared memory), `Error during service startup sequence. Manual restart may be required`.
- Device re-open after resume: Bragi provider "Trying to attach"/"Force release"/"Set … was disconnected"; recreating `BragiPhysicalDevice` when a path is absent ("not found. Crate new"). **[I]** Recovery after resume therefore relies on the OS firing remove/arrive for the HID interfaces (which HidEnumerator must handle).

## 3. Discovery pipeline (text diagram)

```
                         OS: USB/HID arrival-removal, SMBus (via LLAccess driver), BT, audio
                                          |
                       +------------------+--------------------+
                       |                                       |
        HidEnumerator.dev.cue (hotplug source)        LLAccess (SMBus DRAM probe, no HID dep)
          |            \                                       |
          |             \--> DataMiningUsb (telemetry, spam-guard: path+vid+pid, per-key counter)
          v                                                    v
  DeviceListingProvider impls  (Bragi: settle-debounce -> snapshot diff -> "Connected devices changed")
  [Bragi / Hid-USB / WMI / legacy / LLAccess ...]  provide ids only: DeviceIds {vid,pid,[serial],subdevice ids}
          | observers (DeviceListerObserver)
          v
  DeviceLister.dev.cue  (aggregate; errors attributed to provider; DeviceListerFilter / setUsbDevicesFilter)
          |
  [CorsairDeviceListerService.exe hosts the above in "Service" mode ] -- Qt RemoteObjects IPC -->
          v
  iCUE.dll  cue.dev.lister_watcher ("Devices ready received from service")
          |
  pkg_manager stage1: "Search listers packages" (*-device-listing) -> "Process detected devices"
          |   unknown ids -> "ignoring detected device due to pending installation of required packages"
          v
  pkg_manager stage2: install dev-*/devplugin-* -> "Restart DeviceLister" -> "Load new modules"
          |
          v
  DeviceManager.dev.cue (manifest match: UsbIds/HidProto/DeviceEnumerator; other workers)  -> device objects
          |
  Per-device controllers (DeviceExtensionsController etc.), SubdeviceEnumerationController (Bragi/iCUE LINK children)
```
Sleep/resume: `SessionWatcher` --> SDK/lighting “working state” (listing impact unproven).

## 4. Comparison vs ai-corsair-hub
Code read: `crates/hid/src/discovery.rs` (130 lines), `apps/gui/src/hardware_thread.rs` (`RealHubBackend::reopen` @~895, `build_device_tree` @1055, `open_psu` @1095).
- Discovery = one-shot/snapshot `HidApi::refresh_devices()` + filter VID `0x1B1C`; `scan_grouped()` groups by `pid:serial` **[repo]**. Refresh is called **only** inside `RealHubBackend::reopen()` after a failure (`grep refresh()` → hardware_thread.rs:903). No change-detection, no hotplug subscription, no observer/event model.
- Gaps vs iCUE:
  1. **No event source** (iCUE: HidEnumerator hotplug; we: poll-on-failure only). A hub that appears after startup, or the PSU being plugged later, isn't noticed until some control path fails.
  2. **No settle/debounce**: USB composite devices appear as several HID interfaces; reading mid-enumeration may see incomplete interface sets (iCUE waits for per-device timers to expire).
  3. **No snapshot diff / change event**: callers re-scan and rebuild whole trees (`build_device_tree` each call).
  4. **No flood guard** for a device that re-enumerates repeatedly (DataMiningUsb spam counter).
  5. **No provider error aggregation / isolation** (iCUE attributes errors to the provider and continues). Ours is `anyhow` single-path.
  6. **No resume/session handling**: nothing listens for power/session events; recovery relies on write failure → `reopen`.
  7. **Identity**: we key by `pid:serial`, falling back to `"unknown"` when the serial is missing (`unwrap_or("unknown")`) — two serial-less devices would collide. iCUE builds `vid/pid/serial/index` composite ids.
  8. No separation of "who is present" (cheap id listing) from "what model" (manifest/package), which iCUE uses to avoid touching devices it doesn't support.

## 5. Recommendations (prioritised, Rust-implementable)
1. **Hotplug event source (P0)**: register for device-interface notifications for the HID class GUID `{4D1E55B2-F16F-11CF-88CB-001111000030}` using `CM_Register_Notification` (windows crate, `CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE`; fires `CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL/REMOVAL`), or a message-only window with `RegisterDeviceNotificationW` + `WM_DEVICECHANGE`. Forward events into the existing hardware-thread command channel as `HwCommand::DevicesChanged`. (iCUE evidence: Bragi/DataMining observer model §2.3.)
2. **Debounce + snapshot diff (P0)**: on any event, (re)start a ~500 ms–1 s settle timer (iCUE's constants unknown, treat as tunable); when it fires, run `refresh_devices()`, build `BTreeSet<DeviceKey{vid,pid,serial,path-or-container-id}>`, diff vs previous, emit `Added/Removed` only if changed ("Same devices" short-circuit).
3. **Resume/session handling (P1)**: subscribe to `PBT_APMRESUMEAUTOMATIC/PBT_APMSUSPEND` (`RegisterSuspendResumeNotification` or `WM_POWERBROADCAST`) and `WTS_SESSION_*`. On resume: mark hubs stale, delay a few seconds, force refresh+reopen+`initialize` (the hubs expect re-init; cf. G04 notes), then resume control. Don't rely on write failure alone.
4. **Flood guard (P2)**: per-device-key insertion counter within a window; if > N in T, quarantine with a warn (mirrors DataMiningUsb §2.3).
5. **Stable identity (P2)**: use the container ID / parent devnode (`CM_Get_Parent`, `DEVPKEY_Device_ContainerId`) in the key rather than `"unknown"` serial fallback. iCUE's DataMining module uses `CM_Get_Parent`/`CM_Get_DevNode_Property*` for this.
6. **Provider trait (P3)**: introduce `trait DeviceProvider { fn snapshot(&mut self)->Result<Vec<DeviceId>>; }` with HID/USB provider and (later) LHM/NVML providers; aggregate errors per provider, never abort the others (iCUE `DeviceLister::Error{provider}`).
7. **Do not copy** the lister-service/IPC/package-manager split: iCUE needs it for a large catalogue + downloads; this repo supports a fixed hardware set. A single in-process discovery thread suffices.

## 6. Open questions
- Exact settle/timeout ms in Bragi (`QTimer::setInterval` / per-record `QElapsedTimer` timeout values) — needs decompilation of the code that creates pending records (callers of `FUN_18000a410` slot connection). **[U]**
- Whether any periodic poll exists in `HidEnumerator`/`DeviceLister` (other worker: HidEnumerator).
- Cross-provider merge/priority in `DeviceLister` — implementation is in a DLL/exe not decompiled here (`DeviceLister` impl module; iCUE.dll too large for Ghidra within 330 s — rerun with `REA_GHIDRA_STARTUP_TIMEOUT_MS` ≥ 1,800,000 and `REA_GHIDRA_DISABLED_ANALYZERS`, or analyse `CorsairDeviceListerService.exe` with Ghidra instead; 365 KB, tractable).
- Link between `SessionWatcher` power events and lister stop/start (strings found only for SDK server and working-state; need xrefs from `SessionWatcher` interface-id users).
- LLAccess rescan trigger and which driver backs the vtable call at `this+0x70`/+0x158 (likely the “LLAccess” SMBus kernel service) — not examined; no hardware interaction performed.
- Where the Qt plugin search path/ordering for `modules\*.dll` is defined (iCUE.dll loader; `QPluginLoader` not traced).

## Evidence index
- `E\str_*.txt` string dumps; `E\strings.ps1` extractor.
- `E\trace_bragi_enum.json` (evidence_id `ev_9977714e193bacfb454a09a40a90924f29da6f528bbbcdf0319519a6fa6b8897`); `E\dec_bragi_a410.json`, `E\dec_0x18000a8e0.json`, `E\dec_0x180001040.json`; `E\ll_dec1.json`, `E\ll_dec2.json`; `E\dm1.json`, `E\dm_dec.json`; `E\trace_suspend.json` (the iCUE.dll timeout).
- Files modified: none outside `E\` and this report. No iCUE executables run, nothing sent to USB.
