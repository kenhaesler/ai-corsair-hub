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

  <!-- Rails inline, to the right of the hero+chips. Each pill packs
       all four values (name / power / voltage / current) so the
       previously-empty ~60% of the row now carries the full rail
       breakdown without growing the section's height. -->
  <div class="rails-row">
    {#each psu.rails as rail}
      <div class="rail-pill">
        <span class="rail-name">{rail.name}</span>
        <span class="rail-power tabular-nums">{rail.power.toFixed(0)}W</span>
        <span class="rail-sub tabular-nums">{rail.voltage.toFixed(2)}V</span>
        <span class="rail-sub tabular-nums">{rail.current.toFixed(1)}A</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .psu-layout {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
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
  /* Rails row — 3 compact pills side-by-side. Each pill is
     2×2: rail name + big power on top, voltage + current on
     the line below. Fills the horizontal space to the right
     of the chips that was previously dead-empty. */
  .rails-row {
    display: flex;
    gap: 8px;
    margin-left: auto; /* push the pills to the right edge */
    flex-wrap: wrap;
  }
  .rail-pill {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 10px;
    align-items: baseline;
    padding: 5px 12px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    min-width: 120px;
  }
  .rail-name {
    font-size: 11px;
    font-weight: 700;
    color: var(--text-secondary);
  }
  .rail-power {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
    text-align: right;
  }
  .rail-sub {
    font-size: 10px;
    color: var(--text-muted);
  }
  .rail-sub:last-child {
    text-align: right;
  }
</style>
