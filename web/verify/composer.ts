// The agent drawer's composer and what waits pinned over it, on made-up data, at the width the app opens the drawer. No server,
// no sign-in: a fake server answers the page's real client core. Open /verify/composer.html; ?case=busy|paused|idle picks the
// agent (default busy: an engineer mid-turn, its session taking input, so Send queues and Steer and Interrupt are offered, the
// server saying under the box that a message waits for the turn (+2 queued); one message of the owner's still queued, which he
// may take back into the box; an earlier group of tool calls with one failed; and its permission prompt pinned, which he may
// refuse with a reason). paused: the Chair of a paused chain, the box saying the Chair reads it first on Resume. idle: an agent
// that reads a message at once, so nothing is said under the box. ?width=<px> sets the drawer (the app allows 320 to 720).
// The calls the buttons make are kept in window.calls. ?send=slow answers a message after 1.5 s; ?send=fail fails the first
// one (the next goes). The connection banner shows over the drawer, and window.link.down() / window.link.up() take the server
// away and bring it back, to see messages sent offline wait and go; window.link.newBuild() turns the page away as another
// build, and the messages that wait go from the page loaded again (ADR-0094).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import CardDrawer from '../src/components/CardDrawer.vue';
import ConnectionBanner from '../src/components/ConnectionBanner.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { Toaster } from '../src/components/ui/sonner';
import { apiKey } from '../src/core/api';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { fakeDrafts } from './fakeDrafts';
import { agentStands, stands } from './standingSeed';
import { tool, turnEnd } from './activitySeed';
import type { AgentStanding, Delivery, EventKind, EventRead, Image } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, cardRecord, chainRead, chainRecord, inboxRecord, optionOf, permissionAsk, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const which = params.get('case') ?? 'busy';
let sending = params.get('send');
let failures = sending === 'fail' ? 1 : 0;
const width = params.get('width') ?? '420';
const MIN = 60_000;
const t0 = Date.now() - 20 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();
const owner = actorOf('owner', 'alexp');

const paused = which === 'paused';
const busy = which === 'busy';
const chain = chainRecord({ id: 'c1', title: 'Retire the legacy retention job', goal: 'Move event retention to a migration the gate runs.', status: paused ? 'paused' : 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(Date.now()), rev: 1 });
const open = cardRecord({
  id: paused ? 'chair' : 'eng', home: 'c1', agent: paused ? 'chair' : 'engineer', name: paused ? 'Chair' : 'Engineer · retention migration', origin: paused ? 'chair' : 'hired',
  model: 'opus', mode: 'auto', cwd: '/', sessionId: 's-eng', status: busy ? 'running' : 'idle', plan: [], ctxUsed: 42_000, ctxSize: 200_000, turns: 4,
  queued: busy ? 2 : 0, createdAt: iso(t0), updatedAt: iso(Date.now()), rev: 1,
});

