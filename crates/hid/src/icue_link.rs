use anyhow::{bail, Context, Result};
use hidapi::HidDevice;
use serde::Serialize;
use std::fmt;
use std::thread;
use std::time::Duration;
use tracing::{debug, trace, warn};

// Wire protocol constants
const PACKET_SIZE: usize = 512;
const WRITE_SIZE: usize = 513; // report ID + 512
const HEADER_SIZE: usize = 3; // [report_id, 0x00, 0x01]
const DATA_INTERFACE: i32 = 0; // MI_00
const READ_TIMEOUT_MS: i32 = 500;
const SOFTWARE_MODE_DELAY_MS: u64 = 500;

// Commands (placed at byte[3] of write buffer)
const CMD_WAKE: [u8; 4] = [0x01, 0x03, 0x00, 0x02]; // Enter software mode
const CMD_SLEEP: [u8; 4] = [0x01, 0x03, 0x00, 0x01]; // Enter hardware mode
const CMD_FIRMWARE: [u8; 2] = [0x02, 0x13]; // Get firmware version
const CMD_OPEN: [u8; 2] = [0x0D, 0x01]; // Open endpoint
const CMD_CLOSE: [u8; 3] = [0x05, 0x01, 0x01]; // Close endpoint
const CMD_READ: [u8; 2] = [0x08, 0x01]; // Read from endpoint
const CMD_WRITE: [u8; 2] = [0x06, 0x01]; // Write to endpoint

// Endpoint modes
const MODE_SPEEDS: u8 = 0x17;
const MODE_SET_SPEED: u8 = 0x18;
const MODE_TEMPS: u8 = 0x21;
const MODE_DEVICES: u8 = 0x36;

// Data types for response matching and write payload headers
const DATA_TYPE_DEVICES: [u8; 2] = [0x21, 0x00];
#[cfg(test)]
const DATA_TYPE_SPEEDS: [u8; 2] = [0x25, 0x00];
#[cfg(test)]
const DATA_TYPE_TEMPS: [u8; 2] = [0x10, 0x00];
const DATA_TYPE_SET_SPEED: [u8; 2] = [0x07, 0x00];

// Response status codes at response[3]
const STATUS_OK: u8 = 0x00;
#[allow(dead_code)]
const STATUS_WRONG_MODE: u8 = 0x03;

// --- Public types ---

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u16,
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinkDeviceType {
    QxFan,        // 0x01
    LxFan,        // 0x02
    RxMaxRgbFan,  // 0x03
    RxMaxFan,     // 0x04
    LinkAdapter,  // 0x05
    LiquidCooler, // 0x07
    WaterBlock,   // 0x09
    GpuBlock,     // 0x0A
    Psu,          // 0x0B
    PumpXd5,      // 0x0C
    Xg7Block,     // 0x0D
    RxRgbFan,     // 0x0F
    VrmCooler,    // 0x10
    TitanCooler,  // 0x11
    RxFan,        // 0x13
    PumpXd6,      // 0x19
    CommanderDuo, // 0x1B
    Unknown(u8),
}

