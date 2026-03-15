use anyhow::{bail, Context, Result};
use hidapi::HidDevice;
use serde::Serialize;
use tracing::{debug, trace};

// Wire protocol constants (from liquidctl corsair_hid_psu driver)
const PSU_PACKET_SIZE: usize = 64;
const PSU_WRITE_SIZE: usize = 65; // report ID + 64

// Commands (PMBus-inspired)
const CMD_INIT: u8 = 0xFE;
const CMD_SELECT_RAIL: u8 = 0x00;
const CMD_READ_TEMP1: u8 = 0x8D; // VRM temperature
const CMD_READ_TEMP2: u8 = 0x8E; // Case/ambient temperature
const CMD_READ_FAN: u8 = 0x90; // Fan RPM
const CMD_READ_VIN: u8 = 0x88; // AC input voltage
const CMD_READ_VOUT: u8 = 0x8B; // Rail output voltage
const CMD_READ_IOUT: u8 = 0x8C; // Rail output current
const CMD_READ_POUT: u8 = 0x96; // Rail output power
const CMD_READ_TOTAL_POWER: u8 = 0xEE; // Total output power

// Rail identifiers
const RAIL_12V: u8 = 0x00;
const RAIL_5V: u8 = 0x01;
const RAIL_3V3: u8 = 0x02;

const READ_TIMEOUT_MS: i32 = 500;

/// A single rail's voltage, current, and power readings.
#[derive(Debug, Clone, Serialize)]
pub struct RailReading {
    pub voltage: f64,
    pub current: f64,
    pub power: f64,
}

/// Complete PSU status snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct PsuStatus {
    pub temp_vrm: f64,
    pub temp_case: f64,
    pub fan_rpm: u16,
    pub input_voltage: f64,
    pub rail_12v: RailReading,
    pub rail_5v: RailReading,
    pub rail_3v3: RailReading,
    pub total_power: f64,
}

/// Driver for Corsair HX/RM-series PSUs over USB HID.
pub struct CorsairPsu {
    device: HidDevice,
    serial: String,
}

impl CorsairPsu {
    pub fn new(device: HidDevice, serial: String) -> Self {
        Self { device, serial }
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// PSU uses interface 0 (only one interface).
    pub fn data_interface() -> i32 {
        0
    }

    /// Send init command (0xFE, param 0x03).
    pub fn initialize(&self) -> Result<()> {
        debug!(serial = self.serial.as_str(), "Initializing PSU");
        self.send_command(CMD_INIT, &[0x03])?;
        Ok(())
    }

    /// Read VRM temperature in °C.
    pub fn read_temp_vrm(&self) -> Result<f64> {
        let resp = self.send_command(CMD_READ_TEMP1, &[])?;
        Ok(linear11_to_f64(parse_linear11_response(&resp)?))
    }

    /// Read case/ambient temperature in °C.
    pub fn read_temp_case(&self) -> Result<f64> {
        let resp = self.send_command(CMD_READ_TEMP2, &[])?;
        Ok(linear11_to_f64(parse_linear11_response(&resp)?))
    }

    /// Read PSU fan RPM.
    pub fn read_fan_rpm(&self) -> Result<u16> {
        let resp = self.send_command(CMD_READ_FAN, &[])?;
        let rpm = linear11_to_f64(parse_linear11_response(&resp)?);
        Ok(rpm.round() as u16)
    }

    /// Read AC input voltage.
    pub fn read_input_voltage(&self) -> Result<f64> {
        let resp = self.send_command(CMD_READ_VIN, &[])?;
        Ok(linear11_to_f64(parse_linear11_response(&resp)?))
    }

    /// Read a single rail's voltage, current, and power.
    pub fn read_rail(&self, rail: u8) -> Result<RailReading> {
        self.send_command(CMD_SELECT_RAIL, &[rail])?;

        let v_resp = self.send_command(CMD_READ_VOUT, &[])?;
        let voltage = linear11_to_f64(parse_linear11_response(&v_resp)?);

        let i_resp = self.send_command(CMD_READ_IOUT, &[])?;
        let current = linear11_to_f64(parse_linear11_response(&i_resp)?);

        let p_resp = self.send_command(CMD_READ_POUT, &[])?;
        let power = linear11_to_f64(parse_linear11_response(&p_resp)?);

        Ok(RailReading {
            voltage,
            current,
            power,
        })
    }

    /// Read total output power in watts.
    pub fn read_total_power(&self) -> Result<f64> {
        let resp = self.send_command(CMD_READ_TOTAL_POWER, &[])?;
        Ok(linear11_to_f64(parse_linear11_response(&resp)?))
    }

    /// Read all PSU sensors in one call.
    pub fn read_all(&self) -> Result<PsuStatus> {
        let temp_vrm = self.read_temp_vrm()?;
        let temp_case = self.read_temp_case()?;
        let fan_rpm = self.read_fan_rpm()?;
        let input_voltage = self.read_input_voltage()?;
        let rail_12v = self.read_rail(RAIL_12V)?;
        let rail_5v = self.read_rail(RAIL_5V)?;
        let rail_3v3 = self.read_rail(RAIL_3V3)?;
        let total_power = self.read_total_power()?;

        Ok(PsuStatus {
            temp_vrm,
            temp_case,
            fan_rpm,
            input_voltage,
            rail_12v,
            rail_5v,
            rail_3v3,
            total_power,
        })
    }

    // --- Low-level HID transfer ---

    fn send_command(&self, cmd: u8, params: &[u8]) -> Result<Vec<u8>> {
        let packet = build_psu_packet(cmd, params);

        trace!(
            serial = self.serial.as_str(),
            data = hex_string(&packet[..packet.len().min(16)]),
            "PSU TX"
        );

        self.device
            .write(&packet)
            .context("Failed to write to PSU")?;

        let mut buf = vec![0u8; PSU_PACKET_SIZE];
        let bytes_read = self
            .device
            .read_timeout(&mut buf, READ_TIMEOUT_MS)
            .context("Failed to read from PSU")?;

        if bytes_read == 0 {
            bail!("No response from PSU (timeout)");
        }

        buf.truncate(bytes_read);

        trace!(
            serial = self.serial.as_str(),
            data = hex_string(&buf[..buf.len().min(16)]),
            "PSU RX"
        );

        Ok(buf)
    }
}

// --- Packet building ---

fn build_psu_packet(cmd: u8, params: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; PSU_WRITE_SIZE]; // 65 bytes
    buf[0] = 0x00; // HID report ID
    buf[1] = (1 + params.len()) as u8; // length: cmd + params
    buf[2] = cmd;
    buf[3..3 + params.len()].copy_from_slice(params);
    buf
}

