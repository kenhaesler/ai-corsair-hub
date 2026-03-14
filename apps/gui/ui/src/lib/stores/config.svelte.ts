import { getConfig, saveConfig as apiSaveConfig, validateConfig } from '../api';
import type { AppConfig } from '../types';

class ConfigStore {
  config = $state<AppConfig | null>(null);
  error = $state<string | null>(null);
  saving = $state(false);
}

export const configStore = new ConfigStore();

export async function loadConfig() {
  try {
    configStore.config = await getConfig();
    configStore.error = null;
  } catch (e) {
    configStore.error = String(e);
  }
}

export async function saveCurrentConfig() {
  if (!configStore.config) return;
  configStore.saving = true;
  configStore.error = null;
  try {
    await validateConfig(configStore.config);
    await apiSaveConfig(configStore.config);
  } catch (e) {
    configStore.error = String(e);
  } finally {
    configStore.saving = false;
  }
}
