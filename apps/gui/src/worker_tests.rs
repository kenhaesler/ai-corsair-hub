//! Characterization tests for the GUI hardware worker, driven through the
//! hardware-free harness in `worker_harness`.
//!
//! Two kinds of test live here:
//!   * plain `#[test]`s pin CURRENT behaviour (including behaviour that the
//!     reliability audit calls a bug -- these must keep passing until the
//!     fixing PR deliberately changes them);
//!   * `#[ignore = "bug regression: audit #N, fixed in PR X"]` tests assert
//!     the DESIRED behaviour and fail on the unfixed baseline. The fixing PR
//!     un-ignores them (and flips/removes the matching characterization test).
//!
//! Expected values are hand-computed in comments; production helpers such as
//! `port_power_factor` are never called to produce expectations.

use std::collections::HashMap;
use std::time::Duration;

use corsair_common::config::{
    BlendMode, CurvePoint, EffectConfig, FanGroupConfig, FanMode, LayerConfig, RgbConfig,
    RgbDeviceRef, RgbZoneConfig, TempSourceConfig,
};
use corsair_hid::{LinkDeviceType, PsuStatus, RailReading};
use corsair_rgb::Rgb;

use crate::dto::ManualDutyResult;
use crate::hardware_thread::StepOutcome;
use crate::state::HwCommand;
use crate::worker_harness::{
    device, fixed_group, hub_info_for, v1_config, Harness, HubSpec, RecordingHub,
};

use std::sync::Arc;

const MS: fn(u64) -> Duration = Duration::from_millis;

/// One hub, two QX fans on channels 1 and 2 (34 LEDs each by type default).
fn fan_hub(serial: &'static str) -> HubSpec {
    HubSpec {
        serial,
        devices: vec![
            device(1, &format!("{serial}_FAN1"), LinkDeviceType::QxFan),
            device(2, &format!("{serial}_FAN2"), LinkDeviceType::QxFan),
        ],
        led_counts: HashMap::new(),
        pump_channels: Vec::new(),
    }
}

fn rgb_off() -> RgbConfig {
    RgbConfig {
        enabled: false,
        brightness: 80,
        fps: 30,
        hardware_output: false,
        zones: Vec::new(),
        presets: Vec::new(),
    }
}

/// Fixed-duty harness: HUB0 channels 1+2 at 50 %, RGB off, 1000 ms poll.
fn fixed_harness() -> Harness {
    Harness::new(
        v1_config(
            vec![fixed_group("g", "HUB0", &[1, 2], 50.0)],
            rgb_off(),
            1000,
        ),
        vec![fan_hub("HUB0")],
        &[("cpu", 40.0)],
    )
}

/// Curve group on the "cpu" sensor: 30 C -> 20 %, 80 C -> 100 %, 5 C
/// hysteresis, ramp-up `ramp_up` %/s, ramp-down 2 %/s.
fn curve_group(ramp_up: f64) -> FanGroupConfig {
    FanGroupConfig {
        name: "g".to_string(),
        channels: vec![1, 2],
        hub_serial: Some("HUB0".to_string()),
        device_ids: Vec::new(),
        mode: FanMode::Curve {
            points: vec![
                CurvePoint { temp: 30.0, duty: 20.0 },
                CurvePoint { temp: 80.0, duty: 100.0 },
            ],
            hysteresis: 5.0,
            ramp_rate: ramp_up,
            ramp_down_rate: Some(2.0),
            temp_source: TempSourceConfig {
                sensors: vec!["cpu".to_string()],
                weights: vec![1.0],
            },
        },
        preset_applied: None,
    }
}

fn rgb_on(fps: u8) -> RgbConfig {
    RgbConfig {
        enabled: true,
        fps,
        ..rgb_off()
    }
}

/// Duty (second tuple element) of channel 1 in the most recent set_speeds call.
fn last_ch1_duty(h: &Harness, hub: &str) -> u8 {
    h.hub(hub)
        .speed_calls()
        .last()
        .expect("no set_speeds call")
        .iter()
        .find(|(c, _)| *c == 1)
        .expect("channel 1 not commanded")
        .1
}

fn json(payload: &str) -> serde_json::Value {
    serde_json::from_str(payload).expect("event payload is valid JSON")
}

// ---------------------------------------------------------------------------
// Cadence (audit #1)
// ---------------------------------------------------------------------------

