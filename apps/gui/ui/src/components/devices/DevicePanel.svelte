<script lang="ts">
  import { getDevices, rgbIdentifyChannels, type ChannelIdentity } from '../../lib/api';
  import type { DeviceTree } from '../../lib/types';
  import HubDetail from './HubDetail.svelte';
  import { loadConfig, configStore } from '../../lib/stores/config.svelte';
  import { onMount } from 'svelte';

  let devices = $state<DeviceTree | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  // Channel-identify diagnostic state. When active, every enumerated
  // RGB device is painted a unique colour for IDENTIFY_SECS seconds;
  // we render a legend here so the user can correlate what they
  // physically see with the channel numbers the firmware reports.
  const IDENTIFY_SECS = 20;
  let identifyMap = $state<ChannelIdentity[] | null>(null);
  let identifyRunning = $state(false);
  let identifyDeadline = $state<number | null>(null);
  let identifyRemaining = $state(0);

  $effect(() => {
    if (!identifyRunning || identifyDeadline == null) return;
    const tick = () => {
      const left = Math.max(0, Math.ceil((identifyDeadline! - Date.now()) / 1000));
      identifyRemaining = left;
      if (left <= 0) {
        identifyRunning = false;
        identifyDeadline = null;
      }
    };
    tick();
    const handle = setInterval(tick, 500);
    return () => clearInterval(handle);
  });

  async function runIdentify() {
    if (identifyRunning) return;
    try {
      const mapping = await rgbIdentifyChannels(IDENTIFY_SECS);
      identifyMap = mapping;
      identifyRunning = true;
      identifyDeadline = Date.now() + IDENTIFY_SECS * 1000;
      identifyRemaining = IDENTIFY_SECS;
    } catch (e) {
      error = `Identify failed: ${String(e)}`;
    }
  }

  onMount(async () => {
    // Config is needed so HubDetail can render user-set friendly names in
    // the Name column. loadConfig is a no-op if the config is already in
    // the store.
    if (!configStore.config) {
      await loadConfig();
    }
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

  function shortSerial(s: string): string {
    return s.length > 8 ? s.slice(-4) : s;
  }

  /** Group identify rows by hub serial for legend rendering. */
  function groupByHub(rows: ChannelIdentity[]): { serial: string; rows: ChannelIdentity[] }[] {
    const byHub = new Map<string, ChannelIdentity[]>();
    for (const r of rows) {
      if (!byHub.has(r.hub_serial)) byHub.set(r.hub_serial, []);
      byHub.get(r.hub_serial)!.push(r);
    }
    return Array.from(byHub.entries()).map(([serial, rows]) => ({
      serial,
      rows: rows.slice().sort((a, b) => a.channel - b.channel),
    }));
  }
</script>

<div class="device-panel scroll-area">
  <div class="header-row">
    <h3 class="section-title">USB Devices</h3>
    <div class="header-actions">
      <button class="identify-btn" onclick={runIdentify} disabled={identifyRunning}>
        {identifyRunning ? `Identifying… ${identifyRemaining}s` : 'Identify Channels'}
      </button>
      <button onclick={refresh} disabled={loading}>
        {loading ? 'Scanning...' : 'Refresh'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="banner error">{error}</div>
  {/if}

  {#if identifyMap && identifyRunning}
    <div class="identify-legend">
      <p class="legend-intro">
        Every device now lights up a unique colour for {IDENTIFY_SECS} seconds.
        Look at your rig and match each physical fan/strip to the colour below
        — that tells you which channel number the firmware actually assigned.
      </p>
      {#each groupByHub(identifyMap) as hubGroup}
        <div class="legend-hub">
          <span class="legend-hub-title">Hub {shortSerial(hubGroup.serial)}</span>
          <div class="legend-rows">
            {#each hubGroup.rows as row}
              <div class="legend-row">
                <span
                  class="legend-swatch"
                  style="background: rgb({row.rgb[0]},{row.rgb[1]},{row.rgb[2]})"
                ></span>
                <span class="legend-ch">Ch {row.channel}</span>
                <span class="legend-type">{row.device_type}</span>
                <span class="legend-color-name">{row.color_name}</span>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
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
  .header-actions {
    display: flex;
    gap: 6px;
  }
  .identify-btn {
    background: var(--accent);
    color: var(--bg-base);
    border: 1px solid var(--accent);
    font-weight: 600;
  }
  .identify-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .identify-legend {
    margin-bottom: 16px;
    padding: 12px 16px;
    background: var(--bg-card-inset);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius);
  }
  .legend-intro {
    margin: 0 0 12px;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.4;
  }
  .legend-hub {
    margin-bottom: 10px;
  }
  .legend-hub:last-child {
    margin-bottom: 0;
  }
  .legend-hub-title {
    display: block;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--text-muted);
    margin-bottom: 6px;
  }
  .legend-rows {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 4px 16px;
  }
  .legend-row {
    display: grid;
    grid-template-columns: 18px 40px 1fr auto;
    gap: 8px;
    align-items: center;
    font-size: 12px;
    color: var(--text-primary);
  }
  .legend-swatch {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.2);
  }
  .legend-ch {
    font-weight: 600;
    font-family: var(--font-mono);
  }
  .legend-type {
    color: var(--text-muted);
    font-size: 11px;
  }
  .legend-color-name {
    color: var(--text-secondary);
    font-size: 11px;
    text-align: right;
  }
</style>
