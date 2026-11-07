<!--
  RowChart: the wide inline chart on a Summary row. Monotone line with a
  vertical gradient fill, end dot, faint unlabelled min/max ticks (values are
  not repeated here: the row's figure and sentence are their single home),
  and a scrub marker while the app is being rewound. Dashboard-local (lib/charts/Sparkline stays the compact variant).
-->
<script lang="ts">
  import { linear, monotoneArea, monotonePath, nearestIndex, type Pt } from '../../lib/charts/monotone';
  import { scrub } from '../../lib/stores/scrub.svelte';

  interface Props {
    data: [number, number][];
    color: string;
    height?: number;
    /** fixed y domain (e.g. 0–100 for duty); auto-padded when omitted */
    min?: number;
    max?: number;
    /** decimals of the data (used to detect a flat series) */
    decimals?: number;
    ticks?: boolean;
    /** plain click (no drag) handler, e.g. open the detail page */
    onopen?: () => void;
  }
  let { data, color, height = 88, min, max, decimals = 0, ticks = true, onopen }: Props = $props();

  let width = $state(0);
  const uid = `rc-${Math.random().toString(36).slice(2, 8)}`;
  const PAD = 5;

  const geo = $derived.by(() => {
    if (data.length < 2 || width <= 40) return null;
    let lo = Infinity,
      hi = -Infinity;
    for (const [, v] of data) {
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    const span = Math.max(hi - lo, 1);
    const y0 = min ?? lo - span * 0.25;
    const y1 = max ?? hi + span * 0.2;
    const plotR = width - PAD - 2;
    const x = linear([data[0][0], data[data.length - 1][0]], [PAD, plotR]);
    const y = linear([y0, y1], [height - PAD, PAD + 2]);
    const pts: Pt[] = data.map(([t, v]) => [x(t), y(v)]);
    return {
      x,
      pts,
      plotR,
      line: monotonePath(pts),
      area: monotoneArea(pts, height),
      lo: { v: lo, y: y(lo) },
      hi: { v: hi, y: y(hi) },
      flat: hi - lo < Math.pow(10, -decimals),
    };
  });

  const marker = $derived.by(() => {
    if (!geo || !scrub.active || scrub.ts === null) return null;
    if (scrub.ts < data[0][0] - 1000) return null;
    const i = nearestIndex(
      data.map((d) => d[0]),
      scrub.ts,
    );
    return geo.pts[i];
  });
  const last = $derived(geo ? geo.pts[geo.pts.length - 1] : null);

  // Press-drag scrubs the whole app (same as the big charts); a plain click
  // falls through to `onopen`. A 4 px threshold separates the two.
  let downX: number | null = null;
  let dragging = $state(false);
  function tsAt(e: PointerEvent): number | null {
    if (!geo) return null;
    const rect = (e.currentTarget as Element).getBoundingClientRect();
    const px = Math.min(geo.plotR, Math.max(PAD, e.clientX - rect.left));
    return geo.x.invert(px);
  }
  function onDown(e: PointerEvent) {
    if (e.button !== 0 || !geo) return;
    downX = e.clientX;
  }
  function onMove(e: PointerEvent) {
    if (downX === null) return;
    if (!dragging && Math.abs(e.clientX - downX) > 4) {
      dragging = true;
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      const ts = tsAt(e);
      if (ts !== null) scrub.begin(ts, 'row');
    } else if (dragging) {
      const ts = tsAt(e);
      if (ts !== null) scrub.set(ts);
    }
  }
  function onUp() {
    if (dragging) {
      dragging = false;
      scrub.release();
    } else if (downX !== null) {
      onopen?.();
    }
    downX = null;
  }
  function onCancel() {
    if (dragging) scrub.release();
    dragging = false;
    downX = null;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="rc"
  class:dragging
  style:height="{height}px"
  bind:clientWidth={width}
  onpointerdown={onDown}
  onpointermove={onMove}
  onpointerup={onUp}
  onpointercancel={onCancel}
>
  {#if geo}
    <svg {width} {height} aria-hidden="true">
      <defs>
        <linearGradient id="{uid}-g" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color={color} stop-opacity="0.28" />
          <stop offset="75%" stop-color={color} stop-opacity="0.04" />
          <stop offset="100%" stop-color={color} stop-opacity="0" />
        </linearGradient>
      </defs>
      {#if ticks && !geo.flat}
        <line class="tick" x1={PAD} x2={geo.plotR} y1={geo.hi.y} y2={geo.hi.y} />
        <line class="tick" x1={PAD} x2={geo.plotR} y1={geo.lo.y} y2={geo.lo.y} />
      {/if}
      <path d={geo.area} fill="url(#{uid}-g)" />
      <path
        d={geo.line}
        fill="none"
        stroke={color}
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        opacity={marker ? 0.5 : 1}
      />
      {#if marker}
        <line class="mline" x1={marker[0]} x2={marker[0]} y1="0" y2={height} />
        <circle cx={marker[0]} cy={marker[1]} r="4" fill={color} stroke="var(--bg-0)" stroke-width="2" />
      {:else if last}
        <circle cx={last[0]} cy={last[1]} r="6" fill={color} opacity="0.18" />
        <circle cx={last[0]} cy={last[1]} r="3.25" fill={color} stroke="var(--bg-0)" stroke-width="1.5" />
      {/if}
    </svg>
  {/if}
</div>

<style>
  .rc {
    width: 100%;
    min-width: 0;
    cursor: ew-resize;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .rc.dragging {
    cursor: grabbing;
  }
  svg {
    display: block;
    overflow: visible;
  }
  .tick {
    stroke: rgba(255, 255, 255, 0.16);
    stroke-width: 1;
    stroke-dasharray: 1 3;
    stroke-linecap: round;
  }
  .mline {
    stroke: var(--accent);
    stroke-width: 1;
    opacity: 0.75;
    shape-rendering: crispEdges;
  }
</style>
