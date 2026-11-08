<!--
  History (inventory 1.6): temps solid with gradient fill per source (left
  axis 20–100 °C, labels 30–90), group duties dashed (right axis 20–100 %),
  end labels on hover/scrub only (progressive disclosure: the Pinned rows are
  the single home of current values), legend, "Collecting data…" before 2 samples, hover readout and
  scrub. Range defaults to 1 min (= "History (60s)").
-->
<script lang="ts">
  import StockChart, { type ChartSeries } from '../../lib/charts/StockChart.svelte';
  import Segmented from '../../components/ui/Segmented.svelte';
  import { display, type Range } from '../../lib/stores/scrub.svelte';
  import { dutyOf, groupColor, sourceColor, sourceLabel, tempOf } from '../../lib/format';

  let range = $state<Range>(60);
  const live = $derived(display.live);
  const sources = $derived(live?.temperatures.map((t) => t.source) ?? []);
  const groups = $derived(live?.group_duties.map((g) => g.name) ?? []);

  const series = $derived.by((): ChartSeries[] => [
    ...sources.map((src) => ({
      id: `t-${src}`,
      label: sourceLabel(src),
      color: sourceColor(src),
      data: display.series((s) => tempOf(s, src), range),
      axis: 'left' as const,
      unit: '°',
      decimals: 1,
    })),
    ...groups.map((g, i) => ({
      id: `g-${i}`,
      label: g,
      color: groupColor(i),
      data: display.series((s) => dutyOf(s, g), range),
      axis: 'right' as const,
      dashed: true,
      unit: '%',
      decimals: 0,
    })),
  ]);

  const rangeLabel = $derived(range === 60 ? 'Last 60 seconds' : range === 300 ? 'Last 5 minutes' : 'This session');
</script>

<section class="section" aria-labelledby="hist-h">
  <div class="section-head">
    <div class="titles">
      <h2 id="hist-h" class="t-title2">History</h2>
      <span class="range t-callout">{rangeLabel}</span>
    </div>
    <Segmented
      label="History range"
      size="sm"
      options={[
        { value: 60, label: '1 min' },
        { value: 300, label: '5 min' },
        { value: 'all', label: 'Session' },
      ]}
      bind:value={range}
    />
  </div>
  <div class="panel">
    <StockChart
      id="history"
      label="History: temperatures and fan duty, {rangeLabel.toLowerCase()}"
      {series}
      {range}
      height={300}
      endLabels="interactive"
      left={{ min: 20, max: 100, ticks: [40, 60, 80, 100], format: (v) => `${v}°` }}
      right={{ min: 0, max: 100, ticks: [25, 50, 75, 100], format: (v) => `${v}%` }}
    />
    <div class="legend" aria-label="Legend">
      {#each sources as src (src)}
        <span class="lg"><span class="sw solid" style:background={sourceColor(src)}></span>{sourceLabel(src)}</span>
      {/each}
      {#if groups.length}<span class="div" aria-hidden="true"></span>{/if}
      {#each groups as g, i (g)}
        <span class="lg"><span class="sw dash" style:color={groupColor(i)}></span>{g}</span>
      {/each}
      <span class="hint t-subhead">Drag across the chart to rewind the whole app</span>
    </div>
  </div>
</section>

<style>
  .titles {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .range {
    color: var(--t3);
  }
  .panel {
    padding: 16px 16px 14px 12px;
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    margin: 10px 0 0 36px;
    font-size: 12px;
    color: var(--t2);
  }
  .lg {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .sw {
    display: inline-block;
    width: 14px;
    height: 3px;
    border-radius: 2px;
  }
  .sw.dash {
    background: repeating-linear-gradient(90deg, currentColor 0 4px, transparent 4px 7px);
  }
  .div {
    width: 1px;
    height: 12px;
    background: var(--hair-strong);
  }
  .hint {
    margin-left: auto;
    color: var(--t3);
  }
</style>
