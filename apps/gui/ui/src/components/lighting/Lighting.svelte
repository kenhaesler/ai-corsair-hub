<script lang="ts">
  import { onMount } from 'svelte';
  import {
    flushRgbSave,
    initializeRgbConfig,
    rgb,
    scheduleRgbSave,
  } from '../../lib/stores/rgb.svelte';
  import { configStore, loadConfig } from '../../lib/stores/config.svelte';
  import { getDevices } from '../../lib/api';
  import type { AppConfig, RgbConfig, RgbZoneConfig, RgbDeviceRef, HubSnapshot } from '../../lib/types';
  import type { RgbPreset } from '../../lib/types';
  import ZonePanel from './ZonePanel.svelte';
  import HardwarePreview from './HardwarePreview.svelte';
  import LayerStack from './LayerStack.svelte';
  import FlowEditor from './FlowEditor.svelte';
  import PresetManager from './PresetManager.svelte';
  import type { FlowConfig } from '../../lib/types';

  let config = $state<RgbConfig>({
    enabled: false,
    brightness: 80,
    fps: 30,
    hardware_output: false,
    zones: [],
    presets: [],
  });

  let selectedZone = $state(0);

  /** All discovered hubs + their devices — for device assignment UI */
  let configLoaded = $state(false);
  let devicesLoaded = $state(false);
  let devicesError = $state<string | null>(null);
  let availableHubs = $state<HubSnapshot[]>([]);

  let devicesLoading = false;

  function syncConfig(appConfig: AppConfig) {
    if (configLoaded) return;
    config = structuredClone(appConfig.rgb);
    initializeRgbConfig(config);
    configLoaded = true;
  }

  function ensureDefaultZone() {
    if (!configLoaded || !devicesLoaded || config.zones.length > 0 || availableHubs.length === 0) {
      return;
    }

    const devices: RgbDeviceRef[] = availableHubs.flatMap((hub) =>
      hub.devices.map((device) => ({
        hub_serial: hub.serial,
        channel: device.channel,
        device_id: device.device_id,
      })),
    );
    config.zones = [{
      name: 'All Fans',
      devices,
      layers: [{
        effect: { type: 'Static', color: { r: 96, g: 165, b: 250 } },
        blend_mode: 'Normal',
        opacity: 1,
        enabled: true,
      }],
      brightness: 100,
      flow: null,
    }];
    saveConfig(true);
  }

  async function reloadDevices() {
    if (devicesLoading) return;
    devicesLoading = true;
    devicesError = null;
    try {
      availableHubs = (await getDevices()).hubs;
      devicesLoaded = true;
      ensureDefaultZone();
    } catch (error) {
      devicesLoaded = false;
      devicesError = String(error);
    } finally {
      devicesLoading = false;
    }
  }

  $effect(() => {
    const appConfig = configStore.config;
    if (appConfig && !configLoaded) {
      syncConfig(appConfig);
      void reloadDevices();
    }
  });

  onMount(async () => {
    const appConfig = await loadConfig();
    if (appConfig) syncConfig(appConfig);
    await reloadDevices();
  });
  function currentZone(): RgbZoneConfig | null {
    return config.zones[selectedZone] ?? null;
  }

  function saveConfig(immediate = false) {
    scheduleRgbSave(config, immediate);
  }

  function toggleEnabled() {
    config.enabled = !config.enabled;
    saveConfig(true);
  }

  function toggleHardwareOutput() {
    config.hardware_output = !config.hardware_output;
    saveConfig(true);
  }

  function handleBrightness(e: Event) {
    config.brightness = parseInt((e.target as HTMLInputElement).value);
    saveConfig();
  }

  function handleZoneUpdate(zone: RgbZoneConfig) {
    config.zones[selectedZone] = zone;
    config = config; // trigger reactivity
    saveConfig();
  }

  function addZone() {
    config.zones = [...config.zones, {
      name: `Zone ${config.zones.length + 1}`,
      devices: [],
      layers: [{
        effect: { type: 'Static', color: { r: 96, g: 165, b: 250 } },
        blend_mode: 'Normal',
        opacity: 1.0,
        enabled: true,
      }],
      brightness: 100,
      flow: null,
    }];
    selectedZone = config.zones.length - 1;
    saveConfig(true);
  }

  function deleteZone(index: number) {
    config.zones = config.zones.filter((_, i) => i !== index);
    if (selectedZone >= config.zones.length) {
      selectedZone = Math.max(0, config.zones.length - 1);
    }
    saveConfig(true);
  }

  function applyPresetZones(zones: RgbZoneConfig[]) {
    config.zones = zones;
    selectedZone = 0;
    saveConfig(true);
  }

  function handleFlowChange(flow: FlowConfig | null) {
    const zone = currentZone();
    if (!zone) return;
    config.zones[selectedZone] = { ...zone, flow };
    config = config; // trigger reactivity
    saveConfig();
  }

  function updatePresets(presets: RgbPreset[]) {
    config.presets = presets;
    saveConfig(true);
  }
