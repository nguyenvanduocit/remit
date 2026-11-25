// The page's one client core (crates/client-web, ADR-0072 decision 10) and the models it draws (ADR-0095). main.ts
// starts it on the browser's WebTransport; a verify page or a test, on the core's fake server (verify/fakeServer.ts). The
// replica modules (catalog.ts, openChain.ts, assistant.ts, openEvent.ts, archived.ts, drafts.ts) and components draw its
// models through useModel; what the core says the owner should notice is worded here, as a line of feedback. Nothing
// here decides: the core does.
import { createCore, createPage, type Call, type Core, type Keeping, type Open } from './core/client';
import { createApi, type Api, type CallArgs, type CallAnswers } from './core/generated/api.generated';
import type { Ask, Key, Notice, What } from './core/generated/replica/types.generated';
import { toast } from './act';
import { noteDuelMove } from './duels';

const page = createPage();
let core: Core | null = null;

/** The core's state as it last said it; none before it is started. */
export const coreState = page.state;

/** Draws one of the core's models (core/client.ts `Page.useModel`). */
export const useModel = page.useModel;

// What the owner asked for that did not happen, in his words.
const COULD_NOT: Record<What, string> = {
  openChain: 'Could not open the chain',
  loadStream: 'Could not load the agent’s activity',
  loadEarlier: 'Could not load the agent’s earlier activity',
  loadAssistant: 'Could not load the Assistant’s conversation',
  loadAtWork: 'Could not load who is at work',
  openEvent: 'Could not open the event',
  readArchived: 'Could not load the archived chains',
  readRepos: 'Could not load the repositories',
  saveDraft: 'Your words were not saved',
  deleteDraft: 'Could not delete the draft',
};

const INBOX_OPENED: Record<string, (title: string) => string> = {
  question: (t) => `The Chair asks you: ${t}`,
  error: (t) => t,
  credential: (t) => `A credential is needed: ${t}`,
  change: (t) => `A change waits for you: ${t}`,
  exception: (t) => `An exception waits for you: ${t}`,
};

function say(n: Notice): void {
  switch (n.notice) {
    case 'chainDone':
      return toast(`Chain done: ${n.title}`);
    case 'turnFinished':
      return toast(`${n.name} finished its turn · report sent to the Chair`);
    case 'inboxOpened':
      return toast((INBOX_OPENED[n.kind] ?? ((t: string) => `Agent requests permission: ${t}`))(n.title));
    case 'duelsMoved':
      return noteDuelMove();
    case 'couldNot':
      return toast(`${COULD_NOT[n.what]}: ${n.failure.message}`);
  }
}

/** Starts the page's core over `open` and dials: what an earlier one held is let go, and the sends `keeping` holds from
 * an earlier page go once the link is up. */
export function startCore(open: Open, keeping?: Keeping): Core {
  core?.close();
  core = createCore(open, say, page, keeping);
  core.ask({ ask: 'connect' });
  return core;
}

function running(): Core {
  if (!core) throw new Error('the client core is not started: main.ts (or the verify page, or the test) starts it');
  return core;
}

/** One of the page's asks of the core: open a chain, load a stream, write a draft. */
export const ask = (a: Ask): void => running().ask(a);

/** The server's calls, through the core: it adds a changing call's request id. The core answers with the server's JSON
 * for the call, which the contract types. */
export const api: Api = createApi(<M extends keyof CallArgs>(method: M, args: CallArgs[M]) => running().call({ method, args } as Call) as Promise<CallAnswers[M]>);

/** A call that is one of a place's actions, one at a time: asking the same `key` again after an answer that never came
 * retries that action, with its request id (the core's `Retry`). */
export const callIn = (place: string, key: string, call: Call): Promise<unknown> => running().call(call, { place, key });

/** Sends drafts once the server has what was typed in them, by `call` naming them, now or once the link is back; the
 * drafts sent leave the page. Its row in the `outbox` model shows `text` at `at` meanwhile (ADR-0094). */
export const sendDrafts = (ids: Key[], call: Call, at: string, text: string): Promise<unknown> => running().send(ids, call, at, text);

/** A name for a place whose actions are retried as one (callIn): unique on the page. */
let places = 0;
export const newPlace = (what: string): string => `${what}#${++places}`;