let n = 0;
const ev = <K extends EventKind>(kind: K, min: number, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++n, home: 'c1', cardId: open.id, createdAt: iso(t0 + min * MIN), updatedAt: iso(t0 + min * MIN + 4000) }, kind, extra);
const events: EventRead[] = [
  ev('prompt', 1, { title: 'Write the retention migration', text: '[message from the Chair]\nWrite a migration that deletes events older than 90 days, and let the gate run it.', by: actorOf('agent', 'chair'), status: 'delivered' }),
  ev('tool', 2, { title: 'cargo test retention', ...tool('execute', 'cargo', 'test retention_keeps_90_days', 'failed') }),
  ev('tool', 2.2, { title: 'crates/store/migrations', ...tool('read', 'Read', 'crates/store/migrations/0091.sql') }),
  ev('tool', 2.4, { title: 'crates/store/src/events.rs', ...tool('edit', 'Edit', 'crates/store/src/events.rs') }),
  ev('message', 3, { text: 'The migration is ready; **one test still fails** (`retention_keeps_90_days`), retrying with a fixed clock.' }),
  ...(busy
    ? [
        ev('prompt', 4, { title: 'Your message', text: 'Use the shop timezone from shops.tz, not UTC, when the test builds its clock.', by: owner, status: 'queued' }),
        ev('tool', 5, { title: 'cargo test', ...tool('execute', 'cargo', 'test -p store', 'running') }),
      ]
    : [ev('stop', 4, { ...turnEnd(), status: 'end_turn' })]),
];
const permission = inboxRecord({
  id: 'i_perm', home: 'c1', cardId: open.id, kind: 'permission', title: 'Run psql -c "DELETE FROM events WHERE created_at < now() - interval \'90 days\'"',
  // The server's words (orch permission_ask.rs): the action, why it waits, what is risky, then the call's input for the
  // agents that decide; here a call the auto-mode classifier refused that came back to be asked (ADR-0083).
  detail: `Claude Code's safety check stopped it (Safety Bypass Flag), so it runs only if it is allowed.
Risk high: cannot be undone, production, outside the sandbox, network.
{
  "command": "psql -c \"DELETE FROM events WHERE created_at < now() - interval '90 days'\"",
  "dangerouslyDisableSandbox": true,
  "description": "Delete old events on prod"
}`,
  permission: permissionAsk({
    action: `Run psql -c "DELETE FROM events WHERE created_at < now() - interval '90 days'"`,
    said: 'Delete old events on prod',
    reason: "Claude Code's safety check stopped it (Safety Bypass Flag), so it runs only if it is allowed.",
    traits: [
      { kind: 'irreversible', says: 'cannot be undone' },
      { kind: 'production', says: 'production' },
      { kind: 'unsandboxed', says: 'outside the sandbox' },
      { kind: 'network', says: 'network' },
    ],
    risk: { kind: 'high', says: 'High risk' },
    tool: 'Bash',
    command: `psql -c "DELETE FROM events WHERE created_at < now() - interval '90 days'"`,
  }),
  options: [optionOf('allow', 'Allow', 'allow_once'), optionOf('reject', 'Refuse', 'reject_once')], status: 'open', createdAt: iso(t0 + 5 * MIN), rev: 1,
});
const ON_SEND: Record<string, string | undefined> = {
  busy: 'Waits for the current turn (+2 queued)',
  paused: 'The chain is paused: the Chair reads it first on Resume',
};
const standing = (): AgentStanding => ({
  ...agentStands('c1', open.id, busy ? 'Needs you' : paused ? 'Paused' : 'Idle', busy ? 'needs' : 'idle', busy ? 'Approve: Run psql -c "DELETE FROM events …"' : 'Turn done', busy ? 'needs' : 'done', busy ? [permission.id] : [],
    { asks: busy ? ['message', 'steer', 'interrupt', 'stop'] : ['message', 'stop'] }),
  ...(ON_SEND[which] && { onSend: ON_SEND[which] }),
});

