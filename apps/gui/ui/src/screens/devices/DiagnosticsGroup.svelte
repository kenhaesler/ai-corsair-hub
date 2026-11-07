<!--
  Diagnostics (inventory 3.1): Identify Channels, LED marker mode + run, and
  the Walk-only device / step rows. One grouped inset; a hairline countdown
  runs along the bottom of whichever row is active.
-->
<script lang="ts">
  import { slide } from 'svelte/transition';
  import { expoOut } from 'svelte/easing';
  import ScanLine from 'lucide-svelte/icons/scan-line';
  import Crosshair from 'lucide-svelte/icons/crosshair';
  import Footprints from 'lucide-svelte/icons/footprints';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import InsetRow from '../../components/ui/InsetRow.svelte';
  import Button from '../../components/ui/Button.svelte';
  import Select from '../../components/ui/Select.svelte';
  import NumberField from '../../components/ui/NumberField.svelte';
  import { reducedMotion } from '../../lib/motion.svelte';
  import type { Diagnostics, MarkerKind } from './diagnostics.svelte';

  interface Props {
    diag: Diagnostics;
    walkDevices: { id: string; label: string }[];
  }
  let { diag, walkDevices }: Props = $props();

  const MODES: { value: MarkerKind; label: string }[] = [
    { value: 'markers', label: 'Markers' },
    { value: 'markers_black', label: 'Markers (black body)' },
    { value: 'ruler', label: 'Ruler' },
    { value: 'walk', label: 'Walk' },
  ];
  const deviceOptions = $derived([{ value: '', label: 'Device…' }, ...walkDevices.map((d) => ({ value: d.id, label: d.label }))]);

  const slideOpts = $derived({ duration: reducedMotion.current ? 0 : 320, easing: expoOut });

</script>

<InsetGroup title="Diagnostics" id="diag">
  <div class="rw">
    <InsetRow
      icon={ScanLine}
      iconColor="var(--s-group-3)"
      label="Channel identification"
    >
      <Button size="sm" onclick={() => diag.runIdentify()} disabled={diag.busy} loading={diag.identifyRunning}>
        {diag.identifyRunning ? `Identifying… ${diag.identifyRemaining}s` : 'Identify Channels'}
      </Button>
    </InsetRow>
    {#if diag.identifyRunning}
      <span class="prog" aria-hidden="true" style:transform="scaleX({diag.identifyFrac})"></span>
    {/if}
  </div>

  <div class="rw">
    <InsetRow
      icon={Crosshair}
      iconColor="var(--s-group-4)"
      label="LED slot markers"
    >
      <Select
        size="sm"
        label="LED marker mode"
        options={MODES}
        bind:value={diag.modeKind}
        disabled={diag.busy}
      />
      <Button size="sm" onclick={() => diag.runMarkers()} disabled={diag.busy} loading={diag.markerRunning}>
        {diag.markerRunning ? `LED markers… ${diag.markerRemaining}s` : 'LED markers'}
      </Button>
    </InsetRow>
    {#if diag.markerRunning}
      <span class="prog" aria-hidden="true" style:transform="scaleX({diag.markerFrac})"></span>
    {/if}
  </div>

  {#if diag.modeKind === 'walk'}
    <div class="walk" transition:slide={slideOpts}>
      <InsetRow icon={Footprints} iconColor="var(--s-group-5)" label="Walk target">
        <Select size="sm" label="Walk device" options={deviceOptions} bind:value={diag.walkDeviceId} disabled={diag.busy} />
        <NumberField
          label="Walk step (ms)"
          bind:value={diag.walkStepMs}
          min={100}
          max={2000}
          step={50}
          unit="ms"
          width={64}
          disabled={diag.busy}
        />
      </InsetRow>
    </div>
  {/if}
</InsetGroup>

<style>
  .rw {
    position: relative;
  }
  .walk {
    overflow: hidden;
  }
  .prog {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    background: var(--accent);
    transform-origin: left center;
    transition: transform 500ms linear;
    opacity: 0.85;
  }
  @media (prefers-reduced-motion: reduce) {
    .prog {
      transition: none;
    }
  }
</style>
