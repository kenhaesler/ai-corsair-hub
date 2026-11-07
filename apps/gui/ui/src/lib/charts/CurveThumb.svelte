<!--
  CurveThumb: fan-curve thumbnail normalised to the curve's own temperature
  range (so a 28–45 °C preset fills the box instead of hugging the left).
  Duty axis is always 0–100 %. Rich gradient fill, faint grid and baseline,
  soft glow on the line.
-->
<script lang="ts">
  import type { CurvePoint } from '../types';

  interface Props {
    points: CurvePoint[];
    color?: string;
    height?: number;
    /** optional live marker temperature (°C) */
    marker?: number | null;
  }
  let { points, color = 'var(--accent)', height = 40, marker = null }: Props = $props();

  const uid = `ct-${Math.random().toString(36).slice(2, 8)}`;
  const W = 100;
  const H = 40;
  const TOP = 3;
  const BOT = 37;

  const domain = $derived.by(() => {
    const temps = points.map((p) => p.temp);
    const tMin = Math.min(...temps);
    const tMax = Math.max(...temps);
    return { tMin, range: Math.max(tMax - tMin, 1) };
  });
  const px = (t: number) => ((t - domain.tMin) / domain.range) * W;
  const py = (d: number) => BOT - (d / 100) * (BOT - TOP);
  const line = $derived(points.map((p, i) => `${i === 0 ? 'M' : 'L'}${px(p.temp)},${py(p.duty)}`).join(' '));
  const area = $derived(points.length ? `${line} L${W},${BOT} L0,${BOT} Z` : '');
</script>

<svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" style:height="{height}px" aria-hidden="true">
  <defs>
    <linearGradient id="{uid}-g" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color={color} stop-opacity="0.5" />
      <stop offset="55%" stop-color={color} stop-opacity="0.16" />
      <stop offset="100%" stop-color={color} stop-opacity="0.02" />
    </linearGradient>
  </defs>
  {#each [0.25, 0.5, 0.75] as g (g)}
    <line x1="0" x2={W} y1={TOP + (BOT - TOP) * g} y2={TOP + (BOT - TOP) * g} class="grid" vector-effect="non-scaling-stroke" />
  {/each}
  {#each [0.25, 0.5, 0.75] as g (g)}
    <line x1={W * g} x2={W * g} y1={TOP} y2={BOT} class="grid v" vector-effect="non-scaling-stroke" />
  {/each}
  <line x1="0" x2={W} y1={BOT} y2={BOT} class="base" vector-effect="non-scaling-stroke" />
  <path d={area} fill="url(#{uid}-g)" />
  <path
    d={line}
    fill="none"
    stroke={color}
    stroke-width="5"
    stroke-opacity="0.18"
    stroke-linejoin="round"
    stroke-linecap="round"
    vector-effect="non-scaling-stroke"
  />
  <path
    d={line}
    fill="none"
    stroke={color}
    stroke-width="2"
    stroke-linejoin="round"
    stroke-linecap="round"
    vector-effect="non-scaling-stroke"
  />
  {#if marker !== null && marker >= domain.tMin && marker <= domain.tMin + domain.range}
    <line x1={px(marker)} x2={px(marker)} y1="0" y2={H} class="mk" vector-effect="non-scaling-stroke" />
  {/if}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    overflow: visible;
  }
  .grid {
    stroke: rgba(255, 255, 255, 0.06);
    stroke-width: 1;
  }
  .grid.v {
    stroke: rgba(255, 255, 255, 0.035);
  }
  .base {
    stroke: rgba(255, 255, 255, 0.16);
    stroke-width: 1;
  }
  .mk {
    stroke: rgba(255, 255, 255, 0.45);
    stroke-width: 1;
    stroke-dasharray: 2 3;
  }
</style>
