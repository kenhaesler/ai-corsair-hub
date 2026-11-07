<!--
  Cross-Device Flow (inventory 4.6). On/Off (Off writes flow=null);
  direction choices with their descriptions and a small diagram; delay per
  device 0–500 ms (step 10) committed on release; defaults on enable
  80 ms Forward; total span line with the real formulas.
-->
<script lang="ts">
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import Toggle from '../../components/ui/Toggle.svelte';
  import Slider from '../../components/ui/Slider.svelte';
  import type { FlowDirection, RgbZoneConfig } from '../../lib/types';

  interface Props {
    zone: RgbZoneConfig;
    onsave: () => void;
  }
  let { zone, onsave }: Props = $props();

  const DEFAULT_DELAY = 80;
  const DEFAULT_DIR: FlowDirection = 'Forward';
  const DIRECTIONS: { id: FlowDirection; label: string; hint: string }[] = [
    { id: 'Forward', label: 'Forward', hint: 'First device → last' },
    { id: 'Reverse', label: 'Reverse', hint: 'Last device → first' },
    { id: 'CenterOut', label: 'Center out', hint: 'Middle devices → edges' },
    { id: 'EdgeIn', label: 'Edge in', hint: 'Edges → middle devices' },
  ];

  let draftDelay = $state(DEFAULT_DELAY);
  let draftDir = $state<FlowDirection>(DEFAULT_DIR);
  $effect(() => {
    if (zone.flow) {
      draftDelay = zone.flow.delay_per_device_ms;
      draftDir = zone.flow.direction;
    }
  });

  const on = $derived(zone.flow !== null);
  const n = $derived(zone.devices.length);

  function toggle(v: boolean) {
    zone.flow = v ? { delay_per_device_ms: draftDelay, direction: draftDir } : null;
    onsave();
  }
  function setDir(d: FlowDirection) {
    draftDir = d;
    if (zone.flow) {
      zone.flow.direction = d;
      onsave();
    }
  }
  function commitDelay(v: number) {
    draftDelay = v;
    if (zone.flow) {
      zone.flow.delay_per_device_ms = v;
      onsave();
    }
  }

  function dirKey(e: KeyboardEvent) {
    const i = DIRECTIONS.findIndex((d) => d.id === draftDir);
    const delta = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!delta) return;
    e.preventDefault();
    const next = DIRECTIONS[(i + delta + DIRECTIONS.length) % DIRECTIONS.length];
    setDir(next.id);
    (e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role=radio]')[DIRECTIONS.indexOf(next)]?.focus();
  }

  const span = $derived.by(() => {
    if (!on || n <= 1) return 0;
    return draftDir === 'Forward' || draftDir === 'Reverse' ? draftDelay * (n - 1) : draftDelay * Math.floor((n - 1) / 2);
  });

  /** relative start time of dot k of 5 for a direction (0 = first) */
  function order(dir: FlowDirection, k: number): number {
    const mid = 2;
    switch (dir) {
      case 'Forward':
        return k / 4;
      case 'Reverse':
        return (4 - k) / 4;
      case 'CenterOut':
        return Math.abs(k - mid) / 2;
      case 'EdgeIn':
        return (mid - Math.abs(k - mid)) / 2;
    }
  }
</script>

<InsetGroup title="Cross-Device Flow">
  {#snippet action()}
    <span class="act">
      <span class="state" aria-hidden="true">{on ? 'On' : 'Off'}</span>
      <Toggle checked={on} label="Cross-Device Flow" onchange={toggle} />
    </span>
  {/snippet}

  {#if !on}
    <p class="off t-callout">
      Animate effects across the {n} device{n === 1 ? '' : 's'} in this zone with a staggered time offset — fire sweeps, rainbow cascades,
      etc.
    </p>
  {:else}
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div class="dirs" role="radiogroup" aria-label="Direction" onkeydown={dirKey}>
      {#each DIRECTIONS as d (d.id)}
        <button
          type="button"
          role="radio"
          aria-checked={draftDir === d.id}
          tabindex={draftDir === d.id ? 0 : -1}
          class="dir"
          class:sel={draftDir === d.id}
          onclick={() => setDir(d.id)}
        >
          <span class="dots" aria-hidden="true">
            {#each [0, 1, 2, 3, 4] as k (k)}
              <span class="d" style:--o={1 - order(d.id, k) * 0.78}></span>
            {/each}
          </span>
          <span class="dl">{d.label}</span>
          <span class="dh">{d.hint}</span>
        </button>
      {/each}
    </div>
    <div class="delay">
      <Slider
        label="Delay per device"
        min={0}
        max={500}
        step={10}
        bind:value={draftDelay}
        format={(v) => `${v} ms`}
        oncommit={commitDelay}
      />
      {#if span > 0}
        <p class="span t-callout num">Total span: {span} ms across {n} devices</p>
      {/if}
    </div>
  {/if}
</InsetGroup>

<style>
  .act {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .state {
    font-size: 13px;
    line-height: 18px;
    font-weight: 500;
    color: var(--t2);
  }
  .off {
    padding: 14px 16px;
    color: var(--t2);
    max-width: 68ch;
  }
  .dirs {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    padding: 12px;
  }
  .dir {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-rows: auto auto;
    column-gap: 12px;
    align-items: center;
    padding: 10px 12px;
    border: 0;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px var(--hair);
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .dir:hover {
    background: rgba(255, 255, 255, 0.055);
  }
  .dir:active {
    transform: scale(0.98);
  }
  .dir.sel {
    background: var(--accent-softer);
    box-shadow: inset 0 0 0 1px rgba(124, 156, 255, 0.55);
  }
  .dir:focus-visible {
    box-shadow:
      inset 0 0 0 1px rgba(124, 156, 255, 0.55),
      var(--focus-ring);
  }
  .dots {
    grid-row: 1 / span 2;
    display: flex;
    gap: 3px;
  }
  .d {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--t3);
    opacity: var(--o);
  }
  .sel .d {
    background: var(--accent-strong);
    box-shadow: 0 0 6px -1px var(--accent);
  }
  .dl {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--t1);
  }
  .dh {
    font-size: 12px;
    line-height: 16px;
    color: var(--t3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .delay {
    padding: 12px 16px 14px;
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .span {
    margin-top: 6px;
    color: var(--t3);
  }
</style>
