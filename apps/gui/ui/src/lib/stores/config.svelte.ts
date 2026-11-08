// Mirrors apps/gui/ui/src/lib/stores/config.svelte.ts, plus:
//  - `dirty`: true when `config` differs from the last loaded/saved copy
//  - toasts on save success/failure (the real app only sets `error`)

import { getConfig, saveConfig as apiSaveConfig, validateConfig } from '../api';
import type { AppConfig, FanGroupConfig } from '../types';
import { toasts } from './toasts.svelte';

class ConfigStore {
  config = $state<AppConfig | null>(null);
  error = $state<string | null>(null);
  saving = $state(false);
  /** JSON of the last loaded/saved config. */
  baseline = $state<string>('');
  /** Bumps whenever the editing session restarts (load, save, revert): editors drop their undo history. */
  epoch = $state(0);
  /** Unsaved edits present. */
  dirty = $derived(this.config !== null && JSON.stringify(this.config) !== this.baseline);
  /** The last saved config: what the control loop is actually running (never the unsaved draft). */
  saved = $derived.by((): AppConfig | null => {
    if (!this.baseline) return this.config;
    try {
      return JSON.parse(this.baseline) as AppConfig;
    } catch {
      return this.config;
    }
  });
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
    configStore.epoch++;
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
    configStore.epoch++;
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

/**
 * Save only the General section. The payload is the last saved config with
 * the current `general` on top, so unsaved Fan Control edits are neither
 * committed nor lost. Both config and baseline receive the new `general`.
 */
export async function saveGeneralSettings(): Promise<boolean> {
  if (!configStore.config) return false;
  configStore.saving = true;
  configStore.error = null;
  try {
    const general = $state.snapshot(configStore.config.general) as AppConfig['general'];
    const base = configStore.baseline ? (JSON.parse(configStore.baseline) as AppConfig) : ($state.snapshot(configStore.config) as AppConfig);
    const payload: AppConfig = { ...base, general };
    await validateConfig(payload);
    await apiSaveConfig(payload);
    const fresh = await getConfig();
    configStore.config.general = fresh.general;
    configStore.baseline = JSON.stringify({ ...base, general: fresh.general });
    toasts.success('Settings saved');
    return true;
  } catch (e) {
    configStore.error = String(e);
    toasts.error('Could not save settings', { detail: String(e) });
    return false;
  } finally {
    configStore.saving = false;
  }
}

/**
 * After the backend changed fan groups on its own (a Quick Preset applied),
 * pull the fresh config and merge only the affected groups (all when `names`
 * is null) into both config and baseline. Other groups keep their unsaved
 * edits.
 */
export async function mergeGroupsFromBackend(names: string[] | null) {
  const fresh = await getConfig();
  if (!configStore.config) {
    configStore.config = fresh;
    configStore.baseline = JSON.stringify(fresh);
    return;
  }
  const merge = (cfg: AppConfig) => {
    if (names === null) {
      cfg.fan_groups = fresh.fan_groups;
      return;
    }
    for (const g of fresh.fan_groups) {
      if (!names.includes(g.name)) continue;
      const i = cfg.fan_groups.findIndex((x) => x.name === g.name);
      if (i >= 0) cfg.fan_groups[i] = g;
      else cfg.fan_groups.push(g);
    }
  };
  merge(configStore.config);
  try {
    const base = configStore.baseline ? (JSON.parse(configStore.baseline) as AppConfig) : fresh;
    merge(base);
    configStore.baseline = JSON.stringify(base);
  } catch {
    configStore.baseline = JSON.stringify(fresh);
  }
}

/**
 * Persist exactly these fan groups (by name) on top of the last saved
 * config, leaving every other unsaved edit alone; then merge them into
 * config and baseline. Used by Undo after a Quick Preset.
 */
export async function saveGroups(groups: FanGroupConfig[]): Promise<boolean> {
  if (!configStore.config) return false;
  configStore.saving = true;
  try {
    const base = configStore.baseline ? (JSON.parse(configStore.baseline) as AppConfig) : ($state.snapshot(configStore.config) as AppConfig);
    const payload: AppConfig = {
      ...base,
      fan_groups: base.fan_groups.map((g) => groups.find((x) => x.name === g.name) ?? g),
    };
    await validateConfig(payload);
    await apiSaveConfig(payload);
    await mergeGroupsFromBackend(groups.map((g) => g.name));
    return true;
  } catch (e) {
    configStore.error = String(e);
    toasts.error('Could not save fan groups', { detail: String(e) });
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
  configStore.epoch++;
}
