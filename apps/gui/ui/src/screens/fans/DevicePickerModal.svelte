<!--
  DevicePickerModal (inventory 0.5). Used by "Edit devices" and NewGroupDialog.
  - title from prop; "N selected"; "All online" (adds every enumerated id,
    keeps orphans) and "Clear"
  - rows grouped "Hub <i+1> · <last4>"; each row checkbox,
    "<device_type> · <displayNameFromTree>" and a "Ch N" tag
  - orphan section "Offline (referenced but not enumerated)" with
    "Device <8-char id> (offline)" rows, checkable so they can be removed
  - empty state copy; footer Cancel / "Save (N)"; Esc + backdrop cancel
  - draft is a local copy reset on every open
-->
<script lang="ts">
  import Sheet from '../../components/ui/Sheet.svelte';
  import Button from '../../components/ui/Button.svelte';
  import Checkbox from '../../components/ui/Checkbox.svelte';
  import Unplug from 'lucide-svelte/icons/unplug';
  import Fan from 'lucide-svelte/icons/fan';
  import Cable from 'lucide-svelte/icons/cable';
  import { displayNameFromTree, shortDeviceId, shortHubSerial } from '../../lib/identity';
  import type { AppConfig, HubSnapshot } from '../../lib/types';

  interface Props {
    open: boolean;
    title: string;
    /** the group being edited (its own membership is not "owned by another group") */
    groupName?: string;
    selected: string[];
    availableHubs: HubSnapshot[];
    config: AppConfig | null;
    onsave: (deviceIds: string[]) => void;
    oncancel: () => void;
  }
  let { open = $bindable(), title, groupName, selected, availableHubs, config, onsave, oncancel }: Props = $props();
  const ownerOf = (id: string) => config?.fan_groups.find((g) => g.name !== groupName && (g.device_ids ?? []).includes(id))?.name ?? null;

  let draft = $state<string[]>([]);
  $effect(() => {
    if (open) draft = [...selected];
  });

  interface Row {
    device_id: string;
    channel: number;
    label: string;
    device_type: string;
    enumerated: boolean;
    /** another fan group that already lists this device */
    ownedBy: string | null;
  }
  interface Group {
    key: string;
    label: string;
    serial: string | null;
    rows: Row[];
  }

  const enumeratedIds = $derived(
    new Set(availableHubs.flatMap((h) => h.devices.map((d) => d.device_id).filter(Boolean))),
  );

  const groups = $derived.by<Group[]>(() => {
    const out: Group[] = [];
    availableHubs.forEach((hub, i) => {
      const rows: Row[] = hub.devices
        .filter((d) => d.device_id)
        .map((d) => ({
          device_id: d.device_id,
          channel: d.channel,
          device_type: d.device_type,
          label: `${d.device_type} · ${displayNameFromTree(d.device_id, { config, hubs: availableHubs })}`,
          enumerated: true,
          ownedBy: ownerOf(d.device_id),
        }));
      if (rows.length) out.push({ key: hub.serial, label: `Hub ${i + 1} · ${shortHubSerial(hub.serial)}`, serial: hub.serial, rows });
    });
    return out;
  });

  // Orphan rows stay listed after being unchecked so the choice can be undone.
  let orphanMemo = $state<string[]>([]);
  $effect(() => {
    if (open) orphanMemo = selected.filter((id) => !enumeratedIds.has(id));
  });
  const shown = $derived.by<Group[]>(() => {
    const g = [...groups];
    const ids = [...new Set([...orphanMemo, ...draft.filter((id) => !enumeratedIds.has(id))])];
    if (ids.length)
      g.push({
        key: '__orphans',
        label: 'Offline (referenced but not enumerated)',
        serial: null,
        rows: ids.map((id) => ({
          device_id: id,
          channel: 0,
          device_type: 'unknown',
          label: `Device ${shortDeviceId(id)} (offline)`,
          ownedBy: ownerOf(id),
          enumerated: false,
        })),
      });
    return g;
  });

  function toggle(id: string, on: boolean) {
    draft = on ? (draft.includes(id) ? draft : [...draft, id]) : draft.filter((d) => d !== id);
  }
  function allOnline() {
    const ids = new Set(draft);
    for (const id of enumeratedIds) ids.add(id);
    draft = [...ids];
  }
  function clear() {
    draft = [];
  }
  function groupState(g: Group): { all: boolean; some: boolean } {
    const n = g.rows.filter((r) => draft.includes(r.device_id)).length;
    return { all: n === g.rows.length && n > 0, some: n > 0 && n < g.rows.length };
  }
  function toggleGroup(g: Group, on: boolean) {
    const ids = g.rows.map((r) => r.device_id);
    draft = on ? [...new Set([...draft, ...ids])] : draft.filter((d) => !ids.includes(d));
  }
  const isFan = (t: string) => /fan|qx|rx|lx/i.test(t);
