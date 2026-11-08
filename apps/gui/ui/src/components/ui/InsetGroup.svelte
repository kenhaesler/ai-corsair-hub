<!--
  InsetGroup: the "technical luminous" panel every page sections with.
  Graphite card with the hairline column grid, a 44px header row in mono
  micro-caps (title left, optional action right) and an optional footer
  caption below. Children are usually InsetRow components; rows get
  hairline separators automatically.
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
    /** no hairlines between direct children (for custom bodies) */
    flush?: boolean;
  }
  let { title, footer, action, children, id, flush = false }: Props = $props();
  const hid = $derived(id ? `${id}-title` : undefined);
</script>

<section class="ig" {id} aria-labelledby={title ? hid : undefined}>
  <div class="ig-card">
    {#if title || action}
      <div class="ig-head">
        {#if title}<h3 class="ig-title" id={hid}>{title}</h3>{/if}
        {#if action}<div class="ig-action">{@render action()}</div>{/if}
      </div>
    {/if}
    <div class="ig-body" class:flush>
      {@render children()}
    </div>
  </div>
  {#if footer}<p class="ig-foot t-callout">{footer}</p>{/if}
</section>

<style>
  .ig {
    min-width: 0;
  }
  .ig-card {
    border-radius: 16px;
    background: var(--panel-grid), var(--panel);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      0 24px 48px -32px rgb(0 0 0 / 0.9);
    overflow: hidden;
  }
  .ig-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 44px;
    padding: 0 18px;
    border-bottom: 1px solid var(--hair);
  }
  .ig-title {
    flex: none;
    font: 500 11px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t2);
  }
  .ig-action {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .ig-body:not(.flush) > :global(* + *) {
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .ig-foot {
    color: var(--t3);
    padding: 8px 4px 0;
    max-width: 75ch;
  }
</style>
