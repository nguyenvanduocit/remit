// The verify pages' SharedWorker: the core every tab of a verify page shares, as the app's (src/core/worker.ts), over
// the core's fake server (crates/client-web `fake`) instead of WebTransport. The server's half lives here beside the
// core; the seed it answers from lives in the pages (verify/fake.ts): each call goes to the oldest tab still open, and
// its answer comes back. What a page plays (a push, a lost connection, a refusal) comes here and the fake server does it.
import init, { FakeServer, Tabs } from '../src/core/pkg/fake/client_web.js';
import type { RefusalCode } from '../src/core/generated/replica/types.generated';

/** What a verify tab says to the fake server, beside what it says to the core (fake.ts). */
export type ToServer =
  | 'seed'
  | 'bye'
  | 'lose'
  | 'goDown'
  | 'comeUp'
  | { answered: number; result: unknown }
  | { push: string }
  | { refuse: { code: RefusalCode; reason: string } };

/** What the fake server asks of the seed tab. */
export interface ToSeed {
  call: unknown;
  id: number;
}

const waiting: MessagePort[] = [];
const seeds: MessagePort[] = [];
const answers = new Map<number, (result: unknown) => void>();
let calls = 0;
let tabs: Tabs | undefined;

globalThis.addEventListener('connect', (e) => {
  const port = e instanceof MessageEvent ? e.ports[0] : undefined;
  if (!port) return;
  if (tabs) serve(port);
  else waiting.push(port);
});

await init();
const server = new FakeServer((call: unknown) => {
  const seed = seeds[0];
  if (!seed) return { err: { code: 'FAILED', message: 'no verify page is open to answer' } };
  calls += 1;
  const id = calls;
  const asked: ToSeed = { call, id };
  return new Promise((resolve) => {
    answers.set(id, resolve);
    seed.postMessage({ fake: asked });
  });
});
tabs = Tabs.fake(server);
for (const port of waiting.splice(0)) serve(port);

function isToServer(data: unknown): data is { fake: ToServer } {
  return typeof data === 'object' && data !== null && 'fake' in data;
}

// Both listeners are set before the port starts (the core's sets `onmessage`), so neither misses the tab's first words.
function serve(port: MessagePort): void {
  port.addEventListener('message', (e: MessageEvent<unknown>) => {
    if (isToServer(e.data)) play(port, e.data.fake);
  });
  tabs?.connect(port);
}

function play(port: MessagePort, said: ToServer): void {
  if (said === 'seed') seeds.push(port);
  else if (said === 'bye') seeds.splice(0, seeds.length, ...seeds.filter((p) => p !== port));
  else if (said === 'lose') server.lose();
  else if (said === 'goDown') server.goDown();
  else if (said === 'comeUp') server.comeUp();
  else if ('answered' in said) {
    answers.get(said.answered)?.(said.result);
    answers.delete(said.answered);
  } else if ('push' in said) server.push(undefined, said.push);
  else server.refuse(said.refuse.code, said.refuse.reason);
}
