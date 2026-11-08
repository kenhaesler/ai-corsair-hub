// Browser preview mode: initial mock backend data.
//
// Only reachable through the vite `preview` mode aliases (see
// vite.config.ts). Never imported by production code, so none of this ships
// in the Tauri bundle. Shapes mirror the Rust DTOs in apps/gui/src/dto.rs,
// diagnostics.rs and the curated preset asset apps/gui/assets/presets/fans.toml.
//
// The rig is deliberately generic: two iCUE LINK hubs, ten QX fans, a pump,
// a water block and a LINK Adapter with two strips. It is not modelled on any
// real machine's physical layout.

import type {
  AppConfig,
  DetectedDevice,
  FanPresetDef,
  RgbConfig,
  RgbDeviceRef,
  StripSegment,
} from '../lib/types';

/** Distinctive marker; `npm run build` output is grepped for it to prove preview code never ships. */
export const PREVIEW_FIXTURE_MARKER = 'corsair-hub-preview-fixture-v1';

export interface MockDevice {
  hub_serial: string;
  channel: number;
  device_type: string;
  model: number;
  device_id: string;
  /** Effective LED count (firmware 0x1d table value). 0 = no LEDs. */
  led_count: number;
  /** Fans and pumps report RPM; blocks and adapters don't. */
  kind: 'fan' | 'pump' | 'none';
  strips?: StripSegment[];
}

export interface MockHub {
  serial: string;
  firmware: string;
  devices: MockDevice[];
}

export const HUB_A = '5A17C0DE9B3F4E21A8D6F0C4B2E97A11';
export const HUB_B = '9E04B6F3C1A84D7FB25E6A0D8C3F1B22';

/** Stable 26-hex device ids, burned-in style (prefix 0100 like real LINK ids). */
function devId(hubIdx: number, channel: number): string {
  const hash = (((hubIdx + 1) * 0x9e3779b1) ^ (channel * 0x85ebca6b)) >>> 0;
  const hex = hash.toString(16).toUpperCase().padStart(8, '0');
  const tail = (hubIdx * 32 + channel).toString(16).toUpperCase().padStart(4, '0');
  // 4 + 8 + 10 + 4 = 26 hex chars.
  return `0100${hex}5A3C1E0F7B${tail}`;
}

function qx(hub: string, hubIdx: number, channel: number): MockDevice {
  return {
    hub_serial: hub,
    channel,
    device_type: 'QX Fan',
    model: 0,
    device_id: devId(hubIdx, channel),
    led_count: 34,
    kind: 'fan',
  };
}

export function buildHubs(): MockHub[] {
  return [
    {
      serial: HUB_A,
      firmware: '2.7.236',
      devices: [
        qx(HUB_A, 0, 1),
        qx(HUB_A, 0, 2),
        qx(HUB_A, 0, 3),
        qx(HUB_A, 0, 4),
        qx(HUB_A, 0, 5),
        qx(HUB_A, 0, 6),
        {
          hub_serial: HUB_A,
          channel: 7,
          device_type: 'LINK Adapter',
          model: 0,
          device_id: devId(0, 7),
          led_count: 80,
          kind: 'none',
          strips: [
            { index: 0, led_start: 0, led_count: 40 },
            { index: 1, led_start: 40, led_count: 40 },
          ],
        },
      ],
    },
    {
      serial: HUB_B,
      firmware: '2.7.236',
      devices: [
        qx(HUB_B, 1, 1),
        qx(HUB_B, 1, 2),
        qx(HUB_B, 1, 3),
        qx(HUB_B, 1, 4),
        {
          hub_serial: HUB_B,
          channel: 5,
          device_type: 'XD5 Pump',
          model: 0,
          device_id: devId(1, 5),
          led_count: 22,
          kind: 'pump',
        },
        {
          hub_serial: HUB_B,
          channel: 6,
          device_type: 'Water Block',
          model: 0,
          device_id: devId(1, 6),
          led_count: 24,
          kind: 'none',
        },
      ],
    },
  ];
}

export const PSU_SERIAL = 'HX1500I-PREVIEW-0001';