// Before the fake server: a page loaded again sends what waited as the core starts, which saves its draft here; what it
// pushes goes once the fake is up, after its answer, as a server's does.
const drafts = fakeDrafts(`composer-${which}`, (name, arg) => queueMicrotask(() => fake.push(name as never, arg as never)));
const calls: string[] = [];
const images = new Map<string, Image>();
(window as unknown as { calls: string[] }).calls = calls;
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], inbox: busy ? [permission] : [], standings: [stands('c1', paused ? 'Paused' : 'Running', paused ? 'paused' : 'running')] }),
  draftSave: (d, base) => drafts.draftSave(d, base),
  draftDelete: (id) => drafts.draftDelete(id),
  chainGet: () => ({ chain: chainRead(chain), cards: [open], edges: [], retrospectives: [], decisions: [], datadog: [], landings: [], drafts: drafts.live('c1'), workflows: [], helps: [], verification: { chainId: 'c1', gates: [], judges: [], rev: 0 }, standing: stands('c1', 'Running', 'running'), agents: [standing()] }),
  chainEvents: () => ({ events, truncated: false }),
  cardEvents: () => ({ events, truncated: false }),
  cardPrompt: (draftId, delivery) => {
    if (failures > 0) {
      failures -= 1;
      throw { code: 'FAILED', message: 'the agent was stopped before it read it' };
    }
    if (sending === 'slow') {
      sending = null;
      return new Promise<EventRead>((done) => setTimeout(() => done(prompt(draftId, delivery)), 1500));
    }
    return prompt(draftId, delivery);
  },
  // A queued message taken back: it leaves the stream and its words are the box's draft again, refused while the box holds words.
  cardPromptWithdraw: (eventId) => {
    const e = events.find((x) => x.event.id === eventId);
    if (!e || e.event.status !== 'queued') throw { code: 'FAILED', message: 'the agent has read it already' };
    const draft = drafts.draftSave({ id: `wd${eventId}`, home: 'c1', cardId: open.id, text: e.event.text, images: [] }, 0);
    calls.push(`cardPromptWithdraw ${eventId}`);
    events.splice(events.indexOf(e), 1);
    fake.push('eventDeleted', { id: eventId, home: 'c1', cardId: open.id });
    return draft;
  },
  inboxAnswer: (itemId, optionId, text) => {
    calls.push(`inboxAnswer ${itemId} ${optionId} ${JSON.stringify(text)}`);
    fake.push('inboxUpdated', { ...permission, status: 'answered', rev: 2 });
    fake.push('agentStandingUpdated', { ...standing(), label: 'Running', tone: 'running', doing: 'Running a command…', doingTone: 'busy', waiting: [], rev: 100 });
    return {};
  },
  // An image put is kept as given and read back by its id, as the server keeps its bytes.
  imagePut: (id, home, image) => (calls.push(`imagePut ${id} ${home} ${image.mediaType} ${image.data.length}`), images.set(id, image), {}),
  imageGet: (id) => {
    const i = images.get(id);
    if (!i) throw { code: 'NOT_FOUND', message: `no image ${id}` };
    return i;
  },
  cardCancel: (id) => (calls.push(`cardCancel ${id}`), {}),
  cardStop: (id) => (calls.push(`cardStop ${id}`), open),
});
// A message sent: it ends the draft and lands in the stream as the delivery says it meets the turn in progress.
function prompt(draftId: string, delivery: Delivery): EventRead {
  const text = drafts.get(draftId)?.text ?? '';
  const shots = drafts.get(draftId)?.images ?? [];
  calls.push(`cardPrompt ${delivery} ${text} images=${shots.length}`);
  drafts.end([draftId]);
  const e = ev('prompt', 6, { title: 'Your message', text: `${text}`, by: owner, status: busy && delivery === 'queue' ? 'queued' : delivery === 'steer' ? 'steered' : 'delivered', createdAt: iso(Date.now()), updatedAt: iso(Date.now()), ...(shots.length && { data: { images: shots } }) });
  events.push(e);
  queueMicrotask(() => fake.push('eventAdded', e));
  return e;
}
openChain('c1');

const stub = { render: () => null };
const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: stub }, { path: '/chains/:id/:tab?', name: 'chain', component: stub }] });
await router.push('/');
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () => [
      h('div', { style: `display:flex;flex-direction:column;width:${width}px;height:720px;margin:16px;border:1px solid var(--line)` }, [
        h(ConnectionBanner),
        h('div', { style: 'display:flex;flex:1;min-height:0' }, [opened.view ? h(CardDrawer, { view: opened.view, card: opened.view.cards[0] }) : null]),
      ]),
      h(Toaster),
    ]),
})
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
(window as unknown as { link: { down(): void; up(): void; newBuild(): void } }).link = {
  down: () => fake.goDown(),
  up: () => fake.comeUp(),
  newBuild: () => fake.refuse('new_bundle', 'the server runs a newer build'),
};
