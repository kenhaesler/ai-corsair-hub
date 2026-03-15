<script lang="ts">
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';

  interface Props {
    name: string;
    duty: number;
    dutyHistory?: number[];
    avgRpm?: number | null;
  }

  let { name, duty, dutyHistory = [], avgRpm = null }: Props = $props();

  const tweened = new Tween(0, { duration: 400, easing: cubicOut });
  $effect(() => { tweened.target = duty; });

  // Sparkline from duty history (last 30 points)
  const sparkPoints = $derived.by(() => {
    const pts = dutyHistory.slice(-30);
    if (pts.length < 2) return '';
    const w = 100;
    const h = 16;
    const minV = Math.max(0, Math.min(...pts) - 2);
    const maxV = Math.min(100, Math.max(...pts) + 2);
    const range = maxV - minV || 1;
    return pts
      .map((v, i) => `${(i / (pts.length - 1)) * w},${h - ((v - minV) / range) * h}`)
      .join(' ');
  });
</script>

<div class="fan-card">
  <div class="top-row">
    <span class="name">{name}</span>
    <div class="values">
      <span class="duty tabular-nums">{Math.round(tweened.current)}%</span>
      {#if avgRpm !== null && avgRpm !== undefined}
        <span class="rpm tabular-nums">~{Math.round(avgRpm)} RPM</span>
      {/if}
    </div>
  </div>
  <div class="bar-container">
    <div
      class="bar-fill"
      style="width: {tweened.current}%; background: var(--accent)"
    ></div>
  </div>
  {#if sparkPoints.length > 0}
    <svg viewBox="0 0 100 16" class="sparkline" preserveAspectRatio="none">
      <polyline
        points={sparkPoints}
        fill="none"
        stroke="var(--accent)"
        stroke-width="1.5"
        stroke-linejoin="round"
        stroke-linecap="round"
        opacity="0.35"
      />
    </svg>
  {/if}
</div>

<style>
  .fan-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .top-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .name {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .values {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-shrink: 0;
  }
  .duty {
    font-size: 18px;
    font-weight: 700;
    color: var(--accent);
  }
  .rpm {
    font-size: 10px;
    color: var(--text-muted);
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
