// The Signals area's Events page (EventsView: the events listed, the one open beside them), the event signals' pages on its
// Signals tab (SignalsView: settings, sensors, key, readings, subscriptions) and their subscriptions on its Subscriptions tab
// (SubscriptionsView), in the whole app, on made-up data shaped like a busy owner's: events no subscription held for, ones that
// told a Chair, started a chain, ran a job, a comment of Remit's own that told nobody, one with a very long signal name, a
// long outcome and a long body cut, events that came by every way (a door, the CLI, a sensor of GitHub issues, a sensor of Datadog
// monitors that has gone silent, an agent), and 60+ rows. No server, no sign-in: a fake client answers the calls, through the
// same sync as the real app. Open /verify/events.html; ?at=/events/<address> opens an event (its address is
// `event:<signal>#<n>`; event:gitlab#5 told a Chair, #7 fired three subscriptions, #12 has a long outcome, #1 and #3 no
// subscription held for, #21 an event a sensor read from GitHub, #23 the CLI sent, #24 an agent wrote); ?at=/signals/signal:event:<name> a
// signal's page, ?at=/subscriptions/<id> a subscription; ?rows=<n> sizes the list (default 64); ?empty=1 has no event yet;
// ?signals=<n> lists the first n event signals (default 5: the paused one, the GitHub sensor's and the silent Datadog sensors' among them, one of those paused;
// 9 adds the Grafana script sensor's, the deploy script's and the builtin); ?change=1 has the Assistant's sensor change wait for the owner's Apply in its inbox;
// ?arrive=<s> has the server play an event's life: a new one arrives with no subscription, and a subscription fires on it a few
// seconds later. What the page was asked is in window.calls; window.push(name, arg) is the server pushing a change
// (eventSignalUpdated, eventSignalDeleted, sensorUpdated, sensorDeleted: the owner told the Assistant).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { createMemoryHistory } from 'vue-router';
import Shell from '../src/components/Shell.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { signedIn } from '../src/session';
import { appRouter } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { fakeDrafts } from './fakeDrafts';
import { ASSISTANT, ASSISTANT_CARD, assistantCard, assistantEvents, assistantStanding, assistantView } from './assistantSeed';
import { eventsSeed, firingOf, listed, hookChains, hookStandings, eventSignalKeyOf, eventSignalsSeed, sensorsSeed, SILENT_SENSORS } from './eventsSeed';
import type { Actor, EventRecord, EventSignal, InboxItem, Sensor, SignalBoard, SignalLine, Snapshot, SubscriptionLine, ToolSource } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { agentStands } from './standingSeed';
import { signalSourcesSeed } from './signalSourcesSeed';
import { actorOf, chainRead, inboundEvent, inboxRecord, optionOf, seedEvent, signalLine, subscriptionLine } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
const OWNER: Actor = actorOf('owner', 'owner@example.com', 'web');
const ASSISTANT_ACTOR: Actor = actorOf('assistant', ASSISTANT_CARD);
const calls: unknown[] = [];
Object.assign(window, { calls });