/// RGB off, 1000 ms poll: one fan tick, one sensor-update and one 1000 ms
/// sleep per step; no RGB traffic.
#[test]
fn baseline_cadence_rgb_disabled() {
    let mut h = fixed_harness();
    h.steps(5);

    let calls = h.hub("HUB0").speed_calls();
    assert_eq!(calls.len(), 5, "one set_speeds per step");
    for c in &calls {
        assert_eq!(c, &vec![(1, 50), (2, 50)]);
    }
    assert_eq!(h.clock.sleeps(), vec![MS(1000); 5]);
    assert_eq!(h.clock.elapsed(), MS(5000));
    assert_eq!(h.events.count("sensor-update"), 5);
    assert_eq!(h.events.count("rgb-frame"), 0);
    assert_eq!(h.events.count("hub-lost"), 0);
}

/// Audit #1 (characterization): enabling RGB at 30 FPS shortens the loop
/// period to 33 ms and every iteration runs a full fan tick, so ~30 fan
/// ticks happen per fake second instead of 1.
#[test]
fn rgb_30fps_runs_full_fan_tick_every_step() {
    let mut h = Harness::new(
        v1_config(vec![fixed_group("g", "HUB0", &[1, 2], 50.0)], rgb_on(30), 1000),
        vec![fan_hub("HUB0")],
        &[("cpu", 40.0)],
    );
    h.steps(30);

    // 1000 / 30 = 33 ms (integer division) -> sleep target min(1000, 33).
    assert_eq!(h.clock.sleeps(), vec![MS(33); 30]);
    assert_eq!(h.clock.elapsed(), MS(990));
    assert_eq!(
        h.hub("HUB0").speed_calls().len(),
        30,
        "a fan tick (set_speeds) on every step"
    );
    assert_eq!(h.events.count("sensor-update"), 30);
    // Step 1 runs at t=0 (< 33 ms since last_rgb_tick); steps 2..=30 render.
    assert_eq!(h.events.count("rgb-frame"), 29);
}

/// Audit #1 (desired): fan tick rate must not depend on RGB FPS.
#[test]
#[ignore = "bug regression: audit #1, fixed in PR 4"]
fn fan_tick_rate_is_independent_of_rgb_fps() {
    let mut h = Harness::new(
        v1_config(vec![fixed_group("g", "HUB0", &[1, 2], 50.0)], rgb_on(30), 1000),
        vec![fan_hub("HUB0")],
        &[("cpu", 40.0)],
    );
    h.steps(30); // ~0.99 s of fake time
    let ticks = h.hub("HUB0").speed_calls().len();
    assert!(ticks <= 2, "expected ~1 fan tick per 1000 ms poll, got {ticks}");
}

/// Audit #1 consequence (characterization): the acoustic ramp is advanced by
/// `poll_interval_ms` per tick, not by elapsed time, so a 3 %/s ramp rises
/// 3 % per *tick*. 30 ticks at 30 FPS (~0.99 s) reach 90 %.
///
/// Hand computation: cpu = 80 C >= curve end -> raw 100 %. First tick
/// |80-0| >= 5 C hysteresis -> target 100. dt = 1.0 s (poll 1000 ms) so
/// each tick adds 3.0: after k ticks current = 3k; k = 30 -> 90 -> commanded
/// 90 (above the 20 % fan minimum).
#[test]
fn rgb_30fps_ramp_advances_per_tick_not_per_second() {
    let mut h = Harness::new(
        v1_config(vec![curve_group(3.0)], rgb_on(30), 1000),
        vec![fan_hub("HUB0")],
        &[("cpu", 80.0)],
    );
    h.steps(30);
    assert_eq!(h.clock.elapsed(), MS(990));
    assert_eq!(last_ch1_duty(&h, "HUB0"), 90);
}

/// Audit #1 (desired): a 3 %/s ramp advances by elapsed time -- after ~1 s
/// it is at about 3 %, i.e. still pinned to the 20 % fan minimum.
#[test]
#[ignore = "bug regression: audit #1, fixed in PR 4"]
fn ramp_advances_with_elapsed_time_not_ticks() {
    let mut h = Harness::new(
        v1_config(vec![curve_group(3.0)], rgb_on(30), 1000),
        vec![fan_hub("HUB0")],
        &[("cpu", 80.0)],
    );
    h.steps(30);
    assert!(last_ch1_duty(&h, "HUB0") <= 23, "got {}", last_ch1_duty(&h, "HUB0"));
}

