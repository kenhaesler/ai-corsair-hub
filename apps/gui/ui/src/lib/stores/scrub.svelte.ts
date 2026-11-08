// SIGNATURE: time-scrub.
//
// Dragging across any history chart rewinds the whole app to that moment.
// `scrub` holds the scrub position as a timestamp (stable while new data
// keeps arriving). `display` is what every visual reads:
//
//   display.snapshot          SystemSnapshot shown right now (past while scrubbing, else live,
//                             blended on release so numbers glide home)
//   display.live              latest live snapshot
//   display.scrubbing         scrub.active
//   display.offsetSec         seconds behind live (0 when live)
//   display.timestamp         timestamp of display.snapshot
//   display.maxTemp           hottest temperature in display.snapshot
//   display.thermalT          clamp((maxTemp-38)/(82-38)) for ambient & accents
//   display.timeline          full timeline (session 5 s samples + 1 Hz history)
//   display.historyWindow(s)  live-anchored history for the last s seconds ('all' = session)
//   display.series(fn, s)     [ts, value][] for a chart, from historyWindow(s)
//
//   scrub.begin(ts?) / scrub.set(ts) / scrub.step(seconds) / scrub.release()
//
// NEVER read sensors.snapshot directly for display.

import { Spring } from 'svelte/motion';
import { springs, reducedMotion } from '../motion.svelte';
import { maxTemp } from '../format';
import type { SystemSnapshot } from '../types';
import { sensors } from './sensors.svelte';

export type Range = 60 | 300 | 'all';

function nearest(list: SystemSnapshot[], ts: number): SystemSnapshot | null {
  if (list.length === 0) return null;
  let lo = 0,
    hi = list.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (list[mid].timestamp_ms < ts) lo = mid + 1;
    else hi = mid;
  }
  const a = list[lo];
  const b = list[lo - 1];
  if (b && Math.abs(b.timestamp_ms - ts) < Math.abs(a.timestamp_ms - ts)) return b;
  return a;
}

