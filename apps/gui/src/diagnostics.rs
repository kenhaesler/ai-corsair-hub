//! LED marker diagnostic (G05): marker frames, report DTOs and export helpers.
//!
//! The diagnostic proves where every LED slot lands physically using exactly
//! the production addressing: it is driven from `ControlLoop::rgb_slot_manifest`
//! (effective LED counts, wire order) and its frames travel through the same
//! power-factor conversion and `send_rgb_frames` path as normal rendering.
//! This module only builds data; the worker owns timing and hardware I/O.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use corsair_common::config::AppConfig;
use corsair_fancontrol::HubSlotManifest;
use corsair_rgb::{Rgb, RgbFrame};
use serde::Serialize;

use crate::hardware_thread::identify_color;

/// First LED of every slot.
pub const MARKER_FIRST: [u8; 3] = [255, 255, 255];
/// Second LED of every slot with at least 3 LEDs (shows direction).
pub const MARKER_SECOND: [u8; 3] = [0, 255, 0];
/// Last LED of every slot.
pub const MARKER_LAST: [u8; 3] = [255, 0, 0];

#[derive(Debug, Clone, Serialize)]
pub struct LegendEntry {
    pub label: &'static str,
    pub rgb: [u8; 3],
    pub meaning: &'static str,
}

