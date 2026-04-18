use serde::Serialize;

use crate::Rgb;

/// A single frame of LED data for one device channel.
///
/// During the V1→V2 identity refactor this struct carries BOTH the legacy
/// (hub_serial, channel) location keys AND the stable `device_id`. The
/// renderer populates `device_id` when the device picker provided one;
/// otherwise it stays empty and downstream code falls back to the legacy
/// path. A later step (PR2 Step 5) drops hub_serial/channel in favor of
/// device_id-only once all call sites are migrated.
#[derive(Debug, Clone, Serialize)]
pub struct RgbFrame {
    pub hub_serial: String,
    pub channel: u8,
    /// Stable device identity. Empty string during the transition when the
    /// zone targets came from a V1 config (hub_serial + channel only).
    pub device_id: String,
    pub leds: Vec<Rgb>,
}
