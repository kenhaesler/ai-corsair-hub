<script lang="ts">
  import type { EffectConfig, RgbColor } from '../../lib/types';
  import ColorPicker from './ColorPicker.svelte';
  import GradientEditor from './GradientEditor.svelte';

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

  // Gradient effect stores a flat color array. The editor speaks
  // [position, color] stops; synthesize even-spaced positions on read
  // and discard them on write — the renderer still interpolates evenly.
  function colorsToStops(colors: RgbColor[]): [number, RgbColor][] {
    if (colors.length === 0) return [];
    if (colors.length === 1) return [[0, colors[0]]];
    return colors.map((c, i) => [(i / (colors.length - 1)) * 100, c]);
  }
</script>

<div class="effect-controls">
  {#if effect.type === 'Static'}
    <div class="control-row">
      <span class="control-label">Color</span>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>

  {:else if effect.type === 'Breathing'}
    <div class="control-row">
      <span class="control-label">Color</span>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.1" max="5" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'ColorCycle'}
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.1" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <span class="control-label">Saturation</span>
      <input aria-label="Saturation" type="range" min="0" max="1" step="0.05" value={effect.saturation}
        oninput={(e) => update({ saturation: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{(effect.saturation * 100).toFixed(0)}%</span>
    </div>

  {:else if effect.type === 'RainbowWave'}
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.05" max="2" step="0.05" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(2)}</span>
    </div>
    <div class="control-row">
      <span class="control-label">Wavelength</span>
      <input aria-label="Wavelength" type="range" min="0.5" max="5" step="0.1" value={effect.wavelength}
        oninput={(e) => update({ wavelength: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.wavelength.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'SpectrumShift'}
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.05" max="2" step="0.05" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(2)}</span>
    </div>

  {:else if effect.type === 'Fire'}
    <div class="control-row">
      <span class="control-label">Intensity</span>
      <input aria-label="Intensity" type="range" min="0.1" max="1.5" step="0.1" value={effect.intensity}
        oninput={(e) => update({ intensity: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.intensity.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.2" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Aurora'}
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.1" max="2" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <span class="control-label">Spread</span>
      <input aria-label="Spread" type="range" min="0.5" max="3" step="0.1" value={effect.color_spread}
        oninput={(e) => update({ color_spread: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.color_spread.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Candle'}
    <div class="control-row">
      <span class="control-label">Color</span>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">Flicker</span>
      <input aria-label="Flicker" type="range" min="0.5" max="8" step="0.5" value={effect.flicker_speed}
        oninput={(e) => update({ flicker_speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.flicker_speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Starfield'}
    <div class="control-row">
      <span class="control-label">Density</span>
      <input aria-label="Density" type="range" min="0.1" max="1" step="0.1" value={effect.density}
        oninput={(e) => update({ density: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{(effect.density * 100).toFixed(0)}%</span>
    </div>
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.2" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>

  {:else if effect.type === 'Rain'}
    <div class="control-row">
      <span class="control-label">Color</span>
      <ColorPicker color={effect.color} onchange={(c) => updateColor('color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.2" max="3" step="0.1" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(1)}</span>
    </div>
    <div class="control-row">
      <span class="control-label">Density</span>
      <input aria-label="Density" type="range" min="0.1" max="1" step="0.1" value={effect.density}
        oninput={(e) => update({ density: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{(effect.density * 100).toFixed(0)}%</span>
    </div>

  {:else if effect.type === 'TemperatureMap'}
    <div class="control-row">
      <span class="control-label">Glow on spike</span>
      <input aria-label="Glow on spike" type="checkbox" checked={effect.glow_on_spike}
        onchange={(e) => update({ glow_on_spike: (e.target as HTMLInputElement).checked })} />
    </div>
    <GradientEditor
      stops={effect.gradient}
      unit="°C"
      onchange={(stops) => update({ gradient: stops })}
    />

  {:else if effect.type === 'ThermalPulse'}
    <div class="control-row">
      <span class="control-label">Cold Color</span>
      <ColorPicker color={effect.cold_color} onchange={(c) => updateColor('cold_color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">Hot Color</span>
      <ColorPicker color={effect.hot_color} onchange={(c) => updateColor('hot_color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">Min Temp</span>
      <input aria-label="Min Temp" type="number" min="0" max="100" value={effect.min_temp}
        onchange={(e) => update({ min_temp: parseFloat((e.target as HTMLInputElement).value) })} />
    </div>
    <div class="control-row">
      <span class="control-label">Max Temp</span>
      <input aria-label="Max Temp" type="number" min="0" max="120" value={effect.max_temp}
        onchange={(e) => update({ max_temp: parseFloat((e.target as HTMLInputElement).value) })} />
    </div>

  {:else if effect.type === 'DutyMeter'}
    <div class="control-row">
      <span class="control-label">Low Color</span>
      <ColorPicker color={effect.low_color} onchange={(c) => updateColor('low_color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">High Color</span>
      <ColorPicker color={effect.high_color} onchange={(c) => updateColor('high_color', c)} />
    </div>

  {:else if effect.type === 'GpuMeter'}
    <div class="control-row">
      <span class="control-label">Idle Color</span>
      <ColorPicker color={effect.low_color} onchange={(c) => updateColor('low_color', c)} />
    </div>
    <div class="control-row">
      <span class="control-label">Load Color</span>
      <ColorPicker color={effect.high_color} onchange={(c) => updateColor('high_color', c)} />
    </div>
    <p class="hint">LEDs fill with GPU utilisation (0–100 %). Useful because GPU util leads temperature by a couple of seconds on scene transitions.</p>

  {:else if effect.type === 'Gradient'}
    <div class="control-row">
      <span class="control-label">Speed</span>
      <input aria-label="Speed" type="range" min="0.05" max="2" step="0.05" value={effect.speed}
        oninput={(e) => update({ speed: parseFloat((e.target as HTMLInputElement).value) })} />
      <span class="value">{effect.speed.toFixed(2)}</span>
    </div>
    <GradientEditor
      stops={colorsToStops(effect.colors)}
      unit="%"
      onchange={(stops) => update({ colors: stops.map(([, c]) => c) })}
    />
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
  .control-label {
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
</style>
