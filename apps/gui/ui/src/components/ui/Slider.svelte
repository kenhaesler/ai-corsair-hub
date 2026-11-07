<!--
  Slider: styled native range input with a filled track.
  `oninput` fires live while dragging; `oncommit` fires on release (and on
  keyboard changes). Optional `readout` snippet renders to the right; by
  default the formatted value is shown.

  <Slider label="Poll interval" min={100} max={5000} step={100} bind:value
          format={(v) => `${v}ms`} oncommit={save} />
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    label: string;
    /** hide the visible label (still used as aria-label) */
    hideLabel?: boolean;
    disabled?: boolean;
    format?: (v: number) => string;
    showValue?: boolean;
    /** fill colour of the active track */
    color?: string;
    oninput?: (v: number) => void;
    oncommit?: (v: number) => void;
    readout?: Snippet<[number]>;
    id?: string;
  }
  let {
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    label,
    hideLabel = false,
    disabled = false,
    format = (v: number) => String(v),
    showValue = true,
    color = 'var(--accent)',
    oninput,
    oncommit,
    readout,
    id = `sl-${Math.random().toString(36).slice(2, 8)}`,
  }: Props = $props();

  const pct = $derived(max === min ? 0 : ((value - min) / (max - min)) * 100);
</script>

<div class="slider" class:disabled>
  {#if !hideLabel}
    <label class="lab t-subhead" for={id}>{label}</label>
  {/if}
  <div class="row">
    <input
      {id}
      type="range"
      {min}
      {max}
      {step}
      {disabled}
      aria-label={hideLabel ? label : undefined}
      aria-valuetext={format(value)}
      bind:value
      style:--pct="{pct}%"
      style:--fill={color}
      oninput={() => oninput?.(value)}
      onchange={() => oncommit?.(value)}
    />
    {#if readout}
      {@render readout(value)}
    {:else if showValue}
      <output class="val num" for={id}>{format(value)}</output>
    {/if}
  </div>
</div>

<style>
  .slider {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .lab {
    color: var(--t2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .val {
    min-width: 56px;
    text-align: right;
    font-size: 13px;
    font-weight: 600;
    color: var(--t1);
  }
  input {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    min-width: 80px;
    height: 22px;
    margin: 0;
    background: transparent;
    cursor: pointer;
    border-radius: 999px;
  }
  input:focus-visible {
    box-shadow: none;
  }
  input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--fill) 0%,
      var(--fill) var(--pct),
      rgba(255, 255, 255, 0.12) var(--pct),
      rgba(255, 255, 255, 0.12) 100%
    );
  }
  input::-moz-range-track {
    height: 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
  }
  input::-moz-range-progress {
    height: 4px;
    border-radius: 999px;
    background: var(--fill);
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    margin-top: -7px;
    border-radius: 50%;
    background: #f4f6fb;
    border: 0;
    box-shadow:
      var(--sh-knob),
      0 0 0 0.5px rgb(0 0 0 / 0.2);
    transition:
      transform var(--dur-press) ease-out,
      box-shadow var(--dur-fast) ease-out;
  }
  input::-moz-range-thumb {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #f4f6fb;
    border: 0;
    box-shadow: var(--sh-knob);
  }
  input:active::-webkit-slider-thumb {
    transform: scale(1.08);
  }
  input:focus-visible::-webkit-slider-thumb {
    box-shadow:
      var(--sh-knob),
      var(--focus-ring);
  }
  input:focus-visible::-moz-range-thumb {
    box-shadow: var(--focus-ring);
  }
  .disabled {
    opacity: 0.45;
  }
  .disabled input {
    cursor: default;
  }
</style>
