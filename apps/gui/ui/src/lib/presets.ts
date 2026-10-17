// Shared preset-state derivation. Kept separate from PresetStrip.svelte
// so the New-Group dialog and other components that want to show an
// "Active / Modified / Off" tag on a preset can reuse the same logic.

import type { AppConfig, FanGroupConfig, FanPresetDef } from './types';

export type PresetState = 'active' | 'modified' | 'off';

/**
 * Derive the three-state indicator for a single preset against the
 * current config.
 *
 * - `active`: every targeted group has `preset_applied === preset.id`.
 *   The backend sets this tag on apply and clears it on drift, so
 *   `active` means "the last thing applied and nothing has edited
 *   since".
 * - `modified`: at least one targeted group still carries
 *   `preset_applied === preset.id` (so the preset was the last to be
 *   applied) but the structural comparison to canonical values shows
 *   drift. This is the "user tweaked the curve" state.
 * - `off`: no targeted group has this preset tagged — something else
 *   is active, or the group was just reshaped.
 *
 * When `groupName` is null/undefined the function considers every
 * group; when set, only the named group is considered (matching the
 * `apply_preset_to_group` command scope).
 */
export function computePresetState(
  config: AppConfig | null | undefined,
  preset: FanPresetDef,
  groupName: string | null = null,
): PresetState {
  const groups = config?.fan_groups ?? [];
  const targeted = groupName
    ? groups.filter((g) => g.name === groupName)
    : groups;

  if (targeted.length === 0) return 'off';

  // A group is "active" for this preset if its tag matches AND a
  // structural comparison to canonical values passes. Modified ==
  // tag matches but values drifted.
  let anyTagged = false;
  let allActive = true;

  for (const group of targeted) {
    if (group.preset_applied === preset.id) {
      anyTagged = true;
      if (!groupMatchesPreset(group, preset)) {
        allActive = false;
      }
    } else {
      allActive = false;
    }
  }

  if (!anyTagged) return 'off';
  return allActive ? 'active' : 'modified';
}

function almostEqual(a: number, b: number, eps = 0.001): boolean {
  return Math.abs(a - b) < eps;
}

/**
 * Structural compare of a group's mode to a preset's canonical values.
 * Mirrors `reconcile_preset_applied` in the Rust side — the two must
 * agree for the Active/Modified distinction to make sense.
 *
 * Only Curve mode can match a curve-valued preset. Sensor weighting
 * deliberately isn't part of the comparison: a machine with no GPU
 * resolves a preset's 0.6/0.4 to 1.0 CPU at apply-time, and that's
 * not considered drift.
 */
export function groupMatchesPreset(
  group: FanGroupConfig,
  preset: FanPresetDef,
): boolean {
  if (group.mode.type !== 'curve') return false;
  const { points, hysteresis, ramp_rate, ramp_down_rate } = group.mode;
  if (points.length !== preset.points.length) return false;
  for (let i = 0; i < points.length; i++) {
    if (!almostEqual(points[i].temp, preset.points[i].temp)) return false;
    if (!almostEqual(points[i].duty, preset.points[i].duty)) return false;
  }
  if (!almostEqual(hysteresis, preset.hysteresis)) return false;
  if (!almostEqual(ramp_rate, preset.ramp_up)) return false;
  // ramp_down_rate is a v0.3 addition; only match if explicitly set.
  if (ramp_down_rate == null) return false;
  if (!almostEqual(ramp_down_rate, preset.ramp_down)) return false;
  return true;
}
