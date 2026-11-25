// Serves the pages in this folder against the real components, with no backend and no sign-in: `npm run verify`.
import path from 'node:path';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import vue from '@vitejs/plugin-vue';
import { vueOptions } from '../vueOptions';

const web = path.resolve(__dirname, '..');
export default defineConfig({
  root: web,
  define: { __BUNDLE__: JSON.stringify('') }, // made-up data, no server: no build to name
  resolve: { alias: { '@': path.join(web, 'src') } },
  plugins: [vue(vueOptions), tailwindcss()],
  worker: { format: 'es' }, // the core's SharedWorker (verify/worker.ts) is a module, as the app's
  server: { port: 5299, strictPort: true, host: '127.0.0.1' },
});
