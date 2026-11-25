// The Subscriptions tab as ADR-0064 step 7 surfaces it, on made-up data shaped as the server lays it out (orch/signalboard.go): a
// subscription held in place on a firing that failed for good (a long title and a long, unbroken error), one held because no processor
// does its action, a routine whose next run waits on its last one, a check-back whose delivery waits for a slot without failing, an
// event subscription with three near-misses (a false condition, one that could not be read, an overlap), one its owner paused and a
// routine that ended, beside a few ordinary lines; the board counts the two held. ?inbox=1 puts the held subscription's inbox item
// (orch/record.go raiseHeld) in a column beside the page, as the board's first column shows it: its link opens the line.
// No server, no sign-in: a fake server answers the calls of the page's real client core. Open /verify/holds.html; window.router is
// the router, and ?open=<id> opens a line (sub_notify, the held one; sub_pipeline, the near-misses; sub_triage, a screen that dropped two events; sub_cb_w1, the waiting one). An answer shows in the console as `asked: <method> <args>`.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter, RouterView } from 'vue-router';
import InboxCallout from '../src/components/InboxCallout.vue';
import TopBar from '../src/components/TopBar.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { Toaster } from '../src/components/ui/sonner';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { signalSourcesSeed } from './signalSourcesSeed';
import type { Chain, InboxItem, SignalBoard, SignalDetail, SignalLine, SubscriptionLine } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { chainRead, chainRecord, firingLine, inboxRecord, nearMissRecord, optionOf, readingLine, signalLine, subscriptionLine } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();
const chain = (id: string, title: string): Chain => chainRecord({ id, title, goal: 'g', status: 'waiting', chairId: `${id}-chair`, workdir: '/', createdAt: iso(-5 * 60 * MIN), updatedAt: iso(-MIN), rev: 1 });
// What lifts a hold, as the server words it after the hold (signalboard.go holdWords).
const lifts = 'it waits in place until it is retried (resume_subscription) or the firing is given up (skip_firing)';

