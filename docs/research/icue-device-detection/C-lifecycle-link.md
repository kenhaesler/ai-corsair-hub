# Task C - Device lifecycle and iCUE LINK sub-device discovery

Work dir: `scratchpad\re\C\` (string/proc dumps `*_strs.txt`, `*_procs.txt`; extracted manifests in `rcc\`; decompiles `dct_dec_*.json`, `bp_dec_*.json`).
Legend: **[O]** observed in binary/data, **[I]** inference, **[U]** unknown. Binaries are under `C:\Program Files\Corsair\Corsair iCUE5 Software` (iCUE 5.42.70, from `config.cuecfg`).

## 1. Summary

- **`ClinkProto.dll` is not iCUE LINK.** It is the legacy *Corsair Link* client (JSON over a local socket to Corsair.Service: `DeviceGetList`, `SensorGetValues`, `SetLimitedSwMode`, Commander Pro, Platinum, AX1600i). [O] `CP_strs.txt` 0x180097924..0x18009f824. Nothing in it applies to the Hub.
- **The iCUE LINK hub runs on the Bragi HID protocol** (`HidProto = Bragi_lighting_node`; sub-devices use `Bragi_link_ecosystem_subdevice`). [O] `link_system_hub_manifest.cuebmf`, `qx_rgb_manifest.cuebmf` (extracted from `devices\*.rcc`). The real logic lives in `BragiProtocols.dll` and `DeviceControllers.dll`. `LinkSubdevicesController.dll`, `ComplexDeviceController.dll`, `DeviceProxy.dll` and `DeviceNotifications.dll` are thin facades.
- **Sub-device discovery is hub-driven, not poll-driven.** `LinkEcosystemSubdevicesController::enumerateSubdevices` (DeviceControllers 0x180026ac0) enqueues one `cmd::StartSubdeviceDetection`. The controller then reacts to notification id `0x3b`, which calls `subdevicesChanged` (`onNotificationReceived` 0x180028140). [O] Candidate notification classes: `ConnectedSubdevicesBitmapChanged`, `RouterSubdevicesChanged`, `UnknownSubdeviceConnected`, `LightingChannelsAutodetectionCompleted`, `DeviceReseted` (DN_procs). Which class is id 0x3b is **[U]**.
- **The enumerator reads a "connected subdevice bitmap".** Strings: `Failed to read connected subdevice bitmap.`, `connected subdevices:`, `Finish enumeration task`, `Can't finish init. Subdevice is not emitted` (BragiProtocols 0x1802a4930..0x1802a4978). Commands: `GetConnectedSubdevicesBitmap`, `GetConnectedSubdevices`, `GetConnectedSubdevicesInfo` (type + id per sub-device), `GetSubdeviceId`, `DelegateToSubdevice`, `GetConnectedLedChannelCounts`, `GetConnectedFixedLedChannelTypes`, `StartLightingChannelsAutodetection` (DeviceCommands strings 0x1800811b4..0x180081724).
- **Liveness uses per-sub-device ping.** The Hub manifest sets `RequiredPingInterval = 30000` (ms); the LINK Adapter and QX manifests set 0. `BragiPingController` and `PingControllerManager` log `Failed to ping device in time, removing subdevice` and `reached zero, removing subdevice` (0x1802a4800, 0x1802a4850). A sub-device that misses pings is removed from the tree (a countdown, exact count [U]).
- **Type IDs.** `corsairDeviceId` is the sub-device type. 1 = QX RGB, 5 = LINK Adapter and 7 = LINK H100i RGB are confirmed directly. iCUE does not distinguish QX120 from QX140: both are ID 1 (see section 2.3).
- **LS350 is not a device type.** It is an LED channel type (`LSAuroraSeries`) hosted by the LINK Adapter (ID 5), made of `Strip40Leds` or `Strip49Leds` stages. The adapter's LED-count table is decoded by total count: 80 LEDs = 2x Strip40Leds, which matches the 80 seen on ch15 in this repo.
- **The Hub supports 24 channels.** `MaxSubdevicesOnSide = 12` plus `Exceeded subdevices number on the Port 0/1` gives 2 ports x 12; `DefaultLedChannelCounts = [24]`, `MaxLedsCountAcrossAllChannels = 476`, `SensorsChannelsRange = 26`. Our `read_led_counts` rejects `channel_count > 20`, which is too low.
- iCUE persists detected sub-devices per hub as `DetectedSubdevices` = `QMap<int type, QList<QString id>>` (see 2.4).

## 2. Findings

### 2.1 Device state machine / open sequence (partial)

