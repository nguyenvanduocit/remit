// Inbox items that no longer wait, as the owner reads them (InboxCallout, ADR-0087), beside one that still does: a permission the
// Assistant decided for him, whose line in its feed says why and offers Overturn; one it decided that he overturned; a change
// he applied himself; an error Remit dismissed when the work moved again; a question withdrawn with no one deciding; and an
// open permission the Assistant left to him. Overturn sends `overturn`, and the feed line comes back with his word (?fail=1:
// the server refuses it). Open /verify/inboxdecided.html; ?width=<px> narrows the column.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import InboxCallout from '../src/components/InboxCallout.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { fakeServer, snapshotOf } from './fake';
import type { Actor, AssistantFeed, Decision, InboxItem, PermissionAsk } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, decisionRecord, inboxRecord, optionOf, permissionAsk } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (minAgo: number) => new Date(Date.now() - minAgo * MIN).toISOString();
const ASSISTANT: Actor = actorOf('assistant', 'assistant');
const OWNER: Actor = actorOf('owner', 'owner@example.com', 'web');
const item = (id: string, kind: InboxItem['kind'], title: string, detail: string, extra: Partial<InboxItem>): InboxItem =>
  inboxRecord({ id, home: 'busy', cardId: 'eng', kind, title, detail, status: 'answered', createdAt: iso(30), ...extra });
// The call in the server's words (orch permission_ask.rs), and once decided who ended it.
const ask = (action: string, command: string, traits: PermissionAsk['traits'], risk: PermissionAsk['risk'], reason: string, end?: Pick<PermissionAsk, 'outcome' | 'decided'>): PermissionAsk =>
  permissionAsk({ action, reason, traits, risk, tool: 'Bash', command, ...end });
const HIGH = { kind: 'high', says: 'High risk' } as const;
const MEDIUM = { kind: 'medium', says: 'Medium risk' } as const;
const allowDeny = [optionOf('a', 'Allow', 'allow_once'), optionOf('r', 'Refuse', 'reject_once')];

const items: InboxItem[] = [
  item('i_rm', 'permission', 'Run rm -rf /tmp/backfill-cache', 'Engineer wants to run `rm -rf /tmp/backfill-cache` in the chain\'s worktree.', { options: allowDeny, answer: 'Allow, once', answeredBy: ASSISTANT, answeredAt: iso(4),
    permission: ask('Run rm -rf /tmp/backfill-cache', 'rm -rf /tmp/backfill-cache', [{ kind: 'irreversible', says: 'cannot be undone' }, { kind: 'outside_worktree', says: 'writes outside the worktree' }], HIGH, 'It removes what cannot be brought back and writes outside its working directory, so it runs only if it is allowed.', { outcome: 'allowed', decided: 'Allowed by the Assistant, for you' }) }),
  item('i_push', 'permission', 'Push main to origin', 'Release wants to push to origin/main.', { options: allowDeny, answer: 'Refuse: let the merge queue land it instead', answeredBy: ASSISTANT, answeredAt: iso(18),
    permission: ask('Push main to origin', 'git push origin main', [{ kind: 'network', says: 'network' }], MEDIUM, 'It reaches another machine, so it runs only if it is allowed.', { outcome: 'refused', decided: 'Refused by the Assistant, for you' }) }),
  item('i_hire', 'change', 'Chair asks to hire Analyst #3 from the analyst template', 'Chair asks to write employee `analyst-3`.', { answer: 'Applied', answeredBy: OWNER, changeKind: 'employee' }),
  item('i_err', 'error', 'Engineer failed: the checks of !60 timed out', 'Error: the checks run past 30 minutes', { answer: 'work moved again', answeredBy: actorOf('remit') }),
  item('i_q', 'question', 'Which repo should the fix go in?', 'Which repo should the fix go in?', { status: 'cancelled' }),
  item('i_open', 'permission', 'Delete every event on production', 'Engineer wants to delete from events on production. You told the Assistant to ask you before anything writes to production.', { status: 'open', options: allowDeny,
    permission: ask('Delete every event on production', 'psql -h prod -c "delete from events"', [{ kind: 'irreversible', says: 'cannot be undone' }, { kind: 'production', says: 'production' }, { kind: 'network', says: 'network' }], HIGH, 'It removes what cannot be brought back and touches production, so it runs only if it is allowed.') }),
];
const d = (id: string, minAgo: number, text: string, why: string, it: string): Decision =>
  decisionRecord({ id, chainId: 'busy', kind: 'assist', by: ASSISTANT, text, why, item: it, createdAt: iso(minAgo) });
const feed: AssistantFeed = {
  unseen: 0,
  lines: [
    { answered: true, decision: d('d_rm', 4, 'Decided for you: Engineer asks to run `rm -rf /tmp/backfill-cache` → allow, once', 'The cache is rebuilt on the next run and the path is inside the chain\'s worktree; you allowed the same clean-up twice last week.', 'i_rm'),
      dissent: decisionRecord({ id: 'd_dis', chainId: 'busy', kind: 'dissent', by: actorOf('agent', 'c_so'), text: 'A second judge decides otherwise: Engineer asks to run `rm -rf /tmp/backfill-cache` → Refuse', why: 'The backfill reads that cache until it finishes, and the run is half done: removing it now restarts six hours of work.\n\nHow they differ: one lets the clean-up run, the other holds it until the backfill ends.', item: 'i_rm', createdAt: iso(2) }) },
    { answered: true, decision: d('d_push', 18, 'Decided for you: Release asks to push to origin/main → refuse: let the merge queue land it instead', 'You never let a chain push to main; the merge queue lands every approved change.', 'i_push'),
      overturn: decisionRecord({ id: 'd_ov', chainId: 'busy', kind: 'overturn', by: OWNER, text: 'Overturned', why: 'Allow it this once: the hotfix is already reviewed in #ops.', item: 'i_push', overturns: 'd_push', createdAt: iso(9) }) },
    { answered: false, decision: d('d_open', 2, 'Left to you: Engineer asks to delete from events on production', 'Your rule: ask you before anything writes to production.', 'i_open') },
  ],
};
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ inbox: items, assistantFeed: feed }),
  inboxAnswer: () => ({}),
  overturn: (decisionId, word) => {
    if (params.get('fail')) throw new Error('the server refused it');
    const l = feed.lines.find((x) => x.decision.id === decisionId);
    if (!l || l.overturn || !l.answered) throw new Error(`decision ${decisionId} is no answer the Assistant gave for you`);
    l.overturn = decisionRecord({ id: 'd_ov_' + decisionId, chainId: 'busy', kind: 'overturn', by: OWNER, text: 'Overturned', why: word.trim(), item: l.decision.item, overturns: decisionId, createdAt: new Date().toISOString() });
    fake.push('assistantFeedUpdated', structuredClone(feed));
    return l.overturn;
  },
});
const width = params.get('width') ?? '440';
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex flex-col gap-3', style: { width: `${width}px`, padding: '16px' } }, items.map((i) => h(InboxCallout, { key: i.id, item: i })))) })
  .use(createRouter({ history: createMemoryHistory(), routes }))
  .provide(apiKey, api)
  .mount('#app');
