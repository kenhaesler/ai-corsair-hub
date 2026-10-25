<script lang="ts">
  import type { RgbFrameDto, RgbZoneConfig, RgbDeviceRef, HubSnapshot, HubDeviceEntry, StripSegment } from '../../lib/types';
  import { displayName, isDeviceRefV2 } from '../../lib/identity';
  import { configStore } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import FanPreview from './FanPreview.svelte';
  import StripPreview from './StripPreview.svelte';

  interface Props {
    zone: RgbZoneConfig | null;
    frames: RgbFrameDto[];
    /** Device tree; used to pick a renderer per device type. Optional. */
    hubs?: HubSnapshot[];
  }

  let { zone, frames, hubs = [] }: Props = $props();

  function getFrameForDevice(ref: RgbDeviceRef): [number, number, number][] | null {
    // Prefer matching by device_id (stable). Fall back to (hub_serial,
    // channel) for V1 entries and legacy frames that lack device_id.
    if (isDeviceRefV2(ref) && ref.device_id) {
      const byId = frames.find((f) => f.device_id === ref.device_id);
      if (byId) return byId.leds;
    }
    const byLoc = frames.find(
      (f) => f.hub_serial === ref.hub_serial && f.channel === ref.channel,
    );
    return byLoc?.leds ?? null;
  }

  function findDevice(ref: RgbDeviceRef): HubDeviceEntry | null {
    for (const hub of hubs) {
      for (const d of hub.devices) {
        if (isDeviceRefV2(ref) && ref.device_id) {
          if (d.device_id === ref.device_id) return d;
        } else if (hub.serial === ref.hub_serial && d.channel === ref.channel) {
          return d;
        }
      }
    }
    return null;
  }

  /** Fans (and unknown types, which keep the legacy look) use the ring. */
  function isFan(dev: HubDeviceEntry | null): boolean {
    return !dev || /fan/i.test(dev.device_type);
  }

  function segmentLeds(
    leds: [number, number, number][] | null,
    seg: StripSegment,
  ): [number, number, number][] | null {
    return leds ? leds.slice(seg.led_start, seg.led_start + seg.led_count) : null;
  }

  function labelForRef(ref: RgbDeviceRef): string {
    if (isDeviceRefV2(ref) && ref.device_id) {
      return displayName(ref.device_id, {
        config: configStore.config,
        snapshot: sensors.snapshot,
      });
    }
    return `Ch ${ref.channel}`;
  }
</script>

<div class="hardware-preview">
  {#if zone && zone.devices.length > 0}
    <div class="device-grid">
      {#each zone.devices as device}
        {@const leds = getFrameForDevice(device)}
        {@const dev = findDevice(device)}
        {@const label = labelForRef(device)}
        {#if isFan(dev)}
          <div class="device-slot">
            <FanPreview {leds} {label} size={120} />
          </div>
        {:else if dev?.strips && dev.strips.length > 0}
          {#each dev.strips as seg (seg.index)}
            <div class="device-slot">
              <StripPreview
                leds={segmentLeds(leds, seg)}
                label={`${label} · Strip ${seg.index + 1} (${seg.led_count} LEDs)`}
              />
            </div>
          {/each}
        {:else}
          <div class="device-slot">
            <StripPreview {leds} {label} />
          </div>
        {/if}
      {/each}
    </div>
  {:else}
    <div class="empty-preview">
      <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1">
        <circle cx="12" cy="12" r="10" />
        <path d="M12 6v6l3 3" />
      </svg>
      <p>Add devices to this zone to preview</p>
      <p class="hint">Assign hub channels from the zone settings</p>
    </div>
  {/if}
</div>

<style>
  .hardware-preview {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .device-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    justify-content: center;
    align-items: center;
  }
  .device-slot {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .empty-preview {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
    text-align: center;
  }
  .empty-preview p {
    font-size: 12px;
    margin: 0;
  }
  .empty-preview .hint {
    font-size: 11px;
    color: var(--text-muted);
  }
</style>
