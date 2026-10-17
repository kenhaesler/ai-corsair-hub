//! Early numeric/shape validation for [`AppConfig`].
//!
//! Runs on every load and every persist path (via
//! `corsair_fancontrol::control_loop::validate_config`). It rejects values the
//! runtime cannot meaningfully act on (NaN/inf, out-of-range duties, absurd
//! LED counts) with a precise field path. It never clamps or replaces values.
//!
//! Deliberately lenient: a bound that rejects a config a real user has today
//! would force them into recovery mode. Fan-ownership rules live elsewhere.

use crate::config::{
    AppConfig, FanMode, LayerConfig, RgbZoneConfig, TempSourceConfig, v2::DeviceEntry,
};
use corsair_rgb::EffectConfig;

/// Largest accepted LED-count override. Explicit 0 is allowed.
pub const MAX_LED_COUNT: u16 = 255;
/// Highest accepted RGB frame rate (0 is allowed: runtime falls back to ~30 fps).
pub const MAX_RGB_FPS: u8 = 120;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{path}: {message}")]
pub struct ConfigValidationError {
    /// Field path, e.g. `fan_groups[2].mode.hysteresis`.
    pub path: String,
    pub message: String,
}

type Result<T> = std::result::Result<T, ConfigValidationError>;

fn err<T>(path: impl Into<String>, message: impl Into<String>) -> Result<T> {
    Err(ConfigValidationError {
        path: path.into(),
        message: message.into(),
    })
}

fn finite(path: &str, v: f64) -> Result<()> {
    if v.is_finite() {
        Ok(())
    } else {
        err(path, format!("must be a finite number, got {v}"))
    }
}

fn duty(path: &str, v: f64) -> Result<()> {
    finite(path, v)?;
    if !(0.0..=100.0).contains(&v) {
        return err(path, format!("must be within 0..=100, got {v}"));
    }
    Ok(())
}

/// Validate numeric sanity and shape bounds. Returns the first error found.
pub fn validate(config: &AppConfig) -> Result<()> {
    for (i, g) in config.fan_groups.iter().enumerate() {
        let base = format!("fan_groups[{i}].mode");
        match &g.mode {
            FanMode::Fixed { duty_percent } => duty(&format!("{base}.duty_percent"), *duty_percent)?,
            FanMode::Curve {
                points,
                hysteresis,
                ramp_rate,
                ramp_down_rate,
                temp_source,
            } => {
                for (j, p) in points.iter().enumerate() {
                    finite(&format!("{base}.points[{j}].temp"), p.temp)?;
                    duty(&format!("{base}.points[{j}].duty"), p.duty)?;
                }
                let p = format!("{base}.hysteresis");
                finite(&p, *hysteresis)?;
                if *hysteresis < 0.0 {
                    return err(p, format!("must be >= 0, got {hysteresis}"));
                }
                let p = format!("{base}.ramp_rate");
                finite(&p, *ramp_rate)?;
                if *ramp_rate <= 0.0 {
                    return err(p, format!("must be > 0, got {ramp_rate}"));
                }
                if let Some(r) = ramp_down_rate {
                    let p = format!("{base}.ramp_down_rate");
                    finite(&p, *r)?;
                    if *r <= 0.0 {
                        return err(p, format!("must be > 0, got {r}"));
                    }
                }
                temp_source_finite(&format!("{base}.temp_source"), temp_source)?;
            }
            FanMode::Pid {
                target_temp,
                kp,
                ki,
                kd,
                min_duty,
                max_duty,
                temp_source,
            } => {
                finite(&format!("{base}.target_temp"), *target_temp)?;
                finite(&format!("{base}.kp"), *kp)?;
                finite(&format!("{base}.ki"), *ki)?;
                finite(&format!("{base}.kd"), *kd)?;
                duty(&format!("{base}.min_duty"), *min_duty)?;
                duty(&format!("{base}.max_duty"), *max_duty)?;
                if min_duty > max_duty {
                    return err(
                        format!("{base}.min_duty"),
                        format!("must be <= max_duty ({max_duty}), got {min_duty}"),
                    );
                }
                temp_source_finite(&format!("{base}.temp_source"), temp_source)?;
            }
        }
    }

    let rgb = &config.rgb;
    if rgb.brightness > 100 {
        return err("rgb.brightness", format!("must be <= 100, got {}", rgb.brightness));
    }
    if rgb.fps > MAX_RGB_FPS {
        return err("rgb.fps", format!("must be <= {MAX_RGB_FPS}, got {}", rgb.fps));
    }
    zones("rgb.zones", &rgb.zones)?;
    for (i, preset) in rgb.presets.iter().enumerate() {
        zones(&format!("rgb.presets[{i}].zones"), &preset.zones)?;
    }

    for (i, o) in config.device_overrides.iter().enumerate() {
        led_count(&format!("device_overrides[{i}].led_count"), o.led_count)?;
    }
    for (i, d) in config.devices.iter().enumerate() {
        let DeviceEntry { led_count: lc, .. } = d;
        if let Some(n) = lc {
            led_count(&format!("devices[{i}].led_count"), *n)?;
        }
    }
    Ok(())
}

