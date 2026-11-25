// How the single-file components are compiled, for the app (vite.config.ts) and the verify pages (verify/vite.config.ts):
// <diffs-container> is the element @pierre/diffs defines and draws into, not a Vue component.
import type { Options } from '@vitejs/plugin-vue';

export const vueOptions: Options = { template: { compilerOptions: { isCustomElement: (tag) => tag === 'diffs-container' } } };