</script>

<Sheet bind:open {title} width={520} onclose={oncancel}>
  <div class="bar">
    <span class="count num" aria-live="polite">{draft.length} selected</span>
    <div class="bar-actions">
      <Button variant="plain" size="sm" onclick={allOnline} disabled={enumeratedIds.size === 0}>All online</Button>
      <Button variant="plain" size="sm" onclick={clear} disabled={draft.length === 0}>Clear</Button>
    </div>
  </div>

  {#if shown.length === 0}
    <div class="empty">
      <span class="empty-ic" aria-hidden="true"><Unplug size={20} strokeWidth={1.75} /></span>
      <p class="t-callout">No hubs currently enumerated. Reconnect the iCUE LINK hub to populate the list.</p>
    </div>
  {:else}
    <div class="groups">
      {#each shown as g (g.key)}
        {@const st = groupState(g)}
        <section class="grp" class:orphans={!g.serial} aria-label={g.label}>
          <div class="grp-head">
            <h3 class="grp-title">{g.label}</h3>
            {#if g.serial}
              <Checkbox
                checked={st.all}
                indeterminate={st.some}
                label="Select all on {g.label}"
                onchange={(v) => toggleGroup(g, v)}
              />
            {/if}
          </div>
          <div class="rows">
            {#each g.rows as r (r.device_id)}
              <div class="row" class:on={draft.includes(r.device_id)} class:offline={!r.enumerated}>
                <Checkbox checked={draft.includes(r.device_id)} onchange={(v) => toggle(r.device_id, v)}>
                  <span class="row-inner">
                    <span class="dev-ic" aria-hidden="true">
                      {#if !r.enumerated}
                        <Unplug size={13} strokeWidth={2} />
                      {:else if isFan(r.device_type)}
                        <Fan size={13} strokeWidth={2} />
                      {:else}
                        <Cable size={13} strokeWidth={2} />
                      {/if}
                    </span>
                    <span class="lbl">{r.label}</span>
                    {#if r.ownedBy}<span class="owner" title="Already in {r.ownedBy}; a device follows the first group that lists it">in {r.ownedBy}</span>{/if}
                    {#if r.enumerated}<span class="ch num">Ch {r.channel}</span>{/if}
                  </span>
                </Checkbox>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}

  {#snippet footer()}
    <Button variant="secondary" onclick={oncancel}>Cancel</Button>
    <Button variant="primary" onclick={() => onsave([...draft])}>Save ({draft.length})</Button>
  {/snippet}
</Sheet>

<style>
  .bar {
    position: sticky;
    top: -12px;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: -12px -24px 8px;
    padding: 10px 24px 10px;
    background: linear-gradient(var(--mat-sheet) 70%, transparent);
  }
  .count {
    font-size: 13px;
    font-weight: 600;
    color: var(--t2);
  }
  .bar-actions {
    display: flex;
    gap: 4px;
    margin-right: -8px;
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .grp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 14px 8px 4px;
  }
  .grp-title {
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--t2);
    font-variant-numeric: tabular-nums;
  }
  .orphans .grp-title {
    color: var(--warn-text);
  }
  .rows {
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    overflow: hidden;
  }
  .row + .row {
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .row {
    transition: background-color var(--dur-fast) ease-out;
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .row.on {
    background: var(--accent-softer);
  }
  .row :global(.cb) {
    display: flex;
    width: 100%;
    min-height: 40px;
    padding: 0 14px;
  }
  .row :global(.cb .text) {
    flex: 1;
  }
  .row-inner {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .dev-ic {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
    color: var(--t2);
  }
  .owner {
    flex: none;
    font: 500 10.5px var(--font-mono);
    letter-spacing: 0.04em;
    color: var(--t3);
    padding: 2px 6px;
    border-radius: 5px;
    box-shadow: inset 0 0 0 1px var(--hair);
    white-space: nowrap;
  }
  .offline .dev-ic {
    background: var(--warn-surface);
    color: var(--warn);
  }
  .lbl {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .offline .lbl {
    color: var(--t2);
  }
  .ch {
    flex: none;
    height: 20px;
    padding: 0 7px;
    display: inline-flex;
    align-items: center;
    border-radius: var(--r-chip);
    background: rgba(255, 255, 255, 0.07);
    color: var(--t2);
    font-size: 11px;
    font-weight: 600;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 28px 16px 20px;
    text-align: center;
    color: var(--t2);
  }
  .empty p {
    max-width: 40ch;
  }
  .empty-ic {
    width: 44px;
    height: 44px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
  }
</style>
