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
            <!-- Wrap each temp in a cell so GPU metrics render
                 directly beneath the GPU card rather than under
                 the full row — this keeps "GPU" information
                 visually grouped in one column. -->
            <div class="temp-cell">
              <TempCard
                label={temp.source.toUpperCase()}
                value={temp.celsius}
                history={tempHistory(temp.source)}
              />
              {#if temp.source === 'cpu' && data.cpu_metrics}
                {@const c = data.cpu_metrics}
                <!-- Single-row sensor strip: label + bar + value + metrics
                     all inline so the Temperatures box doesn't grow too
                     tall and push the History chart off the bottom. -->
                <div class="sensor-extras" title="CPU · {c.load_pct.toFixed(0)}% load · {c.power_watts.toFixed(0)} W · {c.clock_mhz} MHz">
                  <span class="mini-label">LOAD</span>
                  <div class="util-bar">
                    <div class="util-fill util-fill-cpu" style="width: {Math.min(c.load_pct, 100)}%"></div>
                  </div>
                  <span class="util-value util-value-cpu tabular-nums">{c.load_pct.toFixed(0)}%</span>
                  <span class="dot"></span>
                  <span class="metric tabular-nums">{c.power_watts.toFixed(0)} W</span>
                  <span class="metric tabular-nums">{c.clock_mhz} MHz</span>
                </div>
              {/if}
              {#if temp.source === 'gpu' && data.gpu_metrics}
                {@const g = data.gpu_metrics}
                <div class="sensor-extras" title="GPU · {g.utilization_pct}% util · {g.power_watts.toFixed(0)} W · {g.clock_mhz} MHz · {(g.memory_used_mb / 1024).toFixed(1)} GB VRAM">
                  <span class="mini-label">UTIL</span>
                  <div class="util-bar">
                    <div class="util-fill" style="width: {g.utilization_pct}%"></div>
                  </div>
                  <span class="util-value tabular-nums">{g.utilization_pct}%</span>
                  <span class="dot"></span>
                  <span class="metric tabular-nums">{g.power_watts.toFixed(0)} W</span>
                  <span class="metric tabular-nums">{g.clock_mhz} MHz</span>
                  <span class="metric tabular-nums">{(g.memory_used_mb / 1024).toFixed(1)} GB</span>
                </div>
              {/if}
            </div>
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
    gap: 10px;
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
  /* Temp grid: auto-fit with 180px min so CPU and GPU stretch
     to ~50% each. align-items: start stops CPU stretching to
     match GPU's taller cell (which includes the extras block). */
  .temp-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 8px;
    align-items: start;
  }
  /* Fan grid keeps 130px min so the owner's typical 4 fan groups
     (top/rear/side/bottom) all fit on one row in the ~620px
     panel without wrapping. */
  .fan-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 8px;
  }
  .temp-cell {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  /* Shared CPU / GPU sensor-extras strip — stacked directly under
     each sensor's temp card so related info stays in one column.
     Single compact row: label + bar + value + inline metrics. */
  .sensor-extras {
    padding: 5px 8px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .mini-label {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .util-bar {
    flex: 1 1 60px;
    min-width: 50px;
    height: 5px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 3px;
    overflow: hidden;
  }
  .util-fill {
    height: 100%;
    background: linear-gradient(to right, var(--accent), #c084fc);
    transition: width 0.25s ease;
  }
  /* CPU uses the same amber → red gradient the CPU temp bar
     already runs on so a high load visually echoes a rising
     temp (without literally colour-mixing two different scales). */
  .util-fill-cpu {
    background: linear-gradient(to right, #fbbf24, #f87171);
  }
  .util-value {
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    min-width: 32px;
    text-align: right;
  }
  .util-value-cpu {
    color: #fbbf24;
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
  }
  .metric {
    font-size: 10px;
    color: var(--text-secondary);
    white-space: nowrap;
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
