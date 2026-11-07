<!--
  PSU section (inventory 3.6): model from psu.model (never hard-coded) and the
  serial. The live output reads display.snapshot so it follows the scrub.
-->
<script lang="ts">
  import Power from 'lucide-svelte/icons/power';
  import type { PsuDeviceInfo } from '../../lib/types';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import { display } from '../../lib/stores/scrub.svelte';

  interface Props {
    psu: PsuDeviceInfo;
  }
  let { psu }: Props = $props();

  const name = $derived(/^corsair\b/i.test(psu.model) ? psu.model : `Corsair ${psu.model}`);
  const live = $derived(display.snapshot?.psu ?? null);
</script>

<section class="psu" aria-labelledby="psu-title">
  <span class="tile" aria-hidden="true"><Power size={22} strokeWidth={1.75} /></span>
  <div class="id">
    <h3 id="psu-title" class="t-title3">{name}</h3>
    <p class="serial">
      <span class="lab">Serial</span>
      <span class="mono">{psu.serial}</span>
    </p>
  </div>
  {#if live}
    <div class="out">
      <span class="fig"><AnimatedNumber value={live.total_power} class="t-figure-m" /><span class="u">W</span></span>
      <span class="cap">Output</span>
    </div>
  {/if}
</section>

<style>
  .psu {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
  }
  .tile {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: var(--s-power);
    background: rgba(255, 214, 10, 0.1);
    box-shadow: inset 0 0 0 1px rgba(255, 214, 10, 0.22);
  }
  .id {
    min-width: 0;
    flex: 1;
  }
  .serial {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px var(--sp-2);
    margin-top: 2px;
    font-size: 12px;
    line-height: 16px;
  }
  .lab {
    color: var(--t3);
    font-weight: 500;
  }
  .serial .mono {
    color: var(--t2);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .out {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .fig {
    display: inline-flex;
    align-items: baseline;
    gap: 2px;
    color: var(--t1);
  }
  .u {
    font-size: 13px;
    font-weight: 600;
    color: var(--t2);
  }
  .cap {
    font-size: 11px;
    line-height: 14px;
    color: var(--t3);
    font-weight: 500;
  }
</style>
