<!--
  InsetRow: one row of an InsetGroup. Label (+ optional sublabel / icon) on
  the left, control (children) on the right. With `onclick` the whole row
  becomes a button with a chevron. `stacked` puts children below the label
  (for wide controls like sliders or editors).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import type { IconComponent } from '../../lib/icon';

  interface Props {
    label: string;
    sublabel?: string;
    icon?: IconComponent;
    /** tile colour behind the icon */
    iconColor?: string;
    stacked?: boolean;
    onclick?: () => void;
    /** id of the control this row labels (renders a <label for>) */
    for?: string;
    children?: Snippet;
  }
  let { label, sublabel, icon: Icon, iconColor = 'var(--accent)', stacked = false, onclick, for: forId, children }: Props = $props();
</script>

{#snippet lead()}
  {#if Icon}
    <span class="tile" style:background={iconColor} aria-hidden="true"><Icon size={14} strokeWidth={2.25} /></span>
  {/if}
  <span class="labels">
    {#if forId}
      <label class="label" for={forId}>{label}</label>
    {:else}
      <span class="label">{label}</span>
    {/if}
    {#if sublabel}<span class="sub">{sublabel}</span>{/if}
  </span>
{/snippet}

{#if onclick}
  <button type="button" class="row clickable" {onclick}>
    {@render lead()}
    <span class="ctrl">
      {#if children}{@render children()}{/if}
      <ChevronRight size={14} strokeWidth={2.25} class="chev" />
    </span>
  </button>
{:else}
  <div class="row" class:stacked>
    <div class="lead">{@render lead()}</div>
    {#if children}<div class="ctrl">{@render children()}</div>{/if}
  </div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 44px;
    padding: 8px 14px;
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: var(--t1);
  }
  .row.stacked {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
    padding: 12px 14px 14px;
  }
  .lead {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .clickable {
    cursor: pointer;
    transition: background-color var(--dur-fast) ease-out;
  }
  .clickable:hover {
    background: rgba(255, 255, 255, 0.035);
  }
  .clickable:active {
    background: rgba(255, 255, 255, 0.06);
  }
  .clickable:focus-visible {
    box-shadow: inset var(--focus-ring);
  }
  .tile {
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    color: #0b0d12;
  }
  .labels {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .label {
    font-size: 13px;
    font-weight: 500;
    line-height: 18px;
    color: var(--t1);
  }
  .sub {
    font-size: 12px;
    line-height: 16px;
    color: var(--t3);
  }
  .ctrl {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: var(--t2);
    font-size: 13px;
  }
  .row :global(.chev) {
    color: var(--t3);
  }
</style>
