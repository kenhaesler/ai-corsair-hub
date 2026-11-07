// Highlight model for the Summary page: one per temperature source, one per
// fan group, one for PSU power. Each carries an accessor for chart series
// and a generated context sentence comparing "now" with recent history.

import Thermometer from 'lucide-svelte/icons/thermometer';
import Cpu from 'lucide-svelte/icons/cpu';
import Microchip from 'lucide-svelte/icons/microchip';
import Fan from 'lucide-svelte/icons/fan';
import Zap from 'lucide-svelte/icons/zap';
import type { IconComponent } from '../../lib/icon';
import type { AppConfig, SystemSnapshot } from '../../lib/types';
import {
  POWER_COLOR,
  dutyOf,
  groupColor,
  groupRpm,
  sourceColor,
  sourceLabel,
  tempOf,
  thermalColor,
} from '../../lib/format';
import { stats } from '../../lib/charts/monotone';

export type HighlightKind = 'temp' | 'group' | 'power';

export interface Highlight {
  key: string;
  kind: HighlightKind;
  id: string;
  /** caption label (category) */
  label: string;
  /** detail page title */
  title: string;
  color: string;
  icon: IconComponent;
  unit: string;
  /** unit for compact chart labels */
  chartUnit: string;
  decimals: number;
  pick: (s: SystemSnapshot) => number | null;
  /** bar range */
  barMax: number;
  barColor: (v: number) => string;
}

const sourceIcon = (src: string): IconComponent => (src === 'cpu' ? Cpu : src === 'gpu' ? Microchip : Thermometer);

export function buildHighlights(snap: SystemSnapshot | null): Highlight[] {
  if (!snap) return [];
  const out: Highlight[] = [];
  for (const t of snap.temperatures) {
    out.push({
      key: `temp:${t.source}`,
      kind: 'temp',
      id: t.source,
      label: sourceLabel(t.source),
      title: sourceLabel(t.source),
      color: sourceColor(t.source),
      icon: sourceIcon(t.source),
      unit: '°C',
      chartUnit: '°',
      decimals: 1,
      pick: (s) => tempOf(s, t.source),
      barMax: 100,
      barColor: thermalColor,
    });
  }
  snap.group_duties.forEach((g, i) => {
    const color = groupColor(i);
    out.push({
      key: `group:${g.name}`,
      kind: 'group',
      id: g.name,
      label: g.name,
      title: g.name,
      color,
      icon: Fan,
      unit: '%',
      chartUnit: '%',
      decimals: 0,
      pick: (s) => dutyOf(s, g.name),
      barMax: 100,
      barColor: () => color,
    });
  });
  if (snap.psu) {
    out.push({
      key: 'power:psu',
      kind: 'power',
      id: 'psu',
      label: 'Total output',
      title: 'Power Draw',
      color: POWER_COLOR,
      icon: Zap,
      unit: 'W',
      chartUnit: ' W',
      decimals: 0,
      pick: (s) => s.psu?.total_power ?? null,
      barMax: 1500,
      barColor: () => POWER_COLOR,
    });
  }
  return out;
}

// ---------------------------------------------------------------------------
// Context sentences
// ---------------------------------------------------------------------------

/** Samples within [ts - seconds, ts] from a timeline (oldest first). */
export function windowBefore(timeline: SystemSnapshot[], ts: number, seconds: number): SystemSnapshot[] {
  const start = ts - seconds * 1000;
  return timeline.filter((s) => s.timestamp_ms >= start && s.timestamp_ms <= ts + 1);
}

function valueAgo(win: SystemSnapshot[], pick: (s: SystemSnapshot) => number | null, ts: number, seconds: number) {
  const target = ts - seconds * 1000;
  let best: number | null = null;
  let bestD = Infinity;
  for (const s of win) {
    const d = Math.abs(s.timestamp_ms - target);
    if (d < bestD) {
      const v = pick(s);
      if (v !== null) {
        best = v;
        bestD = d;
      }
    }
  }
  return bestD < 6000 ? best : null;
}

const SRC_NAME: Record<string, string> = { cpu: 'CPU', gpu: 'GPU', psu_vrm: 'PSU VRM', psu_case: 'PSU case' };
const srcName = (s: string) => SRC_NAME[s] ?? sourceLabel(s);

/**
 * Context sentence for a highlight.
 *
 * No-redundancy rules: a sentence never restates the row's own figure, never
 * restates state owned by an alert row (emergency / stale), and in `detail`
 * mode never restates an absolute min/avg/max that the detail chart already
 * annotates; it speaks in deltas and trends instead.
 */
