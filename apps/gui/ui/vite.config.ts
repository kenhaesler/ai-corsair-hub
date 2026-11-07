import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const shim = (name: string) => fileURLToPath(new URL(`./src/preview/shims/${name}.ts`, import.meta.url));

// Browsers request /favicon.ico (the Tauri webview doesn't); answer it so the
// preview console stays free of 404 noise.
const previewFavicon: Plugin = {
  name: 'preview-favicon',
  configureServer(server) {
    server.middlewares.use((req, res, next) => {
      if (req.url === '/favicon.ico') {
        res.statusCode = 204;
        res.end();
        return;
      }
      next();
    });
  },
};

export default defineConfig(({ mode }) => {
  // `npm run dev:preview` (vite --mode preview): run the real UI in a plain
  // browser against an in-memory mock backend. The Tauri modules are swapped
  // for shims here and ONLY here, so no preview code reaches `vite build` /
  // `tauri dev` / `tauri build`.
  const browserPreview = mode === 'preview';
  const appVersion = browserPreview
    ? (JSON.parse(readFileSync(new URL('../tauri.conf.json', import.meta.url), 'utf8')) as { version: string }).version
    : '';

  return {
    plugins: browserPreview ? [svelte(), previewFavicon] : [svelte()],
    clearScreen: false,
    server: browserPreview
      ? { port: 5174, strictPort: false }
      : {
          port: 5173,
          strictPort: true,
        },
    resolve: browserPreview
      ? {
          alias: [
            { find: /^@tauri-apps\/api\/core$/, replacement: shim('tauri-core') },
            { find: /^@tauri-apps\/api\/event$/, replacement: shim('tauri-event') },
            { find: /^@tauri-apps\/api\/window$/, replacement: shim('tauri-window') },
            { find: /^@tauri-apps\/api\/app$/, replacement: shim('tauri-app') },
            { find: /^@tauri-apps\/plugin-updater$/, replacement: shim('plugin-updater') },
          ],
        }
      : undefined,
    define: browserPreview ? { __PREVIEW_APP_VERSION__: JSON.stringify(appVersion) } : undefined,
    build: {
      target: 'chrome105',
      sourcemap: !!process.env.TAURI_ENV_DEBUG,
      minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    },
  };
});
