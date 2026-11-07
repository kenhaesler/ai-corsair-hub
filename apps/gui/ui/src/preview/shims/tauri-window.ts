// Browser preview shim for `@tauri-apps/api/window` (aliased in vite preview mode only).
// Window controls are no-ops in a plain browser tab.

// The real window is transparent with a `micaDark` effect (tauri.conf.json),
// and app.css forces html/body to `transparent !important`. A browser would
// show white through that, so paint an approximation of Mica Dark behind the
// page. Higher specificity than app.css's `html, body` rule, so it wins.
const backdrop = document.createElement('style');
backdrop.dataset.preview = 'mica-dark-backdrop';
backdrop.textContent = 'html:root { background: #202020 !important; color-scheme: dark; }';
document.head.appendChild(backdrop);

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
