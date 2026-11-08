// Browser preview mode: in-memory mock of the Tauri backend.
//
// One handler per command in src/lib/api.ts (names match the Rust
// #[tauri::command] fns in apps/gui/src/commands.rs; argument keys are the
// camelCase names the frontend sends). A small simulator emits the same
// events the hardware thread emits (`sensor-update`, `rgb-frame`) with
// slowly-varying, internally consistent values: temps follow a drifting
// workload, group duties follow each group's fixed/curve/PID mode, RPMs follow
// duty, PSU power follows CPU+GPU power.
//
// Errors are rejected as plain strings, like Tauri's `Result<_, String>`.

import type {
  AppConfig,
  CpuMetricsDto,
  DeviceTree,
  FanGroupConfig,
  FanPresetDef,
  FanReading,
  GpuMetricsDto,
  GroupDuty,
  PsuSnapshot,
  RgbConfig,
  RgbFrameDto,
  SystemSnapshot,
  TempSourceConfig,
} from '../lib/types';
import type { ChannelIdentity, LedMarkerReport, MarkerHub, MarkerLegendEntry, MarkerMode } from '../lib/api';
import { emit, hasListeners, onSubscribe } from './bus';
import {
  AVAILABLE_SENSORS,
  FAN_PRESETS,
  PREVIEW_FIXTURE_MARKER,
  PSU_SERIAL,
  buildConfig,
  buildDetected,
  buildHubs,
  identifyColor,
  type MockDevice,
} from './fixtures';
import { renderFrames } from './rgb-render';

declare const __PREVIEW_APP_VERSION__: string;

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

// ---------------------------------------------------------------- state

const hubs = buildHubs();
const allDevices: MockDevice[] = hubs.flatMap((h) => h.devices);
let config: AppConfig = buildConfig(hubs);
let rgbEnabled = config.rgb.enabled;

