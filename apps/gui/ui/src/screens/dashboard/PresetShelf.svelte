<!--
  Quick Presets shelf (inventory 1.4): preset library loaded once, "Apply
  to" select (only with >1 group; disabled while applying; resets to All if
  the group disappears), tiles with curve thumbnail, name, 2-line
  description (full text in tooltip), meta, active / modified / off states,
  applying pulse, Revert under modified tiles.
  Addition: in "All groups" mode each tile names the groups it is active on.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import RotateCcw from 'lucide-svelte/icons/rotate-ccw';
  import { applyPresetToGroup, getFanPresets } from '../../lib/api';
  import { configStore, mergeGroupsFromBackend } from '../../lib/stores/config.svelte';
  import { toasts } from '../../lib/stores/toasts.svelte';
  import { computePresetState, groupMatchesPreset, type PresetState } from '../../lib/presets';
  import type { FanPresetDef } from '../../lib/types';
  import CurveThumb from '../../lib/charts/CurveThumb.svelte';
  import Select from '../../components/ui/Select.svelte';
  import Badge from '../../components/ui/Badge.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import Skeleton from '../../components/ui/Skeleton.svelte';

  let presets = $state<FanPresetDef[]>([]);
  let applying = $state<string | null>(null);
  let selectedGroup = $state<string | null>(null);

  onMount(async () => {
    try {
      presets = await getFanPresets();
    } catch (e) {
      presets = [];
      toasts.error('Could not load fan presets', { detail: String(e) });
    }
  });

  const groupNames = $derived(configStore.config?.fan_groups?.map((g) => g.name) ?? []);
  $effect(() => {
    if (selectedGroup && !groupNames.includes(selectedGroup)) selectedGroup = null;
  });

  function stateFor(p: FanPresetDef): PresetState {
    return computePresetState(configStore.config, p, selectedGroup);
  }
  function activeOn(p: FanPresetDef): string[] {
    return (configStore.config?.fan_groups ?? [])
      .filter((g) => g.preset_applied === p.id && groupMatchesPreset(g, p))
      .map((g) => g.name);
  }

  async function doApply(p: FanPresetDef) {
    applying = p.id;
    try {
      await applyPresetToGroup(p.id, selectedGroup);
      await mergeGroupsFromBackend(selectedGroup ? [selectedGroup] : null);
      toasts.success(`${p.name} applied`, { detail: selectedGroup ?? 'All groups' });
    } catch (e) {
      toasts.error(`Could not apply ${p.name}`, { detail: String(e) });
    }
    applying = null;
  }
</script>

