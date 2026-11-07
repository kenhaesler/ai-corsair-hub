<!--
  Sheet: modal dialog. Backdrop blur fades in; the panel springs from
  scale .96 to 1. Focus is trapped and restored; Escape and backdrop click
  call `onclose` (unless `dismissable` is false). Rendered in a portal.

  <Sheet bind:open title="Devices in Radiator Fans" onclose={...}>
    ...body...
    {#snippet footer()}<Button>Cancel</Button><Button variant="primary">Save</Button>{/snippet}
  </Sheet>
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import X from 'lucide-svelte/icons/x';
  import { portal, focusTrap } from '../../lib/dom';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';

  interface Props {
    open: boolean;
    title?: string;
    subtitle?: string;
    width?: number;
    dismissable?: boolean;
    /** show the × close button in the header */
    closeButton?: boolean;
    role?: 'dialog' | 'alertdialog';
    /** ids for aria-labelledby / aria-describedby when using a custom header */
    labelledby?: string;
    describedby?: string;
    onclose?: () => void;
    onkeydown?: (e: KeyboardEvent) => void;
    header?: Snippet;
    children: Snippet;
    footer?: Snippet;
  }
  let {
    open = $bindable(),
    title,
    subtitle,
    width = 520,
    dismissable = true,
    closeButton = true,
    role = 'dialog',
    labelledby,
    describedby,
    onclose,
    onkeydown,
    header,
    children,
    footer,
  }: Props = $props();

  const tid = `sh-${Math.random().toString(36).slice(2, 8)}`;
  const p = createSpring(0, { ...springs.sheet, precision: 0.002 });
  let mounted = $state(false);

  $effect(() => {
    if (open) {
      mounted = true;
      springTo(p, 1);
    } else if (mounted) {
      springTo(p, 0).then(() => {
        if (!open) mounted = false;
      });
    }
  });

  function close() {
    if (!dismissable) return;
    onclose?.();
  }
  function onKey(e: KeyboardEvent) {
    onkeydown?.(e);
    if (e.defaultPrevented) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }
  const backdrop = $derived(Math.min(1, Math.max(0, p.current)));
</script>

{#if mounted}
  <div class="layer" use:portal>
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="backdrop" style:opacity={backdrop} onclick={close}></div>
    <div
      class="panel"
      {role}
      aria-modal="true"
      aria-labelledby={labelledby ?? (title ? tid : undefined)}
      aria-describedby={describedby}
      tabindex="-1"
      style:width="min({width}px, calc(100vw - 48px))"
      style:opacity={backdrop}
      style:transform="translate(-50%, -50%) scale({0.96 + 0.04 * p.current})"
      inert={!open}
      use:focusTrap
      onkeydown={onKey}
    >
      {#if header}
        {@render header()}
      {:else if title}
        <header class="head">
          <div class="titles">
            <h2 id={tid} class="t-title3">{title}</h2>
            {#if subtitle}<p class="sub t-callout">{subtitle}</p>{/if}
          </div>
          {#if closeButton && dismissable}
            <button type="button" class="x" aria-label="Close" title="Close" onclick={close}>
              <X size={15} strokeWidth={2.25} />
            </button>
          {/if}
        </header>
      {/if}
      <div class="body">{@render children()}</div>
      {#if footer}<footer class="foot">{@render footer()}</footer>{/if}
    </div>
  </div>
{/if}

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 100;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgba(4, 5, 8, 0.52);
    -webkit-backdrop-filter: blur(10px) saturate(120%);
    backdrop-filter: blur(10px) saturate(120%);
  }
  .panel {
    position: absolute;
    left: 50%;
    top: 50%;
    max-height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    border-radius: var(--r-sheet);
    background: var(--mat-sheet);
    -webkit-backdrop-filter: blur(40px) saturate(180%);
    backdrop-filter: blur(40px) saturate(180%);
    box-shadow:
      var(--sh-pop),
      inset 0 1px 0 rgba(255, 255, 255, 0.07);
    overscroll-behavior: contain;
    will-change: transform, opacity;
  }
  .panel:focus-visible {
    box-shadow: var(--sh-pop);
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 20px 4px 24px;
  }
  .titles {
    min-width: 0;
  }
  .sub {
    color: var(--t2);
    margin-top: 4px;
  }
  .x {
    flex: none;
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    color: var(--t2);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out;
  }
  .x:hover {
    background: rgba(255, 255, 255, 0.14);
    color: var(--t1);
  }
  .body {
    padding: 12px 24px 20px;
    overflow: auto;
    min-height: 0;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
    padding: 14px 20px;
    box-shadow: inset 0 1px 0 var(--hair);
  }
</style>
