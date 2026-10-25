//! Static catalog of Corsair USB devices and iCUE LINK sub-device type ids.
//!
//! Data only: every entry cites its source in the comment above it. Only the
//! iCUE LINK System Hub (0x0C3F) and the HX1500i (0x1C1F) are `Supported`; all
//! other entries exist so detection can name a device and decline to drive it.
//!
//! Source key: "D-clink" / "B-manifests" = `docs/research/icue-device-detection/`
//! sections; OpenLinkHub `src/devices/devices.go` (decimal PIDs converted to hex,
//! commit 949561e); liquidctl `liquidctl/driver/*.py` `_MATCHES` lists.

use crate::types::CORSAIR_VID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceFamily {
    IcueLinkHub,
    CommanderCore,
    CommanderPro,
    LightingNode,
    HidPsu,
    LinkPsuDongle,
    AsetekCooler,
    PlatinumCooler,
    Keyboard,
    Mouse,
    Headset,
    Other,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    IcueLink,
    HidPsu,
    CommanderCore,
    CommanderPro,
    Asetek,
    HydroPlatinum,
    Legacy,
    Bragi,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportLevel {
    Supported,
    DetectOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDef {
    pub pid: u16,
    pub name: &'static str,
    pub family: DeviceFamily,
    pub protocol: Protocol,
    /// HID interface number carrying the protocol, where known.
    pub data_interface: Option<i32>,
    pub is_bootloader: bool,
    pub support: SupportLevel,
}

const fn dev(
    pid: u16,
    name: &'static str,
    family: DeviceFamily,
    protocol: Protocol,
    data_interface: Option<i32>,
    is_bootloader: bool,
    support: SupportLevel,
) -> DeviceDef {
    DeviceDef {
        pid,
        name,
        family,
        protocol,
        data_interface,
        is_bootloader,
        support,
    }
}

/// Look up a device by USB ids. Only Corsair's VID is catalogued.
pub fn lookup(vid: u16, pid: u16) -> Option<&'static DeviceDef> {
    if vid != CORSAIR_VID {
        return None;
    }
    DEVICES
        .binary_search_by_key(&pid, |d| d.pid)
        .ok()
        .map(|i| &DEVICES[i])
}

/// Every catalogued device, sorted by PID.
pub fn all() -> &'static [DeviceDef] {
    DEVICES
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkCategory {
    Fan,
    PumpFan,
    LiquidCooler,
    LightingNode,
    Psu,
    DashboardLcd,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkSubdeviceDef {
    /// iCUE `corsairDeviceId` (hub `SupportedCorsairSubdevices` key).
    pub type_id: u16,
    pub category: LinkCategory,
    /// Only where an iCUE manifest or emulation entry names the model.
    pub model: Option<&'static str>,
    /// Only where a manifest gives a fixed LED count.
    pub default_led_count: Option<u16>,
}

const fn sub(
    type_id: u16,
    category: LinkCategory,
    model: Option<&'static str>,
    default_led_count: Option<u16>,
) -> LinkSubdeviceDef {
    LinkSubdeviceDef {
        type_id,
        category,
        model,
        default_led_count,
    }
}

/// Look up an iCUE LINK sub-device by its full 16-bit type id (low byte = type,
/// high byte = variant; variants are not folded onto the base type).
pub fn link_subdevice(type_id: u16) -> Option<&'static LinkSubdeviceDef> {
    LINK_SUBDEVICES
        .binary_search_by_key(&type_id, |d| d.type_id)
        .ok()
        .map(|i| &LINK_SUBDEVICES[i])
}

use LinkCategory as C;

// Categories: B-manifests 2.2 (hub `SupportedCorsairSubdevices`, decimal ids) and
// C-lifecycle-link 2.3. Models/LED counts only where those documents evidence them.
// NOTE id 0x0A: iCUE's hub manifest says Fan, but crates/hid `LinkDeviceType`
// maps 0x0A to GpuBlock. The catalog follows iCUE; the conflict is unresolved.
pub static LINK_SUBDEVICES: &[LinkSubdeviceDef] = &[
    // qx_rgb manifest; QX120 and QX140 share this id
    sub(0x0001, C::Fan, Some("iCUE LINK QX RGB"), Some(34)),
    sub(0x0002, C::Fan, None, None),
    sub(0x0003, C::Fan, None, None),
    sub(0x0004, C::Fan, None, None),
    // link_adapter manifest; hosts the LS350 strips, 0 fixed LEDs
    sub(0x0005, C::LightingNode, Some("iCUE LINK Adapter"), None),
    sub(0x0006, C::LiquidCooler, None, None),
    // C-lifecycle-link: DeviceEmulation entry
    sub(0x0007, C::LiquidCooler, Some("iCUE LINK H100i RGB"), None),
    sub(0x0009, C::LiquidCooler, None, None),
    sub(0x000A, C::Fan, None, None),
    sub(0x000B, C::Psu, None, None),
    sub(0x000C, C::LiquidCooler, None, None),
    sub(0x000D, C::LightingNode, None, None),
    sub(0x000E, C::LiquidCooler, None, None),
    sub(0x000F, C::Fan, None, None),
    sub(0x0010, C::PumpFan, None, None),
    sub(0x0011, C::LiquidCooler, None, None),
    sub(0x0013, C::Fan, None, None),
    sub(0x0014, C::LightingNode, None, None),
    sub(0x0015, C::Psu, None, None),
    sub(0x0019, C::LiquidCooler, None, None),
    sub(0x001A, C::Fan, None, None),
    sub(0x001B, C::LightingNode, None, None),
    sub(0x001D, C::Psu, None, None),
    sub(0x0105, C::LightingNode, None, None),
    sub(0x0107, C::LiquidCooler, None, None),
    sub(0x010B, C::Psu, None, None),
    sub(0x0111, C::LiquidCooler, None, None),
    sub(0x0115, C::Psu, None, None),
    sub(0x011A, C::LiquidCooler, None, None),
    sub(0x0205, C::LightingNode, None, None),
    sub(0x0207, C::LiquidCooler, None, None),
    sub(0x020B, C::Psu, None, None),
    sub(0x0211, C::LiquidCooler, None, None),
    sub(0x0307, C::LiquidCooler, None, None),
    sub(0x0311, C::LiquidCooler, None, None),
    sub(0x0407, C::LiquidCooler, None, None),
    sub(0x0411, C::LiquidCooler, None, None),
    sub(0x0507, C::LiquidCooler, None, None),
    sub(0x0511, C::LiquidCooler, None, None),
    sub(0x081A, C::DashboardLcd, None, None),
];

pub static DEVICES: &[DeviceDef] = &[
    // B-manifests §2.2
    dev(
        0x0006,
        "Dominator Titanium RGB (DRAM, LLAccess/Kopis)",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // B-manifests §2.2
    dev(
        0x040B,
        "Dominator Titanium RGB (DRAM, LLAccess/Kopis)",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (st100)
    dev(
        0x0A34,
        "ST100 LED driver",
        DeviceFamily::LightingNode,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A3D,
        "Corsair VIRTUOSO SE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A3E,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A3F,
        "Corsair VIRTUOSO SE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A40,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A41,
        "Corsair VIRTUOSO",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A42,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A43,
        "Corsair VIRTUOSO",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A44,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A51,
        "Corsair VOID ELITE WIRELESS",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A55,
        "Corsair VOID ELITE WIRELESS",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A62,
        "Corsair VIRTUOSO RGB WIRELESS XT",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A64,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A69,
        "Corsair HS80 RGB WIRELESS",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A6B,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A73,
        "Corsair HEADSET DONGLE",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A88,
        "Corsair HS80 RGB USB",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x0A97,
        "Corsair HS80 MAX WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl asetek; OpenLinkHub devices.go (hydro)
    dev(
        0x0C02,
        "Corsair Hydro H80i GT",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl asetek; OpenLinkHub devices.go (hydro)
    dev(
        0x0C03,
        "Corsair Hydro H100i GTX",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (Coolit)
    dev(
        0x0C04,
        "Coolit bridge (Commander/Link)",
        DeviceFamily::Other,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (RobbinsPSU)
    dev(
        0x0C06,
        "Robbins PSU (RM, low-level)",
        DeviceFamily::HidPsu,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl asetek; OpenLinkHub devices.go (hydro)
    dev(
        0x0C07,
        "Corsair Hydro H110i GTX",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl asetek; OpenLinkHub devices.go (hydro)
    dev(
        0x0C08,
        "Corsair Hydro H80i v2",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl asetek; OpenLinkHub devices.go (hydro)
    dev(
        0x0C09,
        "Corsair Hydro H100i v2",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl asetek; OpenLinkHub devices.go (hydro)
    dev(
        0x0C0A,
        "Corsair Hydro H115i",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (Antigua); liquidctl commander_pro; OpenLinkHub devices.go (lnpro)
    dev(
        0x0C0B,
        "Corsair Lighting Node Pro",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x0C0B)
    dev(
        0x0C0C,
        "Corsair Lighting Node Pro bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (Barbuda); liquidctl commander_pro; OpenLinkHub devices.go (cpro)
    dev(
        0x0C10,
        "Corsair Commander Pro",
        DeviceFamily::CommanderPro,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x0C10)
    dev(
        0x0C11,
        "Corsair Commander Pro bootloader",
        DeviceFamily::CommanderPro,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (AsetekPro)
    dev(
        0x0C12,
        "Corsair Hydro H150i Platinum",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (AsetekPro)
    dev(
        0x0C13,
        "Corsair Hydro H115i Platinum",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (CujoCooler); OpenLinkHub devices.go
    dev(
        0x0C14,
        "Corsair One (Cujo cooler)",
        DeviceFamily::Other,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (AsetekPro)
    dev(
        0x0C15,
        "Corsair Hydro H100i Platinum",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (AsetekPro)
    dev(
        0x0C16,
        "Asetek Pro cooler (model unnamed)",
        DeviceFamily::AsetekCooler,
        Protocol::Asetek,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C17,
        "Corsair Hydro H115i RGB Platinum",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C18,
        "Corsair Hydro H100i RGB Platinum",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C19,
        "Corsair Hydro H100i RGB Platinum SE",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (Kauai); liquidctl commander_pro; OpenLinkHub devices.go (lncore)
    dev(
        0x0C1A,
        "Corsair Lighting Node Core",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x0C1A)
    dev(
        0x0C1B,
        "Corsair Lighting Node Core bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // existing CorsairDevice; liquidctl commander_core; OpenLinkHub devices.go
    dev(
        0x0C1C,
        "Corsair Commander Core",
        DeviceFamily::CommanderCore,
        Protocol::CommanderCore,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x0C1E,
        "Pro-family lighting device 'Borealis'",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x0C1E)
    dev(
        0x0C1F,
        "Pro-family lighting device 'Borealis' bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C20,
        "Corsair Hydro H100i Pro XT",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C21,
        "Corsair Hydro H115i Pro XT",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C22,
        "Corsair Hydro H150i Pro XT",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (Lightsabers); OpenLinkHub devices.go (lt100)
    dev(
        0x0C23,
        "LT100 Smart Lighting Tower",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x0C23)
    dev(
        0x0C24,
        "LT100 Smart Lighting Tower bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C29,
        "Corsair Hydro H60i Pro XT",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // existing CorsairDevice; liquidctl commander_core; OpenLinkHub devices.go
    dev(
        0x0C2A,
        "Corsair Commander Core XT",
        DeviceFamily::CommanderCore,
        Protocol::CommanderCore,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C2D,
        "Corsair iCUE H100i RGB Pro XT (alt revision)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C2E,
        "Platinum-family cooler (model unnamed)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C2F,
        "Platinum-family cooler (model unnamed)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C30,
        "Platinum-family cooler (model unnamed)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // existing CorsairDevice; liquidctl commander_core; OpenLinkHub devices.go (cc)
    dev(
        0x0C32,
        "Corsair Commander ST",
        DeviceFamily::CommanderCore,
        Protocol::CommanderCore,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (lcd, cc)
    dev(
        0x0C33,
        "iCUE LCD pump cover (model unnamed)",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C34,
        "Platinum-family cooler (model unnamed)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C35,
        "Corsair iCUE H100i Elite RGB",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C36,
        "Corsair iCUE H115i Elite RGB",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C37,
        "Corsair iCUE H150i Elite RGB",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (lcd, cc)
    dev(
        0x0C39,
        "iCUE LCD pump cover (model unnamed)",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // B-manifests §2.2; D-clink §2.3 (absent from clink); OpenLinkHub devices.go (lsh)
    dev(
        0x0C3F,
        "iCUE LINK System Hub",
        DeviceFamily::IcueLinkHub,
        Protocol::IcueLink,
        Some(0),
        false,
        SupportLevel::Supported,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C40,
        "Corsair iCUE H100i Elite RGB (White)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (PlatinumCooler)
    dev(
        0x0C41,
        "Corsair iCUE H150i Elite RGB (White)",
        DeviceFamily::PlatinumCooler,
        Protocol::HydroPlatinum,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (xc7)
    dev(
        0x0C42,
        "XC7 Elite LCD CPU water block",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (lcd)
    dev(
        0x0C43,
        "iCUE LCD pump cover (model unnamed)",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (lcd, AIO)
    dev(
        0x0C4E,
        "iCUE LCD AIO (model unnamed)",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (nautilusLcd)
    dev(
        0x0C55,
        "iCUE Nautilus LCD cap",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (cduo)
    dev(
        0x0C56,
        "iCUE Commander Duo",
        DeviceFamily::CommanderCore,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (nautilusLcd)
    dev(
        0x0C57,
        "iCUE Nautilus LCD cap",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B11,
        "Corsair K95",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B17,
        "Corsair K65 RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B1E,
        "Corsair SCIMITAR RGB",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B2D,
        "Corsair K95 PLATINUM",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B2E,
        "Corsair M65 PRO RGB",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B33,
        "Corsair K70 LUX RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B34,
        "Corsair GLAIVE RGB",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B36,
        "Corsair K70 LUX",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B38,
        "Corsair K70 RGB RAPIDFIRE",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B39,
        "Corsair K65 RGB RAPIDFIRE",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B3B,
        "Corsair MM800 RGB POLARIS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B3D,
        "Corsair K55 RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B3E,
        "Corsair SCIMITAR PRO RGB",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B48,
        "Corsair STRAFE RGB MK2",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B49,
        "Corsair K70 RGB MK2",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B4B,
        "Corsair DARK CORE RGB SE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B4C,
        "Corsair IRONCLAW RGB WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B4F,
        "Corsair K68 RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B51,
        "Corsair DARK CORE RGB SE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B55,
        "Corsair K70 RGB MK2",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B5A,
        "Corsair M65 RGB ELITE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B5C,
        "Corsair NIGHTSWORD RGB",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B5D,
        "Corsair IRONCLAW RGB",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B5E,
        "Corsair HARPOON",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B65,
        "Corsair SLIPSTREAM WIRELESS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B6B,
        "Corsair K70 RGB MK2",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B6E,
        "Corsair K57 RGB WIRELESS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B70,
        "Corsair M55 RGB PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B73,
        "Corsair K70 RGB TKL",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B74,
        "Corsair GLAIVE RGB PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B75,
        "Corsair HARPOON RGB PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B79,
        "Corsair SABRE RGB PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B7A,
        "Corsair SABRE PRO CS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B7C,
        "Corsair K100",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B7D,
        "Corsair K100",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B7E,
        "Corsair DARK CORE RGB PRO SE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B7F,
        "Corsair SLIPSTREAM WIRELESS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B80,
        "Corsair DARK CORE RGB PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B81,
        "Corsair DARK CORE RGB PRO Gaming Dongle",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B89,
        "Corsair K95 PLATINUM XT",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B8B,
        "Corsair SCIMITAR RGB ELITE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go (nexus)
    dev(
        0x1B8E,
        "iCUE NEXUS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B93,
        "Corsair KATAR PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B94,
        "Corsair KATAR PRO WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B98,
        "Corsair SABRE RGB PRO WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B9B,
        "Corsair MM700 RGB",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1B9E,
        "Corsair M65 RGB ULTRA",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BA0,
        "Corsair K60 RGB PRO",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BA1,
        "Corsair K55 RGB PRO XT",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BA4,
        "Corsair K55 PRO RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BA6,
        "Corsair SLIPSTREAM WIRELESS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BAB,
        "Corsair K100 AIR",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BAC,
        "Corsair KATAR PRO XT",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BAD,
        "Corsair K60 RGB PRO LP",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BAF,
        "Corsair K65 RGB MINI",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BB2,
        "Corsair DARKSTAR RGB WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BB3,
        "Corsair K70 PRO",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BB5,
        "Corsair M65 RGB ULTRA WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BB6,
        "Corsair K70 PPO MINI",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BB8,
        "Corsair NIGHTSABRE WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BB9,
        "Corsair K70 RGB TKL CS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BC0,
        "Corsair K70 MAX",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BC3,
        "Corsair K65 RGB MINI",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BC4,
        "Corsair K70 PRO",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BC5,
        "Corsair K100",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BC6,
        "Corsair K70 PRO",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BC9,
        "Corsair MM700 3XL RGB",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BCF,
        "Corsair K65 RGB MINI",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BD4,
        "Corsair K70 PRO",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BD6,
        "Corsair M65 RGB ULTRA WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BD7,
        "Corsair K65 PRO MINI",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BDB,
        "Corsair SCIMITAR RGB ELITE WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BDC,
        "Corsair SLIPSTREAM WIRELESS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BE3,
        "Corsair SCIMITAR RGB ELITE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BF0,
        "Corsair M75",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BF1,
        "Corsair M75 WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BF2,
        "Corsair M75 AIR WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BFD,
        "Corsair K70 CORE RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BFE,
        "Corsair K55 CORE RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x1BFF,
        "Corsair K70 CORE RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (FlextronicsPSU)
    dev(
        0x1C00,
        "Flextronics PSU (via Link dongle)",
        DeviceFamily::LinkPsuDongle,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (FlextronicsPSU)
    dev(
        0x1C02,
        "Flextronics PSU (via Link dongle)",
        DeviceFamily::LinkPsuDongle,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1C03,
        "Corsair HX550i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1C04,
        "Corsair HX650i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C05,
        "Corsair HX750i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C06,
        "Corsair HX850i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C07,
        "Corsair HX1000i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C08,
        "Corsair HX1200i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1C09,
        "Corsair RM550i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu
    dev(
        0x1C0A,
        "Corsair RM650i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu
    dev(
        0x1C0B,
        "Corsair RM750i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C0C,
        "Corsair RM850i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C0D,
        "Corsair RM1000i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (FlextronicsPSU)
    dev(
        0x1C0E,
        "Corsair AX850i (via Link dongle)",
        DeviceFamily::LinkPsuDongle,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (FlextronicsPSU)
    dev(
        0x1C0F,
        "Corsair AX1000i (via Link dongle)",
        DeviceFamily::LinkPsuDongle,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (FlextronicsPSU)
    dev(
        0x1C10,
        "Corsair AX1300i (via Link dongle)",
        DeviceFamily::LinkPsuDongle,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (FlextronicsPSU)
    dev(
        0x1C11,
        "Corsair AX1600i (via Link dongle)",
        DeviceFamily::LinkPsuDongle,
        Protocol::Legacy,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C1E,
        "Corsair HX1000i (2021)",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; B-manifests §2.2; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C1F,
        "Corsair HX1500i",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::Supported,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C23,
        "Corsair HX1200i (2023)",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1C24,
        "Corsair AX1300i (Axion)",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1C25,
        "Corsair AX1600i (Axion)",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3; liquidctl corsair_hid_psu; OpenLinkHub devices.go
    dev(
        0x1C27,
        "Corsair HX1200i (2025)",
        DeviceFamily::HidPsu,
        Protocol::HidPsu,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (Jotun); liquidctl commander_pro; OpenLinkHub devices.go (cpro)
    dev(
        0x1D00,
        "Corsair Obsidian 1000D hub (Commander Pro)",
        DeviceFamily::CommanderPro,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x1D00)
    dev(
        0x1D01,
        "Corsair Obsidian 1000D hub (Commander Pro) bootloader",
        DeviceFamily::CommanderPro,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1D02,
        "Pro-family lighting device 'Revenant'",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x1D02)
    dev(
        0x1D03,
        "Pro-family lighting device 'Revenant' bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (SpecOmega); OpenLinkHub devices.go (lnpro)
    dev(
        0x1D04,
        "Lighting Node Pro (SPEC-OMEGA RGB bundle)",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x1D04)
    dev(
        0x1D05,
        "Lighting Node Pro (SPEC-OMEGA RGB bundle) bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1D06,
        "Pro-family lighting device 'Snark'",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x1D06)
    dev(
        0x1D07,
        "Pro-family lighting device 'Snark' bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1D08,
        "Pro-family lighting device 'Surtur'",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x1D08)
    dev(
        0x1D09,
        "Pro-family lighting device 'Surtur' bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3
    dev(
        0x1D0A,
        "Pro-family lighting device 'Crystal'",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // D-clink §2.3 (boot PID of 0x1D0A)
    dev(
        0x1D0B,
        "Pro-family lighting device 'Crystal' bootloader",
        DeviceFamily::LightingNode,
        Protocol::CommanderPro,
        None,
        true,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A02,
        "Corsair VIRTUOSO MAX WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A03,
        "Corsair VIRTUOSO MAX WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A05,
        "Corsair VOID MAX WIRELESS V2",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A08,
        "Corsair VOID WIRELESS V2",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A0C,
        "Corsair HS80 MAX WIRELESS",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A0F,
        "Corsair HS80 MAX WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2A12,
        "Corsair VOID MAX WIRELESS V2",
        DeviceFamily::Headset,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B00,
        "Corsair SLIPSTREAM WIRELESS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B01,
        "Corsair K70 CORE TKL",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B02,
        "Corsair K70 CORE TKL",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B03,
        "Corsair M55",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B04,
        "Corsair MAKR 75",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B05,
        "Corsair K55 CORE RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B07,
        "Corsair K65 PLUS WIRELESS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B08,
        "Corsair M75 WIRELESS",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B0A,
        "Corsair K70 CORE RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B0D,
        "Corsair VANGUARD 96",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B0E,
        "Corsair VANGUARD 96 PRO",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B0F,
        "Corsair VANGUARD 96 WIRELESS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B10,
        "Corsair K65 PLUS WIRELESS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B11,
        "Corsair K65 PLUS WIRELESS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B14,
        "Corsair K70 PRO TKL",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B1B,
        "Corsair SLIPSTREAM WIRELESS",
        DeviceFamily::Other,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B1C,
        "Corsair NIGHTSWORD V2",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B20,
        "Corsair K55 CORE TKL RGB",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B21,
        "Corsair VANGUARD 99 AIR",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B22,
        "Corsair SCIMITAR ELITE WIRELESS SE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B23,
        "Corsair K65 PLUS WIRELESS",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B28,
        "Corsair SABRE V2 PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B29,
        "Corsair SABRE V2 PRO MG",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B2A,
        "Corsair SABRE V2 PRO DONGLE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B2E,
        "Corsair HARPOON V2",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B31,
        "Corsair CLIPPER MINI PRO 60",
        DeviceFamily::Keyboard,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B32,
        "Corsair IRONCLAW WIRELESS SE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x2B34,
        "Corsair SABRE V2 PRO DONGLE",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x3A05,
        "Corsair SCUF ENVISION PRO V2",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x3A08,
        "Corsair SCUF PC Controller Dongle V2",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x434D,
        "Corsair SCUF ENVISION PRO",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
    // OpenLinkHub devices.go
    dev(
        0x434E,
        "Corsair SCUF PC Controller Dongle",
        DeviceFamily::Mouse,
        Protocol::Unknown,
        None,
        false,
        SupportLevel::DetectOnly,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CorsairDevice;

    #[test]
    fn lookup_hits() {
        let hub = lookup(CORSAIR_VID, 0x0C3F).unwrap();
        assert_eq!(hub.family, DeviceFamily::IcueLinkHub);
        assert_eq!(hub.protocol, Protocol::IcueLink);
        assert_eq!(hub.data_interface, Some(0));
        let psu = lookup(CORSAIR_VID, 0x1C1F).unwrap();
        assert_eq!(psu.family, DeviceFamily::HidPsu);
        assert_eq!(psu.name, "Corsair HX1500i");
    }

    #[test]
    fn lookup_misses() {
        assert!(lookup(0x1234, 0x0C3F).is_none());
        assert!(lookup(CORSAIR_VID, 0xFFFF).is_none());
    }

    #[test]
    fn pids_unique_and_sorted() {
        for w in all().windows(2) {
            assert!(
                w[0].pid < w[1].pid,
                "{:#06X} not strictly before {:#06X}",
                w[0].pid,
                w[1].pid
            );
        }
        for w in LINK_SUBDEVICES.windows(2) {
            assert!(w[0].type_id < w[1].type_id);
        }
    }

    #[test]
    fn only_hub_and_hx1500i_supported() {
        let supported: Vec<u16> = all()
            .iter()
            .filter(|d| d.support == SupportLevel::Supported)
            .map(|d| d.pid)
            .collect();
        assert_eq!(supported, vec![0x0C3F, 0x1C1F]);
    }

    #[test]
    fn bootloader_pids_flagged() {
        assert!(lookup(CORSAIR_VID, 0x0C11).unwrap().is_bootloader);
        assert!(!lookup(CORSAIR_VID, 0x0C10).unwrap().is_bootloader);
    }

    #[test]
    fn existing_corsair_device_pids_are_catalogued() {
        for pid in [0x0C3F, 0x0C10, 0x0C1C, 0x0C2A, 0x0C32, 0x0C0B, 0x1C1F] {
            assert!(lookup(CORSAIR_VID, pid).is_some(), "{pid:#06X} missing");
            assert_eq!(CorsairDevice::from_pid(pid).pid(), pid);
        }
    }

    #[test]
    fn unknown_variant_catalog_name() {
        assert_eq!(
            CorsairDevice::from_pid(0x1C05).catalog_name(),
            Some("Corsair HX750i")
        );
        assert_eq!(CorsairDevice::from_pid(0xFFFF).catalog_name(), None);
        assert_eq!(CorsairDevice::IcueLinkHub.catalog_name(), None);
    }

    #[test]
    fn link_subdevices() {
        let qx = link_subdevice(1).unwrap();
        assert_eq!(qx.category, LinkCategory::Fan);
        assert_eq!(qx.default_led_count, Some(34));
        assert_eq!(
            link_subdevice(5).unwrap().category,
            LinkCategory::LightingNode
        );
        assert_eq!(
            link_subdevice(7).unwrap().category,
            LinkCategory::LiquidCooler
        );
        assert_eq!(link_subdevice(0x0B).unwrap().category, LinkCategory::Psu);
        assert_eq!(
            link_subdevice(0x10).unwrap().category,
            LinkCategory::PumpFan
        );
        assert_eq!(
            link_subdevice(0x081A).unwrap().category,
            LinkCategory::DashboardLcd
        );
        assert!(link_subdevice(0x0008).is_none());
        assert!(link_subdevice(0x7777).is_none());
    }
}
