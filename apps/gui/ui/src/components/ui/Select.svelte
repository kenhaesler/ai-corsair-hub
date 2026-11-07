<!--
  Select: styled native <select> (keeps OS keyboard behaviour).
  Options use `value: string | number | null`; null works (e.g. "All groups").
-->
<script lang="ts" generics="T extends string | number | null">
  import ChevronDown from 'lucide-svelte/icons/chevron-down';

  interface Option {
    value: T;
    label: string;
    disabled?: boolean;
  }
  interface Props {
    value: T;
    options: Option[];
    label: string;
    /** render the label visibly before the control */
    showLabel?: boolean;
    disabled?: boolean;
    size?: 'sm' | 'md';
    onchange?: (v: T) => void;
    id?: string;
  }
  let {
    value = $bindable(),
    options,
    label,
    showLabel = false,
    disabled = false,
    size = 'md',
    onchange,
    id = `sel-${Math.random().toString(36).slice(2, 8)}`,
  }: Props = $props();

  const idx = $derived(Math.max(0, options.findIndex((o) => o.value === value)));
  function change(e: Event) {
    const i = Number((e.currentTarget as HTMLSelectElement).value);
    value = options[i].value;
    onchange?.(value);
  }
</script>

<div class="sel-wrap {size}" class:disabled>
  {#if showLabel}<label for={id} class="lab">{label}</label>{/if}
  <div class="control">
    <select
      {id}
      {disabled}
      aria-label={showLabel ? undefined : label}
      value={String(idx)}
      onchange={change}
    >
      {#each options as o, i (i)}
        <option value={String(i)} disabled={o.disabled}>{o.label}</option>
      {/each}
    </select>
    <span class="chev" aria-hidden="true"><ChevronDown size={14} strokeWidth={2.25} /></span>
  </div>
</div>

<style>
  .sel-wrap {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .lab {
    font-size: 12px;
    font-weight: 500;
    color: var(--t2);
    white-space: nowrap;
  }
  .control {
    position: relative;
    min-width: 0;
  }
  select {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    border: 0;
    border-radius: 8px;
    background: var(--mat-raised);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    color: var(--t1);
    font-weight: 500;
    cursor: pointer;
    transition: background-color var(--dur-fast) ease-out;
  }
  .md select {
    height: 30px;
    padding: 0 30px 0 11px;
    font-size: 13px;
  }
  .sm select {
    height: 26px;
    padding: 0 26px 0 9px;
    font-size: 12px;
  }
  select:hover:not(:disabled) {
    background: var(--mat-raised-hover);
  }
  select:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }
  select:disabled {
    cursor: default;
  }
  option {
    background: #1b1d25;
    color: var(--t1);
  }
  .chev {
    position: absolute;
    right: 9px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--t2);
    pointer-events: none;
    display: grid;
  }
  .disabled {
    opacity: 0.45;
  }
</style>
