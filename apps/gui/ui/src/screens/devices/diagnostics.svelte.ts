// Diagnostics controller for the Devices screen (inventory 3.1, 3.3, 3.4).
//
// Identify Channels and the LED marker diagnostic share one rule: only one
// runs at a time. Both show a 500 ms countdown; Walk additionally derives its
// current LED client-side every 100 ms from the report timing.

import { rgbIdentifyChannels, rgbMarkerDiagnostic, type ChannelIdentity, type LedMarkerReport, type MarkerMode } from '../../lib/api';

export const IDENTIFY_SECS = 20;
export const MARKER_SECS = 60;

export type MarkerKind = 'markers' | 'markers_black' | 'ruler' | 'walk';

export class Diagnostics {
  identifyMap = $state<ChannelIdentity[] | null>(null);
  identifyRunning = $state(false);
  identifyRemaining = $state(0);
  /** 1 -> 0 over the run (drives the thin progress line). */
  identifyFrac = $state(0);

  markerReport = $state<LedMarkerReport | null>(null);
  markerRunning = $state(false);
  markerRemaining = $state(0);
  markerFrac = $state(0);

  modeKind = $state<MarkerKind>('markers');
  walkDeviceId = $state('');
  walkStepMs = $state(400);
  walkLed = $state(0);

  busy = $derived(this.identifyRunning || this.markerRunning);

  #idTimer: ReturnType<typeof setInterval> | undefined;
  #mkTimer: ReturnType<typeof setInterval> | undefined;
  #walkTimer: ReturnType<typeof setInterval> | undefined;
  #onError: (message: string) => void;

  constructor(onError: (message: string) => void) {
    this.#onError = onError;
  }

  #startIdentifyClock(deadline: number, total: number) {
    clearInterval(this.#idTimer);
    const tick = () => {
      const msLeft = Math.max(0, deadline - Date.now());
      this.identifyRemaining = Math.ceil(msLeft / 1000);
      this.identifyFrac = msLeft / (total * 1000);
      if (msLeft <= 0) {
        this.identifyRunning = false;
        clearInterval(this.#idTimer);
      }
    };
    tick();
    this.#idTimer = setInterval(tick, 500);
  }

  #startMarkerClock(deadline: number, total: number) {
    clearInterval(this.#mkTimer);
    const tick = () => {
      const msLeft = Math.max(0, deadline - Date.now());
      this.markerRemaining = Math.ceil(msLeft / 1000);
      this.markerFrac = msLeft / (total * 1000);
      if (msLeft <= 0) {
        this.markerRunning = false;
        clearInterval(this.#mkTimer);
        clearInterval(this.#walkTimer);
      }
    };
    tick();
    this.#mkTimer = setInterval(tick, 500);
  }

  #startWalkClock(report: LedMarkerReport) {
    clearInterval(this.#walkTimer);
    const walk = report.walk;
    if (!walk) return;
    const tick = () => {
      this.walkLed =
        walk.led_count > 0 ? Math.floor(Math.max(0, Date.now() - walk.started_at) / walk.step_ms) % walk.led_count : 0;
    };
    tick();
    this.#walkTimer = setInterval(tick, 100);
  }

  async runMarkers() {
    if (this.busy) return;
    let mode: MarkerMode | undefined;
    if (this.modeKind === 'walk') {
      if (!this.walkDeviceId) {
        this.#onError('LED markers failed: pick a device for Walk mode');
        return;
      }
      const step = Math.min(2000, Math.max(100, Math.round(Number(this.walkStepMs)) || 400));
      mode = { kind: 'walk', device_id: this.walkDeviceId, step_ms: step };
    } else if (this.modeKind !== 'markers') {
      mode = { kind: this.modeKind };
    }
    try {
      const report = await rgbMarkerDiagnostic(MARKER_SECS, mode);
      this.markerReport = report;
      this.markerRunning = true;
      this.#startMarkerClock(Date.now() + report.duration_secs * 1000, report.duration_secs);
      this.#startWalkClock(report);
    } catch (e) {
      this.#onError(`LED markers failed: ${String(e)}`);
    }
  }

  async runIdentify() {
    if (this.busy) return;
    try {
      const mapping = await rgbIdentifyChannels(IDENTIFY_SECS);
      this.identifyMap = mapping;
      // Identify cancels the marker diagnostic.
      this.markerRunning = false;
      clearInterval(this.#mkTimer);
      clearInterval(this.#walkTimer);
      this.identifyRunning = true;
      this.#startIdentifyClock(Date.now() + IDENTIFY_SECS * 1000, IDENTIFY_SECS);
    } catch (e) {
      this.#onError(`Identify failed: ${String(e)}`);
    }
  }

  dispose() {
    clearInterval(this.#idTimer);
    clearInterval(this.#mkTimer);
    clearInterval(this.#walkTimer);
  }
}
