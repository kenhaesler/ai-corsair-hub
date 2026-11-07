<!--
  EffectControls (inventory 4.9): every parameter of every effect, with the
  same ranges/steps as the real app and a numeric readout. Sliders apply
  live; the page coalesces the saves.
-->
<script lang="ts">
  import Slider from '../../components/ui/Slider.svelte';
  import Checkbox from '../../components/ui/Checkbox.svelte';
  import NumberField from '../../components/ui/NumberField.svelte';
  import type { EffectConfig, RgbColor } from '../../lib/types';
  import ColorPicker from './ColorPicker.svelte';
  import GradientEditor from './GradientEditor.svelte';

  interface Props {
    effect: EffectConfig;
    onchange: (e: EffectConfig) => void;
  }
  let { effect, onchange }: Props = $props();
  const uid = Math.random().toString(36).slice(2, 7);

  function set(partial: Record<string, unknown>) {
    onchange({ ...effect, ...partial } as EffectConfig);
  }

  const f1 = (v: number) => v.toFixed(1);
  const f2 = (v: number) => v.toFixed(2);
  const pct = (v: number) => `${(v * 100).toFixed(0)}%`;

  /** Gradient stores flat colours; the editor speaks even-spaced [pos, colour] stops. */
  function colorsToStops(colors: RgbColor[]): [number, RgbColor][] {
    if (colors.length === 0) return [];
    if (colors.length === 1) return [[0, colors[0]]];
    return colors.map((c, i) => [Math.round((i / (colors.length - 1)) * 100), c]);
  }
  /** Editing a position re-orders the colours (positions themselves stay even). */
  function stopsToColors(stops: [number, RgbColor][]): RgbColor[] {
    return [...stops].sort((a, b) => a[0] - b[0]).map(([, c]) => c);
  }
</script>

{#snippet slider(key: string, label: string, value: number, min: number, max: number, step: number, fmt: (v: number) => string)}
  <div class="row">
    <label class="lab" for="{uid}-{key}">{label}</label>
    <Slider
      id="{uid}-{key}"
      {label}
      hideLabel
      {min}
      {max}
      {step}
      {value}
      format={fmt}
      oninput={(v) => set({ [key]: Math.round(v * 1000) / 1000 })}
    />
  </div>
{/snippet}

{#snippet color(key: string, label: string, value: RgbColor)}
  <div class="row">
    <span class="lab">{label}</span>
    <div class="ctl"><ColorPicker color={value} {label} onchange={(c) => set({ [key]: c })} /></div>
  </div>
{/snippet}

<div class="ec">
  {#if effect.type === 'Static'}
    {@render color('color', 'Color', effect.color)}
  {:else if effect.type === 'Breathing'}
    {@render color('color', 'Color', effect.color)}
    {@render slider('speed', 'Speed', effect.speed, 0.1, 5, 0.1, f1)}
  {:else if effect.type === 'ColorCycle'}
    {@render slider('speed', 'Speed', effect.speed, 0.1, 3, 0.1, f1)}
    {@render slider('saturation', 'Saturation', effect.saturation, 0, 1, 0.05, pct)}
  {:else if effect.type === 'RainbowWave'}
    {@render slider('speed', 'Speed', effect.speed, 0.05, 2, 0.05, f2)}
    {@render slider('wavelength', 'Wavelength', effect.wavelength, 0.5, 5, 0.1, f1)}
  {:else if effect.type === 'SpectrumShift'}
    {@render slider('speed', 'Speed', effect.speed, 0.05, 2, 0.05, f2)}
  {:else if effect.type === 'Fire'}
    {@render slider('intensity', 'Intensity', effect.intensity, 0.1, 1.5, 0.1, f1)}
    {@render slider('speed', 'Speed', effect.speed, 0.2, 3, 0.1, f1)}
  {:else if effect.type === 'Aurora'}
    {@render slider('speed', 'Speed', effect.speed, 0.1, 2, 0.1, f1)}
    {@render slider('color_spread', 'Spread', effect.color_spread, 0.5, 3, 0.1, f1)}
  {:else if effect.type === 'Candle'}
    {@render color('color', 'Color', effect.color)}
    {@render slider('flicker_speed', 'Flicker', effect.flicker_speed, 0.5, 8, 0.5, f1)}
  {:else if effect.type === 'Starfield'}
    {@render slider('density', 'Density', effect.density, 0.1, 1, 0.1, pct)}
    {@render slider('speed', 'Speed', effect.speed, 0.2, 3, 0.1, f1)}
  {:else if effect.type === 'Rain'}
    {@render color('color', 'Color', effect.color)}
    {@render slider('speed', 'Speed', effect.speed, 0.2, 3, 0.1, f1)}
    {@render slider('density', 'Density', effect.density, 0.1, 1, 0.1, pct)}
  {:else if effect.type === 'TemperatureMap'}
    <div class="row">
      <span class="lab">Spikes</span>
      <div class="ctl">
        <Checkbox checked={effect.glow_on_spike} onchange={(v) => set({ glow_on_spike: v })}>Glow on spike</Checkbox>
      </div>
    </div>
    <div class="block">
      <span class="lab block-lab">Temperature colours</span>
      <GradientEditor stops={effect.gradient} unit="°C" noun="Temperature stop" onchange={(s) => set({ gradient: s })} />
    </div>
  {:else if effect.type === 'ThermalPulse'}
    {@render color('cold_color', 'Cold Color', effect.cold_color)}
    {@render color('hot_color', 'Hot Color', effect.hot_color)}
    <div class="row">
      <span class="lab">Min Temp</span>
      <div class="ctl">
        <NumberField label="Min Temp" value={effect.min_temp} min={0} max={100} step={1} unit="°C" width={64} onchange={(v) => set({ min_temp: v })} />
      </div>
    </div>
    <div class="row">
      <span class="lab">Max Temp</span>
      <div class="ctl">
        <NumberField label="Max Temp" value={effect.max_temp} min={0} max={120} step={1} unit="°C" width={64} onchange={(v) => set({ max_temp: v })} />
      </div>
    </div>
  {:else if effect.type === 'DutyMeter'}
    {@render color('low_color', 'Low Color', effect.low_color)}
    {@render color('high_color', 'High Color', effect.high_color)}
  {:else if effect.type === 'GpuMeter'}
    {@render color('low_color', 'Idle Color', effect.low_color)}
    {@render color('high_color', 'Load Color', effect.high_color)}
    <p class="hint t-callout">
      LEDs fill with GPU utilisation (0–100 %). Useful because GPU util leads temperature by a couple of seconds on scene transitions.
    </p>
  {:else if effect.type === 'Gradient'}
    {@render slider('speed', 'Speed', effect.speed, 0.05, 2, 0.05, f2)}
    <div class="block">
      <span class="lab block-lab">Colours</span>
      <GradientEditor
        stops={colorsToStops(effect.colors)}
        unit="%"
        noun="Colour stop"
        onchange={(s) => set({ colors: stopsToColors(s) })}
      />
    </div>
  {/if}
</div>

<style>
  .ec {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    min-height: 36px;
  }
  .lab {
    font-size: 13px;
    line-height: 18px;
    font-weight: 500;
    color: var(--t2);
  }
  .ctl {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    min-width: 0;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 10px;
  }
  .block-lab {
    color: var(--t2);
  }
  .hint {
    margin-top: 6px;
    color: var(--t3);
    max-width: 60ch;
  }
</style>
