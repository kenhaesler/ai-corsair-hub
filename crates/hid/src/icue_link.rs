use anyhow::{Context, Result, bail};
use hidapi::HidDevice;
use serde::Serialize;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, trace, warn};

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
const CMD_OPEN: [u8; 2] = [0x0D, 0x01]; // Open data endpoint
const CMD_CLOSE: [u8; 3] = [0x05, 0x01, 0x01]; // Close endpoint selected by mode byte
const CMD_READ: [u8; 2] = [0x08, 0x01]; // Read from data endpoint
const CMD_WRITE: [u8; 2] = [0x06, 0x01]; // Write to data endpoint

// Color endpoint commands (separate from data endpoint)
const CMD_OPEN_COLOR: [u8; 2] = [0x0D, 0x00]; // Open color endpoint
const CMD_CLOSE_COLOR: [u8; 2] = [0x05, 0x01]; // Close color endpoint
const CMD_WRITE_COLOR: [u8; 2] = [0x06, 0x00]; // First chunk of color data
const CMD_WRITE_COLOR_CONT: [u8; 2] = [0x07, 0x00]; // Continuation chunks
#[allow(dead_code)]
const CMD_RESET_LED_POWER: [u8; 2] = [0x15, 0x01]; // Reset LED power state

// Endpoint modes
const MODE_SPEEDS: u8 = 0x17;
const MODE_SET_SPEED: u8 = 0x18;
const MODE_TEMPS: u8 = 0x21;
const MODE_SET_COLOR: u8 = 0x22;
const MODE_DEVICES: u8 = 0x36;

// Color payload constants
const DATA_TYPE_SET_COLOR: [u8; 2] = [0x12, 0x00];
const MAX_COLOR_PAYLOAD: usize = 508; // max bytes per USB transfer

// Data types for response matching and write payload headers
const DATA_TYPE_DEVICES: [u8; 2] = [0x21, 0x00];
const DATA_TYPE_SPEEDS: [u8; 2] = [0x25, 0x00];
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
    LsStrip,      // TBD — LS350 Aurora strip (type byte from enumeration)
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
            Self::LsStrip => "LS Strip",
            Self::Unknown(b) => {
                // Can't return dynamic string from &'static str, use generic label
                let _ = b;
                "Unknown Device"
            }
        }
    }

    pub fn is_pump(&self) -> bool {
        matches!(
            self,
            Self::PumpXd5 | Self::PumpXd6 | Self::LiquidCooler | Self::TitanCooler
        )
    }

    pub fn is_speed_controllable(&self) -> bool {
        matches!(
            self,
            Self::QxFan
                | Self::LxFan
                | Self::RxMaxRgbFan
                | Self::RxMaxFan
                | Self::RxRgbFan
                | Self::RxFan
                | Self::PumpXd5
                | Self::PumpXd6
                | Self::LiquidCooler
                | Self::TitanCooler
        )
    }

    /// Whether this device has addressable RGB LEDs.
    pub fn has_rgb(&self) -> bool {
        matches!(
            self,
            Self::QxFan
                | Self::LxFan
                | Self::RxMaxRgbFan
                | Self::RxRgbFan
                | Self::LsStrip
                | Self::LinkAdapter
                | Self::Xg7Block
                | Self::LiquidCooler
                | Self::TitanCooler
        )
    }

    /// Number of addressable LEDs on this device.
    /// Values confirmed from OpenLinkHub (github.com/jurkovic-nikola/OpenLinkHub).
    pub fn led_count(&self) -> u16 {
        match self {
            Self::QxFan => 34,
            Self::LxFan => 18,
            Self::RxMaxRgbFan => 8,
            Self::RxRgbFan => 8,
            Self::LiquidCooler => 20,
            Self::WaterBlock => 24,
            Self::GpuBlock => 22,
            Self::PumpXd5 => 22,
            Self::Xg7Block => 16,
            Self::TitanCooler => 20,
            Self::PumpXd6 => 22,
            Self::LinkAdapter => 21, // LS350 default; overridden by 0x1d table at runtime
            Self::LsStrip => 21,     // dynamic via LINK Adapter, 21 default for LS350
            _ => 0,
        }
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
    /// Per-channel LED counts read from the hub firmware.
    /// Authoritative — accounts for LINK Adapters, strip configurations, etc.
    #[serde(skip)]
    pub led_counts: std::collections::HashMap<u8, u16>,
}

// --- Transport abstraction ---

/// Control-loop-facing transport trait for a single iCUE LINK Hub.
///
/// The concrete implementation is `IcueLinkHub` (HID-backed). The trait exists
/// so the fan-control loop can swap in mock implementations for integration
/// tests — notably to exercise hub-failure and shutdown-timeout paths that
/// would otherwise require physical hardware.
///
/// All methods take `&self` because `IcueLinkHub` serializes access through an
/// internal mutex. The `Send + Sync + 'static` bound enables moving a trait
/// object into a `std::thread::spawn` closure for bounded-timeout shutdown
/// (see `ControlLoop::shutdown_hardware`).
pub trait IcueLinkTransport: Send + Sync + 'static {
    fn set_speeds(&self, targets: &[(u8, u8)]) -> Result<()>;
    fn get_speeds(&self) -> Result<Vec<FanSpeed>>;
    fn get_temperatures(&self) -> Result<Vec<TemperatureReading>>;
    fn set_rgb(&self, channel_leds: &[(u8, &[[u8; 3]])]) -> Result<()>;
    fn enter_hardware_mode(&self) -> Result<()>;
}

