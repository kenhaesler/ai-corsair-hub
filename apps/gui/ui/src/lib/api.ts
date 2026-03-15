import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DeviceTree, RgbConfig, SystemSnapshot } from './types';

export const getSnapshot = () => invoke<SystemSnapshot>('get_snapshot');
export const getDevices = () => invoke<DeviceTree>('get_devices');
export const getConfig = () => invoke<AppConfig>('get_config');
export const saveConfig = (config: AppConfig) => invoke('save_config', { config });
export const applyPreset = (preset: string) => invoke('apply_preset', { preset });
export const setManualDuty = (hubSerial: string, channels: number[], duty: number) =>
  invoke('set_manual_duty', { hub_serial: hubSerial, channels, duty });
export const validateConfig = (config: AppConfig) => invoke('validate_config', { config });
export const setRgbConfig = (config: RgbConfig) => invoke('set_rgb_config', { config });
export const setRgbEnabled = (enabled: boolean) => invoke('set_rgb_enabled', { enabled });