const store = new Map<string, EventRecord>(params.get('empty') ? [] : eventsSeed(Number(params.get('rows') ?? 64)).map((e) => [e.id, e]));
const newestFirst = () => [...store.values()].sort((a, b) => b.at.localeCompare(a.at));
let nextId = 10_000;
// The event signals: a rail changes one and pushes it, as the server does.
const eventSignals = new Map<string, EventSignal>(eventSignalsSeed(Number(params.get('signals') ?? 5)).map((s) => [s.id, s]));
// The sensors of the listed event signals, and the scripts of chains; an owner's word to the Assistant changes one and pushes it.
const sensors = new Map<string, Sensor>(sensorsSeed().map((s) => [s.id, s]));
const sensorsListed = () => [...sensors.values()].filter((s) => s.chain || [...eventSignals.values()].some((es) => s.signal === `event:${es.name}`));
const signalsListed = () => [...eventSignals.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
const rail = (name: string, paused: boolean) => async (id: string) => {
  const s = eventSignals.get(id);
  if (!s) throw new Error(`event signal ${id} not found`);
  calls.push({ [name]: id });
  const next: EventSignal = { ...s, paused, by: OWNER, updatedAt: iso(Date.now()), rev: s.rev + 1 };
  eventSignals.set(id, next);
  void send('eventSignalUpdated', next);
  void send('signalBoardUpdated', signalBoard());
  return { ok: true } as never;
};
// The Assistant, ready, and its conversation; the owner's messages to it are kept in the stream.
const assistantStream = assistantEvents(Date.now(), 3).filter((e) => e.event.status !== 'queued');
// ?change=1: the Assistant changed what a sensor reads, so it waits for the owner's Apply in the Assistant's inbox
// (orch/change_item.go), in the server's words: kind `sensor`, its one field `reads`.
const sensorChange = inboxRecord({
  id: 'i_sensor', changeKind: 'sensor', home: ASSISTANT, cardId: ASSISTANT_CARD, kind: 'change', title: 'Assistant asks to write sensor shop-issues', status: 'open', createdAt: iso(Date.now() - 2 * MIN), rev: 1,
  detail: 'Assistant asks to write sensor `shop-issues`. These fields move a rail, so it is not written until you answer:\n\n- **reads**: `{"signal":"event:shop-issues","source":"github","connection":"acme","query":"acme/shop label:customer state:open"}` → `{"signal":"event:shop-issues","source":"github","connection":"acme","query":"acme/shop state:all"}`\n\nApply writes it as shown. Not now writes nothing, and the agent is told.',
  options: [optionOf('apply', 'Apply'), optionOf('not_now', 'Not now')],
});
const inbox: InboxItem[] = params.get('change') ? [sensorChange] : [];

const drafts = fakeDrafts('events', (name, arg) => send(name as never, arg as never));
const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

// What the server says reaches the browser as a copy, as it does over the link (the browser freezes what it is given).
const send: typeof fake.push = (name, arg) => fake.push(name, structuredClone(arg));
// Each signal's subscriptions, one line each as the server lays them out (signalboard.go): one for each way an event of the
// signal was acted on in the seeded firings, so the page's lines and the events' firings name the same subscriptions.
const linesOf = (es: EventSignal): SubscriptionLine[] => {
  const seen = new Map<string, EventRecord['firings'][number]>();
  for (const e of store.values()) if (e.signal === es.name) for (const f of e.firings) if (f.subscriptionId) seen.set(f.subscriptionId, f);
  return [...seen].map(([id, f]) => (subscriptionLine({
    id, kind: 'event', title: f.title, eventSignalId: es.id, source: 'event', key: `event:${es.name}`,
    what: f.action === 'tell' ? 'Tells the Chair of the chain the event is about, as evidence.' : f.action === 'start' ? 'Starts a chain for each event it holds for.' : 'Runs a job for each event it holds for.',
    state: es.paused ? 'paused' : 'waiting', group: es.paused ? 'idle' : 'coming', control: '', lastFired: f.at,
  })));
};
// The readings each signal has: one for each of its events the store holds, newest first, each naming its event.
const readingsOf = (name: string) =>
  newestFirst().filter((e) => e.signal === name).map((e) => ({ rev: e.rev, at: e.at, cause: e.via === 'poll' ? 'a sensor' : e.via === 'agent' ? 'an agent (emit_event)' : `${name} (${e.via === 'cli' ? 'remit emit' : 'door'})`, value: JSON.stringify({ event: e.event }), truncated: false, eventId: e.id }));
// And each is the signal `event:<name>`, listed from the moment it is made, its page the signal's settings (signalboard.go
// signalLines); one whose sensors failed their last reads is silent.
const signalOf = (es: EventSignal): SignalLine => {
  const read = readingsOf(es.name);
  return signalLine({ id: `signal:event:${es.name}`, key: `event:${es.name}`, source: 'event', title: `Events of ${es.name}`, rev: read[0]?.rev ?? 0, subscribers: linesOf(es).length, eventSignalId: es.id,
    ...(read[0] ? { at: read[0].at, cause: read[0].cause } : {}), ...(SILENT_SENSORS[es.name] ? { silent: SILENT_SENSORS[es.name], silentSensors: sensorsSeed().filter((x) => x.signal === `event:${es.name}` && !x.paused).map((x) => ({ sensorId: x.id, why: SILENT_SENSORS[es.name] })) } : {}) });
};
let boardRev = 0;
const signalBoard = (): SignalBoard => {
  const held = signalsListed();
  const lines = held.flatMap(linesOf);
  const signals = held.map(signalOf);
  return { sources: signalSourcesSeed(signals, lines), subscriptions: lines, signals, signalsMore: 0, held: 0, rev: ++boardRev };
};
// The sources the sensors read, as the Tools tab names them (the snapshot's sources).
const SENSED_SOURCES: ToolSource[] = [
  { id: 'github', label: 'GitHub', capability: 'customers', tools: [], connections: [] },
  { id: 'datadog', label: 'Datadog', capability: 'observe', tools: [], connections: [] },
];
const copy = <T>(x: T): T => structuredClone(x);
const fake = fakeServer({
  // What opens is an event signal: its readings are its events the store holds.
  signalGet: async (address) => {
    // A subscription's page shows the readings of the signal it hears.
    const key = signalsListed().flatMap(linesOf).find((l) => l.id === address)?.key;
    const name = (key ?? address).replace(/^(signal:)?event:/, '');
    return { title: `Events of ${name}`, key: `event:${name}`, readings: readingsOf(name), firings: newestFirst().filter((e) => e.signal === name).flatMap((e) => e.firings) };
  },
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete as never,
  appSnapshot: async () => copy(snapshotOf({ objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [], chains: hookChains.map((c) => chainRead(c)), inbox, playbooks: [], capabilities: [], standings: hookStandings,
    workload: { running: 1, capacity: 4, pressure: '', queued: 1, waiting: 1, nextCheckBack: iso(Date.now() + 30 * MIN), rev: 1 }, signals: signalBoard(), insights: [], agents: [], skills: [], sources: SENSED_SOURCES, sensors: sensorsListed(), events: newestFirst().map(listed), eventSignals: signalsListed(), clientKeys: [], feedback: [],
    assistant: { ...assistantView(Date.now(), assistantCard(Date.now(), { status: 'idle', queued: 0, activity: '' }), (params.get('change') ? agentStands(ASSISTANT, ASSISTANT_CARD, 'Needs you', 'needs', 'Waits for your Apply', 'needs', [sensorChange.id], { asks: ['message', 'restart'] }) : assistantStanding('ready'))), drafts: drafts.live(ASSISTANT) } })),
  eventGet: async (id) => {
    const e = store.get(id);
    if (!e) throw new Error(`event ${id} not found`);
    return copy(e);
  },
  cardEvents: async (cardId) => ({ events: copy(cardId === ASSISTANT_CARD ? assistantStream : []), truncated: false }),
  // A key is read from the server, a moment later; the server may refuse (it holds a hash, or the key was lost).
  eventSignalKey: async (id) => {
    calls.push({ eventSignalKey: id });
    await wait(350);
    if (params.get('keyfail') || !eventSignals.has(id)) throw new Error('the key of this event signal cannot be read back: ask the Assistant to rotate it');
    return { key: eventSignalKeyOf(id) };
  },
  eventSignalPause: rail('eventSignalPause', true) as never,
  eventSignalResume: rail('eventSignalResume', false) as never,
  // The owner's message to the Assistant, kept in its stream as the server keeps it.
  assistantPrompt: async (draftId, where, delivery) => {
    const draft = drafts.get(draftId);
    if (!draft) throw new Error('this message was sent or deleted already');
    calls.push({ assistantPrompt: { draftId, text: draft.text, where, delivery } });
    const e = seedEvent({ id: ++nextId, home: ASSISTANT, cardId: ASSISTANT_CARD, createdAt: iso(Date.now()), updatedAt: iso(Date.now()) }, 'prompt',
      { by: OWNER, status: 'delivered', title: 'Your message', text: `${draft.text}`, data: { placed: { where, label: 'the Subscriptions page' } } });
    assistantStream.push(e);
    drafts.end([draftId]);
    void send('eventAdded', e);
    return e;
  },
  inboxAnswer: async (itemId, optionId) => {
    const it = inbox.find((i) => i.id === itemId)!;
    calls.push({ inboxAnswer: { itemId, optionId } });
    void send('inboxUpdated', { ...it, status: 'answered', answer: optionId, rev: it.rev + 1 });
    // Apply writes the sensor as the item showed it, recorded with who let it through (change_item.go applySensor).
    const issues = sensors.get('sn_i4');
    if (itemId === sensorChange.id && optionId === 'apply' && issues) {
      const next: Sensor = { ...issues, query: 'acme/shop state:all', by: ASSISTANT_ACTOR, updatedAt: iso(Date.now()), rev: issues.rev + 1 };
      sensors.set(next.id, next);
      void send('sensorUpdated', next);
    }
    return { ok: true } as never;
  },
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/events');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
Object.assign(window, { router, push: send, lose: fake.lose });

// An event's life, played: a new one arrives and no subscription holds for it; a few seconds later a subscription fires on it
// (the page open on it takes the firing as it is pushed).
const arrive = Number(params.get('arrive') ?? 0);
if (arrive) {
  setTimeout(() => {
    const fresh: EventRecord = { id: `event:gitlab#${nextId++}`, signal: 'gitlab', rev: nextId, at: iso(Date.now()), via: 'door', event: inboundEvent({ via: 'door', body: '{"object_kind":"pipeline"}' }), firings: [] };
    store.set(fresh.id, fresh);
    void send('eventRead', listed(fresh));
    setTimeout(() => {
      const fired = { ...fresh, firings: [firingOf('tell', 'told the Chair of ch_deploy: the pipeline of main failed at test', Date.now(), 'sub_tell_gitlab')] };
      store.set(fired.id, fired);
      void send('eventRead', listed(fired));
    }, 3000);
  }, arrive * 1000);
}
