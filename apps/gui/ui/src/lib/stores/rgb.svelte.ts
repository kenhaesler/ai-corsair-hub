// RGB state. `initRgbListener()` subscribes to the backend's `rgb-frame`
// event (emitted every render tick while the RGB renderer runs). Frames only
// feed the Lighting live preview, so consumers pair it with
// `stopRgbListener()` on unmount (ref-counted): no per-frame IPC handling
// while Lighting isn't mounted.

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { setRgbConfig } from '../api';
import type { RgbConfig, RgbFrameDto, RgbPreset } from '../types';
import { toasts } from './toasts.svelte';

class RgbStore {
  frames = $state.raw<RgbFrameDto[]>([]);
  config = $state<RgbConfig | null>(null);
  selectedZone = $state<number>(0);
  saving = $state(false);
  /** Lighting presets saved in config.rgb.presets. */
  get presets(): RgbPreset[] {
    return this.config?.presets ?? [];
  }
}

export const rgb = new RgbStore();

let unlisten: UnlistenFn | null = null;
let subscribing: Promise<void> | null = null;
let refs = 0;

/** Ref-counted: call once per consumer; pair with stopRgbListener(). */
export async function initRgbListener() {
  refs++;
  if (unlisten) return;
  subscribing ??= listen<RgbFrameDto[]>('rgb-frame', (event) => {
    rgb.frames = event.payload;
  }).then((off) => {
    subscribing = null;
    // The last consumer may have stopped while `listen` was pending.
    if (refs === 0) off();
    else unlisten = off;
  });
  await subscribing;
}

export function stopRgbListener() {
  refs = Math.max(0, refs - 1);
  if (refs === 0 && unlisten) {
    unlisten();
    unlisten = null;
  }
}

/** Lighting auto-saves on every change (setRgbConfig). Errors become toasts. */
export async function saveRgbConfig(next: RgbConfig): Promise<boolean> {
  rgb.config = next;
  rgb.saving = true;
  try {
    await setRgbConfig($state.snapshot(next) as RgbConfig);
    return true;
  } catch (e) {
    toasts.error('Lighting change was not saved', { detail: String(e) });
    return false;
  } finally {
    rgb.saving = false;
  }
}
