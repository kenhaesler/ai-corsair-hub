<script lang="ts">
  import { sensors } from '../../lib/stores/sensors.svelte';
  import TempCard from './TempCard.svelte';
  import FanCard from './FanCard.svelte';
  import PsuPanel from './PsuPanel.svelte';
  import MiniChart from './MiniChart.svelte';

  const data = $derived(sensors.snapshot);
</script>

<div class="dashboard scroll-area">
  {#if sensors.hwError}
    <div class="banner error">{sensors.hwError}</div>
  {/if}

  {#if data?.emergency}
    <div class="banner error">EMERGENCY: Critical temperature detected — all fans at 100%</div>
  {/if}

  {#if data?.any_stale}
    <div class="banner">Sensor data stale — failsafe mode active (70%)</div>
  {/if}

  {#if data}
    <div class="grid-2">
      <section class="card">
        <h3 class="section-title">Temperatures</h3>
        <div class="temp-list">
          {#each data.temperatures as temp}
            <TempCard label={temp.source.toUpperCase()} value={temp.celsius} />
          {/each}
          {#if data.temperatures.length === 0}
            <p class="empty">No temperature sources available</p>
          {/if}
        </div>
      </section>

      <section class="card">
        <h3 class="section-title">Fan Groups</h3>
        <div class="fan-list">
          {#each data.group_duties as group}
            <FanCard name={group.name} duty={group.duty_percent} />
          {/each}
          {#if data.group_duties.length === 0}
            <p class="empty">No fan groups configured</p>
          {/if}
        </div>
      </section>
    </div>

    {#if data.psu}
      <section class="card" style="margin-top: 12px">
        <PsuPanel psu={data.psu} />
      </section>
    {/if}

    <section class="card" style="margin-top: 12px">
      <h3 class="section-title">History (60s)</h3>
      <MiniChart />
    </section>
  {:else}
    <div class="loading">
      <p>Connecting to hardware...</p>
    </div>
  {/if}
</div>

<style>
  .dashboard {
    display: flex;
    flex-direction: column;
  }
  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-secondary);
    margin: 0 0 12px 0;
  }
  .temp-list, .fan-list {
    display: flex;
    flex-direction: column;
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
</style>