const ERROR = `plugin acme.notify: POST https://hooks.slack.example.com/services/T024BE7LD/B08J1QWJ8E5/AbCdEfGhIjKlMnOpQrStUvWxYz0123456789abcdefghijklmnop?channel=release-train-and-deploy-announcements: 502 Bad Gateway
upstream connect error or disconnect/reset before headers. reset reason: connection termination
retried 5 times over 31m 12s (backoff 30s, 1m, 2m, 4m, 8m, 16m); last attempt at 11:02:47 answered in 30.0s (the processor's deadline)
<html><body><h1>502</h1> **not bold** <script>document.title = "pwned"</script></body></html>`;
const notify: SubscriptionLine = subscriptionLine({
  id: 'sub_notify', kind: 'event', title: 'Notify the release channel of every merged MR of the payments and checkout services, with the changelog and who approved it',
  source: 'event', key: 'event:gitlab', condition: "body.object_kind == 'merge_request' && body.object_attributes.state == 'merged'",
  what: 'Runs the plugin acme.notify with the merge request, its changelog and its approvals.', state: 'held', group: 'attention', control: '', lastRead: iso(-2 * MIN), lastFired: iso(-3 * 60 * MIN),
  held: `a firing failed for good (${ERROR}, on event:gitlab): ${lifts}`,
  hold: { reason: 'firing_failed', detail: ERROR, firing: { subscription: 'sub_notify', key: 'event:gitlab', rev: 41 }, since: iso(-37 * MIN) },
});
const noProcessor: SubscriptionLine = subscriptionLine({
  id: 'sub_score', kind: 'automation', title: 'Score each finished chain', source: 'fact', key: 'fact:chain.finished',
  what: 'Runs the plugin acme.scorer on each finished chain.', state: 'held', group: 'attention', control: '', lastRead: iso(-12 * MIN),
  held: `no processor does its action (plugin:acme.scorer is not enabled, on fact:chain.finished): ${lifts}`,
  hold: { reason: 'processor_missing', detail: 'plugin:acme.scorer is not enabled', firing: { subscription: 'sub_score', key: 'fact:chain.finished', rev: 7 }, since: iso(-3 * 60 * MIN) },
});
// A routine whose run is at work: its next firing is recorded and waits on it, and the line says so (signalboard.go heldWhy).
const lastRun = 'its last run is still at work: the next starts once it is done';
const weekly: SubscriptionLine = subscriptionLine({
  id: 'rt_weekly', kind: 'routine', title: 'Weekly report for the shops that signed up this month', chainId: 'r1', source: 'clock', at: iso(-3 * MIN),
  what: 'Write the weekly report of the new shops · cron 0 8 * * 1', state: 'held', group: 'attention', control: '', held: `waits: ${lastRun}`, waiting: lastRun,
});
// A check-back due whose delivery found every slot taken: it waits without failing, and is tried again on the next pass.
const deploy: SubscriptionLine = subscriptionLine({
  id: 'sub_cb_w1', kind: 'check_back', title: 'Deploy of MR !41 to staging', chainId: 'w1', source: 'clock', at: iso(-MIN),
  what: 'Check the health endpoint and the error rate for the last 10 minutes.', state: 'due', group: 'attention', control: '', waiting: 'all 3 slots are taken',
});
const pipeline: SubscriptionLine = subscriptionLine({
  id: 'sub_pipeline', kind: 'event', title: 'Tell the Chair of a failed pipeline', source: 'event', key: 'event:gitlab', condition: "body.object_attributes.status == 'failed'",
  what: 'Tells the Chair of the chain the pipeline is about, as evidence.', state: 'waiting', group: 'coming', control: '', lastRead: iso(-2 * MIN), lastFired: iso(-26 * 60 * MIN),
  nearMisses: [
    nearMissRecord({ subscriptionId: 'sub_pipeline', seq: 9120, key: 'event:gitlab', reason: 'condition_false', at: iso(-2 * MIN) }),
    nearMissRecord({ subscriptionId: 'sub_pipeline', seq: 9051, key: 'event:gitlab', reason: 'condition_error', detail: 'no such key: body.object_attributes.status (the event is a note: its object_attributes have noteable_type, note, author_id, project_id, commit_id and line_code, no status)', at: iso(-9 * MIN) }),
    nearMissRecord({ subscriptionId: 'sub_pipeline', seq: 8807, key: 'event:gitlab', reason: 'overlap', at: iso(-41 * MIN) }),
  ],
});
// ?longScreen=1: a screen at the most the server takes (600 characters), as one unbroken run of words with a long path in it.
const LONG_SCREEN = `${'The alert is one a person must fix and not a deploy that a retry already cured, or a flapping check that clears by itself within minutes, or a page about a service that is being drained. '.repeat(3)}See /var/lib/very/long/path/to/a/runbook/that/does/not/break/anywhere/at/all/${'x'.repeat(40)} for what a person must do.`.slice(0, 600);
// A start whose screen drops what needs no chain (ADR-0118): the line says when an event is worth one, and its near-misses say what
// the decider judged not worth one, in the same list as a false condition.
const triage: SubscriptionLine = subscriptionLine({
  id: 'sub_triage', kind: 'event', title: 'Start a chain for each failed deploy', source: 'event', key: 'event:grafana', condition: "body.state == 'alerting'",
  screen: params.get('longScreen') ? LONG_SCREEN : 'The alert is one a person must fix, not a deploy that a retry already cured.',
  what: 'Starts a chain from the alert: Find out why the deploy failed and fix it.', state: 'waiting', group: 'coming', control: '', lastRead: iso(-3 * MIN), lastFired: iso(-5 * 60 * MIN),
  nearMisses: [
    nearMissRecord({ subscriptionId: 'sub_triage', seq: 9210, key: 'event:grafana', reason: 'screened', detail: '96% sure it needs no chain', at: iso(-3 * MIN) }),
    nearMissRecord({ subscriptionId: 'sub_triage', seq: 9188, key: 'event:grafana', reason: 'condition_false', at: iso(-14 * MIN) }),
    nearMissRecord({ subscriptionId: 'sub_triage', seq: 9102, key: 'event:grafana', reason: 'screened', detail: '91% sure it needs no chain', at: iso(-52 * MIN) }),
  ],
});
const comments: SubscriptionLine = subscriptionLine({
  id: 'sub_comments', kind: 'event', title: 'Log every comment', source: 'event', key: 'event:gitlab', condition: "body.object_kind == 'note'",
  what: 'Writes each comment down.', state: 'paused', group: 'idle', control: 'paused', lastRead: iso(-2 * MIN),
});
const launch: SubscriptionLine = subscriptionLine({
  id: 'rt_launch', kind: 'routine', title: 'Close the launch checklist', source: 'clock', what: 'Tick the launch checklist off once · once', state: 'over', group: 'idle', control: 'ended', lastFired: iso(-3 * 24 * 60 * MIN),
});
const audit: SubscriptionLine = subscriptionLine({ id: 'rt_audit', kind: 'routine', title: 'Dependency audit of the API', source: 'clock', at: iso(3 * 60 * MIN), what: 'Audit the Go dependencies · cron 0 9 * * 1-5', state: 'waiting', group: 'coming', control: '' });
const subscriptions = [notify, noProcessor, weekly, deploy, audit, pipeline, triage, comments, launch];

