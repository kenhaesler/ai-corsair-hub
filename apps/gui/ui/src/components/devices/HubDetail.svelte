<script lang="ts">
  import type { HubSnapshot } from '../../lib/types';
  import { displayNameFromTree, shortHubSerial } from '../../lib/identity';
  import { configStore, loadConfig } from '../../lib/stores/config.svelte';
  import { renameDevice } from '../../lib/api';

  interface Props {
    hub: HubSnapshot;
  }

  let { hub }: Props = $props();

  /**
   * device_id of the row that's currently being renamed, or null when no
   * row is in edit mode. Separately tracked `editValue` holds the draft
   * string (so we don't write through on every keystroke).
   */
  let editingId = $state<string | null>(null);
  let editValue = $state('');
  let saving = $state(false);

  function startEdit(device_id: string) {
    editingId = device_id;
    const entry = configStore.config?.devices?.find((d) => d.device_id === device_id);
    editValue = entry?.name ?? '';
    // Defer focus to next tick so the input is mounted.
    queueMicrotask(() => {
      const el = document.querySelector<HTMLInputElement>(
        `input[data-rename-id="${device_id}"]`,
      );
      el?.focus();
      el?.select();
    });
  }

  function cancelEdit() {
    editingId = null;
    editValue = '';
  }

  async function commitEdit() {
    if (editingId == null) return;
    const device_id = editingId;
    const name = editValue.trim();
    const currentEntry = configStore.config?.devices?.find((d) => d.device_id === device_id);
    const currentName = currentEntry?.name ?? '';
    if (name === currentName) {
      cancelEdit();
      return;
    }
    saving = true;
    try {
      // Empty name clears the rename on the backend. The backend
      // persists atomically, so a crash between here and disk either
      // leaves the old name or the new — no torn state.
      await renameDevice(device_id, name);
      // Refresh config so the UI reflects the backend's authoritative
      // state (in particular so displayName() picks up the new label).
      await loadConfig();
    } catch (e) {
      console.error('rename_device failed', e);
    } finally {
      saving = false;
      editingId = null;
      editValue = '';
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitEdit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelEdit();
    }
  }
</script>

<div class="hub-detail">
  <div class="hub-header">
    <h4>iCUE LINK Hub — {shortHubSerial(hub.serial)}</h4>
    <span class="meta mono">FW {hub.firmware}</span>
  </div>
  <div class="serial mono">{hub.serial}</div>

  <table class="device-table">
    <thead>
      <tr>
        <th>Ch</th>
        <th>Type</th>
        <th>Name</th>
        <th>Model</th>
        <th>ID</th>
        <th>RPM</th>
      </tr>
    </thead>
    <tbody>
      {#each hub.devices as device}
        <tr>
          <td class="tabular-nums">{device.channel}</td>
          <td>{device.device_type}</td>
          <td class="name-cell">
            {#if editingId === device.device_id}
              <input
                class="rename-input"
                data-rename-id={device.device_id}
                type="text"
                bind:value={editValue}
                onblur={commitEdit}
                onkeydown={onKeydown}
                disabled={saving}
                placeholder="Friendly name"
                maxlength="64"
              />
            {:else}
              <button
                class="rename-trigger"
                onclick={() => startEdit(device.device_id)}
                title="Click to rename"
              >
                {displayNameFromTree(device.device_id, { config: configStore.config, hubs: [hub] })}
              </button>
            {/if}
          </td>
          <td class="tabular-nums">0x{device.model.toString(16).padStart(2, '0').toUpperCase()}</td>
          <td class="mono">{device.device_id.slice(0, 12)}...</td>
          <td class="tabular-nums">{device.rpm ?? '—'}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .hub-detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .hub-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .hub-header h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .meta {
    font-size: 11px;
    color: var(--text-muted);
  }
  .serial {
    font-size: 10px;
    color: var(--text-muted);
    word-break: break-all;
  }
  .device-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th {
    text-align: left;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 4px 8px;
    border-bottom: 1px solid rgba(255,255,255,0.03);
  }
  .name-cell {
    min-width: 140px;
    max-width: 220px;
  }
  .rename-trigger {
    background: transparent;
    border: 1px dashed transparent;
    border-radius: 4px;
    color: var(--text-primary);
    padding: 2px 6px;
    margin: -2px -6px;
    cursor: text;
    font-size: 12px;
    text-align: left;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename-trigger:hover {
    border-color: var(--border);
    background: rgba(255, 255, 255, 0.03);
  }
  .rename-input {
    width: 100%;
    background: var(--bg-elevated);
    border: 1px solid var(--accent-dim);
    border-radius: 4px;
    color: var(--text-primary);
    padding: 2px 6px;
    font-size: 12px;
    font-family: inherit;
  }
  .rename-input:focus {
    outline: none;
    border-color: var(--accent);
  }
</style>
