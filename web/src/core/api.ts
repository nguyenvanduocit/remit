// The server's calls (crates/wire app contract, generated into generated/api.generated.ts), as a component asks them.
// A channel provides them under apiKey: main.ts and the verify pages the page's core's (replica.ts api); a component
// test, stubs of the calls it makes.
import { inject, type InjectionKey } from 'vue';
import type { Api } from './generated/api.generated';

export const apiKey: InjectionKey<Api> = Symbol('api');

export function useApi(): Api {
  const api = inject(apiKey, null);
  if (!api) throw new Error('no server calls provided: provide apiKey (main.ts, the verify page, or the test)');
  return api;
}
