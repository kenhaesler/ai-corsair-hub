// Overview ("technical luminous" Summary) model: headline + reason copy,
// per-group reason lines / status tags / operating envelopes, the heat colour
// ramp, and the control-path source list. Pure functions of the displayed
// snapshot and the config, so the whole page follows the time-scrub.

import type { AppConfig, FanGroupConfig, SystemSnapshot, TempSourceConfig } from '../../lib/types';
import { groupRpm, tempOf } from '../../lib/format';

/** Backend emergency thresholds (crates/fancontrol/src/control_loop.rs). */
export const CRITICAL_CPU = 95;
export const CRITICAL_GPU = 90;

// ---------------------------------------------------------------------------
// Heat colour: cool blue → mint → amber → red. Each value glows in its own
// temperature's colour; duty/power map onto the same ramp.
// ---------------------------------------------------------------------------
const HEAT: [number, [number, number, number]][] = [
  [38, [127, 180, 255]],
  [60, [143, 224, 200]],
  [72, [255, 195, 107]],
  [86, [255, 106, 85]],
];
export function heatColor(t: number): string {
  if (t <= HEAT[0][0]) return `rgb(${HEAT[0][1].join(' ')})`;
  for (let i = 1; i < HEAT.length; i++) {
    const [b, cb] = HEAT[i];
    if (t <= b) {
      const [a, ca] = HEAT[i - 1];
      const f = (t - a) / (b - a);
      return `rgb(${ca.map((v, j) => Math.round(v + (cb[j] - v) * f)).join(' ')})`;
    }
  }
  return `rgb(${HEAT[HEAT.length - 1][1].join(' ')})`;
}
export const dutyColor = (d: number) => heatColor(35 + d * 0.55);
export const powerColor = (w: number, max: number) => heatColor(35 + (w / max) * 60);

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------
const NAMES: Record<string, string> = { cpu: 'CPU', gpu: 'GPU', psu_vrm: 'PSU VRM', psu_case: 'PSU case' };
export const srcShort = (s: string) => NAMES[s] ?? s.replace(/_/g, ' ').replace(/^\w/, (c) => c.toUpperCase());
const LONG: Record<string, string> = { cpu: 'CPU package', gpu: 'GPU core' };
export const srcLong = (s: string) => LONG[s] ?? srcShort(s);

/** "63% GPU + 37% CPU", or just "CPU" for a single source. */
export function mixText(ts: TempSourceConfig): string {
  const total = ts.weights.reduce((a, b) => a + b, 0) || 1;
  if (ts.sensors.length <= 1) return srcShort(ts.sensors[0] ?? 'cpu');
  return ts.sensors
    .map((s, i) => ({ s, w: ts.weights[i] ?? 0 }))
    .sort((a, b) => b.w - a.w)
    .map(({ s, w }) => `${Math.round((w / total) * 100)}% ${srcShort(s)}`)
    .join(' + ');
}

