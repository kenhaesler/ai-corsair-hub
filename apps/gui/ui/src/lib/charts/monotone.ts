// Chart math: linear scales, nice ticks, and monotone cubic interpolation
// (Fritsch–Carlson) so lines never overshoot their data, the way Stocks and
// Health draw them.

export type Pt = [number, number];

export interface Scale {
  (v: number): number;
  domain: [number, number];
  range: [number, number];
  invert(px: number): number;
}

export function linear(domain: [number, number], range: [number, number]): Scale {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  const k = d1 === d0 ? 0 : (r1 - r0) / (d1 - d0);
  const f = ((v: number) => r0 + (v - d0) * k) as Scale;
  f.domain = domain;
  f.range = range;
  f.invert = (px: number) => (k === 0 ? d0 : d0 + (px - r0) / k);
  return f;
}

/** "Nice" step for roughly `count` ticks across [min,max]. */
export function niceStep(min: number, max: number, count = 5): number {
  const span = Math.max(1e-9, max - min);
  const raw = span / Math.max(1, count);
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  const norm = raw / mag;
  const nice = norm < 1.5 ? 1 : norm < 3 ? 2 : norm < 7 ? 5 : 10;
  return nice * mag;
}

export function niceTicks(min: number, max: number, count = 5): number[] {
  const step = niceStep(min, max, count);
  const start = Math.ceil(min / step) * step;
  const out: number[] = [];
  for (let v = start; v <= max + step * 1e-6; v += step) out.push(Math.round(v / step) * step);
  return out;
}

/** Expand [min,max] outward to nice bounds with some padding. */
export function niceDomain(min: number, max: number, count = 4, pad = 0.12): [number, number] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, 1];
  if (max - min < 1e-6) {
    min -= 1;
    max += 1;
  }
  const p = (max - min) * pad;
  const step = niceStep(min - p, max + p, count);
  return [Math.floor((min - p) / step) * step, Math.ceil((max + p) / step) * step];
}

/** Fritsch–Carlson tangents for monotone cubic Hermite interpolation. */
function tangents(pts: Pt[]): number[] {
  const n = pts.length;
  const m = new Array<number>(n).fill(0);
  if (n < 2) return m;
  const d = new Array<number>(n - 1);
  for (let i = 0; i < n - 1; i++) {
    const dx = pts[i + 1][0] - pts[i][0];
    d[i] = dx === 0 ? 0 : (pts[i + 1][1] - pts[i][1]) / dx;
  }
  m[0] = d[0];
  m[n - 1] = d[n - 2];
  for (let i = 1; i < n - 1; i++) {
    m[i] = d[i - 1] * d[i] <= 0 ? 0 : (d[i - 1] + d[i]) / 2;
  }
  for (let i = 0; i < n - 1; i++) {
    if (d[i] === 0) {
      m[i] = 0;
      m[i + 1] = 0;
      continue;
    }
    const a = m[i] / d[i];
    const b = m[i + 1] / d[i];
    const s = a * a + b * b;
    if (s > 9) {
      const t = 3 / Math.sqrt(s);
      m[i] = t * a * d[i];
      m[i + 1] = t * b * d[i];
    }
  }
  return m;
}

const r2 = (v: number) => Math.round(v * 100) / 100;

/** SVG path through pixel points using monotone cubic segments. */
export function monotonePath(pts: Pt[]): string {
  const n = pts.length;
  if (n === 0) return '';
  if (n === 1) return `M${r2(pts[0][0])},${r2(pts[0][1])}`;
  if (n === 2) return `M${r2(pts[0][0])},${r2(pts[0][1])}L${r2(pts[1][0])},${r2(pts[1][1])}`;
  const m = tangents(pts);
  let d = `M${r2(pts[0][0])},${r2(pts[0][1])}`;
  for (let i = 0; i < n - 1; i++) {
    const [x0, y0] = pts[i];
    const [x1, y1] = pts[i + 1];
    const h = (x1 - x0) / 3;
    d += `C${r2(x0 + h)},${r2(y0 + m[i] * h)},${r2(x1 - h)},${r2(y1 - m[i + 1] * h)},${r2(x1)},${r2(y1)}`;
  }
  return d;
}

/** Closed area path under a monotone line down to `baseY`. */
export function monotoneArea(pts: Pt[], baseY: number): string {
  if (pts.length < 2) return '';
  const line = monotonePath(pts);
  const last = pts[pts.length - 1];
  const first = pts[0];
  return `${line}L${r2(last[0])},${r2(baseY)}L${r2(first[0])},${r2(baseY)}Z`;
}

/** Straight-segment path (curve editors, preset thumbnails). */
export function linePath(pts: Pt[]): string {
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'}${r2(p[0])},${r2(p[1])}`).join('');
}

/** Index of the point nearest to x (points sorted by x). */
export function nearestIndex(xs: number[], x: number): number {
  if (xs.length === 0) return -1;
  let lo = 0,
    hi = xs.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (xs[mid] < x) lo = mid + 1;
    else hi = mid;
  }
  if (lo > 0 && Math.abs(xs[lo - 1] - x) <= Math.abs(xs[lo] - x)) return lo - 1;
  return lo;
}

/** Simple stats for annotations. */
export function stats(values: number[]): { min: number; max: number; avg: number } | null {
  if (values.length === 0) return null;
  let min = Infinity,
    max = -Infinity,
    sum = 0;
  for (const v of values) {
    if (v < min) min = v;
    if (v > max) max = v;
    sum += v;
  }
  return { min, max, avg: sum / values.length };
}
