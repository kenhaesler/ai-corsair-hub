<!--
  SpecPanel: the Summary's reading instrument (Technical-Specifications style).
  Tabs: Temperatures / Fan groups / Power / Speeds, with a sliding lit
  underline. Each row: name + context line, a luminous bar on a track (lit
  cap, drifting sparks) over a ghost "operating envelope" box, and the value.
  Rows open their detail page (morph). Edit mode keeps the pin / reorder
  capability: hide rows from the Summary and change their order.
  Reads `display`, so it follows the time-scrub.
-->
<script lang="ts">
  import Thermometer from 'lucide-svelte/icons/thermometer';
  import Fan from 'lucide-svelte/icons/fan';
  import Zap from 'lucide-svelte/icons/zap';
  import Activity from 'lucide-svelte/icons/activity';
  import Cpu from 'lucide-svelte/icons/cpu';
  import Microchip from 'lucide-svelte/icons/microchip';
  import Eye from 'lucide-svelte/icons/eye';
  import EyeOff from 'lucide-svelte/icons/eye-off';
  import ChevronUp from 'lucide-svelte/icons/chevron-up';
  import ChevronDown from 'lucide-svelte/icons/chevron-down';
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import type { IconComponent } from '../../lib/icon';
  import { display } from '../../lib/stores/scrub.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { dutyOf, groupRpm } from '../../lib/format';
  import { dutyColor, findGroup, groupEnvelope, groupWhy, heatColor, powerColor, srcLong } from './overview';

  type TabId = 'temps' | 'groups' | 'power' | 'speeds';
  const TABS: { id: TabId; label: string; icon: IconComponent }[] = [
    { id: 'temps', label: 'Temperatures', icon: Thermometer },
    { id: 'groups', label: 'Fan groups', icon: Fan },
    { id: 'power', label: 'Power', icon: Zap },
    { id: 'speeds', label: 'Speeds', icon: Activity },
  ];
  let tab = $state<TabId>('temps');
  const ti = $derived(TABS.findIndex((t) => t.id === tab));

  interface Row {
    key: string;
    /** highlight key for pin/detail, null for non-pinnable rows (rails, speeds) */
    hl: string | null;
    icon: IconComponent;
    name: string;
    sub: string;
    value: number;
    dec: number;
    unit: string;
    min: number;
    max: number;
    env: [number, number] | null;
    color: string;
  }

  const s = $derived(display.snapshot);
  const emergency = $derived(!!sensors.snapshot?.emergency && !display.scrubbing);
  const cfg = $derived(configStore.config);

  function tempSub(src: string): string {
    if (!s) return '';
    if (src === 'cpu' && s.cpu_metrics) {
      const m = s.cpu_metrics;
      return `Load ${Math.round(m.load_pct)}% · ${Math.round(m.power_watts)} W · ${Math.round(m.clock_mhz).toLocaleString('en-US')} MHz`;
    }
    if (src === 'gpu' && s.gpu_metrics) {
      const m = s.gpu_metrics;
      return `Utilisation ${Math.round(m.utilization_pct)}% · ${Math.round(m.power_watts)} W · ${(m.memory_used_mb / 1024).toFixed(1)} GB`;
    }
    if (src === 'psu_vrm') return 'Power supply · voltage regulators';
    if (src === 'psu_case') return 'Power supply · case';
    if (src.startsWith('water')) return 'Coolant';
    return 'Sensor';
  }

  const PSU_SCALE = 1600;
  const rows = $derived.by((): Row[] => {
    if (!s) return [];
    if (tab === 'temps')
      return s.temperatures.map((t) => ({
        key: `temp:${t.source}`,
        hl: `temp:${t.source}`,
        icon: t.source === 'cpu' ? Cpu : t.source === 'gpu' ? Microchip : Thermometer,
        name: srcLong(t.source),
        sub: tempSub(t.source),
        value: t.celsius,
        dec: 1,
        unit: '°',
        min: 20,
        max: 100,
        env: [30, 70],
        color: heatColor(t.celsius),
      }));
    if (tab === 'groups')
      return s.group_duties.map((g) => {
        const grp = findGroup(cfg, g.name);
        return {
          key: `group:${g.name}`,
          hl: `group:${g.name}`,
          icon: Fan,
          name: g.name,
          sub: groupWhy(grp, s, emergency),
          value: g.duty_percent,
          dec: 0,
          unit: '%',
          min: 0,
          max: 100,
          env: groupEnvelope(grp),
          color: dutyColor(g.duty_percent),
        };
      });
    if (tab === 'power') {
      if (!s.psu) return [];
      const p = s.psu;
      return [
        {
          key: 'power:psu',
          hl: 'power:psu',
          icon: Zap,
          name: 'Total output',
          sub: `Input ${p.input_voltage.toFixed(0)} V · PSU fan ${Math.round(p.fan_rpm).toLocaleString('en-US')} rpm`,
          value: p.total_power,
          dec: 0,
          unit: 'W',
          min: 0,
          max: PSU_SCALE,
          env: null,
          color: powerColor(p.total_power, PSU_SCALE),
        },
        ...p.rails.map((r) => ({
          key: `rail:${r.name}`,
          hl: null,
          icon: Zap,
          name: `${r.name} rail`,
          sub: `${r.voltage.toFixed(2)} V · ${r.current.toFixed(1)} A`,
          value: r.power,
          dec: 0,
          unit: 'W',
          min: 0,
          max: /12/.test(r.name) ? PSU_SCALE : 150,
          env: null,
          color: powerColor(r.power, /12/.test(r.name) ? PSU_SCALE : 150),
        })),
      ];
    }
    // speeds: one shared, honest scale (pumps spin faster than fans)
    const rpms = s.group_duties.map((g) => ({ g, rpm: groupRpm(s, g.name) }));
    const top = Math.max(2000, ...rpms.map((x) => x.rpm ?? 0));
    const scale = Math.ceil((top * 1.1) / 500) * 500;
    return rpms.map(({ g, rpm }) => ({
      key: `rpm:${g.name}`,
      hl: null,
      icon: Activity,
      name: g.name,
      sub: `${s.fans.filter((f) => f.group_name === g.name).length} devices · ${Math.round(dutyOf(s, g.name) ?? 0)}% duty`,
      value: rpm ?? 0,
      dec: 0,
      unit: 'rpm',
      min: 0,
      max: scale,
      env: null,
      color: dutyColor(dutyOf(s, g.name) ?? 0),
    }));
  });

  // Pins: order + hide (temps, groups, PSU total), preserved from the previous Summary.
  const pinKeys = $derived(rows.filter((r) => r.hl).map((r) => r.hl!));
  const visible = $derived.by(() => {
    if (!pinKeys.length) return rows;
    const order = ui.orderHighlights(pinKeys, ui.editingPins);
    const pinned = order.map((k) => rows.find((r) => r.hl === k)!).filter(Boolean);
    return [...pinned, ...rows.filter((r) => !r.hl)];
  });
  function move(key: string, delta: number) {
    const order = ui.orderHighlights(pinKeys, true);
    const i = order.indexOf(key);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    const all = ui.pins.order.length ? ui.pins.order.filter((k) => !order.includes(k)) : [];
    ui.pins.order = [...order, ...all];
    ui.persistPins();
  }

  const ticks = $derived.by(() => {
    const r = visible[0];
    if (!r || tab === 'power') return [];
    const n = tab === 'temps' ? 8 : tab === 'groups' ? 10 : 8;
    return Array.from({ length: n + 1 }, (_, i) => Math.round(r.min + ((r.max - r.min) * i) / n));
  });
  const frac = (r: Row, v: number) => Math.max(0, Math.min(1, (v - r.min) / (r.max - r.min)));
  const fmtV = (r: Row) => (r.dec ? r.value.toFixed(r.dec) : Math.round(r.value).toLocaleString('en-US'));

  let tabEls = $state<HTMLButtonElement[]>([]);
  function onTabKey(e: KeyboardEvent) {
    let j = ti;
    if (e.key === 'ArrowRight') j = (ti + 1) % TABS.length;
    else if (e.key === 'ArrowLeft') j = (ti - 1 + TABS.length) % TABS.length;
    else return;
    e.preventDefault();
    tab = TABS[j].id;
    tabEls[j]?.focus();
  }
  const canEdit = $derived(tab !== 'speeds' && pinKeys.length > 0);
