use serde::Serialize;

use crate::Rgb;

/// A single frame of LED data for one device channel.
#[derive(Debug, Clone, Serialize)]
pub struct RgbFrame {
    pub hub_serial: String,
    pub channel: u8,
    pub leds: Vec<Rgb>,
}
