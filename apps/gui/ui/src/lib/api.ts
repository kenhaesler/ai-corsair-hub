import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DeviceTree, FanPresetDef, RgbConfig, StartupConfigIssue, SystemSnapshot } from './types';

/**
 * Result of `set_manual_duty_by_device_id` — mirrors the Rust
 * `ManualDutyResult` DTO. The caller gets a per-id breakdown so a mixed
 * success/unresolved response isn't hidden as a blanket error.
 */
export interface ManualDutyResult {
  applied: string[];
  unresolved: string[];
}

export const getSnapshot = () => invoke<SystemSnapshot>('get_snapshot');
export const getDevices = () => invoke<DeviceTree>('get_devices');
export const getConfig = () => invoke<AppConfig>('get_config');
/** Non-null when config.toml failed to load at startup (file was preserved). */
export const getStartupConfigIssue = () =>
  invoke<StartupConfigIssue | null>('get_startup_config_issue');

/**
 * Names of sensors currently initialized in the control loop. Returned
 * values are identifiers the control loop's weighted-fusion logic
 * understands (e.g. `cpu`, `gpu`, `psu_vrm`, `psu_case`). Used to
 * populate the per-group sensor-weighting chip picker so we never
 * offer a sensor the machine doesn't have.
 */
export const listAvailableSensors = () => invoke<string[]>('list_available_sensors');
export const saveConfig = (config: AppConfig) => invoke('save_config', { config });
/** Apply a curated preset to every configured fan group. */
export const applyPreset = (preset: string) => invoke('apply_preset', { preset });

/**
 * Apply a curated preset to a single named group, or to every group
 * when `groupName` is omitted/null. Introduced in v0.3 for per-group
 * preset control (e.g. "Competitive FPS" on radiators while chassis
 * fans stay on "Quiet Productivity").
 */
export const applyPresetToGroup = (preset: string, groupName: string | null = null) =>
  invoke('apply_preset_to_group', { preset, groupName });

/**
 * Return the full curated fan-preset library. The library is a
 * compile-time asset so this is effectively instant — callers can
 * fetch once at mount and cache in component state.
 */
export const getFanPresets = () => invoke<FanPresetDef[]>('get_fan_presets');

/**
 * @deprecated Use {@link setManualDutyByDeviceId}; channel-based
 * addressing is fragile across topology changes. Retained for one
 * release so the unmigrated UI keeps working.
 */
export const setManualDuty = (hubSerial: string, channels: number[], duty: number) =>
  invoke('set_manual_duty', { hubSerial, channels, duty });

/** V2 duty command: resolves each device_id via the live registry. */
export const setManualDutyByDeviceId = (deviceIds: string[], duty: number) =>
  invoke<ManualDutyResult>('set_manual_duty_by_device_id', { deviceIds, duty });

/**
 * Rename (or clear) a device's friendly name by its stable device_id.
 * Empty `name` clears the entry, reverting to the system-generated
 * fallback label. Persisted atomically on the backend.
 */
export const renameDevice = (deviceId: string, name: string) =>
  invoke('rename_device', { deviceId, name });

export const validateConfig = (config: AppConfig) => invoke('validate_config', { config });
export const setRgbConfig = (config: RgbConfig) => invoke('set_rgb_config', { config });
export const setRgbEnabled = (enabled: boolean) => invoke('set_rgb_enabled', { enabled });

/**
 * One row in the channel-identify diagnostic mapping returned by
 * `rgbIdentifyChannels`. Mirrors the Rust `ChannelIdentity` DTO.
 */
export interface ChannelIdentity {
  hub_serial: string;
  channel: number;
  device_id: string;
  device_type: string;
  color_name: string;
  rgb: [number, number, number];
}

/**
 * Paint every enumerated RGB device a distinct colour for
 * `durationSecs` seconds so the user can correlate physical
 * positions in their rig with channel numbers. Returns the colour
 * mapping so the UI can render a legend. Requires `rgb.enabled`
 * and `rgb.hardware_output` in the config to actually light LEDs.
 */
export const rgbIdentifyChannels = (durationSecs: number) =>
  invoke<ChannelIdentity[]>('rgb_identify_channels', { durationSecs });

/** One LED slot on a hub, in wire order. Mirrors Rust `MarkerSlotDto`. */
export interface MarkerSlot {
  device_id: string;
  name: string | null;
  channel: number;
  device_type: string;
  led_count: number;
  /** v2_override | v1_override | firmware_table | type_default */
  count_source: string;
  start_led: number;
  /** Exclusive. */
  end_led: number;
  start_byte: number;
  body_color_name: string;
  body_rgb: [number, number, number];
}

/** Device with zero effective LEDs (no wire slot). */
export interface MarkerSkipped {
  device_id: string;
  name: string | null;
  channel: number;
  device_type: string;
  count_source: string;
}

export interface MarkerHub {
  hub_serial: string;
  healthy: boolean;
  firmware: string;
  /** Raw 0x1d LED-count table, keyed by channel. */
  firmware_led_counts: Record<string, number>;
  total_leds: number;
  slots: MarkerSlot[];
  skipped_zero_count: MarkerSkipped[];
}

export interface MarkerLegendEntry {
  label: string;
  rgb: [number, number, number];
  meaning: string;
}

/** Diagnostic variant. Omitted = `markers`. Mirrors Rust `MarkerMode`. */
export type MarkerMode =
  | { kind: 'markers' }
  | { kind: 'markers_black' }
  | { kind: 'ruler' }
  | { kind: 'walk'; device_id: string; step_ms?: number };

/** Walk-mode timing, so the UI can derive the current LED client-side. */
export interface WalkReport {
  device_id: string;
  /** Epoch ms when the walk started. */
  started_at: number;
  step_ms: number;
  led_count: number;
}

/** Reply of `rgbMarkerDiagnostic`. Mirrors Rust `LedMarkerReport`. */
export interface LedMarkerReport {
  app_version: string;
  generated_at_utc: string;
  duration_secs: number;
  mode: 'markers' | 'markers_black' | 'ruler' | 'walk';
  /** Present only for `walk` mode. */
  walk: WalkReport | null;
  total_leds: number;
  /** Port power-protection scaling applied to marker colours (0-1). */
  power_factor: number;
  legend: MarkerLegendEntry[];
  hubs: MarkerHub[];
  file_path: string | null;
  file_error: string | null;
}

/**
 * LED marker diagnostic: for `durationSecs` (clamped 5-300 by the backend)
 * every LED slot on every hub shows white (first LED), green (second LED),
 * red (last LED) and its palette colour elsewhere, through the production
 * RGB path. Also exports the slot manifest as JSON. `mode` selects a variant
 * (black body, decade ruler, single-LED walk); omit for the default markers.
 * Requires `rgb.enabled` and `rgb.hardware_output`; never saves config.
 */
export const rgbMarkerDiagnostic = (durationSecs: number, mode?: MarkerMode) =>
  invoke<LedMarkerReport>('rgb_marker_diagnostic', { durationSecs, mode });
