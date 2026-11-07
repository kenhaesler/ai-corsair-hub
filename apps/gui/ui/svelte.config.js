import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  // script: true runs TypeScript through esbuild first; Svelte's built-in type
  // stripping leaves optional-parameter markers (`x?: T`) in the output.
  preprocess: vitePreprocess({ script: true }),
};