export function contextSentence(
  h: Highlight,
  now: SystemSnapshot,
  timeline: SystemSnapshot[],
  config: AppConfig | null,
  mode: 'summary' | 'detail' = 'summary',
): string {
  const ts = now.timestamp_ms;
  const win = windowBefore(timeline, ts, 300);
  const v = h.pick(now);
  if (v === null) return 'No reading right now.';
  const vals = win.map(h.pick).filter((x): x is number => x !== null);
  const st = stats(vals);
  if (!st || vals.length < 10) return 'Collecting history for comparison…';
  const last2 = windowBefore(win, ts, 120)
    .map(h.pick)
    .filter((x): x is number => x !== null);
  const st2 = stats(last2)!;
  const ago30 = valueAgo(win, h.pick, ts, 30);
  const slope = ago30 === null ? 0 : v - ago30;
  const summary = mode === 'summary';

  if (h.kind === 'temp') {
    const name = srcName(h.id);
    if (st2 && st2.max - st2.min < 2.5 && Math.abs(v - st.avg) < 1.5)
      return `${name} has held within ${(st2.max - st2.min).toFixed(1)}° for the last 2 minutes.`;
    if (slope > 2.5) return `${name} is climbing — up ${slope.toFixed(1)}° in the last 30 seconds.`;
    if (slope < -2.5) return `${name} is cooling down — ${Math.abs(slope).toFixed(1)}° lower than 30 seconds ago.`;
    const d = v - st.avg;
    if (Math.abs(d) < 0.6) return `${name} is right at its 5-minute average.`;
    if (st.max - v > 8 && d < 0)
      return summary
        ? `${name} is ${(st.max - v).toFixed(1)}° below its 5-minute peak of ${st.max.toFixed(1)}°.`
        : `${name} is ${(st.max - v).toFixed(1)}° below its 5-minute peak.`;
    return `${name} is ${Math.abs(d).toFixed(1)}° ${d < 0 ? 'cooler' : 'warmer'} than its 5-minute average.`;
  }

  if (h.kind === 'group') {
    const group = config?.fan_groups.find((g) => g.name === h.id);
    if (group?.mode.type === 'fixed') return 'Fixed duty — this group ignores temperature.';
    const rpm = groupRpm(now, h.id);
    const rpms = win.map((s) => groupRpm(s, h.id)).filter((x): x is number => x !== null);
    const rpmAvg = rpms.length ? rpms.reduce((a, b) => a + b, 0) / rpms.length : null;
    // which sensor is driving it?
    let driver = '';
    if (group) {
      let bestDev = 0;
      for (const sname of group.mode.temp_source.sensors) {
        const tNow = tempOf(now, sname);
        const tv = win.map((s) => tempOf(s, sname)).filter((x): x is number => x !== null);
        if (tNow === null || tv.length === 0) continue;
        const dev = tNow - tv.reduce((a, b) => a + b, 0) / tv.length;
        if (Math.abs(dev) > Math.abs(bestDev)) {
          bestDev = dev;
          driver = srcName(sname);
        }
      }
      if (Math.abs(bestDev) < 1.5) driver = '';
    }
    if (st2 && st2.max - st2.min < 3) return 'Holding steady for the last 2 minutes.';
    if (rpm !== null && rpmAvg !== null) {
      const d = rpm - rpmAvg;
      if (Math.abs(d) >= 40) {
        const dir = d > 0 ? 'above' : 'below';
        const why = driver ? (d > 0 ? ` — reacting to the ${driver}.` : ` — the ${driver} is easing off.`) : '.';
        return `Fans are ${Math.round(Math.abs(d) / 10) * 10} RPM ${dir} their 5-minute average${why}`;
      }
    }
    if (slope > 3) return `Ramping up — ${Math.round(slope)}% higher than 30 seconds ago.`;
    if (slope < -3) return `Easing off — ${Math.round(Math.abs(slope))}% lower than 30 seconds ago.`;
    return summary
      ? `Running close to its 5-minute average of ${Math.round(st.avg)}%.`
      : 'Running close to its 5-minute average.';
  }

  // power
  if (st2 && st2.max - st2.min < 25) return `Held within ${Math.round(st2.max - st2.min)} W for the last 2 minutes.`;
  if (slope > 60) {
    const g0 = valueAgo(win, (s) => s.gpu_metrics?.power_watts ?? null, ts, 30);
    const gNow = now.gpu_metrics?.power_watts ?? null;
    const gpuDriven = g0 !== null && gNow !== null && gNow - g0 > slope * 0.5;
    return `Up ${Math.round(slope)} W in the last 30 seconds${gpuDriven ? ' — the GPU is under load.' : '.'}`;
  }
  if (slope < -60) return `Down ${Math.round(Math.abs(slope))} W from 30 seconds ago.`;
  if (st.max - v > 80) return `${Math.round(st.max - v)} W below its 5-minute peak.`;
  const d = v - st.avg;
  if (Math.abs(d) < 10) return 'Right at its 5-minute average.';
  return `${Math.round(Math.abs(d))} W ${d > 0 ? 'above' : 'below'} its 5-minute average.`;
}

/**
 * Temperature-state phrase for the Summary subtitle (inventory 1.7 rule).
 * Emergency / stale are owned by their alert rows and are not repeated here.
 */
export function statusPhrase(maxT: number): string {
  if (maxT > 85) return 'Critical temperature';
  if (maxT > 70) return 'Running warm';
  return 'Temperatures normal';
}
