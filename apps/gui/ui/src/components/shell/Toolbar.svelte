<!--
  Toolbar (52px), overlaid on the page so content scrolls beneath it.
  Leading: the collapsed page title (fades in as the large title scrolls
  under) or a "‹ Back" button on detail pages. Centre: the tab strip.
  Trailing: actions the active page registered via <ToolbarActions>.
  Material + hairline fade in with the page's scroll (scroll-linked).
-->
<script lang="ts">
  import ChevronLeft from 'lucide-svelte/icons/chevron-left';
  import TabStrip from './TabStrip.svelte';
  import { ui } from '../../lib/stores/ui.svelte';

  const chrome = $derived(ui.chrome[ui.activeTab]);
  const c = $derived(Math.max(0, Math.min(1, chrome.collapse)));
  const actions = $derived(ui.actions[ui.activeTab]);
</script>

<div class="toolbar" data-tauri-drag-region>
  <div class="mat" style:opacity={c} aria-hidden="true"></div>
  <div class="lead">
    {#if chrome.back}
      <button type="button" class="back" onclick={chrome.back.onback} title="Back (Esc)">
        <ChevronLeft size={18} strokeWidth={2.25} />
        <span>{chrome.back.label}</span>
      </button>
    {/if}
    <span
      class="ctitle"
      class:shift={!!chrome.back}
      style:opacity={c}
      style:transform="translateY({(1 - c) * 6}px)"
      aria-hidden={c < 0.5}
    >
      {chrome.title}
    </span>
  </div>
  <div class="center"><TabStrip /></div>
  <div class="trail">
    {#if actions}{@render actions()}{/if}
  </div>
</div>

<style>
  .toolbar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: var(--toolbar-h);
    z-index: 20;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 16px;
    padding: 0 16px;
    -webkit-app-region: drag;
  }
  .mat {
    position: absolute;
    inset: 0;
    background: rgba(14, 16, 22, 0.62);
    -webkit-backdrop-filter: var(--mat-blur);
    backdrop-filter: var(--mat-blur);
    box-shadow: inset 0 -1px 0 var(--hair);
    pointer-events: none;
    z-index: -1;
  }
  .lead,
  .trail {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    -webkit-app-region: no-drag;
  }
  .trail {
    justify-content: flex-end;
  }
  .center {
    display: flex;
    justify-content: center;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 30px;
    padding: 0 10px 0 4px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--accent-strong);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background-color var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .back:hover {
    background: var(--accent-softer);
  }
  .back:active {
    transform: scale(0.97);
  }
  .ctitle {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--t1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    pointer-events: none;
    padding-left: 4px;
  }
  .ctitle.shift {
    padding-left: 0;
  }
</style>
