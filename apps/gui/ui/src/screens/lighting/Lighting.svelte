<!--
  Lighting (inventory 4). Hero live preview stage on black glass, then a
  split of grouped inset sections: Zones / Devices / Order / Presets on the
  left, Cross-Device Flow and the Layer stack on the right. Master controls
  live in the toolbar's trailing slot. Every change auto-saves
  (setRgbConfig, coalesced); errors become toasts.
  The rgb-frame listener runs only while this tab is active.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import ToolbarActions from '../../components/shell/ToolbarActions.svelte';
  import Skeleton from '../../components/ui/Skeleton.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { initRgbListener, rgb, stopRgbListener } from '../../lib/stores/rgb.svelte';
  import { lighting, loadLighting, persist } from './state.svelte';
  import LiveStage from './LiveStage.svelte';
  import MasterControls from './MasterControls.svelte';
  import ZonesSection from './ZonesSection.svelte';
  import PresetsSection from './PresetsSection.svelte';
  import FlowSection from './FlowSection.svelte';
  import LayersSection from './LayersSection.svelte';
  import { ledCountOf, refMatches } from './effects';

  const active = $derived(ui.activeTab === 'lighting');

  onMount(() => {
    loadLighting();
  });

  // frames only while this page is the active tab
  $effect(() => {
    if (!active) return;
    initRgbListener();
    return () => stopRgbListener();
  });

  const cfg = $derived(rgb.config);
  const zone = $derived(cfg?.zones[rgb.selectedZone] ?? null);
  const save = () => persist();

  /** Lit devices that no zone lists (they stay dark). Shown nowhere else on the page. */
  const unzoned = $derived.by(() => {
    if (!cfg || cfg.zones.length === 0) return null;
    let n = 0;
    for (const hub of lighting.hubs)
      for (const d of hub.devices) {
        if (ledCountOf(hub.serial, d, rgb.frames) === 0) continue;
        if (!cfg.zones.some((z) => z.devices.some((r) => refMatches(r, hub.serial, d)))) n++;
      }
    return n;
  });

  /** live while dragging (the preview follows); persist coalesces the saves */
  function zoneBrightness(v: number) {
    if (!zone) return;
    zone.brightness = v;
    persist();
  }
</script>

<ToolbarActions>
  <MasterControls />
</ToolbarActions>

{#snippet unzonedLine()}
  {#if unzoned === 0}
    <span>Every lit device belongs to a zone</span>
  {:else}
    <span>{unzoned} lit {unzoned === 1 ? 'device is' : 'devices are'} in no zone and stay dark</span>
  {/if}
{/snippet}

<LargeTitlePage title="Lighting" subtitle={unzoned === null || lighting.hubs.length === 0 ? undefined : unzonedLine} maxWidth={1440}>

  {#if !lighting.loaded}
    <div class="stage-skel" aria-busy="true"><Skeleton height="300px" /></div>
  {:else}
    <LiveStage {zone} hubs={lighting.hubs} {active} loaded={lighting.loaded} onzonebrightness={zoneBrightness} />
  {/if}

  <div class="split">
    <div class="left">
      {#if lighting.loaded}
        <ZonesSection hubs={lighting.hubs} onsave={save} />
      {:else}
        <Skeleton height="180px" />
        <Skeleton height="260px" />
      {/if}
    </div>
    <div class="right">
      {#if !lighting.loaded}
        <Skeleton height="140px" />
        <Skeleton height="220px" />
      {:else if zone}
        {#key rgb.selectedZone}
          <FlowSection {zone} onsave={save} />
          <LayersSection {zone} onsave={save} />
        {/key}
        <PresetsSection />
      {:else}
        <!-- no zone: Flow/Layers need one; the "No zones configured" state lives in Zones -->
        <PresetsSection />
      {/if}
    </div>
  </div>
</LargeTitlePage>

<style>
  .stage-skel :global(*) {
    border-radius: var(--r-sheet);
  }
  .split {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 32px;
    margin-top: 40px;
    align-items: start;
  }
  .left,
  .right {
    display: flex;
    flex-direction: column;
    gap: 28px;
    min-width: 0;
  }
</style>
