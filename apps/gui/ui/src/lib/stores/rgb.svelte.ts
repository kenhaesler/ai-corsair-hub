import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { RgbConfig, RgbFrameDto } from '../types';
import { setRgbConfig } from '../api';
import {
  acceptRgbConfig,
  configStore,
  discardConfigChanges,
  setRgbDraft,
} from './config.svelte';

const SAVE_DEBOUNCE_MS = 250;

class RgbStore {
  frames = $state.raw<RgbFrameDto[]>([]);
  config = $state<RgbConfig | null>(null);
  selectedZone = $state(0);
  saving = $state(false);
  dirty = $state(false);
  error = $state<string | null>(null);
  listenerError = $state<string | null>(null);
}

export const rgb = new RgbStore();

let revision = 0;
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let saveWorker: Promise<boolean> | null = null;

export function initializeRgbConfig(config: RgbConfig, force = false) {
  if (rgb.config && !force) return;
  rgb.config = structuredClone(config);
  rgb.dirty = false;
  rgb.error = null;
}

export function scheduleRgbSave(config: RgbConfig, immediate = false) {
  rgb.config = structuredClone(config);
  rgb.dirty = true;
  rgb.error = null;
  setRgbDraft(config);
  revision += 1;

  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = null;
  if (immediate) {
    void flushRgbSave();
  } else {
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void flushRgbSave();
    }, SAVE_DEBOUNCE_MS);
  }
}

async function runSaveWorker(): Promise<boolean> {
  let lastAttempted = -1;
  rgb.saving = true;

  try {
    while (rgb.config && revision !== lastAttempted) {
      const attemptRevision = revision;
      const snapshot = structuredClone(rgb.config);
      lastAttempted = attemptRevision;
      try {
        await setRgbConfig(snapshot);
        acceptRgbConfig(snapshot, revision === attemptRevision);
        if (revision === attemptRevision) {
          rgb.dirty = false;
          rgb.error = null;
        }
      } catch (error) {
        rgb.error = String(error);
        if (revision === attemptRevision) break;
      }
    }
  } finally {
    rgb.saving = false;
  }

  return !rgb.dirty;
}

export function flushRgbSave(): Promise<boolean> {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = null;
  if (!rgb.config || !rgb.dirty) return Promise.resolve(true);
  if (saveWorker) return saveWorker;

  saveWorker = runSaveWorker().finally(() => {
    saveWorker = null;
  });
  return saveWorker;
}

export function disposeRgbSaveQueue() {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = null;
}

export function discardAllConfigChanges(): Promise<boolean> {
  const baselineRgb = configStore.baseline?.rgb
    ? structuredClone(configStore.baseline.rgb)
    : null;
  discardConfigChanges();
  if (!baselineRgb) return Promise.resolve(true);

  const activeWorker = saveWorker;
  revision += 1;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = null;
  rgb.config = baselineRgb;
  rgb.error = null;

  if (!activeWorker) {
    rgb.dirty = false;
    return Promise.resolve(true);
  }

  // The in-flight write cannot be cancelled. Its worker observes the new
  // revision and immediately writes the baseline back before resolving.
  rgb.dirty = true;
  return activeWorker;
}

let listenerPromise: Promise<UnlistenFn> | null = null;
let listenerCleanup: UnlistenFn | null = null;
let listenerGeneration = 0;
const noopUnlisten: UnlistenFn = () => {};

export function initRgbListener(): Promise<UnlistenFn> {
  if (listenerCleanup) return Promise.resolve(listenerCleanup);
  if (listenerPromise) return listenerPromise;

  const generation = ++listenerGeneration;
  const setup = listen<RgbFrameDto[]>('rgb-frame', (event) => {
    if (generation === listenerGeneration) rgb.frames = event.payload;
  })
    .then((unlisten) => {
      if (generation !== listenerGeneration) {
        unlisten();
        return noopUnlisten;
      }

      const cleanup = () => {
        unlisten();
        if (listenerCleanup === cleanup) listenerCleanup = null;
      };
      listenerCleanup = cleanup;
      rgb.listenerError = null;
      return cleanup;
    })
    .catch((error) => {
      if (generation !== listenerGeneration) return noopUnlisten;
      rgb.listenerError = String(error);
      throw error;
    });
  listenerPromise = setup;
  void setup.then(
    () => {
      if (listenerPromise === setup) listenerPromise = null;
    },
    () => {
      if (listenerPromise === setup) listenerPromise = null;
    },
  );

  return setup;
}

export function disposeRgbListener() {
  listenerGeneration += 1;
  listenerCleanup?.();
  listenerCleanup = null;
  listenerPromise = null;
}
