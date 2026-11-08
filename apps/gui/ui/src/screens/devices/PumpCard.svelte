<!--
  Pumps panel (Devices): every pump we can see, with its source, live rpm
  and the duty the controlling side applies. Motherboard pumps come from
  LibreHardwareMonitor (BIOS controls them); LINK pumps from the hubs.
  Reads display.snapshot, so it follows the time-scrub.
-->
<script lang="ts">
  import Droplet from 'lucide-svelte/icons/droplet';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import type { PumpReading } from '../../lib/types';

  interface Props {
    pumps: PumpReading[];
  }
  let { pumps }: Props = $props();
</script>

<InsetGroup title="Pumps" footer="A pump on a motherboard header is driven by the BIOS fan curve; Corsair Hub reads it and watches for a stall.">
  {#each pumps as p (p.id)}
    <div class="row" style:--c="var(--s-water)">
      <span class="tile" aria-hidden="true"><Droplet size={14} strokeWidth={2} /></span>
      <div class="nm">
        <b>{p.name}</b>
        <span>{p.source === 'link' ? 'iCUE LINK pump' : 'Motherboard header'}{p.duty_pct !== null ? ` · ${Math.round(p.duty_pct)}% duty` : ''}</span>
      </div>
      <span class="val num"><AnimatedNumber value={p.rpm} /><small>rpm</small></span>
    </div>
  {/each}
</InsetGroup>

<style>
  .row {
    display: grid;
    grid-template-columns: 26px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    min-height: 56px;
    padding: 8px 18px;
  }
  .tile {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    color: var(--s-water);
  }
  .nm {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .nm b {
    font-size: 14px;
    font-weight: 500;
  }
  .nm span {
    font-size: 12px;
    color: var(--t3);
  }
  .val {
    font-weight: 300;
    font-size: 26px;
    letter-spacing: -0.03em;
    color: color-mix(in oklab, var(--c) 40%, #fff);
    text-shadow: 0 0 20px color-mix(in oklab, var(--c) 40%, transparent);
  }
  .val small {
    font-size: 12px;
    color: var(--t3);
    margin-left: 4px;
    text-shadow: none;
  }
</style>