- DeviceManager is a generic matcher. Enumerators (HID, Bragi, legacy) offer "channels" keyed by `usb-vid`/`usb-pid`/`corsair-id`. DeviceManager tries manifests; logs: `Candidate device for ... Ignoring.`, `Failed to open channel`, `Device ... refused channel`, `Started constructing`, `Created`, `Finished constructing` / `Failed constructing`, `Attempted to forget connected device`, `Forgotten` (DM_strs 0x18002bd30..0x18002c498). [O] Decompile of the channel-offer function (DM FUN_18000d830) shows: `open channel` -> if null: log "Failed to open channel"; else manifest accepts or "refused channel". There is no retry loop in this function; retry/backoff lives in the enumerators (e.g. `BragiDummyDongleEnumerator::onReconnectTimerTimeout`, 0x1802a0e1c). [I]
- The Hub appears as `EMULATED_LINK System Hub(vid=1b1c, pid=c3f)_0` and each sub-device as `EmulatedDeviceType::LinkSystemSubdevice` (DeviceEmulation 0x180065590, enum 0x18006d65c..0x18006d67e: `LinkSystemHub`, `LinkSystemSubdevice`, `LinkSystemSubdeviceLcd`). [O]
- Operating modes (`DeviceOperatingMode`): Invalid, HardwareGaming, SoftwareGaming, Standard, ManufacturingTest, MouseCalibration, **McuBootloader**, **LimitedSoftwareMode** (DC 0x18007dfe4..0x18007e063). Hub manifest: `SupportsLimitedHostControlledMode` / `SupportsSelfOperatedModeOnly` property names exist; `BragiFwWorkarounds.resetToDefaultOnOperatingModeChangeFail`. Log `initialOperatingMode is equal to OperatingMode::Invalid - its seems need fix of firmware` (BP 0x1802a93c0). Firmware update: `SetFirmwareUpdatingState`, `McuBootloader`, `BragiMcuBootloaderFirmwareUpdateProcess`; `SupportsSecondaryBootloaderVersion = true` on the Hub. [O] Exact enter/exit byte sequences: [U].
- Two hubs: nothing links two hubs together. Each Hub is its own `LinkSystemHub` device with its own serial-keyed config section; `isMasterController`/`masterController`/`supportsSharingLogic` exist on the controller (LSC exports) but the manifest has `IsMasterController=true` for the hub only. Cross-hub ordering [U].
- Dead hub: `Main subdevice disconnected!`, `Trying to remove main subdevice for`, `Removing timed out report for subdevice`, `Skipping reply for other session`, `Command not send. Subdevice disabled.` (BP 0x1802a6748..0x1802a79f8). So a command timeout drops the pending report and the IO error path removes the main subdevice (and with it all LINK children). [O] strings / [I] flow.

### 2.2 Sub-device discovery flow [O unless noted]

1. Hub device appears (USB 1B1C:0C3F, `HidProto=Bragi_lighting_node`); `LinkEcosystemSubdevicesController` attaches (`attachToDevice`).
2. `enumerateSubdevices()`: sets `enumerationActive=true`, `initialized=false`, queues `StartSubdeviceDetection` (DC cmd) on a CommandsTransaction.
3. Hub firmware reports connected sub-devices (bitmap / info list: type id + id string per channel); `BragiSubdeviceEnumerator` creates a sub-device for each (`Cannot create subdevice for:`, `Subdevice with no manifest lost before processed for CreateDevice.`, `Failed to get vid, pid from subdevice id: %1`, `Unknown logical subdevice prototype`).
4. Each sub-device gets a manifest by `corsairDeviceId` (`DeviceKeys: {"type":"CorsairDeviceId","data":{"corsairDeviceId":N}}`), then it is initialized; controller flags `initialized` after `onAllSubcontainersInitialized`.
5. Unknown ids raise `UnknownSubdeviceConnected(usbId)` and fall back to `FallbackManifest` (`Generic Subdevice`).
6. Changes: notification -> `subdevicesChanged`; the bitmap read is repeated. A removed fan is dropped when ping fails (2.1). Autodetect of LED channel types is a separate sequence: `StartLightingChannelsAutodetection` -> `LightingChannelsAutodetectionCompleted`; iCUE caches completion (`ChannelTypesAutodetectionDone`, `DoChannelAutodetectOnlyOnce=true` on the adapter).
- Re-poll period of the bitmap: [U] (no timer constant found; the string-driven path suggests event-driven refresh plus the 30 s ping).

### 2.3 Type id table (evidence)

`SupportedCorsairSubdevices` is in `link_system_hub_manifest.cuebmf`. ID = low byte type, high byte variant (261=0x105 ...) [I].

