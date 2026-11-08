// Fan Control helpers: mode defaults (inventory 2.2 / 2.7), curve validation
// (2.3), sensor labels (2.5) and small curve math shared by the editors.

import type { CurvePoint, FanMode, TempSourceConfig } from '../../lib/types';

export type ModeType = FanMode['type'];

export const MODE_LABEL: Record<ModeType, string> = { fixed: 'Fixed', curve: 'Curve', pid: 'PID' };

export const MODE_OPTIONS: { value: ModeType; label: string }[] = [
  { value: 'fixed', label: 'Fixed' },
  { value: 'curve', label: 'Curve' },
  { value: 'pid', label: 'PID' },
];

export const MODE_BLURB: Record<ModeType, string> = {
  fixed: 'Holds one constant duty, whatever the temperature.',
  curve: 'Maps the sensor mix temperature to a duty along your curve.',
  pid: 'Drives duty to hold the sensor mix at a target temperature.',
};

/**
 * Defaults when switching mode (inventory 2.2). `rampDownNull` is used by
 * NewGroupDialog (2.7: curve `ramp_down_rate` null); the mode selector in
 * the editor leaves it absent, exactly like the real app.
 */
export function defaultMode(type: ModeType, rampDownNull = false): FanMode {
  switch (type) {
    case 'fixed':
      return { type: 'fixed', duty_percent: 50 };
    case 'curve': {
      const m: FanMode = {
        type: 'curve',
        points: [
          { temp: 30, duty: 25 },
          { temp: 50, duty: 40 },
          { temp: 70, duty: 70 },
          { temp: 85, duty: 100 },
        ],
        hysteresis: 3,
        ramp_rate: 5,
        temp_source: { sensors: ['cpu'], weights: [1] },
      };
      if (rampDownNull) m.ramp_down_rate = null;
      return m;
    }
    case 'pid':
      return {
        type: 'pid',
        target_temp: 65,
        kp: 2.5,
        ki: 0.15,
        kd: 1.0,
        min_duty: 25,
        max_duty: 100,
        temp_source: { sensors: ['cpu'], weights: [1] },
      };
  }
}

// ---------------------------------------------------------------------------
// Curve validation (inventory 2.3; first of each kind)
// ---------------------------------------------------------------------------
export interface CurveIssues {
  error: string | null;
  warn: string | null;
}

export function curveIssues(points: CurvePoint[]): CurveIssues {
  const sorted = [...points].sort((a, b) => a.temp - b.temp);
  let error: string | null = null;
  let warn: string | null = null;
  for (let i = 1; i < sorted.length; i++) {
    if (Math.abs(sorted[i].temp - sorted[i - 1].temp) < 0.001) {
      error = `Duplicate point at ${sorted[i].temp}°C — fan behavior would be ambiguous.`;
      break;
    }
  }
  for (let i = 1; i < sorted.length; i++) {
    if (sorted[i].duty < sorted[i - 1].duty) {
      warn = `Duty drops between ${sorted[i - 1].temp}°C and ${sorted[i].temp}°C — fans may oscillate.`;
      break;
    }
  }
  return { error, warn };
}

export function modeHasCurveError(mode: FanMode): boolean {
  return mode.type === 'curve' && curveIssues(mode.points).error !== null;
}

/** Linear interpolation along the curve, clamped to the end points (matches the control loop). */
export function interpDuty(points: CurvePoint[], t: number): number {
  if (points.length === 0) return 0;
  if (t <= points[0].temp) return points[0].duty;
  const last = points[points.length - 1];
  if (t >= last.temp) return last.duty;
  for (let i = 0; i < points.length - 1; i++) {
    const a = points[i];
    const b = points[i + 1];
    if (t >= a.temp && t <= b.temp) {
      const span = b.temp - a.temp;
      return span <= 0 ? b.duty : a.duty + ((b.duty - a.duty) * (t - a.temp)) / span;
    }
  }
  return last.duty;
}

// ---------------------------------------------------------------------------
// Sensors (inventory 2.5)
// ---------------------------------------------------------------------------
const FRIENDLY: Record<string, string> = {
  cpu: 'CPU',
  gpu: 'GPU',
  psu_vrm: 'PSU VRM',
  psu_case: 'PSU case',
  water_inline: 'Coolant',
  board_system: 'Board',
  chipset: 'Chipset',
  vrm: 'VRM',
  dimm: 'Memory',
  cpu_ccd: 'CPU cores',
  gpu_memory: 'GPU memory',
};
export function sensorLabel(id: string): string {
  return FRIENDLY[id] ?? id;
}

/** Weighted mean of the temps referenced by a temp source (null when none readable). */
export function mixTemp(src: TempSourceConfig, temps: { source: string; celsius: number }[] | undefined): number | null {
  if (!temps) return null;
  let sw = 0;
  let st = 0;
  src.sensors.forEach((id, i) => {
    const t = temps.find((x) => x.source === id);
    const w = src.weights[i] ?? 1;
    if (t && w > 0) {
      sw += w;
      st += w * t.celsius;
    }
  });
  return sw > 0 ? st / sw : null;
}

export const clampN = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
