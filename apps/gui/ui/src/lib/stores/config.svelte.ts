import { getConfig, saveConfig as apiSaveConfig, validateConfig } from '../api';
import type { AppConfig, RgbConfig } from '../types';

function clone<T>(value: T): T {
  return structuredClone(value);
}

function serialized(value: AppConfig | null): string {
  return value ? JSON.stringify(value) : '';
}

class ConfigStore {
  config = $state<AppConfig | null>(null);
  error = $state<string | null>(null);
  loading = $state(false);
  loaded = $state(false);
  saving = $state(false);
  dirty = $state(false);
  baseline = $state.raw<AppConfig | null>(null);
}

export const configStore = new ConfigStore();

let loadPromise: Promise<AppConfig | null> | null = null;
let loadGeneration = 0;

export async function loadConfig(force = false): Promise<AppConfig | null> {
  if (!force && configStore.loaded) return configStore.config;
  if (!force && loadPromise) return loadPromise;

  const generation = ++loadGeneration;
  configStore.loading = true;
  configStore.error = null;
  const request = getConfig()
    .then((config) => {
      if (generation !== loadGeneration) return configStore.config;
      configStore.config = clone(config);
      configStore.baseline = clone(config);
      configStore.dirty = false;
      configStore.loaded = true;
      return configStore.config;
    })
    .catch((error) => {
      if (generation !== loadGeneration) return configStore.config;
      configStore.error = String(error);
      return null;
    })
    .finally(() => {
      if (generation !== loadGeneration) return;
      configStore.loading = false;
      if (loadPromise === request) loadPromise = null;
    });
  loadPromise = request;

  return request;
}

export function replaceConfig(config: AppConfig, markDirty = true) {
  configStore.config = config;
  if (markDirty) {
    configStore.dirty = serialized(config) !== serialized(configStore.baseline);
  }
}

export function discardConfigChanges() {
  if (!configStore.baseline) return;
  configStore.config = clone(configStore.baseline);
  configStore.dirty = false;
  configStore.error = null;
}

/** Keep RGB drafts in the authoritative app config so another save cannot persist stale lighting. */
export function setRgbDraft(rgb: RgbConfig) {
  if (!configStore.config) return;
  replaceConfig({ ...configStore.config, rgb: clone(rgb) });
}

/** Mark a successfully persisted RGB section as the new baseline without losing other dirty edits. */
export function acceptRgbConfig(rgb: RgbConfig, updateCurrent = true) {
  if (configStore.config && updateCurrent) {
    configStore.config = { ...configStore.config, rgb: clone(rgb) };
  }
  if (configStore.baseline) {
    configStore.baseline = { ...configStore.baseline, rgb: clone(rgb) };
  }
  configStore.dirty = serialized(configStore.config) !== serialized(configStore.baseline);
}


function withDeviceName(config: AppConfig, deviceId: string, name: string): AppConfig {
  const devices = clone(config.devices ?? []);
  const index = devices.findIndex((device) => device.device_id === deviceId);

  if (name) {
    if (index >= 0) devices[index] = { ...devices[index], name };
    else devices.push({ device_id: deviceId, name });
  } else if (index >= 0) {
    const updated = { ...devices[index] };
    delete updated.name;
    if (updated.led_count === undefined) devices.splice(index, 1);
    else devices[index] = updated;
  }

  return { ...config, devices };
}

export function acceptDeviceName(deviceId: string, name: string) {
  if (configStore.config) {
    configStore.config = withDeviceName(configStore.config, deviceId, name);
  }
  if (configStore.baseline) {
    configStore.baseline = withDeviceName(configStore.baseline, deviceId, name);
  }
  configStore.dirty = serialized(configStore.config) !== serialized(configStore.baseline);
}

export async function saveCurrentConfig(): Promise<boolean> {
  if (!configStore.config || configStore.saving) return false;
  const snapshot = clone(configStore.config);
  configStore.saving = true;
  configStore.error = null;
  try {
    await validateConfig(snapshot);
    await apiSaveConfig(snapshot);
    configStore.baseline = clone(snapshot);
    configStore.dirty = serialized(configStore.config) !== serialized(snapshot);
    return true;
  } catch (error) {
    configStore.error = String(error);
    return false;
  } finally {
    configStore.saving = false;
  }
}
