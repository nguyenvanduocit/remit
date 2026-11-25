// A test's fake server: the browser's build of the core with `fake` (crates/client-web/bind.sh, pkg/fake), its .wasm
// read from disk. Read by path: Vite rewrites `new URL(…, import.meta.url)` into a URL of the page's location under
// happy-dom, which a file read cannot take.
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as pkg from '../src/core/pkg/fake/client_web.js';
import { inPage, startFake, type Answers, type Fake, type FakePkg } from './fakeServer';

export { snapshotOf } from './fakeServer';

pkg.initSync({ module: readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../src/core/pkg/fake/client_web_bg.wasm')) });

/** Starts the page's core on a fake server answering from `answers`, in the page itself: a test has one tab. */
export const fakeServer = (answers: Answers): Fake => {
  const fake = pkg as unknown as FakePkg;
  return startFake(fake, answers, inPage(fake));
};
