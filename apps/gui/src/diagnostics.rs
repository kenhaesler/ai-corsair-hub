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
use serde::{Deserialize, Serialize};

use crate::hardware_thread::identify_color;

/// Which pattern the marker diagnostic shows. `Markers` is the original G05
/// output; the others are black-background variants for counting LEDs.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarkerMode {
    #[default]
    Markers,
    MarkersBlack,
    Ruler,
    Walk {
        device_id: String,
        #[serde(default)]
        step_ms: Option<u64>,
    },
}

impl MarkerMode {
    pub fn name(&self) -> &'static str {
        match self {
            MarkerMode::Markers => "markers",
            MarkerMode::MarkersBlack => "markers_black",
            MarkerMode::Ruler => "ruler",
            MarkerMode::Walk { .. } => "walk",
        }
    }
}

/// Walk mode parameters echoed in the report/manifest.
#[derive(Debug, Clone, Serialize)]
pub struct WalkReport {
    pub device_id: String,
    /// Epoch milliseconds (wall clock) when the walk started.
    pub started_at: u64,
    pub step_ms: u64,
    pub led_count: usize,
}

pub const WALK_STEP_DEFAULT_MS: u64 = 400;

/// Walk step clamped to 100..=2000 ms (default 400).
pub fn walk_step_ms(step_ms: Option<u64>) -> u64 {
    step_ms.unwrap_or(WALK_STEP_DEFAULT_MS).clamp(100, 2000)
}

/// First LED of every slot.
pub const MARKER_FIRST: [u8; 3] = [255, 255, 255];
/// Second LED of every slot with at least 3 LEDs (shows direction).
pub const MARKER_SECOND: [u8; 3] = [0, 255, 0];
/// Last LED of every slot.
pub const MARKER_LAST: [u8; 3] = [255, 0, 0];
/// Ruler mode: every 10th LED.
pub const RULER_TEN: [u8; 3] = [255, 255, 0];
/// Ruler mode: every 5th LED that is not a multiple of 10.
pub const RULER_FIVE: [u8; 3] = [0, 0, 255];

#[derive(Debug, Clone, Serialize)]
pub struct LegendEntry {
    pub label: &'static str,
    pub rgb: [u8; 3],
    pub meaning: &'static str,
}

