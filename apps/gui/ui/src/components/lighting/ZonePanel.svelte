<script lang="ts">
  import type { RgbZoneConfig, RgbDeviceRef, HubSnapshot, HubDeviceEntry } from '../../lib/types';
  import { displayNameFromTree, shortHubSerial } from '../../lib/identity';
  import { configStore } from '../../lib/stores/config.svelte';

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

  interface PickerEntry {
    device_id: string;
    hub_serial: string;
    channel: number;
    label: string;
    device_type: string;
  }

  /**
   * All available devices across all hubs, grouped by hub. The label uses
   * `displayNameFromTree` so a user-set friendly name takes precedence over
   * the "Hub XXXX Ch N" fallback. The device_type is shown alongside so the
   * user can tell a fan from a strip at a glance.
   */
  function groupedDevices(): { hubLabel: string; serial: string; devices: PickerEntry[] }[] {
    return availableHubs.map((hub, i) => ({
      hubLabel: `Hub ${i + 1} · ${shortHubSerial(hub.serial)}`,
      serial: hub.serial,
      devices: hub.devices.map((dev: HubDeviceEntry) => {
        const name = displayNameFromTree(dev.device_id, {
          config: configStore.config,
          hubs: availableHubs,
        });
        return {
          device_id: dev.device_id,
          hub_serial: hub.serial,
          channel: dev.channel,
          device_type: dev.device_type,
          label: `${dev.device_type} — ${name}`,
        };
      }),
    }));
  }

  /**
   * Membership test. Prefers device_id matching (V2); falls through to
   * (hub_serial, channel) matching for V1 entries that haven't been
   * upgraded yet. This keeps a mixed-schema zone functional while the user
   * is mid-migration.
   */
  function isDeviceInZone(zone: RgbZoneConfig, entry: PickerEntry): boolean {
    return zone.devices.some((d) => {
      if (d.device_id && entry.device_id && d.device_id === entry.device_id) {
        return true;
      }
      return d.hub_serial === entry.hub_serial && d.channel === entry.channel;
    });
  }

  /**
   * Toggle membership. Adds new entries in V2 shape (device_id populated)
   * and also carries (hub_serial, channel) so a backend loader that still
   * honours the V1 path can resolve the device. Removal matches by either
   * identity — whichever the stored entry was keyed by.
   */
  function toggleDevice(entry: PickerEntry) {
    const zone = zones[selectedZone];
    if (!zone) return;

    if (isDeviceInZone(zone, entry)) {
      zone.devices = zone.devices.filter((d) => {
        const idMatch =
          !!d.device_id && !!entry.device_id && d.device_id === entry.device_id;
        const locMatch =
          d.hub_serial === entry.hub_serial && d.channel === entry.channel;
        return !(idMatch || locMatch);
      });
    } else {
      zone.devices = [
        ...zone.devices,
        {
          hub_serial: entry.hub_serial,
          channel: entry.channel,
          device_id: entry.device_id,
        },
      ];
    }
    onzoneupdate({ ...zone });
  }

  function addAllDevices() {
    const zone = zones[selectedZone];
    if (!zone) return;
    const all: RgbDeviceRef[] = [];
    for (const hub of availableHubs) {
      for (const dev of hub.devices) {
        all.push({
          hub_serial: hub.serial,
          channel: dev.channel,
          device_id: dev.device_id,
        });
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
                    checked={isDeviceInZone(zones[selectedZone], dev)}
                    onchange={() => toggleDevice(dev)}
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
