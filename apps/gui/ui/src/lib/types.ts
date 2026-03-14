// Mirrors Rust DTOs exactly

export interface SystemSnapshot {
  timestamp_ms: number;
  temperatures: TempReading[];
  fans: FanReading[];
  psu: PsuSnapshot | null;
  group_duties: GroupDuty[];
  emergency: boolean;
  any_stale: boolean;
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
}

export interface GeneralConfig {
  poll_interval_ms: number;
  log_level: string;
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
