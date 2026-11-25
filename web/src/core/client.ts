// The page's side of the client core: it hands the core the page's asks, keeps the rows of the models the page draws as
// the core tells them (ADR-0095 decision 13), keeps the core's state, settles calls by their ticket and passes notices on
// for the shell to word. It decides nothing; the core (`crates/client-web`) does. Which binding runs it (the SharedWorker every tab
// shares, `overPort`, or a core in the page itself in tests) is the caller's: `open` makes it with the sink the core
// tells.
import { computed, getCurrentScope, onScopeDispose, shallowReactive, shallowRef, type ComputedRef, type ShallowRef } from 'vue';
import type { CallArgs } from './generated/api.generated';
import type { FromTab } from './generated/client_web/types.generated';
import type { Ask, Change, Failure, Freshness, Kept, Key, Model, Notice, Retry, Row, State, Told } from './generated/replica/types.generated';

/** One of the contract's calls as on the wire; a changing call's `request` is the core's to add. */
export type Call = { [M in keyof CallArgs]: { method: M; args: CallArgs[M] } }[keyof CallArgs];

/** The core as one tab uses it: the SharedWorker's port (`overPort`), or a wasm-bindgen `Tab` of a core in the page. */
export interface Binding {
  ask(ask: Ask): void;
  free(): void;
}

/** A call's failure, with the code a channel acts on. */
export class CallError extends Error {
  readonly code: Failure['code'];
  constructor(failure: Failure) {
    super(failure.message);
    this.code = failure.code;
  }
}

/** A model the page draws: its rows by record id, in the model's order, and how fresh they are. A row is replaced
 * whole when it changes, never edited in place, so what reads one row re-renders when that row changes and no other. */
export interface Shown<T> {
  readonly rows: ReadonlyMap<Key, T>;
  readonly status: Readonly<ShallowRef<Freshness>>;
}

/** A model's rows, in its order, as one list until a row changes. */
export const inOrder = <T>(m: Shown<T>): ComputedRef<T[]> => computed(() => [...m.rows.values()]);

/** The record of a model of one. */
export const oneOf = <T>(m: Shown<T>): T | null => m.rows.get('') ?? null;

/** What the page draws, whichever core runs: the core's state and the models in use. A page has one (replica.ts). */
export interface Page {
  /** None until the core first says where it stands. */
  readonly state: ShallowRef<State | undefined>;
  /** Draws `model`: the core is asked to show it while something uses it, and to hide it once the last effect scope
   * that used it is disposed. Used outside any scope (a module's own), it is shown for the page's life. */
  useModel<T>(model: Model): Shown<T>;
  /** A core runs: every model in use is shown to it. None: no core runs. */
  attach(ask: ((a: Ask) => void) | null): void;
  /** What the core told about the models and its state. */
  tell(told: Told): void;
}

/** A record as the server sent it: a JSON object. */
type Fields = { [field: string]: unknown };