impl IcueLinkTransport for IcueLinkHub {
    fn set_speeds(&self, targets: &[(u8, u8)]) -> Result<()> {
        IcueLinkHub::set_speeds(self, targets)
    }
    fn get_speeds(&self) -> Result<Vec<FanSpeed>> {
        IcueLinkHub::get_speeds(self)
    }
    fn get_temperatures(&self) -> Result<Vec<TemperatureReading>> {
        IcueLinkHub::get_temperatures(self)
    }
    fn set_rgb(&self, channel_leds: &[(u8, &[[u8; 3]])]) -> Result<()> {
        IcueLinkHub::set_rgb(self, channel_leds)
    }
    fn enter_hardware_mode(&self) -> Result<()> {
        IcueLinkHub::enter_hardware_mode(self)
    }
}

// --- Hub implementation ---

/// Inner state for a single iCUE LINK Hub connection, shared across `IcueLinkHub`
/// clones via `Arc`. The `HidDevice` handle and color-endpoint flag live behind a
/// single mutex so HID transfers are serialized (required by the protocol —
/// concurrent TX on the same handle would interleave packets).
///
/// Keeping both in one mutex also ensures that `enter_hardware_mode` sees a
/// consistent view of `color_endpoint_open` and avoids a race where a second
/// thread flips the flag between our check and our close.
struct IcueLinkHubInner {
    device: HidDevice,
    color_endpoint_open: bool,
    software_mode_active: bool,
}

impl Drop for IcueLinkHubInner {
    fn drop(&mut self) {
        if !self.software_mode_active {
            return;
        }

        // Final safety net for partial initialization and unexpected owner
        // teardown. Normal shutdown clears the flag and avoids this path.
        if self.color_endpoint_open {
            let _ = best_effort_exchange(&self.device, &CMD_CLOSE_COLOR, &[]);
            self.color_endpoint_open = false;
        }
        if let Err(error) = best_effort_exchange(&self.device, &CMD_SLEEP, &[]) {
            warn!(%error, "Failed to restore hardware mode while dropping hub");
        }
        self.software_mode_active = false;
    }
}

// SAFETY: `hidapi::HidDevice` is `Send` on all supported platforms (Windows,
// Linux, macOS) — see hidapi-2.6's platform modules. It is not `Sync`, but we
// never share it across threads concurrently; the `Mutex` enforces single-
// threaded access. The `Mutex` is itself `Send + Sync`, so the enclosing
// `Arc<Mutex<IcueLinkHubInner>>` is `Send + Sync` too.

/// Handle to a Corsair iCUE LINK System Hub.
///
/// Cheap to clone (`Arc`-internal): clones share the same underlying HID device
/// handle and serialize protocol access through an internal mutex. This makes
/// it safe to move a handle into a short-lived worker thread (e.g. for bounded-
/// timeout shutdown) without refactoring the caller's ownership model.
pub struct IcueLinkHub {
    inner: Arc<Mutex<IcueLinkHubInner>>,
    serial: Arc<str>,
}

impl Clone for IcueLinkHub {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            serial: Arc::clone(&self.serial),
        }
    }
}

