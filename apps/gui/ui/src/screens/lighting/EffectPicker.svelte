<!--
  Effect picker (inventory 4.8): "Choose Effect" sheet with × close.
  Colour-coded categories Basic / Motion / Organic / Reactive; each tile
  previews the effect and adds it with the template defaults.
-->
<script lang="ts">
  import Sheet from '../../components/ui/Sheet.svelte';
  import type { EffectConfig } from '../../lib/types';
  import { CATEGORIES, EFFECTS, categoryColor, effectSwatch } from './effects';

  interface Props {
    open: boolean;
    onselect: (e: EffectConfig) => void;
    onclose: () => void;
  }
  let { open = $bindable(), onselect, onclose }: Props = $props();
</script>

<Sheet bind:open title="Choose Effect" subtitle="Added on top of the stack. Normal blend, full opacity." width={600} {onclose}>
  <div class="cats">
    {#each CATEGORIES as cat (cat)}
      <section class="cat" aria-labelledby="ep-{cat}">
        <h3 class="cat-h" id="ep-{cat}" style:--cc={categoryColor(cat)}>
          <span class="dot" aria-hidden="true"></span>{cat}
        </h3>
        <div class="grid">
          {#each EFFECTS.filter((e) => e.category === cat) as e (e.name)}
            <button type="button" class="tile" style:--cc={categoryColor(cat)} onclick={() => onselect(structuredClone(e.config))}>
              <span class="sw" style:background={effectSwatch(e.config)} aria-hidden="true"></span>
              <span class="nm">{e.name}</span>
              <span class="bl">{e.blurb}</span>
            </button>
          {/each}
        </div>
      </section>
    {/each}
  </div>
</Sheet>

<style>
  .cats {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .cat-h {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--cc);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--cc);
    box-shadow: 0 0 8px -1px var(--cc);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 10px 12px 12px;
    border: 0;
    border-radius: 12px;
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .tile:hover {
    background: var(--mat-2-hover);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--hair-inner);
  }
  .tile:active {
    transform: scale(0.97);
  }
  .tile:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }
  .sw {
    align-self: stretch;
    height: 8px;
    margin-bottom: 8px;
    border-radius: 999px;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.3);
  }
  .nm {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--t1);
  }
  .bl {
    font-size: 12px;
    line-height: 16px;
    color: var(--t3);
  }
</style>
