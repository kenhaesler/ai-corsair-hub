<!--
  NumberField: numeric input with unit suffix and ± steppers. Value is
  clamped to [min,max] and committed on blur / Enter / stepper click
  (`onchange`). ArrowUp/Down step (Shift = ×10).
-->
<script lang="ts">
  import Minus from 'lucide-svelte/icons/minus';
  import Plus from 'lucide-svelte/icons/plus';

  interface Props {
    value: number;
    label: string;
    showLabel?: boolean;
    min?: number;
    max?: number;
    step?: number;
    decimals?: number;
    unit?: string;
    disabled?: boolean;
    steppers?: boolean;
    width?: number;
    onchange?: (v: number) => void;
    id?: string;
  }
  let {
    value = $bindable(),
    label,
    showLabel = false,
    min = -Infinity,
    max = Infinity,
    step = 1,
    decimals = 0,
    unit = '',
    disabled = false,
    steppers = true,
    width = 72,
    onchange,
    id = `nf-${Math.random().toString(36).slice(2, 8)}`,
  }: Props = $props();

  let text = $state('');
  let editing = $state(false);
  $effect(() => {
    if (!editing) text = Number.isFinite(value) ? value.toFixed(decimals) : '';
  });

  function commit(v: number) {
    if (!Number.isFinite(v)) {
      text = value.toFixed(decimals);
      return;
    }
    const q = Math.round(v / step) * step;
    const c = Math.min(max, Math.max(min, Number(q.toFixed(Math.max(decimals, 4)))));
    value = c;
    text = c.toFixed(decimals);
    onchange?.(c);
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      commit(parseFloat(text));
      (e.currentTarget as HTMLInputElement).select();
    } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const d = (e.key === 'ArrowUp' ? 1 : -1) * step * (e.shiftKey ? 10 : 1);
      commit((parseFloat(text) || value) + d);
    } else if (e.key === 'Escape') {
      text = value.toFixed(decimals);
      (e.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<div class="nf" class:disabled>
  {#if showLabel}<label class="lab" for={id}>{label}</label>{/if}
  <div class="control">
    {#if steppers}
      <button
        type="button"
        class="st"
        aria-label="Decrease {label}"
        tabindex="-1"
        disabled={disabled || value <= min}
        onclick={() => commit(value - step)}><Minus size={12} strokeWidth={2.5} /></button
      >
    {/if}
    <div class="field" style:width="{width}px">
      <input
        {id}
        type="text"
        inputmode="decimal"
        autocomplete="off"
        spellcheck="false"
        class="num"
        aria-label={showLabel ? undefined : label}
        {disabled}
        bind:value={text}
        onfocus={(e) => {
          editing = true;
          (e.currentTarget as HTMLInputElement).select();
        }}
        onblur={() => {
          editing = false;
          commit(parseFloat(text));
        }}
        onkeydown={onKey}
      />
      {#if unit}<span class="unit" aria-hidden="true">{unit}</span>{/if}
    </div>
    {#if steppers}
      <button
        type="button"
        class="st"
        aria-label="Increase {label}"
        tabindex="-1"
        disabled={disabled || value >= max}
        onclick={() => commit(value + step)}><Plus size={12} strokeWidth={2.5} /></button
      >
    {/if}
  </div>
</div>

<style>
  .nf {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .lab {
    font-size: 12px;
    font-weight: 500;
    color: var(--t2);
  }
  .control {
    display: inline-flex;
    align-items: center;
    height: 30px;
    border-radius: 8px;
    background: var(--mat-raised);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
  }
  .control:focus-within {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }
  .field {
    position: relative;
    display: flex;
    align-items: center;
    height: 100%;
  }
  input {
    width: 100%;
    height: 100%;
    border: 0;
    background: transparent;
    text-align: right;
    padding: 0 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--t1);
  }
  input:focus-visible {
    box-shadow: none;
  }
  .unit {
    font-size: 12px;
    color: var(--t3);
    padding-right: 6px;
    white-space: nowrap;
  }
  .st {
    width: 24px;
    height: 100%;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--t2);
    cursor: pointer;
    border-radius: 8px;
    transition:
      background-color var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out;
  }
  .st:hover:not(:disabled) {
    color: var(--t1);
    background: rgba(255, 255, 255, 0.06);
  }
  .st:active:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
  }
  .st:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .disabled {
    opacity: 0.45;
  }
</style>
