<script lang="ts">
  import type { PsuSnapshot } from '../../lib/types';

  interface Props {
    psu: PsuSnapshot;
  }

  let { psu }: Props = $props();
</script>

<h3 class="section-title">Power Supply</h3>
<div class="psu-grid">
  <div class="psu-summary">
    <div class="stat">
      <span class="stat-label">Total</span>
      <span class="stat-value tabular-nums">{psu.total_power.toFixed(0)}W</span>
    </div>
    <div class="stat">
      <span class="stat-label">Input</span>
      <span class="stat-value tabular-nums">{psu.input_voltage.toFixed(0)}V</span>
    </div>
    <div class="stat">
      <span class="stat-label">Fan</span>
      <span class="stat-value tabular-nums">{psu.fan_rpm} RPM</span>
    </div>
    <div class="stat">
      <span class="stat-label">VRM</span>
      <span class="stat-value tabular-nums">{psu.temp_vrm.toFixed(1)}°C</span>
    </div>
    <div class="stat">
      <span class="stat-label">Case</span>
      <span class="stat-value tabular-nums">{psu.temp_case.toFixed(1)}°C</span>
    </div>
  </div>
  <div class="rails">
    {#each psu.rails as rail}
      <div class="rail">
        <span class="rail-name">{rail.name}</span>
        <span class="rail-val tabular-nums">{rail.voltage.toFixed(2)}V</span>
        <span class="rail-val tabular-nums">{rail.current.toFixed(1)}A</span>
        <span class="rail-val tabular-nums">{rail.power.toFixed(0)}W</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-secondary);
    margin: 0 0 12px 0;
  }
  .psu-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 16px;
  }
  .psu-summary {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .stat {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .stat-label {
    font-size: 11px;
    color: var(--text-muted);
    width: 40px;
  }
  .stat-value {
    font-size: 13px;
    font-weight: 500;
  }
  .rails {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rail {
    display: grid;
    grid-template-columns: 48px 72px 64px 56px;
    gap: 8px;
    align-items: baseline;
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
