<!--
  LargeTitlePage: the page frame every screen uses.
  - scroll container that runs beneath the translucent toolbar
  - global banners (startup config issue) at the top, on every screen
  - 34px large title + subtitle (string or snippet) + optional trailing
    header content (e.g. an "Edit" button)
  - scroll-linked collapse: as the title passes under the toolbar the
    toolbar gains its material and a 15px copy of the title fades in
  - `back` shows "‹ label" in the toolbar (Esc also triggers it)

  <LargeTitlePage title="Fan Control" subtitle="3 groups · 10 fans">...</LargeTitlePage>
-->
<script lang="ts">
  import { getContext, untrack, type Snippet } from 'svelte';
  import { ui, type Tab } from '../../lib/stores/ui.svelte';
  import { PAGE_TAB } from './context';
  import GlobalBanners from './GlobalBanners.svelte';

  interface Props {
    title: string;
    subtitle?: string | Snippet;
    back?: { label: string; onback: () => void } | null;
    /** when false, this instance stops driving the toolbar (e.g. hidden summary under a detail page) */
    active?: boolean;
    headerTrailing?: Snippet;
    /** view-transition-name for the title block */
    titleTransition?: string;
    maxWidth?: number;
    children: Snippet;
  }
  let {
    title,
    subtitle,
    back = null,
    active = true,
    headerTrailing,
    titleTransition,
    maxWidth,
    children,
  }: Props = $props();

  const tab = getContext<Tab | undefined>(PAGE_TAB) ?? 'dashboard';
  let scroller: HTMLDivElement;
  let titleEl: HTMLHeadingElement;
  let collapse = $state(0);

  let lastScroll = 0;

  // Restore scroll when re-activated (e.g. back from a detail page that hid this one).
  $effect(() => {
    if (active && scroller && lastScroll > 0 && scroller.scrollTop === 0) {
      scroller.scrollTop = lastScroll;
      measure();
    }
  });

  function measure() {
    if (!scroller || !titleEl) return;
    if (active && scroller.offsetParent !== null) lastScroll = scroller.scrollTop;
    const toolbarH = 52;
    // title's bottom edge in scroll coordinates
    const bottom = titleEl.offsetTop + titleEl.offsetHeight;
    const start = bottom - toolbarH - 24;
    collapse = Math.max(0, Math.min(1, (scroller.scrollTop - start) / 24));
  }

  $effect(() => {
    if (!active) return;
    const next = { title, collapse, back };
    untrack(() => {
      ui.chrome[tab] = next;
    });
  });
</script>

<div class="page-scroll" bind:this={scroller} onscroll={measure}>
  <div class="page-inner" style:max-width={maxWidth ? `${maxWidth}px` : undefined}>
    <GlobalBanners />
    <header class="lt-head">
      <div class="lt-titles" style:view-transition-name={titleTransition}>
        <h1 class="t-large-title" bind:this={titleEl}>{title}</h1>
        {#if typeof subtitle === 'string'}
          <p class="lt-sub">{subtitle}</p>
        {:else if subtitle}
          <div class="lt-sub">{@render subtitle()}</div>
        {/if}
      </div>
      {#if headerTrailing}<div class="lt-trail">{@render headerTrailing()}</div>{/if}
    </header>
    {@render children()}
  </div>
</div>

<style>
  .page-scroll {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
    scroll-padding-top: calc(var(--toolbar-h) + 16px);
    overscroll-behavior: contain;
  }
  .page-inner {
    max-width: calc(var(--content-max) + var(--gutter) * 2);
    margin: 0 auto;
    padding: calc(var(--toolbar-h) + 20px) var(--gutter) 64px;
  }
  .lt-head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: 24px;
  }
  .lt-titles {
    min-width: 0;
  }
  h1 {
    color: var(--t1);
  }
  .lt-sub {
    margin-top: 4px;
    font-size: 15px;
    line-height: 21px;
    color: var(--t2);
    font-variant-numeric: tabular-nums;
  }
  .lt-trail {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding-bottom: 4px;
  }
</style>
