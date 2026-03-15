<script lang="ts">
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';

  interface Props {
    label: string;
    value: number;
    max?: number;
    history?: number[];
  }

  let { label, value, max = 100, history = [] }: Props = $props();

  const tweened = new Tween(0, { duration: 400, easing: cubicOut });
  $effect(() => { tweened.target = value; });

  function tempColor(temp: number): string {
    if (temp < 40) return 'var(--temp-cold)';
    if (temp < 60) return `color-mix(in srgb, var(--temp-cold) ${Math.round((60 - temp) / 20 * 100)}%, var(--temp-warm))`;
    if (temp < 80) return `color-mix(in srgb, var(--temp-warm) ${Math.round((80 - temp) / 20 * 100)}%, var(--temp-hot))`;
    return 'var(--temp-hot)';
  }

  // Sparkline from history (last 30 points)
  const sparkPoints = $derived.by(() => {
    const pts = history.slice(-30);
    if (pts.length < 2) return '';
    const w = 100;
    const h = 16;
    const minV = Math.min(...pts) - 2;
    const maxV = Math.max(...pts) + 2;
    const range = maxV - minV || 1;
    return pts
      .map((v, i) => `${(i / (pts.length - 1)) * w},${h - ((v - minV) / range) * h}`)
      .join(' ');
  });
</script>

<div class="temp-card">
  <div class="top-row">
    <span class="label">{label}</span>
    <span class="value tabular-nums" style="color: {tempColor(tweened.current)}">{tweened.current.toFixed(1)}°C</span>
  </div>
  <div class="bar-container">
    <div
      class="bar-fill"
      style="width: {Math.min((tweened.current / max) * 100, 100)}%; background: {tempColor(tweened.current)}"
    ></div>
  </div>
  {#if sparkPoints.length > 0}
    <svg viewBox="0 0 100 16" class="sparkline" preserveAspectRatio="none">
      <polyline
        points={sparkPoints}
        fill="none"
        stroke={tempColor(tweened.current)}
        stroke-width="1.5"
        stroke-linejoin="round"
        stroke-linecap="round"
        opacity="0.4"
      />
    </svg>
  {/if}
</div>

<style>
  .temp-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .top-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .value {
    font-size: 18px;
    font-weight: 700;
  }
  .bar-container {
    width: 100%;
    height: 10px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 5px;
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    border-radius: 5px;
    transition: width 0.4s cubic-bezier(0.25, 0.1, 0.25, 1);
  }
  .sparkline {
    width: 100%;
    height: 16px;
    display: block;
  }
</style>
