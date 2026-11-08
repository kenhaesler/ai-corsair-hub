<!--
  One layer of the stack (inventory 4.7): enable toggle (disabled layer is
  dimmed), effect name, blend select, opacity 0–100 % in steps of 5, move
  up / move down / remove, and an expandable body with EffectControls.
  The body opens on a spring (height), like Disclosure.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import ArrowUp from 'lucide-svelte/icons/arrow-up';
  import ArrowDown from 'lucide-svelte/icons/arrow-down';
  import Trash from 'lucide-svelte/icons/trash-2';
  import Toggle from '../../components/ui/Toggle.svelte';
  import Select from '../../components/ui/Select.svelte';
  import Slider from '../../components/ui/Slider.svelte';
  import IconButton from '../../components/ui/IconButton.svelte';
  import { createSpring, springTo } from '../../lib/motion.svelte';
  import type { BlendMode, EffectConfig, LayerConfig } from '../../lib/types';
  import EffectControls from './EffectControls.svelte';
  import { categoryColor, effectCategory, effectName, effectSwatch } from './effects';

  interface Props {
    layer: LayerConfig;
    /** 1-based position from the top, for labels */
    position: number;
    expanded: boolean;
    canUp: boolean;
    canDown: boolean;
    canRemove: boolean;
    ontoggle: () => void;
    onchange: (patch: Partial<LayerConfig>) => void;
    oneffect: (e: EffectConfig) => void;
    onup: () => void;
    ondown: () => void;
    onremove: () => void;
  }
  let { layer, position, expanded, canUp, canDown, canRemove, ontoggle, onchange, oneffect, onup, ondown, onremove }: Props = $props();

  const BLENDS: { value: BlendMode; label: string }[] = [
    { value: 'Normal', label: 'Normal' },
    { value: 'Add', label: 'Add' },
    { value: 'Multiply', label: 'Multiply' },
    { value: 'Screen', label: 'Screen' },
    { value: 'Overlay', label: 'Overlay' },
  ];

  const id = `ly-${Math.random().toString(36).slice(2, 8)}`;
  const name = $derived(effectName(layer.effect));
  const cat = $derived(effectCategory(layer.effect));

  // spring-height body
  const startOpen = untrack(() => expanded);
  const h = createSpring(startOpen ? 1 : 0, { stiffness: 0.16, damping: 0.86, precision: 0.002 });
  let mounted = $state(startOpen);
  let inner = $state<HTMLDivElement>();
  let contentH = $state(0);
  $effect(() => {
    const open = expanded;
    if (open) mounted = true;
    springTo(h, open ? 1 : 0).then(() => {
      if (!expanded) mounted = false;
    });
  });
  $effect(() => {
    const el = inner;
    if (!el) return;
    const ro = new ResizeObserver(() => (contentH = el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  });
  const settled = $derived(expanded && h.current > 0.999);
  let opacity = $state(100);
  $effect(() => {
    opacity = Math.round(layer.opacity * 100);
  });
</script>

<div class="layer" class:off={!layer.enabled} class:open={expanded}>
  <div class="head">
    <Toggle
      size="sm"
      checked={layer.enabled}
      label="{layer.enabled ? 'Disable' : 'Enable'} {name} layer"
      onchange={(v) => onchange({ enabled: v })}
    />
    <button type="button" class="title" aria-expanded={expanded} aria-controls={id} onclick={ontoggle}>
      <span class="sw" class:hide={expanded} style:background={effectSwatch(layer.effect)} aria-hidden="true"></span>
      <span class="nm">{name}</span>
      <span class="cat" style:color={categoryColor(cat)}>{cat}</span>
      <span class="chev" class:rot={expanded} aria-hidden="true"><ChevronRight size={14} strokeWidth={2.25} /></span>
    </button>
    <div class="acts">
      <IconButton icon={ArrowUp} size="sm" label="Move up" disabled={!canUp} onclick={onup} />
      <IconButton icon={ArrowDown} size="sm" label="Move down" disabled={!canDown} onclick={ondown} />
      <IconButton icon={Trash} size="sm" label="Remove" disabled={!canRemove} onclick={onremove} />
    </div>
  </div>

  <div class="mix">
    <Select size="sm" label="Blend" showLabel value={layer.blend_mode} options={BLENDS} onchange={(v) => onchange({ blend_mode: v })} />
    <div class="op">
      <Slider
        label="Opacity of layer {position}"
        hideLabel
        min={0}
        max={100}
        step={5}
        bind:value={opacity}
        format={(v) => `${v}%`}
        oninput={(v) => onchange({ opacity: v / 100 })}
      />
    </div>
  </div>

  {#if mounted}
    <div class="body" {id} style:height={settled ? 'auto' : `${contentH * Math.max(0, h.current)}px`} style:opacity={Math.min(1, h.current * 1.6)}>
      <div class="inner" bind:this={inner}>
        <EffectControls effect={layer.effect} onchange={oneffect} />
      </div>
    </div>
  {/if}
</div>

<style>
  .layer {
    padding: 10px 12px 12px 14px;
    transition: background-color var(--dur-fast) ease-out;
  }
  .layer.open {
    background: rgba(255, 255, 255, 0.02);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px;
    margin-left: -4px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) ease-out;
  }
  .title:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .title:active {
    background: rgba(255, 255, 255, 0.07);
  }
  .sw {
    flex: none;
    width: 28px;
    height: 10px;
    border-radius: 999px;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.3);
  }
  /* while expanded the colour pickers below show these colours; the swatch steps aside */
  .sw {
    transition:
      opacity var(--dur-med) var(--ease-out-expo),
      width var(--dur-med) var(--ease-out-expo),
      margin var(--dur-med) var(--ease-out-expo);
  }
  .sw.hide {
    opacity: 0;
    width: 0;
    margin-right: -10px;
  }
  .nm {
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
    letter-spacing: -0.006em;
    color: var(--t1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cat {
    font-size: 12px;
    line-height: 16px;
    font-weight: 500;
    opacity: 0.85;
  }
  .chev {
    margin-left: auto;
    display: grid;
    color: var(--t3);
    transition: transform var(--dur-med) var(--ease-out-expo);
  }
  .chev.rot {
    transform: rotate(90deg);
  }
  .acts {
    display: flex;
    gap: 2px;
  }
  .mix {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 16px;
    padding: 6px 0 0 50px;
  }
  .op {
    min-width: 0;
  }
  .off .mix,
  .off .sw,
  .off .nm,
  .off .cat,
  .off .body {
    opacity: 0.45;
  }
  .body {
    overflow: hidden;
  }
  .inner {
    padding: 14px 0 2px 50px;
  }
  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
</style>
