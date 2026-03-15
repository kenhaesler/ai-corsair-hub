import { listen } from '@tauri-apps/api/event';
import type { SystemSnapshot } from '../types';

const MAX_HISTORY = 300; // 5 min at 1/sec

export interface HubStatus {
  serial: string;
  state: 'lost' | 'recovered' | 'ok';
  /** Timestamp when the state was set (for auto-dismiss) */
  since: number;
}

class SensorStore {
  snapshot = $state.raw<SystemSnapshot | null>(null);
  history = $state.raw<SystemSnapshot[]>([]);
  hwError = $state<string | null>(null);
  hubStatus = $state.raw<HubStatus[]>([]);
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

  await listen<string>('hub-lost', (event) => {
    const serial = event.payload;
    sensors.hubStatus = [
      ...sensors.hubStatus.filter(h => h.serial !== serial),
      { serial, state: 'lost', since: Date.now() },
    ];
  });

  await listen<string>('hub-recovered', (event) => {
    const serial = event.payload;
    sensors.hubStatus = [
      ...sensors.hubStatus.filter(h => h.serial !== serial),
      { serial, state: 'recovered', since: Date.now() },
    ];
    // Auto-dismiss recovered banner after 5 seconds
    setTimeout(() => {
      sensors.hubStatus = sensors.hubStatus.filter(
        h => !(h.serial === serial && h.state === 'recovered')
      );
    }, 5000);
  });
}
