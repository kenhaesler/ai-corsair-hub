import { describe, expect, it } from 'vitest';
import { computePresetState, groupMatchesPreset } from './presets';
import type { AppConfig, FanGroupConfig, FanPresetDef } from './types';

const preset: FanPresetDef = {
  id: 'quiet',
  name: 'Quiet',
  description: 'Quiet curve',
  points: [
    { temp: 30, duty: 25 },
    { temp: 60, duty: 60 },
  ],
  hysteresis: 2,
  ramp_up: 8,
  ramp_down: 3,
  cpu_weight: 1,
  gpu_weight: 0,
  water_weight: 0,
};

function group(name: string, tagged = true): FanGroupConfig {
  return {
    name,
    device_ids: [],
    mode: {
      type: 'curve',
      points: structuredClone(preset.points),
      hysteresis: preset.hysteresis,
      ramp_rate: preset.ramp_up,
      ramp_down_rate: preset.ramp_down,
      temp_source: { sensors: ['cpu'], weights: [1] },
    },
    preset_applied: tagged ? preset.id : null,
  };
}

function config(groups: FanGroupConfig[]): AppConfig {
  return {
    general: { poll_interval_ms: 1000, log_level: 'info', lhm_exe_path: null },
    fan_groups: groups,
    rgb: {
      enabled: false,
      brightness: 80,
      fps: 30,
      hardware_output: false,
      zones: [],
      presets: [],
    },
  };
}

describe('preset state', () => {
  it('reports active only when every targeted group still matches', () => {
    expect(computePresetState(config([group('top'), group('side')]), preset)).toBe('active');
  });

  it('reports modified when a tagged group drifts from canonical values', () => {
    const changed = group('top');
    if (changed.mode.type === 'curve') changed.mode.points[0].duty = 40;

    expect(groupMatchesPreset(changed, preset)).toBe(false);
    expect(computePresetState(config([changed]), preset)).toBe('modified');
  });

  it('reports off for untagged groups and respects group scope', () => {
    const groups = [group('top'), group('side', false)];
    expect(computePresetState(config(groups), preset)).toBe('modified');
    expect(computePresetState(config(groups), preset, 'side')).toBe('off');
    expect(computePresetState(config(groups), preset, 'missing')).toBe('off');
  });
});