| id (hex) | iCUE category (Hub manifest) | Model evidence | Our `LinkDeviceType` | Match |
|---|---|---|---|---|
| 1 | Fan | **QX RGB** `qx_rgb_manifest.cuebmf` (`corsairDeviceId:1`, "iCUE LINK QX RGB", `QxRgbFanHub`=34 LEDs); `EMULATED_QX RGB(corsairDeviceId=1)` DeviceEmulation 0x180065710; user config: 10 sub-devices under key 1 = 9x QX140 + 1x QX120 | QxFan | OK. QX120 and QX140 are the same id; no per-size manifest. |
| 2 | Fan | category only | LxFan | category OK; model name [I] |
| 3 | Fan | category only | RxMaxRgbFan | OK (category) |
| 4 | Fan | category only | RxMaxFan | OK (category) |
| 5 | LightingNode | **LINK Adapter** `link_adapter_manifest.cuebmf` (`corsairDeviceId:5`, "iCUE LINK ADAPTER"); user config key 5 = 1 adapter (the LS350 host) | LinkAdapter | OK |
| 6 | LiquidCooler | category only | - | **missing** |
| 7 | LiquidCooler | **LINK H100i RGB** DeviceEmulation 0x180065738 | LiquidCooler | OK |
| 9 | LiquidCooler | category only | WaterBlock | OK (category) |
| 0x0A | **Fan** | category only | GpuBlock | **CONFLICT**: iCUE says Fan |
| 0x0B | PSU | category only | Psu | OK |
| 0x0C | LiquidCooler | category only (XD5Elite likely, [I]) | PumpXd5 | OK |
| 0x0D | LightingNode | category only | Xg7Block | OK |
| 0x0E | LiquidCooler | - | - | missing |
| 0x0F | Fan | - | RxRgbFan | OK |
| 0x10 | PumpFan | - | VrmCooler | OK-ish |
| 0x11 | LiquidCooler | - | TitanCooler | OK |
| 0x13 | Fan | - | RxFan | OK |
| 0x14 | LightingNode | - | - | missing |
| 0x15 | PSU | - | - | missing |
| 0x19 | LiquidCooler | - | PumpXd6 | OK |
| 0x1A | Fan | - | - | missing |
| 0x1B | LightingNode | - | CommanderDuo | OK |
| 0x1D | PSU | - | - | missing |
| 0x105, 0x107, 0x10B, 0x111, 0x115, 0x11A, 0x205, 0x207, 0x20B, 0x211, 0x307, 0x311, 0x407, 0x411, 0x507, 0x511, 0x81A | Lighting/Cooler/PSU/DashboardLCD (0x81A) | variants of 5, 7, 0xB, 0x11, 0x15, 0x1A | none | **unmapped**; our parser reads only the low byte |

Model names in the Hub's cooling list (no ids): LinkH150iRgb, LinkH170iRgb, XD5ElitePump, XD6ElitePump, VRMCoolerModule, LinkTitanRgb, XG3Hybrid. Names for ids 2, 3, 4, 6, 9+ beyond category are **not installed locally** (only the manifests for the user's own devices are shipped as `.rcc`), so the table above is the maximum evidence available.

**LS350 / LED channel types:** the adapter manifest lists `LSAuroraSeries` (stages `Strip40Leds`, `Strip49Leds`, max 2 stages, `maxLedsCountForOneChannel` 204). `LedChannelConfigurationsByLedCountMap` decodes the adapter's reported LED total: 40 = 1 strip, 80 = 2x40, 89 = 40+49, 98 = 2x49, 120/129/138/147 = 3 strips, 160-200 = 4-5 strips. Strip40Leds = LS350 is [I] (the user's two strips decode to 80). Full channel-type LED table in `link_system_hub_manifest.cuebmf` (`LedChannelTypeLedsCount`: QxRgbFanHub 34, LxRgbFan 18, RxRgbFan 8, XD5/XD6ElitePumpRes 22, XG7Rgb 16, Link5000tRgbChassis 204, Strip15/21/27/32/38/40/49/84Leds ...).

### 2.4 Identity and ordering

- Sub-device id = 26 hex characters (13 bytes), all of the user's start `0100` [O config.cuecfg]. Several share a longer prefix, so it is not a serial-number prefix per model. It is a per-unit id.
- iCUE persists per hub: `DetectedSubdevices` = Qt `QMap<int,QList<QString>>`: key = type id, value = ordered id list (hub A: key1 x6; hub B: key1 x4 + key5 x1). Also `FirstDetected`, `LedChannelCounts`, `LedChannelTypes`, `LedChannelPositions`, `UniqueDeviceId` (GUID) per device.
- Ordering is by chain position (the Hub index list), with `MaxSubdevicesOnSide=12` per port [I].

## 3. Comparison with ai-corsair-hub

Read: `crates/hid/src/discovery.rs`, `icue_link.rs`, `crates/fancontrol/src/control_loop.rs`.