impl LinkDeviceType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x01 => Self::QxFan,
            0x02 => Self::LxFan,
            0x03 => Self::RxMaxRgbFan,
            0x04 => Self::RxMaxFan,
            0x05 => Self::LinkAdapter,
            0x07 => Self::LiquidCooler,
            0x09 => Self::WaterBlock,
            0x0A => Self::GpuBlock,
            0x0B => Self::Psu,
            0x0C => Self::PumpXd5,
            0x0D => Self::Xg7Block,
            0x0F => Self::RxRgbFan,
            0x10 => Self::VrmCooler,
            0x11 => Self::TitanCooler,
            0x13 => Self::RxFan,
            0x19 => Self::PumpXd6,
            0x1B => Self::CommanderDuo,
            other => Self::Unknown(other),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::QxFan => "QX Fan",
            Self::LxFan => "LX Fan",
            Self::RxMaxRgbFan => "RX MAX RGB Fan",
            Self::RxMaxFan => "RX MAX Fan",
            Self::LinkAdapter => "LINK Adapter",
            Self::LiquidCooler => "Liquid Cooler",
            Self::WaterBlock => "Water Block",
            Self::GpuBlock => "GPU Block",
            Self::Psu => "PSU",
            Self::PumpXd5 => "XD5 Pump",
            Self::Xg7Block => "XG7 RGB Block",
            Self::RxRgbFan => "RX RGB Fan",
            Self::VrmCooler => "VRM Cooler",
            Self::TitanCooler => "Titan Cooler",
            Self::RxFan => "RX Fan",
            Self::PumpXd6 => "XD6 Pump",
            Self::CommanderDuo => "Commander DUO",
            Self::Unknown(b) => {
                // Can't return dynamic string from &'static str, use generic label
                let _ = b;
                "Unknown Device"
            }
        }
    }

    pub fn is_pump(&self) -> bool {
        matches!(self, Self::PumpXd5 | Self::PumpXd6)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkDevice {
    pub channel: u8,
    pub device_type: LinkDeviceType,
    pub model: u8,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FanSpeed {
    pub channel: u8,
    pub rpm: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemperatureReading {
    pub channel: u8,
    pub temp_celsius: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubInfo {
    pub firmware: FirmwareVersion,
    pub devices: Vec<LinkDevice>,
}

// --- Hub implementation ---

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

    pub fn data_interface() -> i32 {
        DATA_INTERFACE
    }

    // --- Low-level transfer ---

    /// Send a command with optional data, read 512-byte response, check status.
    fn transfer(&self, cmd: &[u8], data: &[u8]) -> Result<Vec<u8>> {
        let packet = build_packet(cmd, data);

        trace!(
            serial = self.serial.as_str(),
            data = hex_string(&packet[..packet.len().min(32)]),
            len = packet.len(),
            "TX"
        );

        self.device
            .write(&packet)
            .context("Failed to write to iCUE LINK Hub")?;

        let mut buf = vec![0u8; PACKET_SIZE];
        let bytes_read = self
            .device
            .read_timeout(&mut buf, READ_TIMEOUT_MS)
            .context("Failed to read from iCUE LINK Hub")?;

        if bytes_read == 0 {
            bail!("No response from hub (timeout)");
        }

        buf.truncate(bytes_read);

        trace!(
            serial = self.serial.as_str(),
            data = hex_string(&buf[..buf.len().min(32)]),
            len = bytes_read,
            "RX"
        );

        Ok(buf)
    }

    /// Endpoint read: close → open(mode) → read → close
    fn read_endpoint(&self, mode: u8) -> Result<Vec<u8>> {
        // Close any stale endpoint
        self.transfer(&CMD_CLOSE, &[])?;

        // Open for the requested mode
        self.transfer(&CMD_OPEN, &[mode])?;

        // Read data
        let response = self.transfer(&CMD_READ, &[])?;

        // Close endpoint
        self.transfer(&CMD_CLOSE, &[])?;

        Ok(response)
    }

    /// Endpoint write: close → open(mode) → write(data) → close
    fn write_endpoint(&self, mode: u8, data: &[u8]) -> Result<()> {
        self.transfer(&CMD_CLOSE, &[])?;
        self.transfer(&CMD_OPEN, &[mode])?;
        self.transfer(&CMD_WRITE, data)?;
        self.transfer(&CMD_CLOSE, &[])?;
        Ok(())
    }

    // --- High-level API ---

    /// Get hub firmware version.
    pub fn get_firmware_version(&self) -> Result<FirmwareVersion> {
        let resp = self.transfer(&CMD_FIRMWARE, &[])?;
        parse_firmware_version(&resp)
    }

    /// Enter software control mode. Must be called before reading/writing endpoints.
    /// Includes a 500ms delay for the hub to stabilize.
    pub fn enter_software_mode(&self) -> Result<()> {
        debug!(serial = self.serial.as_str(), "Entering software mode");
        let resp = self.transfer(&CMD_WAKE, &[])?;

        if resp.len() > 3 && resp[3] != STATUS_OK {
            warn!(
                serial = self.serial.as_str(),
                status = resp[3],
                "Non-OK status entering software mode"
            );
        }

        thread::sleep(Duration::from_millis(SOFTWARE_MODE_DELAY_MS));
        Ok(())
    }

    /// Return hub to hardware (firmware) control mode.
    pub fn enter_hardware_mode(&self) -> Result<()> {
        debug!(serial = self.serial.as_str(), "Entering hardware mode");
        self.transfer(&CMD_SLEEP, &[])?;
        Ok(())
    }

    /// Enumerate connected iCUE LINK devices on the daisy chain.
    pub fn enumerate_devices(&self) -> Result<Vec<LinkDevice>> {
        let resp = self.read_endpoint(MODE_DEVICES)?;

        // Check for continuation: if response[4:6] matches DATA_TYPE_DEVICES, read more
        let mut payload = Vec::new();
        if resp.len() > 6 {
            payload.extend_from_slice(&resp[4..]);
        }

        // Check if we need continuation reads
        if resp.len() > 5 && resp[4] == DATA_TYPE_DEVICES[0] && resp[5] == DATA_TYPE_DEVICES[1] {
            // There may be more data — do another read cycle
            let resp2 = self.read_endpoint(MODE_DEVICES)?;
            if resp2.len() > 4 {
                payload.extend_from_slice(&resp2[4..]);
            }
        }

        parse_device_entries(&resp)
    }

    /// Read current fan/pump RPMs.
    pub fn get_speeds(&self) -> Result<Vec<FanSpeed>> {
        let resp = self.read_endpoint(MODE_SPEEDS)?;
        parse_speed_entries(&resp)
    }

    /// Read temperatures from sensors on connected devices.
    pub fn get_temperatures(&self) -> Result<Vec<TemperatureReading>> {
        let resp = self.read_endpoint(MODE_TEMPS)?;
        parse_temperature_entries(&resp)
    }

    /// Set fan/pump speeds. Each entry is (channel, percent 0-100).
    /// Minimum 20% for fans, 50% for pumps (caller must enforce pump minimum).
    pub fn set_speeds(&self, targets: &[(u8, u8)]) -> Result<()> {
        let data = build_speed_payload(targets);

        // Retry up to 3 times on failure
        let mut last_err = None;
        for attempt in 0..3 {
            match self.write_endpoint(MODE_SET_SPEED, &data) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(
                        serial = self.serial.as_str(),
                        attempt = attempt + 1,
                        error = %e,
                        "set_speeds failed, retrying"
                    );
                    last_err = Some(e);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Full initialization: get firmware, enter software mode, enumerate devices.
    pub fn initialize(&self) -> Result<HubInfo> {
        let firmware = self.get_firmware_version()?;
        debug!(
            serial = self.serial.as_str(),
            firmware = %firmware,
            "Hub firmware"
        );

        self.enter_software_mode()?;

        let devices = self.enumerate_devices()?;
        debug!(
            serial = self.serial.as_str(),
            count = devices.len(),
            "Enumerated devices"
        );

        Ok(HubInfo { firmware, devices })
    }
}

// --- Packet building ---

fn build_packet(cmd: &[u8], data: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; WRITE_SIZE];
    buf[0] = 0x00; // HID report ID
    buf[1] = 0x00; // Normal (not PSU subdevice)
    buf[2] = 0x01; // Command flag — always 0x01

    let cmd_start = HEADER_SIZE;
    buf[cmd_start..cmd_start + cmd.len()].copy_from_slice(cmd);

    let data_start = cmd_start + cmd.len();
    if !data.is_empty() {
        buf[data_start..data_start + data.len()].copy_from_slice(data);
    }

    buf
}

// --- Response parsing ---

fn parse_firmware_version(resp: &[u8]) -> Result<FirmwareVersion> {
    // Response layout: [xx, xx, 0x02, status, major, minor, patch_lo, patch_hi, ...]
    if resp.len() < 8 {
        bail!(
            "Firmware response too short: {} bytes (need 8)",
            resp.len()
        );
    }
    Ok(FirmwareVersion {
        major: resp[4],
        minor: resp[5],
        patch: u16::from_le_bytes([resp[6], resp[7]]),
    })
}

fn parse_device_entries(resp: &[u8]) -> Result<Vec<LinkDevice>> {
    // Response: [xx, xx, cmd_echo, status, data_type_hi, data_type_lo, count, payload...]
    if resp.len() < 7 {
        bail!(
            "Device enumeration response too short: {} bytes",
            resp.len()
        );
    }

    let count = resp[6] as usize;
    let mut devices = Vec::new();
    let mut pos = 7; // Start of per-device entries
    let mut channel: u8 = 0;

    for _ in 0..count {
        channel += 1;

        // Each entry: [reserved, reserved, type, model, reserved, reserved, reserved, id_len, id_bytes...]
        if pos + 8 > resp.len() {
            break; // Ran out of data
        }

        let device_type_byte = resp[pos + 2];
        let model = resp[pos + 3];
        let id_len = resp[pos + 7] as usize;
        pos += 8;

        if id_len == 0 {
            // Empty channel, skip
            continue;
        }

        if pos + id_len > resp.len() {
            break; // Ran out of data for ID string
        }

        let device_id = String::from_utf8_lossy(&resp[pos..pos + id_len]).to_string();
        pos += id_len;

        devices.push(LinkDevice {
            channel,
            device_type: LinkDeviceType::from_byte(device_type_byte),
            model,
            device_id,
        });
    }

    Ok(devices)
}

fn parse_speed_entries(resp: &[u8]) -> Result<Vec<FanSpeed>> {
    // Response: [xx, xx, cmd_echo, status, data_type_hi, data_type_lo, count, entries...]
    // Entry: [status, value_lo, value_hi] — 3 bytes each
    // Index 0 is a validity indicator, real channels start at index 1
    if resp.len() < 7 {
        bail!("Speed response too short: {} bytes", resp.len());
    }

    let count = resp[6] as usize;
    let mut speeds = Vec::new();
    let entry_start = 7;

    for i in 1..count {
        // Skip index 0 (validity indicator)
        let offset = entry_start + i * 3;
        if offset + 3 > resp.len() {
            break;
        }

        let status = resp[offset];
        if status != 0x00 {
            continue; // Invalid reading
        }

        let rpm = u16::from_le_bytes([resp[offset + 1], resp[offset + 2]]);
        speeds.push(FanSpeed {
            channel: i as u8,
            rpm,
        });
    }

    Ok(speeds)
}

fn parse_temperature_entries(resp: &[u8]) -> Result<Vec<TemperatureReading>> {
    // Same format as speeds but values are signed int16 / 10.0 = degrees Celsius
    if resp.len() < 7 {
        bail!("Temperature response too short: {} bytes", resp.len());
    }

    let count = resp[6] as usize;
    let mut temps = Vec::new();
    let entry_start = 7;

    for i in 1..count {
        // Skip index 0 (validity indicator)
        let offset = entry_start + i * 3;
        if offset + 3 > resp.len() {
            break;
        }

        let status = resp[offset];
        if status != 0x00 {
            continue;
        }

        let raw = i16::from_le_bytes([resp[offset + 1], resp[offset + 2]]);
        let celsius = raw as f64 / 10.0;

        // Filter out nonsensical readings
        if celsius > 0.0 && celsius < 150.0 {
            temps.push(TemperatureReading {
                channel: i as u8,
                temp_celsius: celsius,
            });
        }
    }

    Ok(temps)
}

fn build_speed_payload(targets: &[(u8, u8)]) -> Vec<u8> {
    // Payload format:
    // [payload_len_lo, payload_len_hi, 0x00, 0x00, data_type_hi, data_type_lo, count, entries...]
    // Entry: [channel_id, 0x00, percent, 0x00]
    let entry_size = 4;
    let entries_len = targets.len() * entry_size;
    let payload_content_len = 2 + 2 + 1 + entries_len; // reserved + data_type + count + entries
    let total_len = (payload_content_len + 2) as u16; // +2 for the length field itself

    let mut data = Vec::new();
    data.extend_from_slice(&total_len.to_le_bytes());
    data.push(0x00);
    data.push(0x00);
    data.extend_from_slice(&DATA_TYPE_SET_SPEED);
    data.push(targets.len() as u8);

    for &(channel, percent) in targets {
        data.push(channel);
        data.push(0x00);
        data.push(percent);
        data.push(0x00);
    }

    data
}

fn hex_string(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_packet_size() {
        let packet = build_packet(&CMD_FIRMWARE, &[]);
        assert_eq!(packet.len(), WRITE_SIZE); // 513 bytes
    }

    #[test]
    fn test_build_packet_header() {
        let packet = build_packet(&CMD_FIRMWARE, &[]);
        assert_eq!(packet[0], 0x00); // Report ID
        assert_eq!(packet[1], 0x00); // Normal
        assert_eq!(packet[2], 0x01); // Command flag
        // CMD_FIRMWARE starts at byte 3
        assert_eq!(packet[3], 0x02);
        assert_eq!(packet[4], 0x13);
    }

    #[test]
    fn test_build_packet_with_data() {
        let packet = build_packet(&CMD_OPEN, &[MODE_SPEEDS]);
        assert_eq!(packet[0], 0x00);
        assert_eq!(packet[1], 0x00);
        assert_eq!(packet[2], 0x01);
        // CMD_OPEN
        assert_eq!(packet[3], 0x0D);
        assert_eq!(packet[4], 0x01);
        // Data (mode)
        assert_eq!(packet[5], 0x17);
        // Rest should be zeros
        assert_eq!(packet[6], 0x00);
    }

    #[test]
    fn test_parse_firmware_version() {
        // Simulate response: [00, 00, 0x02, 0x00, major=2, minor=9, patch_lo=0xE8, patch_hi=0x01]
        let mut resp = vec![0u8; PACKET_SIZE];
        resp[2] = 0x02; // command echo
        resp[3] = 0x00; // STATUS_OK
        resp[4] = 0x02; // major
        resp[5] = 0x09; // minor
        resp[6] = 0xE8; // patch lo
        resp[7] = 0x01; // patch hi  -> 0x01E8 = 488

        let fw = parse_firmware_version(&resp).unwrap();
        assert_eq!(fw.major, 2);
        assert_eq!(fw.minor, 9);
        assert_eq!(fw.patch, 488);
        assert_eq!(fw.to_string(), "2.9.488");
    }

    #[test]
    fn test_parse_device_entries() {
        // Build a mock response with 2 devices
        let mut resp = vec![0u8; 128];
        resp[2] = 0x08; // cmd echo (read)
        resp[3] = 0x00; // STATUS_OK
        resp[4] = DATA_TYPE_DEVICES[0];
        resp[5] = DATA_TYPE_DEVICES[1];
        resp[6] = 2; // 2 channels

        // Device 1 at pos 7: QX Fan (type=0x01), model=0x05, id="FAN1"
        resp[7] = 0x00; // reserved
        resp[8] = 0x00; // reserved
        resp[9] = 0x01; // device type: QX Fan
        resp[10] = 0x05; // model
        resp[11] = 0x00; // reserved
        resp[12] = 0x00; // reserved
        resp[13] = 0x00; // reserved
        resp[14] = 4; // id_len = 4
        resp[15] = b'F';
        resp[16] = b'A';
        resp[17] = b'N';
        resp[18] = b'1';

        // Device 2 at pos 19: LX Fan (type=0x02), model=0x03, id="FAN2"
        resp[19] = 0x00;
        resp[20] = 0x00;
        resp[21] = 0x02; // device type: LX Fan
        resp[22] = 0x03; // model
        resp[23] = 0x00;
        resp[24] = 0x00;
        resp[25] = 0x00;
        resp[26] = 4; // id_len
        resp[27] = b'F';
        resp[28] = b'A';
        resp[29] = b'N';
        resp[30] = b'2';

        let devices = parse_device_entries(&resp).unwrap();
        assert_eq!(devices.len(), 2);

        assert_eq!(devices[0].channel, 1);
        assert_eq!(devices[0].device_type, LinkDeviceType::QxFan);
        assert_eq!(devices[0].model, 0x05);
        assert_eq!(devices[0].device_id, "FAN1");

        assert_eq!(devices[1].channel, 2);
        assert_eq!(devices[1].device_type, LinkDeviceType::LxFan);
        assert_eq!(devices[1].model, 0x03);
        assert_eq!(devices[1].device_id, "FAN2");
    }

    #[test]
    fn test_parse_speed_entries() {
        // Mock response: 3 entries (index 0 = validity, 1 and 2 are real channels)
        let mut resp = vec![0u8; 64];
        resp[2] = 0x08; // cmd echo
        resp[3] = 0x00; // STATUS_OK
        resp[4] = DATA_TYPE_SPEEDS[0];
        resp[5] = DATA_TYPE_SPEEDS[1];
        resp[6] = 3; // count: 3 entries (index 0-2)

        // Index 0 (validity indicator) at offset 7
        resp[7] = 0x00;
        resp[8] = 0x00;
        resp[9] = 0x00;

        // Index 1 (channel 1): 850 RPM = 0x0352
        resp[10] = 0x00; // status OK
        resp[11] = 0x52; // RPM lo
        resp[12] = 0x03; // RPM hi

        // Index 2 (channel 2): 920 RPM = 0x0398
        resp[13] = 0x00; // status OK
        resp[14] = 0x98; // RPM lo
        resp[15] = 0x03; // RPM hi

        let speeds = parse_speed_entries(&resp).unwrap();
        assert_eq!(speeds.len(), 2);
        assert_eq!(speeds[0].channel, 1);
        assert_eq!(speeds[0].rpm, 850);
        assert_eq!(speeds[1].channel, 2);
        assert_eq!(speeds[1].rpm, 920);
    }

    #[test]
    fn test_parse_temperature_entries() {
        // Mock response: 2 entries (index 0 = validity, 1 = real channel)
        let mut resp = vec![0u8; 64];
        resp[2] = 0x08;
        resp[3] = 0x00;
        resp[4] = DATA_TYPE_TEMPS[0];
        resp[5] = DATA_TYPE_TEMPS[1];
        resp[6] = 2; // count

        // Index 0 (validity)
        resp[7] = 0x00;
        resp[8] = 0x00;
        resp[9] = 0x00;

        // Index 1: 34.2°C = 342 = 0x0156
        resp[10] = 0x00; // status OK
        resp[11] = 0x56; // lo
        resp[12] = 0x01; // hi

        let temps = parse_temperature_entries(&resp).unwrap();
        assert_eq!(temps.len(), 1);
        assert_eq!(temps[0].channel, 1);
        assert!((temps[0].temp_celsius - 34.2).abs() < 0.01);
    }

    #[test]
    fn test_device_type_from_byte() {
        assert_eq!(LinkDeviceType::from_byte(0x01), LinkDeviceType::QxFan);
        assert_eq!(LinkDeviceType::from_byte(0x02), LinkDeviceType::LxFan);
        assert_eq!(
            LinkDeviceType::from_byte(0x03),
            LinkDeviceType::RxMaxRgbFan
        );
        assert_eq!(LinkDeviceType::from_byte(0x04), LinkDeviceType::RxMaxFan);
        assert_eq!(LinkDeviceType::from_byte(0x05), LinkDeviceType::LinkAdapter);
        assert_eq!(
            LinkDeviceType::from_byte(0x07),
            LinkDeviceType::LiquidCooler
        );
        assert_eq!(LinkDeviceType::from_byte(0x09), LinkDeviceType::WaterBlock);
        assert_eq!(LinkDeviceType::from_byte(0x0A), LinkDeviceType::GpuBlock);
        assert_eq!(LinkDeviceType::from_byte(0x0B), LinkDeviceType::Psu);
        assert_eq!(LinkDeviceType::from_byte(0x0C), LinkDeviceType::PumpXd5);
        assert_eq!(LinkDeviceType::from_byte(0x0D), LinkDeviceType::Xg7Block);
        assert_eq!(LinkDeviceType::from_byte(0x0F), LinkDeviceType::RxRgbFan);
        assert_eq!(LinkDeviceType::from_byte(0x10), LinkDeviceType::VrmCooler);
        assert_eq!(
            LinkDeviceType::from_byte(0x11),
            LinkDeviceType::TitanCooler
        );
        assert_eq!(LinkDeviceType::from_byte(0x13), LinkDeviceType::RxFan);
        assert_eq!(LinkDeviceType::from_byte(0x19), LinkDeviceType::PumpXd6);
        assert_eq!(
            LinkDeviceType::from_byte(0x1B),
            LinkDeviceType::CommanderDuo
        );
        assert_eq!(
            LinkDeviceType::from_byte(0xFF),
            LinkDeviceType::Unknown(0xFF)
        );
    }

    #[test]
    fn test_build_speed_payload() {
        let payload = build_speed_payload(&[(1, 50), (2, 75)]);

        // Check data type
        assert_eq!(payload[4], DATA_TYPE_SET_SPEED[0]); // 0x07
        assert_eq!(payload[5], DATA_TYPE_SET_SPEED[1]); // 0x00

        // Check channel count
        assert_eq!(payload[6], 2);

        // Check entry 1: channel=1, percent=50
        assert_eq!(payload[7], 1);
        assert_eq!(payload[8], 0x00);
        assert_eq!(payload[9], 50);
        assert_eq!(payload[10], 0x00);

        // Check entry 2: channel=2, percent=75
        assert_eq!(payload[11], 2);
        assert_eq!(payload[12], 0x00);
        assert_eq!(payload[13], 75);
        assert_eq!(payload[14], 0x00);
    }

    #[test]
    fn test_device_type_name_and_pump() {
        assert_eq!(LinkDeviceType::QxFan.name(), "QX Fan");
        assert!(!LinkDeviceType::QxFan.is_pump());
        assert!(LinkDeviceType::PumpXd5.is_pump());
        assert!(LinkDeviceType::PumpXd6.is_pump());
        assert_eq!(LinkDeviceType::PumpXd5.name(), "XD5 Pump");
    }
}
