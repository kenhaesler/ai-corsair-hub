// Browser preview mode: approximate RGB effect renderer.
//
// Not a port of crates/rgb — just close enough that the Lighting page's
// hardware preview animates plausibly for every effect type and reacts to
// config edits (effects, layers, blend modes, brightness, flow).

import type { BlendMode, EffectConfig, FlowConfig, RgbColor, RgbConfig, RgbFrameDto } from '../lib/types';
import type { MockDevice } from './fixtures';

type Rgb = [number, number, number];

export interface RenderInputs {
  cpuTemp: number;
  gpuTemp: number;
  gpuUtil: number;
  avgDuty: number;
}

const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);
const c = (col: RgbColor): Rgb => [col.r, col.g, col.b];
const lerp = (a: Rgb, b: Rgb, t: number): Rgb => [
  a[0] + (b[0] - a[0]) * t,
  a[1] + (b[1] - a[1]) * t,
  a[2] + (b[2] - a[2]) * t,
];
const scale = (a: Rgb, k: number): Rgb => [a[0] * k, a[1] * k, a[2] * k];

function hsv(h: number, s: number, v: number): Rgb {
  h = ((h % 1) + 1) % 1;
  const i = Math.floor(h * 6);
  const f = h * 6 - i;
  const p = v * (1 - s);
  const q = v * (1 - f * s);
  const t = v * (1 - (1 - f) * s);
  const [r, g, b] = [
    [v, t, p],
    [q, v, p],
    [p, v, t],
    [p, q, v],
    [t, p, v],
    [v, p, q],
  ][i % 6];
  return [r * 255, g * 255, b * 255];
}

