// Browser preview mode: in-page replacement for Tauri's event channel.

export interface PreviewEvent<T> {
  event: string;
  id: number;
  payload: T;
}

type Handler = (event: PreviewEvent<unknown>) => void;

const handlers = new Map<string, Map<number, Handler>>();
let nextId = 1;
const subscribeHooks: ((event: string) => void)[] = [];

export function subscribe(event: string, handler: Handler): () => void {
  const id = nextId++;
  let bucket = handlers.get(event);
  if (!bucket) {
    bucket = new Map();
    handlers.set(event, bucket);
  }
  bucket.set(id, handler);
  for (const hook of subscribeHooks) hook(event);
  return () => {
    handlers.get(event)?.delete(id);
  };
}

/** Called whenever a listener registers; lets the simulator react (e.g. backfill history). */
export function onSubscribe(hook: (event: string) => void): void {
  subscribeHooks.push(hook);
}

export function hasListeners(event: string): boolean {
  return (handlers.get(event)?.size ?? 0) > 0;
}

export function emit<T>(event: string, payload: T): void {
  const bucket = handlers.get(event);
  if (!bucket) return;
  // Tauri delivers a JSON copy; mimic that so listeners never share state with the mock backend.
  const json = JSON.stringify(payload);
  for (const [id, handler] of [...bucket]) {
    handler({ event, id, payload: JSON.parse(json) as unknown });
  }
}
