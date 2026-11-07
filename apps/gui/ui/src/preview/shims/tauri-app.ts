// Browser preview shim for `@tauri-apps/api/app` (aliased in vite preview mode only).
declare const __PREVIEW_APP_VERSION__: string;

export async function getVersion(): Promise<string> {
  return __PREVIEW_APP_VERSION__;
}

export async function getName(): Promise<string> {
  return 'Corsair Hub';
}
