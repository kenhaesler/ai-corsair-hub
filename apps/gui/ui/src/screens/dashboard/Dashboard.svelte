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
  import Thermometer from 'lucide-svelte/icons/thermometer';
  import Fan from 'lucide-svelte/icons/fan';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import Button from '../../components/ui/Button.svelte';
  import EmptyState from '../../components/ui/EmptyState.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import { display } from '../../lib/stores/scrub.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { statusLevel } from '../../lib/format';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { buildHighlights, statusPhrase, type Highlight } from './highlights';
  import HighlightRow from './HighlightRow.svelte';
  import AlertRows from './AlertRows.svelte';
  import PresetShelf from './PresetShelf.svelte';
  import PowerSupply from './PowerSupply.svelte';
  import HistorySection from './HistorySection.svelte';
  import Detail from './Detail.svelte';

  const all = $derived(buildHighlights(display.live));
  const keys = $derived(all.map((h) => h.key));
  const ordered = $derived(
    ui.orderHighlights(keys, ui.editingPins).map((k) => all.find((h) => h.key === k)!),
  );
  const detailH = $derived(ui.detail ? all.find((h) => h.key === ui.detail) : undefined);

  // Pinned feed grouped like Health: one inset container per category.
  const GROUPS: { kind: Highlight['kind']; title: string }[] = [
    { kind: 'temp', title: 'Temperatures' },
    { kind: 'group', title: 'Fans' },
    { kind: 'power', title: 'Power Supply' },
  ];
  const sections = $derived(
    GROUPS.map((g) => ({ ...g, rows: ordered.filter((h) => h.kind === g.kind) })),
  );
  const kindOf = (key: string) => all.find((h) => h.key === key)?.kind;

  /** Reorder within a category (the full pin order keeps other categories in place). */
  function moveWithin(key: string, delta: number) {
    const order = ui.orderHighlights(keys, true);
    const kind = kindOf(key);
    const same = order.filter((k) => kindOf(k) === kind);
    const i = same.indexOf(key);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= same.length) return;
    [same[i], same[j]] = [same[j], same[i]];
    let n = 0;
    ui.pins.order = order.map((k) => (kindOf(k) === kind ? same[n++] : k));
    ui.persistPins();
  }
  const s = $derived(display.snapshot);
  const hasTemps = $derived((s?.temperatures.length ?? 0) > 0);
  const hasGroups = $derived((s?.group_duties.length ?? 0) > 0);
  // Emergency / stale are owned by their alert rows; the subtitle steps aside.
  const alertOwnsState = $derived(!!sensors.snapshot?.emergency || !!sensors.snapshot?.any_stale);
  const level = $derived(statusLevel(display.maxTemp));

  // If the detail target disappears (group deleted), go back to Summary.
  $effect(() => {
    if (ui.detail && display.live && !detailH) ui.detail = null;
  });
</script>

<div class="summary" class:hidden={!!detailH}>
  {#snippet subtitle()}
    {#if s}
      <span class="state"><span class="sdot {level}" aria-hidden="true"></span>{statusPhrase(display.maxTemp)}</span>
    {:else}
      <span>Waiting for the first reading</span>
    {/if}
  {/snippet}
  <LargeTitlePage title="Summary" active={!detailH} subtitle={alertOwnsState ? undefined : subtitle}>

    {#if !s}
      <div class="connecting" role="status">
        <Spinner size={18} />
        <span>Connecting to hardware...</span>
      </div>
    {:else}
      <AlertRows />

      <section aria-labelledby="pinned-h">
        <div class="section-head">
          <h2 id="pinned-h" class="t-title2">Pinned</h2>
          {#if ui.editingPins}
            <Button variant="primary" size="sm" onclick={() => (ui.editingPins = false)}>Done</Button>
          {:else}
            <Button variant="plain" size="sm" onclick={() => (ui.editingPins = true)}>Edit</Button>
          {/if}
        </div>
        {#if ui.editingPins}
          <p class="edit-hint t-callout">Pin or unpin highlights and change their order. Unpinned rows stay hidden from the Summary.</p>
        {/if}
        {#each sections as sec (sec.kind)}
          {#if sec.rows.length || (sec.kind === 'temp' && !hasTemps) || (sec.kind === 'group' && !hasGroups) || (sec.kind === 'power' && s.psu)}
            <div class="feed-group">
              <h3 class="feed-title">{sec.title}</h3>
              <div class="feed">
                {#each sec.rows as h, i (h.key)}
                  <HighlightRow
                    {h}
                    morph={ui.morphKey === h.key}
                    editing={ui.editingPins}
                    pinned={!ui.pins.hidden.includes(h.key)}
                    canUp={i > 0}
                    canDown={i < sec.rows.length - 1}
                    onopen={() => ui.openDetail(h.key)}
                    ontogglepin={() => ui.togglePin(h.key, keys)}
                    onmove={(d) => moveWithin(h.key, d)}
                  />
                {/each}
                {#if sec.kind === 'temp' && !hasTemps}
                  <EmptyState icon={Thermometer} title="No temperature sources available" compact />
                {/if}
                {#if sec.kind === 'power' && s.psu}
                  <PowerSupply />
                {/if}
                {#if sec.kind === 'group' && !hasGroups}
                  <EmptyState icon={Fan} title="No fan groups configured" message="Create a group in Fan Control to start managing fans." compact />
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </section>

      <PresetShelf />
      <HistorySection />
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
  .state {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .sdot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
  }
  .sdot.ok {
    background: var(--ok);
    box-shadow: 0 0 0 3px rgba(50, 215, 75, 0.18);
  }
  .sdot.warn {
    background: var(--warn);
    box-shadow: 0 0 0 3px rgba(255, 159, 10, 0.2);
  }
  .sdot.error {
    background: var(--error);
    box-shadow: 0 0 0 3px rgba(255, 69, 58, 0.22);
  }
  .connecting {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 240px;
    color: var(--t2);
  }
  .edit-hint {
    color: var(--t3);
    margin: -4px 0 12px;
  }
  .feed-group + .feed-group {
    margin-top: 20px;
  }
  .feed-title {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--t2);
    padding: 0 4px 8px;
  }
  .feed {
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    overflow: hidden;
  }
  .feed > :global(* + *) {
    box-shadow: inset 0 1px 0 var(--hair);
  }
</style>
