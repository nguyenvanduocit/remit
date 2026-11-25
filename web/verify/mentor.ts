// A mentor's app (MentorShell, ADR-0089) on made-up data, through the page's real client core: the server says the page
// came in as Lan, a mentor, and lays out her board (Asked, Helping, Finished). No server, no sign-in. Open
// /verify/mentor.html?at=<path>: ?at=/board, ?at=/chains/landing (the help she was asked: Accept or Decline),
// ?at=/chains/checkout (the agent she helps: its stream, and her box to message it). Accept moves the asked help to Helping
// and opens its stream; a message she sends shows in the stream as hers. What the page was asked is in window.calls.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { createMemoryHistory } from 'vue-router';
import App from '../src/App.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { signedIn } from '../src/session';
import { appRouter } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { fakeDrafts } from './fakeDrafts';
import { agentStands, boardSeed, stands } from './standingSeed';
import { tool, turnEnd } from './activitySeed';
import type { Card, Chain, ChainStanding, ChainView, EventKind, EventRead, Help, HelpState } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, cardRecord, chainRead, chainRecord, mentorRecord, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();
const calls: unknown[] = [];
(window as unknown as { calls: unknown[] }).calls = calls;

const chain = (id: string, title: string, status: Chain['status'], ageMin: number): Chain => chainRecord({
  id, title, goal: `${title}.`, status, chairId: `${id}-chair`, workdir: `~/.remit/work/${id}`, origin: 'owner',
  createdAt: iso(now - ageMin * MIN), updatedAt: iso(now - 4 * MIN),
});
const chains = [
  chain('landing', 'Landing page for Lights Daily', 'running', 120),
  chain('checkout', 'Checkout copy for the coupon change', 'running', 300),
  chain('onboarding', 'Onboarding emails, first week', 'done', 3000),
];
const card = (chainId: string, id: string, name: string): Card => cardRecord({
  id, home: chainId, agent: 'designer', name, origin: 'hired', model: 'opus', mode: 'auto', cwd: `~/.remit/work/${chainId}`, status: 'idle', plan: [], ctxUsed: 61_000, ctxSize: 200_000,
  turns: 4, queued: 0, sessionId: `s-${id}`, createdAt: iso(now - 100 * MIN), updatedAt: iso(now - 3 * MIN), rev: 1,
  brief: 'For: first-time players who land from Reddit.\nMake the landing page say in one screen what Lights Daily is.',
});
const cards: Record<string, Card> = {
  landing: card('landing', 'designer-landing', 'Designer · landing page'),
  checkout: card('checkout', 'writer-checkout', 'Writer · checkout copy'),
  onboarding: card('onboarding', 'writer-onboarding', 'Writer · onboarding emails'),
};
const help = (id: string, chainId: string, state: HelpState, problem: string, rev = 1): Help => ({
  id, chain: chainId, card: cards[chainId].id, mentor: 'm_lan', problem, state, offeredAt: iso(now - 40 * MIN), deadline: iso(now + 20 * 60 * MIN),
  ...(state !== 'offered' && { answeredAt: iso(now - 30 * MIN) }), rev,
}) as Help;
const helps: Help[] = [
  help('h_landing', 'landing', 'offered', 'Taste: which of the two hero lines reads as a game, not an app?'),
  help('h_checkout', 'checkout', 'accepted', 'Tone: the coupon error should not sound like the shopper did something wrong.'),
  help('h_onboarding', 'onboarding', 'ended', 'Whether day 3 is too early for the referral ask.'),
];
const columnOf = (h: Help) => (h.state === 'offered' ? 'asked' : h.state === 'accepted' ? 'helping' : 'finished');
const boardOf = () => boardSeed(
  { asked: helps.filter((h) => columnOf(h) === 'asked').map((h) => h.chain), helping: helps.filter((h) => columnOf(h) === 'helping').map((h) => h.chain), finished: helps.filter((h) => columnOf(h) === 'finished').map((h) => h.chain) },
  [], 0, [], ['asked', 'helping', 'finished'],
);
const word = { offered: 'Asked', accepted: 'Helping', declined: 'Declined', ended: 'Ended' } as const;
const standingOf = (h: Help): ChainStanding => stands(h.chain, `${word[h.state]} · ${h.problem}`, h.state === 'offered' ? 'needs' : h.state === 'accepted' ? 'running' : 'done', { from: '' });

