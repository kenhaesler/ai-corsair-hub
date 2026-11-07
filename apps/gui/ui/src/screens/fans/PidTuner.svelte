<!--
  PidTuner (inventory 2.6): Target Temp slider 30–85 °C, Kp (0.1–20, 0.1),
  Ki (0–5, 0.01), Kd (0–10, 0.1) number inputs, Min/Max Duty sliders 0–100.
  The live sensor-mix temperature rides on the Target Temp track itself (its
  single home in PID mode; reads display, so it follows the time-scrub), and
  a short sentence states only the derived gap to target.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Slider from '../../components/ui/Slider.svelte';
  import NumberField from '../../components/ui/NumberField.svelte';
  import { thermalColor } from '../../lib/format';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';

  type Vals = { targetTemp: number; kp: number; ki: number; kd: number; minDuty: number; maxDuty: number };
  interface Props extends Vals {
    color?: string;
    /** live mix temperature (°C) or null */
    liveTemp: number | null;
    onchange: (v: Vals) => void;
    oncheckpoint?: (key: string) => void;
  }
  let { targetTemp, kp, ki, kd, minDuty, maxDuty, color = 'var(--accent)', liveTemp, onchange, oncheckpoint }: Props =
    $props();

  function emit(over: Partial<Vals>, key: string) {
    oncheckpoint?.(`pid-${key}`);
    onchange({ targetTemp, kp, ki, kd, minDuty, maxDuty, ...over });
  }

  // Marker geometry on the Target Temp slider (30–85 °C). The Slider row is
  // [range input][12px gap][56px readout]; the thumb is 18px wide.
  const T0 = 30;
  const T1 = 85;
  const frac = (t: number) => (Math.min(T1, Math.max(T0, t)) - T0) / (T1 - T0);
  let rowW = $state(600);
  const trackW = $derived(Math.max(40, rowW - 68));
  const live = createSpring(
    untrack(() => frac(liveTemp ?? targetTemp)),
    springs.bar,
  );
  $effect(() => {
    if (liveTemp !== null) springTo(live, frac(liveTemp));
  });
  const markerX = $derived(9 + live.current * (trackW - 18));
  const outOfRange = $derived(liveTemp !== null && (liveTemp < T0 || liveTemp > T1));

  const delta = $derived(liveTemp !== null ? liveTemp - targetTemp : null);
  const sentence = $derived.by(() => {
    if (delta === null) return 'Waiting for a sensor reading.';
    if (Math.abs(delta) < 0.5) return 'Holding on target.';
    const d = Math.abs(delta).toFixed(1);
    return delta < 0 ? `${d}° under target, so the loop eases fans toward their minimum.` : `${d}° over target, so the loop pushes fans harder.`;
  });
</script>

<div class="pid">
  <div class="target">
    <div class="target-sl" bind:clientWidth={rowW}>
      <Slider
        label="Target Temp"
        min={30}
        max={85}
        step={1}
        {color}
        value={targetTemp}
        format={(v) => `${v}°C`}
        oninput={(v) => emit({ targetTemp: v }, 'target')}
      />
      {#if liveTemp !== null}
        <div class="marker" style:left="{markerX}px" style:--lc={thermalColor(liveTemp)} aria-hidden="true">
          <span class="mdot"></span>
          <span class="mlabel num">Mix {liveTemp.toFixed(1)}°{outOfRange ? (liveTemp < T0 ? ' ‹' : ' ›') : ''}</span>
        </div>
      {/if}
    </div>
    <p class="sentence t-callout" aria-live="polite">{sentence}</p>
  </div>

  <div class="grid">
    <div class="gains">
      <div class="gain">
        <NumberField label="Kp" showLabel value={kp} min={0.1} max={20} step={0.1} decimals={1} width={64} onchange={(v) => emit({ kp: v }, 'kp')} />
        <span class="gh">Proportional</span>
      </div>
      <div class="gain">
        <NumberField label="Ki" showLabel value={ki} min={0} max={5} step={0.01} decimals={2} width={64} onchange={(v) => emit({ ki: v }, 'ki')} />
        <span class="gh">Integral</span>
      </div>
      <div class="gain">
        <NumberField label="Kd" showLabel value={kd} min={0} max={10} step={0.1} decimals={1} width={64} onchange={(v) => emit({ kd: v }, 'kd')} />
        <span class="gh">Derivative</span>
      </div>
    </div>
    <div class="cell">
      <Slider
        label="Min Duty"
        min={0}
        max={100}
        step={1}
        {color}
        value={minDuty}
        format={(v) => `${v}%`}
        oninput={(v) => emit({ minDuty: v }, 'min')}
      />
    </div>
    <div class="cell">
      <Slider
        label="Max Duty"
        min={0}
        max={100}
        step={1}
        {color}
        value={maxDuty}
        format={(v) => `${v}%`}
        oninput={(v) => emit({ maxDuty: v }, 'max')}
      />
    </div>
    {#if minDuty > maxDuty}
      <p class="warn wide" role="status">Min duty is above max duty. Saving will be rejected until it is lower.</p>
    {/if}
  </div>
</div>

<style>
  .pid {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 16px 14px 18px;
  }
  .target {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .target-sl {
    position: relative;
    padding-bottom: 22px;
  }
  .marker {
    position: absolute;
    /* label row (16) + gap (6) + half the 22px input row */
    top: 33px;
    width: 0;
    pointer-events: none;
  }
  .mdot {
    position: absolute;
    left: -5px;
    top: -5px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--lc);
    box-shadow:
      0 0 0 2px #0d0f14,
      0 2px 6px -1px rgb(0 0 0 / 0.6);
  }
  .mlabel {
    position: absolute;
    top: 12px;
    left: 0;
    transform: translateX(-50%);
    font-size: 11px;
    font-weight: 600;
    color: var(--t2);
    white-space: nowrap;
  }
  .sentence {
    color: var(--t2);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 18px 32px;
    padding-top: 18px;
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .wide {
    grid-column: 1 / -1;
  }
  .cell {
    min-width: 0;
  }
  .gains {
    grid-column: 1 / -1;
    display: flex;
    gap: 28px;
    flex-wrap: wrap;
  }
  .gain {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .gain :global(.lab) {
    min-width: 18px;
    font-weight: 600;
    color: var(--t1);
  }
  .gh {
    font-size: 12px;
    color: var(--t3);
  }
  .warn {
    font-size: 12px;
    color: var(--warn-text);
  }
</style>
