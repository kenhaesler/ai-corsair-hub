use serde::Serialize;

use crate::Rgb;

/// A single frame of LED data for one device.
///
/// Post-Step-5 (PR2): the frame is keyed by stable `device_id` only. The
/// renderer no longer emits frames with `(hub_serial, channel)` location
/// tags — the control loop's `send_rgb_frames` resolves each `device_id` via
/// the runtime registry immediately before the wire write. If a DeviceTarget
/// reaches frame construction without a device_id, that is a bug upstream of
/// the renderer (the registry lookup in `apply_rgb_config` should have
/// populated it, or skipped the device outright).
#[derive(Debug, Clone, Serialize)]
pub struct RgbFrame {
    /// Stable device identity (26-hex string burned in at manufacturing).
    /// Mandatory as of Step 5.
    pub device_id: String,
    pub leds: Vec<Rgb>,
}
