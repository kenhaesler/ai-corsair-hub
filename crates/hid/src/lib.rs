pub mod corsair_psu;
pub mod discovery;
pub mod icue_link;

pub use corsair_psu::{CorsairPsu, PsuStatus, RailReading};
pub use discovery::{DeviceGroup, DeviceScanner, InterfaceInfo};
pub use icue_link::{
    FanSpeed, FirmwareVersion, HubInfo, IcueLinkHub, LinkDevice, LinkDeviceType,
    TemperatureReading, port_power_factor,
};

pub(crate) fn hex_string(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
