<!--
  InsetGroup: System-Settings-style grouped list section. Optional title
  above, footer caption below. Children are usually InsetRow components;
  rows get hairline separators automatically.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title?: string;
    footer?: string;
    /** trailing content in the title row (e.g. a button) */
    action?: Snippet;
    children: Snippet;
    id?: string;
  }
  let { title, footer, action, children, id }: Props = $props();
  const hid = $derived(id ? `${id}-title` : undefined);
</script>

<section class="ig" {id} aria-labelledby={title ? hid : undefined}>
  {#if title || action}
    <div class="ig-head">
      {#if title}<h3 class="ig-title t-headline" id={hid}>{title}</h3>{/if}
      {#if action}<div class="ig-action">{@render action()}</div>{/if}
    </div>
  {/if}
  <div class="ig-body">
    {@render children()}
  </div>
  {#if footer}<p class="ig-foot t-callout">{footer}</p>{/if}
</section>

<style>
  .ig {
    min-width: 0;
  }
  .ig-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 4px 8px;
    min-height: 28px;
  }
  .ig-title {
    color: var(--t1);
  }
  .ig-body {
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    overflow: hidden;
  }
  .ig-body > :global(* + *) {
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .ig-foot {
    color: var(--t3);
    padding: 8px 4px 0;
    max-width: 75ch;
  }
</style>
