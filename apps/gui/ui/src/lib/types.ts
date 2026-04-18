// Mirrors Rust DTOs exactly

export interface SystemSnapshot {
  timestamp_ms: number;
  temperatures: TempReading[];
  fans: FanReading[];
  psu: PsuSnapshot | null;
  group_duties: GroupDuty[];
  emergency: boolean;
  any_stale: boolean;
  hub_health: HubHealth[];
}

export interface HubHealth {
  serial: string;
  healthy: boolean;
  consecutive_failures: number;
}

export interface TempReading {
  source: string;
  celsius: number;
}

export interface FanReading {
  hub_serial: string;
  channel: number;
  /**
   * Stable device identity (26-hex string burned into each device at
   * manufacturing). Empty string when the device isn't in the hub's
   * enumerated list at this moment; callers should fall back to
   * `(hub_serial, channel)` for display in that case.
   */
  device_id: string;
  rpm: number;
  duty_percent: number;
  group_name: string | null;
}

export interface GroupDuty {
  name: string;
  duty_percent: number;
}

export interface PsuSnapshot {
  temp_vrm: number;
  temp_case: number;
  fan_rpm: number;
  input_voltage: number;
  rails: RailSnapshot[];
  total_power: number;
}

export interface RailSnapshot {
  name: string;
  voltage: number;
  current: number;
  power: number;
}

export interface DeviceTree {
  hubs: HubSnapshot[];
  psu: PsuDeviceInfo | null;
}

export interface HubSnapshot {
  serial: string;
  firmware: string;
  devices: HubDeviceEntry[];
}

export interface HubDeviceEntry {
  channel: number;
  device_type: string;
  model: number;
  device_id: string;
  rpm: number | null;
}

export interface PsuDeviceInfo {
  serial: string;
  model: string;
}

// Config types — mirrors Rust AppConfig

export interface AppConfig {
  /**
   * Schema version. Missing on V1 configs (defaulted to 1 by the Rust side);
   * explicitly `2` on V2 configs after migration.
   */
  schema_version?: number;
  general: GeneralConfig;
  fan_groups: FanGroupConfig[];
  rgb: RgbConfig;
  /**
   * V2 per-device metadata (friendly names, LED-count overrides), keyed by
   * `device_id`. Empty on V1 configs; populated after migration.
   */
  devices?: DeviceEntry[];
}

/**
 * V2 per-device metadata row. Keyed by the stable `device_id`. Both `name`
 * and `led_count` are optional — a device not listed here uses its type
 * default.
 */
export interface DeviceEntry {
  device_id: string;
  name?: string;
  led_count?: number;
}

export interface GeneralConfig {
  poll_interval_ms: number;
  log_level: string;
  lhm_exe_path: string | null;
}

export interface FanGroupConfig {
  name: string;
  /** V1 identity: channels on `hub_serial`. Empty on a V2 group. */
  channels?: number[];
  /** V1 identity: hub_serial that owns `channels`. Absent on a V2 group. */
  hub_serial?: string | null;
  /**
   * V2 identity: stable device_ids. Empty on a V1 group; populated on a V2
   * group. When non-empty this is authoritative and `channels` /
   * `hub_serial` are ignored by the control loop.
   */
  device_ids?: string[];
  mode: FanMode;
}

export type FanMode =
  | { type: 'fixed'; duty_percent: number }
  | { type: 'curve'; points: CurvePoint[]; hysteresis: number; ramp_rate: number; temp_source: TempSourceConfig }
  | { type: 'pid'; target_temp: number; kp: number; ki: number; kd: number; min_duty: number; max_duty: number; temp_source: TempSourceConfig };

export interface CurvePoint {
  temp: number;
  duty: number;
}

export interface TempSourceConfig {
  sensors: string[];
  weights: number[];
}

// --- RGB types ---

export interface RgbConfig {
  enabled: boolean;
  brightness: number;
  fps: number;
  hardware_output: boolean;
  zones: RgbZoneConfig[];
  presets: RgbPreset[];
}

export interface RgbZoneConfig {
  name: string;
  devices: RgbDeviceRef[];
  layers: LayerConfig[];
  brightness: number;
  flow: FlowConfig | null;
}

export interface LayerConfig {
  effect: EffectConfig;
  blend_mode: BlendMode;
  opacity: number;
  enabled: boolean;
}

export interface RgbDeviceRef {
  /** V1 identity (defaults to empty string on a V2-only entry). */
  hub_serial: string;
  /** V1 identity (defaults to 0 on a V2-only entry). */
  channel: number;
  /**
   * V2 identity: stable device_id. Empty string on a V1 entry. Callers that
   * produce new entries should populate this field (and keep hub_serial +
   * channel populated during the V1→V2 transition for older loaders).
   */
  device_id?: string;
}

export interface RgbPreset {
  name: string;
  zones: RgbZoneConfig[];
}

export interface FlowConfig {
  delay_per_device_ms: number;
  direction: FlowDirection;
}

export type FlowDirection = 'Forward' | 'Reverse' | 'CenterOut' | 'EdgeIn';

export type BlendMode = 'Normal' | 'Add' | 'Multiply' | 'Screen' | 'Overlay';

export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

export type EffectConfig =
  | { type: 'Static'; color: RgbColor }
  | { type: 'Breathing'; color: RgbColor; speed: number }
  | { type: 'ColorCycle'; speed: number; saturation: number }
  | { type: 'RainbowWave'; speed: number; wavelength: number }
  | { type: 'SpectrumShift'; speed: number }
  | { type: 'Fire'; intensity: number; speed: number }
  | { type: 'Aurora'; speed: number; color_spread: number }
  | { type: 'Candle'; color: RgbColor; flicker_speed: number }
  | { type: 'Starfield'; density: number; speed: number }
  | { type: 'Rain'; color: RgbColor; speed: number; density: number }
  | { type: 'TemperatureMap'; gradient: [number, RgbColor][]; glow_on_spike: boolean }
  | { type: 'ThermalPulse'; cold_color: RgbColor; hot_color: RgbColor; min_temp: number; max_temp: number }
  | { type: 'DutyMeter'; low_color: RgbColor; high_color: RgbColor }
  | { type: 'Gradient'; colors: RgbColor[]; speed: number };

export interface RgbFrameDto {
  hub_serial: string;
  channel: number;
  /**
   * Stable device identity. Always populated by the backend. Prefer this
   * over `(hub_serial, channel)` when correlating a frame to a device_ids
   * list; the location fields are runtime-resolved and may be empty if
   * the device was orphaned between render and emit.
   */
  device_id: string;
  leds: [number, number, number][];
}
