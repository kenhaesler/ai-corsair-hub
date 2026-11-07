<!--
  Disclosure: collapsible section with a spring-animated height.
  Header is a button (aria-expanded/aria-controls). Optional `subtitle`,
  `trailing` snippet (rendered outside the button so it can hold controls).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import { createSpring, springTo } from '../../lib/motion.svelte';

  interface Props {
    title: string;
    subtitle?: string;
    open?: boolean;
    trailing?: Snippet;
    children: Snippet;
    ontoggle?: (open: boolean) => void;
  }
  let { title, subtitle, open = $bindable(false), trailing, children, ontoggle }: Props = $props();

  const id = `dc-${Math.random().toString(36).slice(2, 8)}`;
  let inner = $state<HTMLDivElement>();
  let contentH = $state(0);
  const h = createSpring(open ? 1 : 0, { stiffness: 0.16, damping: 0.86, precision: 0.002 });
  let mounted = $state(open);

  $effect(() => {
    if (open) mounted = true;
    springTo(h, open ? 1 : 0).then(() => {
      if (!open) mounted = false;
    });
  });

  $effect(() => {
    const el = inner;
    if (!el) return;
    const ro = new ResizeObserver(() => (contentH = el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  });

  const settledOpen = $derived(open && h.current > 0.999);

  function toggle() {
    open = !open;
    ontoggle?.(open);
  }
</script>

<div class="disc" class:open>
  <div class="head">
    <button type="button" class="trigger" aria-expanded={open} aria-controls={id} onclick={toggle}>
      <span class="chev" style:transform="rotate({h.current * 90}deg)" aria-hidden="true">
        <ChevronRight size={14} strokeWidth={2.5} />
      </span>
      <span class="titles">
        <span class="t-headline">{title}</span>
        {#if subtitle}<span class="sub t-callout">{subtitle}</span>{/if}
      </span>
    </button>
    {#if trailing}<div class="trail">{@render trailing()}</div>{/if}
  </div>
  <div
    {id}
    class="body"
    role="region"
    aria-label={title}
    style:height={settledOpen ? 'auto' : `${h.current * contentH}px`}
    style:opacity={0.25 + h.current * 0.75}
    inert={!open}
  >
    {#if mounted}
      <div class="inner" bind:this={inner}>{@render children()}</div>
    {/if}
  </div>
</div>

<style>
  .disc {
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .trigger {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    margin: 0 -4px;
    border: 0;
    background: transparent;
    color: var(--t1);
    text-align: left;
    cursor: pointer;
    border-radius: 8px;
  }
  .trigger:hover .chev {
    color: var(--t1);
  }
  .chev {
    display: grid;
    place-items: center;
    width: 16px;
    color: var(--t3);
    transition: color var(--dur-fast) ease-out;
  }
  .titles {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .sub {
    color: var(--t3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .body {
    overflow: hidden;
  }
  .inner {
    padding-top: 8px;
  }
</style>