pub fn legend() -> Vec<LegendEntry> {
    vec![
        LegendEntry {
            label: "white",
            rgb: MARKER_FIRST,
            meaning: "first LED of the slot (LED 0)",
        },
        LegendEntry {
            label: "green",
            rgb: MARKER_SECOND,
            meaning: "second LED (LED 1, shows direction; slots with 3+ LEDs)",
        },
        LegendEntry {
            label: "red",
            rgb: MARKER_LAST,
            meaning: "last LED of the slot",
        },
        LegendEntry {
            label: "body colour",
            rgb: [0, 0, 0],
            meaning: "every other LED uses the device's Identify palette colour \
                      (see body_color_name per slot)",
        },
    ]
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkerSlotDto {
    pub device_id: String,
    pub name: Option<String>,
    pub channel: u8,
    pub device_type: String,
    pub led_count: usize,
    pub count_source: &'static str,
    pub start_led: usize,
    /// Exclusive.
    pub end_led: usize,
    pub start_byte: usize,
    pub body_color_name: String,
    pub body_rgb: [u8; 3],
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkerSkippedDto {
    pub device_id: String,
    pub name: Option<String>,
    pub channel: u8,
    pub device_type: String,
    pub count_source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkerHubDto {
    pub hub_serial: String,
    pub healthy: bool,
    pub firmware: String,
    /// Raw 0x1d table as reported by the hub firmware (channel -> LEDs).
    pub firmware_led_counts: BTreeMap<String, u16>,
    pub total_leds: usize,
    /// Wire order (ascending channel).
    pub slots: Vec<MarkerSlotDto>,
    pub skipped_zero_count: Vec<MarkerSkippedDto>,
}

/// Reply of the diagnostic command; also the content of the exported file
/// (with `file_path`/`file_error` unset at export time).
#[derive(Debug, Clone, Serialize)]
pub struct LedMarkerReport {
    pub app_version: &'static str,
    pub generated_at_utc: String,
    pub duration_secs: u64,
    /// Total LEDs across all marker frames (drives the port power factor).
    pub total_leds: usize,
    /// Port power-protection scaling applied to the marker colours.
    pub power_factor: f32,
    pub legend: Vec<LegendEntry>,
    pub hubs: Vec<MarkerHubDto>,
    pub file_path: Option<String>,
    pub file_error: Option<String>,
}

/// Marker colours for one slot of `n` LEDs: white first, green second (n >= 3),
/// red last (n >= 2), everything else `body`. A 1-LED slot shows white.
pub fn marker_leds(n: usize, body: [u8; 3]) -> Vec<Rgb> {
    let mut leds = vec![Rgb::new(body[0], body[1], body[2]); n];
    if n >= 2 {
        let l = MARKER_LAST;
        leds[n - 1] = Rgb::new(l[0], l[1], l[2]);
    }
    if n >= 3 {
        let g = MARKER_SECOND;
        leds[1] = Rgb::new(g[0], g[1], g[2]);
    }
    if n >= 1 {
        let w = MARKER_FIRST;
        leds[0] = Rgb::new(w[0], w[1], w[2]);
    }
    leds
}

/// Build marker frames (one per slot, keyed by device_id, at the slot's
/// effective length) and the per-hub DTOs from a slot manifest. Includes
/// devices that no zone references. Hubs are indexed in manifest order so the
/// body colour follows the Identify rule `(hub_idx * 8 + channel) % 16`.
pub fn build_marker(
    manifest: &[HubSlotManifest],
    config: &AppConfig,
) -> (Vec<RgbFrame>, Vec<MarkerHubDto>) {
    let name_of = |id: &str| {
        config
            .device_entry(id)
            .and_then(|d| d.name.clone())
            .filter(|n| !n.is_empty())
    };
    let mut frames = Vec::new();
    let mut hubs = Vec::new();
    for (hub_idx, hub) in manifest.iter().enumerate() {
        let mut slots = Vec::with_capacity(hub.slots.len());
        for s in &hub.slots {
            let (body, color_name) = identify_color(hub_idx, s.channel);
            frames.push(RgbFrame {
                device_id: s.device_id.clone(),
                leds: marker_leds(s.led_count, body),
            });
            slots.push(MarkerSlotDto {
                device_id: s.device_id.clone(),
                name: name_of(&s.device_id),
                channel: s.channel,
                device_type: s.device_type.clone(),
                led_count: s.led_count,
                count_source: s.count_source.as_str(),
                start_led: s.start_led,
                end_led: s.end_led,
                start_byte: s.start_byte,
                body_color_name: color_name.to_string(),
                body_rgb: body,
            });
        }
        hubs.push(MarkerHubDto {
            hub_serial: hub.hub_serial.clone(),
            healthy: hub.healthy,
            firmware: hub.firmware.clone(),
            firmware_led_counts: hub
                .firmware_led_counts
                .iter()
                .map(|(c, n)| (c.to_string(), *n))
                .collect(),
            total_leds: hub.total_leds,
            slots,
            skipped_zero_count: hub
                .skipped_zero_count
                .iter()
                .map(|d| MarkerSkippedDto {
                    device_id: d.device_id.clone(),
                    name: name_of(&d.device_id),
                    channel: d.channel,
                    device_type: d.device_type.clone(),
                    count_source: d.count_source.as_str(),
                })
                .collect(),
        });
    }
    (frames, hubs)
}

/// `(compact "YYYYMMDD-HHMMSS", ISO-8601 "YYYY-MM-DDTHH:MM:SSZ")`, both UTC.
pub fn utc_stamps(t: SystemTime) -> (String, String) {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (
        format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}"),
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"),
    )
}

pub fn manifest_file_name(compact_stamp: &str) -> String {
    format!("led-manifest-{compact_stamp}.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn marker_leds_layout_by_length() {
        let body = [10, 20, 30];
        let b = Rgb::new(10, 20, 30);
        let w = Rgb::new(255, 255, 255);
        let g = Rgb::new(0, 255, 0);
        let r = Rgb::new(255, 0, 0);
        assert!(marker_leds(0, body).is_empty());
        assert_eq!(marker_leds(1, body), vec![w]);
        assert_eq!(marker_leds(2, body), vec![w, r]);
        assert_eq!(marker_leds(3, body), vec![w, g, r]);
        assert_eq!(marker_leds(5, body), vec![w, g, b, b, r]);
    }

    #[test]
    fn utc_stamps_hand_computed() {
        let at = |s: u64| UNIX_EPOCH + Duration::from_secs(s);
        // 1_700_000_000 = 19675 days + 80000 s = 2023-11-14 22:13:20 UTC.
        assert_eq!(
            utc_stamps(at(1_700_000_000)),
            ("20231114-221320".to_string(), "2023-11-14T22:13:20Z".to_string())
        );
        assert_eq!(utc_stamps(at(0)).0, "19700101-000000");
        // 11016 days after the epoch is 2000-02-29 (leap day).
        assert_eq!(utc_stamps(at(951_782_400)).0, "20000229-000000");
        assert_eq!(
            manifest_file_name("20231114-221320"),
            "led-manifest-20231114-221320.json"
        );
    }
}