export function buildDetected(): { detected: DetectedDevice[]; excluded_count: number } {
  // Sorted by (support rank, name, key) exactly as dto::detected_devices does.
  const detected: DetectedDevice[] = [
    {
      key: `1B1C:1C1F:${PSU_SERIAL}`,
      vid: 0x1b1c,
      pid: 0x1c1f,
      name: 'HX1500i PSU',
      family: 'HidPsu',
      support: 'supported',
      serial: PSU_SERIAL,
      interface_count: 1,
      bootloader: false,
      status: 'ok',
    },
    {
      key: `1B1C:0C3F:${HUB_A}`,
      vid: 0x1b1c,
      pid: 0x0c3f,
      name: 'iCUE LINK System Hub',
      family: 'IcueLinkHub',
      support: 'supported',
      serial: HUB_A,
      interface_count: 2,
      bootloader: false,
      status: 'ok',
    },
    {
      key: `1B1C:0C3F:${HUB_B}`,
      vid: 0x1b1c,
      pid: 0x0c3f,
      name: 'iCUE LINK System Hub',
      family: 'IcueLinkHub',
      support: 'supported',
      serial: HUB_B,
      interface_count: 2,
      bootloader: false,
      status: 'ok',
    },
    {
      key: '1B1C:1B4F:K68-PREVIEW-77',
      vid: 0x1b1c,
      pid: 0x1b4f,
      name: 'Corsair K68 RGB',
      family: 'Keyboard',
      support: 'detect_only',
      serial: 'K68-PREVIEW-77',
      interface_count: 3,
      bootloader: false,
      status: 'ok',
    },
    {
      key: '1B1C:1B1E:usb:7&2F1A90C&0&4',
      vid: 0x1b1c,
      pid: 0x1b1e,
      name: 'Corsair SCIMITAR RGB',
      family: 'Mouse',
      support: 'detect_only',
      serial: null,
      interface_count: 2,
      bootloader: false,
      status: 'not_ready',
    },
  ];
  return { detected, excluded_count: 1 };
}

/** Curated preset library — values copied from apps/gui/assets/presets/fans.toml. */
export const FAN_PRESETS: FanPresetDef[] = [
  {
    id: 'silent',
    name: 'Silent',
    description: 'Quiet default — relies on radiator thermal mass',
    hysteresis: 5.0,
    ramp_up: 3.0,
    ramp_down: 1.5,
    cpu_weight: 0.6,
    gpu_weight: 0.4,
    water_weight: 0.0,
    power_feed_forward: 2.0,
    points: [
      { temp: 30, duty: 20 },
      { temp: 50, duty: 30 },
      { temp: 65, duty: 50 },
      { temp: 80, duty: 80 },
      { temp: 90, duty: 100 },
    ],
  },
  {
    id: 'balanced',
    name: 'Balanced',
    description: 'Daily driver — quiet at idle, cool under load',
    hysteresis: 4.0,
    ramp_up: 4.0,
    ramp_down: 2.0,
    cpu_weight: 0.5,
    gpu_weight: 0.5,
    water_weight: 0.0,
    power_feed_forward: 4.0,
    points: [
      { temp: 30, duty: 22 },
      { temp: 50, duty: 32 },
      { temp: 65, duty: 55 },
      { temp: 78, duty: 85 },
      { temp: 88, duty: 100 },
    ],
  },
  {
    id: 'gaming',
    name: 'Gaming',
    description: 'GPU-weighted, ramps early on power spikes',
    hysteresis: 3.0,
    ramp_up: 6.0,
    ramp_down: 3.0,
    cpu_weight: 0.4,
    gpu_weight: 0.6,
    water_weight: 0.0,
    power_feed_forward: 6.0,
    points: [
      { temp: 30, duty: 25 },
      { temp: 50, duty: 40 },
      { temp: 62, duty: 60 },
      { temp: 72, duty: 85 },
      { temp: 80, duty: 100 },
    ],
  },
  {
    id: 'performance',
    name: 'Performance',
    description: 'Aggressive sustained — keeps temps low for hours',
    hysteresis: 2.0,
    ramp_up: 10.0,
    ramp_down: 5.0,
    cpu_weight: 0.5,
    gpu_weight: 0.5,
    water_weight: 0.0,
    power_feed_forward: 6.0,
    points: [
      { temp: 30, duty: 30 },
      { temp: 40, duty: 50 },
      { temp: 55, duty: 70 },
      { temp: 70, duty: 90 },
      { temp: 80, duty: 100 },
    ],
  },
  {
    id: 'benchmark',
    name: 'Benchmark / Stress',
    description: 'Cinebench, 3DMark, OCCT — max cooling, loud by design',
    hysteresis: 1.0,
    ramp_up: 12.0,
    ramp_down: 6.0,
    cpu_weight: 0.5,
    gpu_weight: 0.5,
    water_weight: 0.0,
    power_feed_forward: 8.0,
    points: [
      { temp: 30, duty: 50 },
      { temp: 45, duty: 70 },
      { temp: 55, duty: 85 },
      { temp: 65, duty: 95 },
      { temp: 72, duty: 100 },
    ],
  },
  {
    id: 'water_priority',
    name: 'Water Priority',
    description: 'Targets coolant temperature — the right control variable for a custom loop; falls back to CPU/GPU without a coolant sensor',
    hysteresis: 2.0,
    ramp_up: 6.0,
    ramp_down: 3.0,
    cpu_weight: 0.6,
    gpu_weight: 0.4,
    water_weight: 1.0,
    power_feed_forward: 3.0,
    points: [
      { temp: 28, duty: 30 },
      { temp: 34, duty: 55 },
      { temp: 40, duty: 80 },
      { temp: 45, duty: 100 },
    ],
  },
  {
    id: 'case_airflow',
    name: 'Case airflow',
    description: 'For exhaust fans: follows motherboard temperature, keeps memory and chipset cool',
    hysteresis: 2.0,
    ramp_up: 4.0,
    ramp_down: 2.0,
    cpu_weight: 0.0,
    gpu_weight: 0.0,
    water_weight: 0.0,
    board_weight: 1.0,
    power_feed_forward: 0.0,
    points: [
      { temp: 35, duty: 30 },
      { temp: 45, duty: 45 },
      { temp: 55, duty: 65 },
      { temp: 65, duty: 100 },
    ],
  },
];

