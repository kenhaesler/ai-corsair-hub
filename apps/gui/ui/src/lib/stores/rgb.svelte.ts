import { listen } from '@tauri-apps/api/event';
import type { RgbConfig, RgbFrameDto } from '../types';

class RgbStore {
  frames = $state.raw<RgbFrameDto[]>([]);
  config = $state<RgbConfig | null>(null);
  selectedZone = $state<number>(0);
  saving = $state(false);
}

export const rgb = new RgbStore();

let initialized = false;

export async function initRgbListener() {
  if (initialized) return;
  initialized = true;

  await listen<RgbFrameDto[]>('rgb-frame', (event) => {
    rgb.frames = event.payload;
  });
}