/** Combined timeline: session samples older than the 1 Hz history, then history. */
function buildTimeline(): SystemSnapshot[] {
  const h = sensors.history;
  const first = h[0]?.timestamp_ms ?? Infinity;
  const older = sensors.session.filter((s) => s.timestamp_ms < first - 500);
  return older.length ? [...older, ...h] : h;
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

/** Blend two snapshots (structure from `b`, values interpolated from `a`). */
function lerpSnapshot(a: SystemSnapshot, b: SystemSnapshot, t: number): SystemSnapshot {
  if (t >= 1) return b;
  return {
    ...b,
    temperatures: b.temperatures.map((x) => {
      const o = a.temperatures.find((y) => y.source === x.source);
      return o ? { ...x, celsius: lerp(o.celsius, x.celsius, t) } : x;
    }),
    group_duties: b.group_duties.map((x) => {
      const o = a.group_duties.find((y) => y.name === x.name);
      return o ? { ...x, duty_percent: lerp(o.duty_percent, x.duty_percent, t) } : x;
    }),
    fans: b.fans.map((x) => {
      const o = a.fans.find((y) => y.device_id === x.device_id && y.channel === x.channel);
      return o ? { ...x, rpm: lerp(o.rpm, x.rpm, t), duty_percent: lerp(o.duty_percent, x.duty_percent, t) } : x;
    }),
    psu:
      b.psu && a.psu
        ? {
            ...b.psu,
            temp_vrm: lerp(a.psu.temp_vrm, b.psu.temp_vrm, t),
            temp_case: lerp(a.psu.temp_case, b.psu.temp_case, t),
            fan_rpm: lerp(a.psu.fan_rpm, b.psu.fan_rpm, t),
            input_voltage: lerp(a.psu.input_voltage, b.psu.input_voltage, t),
            total_power: lerp(a.psu.total_power, b.psu.total_power, t),
            rails: b.psu.rails.map((r) => {
              const o = a.psu!.rails.find((y) => y.name === r.name);
              return o
                ? {
                    ...r,
                    voltage: lerp(o.voltage, r.voltage, t),
                    current: lerp(o.current, r.current, t),
                    power: lerp(o.power, r.power, t),
                  }
                : r;
            }),
          }
        : b.psu,
    cpu_metrics:
      b.cpu_metrics && a.cpu_metrics
        ? {
            load_pct: lerp(a.cpu_metrics.load_pct, b.cpu_metrics.load_pct, t),
            power_watts: lerp(a.cpu_metrics.power_watts, b.cpu_metrics.power_watts, t),
            clock_mhz: Math.round(lerp(a.cpu_metrics.clock_mhz, b.cpu_metrics.clock_mhz, t)),
          }
        : b.cpu_metrics,
    gpu_metrics:
      b.gpu_metrics && a.gpu_metrics
        ? {
            utilization_pct: Math.round(lerp(a.gpu_metrics.utilization_pct, b.gpu_metrics.utilization_pct, t)),
            power_watts: lerp(a.gpu_metrics.power_watts, b.gpu_metrics.power_watts, t),
            clock_mhz: Math.round(lerp(a.gpu_metrics.clock_mhz, b.gpu_metrics.clock_mhz, t)),
            memory_used_mb: lerp(a.gpu_metrics.memory_used_mb, b.gpu_metrics.memory_used_mb, t),
            temp_celsius: lerp(a.gpu_metrics.temp_celsius, b.gpu_metrics.temp_celsius, t),
          }
        : b.gpu_metrics,
  };
}

// ---------------------------------------------------------------------------
// Scrub store
// ---------------------------------------------------------------------------
class ScrubStore {
  active = $state(false);
  /** Timestamp under the cursor while active. */
  ts = $state<number | null>(null);
  /** Id of the chart that owns the drag (for styling the source chart). */
  source = $state<string | null>(null);

  /** Start scrubbing (optionally at a timestamp; defaults to live). */
  begin(ts?: number, source: string | null = null) {
    const live = sensors.snapshot?.timestamp_ms ?? Date.now();
    display.cancelBlend();
    this.ts = ts ?? live;
    this.source = source;
    this.active = true;
  }

  /** Move the scrub position to a timestamp (clamped to the timeline). */
  set(ts: number) {
    if (!this.active) this.begin(ts);
    const tl = display.timeline;
    if (tl.length === 0) return;
    this.ts = Math.min(tl[tl.length - 1].timestamp_ms, Math.max(tl[0].timestamp_ms, ts));
  }

  /** Move by ±seconds (keyboard). Begins at live when not active. */
  step(seconds: number, source: string | null = null) {
    if (!this.active) this.begin(undefined, source);
    this.set((this.ts ?? Date.now()) + seconds * 1000);
  }

  /** Spring back to live. */
  release() {
    if (!this.active) return;
    const from = display.snapshot;
    this.active = false;
    this.ts = null;
    this.source = null;
    display.startBlend(from);
  }
}

export const scrub = new ScrubStore();

// ---------------------------------------------------------------------------
// Display store
// ---------------------------------------------------------------------------
class DisplayStore {
  /** 0 = frozen scrub values, 1 = live. Springs on release. */
  private blend = new Spring(1, { ...springs.scrubBack });
  private frozen = $state.raw<SystemSnapshot | null>(null);

  /** Combined timeline (session samples older than history, then 1 Hz history). Oldest first. */
  timeline = $derived.by(buildTimeline);

  get live(): SystemSnapshot | null {
    return sensors.snapshot;
  }

  get scrubbing(): boolean {
    return scrub.active;
  }

  /** The past sample under the scrub cursor. */
  private scrubbed = $derived.by(() => {
    if (!scrub.active || scrub.ts === null) return null;
    return nearest(this.timeline, scrub.ts);
  });

  snapshot = $derived.by((): SystemSnapshot | null => {
    const live = sensors.snapshot;
    if (scrub.active) return this.scrubbed ?? live;
    const f = this.frozen;
    const b = this.blend.current;
    if (f && live && b < 0.999) return lerpSnapshot(f, live, b);
    return live;
  });

  timestamp = $derived(this.snapshot?.timestamp_ms ?? Date.now());

  offsetSec = $derived.by(() => {
    if (!scrub.active || !this.scrubbed || !sensors.snapshot) return 0;
    return Math.max(0, (sensors.snapshot.timestamp_ms - this.scrubbed.timestamp_ms) / 1000);
  });

  maxTemp = $derived(maxTemp(this.snapshot));

  thermalT = $derived(Math.min(1, Math.max(0, (this.maxTemp - 38) / (82 - 38))));

  /** @internal */
  startBlend(from: SystemSnapshot | null) {
    if (!from || reducedMotion.current) {
      this.frozen = null;
      this.blend.set(1, { instant: true });
      return;
    }
    this.frozen = from;
    this.blend.set(0, { instant: true });
    this.blend.target = 1;
  }
  /** @internal */
  cancelBlend() {
    this.frozen = null;
    this.blend.set(1, { instant: true });
  }

  /** History for the last `range` seconds anchored at live (not at the scrub). */
  historyWindow(range: Range | number): SystemSnapshot[] {
    if (range === 'all') return this.timeline;
    const h = sensors.history;
    const end = h[h.length - 1]?.timestamp_ms ?? Date.now();
    const start = end - range * 1000;
    if (range > 300) {
      const tl = this.timeline;
      const j = tl.findIndex((s) => s.timestamp_ms >= start);
      return j < 0 ? [] : tl.slice(Math.max(0, j - 1));
    }
    // history is 1 Hz: slice from the end, keeping one sample before the
    // window so lines enter from the chart's left edge
    let i = h.length - 1;
    while (i > 0 && h[i - 1].timestamp_ms >= start) i--;
    return h.slice(Math.max(0, i - 1));
  }

  /** Values for a chart series: [timestamp, value] pairs (null values skipped). */
  series(pick: (s: SystemSnapshot) => number | null | undefined, range: Range | number): [number, number][] {
    const out: [number, number][] = [];
    for (const s of this.historyWindow(range)) {
      const v = pick(s);
      if (v !== null && v !== undefined && Number.isFinite(v)) out.push([s.timestamp_ms, v]);
    }
    return out;
  }
}

export const display = new DisplayStore();

// Stores hold live listeners and buffers; a hot swap of this module would
// leave the app wired to the old instance. Reload the page instead.
if (import.meta.hot) import.meta.hot.accept(() => window.location.reload());
