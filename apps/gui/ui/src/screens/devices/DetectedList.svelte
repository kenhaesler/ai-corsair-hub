<!--
  "All Corsair devices" (inventory 3.6): one row per DetectedDevice, with the
  family glyph, name, PID / S/N in mono, support badge, status badge (when not
  ok) and a Bootloader badge. Footnote: filtered-interface count. The S/N of a
  device already shown as a hub section or the PSU card is not repeated here
  (a hub row carries a "Hub XXXX" cross-reference instead).
-->
<script lang="ts">
  import Microchip from 'lucide-svelte/icons/microchip';
  import Power from 'lucide-svelte/icons/power';
  import Keyboard from 'lucide-svelte/icons/keyboard';
  import Mouse from 'lucide-svelte/icons/mouse';
  import Headphones from 'lucide-svelte/icons/headphones';
  import HubGlyph from './HubGlyph.svelte';
  import BluetoothOff from 'lucide-svelte/icons/bluetooth-off';
  import type { DetectedDevice } from '../../lib/types';
  import { shortHubSerial } from '../../lib/identity';
  import type { IconComponent } from '../../lib/icon';
  import Badge from '../../components/ui/Badge.svelte';

  interface Props {
    devices: DetectedDevice[];
    excluded: number;
    /** serials already shown in the hub sections / PSU card above */
    hubSerials: string[];
    psuSerial: string | null;
  }
  let { devices, excluded, hubSerials, psuSerial }: Props = $props();

  const SUPPORT_LABEL: Record<string, string> = {
    supported: 'Supported',
    detect_only: 'Detected - no driver',
    unknown: 'Unknown',
  };
  const STATUS_LABEL: Record<string, string> = {
    not_ready: 'Not ready',
    quarantined: 'Quarantined',
    backoff: 'Retrying',
  };
  const supportLabel = (d: DetectedDevice) => SUPPORT_LABEL[d.support] ?? 'Unknown';
  const pidHex = (d: DetectedDevice) => `0x${d.pid.toString(16).toUpperCase().padStart(4, '0')}`;

  function iconFor(d: DetectedDevice): IconComponent {
    const f = d.family.toLowerCase();
    if (f.includes('keyboard')) return Keyboard;
    if (f.includes('mouse')) return Mouse;
    if (f.includes('headset') || f.includes('audio')) return Headphones;
    if (f.includes('psu')) return Power;
    if (f.includes('link')) return HubGlyph as unknown as IconComponent;
    return Microchip;
  }
</script>

<ul class="list">
  {#each devices as d (d.key)}
    {@const Icon = iconFor(d)}
    <li class="row">
      <span class="tile" class:muted={d.support !== 'supported'} aria-hidden="true"><Icon size={16} strokeWidth={2} /></span>
      <div class="main">
        <span class="name t-headline">{d.name}</span>
        <span class="meta">
          <span class="mono">PID {pidHex(d)}</span>
          {#if d.serial && hubSerials.includes(d.serial)}
            <span class="mono">Hub {shortHubSerial(d.serial)}</span>
          {:else if d.serial && d.serial !== psuSerial}
            <span class="mono">S/N {d.serial}</span>
          {/if}
        </span>
      </div>
      <div class="badges">
        <Badge tone={d.support === 'supported' ? 'success' : d.support === 'detect_only' ? 'info' : 'neutral'} dashed={d.support === 'unknown'}>{supportLabel(d)}</Badge>
        {#if d.status !== 'ok'}
          <Badge tone={d.status === 'quarantined' ? 'error' : 'warn'}>{STATUS_LABEL[d.status] ?? d.status}</Badge>
        {/if}
        {#if d.bootloader}
          <Badge tone="warn">Bootloader</Badge>
        {/if}
      </div>
    </li>
  {/each}
</ul>
{#if excluded > 0}
  <p class="note t-callout">
    <BluetoothOff size={14} strokeWidth={2} />
    <span><span class="num">{excluded}</span> interfaces filtered (Bluetooth / driver problem)</span>
  </p>
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 56px;
    padding: 8px var(--sp-4);
    transition: background-color var(--dur-fast) ease-out;
  }
  .row + .row {
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .tile {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: var(--accent-strong);
    background: var(--accent-softer);
    box-shadow: inset 0 0 0 1px var(--info-hair);
  }
  .tile.muted {
    color: var(--t2);
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 1px var(--hair);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name {
    color: var(--t1);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--sp-3);
  }
  .meta .mono {
    font-size: 11px;
    line-height: 16px;
    color: var(--t3);
    overflow-wrap: anywhere;
  }
  .badges {
    flex: none;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: var(--sp-3);
    padding: 0 4px;
    color: var(--t3);
  }
</style>
