<script lang="ts">
  import type { RgbZoneConfig, RgbDeviceRef, HubSnapshot } from '../../lib/types';

  interface Props {
    zones: RgbZoneConfig[];
    selectedZone: number;
    availableHubs: HubSnapshot[];
    onselect: (index: number) => void;
    onadd: () => void;
    ondelete: (index: number) => void;
    onzoneupdate: (zone: RgbZoneConfig) => void;
  }

  let { zones, selectedZone, availableHubs, onselect, onadd, ondelete, onzoneupdate }: Props = $props();

  let editingName = $state<number | null>(null);
  let editValue = $state('');

  function startRename(index: number) {
    editingName = index;
    editValue = zones[index].name;
  }

  function finishRename(index: number) {
    if (editValue.trim()) {
      zones[index].name = editValue.trim();
      onzoneupdate(zones[index]);
    }
    editingName = null;
  }

  /** All available devices across all hubs, grouped by hub */
  function groupedDevices(): { hubLabel: string; serial: string; devices: { channel: number; label: string }[] }[] {
    return availableHubs.map((hub, i) => ({
      hubLabel: `Hub ${i + 1}`,
      serial: hub.serial,
      devices: hub.devices.map(dev => ({
        channel: dev.channel,
        label: `${dev.device_type} — Ch ${dev.channel}`,
      })),
    }));
  }

  function isDeviceInZone(zone: RgbZoneConfig, hubSerial: string, channel: number): boolean {
    return zone.devices.some(d => d.hub_serial === hubSerial && d.channel === channel);
  }

  function toggleDevice(hubSerial: string, channel: number) {
    const zone = zones[selectedZone];
    if (!zone) return;

    if (isDeviceInZone(zone, hubSerial, channel)) {
      zone.devices = zone.devices.filter(d => !(d.hub_serial === hubSerial && d.channel === channel));
    } else {
      zone.devices = [...zone.devices, { hub_serial: hubSerial, channel }];
    }
    onzoneupdate({ ...zone });
  }

  function addAllDevices() {
    const zone = zones[selectedZone];
    if (!zone) return;
    const all: RgbDeviceRef[] = [];
    for (const hub of availableHubs) {
      for (const dev of hub.devices) {
        all.push({ hub_serial: hub.serial, channel: dev.channel });
      }
    }
    zone.devices = all;
    onzoneupdate({ ...zone });
  }

  function clearDevices() {
    const zone = zones[selectedZone];
    if (!zone) return;
    zone.devices = [];
    onzoneupdate({ ...zone });
  }
</script>

<div class="zone-panel">
  <h3 class="section-title">Zones</h3>

  <div class="zone-list">
    {#each zones as zone, i}
      <button
        class="zone-item"
        class:active={selectedZone === i}
        onclick={() => onselect(i)}
        ondblclick={() => startRename(i)}
      >
        {#if editingName === i}
          <input
            class="rename-input"
            type="text"
            bind:value={editValue}
            onblur={() => finishRename(i)}
            onkeydown={(e) => { if (e.key === 'Enter') finishRename(i); if (e.key === 'Escape') editingName = null; }}
          />
        {:else}
          <span class="zone-name">{zone.name}</span>
          <span class="zone-meta">{zone.devices.length} dev</span>
        {/if}

        {#if zones.length > 1}
          <button
            class="zone-delete"
            onclick={(e) => { e.stopPropagation(); ondelete(i); }}
            title="Delete zone"
          >&times;</button>
        {/if}
      </button>
    {/each}
  </div>

  <button class="add-zone" onclick={onadd}>+ Add Zone</button>

  <!-- Device Assignment for selected zone -->
  {#if zones[selectedZone]}
    <div class="device-section">
      <div class="device-header">
        <h3 class="section-title">Devices</h3>
        <div class="device-actions">
          <button class="small-btn" onclick={addAllDevices} title="Add all">All</button>
          <button class="small-btn" onclick={clearDevices} title="Clear all">Clear</button>
        </div>
      </div>

      {#if availableHubs.length === 0}
        <p class="no-devices">No hubs detected</p>
      {:else}
        <div class="device-list">
          {#each groupedDevices() as group}
            <div class="hub-group">
              <span class="hub-label">{group.hubLabel}</span>
              {#each group.devices as dev}
                <label class="device-row">
                  <input
                    type="checkbox"
                    checked={isDeviceInZone(zones[selectedZone], group.serial, dev.channel)}
                    onchange={() => toggleDevice(group.serial, dev.channel)}
                  />
                  <span class="device-label">{dev.label}</span>
                </label>
              {/each}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .zone-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .zone-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .zone-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
    text-align: left;
    font-size: 12px;
    color: var(--text-secondary);
    transition: all 0.15s;
  }
  .zone-item:hover {
    background: var(--bg-card);
    border-color: var(--border);
    color: var(--text-primary);
  }
  .zone-item.active {
    background: var(--bg-card-active);
    border-color: var(--accent-dim);
    color: var(--text-primary);
  }
  .zone-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .zone-meta {
    font-size: 10px;
    color: var(--text-muted);
  }
  .zone-delete {
    opacity: 0;
    padding: 0 4px;
    font-size: 14px;
    line-height: 1;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    transition: opacity 0.1s;
  }
  .zone-item:hover .zone-delete {
    opacity: 1;
  }
  .zone-delete:hover {
    color: var(--danger);
  }
  .rename-input {
    flex: 1;
    font-size: 12px;
    padding: 2px 4px;
  }
  .add-zone {
    font-size: 11px;
    padding: 6px;
    color: var(--accent);
    background: transparent;
    border: 1px dashed var(--border);
  }
  .add-zone:hover {
    border-color: var(--accent-dim);
    background: var(--bg-card);
  }

  /* Device assignment */
  .device-section {
    margin-top: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .device-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
  }
  .device-header .section-title {
    margin: 0;
  }
  .device-actions {
    display: flex;
    gap: 4px;
  }
  .small-btn {
    font-size: 9px;
    padding: 2px 6px;
    color: var(--text-muted);
    background: transparent;
    border: 1px solid var(--border);
    cursor: pointer;
    border-radius: 3px;
  }
  .small-btn:hover {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .no-devices {
    font-size: 11px;
    color: var(--text-muted);
    margin: 4px 0;
  }
  .device-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }
  .device-list::-webkit-scrollbar { width: 5px; }
  .device-list::-webkit-scrollbar-track { background: transparent; }
  .device-list::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 3px; }
  .device-list::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.2); }
  .hub-group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .hub-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    padding: 2px 4px;
  }
  .device-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 11px;
    color: var(--text-secondary);
  }
  .device-row:hover {
    background: var(--bg-card);
    color: var(--text-primary);
  }
  .device-row input[type="checkbox"] {
    accent-color: var(--accent);
    flex-shrink: 0;
  }
  .device-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