/** Cheap deterministic 0..1 hash. */
function hash(a: number, b: number, c2 = 0): number {
  let h = (a * 374761393 + b * 668265263 + c2 * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

function gradientAt(stops: Rgb[], t: number): Rgb {
  if (stops.length === 0) return [0, 0, 0];
  if (stops.length === 1) return stops[0];
  const x = (((t % 1) + 1) % 1) * stops.length;
  const i = Math.floor(x);
  return lerp(stops[i % stops.length], stops[(i + 1) % stops.length], x - i);
}

function effectColor(
  e: EffectConfig,
  t: number,
  led: number,
  n: number,
  devOrder: number,
  ins: RenderInputs,
): Rgb {
  const pos = n > 0 ? led / n : 0;
  switch (e.type) {
    case 'Static':
      return c(e.color);
    case 'Breathing':
      return scale(c(e.color), 0.08 + 0.92 * (0.5 - 0.5 * Math.cos(2 * Math.PI * t * e.speed * 0.5)));
    case 'ColorCycle':
      return hsv(t * e.speed * 0.1, e.saturation, 1);
    case 'RainbowWave':
      return hsv(pos / Math.max(0.1, e.wavelength) + devOrder * 0.08 - t * e.speed * 0.5, 1, 1);
    case 'SpectrumShift':
      return hsv(t * e.speed * 0.15, 1, 1);
    case 'Fire': {
      const flick = hash(led, devOrder, Math.floor(t * 12 * e.speed));
      const heat = clamp01((0.35 + 0.65 * flick) * e.intensity);
      return [255 * Math.min(1, heat * 1.4), 140 * heat * heat, 20 * heat * heat * heat];
    }
    case 'Aurora': {
      const w = Math.sin(pos * Math.PI * 2 * e.color_spread + t * e.speed * 1.3 + devOrder);
      const w2 = Math.sin(pos * Math.PI * 4 - t * e.speed * 0.7);
      return lerp([20, 230, 140], [140, 60, 255], clamp01(0.5 + 0.35 * w + 0.15 * w2));
    }
    case 'Candle': {
      const k = 0.55 + 0.45 * hash(devOrder, 7, Math.floor(t * e.flicker_speed * 3));
      return scale(c(e.color), k);
    }
    case 'Starfield': {
      const slot = Math.floor(t * e.speed * 2);
      const on = hash(led, devOrder, slot) < e.density * 0.35;
      const phase = (t * e.speed * 2) % 1;
      return on ? scale([255, 255, 255], Math.sin(phase * Math.PI)) : [0, 0, 0];
    }
    case 'Rain': {
      const lane = hash(devOrder, led % 7, 3);
      const drop = (((t * e.speed * 0.8 + lane) % 1) + 1) % 1;
      const d = Math.abs(pos - drop);
      const k = d < 0.12 * e.density + 0.03 ? 1 - d / (0.12 * e.density + 0.03) : 0;
      return scale(c(e.color), k);
    }
    case 'TemperatureMap': {
      const g = [...e.gradient].sort((a, b) => a[0] - b[0]);
      if (g.length === 0) return [0, 0, 0];
      const temp = ins.cpuTemp;
      if (temp <= g[0][0]) return c(g[0][1]);
      for (let i = 1; i < g.length; i++) {
        if (temp <= g[i][0]) {
          const k = (temp - g[i - 1][0]) / Math.max(0.001, g[i][0] - g[i - 1][0]);
          return lerp(c(g[i - 1][1]), c(g[i][1]), k);
        }
      }
      return c(g[g.length - 1][1]);
    }
    case 'ThermalPulse': {
      const k = clamp01((ins.cpuTemp - e.min_temp) / Math.max(0.001, e.max_temp - e.min_temp));
      const pulse = 0.65 + 0.35 * Math.sin(2 * Math.PI * t * (0.3 + k * 1.2));
      return scale(lerp(c(e.cold_color), c(e.hot_color), k), pulse);
    }
    case 'DutyMeter': {
      const fill = ins.avgDuty / 100;
      return pos <= fill ? lerp(c(e.low_color), c(e.high_color), pos) : [0, 0, 0];
    }
    case 'GpuMeter': {
      const fill = ins.gpuUtil / 100;
      return pos <= fill ? lerp(c(e.low_color), c(e.high_color), pos) : [0, 0, 0];
    }
    case 'Gradient':
      return gradientAt(e.colors.map(c), pos + devOrder * 0.1 + t * e.speed * 0.2);
  }
  return [0, 0, 0];
}

function blend(base: Rgb, top: Rgb, mode: BlendMode, opacity: number): Rgb {
  const out: Rgb = [0, 0, 0];
  for (let i = 0; i < 3; i++) {
    const b = base[i] / 255;
    const tp = top[i] / 255;
    let v: number;
    switch (mode) {
      case 'Add':
        v = Math.min(1, b + tp);
        break;
      case 'Multiply':
        v = b * tp;
        break;
      case 'Screen':
        v = 1 - (1 - b) * (1 - tp);
        break;
      case 'Overlay':
        v = b < 0.5 ? 2 * b * tp : 1 - 2 * (1 - b) * (1 - tp);
        break;
      default:
        v = tp;
    }
    out[i] = (b + (v - b) * opacity) * 255;
  }
  return out;
}

function flowOrder(flow: FlowConfig | null, k: number, count: number): number {
  if (!flow) return 0;
  const center = (count - 1) / 2;
  switch (flow.direction) {
    case 'Reverse':
      return count - 1 - k;
    case 'CenterOut':
      return Math.abs(k - center);
    case 'EdgeIn':
      return center - Math.abs(k - center);
    default:
      return k;
  }
}

/** Render one frame for every device referenced by an enabled zone. */
export function renderFrames(
  rgb: RgbConfig,
  devices: MockDevice[],
  tSec: number,
  ins: RenderInputs,
): RgbFrameDto[] {
  const frames = new Map<string, RgbFrameDto>();
  const master = clamp01(rgb.brightness / 100);
  for (const zone of rgb.zones) {
    const zoneDevs = zone.devices
      .map((ref) =>
        devices.find((d) =>
          ref.device_id ? d.device_id === ref.device_id : d.hub_serial === ref.hub_serial && d.channel === ref.channel,
        ),
      )
      .filter((d): d is MockDevice => !!d && d.led_count > 0);
    const k = master * clamp01(zone.brightness / 100);
    zoneDevs.forEach((dev, idx) => {
      const order = flowOrder(zone.flow, idx, zoneDevs.length);
      const t = tSec - (zone.flow ? (zone.flow.delay_per_device_ms * order) / 1000 : 0);
      const leds: [number, number, number][] = [];
      for (let i = 0; i < dev.led_count; i++) {
        let col: Rgb = [0, 0, 0];
        for (const layer of zone.layers) {
          if (!layer.enabled) continue;
          col = blend(col, effectColor(layer.effect, t, i, dev.led_count, idx, ins), layer.blend_mode, clamp01(layer.opacity));
        }
        leds.push([
          Math.round(Math.min(255, col[0] * k)),
          Math.round(Math.min(255, col[1] * k)),
          Math.round(Math.min(255, col[2] * k)),
        ]);
      }
      frames.set(dev.device_id, { hub_serial: dev.hub_serial, channel: dev.channel, device_id: dev.device_id, leds });
    });
  }
  return [...frames.values()];
}
