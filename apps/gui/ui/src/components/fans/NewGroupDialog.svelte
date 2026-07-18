<script lang="ts">
  import type { AppConfig, FanGroupConfig, FanMode, HubSnapshot } from '../../lib/types';
  import DevicePickerModal from '../devices/DevicePickerModal.svelte';
  import ModeSelector from './ModeSelector.svelte';

  interface Props {
    open: boolean;
    config: AppConfig | null;
    availableHubs: HubSnapshot[];
    onsave: (group: FanGroupConfig) => void;
    oncancel: () => void;
  }

  let { open = $bindable(), config, availableHubs, onsave, oncancel }: Props = $props();

  let name = $state('');
  let mode = $state<'fixed' | 'curve' | 'pid'>('curve');
  let deviceIds = $state<string[]>([]);
  let pickerOpen = $state(false);

  // Reset on every open so the dialog isn't sticky.
  $effect(() => {
    if (open) {
      name = '';
      mode = 'curve';
      deviceIds = [];
      pickerOpen = false;
    }
  });

  const trimmedName = $derived(name.trim());
  const existingNames = $derived(
    new Set(config?.fan_groups.map((g) => g.name) ?? []),
  );
  const duplicate = $derived(
    trimmedName.length > 0 && existingNames.has(trimmedName),
  );
  const canSave = $derived(
    trimmedName.length > 0 && !duplicate && deviceIds.length > 0,
  );

  function buildInitialMode(type: 'fixed' | 'curve' | 'pid'): FanMode {
    switch (type) {
      case 'fixed':
        return { type: 'fixed', duty_percent: 50 };
      case 'curve':
        return {
          type: 'curve',
          points: [
            { temp: 30, duty: 25 },
            { temp: 50, duty: 40 },
            { temp: 70, duty: 70 },
            { temp: 85, duty: 100 },
          ],
          hysteresis: 3,
          ramp_rate: 5,
          ramp_down_rate: null,
          temp_source: { sensors: ['cpu'], weights: [1] },
        };
      case 'pid':
        return {
          type: 'pid',
          target_temp: 65,
          kp: 2.5,
          ki: 0.15,
          kd: 1.0,
          min_duty: 25,
          max_duty: 100,
          temp_source: { sensors: ['cpu'], weights: [1] },
        };
    }
  }

  function save() {
    if (!canSave) return;
    onsave({
      name: trimmedName,
      channels: [],
      hub_serial: null,
      device_ids: deviceIds,
      mode: buildInitialMode(mode),
      preset_applied: null,
    });
  }

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) oncancel();
  }
  function handleBackdropKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Escape') oncancel();
  }
  function handleWindowKey(e: KeyboardEvent) {
    if (!open || pickerOpen) return;
    if (e.key === 'Escape') oncancel();
    if (e.key === 'Enter' && canSave) save();
  }
</script>

<svelte:window onkeydown={handleWindowKey} />

{#if open}
  <div
    class="backdrop"
    role="button"
    tabindex="-1"
    onclick={handleBackdrop}
    onkeydown={handleBackdropKey}
    aria-label="Close new-group dialog"
  >
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="new-group-title">
      <header>
        <h3 id="new-group-title">New fan group</h3>
        <button class="close" onclick={oncancel} aria-label="Close">&times;</button>
      </header>

      <div class="body">
        <label class="field">
          <span class="label">Name</span>
          <input
            type="text"
            bind:value={name}
            placeholder="e.g. Radiator fans"
            maxlength="64"
            aria-invalid={duplicate}
          />
          {#if duplicate}
            <span class="field-error">A fan group named “{trimmedName}” already exists.</span>
          {/if}
        </label>

        <div class="field">
          <span class="label">Control mode</span>
          <ModeSelector value={mode} onchange={(m) => (mode = m)} />
          <span class="hint">Curve is a good default — you can change this later.</span>
        </div>

        <div class="field">
          <span class="label">Devices ({deviceIds.length})</span>
          <button class="pick-btn" onclick={() => (pickerOpen = true)}>
            {deviceIds.length === 0 ? 'Select devices…' : 'Change selection…'}
          </button>
          {#if deviceIds.length === 0}
            <span class="hint">Pick at least one fan or strip for the group to do anything.</span>
          {/if}
        </div>
      </div>

      <footer>
        <button class="cancel" onclick={oncancel}>Cancel</button>
        <button class="primary" disabled={!canSave} onclick={save}>Create group</button>
      </footer>
    </div>
  </div>

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
    z-index: 110;
    padding: 24px;
    border: none;
    cursor: default;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
    width: min(480px, 100%);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }
  h3 {
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
  .body {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .field input[type='text'] {
    padding: 6px 10px;
    background: var(--bg-input, var(--bg-card-inset));
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 13px;
    font-family: inherit;
  }
  .field input[type='text']:focus {
    outline: none;
    border-color: var(--accent);
  }
  .field input[aria-invalid='true'] {
    border-color: var(--danger);
  }
  .field-error {
    color: var(--danger);
    font-size: 11px;
  }
  .hint {
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.4;
  }
  .pick-btn {
    align-self: flex-start;
    padding: 6px 12px;
    background: transparent;
    color: var(--text-secondary);
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    font-size: 12px;
    cursor: pointer;
  }
  .pick-btn:hover {
    color: var(--accent);
    border-color: var(--accent-dim);
    border-style: solid;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.02);
  }
  footer button {
    padding: 6px 14px;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 12px;
    font-family: inherit;
  }
  .cancel {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-secondary);
  }
  .cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }
  .primary {
    background: var(--accent);
    color: var(--bg-base);
    border: 1px solid var(--accent);
    font-weight: 600;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .primary:not(:disabled):hover {
    filter: brightness(1.1);
  }
</style>
