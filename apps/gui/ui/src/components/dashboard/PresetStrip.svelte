<script lang="ts">
  import { onMount } from 'svelte';
  import { applyPresetToGroup, getFanPresets } from '../../lib/api';
  import { loadConfig, configStore } from '../../lib/stores/config.svelte';
  import { computePresetState, type PresetState } from '../../lib/presets';
  import type { FanPresetDef } from '../../lib/types';

  let presets = $state<FanPresetDef[]>([]);
  let applying = $state<string | null>(null);
  let selectedGroup = $state<string | null>(null); // null = all groups

  onMount(async () => {
    try {
      presets = await getFanPresets();
    } catch {
      presets = [];
    }
  });

  // Available group names — derived from the live config so adding a
  // new group in the Fans tab updates the selector without reload.
  const groupNames = $derived(
    configStore.config?.fan_groups?.map((g) => g.name) ?? [],
  );

  // If the selected group gets deleted elsewhere, fall back to All.
  $effect(() => {
    if (selectedGroup && !groupNames.includes(selectedGroup)) {
      selectedGroup = null;
    }
  });

  function stateFor(preset: FanPresetDef): PresetState {
    return computePresetState(configStore.config, preset, selectedGroup);
  }

  // Each preset's curve preview normalizes to its own temp domain so
  // presets with a narrow range (e.g. water_priority at 28–45°C) fill
  // the card instead of being squashed into the leftmost sliver.
  function curveDomain(points: { temp: number; duty: number }[]) {
    const temps = points.map((p) => p.temp);
    return { tMin: Math.min(...temps), tMax: Math.max(...temps) };
  }

  function curvePath(points: { temp: number; duty: number }[]): string {
    const w = 100;
    const h = 40;
    const { tMin, tMax } = curveDomain(points);
    const range = Math.max(tMax - tMin, 1);
    return points
      .map((p, i) => {
        const x = ((p.temp - tMin) / range) * w;
        const y = h - (p.duty / 100) * h;
        return `${i === 0 ? 'M' : 'L'} ${x} ${y}`;
      })
      .join(' ');
  }

  function curveFill(points: { temp: number; duty: number }[]): string {
    const h = 40;
    const path = curvePath(points);
    return `${path} L 100 ${h} L 0 ${h} Z`;
  }

  async function doApply(preset: FanPresetDef) {
    applying = preset.id;
    try {
      await applyPresetToGroup(preset.id, selectedGroup);
      await loadConfig();
    } catch {
      // Swallow — the caller sees the unchanged indicator and the error
      // bubble is handled globally by the hardware-error toast.
    }
    applying = null;
  }
</script>

<div class="preset-strip-wrap">
  {#if groupNames.length > 1}
    <div class="preset-strip-header">
      <label class="group-select">
        <span>Apply to</span>
        <select bind:value={selectedGroup} disabled={applying !== null}>
          <option value={null}>All groups</option>
          {#each groupNames as name}
            <option value={name}>{name}</option>
          {/each}
        </select>
      </label>
    </div>
  {/if}

  {#if presets.length === 0}
    <div class="empty">Loading presets…</div>
  {:else}
    <div class="preset-rail">
      {#each presets as preset (preset.id)}
        {@const state = stateFor(preset)}
        <div class="preset-slot">
          <button
            class="preset-card"
            class:active={state === 'active'}
            class:modified={state === 'modified'}
            class:applying={applying === preset.id}
            onclick={() => doApply(preset)}
            disabled={applying !== null}
            title={preset.description}
          >
            <svg viewBox="0 0 100 40" class="curve-preview" preserveAspectRatio="none">
              <!-- vector-effect="non-scaling-stroke" keeps the stroke
                   width pixel-perfect regardless of anisotropic
                   stretching (SVG is 100×40 but the card is much
                   wider than that, so stroke-width: 2 gets smeared
                   horizontally without this attribute — looks jagged). -->
              <path
                d={curveFill(preset.points)}
                fill="rgba(96, 165, 250, 0.1)"
                vector-effect="non-scaling-stroke"
              />
              <path
                d={curvePath(preset.points)}
                fill="none"
                stroke="var(--accent)"
                stroke-width="2"
                stroke-linejoin="round"
                stroke-linecap="round"
                vector-effect="non-scaling-stroke"
              />
            </svg>
            <span class="preset-name">{preset.name}</span>
            <span class="preset-desc">{preset.description}</span>
            <span class="preset-meta">
              hy:{preset.hysteresis}° ↑{preset.ramp_up}%/s ↓{preset.ramp_down}%/s
            </span>
            {#if state === 'modified'}
              <span class="badge-modified">Modified</span>
            {/if}
          </button>
          {#if state === 'modified' && applying === null}
            <button
              class="revert-btn"
              onclick={() => doApply(preset)}
              title="Revert this group back to {preset.name}'s canonical values"
            >
              Revert
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .preset-strip-wrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .preset-strip-header {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 12px;
  }
  .group-select {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: var(--text-muted);
  }
  .group-select select {
    background: var(--bg-card-inset);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 6px;
    font-size: 11px;
    font-family: inherit;
    cursor: pointer;
  }
  .group-select select:hover {
    border-color: var(--border-bright);
  }
  .empty {
    padding: 10px 12px;
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-card-inset);
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .preset-rail {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 8px;
    align-items: stretch;
  }
  .preset-slot {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .preset-card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 7px 10px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s,
      transform 0.1s,
      box-shadow 0.15s;
    text-align: center;
    height: 100%;
  }
  .preset-card:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-bright);
  }
  .preset-card:active {
    transform: scale(0.98);
  }
  .preset-card.active {
    border-color: var(--accent-dim);
    background: var(--bg-card-active);
    box-shadow: var(--shadow-glow);
  }
  .preset-card.modified {
    border-color: var(--warning);
    border-style: dashed;
  }
  .preset-card.applying {
    animation: pulse 0.6s ease infinite;
  }
  .preset-card:disabled {
    cursor: default;
  }
  .curve-preview {
    width: 100%;
    height: 28px;
    display: block;
  }
  .preset-name {
    font-size: 12px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .preset-desc {
    font-size: 10px;
    color: var(--text-muted);
    line-height: 1.25;
    flex: 1;
    /* Cap to 2 lines — descriptions of varying length shouldn't
       make the whole rail grow to the tallest card. */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .preset-meta {
    font-size: 9px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    white-space: nowrap;
    margin-top: auto;
  }
  .badge-modified {
    position: absolute;
    top: 4px;
    right: 6px;
    padding: 1px 6px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--warning);
    background: rgba(251, 191, 36, 0.12);
    border: 1px solid var(--warning);
    border-radius: 4px;
  }
  .revert-btn {
    padding: 3px 8px;
    background: transparent;
    color: var(--warning);
    border: 1px solid var(--warning);
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s;
  }
  .revert-btn:hover {
    background: rgba(251, 191, 36, 0.1);
  }
</style>
