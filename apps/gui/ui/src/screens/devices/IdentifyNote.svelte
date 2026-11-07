<!--
  Identify intro (inventory 3.3). The per-device legend (swatch, Ch N, type,
  colour name) lives in each hub's table as a Colour column while Identify
  runs, so nothing is listed twice.
-->
<script lang="ts">
  import Info from 'lucide-svelte/icons/info';
  import { IDENTIFY_SECS } from './diagnostics.svelte';
</script>

<p class="note t-callout">
  <Info size={15} strokeWidth={2.25} />
  <span>
    Every device now lights up a unique colour for {IDENTIFY_SECS} seconds. Look at your rig and match each physical
    fan/strip to the colour in the hub tables below — that tells you which channel number the firmware actually assigned.
  </span>
</p>

<style>
  .note {
    display: flex;
    gap: var(--sp-3);
    margin-top: var(--sp-4);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--r-row);
    background: var(--info-surface);
    box-shadow: inset 0 0 0 1px var(--info-hair);
    color: var(--info-text);
    max-width: 78ch;
  }
  .note :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent);
  }
</style>
