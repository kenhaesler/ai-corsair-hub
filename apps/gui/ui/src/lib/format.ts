// Formatting, colour and labelling helpers shared by every screen.

import type { SystemSnapshot } from './types';

// ---------------------------------------------------------------------------
// Series colours (DESIGN.md "Series")
// ---------------------------------------------------------------------------
// Families share a hue: CPU blues, GPU greens, board warm tones, PSU
// yellow/red, coolant cyan. Unknown sources fall back to violet.
const SOURCE_COLORS: Record<string, string> = {
  cpu: '#5AA9FF',
  cpu_ccd: '#93C5FD',
  gpu: '#34D399',
  gpu_memory: '#A3E635',
  board_system: '#FBBF24',
  chipset: '#FB923C',
  vrm: '#FB7185',
  dimm: '#E879F9',
  psu_vrm: '#FDE047',
  psu_case: '#F87171',
};
export const WATER_COLOR = '#5EE1E6';
export const OTHER_COLOR = '#A78BFA';
export const POWER_COLOR = '#FFD60A';
export const GROUP_COLORS = ['#7DD3FC', '#C4B5FD', '#5EEAD4', '#F9A8D4', '#FDE68A'];

export function sourceColor(source: string): string {
  if (SOURCE_COLORS[source]) return SOURCE_COLORS[source];
  if (source.startsWith('water')) return WATER_COLOR;
  return OTHER_COLOR;
}
export function groupColor(index: number): string {
  return GROUP_COLORS[((index % GROUP_COLORS.length) + GROUP_COLORS.length) % GROUP_COLORS.length];
}

const SOURCE_LABEL: Record<string, string> = {
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
/** Display label of a temperature source, one spelling everywhere ("PSU case", "Water"). */
export function sourceLabel(source: string): string {
  return SOURCE_LABEL[source] ?? source.replace(/_/g, ' ').replace(/^\w/, (c) => c.toUpperCase());
}

// ---------------------------------------------------------------------------
// Thermal ramp (inventory 1.2 interpolation; DESIGN.md colours)
//   <=40 cold blue -> 60 warm -> 80 hot -> >=80 red (fast hand-off by 84)
// ---------------------------------------------------------------------------
// Interpolated in OKLCH along the LONG hue path through violet/magenta, so
// the cool -> warm ramp never passes through green (which would collide with
// the GPU series colour). Anchors: <=40 ice blue, 50 violet, 60 amber,
// 80 orange-red, >=84 hot red.
type LCH = [number, number, number]; // L 0..1, C, H degrees (unwrapped)
const STOPS: [number, LCH][] = [
  [40, [0.74, 0.13, 248]], // ice blue
  [50, [0.7, 0.14, 302]], // violet
  [60, [0.83, 0.14, 440]], // amber (80 deg, unwrapped past 360 for the long path)
  [80, [0.7, 0.19, 400]], // orange-red (40 deg)
  [84, [0.65, 0.22, 387]], // hot red (27 deg)
];

function oklchToRgb([L, C, H]: LCH): [number, number, number] {
  const h = (H * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ ** 3,
    m = m_ ** 3,
    s = s_ ** 3;
  const lin = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  return lin.map((v) => {
    const c = Math.min(1, Math.max(0, v));
    const g = c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;
    return Math.round(g * 255);
  }) as [number, number, number];
}

export function thermalLch(c: number): LCH {
  if (c <= STOPS[0][0]) return STOPS[0][1];
  for (let i = 0; i < STOPS.length - 1; i++) {
    const [t0, a] = STOPS[i];
    const [t1, b] = STOPS[i + 1];
    if (c <= t1) {
      const t = (c - t0) / (t1 - t0);
      return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    }
  }
  return STOPS[STOPS.length - 1][1];
}
export function thermalColor(c: number): string {
  const [r, g, b] = oklchToRgb(thermalLch(c));
  return `rgb(${r} ${g} ${b})`;
}
/** Near-white with a subtle thermal tint, for large figures (colour lives in bar and caption). */
export function thermalInk(c: number): string {
  const [, C, H] = thermalLch(c);
  const [r, g, b] = oklchToRgb([0.95, Math.min(0.035, C * 0.25), H]);
  return `rgb(${r} ${g} ${b})`;
}

/** Status dot colour (inventory 1.7): red >85, amber >70, else green. */
export function statusLevel(maxTemp: number): 'error' | 'warn' | 'ok' {
  return maxTemp > 85 ? 'error' : maxTemp > 70 ? 'warn' : 'ok';
}

// ---------------------------------------------------------------------------
// Snapshot helpers
// ---------------------------------------------------------------------------
export function tempOf(s: SystemSnapshot | null | undefined, source: string): number | null {
  const t = s?.temperatures.find((x) => x.source === source);
  return t ? t.celsius : null;
}
export function dutyOf(s: SystemSnapshot | null | undefined, group: string): number | null {
  const g = s?.group_duties.find((x) => x.name === group);
  return g ? g.duty_percent : null;
}
/** Mean RPM of the fans in a group (null when the group has none). */
export function groupRpm(s: SystemSnapshot | null | undefined, group: string): number | null {
  if (!s) return null;
  const fans = s.fans.filter((f) => f.group_name === group);
  if (fans.length === 0) return null;
  return fans.reduce((a, f) => a + f.rpm, 0) / fans.length;
}
export function maxTemp(s: SystemSnapshot | null | undefined): number {
  return s ? Math.max(0, ...s.temperatures.map((t) => t.celsius)) : 0;
}
export function meanDuty(s: SystemSnapshot | null | undefined): number {
  if (!s || s.group_duties.length === 0) return 0;
  return s.group_duties.reduce((a, g) => a + g.duty_percent, 0) / s.group_duties.length;
}

// ---------------------------------------------------------------------------
// Number / time formatting
// ---------------------------------------------------------------------------
export const fmt = (v: number, dp = 0) => (Number.isFinite(v) ? v.toFixed(dp) : '–');
export function fmtSigned(v: number, dp = 1): string {
  const s = Math.abs(v).toFixed(dp);
  if (Number(s) === 0) return `±${s}`;
  return v > 0 ? `+${s}` : `−${s}`;
}
/** "1:42" style, for the scrub pill offset. */
export function fmtOffset(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${String(r).padStart(2, '0')}`;
}
export function fmtClock(ms: number, withSeconds = true): string {
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  const ss = String(d.getSeconds()).padStart(2, '0');
  return withSeconds ? `${hh}:${mm}:${ss}` : `${hh}:${mm}`;
}
/** Byte sizes in B / KiB / MiB / GiB (Settings updater parity). */
export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KiB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MiB`;
  return `${(n / 1024 ** 3).toFixed(2)} GiB`;
}
