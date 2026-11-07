<!--
  Lighting presets (inventory 4.4): the 7 built-ins, a divider, then user
  presets (config.presets). Applying replaces every zone (device
  assignments kept per zone index when the existing zone has devices),
  selects zone 0 and saves. "Save Current" opens an inline name field
  (Enter saves) with Save / Cancel. Additions: rename (double-click or the
  pencil) and delete (with confirm) for user presets.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Pencil from 'lucide-svelte/icons/pencil';
  import Trash from 'lucide-svelte/icons/trash-2';
  import Bookmark from 'lucide-svelte/icons/bookmark-plus';
  import Button from '../../components/ui/Button.svelte';
  import IconButton from '../../components/ui/IconButton.svelte';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import ConfirmDialog from '../../components/ui/ConfirmDialog.svelte';
  import { rgb } from '../../lib/stores/rgb.svelte';
  import type { RgbPreset } from '../../lib/types';
  import { applyPreset, deletePreset, renamePreset, savePreset } from './state.svelte';
  import { BUILTIN_PRESETS, layerSummary, zoneSwatch } from './effects';

  const user = $derived(rgb.config?.presets ?? []);

  // ---- save current ----
  let saving = $state(false);
  let name = $state('');
  let saveInput = $state<HTMLInputElement>();
  async function openSave() {
    saving = true;
    name = '';
    await tick();
    saveInput?.focus();
  }
  function doSave() {
    if (savePreset(name)) {
      saving = false;
      name = '';
    }
  }

  // ---- rename / delete user presets ----
  let editing = $state<number | null>(null);
  let draft = $state('');
  let renameInput = $state<HTMLInputElement>();
  async function startRename(i: number) {
    editing = i;
    draft = user[i].name;
    await tick();
    renameInput?.focus();
    renameInput?.select();
  }
  function commitRename() {
    if (editing === null) return;
    const i = editing;
    editing = null;
    if (draft.trim()) renamePreset(i, draft);
  }
  let delIdx = $state<number | null>(null);
  let delOpen = $state(false);

  function meta(p: RgbPreset): string {
    const z = p.zones[0];
    if (!z) return 'Empty';
    const zones = p.zones.length > 1 ? `${p.zones.length} zones · ` : '';
    return `${zones}${layerSummary(z.layers)}`;
  }
</script>

<InsetGroup title="Presets" footer="Applying a preset replaces every zone. Device assignments are kept zone by zone.">
  {#snippet action()}
    {#if !saving}
      <Button variant="plain" size="sm" icon={Bookmark} onclick={openSave} disabled={!rgb.config || rgb.config.zones.length === 0}>Save Current</Button>
    {/if}
  {/snippet}

  {#if saving}
    <form
      class="save"
      onsubmit={(e) => {
        e.preventDefault();
        doSave();
      }}
    >
      <input
        bind:this={saveInput}
        class="field"
        type="text"
        placeholder="Preset name"
        aria-label="Preset name"
        bind:value={name}
        onkeydown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            saving = false;
          }
        }}
      />
      <Button size="sm" variant="primary" type="submit" disabled={!name.trim()}>Save</Button>
      <Button size="sm" variant="plain" onclick={() => (saving = false)}>Cancel</Button>
    </form>
  {/if}

  {#each BUILTIN_PRESETS as p (p.name)}
    <button type="button" class="prow" onclick={() => applyPreset(p)}>
      <span class="psw" style:background={zoneSwatch(p.zones[0])} aria-hidden="true"></span>
      <span class="ptext">
        <span class="pname">{p.name}</span>
        <span class="pmeta">{p.blurb}</span>
      </span>
      <span class="papply" aria-hidden="true">Apply</span>
    </button>
  {/each}

  <div class="divider">
    <span>Your presets</span>
  </div>

  {#if user.length === 0}
    <p class="none t-callout">Presets you save appear here.</p>
  {/if}
  {#each user as p, i (i)}
    <div class="urow">
      {#if editing === i}
        <span class="psw" style:background={p.zones[0] ? zoneSwatch(p.zones[0]) : '#111'} aria-hidden="true"></span>
        <input
          bind:this={renameInput}
          class="field rename"
          type="text"
          aria-label="Preset name"
          bind:value={draft}
          onblur={commitRename}
          onkeydown={(e) => {
            if (e.key === 'Enter') commitRename();
            else if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              editing = null;
            }
          }}
        />
      {:else}
        <button type="button" class="prow inner" onclick={() => applyPreset(p)} ondblclick={() => startRename(i)}>
          <span class="psw" style:background={p.zones[0] ? zoneSwatch(p.zones[0]) : '#111'} aria-hidden="true"></span>
          <span class="ptext">
            <span class="pname">{p.name}</span>
            <span class="pmeta">{meta(p)}</span>
          </span>
        </button>
        <span class="uacts">
          <IconButton icon={Pencil} size="sm" label="Rename preset" onclick={() => startRename(i)} />
          <IconButton
            icon={Trash}
            size="sm"
            label="Delete preset"
            onclick={() => {
              delIdx = i;
              delOpen = true;
            }}
          />
        </span>
      {/if}
    </div>
  {/each}
</InsetGroup>

<ConfirmDialog
  bind:open={delOpen}
  title={delIdx !== null && user[delIdx] ? `Delete “${user[delIdx].name}”?` : 'Delete preset?'}
  message="The preset is removed from your library. Your current zones are not changed."
  confirmLabel="Delete Preset"
  destructive
  onconfirm={() => {
    if (delIdx !== null) deletePreset(delIdx);
    delOpen = false;
  }}
  oncancel={() => (delOpen = false)}
/>

<style>
  .save {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.02);
  }
  .field {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.3);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    font-size: 13px;
    color: var(--t1);
  }
  .field::placeholder {
    color: var(--t3);
  }
  .field:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
  }
  .prow {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 52px;
    padding: 8px 14px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) ease-out;
  }
  .prow:hover {
    background: rgba(255, 255, 255, 0.035);
  }
  .prow:active {
    background: rgba(255, 255, 255, 0.06);
  }
  .prow:focus-visible {
    box-shadow: inset var(--focus-ring);
  }
  .psw {
    flex: none;
    width: 36px;
    height: 22px;
    border-radius: 7px;
    box-shadow:
      inset 0 0 0 1px rgb(0 0 0 / 0.35),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
  }
  .ptext {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .pname {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--t1);
  }
  .pmeta {
    font-size: 12px;
    line-height: 16px;
    color: var(--t3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .papply {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent-strong);
    opacity: 0;
    transform: translateX(-4px);
    transition:
      opacity var(--dur-fast) ease-out,
      transform var(--dur-med) var(--ease-out-expo);
  }
  .prow:hover .papply,
  .prow:focus-visible .papply {
    opacity: 1;
    transform: none;
  }
  .divider {
    padding: 14px 16px 6px;
    background: rgba(255, 255, 255, 0.015);
  }
  .divider span {
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--t2);
  }
  .none {
    padding: 12px 16px 14px;
    color: var(--t3);
  }
  .urow {
    position: relative;
    display: flex;
    align-items: center;
    min-height: 52px;
  }
  .urow:has(.rename) .psw {
    margin-left: 14px;
  }
  .urow .rename {
    margin: 0 12px;
  }
  .prow.inner {
    padding-right: 80px;
  }
  .uacts {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--dur-fast) ease-out;
  }
  .urow:hover .uacts,
  .urow:focus-within .uacts {
    opacity: 1;
  }
</style>
