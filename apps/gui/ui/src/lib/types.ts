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
  general: GeneralConfig;
  fan_groups: FanGroupConfig[];
  rgb: RgbConfig;
}

export interface GeneralConfig {
  poll_interval_ms: number;
  log_level: string;
  lhm_exe_path: string | null;
}

export interface FanGroupConfig {
  name: string;
  channels: number[];
  hub_serial: string | null;
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
  hub_serial: string;
  channel: number;
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
  leds: [number, number, number][];
}
