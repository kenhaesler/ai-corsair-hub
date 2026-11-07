// Lighting helpers: effect templates (inventory 4.8), built-in presets
// (4.4), friendly names, swatch previews and zone/device matching.

import type {
  EffectConfig,
  HubDeviceEntry,
  HubSnapshot,
  LayerConfig,
  RgbColor,
  RgbDeviceRef,
  RgbFrameDto,
  RgbPreset,
  RgbZoneConfig,
} from '../../lib/types';

export type EffectType = EffectConfig['type'];
export type Category = 'Basic' | 'Motion' | 'Organic' | 'Reactive';

export interface EffectTemplate {
  name: string;
  category: Category;
  config: EffectConfig;
  blurb: string;
}

const c = (r: number, g: number, b: number): RgbColor => ({ r, g, b });

/** Effect picker templates, exactly as apps/gui EffectPicker.svelte. */
export const EFFECTS: EffectTemplate[] = [
  { name: 'Static', category: 'Basic', config: { type: 'Static', color: c(96, 165, 250) }, blurb: 'One steady colour' },
  { name: 'Breathing', category: 'Basic', config: { type: 'Breathing', color: c(96, 165, 250), speed: 1.0 }, blurb: 'Slow fade in and out' },
  { name: 'Color Cycle', category: 'Motion', config: { type: 'ColorCycle', speed: 0.5, saturation: 1.0 }, blurb: 'Every LED walks the hue wheel' },
  { name: 'Rainbow Wave', category: 'Motion', config: { type: 'RainbowWave', speed: 0.3, wavelength: 1.0 }, blurb: 'Spectrum travelling along LEDs' },
  { name: 'Spectrum Shift', category: 'Motion', config: { type: 'SpectrumShift', speed: 0.2 }, blurb: 'Gentle hue drift, near-uniform' },
  {
    name: 'Gradient',
    category: 'Motion',
    config: { type: 'Gradient', colors: [c(255, 0, 128), c(0, 200, 255)], speed: 0.3 },
    blurb: 'Your colours, scrolling',
  },
  { name: 'Fire', category: 'Organic', config: { type: 'Fire', intensity: 0.8, speed: 1.0 }, blurb: 'Flickering embers' },
  { name: 'Aurora', category: 'Organic', config: { type: 'Aurora', speed: 0.5, color_spread: 1.0 }, blurb: 'Drifting green to violet' },
  { name: 'Candle', category: 'Organic', config: { type: 'Candle', color: c(255, 160, 40), flicker_speed: 3.0 }, blurb: 'Warm, uneven flicker' },
  { name: 'Starfield', category: 'Organic', config: { type: 'Starfield', density: 0.6, speed: 1.0 }, blurb: 'Random twinkling points' },
  { name: 'Rain', category: 'Organic', config: { type: 'Rain', color: c(60, 160, 255), speed: 1.0, density: 0.5 }, blurb: 'Falling droplets' },
  {
    name: 'Temp Map',
    category: 'Reactive',
    config: {
      type: 'TemperatureMap',
      gradient: [
        [30, c(60, 165, 250)],
        [60, c(250, 190, 40)],
        [85, c(248, 113, 113)],
      ],
      glow_on_spike: true,
    },
    blurb: 'Colour follows temperature',
  },
  {
    name: 'Thermal Pulse',
    category: 'Reactive',
    config: { type: 'ThermalPulse', cold_color: c(60, 165, 250), hot_color: c(248, 113, 113), min_temp: 30, max_temp: 90 },
    blurb: 'Pulse quickens as it warms',
  },
  {
    name: 'Duty Meter',
    category: 'Reactive',
    config: { type: 'DutyMeter', low_color: c(52, 211, 153), high_color: c(248, 113, 113) },
    blurb: 'LEDs fill with fan duty',
  },
  {
    name: 'GPU Meter',
    category: 'Reactive',
    config: { type: 'GpuMeter', low_color: c(96, 165, 250), high_color: c(192, 132, 252) },
    blurb: 'LEDs fill with GPU load',
  },
];

export const CATEGORIES: Category[] = ['Basic', 'Motion', 'Organic', 'Reactive'];

/** Category colour coding (tokens only). */
export function categoryColor(cat: Category): string {
  switch (cat) {
    case 'Basic':
      return 'var(--t2)';
    case 'Motion':
      return 'var(--accent-strong)';
    case 'Organic':
      return 'var(--s-psu-vrm)';
    case 'Reactive':
      return 'var(--s-gpu)';
  }
}

export function effectName(e: EffectConfig): string {
  return EFFECTS.find((t) => t.config.type === e.type)?.name ?? e.type;
}
export function effectCategory(e: EffectConfig): Category {
  return EFFECTS.find((t) => t.config.type === e.type)?.category ?? 'Basic';
}

