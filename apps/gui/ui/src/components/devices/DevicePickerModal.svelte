<script lang="ts">
  import type { AppConfig, HubSnapshot, HubDeviceEntry } from '../../lib/types';
  import { displayNameFromTree, shortHubSerial, shortDeviceId } from '../../lib/identity';

  interface Props {
    /** Modal visibility. Parent binds and toggles. */
    open: boolean;
    /** Title shown at the top of the modal (e.g. the fan group name). */
    title: string;
    /** Devices currently selected in the bound entity (group or zone). */
    selected: string[];
    /** All hubs and their enumerated devices. */
    availableHubs: HubSnapshot[];
    /** Full config — used to surface user-set friendly names in the list. */
    config: AppConfig | null;
    /** Called with the final device_ids when the user clicks Save. */
    onsave: (deviceIds: string[]) => void;
    /** Called when the user dismisses without saving. */
    oncancel: () => void;
  }

  let {
    open = $bindable(),
    title,
    selected,
    availableHubs,
    config,
    onsave,
    oncancel,
  }: Props = $props();

  /**
   * Local editable copy of the selection. We don't mutate `selected`
   * directly — the parent stays the source of truth until the user
   * saves. Reset whenever the modal is re-opened.
   */
  let draft = $state<string[]>([]);

  $effect(() => {
    if (open) {
      draft = [...selected];
    }
  });

  interface PickerRow {
    device_id: string;
    hub_serial: string;
    channel: number;
    label: string;
    device_type: string;
    enumerated: boolean;
  }

  /**
   * All rows for the picker. Combines currently-enumerated devices with
   * any device_ids that are in the draft but not currently online (orphans
   * — shown with an "(offline)" suffix so the user understands why the
   * label is a bare id prefix).
   */
  const rows = $derived.by<PickerRow[]>(() => {
    const enumerated: PickerRow[] = [];
    const seen = new Set<string>();
    for (const hub of availableHubs) {
      for (const dev of hub.devices as HubDeviceEntry[]) {
        if (!dev.device_id) continue;
        seen.add(dev.device_id);
        const name = displayNameFromTree(dev.device_id, { config, hubs: availableHubs });
        enumerated.push({
          device_id: dev.device_id,
          hub_serial: hub.serial,
          channel: dev.channel,
          label: `${dev.device_type} · ${name}`,
          device_type: dev.device_type,
          enumerated: true,
        });
      }
    }
    const orphans: PickerRow[] = [];
    for (const id of draft) {
      if (!seen.has(id)) {
        orphans.push({
          device_id: id,
          hub_serial: '',
          channel: 0,
          label: `Device ${shortDeviceId(id)} (offline)`,
          device_type: 'unknown',
          enumerated: false,
        });
      }
    }
    return [...enumerated, ...orphans];
  });

  /**
   * Group rows by hub for visual presentation. Orphans collect into a
   * single pseudo-group so the user sees them separately.
   */
  const grouped = $derived.by(() => {
    const groups: { label: string; rows: PickerRow[] }[] = [];
    for (const [i, hub] of availableHubs.entries()) {
      const hubRows = rows.filter((r) => r.enumerated && r.hub_serial === hub.serial);
      if (hubRows.length > 0) {
        groups.push({
          label: `Hub ${i + 1} · ${shortHubSerial(hub.serial)}`,
          rows: hubRows,
        });
      }
    }
    const orphanRows = rows.filter((r) => !r.enumerated);
    if (orphanRows.length > 0) {
      groups.push({ label: 'Offline (referenced but not enumerated)', rows: orphanRows });
    }
    return groups;
  });

  function isSelected(device_id: string): boolean {
    return draft.includes(device_id);
  }

  function toggle(device_id: string) {
    if (isSelected(device_id)) {
      draft = draft.filter((d) => d !== device_id);
    } else {
      draft = [...draft, device_id];
    }
  }

  function selectAll() {
    // All currently-enumerated device_ids plus any orphans already in
    // the draft (we don't add new orphans — the user can only expand to
    // what's online).
    const ids = new Set<string>(draft);
    for (const hub of availableHubs) {
      for (const dev of hub.devices) {
        if (dev.device_id) ids.add(dev.device_id);
      }
    }
    draft = [...ids];
  }

  function clearAll() {
    draft = [];
  }

  function save() {
    onsave([...draft]);
  }

  function cancel() {
    oncancel();
  }

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      cancel();
    }
  }

  function handleBackdropKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Enter' || e.key === ' ') {
      cancel();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      cancel();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div
    class="backdrop"
    role="button"
    tabindex="-1"
    onclick={handleBackdrop}
    onkeydown={handleBackdropKey}
    aria-label="Close picker"
  >
    <div class="modal" role="dialog" aria-modal="true" aria-label={title}>
      <header class="header">
        <h3>{title}</h3>
        <button class="close" onclick={cancel} aria-label="Close">&times;</button>
      </header>

      <div class="toolbar">
        <span class="count">{draft.length} selected</span>
        <div class="toolbar-actions">
          <button class="small-btn" onclick={selectAll}>All online</button>
          <button class="small-btn" onclick={clearAll}>Clear</button>
        </div>
      </div>

      <div class="body">
        {#if grouped.length === 0}
          <p class="empty">No hubs currently enumerated. Reconnect the iCUE LINK hub to populate the list.</p>
        {:else}
          {#each grouped as group}
            <section class="hub-group">
              <span class="hub-label">{group.label}</span>
              {#each group.rows as row}
                <label class="device-row" class:offline={!row.enumerated}>
                  <input
                    type="checkbox"
                    checked={isSelected(row.device_id)}
                    onchange={() => toggle(row.device_id)}
                  />
                  <span class="label">{row.label}</span>
                  {#if row.enumerated}
                    <span class="loc">Ch {row.channel}</span>
                  {/if}
                </label>
              {/each}
            </section>
          {/each}
        {/if}
      </div>

      <footer class="footer">
        <button class="cancel" onclick={cancel}>Cancel</button>
        <button class="primary" onclick={save}>Save ({draft.length})</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 24px;
    border: none;
    cursor: default;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
    width: min(520px, 100%);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }
  .header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    padding: 0 4px;
  }
  .close:hover {
    color: var(--text-primary);
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 18px;
    border-bottom: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.02);
  }
  .count {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .toolbar-actions {
    display: flex;
    gap: 6px;
  }
  .small-btn {
    font-size: 10px;
    padding: 3px 8px;
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
  }
  .small-btn:hover {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 12px 18px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }
  .body::-webkit-scrollbar {
    width: 5px;
  }
  .body::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
  }
  .hub-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 12px;
  }
  .hub-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    padding: 4px 0;
  }
  .device-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .device-row:hover {
    background: var(--bg-card);
    color: var(--text-primary);
  }
  .device-row.offline {
    color: var(--text-muted);
    font-style: italic;
  }
  .device-row input[type="checkbox"] {
    accent-color: var(--accent);
  }
  .device-row .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .device-row .loc {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
  .empty {
    color: var(--text-muted);
    font-size: 12px;
    text-align: center;
    padding: 24px 12px;
  }
  .footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.02);
  }
  .cancel {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-secondary);
    padding: 6px 14px;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 12px;
  }
  .cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }
  .primary {
    background: var(--accent);
    color: var(--bg-base);
    border: 1px solid var(--accent);
    padding: 6px 14px;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
  }
  .primary:hover {
    filter: brightness(1.1);
  }
</style>
