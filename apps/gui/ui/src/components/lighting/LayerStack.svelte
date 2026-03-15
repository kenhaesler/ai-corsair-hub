<script lang="ts">
  import type { RgbZoneConfig, LayerConfig, BlendMode } from '../../lib/types';
  import EffectPicker from './EffectPicker.svelte';
  import EffectControls from './EffectControls.svelte';

  interface Props {
    zone: RgbZoneConfig;
    onupdate: (zone: RgbZoneConfig) => void;
  }

  let { zone, onupdate }: Props = $props();

  let showPicker = $state(false);
  let editingLayer = $state<number | null>(null);

  const blendModes: BlendMode[] = ['Normal', 'Add', 'Multiply', 'Screen', 'Overlay'];

  function updateLayer(index: number, updates: Partial<LayerConfig>) {
    const newLayers = [...zone.layers];
    newLayers[index] = { ...newLayers[index], ...updates };
    onupdate({ ...zone, layers: newLayers });
  }

  function removeLayer(index: number) {
    if (zone.layers.length <= 1) return;
    const newLayers = zone.layers.filter((_, i) => i !== index);
    onupdate({ ...zone, layers: newLayers });
    if (editingLayer === index) editingLayer = null;
    else if (editingLayer !== null && editingLayer > index) editingLayer--;
  }

  function addLayer(effect: LayerConfig['effect']) {
    const newLayer: LayerConfig = {
      effect,
      blend_mode: 'Normal',
      opacity: 1.0,
      enabled: true,
    };
    onupdate({ ...zone, layers: [...zone.layers, newLayer] });
    showPicker = false;
    editingLayer = zone.layers.length; // select the new layer
  }

  function moveLayer(from: number, to: number) {
    const newLayers = [...zone.layers];
    const [moved] = newLayers.splice(from, 1);
    newLayers.splice(to, 0, moved);
    onupdate({ ...zone, layers: newLayers });
    if (editingLayer === from) editingLayer = to;
  }
</script>

<div class="layer-stack">
  <h3 class="section-title">Layers</h3>

  <!-- Layers displayed top (last) to bottom (first) -->
  <div class="layers">
    {#each [...zone.layers].reverse() as layer, ri}
      {@const i = zone.layers.length - 1 - ri}
      <div
        class="layer-card"
        class:active={editingLayer === i}
        class:disabled={!layer.enabled}
      >
        <div class="layer-header" onclick={() => { editingLayer = editingLayer === i ? null : i; }}>
          <button
            class="layer-toggle"
            onclick={(e: MouseEvent) => { e.stopPropagation(); updateLayer(i, { enabled: !layer.enabled }); }}
            title={layer.enabled ? 'Disable' : 'Enable'}
          >
            <div class="toggle-dot" class:on={layer.enabled}></div>
          </button>

          <span class="layer-name">{layer.effect.type}</span>

          <select
            class="blend-select"
            value={layer.blend_mode}
            onchange={(e: Event) => { e.stopPropagation(); updateLayer(i, { blend_mode: (e.target as HTMLSelectElement).value as BlendMode }); }}
          >
            {#each blendModes as mode}
              <option value={mode}>{mode}</option>
            {/each}
          </select>

          <input
            class="opacity-input"
            type="number"
            min="0"
            max="100"
            step="5"
            value={Math.round(layer.opacity * 100)}
            onclick={(e: MouseEvent) => e.stopPropagation()}
            onchange={(e: Event) => { e.stopPropagation(); updateLayer(i, { opacity: parseInt((e.target as HTMLInputElement).value) / 100 }); }}
          />
          <span class="opacity-pct">%</span>

          <div class="layer-actions">
            {#if i < zone.layers.length - 1}
              <button class="move-btn" onclick={(e: MouseEvent) => { e.stopPropagation(); moveLayer(i, i + 1); }} title="Move up">&uarr;</button>
            {/if}
            {#if i > 0}
              <button class="move-btn" onclick={(e: MouseEvent) => { e.stopPropagation(); moveLayer(i, i - 1); }} title="Move down">&darr;</button>
            {/if}
            {#if zone.layers.length > 1}
              <button class="delete-btn" onclick={(e: MouseEvent) => { e.stopPropagation(); removeLayer(i); }} title="Remove">&times;</button>
            {/if}
          </div>
        </div>

        {#if editingLayer === i}
          <div class="layer-controls">
            <EffectControls
              effect={layer.effect}
              onchange={(effect) => updateLayer(i, { effect })}
            />
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if showPicker}
    <EffectPicker onselect={addLayer} onclose={() => { showPicker = false; }} />
  {:else}
    <button class="add-layer" onclick={() => { showPicker = true; }}>+ Add Layer</button>
  {/if}
</div>

<style>
  .layer-stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .layers {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .layer-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    transition: all 0.15s;
  }
  .layer-card.active {
    border-color: var(--accent-dim);
  }
  .layer-card.disabled {
    opacity: 0.5;
  }
  .layer-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    cursor: pointer;
    font-size: 12px;
  }
  .layer-header:hover {
    background: var(--bg-card-hover);
  }
  .layer-toggle {
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .toggle-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: background 0.15s;
  }
  .toggle-dot.on {
    background: var(--success);
  }
  .layer-name {
    flex: 1;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .blend-select {
    width: 70px;
    padding: 2px 4px;
    font-size: 10px;
  }
  .opacity-input {
    width: 36px;
    padding: 2px 4px;
    font-size: 10px;
    text-align: right;
  }
  .opacity-pct {
    font-size: 10px;
    color: var(--text-muted);
  }
  .layer-actions {
    display: flex;
    gap: 2px;
  }
  .move-btn, .delete-btn {
    padding: 0 4px;
    font-size: 12px;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    line-height: 1;
  }
  .move-btn:hover {
    color: var(--text-primary);
  }
  .delete-btn:hover {
    color: var(--danger);
  }
  .layer-controls {
    padding: 8px;
    border-top: 1px solid var(--border);
    background: rgba(0,0,0,0.05);
  }
  .add-layer {
    font-size: 11px;
    padding: 6px;
    color: var(--accent);
    background: transparent;
    border: 1px dashed var(--border);
  }
  .add-layer:hover {
    border-color: var(--accent-dim);
    background: var(--bg-card);
  }
</style>
