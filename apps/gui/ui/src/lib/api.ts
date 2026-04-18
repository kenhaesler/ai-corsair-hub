import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DeviceTree, RgbConfig, SystemSnapshot } from './types';

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
export const saveConfig = (config: AppConfig) => invoke('save_config', { config });
export const applyPreset = (preset: string) => invoke('apply_preset', { preset });

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
