// Mirrors apps/gui/ui/src/lib/stores/config.svelte.ts, plus:
//  - `dirty`: true when `config` differs from the last loaded/saved copy
//  - toasts on save success/failure (the real app only sets `error`)

import { getConfig, saveConfig as apiSaveConfig, validateConfig } from '../api';
import type { AppConfig } from '../types';
import { toasts } from './toasts.svelte';

class ConfigStore {
  config = $state<AppConfig | null>(null);
  error = $state<string | null>(null);
  saving = $state(false);
  /** JSON of the last loaded/saved config. */
  baseline = $state<string>('');
  /** Unsaved edits present. */
  dirty = $derived(this.config !== null && JSON.stringify(this.config) !== this.baseline);
}

export const configStore = new ConfigStore();

/**
 * Reflect a device rename the backend already saved, without reloading the
 * whole config (which would discard unsaved Fan Control edits). Updates the
 * baseline too, so the rename never counts as an unsaved change.
 */
export function applyDeviceName(deviceId: string, name: string) {
  const patch = (cfg: AppConfig | null) => {
    if (!cfg) return;
    cfg.devices ??= [];
    const row = cfg.devices.find((d) => d.device_id === deviceId);
    if (row) row.name = name || undefined;
    else if (name) cfg.devices.push({ device_id: deviceId, name });
  };
  patch(configStore.config);
  if (configStore.baseline) {
    try {
      const base = JSON.parse(configStore.baseline) as AppConfig;
      patch(base);
      configStore.baseline = JSON.stringify(base);
    } catch {
      /* keep the old baseline */
    }
  }
}

export async function loadConfig() {
  try {
    const cfg = await getConfig();
    configStore.config = cfg;
    configStore.baseline = JSON.stringify(cfg);
    configStore.error = null;
  } catch (e) {
    configStore.error = String(e);
  }
}

/**
 * validateConfig then saveConfig (same order as the real store). Failure
 * sets `error` and shows a toast. Resolves to true on success.
 */
export async function saveCurrentConfig(opts: { successMessage?: string | null } = {}): Promise<boolean> {
  if (!configStore.config) return false;
  configStore.saving = true;
  configStore.error = null;
  try {
    const snapshot = $state.snapshot(configStore.config) as AppConfig;
    await validateConfig(snapshot);
    await apiSaveConfig(snapshot);
    // re-read so backend-side reconciliation (e.g. preset_applied) shows up
    const fresh = await getConfig();
    configStore.config = fresh;
    configStore.baseline = JSON.stringify(fresh);
    const msg = opts.successMessage === undefined ? 'Configuration saved' : opts.successMessage;
    if (msg) toasts.success(msg);
    return true;
  } catch (e) {
    configStore.error = String(e);
    toasts.error('Could not save configuration', { detail: String(e) });
    return false;
  } finally {
    configStore.saving = false;
  }
}

/** Throw away unsaved edits. */
export function revertConfig() {
  if (!configStore.baseline) return;
  configStore.config = JSON.parse(configStore.baseline);
  configStore.error = null;
}
