<script lang="ts">
  import type { PsuSnapshot } from '../../lib/types';

  interface Props {
    psu: PsuSnapshot;
    powerHistory?: number[];
  }

  let { psu, powerHistory = [] }: Props = $props();

  // Power sparkline
  const sparkPoints = $derived.by(() => {
    const pts = powerHistory.slice(-30);
    if (pts.length < 2) return '';
    const w = 60;
    const h = 20;
    const minV = Math.max(0, Math.min(...pts) - 20);
    const maxV = Math.max(...pts) + 20;
    const range = maxV - minV || 1;
    return pts
      .map((v, i) => `${(i / (pts.length - 1)) * w},${h - ((v - minV) / range) * h}`)
      .join(' ');
  });
</script>

<h3 class="section-title">Power Supply</h3>
<div class="psu-layout">
  <div class="hero-power">
    <span class="power-value tabular-nums">{psu.total_power.toFixed(0)}</span>
    <span class="power-unit">W</span>
    {#if sparkPoints.length > 0}
      <svg viewBox="0 0 60 20" class="power-spark" preserveAspectRatio="none">
        <polyline
          points={sparkPoints}
          fill="none"
          stroke="var(--warning)"
          stroke-width="1.5"
          stroke-linejoin="round"
          stroke-linecap="round"
          opacity="0.5"
        />
      </svg>
    {/if}
  </div>

  <div class="stat-chips">
    <div class="chip">
      <span class="chip-label">Input</span>
      <span class="chip-value tabular-nums">{psu.input_voltage.toFixed(0)}V</span>
    </div>
    <div class="chip">
      <span class="chip-label">Fan</span>
      <span class="chip-value tabular-nums">{psu.fan_rpm} RPM</span>
    </div>
    <div class="chip">
      <span class="chip-label">VRM</span>
      <span class="chip-value tabular-nums">{psu.temp_vrm.toFixed(1)}°C</span>
    </div>
    <div class="chip">
      <span class="chip-label">Case</span>
      <span class="chip-value tabular-nums">{psu.temp_case.toFixed(1)}°C</span>
    </div>
  </div>
</div>

<div class="rails">
  {#each psu.rails as rail, i}
    <div class="rail" class:alt={i % 2 === 1}>
      <span class="rail-name">{rail.name}</span>
      <span class="rail-val tabular-nums">{rail.voltage.toFixed(2)}V</span>
      <span class="rail-val tabular-nums">{rail.current.toFixed(1)}A</span>
      <span class="rail-val tabular-nums">{rail.power.toFixed(0)}W</span>
    </div>
  {/each}
</div>

<style>
  .psu-layout {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 12px;
  }
  .hero-power {
    display: flex;
    align-items: baseline;
    gap: 4px;
    flex-shrink: 0;
  }
  .power-value {
    font-size: 28px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .power-unit {
    font-size: 14px;
    color: var(--text-muted);
    margin-right: 8px;
  }
  .power-spark {
    width: 60px;
    height: 20px;
    align-self: center;
  }
  .stat-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 4px 10px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .chip-label {
    font-size: 10px;
    color: var(--text-muted);
  }
  .chip-value {
    font-size: 12px;
    font-weight: 500;
  }
  .rails {
    display: flex;
    flex-direction: column;
    gap: 0;
  }
  .rail {
    display: grid;
    grid-template-columns: 48px 72px 64px 56px;
    gap: 8px;
    align-items: baseline;
    padding: 4px 8px;
    border-radius: 4px;
  }
  .rail.alt {
    background: rgba(255, 255, 255, 0.02);
  }
  .rail-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .rail-val {
    font-size: 12px;
    text-align: right;
    color: var(--text-primary);
  }
</style>
