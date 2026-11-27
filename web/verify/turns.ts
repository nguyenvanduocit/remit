// An agent's turns as their rows tell them (TurnLine, the server's TurnSummary), in its drawer and in the chain's Live feed side
// by side, on made-up data. No server, no sign-in. Open /verify/turns.html.
// The engineer's turns: a busy one (fourteen steps, a test run three times, a file edited three times), a turn of one step
// repeated many times, a turn where a build failed and a delete was refused, a run of short turns, a turn three hours after
// the one before, a turn too long for the server to read whole (`partial`), a turn cancelled before it ended, and the turn it
// is in now, still running, after a restart cut off its previous turn. Its replies carry Helpful / Not helpful / Correct reply.
// window.add(n) plays the server adding n events to the running turn (n tool calls and a message), as the scroll check does;
// ?play=<ms> adds one every <ms>.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import CardDrawer from '../src/components/CardDrawer.vue';
import LiveView from '../src/components/LiveView.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { Toaster } from '../src/components/ui/sonner';
import { apiKey } from '../src/core/api';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { agentStands, stands } from './standingSeed';
import type { EventKind, EventRead, StopReason, ToolGroup, ToolState, Touched, TurnFailure } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, cardRecord, chainRead, chainRecord, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const t0 = Date.now() - 6 * 60 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const chain = chainRecord({ id: 'c1', title: 'Orders status column', goal: 'Add a status column to orders without taking checkout down.', status: 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(Date.now()) });
const card = cardRecord({ id: 'eng', home: 'c1', agent: 'engineer', name: 'Engineer · orders migration', origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', sessionId: 's-eng', status: 'running', activity: 'running the tests', plan: [], ctxUsed: 82_000, ctxSize: 200_000, turns: 9, queued: 0, createdAt: iso(t0), updatedAt: iso(Date.now()), rev: 1 });

let n = 0;
let at = t0;
const events: EventRead[] = [];
const ev = <K extends EventKind>(kind: K, gapMs: number, extra: EventSeed<K> = {}): EventRead => {
  at += gapMs;
  const e = seedEvent({ id: ++n, home: 'c1', cardId: 'eng', createdAt: iso(at), updatedAt: iso(at + 1500) }, kind, extra);
  events.push(e);
  return e;
};
const chair = actorOf('agent', 'chair');
const prompt = (text: string, gap = 20_000) => ev('prompt', gap, { title: text.split('\n')[0], text: '[from the Chair]\n' + text, by: chair, status: 'delivered' });
const tool = (group: ToolGroup, name: string, summary: string, status: ToolState = 'completed') =>
  ev('tool', 2_000, { activity: { type: 'tool', group, name, summary, status }, title: summary, toolCallId: 'tc' + (n + 1), status: status === 'running' ? 'in_progress' : status });
const say = (text: string) => ev('message', 3_000, { text });
const touched = (group: ToolGroup, name: string, summary: string, count = 1): Touched => ({ group, name, summary, count });
const end = (ms: number, touchedList: Touched[], failures: TurnFailure[] = [], partial = false, status: StopReason = 'end_turn') =>
  ev('stop', 1_000, { status, activity: { type: 'turn_end', ms, turn: { touched: touchedList, failures, partial } } });

// A busy turn: fourteen steps, three runs of the same test, the same migration edited three times.
prompt('Add `status` to `orders` with a backfill, and keep checkout up.');
tool('read', 'Read', 'crates/store/migrations/0074.sql');
tool('read', 'Read', 'crates/store/src/orders.rs');
tool('search', 'rg', 'rg -n "status" crates/store');
ev('thought', 2_000, { text: 'The column must be nullable first, then backfilled in batches of 5,000, then set NOT NULL, so no step locks the table for long.' });
tool('edit', 'Edit', 'crates/store/migrations/0075.sql');
tool('execute', 'cargo', 'cargo nextest run -p store');
tool('edit', 'Edit', 'crates/store/migrations/0075.sql');
tool('execute', 'cargo', 'cargo nextest run -p store');
tool('read', 'Read', 'crates/store/src/orders.rs');
tool('edit', 'Edit', 'crates/store/migrations/0075.sql');
tool('edit', 'Edit', 'crates/store/src/orders.rs');
tool('execute', 'cargo', 'cargo nextest run -p store');
tool('execute', 'cargo', 'cargo clippy -p store --all-targets -- -D warnings');
tool('remit', 'remit', 'submit');
say('The migration adds `status` nullable, backfills it in batches of 5,000 and then sets it NOT NULL: no step holds a lock on `orders` for more than a second. The store tests pass.');
end(41_000, [
  touched('read', 'Read', 'crates/store/migrations/0074.sql'),
  touched('read', 'Read', 'crates/store/src/orders.rs', 2),
  touched('search', 'rg', 'rg -n "status" crates/store'),
  touched('edit', 'Edit', 'crates/store/migrations/0075.sql', 3),
  touched('execute', 'cargo', 'cargo nextest run -p store', 3),
  touched('edit', 'Edit', 'crates/store/src/orders.rs'),
  touched('execute', 'cargo', 'cargo clippy -p store --all-targets -- -D warnings'),
  touched('remit', 'remit', 'submit'),
]);

// A turn of one step repeated: the same file read eleven times.
prompt('Check the backfill against the replica before you merge.');
for (let i = 0; i < 11; i++) tool('execute', 'psql', 'psql replica -c "select count(*) from orders where status is null"');
say('The replica shows 0 rows left without a status after the eleventh check.');
end(96_000, [touched('execute', 'psql', 'psql replica -c "select count(*) from orders where status is null"', 11)]);

// A failed turn: the build failed twice and a delete was refused.
prompt('Clear the build cache and build again.');
const f1 = tool('execute', 'cargo', 'cargo build --release', 'failed');
const r1 = ev('system', 1_000, { activity: { type: 'decided', request: 'rm -rf target', allowed: false }, by: actorOf('remit', 'rule'), text: 'Deleting outside the worktree is refused.', title: 'rm -rf target' });
const f2 = tool('execute', 'cargo', 'cargo build --release', 'failed');
say('The release build fails on a missing `openssl` header, and I may not clear `target`. I need the header installed to go on.');
end(18_000, [touched('execute', 'cargo', 'cargo build --release', 2)], [
  { eventId: f1.event.id, kind: 'failed', name: 'cargo', summary: 'cargo build --release: exit 101, openssl/ssl.h not found' },
  { eventId: r1.event.id, kind: 'refused', name: 'Bash', summary: 'rm -rf target' },
  { eventId: f2.event.id, kind: 'failed', name: 'cargo', summary: 'cargo build --release: exit 101, openssl/ssl.h not found' },
]);

// A run of short turns: a question, one look, an answer.
for (const [q, file] of [['Which file holds the backfill?', 'crates/store/migrations/0075.sql'], ['Batch size?', 'crates/store/migrations/0075.sql'], ['Does checkout read status yet?', 'web/src/checkout.ts']] as const) {
  prompt(q, 8_000);
  tool('read', 'Read', file);
  say(`In \`${file}\`.`);
  end(3_000, [touched('read', 'Read', file)]);
}

// A long idle gap: the next turn comes three hours later.
at += 3 * 60 * MIN;
prompt('The header is installed: build and ship to the canary.');
const many: Touched[] = Array.from({ length: 14 }, (_, i) => touched(i % 3 ? 'read' : 'edit', i % 3 ? 'Read' : 'Edit', `crates/orders/src/module_${i}.rs`, (i % 4) + 1));
for (let i = 0; i < 6; i++) tool('read', 'Read', `crates/orders/src/module_${i}.rs`);
say('Built, and the canary stage takes 5% of checkout now.');
end(612_000, many, [], true);

// A turn cancelled before it ended.
prompt('Summarise the whole migration for the review.');
tool('read', 'Read', 'docs/adr/0091-an-agent-shows-a-block.md');
end(54_000, [touched('read', 'Read', 'docs/adr/0091-an-agent-shows-a-block.md')], [], false, 'cancelled');

// The server restarted before this turn wrote its summary.
prompt('Watch the canary for ten minutes and report.', 30_000);
tool('read', 'Read', 'canary/config.toml');
say('I found the health check for the canary.');
ev('thought', 2_000, { text: 'I can check the health endpoint directly.' });
tool('execute', 'curl', 'curl -s canary/healthz', 'failed');

// The turn it is in now.
ev('prompt', 30_000, { title: 'Resume after a restart', status: 'delivered' });
tool('execute', 'curl', 'curl -s canary/healthz', 'running');

const calls: string[] = [];
(window as unknown as { calls: string[] }).calls = calls;
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)] }),
  chainGet: () => ({ chain: chainRead(chain), cards: [card], edges: [], retrospectives: [], decisions: [], datadog: [], landings: [], drafts: [], workflows: [], helps: [], verification: { chainId: 'c1', gates: [], judges: [], rev: 0 }, standing: stands('c1', 'Running', 'running'),
    agents: [agentStands('c1', 'eng', 'Running', 'running', 'Running a command…', 'busy', [], { asks: ['message', 'steer', 'interrupt', 'stop'] })] }),
  chainEvents: () => ({ events, truncated: false }),
  cardEvents: () => ({ events, truncated: false }),
  turnTeach: (eventId, mark, words) => (calls.push(`turnTeach ${eventId} ${mark} ${JSON.stringify(words)}`), {}),
});

