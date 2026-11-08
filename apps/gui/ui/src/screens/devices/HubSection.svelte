<!--
  Hub section (inventory 3.5): title "iCUE LINK Hub — XXXX", FW, full serial in
  mono, the daisy-chain strip, and the channel table (Ch, Type, Name with
  inline rename, Model, ID, RPM). RPM reads display.snapshot, so the table
  follows the time-scrub. While Identify or the LED marker diagnostic runs, the
  table swaps Model/ID for the diagnostic columns (colour; or LEDs, count
  source, LED range) and the header meta carries the marker line, so each
  fact has one home. Rename: button+pencil -> input (Enter/blur commit,
  Esc cancel, disabled while saving); commit = renameDevice then loadConfig.
-->
<script lang="ts">
  import Pencil from 'lucide-svelte/icons/pencil';
  import type { HubSnapshot } from '../../lib/types';
  import { displayNameFromTree, shortHubSerial, deviceTypeLabel } from '../../lib/identity';
  import { applyDeviceName, configStore, loadConfig } from '../../lib/stores/config.svelte';
  import { display } from '../../lib/stores/scrub.svelte';
  import { toasts } from '../../lib/stores/toasts.svelte';
  import { renameDevice } from '../../lib/api';
  import { groupColor } from '../../lib/format';
  import Badge from '../../components/ui/Badge.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import Bar from '../../components/ui/Bar.svelte';
  import ChainStrip from './ChainStrip.svelte';
  import type { Diagnostics } from './diagnostics.svelte';

  interface Props {
    hub: HubSnapshot;
    diag: Diagnostics;
  }
  let { hub, diag }: Props = $props();

  let editingId = $state<string | null>(null);
  let editValue = $state('');
  let saving = $state(false);
  let hovered = $state<string | null>(null);

  const health = $derived(display.live?.hub_health.find((h) => h.serial === hub.serial) ?? null);
  const rows = $derived(hub.devices);

  // Diagnostics merged into this hub's table.
  const mode = $derived<'idle' | 'identify' | 'markers'>(
    diag.identifyRunning && diag.identifyMap ? 'identify' : diag.markerRunning && diag.markerReport ? 'markers' : 'idle',
  );
  const idByDevice = $derived(
    new Map((mode === 'identify' ? (diag.identifyMap ?? []) : []).filter((r) => r.hub_serial === hub.serial).map((r) => [r.device_id, r])),
  );
  const mk = $derived(mode === 'markers' ? (diag.markerReport?.hubs.find((h) => h.hub_serial === hub.serial) ?? null) : null);
  const slotByDevice = $derived(new Map((mk?.slots ?? []).map((x) => [x.device_id, x])));
  const skippedByDevice = $derived(new Map((mk?.skipped_zero_count ?? []).map((x) => [x.device_id, x])));
  const rgb = (c: [number, number, number]) => `rgb(${c[0]},${c[1]},${c[2]})`;

  const savedName = (id: string) => configStore.config?.devices?.find((d) => d.device_id === id)?.name ?? '';
  const labelOf = (id: string) => displayNameFromTree(id, { config: configStore.config, hubs: [hub] });
  const fanOf = (id: string) => display.snapshot?.fans.find((f) => f.device_id === id);
  const rpmOf = (id: string, fallback: number | null) => fanOf(id)?.rpm ?? fallback;
  const hex = (n: number) => `0x${n.toString(16).padStart(2, '0').toUpperCase()}`;

  function groupColorOf(id: string): string {
    const name = fanOf(id)?.group_name;
    const i = name ? (configStore.config?.fan_groups.findIndex((g) => g.name === name) ?? -1) : -1;
    return groupColor(Math.max(0, i));
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function startEdit(id: string) {
    editingId = id;
    editValue = savedName(id);
  }

  function cancelEdit() {
    editingId = null;
    editValue = '';
  }

  async function commitEdit() {
    if (editingId == null || saving) return;
    const id = editingId;
    const name = editValue.trim();
    if (name === savedName(id)) {
      cancelEdit();
      return;
    }
    saving = true;
    try {
      await renameDevice(id, name);
      if (configStore.config) applyDeviceName(id, name);
      else await loadConfig();
      if (name) toasts.success(`Renamed to “${name}”`);
      else toasts.success('Name cleared', { detail: `Showing ${labelOf(id)}` });
    } catch (e) {
      toasts.error('Could not rename device', { detail: String(e) });
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
      e.stopPropagation();
      cancelEdit();
    }
  }
</script>

<section class="hub" aria-labelledby="hub-{hub.serial}">
  <header class="head">
    <div class="titles">
      <h3 id="hub-{hub.serial}" class="t-title3">iCUE LINK Hub — {shortHubSerial(hub.serial)}</h3>
      <p class="meta">
        <span class="fw num">{`FW ${hub.firmware}${mk ? ` · ${mk.total_leds} LEDs` : ''}`}</span>
        {#if mk && !mk.healthy}<span class="unhealthy">(unhealthy — not written)</span>{/if}
        <span class="serial mono">{hub.serial}</span>
      </p>
    </div>
    {#if health && !(mk && !mk.healthy)}
      <Badge size="md" dot tone={health.healthy ? 'success' : 'warn'}>
        {health.healthy ? 'Responding' : `Not responding · ${health.consecutive_failures} failures`}
      </Badge>
    {/if}
  </header>

  <div class="panel">
    <ChainStrip {hub} bind:active={hovered} />
    <div class="scroll">
      <table>
        <colgroup>
          <col class="w-ch" />
          <col class="w-type" />
          <col class="w-name" />
          {#if mode === 'idle'}
            <col class="w-model" />
            <col class="w-id" />
          {:else if mode === 'identify'}
            <col class="w-color" />
          {:else}
            <col class="w-leds" />
            <col class="w-src" />
            <col class="w-range" />
          {/if}
          <col class="w-rpm" />
        </colgroup>
        <thead>
          <tr>
            <th scope="col" class="c-ch">Ch</th>
            <th scope="col">Type</th>
            <th scope="col">Name</th>
            {#if mode === 'idle'}
              <th scope="col">Model</th>
              <th scope="col">ID</th>
            {:else if mode === 'identify'}
              <th scope="col" class="dg">Colour</th>
            {:else}
              <th scope="col" class="dg r">LEDs</th>
              <th scope="col" class="dg">Count source</th>
              <th scope="col" class="dg r">LED range</th>
            {/if}
            <th scope="col" class="r">RPM</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as device (device.device_id)}
            {@const rpm = rpmOf(device.device_id, device.rpm)}
            {@const label = labelOf(device.device_id)}
            {@const slot = slotByDevice.get(device.device_id)}
            {@const skip = skippedByDevice.get(device.device_id)}
            {@const ident = idByDevice.get(device.device_id)}
            <tr
              class:hot={hovered === device.device_id}
              class:skipped={mode === 'markers' && skippedByDevice.has(device.device_id)}
              onmouseenter={() => (hovered = device.device_id)}
              onmouseleave={() => (hovered = null)}
            >
              <td class="c-ch num strong">{device.channel}</td>
              <td class="type">{deviceTypeLabel(device)}</td>
              <td class="name">
                {#if editingId === device.device_id}
                  <span class="edit">
                    <input
                      class="rename-input"
                      type="text"
                      use:focusSelect
                      bind:value={editValue}
                      onblur={commitEdit}
                      onkeydown={onKeydown}
                      disabled={saving}
                      placeholder="Friendly name"
                      maxlength="64"
                      aria-label="Friendly name for {label}"
                      spellcheck="false"
                      autocomplete="off"
                    />
                    {#if saving}<span class="busy"><Spinner size={14} /></span>{/if}
                  </span>
                {:else}
                  {#if slot}<span class="bsw" style:background={rgb(slot.body_rgb)} aria-hidden="true"></span>{/if}
                  <button
                    type="button"
                    class="rename-trigger"
                    onclick={() => startEdit(device.device_id)}
                    title="Click to rename"
                    aria-label="Rename {label}"
                  >
                    <span class="rename-label">{label}</span>
                    <Pencil size={12} strokeWidth={2.25} class="pencil" />
                  </button>
                {/if}
              </td>
              {#if mode === 'idle'}
                <td class="num t2">{hex(device.model)}</td>
                <td class="mono t2">{device.device_id.slice(0, 12)}...</td>
              {:else if mode === 'identify'}
                <td class="dg">
                  {#if ident}
                    <span class="cname"><span class="sw" style:background={rgb(ident.rgb)}></span>{ident.color_name}</span>
                  {:else}
                    <span class="t3">—</span>
                  {/if}
                </td>
              {:else}
                <td class="dg num r">{slot ? slot.led_count : skip ? 0 : '—'}</td>
                <td class="dg"><span class="src mono">{slot?.count_source ?? skip?.count_source ?? '—'}</span></td>
                <td class="dg num r">{slot ? `${slot.start_led}–${slot.end_led - 1}` : skip ? 'no slot (skipped)' : '—'}</td>
              {/if}
              <td class="r">
                {#if rpm !== null}
                  <span class="rpm">
                    <span class="rbar">
                      <Bar
                        value={fanOf(device.device_id)?.duty_percent ?? 0}
                        max={100}
                        height={4}
                        color={groupColorOf(device.device_id)}
                        label="Duty of {label}"
                        valueText="{Math.round(fanOf(device.device_id)?.duty_percent ?? 0)}% duty"
                      />
                    </span>
                    <AnimatedNumber value={rpm} class="rnum" group />
                  </span>
                {:else}
                  <span class="t3">—</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
</section>

<style>
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-4);
    margin-bottom: var(--sp-3);
  }
  .titles {
    min-width: 0;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px var(--sp-3);
    margin-top: 2px;
    font-size: 12px;
    line-height: 16px;
    color: var(--t2);
  }
  .fw {
    font-weight: 600;
  }
  .serial {
    color: var(--t3);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .head :global(h3) {
    font-weight: 500;
  }
  .panel {
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
    overflow: hidden;
  }
  .scroll {
    overflow-x: auto;
  }
  table {
    width: 100%;
    min-width: 760px;
    table-layout: fixed;
    border-collapse: collapse;
    font-size: 13px;
    line-height: 18px;
  }
  th {
    padding: 12px 14px 10px;
    text-align: left;
    font: 500 10.5px var(--font-mono);
    line-height: 14px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t3);
    white-space: nowrap;
  }
  td {
    height: 44px;
    padding: 0 14px;
    vertical-align: middle;
    color: var(--t1);
    box-shadow: inset 0 1px 0 var(--hair);
    transition: background-color var(--dur-fast) ease-out;
  }
  tbody tr.hot td {
    background: rgba(255, 255, 255, 0.04);
  }
  .w-ch {
    width: 56px;
  }
  .w-type {
    width: 30%;
  }
  .w-name {
    width: 27%;
  }
  .w-model {
    width: 84px;
  }
  .w-id {
    width: 150px;
  }
  .w-color {
    width: 230px;
  }
  .w-leds {
    width: 80px;
  }
  .w-src {
    width: 150px;
  }
  .w-range {
    width: 150px;
  }
  .w-rpm {
    width: 150px;
  }
  .r {
    text-align: right;
  }
  .strong {
    font-weight: 600;
  }
  .t2 {
    color: var(--t2);
  }
  .t3 {
    color: var(--t3);
  }
  .mono {
    font-size: 12px;
    white-space: nowrap;
  }
  .type {
    color: var(--t2);
  }

  .name {
    overflow: hidden;
  }
  .bsw {
    display: inline-block;
    vertical-align: middle;
    width: 12px;
    height: 12px;
    margin-right: 4px;
    border-radius: 4px;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.3);
  }
  .dg {
    animation: dg-in 320ms var(--ease-out-expo) both;
  }
  @keyframes dg-in {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dg {
      animation: none;
    }
  }
  .cname {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-weight: 500;
  }
  .sw {
    width: 16px;
    height: 16px;
    border-radius: 6px;
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.22),
      0 2px 4px -1px rgb(0 0 0 / 0.55);
  }
  .src {
    padding: 2px 6px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--t2);
    font-size: 11px;
  }
  .unhealthy {
    color: var(--warn-text);
    font-weight: 500;
  }
  tbody tr.skipped td {
    color: var(--t3);
  }
  .type {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename-trigger {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
    margin: 0 -8px;
    padding: 4px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--t1);
    font-weight: 500;
    cursor: text;
    text-align: left;
    transition: background-color var(--dur-fast) ease-out;
  }
  .rename-trigger:hover {
    background: var(--mat-raised);
  }
  .rename-trigger:active {
    background: var(--mat-pressed);
  }
  .rename-trigger:focus-visible {
    box-shadow: var(--focus-ring);
  }
  .rename-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename-trigger :global(.pencil) {
    flex: none;
    color: var(--t3);
    transition: color var(--dur-fast) ease-out;
  }
  .rename-trigger:hover :global(.pencil),
  .rename-trigger:focus-visible :global(.pencil) {
    color: var(--accent-strong);
  }
  .edit {
    position: relative;
    display: block;
    margin: 0 -4px;
  }
  .rename-input {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.3);
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
    color: var(--t1);
    font-size: 13px;
  }
  .rename-input::placeholder {
    color: var(--t3);
  }
  .rename-input:disabled {
    opacity: 0.6;
  }
  .busy {
    position: absolute;
    right: 10px;
    top: 8px;
    color: var(--t2);
    display: grid;
  }

  .rpm {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
  }
  .rbar {
    width: 56px;
  }
  .rpm :global(.rnum) {
    min-width: 44px;
    text-align: right;
    font-weight: 600;
  }
</style>