export const rgbCss = (k: RgbColor, a = 1) => (a === 1 ? `rgb(${k.r},${k.g},${k.b})` : `rgba(${k.r},${k.g},${k.b},${a})`);

const RAINBOW = 'linear-gradient(90deg,#ff3b3b,#ffb02e,#f5e663,#34d399,#38bdf8,#7c6cff,#e04ff0,#ff3b3b)';
const AURORA = 'linear-gradient(90deg,rgb(0,180,60),rgb(0,220,180),rgb(30,100,255),rgb(130,50,220),rgb(200,50,180))';
const FIRE = 'linear-gradient(90deg,rgb(40,6,0),rgb(180,30,0),rgb(255,120,0),rgb(255,220,50),rgb(255,250,210))';

/** A CSS background that previews an effect (used by chips, tiles, preset rows). */
export function effectSwatch(e: EffectConfig): string {
  switch (e.type) {
    case 'Static':
      return rgbCss(e.color);
    case 'Breathing':
      return `linear-gradient(90deg, ${rgbCss(e.color, 0.25)}, ${rgbCss(e.color)} 50%, ${rgbCss(e.color, 0.25)})`;
    case 'Candle':
      return `linear-gradient(90deg, ${rgbCss(e.color, 0.55)}, ${rgbCss(e.color)} 40%, ${rgbCss(e.color, 0.7)} 70%, ${rgbCss(e.color)})`;
    case 'Rain':
      return `linear-gradient(90deg, ${rgbCss(e.color, 0.15)}, ${rgbCss(e.color)} 18%, ${rgbCss(e.color, 0.15)} 30%, ${rgbCss(e.color, 0.15)} 55%, ${rgbCss(e.color)} 70%, ${rgbCss(e.color, 0.15)} 84%)`;
    case 'ColorCycle':
    case 'RainbowWave':
    case 'SpectrumShift':
      return RAINBOW;
    case 'Aurora':
      return AURORA;
    case 'Fire':
      return FIRE;
    case 'Starfield':
      return 'radial-gradient(circle at 18% 50%, #fff 0 1.5px, transparent 2.5px), radial-gradient(circle at 46% 30%, #fff 0 1px, transparent 2px), radial-gradient(circle at 72% 64%, #fff 0 1.5px, transparent 2.5px), radial-gradient(circle at 90% 36%, #fff 0 1px, transparent 2px), #0d0f16';
    case 'Gradient':
      return e.colors.length === 0
        ? '#111'
        : e.colors.length === 1
          ? rgbCss(e.colors[0])
          : `linear-gradient(90deg, ${e.colors.map((k) => rgbCss(k)).join(', ')})`;
    case 'TemperatureMap':
      return stopsCss(e.gradient);
    case 'ThermalPulse':
      return `linear-gradient(90deg, ${rgbCss(e.cold_color)}, ${rgbCss(e.hot_color)})`;
    case 'DutyMeter':
    case 'GpuMeter':
      return `linear-gradient(90deg, ${rgbCss(e.low_color)}, ${rgbCss(e.high_color)} 62%, ${rgbCss(e.high_color, 0.12)} 62%)`;
  }
}

/** Sorted, normalised CSS gradient for [value, colour] stops (inventory 4.10). */
export function stopsCss(stops: [number, RgbColor][]): string {
  if (stops.length === 0) return '#111';
  if (stops.length === 1) return rgbCss(stops[0][1]);
  const sorted = [...stops].sort((a, b) => a[0] - b[0]);
  const min = sorted[0][0];
  const range = sorted[sorted.length - 1][0] - min || 1;
  return `linear-gradient(90deg, ${sorted.map(([v, k]) => `${rgbCss(k)} ${(((v - min) / range) * 100).toFixed(1)}%`).join(', ')})`;
}

/** Swatch for a whole zone: the first enabled layer (bottom of the stack). */
export function zoneSwatch(z: RgbZoneConfig): string {
  const base = z.layers.find((l) => l.enabled) ?? z.layers[0];
  return base ? effectSwatch(base.effect) : '#111';
}

export function layerSummary(layers: LayerConfig[]): string {
  if (layers.length === 0) return 'No layers';
  return layers.map((l) => effectName(l.effect)).join(' + ');
}

export const newLayer = (effect: EffectConfig): LayerConfig => ({ effect, blend_mode: 'Normal', opacity: 1.0, enabled: true });

export const defaultBlueLayer = (): LayerConfig => newLayer({ type: 'Static', color: c(96, 165, 250) });

