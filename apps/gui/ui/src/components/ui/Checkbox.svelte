<!--
  Checkbox: native input (keyboard + forms) with a custom box. The whole
  row is the hit target. Children render as the label.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Check from 'lucide-svelte/icons/check';
  import Minus from 'lucide-svelte/icons/minus';

  interface Props {
    checked: boolean;
    indeterminate?: boolean;
    disabled?: boolean;
    /** accessible name when there is no visible label */
    label?: string;
    onchange?: (v: boolean) => void;
    children?: Snippet;
  }
  let { checked = $bindable(), indeterminate = false, disabled = false, label, onchange, children }: Props = $props();
</script>

<label class="cb" class:disabled>
  <input
    type="checkbox"
    bind:checked
    {indeterminate}
    {disabled}
    aria-label={children ? undefined : label}
    onchange={() => onchange?.(checked)}
  />
  <span class="box" class:on={checked || indeterminate} aria-hidden="true">
    {#if indeterminate}
      <Minus size={12} strokeWidth={3} />
    {:else if checked}
      <Check size={12} strokeWidth={3} />
    {/if}
  </span>
  {#if children}<span class="text">{@render children()}</span>{/if}
</label>

<style>
  .cb {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    min-width: 0;
    font-size: 13px;
    color: var(--t1);
  }
  .disabled {
    opacity: 0.45;
    cursor: default;
  }
  input {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
    pointer-events: none;
  }
  .box {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    color: var(--accent-ink);
    transition:
      background-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .cb:hover:not(.disabled) .box {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.24);
  }
  .box.on {
    background: var(--accent);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.25);
  }
  .cb:active:not(.disabled) .box {
    transform: scale(0.92);
  }
  input:focus-visible + .box {
    box-shadow: var(--focus-ring);
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
