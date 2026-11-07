<!--
  Sparkline: small inline chart with gradient fill and an end dot. While the
  app is being scrubbed it shows a marker at the scrubbed timestamp.
-->
<script lang="ts">
  import { linear, monotoneArea, monotonePath, nearestIndex, type Pt } from './monotone';
  import { scrub } from '../stores/scrub.svelte';

  interface Props {
    /** [timestamp_ms, value] pairs, oldest first */
    data: [number, number][];
    color: string;
    height?: number;
    /** fixed y domain; auto (padded) when omitted */
    min?: number;
    max?: number;
    fill?: boolean;
    /** accessible description; decorative when omitted */
    label?: string;
  }
  let { data, color, height = 36, min, max, fill = true, label }: Props = $props();

  let width = $state(0);
  const uid = `sp-${Math.random().toString(36).slice(2, 8)}`;
  const P = 4; // inner padding for the dot

  const geo = $derived.by(() => {
    if (data.length < 2 || width <= 0) return null;
    let lo = Infinity,
      hi = -Infinity;
    for (const [, v] of data) {
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    const span = Math.max(hi - lo, 1);
    const y0 = min ?? lo - span * 0.18;
    const y1 = max ?? hi + span * 0.18;
    const x = linear([data[0][0], data[data.length - 1][0]], [P, width - P]);
    const y = linear([y0, y1], [height - P, P]);
    const pts: Pt[] = data.map(([t, v]) => [x(t), y(v)]);
    return { x, y, pts, line: monotonePath(pts), area: fill ? monotoneArea(pts, height) : '' };
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
</script>

<div class="spark" style:height="{height}px" bind:clientWidth={width}>
  {#if geo}
    <svg {width} {height} role={label ? 'img' : undefined} aria-label={label} aria-hidden={label ? undefined : 'true'}>
      <defs>
        <linearGradient id="{uid}-g" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color={color} stop-opacity="0.3" />
          <stop offset="100%" stop-color={color} stop-opacity="0" />
        </linearGradient>
      </defs>
      {#if geo.area}<path d={geo.area} fill="url(#{uid}-g)" />{/if}
      <path
        d={geo.line}
        fill="none"
        stroke={color}
        stroke-width="1.75"
        stroke-linecap="round"
        stroke-linejoin="round"
        opacity={marker ? 0.55 : 1}
      />
      {#if marker}
        <line class="mline" x1={marker[0]} x2={marker[0]} y1="0" y2={height} />
        <circle cx={marker[0]} cy={marker[1]} r="3.5" fill={color} stroke="var(--bg-0)" stroke-width="1.5" />
      {:else if last}
        <circle cx={last[0]} cy={last[1]} r="3" fill={color} stroke="var(--bg-0)" stroke-width="1.5" />
      {/if}
    </svg>
  {/if}
</div>

<style>
  .spark {
    width: 100%;
    min-width: 0;
  }
  svg {
    display: block;
    overflow: visible;
  }
  .mline {
    stroke: var(--accent);
    stroke-width: 1;
    opacity: 0.7;
    shape-rendering: crispEdges;
  }
</style>