fn led_count(path: &str, n: u16) -> Result<()> {
    if n > MAX_LED_COUNT {
        return err(path, format!("must be <= {MAX_LED_COUNT}, got {n}"));
    }
    Ok(())
}

fn temp_source_finite(path: &str, ts: &TempSourceConfig) -> Result<()> {
    for (i, w) in ts.weights.iter().enumerate() {
        finite(&format!("{path}.weights[{i}]"), *w)?;
    }
    Ok(())
}

fn zones(base: &str, zones: &[RgbZoneConfig]) -> Result<()> {
    for (i, z) in zones.iter().enumerate() {
        let zp = format!("{base}[{i}]");
        if z.brightness > 100 {
            return err(format!("{zp}.brightness"), format!("must be <= 100, got {}", z.brightness));
        }
        if let Some(f) = &z.flow {
            let p = format!("{zp}.flow.delay_per_device_ms");
            finite(&p, f64::from(f.delay_per_device_ms))?;
            if f.delay_per_device_ms < 0.0 {
                return err(p, format!("must be >= 0, got {}", f.delay_per_device_ms));
            }
        }
        for (j, l) in z.layers.iter().enumerate() {
            layer(&format!("{zp}.layers[{j}]"), l)?;
        }
    }
    Ok(())
}

fn layer(base: &str, l: &LayerConfig) -> Result<()> {
    // Opacity is clamped to 0..=1 by the blender; only reject non-finite.
    finite(&format!("{base}.opacity"), f64::from(l.opacity))?;
    effect(&format!("{base}.effect"), &l.effect)
}

