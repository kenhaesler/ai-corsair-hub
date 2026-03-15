<script lang="ts">
  import type { RgbPreset, RgbZoneConfig } from '../../lib/types';

  interface Props {
    presets: RgbPreset[];
    currentZones: RgbZoneConfig[];
    onapply: (zones: RgbZoneConfig[]) => void;
  }

  let { presets, currentZones, onapply }: Props = $props();

  let saveName = $state('');
  let showSave = $state(false);

  // Built-in presets
  const builtins: RgbPreset[] = [
    {
      name: 'Ocean Calm',
      zones: [{
        name: 'All',
        devices: [],
        layers: [
          { effect: { type: 'RainbowWave', speed: 0.1, wavelength: 2.0 }, blend_mode: 'Normal', opacity: 1.0, enabled: true },
          { effect: { type: 'Breathing', color: { r: 255, g: 255, b: 255 }, speed: 0.3 }, blend_mode: 'Screen', opacity: 0.3, enabled: true },
        ],
        brightness: 80,
        flow: null,
      }],
    },
    {
      name: 'Inferno',
      zones: [{
        name: 'All',
        devices: [],
        layers: [
          { effect: { type: 'Fire', intensity: 0.9, speed: 1.2 }, blend_mode: 'Normal', opacity: 1.0, enabled: true },
          { effect: { type: 'Candle', color: { r: 255, g: 160, b: 40 }, flicker_speed: 3.0 }, blend_mode: 'Add', opacity: 0.4, enabled: true },
        ],
        brightness: 100,
        flow: null,
      }],
    },
    {
      name: 'Arctic',
      zones: [{
        name: 'All',
        devices: [],
        layers: [
          { effect: { type: 'Static', color: { r: 100, g: 180, b: 255 } }, blend_mode: 'Normal', opacity: 1.0, enabled: true },
          { effect: { type: 'Starfield', density: 0.6, speed: 0.8 }, blend_mode: 'Add', opacity: 0.6, enabled: true },
        ],
        brightness: 90,
        flow: null,
      }],
    },
    {
      name: 'Cyberpunk',
      zones: [{
        name: 'All',
        devices: [],
        layers: [
          { effect: { type: 'Gradient', colors: [{ r: 255, g: 50, b: 150 }, { r: 0, g: 200, b: 255 }], speed: 0.2 }, blend_mode: 'Normal', opacity: 1.0, enabled: true },
          { effect: { type: 'SpectrumShift', speed: 0.15 }, blend_mode: 'Overlay', opacity: 0.5, enabled: true },
        ],
        brightness: 100,
        flow: null,
      }],
    },
    {
      name: 'Thermal Monitor',
      zones: [{
        name: 'All',
        devices: [],
        layers: [
          { effect: { type: 'TemperatureMap', gradient: [[30, { r: 60, g: 165, b: 250 }], [55, { r: 52, g: 211, b: 153 }], [70, { r: 250, g: 190, b: 40 }], [85, { r: 248, g: 113, b: 113 }]], glow_on_spike: true }, blend_mode: 'Normal', opacity: 1.0, enabled: true },
        ],
        brightness: 80,
        flow: null,
      }],
    },
    {
      name: 'Silent Pulse',
      zones: [{
        name: 'All',
        devices: [],
        layers: [
          { effect: { type: 'ThermalPulse', cold_color: { r: 60, g: 165, b: 250 }, hot_color: { r: 248, g: 113, b: 113 }, min_temp: 30, max_temp: 85 }, blend_mode: 'Normal', opacity: 1.0, enabled: true },
          { effect: { type: 'Breathing', color: { r: 255, g: 255, b: 255 }, speed: 0.5 }, blend_mode: 'Multiply', opacity: 0.8, enabled: true },
        ],
        brightness: 70,
        flow: null,
      }],
    },
  ];

  function applyPreset(preset: RgbPreset) {
    // Preserve device assignments from current zones if possible
    const newZones = preset.zones.map((pz, i) => {
      const existingDevices = currentZones[i]?.devices ?? [];
      return { ...pz, devices: existingDevices.length > 0 ? existingDevices : pz.devices };
    });
    onapply(newZones);
  }

  function savePreset() {
    if (!saveName.trim()) return;
    presets.push({ name: saveName.trim(), zones: structuredClone(currentZones) });
    saveName = '';
    showSave = false;
  }
</script>

<div class="preset-manager">
  <h3 class="section-title">Presets</h3>

  <div class="preset-list">
    {#each builtins as preset}
      <button class="preset-btn" onclick={() => applyPreset(preset)}>
        {preset.name}
      </button>
    {/each}

    {#if presets.length > 0}
      <div class="divider"></div>
      {#each presets as preset}
        <button class="preset-btn user-preset" onclick={() => applyPreset(preset)}>
          {preset.name}
        </button>
      {/each}
    {/if}
  </div>

  {#if showSave}
    <div class="save-form">
      <input
        type="text"
        bind:value={saveName}
        placeholder="Preset name"
        onkeydown={(e) => { if (e.key === 'Enter') savePreset(); }}
      />
      <button class="primary" onclick={savePreset}>Save</button>
      <button onclick={() => { showSave = false; }}>Cancel</button>
    </div>
  {:else}
    <button class="save-btn" onclick={() => { showSave = true; }}>Save Current</button>
  {/if}
</div>

<style>
  .preset-manager {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .preset-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .preset-btn {
    text-align: left;
    padding: 6px 8px;
    font-size: 11px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--text-secondary);
    cursor: pointer;
  }
  .preset-btn:hover {
    background: var(--bg-card);
    border-color: var(--border);
    color: var(--text-primary);
  }
  .user-preset {
    font-style: italic;
  }
  .divider {
    height: 1px;
    background: var(--border);
    margin: 4px 0;
  }
  .save-form {
    display: flex;
    gap: 4px;
  }
  .save-form input {
    flex: 1;
    font-size: 11px;
  }
  .save-form button {
    font-size: 10px;
    padding: 4px 8px;
  }
  .save-btn {
    font-size: 10px;
    padding: 4px;
    color: var(--text-muted);
    background: transparent;
    border: 1px dashed var(--border);
  }
  .save-btn:hover {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
</style>