/// Audit #1 / GetSnapshot (characterization): the on-demand snapshot runs
/// `ControlLoop::tick`, so one step containing GetSnapshot commands the fans
/// twice (once for the snapshot, once for the step's own tick).
#[test]
fn get_snapshot_causes_extra_control_tick() {
    let mut h = fixed_harness();
    let (reply, outcome) = h.call(|reply| HwCommand::GetSnapshot { reply });
    assert_eq!(outcome, StepOutcome::Continue);
    assert!(reply.is_ok());
    assert_eq!(h.hub("HUB0").speed_calls().len(), 2);
}

/// Audit #1 (desired): reading the dashboard must never command fans.
#[test]
#[ignore = "bug regression: audit #1, fixed in PR 4"]
fn get_snapshot_must_not_command_fans() {
    let mut h = fixed_harness();
    let (reply, _) = h.call(|reply| HwCommand::GetSnapshot { reply });
    assert!(reply.is_ok());
    assert_eq!(
        h.hub("HUB0").speed_calls().len(),
        1,
        "only the step's own tick may command fans"
    );
}

// ---------------------------------------------------------------------------
// Config coherence (audits #5, #8)
// ---------------------------------------------------------------------------

fn new_rgb_config() -> RgbConfig {
    RgbConfig {
        enabled: true,
        brightness: 55,
        fps: 60,
        hardware_output: false,
        zones: Vec::new(),
        presets: Vec::new(),
    }
}

/// Audit #5 (characterization): SetRgbConfig updates the worker's config,
/// renderer inputs and disk, but `ControlLoop::config()` -- what GetConfig
/// returns -- keeps the OLD rgb section.
#[test]
fn set_rgb_config_then_get_config_returns_old_rgb_section() {
    let mut h = fixed_harness();
    let (reply, _) = h.call(|reply| HwCommand::SetRgbConfig {
        config: new_rgb_config(),
        reply,
    });
    assert!(reply.is_ok());

    // Worker, renderer inputs and persisted copy carry the new section...
    assert_eq!(h.worker.config.rgb.brightness, 55);
    assert!(h.worker.rgb_enabled);
    assert_eq!(h.worker.rgb_interval, MS(16)); // 1000 / 60
    assert_eq!(h.store.saves().last().unwrap().rgb.brightness, 55);

    // ...but GetConfig reads the control loop's stale copy.
    let (cfg, _) = h.call(|reply| HwCommand::GetConfig { reply });
    let cfg = cfg.unwrap();
    assert_eq!(cfg.rgb.brightness, 80, "stale rgb section (audit #5)");
    assert!(!cfg.rgb.enabled);
}

/// Audit #5 (desired): GetConfig reflects the RGB config just set.
#[test]
#[ignore = "bug regression: audit #5, fixed in PR 5"]
fn get_config_reflects_set_rgb_config() {
    let mut h = fixed_harness();
    let (reply, _) = h.call(|reply| HwCommand::SetRgbConfig {
        config: new_rgb_config(),
        reply,
    });
    assert!(reply.is_ok());
    let (cfg, _) = h.call(|reply| HwCommand::GetConfig { reply });
    assert_eq!(cfg.unwrap().rgb.brightness, 55);
}

/// Audit #5 (characterization): SetRgbEnabled applies live and replies Ok
/// even when persisting fails (the failure is only logged).
#[test]
fn set_rgb_enabled_with_failing_store_replies_ok() {
    let mut h = fixed_harness();
    h.store.set_fail(true);
    let (reply, _) = h.call(|reply| HwCommand::SetRgbEnabled { enabled: true, reply });
    assert_eq!(reply, Ok(()));
    assert!(h.worker.rgb_enabled, "applied live despite save failure");
    assert_eq!(h.store.attempts(), 1);
    assert!(h.store.saves().is_empty());
}

/// Audit #5 (desired): a persistence failure is reported to the caller.
#[test]
#[ignore = "bug regression: audit #5, fixed in PR 5"]
fn set_rgb_enabled_with_failing_store_reports_error() {
    let mut h = fixed_harness();
    h.store.set_fail(true);
    let (reply, _) = h.call(|reply| HwCommand::SetRgbEnabled { enabled: true, reply });
    assert!(reply.is_err(), "save failure must propagate");
}

