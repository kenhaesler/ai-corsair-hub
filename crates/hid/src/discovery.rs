use anyhow::{Context, Result};
use corsair_common::{CorsairDevice, DeviceInfo, CORSAIR_VID};
use hidapi::HidApi;
use serde::Serialize;
use std::ffi::CString;
use tracing::{debug, info};

use crate::topology::{DevnodeContext, RawInterface, Snapshot};

#[cfg(windows)]
fn devnode_context(path: &str) -> DevnodeContext {
    use crate::devnode::{interface_path_to_instance_id, query_devnode, usb_device_instance_id};

    let Some(instance_id) = interface_path_to_instance_id(path) else {
        return DevnodeContext::default();
    };
    let info = query_devnode(&instance_id).ok();
    let usb_instance_id = usb_device_instance_id(&instance_id);
    DevnodeContext {
        instance_id: Some(instance_id),
        info,
        usb_instance_id,
    }
}

#[cfg(not(windows))]
fn devnode_context(_path: &str) -> DevnodeContext {
    DevnodeContext::default()
}

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

    /// Enriched enumeration: Corsair interfaces grouped by [`DeviceKey`], with Bluetooth
    /// transports and devnodes with a problem recorded in `excluded`.
    pub fn snapshot(&self) -> Snapshot {
        let records = self
            .api
            .device_list()
            .filter(|dev| dev.vendor_id() == CORSAIR_VID)
            .map(|dev| {
                let path = dev.path().to_string_lossy().to_string();
                let ctx = devnode_context(&path);
                let raw = RawInterface {
                    vid: dev.vendor_id(),
                    pid: dev.product_id(),
                    serial: dev.serial_number().map(String::from),
                    interface_number: dev.interface_number(),
                    path,
                    usage_page: dev.usage_page(),
                    usage: dev.usage(),
                };
                (raw, ctx)
            })
            .collect::<Vec<_>>();
        let snapshot = Snapshot::from_records(records);
        debug!(
            devices = snapshot.devices.len(),
            excluded = snapshot.excluded.len(),
            "Enumerated Corsair devices"
        );
        snapshot
    }

    /// Open a specific interface by its hidapi path (as reported in a [`Snapshot`]).
    pub fn open_path(&self, path: &str) -> Result<hidapi::HidDevice> {
        let c_path = CString::new(path).context("HID path contains an interior NUL")?;
        self.api
            .open_path(&c_path)
            .with_context(|| format!("Failed to open HID path {path}"))
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
