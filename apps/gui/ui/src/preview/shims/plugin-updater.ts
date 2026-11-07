// Browser preview shim for `@tauri-apps/plugin-updater` (aliased in vite preview mode only).
// Always reports "up to date".
export async function check(): Promise<null> {
  return null;
}
