<!--
  Highlight detail page. Shares view-transition-names with its summary row
  (card, figure, chart) so the row morphs into this page.
  Contents: figureXL value (single home of the current value), context
  sentence (deltas only, never an annotated number), range (1 min / 5 min /
  Session), large scrubbable StockChart whose min/avg/max annotations are the
  single home of those stats, related metrics per kind.
-->
<script lang="ts">
  import Fan from 'lucide-svelte/icons/fan';
  import SlidersHorizontal from 'lucide-svelte/icons/sliders-horizontal';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import Segmented from '../../components/ui/Segmented.svelte';
  import Button from '../../components/ui/Button.svelte';
  import Bar from '../../components/ui/Bar.svelte';
  import Toggle from '../../components/ui/Toggle.svelte';
  import EmptyState from '../../components/ui/EmptyState.svelte';
  import StockChart, { type ChartSeries } from '../../lib/charts/StockChart.svelte';
  import { display, type Range } from '../../lib/stores/scrub.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { stats } from '../../lib/charts/monotone';
  import { displayName, shortDeviceId, shortHubSerial } from '../../lib/identity';
  import { POWER_COLOR, sourceColor, thermalInk } from '../../lib/format';
  import { contextSentence, type Highlight } from './highlights';
  import AlertRows from './AlertRows.svelte';
  import RowChart from './RowChart.svelte';

  interface Props {
    h: Highlight;
    morph: boolean;
  }
  let { h, morph }: Props = $props();

  let range = $state<Range>(300);
  let annotate = $state(true);

  const snap = $derived(display.snapshot);
  const value = $derived(snap ? h.pick(snap) : null);
  const data = $derived(display.series(h.pick, range));
  const sentence = $derived(snap ? contextSentence(h, snap, display.timeline, configStore.config, 'detail') : '');
  const groupDuty = $derived(h.kind === 'group' && snap ? h.pick(snap) : null);
  // The large title already names the source; the caption names its category.
  const category = $derived(h.kind === 'temp' ? 'Temperature' : h.kind === 'group' ? 'Fan Group' : 'PSU Output');
  const vt = (p: string) => (morph ? `hl-${p}` : undefined);

  const main = $derived<ChartSeries[]>([
    {
      id: 'main',
      label: h.title,
      color: h.color,
      data,
      unit: h.chartUnit,
      decimals: h.decimals,
    },
  ]);

  // ---- related metrics ----
  interface Mini {
    id: string;
    label: string;
    color: string;
    unit: string;
    decimals: number;
    pick: (s: NonNullable<typeof snap>) => number | null;
  }
  const related = $derived.by((): Mini[] => {
    if (h.kind === 'temp' && h.id === 'cpu')
      return [
        { id: 'load', label: 'Load', color: '#5AA9FF', unit: '%', decimals: 0, pick: (s) => s.cpu_metrics?.load_pct ?? null },
        { id: 'power', label: 'Package power', color: POWER_COLOR, unit: ' W', decimals: 0, pick: (s) => s.cpu_metrics?.power_watts ?? null },
        { id: 'clock', label: 'Max core clock', color: '#A78BFA', unit: ' MHz', decimals: 0, pick: (s) => s.cpu_metrics?.clock_mhz ?? null },
      ];
    if (h.kind === 'temp' && h.id === 'gpu')
      return [
        { id: 'util', label: 'Utilisation', color: '#34D399', unit: '%', decimals: 0, pick: (s) => s.gpu_metrics?.utilization_pct ?? null },
        { id: 'power', label: 'Board power', color: POWER_COLOR, unit: ' W', decimals: 0, pick: (s) => s.gpu_metrics?.power_watts ?? null },
        { id: 'clock', label: 'Core clock', color: '#A78BFA', unit: ' MHz', decimals: 0, pick: (s) => s.gpu_metrics?.clock_mhz ?? null },
        { id: 'vram', label: 'VRAM used', color: '#5EE1E6', unit: ' GB', decimals: 1, pick: (s) => (s.gpu_metrics ? s.gpu_metrics.memory_used_mb / 1024 : null) },
      ];
    if (h.kind === 'temp')
      return [
        { id: 'power', label: 'PSU output', color: POWER_COLOR, unit: ' W', decimals: 0, pick: (s) => s.psu?.total_power ?? null },
        { id: 'fan', label: 'PSU fan', color: '#7DD3FC', unit: ' RPM', decimals: 0, pick: (s) => s.psu?.fan_rpm ?? null },
      ];
    if (h.kind === 'power')
      return [
        { id: 'cpu', label: 'CPU package', color: sourceColor('cpu'), unit: ' W', decimals: 0, pick: (s) => s.cpu_metrics?.power_watts ?? null },
        { id: 'gpu', label: 'GPU board', color: sourceColor('gpu'), unit: ' W', decimals: 0, pick: (s) => s.gpu_metrics?.power_watts ?? null },
        { id: 'vin', label: 'Input voltage', color: '#A78BFA', unit: ' V', decimals: 0, pick: (s) => s.psu?.input_voltage ?? null },
      ];
    return [];
  });

  // fan group
  const group = $derived(h.kind === 'group' ? configStore.config?.fan_groups.find((g) => g.name === h.id) : undefined);
  const fans = $derived(h.kind === 'group' && snap ? snap.fans.filter((f) => f.group_name === h.id) : []);
  const modeSummary = $derived.by(() => {
    const m = group?.mode;
    if (!m) return '';
    if (m.type === 'fixed') return `Fixed at ${m.duty_percent}%`;
    const src = m.temp_source.sensors
      .map((s, i) => `${s.toUpperCase()} ${(m.temp_source.weights[i] ?? 1).toFixed(1)}`)
      .join(' · ');
    if (m.type === 'curve') return `Curve · ${m.points.length} points · hysteresis ${m.hysteresis}° · ${src}`;
    return `PID · target ${m.target_temp}° · ${m.min_duty}–${m.max_duty}% · ${src}`;
  });

  const rangeWords = $derived(range === 60 ? 'in the last minute' : range === 300 ? 'over 5 minutes' : 'this session');
  function relStats(ser: [number, number][]) {
    if (ser.length < 2) return null;
    const end = ser[ser.length - 1][0];
    const from = range === 'all' ? -Infinity : end - range * 1000;
    return stats(ser.filter((d) => d[0] >= from).map((d) => d[1]));
  }

  const hasUserName = (id: string) =>
    !!configStore.config?.devices?.find((d) => d.device_id === id && d.name && d.name.trim().length > 0);

  function editCurve() {
    ui.fanFocusGroup = h.id;
    ui.goTab('fans');
  }
