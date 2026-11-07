// Browser preview shim for `@tauri-apps/api/event` (aliased in vite preview mode only).
import { emit as busEmit, subscribe, type PreviewEvent } from '../bus';
import { ensureStarted } from '../backend';

export type UnlistenFn = () => void;
export type EventName = string;
export type Event<T> = PreviewEvent<T>;
export type EventCallback<T> = (event: Event<T>) => void;

export async function listen<T>(event: EventName, handler: EventCallback<T>): Promise<UnlistenFn> {
  ensureStarted();
  return subscribe(event, handler as EventCallback<unknown>);
}

export async function once<T>(event: EventName, handler: EventCallback<T>): Promise<UnlistenFn> {
  ensureStarted();
  const off = subscribe(event, (e) => {
    off();
    (handler as EventCallback<unknown>)(e);
  });
  return off;
}

export async function emit<T>(event: string, payload?: T): Promise<void> {
  busEmit(event, payload ?? null);
}
