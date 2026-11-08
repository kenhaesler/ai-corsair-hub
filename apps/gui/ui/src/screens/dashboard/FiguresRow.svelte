<!--
  FiguresRow: the four figures that answer "how is it doing" in one glance,
  plus the active fan preset (jumps to the Presets section below the fold).
  Each figure glows in its own value's colour.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import ArrowDown from 'lucide-svelte/icons/arrow-down';
  import { display } from '../../lib/stores/scrub.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { getFanPresets } from '../../lib/api';
  import { groupMatchesPreset } from '../../lib/presets';
  import type { FanPresetDef } from '../../lib/types';
  import { meanDuty } from '../../lib/format';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import { CRITICAL_CPU, CRITICAL_GPU, dutyColor, headroom, heatColor, meanGroupRpm, powerColor, srcLong } from './overview';

  interface Props {
    onpresets: () => void;
  }
  let { onpresets }: Props = $props();

  let presets = $state<FanPresetDef[]>([]);
  onMount(() => {
    getFanPresets()
      .then((p) => (presets = p))
      .catch(() => (presets = []));
  });

  const s = $derived(display.snapshot);
  const hottest = $derived(s ? [...s.temperatures].sort((a, b) => b.celsius - a.celsius)[0] : undefined);
  const duty = $derived(meanDuty(s));
  const rpm = $derived(s ? meanGroupRpm(s) : null);
  const hr = $derived(s ? headroom(s) : null);

  const preset = $derived.by(() => {
    const groups = configStore.config?.fan_groups ?? [];
    if (!groups.length) return { name: 'None', note: 'No fan groups' };
    const ids = [...new Set(groups.map((g) => g.preset_applied ?? null))];
    if (ids.length > 1) return { name: 'Mixed', note: 'Groups use different presets' };
    const p = presets.find((x) => x.id === ids[0]);
    if (!p) return { name: 'Custom', note: 'Your own curves' };
    const modified = groups.some((g) => !groupMatchesPreset(g, p));
    return { name: p.name, note: modified ? 'Modified since applied' : p.description };
  });
</script>

<div class="figs">
  <div class="fig" style:--c={s?.psu ? powerColor(s.psu.total_power, 1600) : 'var(--t3)'}>
    <span class="k">Power draw</span>
    {#if s?.psu}
      <span class="n"><AnimatedNumber value={s.psu.total_power} decimals={0} /><small>W</small></span>
      <span class="d">Input {s.psu.input_voltage.toFixed(0)} V · {s.psu.rails.length} rails</span>
    {:else}
      <span class="n dim">–</span><span class="d">No Corsair PSU detected</span>
    {/if}
  </div>
  <div class="fig" style:--c={dutyColor(duty)}>
    <span class="k">Average fan</span>
    <span class="n"><AnimatedNumber value={duty} decimals={0} /><small>%</small></span>
    <span class="d">{s?.fans.length ?? 0} device{(s?.fans.length ?? 0) === 1 ? '' : 's'}{rpm !== null ? ` · ~${Math.round(rpm).toLocaleString('en-US')} rpm` : ''}</span>
  </div>
  <div class="fig" style:--c={hottest ? heatColor(hottest.celsius) : 'var(--t3)'}>
    <span class="k">Hottest part</span>
    {#if hottest}
      <span class="n"><AnimatedNumber value={hottest.celsius} decimals={1} /><small>°C</small></span>
      <span class="d">{srcLong(hottest.source)}</span>
    {:else}
      <span class="n dim">–</span><span class="d">No sensors</span>
    {/if}
  </div>
  <div class="fig" style:--c={hr ? heatColor(100 - hr.value * 2.2) : 'var(--t3)'}>
    <span class="k">Headroom</span>
    {#if hr}
      <span class="n"><AnimatedNumber value={Math.max(0, hr.value)} decimals={1} /><small>°C</small></span>
      <span class="d">{hr.part} to its {hr.part === 'CPU' ? CRITICAL_CPU : CRITICAL_GPU} °C override</span>
    {:else}
      <span class="n dim">–</span><span class="d">No CPU or GPU sensor</span>
    {/if}
  </div>
  <button type="button" class="fig preset" onclick={onpresets} title="Show fan presets">
    <span class="k">Fan preset</span>
    <span class="pn">{preset.name}<span class="jump"><ArrowDown size={13} strokeWidth={2.2} /></span></span>
    <span class="d">{preset.note}</span>
  </button>
</div>

<style>
  .figs {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr)) minmax(0, 1.3fr);
    border-radius: 16px;
    background: rgba(12, 13, 16, 0.8);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    -webkit-backdrop-filter: var(--mat-blur);
    backdrop-filter: var(--mat-blur);
  }
  .fig {
    display: grid;
    align-content: space-between;
    gap: 8px;
    padding: 18px 22px;
    min-width: 0;
    border: 0;
    border-left: 1px solid var(--hair);
    background: none;
    color: inherit;
    text-align: left;
    font: inherit;
  }
  .fig:first-child {
    border-left: 0;
  }
  .k {
    font: 500 10.5px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t3);
  }
  .n {
    font-weight: 300;
    font-size: 40px;
    line-height: 1;
    letter-spacing: -0.04em;
    font-variant-numeric: tabular-nums;
    color: color-mix(in oklab, var(--c) 35%, #fff);
    text-shadow: 0 0 28px color-mix(in oklab, var(--c) 40%, transparent);
  }
  .n small {
    font-size: 15px;
    color: var(--t3);
    margin-left: 4px;
    letter-spacing: 0;
    text-shadow: none;
  }
  .n.dim {
    color: var(--t3);
    text-shadow: none;
  }
  .d {
    font-size: 12.5px;
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .preset {
    cursor: pointer;
    border-radius: 0 16px 16px 0;
    transition: background-color 0.2s;
  }
  .preset:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .pn {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 26px;
    font-weight: 400;
    letter-spacing: -0.025em;
    line-height: 1.1;
  }
  .jump {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 7px;
    background: var(--accent);
    color: var(--accent-ink);
  }
</style>
