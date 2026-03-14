<script lang="ts">
  import { getDevices } from '../../lib/api';
  import type { DeviceTree } from '../../lib/types';
  import HubDetail from './HubDetail.svelte';
  import { onMount } from 'svelte';

  let devices = $state<DeviceTree | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  onMount(async () => {
    await refresh();
  });

  async function refresh() {
    loading = true;
    error = null;
    try {
      devices = await getDevices();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="device-panel scroll-area">
  <div class="header-row">
    <h3 class="section-title">USB Devices</h3>
    <button onclick={refresh} disabled={loading}>
      {loading ? 'Scanning...' : 'Refresh'}
    </button>
  </div>

  {#if error}
    <div class="banner error">{error}</div>
  {/if}

  {#if devices}
    {#each devices.hubs as hub}
      <div class="card" style="margin-bottom: 12px">
        <HubDetail {hub} />
      </div>
    {/each}

    {#if devices.psu}
      <div class="card">
        <h4 class="psu-title">Corsair HX1500i</h4>
        <div class="psu-info">
          <span class="label">Serial</span>
          <span class="mono">{devices.psu.serial}</span>
        </div>
      </div>
    {/if}

    {#if devices.hubs.length === 0 && !devices.psu}
      <div class="card empty">
        <p>No Corsair devices detected on USB bus.</p>
      </div>
    {/if}
  {:else if !loading}
    <div class="card empty">
      <p>Unable to scan devices.</p>
    </div>
  {/if}
</div>

<style>
  .device-panel {
    display: flex;
    flex-direction: column;
  }
  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-secondary);
    margin: 0;
  }
  .psu-title {
    margin: 0 0 8px 0;
    font-size: 14px;
    font-weight: 600;
  }
  .psu-info {
    display: flex;
    gap: 8px;
    font-size: 12px;
  }
  .label {
    color: var(--text-muted);
  }
  .empty {
    text-align: center;
    padding: 40px;
    color: var(--text-secondary);
  }
</style>
