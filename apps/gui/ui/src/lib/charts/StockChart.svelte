<!--
  StockChart: Stocks-grade interactive time-series chart.

  - multiple series: solid with vertical gradient fill, or dashed
  - optional dual y-axes (left/right), auto or fixed domains
  - end labels of the last value, min/avg/max annotations (toggle)
  - hover crosshair + readout bubble
  - press-drag scrubs the whole app (drives the global scrub store);
    every visible chart shows the synchronised crosshair while scrubbing
  - keyboard: ←/→ scrub 1 s (Shift = 10 s), Esc returns to live
  - streams smoothly: new samples slide in from the right (transform only)
-->
<script lang="ts" module>
  export interface ChartSeries {
    id: string;
    label: string;
    color: string;
    /** [timestamp_ms, value] pairs, oldest first */
    data: [number, number][];
    axis?: 'left' | 'right';
    dashed?: boolean;
    /** gradient fill under the line (default: true unless dashed) */
    fill?: boolean;
    /** unit appended to values: '°', '%', ' W', ' RPM' */
    unit?: string;
    decimals?: number;
  }
  export interface ChartAxis {
    min?: number;
    max?: number;
    ticks?: number[];
    format?: (v: number) => string;
  }
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import { Tween } from 'svelte/motion';
  import { linear as easeLinear } from 'svelte/easing';
  import { linear, monotoneArea, monotonePath, nearestIndex, niceDomain, niceTicks, stats, type Pt } from './monotone';
  import { scrub, display, type Range } from '../stores/scrub.svelte';
  import { reducedMotion } from '../motion.svelte';
  import { fmtClock, fmtOffset } from '../format';

  interface Props {
    id: string;
    label: string;
    series: ChartSeries[];
    range?: Range;
    height?: number;
    left?: ChartAxis | null;
    right?: ChartAxis | null;
    /** min/avg/max annotations for the first left-axis series */
    annotations?: boolean;
    /**
     * End labels of the last value. 'interactive' = progressive disclosure: shown
     * only while hovering or scrubbing (gutter space is still reserved).
     */
    endLabels?: boolean | 'interactive';
    xAxis?: boolean;
    emptyText?: string;
    minPoints?: number;
  }

  let {
    id,
    label,
    series,
    range = 60,
    height = 240,
    left = {},
    right = null,
    annotations = false,
    endLabels = true,
    xAxis = true,
    emptyText = 'Collecting data…',
    minPoints = 2,
  }: Props = $props();

  let width = $state(0);
  let hoverPx = $state<number | null>(null);
  let dragging = $state(false);
  let focused = $state(false);

  // stable per instance: gradient/clip ids must not change if `id` changes
  const uid = untrack(() => `sc-${id.replace(/[^a-zA-Z0-9_-]/g, '_')}-${Math.random().toString(36).slice(2, 7)}`);

  const pad = $derived({
    top: 14,
    bottom: xAxis ? 26 : 8,
    left: left ? 36 : 8,
    // right gutter holds end labels and (when on) the min/avg/max labels
    right: Math.max(endLabels ? 58 : 10, annotations ? 84 : 0) + (right ? 30 : 0),
  });
  const plotW = $derived(Math.max(10, width - pad.left - pad.right));
  const plotH = $derived(Math.max(10, height - pad.top - pad.bottom));

  const enough = $derived(series.some((s) => s.data.length >= minPoints));

  // ---- x domain: anchored at the newest sample (live end) ----
  const endTs = $derived(Math.max(0, ...series.map((s) => (s.data.length ? s.data[s.data.length - 1][0] : 0))));
  const startTs = $derived.by(() => {
    if (range === 'all') {
      const firsts = series.filter((s) => s.data.length).map((s) => s.data[0][0]);
      return firsts.length ? Math.min(...firsts) : endTs - 60_000;
    }
    return endTs - range * 1000;
  });
  const x = $derived(linear([startTs, endTs], [pad.left, pad.left + plotW]));

  // ---- y domains ----
  function domainFor(axis: ChartAxis | null | undefined, which: 'left' | 'right'): [number, number] {
    if (axis && axis.min !== undefined && axis.max !== undefined) return [axis.min, axis.max];
    let lo = Infinity,
      hi = -Infinity;
    for (const s of series) {
      if ((s.axis ?? 'left') !== which) continue;
      for (const [t, v] of s.data) {
        if (t < startTs) continue;
        if (v < lo) lo = v;
        if (v > hi) hi = v;
      }
    }
    if (!Number.isFinite(lo)) return [0, 100];
    const [a, b] = niceDomain(lo, hi, 4, 0.18);
    return [axis?.min ?? a, axis?.max ?? b];
  }
  const yl = $derived(linear(domainFor(left, 'left'), [pad.top + plotH, pad.top]));
  const yr = $derived(linear(domainFor(right, 'right'), [pad.top + plotH, pad.top]));
  const yOf = (s: ChartSeries) => ((s.axis ?? 'left') === 'right' ? yr : yl);

  const leftTicks = $derived(left?.ticks ?? niceTicks(yl.domain[0], yl.domain[1], 4));
  const rightTicks = $derived(right ? (right.ticks ?? niceTicks(yr.domain[0], yr.domain[1], 4)) : []);
  const fmtL = (v: number) => (left?.format ? left.format(v) : `${Math.round(v * 10) / 10}`);
  const fmtR = (v: number) => (right?.format ? right.format(v) : `${Math.round(v)}`);

  // ---- paths ----
  const visible = (s: ChartSeries) => {
    // include one point before the window so the line enters from the edge
    const d = s.data;
    let i = 0;
    while (i < d.length - 1 && d[i + 1][0] < startTs) i++;
    return d.slice(i);
  };
  const paths = $derived(
    series.map((s) => {
      const y = yOf(s);
      const pts: Pt[] = visible(s).map(([t, v]) => [x(t), y(v)]);
      const fill = s.fill ?? !s.dashed;
      return {
        s,
        line: monotonePath(pts),
        area: fill ? monotoneArea(pts, pad.top + plotH) : '',
        last: pts[pts.length - 1] as Pt | undefined,
      };
    }),
  );

  // ---- streaming slide-in ----
  const slide = new Tween(0, { duration: 0 });
  let prevEnd = 0;
  $effect(() => {
    const e = endTs;
    const msPerPx = (endTs - startTs) / plotW;
    if (prevEnd && e > prevEnd && range !== 'all' && !reducedMotion.current && msPerPx > 0) {
      const dx = (e - prevEnd) / msPerPx;
      if (dx < plotW / 4) {
        slide.set(dx, { duration: 0 });
        slide.set(0, { duration: 900, easing: easeLinear });
      }
    }
    prevEnd = e;
  });

  // ---- annotations (first left-axis series) ----
  const annot = $derived.by(() => {
    if (!annotations) return null;
    const s = series.find((q) => (q.axis ?? 'left') === 'left' && q.data.length >= minPoints);
    if (!s) return null;
    const vals = s.data.filter(([t]) => t >= startTs).map(([, v]) => v);
    const st = stats(vals);
    if (!st) return null;
    const dp = s.decimals ?? 1;
    const u = s.unit ?? '';
    return {
      color: s.color,
      rows: [
        { k: 'Max', v: st.max, y: yl(st.max), text: `${st.max.toFixed(dp)}${u}` },
        { k: 'Avg', v: st.avg, y: yl(st.avg), text: `${st.avg.toFixed(dp)}${u}` },
        { k: 'Min', v: st.min, y: yl(st.min), text: `${st.min.toFixed(dp)}${u}` },
      ],
    };
  });

  // ---- end labels with collision avoidance ----
  // End labels and annotation labels share the right gutter and one
  // collision-avoidance pass, so neither ever sits on top of the series.
  const endLabelPos = $derived.by(() => {
    const items: { id: string; color: string; text: string; y: number; kind: 'end' | 'annot' }[] = [];
    if (endLabels === true || (endLabels === 'interactive' && cursor !== null))
      for (const p of paths) {
        if (!p.last || !p.s.data.length) continue;
        const v = p.s.data[p.s.data.length - 1][1];
        items.push({
          id: p.s.id,
          color: p.s.color,
          text: `${v.toFixed(p.s.decimals ?? 1)}${p.s.unit ?? ''}`,
          y: p.last[1],
          kind: 'end',
        });
      }
    if (annot)
      for (const r of annot.rows) items.push({ id: `annot-${r.k}`, color: annot.color, text: `${r.k} ${r.text}`, y: r.y, kind: 'annot' });
    items.sort((a, b) => a.y - b.y);
    const gap = 17;
    for (let i = 1; i < items.length; i++) if (items[i].y - items[i - 1].y < gap) items[i].y = items[i - 1].y + gap;
    const maxY = pad.top + plotH + 4;
    for (let i = items.length - 1; i >= 0; i--) {
      const limit = i === items.length - 1 ? maxY : items[i + 1].y - gap;
      if (items[i].y > limit) items[i].y = limit;
    }
    return items;
  });

  // ---- x axis labels (relative to live) ----
  const xTicks = $derived.by(() => {
    if (!xAxis) return [];
    const spanS = (endTs - startTs) / 1000;
    let stepS: number;
    if (spanS <= 75) stepS = 15;
    else if (spanS <= 330) stepS = 60;
    else stepS = Math.max(300, Math.round(spanS / 5 / 300) * 300);
    const out: { x: number; text: string }[] = [];
    for (let k = 0; k * stepS <= spanS + 0.5; k++) {
      const s = k * stepS;
      const text = k === 0 ? 'Now' : stepS < 60 ? `−${s}s` : `−${Math.round(s / 60)}m`;
      out.push({ x: x(endTs - s * 1000), text });
    }
    return out;
  });

  // ---- crosshair: own hover, or the global scrub ----
  const cursorTs = $derived.by((): number | null => {
    if (scrub.active && scrub.ts !== null) return scrub.ts;
    if (hoverPx !== null) return x.invert(hoverPx - slide.current);
    return null;
  });
  const cursor = $derived.by(() => {
    if (cursorTs === null || !enough) return null;
    if (cursorTs < startTs - 1000 || cursorTs > endTs + 1000) return null;
    let snapTs = cursorTs;
    const rows = series
      .filter((s) => s.data.length)
      .map((s) => {
        const xs = s.data.map((d) => d[0]);
        const i = nearestIndex(xs, cursorTs);
        const [t, v] = s.data[i];
        snapTs = t;
        return { s, t, v, y: yOf(s)(v) };
      });
    return { ts: snapTs, px: x(snapTs) + slide.current, rows };
  });
  const showBubble = $derived(
    cursor !== null && (hoverPx !== null || dragging || (scrub.active && (scrub.source === id || focused))),
  );

  // ---- pointer handling ----
  function localX(e: PointerEvent): number {
    const rect = (e.currentTarget as Element).getBoundingClientRect();
    return Math.min(pad.left + plotW, Math.max(pad.left, e.clientX - rect.left));
  }
  function tsAt(px: number) {
    return x.invert(px - slide.current);
  }
  function onMove(e: PointerEvent) {
    const px = localX(e);
    hoverPx = px;
    if (dragging) scrub.set(tsAt(px));
  }
  function onDown(e: PointerEvent) {
    if (!enough || e.button !== 0) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    dragging = true;
    const px = localX(e);
    hoverPx = px;
    scrub.begin(tsAt(px), id);
  }
  function onUp() {
    if (!dragging) return;
    dragging = false;
    scrub.release();
  }
  function onLeave() {
    if (!dragging) hoverPx = null;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      e.preventDefault();
      const dir = e.key === 'ArrowLeft' ? -1 : 1;
      const stepS = range === 'all' ? 5 : 1;
      scrub.step(dir * (e.shiftKey ? 10 : 1) * stepS, id);
    } else if (e.key === 'Escape' && scrub.active) {
      e.preventDefault();
      e.stopPropagation();
      scrub.release();
    } else if (e.key === 'Home' && enough) {
      e.preventDefault();
      scrub.begin(startTs, id);
    } else if (e.key === 'End' && scrub.active) {
      e.preventDefault();
      scrub.release();
    }
  }
  function onBlur() {
    focused = false;
    if (scrub.active && scrub.source === id && !dragging) scrub.release();
  }

  const ariaText = $derived.by(() => {
    if (!cursor || !scrub.active) return 'Live';
    const vals = cursor.rows.map((r) => `${r.s.label} ${fmtVal(r.s, r.v)}`).join(', ');
    return `${fmtOffset(Math.max(0, (endTs - cursor.ts) / 1000))} ago: ${vals}`;
  });
  const fmtVal = (s: ChartSeries, v: number) => `${v.toFixed(s.decimals ?? 1)}${s.unit ?? ''}`;
  const bubbleLeft = $derived(cursor ? cursor.px > pad.left + plotW * 0.62 : false);
