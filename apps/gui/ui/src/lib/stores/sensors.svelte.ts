// Live sensor state, fed by the backend's push events (no polling: the
// hardware thread emits `sensor-update` every `general.poll_interval_ms`).
//
//   sensor-update  -> snapshot + 300-sample history ring (+ 5 s session timeline)
//   hw-error       -> hwError
//   hub-lost       -> hubStatus 'lost'
//   hub-recovered  -> hubStatus 'recovered', auto-removed after 5 s
//
// Stale / emergency state is carried on each snapshot (`any_stale`,
// `emergency`) and rendered by the Dashboard alert rows.
//
// Screens should NOT read `sensors.snapshot` for display: read
// `display.snapshot` from ./scrub.svelte.ts so everything follows the scrub.

import { listen } from '@tauri-apps/api/event';
import type { SystemSnapshot } from '../types';

export const MAX_HISTORY = 300; // 5 min at 1/sec
const SESSION_STEP_MS = 5000;
const MAX_SESSION = 1440; // 2 h at 5 s
const RECOVERED_DISMISS_MS = 5000;

export interface HubStatus {
  serial: string;
  state: 'lost' | 'recovered' | 'ok';
  /** Timestamp when the state was set (for auto-dismiss) */
  since: number;
}

class SensorStore {
  snapshot = $state.raw<SystemSnapshot | null>(null);
  history = $state.raw<SystemSnapshot[]>([]);
  /** Older, downsampled samples (5 s). Oldest first. Powers the "Session" range. */
  session = $state.raw<SystemSnapshot[]>([]);
  hwError = $state<string | null>(null);
  hubStatus = $state.raw<HubStatus[]>([]);
}

export const sensors = new SensorStore();

let initialized = false;

export async function initSensorListener() {
  if (initialized) return;
  initialized = true;

  await listen<SystemSnapshot>('sensor-update', (event) => {
    const snap = event.payload;
    sensors.snapshot = snap;
    sensors.history = [...sensors.history.slice(-(MAX_HISTORY - 1)), snap];
    const lastSession = sensors.session[sensors.session.length - 1];
    if (!lastSession || snap.timestamp_ms - lastSession.timestamp_ms >= SESSION_STEP_MS) {
      sensors.session = [...sensors.session.slice(-(MAX_SESSION - 1)), snap];
    }
  });

  await listen<string>('hw-error', (event) => {
    sensors.hwError = event.payload;
  });

  await listen<string>('hub-lost', (event) => {
    const serial = event.payload;
    sensors.hubStatus = [
      ...sensors.hubStatus.filter((h) => h.serial !== serial),
      { serial, state: 'lost', since: Date.now() },
    ];
  });

  await listen<string>('hub-recovered', (event) => {
    const serial = event.payload;
    sensors.hubStatus = [
      ...sensors.hubStatus.filter((h) => h.serial !== serial),
      { serial, state: 'recovered', since: Date.now() },
    ];
    // Auto-dismiss recovered banner after 5 seconds
    setTimeout(() => {
      sensors.hubStatus = sensors.hubStatus.filter((h) => !(h.serial === serial && h.state === 'recovered'));
    }, RECOVERED_DISMISS_MS);
  });
}
