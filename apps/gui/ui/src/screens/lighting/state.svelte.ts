// Lighting page controller. `rgb.config` (the shared store) is the source
// of truth; every mutation goes through `persist()`, which coalesces bursts
// (slider drags) into at most one in-flight setRgbConfig plus one trailing
// save, so the last state always lands.

import { getConfig, getDevices, setRgbEnabled } from '../../lib/api';
import { rgb, saveRgbConfig } from '../../lib/stores/rgb.svelte';
import { configStore } from '../../lib/stores/config.svelte';
import { toasts } from '../../lib/stores/toasts.svelte';
import type { HubSnapshot, RgbConfig, RgbPreset, RgbZoneConfig } from '../../lib/types';
import { allDeviceRefs, defaultBlueLayer } from './effects';

export const DEFAULT_RGB = (): RgbConfig => ({
  enabled: false,
  brightness: 80,
  fps: 30,
  hardware_output: false,
  zones: [],
  presets: [],
});

class LightingState {
  hubs = $state.raw<HubSnapshot[]>([]);
  loaded = $state(false);
  loadError = $state<string | null>(null);
  /** device ref key hovered in the zone device list (highlights it on the stage) */
  hoverKey = $state<string | null>(null);
  /** in-flight or queued saves (drives the "Saving..." indicator together with rgb.saving) */
  pending = $state(0);

  get config(): RgbConfig | null {
    return rgb.config;
  }
  get zone(): RgbZoneConfig | null {
    return rgb.config?.zones[rgb.selectedZone] ?? null;
  }
}

export const lighting = new LightingState();

// ---------------------------------------------------------------------------
// Load (inventory 4.1)
// ---------------------------------------------------------------------------
let loading: Promise<void> | null = null;

