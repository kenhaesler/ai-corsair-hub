/**
 * Device identity helpers.
 *
 * device_id is the stable, factory-burned identity for every iCUE LINK
 * device. Channels are a daisy-chain detail that renumbers whenever the
 * physical topology changes; hub_serial is orientation, not identity.
 *
 * These helpers centralise the logic for turning a device_id into a string
 * the user can read. The precedence rules are intentionally conservative:
 * prefer the user's own label, fall back to a location-derived label, and
 * only as a last resort expose the raw device_id prefix. An orphaned
 * device (one referenced by config but not currently enumerated) renders
 * as an 8-char id prefix with a hint at the call site that it's offline.
 */

import type { AppConfig, SystemSnapshot, HubSnapshot, HubDeviceEntry } from './types';

/**
 * Return a human-readable label for a device identified by `device_id`.
 *
 * Precedence:
 *   1. User-set friendly name from `config.devices[]`, if present and non-empty.
 *   2. Location-derived label ("Hub XXXX Ch N") from the latest snapshot, if
 *      the device is currently enumerated. XXXX is the last 4 hex chars of
 *      `hub_serial`, giving visual differentiation between the user's two
 *      identically-modelled hubs without taking up a full 32-char serial.
 *   3. Device id prefix fallback ("Device 0100224A") when neither a name
 *      nor a current location is available (orphaned device).
 */
export function displayName(
  device_id: string,
  ctx: { config: AppConfig | null; snapshot: SystemSnapshot | null | undefined }
): string {
  if (!device_id) return 'Unknown device';

  // 1. User-set friendly name
  const entry = ctx.config?.devices?.find((d) => d.device_id === device_id);
  if (entry?.name && entry.name.trim().length > 0) {
    return entry.name;
  }

  // 2. Location-derived label. The snapshot carries a `fans` list but that
  //    doesn't include every device type (it omits strips, pumps etc.). So
  //    we walk the broader device tree if the caller passed one; for now
  //    we fall back to the fans list in the snapshot which does carry
  //    device_id populated by the backend.
  const fan = ctx.snapshot?.fans.find((f) => f.device_id === device_id);
  if (fan && fan.hub_serial) {
    return `Hub ${shortHubSerial(fan.hub_serial)} Ch ${fan.channel}`;
  }

  // 3. Orphaned device fallback
  return `Device ${shortDeviceId(device_id)}`;
}

/**
 * Same precedence as {@link displayName} but takes a pre-built device tree
 * as the location source instead of the snapshot's fan list. Use this when
 * rendering strip/pump devices that don't appear under `snapshot.fans`.
 */
export function displayNameFromTree(
  device_id: string,
  ctx: { config: AppConfig | null; hubs: HubSnapshot[] }
): string {
  if (!device_id) return 'Unknown device';

  const entry = ctx.config?.devices?.find((d) => d.device_id === device_id);
  if (entry?.name && entry.name.trim().length > 0) {
    return entry.name;
  }

  for (const hub of ctx.hubs) {
    const dev = hub.devices.find((d: HubDeviceEntry) => d.device_id === device_id);
    if (dev) {
      return `Hub ${shortHubSerial(hub.serial)} Ch ${dev.channel}`;
    }
  }

  return `Device ${shortDeviceId(device_id)}`;
}

/**
 * Last 4 hex chars of a hub serial — a short, human-comparable handle for
 * a hub. Hub serials are 32-char hex strings; the last 4 are sufficient to
 * distinguish any realistic number of hubs a single user will have.
 */
export function shortHubSerial(hub_serial: string): string {
  if (!hub_serial) return '????';
  return hub_serial.slice(-4).toUpperCase();
}

/**
 * First N chars of a device_id for compact orphan display.
 */
export function shortDeviceId(device_id: string, len = 8): string {
  if (!device_id) return '????????';
  return device_id.slice(0, len).toUpperCase();
}

/**
 * Is this fan group configured in V2 form (device_ids populated)?
 * Mirrors the backend's `FanGroupConfig.device_ids.is_empty()` check.
 */
export function isFanGroupV2(group: { device_ids?: string[] }): boolean {
  return Array.isArray(group.device_ids) && group.device_ids.length > 0;
}

/**
 * Does this zone device reference carry a V2 identity (non-empty device_id)?
 */
export function isDeviceRefV2(ref: { device_id?: string }): boolean {
  return typeof ref.device_id === 'string' && ref.device_id.length > 0;
}