/// Exhaustive on purpose: a new `EffectConfig` variant must be classified here.
fn effect(base: &str, e: &EffectConfig) -> Result<()> {
    let f = |name: &str, v: f32| finite(&format!("{base}.{name}"), f64::from(v));
    match e {
        EffectConfig::Static { .. }
        | EffectConfig::DutyMeter { .. }
        | EffectConfig::GpuMeter { .. } => Ok(()),
        EffectConfig::Breathing { speed, .. } | EffectConfig::SpectrumShift { speed } => {
            f("speed", *speed)
        }
        EffectConfig::ColorCycle { speed, saturation } => {
            f("speed", *speed)?;
            f("saturation", *saturation)
        }
        EffectConfig::RainbowWave { speed, wavelength } => {
            f("speed", *speed)?;
            f("wavelength", *wavelength)
        }
        EffectConfig::Fire { intensity, speed } => {
            f("intensity", *intensity)?;
            f("speed", *speed)
        }
        EffectConfig::Aurora { speed, color_spread } => {
            f("speed", *speed)?;
            f("color_spread", *color_spread)
        }
        EffectConfig::Candle { flicker_speed, .. } => f("flicker_speed", *flicker_speed),
        EffectConfig::Starfield { density, speed } => {
            f("density", *density)?;
            f("speed", *speed)
        }
        EffectConfig::Rain { speed, density, .. } => {
            f("speed", *speed)?;
            f("density", *density)
        }
        EffectConfig::TemperatureMap { gradient, .. } => {
            for (i, (t, _)) in gradient.iter().enumerate() {
                finite(&format!("{base}.gradient[{i}].temp"), *t)?;
            }
            Ok(())
        }
        EffectConfig::ThermalPulse { min_temp, max_temp, .. } => {
            finite(&format!("{base}.min_temp"), *min_temp)?;
            finite(&format!("{base}.max_temp"), *max_temp)
        }
        EffectConfig::Gradient { speed, .. } => f("speed", *speed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        CurvePoint, DeviceOverride, FanGroupConfig, FanPresetLibrary, RgbPreset,
    };
    use corsair_rgb::{BlendMode, FlowConfig, FlowDirection, Rgb};

    const BASE: &str = "[general]\npoll_interval_ms = 1000\nlog_level = \"info\"\n";

    fn parse(extra: &str) -> AppConfig {
        toml::from_str(&format!("{BASE}{extra}")).expect("test TOML parses")
    }

    fn assert_rejects(cfg: &AppConfig, path: &str) {
        let e = validate(cfg).expect_err("should be rejected");
        assert_eq!(e.path, path, "unexpected error: {e}");
        assert!(e.to_string().starts_with(&format!("{path}: ")));
    }

    fn curve(extra: &str) -> AppConfig {
        parse(&format!(
            r#"
[[fan_groups]]
name = "g"
channels = [1]
hub_serial = "H"
[fan_groups.mode]
type = "curve"
points = [{{ temp = 30.0, duty = 30.0 }}, {{ temp = 70.0, duty = 90.0 }}]
{extra}
[fan_groups.mode.temp_source]
sensors = ["cpu"]
weights = [1.0]
"#
        ))
    }

    fn pid(min: &str, max: &str) -> AppConfig {
        parse(&format!(
            r#"
[[fan_groups]]
name = "g"
channels = [1]
hub_serial = "H"
[fan_groups.mode]
type = "pid"
target_temp = 60.0
kp = 1.0
ki = 0.1
kd = 0.0
min_duty = {min}
max_duty = {max}
[fan_groups.mode.temp_source]
sensors = ["cpu"]
weights = [1.0]
"#
        ))
    }

    fn zone_cfg(layer: LayerConfig, flow: Option<FlowConfig>) -> RgbZoneConfig {
        RgbZoneConfig {
            name: "z".into(),
            devices: vec![],
            layers: vec![layer],
            brightness: 100,
            flow,
        }
    }

    fn layer_cfg(effect: EffectConfig, opacity: f32) -> LayerConfig {
        LayerConfig {
            effect,
            blend_mode: BlendMode::Normal,
            opacity,
            enabled: true,
        }
    }

    #[test]
    fn default_config_is_valid() {
        validate(&AppConfig::default()).unwrap();
    }

    #[test]
    fn valid_curve_and_pid_pass() {
        validate(&curve("hysteresis = 3.0\nramp_rate = 5.0\nramp_down_rate = 2.0")).unwrap();
        validate(&pid("20.0", "100.0")).unwrap();
        validate(&pid("50.0", "50.0")).unwrap(); // equality is the existing check's job
    }

    #[test]
    fn toml_nan_and_inf_rejected() {
        assert_rejects(
            &curve("hysteresis = nan\nramp_rate = 5.0"),
            "fan_groups[0].mode.hysteresis",
        );
        assert_rejects(
            &curve("hysteresis = 1.0\nramp_rate = inf"),
            "fan_groups[0].mode.ramp_rate",
        );
        let cfg = parse(
            r#"
[[fan_groups]]
name = "g"
channels = [1]
hub_serial = "H"
[fan_groups.mode]
type = "fixed"
duty_percent = nan
"#,
        );
        assert_rejects(&cfg, "fan_groups[0].mode.duty_percent");
        let e = validate(&cfg).unwrap_err();
        assert!(e.message.contains("finite"), "{e}");
    }

    #[test]
    fn constructed_non_finite_values_rejected() {
        let mut cfg = curve("hysteresis = 1.0\nramp_rate = 5.0");
        if let FanMode::Curve { points, .. } = &mut cfg.fan_groups[0].mode {
            points[1].temp = f64::NAN;
            assert_rejects(&cfg, "fan_groups[0].mode.points[1].temp");
        } else {
            unreachable!()
        }
        let mut cfg = curve("hysteresis = 1.0\nramp_rate = 5.0");
        if let FanMode::Curve { temp_source, .. } = &mut cfg.fan_groups[0].mode {
            temp_source.weights[0] = f64::INFINITY;
        }
        assert_rejects(&cfg, "fan_groups[0].mode.temp_source.weights[0]");

        let mut cfg = pid("0.0", "100.0");
        if let FanMode::Pid { kd, .. } = &mut cfg.fan_groups[0].mode {
            *kd = f64::NEG_INFINITY;
        }
        assert_rejects(&cfg, "fan_groups[0].mode.kd");
    }

    #[test]
    fn pid_min_greater_than_max_rejected() {
        assert_rejects(&pid("80.0", "40.0"), "fan_groups[0].mode.min_duty");
    }

    #[test]
    fn hysteresis_and_ramp_bounds() {
        assert_rejects(
            &curve("hysteresis = -0.5\nramp_rate = 5.0"),
            "fan_groups[0].mode.hysteresis",
        );
        assert_rejects(
            &curve("hysteresis = 1.0\nramp_rate = 0.0"),
            "fan_groups[0].mode.ramp_rate",
        );
        assert_rejects(
            &curve("hysteresis = 1.0\nramp_rate = -3.0"),
            "fan_groups[0].mode.ramp_rate",
        );
        assert_rejects(
            &curve("hysteresis = 1.0\nramp_rate = 5.0\nramp_down_rate = 0.0"),
            "fan_groups[0].mode.ramp_down_rate",
        );
        assert_rejects(
            &curve("hysteresis = 1.0\nramp_rate = 5.0\nramp_down_rate = -1.0"),
            "fan_groups[0].mode.ramp_down_rate",
        );
    }

    #[test]
    fn duty_range_enforced() {
        assert_rejects(&pid("0.0", "101.0"), "fan_groups[0].mode.max_duty");
        assert_rejects(&pid("-1.0", "50.0"), "fan_groups[0].mode.min_duty");
        let mut cfg = curve("hysteresis = 1.0\nramp_rate = 5.0");
        if let FanMode::Curve { points, .. } = &mut cfg.fan_groups[0].mode {
            points[0] = CurvePoint { temp: 30.0, duty: 101.0 };
        }
        assert_rejects(&cfg, "fan_groups[0].mode.points[0].duty");
        let mut cfg = AppConfig::default();
        cfg.fan_groups.push(FanGroupConfig {
            name: "g".into(),
            channels: vec![1],
            hub_serial: Some("H".into()),
            device_ids: vec![],
            mode: FanMode::Fixed { duty_percent: -1.0 },
            preset_applied: None,
        });
        assert_rejects(&cfg, "fan_groups[0].mode.duty_percent");
    }

    #[test]
    fn rgb_bounds() {
        let mut cfg = AppConfig::default();
        cfg.rgb.brightness = 101;
        assert_rejects(&cfg, "rgb.brightness");
        let mut cfg = AppConfig::default();
        cfg.rgb.fps = 121;
        assert_rejects(&cfg, "rgb.fps");
        let mut cfg = AppConfig::default();
        cfg.rgb.fps = 0;
        cfg.rgb.brightness = 100;
        validate(&cfg).unwrap();

        let mut cfg = AppConfig::default();
        let mut z = zone_cfg(layer_cfg(EffectConfig::SpectrumShift { speed: 1.0 }, 1.0), None);
        z.brightness = 200;
        cfg.rgb.zones.push(z);
        assert_rejects(&cfg, "rgb.zones[0].brightness");
    }

    #[test]
    fn rgb_non_finite_values_rejected() {
        let flow = |d: f32| {
            Some(FlowConfig { delay_per_device_ms: d, direction: FlowDirection::Forward })
        };
        let ok_layer = || layer_cfg(EffectConfig::SpectrumShift { speed: 1.0 }, 1.0);

        let mut cfg = AppConfig::default();
        cfg.rgb.zones.push(zone_cfg(layer_cfg(EffectConfig::SpectrumShift { speed: 1.0 }, f32::NAN), None));
        assert_rejects(&cfg, "rgb.zones[0].layers[0].opacity");

        let mut cfg = AppConfig::default();
        cfg.rgb.zones.push(zone_cfg(ok_layer(), flow(f32::INFINITY)));
        assert_rejects(&cfg, "rgb.zones[0].flow.delay_per_device_ms");

        let mut cfg = AppConfig::default();
        cfg.rgb.zones.push(zone_cfg(ok_layer(), flow(-1.0)));
        assert_rejects(&cfg, "rgb.zones[0].flow.delay_per_device_ms");

        let mut cfg = AppConfig::default();
        cfg.rgb.zones.push(zone_cfg(
            layer_cfg(EffectConfig::Fire { intensity: 1.0, speed: f32::NAN }, 1.0),
            None,
        ));
        assert_rejects(&cfg, "rgb.zones[0].layers[0].effect.speed");

        let c = Rgb { r: 0, g: 0, b: 0 };
        let mut cfg = AppConfig::default();
        cfg.rgb.zones.push(zone_cfg(
            layer_cfg(
                EffectConfig::ThermalPulse { cold_color: c, hot_color: c, min_temp: 20.0, max_temp: f64::NAN },
                1.0,
            ),
            None,
        ));
        assert_rejects(&cfg, "rgb.zones[0].layers[0].effect.max_temp");

        // Same checks apply inside presets.
        let mut cfg = AppConfig::default();
        cfg.rgb.presets.push(RgbPreset {
            name: "p".into(),
            zones: vec![zone_cfg(
                layer_cfg(
                    EffectConfig::TemperatureMap { gradient: vec![(f64::NAN, c)], glow_on_spike: false },
                    1.0,
                ),
                None,
            )],
        });
        assert_rejects(&cfg, "rgb.presets[0].zones[0].layers[0].effect.gradient[0].temp");
    }

    #[test]
    fn led_count_bounds() {
        let mut cfg = AppConfig::default();
        cfg.device_overrides.push(DeviceOverride { hub_serial: "H".into(), channel: 15, led_count: 80 });
        cfg.device_overrides.push(DeviceOverride { hub_serial: "H".into(), channel: 3, led_count: 0 });
        cfg.devices.push(DeviceEntry { device_id: "A".into(), name: None, led_count: Some(0) });
        cfg.devices.push(DeviceEntry { device_id: "B".into(), name: None, led_count: Some(255) });
        validate(&cfg).unwrap();

        cfg.device_overrides[0].led_count = 256;
        assert_rejects(&cfg, "device_overrides[0].led_count");
        cfg.device_overrides[0].led_count = 80;
        cfg.devices[1].led_count = Some(256);
        assert_rejects(&cfg, "devices[1].led_count");

        let mut cfg = AppConfig::default();
        cfg.devices.push(DeviceEntry { device_id: "A".into(), name: None, led_count: Some(256) });
        assert_rejects(&cfg, "devices[0].led_count");
    }

    #[test]
    fn shipped_fixtures_pass() {
        for (name, src) in [
            ("apps/gui/config.toml", include_str!("../../../apps/gui/config.toml")),
            ("config.example.toml", include_str!("../../../config.example.toml")),
        ] {
            let cfg: AppConfig = toml::from_str(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            validate(&cfg).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn shipped_fan_presets_satisfy_rules() {
        let lib = FanPresetLibrary::parse(include_str!("../../../apps/gui/assets/presets/fans.toml"))
            .unwrap();
        assert!(!lib.presets.is_empty());
        for p in &lib.presets {
            assert!(p.hysteresis.is_finite() && p.hysteresis >= 0.0, "{}", p.id);
            assert!(p.ramp_up.is_finite() && p.ramp_up > 0.0, "{}", p.id);
            assert!(p.ramp_down.is_finite() && p.ramp_down > 0.0, "{}", p.id);
            for pt in &p.points {
                assert!(pt.temp.is_finite() && (0.0..=100.0).contains(&pt.duty), "{}", p.id);
            }
        }
    }
}