</script>

<div class="chart" class:dragging style:height="{height}px" bind:clientWidth={width}>
  {#if !enough}
    <div class="empty t-callout">{emptyText}</div>
  {:else if width > 0}
    <svg
      class="svg"
      {width}
      {height}
      role="slider"
      tabindex="0"
      aria-label="{label}. Drag or use the left and right arrow keys to scrub history; Escape returns to live."
      aria-valuemin={-Math.round((endTs - startTs) / 1000)}
      aria-valuemax={0}
      aria-valuenow={scrub.active && scrub.ts !== null ? -Math.round((endTs - scrub.ts) / 1000) : 0}
      aria-valuetext={ariaText}
      onpointermove={onMove}
      onpointerdown={onDown}
      onpointerup={onUp}
      onpointercancel={onUp}
      onpointerleave={onLeave}
      onkeydown={onKey}
      onfocus={() => (focused = true)}
      onblur={onBlur}
    >
      <defs>
        <clipPath id="{uid}-clip">
          <rect x={pad.left} y={pad.top - 4} width={plotW} height={plotH + 8} />
        </clipPath>
        {#each series as s (s.id)}
          <linearGradient id="{uid}-g-{s.id}" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color={s.color} stop-opacity="0.26" />
            <stop offset="70%" stop-color={s.color} stop-opacity="0.05" />
            <stop offset="100%" stop-color={s.color} stop-opacity="0" />
          </linearGradient>
        {/each}
      </defs>

      <!-- grid + left axis -->
      {#if left}
        {#each leftTicks as t (t)}
          <line class="grid" x1={pad.left} x2={pad.left + plotW} y1={yl(t)} y2={yl(t)} />
          <text class="axis num" x={pad.left - 8} y={yl(t)} text-anchor="end" dominant-baseline="middle">{fmtL(t)}</text>
        {/each}
      {/if}
      {#if right}
        {#each rightTicks as t (t)}
          <text class="axis num" x={pad.left + plotW + pad.right - 4} y={yr(t)} text-anchor="end" dominant-baseline="middle">{fmtR(t)}</text>
        {/each}
      {/if}
      <line class="baseline" x1={pad.left} x2={pad.left + plotW} y1={pad.top + plotH} y2={pad.top + plotH} />

      <!-- annotations -->
      {#if annot}
        {#each annot.rows as r (r.k)}
          <line class="annot" x1={pad.left} x2={pad.left + plotW} y1={r.y} y2={r.y} />
        {/each}
      {/if}

      <!-- series -->
      <g clip-path="url(#{uid}-clip)">
        <g class="stream" style:transform="translateX({slide.current}px)">
          {#each paths as p (p.s.id)}
            {#if p.area}
              <path d={p.area} fill="url(#{uid}-g-{p.s.id})" />
            {/if}
            <path
              d={p.line}
              fill="none"
              stroke={p.s.color}
              stroke-width={p.s.dashed ? 1.5 : 2}
              stroke-dasharray={p.s.dashed ? '5 4' : undefined}
              stroke-linecap="round"
              stroke-linejoin="round"
              opacity={p.s.dashed ? 0.85 : 1}
            />
          {/each}
          {#each paths as p (p.s.id)}
            {#if p.last && !p.s.dashed && !cursor}
              <circle class="end-dot" cx={p.last[0]} cy={p.last[1]} r="3.5" fill={p.s.color} />
            {/if}
          {/each}
        </g>
      </g>

      <!-- end labels -->
      {#each endLabelPos as l (l.id)}
        <g class="end-label" style:transform="translate({pad.left + plotW + 8}px, {l.y}px)">
          {#if l.kind === 'end'}
            <rect x="0" y="-9" rx="5" width={l.text.length * 6.6 + 10} height="18" fill={l.color} fill-opacity="0.16" />
            <text x="5" y="0.5" dominant-baseline="middle" fill={l.color} class="num">{l.text}</text>
          {:else}
            <text x="5" y="0.5" dominant-baseline="middle" class="annot-label num">{l.text}</text>
          {/if}
        </g>
      {/each}

      <!-- x axis -->
      {#each xTicks as t (t.text)}
        <text class="axis num" x={t.x} y={height - 8} text-anchor={t.text === 'Now' ? 'end' : 'middle'}>{t.text}</text>
      {/each}

      <!-- crosshair -->
      {#if cursor}
        <g class="cursor" class:scrub={scrub.active}>
          <line x1={cursor.px} x2={cursor.px} y1={pad.top - 6} y2={pad.top + plotH} />
          {#each cursor.rows as r (r.s.id)}
            <circle cx={cursor.px} cy={r.y} r="4.5" fill="var(--bg-0)" stroke={r.s.color} stroke-width="2" />
          {/each}
        </g>
      {/if}
    </svg>

    {#if cursor && showBubble}
      <div
        class="bubble material"
        class:flip={bubbleLeft}
        style:transform="translate({bubbleLeft ? cursor.px - 12 : cursor.px + 12}px, {pad.top}px) translateX({bubbleLeft ? '-100%' : '0'})"
        aria-hidden="true"
      >
        <div class="b-time num">
          {fmtClock(cursor.ts)}
          {#if display.live && display.live.timestamp_ms - cursor.ts > 1500}
            <span class="b-off">−{fmtOffset((display.live.timestamp_ms - cursor.ts) / 1000)}</span>
          {/if}
        </div>
        {#each cursor.rows as r (r.s.id)}
          <div class="b-row">
            <span class="b-dot" style:background={r.s.color}></span>
            <span class="b-label">{r.s.label}</span>
            <span class="b-val num">{fmtVal(r.s, r.v)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .chart {
    position: relative;
    width: 100%;
    user-select: none;
    -webkit-user-select: none;
  }
  .svg {
    display: block;
    overflow: visible;
    cursor: crosshair;
    border-radius: var(--r-control);
    touch-action: none;
  }
  .svg:focus-visible {
    box-shadow: var(--focus-ring);
  }
  .dragging .svg {
    cursor: grabbing;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--t3);
  }
  .grid {
    stroke: rgba(255, 255, 255, 0.055);
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .baseline {
    stroke: rgba(255, 255, 255, 0.1);
    shape-rendering: crispEdges;
  }
  .axis {
    fill: var(--t3);
    font-size: 11px;
    font-weight: 500;
  }
  .annot {
    stroke: rgba(255, 255, 255, 0.22);
    stroke-dasharray: 1 4;
    stroke-linecap: round;
  }
  .end-label .annot-label {
    fill: var(--t3);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.01em;
  }
  .stream {
    will-change: transform;
  }
  .end-dot {
    stroke: var(--bg-0);
    stroke-width: 2;
  }
  .end-label {
    transition: transform 380ms var(--ease-out-expo);
  }
  .end-label text {
    font-size: 11px;
    font-weight: 600;
  }
  .cursor line {
    stroke: rgba(255, 255, 255, 0.5);
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .cursor.scrub line {
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .bubble {
    position: absolute;
    left: 0;
    top: 0;
    min-width: 148px;
    padding: 8px 10px;
    border-radius: 10px;
    border: 1px solid var(--hair-strong);
    box-shadow: var(--sh-2);
    pointer-events: none;
    z-index: 2;
    background: rgba(20, 22, 30, 0.82);
  }
  .b-time {
    font-size: 11px;
    font-weight: 600;
    color: var(--t2);
    margin-bottom: 4px;
    display: flex;
    gap: 6px;
  }
  .b-off {
    color: var(--accent);
  }
  .b-row {
    display: grid;
    grid-template-columns: 8px 1fr auto;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    line-height: 18px;
  }
  .b-dot {
    width: 8px;
    height: 8px;
    border-radius: 3px;
  }
  .b-label {
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
  }
  .b-val {
    font-weight: 600;
    color: var(--t1);
  }
</style>
