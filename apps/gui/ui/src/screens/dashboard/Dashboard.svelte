<!--
  Dashboard ("Summary"). Alert rows, Pinned highlights grouped as
  Temperatures / Fans / Power Supply (the PSU total row plus its rails and
  Input / Fan breakdown, merged into one group), Quick Presets, History.
  No redundancy: each datum has one home on this screen (status bar hidden
  here, scrub time only in the titlebar pill, History end labels on hover).
  Clicking a highlight morphs into its detail page. Everything reads
  `display`, so the whole page follows the time-scrub.
-->
<script lang="ts">
  import ArrowUpRight from 'lucide-svelte/icons/arrow-up-right';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import ToolbarActions from '../../components/shell/ToolbarActions.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import { display } from '../../lib/stores/scrub.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { meanDuty } from '../../lib/format';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { buildHighlights } from './highlights';
  import AlertRows from './AlertRows.svelte';
  import PresetShelf from './PresetShelf.svelte';
  import HistorySection from './HistorySection.svelte';
  import Detail from './Detail.svelte';
  import SpecPanel from './SpecPanel.svelte';
  import ControlPath from './ControlPath.svelte';
  import FiguresRow from './FiguresRow.svelte';
  import { headline, pathSources, reason } from './overview';

  const all = $derived(buildHighlights(display.live));
  const detailH = $derived(ui.detail ? all.find((h) => h.key === ui.detail) : undefined);

  const s = $derived(display.snapshot);
  const emergency = $derived(!!sensors.snapshot?.emergency && !display.scrubbing);
  // Emergency / stale are owned by their alert rows; the reason line steps aside.
  const alertOwnsState = $derived(!!sensors.snapshot?.emergency || !!sensors.snapshot?.any_stale);
  const hl = $derived(s ? headline(s, emergency, meanDuty(s)) : { lead: '', em: '', tail: '' });
  const why = $derived(s ? reason(s, configStore.saved, emergency) : []);
  const nSources = $derived(s ? pathSources(configStore.saved, s).length : 0);

  let presetsEl = $state<HTMLElement>();
  let historyEl = $state<HTMLElement>();
  const jump = (el: HTMLElement | undefined) => el?.scrollIntoView({ behavior: 'smooth', block: 'start' });

  // If the detail target disappears (group deleted), go back to Summary.
  $effect(() => {
    if (ui.detail && display.live && !detailH) ui.detail = null;
  });
  // Back from Detail: focus returns to the row that opened it.
  let wasDetail = false;
  $effect(() => {
    const now = !!detailH;
    if (wasDetail && !now) {
      const el = ui.detailOpener;
      ui.detailOpener = null;
      if (el && document.contains(el)) queueMicrotask(() => el.focus({ preventScroll: true }));
    }
    wasDetail = now;
  });
</script>

<ToolbarActions active={!detailH}>
  <button type="button" class="btn-history" onclick={() => jump(historyEl)}>
    History<span class="tile"><ArrowUpRight size={13} strokeWidth={2.4} /></span>
  </button>
</ToolbarActions>

