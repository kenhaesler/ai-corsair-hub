import { listen } from '@tauri-apps/api/event';
import type { SystemSnapshot } from '../types';

const MAX_HISTORY = 300; // 5 min at 1/sec

class SensorStore {
  snapshot = $state.raw<SystemSnapshot | null>(null);
  history = $state.raw<SystemSnapshot[]>([]);
  hwError = $state<string | null>(null);
}

export const sensors = new SensorStore();

let initialized = false;

export async function initSensorListener() {
  if (initialized) return;
  initialized = true;

  await listen<SystemSnapshot>('sensor-update', (event) => {
    sensors.snapshot = event.payload;
    sensors.history = [...sensors.history.slice(-(MAX_HISTORY - 1)), event.payload];
  });

  await listen<string>('hw-error', (event) => {
    sensors.hwError = event.payload;
  });
}