| Area | iCUE | Ours | Gap |
|---|---|---|---|
| Enumerate | `StartSubdeviceDetection` then bitmap/info; type id may exceed one byte | `read_endpoint(0x36)` data type `0x21 0x00`; type = `resp[pos+2]`, `model = resp[pos+3]` | We drop variants (`model` byte may be the high byte [I]); ids 6, 0x0E, 0x14, 0x15, 0x1A, 0x1D unmapped; 0x0A conflict |
| Channel cap | 24 channels (2 ports x 12) | `read_led_counts` rejects `> 20` | Possible false fallback on 24-channel hubs |
| LS350 | LED channel type from adapter total; default Strip40 | `LinkAdapter`/`LsStrip` default 21 LEDs | 21 is a Strip21 default, not LS350; derive strips from total (40/80/...) |
| Change detection | event-driven notifications + 30 s ping | enumerate only inside `initialize()` | A fan added or removed is seen only on recovery/restart |
| Dead sub-device | ping timeout drops the sub-device | channel stays in registry | stale channel keeps being written |
| Dead hub | IO error -> remove main subdevice | `hubs_needing_recovery`, backoff 10/20/40/80/120 s, full re-`initialize` | Good; no hub-side heartbeat |
| Identity | type + 26-hex id, QMap persisted | `LinkDevice{channel,type,model,device_id}` by channel | No stable id-based identity across re-enumeration |
| Modes | LimitedSoftwareMode, McuBootloader, firmware-updating state | `0x01 0x03 0x00 0x02` wake / `0x01` sleep only | Bootloader/FW-update mode not detected |

## 4. Recommendations (prioritized)

1. Raise the channel sanity cap from 20 to 24 (`MaxSubdevicesOnSide x 2`); also check the enumerate loop does not assume `<= 20`. Cheap, low risk.
2. Keep type as a `u16` (`type | model<<8`) in `LinkDevice`, behind a check against a real `[O]` capture (log the `model` byte of the 10 QX fans and the adapter; if all are 0 the hypothesis in 2.3 stays unproven). Do **not** alter RGB addressing without byte-level evidence (project rule).
3. Fix LS350 sizing: decode the 0x1d total through the iCUE table (40 = 1, 80 = 2 strips) instead of a 21-LED default; keep the existing "non-zero entry wins" policy.
4. Add a per-fan staleness policy: if a channel returns no RPM/status for N cycles while the hub is healthy, mark it absent instead of writing to it.
5. Add periodic sub-device refresh: re-run `enumerate_devices()` (every ~30 s, aligned with iCUE's ping interval) when idle, and diff against the registry (add/remove events). Use a stable key (type, device_id) rather than channel index, as iCUE does (`DetectedSubdevices`).
6. Persist last-seen sub-devices per hub serial (type -> ordered ids) next to the config, for "device went missing" notices.
7. Resolve conflict 0x0A (GpuBlock vs Fan) before anyone relies on it; add the unknown ids as `Unknown(id)` with category from the Hub manifest table (Fan/LiquidCooler/PSU/LightingNode) so `is_fan`/`is_pump` logic stays correct for unmapped types.

## 5. Open questions

- Which notification id is `0x3b` (RouterSubdevicesChanged vs ConnectedSubdevicesBitmapChanged)? Decompile `LinkEcosystemSubdevicesController::onNotificationReceived` caller map.
- Exact USB opcodes for `StartSubdeviceDetection`, `GetConnectedSubdevicesBitmap`, `Ping` (Bragi property/command ids in `BragiProtocols.dll`; the decompiles of the ping handler at 0x180044e8c and the restart loop 0x180033afe were inconclusive, only logging and helper calls resolved).
- Ping miss count before removal ("reached zero") and bitmap poll period.
- Whether the `model` byte is the high byte of `corsairDeviceId`.
- Names for ids 2, 3, 4, 6, 9, 0x0A, 0x0C-0x1D and the 0x1xx-0x8xx variants (manifests not installed locally).
- Hub behavior on host loss / storable fallback (still open per project memory); `LimitedSoftwareMode` and `SupportsSelfOperatedModeOnly` are leads.

Evidence index: `ClinkProto` = `CP_strs.txt`; manifests = `rcc\link_system_hub`, `rcc\link_adapter`, `rcc\qx_rgb` (`*.cuebmf`, extracted with `tools\rcc.py`); DeviceEmulation strings = `DE_strs.txt`; user config = `%APPDATA%\Corsair\CUE5\config.cuecfg` (`LINK System Hub`/`DetectedSubdevices`). REA evidence ids: `ev_d6537984...` (LSC overview), `ev_e911143b...` (LSC procedures), `ev_87ad7a71...` (DeviceManager xref).