/// SetRgbConfig with a failing store: also replies Ok (same audit #5 class).
#[test]
fn set_rgb_config_with_failing_store_replies_ok() {
    let mut h = fixed_harness();
    h.store.set_fail(true);
    let (reply, _) = h.call(|reply| HwCommand::SetRgbConfig {
        config: new_rgb_config(),
        reply,
    });
    assert_eq!(reply, Ok(()));
    assert_eq!(h.worker.config.rgb.brightness, 55);
}

fn updated_config(duty: f64, poll_ms: u64) -> corsair_common::config::AppConfig {
    v1_config(
        vec![fixed_group("g", "HUB0", &[1, 2], duty)],
        rgb_off(),
        poll_ms,
    )
}

/// UpdateConfig with a failing store: Err reply, nothing applied live.
#[test]
fn update_config_with_failing_store_is_rejected_without_live_apply() {
    let mut h = fixed_harness();
    h.store.set_fail(true);
    let (reply, _) = h.call(|reply| HwCommand::UpdateConfig {
        config: updated_config(80.0, 500),
        reply,
    });
    let err = reply.unwrap_err();
    assert!(err.starts_with("Failed to save config"), "got: {err}");

    // Control loop config, worker config and poll interval unchanged.
    assert_eq!(h.worker.control_loop.config().general.poll_interval_ms, 1000);
    assert_eq!(h.worker.config.general.poll_interval_ms, 1000);
    assert_eq!(h.worker.poll_interval, MS(1000));
    assert!(matches!(
        h.worker.control_loop.config().fan_groups[0].mode,
        FanMode::Fixed { duty_percent } if duty_percent == 50.0
    ));
    // The tick in that very step (and the next) still commands the old 50 %.
    h.steps(1);
    for call in h.hub("HUB0").speed_calls() {
        assert_eq!(call, vec![(1, 50), (2, 50)]);
    }
}

/// UpdateConfig success: saved once, applied live, new poll interval used.
#[test]
fn update_config_success_saves_and_applies_live() {
    let mut h = fixed_harness();
    let (reply, _) = h.call(|reply| HwCommand::UpdateConfig {
        config: updated_config(80.0, 500),
        reply,
    });
    assert_eq!(reply, Ok(()));
    assert_eq!(h.store.saves().len(), 1);
    assert_eq!(h.worker.poll_interval, MS(500));
    // Same step: command processed first, then the tick uses the new 80 %.
    assert_eq!(h.hub("HUB0").speed_calls(), vec![vec![(1, 80), (2, 80)]]);
    // The sleep target is read after command handling in the same iteration,
    // so the very step that applied the config already sleeps 500 ms.
    assert_eq!(h.clock.sleeps(), vec![MS(500)]);
}

fn rename_harness() -> Harness {
    Harness::new(
        v1_config(vec![curve_group(5.0)], rgb_off(), 1000),
        vec![fan_hub("HUB0")],
        &[("cpu", 80.0)],
    )
}

/// Audit #8 (characterization): RenameDevice goes through
/// `ControlLoop::update_config`, which rebuilds every group with a fresh
/// acoustic filter, so a ramped-up group falls back to the start of its ramp.
///
/// Hand computation (cpu = 80 C -> raw 100 %, ramp-up 5 %/s, dt = 1.0 s):
/// tick k commands max(5k, 20): k=10 -> 50. The rename step's tick is #11:
/// without a reset it would be 55; with the reset (current_duty = 0) it is
/// 5 -> raised to the 20 % fan minimum.
#[test]
fn rename_device_resets_running_group_state() {
    let mut h = rename_harness();
    h.steps(10);
    assert_eq!(last_ch1_duty(&h, "HUB0"), 50);

    let (reply, _) = h.call(|reply| HwCommand::RenameDevice {
        device_id: "HUB0_FAN1".to_string(),
        name: "Top Front".to_string(),
        reply,
    });
    assert_eq!(reply, Ok(()));
    assert_eq!(h.store.saves().len(), 1);
    assert_eq!(
        last_ch1_duty(&h, "HUB0"),
        20,
        "duty collapses to the minimum after a rename (audit #8)"
    );
}