impl IcueLinkHub {
    pub fn new(device: HidDevice, serial: String) -> Self {
        Self {
            inner: Arc::new(Mutex::new(IcueLinkHubInner {
                device,
                color_endpoint_open: false,
                software_mode_active: false,
            })),
            serial: Arc::from(serial),
        }
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn data_interface() -> i32 {
        DATA_INTERFACE
    }

    /// Lock the shared inner state. Poisoned locks are recovered — we don't
    /// hold data invariants across panics, only a HID handle, so continuing
    /// after a prior thread's panic is safe.
    fn lock_inner(&self) -> MutexGuard<'_, IcueLinkHubInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    // --- Low-level transfer ---

    /// Send a command with optional data, read 512-byte response, check status.
    ///
    /// Serializes through the inner mutex so the TX/RX pair is atomic against
    /// other callers on the same hub.
    fn transfer(&self, cmd: &[u8], data: &[u8]) -> Result<Vec<u8>> {
        let mut inner = self.lock_inner();
        self.transfer_locked(&mut inner, cmd, data, false)
    }

    /// Perform a request/response exchange while the caller owns the hub
    /// mutex. Endpoint operations use this to keep their complete session
    /// atomic against other control and RGB traffic.
    fn transfer_locked(
        &self,
        inner: &mut IcueLinkHubInner,
        cmd: &[u8],
        data: &[u8],
        allow_non_ok_status: bool,
    ) -> Result<Vec<u8>> {
        let packet = build_packet(cmd, data);

        trace!(
            serial = self.serial.as_ref(),
            data = hex_string(&packet[..packet.len().min(32)]),
            len = packet.len(),
            "TX"
        );

        inner
            .device
            .write(&packet)
            .context("Failed to write to iCUE LINK Hub")?;

        let mut buf = vec![0u8; PACKET_SIZE];
        let bytes_read = inner
            .device
            .read_timeout(&mut buf, READ_TIMEOUT_MS)
            .context("Failed to read from iCUE LINK Hub")?;

        if bytes_read == 0 {
            bail!("No response from hub (timeout)");
        }
        if bytes_read != PACKET_SIZE {
            bail!(
                "Incomplete hub response: received {} bytes, expected {}",
                bytes_read,
                PACKET_SIZE
            );
        }

        buf.truncate(bytes_read);

        trace!(
            serial = self.serial.as_ref(),
            data = hex_string(&buf[..buf.len().min(32)]),
            len = bytes_read,
            "RX"
        );

        validate_response(&buf, cmd, allow_non_ok_status)?;

        Ok(buf)
    }

    /// Endpoint read: close → open(mode) → read → close
    fn read_endpoint(&self, mode: u8) -> Result<Vec<u8>> {
        let mut inner = self.lock_inner();
        self.transfer_locked(&mut inner, &CMD_CLOSE, &[mode], false)?;
        self.transfer_locked(&mut inner, &CMD_OPEN, &[mode], false)?;

        let operation = self.transfer_locked(&mut inner, &CMD_READ, &[], false);
        let close = self
            .transfer_locked(&mut inner, &CMD_CLOSE, &[mode], false)
            .map(|_| ());
        combine_operation_and_close(operation, close)
    }

    /// Endpoint write: close → open(mode) → write(data) → close
    fn write_endpoint(&self, mode: u8, data: &[u8]) -> Result<()> {
        let mut inner = self.lock_inner();
        self.transfer_locked(&mut inner, &CMD_CLOSE, &[mode], false)?;
        self.transfer_locked(&mut inner, &CMD_OPEN, &[mode], false)?;

        let operation = self
            .transfer_locked(&mut inner, &CMD_WRITE, data, false)
            .map(|_| ());
        let close = self
            .transfer_locked(&mut inner, &CMD_CLOSE, &[mode], false)
            .map(|_| ());
        combine_operation_and_close(operation, close)
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
        debug!(serial = self.serial.as_ref(), "Entering software mode");
        let resp = self.transfer(&CMD_WAKE, &[])?;
        debug_assert_eq!(resp[3], STATUS_OK);
        self.lock_inner().software_mode_active = true;

        thread::sleep(Duration::from_millis(SOFTWARE_MODE_DELAY_MS));
        Ok(())
    }

    /// Return hub to hardware (firmware) control mode.
    /// Closes the color endpoint if open, then sends the sleep command.
    pub fn enter_hardware_mode(&self) -> Result<()> {
        debug!(serial = self.serial.as_ref(), "Entering hardware mode");
        let mut inner = self.lock_inner();
        if inner.color_endpoint_open {
            if let Err(e) = self.transfer_locked(&mut inner, &CMD_CLOSE_COLOR, &[], false) {
                warn!(serial = self.serial.as_ref(), error = %e, "Failed to close color EP before hardware mode");
            }
            inner.color_endpoint_open = false;
        }
        self.transfer_locked(&mut inner, &CMD_SLEEP, &[], false)?;
        inner.software_mode_active = false;
        Ok(())
    }

    // --- Color endpoint API ---

    /// Open the color endpoint. Must be called before `write_color()`.
    /// Closes any stale endpoint first, then opens with mode 0x22.
    pub fn open_color_endpoint(&self) -> Result<()> {
        debug!(serial = self.serial.as_ref(), "Opening color endpoint");
        let mut inner = self.lock_inner();
        self.open_color_endpoint_locked(&mut inner)
    }

    fn open_color_endpoint_locked(&self, inner: &mut IcueLinkHubInner) -> Result<()> {
        if inner.color_endpoint_open {
            return Ok(());
        }

        self.transfer_locked(inner, &CMD_CLOSE, &[MODE_SET_COLOR], false)?;
        inner.color_endpoint_open = false;
        let response = self.transfer_locked(inner, &CMD_OPEN_COLOR, &[MODE_SET_COLOR], true)?;
        validate_color_endpoint_open_response(&response)?;
        if response[3] == STATUS_WRONG_MODE {
            debug!(
                serial = self.serial.as_ref(),
                "Hub returned firmware-compatible status 0x03 for color endpoint open"
            );
        }
        inner.color_endpoint_open = true;
        Ok(())
    }

    /// Close the color endpoint.
    pub fn close_color_endpoint(&self) -> Result<()> {
        debug!(serial = self.serial.as_ref(), "Closing color endpoint");
        let mut inner = self.lock_inner();
        self.transfer_locked(&mut inner, &CMD_CLOSE_COLOR, &[], false)?;
        inner.color_endpoint_open = false;
        Ok(())
    }

    /// Send raw RGB data to the hub's color endpoint.
    /// Handles payload framing (length prefix + dataTypeSetColor header)
    /// and chunking at 508-byte boundaries.
    pub fn write_color(&self, rgb_data: &[u8]) -> Result<()> {
        let payload = build_rgb_payload(rgb_data);
        let mut inner = self.lock_inner();
        self.write_color_locked(&mut inner, &payload)
    }

    fn write_color_locked(&self, inner: &mut IcueLinkHubInner, payload: &[u8]) -> Result<()> {
        // Send in chunks of MAX_COLOR_PAYLOAD bytes
        for (i, chunk) in payload.chunks(MAX_COLOR_PAYLOAD).enumerate() {
            let cmd = if i == 0 {
                &CMD_WRITE_COLOR
            } else {
                &CMD_WRITE_COLOR_CONT
            };
            self.transfer_locked(inner, cmd, chunk, false)?;
        }

        Ok(())
    }

    /// High-level RGB API: takes channel→LED color mappings sorted by channel,
    /// flattens into one contiguous RGB buffer, and sends to hardware.
    /// Each LED is `[R, G, B]`. Auto-opens the color endpoint if not already open.
    pub fn set_rgb(&self, channel_leds: &[(u8, &[[u8; 3]])]) -> Result<()> {
        // Flatten all LEDs into one contiguous R,G,B byte buffer, sorted by channel
        let mut sorted: Vec<_> = channel_leds.to_vec();
        sorted.sort_by_key(|(ch, _)| *ch);

        let total_leds: usize = sorted.iter().map(|(_, leds)| leds.len()).sum();
        let mut rgb_bytes = Vec::with_capacity(total_leds * 3);
        for (_, leds) in &sorted {
            for led in *leds {
                rgb_bytes.extend_from_slice(led);
            }
        }

        let payload = build_rgb_payload(&rgb_bytes);
        let mut inner = self.lock_inner();
        self.open_color_endpoint_locked(&mut inner)?;
        let result = self.write_color_locked(&mut inner, &payload);
        if result.is_err() {
            inner.color_endpoint_open = false;
        }
        result
    }

    /// Whether the color endpoint is currently open.
    pub fn color_endpoint_open(&self) -> bool {
        self.lock_inner().color_endpoint_open
    }

    /// Read the hub's per-channel LED count table.
    /// Uses the color endpoint with mode 0x1d to query the hub firmware's
    /// knowledge of how many LEDs each channel has. This is authoritative —
    /// it accounts for LINK Adapters with connected strips, device types
    /// the hub auto-detected, etc.
    ///
    /// Returns a map of channel → LED count.
    pub fn read_led_counts(&self) -> Result<std::collections::HashMap<u8, u16>> {
        // 0x1d can return status 0x03 ("wrong mode") for a short window after
        // software-mode entry on some firmware revisions. Retry with escalating
        // backoff before accepting the fallback-to-defaults path — getting the
        // real LED count from the hub is the difference between a correct
        // buffer and a chain-desync that makes RGB flicker.
        let backoffs = [
            Duration::from_millis(100),
            Duration::from_millis(300),
            Duration::from_millis(800),
        ];

        let mut resp = Vec::new();
        let mut last_status = 0u8;

        for (attempt, backoff) in std::iter::once(Duration::ZERO)
            .chain(backoffs.iter().copied())
            .enumerate()
        {
            if !backoff.is_zero() {
                thread::sleep(backoff);
            }

            // Treat the query as one endpoint session so other traffic cannot
            // change the active mode between open, read, and close.
            let mut inner = self.lock_inner();
            self.transfer_locked(&mut inner, &CMD_CLOSE, &[0x1d], false)?;
            inner.color_endpoint_open = false;
            self.transfer_locked(&mut inner, &CMD_OPEN_COLOR, &[0x1d], false)?;
            let operation = self.transfer_locked(&mut inner, &[0x08, 0x00], &[], true);
            let close = self
                .transfer_locked(&mut inner, &CMD_CLOSE_COLOR, &[], false)
                .map(|_| ());
            inner.color_endpoint_open = false;
            resp = combine_operation_and_close(operation, close)?;

            if resp.len() < 8 {
                bail!("LED count response too short: {} bytes", resp.len());
            }

            last_status = resp[3];
            if last_status == STATUS_OK {
                if attempt > 0 {
                    info!(
                        serial = self.serial.as_ref(),
                        attempt = attempt + 1,
                        "0x1d LED count query succeeded on retry"
                    );
                }
                break;
            }

            warn!(
                serial = self.serial.as_ref(),
                attempt = attempt + 1,
                status = format!("0x{:02X}", last_status),
                "0x1d LED count query returned non-OK status"
            );
        }

        // Log raw response for protocol debugging
        info!(
            serial = self.serial.as_ref(),
            hex = hex_string(&resp[..resp.len().min(64)]),
            len = resp.len(),
            "LED count raw response (0x1d)"
        );

        if last_status != STATUS_OK {
            warn!(
                serial = self.serial.as_ref(),
                status = format!("0x{:02X}", last_status),
                "0x1d LED count query still non-OK after retries — falling back to device type defaults. Consider setting [[device_overrides]] in config.toml if a device has an unexpected LED count."
            );
            return Ok(std::collections::HashMap::new());
        }

        let channel_count = resp[6] as usize;
        let mut counts = std::collections::HashMap::new();

        // Sanity check: no hub has more than ~20 physical channels
        if channel_count > 20 {
            warn!(
                serial = self.serial.as_ref(),
                channel_count,
                "0x1d channel count unreasonable — falling back to device type defaults"
            );
            return Ok(counts);
        }

        // Per-channel data starts at byte 8, each channel is 2 bytes:
        // [device_type_code, led_count]
        for ch in 1..=channel_count {
            let idx = 6 + ch * 2; // index of device_type byte for this channel
            if idx + 1 >= resp.len() {
                break;
            }
            let dev_type = resp[idx];
            let led_count = resp[idx + 1] as u16;
            // Sanity: max 255 LEDs per channel (single-byte encoding limit).
            // LINK Adapters with multiple strips can report 80+ LEDs.
            if led_count > 0 && led_count <= 255 {
                debug!(
                    serial = self.serial.as_ref(),
                    channel = ch,
                    dev_type = format!("0x{:02X}", dev_type),
                    led_count,
                    "  LED table entry"
                );
                counts.insert(ch as u8, led_count);
            }
        }

        info!(
            serial = self.serial.as_ref(),
            channels = ?counts,
            "Read LED counts from hub"
        );

        Ok(counts)
    }

    /// Enumerate connected iCUE LINK devices on the daisy chain.
    pub fn enumerate_devices(&self) -> Result<Vec<LinkDevice>> {
        // Retry up to 3 times — hubs sometimes return stale data from a previous mode
        let mut resp = Vec::new();
        for attempt in 0..3 {
            resp = self.read_endpoint(MODE_DEVICES)?;

            debug!(
                serial = self.serial.as_ref(),
                hex = hex_string(&resp[..resp.len().min(128)]),
                len = resp.len(),
                attempt = attempt + 1,
                "Device enumeration raw response"
            );

            // Validate data type header: bytes [4:6] must be DATA_TYPE_DEVICES (0x21, 0x00)
            if resp.len() > 5 && resp[4] == DATA_TYPE_DEVICES[0] && resp[5] == DATA_TYPE_DEVICES[1]
            {
                break; // Got valid device data
            }

            warn!(
                serial = self.serial.as_ref(),
                got_hi = resp.get(4).copied().unwrap_or(0),
                got_lo = resp.get(5).copied().unwrap_or(0),
                attempt = attempt + 1,
                "Device enumeration got wrong data type, retrying"
            );
            thread::sleep(Duration::from_millis(50));
        }

        // Final validation
        if resp.len() > 5 && (resp[4] != DATA_TYPE_DEVICES[0] || resp[5] != DATA_TYPE_DEVICES[1]) {
            bail!(
                "Device enumeration returned wrong data type after 3 attempts: {:02X} {:02X} (expected {:02X} {:02X})",
                resp[4],
                resp[5],
                DATA_TYPE_DEVICES[0],
                DATA_TYPE_DEVICES[1]
            );
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
        if targets.is_empty() {
            bail!("Speed command requires at least one target");
        }
        let mut channels = std::collections::HashSet::new();
        for (channel, percent) in targets {
            if *channel == 0 {
                bail!("Speed command channel must be non-zero");
            }
            if *percent > 100 {
                bail!("Speed command duty must be 0..100, got {}", percent);
            }
            if !channels.insert(*channel) {
                bail!("Speed command contains duplicate channel {}", channel);
            }
        }
        let data = build_speed_payload(targets);

        // Retry up to 3 times on failure
        let mut last_err = None;
        for attempt in 0..3 {
            match self.write_endpoint(MODE_SET_SPEED, &data) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(
                        serial = self.serial.as_ref(),
                        attempt = attempt + 1,
                        error = %e,
                        "set_speeds failed, retrying"
                    );
                    last_err = Some(e);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        Err(last_err.expect("retry loop must execute at least once"))
    }

    /// Full initialization: get firmware, enter software mode, enumerate devices,
    /// and read the LED count table.
    pub fn initialize(&self) -> Result<HubInfo> {
        let firmware = self.get_firmware_version()?;
        debug!(
            serial = self.serial.as_ref(),
            firmware = %firmware,
            "Hub firmware"
        );

        self.enter_software_mode()?;

        match self.initialize_software_mode(firmware) {
            Ok(info) => Ok(info),
            Err(init_error) => {
                if let Err(restore_error) = self.enter_hardware_mode() {
                    warn!(
                        serial = self.serial.as_ref(),
                        error = %restore_error,
                        "Hub initialization failed and hardware-mode restoration also failed"
                    );
                }
                Err(init_error)
            }
        }
    }

    fn initialize_software_mode(&self, firmware: FirmwareVersion) -> Result<HubInfo> {
        // Read LED count table before device enumeration (matches OpenLinkHub init order)
        let raw_counts = match self.read_led_counts() {
            Ok(counts) => counts,
            Err(e) => {
                warn!(
                    serial = self.serial.as_ref(),
                    error = %e,
                    "Failed to read LED counts — will fall back to device type defaults"
                );
                std::collections::HashMap::new()
            }
        };

        let devices = self.enumerate_devices()?;
        debug!(
            serial = self.serial.as_ref(),
            count = devices.len(),
            "Enumerated devices"
        );

        // Validate 0x1d LED counts against enumerated devices.
        //
        // The firmware's 0x1d table uses `0` on a channel to mean "use the
        // device-type default" — NOT "this channel is invalid". Confirmed
        // empirically by walking a single lit LED across ch 15 on a hub whose
        // 0x1d returned {ch 15: 80, all other channels: 0}: the 80-LED
        // claim was real — both chained LS350 strips plus the LINK Adapter
        // run all 80 addressable positions. Previous heuristic ("reject the
        // whole table unless every enumerated channel reports non-zero")
        // discarded that correct value and left the second strip dark.
        //
        // New policy:
        //  - Keep only non-zero entries that match an enumerated channel.
        //  - Zero entries are dropped (callers fall through to the
        //    device-type default at LED-buffer sizing time).
        //  - Entries for channels that aren't enumerated (firmware stale
        //    data) are dropped defensively.
        //  - Bogus entries (count > 255) are already filtered out in
        //    `read_led_counts`.
        //
        // Users with truly exotic setups (e.g. a hub variant whose firmware
        // reports `0` for a channel that actually has a non-default count)
        // can still pin a value via `[[device_overrides]]` in config.toml.
        let expected_channels: std::collections::HashSet<u8> =
            devices.iter().map(|d| d.channel).collect();
        let led_counts: std::collections::HashMap<u8, u16> = raw_counts
            .into_iter()
            .filter(|(ch, count)| *count > 0 && expected_channels.contains(ch))
            .collect();

        if !led_counts.is_empty() {
            info!(
                serial = self.serial.as_ref(),
                channels = ?led_counts,
                "Accepted 0x1d LED-count overrides (other channels use type defaults)"
            );
        } else {
            debug!(
                serial = self.serial.as_ref(),
                "0x1d returned no non-zero entries — all channels use device-type defaults"
            );
        };

        let info = HubInfo {
            firmware,
            devices,
            led_counts,
        };

        // The color endpoint is independent of the transactional data endpoint.
        // Keep it open while telemetry and speed sessions come and go.
        let mut inner = self.lock_inner();
        self.open_color_endpoint_locked(&mut inner)?;
        Ok(info)
    }
}

fn combine_operation_and_close<T>(operation: Result<T>, close: Result<()>) -> Result<T> {
    match (operation, close) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(operation_error), Ok(())) => Err(operation_error),
        (Ok(_), Err(close_error)) => {
            Err(close_error.context("Endpoint operation succeeded but close failed"))
        }
        (Err(operation_error), Err(close_error)) => {
            Err(operation_error.context(format!("Endpoint close also failed: {close_error:#}")))
        }
    }
}

fn validate_response(resp: &[u8], cmd: &[u8], allow_non_ok_status: bool) -> Result<()> {
    if resp.len() < 4 {
        bail!(
            "Hub response too short: {} bytes (need at least 4)",
            resp.len()
        );
    }
    let expected_echo = *cmd.first().context("Cannot transfer an empty command")?;
    if resp[2] != expected_echo {
        bail!(
            "Hub response command echo mismatch: got 0x{:02X}, expected 0x{:02X}",
            resp[2],
            expected_echo
        );
    }
    if !allow_non_ok_status && resp[3] != STATUS_OK {
        bail!(
            "Hub rejected command 0x{:02X} with status 0x{:02X}",
            expected_echo,
            resp[3]
        );
    }
    Ok(())
}

fn validate_color_endpoint_open_response(resp: &[u8]) -> Result<()> {
    validate_response(resp, &CMD_OPEN_COLOR, true)?;
    match resp[3] {
        STATUS_OK | STATUS_WRONG_MODE => Ok(()),
        status => bail!(
            "Hub rejected color endpoint open with status 0x{:02X}",
            status
        ),
    }
}

fn best_effort_exchange(device: &HidDevice, cmd: &[u8], data: &[u8]) -> Result<()> {
    device
        .write(&build_packet(cmd, data))
        .context("Failed to write cleanup command")?;
    let mut response = [0u8; PACKET_SIZE];
    let bytes_read = device
        .read_timeout(&mut response, READ_TIMEOUT_MS)
        .context("Failed to read cleanup response")?;
    validate_response(&response[..bytes_read], cmd, false)
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
        bail!("Firmware response too short: {} bytes (need 8)", resp.len());
    }
    validate_response(resp, &CMD_FIRMWARE, false)?;
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
    validate_response(resp, &CMD_READ, false)?;
    validate_data_type(resp, DATA_TYPE_DEVICES, "Device enumeration")?;

