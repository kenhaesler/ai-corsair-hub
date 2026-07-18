<script lang="ts">
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { getDevices, listAvailableSensors } from '../../lib/api';
  import type { FanGroupConfig, HubSnapshot } from '../../lib/types';
  import FanGroupCard from './FanGroupCard.svelte';
  import NewGroupDialog from './NewGroupDialog.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import { onMount } from 'svelte';

  /** Latest enumerated hubs — used by the device picker modal. */
  let availableHubs = $state<HubSnapshot[]>([]);
  /**
   * Sensors currently initialized in the control loop — powers the
   * per-group TempSourceEditor chip list. Fetched once on mount; the
   * set only changes when LHM / NVML / PSU come online or drop, which
   * is rare relative to the UI lifetime.
   */
  let availableSensors = $state<string[]>([]);

  onMount(async () => {
    loadConfig();
    try {
      const tree = await getDevices();
      availableHubs = tree.hubs;
    } catch {
      // Non-fatal: the picker will show "no hubs enumerated" until a
      // subsequent refresh succeeds.
    }
    try {
      availableSensors = await listAvailableSensors();
    } catch {
      // Non-fatal: fall back to an empty list — TempSourceEditor will
      // show only sensors the config already references.
    }
  });

  let selectedIndex = $state(0);
  let newGroupOpen = $state(false);
  let deleteTargetIndex = $state<number | null>(null);

  /**
   * Per-group validity (keyed by index). A group marked `false` has a
   * hard curve error — any such group disables the Save button.
   * The FanGroupCard's onvalidate callback drives this map.
   */
  let groupErrors = $state<Record<number, boolean>>({});

  const anyGroupHasError = $derived(Object.values(groupErrors).some((e) => e));

  function updateGroup(index: number, group: FanGroupConfig) {
    if (!configStore.config) return;
    const groups = [...configStore.config.fan_groups];
    groups[index] = group;
    configStore.config = { ...configStore.config, fan_groups: groups };
  }

  function addGroup(group: FanGroupConfig) {
    if (!configStore.config) return;
    const groups = [...configStore.config.fan_groups, group];
    configStore.config = { ...configStore.config, fan_groups: groups };
    selectedIndex = groups.length - 1;
    newGroupOpen = false;
    // Persist immediately — no value in leaving the group un-saved; the
    // user's intent (new group creation) already is a commit action.
    saveCurrentConfig();
  }

  function renameGroup(index: number, newName: string) {
    if (!configStore.config) return;
    // Guard against client-side collisions (server validator also
    // enforces this, but we fail fast to avoid a round-trip).
    const existing = configStore.config.fan_groups.some(
      (g, i) => i !== index && g.name === newName,
    );
    if (existing) {
      configStore.error = `A fan group named "${newName}" already exists.`;
      return;
    }
    const groups = [...configStore.config.fan_groups];
    groups[index] = { ...groups[index], name: newName };
    configStore.config = { ...configStore.config, fan_groups: groups };
    saveCurrentConfig();
  }

  function beginDelete(index: number) {
    deleteTargetIndex = index;
  }
  function confirmDelete() {
    if (deleteTargetIndex == null || !configStore.config) {
      deleteTargetIndex = null;
      return;
    }
    const idx = deleteTargetIndex;
    const groups = configStore.config.fan_groups.filter((_, i) => i !== idx);
    configStore.config = { ...configStore.config, fan_groups: groups };
    // Clear any error state for the removed index so the Save gate
    // doesn't latch on a stale key.
    const nextErrors: Record<number, boolean> = {};
    for (const [k, v] of Object.entries(groupErrors)) {
      const n = Number(k);
      if (n < idx) nextErrors[n] = v;
      else if (n > idx) nextErrors[n - 1] = v;
    }
    groupErrors = nextErrors;
    // Keep selection in-range.
    if (selectedIndex >= groups.length) selectedIndex = Math.max(0, groups.length - 1);
    deleteTargetIndex = null;
    saveCurrentConfig();
  }
  function cancelDelete() {
    deleteTargetIndex = null;
  }

  // Get current CPU temp for curve display
  const cpuTemp = $derived(
    sensors.snapshot?.temperatures.find((t) => t.source === 'cpu')?.celsius,
  );

  // Live duty for each group
  function groupDuty(name: string): number | undefined {
    return sensors.snapshot?.group_duties.find((g) => g.name === name)?.duty_percent;
  }

  // Clamp selected index
  $effect(() => {
    if (
      configStore.config &&
      selectedIndex >= configStore.config.fan_groups.length
    ) {
      selectedIndex = Math.max(0, configStore.config.fan_groups.length - 1);
    }
  });

  const deleteTargetName = $derived(
    deleteTargetIndex != null
      ? configStore.config?.fan_groups[deleteTargetIndex]?.name ?? ''
      : '',
  );
</script>

