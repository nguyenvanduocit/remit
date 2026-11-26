// The agent drawer (header, activity, message box) at the width the app opens it, on made-up data. No server, no sign-in:
// a fake server answers the calls of the page's real client core. Open /verify/drawer.html; ?case=busy|idle|chair|
// chair-idle|stopped|watching|mentor-offered|mentor-helping picks the agent (default busy: a worker mid-turn with a long
// name and an owner message waiting in its queue; watching: an idle worker whose Monitor and background command run on;
// mentor-offered: a worker whose Chair asked Lan, a mentor, to help, no answer yet; mentor-helping: Lan accepted and steers its
// session, her message in its stream); ?width=320 sets the drawer width (the app allows 320 to 720); ?typed=1 has words in the box already, as the
// server's draft; ?handover=1 opens the card whose work the Chair handed to Lan: stopped, with its handover decision. The calls
// the buttons make are kept in window.calls.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import CardDrawer from '../src/components/CardDrawer.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { Toaster } from '../src/components/ui/sonner';
import { apiKey } from '../src/core/api';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { fakeDrafts } from './fakeDrafts';
import { agentStands, stands } from './standingSeed';
import type { Card, DecisionRead, EventKind, EventRead, Help, Mentor } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { turnEnd } from './activitySeed';
import { actorOf, cardRecord, chainRead, chainRecord, inboxRecord, optionOf, decisionRead, decisionRecord, draftRecord, helpRecord, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const which = params.get('case') ?? 'busy';
const width = params.get('width') ?? '420';
const MIN = 60_000;
const t0 = Date.now() - 20 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const chain = chainRecord({ id: 'c1', title: 'adsfns rate-limit', goal: 'Find out how adsfns behaves at its rate limit.', status: 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(Date.now()), rev: 1 });
const card = (id: string, name: string, extra: Partial<Card>): Card =>
  cardRecord({ id, home: 'c1', agent: 'qc', name, origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', sessionId: 's-' + id, status: 'idle', plan: [], ctxUsed: 42_000, ctxSize: 200_000, turns: 3, queued: 0, createdAt: iso(t0), updatedAt: iso(Date.now()), rev: 1, ...extra });
const cards: Record<string, Card> = {
  busy: card('qc', 'QC · adsfns rate-limit behaviour under a burst of 429s', { status: 'running', activity: 'running the tests', queued: 1 }),
  idle: card('qc', 'QC · adsfns rate-limit behaviour under a burst of 429s', { status: 'idle' }),
  stopped: card('qc', 'QC · adsfns rate-limit behaviour under a burst of 429s', { status: 'stopped' }),
  watching: card('qc', 'QC · adsfns rate-limit behaviour under a burst of 429s', { status: 'idle', activity: 'watching: tail the 429 log' }),
  chair: card('chair', 'Chair', { origin: 'chair', agent: 'chair', status: 'running' }),
  'chair-idle': card('chair', 'Chair', { origin: 'chair', agent: 'chair', status: 'idle' }),
  'mentor-offered': card('pay', 'Engineer · checkout button states and their contrast', { agent: 'engineer', status: 'idle' }),
  'mentor-helping': card('pay', 'Engineer · checkout button states and their contrast', { agent: 'engineer', status: 'running', activity: 'restyling the disabled state' }),
};
const handover = params.get('handover') === '1';
const open = handover
  ? card('eng', 'Engineer · pay', { agent: 'engineer', status: 'stopped', activity: 'handed over to Lan' })
  : (cards[which] ?? cards.busy!);
const busy = open.status === 'running';
const watching = which === 'watching';
// What Claude Code did to the session beside the agent's work, as the server records it (agent's NoteKind).
const noted = which === 'notes';
const mentorCase = which.startsWith('mentor-');

// Lan, a mentor in the pool, and her help on the open card as the server sends it: offered and waiting for her answer, or
// accepted, her messages steering the session.
const lan: Mentor = { id: 'mt_lan', label: 'Lan', strengths: 'visual design, accessible colour', status: 'active', rev: 1, createdAt: iso(t0 - 3 * 24 * 60 * MIN) };
const help: Help = helpRecord({
  id: 'help_1', chain: 'c1', card: open.id, mentor: lan.id, problem: 'The disabled button state fails contrast and nobody here can judge the palette.',
  state: which === 'mentor-helping' ? 'accepted' : 'offered', offeredAt: iso(t0 + 4 * MIN), deadline: iso(t0 + 4 * MIN + 2 * 60 * MIN),
  ...(which === 'mentor-helping' && { answeredAt: iso(t0 + 7 * MIN) }), rev: 1,
});

let n = 0;
const ev = <K extends EventKind>(kind: K, min: number, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++n, home: 'c1', cardId: open.id, createdAt: iso(t0 + min * MIN), updatedAt: iso(t0 + min * MIN) }, kind, extra);
const events: EventRead[] = [
  ev('prompt', 1, { text: 'Check how adsfns behaves when the rate limit is hit and report.', by: actorOf('agent', 'chair'), status: 'delivered', title: 'Brief' }),
  ev('message', 6, { text: 'Reading the client and its retry policy first.' }),
  ev('prompt', 15, { text: 'Also check the 429 path.', by: actorOf('owner', 'owner'), status: busy ? 'queued' : 'delivered', title: 'Your message' }),
];
if (handover)
  events.push(
    ev('message', 9, { text: 'The flaky test still fails one run in ten; I could not find the race.' }),
    ev('stop', 9.2, { ...turnEnd(), status: 'end_turn' }),
    ev('prompt', 12, { text: '[from Remit]\nYour work on this task moves to someone else, who carries on in this worktree. Write them a handover note as your whole answer.', by: actorOf('remit', 'handover'), status: 'delivered', title: 'Write a handover note' }),
    ev('message', 12.5, { text: 'Note: retry logic is in pay.go:40; the race is between refill and read. Uncommitted: notes.txt.' }),
    ev('stop', 12.6, { ...turnEnd(), status: 'end_turn' }),
    ev('system', 12.7, { status: 'handed_over', by: actorOf('agent', 'chair'), text: 'The work was handed over to Lan.' }),
  );
if (which === 'mentor-helping')
  events.push(
    ev('prompt', 7.5, { text: 'Lan is your mentor\n[from Remit] Lan, strong in visual design, accessible colour, is now your mentor on this card.', by: actorOf('remit'), status: 'delivered', title: 'Lan is your mentor' }),
    ev('prompt', 16, { text: '[from Lan, your mentor]\nKeep the disabled state at 3:1 against the panel: use the mut token, not a lighter grey, and drop the opacity.', by: actorOf('mentor', help.id), status: 'steered', title: 'Keep the disabled state at 3:1 against the panel' }),
    ev('message', 16.4, { text: 'Switching the disabled text to the mut token and removing the opacity.' }),
  );
if (watching)
  events.push(
    ev('task', 16, { title: 'tail the 429 log until the burst ends', status: 'running', data: { tool: 'Monitor', command: 'tail -f burst.log | grep --line-buffered 429', readable: true, stoppable: true } }),
    ev('task', 16.2, { title: 'Run the whole make test on pristine HEAD clone (cold)', status: 'completed', text: 'Background command "Run the whole make test on pristine HEAD clone (cold)" completed (exit code 0)', data: { tool: 'Bash', command: 'cd $S/head && ( /usr/bin/time -p gmake test > $S/maketest1.log 2>&1; echo "exit=$?" >> $S/maketest1.log )', readable: true, output: '\n[exited with code 0]' } }),
    ev('message', 16.5, { text: 'Started the burst and a Monitor on the 429 log; I will report the backoff when the burst ends.' }),
    ev('stop', 16.6, { ...turnEnd(), status: 'end_turn' }),
  );

if (noted)
  events.push(
    ev('system', 2, { status: 'instructions_read', title: 'CLAUDE.md', text: "Project instructions, at the session's start" }),
    ev('system', 2, { status: 'instructions_read', title: 'AGENTS.md', text: 'Project instructions, included by another' }),
    ev('system', 7, { status: 'retrying', text: 'The request to the model failed (overloaded, HTTP 529); Claude Code tries again in 4 s, attempt 2 of 10.' }),
    ev('message', 8, { text: 'Back: the throttle refills every second, so a burst of 50 hits the limit at request 21.' }),
    ev('system', 10, { status: 'compacted', text: 'Claude Code compacted the conversation (auto): 412k tokens to 38k.' }),
    ev('system', 11, { status: 'model_switched', text: 'Claude Code switched the model from claude-opus-5-5 to claude-sonnet-5-5 on its own.' }),
    ev('system', 12, { status: 'refused', text: 'Kept a change to /work/ch_1/app/.claude/settings.json out of the running session: its settings change only between sessions.' }),
    ev('system', 13, { status: 'refused', text: 'Refused a call of Bash: The chain was paused: Remit starts no more tool calls in this turn. Stop here.' }),
  );

// The Chair asked Lan to help this card and she accepted (the drawer's decisions list).
const decisions: DecisionRead[] = handover
  ? [
      decisionRead(decisionRecord({ id: 'd-handover', chainId: 'c1', kind: 'handover', by: actorOf('agent', 'chair'), text: 'Hand Engineer · pay to Lan.', why: 'Stuck on the flaky test after one send-back: Lan knows the throttle and can reproduce the race on her machine.', cardId: open.id, createdAt: iso(t0 + 11 * MIN) }), false),
    ]
  : mentorCase
    ? [
        decisionRead(decisionRecord({ id: 'd-help-1', chainId: 'c1', kind: 'help', by: actorOf('agent', 'chair'), text: `Asked Lan to help ${open.name}: ${help.problem}`, cardId: open.id, createdAt: help.offeredAt }), false),
        ...(help.state === 'accepted' ? [decisionRead(decisionRecord({ id: 'd-help-2', chainId: 'c1', kind: 'help', by: actorOf('mentor', help.id), text: `Lan accepted to help ${open.name}.`, cardId: open.id, createdAt: help.answeredAt! }), false)] : []),
      ]
    : [];

const questions = which === 'question' ? [inboxRecord({
  id: 'q1', home: chain.id, cardId: open.id, kind: 'question', title: 'When should the service restart?',
  detail: Array.from({ length: 12 }, (_, i) => `Check ${i + 1}: the installed version is ready. The service will load it at its next restart.`).join('\n\n'),
  options: [optionOf('now', 'Restart now'), optionOf('later', 'At the next restart')], status: 'open', createdAt: iso(Date.now()),
})] : [];
if (questions.length) events.push(ev('message', 19, { text: Array.from({ length: 18 }, (_, i) => `Checked service ${i + 1} and recorded its running revision.`).join('\n\n') }));

const calls: string[] = [];
(window as unknown as { calls: string[] }).calls = calls;
const seed = params.get('typed') ? [draftRecord({ id: 'd1', home: 'c1', cardId: open.id, text: 'Run it with a burst of 50 requests too, and say how long the backoff waits.', updatedAt: iso(Date.now()) })] : [];
localStorage.removeItem('verify.drawer.drafts');
const drafts = fakeDrafts('drawer', (name, arg) => fake.push(name as never, arg as never), seed);
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], mentors: [lan], inbox: questions }),
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete,
  chainGet: () => ({ chain: chainRead(chain), cards: [open], edges: [], retrospectives: [], decisions, datadog: [], landings: [], drafts: drafts.live('c1'), workflows: [], helps: mentorCase ? [help] : [], verification: { chainId: 'c1', gates: [], judges: [], rev: 0 }, standing: stands('c1', 'Running', 'running'),
    agents: [
      questions.length ? agentStands(chain.id, open.id, 'Needs you', 'needs', questions[0]!.title, 'needs', questions.map((q) => q.id)) : watching
        ? agentStands('c1', open.id, 'Watching', 'running', 'Watching: tail the 429 log until the burst ends (+1)', 'busy', [], { asks: ['message', 'stop'] })
        : agentStands('c1', open.id, busy ? 'Running' : 'Idle', busy ? 'running' : 'idle', busy ? 'Running a command…' : 'Turn done', busy ? 'busy' : 'done', [],
            { asks: busy ? ['message', 'steer', 'interrupt', 'stop'] : ['message', 'stop'] }),
    ] }),
  chainEvents: () => ({ events, truncated: false }),
  cardEvents: () => ({ events, truncated: false }),
  cardPrompt: (draftId, delivery) => (calls.push(`cardPrompt ${draftId} delivery=${delivery}`), drafts.end([draftId]), events[2]),
  cardCancel: (id) => (calls.push(`cardCancel ${id}`), {}),
  cardStop: (id, why) => (calls.push(`cardStop ${id} why=${why}`), open),
});
openChain('c1');

const stub = { render: () => null };
const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: stub }, { path: '/chains/:id/:tab?', name: 'chain', component: stub }] });
await router.push('/');
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () => [
      h('div', { style: `display:flex;width:${width}px;height:600px;margin:16px;border:1px solid var(--line)` }, [opened.view ? h(CardDrawer, { view: opened.view, card: opened.view.cards[0] }) : null]),
      h(Toaster),
    ]),
})
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