// ---------------------------------------------------------------------------
// Built-in presets: ported verbatim from apps/gui PresetManager.svelte
// ---------------------------------------------------------------------------
const zone1 = (layers: LayerConfig[], brightness: number, flow: RgbZoneConfig['flow'] = null): RgbZoneConfig[] => [
  { name: 'All', devices: [], layers, brightness, flow },
];
const L = (effect: EffectConfig, blend_mode: LayerConfig['blend_mode'], opacity: number): LayerConfig => ({
  effect,
  blend_mode,
  opacity,
  enabled: true,
});

export const BUILTIN_PRESETS: (RgbPreset & { blurb: string })[] = [
  {
    name: 'Ocean Calm',
    blurb: 'Slow rainbow with a soft white breath',
    zones: zone1(
      [
        L({ type: 'RainbowWave', speed: 0.1, wavelength: 2.0 }, 'Normal', 1.0),
        L({ type: 'Breathing', color: c(255, 255, 255), speed: 0.3 }, 'Screen', 0.3),
      ],
      80,
    ),
  },
  {
    name: 'Inferno',
    blurb: 'Fire with warm candle flicker added',
    zones: zone1(
      [
        L({ type: 'Fire', intensity: 0.9, speed: 1.2 }, 'Normal', 1.0),
        L({ type: 'Candle', color: c(255, 160, 40), flicker_speed: 3.0 }, 'Add', 0.4),
      ],
      100,
    ),
  },
  {
    name: 'Arctic',
    blurb: 'Ice blue with twinkling stars',
    zones: zone1(
      [L({ type: 'Static', color: c(100, 180, 255) }, 'Normal', 1.0), L({ type: 'Starfield', density: 0.6, speed: 0.8 }, 'Add', 0.6)],
      90,
    ),
  },
  {
    name: 'Cyberpunk',
    blurb: 'Magenta to cyan, overlaid spectrum',
    zones: zone1(
      [
        L({ type: 'Gradient', colors: [c(255, 50, 150), c(0, 200, 255)], speed: 0.2 }, 'Normal', 1.0),
        L({ type: 'SpectrumShift', speed: 0.15 }, 'Overlay', 0.5),
      ],
      100,
    ),
  },
  {
    name: 'Thermal Monitor',
    blurb: 'Colour maps temperature, glows on spikes',
    zones: zone1(
      [
        L(
          {
            type: 'TemperatureMap',
            gradient: [
              [30, c(60, 165, 250)],
              [55, c(52, 211, 153)],
              [70, c(250, 190, 40)],
              [85, c(248, 113, 113)],
            ],
            glow_on_spike: true,
          },
          'Normal',
          1.0,
        ),
      ],
      80,
    ),
  },
  {
    name: 'Silent Pulse',
    blurb: 'Thermal pulse, softened by a slow breath',
    zones: zone1(
      [
        L({ type: 'ThermalPulse', cold_color: c(60, 165, 250), hot_color: c(248, 113, 113), min_temp: 30, max_temp: 85 }, 'Normal', 1.0),
        L({ type: 'Breathing', color: c(255, 255, 255), speed: 0.5 }, 'Multiply', 0.8),
      ],
      70,
    ),
  },
  {
    // Base layer colour-maps temperature; DutyMeter on Multiply makes the
    // fans that are working hardest glow brighter.
    name: 'Workload Aurora',
    blurb: 'Temperature colour, brightness from fan duty',
    zones: zone1(
      [
        L(
          {
            type: 'TemperatureMap',
            gradient: [
              [30, c(60, 120, 220)],
              [55, c(140, 80, 200)],
              [75, c(240, 140, 60)],
              [90, c(248, 70, 70)],
            ],
            glow_on_spike: true,
          },
          'Normal',
          1.0,
        ),
        L({ type: 'DutyMeter', low_color: c(20, 20, 20), high_color: c(255, 255, 255) }, 'Multiply', 0.6),
      ],
      90,
      { delay_per_device_ms: 60, direction: 'Forward' },
    ),
  },
];

// ---------------------------------------------------------------------------
// Device matching (V2 device_id first, V1 hub_serial+channel fallback)
// ---------------------------------------------------------------------------
export interface ResolvedDevice {
  hub: HubSnapshot | null;
  hubIndex: number;
  dev: HubDeviceEntry | null;
}

export function resolveRef(ref: RgbDeviceRef, hubs: HubSnapshot[]): ResolvedDevice {
  for (let i = 0; i < hubs.length; i++) {
    const hub = hubs[i];
    for (const d of hub.devices) {
      if (ref.device_id) {
        if (d.device_id === ref.device_id) return { hub, hubIndex: i, dev: d };
      } else if (hub.serial === ref.hub_serial && d.channel === ref.channel) {
        return { hub, hubIndex: i, dev: d };
      }
    }
  }
  return { hub: null, hubIndex: -1, dev: null };
}