export function loadLighting(): Promise<void> {
  if (loading) return loading;
  loading = (async () => {
    try {
      const [appConfig, tree] = await Promise.all([getConfig(), getDevices()]);
      rgb.config = appConfig.rgb ?? DEFAULT_RGB();
      lighting.hubs = tree.hubs;
      lighting.loadError = null;

      // No zones but hubs exist: create "All Fans" with every device.
      if (rgb.config.zones.length === 0 && lighting.hubs.length > 0) {
        rgb.config.zones = [
          {
            name: 'All Fans',
            devices: allDeviceRefs(lighting.hubs),
            layers: [defaultBlueLayer()],
            brightness: 100,
            flow: null,
          },
        ];
        rgb.selectedZone = 0;
        await persist();
      }
    } catch (e) {
      rgb.config = DEFAULT_RGB();
      lighting.loadError = String(e);
      toasts.error('Could not load lighting settings', { detail: `${String(e)}. Showing defaults.` });
    } finally {
      if (rgb.selectedZone >= (rgb.config?.zones.length ?? 0)) rgb.selectedZone = 0;
      lighting.loaded = true;
    }
  })();
  return loading;
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------
let inFlight: Promise<boolean> | null = null;
let queued = false;

/** Keep the shared AppConfig copy in step so a later full-config save cannot revert lighting. */
function mirrorIntoAppConfig(next: RgbConfig) {
  const app = configStore.config;
  if (!app) return;
  const snap = $state.snapshot(next) as RgbConfig;
  app.rgb = snap;
  try {
    const base = JSON.parse(configStore.baseline || '{}');
    base.rgb = snap;
    configStore.baseline = JSON.stringify(base);
  } catch {
    /* baseline unavailable: leave it */
  }
}

export async function persist(): Promise<boolean> {
  const cfg = rgb.config;
  if (!cfg) return false;
  if (inFlight) {
    queued = true;
    return inFlight;
  }
  lighting.pending++;
  try {
    do {
      queued = false;
      inFlight = saveRgbConfig(cfg);
      const ok = await inFlight;
      if (ok) mirrorIntoAppConfig(cfg);
      else return false;
    } while (queued);
    return true;
  } finally {
    inFlight = null;
    lighting.pending--;
  }
}

// ---------------------------------------------------------------------------
// Master controls (4.2)
// ---------------------------------------------------------------------------
export async function setEnabled(on: boolean) {
  if (!rgb.config) return;
  rgb.config.enabled = on;
  try {
    await setRgbEnabled(on);
  } catch (e) {
    toasts.error(on ? 'Could not turn lighting on' : 'Could not turn lighting off', { detail: String(e) });
  }
  await persist();
}

// ---------------------------------------------------------------------------
// Zones (4.3)
// ---------------------------------------------------------------------------
export function selectZone(i: number) {
  rgb.selectedZone = i;
}

export function addZone() {
  const cfg = rgb.config;
  if (!cfg) return;
  cfg.zones.push({
    name: `Zone ${cfg.zones.length + 1}`,
    devices: [],
    layers: [defaultBlueLayer()],
    brightness: 100,
    flow: null,
  });
  rgb.selectedZone = cfg.zones.length - 1;
  persist();
}

export function deleteZone(i: number) {
  const cfg = rgb.config;
  if (!cfg || cfg.zones.length <= 1) return;
  cfg.zones.splice(i, 1);
  if (rgb.selectedZone >= cfg.zones.length) rgb.selectedZone = Math.max(0, cfg.zones.length - 1);
  else if (rgb.selectedZone > i) rgb.selectedZone--;
  persist();
}

export function renameZone(i: number, name: string) {
  const cfg = rgb.config;
  const n = name.trim();
  if (!cfg || !n || cfg.zones[i]?.name === n) return;
  cfg.zones[i].name = n;
  persist();
}

// ---------------------------------------------------------------------------
// Presets (4.4)
// ---------------------------------------------------------------------------
export function applyPreset(p: RgbPreset) {
  const cfg = rgb.config;
  if (!cfg) return;
  const current = cfg.zones;
  const prev = current.length;
  const before = structuredClone($state.snapshot(current)) as RgbZoneConfig[];
  const beforeSel = rgb.selectedZone;
  cfg.zones = (structuredClone($state.snapshot(p.zones)) as RgbZoneConfig[]).map((pz, i) => {
    const existing = current[i]?.devices ?? [];
    return { ...pz, devices: existing.length > 0 ? ($state.snapshot(existing) as typeof existing) : pz.devices };
  });
  rgb.selectedZone = 0;
  const n = cfg.zones.length;
  persist().then(
    (ok) =>
      ok &&
      toasts.success(`${p.name} applied`, {
        detail: `${prev} ${prev === 1 ? 'zone' : 'zones'} replaced by ${n}. Device assignments kept.`,
        timeout: 7000,
        action: {
          label: 'Undo',
          run: () => {
            if (!rgb.config) return;
            rgb.config.zones = before;
            rgb.selectedZone = Math.min(beforeSel, before.length - 1);
            persist();
          },
        },
      }),
  );
}

export function savePreset(name: string): boolean {
  const cfg = rgb.config;
  const n = name.trim();
  if (!cfg || !n) return false;
  cfg.presets.push({ name: n, zones: structuredClone($state.snapshot(cfg.zones)) as RgbZoneConfig[] });
  persist().then((ok) => ok && toasts.success(`Saved “${n}”`, { detail: 'Find it under Your presets' }));
  return true;
}

export function renamePreset(i: number, name: string) {
  const cfg = rgb.config;
  const n = name.trim();
  if (!cfg || !n || cfg.presets[i]?.name === n) return;
  cfg.presets[i].name = n;
  persist();
}

export function deletePreset(i: number) {
  const cfg = rgb.config;
  if (!cfg) return;
  const [gone] = cfg.presets.splice(i, 1);
  persist().then((ok) => ok && gone && toasts.info(`Deleted “${gone.name}”`));
}
