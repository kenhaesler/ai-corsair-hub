//! Curated fan-preset library shipped as a compile-time asset.
//!
//! The preset definitions live in `assets/presets/fans.toml` and are
//! embedded into the binary via `include_str!`. Parse cost is paid
//! once (at first access), after which lookup is a linear scan over
//! a `Vec` of ~10 entries — effectively O(1) for UI purposes.
//!
//! Shipping this as data rather than Rust code lets the owner retune
//! curves by editing a TOML file and rebuilding — no Rust edit needed
//! per tweak — while keeping all the robustness of a single statically-
//! linked binary (no runtime "asset missing" failure mode).
//!
//! See `FanPresetLibrary` / `FanPresetDef` in `corsair_common::config`
//! for the schema.

use std::sync::LazyLock;

use corsair_common::config::{FanPresetDef, FanPresetLibrary};

/// Raw TOML asset embedded at compile time.
const FAN_PRESETS_TOML: &str = include_str!("../assets/presets/fans.toml");

/// Parsed preset library. Panics on malformed TOML — which can only
/// happen if someone edited the asset and forgot to run the tests.
static PRESET_LIBRARY: LazyLock<FanPresetLibrary> = LazyLock::new(|| {
    FanPresetLibrary::parse(FAN_PRESETS_TOML)
        .expect("bundled fans.toml must parse — edit the asset and re-run the tests")
});

/// Return the full curated preset list in TOML declaration order.
/// The UI's `get_fan_presets` command wraps this.
pub fn all() -> &'static [FanPresetDef] {
    &PRESET_LIBRARY.presets
}

/// Look up a preset by its stable `id`.
pub fn lookup(id: &str) -> Option<&'static FanPresetDef> {
    PRESET_LIBRARY.lookup(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped TOML must always parse — this is the canary for edits.
    #[test]
    fn bundled_asset_parses() {
        assert!(!all().is_empty(), "fans.toml must contain at least one preset");
    }

    /// Legacy preset ids must remain valid so saved UI state doesn't
    /// orphan after the v0.3 library expansion.
    #[test]
    fn legacy_ids_retained() {
        for id in ["silent", "balanced", "performance"] {
            assert!(
                lookup(id).is_some(),
                "legacy preset id '{}' must stay shippable for back-compat",
                id
            );
        }
    }

    /// Every preset must have a monotonic-non-decreasing-temp curve
    /// with at least 2 points. Catches TOML edit regressions before
    /// they reach the user.
    #[test]
    fn every_preset_has_sane_curve() {
        for preset in all() {
            assert!(
                preset.points.len() >= 2,
                "preset '{}' needs >= 2 curve points",
                preset.id
            );
            let mut last_temp = f64::NEG_INFINITY;
            for p in &preset.points {
                assert!(
                    p.temp > last_temp,
                    "preset '{}' has non-ascending temps at {}°C",
                    preset.id,
                    p.temp
                );
                assert!(
                    p.duty >= 0.0 && p.duty <= 100.0,
                    "preset '{}' has duty {} outside 0..100",
                    preset.id,
                    p.duty
                );
                last_temp = p.temp;
            }
        }
    }

    /// Ramp rates must be positive — a zero ramp would freeze the filter.
    #[test]
    fn every_preset_has_positive_ramps() {
        for preset in all() {
            assert!(
                preset.ramp_up > 0.0,
                "preset '{}' has non-positive ramp_up",
                preset.id
            );
            assert!(
                preset.ramp_down > 0.0,
                "preset '{}' has non-positive ramp_down",
                preset.id
            );
        }
    }

    /// Ids must be unique.
    #[test]
    fn preset_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for preset in all() {
            assert!(
                seen.insert(preset.id.as_str()),
                "duplicate preset id '{}'",
                preset.id
            );
        }
    }
}
