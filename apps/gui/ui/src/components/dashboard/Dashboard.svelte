<script lang="ts">
  import { sensors } from '../../lib/stores/sensors.svelte';
  import TempCard from './TempCard.svelte';
  import FanCard from './FanCard.svelte';
  import PsuPanel from './PsuPanel.svelte';
  import MiniChart from './MiniChart.svelte';
  import PresetStrip from './PresetStrip.svelte';

  const data = $derived(sensors.snapshot);

  // Extract sparkline history for each temp sensor (last 30 points)
  function tempHistory(source: string): number[] {
    return sensors.history.slice(-30).map(snap => {
      const t = snap.temperatures.find(t => t.source === source);
      return t ? t.celsius : 0;
    }).filter(v => v > 0);
  }

  // Extract duty history for each fan group (last 30 points)
  function dutyHistory(groupName: string): number[] {
    return sensors.history.slice(-30).map(snap => {
      const g = snap.group_duties.find(g => g.name === groupName);
      return g ? g.duty_percent : -1;
    }).filter(v => v >= 0);
  }

  // Average RPM for a fan group
  function groupAvgRpm(groupName: string): number | null {
    if (!data) return null;
    const fans = data.fans.filter(f => f.group_name === groupName);
    if (fans.length === 0) return null;
    return fans.reduce((sum, f) => sum + f.rpm, 0) / fans.length;
  }

  // Status bar data
  const maxTemp = $derived(
    data ? Math.max(0, ...data.temperatures.map(t => t.celsius)) : 0
  );
  const statusDot = $derived(
    maxTemp > 85 ? 'var(--danger)' : maxTemp > 70 ? 'var(--warning)' : 'var(--success)'
  );
  const avgDuty = $derived.by(() => {
    if (!data || data.group_duties.length === 0) return 0;
    return Math.round(data.group_duties.reduce((s, g) => s + g.duty_percent, 0) / data.group_duties.length);
  });

  // PSU power history for sparkline
  function psuPowerHistory(): number[] {
    return sensors.history.slice(-30).map(snap => snap.psu?.total_power ?? -1).filter(v => v >= 0);
  }
</script>

<div class="dashboard">
  {#if sensors.hwError}
    <div class="banner error banner-overlay">{sensors.hwError}</div>
  {/if}

  {#if data?.emergency}
    <div class="banner error banner-overlay">EMERGENCY: Critical temperature detected — all fans at 100%</div>
  {/if}

  {#if data?.any_stale}
    <div class="banner banner-overlay">Sensor data stale — failsafe mode active (70%)</div>
  {/if}

  {#each sensors.hubStatus as hub}
    {#if hub.state === 'lost'}
      <div class="banner warning banner-overlay">Hub {hub.serial.slice(0, 8)}... lost — attempting recovery...</div>
    {:else if hub.state === 'recovered'}
      <div class="banner success banner-overlay">Hub {hub.serial.slice(0, 8)}... recovered</div>
    {/if}
  {/each}

  {#if data}
    <div class="dash-grid">
      <!-- Row 1: Temperatures + Fan Groups -->
      <section class="card">
        <h3 class="section-title">Temperatures</h3>
        <div class="temp-grid">
          {#each data.temperatures as temp}
            <TempCard
              label={temp.source.toUpperCase()}
              value={temp.celsius}
              history={tempHistory(temp.source)}
            />
          {/each}
          {#if data.temperatures.length === 0}
            <p class="empty">No temperature sources available</p>
          {/if}
        </div>
      </section>

      <section class="card">
        <h3 class="section-title">Fan Groups</h3>
        <div class="fan-grid">
          {#each data.group_duties as group}
            <FanCard
              name={group.name}
              duty={group.duty_percent}
              dutyHistory={dutyHistory(group.name)}
              avgRpm={groupAvgRpm(group.name)}
            />
          {/each}
          {#if data.group_duties.length === 0}
            <p class="empty">No fan groups configured</p>
          {/if}
        </div>
      </section>

      <!-- Row 2: Quick Presets (full width) -->
      <section class="card presets-row">
        <h3 class="section-title">Quick Presets</h3>
        <PresetStrip />
      </section>

      <!-- Row 3: PSU panel (if present) -->
      {#if data.psu}
        <section class="card psu-row">
          <PsuPanel psu={data.psu} powerHistory={psuPowerHistory()} />
        </section>
      {/if}

      <!-- Row 4: History chart (fills remaining space) -->
      <section class="card chart-row">
        <h3 class="section-title">History (60s)</h3>
        <div class="chart-wrap">
          <MiniChart />
        </div>
      </section>

      <!-- Row 5: Status bar -->
      <div class="status-bar">
        <span class="status-dot" style="background: {statusDot}"></span>
        {#each data.temperatures as temp}
          <span class="status-item">{temp.source.toUpperCase()} {temp.celsius.toFixed(1)}°C</span>
        {/each}
        <span class="status-item">{data.group_duties.length} groups @ {avgDuty}%</span>
        {#if data.psu}
          <span class="status-item">{data.psu.total_power.toFixed(0)}W</span>
        {/if}
      </div>
    </div>
  {:else}
    <div class="loading">
      <p>Connecting to hardware...</p>
    </div>
  {/if}
</div>

<style>
  .dashboard {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 16px;
    position: relative;
  }
  .banner-overlay {
    flex-shrink: 0;
  }
  .dash-grid {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: auto auto auto 1fr auto;
    gap: 12px;
    min-height: 0;
  }
  .presets-row {
    grid-column: 1 / -1;
  }
  .psu-row {
    grid-column: 1 / -1;
  }
  .chart-row {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  .chart-wrap {
    flex: 1;
    min-height: 0;
  }
  .temp-grid, .fan-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 8px;
  }
  .empty {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0;
  }
  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-secondary);
  }

  /* Status bar */
  .status-bar {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 12px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 11px;
    color: var(--text-muted);
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    animation: pulse 2s infinite;
  }
  .status-item {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