/** Sensors the mock control loop has initialized (sorted). */
export const AVAILABLE_SENSORS = ['board_system', 'chipset', 'cpu', 'cpu_ccd', 'dimm', 'gpu', 'gpu_memory', 'psu_case', 'psu_vrm', 'vrm'];

function refs(hubs: MockHub[], pred: (d: MockDevice) => boolean): RgbDeviceRef[] {
  return hubs.flatMap((h) =>
    h.devices.filter(pred).map((d) => ({ hub_serial: h.serial, channel: d.channel, device_id: d.device_id })),
  );
}

function ids(hubs: MockHub[], hub: string, channels: number[]): string[] {
  const h = hubs.find((x) => x.serial === hub)!;
  return channels.map((c) => h.devices.find((d) => d.channel === c)!.device_id);
}

function buildRgb(hubs: MockHub[]): RgbConfig {
  const fans = refs(hubs, (d) => d.device_type === 'QX Fan');
  const cooling = refs(hubs, (d) => d.kind === 'pump' || d.device_type === 'Water Block');
  const strips = refs(hubs, (d) => d.device_type === 'LINK Adapter');
  return {
    enabled: true,
    brightness: 80,
    fps: 30,
    hardware_output: true,
    zones: [
      {
        name: 'Case Fans',
        devices: fans,
        layers: [
          {
            effect: { type: 'RainbowWave', speed: 0.4, wavelength: 2.0 },
            blend_mode: 'Normal',
            opacity: 1.0,
            enabled: true,
          },
        ],
        brightness: 100,
        flow: { delay_per_device_ms: 120, direction: 'Forward' },
      },
      {
        name: 'Cooling Loop',
        devices: cooling,
        layers: [
          {
            effect: {
              type: 'ThermalPulse',
              cold_color: { r: 40, g: 120, b: 255 },
              hot_color: { r: 255, g: 60, b: 30 },
              min_temp: 35,
              max_temp: 80,
            },
            blend_mode: 'Normal',
            opacity: 1.0,
            enabled: true,
          },
        ],
        brightness: 100,
        flow: null,
      },
      {
        name: 'Accent Strips',
        devices: strips,
        layers: [
          {
            effect: {
              type: 'Gradient',
              colors: [
                { r: 120, g: 60, b: 255 },
                { r: 0, g: 200, b: 255 },
                { r: 255, g: 40, b: 160 },
              ],
              speed: 0.3,
            },
            blend_mode: 'Normal',
            opacity: 1.0,
            enabled: true,
          },
          {
            effect: { type: 'Starfield', density: 0.3, speed: 1.0 },
            blend_mode: 'Screen',
            opacity: 0.6,
            enabled: true,
          },
        ],
        brightness: 90,
        flow: null,
      },
    ],
    presets: [
      {
        name: 'Arctic',
        zones: [
          {
            name: 'Everything',
            devices: [...fans, ...cooling, ...strips],
            layers: [
              {
                effect: { type: 'Breathing', color: { r: 140, g: 210, b: 255 }, speed: 0.5 },
                blend_mode: 'Normal',
                opacity: 1.0,
                enabled: true,
              },
            ],
            brightness: 100,
            flow: null,
          },
        ],
      },
    ],
  };
}

