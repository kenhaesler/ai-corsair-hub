<!--
  Fan Control (inventory 2.1). Master-detail as a source list on top:
  group capsules, then the selected group's editor as inset sections.
  Toolbar trailing slot: unsaved-changes dot, Revert, Save Configuration.
  Create / rename / delete auto-save; other edits mark dirty until Save.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Fan from 'lucide-svelte/icons/fan';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import ToolbarActions from '../../components/shell/ToolbarActions.svelte';
  import Button from '../../components/ui/Button.svelte';
  import Banner from '../../components/ui/Banner.svelte';
  import EmptyState from '../../components/ui/EmptyState.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import Skeleton from '../../components/ui/Skeleton.svelte';
  import ConfirmDialog from '../../components/ui/ConfirmDialog.svelte';
  import GroupCapsules from './GroupCapsules.svelte';
  import GroupEditor from './GroupEditor.svelte';
  import NewGroupDialog from './NewGroupDialog.svelte';
  import { configStore, loadConfig, revertConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { toasts } from '../../lib/stores/toasts.svelte';
  import { getDevices, getFanPresets, listAvailableSensors } from '../../lib/api';
  import { groupColor } from '../../lib/format';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';
  import { modeHasCurveError } from './fanModes';
  import { groupMatchesPreset } from '../../lib/presets';
  import type { FanGroupConfig, FanPresetDef, HubSnapshot } from '../../lib/types';

  let availableHubs = $state<HubSnapshot[]>([]);
  let availableSensors = $state<string[]>([]);
  let presets = $state<FanPresetDef[]>([]);

  async function refreshHubs() {
    try {
      availableHubs = (await getDevices()).hubs;
    } catch (e) {
      // the picker shows its empty state
      toasts.error('Could not list devices', { detail: String(e) });
    }
  }

  onMount(() => {
    loadConfig();
    refreshHubs();
    listAvailableSensors()
      .then((s) => (availableSensors = s))
      .catch((e) => toasts.error('Could not list sensors', { detail: String(e) }));
    getFanPresets()
      .then((p) => (presets = p))
      .catch((e) => toasts.error('Could not load fan presets', { detail: String(e) }));
  });

  // re-enumerate when a hub drops or comes back
  $effect(() => {
    void sensors.hubStatus;
    refreshHubs();
  });

  const groups = $derived(configStore.config?.fan_groups ?? []);
  /** index of the selected group; -1 = "All fans" (every group at once) */
  let selected = $state(0);
  const allMode = $derived(selected < 0 && groups.length > 1);
  $effect(() => {
    ui.fanSelectedGroup = allMode ? null : (groups[selected]?.name ?? null);
  });
  $effect(() => {
    if (selected >= groups.length) selected = Math.max(0, groups.length - 1);
    if (selected < 0 && groups.length <= 1) selected = 0;
  });

  // ---- "All fans": a virtual group whose settings are written to the groups
  const groupsDiffer = $derived.by(() => {
    const curves = groups.filter((g) => g.mode.type === 'curve');
    const pool = curves.length ? curves : groups;
    return new Set(pool.map((g) => JSON.stringify(g.mode))).size > 1;
  });
  const mixedModes = $derived(new Set(groups.map((g) => g.mode.type)).size > 1);
  /** The All editor starts from a curve group when there is one (presets are curves). */
  const allLead = $derived(groups.find((g) => g.mode.type === 'curve') ?? groups[0]);
  const allGroup = $derived.by((): FanGroupConfig | null => {
    if (!allLead) return null;
    return {
      name: 'All fans',
      channels: [],
      hub_serial: null,
      device_ids: [...new Set(groups.flatMap((g) => g.device_ids ?? []))],
      mode: structuredClone($state.snapshot(allLead.mode)),
      preset_applied: groups.every((g) => g.preset_applied === allLead.preset_applied) ? allLead.preset_applied : null,
    };
  });
  const describeMode = (g: FanGroupConfig) =>
    g.mode.type === 'fixed' ? `Fixed ${g.mode.duty_percent}%` : g.mode.type === 'pid' ? `PID · ${g.mode.target_temp}°` : 'Curve';
  /** The preset every curve group shares, or null. */
  const sharedPreset = $derived.by(() => {
    const curves = groups.filter((g) => g.mode.type === 'curve');
    if (!curves.length) return null;
    const p = presets.find((p) => curves.every((g) => groupMatchesPreset(g, p)));
    return p?.id ?? null;
  });
  /** Write the whole mode (a conversion) to the groups in `names`. */
  function writeAll(g: FanGroupConfig, names: string[]) {
    if (!configStore.config) return;
    configStore.config.fan_groups = configStore.config.fan_groups.map((x) =>
      names.includes(x.name)
        ? { ...x, mode: structuredClone($state.snapshot(g.mode)), preset_applied: g.preset_applied ?? null }
        : x,
    );
  }
  /**
   * Write only the fields that changed (relative to what the editor showed)
   * to the groups in `names`, so groups with their own curve keep what the
   * user did not touch. A points change carries the preset tag with it.
   */
  const PRESET_FIELDS = ['points', 'hysteresis', 'ramp_rate', 'ramp_down_rate', 'temp_source'];
  function writeChanged(g: FanGroupConfig, names: string[], force: string[] = []) {
    if (!configStore.config || !allGroup) return;
    const before = allGroup.mode as unknown as Record<string, unknown>;
    const after = $state.snapshot(g.mode) as unknown as Record<string, unknown>;
    const changed = [
      ...new Set([
        ...force.filter((k) => k in after),
        ...Object.keys(after).filter((k) => k !== 'type' && JSON.stringify(after[k]) !== JSON.stringify(before[k])),
      ]),
    ];
    const tagChanged = force.length > 0 || g.preset_applied !== allGroup.preset_applied;
    if (!changed.length && !tagChanged) return;
    configStore.config.fan_groups = configStore.config.fan_groups.map((x) => {
      if (!names.includes(x.name)) return x;
      const mode = { ...x.mode } as unknown as Record<string, unknown>;
      for (const k of changed) mode[k] = structuredClone(after[k]);
      return {
        ...x,
        mode: mode as unknown as FanGroupConfig['mode'],
        preset_applied: tagChanged || changed.includes('points') ? (g.preset_applied ?? null) : x.preset_applied,
      };
    });
  }
  let convertOpen = $state(false);
  let convertPending = $state<FanGroupConfig | null>(null);
  let convertList = $state('');
  let controlKey = $state(0);
  let editorApi: { checkpoint: (key: string) => void } | null = null;
  /**
   * Edits (curve points, presets, PID fields, noise, sensors) reach only the
   * groups already in that mode. A deliberate mode switch converts the rest
   * too, after naming them (the pump is the one that matters).
   */
  function updateAll(g: FanGroupConfig, intent: 'mode' | 'edit' | 'preset' = 'edit') {
    const same = groups.filter((x) => x.mode.type === g.mode.type).map((x) => x.name);
    const others = groups.filter((x) => x.mode.type !== g.mode.type);
    if (intent === 'mode') {
      if (!others.length) return;
      convertPending = g;
      convertList = others.map((x) => `${x.name} (${describeMode(x)})`).join(', ');
      convertOpen = true;
      return;
    }
    // a preset is the whole curve recipe: every curve group gets all of it, not just the fields that differ
    writeChanged(g, same, intent === 'preset' ? PRESET_FIELDS : []);
  }
  function confirmConvert() {
    const g = convertPending;
    convertOpen = false;
    convertPending = null;
    if (!g) return;
    editorApi?.checkpoint(`convert-${Date.now()}`);
    // only the groups in another mode are converted; groups already in this mode keep their settings
    writeAll(g, groups.filter((x) => x.mode.type !== g.mode.type).map((x) => x.name));
  }
  function cancelConvert() {
    convertOpen = false;
    convertPending = null;
    controlKey++;
    // the Mode control was rebuilt; give focus back to it
    tick().then(() => document.querySelector<HTMLElement>('#fan-group-editor .mode [role=radio]')?.focus({ preventScroll: true }));
  }
  const snapshotAll = () => JSON.stringify($state.snapshot(configStore.config?.fan_groups ?? []));
  function restoreAll(s: string) {
    if (!configStore.config) return;
    configStore.config.fan_groups = JSON.parse(s) as FanGroupConfig[];
  }

  // Dashboard "Edit curve" → select that group, then clear the request.
  $effect(() => {
    const want = ui.fanFocusGroup;
    if (!want || !configStore.config) return;
    const i = configStore.config.fan_groups.findIndex((g) => g.name === want);
    if (i >= 0) selected = i;
    ui.fanFocusGroup = null;
  });

  // Per-group validity map (gates Save) and unsaved-edit markers.
  const validity = $derived(Object.fromEntries(groups.map((g) => [g.name, !modeHasCurveError(g.mode)])) as Record<string, boolean>);
  const invalid = $derived(new Set(Object.keys(validity).filter((k) => !validity[k])));
  const anyError = $derived(invalid.size > 0);
  const edited = $derived.by(() => {
    const out = new Set<string>();
    if (!configStore.baseline || !configStore.dirty) return out;
    try {
      const base = (JSON.parse(configStore.baseline).fan_groups ?? []) as FanGroupConfig[];
      for (const g of groups) {
        const b = base.find((x) => x.name === g.name);
        if (!b || JSON.stringify(b) !== JSON.stringify(g)) out.add(g.name);
      }
    } catch {
      /* ignore */
    }
    return out;
  });

  function updateGroup(i: number, g: FanGroupConfig) {
    if (!configStore.config) return;
    configStore.config.fan_groups[i] = g;
  }

  /** primary (loud) only while there is something to save */
  const live = $derived(configStore.dirty || configStore.saving);
  const saveTip = $derived(
    anyError ? 'Fix curve errors before saving' : configStore.dirty ? 'Save Configuration (Ctrl+S)' : 'No unsaved changes',
  );
  async function save() {
    if (anyError || configStore.saving || !configStore.dirty) return;
    await saveCurrentConfig();
  }

  // ------------------------------------------------------------ add / rename / delete
  let newOpen = $state(false);
  function addGroup(g: FanGroupConfig) {
    if (!configStore.config) return;
    configStore.config.fan_groups = [...configStore.config.fan_groups, g];
    selected = configStore.config.fan_groups.length - 1;
    newOpen = false;
    saveCurrentConfig({ successMessage: `Created “${g.name}”` });
  }

  function renameGroup(i: number, name: string) {
    if (!configStore.config) return;
    if (configStore.config.fan_groups.some((g, j) => j !== i && g.name === name)) {
      configStore.error = `A fan group named "${name}" already exists.`;
      return;
    }
    configStore.config.fan_groups[i] = { ...configStore.config.fan_groups[i], name };
    saveCurrentConfig({ successMessage: `Renamed to “${name}”` });
  }

  let deleteIndex = $state<number | null>(null);
  let deleteOpen = $state(false);
  let deleteName = $state('');
  function beginDelete(i: number) {
    deleteIndex = i;
    deleteName = groups[i]?.name ?? '';
    deleteOpen = true;
  }
  function confirmDelete() {
    const i = deleteIndex;
    deleteOpen = false;
    deleteIndex = null;
    if (i === null || !configStore.config) return;
    const name = configStore.config.fan_groups[i]?.name;
    configStore.config.fan_groups = configStore.config.fan_groups.filter((_, j) => j !== i);
    if (selected >= configStore.config.fan_groups.length) selected = Math.max(0, configStore.config.fan_groups.length - 1);
    saveCurrentConfig({ successMessage: `Deleted “${name}”` });
  }
  function cancelDelete() {
    deleteOpen = false;
    deleteIndex = null;
  }

  // ------------------------------------------------------------ editor entrance spring
  const enter = createSpring(1, springs.sheet);
  let lastSel: number | null = null;
  $effect(() => {
    const s = selected;
    if (lastSel !== null && s !== lastSel) {
      enter.set(0, { instant: true });
      springTo(enter, 1);
    }
    lastSel = s;
  });

  // ------------------------------------------------------------ subtitle
  // Subtitle carries only what nothing else on the page shows: how many
  // enumerated devices belong to a group (live duty lives in the capsules).
  const subtitle = $derived.by(() => {
    if (!configStore.config) return 'Loading';
    if (groups.length === 0) return 'No groups yet';
    const all = new Set(availableHubs.flatMap((h) => h.devices.map((d) => d.device_id).filter(Boolean)));
    if (all.size === 0) return 'Waiting for hubs to enumerate';
    const assigned = new Set(groups.flatMap((g) => g.device_ids ?? []).filter((id) => all.has(id)));
    const free = all.size - assigned.size;
    return free === 0
      ? `All ${all.size} devices are in a group`
      : `${assigned.size} of ${all.size} devices in a group · ${free} unassigned`;
  });

  function onWindowKey(e: KeyboardEvent) {
    if (ui.activeTab !== 'fans') return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      save();
    }
  }
  const panelId = 'fan-group-editor';