function isFields(value: unknown): value is Fields {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** A row's record: the server sends only objects, so anything else is a defect of the core and throws. */
function fieldsOf(value: unknown): Fields {
  if (!isFields(value)) throw new Error(`the core told a row that is not an object: ${JSON.stringify(value)}`);
  return value;
}

/** A model's name among those in use: its kind, and the card for a card's events. */
const nameOf = (m: Model): string => ('card' in m ? `${m.model}:${m.card}` : m.model);

interface Held {
  model: Model;
  rows: Map<Key, Fields>;
  status: ShallowRef<Freshness>;
  users: number;
}

/** Puts `id` before `before`, keeping the order of the rest. */
function insertBefore(rows: Map<Key, Fields>, id: Key, record: Fields, before: Key): void {
  const entries = [...rows.entries()];
  const at = entries.findIndex(([key]) => key === before);
  entries.splice(at < 0 ? entries.length : at, 0, [id, record]);
  rows.clear();
  for (const [key, value] of entries) rows.set(key, value);
}

function change(rows: Map<Key, Fields>, c: Change): void {
  switch (c.change) {
    case 'put': {
      const record = fieldsOf(c.record);
      if (rows.has(c.id) || c.before === undefined) rows.set(c.id, record);
      else insertBefore(rows, c.id, record, c.before);
      return;
    }
    case 'append': {
      // A streamed chunk grows the text of the event the row holds (an `EventRead`'s `event`).
      const held = rows.get(c.id);
      const event = held?.event;
      if (held && isFields(event)) rows.set(c.id, { ...held, event: { ...event, text: `${typeof event.text === 'string' ? event.text : ''}${c.text}` } });
      return;
    }
    case 'drop':
      rows.delete(c.id);
      return;
  }
}

function fill(rows: Map<Key, Fields>, whole: Row[]): void {
  rows.clear();
  for (const r of whole) rows.set(r.id, fieldsOf(r.record));
}

export function createPage(): Page {
  const state = shallowRef<State | undefined>();
  const models = new Map<string, Held>();
  let ask: ((a: Ask) => void) | null = null;

  function release(name: string, held: Held): void {
    held.users -= 1;
    if (held.users > 0) return;
    models.delete(name);
    ask?.({ ask: 'hide', model: held.model });
  }

  return {
    state,
    useModel<T>(model: Model): Shown<T> {
      const name = nameOf(model);
      let held = models.get(name);
      if (!held) {
        held = { model, rows: shallowReactive(new Map<Key, Fields>()), status: shallowRef<Freshness>({ status: 'repairing' }), users: 0 };
        models.set(name, held);
        ask?.({ ask: 'show', model });
      }
      held.users += 1;
      const mine = held;
      if (getCurrentScope()) onScopeDispose(() => release(name, mine));
      // The rows are the server's JSON for the model's record type, as the contract says it: the core read them.
      return { rows: held.rows as unknown as ReadonlyMap<Key, T>, status: held.status };
    },
    attach(next) {
      ask = next;
      if (!next) return;
      state.value = undefined;
      for (const held of models.values()) next({ ask: 'show', model: held.model });
    },
    tell(t) {
      switch (t.told) {
        case 'model': {
          const held = models.get(nameOf(t.model));
          if (!held) return;
          fill(held.rows, t.rows);
          held.status.value = t.status;
          return;
        }
        case 'rows':
          for (const c of t.changes) {
            const held = models.get(nameOf(c.model));
            if (held) change(held.rows, c);
          }
          return;
        case 'status': {
          const held = models.get(nameOf(t.model));
          if (held) held.status.value = t.status;
          return;
        }
        case 'state':
          state.value = t.state;
          return;
      }
    },
  };
}

export interface Core {
  ask(ask: Ask): void;
  /** One of the contract's calls; a changing call's request id is the core's. Asking the same `retry` again after an
   * answer that never came retries that action. */
  call(call: Call, retry?: Retry): Promise<unknown>;
  /** Sends drafts once the server has what was typed in them, with `call` naming them: at once, or when the link is back
   * (the `outbox` model shows it meanwhile). Settles once it went or failed. */
  send(ids: Key[], call: Call, at: string, text: string): Promise<unknown>;
  close(): void;
}

interface Waiting {
  resolve: (value: unknown) => void;
  reject: (error: CallError) => void;
}

/** Opens the binding with the sink the core tells. */
export type Open = (sink: (told: Told[]) => void) => Binding;

/** 128 random bits as hex (crypto.getRandomValues works where randomUUID needs a secure context). */
export const randomHex = (): string => Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, '0')).join('');

/** This tab's binding to the core the SharedWorker on `port` runs (ADR-0095 decision 4): `start` (the page's start, as
 * JSON) starts it if no tab did, and `credential` is what its next dial carries. The tab holds a Web Lock while it is
 * open, so the worker lets it go however it goes, and says whether the owner sees it. Anything the port carries that
 * is not a batch for the page (the verify pages' seed traffic) is left to whoever else listens. */