/// Audit #8 (desired): a rename must not disturb running cooling state.
#[test]
#[ignore = "bug regression: audit #8, fixed in PR 5"]
fn rename_device_preserves_running_group_state() {
    let mut h = rename_harness();
    h.steps(10);
    let (reply, _) = h.call(|reply| HwCommand::RenameDevice {
        device_id: "HUB0_FAN1".to_string(),
        name: "Top Front".to_string(),
        reply,
    });
    assert_eq!(reply, Ok(()));
    assert_eq!(last_ch1_duty(&h, "HUB0"), 55);
}

/// ApplyPreset with a failing store: warns, still applies live, replies Ok.
#[test]
fn apply_preset_with_failing_store_still_applies() {
    let mut h = fixed_harness();
    h.store.set_fail(true);
    let (reply, _) = h.call(|reply| HwCommand::ApplyPreset {
        preset: "silent".to_string(),
        group_name: None,
        reply,
    });
    assert_eq!(reply, Ok(()));
    assert_eq!(h.store.attempts(), 1);
    assert!(h.store.saves().is_empty());
    assert_eq!(
        h.worker.control_loop.config().fan_groups[0]
            .preset_applied
            .as_deref(),
        Some("silent")
    );
}

// ---------------------------------------------------------------------------
// Simple command round-trips
// ---------------------------------------------------------------------------

#[test]
fn list_sensors_get_devices_and_manual_duty_by_device_id() {
    let mut h = fixed_harness();

    let (sensors, _) = h.call(|reply| HwCommand::ListSensors { reply });
    assert_eq!(sensors.unwrap(), vec!["cpu".to_string()]);

    let (tree, _) = h.call(|reply| HwCommand::GetDevices { reply });
    assert!(tree.is_ok());

    // Manual duty for one resolvable and one unknown id: the manual write
    // happens during command handling, before the step's own tick.
    let before = h.hub("HUB0").speed_calls().len();
    let (res, _) = h.call(|reply| HwCommand::SetManualDutyByDeviceId {
        device_ids: vec!["HUB0_FAN1".to_string(), "NOPE".to_string()],
        duty: 70,
        reply,
    });
    let ManualDutyResult { applied, unresolved } = res.unwrap();
    assert_eq!(applied, vec!["HUB0_FAN1".to_string()]);
    assert_eq!(unresolved, vec!["NOPE".to_string()]);
    let calls = h.hub("HUB0").speed_calls();
    assert_eq!(calls[before], vec![(1, 70)]);
    assert_eq!(calls[before + 1], vec![(1, 50), (2, 50)]);
}

// ---------------------------------------------------------------------------
// Shutdown
// ---------------------------------------------------------------------------

fn two_hub_harness() -> Harness {
    Harness::new(
        v1_config(
            vec![fixed_group("g", "HUB0", &[1, 2], 50.0)],
            rgb_off(),
            1000,
        ),
        vec![fan_hub("HUB0"), fan_hub("HUB1")],
        &[("cpu", 40.0)],
    )
}

#[test]
fn shutdown_command_restores_hardware_mode_once_per_hub_and_exits() {
    let mut h = two_hub_harness();
    h.steps(1);
    let ticks_before = h.hub("HUB0").speed_calls().len();
    let sleeps_before = h.clock.sleeps().len();

    h.send(HwCommand::Shutdown);
    assert_eq!(h.step(), StepOutcome::Exit);

    assert_eq!(h.hub("HUB0").hardware_mode(), 1);
    assert_eq!(h.hub("HUB1").hardware_mode(), 1);
    // Exit is immediate: no control tick and no sleep after Shutdown.
    assert_eq!(h.hub("HUB0").speed_calls().len(), ticks_before);
    assert_eq!(h.clock.sleeps().len(), sleeps_before);
}

#[test]
fn disconnected_channel_restores_hardware_mode_and_exits() {
    let mut h = two_hub_harness();
    h.steps(1);
    h.disconnect();
    assert_eq!(h.step(), StepOutcome::Exit);
    assert_eq!(h.hub("HUB0").hardware_mode(), 1);
    assert_eq!(h.hub("HUB1").hardware_mode(), 1);
}

// ---------------------------------------------------------------------------
// Hub failure and recovery
// ---------------------------------------------------------------------------