export function frameFor(ref: RgbDeviceRef, frames: RgbFrameDto[]): RgbFrameDto | null {
  if (ref.device_id) {
    const byId = frames.find((f) => f.device_id === ref.device_id);
    if (byId) return byId;
  }
  return frames.find((f) => f.hub_serial === ref.hub_serial && f.channel === ref.channel) ?? null;
}

export function refMatches(ref: RgbDeviceRef, hubSerial: string, dev: HubDeviceEntry): boolean {
  if (ref.device_id && dev.device_id && ref.device_id === dev.device_id) return true;
  return ref.hub_serial === hubSerial && ref.channel === dev.channel;
}

export function refKey(ref: RgbDeviceRef): string {
  return ref.device_id || `${ref.hub_serial}:${ref.channel}`;
}

export function allDeviceRefs(hubs: HubSnapshot[]): RgbDeviceRef[] {
  const out: RgbDeviceRef[] = [];
  for (const hub of hubs) for (const d of hub.devices) out.push({ hub_serial: hub.serial, channel: d.channel, device_id: d.device_id });
  return out;
}

// ---------------------------------------------------------------------------
// Colour conversions for the picker
// ---------------------------------------------------------------------------
export function hexFromColor(k: RgbColor): string {
  return '#' + [k.r, k.g, k.b].map((v) => Math.round(v).toString(16).padStart(2, '0')).join('').toUpperCase();
}
export function colorFromHex(hex: string): RgbColor | null {
  const m = hex.trim().match(/^#?([0-9a-f]{6})$/i);
  if (!m) return null;
  return { r: parseInt(m[1].slice(0, 2), 16), g: parseInt(m[1].slice(2, 4), 16), b: parseInt(m[1].slice(4, 6), 16) };
}
/** h 0..360, s/v 0..1 */
export function rgbToHsv({ r, g, b }: RgbColor): { h: number; s: number; v: number } {
  const R = r / 255,
    G = g / 255,
    B = b / 255;
  const max = Math.max(R, G, B),
    min = Math.min(R, G, B);
  const d = max - min;
  let h = 0;
  if (d > 0) {
    if (max === R) h = 60 * (((G - B) / d) % 6);
    else if (max === G) h = 60 * ((B - R) / d + 2);
    else h = 60 * ((R - G) / d + 4);
  }
  if (h < 0) h += 360;
  return { h, s: max === 0 ? 0 : d / max, v: max };
}
export function hsvToRgb(h: number, s: number, v: number): RgbColor {
  h = ((h % 360) + 360) % 360;
  const k = v * s;
  const x = k * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - k;
  let r = 0,
    g = 0,
    b = 0;
  if (h < 60) [r, g, b] = [k, x, 0];
  else if (h < 120) [r, g, b] = [x, k, 0];
  else if (h < 180) [r, g, b] = [0, k, x];
  else if (h < 240) [r, g, b] = [0, x, k];
  else if (h < 300) [r, g, b] = [x, 0, k];
  else [r, g, b] = [k, 0, x];
  return { r: Math.round((r + m) * 255), g: Math.round((g + m) * 255), b: Math.round((b + m) * 255) };
}

/** Preset swatches (inventory 4.11): white, red, orange, yellow, green, cyan, blue, purple, pink. */
export const COLOR_PRESETS: { name: string; color: RgbColor }[] = [
  { name: 'White', color: c(255, 255, 255) },
  { name: 'Red', color: c(255, 0, 0) },
  { name: 'Orange', color: c(255, 140, 0) },
  { name: 'Yellow', color: c(255, 220, 50) },
  { name: 'Green', color: c(52, 211, 153) },
  { name: 'Cyan', color: c(0, 200, 255) },
  { name: 'Blue', color: c(96, 165, 250) },
  { name: 'Purple', color: c(168, 85, 247) },
  { name: 'Pink', color: c(255, 100, 180) },
];

/**
 * LED count for a hub device. HubDeviceEntry carries no count, so: decoded
 * strips, else the live frame length, else 34 for fans; 0 when frames are
 * streaming and none is sent for the device; null when unknown.
 */
export function ledCountOf(hubSerial: string, dev: HubDeviceEntry, frames: RgbFrameDto[]): number | null {
  if (dev.strips && dev.strips.length > 0) return dev.strips.reduce((a, s) => a + s.led_count, 0);
  const f = frameFor({ hub_serial: hubSerial, channel: dev.channel, device_id: dev.device_id }, frames);
  if (f) return f.leds.length;
  if (/fan/i.test(dev.device_type)) return 34;
  return frames.length > 0 ? 0 : null;
}
