<script lang="ts">
  import type { FanGroupConfig, FanMode, HubSnapshot, HubDeviceEntry } from '../../lib/types';
  import { displayName, isFanGroupV2 } from '../../lib/identity';
  import { configStore } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import ModeSelector from './ModeSelector.svelte';
  import CurveEditor from './CurveEditor.svelte';
  import NoiseBehavior from './NoiseBehavior.svelte';
  import PidTuner from './PidTuner.svelte';
  import TempSourceEditor from './TempSourceEditor.svelte';
  import DevicePickerModal from '../devices/DevicePickerModal.svelte';
  import type { TempSourceConfig } from '../../lib/types';

  interface Props {
    group: FanGroupConfig;
    currentTemp?: number;
    onchange: (group: FanGroupConfig) => void;
    /**
     * Validity callback. Fires when the group develops (or clears) a
     * hard curve error so the parent Save button can disable/enable.
     */
    onvalidate?: (hasError: boolean) => void;
    /**
     * Rename callback. The parent commits the name change (it may need
     * to check for duplicate group names before persisting).
     */
    onrename?: (newName: string) => void;
    /** Delete callback. The parent handles the confirm dialog. */
    ondelete?: () => void;
    availableHubs?: HubSnapshot[];
    /**
     * Sensor identifiers the control loop currently knows about. Passed
     * down from FanGroups (fetched once via `list_available_sensors`).
     * Powers the TempSourceEditor — sensors not in this list are shown
     * as unavailable chips so orphan references are visible.
     */
    availableSensors?: string[];
    expanded?: boolean;
  }

  let {
    group,
    currentTemp,
    onchange,
    onvalidate,
    onrename,
    ondelete,
    availableHubs = [],
    availableSensors = [],
    expanded = false,
  }: Props = $props();

  // --- Inline rename of the group header ---
  let renaming = $state(false);
  let renameDraft = $state('');

  function startRename() {
    if (!onrename) return; // not wired = no rename UI
    renameDraft = group.name;
    renaming = true;
  }
  function commitRename() {
    const next = renameDraft.trim();
    renaming = false;
    if (next && next !== group.name) {
      onrename?.(next);
    }
  }
  function cancelRename() {
    renaming = false;
    renameDraft = '';
  }
  function onRenameKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitRename();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelRename();
    }
  }

  let pickerOpen = $state(false);

  /**
   * When the user saves the picker, convert the group to V2 shape:
   * device_ids becomes authoritative, channels + hub_serial are cleared.
   * The backend's dual-key control loop honours device_ids when non-empty
   * (see control_loop::set_speeds path).
   */
  function handlePickerSave(deviceIds: string[]) {
    pickerOpen = false;
    onchange({
      ...group,
      device_ids: deviceIds,
      channels: [],
      hub_serial: null,
    });
  }

  /**
   * Current device_ids for the picker seed. If the group is V1 we
   * convert its (hub_serial, channel) pairs into device_ids at open
   * time by walking the enumerated hubs. Unresolvable channels become
   * orphans inside the picker so the user sees they exist.
   */
  function seedDeviceIds(): string[] {
    if (isFanGroupV2(group)) {
      return group.device_ids ?? [];
    }
    const ids: string[] = [];
    const channels = group.channels ?? [];
    const targetHub = group.hub_serial ?? null;
    for (const hub of availableHubs) {
      if (targetHub && hub.serial !== targetHub) continue;
      for (const dev of hub.devices as HubDeviceEntry[]) {
        if (channels.includes(dev.channel) && dev.device_id) {
          ids.push(dev.device_id);
        }
      }
    }
    return ids;
  }

  let seededIds = $state<string[]>([]);

  function openPicker() {
    seededIds = seedDeviceIds();
    pickerOpen = true;
  }

  const modeType = $derived(group.mode.type);

  // Membership label: prefer device_ids (V2) when present, fall back to
  // the V1 channel list. displayName() handles the user-name / location /
  // orphan-id precedence per-id.
  const membershipLabel = $derived.by(() => {
    if (isFanGroupV2(group)) {
      const ids = group.device_ids ?? [];
      if (ids.length === 0) return 'No devices';
      return ids
        .map((id) =>
          displayName(id, { config: configStore.config, snapshot: sensors.snapshot })
        )
        .join(', ');
    }
    const chans = group.channels ?? [];
    if (chans.length === 0) return 'No channels';
    return `Ch ${chans.join(', ')}`;
  });

  function changeMode(newMode: 'fixed' | 'curve' | 'pid') {
    let mode: FanMode;
    switch (newMode) {
      case 'fixed':
        mode = { type: 'fixed', duty_percent: 50 };
        break;
      case 'curve':
        mode = {
          type: 'curve',
          points: [
            { temp: 30, duty: 25 },
            { temp: 50, duty: 40 },
            { temp: 70, duty: 70 },
            { temp: 85, duty: 100 },
          ],
          hysteresis: 3,
          ramp_rate: 5,
          temp_source: { sensors: ['cpu'], weights: [1] },
        };
        break;
      case 'pid':
        mode = {
          type: 'pid',
          target_temp: 65,
          kp: 2.5,
          ki: 0.15,
          kd: 1.0,
          min_duty: 25,
          max_duty: 100,
          temp_source: { sensors: ['cpu'], weights: [1] },
        };
        break;
    }
    onchange({ ...group, mode });
  }

  function updateNoise(values: { hysteresis: number; rampUp: number; rampDown: number | null }) {
    if (group.mode.type !== 'curve') return;
    onchange({
      ...group,
      mode: {
        ...group.mode,
        hysteresis: values.hysteresis,
        ramp_rate: values.rampUp,
        ramp_down_rate: values.rampDown,
      } as FanMode & { type: 'curve' },
    });
  }

  function updateTempSource(source: TempSourceConfig) {
    if (group.mode.type === 'fixed') return;
    onchange({
      ...group,
      mode: { ...group.mode, temp_source: source } as FanMode,
    });
  }
