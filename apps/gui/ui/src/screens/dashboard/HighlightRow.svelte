<!--
  HighlightRow: one wide row of the Summary feed (Health grammar). Lives
  inside an inset-group container that draws the rounded surface and the
  hairline separators.
    caption (series colour, icon)                                  ›
    big tabular figure + unit            | wide inline chart (5 min)
    context sentence                     | gradient fill, end dot,
    bar (+ CPU/GPU extras)               | faint min/max ticks
  Clicking opens the detail page (shared-element morph). In edit mode it
  shows pin/unpin and reorder controls instead.
-->
<script lang="ts">
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import ArrowUp from 'lucide-svelte/icons/arrow-up';
  import ArrowDown from 'lucide-svelte/icons/arrow-down';
  import Pin from 'lucide-svelte/icons/pin';
  import PinOff from 'lucide-svelte/icons/pin-off';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import Bar from '../../components/ui/Bar.svelte';
  import IconButton from '../../components/ui/IconButton.svelte';
  import { display } from '../../lib/stores/scrub.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { groupRpm, thermalInk } from '../../lib/format';
  import type { Highlight } from './highlights';
  import { contextSentence } from './highlights';
  import RowChart from './RowChart.svelte';

  interface Props {
    h: Highlight;
    morph?: boolean;
    editing?: boolean;
    pinned?: boolean;
    canUp?: boolean;
    canDown?: boolean;
    onopen?: () => void;
    ontogglepin?: () => void;
    onmove?: (delta: number) => void;
  }
  let {
    h,
    morph = false,
    editing = false,
    pinned = true,
    canUp = false,
    canDown = false,
    onopen,
    ontogglepin,
    onmove,
  }: Props = $props();

  const snap = $derived(display.snapshot);
  const value = $derived(snap ? h.pick(snap) : null);
  const spark = $derived(display.series(h.pick, 300));
  const sentence = $derived(snap ? contextSentence(h, snap, display.timeline, configStore.config) : '');
  const rpm = $derived(h.kind === 'group' && snap ? groupRpm(snap, h.id) : null);
  const cpu = $derived(h.kind === 'temp' && h.id === 'cpu' ? snap?.cpu_metrics : null);
  const gpu = $derived(h.kind === 'temp' && h.id === 'gpu' ? snap?.gpu_metrics : null);
  const tooltip = $derived(
    cpu
      ? `CPU · ${cpu.load_pct.toFixed(0)}% load · ${cpu.power_watts.toFixed(0)} W · ${cpu.clock_mhz} MHz`
      : gpu
        ? `GPU · ${gpu.utilization_pct}% util · ${gpu.power_watts.toFixed(0)} W · ${gpu.clock_mhz} MHz · ${(gpu.memory_used_mb / 1024).toFixed(1)} GB VRAM`
        : undefined,
  );
  const vt = (part: string) => (morph ? `hl-${part}` : undefined);
  const Icon = $derived(h.icon);
</script>