</script>

<svelte:window onkeydown={onWindowKey} />

<ToolbarActions>
  <div class="tb">
    {#if configStore.dirty}
      <span class="unsaved" role="status" title="Unsaved changes"><span class="udot" aria-hidden="true"></span><span class="utxt">Unsaved changes</span></span>
      <Button variant="plain" size="sm" onclick={revertConfig} disabled={configStore.saving}>Revert</Button>
    {/if}
    <span class="save-wrap" title={saveTip}>
      <Button
        variant={live ? 'primary' : 'secondary'}
        size="sm"
        loading={configStore.saving}
        disabled={configStore.saving || anyError || !configStore.config || !configStore.dirty}
        onclick={save}
      >
        {#if configStore.saving}Saving...{:else}<span class="lbl-long">Save Configuration</span><span class="lbl-short">Save</span>{/if}
      </Button>
    </span>
  </div>
</ToolbarActions>

<LargeTitlePage title="Fan Control" {subtitle} maxWidth={1440}>
  {#if configStore.error}
    <div class="banner">
      <Banner tone="error" ondismiss={() => (configStore.error = null)}>{configStore.error}</Banner>
    </div>
  {/if}

  {#if !configStore.config}
    <div class="loading" role="status">
      <div class="loading-row"><Spinner size={16} /><span>Loading configuration...</span></div>
      <div class="sk-caps">
        {#each [0, 1, 2] as k (k)}<Skeleton height="86px" radius="14px" />{/each}
      </div>
      <Skeleton height="420px" radius="14px" />
    </div>
  {:else if groups.length === 0}
    <div class="empty">
      <EmptyState
        icon={Fan}
        title="No fan groups configured."
        message="A group ties fans to a curve, a fixed duty or a PID loop. Start with your radiator fans."
      >
        {#snippet action()}
          <Button variant="primary" onclick={() => (newOpen = true)}>+ New Group</Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else}
    <GroupCapsules
      {groups}
      {selected}
      {edited}
      {invalid}
      {panelId}
      onselect={(i) => (selected = i)}
      onnew={() => (newOpen = true)}
      onrename={renameGroup}
    />
    {#if allMode && allGroup}
      <div
        id={panelId}
        role="tabpanel"
        aria-label="All fans"
        class="panel"
        style:opacity={0.4 + 0.6 * enter.current}
        style:transform="translateY({(1 - enter.current) * 10}px)"
      >
        {#key 'all'}
          <GroupEditor
            group={allGroup}
            color="var(--accent-strong)"
            scope="all"
            groupCount={groups.length}
            {groupsDiffer}
            {mixedModes}
            {sharedPreset}
            leadName={allLead?.name}
            {controlKey}
            onready={(api) => (editorApi = api)}
            snapshot={snapshotAll}
            restore={restoreAll}
            {availableHubs}
            {availableSensors}
            {presets}
            onchange={updateAll}
            ondelete={() => {}}
          />
        {/key}
      </div>
    {:else if groups[selected]}
      <div
        id={panelId}
        role="tabpanel"
        aria-label={groups[selected].name}
        class="panel"
        style:opacity={0.4 + 0.6 * enter.current}
        style:transform="translateY({(1 - enter.current) * 10}px)"
      >
        {#key groups[selected].name}
          <GroupEditor
            group={groups[selected]}
            color={groupColor(selected)}
            {availableHubs}
            {availableSensors}
            {presets}
            onchange={(g) => updateGroup(selected, g)}
            ondelete={() => beginDelete(selected)}
          />
        {/key}
      </div>
    {/if}
  {/if}
</LargeTitlePage>

<NewGroupDialog
  bind:open={newOpen}
  config={configStore.config}
  {availableHubs}
  onsave={addGroup}
  oncancel={() => (newOpen = false)}
/>

<ConfirmDialog
  bind:open={convertOpen}
  title="Switch every group to {convertPending ? ({ fixed: 'Fixed', curve: 'Curve', pid: 'PID' } as const)[convertPending.mode.type] : ''} mode?"
  message={`These groups use a different mode and will be converted: ${convertList}. Groups already in this mode keep their own settings.`}
  confirmLabel="Convert"
  focusCancel
  onconfirm={confirmConvert}
  oncancel={cancelConvert}
/>

<ConfirmDialog
  bind:open={deleteOpen}
  title="Delete fan group?"
  message={`Remove "${deleteName}" from the configuration? Fans previously in this group will go to their failsafe duty until another group adopts them.`}
  confirmLabel="Delete"
  destructive
  onconfirm={confirmDelete}
  oncancel={cancelDelete}
/>

<style>
  .tb {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .unsaved {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    font-weight: 500;
    color: var(--t2);
    white-space: nowrap;
  }
  .udot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warn);
    box-shadow: 0 0 0 3px rgba(255, 159, 10, 0.18);
  }
  .save-wrap {
    display: inline-flex;
  }
  .lbl-short {
    display: none;
  }
  @media (max-width: 1100px) {
    .lbl-long {
      display: none;
    }
    .lbl-short {
      display: inline;
    }
    .unsaved {
      display: none;
    }
  }
  @media (max-width: 1240px) {
    .utxt {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
    }
    .unsaved {
      position: relative;
      padding: 0 2px;
    }
  }
  .banner {
    margin-bottom: 20px;
  }
  .loading {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .loading-row {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--t2);
    font-size: 13px;
  }
  .sk-caps {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr)) 124px;
    gap: 1px;
  }
  .empty {
    margin-top: 24px;
    border-radius: 16px;
    background: var(--panel-grid), var(--panel);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      0 24px 48px -32px rgb(0 0 0 / 0.9);
    padding: 32px 0;
  }
  .panel {
    will-change: transform, opacity;
  }
</style>