</script>

<div class="fan-group-card" class:expanded>
  <div class="header">
    {#if renaming}
      <input
        class="group-name-input"
        bind:value={renameDraft}
        onblur={commitRename}
        onkeydown={onRenameKey}
        maxlength="64"
        aria-label="Rename fan group"
      />
    {:else}
      <button
        class="group-name-trigger"
        class:renamable={!!onrename}
        onclick={startRename}
        disabled={!onrename}
        title={onrename ? 'Click to rename group' : undefined}
      >
        <h4 class="group-name">{group.name}</h4>
        {#if onrename}
          <svg
            class="rename-icon"
            viewBox="0 0 16 16"
            width="11"
            height="11"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M12.146 1.854a2 2 0 0 1 2.828 2.828l-8.5 8.5a1 1 0 0 1-.407.244l-3 1a.5.5 0 0 1-.632-.632l1-3a1 1 0 0 1 .244-.407l8.5-8.5zm2.12 2.121a1 1 0 0 0-1.414-1.414L4.5 11.086V11.5h.414l8.353-8.353z"
              fill="currentColor"
            />
          </svg>
        {/if}
      </button>
    {/if}
    <div class="header-right">
      <span class="channels" title={membershipLabel}>{membershipLabel}</span>
      <button class="edit-devices" onclick={openPicker} title="Edit devices in this group">
        Edit devices
      </button>
      {#if ondelete}
        <button class="delete-group" onclick={() => ondelete?.()} title="Delete this fan group">
          Delete
        </button>
      {/if}
    </div>
  </div>

  <DevicePickerModal
    bind:open={pickerOpen}
    title={`Devices in ${group.name}`}
    selected={seededIds}
    {availableHubs}
    config={configStore.config}
    onsave={handlePickerSave}
    oncancel={() => (pickerOpen = false)}
  />

  <ModeSelector value={modeType} onchange={changeMode} />

  <div class="mode-content">
    {#if group.mode.type === 'fixed'}
      <div class="fixed-control">
        <span class="field-label">Duty</span>
        <div class="slider-row">
          <input
            type="range" min="0" max="100" step="1"
            value={group.mode.duty_percent}
            oninput={(e) => onchange({ ...group, mode: { type: 'fixed', duty_percent: Number(e.currentTarget.value) } })}
          />
          <span class="tabular-nums">{group.mode.duty_percent}%</span>
        </div>
      </div>
    {:else if group.mode.type === 'curve'}
      <CurveEditor
        points={group.mode.points}
        {currentTemp}
        hysteresis={group.mode.hysteresis}
        onchange={(points) => onchange({ ...group, mode: { ...group.mode, points } as FanMode & { type: 'curve' } })}
        onvalidate={(hasError) => onvalidate?.(hasError)}
        {expanded}
      />
      <NoiseBehavior
        hysteresis={group.mode.hysteresis}
        rampUp={group.mode.ramp_rate}
        rampDown={group.mode.ramp_down_rate ?? null}
        onchange={updateNoise}
      />
      <TempSourceEditor
        source={group.mode.temp_source}
        {availableSensors}
        onchange={updateTempSource}
      />
    {:else if group.mode.type === 'pid'}
      <PidTuner
        targetTemp={group.mode.target_temp}
        kp={group.mode.kp}
        ki={group.mode.ki}
        kd={group.mode.kd}
        minDuty={group.mode.min_duty}
        maxDuty={group.mode.max_duty}
        onchange={(vals) => onchange({
          ...group,
          mode: {
            ...group.mode,
            target_temp: vals.targetTemp,
            kp: vals.kp,
            ki: vals.ki,
            kd: vals.kd,
            min_duty: vals.minDuty,
            max_duty: vals.maxDuty,
          } as FanMode & { type: 'pid' }
        })}
      />
      <TempSourceEditor
        source={group.mode.temp_source}
        {availableSensors}
        onchange={updateTempSource}
      />
    {/if}
  </div>
</div>

<style>
  .fan-group-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .fan-group-card:not(.expanded) {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 16px;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .group-name {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
  .expanded .group-name {
    font-size: 20px;
  }
  .group-name-trigger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: 1px dashed transparent;
    border-radius: 6px;
    padding: 2px 6px;
    margin: -2px -6px;
    color: inherit;
    cursor: text;
  }
  .group-name-trigger:disabled {
    cursor: default;
  }
  .group-name-trigger.renamable:hover,
  .group-name-trigger.renamable:focus-visible {
    border-color: var(--border);
    background: rgba(255, 255, 255, 0.03);
    outline: none;
  }
  .rename-icon {
    color: var(--text-muted);
    opacity: 0;
    transition: opacity 0.15s, color 0.15s;
  }
  .group-name-trigger.renamable:hover .rename-icon,
  .group-name-trigger.renamable:focus-visible .rename-icon {
    opacity: 1;
    color: var(--accent);
  }
  .group-name-input {
    font-size: 20px;
    font-weight: 600;
    background: var(--bg-elevated);
    border: 1px solid var(--accent-dim);
    border-radius: 6px;
    padding: 2px 8px;
    color: var(--text-primary);
    font-family: inherit;
    width: min(260px, 60%);
  }
  .group-name-input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .channels {
    font-size: 12px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .edit-devices,
  .delete-group {
    font-size: 11px;
    padding: 4px 10px;
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition: all 0.15s;
  }
  .edit-devices:hover {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .delete-group:hover {
    color: var(--danger);
    border-color: var(--danger);
  }
  .mode-content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .fixed-control .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }
  .slider-row span {
    width: 48px;
    text-align: right;
    font-size: 13px;
    font-weight: 500;
  }
</style>