const signals: SignalLine[] = [
  signalLine({ id: 'signal:event:gitlab', key: 'event:gitlab', source: 'event', title: 'Events of gitlab', rev: 44, at: iso(-2 * MIN), cause: 'gitlab (door)', subscribers: 3, eventSignalId: 'wh_1' }),
  signalLine({ id: 'signal:fact:chain.finished', key: 'fact:chain.finished', source: 'fact', title: 'A chain finished', rev: 7, at: iso(-12 * MIN), cause: 'Remit', subscribers: 1 }),
];
const board: SignalBoard = { sources: signalSourcesSeed(signals, subscriptions), subscriptions, signals, signalsMore: 0, held: 2, rev: 1 };

const heldItem = inboxRecord({
  id: 'i_held', home: '', cardId: '', kind: 'held', subscription: 'sub_notify', status: 'open', createdAt: iso(-37 * MIN), rev: 1,
  title: 'Held: Notify the release channel of every merged MR of the payments and checkout services, with the changelog and who approved it',
  detail: `Subscription sub_notify is held: a firing failed for good (plugin acme.notify: POST https://hooks.slack.example.com/services/T024BE7LD/B08J1QWJ8E5/AbCdEfGhIjKlMnOpQrStUvWxYz0123456789: 502 Bad Gateway, on event:gitlab): ${lifts}.\n\nRetry tries the firing again once what failed is fixed, then the ones after it; skip gives the firing up and goes on with the readings after it. Say why: it is recorded. describe_subscription and explain_subscription say where it stands.`,
  options: [optionOf('retry', 'Retry'), optionOf('skip', 'Skip')],
});

const signalGet = async (address: string): Promise<SignalDetail> => {
  console.log('asked: signalGet', JSON.stringify(address));
  const line = subscriptions.find((l) => l.id === address);
  return {
    title: line?.title ?? address,
    key: line?.key ?? '',
    readings: line?.key ? [readingLine({ rev: 44, at: iso(-2 * MIN), cause: 'gitlab (door)', value: '{"object_kind":"note","object_attributes":{"note":"LGTM"}}', truncated: false })] : [],
    firings: address === 'sub_notify' ? [firingLine({ subscriptionId: 'sub_notify', at: iso(-37 * MIN), title: notify.title, key: 'event:gitlab', action: 'run', outcome: `failed after 5 attempts: ${ERROR.split('\n')[0]}`, expired: false })] : [],
  };
};
const chains = [chain('w1', 'Deploy of MR !41 to staging'), chain('r1', 'Weekly report · 02/11 08:00')];
const asked = (name: string, ...args: unknown[]) => console.log('asked:', name, JSON.stringify(args));
fakeServer({
  appSnapshot: () => snapshotOf({ chains: chains.map((c) => chainRead(c)), inbox: [heldItem], signals: board }),
  signalGet,
  inboxAnswer: (...args) => (asked('inboxAnswer', ...args), {}),
  chainCheckBackNow: (...args) => (asked('chainCheckBackNow', ...args), {}),
  chainPause: (...args) => (asked('chainPause', ...args), {}),
  routineRun: (...args) => (asked('routineRun', ...args), chains[1]),
  routinePause: (...args) => (asked('routinePause', ...args), {}),
  routineResume: (...args) => (asked('routineResume', ...args), {}),
});

const router = createRouter({ history: createMemoryHistory(), routes });
Object.assign(window, { router });
const opened = params.get('open');
await router.push(opened ? { name: 'subscriptions', params: { entry: opened } } : { name: 'subscriptions' });
const inbox = !!params.get('inbox');
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () =>
      h('div', { class: 'flex h-screen flex-col' }, [
        h(TopBar, { toReview: 1 }),
        h('div', { class: 'flex min-h-0 flex-1' }, [
          ...(inbox ? [h('aside', { class: 'w-[400px] flex-none overflow-auto border-r border-line bg-bg2 p-3', 'aria-label': 'Waits for you', 'data-inbox': '' }, [h(InboxCallout, { item: heldItem })])] : []),
          h('div', { class: 'flex min-w-0 flex-1 flex-col' }, [h(RouterView)]),
        ]),
        h(Toaster),
      ]),
    ),
})
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
