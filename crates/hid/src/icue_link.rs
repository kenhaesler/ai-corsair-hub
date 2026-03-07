use anyhow::{Context, Result};
use hidapi::HidDevice;
use tracing::{debug, trace};

pub struct IcueLinkHub {
    device: HidDevice,
    serial: String,
}

impl IcueLinkHub {
    pub fn new(device: HidDevice, serial: String) -> Self {
        Self { device, serial }
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// Send a raw HID report and read the response.
    /// This is the foundation for protocol reverse engineering.
    pub fn send_raw(&self, data: &[u8]) -> Result<Vec<u8>> {
        trace!(
            serial = self.serial.as_str(),
            data = hex_string(data),
            "TX"
        );

        self.device
            .write(data)
            .context("Failed to write to iCUE LINK Hub")?;

        let mut buf = vec![0u8; 1024];
        let bytes_read = self
            .device
            .read_timeout(&mut buf, 500)
            .context("Failed to read from iCUE LINK Hub")?;

        buf.truncate(bytes_read);
        trace!(
            serial = self.serial.as_str(),
            data = hex_string(&buf),
            len = bytes_read,
            "RX"
        );

        Ok(buf)
    }

    /// Probe the device to discover its report size and basic info.
    /// Tries common report sizes used by other Corsair devices.
    pub fn probe(&self) -> Result<ProbeResult> {
        debug!(serial = self.serial.as_str(), "Probing iCUE LINK Hub");

        // Try reading the HID report descriptor info
        let mut manufacturer = String::from("unknown");
        let mut product = String::from("unknown");

        if let Ok(m) = self.device.get_manufacturer_string() {
            if let Some(m) = m {
                manufacturer = m;
            }
        }
        if let Ok(p) = self.device.get_product_string() {
            if let Some(p) = p {
                product = p;
            }
        }

        // Try to get serial from device
        let mut serial_from_device = self.serial.clone();
        if let Ok(s) = self.device.get_serial_number_string() {
            if let Some(s) = s {
                serial_from_device = s;
            }
        }

        // Try different known Corsair handshake commands to see what sticks
        let mut responses = Vec::new();

        // Commander Core style handshake: [0x00, 0x01] (open endpoint)
        let probes: &[(&str, &[u8])] = &[
            ("null_report", &[0x00]),
            ("cmd_core_open", &[0x00, 0x01]),
            ("cmd_core_close", &[0x00, 0x05]),
            ("cmd_pro_temp_0", &[0x00, 0x04, 0x01, 0x00]),
            ("firmware_query", &[0x00, 0x01, 0x00]),
        ];

        for (name, cmd) in probes {
            // Pad to various report sizes
            for &size in &[65u8, 97, 128] {
                let mut padded = vec![0u8; size as usize];
                let copy_len = cmd.len().min(padded.len());
                padded[..copy_len].copy_from_slice(&cmd[..copy_len]);

                match self.device.write(&padded) {
                    Ok(_) => {
                        let mut buf = vec![0u8; 1024];
                        match self.device.read_timeout(&mut buf, 200) {
                            Ok(n) if n > 0 => {
                                buf.truncate(n);
                                debug!(
                                    probe = *name,
                                    report_size = size,
                                    response_len = n,
                                    response = hex_string(&buf[..n.min(32)]),
                                    "Got response"
                                );
                                responses.push(ProbeResponse {
                                    probe_name: name.to_string(),
                                    report_size: size as usize,
                                    response: buf,
                                });
                            }
                            Ok(_) => {
                                debug!(probe = *name, report_size = size, "No response (timeout)");
                            }
                            Err(e) => {
                                debug!(
                                    probe = *name,
                                    report_size = size,
                                    error = %e,
                                    "Read error"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        debug!(
                            probe = *name,
                            report_size = size,
                            error = %e,
                            "Write error (wrong report size?)"
                        );
                    }
                }
            }
        }

        Ok(ProbeResult {
            manufacturer,
            product,
            serial: serial_from_device,
            responses,
        })
    }
}

#[derive(Debug)]
pub struct ProbeResult {
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
    pub responses: Vec<ProbeResponse>,
}

#[derive(Debug)]
pub struct ProbeResponse {
    pub probe_name: String,
    pub report_size: usize,
    pub response: Vec<u8>,
}

fn hex_string(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
}