/** Weighted source temperature the curve sees (null if no source reads). */
export function mixTemp(snap: SystemSnapshot, ts: TempSourceConfig): number | null {
  let sum = 0;
  let wsum = 0;
  ts.sensors.forEach((s, i) => {
    const t = tempOf(snap, s);
    if (t !== null) {
      sum += t * (ts.weights[i] ?? 0);
      wsum += ts.weights[i] ?? 0;
    }
  });
  return wsum > 0 ? sum / wsum : null;
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------
export const findGroup = (config: AppConfig | null, name: string) =>
  config?.fan_groups.find((g) => g.name === name);

/** The duty range this group can run in (its operating envelope). */
export function groupEnvelope(g: FanGroupConfig | undefined): [number, number] | null {
  if (!g) return null;
  const m = g.mode;
  if (m.type === 'fixed') return [Math.max(0, m.duty_percent - 2), Math.min(100, m.duty_percent + 2)];
  if (m.type === 'pid') return [m.min_duty, m.max_duty];
  if (!m.points.length) return null;
  const d = m.points.map((p) => p.duty);
  return [Math.min(...d), Math.max(...d)];
}

/** One line explaining why the group runs at its duty. */
export function groupWhy(g: FanGroupConfig | undefined, snap: SystemSnapshot, emergency: boolean): string {
  if (emergency) return 'Emergency override · full speed';
  if (!g) return 'Not in the configuration';
  const m = g.mode;
  if (m.type === 'fixed') return 'Fixed duty · ignores temperature';
  if (m.type === 'pid') {
    const src = srcShort(m.temp_source.sensors[0] ?? 'cpu');
    return `Holding ${src} at ${m.target_temp}° · PID`;
  }
  const t = mixTemp(snap, m.temp_source);
  return `${mixText(m.temp_source)} → ${t === null ? '–' : t.toFixed(1)}° · on its curve`;
}

/** Short status tag for the control path. */
export function groupTag(g: FanGroupConfig | undefined, trend: number, emergency: boolean): string {
  if (emergency) return 'Override';
  if (!g) return 'Unknown';
  if (g.mode.type === 'fixed') return 'Fixed';
  if (g.mode.type === 'pid') return trend > 2 ? 'Pushing' : 'Holding';
  return trend > 2 ? 'Climbing' : trend < -2 ? 'Easing' : 'On curve';
}

// ---------------------------------------------------------------------------
// Control path sources: every sensor a group listens to (CPU, GPU first).
// ---------------------------------------------------------------------------
export function pathSources(config: AppConfig | null, snap: SystemSnapshot): string[] {
  const set = new Set<string>();
  for (const g of config?.fan_groups ?? []) if (g.mode.type !== 'fixed') g.mode.temp_source.sensors.forEach((s) => set.add(s));
  if (set.size === 0) snap.temperatures.forEach((t) => set.add(t.source));
  const rank = (s: string) => (s === 'cpu' ? 0 : s === 'gpu' ? 1 : s.startsWith('water') ? 2 : 3);
  return [...set].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
}

// ---------------------------------------------------------------------------
// Headline + reason
// ---------------------------------------------------------------------------
export interface Headline {
  lead: string;
  em: string;
  tail: string;
}

export function headroom(snap: SystemSnapshot): { value: number; part: string } | null {
  const cpu = tempOf(snap, 'cpu');
  const gpu = tempOf(snap, 'gpu');
  const opts: { value: number; part: string }[] = [];
  if (cpu !== null) opts.push({ value: CRITICAL_CPU - cpu, part: 'CPU' });
  if (gpu !== null) opts.push({ value: CRITICAL_GPU - gpu, part: 'GPU' });
  if (!opts.length) return null;
  return opts.sort((a, b) => a.value - b.value)[0];
}

export function headline(snap: SystemSnapshot, emergency: boolean, meanDuty: number): Headline {
  if (emergency) return { lead: 'Emergency override: ', em: 'every fan at full speed', tail: '.' };
  const hottest = [...snap.temperatures].sort((a, b) => b.celsius - a.celsius)[0];
  const maxT = hottest?.celsius ?? 0;
  if (maxT > 85) return { lead: `The ${srcShort(hottest.source)} is `, em: 'running hot', tail: '.' };
  if (maxT > 70) return { lead: 'Working hard, ', em: 'running warm', tail: '.' };
  if (meanDuty > 45 || maxT > 60) return { lead: 'Working hard, ', em: 'holding steady', tail: '.' };
  return { lead: 'Everything is ', em: 'quiet and cool', tail: '.' };
}

export interface Clause {
  text: string;
  strong?: boolean;
}

/** The reason paragraph: what each group is doing and why, in plain words. */
export function reason(snap: SystemSnapshot, config: AppConfig | null, emergency: boolean): Clause[] {
  if (emergency) {
    const hot = headroom(snap);
    return [
      { text: 'Emergency override. ', strong: true },
      {
        text: `${hot ? `The ${hot.part} reached its critical limit, so ` : ''}every fan group runs at 100% until temperatures fall again. Your curves resume automatically.`,
      },
    ];
  }
  const groups = config?.fan_groups ?? [];
  if (!groups.length) return [{ text: 'No fan groups yet. Create one in Fan Control to let Corsair Hub manage your fans.' }];
  const out: Clause[] = [];
  const shown = groups.slice(0, 3);
  shown.forEach((g, i) => {
    const sep = i === 0 ? '' : i === shown.length - 1 && groups.length <= 3 ? ', and ' : ', ';
    if (sep) out.push({ text: sep });
    out.push({ text: g.name, strong: true });
    const m = g.mode;
    if (m.type === 'fixed') out.push({ text: ` runs at a fixed ${m.duty_percent}%` });
    else if (m.type === 'pid') out.push({ text: ` holds the ${srcShort(m.temp_source.sensors[0] ?? 'cpu')} near ${m.target_temp}°` });
    else out.push({ text: ` follows ${mixText(m.temp_source).replace(/ \+ /g, ' and ')}` });
  });
  if (groups.length > 3) out.push({ text: `, and ${groups.length - 3} more group${groups.length - 3 > 1 ? 's' : ''} follow their own settings` });
  out.push({ text: '.' });
  const hr = headroom(snap);
  if (hr && hr.value < 12) out.push({ text: ` The ${hr.part} is ${hr.value.toFixed(0)}° from its emergency limit.` });
  return out;
}

/** Mean RPM across all fans that belong to a group. */
export function meanGroupRpm(snap: SystemSnapshot): number | null {
  const v = snap.group_duties.map((g) => groupRpm(snap, g.name)).filter((x): x is number => x !== null);
  return v.length ? v.reduce((a, b) => a + b, 0) / v.length : null;
}