export function buildConfig(hubs: MockHub[]): AppConfig {
  const balanced = FAN_PRESETS.find((p) => p.id === 'balanced')!;
  return {
    schema_version: 2,
    general: { poll_interval_ms: 1000, log_level: 'info', lhm_exe_path: null },
    fan_groups: [
      {
        name: 'Front Intake',
        channels: [],
        hub_serial: null,
        device_ids: ids(hubs, HUB_A, [1, 2, 3]),
        mode: {
          type: 'curve',
          points: balanced.points.map((p) => ({ ...p })),
          hysteresis: balanced.hysteresis,
          ramp_rate: balanced.ramp_up,
          ramp_down_rate: balanced.ramp_down,
          temp_source: { sensors: ['cpu', 'gpu'], weights: [0.6, 0.4] },
        },
        preset_applied: 'balanced',
      },
      {
        name: 'Top Exhaust',
        channels: [],
        hub_serial: null,
        device_ids: ids(hubs, HUB_A, [4, 5, 6]),
        mode: {
          type: 'curve',
          points: [
            { temp: 35, duty: 30 },
            { temp: 55, duty: 45 },
            { temp: 70, duty: 70 },
            { temp: 82, duty: 100 },
          ],
          hysteresis: 2.0,
          ramp_rate: 6.0,
          ramp_down_rate: 2.5,
          temp_source: { sensors: ['cpu'], weights: [1.0] },
        },
        preset_applied: null,
      },
      {
        name: 'Bottom Intake',
        channels: [],
        hub_serial: null,
        device_ids: ids(hubs, HUB_B, [1, 2, 3]),
        mode: {
          type: 'pid',
          target_temp: 60,
          kp: 2.0,
          ki: 0.1,
          kd: 0.5,
          min_duty: 25,
          max_duty: 100,
          temp_source: { sensors: ['gpu'], weights: [1.0] },
        },
        preset_applied: null,
      },
      {
        name: 'Rear Exhaust',
        channels: [],
        hub_serial: null,
        device_ids: ids(hubs, HUB_B, [4]),
        mode: { type: 'fixed', duty_percent: 45 },
        preset_applied: null,
      },
      {
        name: 'Pump',
        channels: [],
        hub_serial: null,
        device_ids: ids(hubs, HUB_B, [5]),
        mode: { type: 'fixed', duty_percent: 70 },
        preset_applied: null,
      },
    ],
    rgb: buildRgb(hubs),
    devices: [
      { device_id: ids(hubs, HUB_B, [5])[0], name: 'Pump' },
      { device_id: ids(hubs, HUB_B, [6])[0], name: 'CPU Block' },
      { device_id: ids(hubs, HUB_A, [7])[0], name: 'Accent Strips' },
      { device_id: ids(hubs, HUB_B, [4])[0], name: 'Rear Fan' },
    ],
  };
}

/** Channel-identify palette — same order as IDENTIFY_PALETTE in hardware_thread.rs. */
export const IDENTIFY_PALETTE: [[number, number, number], string][] = [
  [[255, 0, 0], 'red'],
  [[255, 128, 0], 'orange'],
  [[255, 255, 0], 'yellow'],
  [[0, 255, 0], 'green'],
  [[0, 255, 255], 'cyan'],
  [[0, 128, 255], 'blue'],
  [[128, 0, 255], 'purple'],
  [[255, 0, 255], 'magenta'],
  [[255, 0, 128], 'pink'],
  [[255, 255, 255], 'white'],
  [[128, 128, 0], 'olive'],
  [[0, 128, 128], 'teal'],
  [[128, 0, 0], 'dark red'],
  [[0, 128, 0], 'dark green'],
  [[0, 0, 128], 'dark blue'],
  [[255, 128, 128], 'pastel red'],
];

export function identifyColor(hubIdx: number, channel: number): [[number, number, number], string] {
  return IDENTIFY_PALETTE[(hubIdx * 8 + channel) % IDENTIFY_PALETTE.length];
}
