<!--
  Preset quick-start chips for the curve editor (welcome addition, 2.3).
  Each chip shows the preset's curve shape; the one matching the group's
  current curve is highlighted; a preset applied then edited shows "· modified"
  (this replaces a separate preset badge: one home). Applying sets points, hysteresis, ramps and
  the preset's sensor weights.
-->
<script lang="ts">
  import type { FanPresetDef } from '../../lib/types';

  interface Props {
    presets: FanPresetDef[];
    active: string | null;
    /** preset applied to this group but edited since (inventory 0.7 "modified") */
    modified?: string | null;
    color: string;
    /** sensor ids available right now; a coolant preset says so when the sensor is missing */
    availableSensors?: string[];
    onapply: (p: FanPresetDef) => void;
  }
  let { presets, active, modified = null, color, availableSensors = [], onapply }: Props = $props();
  const needsCoolant = (p: FanPresetDef) => p.water_weight > 0 && !availableSensors.includes('water_inline');

  function thumb(p: FanPresetDef): string {
    const ts = p.points.map((q) => q.temp);
    const lo = Math.min(...ts);
    const span = Math.max(1, Math.max(...ts) - lo);
    return p.points
      .map((q, i) => `${i ? 'L' : 'M'}${(((q.temp - lo) / span) * 22 + 1).toFixed(1)},${(13 - (q.duty / 100) * 12).toFixed(1)}`)
      .join('');
  }
</script>

<div class="chips" role="group" aria-label="Start from a preset" style:--gc={color}>
  <span class="lab">Start from</span>
  {#each presets as p (p.id)}
    <button
      type="button"
      class="chip"
      class:on={active === p.id}
      class:mod={modified === p.id}
      aria-pressed={active === p.id}
      title="{p.description}. Applies the curve, hysteresis, ramp rates and sensor weights.{modified === p.id
        ? ' Applied to this group, then modified.'
        : ''}{needsCoolant(p) ? ' No coolant sensor is connected, so it would follow CPU and GPU instead.' : ''}"
      onclick={() => onapply(p)}
    >
      <svg width="24" height="14" viewBox="0 0 24 14" aria-hidden="true"><path d={thumb(p)} /></svg>
      {p.name}{#if modified === p.id}<span class="mod-t">· modified</span>{/if}
    </button>
  {/each}
</div>

<style>
  .chips {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
    padding: 2px 3px;
    margin: -2px -3px;
  }
  .chips::-webkit-scrollbar {
    display: none;
  }
  .lab {
    flex: none;
    font-size: 12px;
    font-weight: 500;
    color: var(--t3);
    margin-right: 2px;
  }
  .chip {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px 0 7px;
    border: 0;
    border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px var(--hair);
    color: var(--t2);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .chip svg path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.7;
  }
  .chip:hover {
    background: rgba(255, 255, 255, 0.09);
    color: var(--t1);
  }
  .chip:active {
    transform: scale(0.96);
  }
  .chip:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }
  .chip.on {
    background: color-mix(in srgb, var(--gc) 18%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--gc) 50%, transparent);
    color: var(--t1);
  }
  .chip.mod {
    outline: 1px dashed var(--warn-hair);
    outline-offset: -1px;
    color: var(--t1);
  }
  .mod-t {
    color: var(--warn-text);
    font-weight: 500;
  }
  .chip.on svg path {
    stroke: var(--gc);
    opacity: 1;
  }
</style>