pub fn legend(mode: &MarkerMode) -> Vec<LegendEntry> {
    let white = |meaning| LegendEntry {
        label: "white",
        rgb: MARKER_FIRST,
        meaning,
    };
    let red = |meaning| LegendEntry {
        label: "red",
        rgb: MARKER_LAST,
        meaning,
    };
    match mode {
        MarkerMode::Markers => {}
        MarkerMode::MarkersBlack => {
            return vec![
                white("first LED of the slot (LED 0)"),
                LegendEntry {
                    label: "green",
                    rgb: MARKER_SECOND,
                    meaning: "second LED (LED 1, shows direction; slots with 3+ LEDs)",
                },
                red("last LED of the slot"),
            ];
        }
        MarkerMode::Ruler => {
            return vec![
                white("first LED of the slot (LED 0)"),
                LegendEntry {
                    label: "yellow",
                    rgb: RULER_TEN,
                    meaning: "every 10th LED (LED 10, 20, 30, ...)",
                },
                LegendEntry {
                    label: "blue",
                    rgb: RULER_FIVE,
                    meaning: "every other 5th LED (LED 5, 15, 25, ...)",
                },
                red("last LED of the slot (overrides the others)"),
            ];
        }
        MarkerMode::Walk { .. } => {
            return vec![
                white("the walking LED on the target slot (one step at a time)"),
                red("last LED of the target slot while the white LED is elsewhere"),
            ];
        }
    }
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
    pub mode: &'static str,
    /// Set in walk mode only.
    pub walk: Option<WalkReport>,
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

fn black(n: usize) -> Vec<Rgb> {
    vec![Rgb::new(0, 0, 0); n]
}

fn set(leds: &mut [Rgb], i: usize, c: [u8; 3]) {
    leds[i] = Rgb::new(c[0], c[1], c[2]);
}

/// Same marker rules as [`marker_leds`] but every other LED is black.
pub fn markers_black_leds(n: usize) -> Vec<Rgb> {
    marker_leds(n, [0, 0, 0])
}

/// Ruler: black, LED 0 white, every 10th yellow, every other 5th blue, last
/// red (overrides everything; a 1-LED slot shows white only).
pub fn ruler_leds(n: usize) -> Vec<Rgb> {
    let mut leds = black(n);
    for i in (5..n).step_by(5) {
        set(&mut leds, i, if i % 10 == 0 { RULER_TEN } else { RULER_FIVE });
    }
    if n >= 2 {
        set(&mut leds, n - 1, MARKER_LAST);
    }
    if n >= 1 {
        set(&mut leds, 0, MARKER_FIRST);
    }
    leds
}

/// Index of the walking LED after `elapsed_ms`.
pub fn walk_index(elapsed_ms: u64, step_ms: u64, n: usize) -> usize {
    if n == 0 || step_ms == 0 {
        return 0;
    }
    ((elapsed_ms / step_ms) % n as u64) as usize
}

/// Walk frame for the target slot: one white LED at `idx`, the last LED red
/// while the white LED is elsewhere, everything else black.
pub fn walk_leds(n: usize, idx: usize) -> Vec<Rgb> {
    let mut leds = black(n);
    if n >= 1 {
        set(&mut leds, n - 1, MARKER_LAST);
        set(&mut leds, idx.min(n - 1), MARKER_FIRST);
    }
    leds
}

/// Build marker frames (one per slot, keyed by device_id, at the slot's
/// effective length) and the per-hub DTOs from a slot manifest. Includes
/// devices that no zone references. Hubs are indexed in manifest order so the
/// body colour follows the Identify rule `(hub_idx * 8 + channel) % 16`.
/// Walk mode returns the step-0 frame for the target and black elsewhere; the
/// worker recomputes the target frame per tick.
pub fn build_marker(
    manifest: &[HubSlotManifest],
    config: &AppConfig,
    mode: &MarkerMode,
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
                leds: match mode {
                    MarkerMode::Markers => marker_leds(s.led_count, body),
                    MarkerMode::MarkersBlack => markers_black_leds(s.led_count),
                    MarkerMode::Ruler => ruler_leds(s.led_count),
                    MarkerMode::Walk { device_id, .. } if *device_id == s.device_id => {
                        walk_leds(s.led_count, 0)
                    }
                    MarkerMode::Walk { .. } => black(s.led_count),
                },
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

    fn rgb(c: [u8; 3]) -> Rgb {
        Rgb::new(c[0], c[1], c[2])
    }

    /// Indices whose colour equals `c`.
    fn at(leds: &[Rgb], c: [u8; 3]) -> Vec<usize> {
        (0..leds.len()).filter(|&i| leds[i] == rgb(c)).collect()
    }

    #[test]
    fn ruler_pattern_n34_and_n80() {
        let r = ruler_leds(34);
        assert_eq!(r.len(), 34);
        assert_eq!(at(&r, MARKER_FIRST), vec![0]);
        assert_eq!(at(&r, RULER_TEN), vec![10, 20, 30]);
        assert_eq!(at(&r, RULER_FIVE), vec![5, 15, 25]);
        assert_eq!(at(&r, MARKER_LAST), vec![33]);
        assert_eq!(at(&r, [0, 0, 0]).len(), 34 - 8);

        let r = ruler_leds(80);
        assert_eq!(at(&r, RULER_TEN), vec![10, 20, 30, 40, 50, 60, 70]);
        assert_eq!(at(&r, RULER_FIVE), vec![5, 15, 25, 35, 45, 55, 65, 75]);
        assert_eq!(at(&r, MARKER_LAST), vec![79]);
        assert_eq!(at(&r, MARKER_FIRST), vec![0]);

        // Last LED overrides a multiple of 10 / 5.
        assert_eq!(at(&ruler_leds(31), MARKER_LAST), vec![30]);
        assert!(at(&ruler_leds(31), RULER_TEN).contains(&20));
        assert_eq!(at(&ruler_leds(16), MARKER_LAST), vec![15]);
        assert_eq!(ruler_leds(1), vec![rgb(MARKER_FIRST)]);
        assert!(ruler_leds(0).is_empty());
    }

    #[test]
    fn markers_black_pattern() {
        let b = Rgb::new(0, 0, 0);
        let (w, g, r) = (rgb(MARKER_FIRST), rgb(MARKER_SECOND), rgb(MARKER_LAST));
        assert!(markers_black_leds(0).is_empty());
        assert_eq!(markers_black_leds(1), vec![w]);
        assert_eq!(markers_black_leds(2), vec![w, r]);
        assert_eq!(markers_black_leds(3), vec![w, g, r]);
        assert_eq!(markers_black_leds(6), vec![w, g, b, b, b, r]);
    }

    #[test]
    fn walk_index_progression_and_wrap() {
        assert_eq!(walk_index(0, 400, 34), 0);
        assert_eq!(walk_index(399, 400, 34), 0);
        assert_eq!(walk_index(400, 400, 34), 1);
        assert_eq!(walk_index(33 * 400, 400, 34), 33);
        assert_eq!(walk_index(34 * 400, 400, 34), 0);
        assert_eq!(walk_index(35 * 400 + 7, 400, 34), 1);
        assert_eq!(walk_index(1000, 400, 0), 0);
        assert_eq!(walk_step_ms(None), 400);
        assert_eq!(walk_step_ms(Some(1)), 100);
        assert_eq!(walk_step_ms(Some(99_999)), 2000);
        assert_eq!(walk_step_ms(Some(250)), 250);
    }

    #[test]
    fn walk_leds_pattern() {
        let l = walk_leds(5, 2);
        assert_eq!(at(&l, MARKER_FIRST), vec![2]);
        assert_eq!(at(&l, MARKER_LAST), vec![4]);
        // White LED on the last position replaces the red one.
        let l = walk_leds(5, 4);
        assert_eq!(at(&l, MARKER_FIRST), vec![4]);
        assert!(at(&l, MARKER_LAST).is_empty());
        assert!(walk_leds(0, 0).is_empty());
    }

    fn slot(id: &str, channel: u8, n: usize, start: usize) -> corsair_fancontrol::SlotManifest {
        corsair_fancontrol::SlotManifest {
            device_id: id.into(),
            channel,
            device_type: "QX".into(),
            led_count: n,
            count_source: corsair_fancontrol::LedCountSource::TypeDefault,
            start_led: start,
            end_led: start + n,
            start_byte: start * 3,
        }
    }

    fn two_slot_manifest() -> Vec<HubSlotManifest> {
        vec![HubSlotManifest {
            hub_serial: "H".into(),
            healthy: true,
            firmware: "0".into(),
            firmware_led_counts: Vec::new(),
            total_leds: 34 + 18,
            slots: vec![slot("A", 1, 34, 0), slot("B", 2, 18, 34)],
            skipped_zero_count: Vec::new(),
        }]
    }

    #[test]
    fn build_marker_modes_over_manifest() {
        let cfg = AppConfig::default();
        let m = two_slot_manifest();

        // Markers mode is byte-identical to marker_leds with the Identify body.
        let (frames, _) = build_marker(&m, &cfg, &MarkerMode::Markers);
        let (body_a, _) = identify_color(0, 1);
        let (body_b, _) = identify_color(0, 2);
        assert_eq!(frames[0].leds, marker_leds(34, body_a));
        assert_eq!(frames[1].leds, marker_leds(18, body_b));

        let (frames, _) = build_marker(&m, &cfg, &MarkerMode::Ruler);
        assert_eq!(frames[0].leds, ruler_leds(34));
        assert_eq!(frames[1].leds, ruler_leds(18));

        let (frames, _) = build_marker(&m, &cfg, &MarkerMode::MarkersBlack);
        assert_eq!(frames[0].leds, markers_black_leds(34));

        let walk = MarkerMode::Walk {
            device_id: "B".into(),
            step_ms: None,
        };
        let (frames, _) = build_marker(&m, &cfg, &walk);
        assert_eq!(frames[0].leds, black(34));
        assert_eq!(frames[1].leds, walk_leds(18, 0));
    }

    #[test]
    fn mode_names_and_deserialize() {
        let cfg = AppConfig::default();
        let (frames, hubs) = build_marker(&[], &cfg, &MarkerMode::Ruler);
        assert!(frames.is_empty() && hubs.is_empty());
        assert_eq!(MarkerMode::default(), MarkerMode::Markers);
        assert_eq!(MarkerMode::Markers.name(), "markers");
        assert_eq!(MarkerMode::MarkersBlack.name(), "markers_black");
        assert_eq!(MarkerMode::Ruler.name(), "ruler");
        let m: MarkerMode =
            serde_json::from_str(r#"{"kind":"walk","device_id":"D"}"#).unwrap();
        assert_eq!(
            m,
            MarkerMode::Walk {
                device_id: "D".into(),
                step_ms: None
            }
        );
        let m: MarkerMode = serde_json::from_str(r#"{"kind":"markers_black"}"#).unwrap();
        assert_eq!(m, MarkerMode::MarkersBlack);
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
