import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DeviceTree, FanPresetDef, RgbConfig, SystemSnapshot } from './types';

/**
 * Result of `set_manual_duty_by_device_id` — mirrors the Rust
 * `ManualDutyResult` DTO. The caller gets a per-id breakdown so a mixed
 * success/unresolved response isn't hidden as a blanket error.
 */
export interface ManualDutyResult {
  applied: string[];
  unresolved: string[];
}

export const getSnapshot = () => invoke<SystemSnapshot>('get_snapshot');
export const getDevices = () => invoke<DeviceTree>('get_devices');
export const getConfig = () => invoke<AppConfig>('get_config');

/**
 * Names of sensors currently initialized in the control loop. Returned
 * values are identifiers the control loop's weighted-fusion logic
 * understands (e.g. `cpu`, `gpu`, `psu_vrm`, `psu_case`). Used to
 * populate the per-group sensor-weighting chip picker so we never
 * offer a sensor the machine doesn't have.
 */
export const listAvailableSensors = () => invoke<string[]>('list_available_sensors');
export const saveConfig = (config: AppConfig) => invoke('save_config', { config });
/** Apply a curated preset to every configured fan group. */
export const applyPreset = (preset: string) => invoke('apply_preset', { preset });

/**
 * Apply a curated preset to a single named group, or to every group
 * when `groupName` is omitted/null. Introduced in v0.3 for per-group
 * preset control (e.g. "Competitive FPS" on radiators while chassis
 * fans stay on "Quiet Productivity").
 */
export const applyPresetToGroup = (preset: string, groupName: string | null = null) =>
  invoke('apply_preset_to_group', { preset, groupName });

/**
 * Return the full curated fan-preset library. The library is a
 * compile-time asset so this is effectively instant — callers can
 * fetch once at mount and cache in component state.
 */
export const getFanPresets = () => invoke<FanPresetDef[]>('get_fan_presets');

/**
 * @deprecated Use {@link setManualDutyByDeviceId}; channel-based
 * addressing is fragile across topology changes. Retained for one
 * release so the unmigrated UI keeps working.
 */
export const setManualDuty = (hubSerial: string, channels: number[], duty: number) =>
  invoke('set_manual_duty', { hubSerial, channels, duty });

/** V2 duty command: resolves each device_id via the live registry. */
export const setManualDutyByDeviceId = (deviceIds: string[], duty: number) =>
  invoke<ManualDutyResult>('set_manual_duty_by_device_id', { deviceIds, duty });

/**
 * Rename (or clear) a device's friendly name by its stable device_id.
 * Empty `name` clears the entry, reverting to the system-generated
 * fallback label. Persisted atomically on the backend.
 */
export const renameDevice = (deviceId: string, name: string) =>
  invoke('rename_device', { deviceId, name });

export const validateConfig = (config: AppConfig) => invoke('validate_config', { config });
export const setRgbConfig = (config: RgbConfig) => invoke('set_rgb_config', { config });
export const setRgbEnabled = (enabled: boolean) => invoke('set_rgb_enabled', { enabled });

/**
 * One row in the channel-identify diagnostic mapping returned by
 * `rgbIdentifyChannels`. Mirrors the Rust `ChannelIdentity` DTO.
 */
export interface ChannelIdentity {
  hub_serial: string;
  channel: number;
  device_id: string;
  device_type: string;
  color_name: string;
  rgb: [number, number, number];
}

/**
 * Paint every enumerated RGB device a distinct colour for
 * `durationSecs` seconds so the user can correlate physical
 * positions in their rig with channel numbers. Returns the colour
 * mapping so the UI can render a legend. Requires `rgb.enabled`
 * and `rgb.hardware_output` in the config to actually light LEDs.
 */
export const rgbIdentifyChannels = (durationSecs: number) =>
  invoke<ChannelIdentity[]>('rgb_identify_channels', { durationSecs });
