<script lang="ts">
  import type { RgbFrameDto, RgbZoneConfig, RgbDeviceRef } from '../../lib/types';
  import { displayName, isDeviceRefV2 } from '../../lib/identity';
  import { configStore } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import FanPreview from './FanPreview.svelte';

  interface Props {
    zone: RgbZoneConfig | null;
    frames: RgbFrameDto[];
  }

  let { zone, frames }: Props = $props();

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
        <div class="device-slot">
          <!-- Assume fan ring for channels on hubs; strip detection would come from enumeration -->
          <FanPreview
            leds={leds}
            label={labelForRef(device)}
            size={120}
          />
        </div>
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
