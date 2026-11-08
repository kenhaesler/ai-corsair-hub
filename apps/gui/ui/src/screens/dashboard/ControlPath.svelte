<!--
  ControlPath: the VeilGuard-style schematic of how cooling decisions flow.
  Sources (every sensor a group listens to) → the control loop → fan groups,
  drawn with orthogonal hairline wires, square ports and travelling light
  pulses (inputs pulse with load, outputs faster as their group's duty rises).
  Each group shows its duty and a status tag (On curve / Climbing / Holding /
  Fixed / Override). Built from the config, so it fits any LINK build.
  Pills open the matching detail page.
-->
<script lang="ts">
  import Cpu from 'lucide-svelte/icons/cpu';
  import Microchip from 'lucide-svelte/icons/microchip';
  import Thermometer from 'lucide-svelte/icons/thermometer';
  import Droplet from 'lucide-svelte/icons/droplet';
  import Fan from 'lucide-svelte/icons/fan';
  import { display } from '../../lib/stores/scrub.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { tempOf } from '../../lib/format';
  import { dutyColor, findGroup, groupTag, heatColor, pathSources, srcShort } from './overview';

  let w = $state(560);
  let h = $state(360);

  const s = $derived(display.snapshot);
  const cfg = $derived(configStore.saved);
  const emergency = $derived(!!sensors.snapshot?.emergency && !display.scrubbing);
  const sources = $derived(s ? pathSources(cfg, s) : []);
  const groups = $derived(s?.group_duties ?? []);

  // duty 30 s ago, for the Climbing / Easing tags
  const before = $derived.by(() => {
    if (!s) return null;
    const win = display.timeline;
    const target = s.timestamp_ms - 30_000;
    for (let i = win.length - 1; i >= 0; i--) if (win[i].timestamp_ms <= target) return win[i];
    return null;
  });
  const trend = (name: string, now: number) => {
    const p = before?.group_duties.find((g) => g.name === name)?.duty_percent;
    return p === undefined ? 0 : now - p;
  };

  // geometry
  const IN_W = 148;
  const OUT_W = $derived(Math.min(268, Math.max(220, w * 0.46)));
  const PAD = 14;
  const HUB = 64;
  const inR = $derived(PAD + IN_W);
  const outL = $derived(w - PAD - OUT_W);
  const hubX = $derived(Math.round((inR + outL) / 2 - HUB / 2));
  const cy = $derived(Math.round(h / 2) - 10);
  const spread = (n: number, step: number) => Array.from({ length: n }, (_, i) => cy + (i - (n - 1) / 2) * step);
  const inY = $derived(spread(sources.length, Math.min(82, (h - 70) / Math.max(1, sources.length))));
  const outY = $derived(spread(groups.length, Math.min(82, (h - 40) / Math.max(1, groups.length))));
  const busIn = $derived(Math.round((inR + hubX) / 2));
  const busOut = $derived(Math.round((hubX + HUB + outL) / 2));
  const inPath = (y: number) => `M${inR} ${y} H${busIn} V${cy} H${hubX}`;
  const outPath = (y: number) => `M${hubX + HUB} ${cy} H${busOut} V${y} H${outL}`;

  const icon = (src: string) => (src === 'cpu' ? Cpu : src === 'gpu' ? Microchip : src.startsWith('water') ? Droplet : Thermometer);
  const loadT = $derived(display.thermalT);
  // Pulse speed follows load/duty, but a running CSS animation jumps whenever
  // its duration changes, so durations are quantised to 0.5 s steps and only
  // move when the value crosses a step.
  const q = (s: number) => (Math.round(s * 2) / 2).toFixed(1);
  const inDur = $derived(q(3.4 - loadT * 1.6));
  const outDur = (duty: number) => q(3.6 - (duty / 100) * 2.4);
</script>

<div class="body" bind:clientWidth={w} bind:clientHeight={h}>
  {#if s}
    <svg class="wires" width={w} height={h} aria-hidden="true">
      {#each inY as y, i (sources[i])}<path d={inPath(y)} />{/each}
      {#each outY as y, i (groups[i].name)}<path d={outPath(y)} />{/each}
    </svg>

    {#each sources as src, i (src)}
      {@const t = tempOf(s, src)}
      {@const Icon = icon(src)}
      <button
        type="button"
        class="pill in"
        style:top="{inY[i]}px"
        style:left="{PAD}px"
        style:width="{IN_W}px"
        style:--c={t === null ? 'var(--t3)' : heatColor(t)}
        onclick={() => ui.openDetail(`temp:${src}`)}
      >
        <span class="ic"><Icon size={14} strokeWidth={1.8} /></span>{srcShort(src)}
        {#if ui.specTab !== 'temps'}<span class="v num">{t === null ? '–' : t.toFixed(1)}°</span>{/if}
      </button>
      <i class="port" style:left="{inR}px" style:top="{inY[i]}px"></i>
      <i
        class="pulse"
        style:offset-path="path('{inPath(inY[i])}')"
        style:--c={t === null ? 'var(--sc)' : heatColor(t)}
        style:animation-duration="{inDur}s"
        style:animation-delay="{-i * 0.7}s"
      ></i>
    {/each}

    <div class="hub" style:left="{hubX}px" style:top="{cy}px" title="Control loop">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
        <circle cx="12" cy="12" r="8.5" /><path d="M12 3.5v4M12 16.5v4M3.5 12h4M16.5 12h4" /><circle cx="12" cy="12" r="2.6" fill="currentColor" />
      </svg>
    </div>
    <span class="hublab" style:left="{hubX + HUB / 2}px" style:top="{cy + HUB / 2 + 14}px">Control loop</span>

    {#each groups as g, i (g.name)}
      {@const grp = findGroup(cfg, g.name)}
      {@const tag = groupTag(grp, trend(g.name, g.duty_percent), emergency)}
      <button
        type="button"
        class="pill out"
        class:alert={tag === 'Override'}
        style:top="{outY[i]}px"
        style:left="{outL}px"
        style:width="{OUT_W}px"
        style:--c={dutyColor(g.duty_percent)}
        onclick={() => ui.openDetail(`group:${g.name}`)}
      >
        <span class="ic"><Fan size={14} strokeWidth={1.8} /></span><span class="gn">{g.name}</span>
        {#if ui.specTab !== 'groups'}<span class="v num">{Math.round(g.duty_percent)}%</span>{/if}
        <span class="tag">{tag}</span>
      </button>
      <i class="port" style:left="{outL}px" style:top="{outY[i]}px"></i>
      <i
        class="pulse"
        style:offset-path="path('{outPath(outY[i])}')"
        style:--c={dutyColor(g.duty_percent)}
        style:animation-duration="{outDur(g.duty_percent)}s"
        style:animation-delay="{-i * 0.5}s"
      ></i>
    {/each}

    {#if groups.length === 0}
      <p class="empty" style:left="{outL}px" style:top="{cy}px">No fan groups yet</p>
    {/if}
  {/if}
</div>

<style>
  .body {
    position: relative;
    min-height: 0;
    overflow: hidden;
  }
  .wires {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .wires path {
    fill: none;
    stroke: rgba(255, 255, 255, 0.16);
    stroke-width: 1;
  }
  .port {
    position: absolute;
    width: 7px;
    height: 7px;
    margin: -3.5px 0 0 -3.5px;
    background: var(--bg-0);
    border: 1px solid rgba(255, 255, 255, 0.4);
    border-radius: 2px;
  }
  .pulse {
    position: absolute;
    left: 0;
    top: 0;
    width: 5px;
    height: 5px;
    margin: -2.5px 0 0 -2.5px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 10px 2px var(--c);
    offset-rotate: 0deg;
    animation: run 2.4s linear infinite;
  }
  @keyframes run {
    from {
      offset-distance: 0%;
      opacity: 0;
    }
    12% {
      opacity: 1;
    }
    88% {
      opacity: 1;
    }
    to {
      offset-distance: 100%;
      opacity: 0;
    }
  }
  .pill {
    position: absolute;
    height: 44px;
    margin-top: -22px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px 0 9px;
    border: 0;
    border-radius: 11px;
    background: #121419;
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      0 8px 24px -12px #000;
    color: var(--t1);
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
    transition:
      background-color 0.2s,
      transform var(--dur-press) ease-out;
  }
  .pill:hover {
    background: #171a20;
  }
  .pill:active {
    transform: scale(0.985);
  }
  .ic {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--c);
  }
  .gn {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v {
    margin-left: auto;
    font-weight: 500;
    color: color-mix(in oklab, var(--c) 50%, #fff);
  }
  .tag {
    flex: none;
    font: 500 10px var(--font-mono);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 4px 7px;
    border-radius: 6px;
    background: rgba(95, 214, 255, 0.07);
    color: #9adfff;
    box-shadow: inset 0 0 0 1px rgba(95, 214, 255, 0.18);
  }
  .alert .tag {
    background: rgba(255, 106, 85, 0.1);
    color: #ffaa9a;
    box-shadow: inset 0 0 0 1px rgba(255, 106, 85, 0.3);
  }
  .hub {
    position: absolute;
    width: 64px;
    height: 64px;
    margin-top: -32px;
    border-radius: 16px;
    background: #f1f2f4;
    color: #0b0c0f;
    display: grid;
    place-items: center;
    box-shadow:
      0 0 0 6px rgba(255, 255, 255, 0.04),
      0 0 40px -6px color-mix(in oklab, var(--sc) 80%, transparent);
  }
  .hub svg {
    width: 28px;
    height: 28px;
  }
  .hub::after {
    content: '';
    position: absolute;
    inset: -14px;
    border-radius: 22px;
    border: 1px solid color-mix(in oklab, var(--sc) 50%, transparent);
    animation: ring 3s ease-out infinite;
  }
  @keyframes ring {
    from {
      transform: scale(0.85);
      opacity: 0.9;
    }
    to {
      transform: scale(1.15);
      opacity: 0;
    }
  }
  .hublab {
    position: absolute;
    translate: -50% 0;
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--bg-0);
    white-space: nowrap;
    font: 500 10px var(--font-mono);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--t3);
  }
  .empty {
    position: absolute;
    color: var(--t3);
    translate: 0 -50%;
  }
  @media (prefers-reduced-motion: reduce) {
    .pulse,
    .hub::after {
      display: none;
    }
  }
</style>