export function overPort(port: MessagePort, start: string, credential: string): Open {
  return (sink) => {
    const lock = `remit-tab:${randomHex()}`;
    // The hello goes once the tab holds its lock, so the worker that waits for the lock waits for the tab to go; what
    // the tab says before then waits behind it.
    let held = false;
    let freed = false;
    let release = (): void => {};
    const early: FromTab[] = [];
    const say = (m: FromTab): void => (held ? port.postMessage(m) : void early.push(m));
    void navigator.locks.request(lock, () => {
      if (freed) return undefined;
      port.postMessage({ say: 'hello', start, credential, lock } satisfies FromTab);
      for (const m of early.splice(0)) port.postMessage(m);
      held = true;
      return new Promise<void>((resolve) => (release = resolve));
    });
    port.onmessage = (e: MessageEvent<unknown>) => {
      if (Array.isArray(e.data)) sink(e.data);
    };
    const seen = (): void => say({ say: 'ask', ask: { ask: 'hidden', hidden: document.hidden } });
    document.addEventListener('visibilitychange', seen);
    seen();
    // What fails in the page and nothing handles goes up in the core's link report (ADR-0119). A call's failure the core
    // counted already, as it settled it.
    const uncaught = (e: ErrorEvent): void => {
      const at = e.filename ? `${e.filename}:${e.lineno}:${e.colno}` : '';
      say({ say: 'ask', ask: { ask: 'fault', kind: 'uncaught', message: e.message || String(e.error), at } });
    };
    const rejected = (e: PromiseRejectionEvent): void => {
      if (e.reason instanceof CallError) return;
      const message = e.reason instanceof Error ? `${e.reason.name}: ${e.reason.message}` : String(e.reason);
      say({ say: 'ask', ask: { ask: 'fault', kind: 'rejection', message, at: '' } });
    };
    window.addEventListener('error', uncaught);
    window.addEventListener('unhandledrejection', rejected);
    return {
      ask: (a) => say({ say: 'ask', ask: a }),
      free: () => {
        document.removeEventListener('visibilitychange', seen);
        window.removeEventListener('error', uncaught);
        window.removeEventListener('unhandledrejection', rejected);
        say({ say: 'bye' });
        freed = true;
        release();
      },
    };
  };
}

/** Where the page keeps the owner's sends that have not gone out, so a page loaded again (a reload, a new build) sends
 * them (ADR-0094). The core says what to keep; this only stores it. */
export interface Keeping {
  load(): Kept | undefined;
  save(kept: Kept): void;
}

/** Keeps the sends in the browser's `storage` under `key`, and nothing once none waits. */
export function keepIn(storage: Storage, key: string): Keeping {
  return {
    load() {
      const raw = storage.getItem(key);
      if (!raw) return undefined;
      const kept: Kept = JSON.parse(raw);
      return kept;
    },
    save: (kept) => (kept.sends.length ? storage.setItem(key, JSON.stringify(kept)) : storage.removeItem(key)),
  };
}

/** Starts a core over `open` for `page`: the models the page uses are shown to it. */
export function createCore(open: Open, onNotice: (n: Notice) => void, page: Page, keeping?: Keeping): Core {
  const waiting = new Map<number, Waiting>();
  let tickets = 0;

  /** The call of `ticket`, no longer waiting. */
  function settled(ticket: number): Waiting | undefined {
    const w = waiting.get(ticket);
    waiting.delete(ticket);
    return w;
  }

  function tell(told: Told[]): void {
    for (const t of told) {
      switch (t.told) {
        case 'answer':
          settled(t.ticket)?.resolve(t.value);
          break;
        case 'failed':
          settled(t.ticket)?.reject(new CallError(t.failure));
          break;
        case 'notice':
          onNotice(t.notice);
          break;
        case 'keep':
          keeping?.save(t.kept);
          break;
        default:
          page.tell(t);
      }
    }
  }

  const binding = open(tell);
  const ask = (a: Ask): void => binding.ask(a);
  page.attach(ask);
  const kept = keeping?.load();
  if (kept) ask({ ask: 'restore', kept });

  function settle(make: (ticket: number) => Ask): Promise<unknown> {
    tickets += 1;
    const ticket = tickets;
    return new Promise((resolve, reject) => {
      waiting.set(ticket, { resolve, reject });
      ask(make(ticket));
    });
  }

  return {
    ask,
    call: (call, retry) => settle((ticket) => ({ ask: 'call', ticket, retry: retry ?? null, call })),
    send: (ids, call, at, text) => settle((ticket) => ({ ask: 'send', ticket, ids, call, at, text })),
    close: () => {
      page.attach(null);
      binding.free();
    },
  };
}