// The server adding to the running turn: a tool call, and every fourth a line the agent says.
let played = 0;
const add = (count = 1) => {
  for (let i = 0; i < count; i++) {
    played++;
    at = Math.max(at + 1_000, Date.now() - 1_000);
    const where = { id: ++n, home: 'c1', cardId: 'eng', createdAt: iso(at), updatedAt: iso(at) };
    fake.push('eventAdded', played % 4
      ? seedEvent(where, 'tool', { activity: { type: 'tool', group: 'execute', name: 'curl', summary: `curl -s canary/healthz  # check ${played}`, status: 'completed' }, title: `check ${played}`, status: 'completed' })
      : seedEvent(where, 'message', { text: `Check ${played}: the canary answers 200 in 41 ms; error rate 0.0%.` }));
  }
};
(window as unknown as { add: typeof add }).add = add;
const play = Number(params.get('play'));
if (play) setInterval(() => add(1), play);
openChain('c1');

const stub = { render: () => null };
const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: stub }, { path: '/chains/:id/:tab?', name: 'chain', component: stub }] });
await router.push('/');
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () => [
      h('div', { style: 'display:flex;gap:16px;margin:16px;height:640px' }, opened.view
        ? [
            h('div', { 'data-pane': 'drawer', style: 'display:flex;width:420px;border:1px solid var(--line)' }, [h(CardDrawer, { view: opened.view, card: opened.view.cards[0] })]),
            h('div', { 'data-pane': 'live', style: 'display:flex;width:640px;border:1px solid var(--line);background:var(--bg)' }, [h(LiveView, { view: opened.view })]),
          ]
        : []),
      h(Toaster),
    ]),
})
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