    let count = resp[6] as usize;
    let mut devices = Vec::new();
    let mut pos = 7; // Start of per-device entries
    let mut channel: u8 = 0;

    for _ in 0..count {
        channel += 1;

        // Each entry: [reserved, reserved, type, model, reserved, reserved, reserved, id_len, id_bytes...]
        if pos + 8 > resp.len() {
            bail!(
                "Device enumeration truncated at channel {}: missing entry header",
                channel
            );
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
            bail!(
                "Device enumeration truncated at channel {}: id length {} exceeds remaining {} bytes",
                channel,
                id_len,
                resp.len().saturating_sub(pos)
            );
        }

        let device_id = std::str::from_utf8(&resp[pos..pos + id_len])
            .with_context(|| format!("Device id at channel {channel} is not valid UTF-8"))?
            .to_string();
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
    validate_response(resp, &CMD_READ, false)?;
    validate_data_type(resp, DATA_TYPE_SPEEDS, "Speed")?;

    let count = resp[6] as usize;
    validate_fixed_entry_length(resp, count, 3, "Speed")?;
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
    validate_response(resp, &CMD_READ, false)?;
    validate_data_type(resp, DATA_TYPE_TEMPS, "Temperature")?;

    let count = resp[6] as usize;
    validate_fixed_entry_length(resp, count, 3, "Temperature")?;
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

fn validate_data_type(resp: &[u8], expected: [u8; 2], label: &str) -> Result<()> {
    if resp[4..6] != expected {
        bail!(
            "{} response data type mismatch: got {:02X} {:02X}, expected {:02X} {:02X}",
            label,
            resp[4],
            resp[5],
            expected[0],
            expected[1]
        );
    }
    Ok(())
}

fn validate_fixed_entry_length(
    resp: &[u8],
    count: usize,
    entry_size: usize,
    label: &str,
) -> Result<()> {
    let required = 7usize
        .checked_add(
            count
                .checked_mul(entry_size)
                .context("Response entry count overflow")?,
        )
        .context("Response length overflow")?;
    if resp.len() < required {
        bail!(
            "{} response truncated: count {} requires {} bytes, got {}",
            label,
            count,
            required,
            resp.len()
        );
    }
    Ok(())
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

/// Build the framed RGB payload for the color endpoint.
/// Format: [len_lo, len_hi, 0x00, 0x00, 0x12, 0x00, R, G, B, R, G, B, ...]
fn build_rgb_payload(rgb_data: &[u8]) -> Vec<u8> {
    // Length prefix covers: itself (2 bytes) + reserved (2) + dataType (2) + rgb data
    let content_len = 2 + 2 + rgb_data.len(); // reserved + dataType + data
    let total_len = (content_len + 2) as u16; // +2 for the length field itself

    let mut payload = Vec::with_capacity(total_len as usize);
    payload.extend_from_slice(&total_len.to_le_bytes());
    payload.push(0x00); // reserved
    payload.push(0x00); // reserved
    payload.extend_from_slice(&DATA_TYPE_SET_COLOR);
    payload.extend_from_slice(rgb_data);
    payload
}

/// Port power protection: returns a brightness scaling factor (0.0–1.0)
/// based on total LED count to prevent USB power issues.
pub fn port_power_factor(total_leds: u16) -> f32 {
    match total_leds {
        0..=238 => 1.0,
        239..=340 => 0.66,
        341..=442 => 0.33,
        _ => 0.10,
    }
}

use crate::hex_string;

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
    fn endpoint_close_packet_includes_selected_mode() {
        let data_close = build_packet(&CMD_CLOSE, &[MODE_SPEEDS]);
        assert_eq!(&data_close[3..7], &[0x05, 0x01, 0x01, MODE_SPEEDS]);

        let color_close = build_packet(&CMD_CLOSE, &[MODE_SET_COLOR]);
        assert_eq!(&color_close[3..7], &[0x05, 0x01, 0x01, MODE_SET_COLOR]);
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
    fn response_validation_rejects_short_wrong_echo_and_status() {
        assert!(validate_response(&[0, 0, 0x02], &CMD_FIRMWARE, false).is_err());
        assert!(validate_response(&[0, 0, 0x08, 0], &CMD_FIRMWARE, false).is_err());
        assert!(validate_response(&[0, 0, 0x02, 3], &CMD_FIRMWARE, false).is_err());
        assert!(validate_response(&[0, 0, 0x02, 3], &CMD_FIRMWARE, true).is_ok());
    }

    #[test]
    fn color_endpoint_open_accepts_only_observed_live_statuses() {
        let ok = [0, 0, CMD_OPEN_COLOR[0], STATUS_OK];
        let firmware_compatible = [0, 0, CMD_OPEN_COLOR[0], STATUS_WRONG_MODE];
        let other_error = [0, 0, CMD_OPEN_COLOR[0], 0x04];

        assert!(validate_color_endpoint_open_response(&ok).is_ok());
        assert!(validate_color_endpoint_open_response(&firmware_compatible).is_ok());
        assert!(validate_color_endpoint_open_response(&other_error).is_err());

        // The exception is command-specific; generic strict validation still
        // rejects status 0x03 for firmware, data, and write operations.
        assert!(validate_response(&firmware_compatible, &CMD_OPEN_COLOR, false).is_err());
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
    fn parse_speed_entries_rejects_truncated_or_wrong_type_response() {
        let mut resp = vec![0u8; 10];
        resp[2] = CMD_READ[0];
        resp[3] = STATUS_OK;
        resp[4..6].copy_from_slice(&DATA_TYPE_SPEEDS);
        resp[6] = 2;
        assert!(parse_speed_entries(&resp).is_err());

        resp.resize(13, 0);
        resp[4..6].copy_from_slice(&DATA_TYPE_TEMPS);
        assert!(parse_speed_entries(&resp).is_err());
    }

    #[test]
    fn parse_device_entries_rejects_truncated_id() {
        let mut resp = vec![0u8; 15];
        resp[2] = CMD_READ[0];
        resp[3] = STATUS_OK;
        resp[4..6].copy_from_slice(&DATA_TYPE_DEVICES);
        resp[6] = 1;
        resp[14] = 4;
        assert!(parse_device_entries(&resp).is_err());
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
        assert_eq!(LinkDeviceType::from_byte(0x03), LinkDeviceType::RxMaxRgbFan);
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
        assert_eq!(LinkDeviceType::from_byte(0x11), LinkDeviceType::TitanCooler);
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

    #[test]
    fn test_build_rgb_payload_format() {
        // 3 LEDs = 9 bytes of RGB data
        let rgb_data = [255, 0, 0, 0, 255, 0, 0, 0, 255];
        let payload = build_rgb_payload(&rgb_data);

        // Length prefix: 2(len) + 2(reserved) + 2(dataType) + 9(data) = 15
        let expected_len: u16 = 15;
        assert_eq!(u16::from_le_bytes([payload[0], payload[1]]), expected_len);
        // Reserved bytes
        assert_eq!(payload[2], 0x00);
        assert_eq!(payload[3], 0x00);
        // dataTypeSetColor
        assert_eq!(payload[4], DATA_TYPE_SET_COLOR[0]); // 0x12
        assert_eq!(payload[5], DATA_TYPE_SET_COLOR[1]); // 0x00
        // RGB data
        assert_eq!(&payload[6..], &rgb_data);
    }

    #[test]
    fn test_build_rgb_payload_empty() {
        let payload = build_rgb_payload(&[]);
        // Length: 2 + 2 + 2 + 0 = 6
        assert_eq!(u16::from_le_bytes([payload[0], payload[1]]), 6);
        assert_eq!(payload.len(), 6);
    }

    #[test]
    fn test_rgb_payload_chunking_small() {
        // Small payload (< 508 bytes) should fit in one chunk
        let rgb_data = vec![0u8; 100 * 3]; // 100 LEDs
        let payload = build_rgb_payload(&rgb_data);
        let chunks: Vec<&[u8]> = payload.chunks(MAX_COLOR_PAYLOAD).collect();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_rgb_payload_chunking_large() {
        // Large payload (> 508 bytes) should require multiple chunks
        // 200 LEDs = 600 bytes + 6 header = 606 bytes
        let rgb_data = vec![0u8; 200 * 3];
        let payload = build_rgb_payload(&rgb_data);
        assert!(payload.len() > MAX_COLOR_PAYLOAD);
        let chunks: Vec<&[u8]> = payload.chunks(MAX_COLOR_PAYLOAD).collect();
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn test_port_power_factor() {
        assert_eq!(port_power_factor(0), 1.0);
        assert_eq!(port_power_factor(238), 1.0);
        assert_eq!(port_power_factor(239), 0.66);
        assert_eq!(port_power_factor(340), 0.66);
        assert_eq!(port_power_factor(341), 0.33);
        assert_eq!(port_power_factor(442), 0.33);
        assert_eq!(port_power_factor(443), 0.10);
        assert_eq!(port_power_factor(1000), 0.10);
    }

    #[test]
    fn test_led_counts_from_openlinkub() {
        // Confirmed values from OpenLinkHub
        assert_eq!(LinkDeviceType::QxFan.led_count(), 34);
        assert_eq!(LinkDeviceType::LxFan.led_count(), 18);
        assert_eq!(LinkDeviceType::RxMaxRgbFan.led_count(), 8);
        assert_eq!(LinkDeviceType::RxRgbFan.led_count(), 8);
        assert_eq!(LinkDeviceType::LiquidCooler.led_count(), 20);
        assert_eq!(LinkDeviceType::WaterBlock.led_count(), 24);
        assert_eq!(LinkDeviceType::GpuBlock.led_count(), 22);
        assert_eq!(LinkDeviceType::PumpXd5.led_count(), 22);
        assert_eq!(LinkDeviceType::Xg7Block.led_count(), 16);
        assert_eq!(LinkDeviceType::TitanCooler.led_count(), 20);
        assert_eq!(LinkDeviceType::PumpXd6.led_count(), 22);
        // LinkAdapter (LS350 strips connect through this)
        assert_eq!(LinkDeviceType::LinkAdapter.led_count(), 21);
        assert_eq!(LinkDeviceType::LsStrip.led_count(), 21);
        // No LEDs
        assert_eq!(LinkDeviceType::RxMaxFan.led_count(), 0);
        assert_eq!(LinkDeviceType::Psu.led_count(), 0);
    }
}
