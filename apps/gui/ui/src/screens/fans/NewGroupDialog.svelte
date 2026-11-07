<!--
  NewGroupDialog (inventory 2.7). Sheet with title "New fan group" and × close.
  Name (max 64, placeholder "e.g. Radiator fans", duplicate error), Control
  mode (default Curve + hint), Devices (N) opening the DevicePickerModal,
  "Create group" gated on name + no duplicate + ≥1 device. Escape cancels,
  Enter creates when valid (suppressed while the nested picker is open),
  backdrop cancels. Resets on every open.
-->
<script lang="ts">
  import Sheet from '../../components/ui/Sheet.svelte';
  import Button from '../../components/ui/Button.svelte';
  import Segmented from '../../components/ui/Segmented.svelte';
  import ListChecks from 'lucide-svelte/icons/list-checks';
  import DevicePickerModal from './DevicePickerModal.svelte';
  import { defaultMode, MODE_OPTIONS, type ModeType } from './fanModes';
  import { displayNameFromTree } from '../../lib/identity';
  import type { AppConfig, FanGroupConfig, HubSnapshot } from '../../lib/types';

  interface Props {
    open: boolean;
    config: AppConfig | null;
    availableHubs: HubSnapshot[];
    onsave: (group: FanGroupConfig) => void;
    oncancel: () => void;
  }
  let { open = $bindable(), config, availableHubs, onsave, oncancel }: Props = $props();

  let name = $state('');
  let mode = $state<ModeType>('curve');
  let deviceIds = $state<string[]>([]);
  let pickerOpen = $state(false);

  $effect(() => {
    if (open) {
      name = '';
      mode = 'curve';
      deviceIds = [];
      pickerOpen = false;
    }
  });

  const trimmed = $derived(name.trim());
  const duplicate = $derived(trimmed.length > 0 && (config?.fan_groups ?? []).some((g) => g.name === trimmed));
  const canSave = $derived(trimmed.length > 0 && !duplicate && deviceIds.length > 0);
  const chosen = $derived(deviceIds.map((id) => displayNameFromTree(id, { config, hubs: availableHubs })));

  function save() {
    if (!canSave) return;
    onsave({
      name: trimmed,
      channels: [],
      hub_serial: null,
      device_ids: [...deviceIds],
      mode: defaultMode(mode, true),
      preset_applied: null,
    });
  }

  function onKey(e: KeyboardEvent) {
    if (pickerOpen) return;
    if (e.key === 'Enter') {
      const t = e.target as HTMLElement;
      // a focused button handles its own Enter
      if (t.closest('button')) return;
      e.preventDefault();
      save();
    }
  }
  const nameId = 'ng-name';
  const errId = 'ng-name-err';
</script>

<Sheet bind:open title="New fan group" width={480} onclose={oncancel} onkeydown={onKey}>
  <div class="form">
    <div class="field">
      <label class="lab" for={nameId}>Name</label>
      <input
        id={nameId}
        class="input"
        type="text"
        name="group-name"
        autocomplete="off"
        spellcheck="false"
        maxlength="64"
        placeholder="e.g. Radiator fans"
        aria-invalid={duplicate || undefined}
        aria-describedby={duplicate ? errId : undefined}
        bind:value={name}
        data-autofocus
      />
      {#if duplicate}
        <p class="err" id={errId} role="alert">A fan group named "{trimmed}" already exists.</p>
      {/if}
    </div>

    <div class="field">
      <span class="lab" id="ng-mode-l">Control mode</span>
      <div><Segmented options={MODE_OPTIONS} bind:value={mode} label="Control mode" /></div>
      <p class="hint">Curve is a good default — you can change this later.</p>
    </div>

    <div class="field">
      <span class="lab num">Devices ({deviceIds.length})</span>
      <div class="devices">
        <Button variant="secondary" icon={ListChecks} onclick={() => (pickerOpen = true)}>
          {deviceIds.length === 0 ? 'Select devices…' : 'Change selection…'}
        </Button>
        {#if chosen.length}
          <p class="chosen" title={chosen.join(', ')}>{chosen.join(', ')}</p>
        {/if}
      </div>
      {#if deviceIds.length === 0}
        <p class="hint">Pick at least one fan or strip for the group to do anything.</p>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    <Button variant="secondary" onclick={oncancel}>Cancel</Button>
    <Button variant="primary" disabled={!canSave} onclick={save}>Create group</Button>
  {/snippet}
</Sheet>

<DevicePickerModal
  bind:open={pickerOpen}
  title="Devices for new group"
  selected={deviceIds}
  {availableHubs}
  {config}
  onsave={(ids) => {
    deviceIds = ids;
    pickerOpen = false;
  }}
  oncancel={() => (pickerOpen = false)}
/>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding-top: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .lab {
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--t2);
  }
  .input {
    width: 100%;
    height: 34px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--r-control);
    background: rgba(0, 0, 0, 0.22);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    color: var(--t1);
    font-size: 14px;
    transition: box-shadow var(--dur-fast) ease-out;
  }
  .input::placeholder {
    color: var(--t3);
  }
  .input:hover {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.22);
  }
  .input:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
  }
  .input[aria-invalid='true'] {
    box-shadow:
      inset 0 0 0 1px var(--error),
      0 0 0 3px rgba(255, 69, 58, 0.22);
  }
  .err {
    font-size: 12px;
    line-height: 16px;
    color: var(--error-text);
  }
  .hint {
    font-size: 12px;
    line-height: 16px;
    color: var(--t3);
  }
  .devices {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .chosen {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