let nextId = 100;
const ev = <K extends EventKind>(chainId: string, cardId: string, kind: K, minAgo: number, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++nextId, home: chainId, cardId, createdAt: iso(now - minAgo * MIN), updatedAt: iso(now - minAgo * MIN) }, kind, extra);
const LAN = actorOf('mentor', 'h_checkout');
const streams: Record<string, EventRead[]> = {
  'writer-checkout': [
    ev('checkout', 'writer-checkout', 'prompt', 31, { by: actorOf('remit'), status: 'delivered', title: 'Lan Nguyen is your mentor', text: '[from the Chair] Lan Nguyen is now your mentor on this card, for: tone of the coupon error.' }),
    ev('checkout', 'writer-checkout', 'message', 30, { text: 'Lan, two questions before I rewrite the coupon errors:\n\n1. Should the expired-coupon line name the date it expired?\n2. "This code is not valid" or "We could not apply this code"?' }),
    ev('checkout', 'writer-checkout', 'tool', 29, { title: 'Read src/checkout/coupon.ts', ...tool('read', 'read', 'src/checkout/coupon.ts'), status: 'completed' }),
    ev('checkout', 'writer-checkout', 'stop', 28, { ...turnEnd(5), status: 'end_turn' }),
  ],
};
// Where the agent stands as the server words it for her (`for_mentor`): as for the owner once she helps, else of the help.
const agentOf = (h: Help, cardId: string) =>
  h.state === 'accepted'
    ? agentStands(h.chain, cardId, 'Idle', 'idle', 'Stopped · message to resume', 'done', [], { asks: ['message'] })
    : h.state === 'offered'
      ? agentStands(h.chain, cardId, 'Asks your help', 'needs', 'Accept to read its session and write to it', 'needs', [], { asks: [] })
      : agentStands(h.chain, cardId, h.state === 'declined' ? 'You declined' : 'Help ended', 'done', h.state === 'declined' ? 'You declined this help' : 'This help has ended', 'done', [], { asks: [] });
const view = (id: string): ChainView => {
  const c = chains.find((x) => x.id === id);
  const h = helps.find((x) => x.chain === id);
  if (!c || !h) throw new Error(`you help no agent of chain ${id}`);
  const k = cards[id];
  return { chain: chainRead(c), cards: [k], edges: [], decisions: [], helps: [h], verification: { chainId: id, gates: [], judges: [], rev: 1 }, datadog: [], landings: [], drafts: drafts.live(id), workflows: [],
    agents: [agentOf(h, k.id)], standing: standingOf(h), retrospectives: [] };
};
const drafts = fakeDrafts('mentor', (name, arg) => fake.push(name as never, arg as never));
const answer = (helpId: string, state: HelpState) => {
  const h = helps.find((x) => x.id === helpId);
  if (!h || h.state !== 'offered') throw new Error(`this help is ${h?.state ?? 'not yours'}: only an open offer is answered`);
  Object.assign(h, { state, answeredAt: new Date().toISOString(), rev: h.rev + 1 });
  fake.push('helpUpdated', { ...h });
  fake.push('boardUpdated', { ...boardOf(), rev: 1000 + h.rev });
  fake.push('chainStandingUpdated', standingOf(h));
  fake.push('agentStandingUpdated', view(h.chain).agents[0]);
  if (state === 'accepted') {
    streams[h.card] = [ev(h.chain, h.card, 'message', 0, { text: 'Lan, which hero line reads as a game to you: "One light a day" or "Light them all before midnight"?' })];
    streams[h.card].forEach((e) => fake.push('eventAdded', e));
  }
  return { ...h };
};
const fake = fakeServer({
  appSnapshot: async () => snapshotOf({
    who: { kind: 'mentor', mentorId: 'm_lan', name: 'Lan Nguyen', keyId: 'wk1' },
    chains: chains.map((c) => chainRead(c)), board: boardOf(), standings: helps.map(standingOf),
    mentors: [mentorRecord({ id: 'm_lan', label: 'Lan Nguyen', status: 'active', createdAt: iso(now - 7 * 24 * 60 * MIN) })],
  }),
  chainGet: async (id) => view(id),
  chainEvents: async (id) => {
    const h = helps.find((x) => x.chain === id);
    return { events: h && h.state !== 'offered' ? (streams[h.card] ?? []) : [], truncated: false };
  },
  cardEvents: async (cardId) => {
    const h = helps.find((x) => x.card === cardId);
    if (!h || h.state === 'offered') throw new Error('this help is offered: you read the card\'s session once you accepted to help it');
    return { events: streams[cardId] ?? [], truncated: false };
  },
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete,
  cardPrompt: async (draftId) => {
    calls.push(['cardPrompt', draftId]);
    const d = drafts.get(draftId);
    if (!d) throw new Error('this message was sent or deleted already');
    const e = ev(d.home, d.cardId!, 'prompt', 0, { by: LAN, status: 'queued', title: "Lan Nguyen's message", text: `[from Lan Nguyen, your mentor]\n${d.text}` });
    drafts.end([draftId]);
    fake.push('eventAdded', e);
    return e;
  },
  helpAccept: async (helpId) => (calls.push(['helpAccept', helpId]), answer(helpId, 'accepted')),
  helpDecline: async (helpId, why) => (calls.push(['helpDecline', helpId, why]), answer(helpId, 'declined')),
});
void fake;
const router = appRouter(createMemoryHistory());
signedIn();
await router.push(params.get('at') ?? '/board');
createApp(App).use(router).provide(apiKey, api).mount('#app');
