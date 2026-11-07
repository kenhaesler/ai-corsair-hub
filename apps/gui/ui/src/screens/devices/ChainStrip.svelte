<!--
  Daisy chain of one hub, in channel order: the hub, then each device as a
  glyph tile joined by a wire. Chain order and device kind are detectable
  facts (HubSnapshot.devices), so nothing here encodes a physical placement.
  Fans turn at a rate that follows their live RPM (rotation only; the number lives in the table) (display.snapshot, so the
  time-scrub rewinds them too); a dot marks the fan group that drives them.
  Hover links a node to its table row. Presentational: the table beside it is
  the accessible form of the same data.
-->
<script lang="ts">
  import Fan from 'lucide-svelte/icons/fan';
  import Cable from 'lucide-svelte/icons/cable';
  import Microchip from 'lucide-svelte/icons/microchip';
  import HubGlyph from './HubGlyph.svelte';
  import type { HubSnapshot } from '../../lib/types';
  import { display } from '../../lib/stores/scrub.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { groupColor } from '../../lib/format';
  import { displayNameFromTree, deviceTypeLabel } from '../../lib/identity';
  import { rafLoop, reducedMotion } from '../../lib/motion.svelte';

  interface Props {
    hub: HubSnapshot;
    /** device_id currently hovered (shared with the table) */
    active: string | null;
  }
  let { hub, active = $bindable() }: Props = $props();

  const MAX_RPM = 2400;
  const devices = $derived(hub.devices.slice().sort((a, b) => a.channel - b.channel));

  const fanOf = (id: string) => display.snapshot?.fans.find((f) => f.device_id === id);

  function groupDot(id: string): { color: string; name: string } | null {
    const name = fanOf(id)?.group_name;
    if (!name) return null;
    const i = configStore.config?.fan_groups.findIndex((g) => g.name === name) ?? -1;
    return { color: groupColor(Math.max(0, i)), name };
  }

  function kind(d: (typeof devices)[number]): 'fan' | 'strips' | 'adapter' | 'other' {
    if (d.strips && d.strips.length > 0) return 'strips';
    if (d.rpm !== null || fanOf(d.device_id)) return 'fan';
    if (/adapter/i.test(d.device_type)) return 'adapter';
    return 'other';
  }

  // Fan rotation: one rAF loop, only while this tab is showing.
  const spinEls = new Map<string, HTMLElement>();
  const angles: Record<string, number> = {};
  function spinner(node: HTMLElement, id: string) {
    spinEls.set(id, node);
    return {
      destroy() {
        if (spinEls.get(id) === node) spinEls.delete(id);
      },
    };
  }
  $effect(() => {
    if (ui.activeTab !== 'devices' || reducedMotion.current) return;
    return rafLoop((dt) => {
      for (const [id, el] of spinEls) {
        const rpm = display.snapshot?.fans.find((f) => f.device_id === id)?.rpm ?? 0;
        const a = ((angles[id] ?? 0) + Math.min(1, rpm / MAX_RPM) * 330 * dt) % 360;
        angles[id] = a;
        el.style.transform = `rotate(${a.toFixed(1)}deg)`;
      }
    });
  });
</script>

<div class="strip" aria-hidden="true">
  <ol class="chain">
    <li class="node">
      <span class="tile hubtile"><HubGlyph size={24} /></span>
      <span class="ch">Hub</span>
    </li>
    {#each devices as d (d.device_id)}
      {@const k = kind(d)}
      {@const dot = k === 'fan' ? groupDot(d.device_id) : null}
      <li class="wire"></li>
      <li
        class="node"
        class:hot={active === d.device_id}
        onmouseenter={() => (active = d.device_id)}
        onmouseleave={() => (active = null)}
        title="{displayNameFromTree(d.device_id, { config: configStore.config, hubs: [hub] })} · {deviceTypeLabel(d)}{dot ? ` · ${dot.name}` : ''}"
      >
        <span class="tile" class:dashed={k === 'adapter'}>
          {#if k === 'fan'}
            <span class="spin" use:spinner={d.device_id}><Fan size={22} strokeWidth={1.75} /></span>
          {:else if k === 'strips'}
            <span class="strips">
              {#each d.strips ?? [] as s (s.index)}
                <span class="lane">
                  {#each Array.from({ length: 7 }, (_, i) => i) as i (i)}
                    <i style:opacity={0.28 + (i / 6) * 0.72}></i>
                  {/each}
                </span>
              {/each}
            </span>
          {:else if k === 'adapter'}
            <Cable size={20} strokeWidth={1.75} />
          {:else}
            <Microchip size={20} strokeWidth={1.75} />
          {/if}
          {#if dot}<span class="gdot" style:background={dot.color}></span>{/if}
        </span>
        <span class="ch num">Ch {d.channel}</span>
      </li>
    {/each}
  </ol>
</div>

<style>
  .strip {
    overflow-x: auto;
    padding: var(--sp-5) var(--sp-5) var(--sp-4);
    background: rgba(0, 0, 0, 0.2);
    box-shadow: inset 0 -1px 0 var(--hair);
  }
  .chain {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    align-items: flex-start;
    width: max-content;
    min-width: 100%;
  }
  .node {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 72px;
    gap: 2px;
    cursor: default;
  }
  .tile {
    position: relative;
    width: 52px;
    height: 52px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    color: var(--t1);
    background: var(--mat-raised);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner),
      0 2px 6px -2px rgb(0 0 0 / 0.5);
    transition:
      background-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      transform var(--dur-med) var(--ease-out-expo);
  }
  .hubtile {
    color: var(--accent-strong);
    background: var(--accent-softer);
    box-shadow:
      inset 0 0 0 1px var(--info-hair),
      var(--hair-inner),
      0 2px 6px -2px rgb(0 0 0 / 0.5);
  }
  .tile.dashed {
    background: transparent;
    box-shadow: none;
    outline: 1px dashed var(--hair-strong);
    outline-offset: -1px;
    color: var(--t3);
  }
  .node.hot .tile {
    background: var(--mat-raised-hover);
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--hair-inner),
      0 4px 10px -3px rgb(0 0 0 / 0.55);
    transform: translateY(-2px);
  }
  .wire {
    flex: none;
    width: 16px;
    height: 2px;
    margin-top: 25px;
    border-radius: 2px;
    background: linear-gradient(90deg, rgba(255, 255, 255, 0.1), rgba(255, 255, 255, 0.2), rgba(255, 255, 255, 0.1));
  }
  .spin {
    display: grid;
    will-change: transform;
  }
  .gdot {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px rgb(24 26 33);
  }
  .strips {
    display: flex;
    flex-direction: column;
    gap: 5px;
    color: var(--s-group-4);
  }
  .lane {
    display: flex;
    gap: 2px;
  }
  .lane i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
  }
  .ch {
    margin-top: 6px;
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--t1);
  }
</style>
