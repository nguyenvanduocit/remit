// A verify page's fake server: the browser's build of the core with `fake` (crates/client-web/bind.sh, pkg/fake), run as
// the app runs it, in a SharedWorker every tab of the page shares (verify/worker.ts, ADR-0095 decision 4): two tabs of
// one verify page are two tabs of one fake session. The worker's fake server asks this page's seed, the oldest tab's,
// for each answer; what the page plays (a push, a lost connection) goes to it.
import init, * as pkg from '../src/core/pkg/fake/client_web.js';
import { keepIn, overPort } from '../src/core/client';
import { START, startFake, type Answers, type Fake, type FakeCall, type FakePkg, type Host } from './fakeServer';
import type { ToSeed, ToServer } from './worker';

export { snapshotOf } from './fakeServer';

// The page's own core checks a seed it cannot read (fakeServer.ts `unreadable`).
await init();

function isToSeed(data: unknown): data is { fake: ToSeed } {
  return typeof data === 'object' && data !== null && 'fake' in data;
}

/** The worker of this verify page (by its path) hosts the core; this tab answers its server's calls while it is the
 * oldest open. */
const inWorker: Host = (answer) => {
  const worker = new SharedWorker(new URL('./worker.ts', import.meta.url), { type: 'module', name: `remit-verify:${location.pathname}` });
  const port = worker.port;
  const say = (fake: ToServer): void => port.postMessage({ fake });
  port.addEventListener('message', (e: MessageEvent<unknown>) => {
    if (!isToSeed(e.data)) return;
    const { call, id } = e.data.fake;
    // The worker hands the call as the server read it: `{ method, args }`.
    void Promise.resolve(answer(call as FakeCall)).then((result) => say({ answered: id, result }));
  });
  say('seed');
  addEventListener('pagehide', () => say('bye'));
  return {
    server: {
      push: (_home, push) => say({ push }),
      lose: () => say('lose'),
      refuse: (code, reason) => say({ refuse: { code, reason } }),
      goDown: () => say('goDown'),
      comeUp: () => say('comeUp'),
    },
    open: overPort(port, START, 'fake'),
  };
};

/** Starts the page's core on a fake server answering from `answers`. */
// A verify page keeps its waiting sends as the app does (main.ts), per page, so a reload sends them.
export const fakeServer = (answers: Answers): Fake =>
  startFake(pkg as unknown as FakePkg, answers, inWorker, keepIn(sessionStorage, `remit.verify.outbox:${location.pathname}`));