<section class="section" aria-labelledby="presets-h">
  <div class="section-head">
    <h2 id="presets-h" class="t-title2">Quick Presets</h2>
    {#if groupNames.length > 1}
      <Select
        label="Apply to"
        showLabel
        size="sm"
        disabled={applying !== null}
        bind:value={selectedGroup}
        options={[{ value: null, label: 'All groups' }, ...groupNames.map((n) => ({ value: n, label: n }))]}
      />
    {/if}
  </div>

  {#if presets.length === 0}
    <div class="loading" aria-busy="true">
      <span class="t-callout t3">Loading presets…</span>
      <div class="sk-row">
        {#each [0, 1, 2, 3] as i (i)}<div class="sk-tile"><Skeleton height="40px" /><Skeleton width="60%" /><Skeleton width="90%" height="11px" /></div>{/each}
      </div>
    </div>
  {:else}
    <div class="rail" role="list">
      {#each presets as p (p.id)}
        {@const st = stateFor(p)}
        {@const on = selectedGroup === null ? activeOn(p) : []}
        <div class="slot" role="listitem">
          <div class="tile {st}" class:applying={applying === p.id} class:busy={applying !== null}>
            <button
              type="button"
              class="hit"
              disabled={applying !== null}
              title={p.description}
              aria-label="Apply {p.name}{selectedGroup ? ` to ${selectedGroup}` : ' to all groups'}"
              aria-pressed={st === 'active'}
              onclick={() => doApply(p)}
            ></button>
            <div class="thumb">
              <CurveThumb
                points={p.points}
                height={60}
                color={st === 'active' ? 'var(--sc)' : st === 'modified' ? 'var(--warn)' : 'rgba(200,210,230,0.6)'}
              />
            </div>
            <div class="name-row">
              <span class="name">{p.name}</span>
              {#if applying === p.id}
                <Spinner size={12} />
              {:else if st === 'active'}
                <Badge tone="accent">Active</Badge>
              {:else if st === 'modified'}
                <Badge tone="warn">Modified</Badge>
              {/if}
            </div>
            <span class="desc">{p.description}</span>
            <span class="meta num">hy:{p.hysteresis}° ↑{p.ramp_up}%/s ↓{p.ramp_down}%/s</span>
            {#if (on.length > 0 && on.length < groupNames.length) || (st === 'modified' && applying === null)}
            <div class="foot">
              {#if on.length > 0 && on.length < groupNames.length}
                <span class="on-groups">On {on.join(', ')}</span>
              {/if}
              {#if st === 'modified' && applying === null}
                <button
                  type="button"
                  class="revert"
                  title="Revert this group back to {p.name}'s canonical values"
                  onclick={() => doApply(p)}
                >
                  <RotateCcw size={11} strokeWidth={2.5} />
                  Revert
                </button>
              {/if}
            </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .rail {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(176px, 1fr);
    gap: 12px;
    overflow-x: auto;
    padding: 4px 4px 10px;
    margin: -4px -4px 0;
    scroll-snap-type: x proximity;
  }
  @media (max-width: 1180px) {
    .rail {
      grid-auto-flow: row;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      grid-auto-columns: unset;
      overflow: visible;
    }
  }
  .slot {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    scroll-snap-align: start;
    min-width: 0;
  }
  .tile {
    flex: 1;
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    padding: 12px 12px 12px;
    border: 0;
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
    color: var(--t1);
    text-align: left;
    cursor: pointer;
    min-height: 196px;
    transition:
      background-color var(--dur-fast) ease-out,
      box-shadow 240ms ease-out,
      transform var(--dur-press) ease-out,
      opacity var(--dur-fast) ease-out;
  }
  .tile:not(.busy):hover {
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.2),
      0 24px 48px -32px rgb(0 0 0 / 0.9);
  }
  .tile:not(.busy):has(.hit:active) {
    transform: scale(0.98);
  }
  .tile.busy:not(.applying) {
    opacity: 0.55;
  }
  .tile:has(.hit:focus-visible) {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }
  .hit {
    position: absolute;
    inset: 0;
    z-index: 1;
    border: 0;
    border-radius: inherit;
    background: transparent;
    cursor: pointer;
  }
  .hit:disabled {
    cursor: default;
  }
  .hit:focus-visible {
    box-shadow: none;
  }
  .tile.active {
    background:
      linear-gradient(180deg, color-mix(in oklab, var(--sc) 14%, transparent), transparent 60%),
      var(--panel-bg);
    box-shadow:
      inset 0 0 0 1px color-mix(in oklab, var(--sc) 55%, transparent),
      inset 0 1px 0 color-mix(in oklab, var(--sc) 70%, #fff),
      0 16px 40px -20px color-mix(in oklab, var(--sc) 60%, transparent);
  }
  .tile.modified {
    box-shadow: none;
    outline: 1px dashed rgba(255, 159, 10, 0.7);
    outline-offset: -1px;
  }
  .tile.applying {
    animation: pulse 1s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      box-shadow:
        inset 0 0 0 1px color-mix(in oklab, var(--sc) 80%, transparent),
        0 0 0 4px color-mix(in oklab, var(--sc) 20%, transparent);
    }
  }
  .thumb {
    margin: 0 0 8px;
    padding: 8px 6px 4px;
    border-radius: 9px;
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.3), rgba(0, 0, 0, 0.16));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
  }
  .name-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    min-height: 20px;
  }
  .name {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.006em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .desc {
    font-size: 12px;
    line-height: 16px;
    color: var(--t2);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-height: 32px;
  }
  .foot {
    padding-top: 2px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    min-height: 22px;
  }
  .meta {
    margin-top: auto;
    padding-top: 6px;
    font: 500 10.5px var(--font-mono);
    letter-spacing: 0.04em;
    color: var(--t3);
    white-space: nowrap;
  }
  .on-groups {
    font-size: 11px;
    color: color-mix(in oklab, var(--sc) 60%, #fff);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .revert {
    position: relative;
    z-index: 2;
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 7px;
    margin-left: auto;
    margin-right: -6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--warn-text);
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color var(--dur-fast) ease-out;
  }
  .revert:hover {
    background: var(--warn-surface);
  }
  .revert:focus-visible {
    box-shadow: 0 0 0 3px rgba(255, 159, 10, 0.45);
  }
  .loading {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sk-row {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }
  .sk-tile {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: var(--r-row);
    background: var(--mat-2);
  }
  @media (prefers-reduced-motion: reduce) {
    .tile.applying {
      animation: none;
      box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--sc) 80%, transparent);
    }
  }
</style>
