// Browser preview shim for `@tauri-apps/api/window` (aliased in vite preview mode only).
// Window controls are no-ops in a plain browser tab.
//
// No backdrop is needed: the UI paints its own opaque canvas (`--bg-0` on
// html/body in styles/global.css), so the browser looks like the real window.

const noop = async (): Promise<void> => {};

const previewWindow = {
  label: 'main',
  minimize: noop,
  maximize: noop,
  unmaximize: noop,
  toggleMaximize: noop,
  close: noop,
  hide: noop,
  show: noop,
  setFocus: noop,
  startDragging: noop,
  isMaximized: async () => false,
};

export function getCurrentWindow() {
  return previewWindow;
}