/// First set_speeds failure emits `hub-lost` exactly once; recovery is
/// attempted immediately (recovery_attempts == 0) via the fake backend; a
/// failed attempt arms the (real-time, 10 s) backoff so later steps do not
/// retry and `hub-recovered` is never emitted.
///
/// Not covered deterministically: the second attempt after the backoff --
/// `ControlLoop` uses its own `Instant` for `last_recovery_attempt`, which
/// the fake clock cannot advance.
#[test]
fn hub_failure_emits_hub_lost_once_and_attempts_recovery_once() {
    let mut h = fixed_harness();
    h.hub("HUB0").set_fail(true);
    h.steps(3);

    assert_eq!(h.events.payloads("hub-lost"), vec!["\"HUB0\"".to_string()]);
    assert_eq!(h.backend.reopen_calls(), vec!["HUB0".to_string()]);
    assert_eq!(h.events.count("hub-recovered"), 0);
    assert_eq!(h.hub("HUB0").speed_calls().len(), 3, "still retried every tick");

    // Health surfaces in the snapshot: 3 consecutive failures by step 3.
    let last = h.events.payloads("sensor-update").pop().unwrap();
    let hh = &json(&last)["hub_health"][0];
    assert_eq!(hh["healthy"], false);
    assert_eq!(hh["consecutive_failures"], 3);
}

#[test]
fn hub_recovery_success_swaps_transport_and_emits_hub_recovered() {
    let mut h = fixed_harness();
    let replacement = RecordingHub::new();
    let info = hub_info_for(&fan_hub("HUB0"));
    h.backend
        .push_reopen_ok(replacement.clone() as Arc<dyn corsair_hid::IcueLinkTransport>, info);
    h.hub("HUB0").set_fail(true);

    h.steps(1); // fails, then recovers in the same step
    assert_eq!(h.events.payloads("hub-lost"), vec!["\"HUB0\"".to_string()]);
    assert_eq!(h.events.payloads("hub-recovered"), vec!["\"HUB0\"".to_string()]);
    assert_eq!(h.backend.reopen_calls(), vec!["HUB0".to_string()]);
    assert!(replacement.speed_calls().is_empty());

    h.steps(2); // now driven through the replacement transport
    assert_eq!(replacement.speed_calls().len(), 2);
    assert_eq!(replacement.speed_calls()[0], vec![(1, 50), (2, 50)]);
    assert_eq!(h.hub("HUB0").speed_calls().len(), 1, "old transport abandoned");
    assert_eq!(h.events.count("hub-lost"), 1);
}

// ---------------------------------------------------------------------------
// PSU cache (Telemetry seam)
// ---------------------------------------------------------------------------

fn good_psu() -> PsuStatus {
    let rail = |v| RailReading { voltage: v, current: 1.0, power: v };
    PsuStatus {
        temp_vrm: 40.0,
        temp_case: 35.0,
        fan_rpm: 800,
        input_voltage: 230.0,
        rail_12v: rail(12.0),
        rail_5v: rail(5.0),
        rail_3v3: rail(3.3),
        total_power: 300.0,
    }
}

fn last_psu_is_present(h: &Harness) -> bool {
    !json(&h.events.payloads("sensor-update").pop().unwrap())["psu"].is_null()
}

/// A failing HID read keeps serving the cached reading for PSU_STALE_LIMIT
/// (10) consecutive failures, then drops it; an invalid LHM reading is
/// rejected by validation and falls through to HID.
#[test]
fn psu_cache_is_served_for_ten_failures_then_dropped() {
    let mut h = fixed_harness();
    {
        let mut t = h.telemetry.0.lock().unwrap();
        let mut bad = good_psu();
        bad.temp_vrm = 500.0; // out of the -10..150 plausible range
        t.psu_lhm = Some(bad);
        t.psu_hid = Some(Ok(good_psu()));
    }
    h.steps(1);
    assert!(last_psu_is_present(&h), "invalid LHM rejected, HID used");
    assert_eq!(h.worker.psu_fail_count, 0);
    assert!(h.worker.last_good_psu.is_some());

    h.telemetry.0.lock().unwrap().psu_lhm = None;
    h.telemetry.0.lock().unwrap().psu_hid = Some(Err("usb gone".to_string()));
    for n in 1..=10 {
        h.steps(1);
        assert_eq!(h.worker.psu_fail_count, n);
        assert!(last_psu_is_present(&h), "cached reading served at failure {n}");
    }
    h.steps(1);
    assert_eq!(h.worker.psu_fail_count, 11);
    assert!(!last_psu_is_present(&h), "cache dropped after 10 failures");
}

