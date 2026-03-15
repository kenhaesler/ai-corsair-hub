<script lang="ts">
  import type { EffectConfig } from '../../lib/types';

  interface Props {
    onselect: (effect: EffectConfig) => void;
    onclose: () => void;
  }

  let { onselect, onclose }: Props = $props();

  interface EffectTemplate {
    name: string;
    category: string;
    config: EffectConfig;
  }

  const effects: EffectTemplate[] = [
    // Solid & Basic
    { name: 'Static', category: 'Basic', config: { type: 'Static', color: { r: 96, g: 165, b: 250 } } },
    { name: 'Breathing', category: 'Basic', config: { type: 'Breathing', color: { r: 96, g: 165, b: 250 }, speed: 1.0 } },

    // Color Motion
    { name: 'Color Cycle', category: 'Motion', config: { type: 'ColorCycle', speed: 0.5, saturation: 1.0 } },
    { name: 'Rainbow Wave', category: 'Motion', config: { type: 'RainbowWave', speed: 0.3, wavelength: 1.0 } },
    { name: 'Spectrum Shift', category: 'Motion', config: { type: 'SpectrumShift', speed: 0.2 } },
    { name: 'Gradient', category: 'Motion', config: { type: 'Gradient', colors: [{ r: 255, g: 0, b: 128 }, { r: 0, g: 200, b: 255 }], speed: 0.3 } },

    // Organic
    { name: 'Fire', category: 'Organic', config: { type: 'Fire', intensity: 0.8, speed: 1.0 } },
    { name: 'Aurora', category: 'Organic', config: { type: 'Aurora', speed: 0.5, color_spread: 1.0 } },
    { name: 'Candle', category: 'Organic', config: { type: 'Candle', color: { r: 255, g: 160, b: 40 }, flicker_speed: 3.0 } },
    { name: 'Starfield', category: 'Organic', config: { type: 'Starfield', density: 0.6, speed: 1.0 } },
    { name: 'Rain', category: 'Organic', config: { type: 'Rain', color: { r: 60, g: 160, b: 255 }, speed: 1.0, density: 0.5 } },

    // Sensor-Reactive
    { name: 'Temp Map', category: 'Reactive', config: { type: 'TemperatureMap', gradient: [[30, { r: 60, g: 165, b: 250 }], [60, { r: 250, g: 190, b: 40 }], [85, { r: 248, g: 113, b: 113 }]], glow_on_spike: true } },
    { name: 'Thermal Pulse', category: 'Reactive', config: { type: 'ThermalPulse', cold_color: { r: 60, g: 165, b: 250 }, hot_color: { r: 248, g: 113, b: 113 }, min_temp: 30, max_temp: 90 } },
    { name: 'Duty Meter', category: 'Reactive', config: { type: 'DutyMeter', low_color: { r: 52, g: 211, b: 153 }, high_color: { r: 248, g: 113, b: 113 } } },
  ];

  const categories = ['Basic', 'Motion', 'Organic', 'Reactive'];

  function categoryColor(cat: string): string {
    switch (cat) {
      case 'Basic': return 'var(--text-secondary)';
      case 'Motion': return 'var(--accent)';
      case 'Organic': return '#f59e0b';
      case 'Reactive': return '#34d399';
      default: return 'var(--text-muted)';
    }
  }
</script>

<div class="effect-picker">
  <div class="picker-header">
    <span class="section-title">Choose Effect</span>
    <button class="close-btn" onclick={onclose}>&times;</button>
  </div>

  {#each categories as cat}
    <div class="category">
      <span class="category-label" style="color: {categoryColor(cat)}">{cat}</span>
      <div class="effect-grid">
        {#each effects.filter(e => e.category === cat) as effect}
          <button
            class="effect-card"
            onclick={() => onselect(effect.config)}
          >
            <span class="effect-name">{effect.name}</span>
          </button>
        {/each}
      </div>
    </div>
  {/each}
</div>

<style>
  .effect-picker {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .picker-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .close-btn {
    padding: 0 6px;
    font-size: 16px;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .close-btn:hover {
    color: var(--text-primary);
  }
  .category {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .category-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .effect-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }
  .effect-card {
    padding: 6px 8px;
    font-size: 11px;
    text-align: left;
    background: var(--bg-elevated);
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--text-primary);
    cursor: pointer;
    transition: all 0.15s;
  }
  .effect-card:hover {
    border-color: var(--accent-dim);
    background: var(--bg-card-active);
  }
  .effect-name {
    font-weight: 500;
  }
</style>
