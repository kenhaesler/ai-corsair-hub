import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getSnapshot } from '../api';
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
  listenerError = $state<string | null>(null);
  hubStatus = $state.raw<HubStatus[]>([]);
}

export const sensors = new SensorStore();

function acceptSnapshot(snapshot: SystemSnapshot) {
  sensors.snapshot = snapshot;
  const latest = sensors.history.at(-1);
  if (latest?.timestamp_ms === snapshot.timestamp_ms) return;
  sensors.history = [...sensors.history.slice(-(MAX_HISTORY - 1)), snapshot];
}

let listenerPromise: Promise<UnlistenFn> | null = null;
let listenerCleanup: UnlistenFn | null = null;
let listenerGeneration = 0;
const listenerSetupCancelled = Symbol('listener-setup-cancelled');
const noopUnlisten: UnlistenFn = () => {};

export function initSensorListener(): Promise<UnlistenFn> {
  if (listenerCleanup) return Promise.resolve(listenerCleanup);
  if (listenerPromise) return listenerPromise;

  const generation = ++listenerGeneration;
  const setup = (async () => {
    const unlisteners: UnlistenFn[] = [];
    const dismissTimers = new Set<ReturnType<typeof setTimeout>>();
    const ensureCurrent = (unlisten?: UnlistenFn) => {
      if (generation === listenerGeneration) return;
      unlisten?.();
      throw listenerSetupCancelled;
    };
    const cleanupResources = () => {
      for (const unlisten of unlisteners) unlisten();
      for (const timer of dismissTimers) clearTimeout(timer);
      dismissTimers.clear();
    };

    try {
      let unlisten = await listen<SystemSnapshot>('sensor-update', (event) => {
        if (generation === listenerGeneration) acceptSnapshot(event.payload);
      });
      ensureCurrent(unlisten);
      unlisteners.push(unlisten);

      unlisten = await listen<string>('hw-error', (event) => {
        if (generation === listenerGeneration) sensors.hwError = event.payload;
      });
      ensureCurrent(unlisten);
      unlisteners.push(unlisten);

      unlisten = await listen<string>('hub-lost', (event) => {
        if (generation !== listenerGeneration) return;
        const serial = event.payload;
        sensors.hubStatus = [
          ...sensors.hubStatus.filter((hub) => hub.serial !== serial),
          { serial, state: 'lost', since: Date.now() },
        ];
      });
      ensureCurrent(unlisten);
      unlisteners.push(unlisten);

      unlisten = await listen<string>('hub-recovered', (event) => {
        if (generation !== listenerGeneration) return;
        const serial = event.payload;
        sensors.hubStatus = [
          ...sensors.hubStatus.filter((hub) => hub.serial !== serial),
          { serial, state: 'recovered', since: Date.now() },
        ];
        const timer = setTimeout(() => {
          sensors.hubStatus = sensors.hubStatus.filter(
            (hub) => !(hub.serial === serial && hub.state === 'recovered'),
          );
          dismissTimers.delete(timer);
        }, 5000);
        dismissTimers.add(timer);
      });
      ensureCurrent(unlisten);
      unlisteners.push(unlisten);

      // Events only cover future ticks; fetch the current state so the first render is immediate.
      const snapshot = await getSnapshot();
      ensureCurrent();
      acceptSnapshot(snapshot);
      const cleanup = () => {
        cleanupResources();
        if (listenerCleanup === cleanup) listenerCleanup = null;
      };
      listenerCleanup = cleanup;
      sensors.listenerError = null;
      return cleanup;
    } catch (error) {
      cleanupResources();
      if (error === listenerSetupCancelled) return noopUnlisten;
      sensors.listenerError = String(error);
      throw error;
    }
  })();
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

export function disposeSensorListener() {
  listenerGeneration += 1;
  listenerCleanup?.();
  listenerCleanup = null;
  listenerPromise = null;
}
