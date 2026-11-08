<!--
  Sensor Mix (TempSourceEditor, inventory 2.5).
  Header meta "N active · Σ x.xx" (rendered by the parent section). One chip
  per sensor, in config order then the other available sensors. A selected
  chip leads its own weight row, so its weight shows once (the row readout,
  2 dp) next to the weight slider (0–2, step .05) and its normalised share.
  Selected-but-unavailable chips are styled "unavailable". Click toggles; the
  last sensor can't be removed; adding gives weight 1. No temperatures here:
  the curve's live marker / PID track is their single home.
-->
<script lang="ts">
  import Slider from '../../components/ui/Slider.svelte';
  import Plus from 'lucide-svelte/icons/plus';
  import X from 'lucide-svelte/icons/x';
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';
  import { sourceColor } from '../../lib/format';
  import { sensorLabel } from './fanModes';
  import type { TempSourceConfig } from '../../lib/types';

  interface Props {
    source: TempSourceConfig;
    availableSensors: string[];
    /** live readings, so each sensor in the mix shows what it reads right now */
    temps?: { source: string; celsius: number }[];
    onchange: (s: TempSourceConfig) => void;
    oncheckpoint?: (key: string) => void;
  }
  let { source, availableSensors, temps = [], onchange, oncheckpoint }: Props = $props();
  const readingOf = (id: string) => temps.find((t) => t.source === id)?.celsius ?? null;

  interface Row {
    id: string;
    selected: boolean;
    weight: number;
    available: boolean;
  }
  const rows = $derived.by<Row[]>(() => {
    const out: Row[] = source.sensors.map((id, i) => ({
      id,
      selected: true,
      weight: source.weights[i] ?? 1,
      available: availableSensors.includes(id),
    }));
    for (const id of availableSensors)
      if (!source.sensors.includes(id)) out.push({ id, selected: false, weight: 0, available: true });
    return out;
  });
  const active = $derived(rows.filter((r) => r.selected));
  const sum = $derived(active.reduce((a, r) => a + r.weight, 0));

  function tip(r: Row): string {
    if (!r.available) return `${sensorLabel(r.id)} is referenced but not currently available`;
    return r.selected ? `Weight ${r.weight.toFixed(2)} — click to remove` : `Click to include ${sensorLabel(r.id)}`;
  }
  function toggle(id: string) {
    const i = source.sensors.indexOf(id);
    oncheckpoint?.(`mix-toggle-${id}-${Date.now()}`);
    if (i >= 0) {
      if (source.sensors.length <= 1) return;
      onchange({ sensors: source.sensors.filter((_, j) => j !== i), weights: source.weights.filter((_, j) => j !== i) });
    } else {
      onchange({ sensors: [...source.sensors, id], weights: [...source.weights, 1] });
    }
  }
  function setWeight(id: string, w: number) {
    const i = source.sensors.indexOf(id);
    if (i < 0) return;
    oncheckpoint?.(`mix-weight-${id}`);
    const weights = [...source.weights];
    weights[i] = Math.round(w * 100) / 100;
    onchange({ ...source, weights });
  }
</script>

{#snippet chip(r: Row)}
  {@const last = r.selected && active.length <= 1}
  <button
    type="button"
    class="chip"
    class:on={r.selected}
    class:unavailable={!r.available}
    class:last
    title={tip(r)}
    aria-pressed={r.selected}
    aria-disabled={last || undefined}
    onclick={() => toggle(r.id)}
    style:--c={sourceColor(r.id)}
  >
    {#if !r.available}
      <TriangleAlert size={12} strokeWidth={2.25} />
    {:else}
      <span class="dot" aria-hidden="true"></span>
    {/if}
    <span class="chip-l">{sensorLabel(r.id)}</span>
    {#if r.selected}
      {#if !last}<X size={12} strokeWidth={2.25} class="chip-x" />{/if}
    {:else}
      <Plus size={12} strokeWidth={2.25} class="chip-x" />
    {/if}
  </button>
{/snippet}

<div class="mix">
  <div class="weights" role="group" aria-label="Sensors in the mix">
    {#each active as r (r.id)}
      <div class="wrow" style:--c={sourceColor(r.id)}>
        <span class="wl">{@render chip(r)}{#if readingOf(r.id) !== null}<span class="wread num">{readingOf(r.id)!.toFixed(1)}°</span>{/if}</span>
        <div class="ws">
          <Slider
            label="{sensorLabel(r.id)} weight"
            hideLabel
            min={0}
            max={2}
            step={0.05}
            color={sourceColor(r.id)}
            value={r.weight}
            format={(v) => v.toFixed(2)}
            oninput={(v) => setWeight(r.id, v)}
          />
        </div>
        <span class="wshare num" title="Share of the weighted mean">{sum > 0 ? Math.round((r.weight / sum) * 100) : 0}%</span>
      </div>
    {/each}
  </div>

  {#if rows.some((r) => !r.selected)}
    <div class="add" role="group" aria-label="Add a sensor">
      <span class="add-l">Add</span>
      {#each rows.filter((r) => !r.selected) as r (r.id)}{@render chip(r)}{/each}
    </div>
  {/if}

  <p class="hint">
    Raw weights; the control loop normalizes to a weighted mean. A radiator group often weights GPU higher; chassis
    fans, CPU.
  </p>
</div>

<style>
  .mix {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 10px 0 10px;
    border: 0;
    border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px var(--hair);
    color: var(--t2);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .chip:hover {
    background: rgba(255, 255, 255, 0.09);
    color: var(--t1);
  }
  .chip:active {
    transform: scale(0.97);
  }
  .chip:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }
  .chip.on {
    background: color-mix(in srgb, var(--c) 16%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 45%, transparent);
    color: var(--t1);
  }
  .chip.last {
    cursor: default;
  }
  .chip.unavailable {
    background: var(--warn-surface);
    box-shadow: inset 0 0 0 1px var(--warn-hair);
    outline: 1px dashed var(--warn-hair);
    outline-offset: 2px;
    color: var(--warn-text);
  }
  .chip :global(.chip-x) {
    color: var(--t3);
    margin-right: -2px;
  }
  .chip:hover :global(.chip-x) {
    color: var(--t1);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
    flex: none;
  }
  .chip:not(.on) .dot {
    opacity: 0.55;
  }
  .weights {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .add {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .add-l {
    font-size: 12px;
    font-weight: 500;
    color: var(--t3);
    margin-right: 2px;
  }
  .wrow {
    display: grid;
    grid-template-columns: 196px minmax(0, 1fr) 44px;
    align-items: center;
    gap: 16px;
    min-height: 36px;
  }
  .wl {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .wread {
    font-size: 13px;
    font-weight: 500;
    color: color-mix(in oklab, var(--c) 45%, #fff);
    white-space: nowrap;
  }
  .wshare {
    font-size: 12px;
    color: var(--t3);
    text-align: right;
  }
  .ws :global(.val) {
    min-width: 40px;
  }
  .hint {
    font-size: 12px;
    line-height: 17px;
    color: var(--t3);
    max-width: 70ch;
  }
</style>
