<!--
  Layers (inventory 4.7): listed top-first (the last array entry renders on
  top), each row expandable; "+ Add Layer" opens the effect picker and the
  new layer arrives auto-expanded. Rows move with a FLIP animation.
-->
<script lang="ts">
  import { flip } from 'svelte/animate';
  import { cubicOut } from 'svelte/easing';
  import Plus from 'lucide-svelte/icons/plus';
  import Layers from 'lucide-svelte/icons/layers';
  import Button from '../../components/ui/Button.svelte';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import EmptyState from '../../components/ui/EmptyState.svelte';
  import { reducedMotion } from '../../lib/motion.svelte';
  import type { EffectConfig, LayerConfig, RgbZoneConfig } from '../../lib/types';
  import LayerRow from './LayerRow.svelte';
  import EffectPicker from './EffectPicker.svelte';
  import { newLayer } from './effects';

  interface Props {
    zone: RgbZoneConfig;
    onsave: () => void;
  }
  let { zone, onsave }: Props = $props();

  // Stable ids per layer object (layers carry no id of their own).
  const ids = new WeakMap<object, number>();
  let seq = 0;
  const idOf = (l: object) => {
    let v = ids.get(l);
    if (v === undefined) {
      v = ++seq;
      ids.set(l, v);
    }
    return v;
  };

  let expanded = $state<number | null>(null);
  let picker = $state(false);

  const rows = $derived(
    zone.layers
      .map((layer, i) => ({ layer, i, id: idOf(layer) }))
      .reverse(),
  );

  function patch(i: number, p: Partial<LayerConfig>) {
    Object.assign(zone.layers[i], p);
    onsave();
  }
  function setEffect(i: number, e: EffectConfig) {
    zone.layers[i].effect = e;
    onsave();
  }
  function move(from: number, to: number) {
    if (to < 0 || to >= zone.layers.length) return;
    const [m] = zone.layers.splice(from, 1);
    zone.layers.splice(to, 0, m);
    onsave();
  }
  function remove(i: number) {
    if (zone.layers.length <= 1) return;
    zone.layers.splice(i, 1);
    onsave();
  }
  function add(e: EffectConfig) {
    zone.layers.push(newLayer(e));
    expanded = idOf(zone.layers[zone.layers.length - 1]);
    picker = false;
    onsave();
  }
</script>

<InsetGroup title="Layers" footer="The top layer blends over the ones beneath it. Disabled layers are skipped.">
  {#snippet action()}
    <Button variant="plain" size="sm" icon={Plus} onclick={() => (picker = true)}><span class="sr-only">{'+ '}</span>Add Layer</Button>
  {/snippet}
  {#if zone.layers.length === 0}
    <div class="empty">
      <EmptyState icon={Layers} title="This zone has no layers" message="Its devices stay dark until you add an effect." compact>
        {#snippet action()}
          <Button size="sm" variant="secondary" icon={Plus} onclick={() => (picker = true)}><span class="sr-only">{'+ '}</span>Add Layer</Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else}
    {#each rows as r, ri (r.id)}
      <div class="slot" animate:flip={{ duration: reducedMotion.current ? 0 : 320, easing: cubicOut }}>
        <LayerRow
          layer={r.layer}
          position={ri + 1}
          expanded={expanded === r.id}
          canUp={r.i < zone.layers.length - 1}
          canDown={r.i > 0}
          canRemove={zone.layers.length > 1}
          ontoggle={() => (expanded = expanded === r.id ? null : r.id)}
          onchange={(p) => patch(r.i, p)}
          oneffect={(e) => setEffect(r.i, e)}
          onup={() => move(r.i, r.i + 1)}
          ondown={() => move(r.i, r.i - 1)}
          onremove={() => remove(r.i)}
        />
      </div>
    {/each}
  {/if}
</InsetGroup>

<EffectPicker bind:open={picker} onselect={add} onclose={() => (picker = false)} />

<style>
  .slot {
    position: relative;
  }
  .empty {
    padding: 8px 0;
  }
</style>