</script>

<div class="spec">
  <div class="tabs" role="tablist" aria-label="Readings" tabindex="-1" onkeydown={onTabKey}>
    {#each TABS as t, i (t.id)}
      {@const Icon = t.icon}
      <button
        type="button"
        role="tab"
        id="spec-tab-{t.id}"
        aria-selected={tab === t.id}
        aria-controls="spec-panel"
        tabindex={tab === t.id ? 0 : -1}
        bind:this={tabEls[i]}
        onclick={() => (tab = t.id)}
      >
        <Icon size={15} strokeWidth={1.75} />{t.label}
      </button>
    {/each}
    <i class="uline" style:transform="translateX({ti * 100}%)" aria-hidden="true"></i>
  </div>

  <div class="panel" id="spec-panel" role="tabpanel" aria-labelledby="spec-tab-{tab}">
    <div class="phd">
      <span>{TABS[ti].label}</span>
      <span class="phd-r">
        {#if tab === 'temps' || tab === 'groups'}<span class="env-key"><i></i>{tab === 'temps' ? 'Normal range' : 'Operating envelope'}</span>{/if}
        {#if canEdit}
          <button type="button" class="edit" onclick={() => (ui.editingPins = !ui.editingPins)}>{ui.editingPins ? 'Done' : 'Edit'}</button>
        {/if}
      </span>
    </div>

    {#key tab}
      <div class="rows" class:editing={ui.editingPins}>
        {#if !s}
          <p class="empty">Waiting for the first reading…</p>
        {:else if visible.length === 0}
          <p class="empty">
            {tab === 'temps' ? 'No temperature sources available.' : tab === 'power' ? 'No Corsair power supply detected.' : 'No fan groups configured. Create one in Fan Control.'}
          </p>
        {/if}
        {#each visible as r, i (r.key)}
          {@const Icon = r.icon}
          {@const hidden = !!r.hl && ui.pins.hidden.includes(r.hl)}
          <div
            class="r"
            class:muted={hidden}
            style:--c={r.color}
            style:--i={i}
            style:view-transition-name={r.hl && ui.morphKey === r.hl ? 'hl-card' : undefined}
          >
            <button
              type="button"
              class="hit"
              disabled={!r.hl || ui.editingPins}
              aria-label={r.hl ? `${r.name}: ${fmtV(r)} ${r.unit}. Open details` : undefined}
              tabindex={r.hl && !ui.editingPins ? 0 : -1}
              onclick={() => r.hl && ui.openDetail(r.hl)}
            ></button>
            <div class="nm">
              <b><Icon size={14} strokeWidth={1.75} />{r.name}</b>
              <span>{r.sub}</span>
            </div>
            <div class="track">
              {#if r.env}<i class="env" style:left="{frac(r, r.env[0]) * 100}%" style:width="{(frac(r, r.env[1]) - frac(r, r.env[0])) * 100}%"></i>{/if}
              <i class="fill" style:transform="scaleX({frac(r, r.value)})"></i>
              <i class="cap" style:left="calc({frac(r, r.value) * 100}% - 1px)"></i>
              <i class="spark" style:left="calc({frac(r, r.value) * 100}% - 40px)" style:animation-delay="{i * 0.6}s"></i>
              <i class="spark" style:left="calc({frac(r, r.value) * 100}% - 40px)" style:animation-delay="{i * 0.6 + 1.3}s"></i>
            </div>
            <div class="val num">{fmtV(r)}<small>{r.unit}</small></div>
            {#if ui.editingPins && r.hl}
              <div class="tools">
                <button type="button" title={hidden ? 'Show on Summary' : 'Hide from Summary'} onclick={() => ui.togglePin(r.hl!, pinKeys)}>
                  {#if hidden}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
                </button>
                <button type="button" title="Move up" disabled={i === 0} onclick={() => move(r.hl!, -1)}><ChevronUp size={14} /></button>
                <button type="button" title="Move down" disabled={i >= pinKeys.length - 1} onclick={() => move(r.hl!, 1)}><ChevronDown size={14} /></button>
              </div>
            {:else if r.hl}
              <span class="go" aria-hidden="true"><ChevronRight size={14} /></span>
            {/if}
          </div>
        {/each}
      </div>
    {/key}

    {#if ticks.length}
      <div class="axis" aria-hidden="true"><span></span><div class="ticks">{#each ticks as t (t)}<span>{t.toLocaleString('en-US')}</span>{/each}</div><span></span></div>
    {/if}
  </div>
</div>

<style>
  .spec {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
  }
  .tabs {
    position: relative;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    padding: 0 6px;
    border-bottom: 1px solid var(--hair);
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 16px 12px 14px;
    border: 0;
    background: none;
    color: var(--t3);
    font-size: 14px;
    cursor: pointer;
    transition: color 0.3s;
  }
  .tabs button:hover {
    color: var(--t2);
  }
  .tabs button[aria-selected='true'] {
    color: var(--t1);
  }
  .tabs button:focus-visible {
    box-shadow: inset 0 0 0 1px var(--accent-strong);
    border-radius: 8px;
  }
  .uline {
    position: absolute;
    bottom: -1px;
    left: 6px;
    width: calc((100% - 12px) / 4);
    height: 1px;
    background: linear-gradient(90deg, transparent, color-mix(in oklab, var(--sc) 70%, #fff), transparent);
    box-shadow: 0 0 12px var(--sc);
    transition: transform 0.55s cubic-bezier(0.3, 1.25, 0.4, 1);
  }
  .panel {
    position: relative;
    margin: 16px;
    border: 1px solid var(--hair);
    border-radius: 12px;
    background: rgba(11, 12, 15, 0.8);
    display: grid;
    grid-template-rows: 44px minmax(0, 1fr) auto;
    min-height: 0;
  }
  .phd {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 18px;
    border-bottom: 1px solid var(--hair);
    font: 500 11px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t2);
  }
  .phd-r {
    display: flex;
    align-items: center;
    gap: 16px;
    color: var(--t3);
  }
  .env-key {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .env-key i {
    width: 16px;
    height: 9px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 2px;
  }
  .edit {
    border: 0;
    background: none;
    color: var(--t1);
    font: 500 12px var(--font-sans);
    letter-spacing: 0;
    text-transform: none;
    padding: 5px 10px;
    border-radius: 7px;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px var(--hair-strong);
  }
  .edit:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .rows {
    display: grid;
    align-content: space-evenly;
    padding: 6px 12px;
    overflow-y: auto;
    min-height: 0;
  }
  .empty {
    color: var(--t3);
    text-align: center;
    padding: 40px 0;
  }
  .r {
    position: relative;
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr) 82px 22px;
    align-items: center;
    gap: 16px;
    min-height: 50px;
    padding: 4px 6px;
    border-radius: 10px;
    animation: rowin 0.6s cubic-bezier(0.2, 0.8, 0.2, 1) backwards;
    animation-delay: calc(var(--i) * 55ms);
    transition: background-color 0.2s;
  }
  .editing .r {
    grid-template-columns: 210px minmax(0, 1fr) 82px 92px;
  }
  .r:has(.hit:not(:disabled):hover) {
    background: rgba(255, 255, 255, 0.03);
  }
  .r.muted {
    opacity: 0.4;
  }
  @keyframes rowin {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  .hit {
    position: absolute;
    inset: 0;
    z-index: 1;
    border: 0;
    background: none;
    border-radius: inherit;
    cursor: pointer;
  }
  .hit:disabled {
    cursor: default;
  }
  .nm {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .nm b {
    display: flex;
    align-items: center;
    gap: 9px;
    font-weight: 500;
    font-size: 14.5px;
  }
  .nm b :global(svg) {
    color: var(--t3);
    flex: none;
  }
  .nm span {
    font-size: 12px;
    color: var(--t3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    position: relative;
    height: 30px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.024);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.04);
  }
  .env {
    position: absolute;
    top: 3px;
    bottom: 3px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.02);
    transition:
      left 0.8s,
      width 0.8s;
  }
  .fill {
    position: absolute;
    inset: 0;
    transform-origin: left;
    border-radius: 3px;
    background: linear-gradient(
      90deg,
      color-mix(in oklab, var(--c) 18%, #15171c),
      color-mix(in oklab, var(--c) 75%, #1a1c22) 70%,
      color-mix(in oklab, var(--c) 40%, #fff)
    );
    box-shadow: 0 0 22px -4px color-mix(in oklab, var(--c) 70%, transparent);
    transition:
      transform 1s cubic-bezier(0.2, 0.8, 0.2, 1),
      background 0.9s;
  }
  .cap {
    position: absolute;
    top: -3px;
    bottom: -3px;
    width: 2px;
    background: #fff;
    border-radius: 2px;
    box-shadow: 0 0 10px 2px color-mix(in oklab, var(--c) 85%, #fff);
    transition: left 1s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .spark {
    position: absolute;
    top: 50%;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: #fff;
    opacity: 0;
    animation: spark 2.6s ease-in-out infinite;
  }
  @keyframes spark {
    0% {
      opacity: 0;
      transform: translate(0, -6px);
    }
    30% {
      opacity: 0.9;
    }
    100% {
      opacity: 0;
      transform: translate(34px, 5px);
    }
  }
  .val {
    text-align: right;
    font-size: 21px;
    font-weight: 400;
    letter-spacing: -0.02em;
    color: color-mix(in oklab, var(--c) 45%, #fff);
  }
  .val small {
    font-size: 12px;
    color: var(--t3);
    margin-left: 2px;
    letter-spacing: 0;
  }
  .go {
    color: var(--t3);
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity 0.2s;
  }
  .r:has(.hit:hover) .go,
  .r:has(.hit:focus-visible) .go {
    opacity: 1;
  }
  .tools {
    position: relative;
    z-index: 2;
    display: flex;
    gap: 2px;
    justify-content: flex-end;
  }
  .tools button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.04);
    color: var(--t2);
    cursor: pointer;
  }
  .tools button:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .axis {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr) 82px 22px;
    gap: 16px;
    padding: 0 18px 12px;
  }
  .ticks {
    display: flex;
    justify-content: space-between;
    font: 11px var(--font-mono);
    color: var(--t3);
  }
  @media (prefers-reduced-motion: reduce) {
    .spark {
      display: none;
    }
    .r {
      animation: none;
    }
    .fill,
    .cap,
    .uline {
      transition: none;
    }
  }
</style>
