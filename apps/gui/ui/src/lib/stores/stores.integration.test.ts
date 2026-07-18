import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppConfig, RgbConfig } from '../types';

const mocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  getSnapshot: vi.fn(),
  listen: vi.fn(),
  saveConfig: vi.fn(),
  setRgbConfig: vi.fn(),
  validateConfig: vi.fn(),
}));

vi.mock('../api', () => ({
  getConfig: mocks.getConfig,
  getSnapshot: mocks.getSnapshot,
  saveConfig: mocks.saveConfig,
  setRgbConfig: mocks.setRgbConfig,
  validateConfig: mocks.validateConfig,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function rgbConfig(brightness: number): RgbConfig {
  return {
    enabled: true,
    brightness,
    fps: 30,
    hardware_output: false,
    zones: [],
    presets: [],
  };
}

function appConfig(brightness: number): AppConfig {
  return {
    general: { poll_interval_ms: 1000, log_level: 'info', lhm_exe_path: null },
    fan_groups: [],
    rgb: rgbConfig(brightness),
  };
}

beforeEach(() => {
  vi.resetModules();
  for (const mock of Object.values(mocks)) mock.mockReset();
  mocks.saveConfig.mockResolvedValue(undefined);
  mocks.validateConfig.mockResolvedValue(undefined);
});

describe('shared store integration', () => {
  it('ignores a stale forced config response', async () => {
    const first = deferred<AppConfig>();
    const second = deferred<AppConfig>();
    mocks.getConfig.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const { configStore, loadConfig } = await import('./config.svelte');

    const firstLoad = loadConfig(true);
    const secondLoad = loadConfig(true);
    second.resolve(appConfig(60));
    await secondLoad;
    first.resolve(appConfig(20));
    await firstLoad;

    expect(configStore.config?.rgb.brightness).toBe(60);
    expect(configStore.loading).toBe(false);
  });

  it('writes the baseline back when RGB is discarded during an in-flight save', async () => {
    const firstWrite = deferred<void>();
    mocks.getConfig.mockResolvedValue(appConfig(25));
    mocks.setRgbConfig
      .mockReturnValueOnce(firstWrite.promise)
      .mockResolvedValueOnce(undefined);

    const config = await import('./config.svelte');
    const lighting = await import('./rgb.svelte');
    await config.loadConfig();
    lighting.initializeRgbConfig(config.configStore.config!.rgb);
    lighting.scheduleRgbSave(rgbConfig(90), true);

    const discarded = lighting.discardAllConfigChanges();
    firstWrite.resolve();
    expect(await discarded).toBe(true);

    expect(mocks.setRgbConfig).toHaveBeenCalledTimes(2);
    expect(mocks.setRgbConfig.mock.calls[1][0].brightness).toBe(25);
    expect(lighting.rgb.config?.brightness).toBe(25);
    expect(lighting.rgb.dirty).toBe(false);
    expect(config.configStore.dirty).toBe(false);
  });

  it('unlistens a late RGB registration and permits a fresh setup', async () => {
    const firstRegistration = deferred<() => void>();
    const firstUnlisten = vi.fn();
    const secondUnlisten = vi.fn();
    mocks.listen
      .mockReturnValueOnce(firstRegistration.promise)
      .mockResolvedValueOnce(secondUnlisten);
    const lighting = await import('./rgb.svelte');

    const staleSetup = lighting.initRgbListener();
    lighting.disposeRgbListener();
    const currentSetup = lighting.initRgbListener();
    firstRegistration.resolve(firstUnlisten);
    await Promise.all([staleSetup, currentSetup]);

    expect(firstUnlisten).toHaveBeenCalledOnce();
    expect(secondUnlisten).not.toHaveBeenCalled();
    lighting.disposeRgbListener();
    expect(secondUnlisten).toHaveBeenCalledOnce();
  });

  it('unlistens a sensor registration that resolves after disposal', async () => {
    const registration = deferred<() => void>();
    const unlisten = vi.fn();
    mocks.listen.mockReturnValueOnce(registration.promise);
    const sensors = await import('./sensors.svelte');

    const setup = sensors.initSensorListener();
    sensors.disposeSensorListener();
    registration.resolve(unlisten);
    await setup;

    expect(unlisten).toHaveBeenCalledOnce();
    expect(mocks.getSnapshot).not.toHaveBeenCalled();
  });
});