<article class="row" class:editing class:unpinned={!pinned} style:view-transition-name={vt('card')} title={tooltip}>
  {#if !editing}
    <button type="button" class="hit" aria-label="{h.title}: open details" onclick={onopen}></button>
  {/if}

  <!-- bare disclosure affordance: scrub time lives only in the titlebar pill -->
  <span class="now" aria-hidden="true"><ChevronRight size={14} strokeWidth={2.25} /></span>

  <div class="body">
    <div class="left">
      <div class="cap" style:color={h.color}>
        <Icon size={14} strokeWidth={2.25} />
        <span class="t-caption">{h.label}</span>
      </div>
      <div class="figure-line">
        <div
          class="figure"
          style:view-transition-name={vt('fig')}
          style:color={h.kind === 'temp' && value !== null ? thermalInk(value) : undefined}
        >
          {#if value !== null}
            <AnimatedNumber value={value} decimals={h.decimals} class="t-figure-l" />
            <span class="unit">{h.unit}</span>
          {:else}
            <span class="t-figure-l dim">–</span>
          {/if}
        </div>
        {#if rpm !== null}
          <span class="aside num">~{Math.round(rpm).toLocaleString('en-US')} RPM</span>
        {/if}
      </div>

      <p class="sentence t-callout">{sentence}</p>

      {#if value !== null}
        <div class="bar">
          <Bar
            value={value}
            max={h.barMax}
            color={h.barColor(value)}
            height={5}
            label="{h.title} {h.kind === 'temp' ? 'temperature' : h.kind === 'group' ? 'duty' : 'power'}"
            valueText="{value.toFixed(h.decimals)}{h.unit}"
          />
        </div>
      {/if}

      {#if cpu}
        <div class="extras">
          <span class="xl">LOAD</span>
          <div class="xbar"><Bar value={Math.min(cpu.load_pct, 100)} color="var(--s-cpu)" height={4} label="CPU load" /></div>
          <span class="xv num">{Math.min(cpu.load_pct, 100).toFixed(0)}%</span>
          <span class="dot" aria-hidden="true"></span>
          <span class="xs num">{cpu.power_watts.toFixed(0)} W</span>
          <span class="xs num">{cpu.clock_mhz} MHz</span>
        </div>
      {/if}
      {#if gpu}
        <div class="extras">
          <span class="xl">UTIL</span>
          <div class="xbar"><Bar value={gpu.utilization_pct} color="var(--s-gpu)" height={4} label="GPU utilisation" /></div>
          <span class="xv num">{gpu.utilization_pct}%</span>
          <span class="dot" aria-hidden="true"></span>
          <span class="xs num">{gpu.power_watts.toFixed(0)} W</span>
          <span class="xs num">{gpu.clock_mhz} MHz</span>
          <span class="xs num">{(gpu.memory_used_mb / 1024).toFixed(1)} GB</span>
        </div>
      {/if}
    </div>

    <div class="right" style:view-transition-name={vt('chart')}>
      <RowChart
        data={spark}
        color={h.color}
        height={78}
        onopen={editing ? undefined : onopen}
        decimals={h.kind === 'temp' ? 1 : 0}
      />
    </div>

    {#if editing}
      <div class="edit">
        <IconButton
          icon={pinned ? PinOff : Pin}
          label={pinned ? `Unpin ${h.title}` : `Pin ${h.title}`}
          variant="raised"
          onclick={ontogglepin}
        />
        <IconButton icon={ArrowUp} label="Move {h.title} up" disabled={!canUp} onclick={() => onmove?.(-1)} />
        <IconButton icon={ArrowDown} label="Move {h.title} down" disabled={!canDown} onclick={() => onmove?.(1)} />
      </div>
    {/if}
  </div>
</article>

<style>
  .row {
    position: relative;
    padding: 16px 20px;
    min-width: 0;
    transition:
      background-color var(--dur-fast) ease-out,
      opacity var(--dur-med) ease-out;
  }
  .row:not(.editing):hover {
    background: rgba(255, 255, 255, 0.028);
  }
  .row:not(.editing):active {
    background: rgba(255, 255, 255, 0.05);
  }
  .row:has(.hit:focus-visible) {
    box-shadow: inset var(--focus-ring);
  }
  .row.unpinned {
    opacity: 0.42;
  }
  .hit {
    position: absolute;
    inset: 0;
    z-index: 1;
    border: 0;
    background: transparent;
    cursor: pointer;
  }
  .hit:focus-visible {
    box-shadow: none;
  }
  .cap {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 16px;
  }
  .now {
    position: absolute;
    top: 14px;
    right: 18px;
    display: inline-flex;
    align-items: center;
    gap: 1px;
    font-size: 11.5px;
    font-weight: 500;
    color: var(--t3);
    pointer-events: none;
    transition: color var(--dur-fast) ease-out;
  }
  .row:not(.editing):hover .now {
    color: var(--t2);
  }
  .editing .now {
    display: none;
  }
  .body {
    display: flex;
    align-items: flex-end;
    gap: 20px;
  }
  .left {
    flex: 1 1 0;
    min-width: 0;
  }
  .right {
    position: relative;
    z-index: 2;
    flex: 0 0 54%;
    min-width: 0;
    padding-bottom: 0;
  }
  .figure-line {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-top: 4px;
  }
  .figure {
    display: flex;
    align-items: baseline;
    gap: 3px;
    color: var(--t1);
  }
  .unit {
    font-size: 17px;
    font-weight: 600;
    color: var(--t2);
    letter-spacing: -0.01em;
  }
  .aside {
    font-size: 13px;
    font-weight: 500;
    color: var(--t2);
  }
  .dim {
    color: var(--t3);
  }
  .sentence {
    margin-top: 2px;
    color: var(--t2);
    min-height: 18px;
  }
  .bar {
    margin-top: 10px;
    max-width: 320px;
  }
  .extras {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
  }
  .xl {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--t3);
  }
  .xbar {
    flex: 0 1 120px;
    min-width: 40px;
  }
  .xv {
    font-weight: 600;
    color: var(--t1);
    min-width: 30px;
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--t3);
    flex: none;
  }
  .edit {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-self: center;
  }
</style>
