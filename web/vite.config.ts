import path from 'node:path';
import { defineConfig, type Plugin } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import vue from '@vitejs/plugin-vue';
import vueDevTools from 'vite-plugin-vue-devtools';
import annotator from 'vite-plugin-ai-annotator';
import { vueOptions } from './vueOptions';

// Names this build. The page sends the name as it connects and the server compares it with the build it embeds
// (dist/bundle.txt, crates/remit/src/web.rs): a page left open across a deploy is told to load again. The dev server runs the
// source, so its page names no build.
const bundle = Date.now().toString(36);
const bundleName: Plugin = {
  name: 'bundle-name',
  apply: 'build',
  generateBundle() {
    this.emitFile({ type: 'asset', fileName: 'bundle.txt', source: bundle });
  },
};

// The dev backend scripts/dev.sh runs, on 4321 beside a real instance's 4320.
const backend = process.env.REMIT_BACKEND ?? 'http://127.0.0.1:4321';

export default defineConfig(({ command }) => ({
  define: { __BUNDLE__: JSON.stringify(command === 'build' ? bundle : '') },
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
  // Dev server only (both dev plugins apply on `serve`). annotator's autoSetupSkills off: it would write its instructions into AGENTS.md and CLAUDE.md.
  plugins: [vue(vueOptions), vueDevTools(), tailwindcss(), bundleName, annotator({ port: 7318, autoSetupSkills: false, autoSetupChannelPlugin: false })],
  build: { outDir: 'dist', emptyOutDir: true },
  // The client core runs in a SharedWorker (src/core/worker.ts), a module that awaits its WASM at the top.
  worker: { format: 'es' },
  // The core asks the server where its link is (/link.json) at each dial and dials it over WebTransport itself, not through Vite.
  // REMIT_BACKEND names the server proxied to: the dev backend by default, another instance (a milestone run) when set.
  server: { proxy: Object.fromEntries(['/link.json', '/auth', '/hooks'].map((p) => [p, backend])) },
}));
