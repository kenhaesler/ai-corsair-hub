<!--
  Zones (inventory 4.3): zone list (name + "N dev"; click selects;
  double-click or F2 renames inline, Enter/blur commits when non-empty,
  Escape cancels; × "Delete zone" only when there is more than one zone,
  behind a confirm), "+ Add Zone", and ONE device list for the selected
  zone:
  - devices in the zone first, in flow order (drag handle, ↑/↓), each with
    a checked checkbox and a "Hub N" tag, because flow order runs across
    hubs and cannot also be grouped by hub;
  - then "Available": the remaining devices grouped "Hub N · XXXX".
  Every row is "<deviceTypeLabel> — <displayNameFromTree>"; the checkbox
  toggles membership (device_id, fallback hub_serial+channel). "All"
  ("Add all"), "Clear" ("Clear all"), "No hubs detected".
  With no zones, the Zones group holds the "No zones configured" state.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Plus from 'lucide-svelte/icons/plus';
  import X from 'lucide-svelte/icons/x';
  import Cable from 'lucide-svelte/icons/cable';
  import Palette from 'lucide-svelte/icons/palette';
  import ArrowUp from 'lucide-svelte/icons/arrow-up';
  import ArrowDown from 'lucide-svelte/icons/arrow-down';
  import GripVertical from 'lucide-svelte/icons/grip-vertical';
  import { flip } from 'svelte/animate';
  import { cubicOut } from 'svelte/easing';
  import { reducedMotion } from '../../lib/motion.svelte';
  import Button from '../../components/ui/Button.svelte';
  import IconButton from '../../components/ui/IconButton.svelte';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import Checkbox from '../../components/ui/Checkbox.svelte';
  import ConfirmDialog from '../../components/ui/ConfirmDialog.svelte';
  import EmptyState from '../../components/ui/EmptyState.svelte';
  import { rgb } from '../../lib/stores/rgb.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { deviceTypeLabel, displayNameFromTree, shortDeviceId, shortHubSerial } from '../../lib/identity';
  import type { HubDeviceEntry, HubSnapshot, RgbDeviceRef } from '../../lib/types';
  import { addZone, deleteZone, lighting, renameZone, selectZone } from './state.svelte';
  import { allDeviceRefs, refKey, refMatches, resolveRef } from './effects';

  interface Props {
    hubs: HubSnapshot[];
    onsave: () => void;
  }
  let { hubs, onsave }: Props = $props();

  const zones = $derived(rgb.config?.zones ?? []);
  const zone = $derived(zones[rgb.selectedZone] ?? null);

  // ---- rename ----
  let editing = $state<number | null>(null);
  let draft = $state('');
  let input = $state<HTMLInputElement>();
  async function startRename(i: number) {
    editing = i;
    draft = zones[i].name;
    await tick();
    input?.focus();
    input?.select();
  }
  function commitRename() {
    if (editing === null) return;
    const i = editing;
    editing = null;
    if (draft.trim()) renameZone(i, draft);
  }

  // ---- delete ----
  let confirmIdx = $state<number | null>(null);
  let confirmOpen = $state(false);
  function askDelete(i: number) {
    confirmIdx = i;
    confirmOpen = true;
  }

  // ---- membership ----
  const name = (id: string) => displayNameFromTree(id, { config: configStore.config, hubs });
  const inZone = (hub: HubSnapshot, d: HubDeviceEntry) => !!zone?.devices.some((r) => refMatches(r, hub.serial, d));
  function toggle(hub: HubSnapshot, d: HubDeviceEntry, on: boolean) {
    if (!zone) return;
    if (on) {
      if (!inZone(hub, d)) zone.devices.push({ hub_serial: hub.serial, channel: d.channel, device_id: d.device_id });
    } else {
      zone.devices = zone.devices.filter((r) => !refMatches(r, hub.serial, d));
    }
    onsave();
  }
  function addAll() {
    if (!zone) return;
    zone.devices = allDeviceRefs(hubs);
    onsave();
  }
  function clearAll() {
    if (!zone) return;
    zone.devices = [];
    onsave();
  }

  // ---- ordered members + available ----
  interface Member {
    ref: RgbDeviceRef;
    key: string;
    label: string;
    hubTag: string | null;
    offline: boolean;
  }
  const members = $derived.by((): Member[] => {
    if (!zone) return [];
    return zone.devices.map((ref) => {
      const { dev, hubIndex } = resolveRef(ref, hubs);
      let label: string;
      if (dev) label = `${deviceTypeLabel(dev)} — ${name(dev.device_id)}`;
      else if (ref.device_id) label = `Device ${shortDeviceId(ref.device_id)} (offline)`;
      else label = `Ch ${ref.channel} (offline)`;
      return { ref, key: refKey(ref), label, hubTag: hubIndex >= 0 ? `Hub ${hubIndex + 1}` : null, offline: !dev };
    });
  });
  const available = $derived(
    hubs
      .map((hub, hi) => ({ hub, hi, devices: hub.devices.filter((d) => !inZone(hub, d)) }))
      .filter((g) => g.devices.length > 0),
  );
  function removeAt(i: number) {
    if (!zone) return;
    zone.devices.splice(i, 1);
    lighting.hoverKey = null;
    onsave();
  }
  function move(from: number, to: number) {
    if (!zone || to < 0 || to >= zone.devices.length || from === to) return;
    const [m] = zone.devices.splice(from, 1);
    zone.devices.splice(to, 0, m);
    onsave();
  }
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