/** Seeded PRNG so a fresh page load produces a repeatable trace. */
let seed = 0x2f6e2b1;
function rand(): number {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = seed;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const noise = (amp: number) => (rand() * 2 - 1) * amp;

interface Sim {
  /** Workload 0..1, eased toward a target that changes every ~25 s. */
  cpuLoad: number;
  gpuLoad: number;
  cpuTarget: number;
  gpuTarget: number;
  retargetIn: number;
  cpuTemp: number;
  gpuTemp: number;
  psuVrm: number;
  psuCase: number;
  psuFan: number;
  groupDuty: Map<string, number>;
  pidIntegral: Map<string, number>;
  pidLastErr: Map<string, number>;
  rpm: Map<string, number>;
  rpmJitter: Map<string, number>;
  /** device_id -> duty from set_manual_duty*; grouped devices hold it for one cycle (next cycle overwrites, as on real hardware). */
  manual: Map<string, number>;
  last: SystemSnapshot | null;
}

const sim: Sim = {
  cpuLoad: 0.25,
  gpuLoad: 0.3,
  cpuTarget: 0.35,
  gpuTarget: 0.45,
  retargetIn: 20,
  cpuTemp: 46,
  gpuTemp: 50,
  psuVrm: 42,
  psuCase: 36,
  psuFan: 0,
  groupDuty: new Map(),
  pidIntegral: new Map(),
  pidLastErr: new Map(),
  rpm: new Map(),
  rpmJitter: new Map(allDevices.map((d) => [d.device_id, 1 + noise(0.04)])),
  manual: new Map(),
  last: null,
};

/** Active diagnostic overriding the RGB renderer (identify / LED markers). */
let diagnostic: { until: number; frames: (now: number) => RgbFrameDto[] } | null = null;

// ---------------------------------------------------------------- simulation

function readings(): Record<string, number> {
  return { cpu: sim.cpuTemp, gpu: sim.gpuTemp, psu_vrm: sim.psuVrm, psu_case: sim.psuCase };
}

function fusedTemp(src: TempSourceConfig): number {
  const r = readings();
  let sum = 0;
  let wsum = 0;
  src.sensors.forEach((s, i) => {
    const w = src.weights[i] ?? 0;
    if (r[s] !== undefined && w > 0) {
      sum += r[s] * w;
      wsum += w;
    }
  });
  return wsum > 0 ? sum / wsum : sim.cpuTemp;
}

function interpCurve(points: { temp: number; duty: number }[], temp: number): number {
  const pts = [...points].sort((a, b) => a.temp - b.temp);
  if (pts.length === 0) return 50;
  if (temp <= pts[0].temp) return pts[0].duty;
  for (let i = 1; i < pts.length; i++) {
    if (temp <= pts[i].temp) {
      const k = (temp - pts[i - 1].temp) / Math.max(0.001, pts[i].temp - pts[i - 1].temp);
      return pts[i - 1].duty + (pts[i].duty - pts[i - 1].duty) * k;
    }
  }
  return pts[pts.length - 1].duty;
}

function stepGroup(g: FanGroupConfig, dt: number): number {
  const prev = sim.groupDuty.get(g.name);
  const m = g.mode;
  let next: number;
  if (m.type === 'fixed') {
    next = m.duty_percent;
  } else if (m.type === 'curve') {
    const target = interpCurve(m.points, fusedTemp(m.temp_source));
    if (prev === undefined) {
      next = target;
    } else if (Math.abs(target - prev) < m.hysteresis * 0.5) {
      next = prev;
    } else {
      const up = m.ramp_rate * dt;
      const down = (m.ramp_down_rate ?? m.ramp_rate * 0.4) * dt;
      next = target > prev ? Math.min(target, prev + up) : Math.max(target, prev - down);
    }
  } else {
    const err = fusedTemp(m.temp_source) - m.target_temp;
    const integral = clamp((sim.pidIntegral.get(g.name) ?? 0) + err * dt, -200, 200);
    const deriv = (err - (sim.pidLastErr.get(g.name) ?? err)) / Math.max(dt, 0.001);
    sim.pidIntegral.set(g.name, integral);
    sim.pidLastErr.set(g.name, err);
    const base = (m.min_duty + m.max_duty) / 2;
    next = clamp(base + m.kp * err + m.ki * integral + m.kd * deriv, m.min_duty, m.max_duty);
  }
  next = clamp(next, 0, 100);
  sim.groupDuty.set(g.name, next);
  return next;
}

function groupDeviceIds(g: FanGroupConfig): string[] {
  if (g.device_ids && g.device_ids.length > 0) return g.device_ids;
  return allDevices
    .filter((d) => d.hub_serial === g.hub_serial && (g.channels ?? []).includes(d.channel))
    .map((d) => d.device_id);
}

function targetRpm(dev: MockDevice, duty: number): number {
  const j = sim.rpmJitter.get(dev.device_id) ?? 1;
  if (dev.kind === 'pump') return (1150 + 36 * duty) * j;
  if (duty <= 0) return 0;
  return (240 + 21.6 * duty) * j; // QX: ~2400 RPM at 100%
}

function step(dt: number): SystemSnapshot {
  // Workload drift.
  sim.retargetIn -= dt;
  if (sim.retargetIn <= 0) {
    sim.retargetIn = 18 + rand() * 20;
    const heavy = rand() < 0.35;
    sim.cpuTarget = heavy ? 0.55 + rand() * 0.4 : 0.08 + rand() * 0.35;
    sim.gpuTarget = heavy ? 0.5 + rand() * 0.45 : 0.05 + rand() * 0.4;
  }
  const ease = 1 - Math.exp(-dt / 6);
  sim.cpuLoad = clamp(sim.cpuLoad + (sim.cpuTarget - sim.cpuLoad) * ease + noise(0.02), 0.02, 1);
  sim.gpuLoad = clamp(sim.gpuLoad + (sim.gpuTarget - sim.gpuLoad) * ease + noise(0.015), 0.01, 1);

  const avgFanDuty = sim.groupDuty.size
    ? [...sim.groupDuty.values()].reduce((a, b) => a + b, 0) / sim.groupDuty.size
    : 40;
  const cooling = (avgFanDuty - 40) * 0.08;
  const heat = 1 - Math.exp(-dt / 10);
  sim.cpuTemp += (36 + 46 * sim.cpuLoad - cooling - sim.cpuTemp) * heat + noise(0.35);
  sim.gpuTemp += (34 + 40 * sim.gpuLoad - cooling * 0.7 - sim.gpuTemp) * heat + noise(0.2);

  const cpuPower = 28 + 190 * sim.cpuLoad + noise(3);
  const gpuPower = 22 + 380 * sim.gpuLoad + noise(5);
  const total = cpuPower + gpuPower + 78 + noise(2);
  sim.psuVrm += (33 + total / 28 - sim.psuVrm) * (1 - Math.exp(-dt / 20)) + noise(0.1);
  sim.psuCase += (30 + total / 55 - sim.psuCase) * (1 - Math.exp(-dt / 25)) + noise(0.08);
  const psuFanTarget = total < 500 ? 0 : 500 + (total - 500) * 1.1;
  sim.psuFan += (psuFanTarget - sim.psuFan) * (1 - Math.exp(-dt / 4));

  // Fan groups -> per-device duty.
  const deviceDuty = new Map<string, { duty: number; group: string | null }>();
  const groupDuties: GroupDuty[] = [];
  for (const g of config.fan_groups) {
    const duty = stepGroup(g, dt);
    const members = groupDeviceIds(g);
    for (const id of members) deviceDuty.set(id, { duty, group: g.name });
    groupDuties.push({ name: g.name, duty_percent: Math.round(duty) });
  }
  for (const [id, duty] of sim.manual) {
    const entry = deviceDuty.get(id);
    deviceDuty.set(id, { duty, group: entry?.group ?? null });
    if (entry) sim.manual.delete(id); // control loop overwrites grouped fans next cycle
  }

  const fans: FanReading[] = [];
  for (const dev of allDevices) {
    if (dev.kind === 'none') continue;
    const d = deviceDuty.get(dev.device_id) ?? { duty: 0, group: null };
    const prev = sim.rpm.get(dev.device_id) ?? targetRpm(dev, d.duty);
    const rpm = Math.max(0, prev + (targetRpm(dev, d.duty) - prev) * (1 - Math.exp(-dt / 1.5)) + noise(8));
    sim.rpm.set(dev.device_id, rpm);
    fans.push({
      hub_serial: dev.hub_serial,
      channel: dev.channel,
      device_id: dev.device_id,
      rpm: Math.round(rpm),
      duty_percent: Math.round(d.duty),
      group_name: d.group,
    });
  }

  const rail12 = (total - 32) / 12.04;
  const psu: PsuSnapshot = {
    temp_vrm: round1(sim.psuVrm),
    temp_case: round1(sim.psuCase),
    fan_rpm: Math.round(sim.psuFan),
    input_voltage: round1(230.2 + noise(0.6)),
    rails: [
      { name: '12V', voltage: round2(12.06 - rail12 * 0.0015), current: round2(rail12), power: round1(rail12 * 12.04) },
      { name: '5V', voltage: round2(5.02 + noise(0.005)), current: round2(3.4 + noise(0.2)), power: round1(17 + noise(1)) },
      { name: '3.3V', voltage: round2(3.33 + noise(0.004)), current: round2(2.1 + noise(0.1)), power: round1(7 + noise(0.4)) },
    ],
    total_power: round1(total),
  };

  const cpu_metrics: CpuMetricsDto = {
    load_pct: round1(sim.cpuLoad * 100),
    power_watts: round1(cpuPower),
    clock_mhz: Math.round(4300 + 1350 * Math.sqrt(sim.cpuLoad) + noise(40)),
  };
  const gpu_metrics: GpuMetricsDto = {
    utilization_pct: Math.round(sim.gpuLoad * 100),
    power_watts: round1(gpuPower),
    clock_mhz: Math.round(sim.gpuLoad < 0.05 ? 210 : 1500 + 1250 * sim.gpuLoad + noise(20)),
    memory_used_mb: Math.round(1900 + 11000 * sim.gpuLoad),
    temp_celsius: round1(sim.gpuTemp),
  };

  const temperatures = Object.entries(readings())
    .map(([source, celsius]) => ({ source, celsius: round1(celsius) }))
    .sort((a, b) => a.source.localeCompare(b.source));

  const snap: SystemSnapshot = {
    timestamp_ms: Date.now(),
    temperatures,
    fans,
    psu,
    group_duties: groupDuties,
    emergency: false,
    any_stale: false,
    hub_health: hubs.map((h) => ({ serial: h.serial, healthy: true, consecutive_failures: 0 })),
    gpu_metrics,
    cpu_metrics,
    // one D5-class pump on a motherboard header, modulated by the BIOS
    pumps: [
      {
        id: '/lpc/preview/0/fan/1',
        name: 'Pump Fan #1',
        rpm: Math.round(2600 + 1400 * Math.min(1, Math.max(0, (sim.cpuTemp - 45) / 35)) + noise(25)),
        duty_pct: Math.round(60 + 35 * Math.min(1, Math.max(0, (sim.cpuTemp - 45) / 35))),
        source: 'motherboard',
      },
    ],
  };
  sim.last = snap;
  return snap;
}

function round1(v: number) {
  return Math.round(v * 10) / 10;
}
function round2(v: number) {
  return Math.round(v * 100) / 100;
}

// ---------------------------------------------------------------- event loops

let started = false;
let sensorTimer: ReturnType<typeof setTimeout> | undefined;
let rgbTimer: ReturnType<typeof setInterval> | undefined;
let backfilled = false;
const t0 = performance.now();

function scheduleSensor() {
  clearTimeout(sensorTimer);
  sensorTimer = setTimeout(() => {
    emit('sensor-update', step(config.general.poll_interval_ms / 1000));
    scheduleSensor();
  }, config.general.poll_interval_ms);
}

function restartRgbTimer() {
  clearInterval(rgbTimer);
  // Real backend renders at rgb.fps (default 30); cap at 30 to stay light in the browser.
  const fps = clamp(config.rgb.fps || 30, 1, 30);
  rgbTimer = setInterval(() => {
    if (!hasListeners('rgb-frame')) return;
    const now = performance.now();
    if (diagnostic && Date.now() < diagnostic.until) {
      emit('rgb-frame', diagnostic.frames(Date.now()));
      return;
    }
    diagnostic = null;
    if (!rgbEnabled) return; // renderer idle: no rgb-frame events
    const s = sim.last;
    emit(
      'rgb-frame',
      renderFrames(config.rgb, allDevices, (now - t0) / 1000, {
        cpuTemp: s?.temperatures.find((t) => t.source === 'cpu')?.celsius ?? sim.cpuTemp,
        gpuTemp: sim.gpuTemp,
        gpuUtil: s?.gpu_metrics?.utilization_pct ?? 0,
        avgDuty: s && s.group_duties.length ? s.group_duties.reduce((a, g) => a + g.duty_percent, 0) / s.group_duties.length : 0,
      }),
    );
  }, 1000 / fps);
}

export function ensureStarted(): void {
  if (started) return;
  started = true;
  console.info(`[preview] mock backend active (${PREVIEW_FIXTURE_MARKER}) — no Tauri, no hardware`);
  // Warm the simulator so the first snapshot is settled.
  for (let i = 0; i < 30; i++) step(1);
  onSubscribe((event) => {
    // The sensors store keeps 5 min of history; seed ~2 min of past samples
    // once so charts and sparklines aren't empty on first paint.
    if (event === 'sensor-update' && !backfilled) {
      backfilled = true;
      setTimeout(() => {
        const n = 120;
        const now = Date.now();
        for (let i = n; i > 0; i--) {
          const snap = step(1);
          snap.timestamp_ms = now - i * 1000;
          emit('sensor-update', snap);
        }
      }, 0);
    }
  });
  scheduleSensor();
  restartRgbTimer();
}

// ---------------------------------------------------------------- validation

function validateFull(cfg: AppConfig): string | null {
  const g = cfg.general;
  if (!Number.isFinite(g.poll_interval_ms) || g.poll_interval_ms < 100 || g.poll_interval_ms > 10000) {
    return `poll_interval_ms must be 100..10000, got ${g.poll_interval_ms}`;
  }
  const names = new Set<string>();
  for (const grp of cfg.fan_groups) {
    if (names.has(grp.name)) return `Duplicate fan group name: '${grp.name}'`;
    names.add(grp.name);
    const hasV2 = (grp.device_ids?.length ?? 0) > 0;
    if (!hasV2) {
      if (!grp.hub_serial) return `Fan group '${grp.name}' missing hub_serial (and no device_ids)`;
      if (!grp.channels || grp.channels.length === 0) return `Fan group '${grp.name}' has no channels (and no device_ids)`;
    }
    const m = grp.mode;
    if (m.type === 'fixed') {
      if (m.duty_percent < 0 || m.duty_percent > 100) {
        return `Fan group '${grp.name}': duty_percent must be 0..100, got ${m.duty_percent}`;
      }
    } else if (m.type === 'curve') {
      if (m.points.length < 2) return `Fan group '${grp.name}': curve requires at least 2 points, got ${m.points.length}`;
      for (const p of m.points) {
        if (p.duty < 0 || p.duty > 100) return `Fan group '${grp.name}': curve duty must be 0..100, got ${p.duty}`;
      }
      const temps = m.points.map((p) => p.temp).sort((a, b) => a - b);
      for (let i = 1; i < temps.length; i++) {
        if (Math.abs(temps[i] - temps[i - 1]) < 0.001) {
          return `Fan group '${grp.name}': curve has duplicate temp points at ${temps[i]}°C`;
        }
      }
      const e = validateTempSource(grp.name, m.temp_source);
      if (e) return e;
    } else {
      if (m.kp <= 0) return `Fan group '${grp.name}': kp must be > 0`;
      if (m.ki < 0) return `Fan group '${grp.name}': ki must be >= 0`;
      if (m.kd < 0) return `Fan group '${grp.name}': kd must be >= 0`;
      if (m.min_duty >= m.max_duty) {
        return `Fan group '${grp.name}': min_duty (${m.min_duty}) must be < max_duty (${m.max_duty})`;
      }
      const e = validateTempSource(grp.name, m.temp_source);
      if (e) return e;
    }
  }
  return null;
}

function validateTempSource(name: string, src: TempSourceConfig): string | null {
  if (src.sensors.length === 0) return `Fan group '${name}': temp_source has no sensors`;
  if (src.sensors.length !== src.weights.length) {
    return `Fan group '${name}': sensors count (${src.sensors.length}) != weights count (${src.weights.length})`;
  }
  for (const w of src.weights) {
    if (w <= 0) return `Fan group '${name}': all weights must be > 0, got ${w}`;
  }
  return null;
}

/** Mirrors hardware_thread::reconcile_preset_applied. */
function reconcilePresetApplied(cfg: AppConfig) {
  for (const g of cfg.fan_groups) {
    if (!g.preset_applied) continue;
    const def = FAN_PRESETS.find((p) => p.id === g.preset_applied);
    const m = g.mode;
    const eq = (a: number, b: number) => Math.abs(a - b) < 0.001;
    const matches =
      !!def &&
      m.type === 'curve' &&
      m.points.length === def.points.length &&
      m.points.every((p, i) => eq(p.temp, def.points[i].temp) && eq(p.duty, def.points[i].duty)) &&
      eq(m.hysteresis, def.hysteresis) &&
      eq(m.ramp_rate, def.ramp_up) &&
      m.ramp_down_rate != null &&
      eq(m.ramp_down_rate, def.ramp_down);
    if (!matches) g.preset_applied = null;
  }
}

function applyConfig(next: AppConfig) {
  const oldPoll = config.general.poll_interval_ms;
  const oldFps = config.rgb.fps;
  config = next;
  // Groups were rebuilt: drop per-group controller state for removed/renamed groups.
  const live = new Set(config.fan_groups.map((g) => g.name));
  for (const k of [...sim.groupDuty.keys()]) if (!live.has(k)) sim.groupDuty.delete(k);
  if (started && oldPoll !== config.general.poll_interval_ms) scheduleSensor();
  if (started && oldFps !== config.rgb.fps) restartRgbTimer();
}

// ---------------------------------------------------------------- commands

function deviceTree(): DeviceTree {
  const { detected, excluded_count } = buildDetected();
  return {
    hubs: hubs.map((h) => ({
      serial: h.serial,
      firmware: h.firmware,
      devices: h.devices.map((d) => {
        const entry = {
          channel: d.channel,
          device_type: d.device_type,
          model: d.model,
          device_id: d.device_id,
          rpm: d.kind === 'none' ? null : Math.round(sim.rpm.get(d.device_id) ?? 0),
        };
        return d.strips ? { ...entry, strips: d.strips } : entry;
      }),
    })),
    psu: { serial: PSU_SERIAL, model: 'HX1500i PSU' },
    detected,
    excluded_count,
  };
}

function applyPreset(presetId: string, groupName: string | null): void {
  const def: FanPresetDef | undefined = FAN_PRESETS.find((p) => p.id === presetId);
  if (!def) throw `Unknown preset: ${presetId}`;
  const sensors: string[] = [];
  const weights: number[] = [];
  for (const [name, w] of [
    ['cpu', def.cpu_weight],
    ['gpu', def.gpu_weight],
    ['water_inline', def.water_weight],
  ] as const) {
    if (w > 0 && AVAILABLE_SENSORS.includes(name)) {
      sensors.push(name);
      weights.push(w);
    }
  }
  if (sensors.length === 0) {
    sensors.push('cpu', 'gpu');
    weights.push(0.6, 0.4);
  }
  const next = clone(config);
  let any = false;
  for (const g of next.fan_groups) {
    if (groupName == null || g.name === groupName) {
      g.mode = {
        type: 'curve',
        points: def.points.map((p) => ({ ...p })),
        hysteresis: def.hysteresis,
        ramp_rate: def.ramp_up,
        ramp_down_rate: def.ramp_down,
        temp_source: { sensors, weights },
      };
      g.preset_applied = def.id;
      any = true;
    }
  }
  if (any) applyConfig(next);
}

function resolveDevice(id: string): MockDevice | undefined {
  return allDevices.find((d) => d.device_id === id);
}

function displayLabel(id: string): string | null {
  return config.devices?.find((d) => d.device_id === id)?.name ?? null;
}

function startIdentify(durationSecs: number): ChannelIdentity[] {
  const out: ChannelIdentity[] = [];
  const colours = new Map<string, [number, number, number]>();
  hubs.forEach((h, hubIdx) => {
    for (const d of h.devices) {
      const [rgb, name] = identifyColor(hubIdx, d.channel);
      colours.set(d.device_id, rgb);
      out.push({
        hub_serial: h.serial,
        channel: d.channel,
        device_id: d.device_id,
        device_type: d.device_type,
        color_name: name,
        rgb,
      });
    }
  });
  const secs = clamp(durationSecs, 3, 120);
  diagnostic = {
    until: Date.now() + secs * 1000,
    frames: () =>
      allDevices
        .filter((d) => d.led_count > 0)
        .map((d) => ({
          hub_serial: d.hub_serial,
          channel: d.channel,
          device_id: d.device_id,
          leds: Array.from({ length: d.led_count }, () => colours.get(d.device_id)!),
        })),
  };
  return out;
}

const WHITE: [number, number, number] = [255, 255, 255];
const GREEN: [number, number, number] = [0, 255, 0];
const RED: [number, number, number] = [255, 0, 0];
const YELLOW: [number, number, number] = [255, 255, 0];
const BLUE: [number, number, number] = [0, 0, 255];
const BLACK: [number, number, number] = [0, 0, 0];

function markerLegend(kind: MarkerMode['kind']): MarkerLegendEntry[] {
  const white = (meaning: string) => ({ label: 'white', rgb: WHITE, meaning });
  const red = (meaning: string) => ({ label: 'red', rgb: RED, meaning });
  const green = { label: 'green', rgb: GREEN, meaning: 'second LED (LED 1, shows direction; slots with 3+ LEDs)' };
  switch (kind) {
    case 'markers_black':
      return [white('first LED of the slot (LED 0)'), green, red('last LED of the slot')];
    case 'ruler':
      return [
        white('first LED of the slot (LED 0)'),
        { label: 'yellow', rgb: YELLOW, meaning: 'every 10th LED (LED 10, 20, 30, ...)' },
        { label: 'blue', rgb: BLUE, meaning: 'every other 5th LED (LED 5, 15, 25, ...)' },
        red('last LED of the slot (overrides the others)'),
      ];
    case 'walk':
      return [
        white('the walking LED on the target slot (one step at a time)'),
        red('last LED of the target slot while the white LED is elsewhere'),
      ];
    default:
      return [
        white('first LED of the slot (LED 0)'),
        green,
        red('last LED of the slot'),
        {
          label: 'body colour',
          rgb: BLACK,
          meaning: "every other LED uses the device's Identify palette colour (see body_color_name per slot)",
        },
      ];
  }
}

function markerLeds(
  mode: MarkerMode,
  dev: MockDevice,
  body: [number, number, number],
  walk: { started: number; step: number } | null,
  now: number,
): [number, number, number][] {
  const n = dev.led_count;
  const leds: [number, number, number][] = [];
  for (let i = 0; i < n; i++) {
    let col: [number, number, number];
    switch (mode.kind) {
      case 'ruler':
        col = i === n - 1 && n >= 2 ? RED : i === 0 ? WHITE : i % 10 === 0 ? YELLOW : i % 5 === 0 ? BLUE : BLACK;
        break;
      case 'walk': {
        if (dev.device_id !== mode.device_id || !walk) {
          col = BLACK;
        } else {
          const cur = Math.floor((now - walk.started) / walk.step) % n;
          col = i === cur ? WHITE : i === n - 1 ? RED : BLACK;
        }
        break;
      }
      default: {
        const fill = mode.kind === 'markers_black' ? BLACK : body;
        col = i === 0 ? WHITE : i === n - 1 && n >= 2 ? RED : i === 1 && n >= 3 ? GREEN : fill;
      }
    }
    leds.push(col);
  }
  return leds;
}

function startMarkers(durationSecs: number, mode: MarkerMode | null | undefined): LedMarkerReport {
  if (!(rgbEnabled && config.rgb.hardware_output)) {
    throw 'Enable RGB with hardware output to run the LED marker diagnostic';
  }
  const m: MarkerMode = mode ?? { kind: 'markers' };
  const secs = clamp(durationSecs, 5, 300);
  if (m.kind === 'walk' && !allDevices.some((d) => d.device_id === m.device_id && d.led_count > 0)) {
    throw `Unknown LED slot for walk mode: ${m.device_id}`;
  }
  const startedAt = Date.now();
  const stepMs = m.kind === 'walk' ? clamp(m.step_ms ?? 400, 100, 2000) : 0;
  const walk = m.kind === 'walk' ? { started: startedAt, step: stepMs } : null;

  const markerHubs: MarkerHub[] = hubs.map((h, hubIdx) => {
    let led = 0;
    const slots = [];
    const skipped = [];
    for (const d of h.devices) {
      if (d.led_count === 0) {
        skipped.push({
          device_id: d.device_id,
          name: displayLabel(d.device_id),
          channel: d.channel,
          device_type: d.device_type,
          count_source: 'type_default',
        });
        continue;
      }
      const [rgb, name] = identifyColor(hubIdx, d.channel);
      slots.push({
        device_id: d.device_id,
        name: displayLabel(d.device_id),
        channel: d.channel,
        device_type: d.device_type,
        led_count: d.led_count,
        count_source: 'firmware_table',
        start_led: led,
        end_led: led + d.led_count,
        start_byte: led * 3,
        body_color_name: name,
        body_rgb: rgb,
      });
      led += d.led_count;
    }
    return {
      hub_serial: h.serial,
      healthy: true,
      firmware: h.firmware,
      firmware_led_counts: Object.fromEntries(h.devices.map((d) => [String(d.channel), d.led_count])),
      total_leds: led,
      slots,
      skipped_zero_count: skipped,
    };
  });

  const totalLeds = markerHubs.reduce((a, h) => a + h.total_leds, 0);
  const iso = new Date(startedAt).toISOString().replace(/\.\d{3}Z$/, 'Z');
  const compact = iso.replace(/[-:]/g, '');
  diagnostic = {
    until: startedAt + secs * 1000,
    frames: (now) =>
      hubs.flatMap((h, hubIdx) =>
        h.devices
          .filter((d) => d.led_count > 0)
          .map((d) => ({
            hub_serial: d.hub_serial,
            channel: d.channel,
            device_id: d.device_id,
            leds: markerLeds(m, d, identifyColor(hubIdx, d.channel)[0], walk, now),
          })),
      ),
  };
  return {
    app_version: __PREVIEW_APP_VERSION__,
    generated_at_utc: iso,
    duration_secs: secs,
    mode: m.kind,
    walk:
      m.kind === 'walk'
        ? {
            device_id: m.device_id,
            started_at: startedAt,
            step_ms: stepMs,
            led_count: resolveDevice(m.device_id)!.led_count,
          }
        : null,
    total_leds: totalLeds,
    power_factor: 1,
    legend: markerLegend(m.kind),
    hubs: markerHubs,
    file_path: `C:\\Users\\preview\\AppData\\Roaming\\corsair-hub\\diagnostics\\led-markers-${compact}.json`,
    file_error: null,
  };
}

type Args = Record<string, unknown>;
type Handler = (args: Args) => unknown;

const handlers: Record<string, Handler> = {
  get_snapshot: () => {
    if (!sim.last) throw 'Hardware thread unavailable';
    return sim.last;
  },
  get_devices: () => deviceTree(),
  get_config: () => config,
  get_startup_config_issue: () => null,
  list_available_sensors: () => AVAILABLE_SENSORS,
  save_config: (a) => {
    const next = clone(a.config as AppConfig);
    const err = validateFull(next);
    if (err) throw `Invalid config: ${err}`;
    reconcilePresetApplied(next);
    applyConfig(next);
    return null;
  },
  validate_config: (a) => {
    const err = validateFull(a.config as AppConfig);
    if (err) throw err;
    return null;
  },
  apply_preset: (a) => {
    applyPreset(a.preset as string, null);
    return null;
  },
  apply_preset_to_group: (a) => {
    applyPreset(a.preset as string, (a.groupName as string | null | undefined) ?? null);
    return null;
  },
  get_fan_presets: () => FAN_PRESETS,
  set_manual_duty: (a) => {
    const duty = a.duty as number;
    if (duty > 100) throw 'duty must be 0-100';
    const hub = hubs.find((h) => h.serial === a.hubSerial);
    if (!hub) throw `Failed to set duty: Hub '${String(a.hubSerial)}' not managed by control loop`;
    for (const ch of a.channels as number[]) {
      const dev = hub.devices.find((d) => d.channel === ch);
      if (dev) sim.manual.set(dev.device_id, duty);
    }
    return null;
  },
  set_manual_duty_by_device_id: (a) => {
    const duty = a.duty as number;
    if (duty > 100) throw 'duty must be 0-100';
    const applied: string[] = [];
    const unresolved: string[] = [];
    for (const id of a.deviceIds as string[]) {
      if (resolveDevice(id)) {
        applied.push(id);
        sim.manual.set(id, duty);
      } else {
        unresolved.push(id);
      }
    }
    return { applied, unresolved };
  },
  rename_device: (a) => {
    const id = a.deviceId as string;
    const name = a.name as string;
    const next = clone(config);
    next.devices = next.devices ?? [];
    if (name === '') {
      next.devices = next.devices.filter((d) => d.device_id !== id);
    } else {
      const existing = next.devices.find((d) => d.device_id === id);
      if (existing) existing.name = name;
      else next.devices.push({ device_id: id, name });
    }
    applyConfig(next);
    return null;
  },
  set_rgb_config: (a) => {
    const next = clone(config);
    next.rgb = clone(a.config as RgbConfig);
    const err = validateFull(next);
    if (err) throw `Invalid RGB config: ${err}`;
    rgbEnabled = next.rgb.enabled;
    diagnostic = null; // any RGB config change ends a running diagnostic
    applyConfig(next);
    return null;
  },
  set_rgb_enabled: (a) => {
    rgbEnabled = a.enabled as boolean;
    diagnostic = null;
    const next = clone(config);
    next.rgb.enabled = rgbEnabled;
    applyConfig(next);
    return null;
  },
  rgb_identify_channels: (a) => startIdentify(a.durationSecs as number),
  rgb_marker_diagnostic: (a) => startMarkers(a.durationSecs as number, a.mode as MarkerMode | undefined),
};

/** Dispatch a command; resolves with a JSON copy (like IPC) or rejects with a string. */
export function dispatch<T>(cmd: string, args: Args = {}): Promise<T> {
  ensureStarted();
  const handler = handlers[cmd];
  return new Promise<T>((resolve, reject) => {
    // Small async hop so callers see real promise timing, not a sync resolve.
    setTimeout(() => {
      if (!handler) {
        reject(`Command ${cmd} not found`);
        return;
      }
      try {
        const out = handler(clone(args ?? {}));
        resolve(out === undefined ? (null as T) : clone(out as T));
      } catch (e) {
        reject(typeof e === 'string' ? e : String(e));
      }
    }, 4);
  });
}