/// Parse a LINEAR11 value from a PSU response.
/// Response format: [length, cmd_echo, data_lo, data_hi, ...]
fn parse_linear11_response(resp: &[u8]) -> Result<u16> {
    if resp.len() < 4 {
        bail!("PSU response too short: {} bytes (need 4)", resp.len());
    }
    Ok(u16::from_le_bytes([resp[2], resp[3]]))
}

/// Decode PMBus LINEAR11 format to f64.
///
/// LINEAR11 is a 16-bit format: bits[15:11] = signed exponent, bits[10:0] = signed mantissa.
/// Value = mantissa × 2^exponent
fn linear11_to_f64(raw: u16) -> f64 {
    // Extract 5-bit signed exponent (bits 15:11)
    let exp = ((raw >> 11) as i16) | if raw & 0x8000 != 0 { !0x1F_i16 } else { 0 };
    // Extract 11-bit signed mantissa (bits 10:0)
    let man = (raw & 0x7FF) as i16 | if raw & 0x0400 != 0 { !0x7FF_i16 } else { 0 };
    man as f64 * 2.0_f64.powi(exp as i32)
}

use crate::hex_string;

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_psu_packet_size() {
        let packet = build_psu_packet(CMD_READ_TEMP1, &[]);
        assert_eq!(packet.len(), PSU_WRITE_SIZE); // 65 bytes
    }

    #[test]
    fn test_build_psu_packet_init() {
        let packet = build_psu_packet(CMD_INIT, &[0x03]);
        assert_eq!(packet[0], 0x00); // report ID
        assert_eq!(packet[1], 0x02); // length: cmd(1) + params(1)
        assert_eq!(packet[2], 0xFE); // CMD_INIT
        assert_eq!(packet[3], 0x03); // param
        // Rest should be zero
        assert_eq!(packet[4], 0x00);
    }

    #[test]
    fn test_build_psu_packet_read_cmd() {
        let packet = build_psu_packet(CMD_READ_TEMP1, &[]);
        assert_eq!(packet[1], 0x01); // length: cmd only
        assert_eq!(packet[2], 0x8D); // CMD_READ_TEMP1
    }

    #[test]
    fn test_linear11_positive() {
        // 12.125V: mantissa=97 (0x061), exponent=-3 (0x1D = 29, signed = -3)
        // 97 * 2^(-3) = 97/8 = 12.125
        let raw: u16 = (0x1D << 11) | 0x061; // exp=-3, man=97
        let val = linear11_to_f64(raw);
        assert!((val - 12.125).abs() < 0.001, "got {}", val);
    }

    #[test]
    fn test_linear11_negative_exponent() {
        // Temperature example: 45.0°C
        // mantissa=360 (0x168), exponent=-3 (0x1D)
        // 360 * 2^(-3) = 360/8 = 45.0
        let raw: u16 = (0x1D << 11) | 0x168;
        let val = linear11_to_f64(raw);
        assert!((val - 45.0).abs() < 0.001, "got {}", val);
    }

    #[test]
    fn test_linear11_zero() {
        assert_eq!(linear11_to_f64(0x0000), 0.0);
    }

    #[test]
    fn test_linear11_large_positive_exponent() {
        // mantissa=100, exponent=2 → 100 * 4 = 400
        let raw: u16 = (0x02 << 11) | 100;
        let val = linear11_to_f64(raw);
        assert!((val - 400.0).abs() < 0.001, "got {}", val);
    }

    #[test]
    fn test_parse_linear11_response() {
        let resp = vec![0x02, 0x8D, 0x68, 0xE9]; // length, cmd_echo, data_lo, data_hi
        let raw = parse_linear11_response(&resp).unwrap();
        assert_eq!(raw, u16::from_le_bytes([0x68, 0xE9]));
    }

    #[test]
    fn test_parse_linear11_response_too_short() {
        let resp = vec![0x02, 0x8D];
        assert!(parse_linear11_response(&resp).is_err());
    }
}
