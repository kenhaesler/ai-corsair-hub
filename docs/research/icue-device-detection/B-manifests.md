# Task B - iCUE 5 device manifests (brief)

Work dir: `scratchpad\re\B\` (`rcc\` extracted bundles, `json\` pretty manifests, `out\` REA/strings output).
Tags: **[O]** observed in binary/data, **[I]** inference.

## 1. Summary
- `devices\*.rcc` are plain Qt RCC v3 bundles (zlib entries). Each holds one `<name>_manifest.cuebmf` **which is plain JSON** (`{"common":{...}}`) plus images/masks/SVGs. [O] (own parser `B\..\rccx.py`)
- `ManifestBinaryProvider.dll` loads them: it `QResource::registerResource`s the RCCs under the app dir `devices` search path and globs `*.cuebmf` / `*_manifest.cuebmf` (strings @0x12ae0/0x12af0 + import `registerResource`; `defaultDeviceSearchPath`). [O]
- Matching is by **DeviceKey equality**, not by name: `ManifestHelpers::findManifest(storage, DeviceKey)` (0x18003d7a0) iterates manifests in storage and compares each manifest's `DeviceKeys` list (property id 0x30) with `DeviceKeyImpl::operator==`; first hit wins. The `QList<DeviceKey>` overload (0x18003d6c0) tries each key in order; the `DeviceUsbId` overload (0x18003d640) just forwards. No hit -> null `shared_ptr`. [O]
- Two key kinds appear in manifests: `{"type":"Usb","data":{vid,pid}}` (top-level USB devices, e.g. the hub) and `{"type":"CorsairDeviceId","data":{corsairDeviceId:N}}` (LINK sub-devices that have no USB identity, e.g. QX fan = 1, LINK adapter = 5). [O]
- `FallbackManifest.dll` is **not** an unknown-device handler: it embeds `:/devices/fallback_manifest.cuebmf` (strings 0x1a3208) providing **default property values** used "when real manifest has no appropriate property" (string 0x1a34c0); `ManifestImpl` ctor takes `FallbackManifestData*` + property hash (ManifestBuilder 0x14d00a). What an unmatched device does is decided by callers outside this task's DLLs. [O/I]
- Unique device ID = **random `QUuid::createUuid()`**, cached persistently in `UniqueDeviceIds::Storage` keyed by (USB key, serial, PairingId, corsairDeviceId); it is not a hash of path/serial. Identity match (`Data::isSameDevice`): same device key AND (if both serials non-empty: serial equal, else PairingId equal). USB instance path is not part of identity. [O]
- QX120 vs QX140 are **not** distinguished by manifest: one `qx_rgb` manifest (corsairDeviceId 1, 34 LEDs `QxRgbFanHub`, max 2400 RPM). LS350 Aurora = LED-channel type `LSAuroraSeries` on the `link_adapter` manifest (device id 5), not its own manifest. [O/I]

## 2. Findings

### 2.1 Manifest schema (JSON `common` object)
Key groups (names from manifests + `UniqueDeviceIds/ManifestHelpers` property-name strings): 
- Identity/matching: `DeviceKeys[]`, `UsbIds[]` (`vid`/`pid` decimal), `DemoUsbId`, `ProtocolChain{id,channels[{bus:"legacy","usb-vid","usb-pid"| "corsair-id"}]}`, `DramInternalPids`, `BootloaderUsbId`, `ExclusionUsbId`, `RequiresExtendedPidDetection`, `PartNumberPrefixes`, `DeviceSerialNumberFormat`.
- Class/protocol: `DeviceEnumerator` (`Bragi`, `CLink`, `LLAccess`, `Clink`...), `HidProto` (enum: Bragi_mouse/_rf, Bragi_headset/_rf, Bragi_kbd/_rf, Bragi_dongle, Bragi_lighting_node, Bragi_mousemat, Bragi_gamepad, Bragi_link_ecosystem_subdevice, ClinkPsu, ClinkLightingNode, ClinkStarCoolers, ClinkPlatinumCoolers, ClinkCorsairOne, ClinkGeneric, Uniwill_cooler, Touchscreen_bs010, PumpDisplay..., plus `Dram`), `DramProtocolType` (`Kopis`), `DeviceType`, `EmulatedDeviceType` (`LinkSystemHub`, `LinkSystemSubdevice`, `LinkSystemSubdeviceLcd`), `RouterImplementations` (`LinkEcosystem`), `SdkDeviceType`, `IsLinkDevice`, `IsMasterController`.
- Sub-device model: `SupportedCorsairSubdevices{id->type}`, `SupportedSeparatedCorsairSubdevices`, `MaxSubdevicesOnSide`, `SupportedChannelConfigurations`, `LedChannelTypeLedsCount`, `LedChannelTypeToGeometryIndexMap`, `DefaultLedChannelTypes/Counts`, `DynamicGeometryTemplates`, `MaxLedsCountAcrossAllChannels`.
- Capabilities/timing: `RequiredPingInterval` (hub 30000 ms), `SupportedCoolingImplementations` (hub `OnBoardControl`,`DirectControl`), `SupportedLightingImplementations`, `LightingsBufferHunkSize` 4096, `SensorsChannelsRange` 26, `SupportedSensorTypes`, `BragiFwWorkarounds`, `FirmwareUpdateOptions`, `PsuPwmRange` (HX1500i min 15 %, max 100 %).
- UI-only: icons, masks, thumbnails, `SubScreensListData`, `Layouts`.

### 2.2 Extracted device definitions (all [O], from `json\*_manifest.json`)
| Device | Manifest | VID | PID | Interface / key | Protocol (`HidProto`) | Enumerator | Notes |
|---|---|---|---|---|---|---|---|
| iCUE LINK System Hub | link_system_hub | 0x1B1C (6940) | 0x0C3F (3135) | key type `Usb`; no HID interface number in manifest | `Bragi_lighting_node` | `Bragi` | Emulated `LinkSystemHub`, router `LinkEcosystem`, ping 30000 ms, 24 channels default, max 476 LEDs, `MaxSubdevicesOnSide` 12, 4096-byte lighting hunk, SDK type `LightingNodePro` |
| HX1500i PSU | hx1500i | 0x1B1C | 0x1C1F (7199) | `UsbIds`/ProtocolChain usb-pid; no interface | `ClinkPsu` | `CLink` | `IsLinkDevice`, PSU PWM 15-100 %, firmware `CLinkOldFormat`, 1 geometry channel; sensors incl. current/efficiency/power/voltage/fan/temp |
| QX RGB fan (QX120 + QX140) | qx_rgb | - | - | key `CorsairDeviceId`=1 (hub subdevice table: "1"->Fan) | `Bragi_link_ecosystem_subdevice` | `Bragi` | `LinkSystemSubdevice`, `QxRgbFanHub` = 34 LEDs, max 2400 RPM, strobing limit 2000 RPM, `CoolingConfigurationType` `QxRgbFan`; no 120/140 split |
| iCUE LINK Adapter (hosts LS350) | link_adapter | - | - | key `CorsairDeviceId`=5 (hub table: "5"->LightingNode) | `Bragi_link_ecosystem_subdevice` | `Bragi` | `DefaultLedChannelTypes ["LSAuroraSeries"]` (0 fixed LEDs, geometry idx 1), `CustomLedChannelExtensionsNames` TopLeft/FrontLeft/BottomLeft/BottomRight/FrontRight/TopRight strip, `SupportsLedAutoDetection` |
| Dominator Titanium RGB | dominator_titanium | 0x1B1C | 0x040B (1035) and 0x0006 | `UsbIds` x2; `DramInternalPids` 2048,2049,2064,2065 (0x800,0x801,0x810,0x811) | `Dram` (`DramProtocolType Kopis`) | `LLAccess` (low-level/SMBus-style, not HID) | `SdkDeviceType MemoryModule`, part-number prefix `CMP`, LED type `DramProModule`/`DramRgbPro12Led` (12 LEDs, inferred from type name), firmware `DramArchiveFormat` |

Hub sub-device id table (`SupportedCorsairSubdevices`, decimal id -> class): 1,2,3,4,10,15,19,26 Fan; 16 PumpFan; 5,13,20,27,261,517 LightingNode; 11,21,29,267,277,523 PSU; 6,7,9,12,14,17,25,263,273,282,519,529,775,785,1031,1041,1287,1297 LiquidCooler; 2074 DashboardLCD. Per-type LED counts: QxRgbFanHub 34, QlRgbFanHub 34, LlFanHub 16, RxRgbFan 8, Strip32Leds 32, Strip38Leds 38, CoolerPump20Leds 20, LinkCoolerScreen40Leds 40.
Other bundles in `devices\`: govee, nanoleaf, philips_hue (network-lights; extracted, not analysed).

### 2.3 Matching and fallback
- Flow [O]: enumerator produces a `DeviceKey` (Usb vid/pid, or CorsairDeviceId for LINK children) -> `findManifest(storage, key)` -> linear scan of manifests' `DeviceKeys`, `DeviceKeyImpl::operator==` (type + data). `ManifestStorage` is a core service (`ManifestStorage.dev.cue.corsair.com`) implemented in ManifestRegistry (`ManifestStorageImpl`); providers (`ManifestProvider.dev...`, e.g. BinaryProvider) feed it. ManifestRegistry itself is a tiny shell (40 KB, no logic strings).
- Fallback [O]: missing properties are filled from `fallback_manifest.cuebmf`, which also carries `NonGenericModelManifests` and `DiyGenericDeviceTypesData` (strings in FallbackManifest 0x1a67f0, 0x1a8e88) and `HiddenForGenericUpdate`. [I] Unmatched devices become "generic" devices (null manifest -> caller decides); the exact generic path was not traced.
- `RequiresExtendedPidDetection`, `ExclusionUsbId`, `CompatibleToImportUsbIds` indicate extra PID rules beyond plain equality. [I]

### 2.4 Unique device IDs (`UniqueDeviceIds.dll`, evidence ev_2530a362... fromDeviceData @0x1800027b0; ev_d440f40a... isSameDevice @0x180002b70; ev_e6043e35... cachedDeviceId @0x180007310)
- `Data{DeviceUsbId(vid,pid)|key, QString serial, PairingId, u16 corsairDeviceId}`; built from the device-property hash: property 0 = serial string, property 5 = PairingId, corsair id from `getDeviceCorsairId(manifest)`. [O]
- `isSameDevice`: keys differ -> false; both serials non-empty -> compare serials; otherwise compare PairingId. [O]
- `Storage::cachedDeviceId`: `allEntries(Data)` (entries matching `isSameDevice`); none -> `QUuid::createUuid()` and store; else reuse the stored QUuid. Persisted via `load/save/saveSerial/changePairingId`. So the stable ID is a stored random GUID, surviving port moves because serial is the key. [O]

## 3. Comparison vs ai-corsair-hub
- `crates/hid/src/discovery.rs`: filters `vendor_id == 0x1B1C`, `CorsairDevice::from_pid` hard-coded table, groups by `"{pid:04X}:{serial}"`, opens by (pid, serial, interface). Gaps:
  - No data-driven manifest; PIDs/protocol/limits live in code. iCUE keys everything off a table (pid -> proto/capabilities).
  - Identity = pid+serial string; no stable GUID, no PairingId fallback, so a device with empty/"unknown" serial (code uses `"unknown"`) collapses with others of the same PID.
  - No separate key for hub children (corsair device id) - our code reads child types from hub protocol; fine, but unknown child ids are not surfaced from a table like `SupportedCorsairSubdevices`.
  - No hub `RequiredPingInterval` (30 s) concept visible in discovery; no per-model PSU PWM floor (15 %).
  - QX120/QX140 treated identically by iCUE too, so no gap there.

## 4. Recommendations (prioritized)
1. Replace `from_pid` with a small static table `struct DeviceDef { vid, pid, proto, interface, caps }` (include 0x0C3F hub, 0x1C1F PSU, and ignore-list); unknown PIDs -> generic/"unsupported" entry with a log, never a panic.
2. Identity: `DeviceId { vid, pid, serial_or_pairing }`; if serial empty use hid path + log a warning; persist a generated `uuid` in config keyed by (vid,pid,serial) to mirror `cachedDeviceId` (uuid crate).
3. Add per-model constants from manifests: hub keep-alive 30 s (confirm against OpenLinkHub before use), 24 channels, 4096-byte lighting hunk, QxRgbFanHub 34 LEDs, QX max 2400 RPM, PSU PWM min 15 %.
4. Optional: model hub child ids (1 Fan/QX RGB, 5 LightingNode/link adapter, 11 PSU...) as an enum with `Unknown(u16)` using the table in 2.2.

## 5. Open questions
- Exact behaviour when `findManifest` returns null (generic device path) - not traced; callers are outside the DLLs assigned.
- How the QX120's physical size/fan type is distinguished (not in manifest; probably runtime/hub data).
- LS350 specifics: confirmed only as `LSAuroraSeries` channel type under the link adapter manifest; whether iCUE reports it as its own LINK device id is unknown.
- `ManifestBuilder.dll` conversion of JSON -> `Manifest` (property id 0x30 = DeviceKeys is inferred from the call constant, not named).
