// Browser preview shim for `@tauri-apps/api/core` (aliased in vite preview mode only).
import { dispatch } from '../backend';

export type InvokeArgs = Record<string, unknown>;

export function invoke<T>(cmd: string, args: InvokeArgs = {}): Promise<T> {
  return dispatch<T>(cmd, args);
}

export function isTauri(): boolean {
  return false;
}
