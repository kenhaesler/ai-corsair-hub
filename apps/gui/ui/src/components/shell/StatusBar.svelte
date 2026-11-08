<!--
  StatusBar (inventory 1.7), 28px. Status dot red >85 °C, amber >70 °C,
  else green; one item per temperature ("CPU 45.2°C"); "N groups @ X%";
  PSU W. Reads `display`, so it follows the time-scrub.

  No-redundancy rule: the bar only shows what the active tab does not show
  itself (static per-tab map below). Hidden entirely on the Dashboard, whose
  Pinned rows are the single home of these values (the status dot leads the
  Summary subtitle there). Scrub time lives only in the titlebar pill.
-->
<script lang="ts">
  import { display } from '../../lib/stores/scrub.svelte';
  import { ui, type Tab } from '../../lib/stores/ui.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { meanDuty, sourceLabel, statusLevel } from '../../lib/format';

  interface Hide {
    all?: boolean;
    temps?: string[];
    groups?: boolean;
    psu?: boolean;
  }
  const HIDE: Record<Tab, Hide> = {
    dashboard: { all: true },
    fans: { groups: true },
    devices: { psu: true },
    lighting: {},
    settings: {},
  };

  // On Fan Control the selected group's editor shows the sensors it listens
  // to (curve marker / PID track), so those are hidden here. A fixed group
  // shows none, so nothing is hidden.
  const fanHidden = $derived.by((): string[] => {
    const g = configStore.config?.fan_groups.find((x) => x.name === ui.fanSelectedGroup);
    if (!g || g.mode.type === 'fixed') return [];
    const src = g.mode.temp_source.sensors;
    // the curve editor's secondary marker is the raw CPU reading
    return g.mode.type === 'curve' ? [...new Set([...src, 'cpu'])] : [...src];
  });
  const hide = $derived<Hide>(ui.activeTab === 'fans' ? { ...HIDE.fans, temps: fanHidden } : HIDE[ui.activeTab]);
  const s = $derived(display.snapshot);
  const level = $derived(statusLevel(display.maxTemp));
  const levelText = $derived(level === 'error' ? 'Critical temperature' : level === 'warn' ? 'Running warm' : 'Normal');
  const temps = $derived(s ? s.temperatures.filter((t) => !hide.temps?.includes(t.source)) : []);
</script>

{#if !hide.all}
  <footer class="status" aria-label="System status">
    {#if s}
      <span class="dot {level}" role="img" aria-label="Status: {levelText}"></span>
      <div class="items">
        {#each temps as t (t.source)}
          <span class="item num">{sourceLabel(t.source)} {t.celsius.toFixed(1)}°C</span>
        {/each}
      </div>
      <div class="right">
        {#if !hide.groups}
          <span class="item num">{s.group_duties.length} groups @ {Math.round(meanDuty(s))}%</span>
        {/if}
        {#if s.psu && !hide.psu}<span class="item num">{s.psu.total_power.toFixed(0)}W</span>{/if}
      </div>
    {:else}
      <span class="dot idle" aria-hidden="true"></span>
      <span class="item">Connecting to hardware...</span>
    {/if}
  </footer>
{/if}

<style>
  .status {
    position: relative;
    z-index: 20;
    height: var(--statusbar-h);
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    font-size: 11.5px;
    color: var(--t2);
    background: rgba(8, 9, 13, 0.55);
    -webkit-backdrop-filter: blur(20px) saturate(150%);
    backdrop-filter: blur(20px) saturate(150%);
    box-shadow: inset 0 1px 0 var(--hair);
    white-space: nowrap;
    overflow: hidden;
  }
  .items,
  .right {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .items {
    flex: 1;
    overflow: hidden;
  }
  .item {
    font-variant-numeric: tabular-nums;
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .dot.ok {
    background: var(--ok);
    box-shadow: 0 0 0 3px rgba(50, 215, 75, 0.18);
  }
  .dot.warn {
    background: var(--warn);
    box-shadow: 0 0 0 3px rgba(255, 159, 10, 0.2);
  }
  .dot.error {
    background: var(--error);
    box-shadow: 0 0 0 3px rgba(255, 69, 58, 0.22);
    animation: beat 1.2s ease-in-out infinite;
  }
  .dot.idle {
    background: var(--t3);
  }
  @keyframes beat {
    50% {
      box-shadow: 0 0 0 5px rgba(255, 69, 58, 0.08);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot.error {
      animation: none;
    }
  }
</style>