</script>

<LargeTitlePage title={h.title} back={{ label: 'Summary', onback: () => ui.openDetail(null) }} subtitle={modeSummary || undefined}>
  <AlertRows />
  <div class="hero" style:view-transition-name={vt('card')}>
    <div class="hero-top">
      <div>
        <div class="cap" style:color={h.color}>
          <span class="t-caption">{category}</span>
        </div>
        <div class="fig" style:view-transition-name={vt('fig')} style:color={h.kind === 'temp' && value !== null ? thermalInk(value) : undefined}>
          {#if value !== null}
            <AnimatedNumber value={value} decimals={h.decimals} class="t-figure-xl" />
            <span class="unit">{h.unit}</span>
          {:else}
            <span class="t-figure-xl t3">–</span>
          {/if}
        </div>
        <p class="sentence">{sentence}</p>
      </div>
      <div class="controls">
        <Segmented
          label="Chart range"
          options={[
            { value: 60, label: '1 min' },
            { value: 300, label: '5 min' },
            { value: 'all', label: 'Session' },
          ]}
          bind:value={range}
        />
        <Toggle label="Min / Avg / Max" showLabel size="sm" bind:checked={annotate} />
      </div>
    </div>

    <div class="chart" style:view-transition-name={vt('chart')}>
      <StockChart
        id="detail-{h.key}"
        label="{h.title} history"
        series={main}
        {range}
        height={320}
        annotations={annotate}
        endLabels={false}
        left={h.kind === 'group' ? { min: 0, max: 100, ticks: [0, 25, 50, 75, 100], format: (v) => `${v}%` } : {}}
      />
    </div>

  </div>

  {#if related.length}
    <section class="section" aria-labelledby="rel-h">
      <div class="section-head"><h2 id="rel-h" class="t-title2">Related</h2></div>
      <div class="feed">
        {#each related as m (m.id)}
          {@const cur = snap ? m.pick(snap) : null}
          {@const ser = display.series(m.pick, range)}
          {@const rs = relStats(ser)}
          <div class="rrow">
            <div class="rcap" style:color={m.color}>
              <span class="t-caption">{m.label}</span>
            </div>
            <div class="rbody">
              <div class="rleft">
                <div class="rfig">
                  {#if cur !== null}
                    <AnimatedNumber value={cur} decimals={m.decimals} class="t-figure-m" />
                    <span class="runit">{m.unit.trim()}</span>
                  {:else}
                    <span class="t-figure-m t3">–</span>
                  {/if}
                </div>
                {#if rs}
                  <p class="rsent t-callout num">
                    Between {rs.min.toFixed(m.decimals)} and {rs.max.toFixed(m.decimals)}{m.unit} {rangeWords} · average {rs.avg.toFixed(m.decimals)}{m.unit}
                  </p>
                {/if}
              </div>
              <div class="rright">
                <RowChart data={ser} color={m.color} height={64} decimals={m.decimals} />
              </div>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if h.kind === 'temp' && h.id.startsWith('psu') && snap?.psu}
    <section class="section">
      <div class="section-head"><h2 class="t-title2">Rails</h2></div>
      <div class="rails">
        {#each snap.psu.rails as r (r.name)}
          <div class="rail"><span class="rn">{r.name}</span><span class="num rv">{r.power.toFixed(0)} W</span><span class="num rs">{r.voltage.toFixed(2)} V · {r.current.toFixed(1)} A</span></div>
        {/each}
      </div>
    </section>
  {/if}

  {#if h.kind === 'power' && snap?.psu}
    <section class="section" aria-labelledby="rails-h">
      <div class="section-head"><h2 id="rails-h" class="t-title2">Rails &amp; Sensors</h2></div>
      <div class="rails">
        {#each snap.psu.rails as r (r.name)}
          <div class="rail"><span class="rn">{r.name}</span><span class="num rv">{r.power.toFixed(0)} W</span><span class="num rs">{r.voltage.toFixed(2)} V · {r.current.toFixed(1)} A</span></div>
        {/each}
        <div class="rail"><span class="rn">PSU fan</span><span class="num rv">{Math.round(snap.psu.fan_rpm)} RPM</span><span class="num rs">{snap.psu.fan_rpm < 1 ? 'Zero-RPM mode' : 'Spinning'}</span></div>
        <div class="rail"><span class="rn">VRM</span><span class="num rv">{snap.psu.temp_vrm.toFixed(1)}°C</span><span class="num rs">PSU internal</span></div>
        <div class="rail"><span class="rn">Case</span><span class="num rv">{snap.psu.temp_case.toFixed(1)}°C</span><span class="num rs">PSU internal</span></div>
      </div>
    </section>
  {/if}

  {#if h.kind === 'group'}
    <section class="section" aria-labelledby="fans-h">
      <div class="section-head">
        <h2 id="fans-h" class="t-title2">Fans in this group</h2>
        <Button variant="secondary" size="sm" icon={SlidersHorizontal} onclick={editCurve}>Edit curve in Fan Control</Button>
      </div>
      {#if fans.length === 0}
        <div class="empty-wrap"><EmptyState icon={Fan} title="No fans reporting" message="This group has no enumerated fans right now." compact /></div>
      {:else}
        <ul class="fan-list">
          {#each fans as f (f.device_id || `${f.hub_serial}-${f.channel}`)}
            <li class="fan">
              <span class="fi" aria-hidden="true"><Fan size={15} strokeWidth={2} /></span>
              <div class="fn">
                <span class="fname">{displayName(f.device_id, { config: configStore.config, snapshot: snap })}</span>
                <!-- location only when the name is user-set (the fallback name already is the location) -->
                <span class="floc">{#if hasUserName(f.device_id)}Hub {shortHubSerial(f.hub_serial)} · Ch {f.channel}{#if f.device_id}{' · '}{/if}{/if}{#if f.device_id}<span class="mono">{shortDeviceId(f.device_id)}</span>{/if}</span>
              </div>
              <!-- per-fan RPM is unique to each fan; duty is shown only when it differs from the group figure -->
              <div class="fbar"><Bar value={f.rpm} max={2400} color={h.color} height={4} label="Fan speed" /></div>
              <span class="fduty num">{groupDuty !== null && Math.abs(f.duty_percent - groupDuty) > 0.5 ? `${f.duty_percent.toFixed(0)}%` : ''}</span>
              <span class="frpm num"><AnimatedNumber value={f.rpm} /> RPM</span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
</LargeTitlePage>

<style>
  .hero {
    padding: 22px 24px 20px;
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
  }
  .hero-top {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 24px;
    margin-bottom: 12px;
  }
  .cap {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .fig {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-top: 6px;
    width: fit-content;
    text-shadow: 0 0 32px color-mix(in oklab, currentColor 35%, transparent);
  }
  .fig :global(.t-figure-xl) {
    font-weight: 300;
    letter-spacing: -0.045em;
  }
  .unit {
    font-size: 20px;
    font-weight: 400;
    color: var(--t3);
    text-shadow: none;
  }
  .sentence {
    margin-top: 8px;
    font-size: 15px;
    line-height: 21px;
    color: var(--t2);
    max-width: 60ch;
  }
  .controls {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 12px;
  }
  .chart {
    margin: 8px -4px 0 -8px;
  }
  .feed {
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
    overflow: hidden;
  }
  .rrow {
    padding: 14px 20px 14px;
  }
  .rrow + .rrow {
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .rcap {
    display: flex;
    align-items: center;
    height: 16px;
  }
  .rbody {
    display: flex;
    align-items: flex-end;
    gap: 28px;
    margin-top: 4px;
  }
  .rleft {
    flex: 1 1 0;
    min-width: 0;
  }
  .rright {
    flex: 0 0 48%;
    min-width: 0;
  }
  .rfig {
    display: flex;
    align-items: baseline;
    gap: 4px;
  }
  .rfig :global(.t-figure-m) {
    font-weight: 300;
    font-size: 28px;
    letter-spacing: -0.03em;
  }
  .runit {
    font-size: 13px;
    font-weight: 400;
    color: var(--t3);
  }
  .rsent {
    margin-top: 4px;
    color: var(--t2);
  }
  .rails {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 10px;
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
  }
  .rn {
    font: 500 10.5px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t3);
  }
  .rv {
    font-size: 26px;
    line-height: 30px;
    font-weight: 300;
    letter-spacing: -0.03em;
  }
  .rs {
    font-size: 12px;
    color: var(--t3);
  }
  .fan-list {
    list-style: none;
    margin: 0;
    padding: 0;
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
    overflow: hidden;
  }
  .fan {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) minmax(80px, 200px) 48px 100px;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
  }
  .fan + .fan {
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .fi {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
    color: var(--t2);
  }
  .fn {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .fname {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .floc {
    font-size: 12px;
    color: var(--t3);
  }
  .fduty {
    text-align: right;
    font-size: 13px;
    color: var(--t2);
  }
  .frpm {
    text-align: right;
    font-size: 13px;
    font-weight: 600;
  }
  .empty-wrap {
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
  }
</style>