<div class="summary" class:hidden={!!detailH}>
  {#snippet header()}
    <header class="intro">
      <h1>
        {#if s}{hl.lead}<em>{hl.em}</em>{hl.tail}{:else}Connecting to your hardware…{/if}
      </h1>
      {#if s && !alertOwnsState}
        <p class="lede">
          {#each why as c, i (i)}{#if c.strong}<b>{c.text}</b>{:else}{c.text}{/if}{/each}
        </p>
      {/if}
    </header>
  {/snippet}
  <LargeTitlePage title="Summary" active={!detailH} maxWidth={1440} {header}>
    {#if !s}
      <div class="connecting" role="status">
        <Spinner size={18} />
        <span>Waiting for the first reading from the hubs…</span>
      </div>
    {:else}
      <AlertRows />
      <div class="main">
        <section class="card" aria-label="Readings"><SpecPanel /></section>
        <section class="card path" aria-labelledby="path-h">
          <div class="chd">
            <span id="path-h">Control path</span>
            <span class="live" class:past={display.scrubbing}><i></i>{display.scrubbing ? 'Rewound' : 'Live'}</span>
          </div>
          <ControlPath />
          <div class="cft">
            <span>{nSources} source{nSources === 1 ? '' : 's'} → control loop → {s.group_duties.length} group{s.group_duties.length === 1 ? '' : 's'}</span>
            <span>{s.fans.length} devices</span>
          </div>
        </section>
      </div>
      <FiguresRow onpresets={() => jump(presetsEl)} />

      <div class="below" bind:this={presetsEl}><PresetShelf /></div>
      <div bind:this={historyEl}><HistorySection /></div>
    {/if}
  </LargeTitlePage>
</div>

{#if detailH}
  <div class="detail">
    {#key detailH.key}
      <Detail h={detailH} morph={ui.morphKey === detailH.key} />
    {/key}
  </div>
{/if}

<style>
  .summary,
  .detail {
    position: absolute;
    inset: 0;
  }
  .summary.hidden {
    display: none;
  }
  /* intro: state headline (state-coloured emphasis) + the plain-words reason */
  .intro {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 440px;
    gap: 56px;
    align-items: end;
    padding: 2px 24px 26px;
  }
  .intro h1 {
    font-weight: 300;
    font-size: 50px;
    line-height: 1.04;
    letter-spacing: -0.035em;
    color: var(--t1);
  }
  .intro h1 em {
    font-style: normal;
    font-weight: 400;
    color: color-mix(in oklab, var(--sc) 62%, #fff);
    transition: color 1.2s;
  }
  .lede {
    font-size: 15px;
    line-height: 1.55;
    color: var(--t2);
  }
  .lede b {
    color: var(--t1);
    font-weight: 500;
  }
  .main {
    display: grid;
    grid-template-columns: minmax(0, 1.38fr) minmax(0, 1fr);
    gap: 20px;
    height: 452px;
    margin-bottom: 20px;
  }
  @media (max-width: 1180px) {
    .main {
      grid-template-columns: minmax(0, 1fr);
      height: auto;
    }
    .main .card {
      height: 452px;
    }
    .main .card.path {
      height: 380px;
    }
    .intro {
      grid-template-columns: minmax(0, 1fr);
      gap: 12px;
      padding-bottom: 20px;
    }
    .intro h1 {
      font-size: 40px;
    }
    .lede {
      max-width: 70ch;
    }
  }
  .card {
    position: relative;
    display: grid;
    min-height: 0;
    border-radius: 16px;
    background: var(--panel-grid), var(--panel);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      0 24px 48px -32px rgb(0 0 0 / 0.9);
    overflow: hidden;
  }
  .card.path {
    grid-template-rows: 44px minmax(0, 1fr) auto;
  }
  .chd,
  .cft {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 18px;
    font: 500 11px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t2);
  }
  .chd {
    border-bottom: 1px solid var(--hair);
  }
  .cft {
    height: 40px;
    border-top: 1px solid var(--hair);
    letter-spacing: 0.04em;
    text-transform: none;
    color: var(--t3);
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--t3);
  }
  .live i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #55e39a;
    box-shadow: 0 0 8px #55e39a;
    animation: blink 2s ease-in-out infinite;
  }
  .live.past i {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
    animation: none;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .below {
    scroll-margin-top: 80px;
  }
  .btn-history {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 5px 0 14px;
    border: 0;
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: transform var(--dur-press) ease-out;
  }
  .btn-history:active {
    transform: scale(0.97);
  }
  .btn-history .tile {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: #0b0c0f;
    color: #fff;
  }
  @media (prefers-reduced-motion: reduce) {
    .live i {
      animation: none;
    }
  }
  .connecting {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 240px;
    color: var(--t2);
  }
</style>
