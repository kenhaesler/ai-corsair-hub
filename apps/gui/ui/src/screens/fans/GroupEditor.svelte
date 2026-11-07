<!--
  GroupEditor (FanGroupCard, inventory 2.2) as System-Settings inset sections:
  Membership (device list with live RPM) · Mode · mode content. The group's
  name, rename and live duty live in its capsule (single home, no redundancy).
  Owns the group's undo history (Ctrl+Z anywhere in the editor).
-->
<script lang="ts">
  import ListChecks from 'lucide-svelte/icons/list-checks';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import Fan from 'lucide-svelte/icons/fan';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import Segmented from '../../components/ui/Segmented.svelte';
  import Slider from '../../components/ui/Slider.svelte';
  import Button from '../../components/ui/Button.svelte';
  import CurveEditor from './CurveEditor.svelte';
  import PresetChips from './PresetChips.svelte';
  import NoiseBehavior from './NoiseBehavior.svelte';
  import SensorMix from './SensorMix.svelte';
  import PidTuner from './PidTuner.svelte';
  import DevicePickerModal from './DevicePickerModal.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { display } from '../../lib/stores/scrub.svelte';
  import { displayName, isFanGroupV2 } from '../../lib/identity';
  import { tempOf } from '../../lib/format';
  import { computePresetState, groupMatchesPreset } from '../../lib/presets';
  import { defaultMode, MODE_BLURB, MODE_OPTIONS, mixTemp, type ModeType } from './fanModes';
  import type { FanGroupConfig, FanMode, FanPresetDef, HubSnapshot, TempSourceConfig } from '../../lib/types';

  interface Props {
    group: FanGroupConfig;
    color: string;
    availableHubs: HubSnapshot[];
    availableSensors: string[];
    presets: FanPresetDef[];
    onchange: (g: FanGroupConfig) => void;
    ondelete: () => void;
  }
  let { group, color, availableHubs, availableSensors, presets, onchange, ondelete }: Props = $props();

  // ------------------------------------------------------------------ undo
  let history = $state<string[]>([]);
  let lastKey = '';
  let lastAt = 0;
  function checkpoint(key: string) {
    const now = performance.now();
    if (key === lastKey && now - lastAt < 900) {
      lastAt = now;
      return;
    }
    lastKey = key;
    lastAt = now;
    history = [...history.slice(-49), JSON.stringify($state.snapshot(group))];
  }
  function undo() {
    const prev = history[history.length - 1];
    if (!prev) return;
    history = history.slice(0, -1);
    lastKey = '';
    const g = JSON.parse(prev) as FanGroupConfig;
    onchange({ ...g, name: group.name });
  }
  function onKey(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.key.toLowerCase() !== 'z') return;
    const t = e.target as HTMLElement;
    if (t.matches('input[type=text], textarea')) return;
    e.preventDefault();
    undo();
  }
  function change(next: FanGroupConfig, key: string) {
    checkpoint(key);
    onchange(next);
  }

  // ------------------------------------------------------------- membership
  let pickerOpen = $state(false);
  let seeded = $state<string[]>([]);
  function seedIds(): string[] {
    if (isFanGroupV2(group)) return [...(group.device_ids ?? [])];
    const ids: string[] = [];
    const chans = group.channels ?? [];
    for (const hub of availableHubs) {
      if (group.hub_serial && hub.serial !== group.hub_serial) continue;
      for (const d of hub.devices) if (chans.includes(d.channel) && d.device_id) ids.push(d.device_id);
    }
    return ids;
  }
  function openPicker() {
    seeded = seedIds();
    pickerOpen = true;
  }
  function savePicker(ids: string[]) {
    pickerOpen = false;
    change({ ...group, device_ids: ids, channels: [], hub_serial: null }, `devices-${Date.now()}`);
  }

  const membership = $derived.by(() => {
    if (isFanGroupV2(group)) {
      const ids = group.device_ids ?? [];
      if (ids.length === 0) return 'No devices';
      return ids.map((id) => displayName(id, { config: configStore.config, snapshot: display.live })).join(', ');
    }
    const ch = group.channels ?? [];
    return ch.length === 0 ? 'No channels' : `Ch ${ch.join(', ')}`;
  });
  const memberCount = $derived(isFanGroupV2(group) ? (group.device_ids?.length ?? 0) : (group.channels?.length ?? 0));

  const snap = $derived(display.snapshot);
  /** Membership chips: one per device, with live RPM when the device reports one. */
  const members = $derived(
    isFanGroupV2(group)
      ? (group.device_ids ?? []).map((id) => ({
          id,
          name: displayName(id, { config: configStore.config, snapshot: display.live }),
          rpm: snap?.fans.find((f) => f.device_id === id)?.rpm ?? null,
        }))
      : [],
  );

  // ------------------------------------------------------------------ mode
  const modeType = $derived(group.mode.type);
  function setMode(t: ModeType) {
    if (t === group.mode.type) return;
    change({ ...group, mode: defaultMode(t) }, `mode-${Date.now()}`);
  }
  function setMode2(m: FanMode, key: string) {
    change({ ...group, mode: m }, key);
  }
  function setSource(src: TempSourceConfig) {
    if (group.mode.type === 'fixed') return;
    onchange({ ...group, mode: { ...group.mode, temp_source: src } });
  }
  /**
   * The preset's sensor weights (cpu / gpu / water). Unavailable sensors are
   * kept so the Sensor Mix shows them as "referenced but not available"
   * (e.g. Water Priority without an inline water sensor falls back to CPU/GPU).
   */
  function presetSource(p: FanPresetDef): TempSourceConfig | null {
    const pairs = (
      [
        ['cpu', p.cpu_weight],
        ['gpu', p.gpu_weight],
        ['water_inline', p.water_weight],
      ] as [string, number][]
    ).filter(([, w]) => w > 0);
    return pairs.length ? { sensors: pairs.map((x) => x[0]), weights: pairs.map((x) => x[1]) } : null;
  }
  function applyPreset(p: FanPresetDef) {
    if (group.mode.type !== 'curve') return;
    change(
      {
        ...group,
        mode: {
          ...group.mode,
          points: p.points.map((q) => ({ ...q })),
          hysteresis: p.hysteresis,
          ramp_rate: p.ramp_up,
          ramp_down_rate: p.ramp_down,
          temp_source: presetSource(p) ?? group.mode.temp_source,
        },
        preset_applied: p.id,
      },
      `preset-${Date.now()}`,
    );
  }
  const activePreset = $derived(presets.find((p) => groupMatchesPreset(group, p))?.id ?? null);
  const taggedPreset = $derived(group.preset_applied ? presets.find((p) => p.id === group.preset_applied) : undefined);
  const modifiedPreset = $derived(
    taggedPreset && computePresetState(configStore.config, taggedPreset, group.name) === 'modified' ? taggedPreset.id : null,
  );

  // ------------------------------------------------------------------ live
  const cpu = $derived(tempOf(snap, 'cpu'));
  const mix = $derived(group.mode.type === 'fixed' ? null : mixTemp(group.mode.temp_source, snap?.temperatures));
  const mixMeta = $derived.by(() => {
    if (group.mode.type === 'fixed') return '';
    const w = group.mode.temp_source.weights;
    const n = group.mode.temp_source.sensors.length;
    const sum = group.mode.temp_source.sensors.reduce((a, _, i) => a + (w[i] ?? 1), 0);
    return `${n} active · Σ ${sum.toFixed(2)}`;
  });

  let noiseOpen = $state(false);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="editor" onkeydown={onKey} style:--gc={color}>
  <div class="sections">
    <div class="strip">
      <section class="seg mem" aria-label="Membership">
        <span class="tile" aria-hidden="true"><Fan size={14} strokeWidth={2.25} /></span>
        <span class="mlabel">Devices <span class="t3 num">{memberCount}</span></span>
        {#if members.length}
          <ul class="chips-m" aria-label={membership} title={membership}>
            {#each members as d (d.id)}
              <li class="dchip">
                <span class="dname">{d.name}</span>{#if d.rpm !== null}<span class="drpm num">{Math.round(d.rpm).toLocaleString('en-US')} rpm</span>{/if}
              </li>
            {/each}
          </ul>
        {:else}
          <span class="msub" title={membership}>{membership}</span>
        {/if}
        <div class="mbtns">
          <Button size="sm" icon={ListChecks} onclick={openPicker} title="Edit devices in this group">Edit devices</Button>
          <Button size="sm" icon={Trash2} class="del-btn" onclick={ondelete} title="Delete this fan group">Delete</Button>
        </div>
      </section>
      <section class="seg mode" aria-label="Mode" title={MODE_BLURB[modeType]}>
        <span class="mlabel">Mode</span>
        <Segmented options={MODE_OPTIONS} value={modeType} label="Control mode" onchange={setMode} />
      </section>
    </div>

    {#if group.mode.type === 'fixed'}
      {@const m = group.mode}
      <InsetGroup title="Duty">
        <div class="fixed">
          <div class="fixed-fig">
            <span class="t-figure-l num">{m.duty_percent}</span><span class="unit">%</span>
          </div>
          <div class="fixed-sl">
            <Slider
              label="Duty"
              hideLabel
              min={0}
              max={100}
              step={1}
              {color}
              value={m.duty_percent}
              format={(v) => `${v}%`}
              oninput={(v) => setMode2({ type: 'fixed', duty_percent: v }, 'fixed-duty')}
            />
            <div class="fixed-scale num" aria-hidden="true"><span>0%</span><span>50%</span><span>100%</span></div>
          </div>
        </div>
      </InsetGroup>
    {:else if group.mode.type === 'curve'}
      {@const m = group.mode}
      <InsetGroup title="Curve">
        {#snippet action()}
          {#if presets.length}
            <PresetChips {presets} active={activePreset} modified={modifiedPreset} {color} onapply={applyPreset} />
          {/if}
        {/snippet}
        <CurveEditor
          points={m.points}
          hysteresis={m.hysteresis}
          {color}
          currentTemp={cpu}
          mixTemp={mix}
          canUndo={history.length > 0}
          onundo={undo}
          oncheckpoint={checkpoint}
          onchange={(points) => onchange({ ...group, mode: { ...m, points } })}
        />
      </InsetGroup>

      <div class="panel">
        <NoiseBehavior
          bind:open={noiseOpen}
          hysteresis={m.hysteresis}
          rampUp={m.ramp_rate}
          rampDown={m.ramp_down_rate ?? null}
          {color}
          onchange={(v) =>
            change(
              { ...group, mode: { ...m, hysteresis: v.hysteresis, ramp_rate: v.rampUp, ramp_down_rate: v.rampDown } },
              `noise-${Date.now()}`,
            )}
        />
      </div>

      <InsetGroup title="Sensor Mix">
        {#snippet action()}<span class="sec-meta num">{mixMeta}</span>{/snippet}
        <SensorMix source={m.temp_source} {availableSensors} onchange={setSource} oncheckpoint={checkpoint} />
      </InsetGroup>
    {:else}
      {@const m = group.mode}
      <InsetGroup title="PID">
        <PidTuner
          targetTemp={m.target_temp}
          kp={m.kp}
          ki={m.ki}
          kd={m.kd}
          minDuty={m.min_duty}
          maxDuty={m.max_duty}
          {color}
          liveTemp={mix}
          oncheckpoint={checkpoint}
          onchange={(v) =>
            onchange({
              ...group,
              mode: {
                ...m,
                target_temp: v.targetTemp,
                kp: v.kp,
                ki: v.ki,
                kd: v.kd,
                min_duty: v.minDuty,
                max_duty: v.maxDuty,
              },
            })}
        />
      </InsetGroup>

      <InsetGroup title="Sensor Mix">
        {#snippet action()}<span class="sec-meta num">{mixMeta}</span>{/snippet}
        <SensorMix source={m.temp_source} {availableSensors} onchange={setSource} oncheckpoint={checkpoint} />
      </InsetGroup>
    {/if}

  </div>
</div>

<DevicePickerModal
  bind:open={pickerOpen}
  title="Devices in {group.name}"
  selected={seeded}
  {availableHubs}
  config={configStore.config}
  onsave={savePicker}
  oncancel={() => (pickerOpen = false)}
/>

<style>
  .editor {
    min-width: 0;
  }
  .editor :global(.ig-action) {
    min-width: 0;
  }

  .chips-m {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dchip {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    height: 24px;
    padding: 0 9px;
    align-items: center;
    border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px var(--hair);
    font-size: 12px;
    white-space: nowrap;
  }
  .dname {
    color: var(--t1);
  }
  .drpm {
    color: var(--t3);
  }
  .strip {
    display: flex;
    align-items: stretch;
    min-height: 52px;
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
  }
  .seg {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    min-width: 0;
  }
  .seg.mem {
    flex: 1 1 auto;
  }
  .seg.mode {
    flex: none;
    box-shadow: inset 1px 0 0 var(--hair);
    padding-left: 16px;
  }
  .seg .msub {
    flex: 1;
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    -webkit-line-clamp: unset;
    line-clamp: unset;
  }
  .seg .mlabel {
    flex: none;
  }
  .mode .mlabel {
    color: var(--t2);
  }

  .sections {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .sec-meta {
    font-size: 12px;
    font-weight: 500;
    color: var(--t3);
  }
  .panel {
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    margin-top: -12px;
  }

  .tile {
    flex: none;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    background: var(--gc);
    color: #0b0d12;
  }
  .mlabel {
    font-size: 13px;
    font-weight: 500;
    line-height: 18px;
    color: var(--t1);
  }
  .msub {
    font-size: 12px;
    line-height: 17px;
    color: var(--t3);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .mbtns {
    flex: none;
    display: flex;
    gap: 6px;
  }
  .mbtns :global(.del-btn) {
    color: var(--error-text);
  }
  .mbtns :global(.del-btn:hover:not(:disabled)) {
    background: var(--error-surface);
  }

  .fixed {
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 18px 20px 20px;
  }
  .fixed-fig .unit {
    font-weight: 600;
    color: var(--t2);
    margin-left: 2px;
    font-size: 17px;
  }
  .fixed-fig {
    flex: none;
    width: 112px;
    display: flex;
    align-items: baseline;
  }
  .fixed-sl {
    flex: 1;
    min-width: 0;
  }
  .fixed-sl :global(.val) {
    display: none;
  }
  .fixed-scale {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    font-size: 11px;
    color: var(--t3);
  }
</style>
