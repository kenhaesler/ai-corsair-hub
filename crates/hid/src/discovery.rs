use anyhow::{Context, Result};
use corsair_common::{CorsairDevice, DeviceInfo, CORSAIR_VID};
use hidapi::HidApi;
use serde::Serialize;
use tracing::{debug, info};

pub struct DeviceScanner {
    api: HidApi,
}

impl DeviceScanner {
    pub fn new() -> Result<Self> {
        let api = HidApi::new().context("Failed to initialize HID API")?;
        Ok(Self { api })
    }

    /// Re-enumerate the USB bus so newly (re-)connected devices become visible.
    pub fn refresh(&mut self) -> Result<()> {
        self.api
            .refresh_devices()
            .context("Failed to refresh HID device list")?;
        Ok(())
    }

    pub fn scan(&self) -> Vec<DeviceInfo> {
        let mut devices = Vec::new();

        for dev in self.api.device_list() {
            if dev.vendor_id() != CORSAIR_VID {
                continue;
            }

            let pid = dev.product_id();
            let device_type = CorsairDevice::from_pid(pid);

            let serial = dev
                .serial_number()
                .unwrap_or("unknown")
                .to_string();

            let path = dev.path().to_string_lossy().to_string();
            let interface = dev.interface_number();

            debug!(
                pid = format!("0x{:04X}", pid),
                serial = serial.as_str(),
                interface,
                "Found Corsair device"
            );

            devices.push(DeviceInfo {
                device_type,
                vid: CORSAIR_VID,
                pid,
                serial,
                path,
                interface_number: interface,
            });
        }

        info!("Found {} Corsair device interface(s)", devices.len());
        devices
    }

    pub fn scan_grouped(&self) -> Vec<DeviceGroup> {
        let all = self.scan();
        let mut groups: std::collections::HashMap<String, DeviceGroup> =
            std::collections::HashMap::new();

        for dev in all {
            let key = format!("{:04X}:{}", dev.pid, dev.serial);
            let group = groups.entry(key).or_insert_with(|| DeviceGroup {
                device_type: dev.device_type,
                vid: dev.vid,
                pid: dev.pid,
                serial: dev.serial.clone(),
                interfaces: Vec::new(),
            });
            group.interfaces.push(InterfaceInfo {
                number: dev.interface_number,
                path: dev.path.clone(),
            });
        }

        let mut result: Vec<DeviceGroup> = groups.into_values().collect();
        result.sort_by(|a, b| a.pid.cmp(&b.pid).then(a.serial.cmp(&b.serial)));
        result
    }

    pub fn open_device(
        &self,
        pid: u16,
        serial: &str,
        interface: i32,
    ) -> Result<hidapi::HidDevice> {
        for dev in self.api.device_list() {
            if dev.vendor_id() == CORSAIR_VID
                && dev.product_id() == pid
                && dev.serial_number() == Some(serial)
                && dev.interface_number() == interface
            {
                let device = dev
                    .open_device(&self.api)
                    .context("Failed to open HID device")?;
                return Ok(device);
            }
        }
        anyhow::bail!(
            "Device not found: PID=0x{:04X}, serial={}, interface={}",
            pid,
            serial,
            interface
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceGroup {
    pub device_type: CorsairDevice,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
    pub interfaces: Vec<InterfaceInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceInfo {
    pub number: i32,
    pub path: String,
}
