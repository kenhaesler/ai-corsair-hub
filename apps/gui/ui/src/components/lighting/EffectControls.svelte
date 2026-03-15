<script lang="ts">
  import type { EffectConfig, RgbColor } from '../../lib/types';
  import ColorPicker from './ColorPicker.svelte';

  interface Props {
    effect: EffectConfig;
    onchange: (effect: EffectConfig) => void;
  }

  let { effect, onchange }: Props = $props();

  function update(partial: Record<string, unknown>) {
    onchange({ ...effect, ...partial } as EffectConfig);
  }

  function updateColor(key: string, color: RgbColor) {
    onchange({ ...effect, [key]: color } as EffectConfig);
  }

  function formatColor(c: RgbColor): string {
    return `rgb(${c.r},${c.g},${c.b})`;
  }
</script>

<div class="effect-controls">
  {#if effect.type === 'Static'}
    <div class="control-row">
      <label>Color</label>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>

  {:else if effect.type === 'Breathing'}
    <div class="control-row">
      <label>Color</label>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.1" max="5" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'ColorCycle'}
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.1" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <label>Saturation</label>
      <input type="range" min="0" max="1" step="0.05" value={effect.saturation}
        oninput={(e) => update({ saturation: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{(effect.saturation * 100).toFixed(0)}%</span>
    </div>

  {:else if effect.type === 'RainbowWave'}
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.05" max="2" step="0.05" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(2)}</span>
    </div>
    <div class="control-row">
      <label>Wavelength</label>
      <input type="range" min="0.5" max="5" step="0.1" value={effect.wavelength}
        oninput={(e) => update({ wavelength: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.wavelength.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'SpectrumShift'}
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.05" max="2" step="0.05" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(2)}</span>
    </div>

  {:else if effect.type === 'Fire'}
    <div class="control-row">
      <label>Intensity</label>
      <input type="range" min="0.1" max="1.5" step="0.1" value={effect.intensity}
        oninput={(e) => update({ intensity: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.intensity.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.2" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Aurora'}
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.1" max="2" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <label>Spread</label>
      <input type="range" min="0.5" max="3" step="0.1" value={effect.color_spread}
        oninput={(e) => update({ color_spread: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.color_spread.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Candle'}
    <div class="control-row">
      <label>Color</label>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>
    <div class="control-row">
      <label>Flicker</label>
      <input type="range" min="0.5" max="8" step="0.5" value={effect.flicker_speed}
        oninput={(e) => update({ flicker_speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.flicker_speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Starfield'}
    <div class="control-row">
      <label>Density</label>
      <input type="range" min="0.1" max="1" step="0.1" value={effect.density}
        oninput={(e) => update({ density: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{(effect.density * 100).toFixed(0)}%</span>
    </div>
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.2" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Rain'}
    <div class="control-row">
      <label>Color</label>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.2" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <label>Density</label>
      <input type="range" min="0.1" max="1" step="0.1" value={effect.density}
        oninput={(e) => update({ density: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{(effect.density * 100).toFixed(0)}%</span>
    </div>

  {:else if effect.type === 'TemperatureMap'}
    <div class="control-row">
      <label>Glow on spike</label>
      <input type="checkbox" checked={effect.glow_on_spike}
        onchange={(e) => update({ glow_on_spike: (e.target as HTMLInputElement).checked })} />
    </div>
    <p class="hint">Gradient: blue (30C) - yellow (60C) - red (85C)</p>

  {:else if effect.type === 'ThermalPulse'}
    <div class="control-row">
      <label>Cold Color</label>
      <ColorPicker color={effect.cold_color} onchange={(c) => updateColor('cold_color', c)} />
    </div>
    <div class="control-row">
      <label>Hot Color</label>
      <ColorPicker color={effect.hot_color} onchange={(c) => updateColor('hot_color', c)} />
    </div>
    <div class="control-row">
      <label>Min Temp</label>
      <input type="number" min="0" max="100" value={effect.min_temp}
        onchange={(e) => update({ min_temp: parseFloat((e.target as HTMLInputElement).value) })} />
    </div>
    <div class="control-row">
      <label>Max Temp</label>
      <input type="number" min="0" max="120" value={effect.max_temp}
        onchange={(e) => update({ max_temp: parseFloat((e.target as HTMLInputElement).value) })} />
    </div>

  {:else if effect.type === 'DutyMeter'}
    <div class="control-row">
      <label>Low Color</label>
      <ColorPicker color={effect.low_color} onchange={(c) => updateColor('low_color', c)} />
    </div>
    <div class="control-row">
      <label>High Color</label>
      <ColorPicker color={effect.high_color} onchange={(c) => updateColor('high_color', c)} />
    </div>

  {:else if effect.type === 'Gradient'}
    <div class="control-row">
      <label>Speed</label>
      <input type="range" min="0.05" max="2" step="0.05" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(2)}</span>
    </div>
    <div class="color-stops">
      {#each effect.colors as color, i}
        <div class="color-stop">
          <span class="stop-label">Stop {i + 1}</span>
          <div class="stop-swatch" style="background: {formatColor(color)}"></div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .effect-controls {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .control-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .control-row label {
    font-size: 11px;
    color: var(--text-secondary);
    min-width: 60px;
    flex-shrink: 0;
  }
  .control-row input[type="range"] {
    flex: 1;
  }
  .control-row .value {
    font-size: 11px;
    color: var(--text-muted);
    width: 36px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .control-row input[type="number"] {
    width: 60px;
  }
  .hint {
    font-size: 10px;
    color: var(--text-muted);
    margin: 0;
  }
  .color-stops {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .color-stop {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .stop-label {
    font-size: 10px;
    color: var(--text-muted);
  }
  .stop-swatch {
    width: 16px;
    height: 16px;
    border-radius: 3px;
    border: 1px solid rgba(255,255,255,0.1);
  }
</style>