</script>

<div class="col">
  <InsetGroup title="Zones">
    {#snippet action()}
      {#if zones.length > 0}
        <Button variant="plain" size="sm" icon={Plus} onclick={addZone}><span class="sr-only">{'+ '}</span>Add Zone</Button>
      {/if}
    {/snippet}
    {#if zones.length === 0}
      <div class="empty">
        <EmptyState icon={Palette} title="No zones configured" message="A zone groups devices that share one stack of effects." compact>
          {#snippet action()}
            <Button variant="primary" size="sm" icon={Plus} onclick={addZone}>Create Zone</Button>
          {/snippet}
        </EmptyState>
      </div>
    {/if}
    {#each zones as z, i (i)}
      <div class="zrow" class:sel={i === rgb.selectedZone}>
        {#if editing === i}
          <input
            bind:this={input}
            class="rename"
            type="text"
            aria-label="Zone name"
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
          <button
            type="button"
            class="zbtn"
            aria-current={i === rgb.selectedZone ? 'true' : undefined}
            title="Double-click to rename"
            onclick={() => selectZone(i)}
            ondblclick={() => startRename(i)}
            onkeydown={(e) => e.key === 'F2' && startRename(i)}
          >
            <span class="zname">{z.name}</span>
            <span class="zmeta num">{z.devices.length} dev</span>
          </button>
          {#if zones.length > 1}
            <span class="zdel">
              <IconButton icon={X} size="sm" label="Delete zone" onclick={() => askDelete(i)} />
            </span>
          {/if}
        {/if}
      </div>
    {/each}
  </InsetGroup>

  {#if zone}
    <InsetGroup title="Devices" footer="Checked devices run first to last in the order shown; drag a row or use the arrows. A device belongs to the first zone that lists it.">
      {#snippet action()}
        <div class="dact">
          <Button variant="secondary" size="sm" title="Add all" onclick={addAll} disabled={hubs.length === 0}>All</Button>
          <Button variant="secondary" size="sm" title="Clear all" onclick={clearAll} disabled={zone.devices.length === 0}>Clear</Button>
        </div>
      {/snippet}
      {#if hubs.length === 0}
        <div class="empty">
          <EmptyState icon={Cable} title="No hubs detected" message="Connect an iCUE LINK hub and its devices appear here." compact />
        </div>
      {:else}
        {#each members as m, i (m.key)}
          <div
            class="drow member"
            class:drag={dragFrom === i}
            class:over={dragOver === i && dragFrom !== null && dragFrom !== i}
            draggable="true"
            role="group"
            aria-label="Position {i + 1}"
            animate:flip={{ duration: reducedMotion.current ? 0 : 280, easing: cubicOut }}
            onmouseenter={() => (lighting.hoverKey = m.key)}
            onmouseleave={() => lighting.hoverKey === m.key && (lighting.hoverKey = null)}
            ondragstart={(e) => {
              dragFrom = i;
              e.dataTransfer?.setData('text/plain', String(i));
              if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
            }}
            ondragover={(e) => {
              e.preventDefault();
              dragOver = i;
            }}
            ondragleave={() => dragOver === i && (dragOver = null)}
            ondrop={(e) => {
              e.preventDefault();
              if (dragFrom !== null) move(dragFrom, i);
              dragFrom = dragOver = null;
            }}
            ondragend={() => (dragFrom = dragOver = null)}
          >
            <span class="grip" aria-hidden="true"><GripVertical size={14} strokeWidth={2} /></span>
            <span class="idx num" aria-hidden="true">{i + 1}</span>
            <Checkbox checked onchange={(v) => !v && removeAt(i)}>
              <span class="dlabel" class:offline={m.offline}>{m.label}</span>
            </Checkbox>
            {#if m.hubTag}<span class="tag">{m.hubTag}</span>{/if}
            <span class="oacts">
              <IconButton icon={ArrowUp} size="sm" label="Move earlier" disabled={i === 0} onclick={() => move(i, i - 1)} />
              <IconButton icon={ArrowDown} size="sm" label="Move later" disabled={i === members.length - 1} onclick={() => move(i, i + 1)} />
            </span>
          </div>
        {/each}
        {#if available.length > 0}
          <div class="avail-h">Available</div>
          {#each available as g (g.hub.serial)}
            <div class="hub"><span class="hub-name">Hub {g.hi + 1} · {shortHubSerial(g.hub.serial)}</span></div>
            {#each g.devices as d (d.device_id || d.channel)}
              <div class="drow">
                <Checkbox checked={false} onchange={(v) => v && toggle(g.hub, d, true)}>
                  <span class="dlabel">{deviceTypeLabel(d)} — {name(d.device_id)}</span>
                </Checkbox>
              </div>
            {/each}
          {/each}
        {/if}
      {/if}
    </InsetGroup>

  {/if}
</div>

<ConfirmDialog
  bind:open={confirmOpen}
  title={confirmIdx !== null && zones[confirmIdx] ? `Delete “${zones[confirmIdx].name}”?` : 'Delete zone?'}
  message="Its layers and flow settings are removed. The devices stay connected and can join another zone."
  confirmLabel="Delete Zone"
  destructive
  onconfirm={() => {
    if (confirmIdx !== null) deleteZone(confirmIdx);
    confirmOpen = false;
  }}
  oncancel={() => (confirmOpen = false)}
/>

<style>
  .col {
    display: flex;
    flex-direction: column;
    gap: 28px;
    min-width: 0;
  }
  .zrow {
    position: relative;
    display: flex;
    align-items: center;
    min-height: 44px;
    transition: background-color var(--dur-fast) ease-out;
  }
  .zrow:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .zrow.sel {
    background: var(--accent-soft);
  }
  .zbtn {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 48px 0 14px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .zbtn:focus-visible {
    box-shadow: inset var(--focus-ring);
  }
  .zname {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
    letter-spacing: -0.006em;
    color: var(--t1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .zmeta {
    font-size: 12px;
    color: var(--t3);
  }
  .sel .zmeta {
    color: var(--info-text);
  }
  .zdel {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    opacity: 0;
    transition: opacity var(--dur-fast) ease-out;
  }
  .zrow:hover .zdel,
  .zrow:focus-within .zdel,
  .zrow.sel .zdel {
    opacity: 1;
  }
  .rename {
    flex: 1;
    min-width: 0;
    height: 30px;
    margin: 0 8px 0 12px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.35);
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
    font-size: 14px;
    font-weight: 600;
    color: var(--t1);
  }

  .dact {
    display: flex;
    gap: 6px;
  }
  .empty {
    padding: 8px 0;
  }
  .hub {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 12px 16px 4px;
  }
  .hub-name {
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--t2);
  }
  .drow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    padding: 4px 16px;
  }
  .drow :global(.cb) {
    min-width: 0;
  }
  .dlabel {
    font-size: 13px;
    color: var(--t1);
  }
  .dlabel.offline {
    color: var(--warn-text);
  }
  .member {
    justify-content: flex-start;
    gap: 8px;
    padding: 2px 8px;
    cursor: grab;
    transition:
      background-color var(--dur-fast) ease-out,
      opacity var(--dur-fast) ease-out;
  }
  .member:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .member.drag {
    opacity: 0.4;
  }
  .member.over {
    background: var(--accent-softer);
  }
  .member :global(.cb) {
    flex: 1;
  }
  .grip {
    display: grid;
    color: var(--t4);
  }
  .member:hover .grip {
    color: var(--t3);
  }
  .idx {
    width: 18px;
    font-size: 12px;
    font-weight: 600;
    color: var(--t3);
    text-align: right;
  }
  .tag {
    flex: none;
    padding: 1px 6px;
    border-radius: var(--r-chip);
    background: rgba(255, 255, 255, 0.06);
    font-size: 11px;
    font-weight: 600;
    color: var(--t2);
  }
  .oacts {
    flex: none;
    display: flex;
    gap: 2px;
  }
  .avail-h {
    padding: 16px 16px 2px;
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--t1);
  }
  .avail-h + .hub {
    padding-top: 6px;
  }
</style>
