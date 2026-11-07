<!--
  GradientEditor (inventory 4.10): preview bar (stops sorted/normalised,
  with a marker per stop), one row per stop (colour picker, numeric value,
  unit label), × remove only when there are more than two stops, and
  "+ Add Stop" (white, at last value + 10, or 50 when empty).
-->
<script lang="ts">
  import X from 'lucide-svelte/icons/x';
  import Plus from 'lucide-svelte/icons/plus';
  import IconButton from '../../components/ui/IconButton.svelte';
  import Button from '../../components/ui/Button.svelte';
  import type { RgbColor } from '../../lib/types';
  import ColorPicker from './ColorPicker.svelte';
  import { rgbCss, stopsCss } from './effects';

  interface Props {
    stops: [number, RgbColor][];
    unit: string;
    /** label used for accessible names, e.g. "Temperature stop" */
    noun?: string;
    onchange: (stops: [number, RgbColor][]) => void;
  }
  let { stops, unit, noun = 'Stop', onchange }: Props = $props();

  const markers = $derived.by(() => {
    if (stops.length < 2) return stops.map(([, c]) => ({ pct: 50, c }));
    const vals = stops.map(([v]) => v);
    const min = Math.min(...vals);
    const range = Math.max(...vals) - min || 1;
    return stops.map(([v, c]) => ({ pct: ((v - min) / range) * 100, c }));
  });

  function update(i: number, value: number, color: RgbColor) {
    onchange(stops.map((s, j) => (j === i ? ([value, color] as [number, RgbColor]) : s)));
  }
  function add() {
    const last = stops[stops.length - 1];
    onchange([...stops, [last ? last[0] + 10 : 50, { r: 255, g: 255, b: 255 }]]);
  }
  function remove(i: number) {
    if (stops.length <= 2) return;
    onchange(stops.filter((_, j) => j !== i));
  }
</script>

<div class="ge">
  <div class="bar-wrap">
    <div class="bar" style:background={stopsCss(stops)} role="img" aria-label="Gradient preview, {stops.length} stops"></div>
    {#each markers as m, i (i)}
      <span class="mark" style:left="{m.pct}%" style:--c={rgbCss(m.c)} aria-hidden="true"></span>
    {/each}
  </div>

  <ul class="stops">
    {#each stops as [value, color], i (i)}
      <li class="stop">
        <ColorPicker {color} size="sm" label="{noun} {i + 1} colour" onchange={(c) => update(i, value, c)} />
        <label class="val">
          <span class="sr-only">{noun} {i + 1} value</span>
          <input
            class="num"
            type="number"
            step="1"
            {value}
            onchange={(e) => {
              const v = Number(e.currentTarget.value);
              if (Number.isFinite(v)) update(i, v, color);
            }}
          />
          <span class="unit">{unit}</span>
        </label>
        {#if stops.length > 2}
          <IconButton icon={X} size="sm" label="Remove stop {i + 1}" onclick={() => remove(i)} />
        {:else}
          <span class="spacer" aria-hidden="true"></span>
        {/if}
      </li>
    {/each}
  </ul>

  <div class="add">
    <Button variant="plain" size="sm" icon={Plus} onclick={add}><span class="sr-only">{'+ '}</span>Add Stop</Button>
  </div>
</div>

<style>
  .ge {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .bar-wrap {
    position: relative;
    margin: 2px 8px 8px;
  }
  .bar {
    height: 14px;
    border-radius: 999px;
    box-shadow:
      inset 0 0 0 1px rgb(0 0 0 / 0.3),
      0 6px 16px -8px rgb(0 0 0 / 0.6);
  }
  .mark {
    position: absolute;
    top: 100%;
    width: 10px;
    height: 10px;
    margin: 3px 0 0 -5px;
    border-radius: 50%;
    background: var(--c);
    box-shadow:
      0 0 0 1.5px var(--bg-0),
      0 0 0 2.5px rgba(255, 255, 255, 0.5);
  }
  .stops {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .stop {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .val {
    display: flex;
    align-items: center;
    height: 28px;
    padding: 0 10px 0 2px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 1px var(--hair);
  }
  .val:focus-within {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
  }
  .val input {
    width: 56px;
    height: 100%;
    padding: 0 0 0 8px;
    border: 0;
    background: transparent;
    font-size: 13px;
    font-weight: 500;
    color: var(--t1);
    text-align: right;
    -moz-appearance: textfield;
    appearance: textfield;
  }
  .val input:focus-visible {
    box-shadow: none;
  }
  .val input::-webkit-inner-spin-button,
  .val input::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  .unit {
    margin-left: 4px;
    font-size: 12px;
    color: var(--t3);
  }
  .spacer {
    width: 28px;
  }
  .add {
    margin-left: -8px;
  }
</style>
