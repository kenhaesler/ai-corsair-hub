<script lang="ts">
  import type { RgbColor } from '../../lib/types';
  import ColorPicker from './ColorPicker.svelte';

  interface Props {
    stops: [number, RgbColor][];
    onchange: (stops: [number, RgbColor][]) => void;
    /** Label for stop values (e.g., "C" for temperature, "%" for position) */
    unit?: string;
  }

  let { stops, onchange, unit = '%' }: Props = $props();

  function updateStop(index: number, value: number, color: RgbColor) {
    const newStops: [number, RgbColor][] = stops.map((s, i) =>
      i === index ? [value, color] : s
    );
    onchange(newStops);
  }

  function addStop() {
    const last = stops[stops.length - 1];
    const newVal = last ? last[0] + 10 : 50;
    onchange([...stops, [newVal, { r: 255, g: 255, b: 255 }]]);
  }

  function removeStop(index: number) {
    if (stops.length <= 2) return;
    onchange(stops.filter((_, i) => i !== index));
  }

  function formatColor(c: RgbColor): string {
    return `rgb(${c.r},${c.g},${c.b})`;
  }

  function gradientCss(): string {
    if (stops.length < 2) return 'transparent';
    const sorted = [...stops].sort((a, b) => a[0] - b[0]);
    const min = sorted[0][0];
    const max = sorted[sorted.length - 1][0];
    const range = max - min || 1;
    const cssStops = sorted.map(([v, c]) => {
      const pct = ((v - min) / range) * 100;
      return `${formatColor(c)} ${pct}%`;
    });
    return `linear-gradient(to right, ${cssStops.join(', ')})`;
  }
</script>

<div class="gradient-editor">
  <div class="gradient-bar" style="background: {gradientCss()}"></div>

  <div class="stop-list">
    {#each stops as [value, color], i}
      <div class="stop-row">
        <ColorPicker {color} onchange={(c) => updateStop(i, value, c)} />
        <input
          type="number"
          class="stop-value"
          {value}
          onchange={(e) => updateStop(i, parseFloat((e.target as HTMLInputElement).value), color)}
        />
        <span class="stop-unit">{unit}</span>
        {#if stops.length > 2}
          <button class="remove-stop" onclick={() => removeStop(i)}>&times;</button>
        {/if}
      </div>
    {/each}
  </div>

  <button class="add-stop" onclick={addStop}>+ Add Stop</button>
</div>

<style>
  .gradient-editor {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .gradient-bar {
    height: 16px;
    border-radius: 4px;
    border: 1px solid var(--border);
  }
  .stop-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .stop-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .stop-value {
    width: 50px;
    font-size: 11px;
  }
  .stop-unit {
    font-size: 10px;
    color: var(--text-muted);
  }
  .remove-stop {
    padding: 0 4px;
    font-size: 14px;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .remove-stop:hover {
    color: var(--danger);
  }
  .add-stop {
    font-size: 10px;
    padding: 3px 6px;
    color: var(--accent);
    background: transparent;
    border: 1px dashed var(--border);
  }
</style>