// ---------------------------------------------------------------------------
// RGB end-to-end bytes
// ---------------------------------------------------------------------------

fn static_zone(channels: &[u8], color: Rgb) -> RgbZoneConfig {
    RgbZoneConfig {
        name: "z".to_string(),
        devices: channels
            .iter()
            .map(|&channel| RgbDeviceRef {
                hub_serial: "HUB0".to_string(),
                channel,
                device_id: String::new(),
            })
            .collect(),
        layers: vec![LayerConfig {
            effect: EffectConfig::Static { color },
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            enabled: true,
        }],
        brightness: 100,
        flow: None,
    }
}

/// `devices`: (channel, type, led_count in the hub's LED table). Zone covers
/// every listed channel with a static colour, master/zone brightness 100.
fn rgb_harness(devices: &[(u8, LinkDeviceType, u16)], color: Rgb) -> Harness {
    let spec = HubSpec {
        serial: "HUB0",
        devices: devices
            .iter()
            .map(|(ch, ty, _)| device(*ch, &format!("DEV{ch}"), ty.clone()))
            .collect(),
        led_counts: devices.iter().map(|(ch, _, n)| (*ch, *n)).collect(),
        pump_channels: Vec::new(),
    };
    let channels: Vec<u8> = devices.iter().map(|(ch, _, _)| *ch).collect();
    let rgb = RgbConfig {
        enabled: true,
        brightness: 100,
        fps: 30,
        hardware_output: true,
        zones: vec![static_zone(&channels, color)],
        presets: Vec::new(),
    };
    Harness::new(v1_config(Vec::new(), rgb, 1000), vec![spec], &[("cpu", 40.0)])
}

/// Render once (the first step at t=0 is not yet due, so jump 100 ms first)
/// and return the single set_rgb call the hub received.
fn render_once(h: &mut Harness) -> Vec<(u8, Vec<[u8; 3]>)> {
    h.clock.advance(MS(100));
    h.steps(1);
    let mut calls = h.hub("HUB0").rgb_calls();
    assert_eq!(calls.len(), 1, "exactly one RGB frame written");
    calls.remove(0)
}

/// Total LEDs 2 x 34 = 68 <= 238 -> no power scaling; bytes pass through.
/// Port power table (read from `port_power_factor`): <=238 -> 1.0,
/// 239..=340 -> 0.66, 341..=442 -> 0.33, >442 -> 0.10.
#[test]
fn rgb_bytes_unscaled_under_power_threshold() {
    let mut h = rgb_harness(
        &[(1, LinkDeviceType::QxFan, 34), (2, LinkDeviceType::QxFan, 34)],
        Rgb::new(200, 100, 50),
    );
    let call = render_once(&mut h);
    assert_eq!(
        call,
        vec![(1, vec![[200, 100, 50]; 34]), (2, vec![[200, 100, 50]; 34])]
    );
}

/// Total 150 + 150 = 300 LEDs -> factor 0.66 (f32 0.66 = 0.6600000262).
/// 200*0.66 = 132.000005 -> 132; 100*0.66 = 66.0000026 -> 66;
/// 50*0.66 = 33.0000013 -> 33.
#[test]
fn rgb_bytes_scaled_by_066_for_300_leds() {
    let mut h = rgb_harness(
        &[
            (1, LinkDeviceType::LinkAdapter, 150),
            (2, LinkDeviceType::LinkAdapter, 150),
        ],
        Rgb::new(200, 100, 50),
    );
    let call = render_once(&mut h);
    assert_eq!(
        call,
        vec![(1, vec![[132, 66, 33]; 150]), (2, vec![[132, 66, 33]; 150])]
    );
}

