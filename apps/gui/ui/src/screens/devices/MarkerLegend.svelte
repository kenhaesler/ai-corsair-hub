<!--
  Marker legend (inventory 3.4), shown while the LED marker diagnostic runs:
  intro + colour legend, markers-mode note, power-factor sentence, Walk's
  live "LED N / led_count" (with a strip of cells) and the manifest status.
  The per-hub part (fw, LED total, health, slot table, skipped rows) lives in
  each hub section's header and table, so hub facts are shown once.
-->
<script lang="ts">
  import FileCheck from 'lucide-svelte/icons/file-check';
  import FileWarning from 'lucide-svelte/icons/file-warning';
  import type { LedMarkerReport } from '../../lib/api';

  interface Props {
    report: LedMarkerReport;
    walkLed: number;
  }
  let { report, walkLed }: Props = $props();

  const rgb = (c: [number, number, number]) => `rgb(${c[0]},${c[1]},${c[2]})`;
  const walkCells = $derived(report.walk ? Array.from({ length: report.walk.led_count }, (_, i) => i) : []);
</script>

<section class="legend" aria-label="Marker legend">
  <p class="intro t-body">
    {report.mode === 'walk' ? 'Walk' : 'Every LED slot shows a marker'} for {report.duration_secs} seconds:
  </p>
  <ul class="key">
    {#each report.legend as entry (entry.label)}
      <li>
        <span class="sw" style:background={rgb(entry.rgb)}></span>
        <span class="lab">{entry.label}</span>
        <span class="eq">= {entry.meaning}</span>
      </li>
    {/each}
  </ul>
  <div class="notes t-callout">
    {#if report.mode === 'markers'}
      <p>All other LEDs use the device's palette colour.</p>
    {/if}
    <p>
      Colours are scaled by port power protection (factor <span class="num">{report.power_factor}</span>, <span class="num">{report.total_leds}</span> LEDs total), exactly like normal rendering.
    </p>
  </div>

  {#if report.walk}
    <div class="walk" aria-live="off">
      <p class="walk-pos t-figure-m">LED {walkLed} / {report.walk.led_count}</p>
      <div class="cells" aria-hidden="true">
        {#each walkCells as i (i)}
          <span class="cell" class:on={i === walkLed} class:past={i < walkLed}></span>
        {/each}
      </div>
    </div>
  {/if}

  {#if report.file_path}
    <p class="manifest t-callout ok">
      <FileCheck size={15} strokeWidth={2} />
      <span>Manifest saved to <span class="mono path">{report.file_path}</span></span>
    </p>
  {:else if report.file_error}
    <p class="manifest t-callout bad">
      <FileWarning size={15} strokeWidth={2} />
      <span>Manifest export failed: {report.file_error}</span>
    </p>
  {/if}
</section>

<style>
  .legend {
    margin-top: var(--sp-4);
    padding: var(--sp-5);
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
  }
  .intro {
    color: var(--t1);
    font-weight: 500;
  }
  .key {
    list-style: none;
    margin: var(--sp-3) 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: var(--sp-1) var(--sp-6);
  }
  .key li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 28px;
    font-size: 13px;
  }
  .sw {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 5px;
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.22),
      0 2px 4px -1px rgb(0 0 0 / 0.5);
  }
  .lab {
    font-weight: 600;
    color: var(--t1);
  }
  .eq {
    color: var(--t2);
  }
  .notes {
    margin-top: var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    color: var(--t2);
    max-width: 75ch;
  }

  .walk {
    margin-top: var(--sp-5);
    padding: var(--sp-4);
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.22);
    box-shadow: inset 0 0 0 1px var(--hair);
  }
  .walk-pos {
    color: var(--t1);
    margin-bottom: var(--sp-3);
  }
  .cells {
    display: flex;
    gap: 2px;
    height: 14px;
  }
  .cell {
    flex: 1 1 0;
    min-width: 2px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.08);
  }
  .cell.past {
    background: rgba(255, 255, 255, 0.16);
  }
  .cell.on {
    background: #fff;
    box-shadow: 0 0 0 1px rgb(255 255 255 / 0.5), 0 2px 8px rgb(255 255 255 / 0.35);
  }

  .manifest {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: var(--sp-5);
    color: var(--t2);
  }
  .manifest :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .manifest.ok :global(svg) {
    color: var(--success);
  }
  .manifest.bad {
    color: var(--warn-text);
  }
  .manifest.bad :global(svg) {
    color: var(--warn);
  }
  .path {
    color: var(--t1);
    overflow-wrap: anywhere;
  }
</style>