</script>


<div class="lighting-page">
{#if configLoaded}
  <div class="lighting-layout">
    <!-- Left: Zone Sidebar -->
    <div class="zone-sidebar">
      <ZonePanel
        zones={config.zones}
        {selectedZone}
        {availableHubs}
        onselect={(i) => { selectedZone = i; }}
        onadd={addZone}
        ondelete={deleteZone}
        onzoneupdate={handleZoneUpdate}
      />
      <PresetManager
        presets={config.presets}
        currentZones={config.zones}
        onapply={applyPresetZones}
        onpresetsupdate={updatePresets}
      />
      {#if devicesError}
        <div class="banner error" aria-live="polite">
          <span>Device discovery failed.</span>
          <button onclick={reloadDevices}>Retry</button>
        </div>
      {:else if !devicesLoaded}
        <span class="saving-indicator">Loading devices...</span>
      {/if}
    </div>

    <!-- Center: Preview -->
    <div class="preview-area">
      <HardwarePreview
        zone={currentZone()}
        frames={rgb.frames}
      />
    </div>

    <!-- Right: Layer Stack -->
    <div class="layer-panel">
      {#if currentZone()}
        <FlowEditor
          flow={currentZone()!.flow ?? null}
          deviceCount={currentZone()!.devices.length}
          onchange={handleFlowChange}
        />
        <LayerStack
          zone={currentZone()!}
          onupdate={handleZoneUpdate}
        />
      {:else}
        <div class="empty-state">
          <p class="text-secondary">No zones configured</p>
          <button class="primary" onclick={addZone}>Create Zone</button>
        </div>
      {/if}
    </div>
  </div>

  <!-- Bottom: Master controls -->
  <div class="master-bar">
    <label class="master-control">
      <span class="label">Master</span>
      <input
        type="range"
        min="0"
        max="100"
        value={config.brightness}
        oninput={handleBrightness}
      />
      <span class="value tabular-nums">{config.brightness}%</span>
    </label>

    <div class="master-right">
      <label class="toggle-label">
        <input
          type="checkbox"
          checked={config.hardware_output}
          onchange={toggleHardwareOutput}
        />
        <span>Hardware</span>
      </label>
      <label class="toggle-label">
        <input
          type="checkbox"
          checked={config.enabled}
          onchange={toggleEnabled}
        />
        <span>Enabled</span>
      </label>
      <span class="fps-badge">{config.fps} FPS</span>
      {#if rgb.saving}
        <span class="saving-indicator">Saving...</span>
      {/if}
    </div>
      {#if rgb.error}
        <button class="save-error" onclick={flushRgbSave} title={rgb.error}>
          Save failed. Retry
        </button>
      {/if}
  </div>
{:else}
  <div class="loading" aria-live="polite">
    Loading lighting configuration...
  </div>
{/if}
</div>

<style>
  .lighting-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
  .lighting-layout {
    flex: 1;
    display: grid;
    grid-template-columns: 260px 1fr 280px;
    gap: 1px;
    background: var(--border);
    overflow: hidden;
  }
  .zone-sidebar {
    background: rgba(0,0,0,0.05);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 12px;
    gap: 12px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }
  .zone-sidebar::-webkit-scrollbar { width: 6px; }
  .zone-sidebar::-webkit-scrollbar-track { background: transparent; }
  .zone-sidebar::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 3px; }
  .zone-sidebar::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.2); }
  .preview-area {
    background: rgba(0,0,0,0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    padding: 16px;
  }
  .layer-panel {
    background: rgba(0,0,0,0.05);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 12px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }
  .layer-panel::-webkit-scrollbar { width: 6px; }
  .layer-panel::-webkit-scrollbar-track { background: transparent; }
  .layer-panel::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 3px; }
  .layer-panel::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.2); }
  .master-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    border-top: 1px solid var(--border);
    background: rgba(0,0,0,0.05);
    flex-shrink: 0;
  }
  .master-control {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    max-width: 400px;
  }
  .master-control .label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-secondary);
    width: 50px;
  }
  .master-control input[type="range"] {
    flex: 1;
  }
  .master-control .value {
    font-size: 12px;
    color: var(--text-primary);
    width: 36px;
    text-align: right;
  }
  .master-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .toggle-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .toggle-label input[type="checkbox"] {
    accent-color: var(--accent);
  }
  .fps-badge {
    font-size: 10px;
    padding: 2px 6px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
  }
  .saving-indicator {
    font-size: 11px;
    color: var(--accent);
    animation: pulse 1s infinite;
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 12px;
  }
  .text-secondary {
    color: var(--text-secondary);
    font-size: 12px;
  }
</style>