/// Total 200 + 200 = 400 LEDs -> factor 0.33 (f32 0.3300000131).
/// 200*0.33 = 66.0000026 -> 66; 100*0.33 = 33.0000013 -> 33;
/// 60*0.33 = 19.8000008 -> 20.
#[test]
fn rgb_bytes_scaled_by_033_for_400_leds() {
    let mut h = rgb_harness(
        &[
            (1, LinkDeviceType::LinkAdapter, 200),
            (2, LinkDeviceType::LinkAdapter, 200),
        ],
        Rgb::new(200, 100, 60),
    );
    let call = render_once(&mut h);
    assert_eq!(
        call,
        vec![(1, vec![[66, 33, 20]; 200]), (2, vec![[66, 33, 20]; 200])]
    );
}

/// Total 150 + 150 + 200 = 500 LEDs -> factor 0.10 (f32 0.1 = 0.1000000015).
/// 200*0.1 = 20.0000003 -> 20; 100*0.1 = 10.0000001 -> 10;
/// 50*0.1 = 5.0000001 -> 5.
#[test]
fn rgb_bytes_scaled_by_010_for_500_leds() {
    let mut h = rgb_harness(
        &[
            (1, LinkDeviceType::LinkAdapter, 150),
            (2, LinkDeviceType::LinkAdapter, 150),
            (3, LinkDeviceType::LinkAdapter, 200),
        ],
        Rgb::new(200, 100, 50),
    );
    let call = render_once(&mut h);
    assert_eq!(
        call,
        vec![
            (1, vec![[20, 10, 5]; 150]),
            (2, vec![[20, 10, 5]; 150]),
            (3, vec![[20, 10, 5]; 200]),
        ]
    );
}

/// hardware_output off: frames are previewed (`rgb-frame`) but nothing is
/// written to the hub.
#[test]
fn rgb_preview_only_does_not_write_hub() {
    let mut h = rgb_harness(&[(1, LinkDeviceType::QxFan, 34)], Rgb::new(1, 2, 3));
    h.worker.config.rgb.hardware_output = false;
    h.clock.advance(MS(100));
    h.steps(1);
    assert_eq!(h.events.count("rgb-frame"), 1);
    assert!(h.hub("HUB0").rgb_calls().is_empty());
}

// ---------------------------------------------------------------------------
// Identify mode
// ---------------------------------------------------------------------------

/// RgbIdentifyChannels recolors each device for the clamped duration, then
/// reverts once the (fake) deadline passes.
///
/// Palette (hardware_thread::IDENTIFY_PALETTE): slot = (hub_idx*8 + channel)
/// % 16 with hub_idx = 0 -> ch1 = slot 1 = orange [255,128,0], ch2 = slot 2 =
/// yellow [255,255,0]. 2 x 34 = 68 LEDs, so no power scaling.
/// The command is handled at t = 100 ms with duration 5 s (inside the 3..120
/// clamp) -> deadline t = 5100 ms.
#[test]
fn identify_mode_recolors_then_reverts_after_deadline() {
    let mut h = rgb_harness(
        &[(1, LinkDeviceType::QxFan, 34), (2, LinkDeviceType::QxFan, 34)],
        Rgb::new(200, 100, 50),
    );
    let orange = [255u8, 128, 0];
    let yellow = [255u8, 255, 0];
    let identify = vec![(1u8, vec![orange; 34]), (2u8, vec![yellow; 34])];
    let normal = vec![(1u8, vec![[200u8, 100, 50]; 34]), (2u8, vec![[200u8, 100, 50]; 34])];

    h.clock.advance(MS(100));
    let (reply, _) = h.call(|reply| HwCommand::RgbIdentifyChannels {
        duration_secs: 5,
        reply,
    });
    let rows = reply.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].channel, rows[0].color_name.as_str(), rows[0].rgb), (1, "orange", orange));
    assert_eq!((rows[1].channel, rows[1].color_name.as_str(), rows[1].rgb), (2, "yellow", yellow));
    // Same step rendered the override.
    assert_eq!(h.hub("HUB0").rgb_calls().last().unwrap(), &identify);

    // Just before the deadline (5099 ms < 5100 ms): still identify colours.
    assert_eq!(h.clock.elapsed(), MS(133));
    h.clock.advance(MS(5099) - h.clock.elapsed());
    h.steps(1);
    assert_eq!(h.hub("HUB0").rgb_calls().last().unwrap(), &identify);

    // The step slept 33 ms -> t = 5132 ms >= deadline: configured colours.
    h.steps(1);
    assert_eq!(h.hub("HUB0").rgb_calls().last().unwrap(), &normal);
    assert!(h.worker.rgb_identify.is_none());
}