<div class="fan-groups">
  {#if configStore.error}
    <div class="banner error" style="margin: 16px 16px 0">{configStore.error}</div>
  {/if}

  {#if configStore.config}
    {#if configStore.config.fan_groups.length === 0}
      <div class="empty-state card" style="margin: 16px">
        <p>No fan groups configured.</p>
        <button class="primary" onclick={() => (newGroupOpen = true)}>+ New Group</button>
      </div>
    {:else}
      <div class="master-detail">
        <!-- Sidebar -->
        <nav class="sidebar">
          <button
            class="sidebar-add"
            onclick={() => (newGroupOpen = true)}
            title="Create a new fan group"
          >
            + New Group
          </button>
          {#each configStore.config.fan_groups as group, i}
            {@const duty = groupDuty(group.name)}
            <button
              class="sidebar-item"
              class:active={selectedIndex === i}
              onclick={() => {
                selectedIndex = i;
              }}
            >
              <span
                class="dot"
                style="background: {duty !== undefined ? 'var(--success)' : 'var(--text-muted)'}"
              ></span>
              <div class="sidebar-info">
                <span class="sidebar-name">{group.name}</span>
                <span class="sidebar-mode">{group.mode.type}</span>
              </div>
              {#if duty !== undefined}
                <span class="sidebar-duty tabular-nums">{Math.round(duty)}%</span>
              {/if}
            </button>
          {/each}
        </nav>

        <!-- Detail panel -->
        <div class="detail">
          {#if configStore.config.fan_groups[selectedIndex]}
            <FanGroupCard
              group={configStore.config.fan_groups[selectedIndex]}
              currentTemp={cpuTemp}
              onchange={(g) => updateGroup(selectedIndex, g)}
              onvalidate={(hasError) => {
                // IMPORTANT: only reassign when the value actually
                // differs. CurveEditor's $effect fires on every parent
                // re-render (the inline onvalidate arrow is a new
                // reference each time), so without this guard we'd
                // spread into a fresh object each tick and loop the
                // tree re-rendering at render speed.
                if (groupErrors[selectedIndex] !== hasError) {
                  groupErrors = { ...groupErrors, [selectedIndex]: hasError };
                }
              }}
              onrename={(newName) => renameGroup(selectedIndex, newName)}
              ondelete={() => beginDelete(selectedIndex)}
              {availableHubs}
              {availableSensors}
              expanded
            />
          {/if}

          <div class="actions">
            <button
              class="primary"
              onclick={saveCurrentConfig}
              disabled={configStore.saving || anyGroupHasError}
              title={anyGroupHasError ? 'Fix curve errors before saving' : undefined}
            >
              {configStore.saving ? 'Saving...' : 'Save Configuration'}
            </button>
          </div>
        </div>
      </div>
    {/if}
  {:else}
    <div class="loading">Loading configuration...</div>
  {/if}

  <NewGroupDialog
    bind:open={newGroupOpen}
    config={configStore.config}
    {availableHubs}
    onsave={addGroup}
    oncancel={() => (newGroupOpen = false)}
  />

  <ConfirmDialog
    open={deleteTargetIndex != null}
    title="Delete fan group?"
    message={`Remove "${deleteTargetName}" from the configuration? Fans previously in this group will go to their failsafe duty until another group adopts them.`}
    confirmLabel="Delete"
    destructive
    onconfirm={confirmDelete}
    oncancel={cancelDelete}
  />
</div>

<style>
  .fan-groups {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .master-detail {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .sidebar {
    width: 200px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 8px;
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .sidebar-add {
    padding: 8px 10px;
    margin-bottom: 6px;
    background: transparent;
    color: var(--accent);
    border: 1px dashed var(--accent-dim);
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
  }
  .sidebar-add:hover {
    background: rgba(96, 165, 250, 0.08);
    border-style: solid;
  }
  .sidebar-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
    text-align: left;
    transition: background 0.15s, border-color 0.15s;
  }
  .sidebar-item:hover {
    background: var(--bg-card-hover);
    border-color: transparent;
  }
  .sidebar-item.active {
    background: var(--bg-card-active);
    border-color: var(--accent-dim);
    box-shadow: var(--shadow-glow);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .sidebar-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .sidebar-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sidebar-mode {
    font-size: 9px;
    color: var(--text-muted);
    text-transform: capitalize;
  }
  .sidebar-duty {
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
    flex-shrink: 0;
  }
  .detail {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 16px;
    overflow-y: auto;
    min-width: 0;
  }
  .actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    flex-shrink: 0;
  }
  .empty-state {
    text-align: center;
    padding: 40px;
    color: var(--text-secondary);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }
  .empty-state .primary {
    padding: 8px 18px;
    background: var(--accent);
    color: var(--bg-base);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
  }
  .empty-state .primary:hover {
    filter: brightness(1.1);
  }
  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-secondary);
  }
</style>
