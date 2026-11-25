// Events for the verify pages, as the server would hold them (made up): each is what arrived (its `event`, which only its page
// reads) and the firings of the subscriptions that held for it, in the server's words. A busy list: events no subscription held
// for, ones that told a Chair, started a chain, ran a job, skipped its own comment, one with a very long signal name and a long
// body cut, and events that came by every way (a door, the CLI, a sensor reading GitHub issues and Datadog monitors, an agent). And
// the event signals, the sensors that fill them and the keys the server would give him.
import type { Actor, ActionKind, Chain, ChainStanding, EventRecord, EventSignal, FiringLine, InboundEvent, Sensor } from '../src/core/generated/types.generated';
import { stands } from './standingSeed';
import { actorOf, chainRecord, eventSignalRecord, firingLine, inboundEvent, sensorRecord } from './records';

const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
export const MR_URL = 'https://gitlab.example.com/acme/billing/-/merge_requests/41';

const gitlabBody = (n: number) =>
  JSON.stringify({ object_kind: 'pipeline', object_attributes: { id: 9100 + n, ref: 'main', status: 'failed', source: 'push', stages: ['build', 'test', 'deploy'], duration: 412 }, project: { id: 41, path_with_namespace: 'acme/billing' }, user: { name: 'Alex Pham' }, builds: [{ id: 1, stage: 'test', name: 'go test ./...', status: 'failed', failure_reason: 'script_failure' }] });
// The other GitLab events the door settled, as GitLab posts them: a merge request merged or closed, a comment on one, a pipeline that passed.
const mergeBody = (action: string) => JSON.stringify({ object_kind: 'merge_request', event_type: 'merge_request', user: { name: 'Alex Pham', username: 'alexp' }, project: { id: 41, path_with_namespace: 'acme/billing' }, object_attributes: { iid: 41, action, state: action === 'merge' ? 'merged' : 'closed', title: 'Retry the billing sync when Shopify answers 429', url: MR_URL, source_branch: 'fix/billing-retry-429', target_branch: 'main' } });
const noteBody = JSON.stringify({ object_kind: 'note', event_type: 'note', user: { name: 'Alice Tran', username: 'alice' }, project: { id: 41, path_with_namespace: 'acme/billing' }, merge_request: { iid: 41, url: MR_URL, source_branch: 'fix/billing-retry-429' }, object_attributes: { noteable_type: 'MergeRequest', system: false, url: `${MR_URL}#note_8812`, note: 'The retry looks right, but the backoff starts at 30s: Shopify asks for 2s after a 429. Can you read Retry-After instead of the constant?\n\nAlso the test only covers one retry.' } });
const passedBody = JSON.stringify({ object_kind: 'pipeline', object_attributes: { id: 31, ref: 'main', status: 'success', duration: 388 }, project: { id: 41, path_with_namespace: 'acme/billing' }, user: { name: 'Alex Pham' } });
const sentryBody = JSON.stringify({ action: 'created', data: { issue: { id: '5521', title: 'TypeError: cannot read properties of undefined (reading "shopId")', culprit: 'save-ad-insights', level: 'error', count: 214 } } });
const HEADERS: Record<string, string> = {
  'Content-Type': 'application/json',
  'User-Agent': 'GitLab/17.4.1-ee',
  'X-Gitlab-Event': 'Pipeline Hook',
  'X-Gitlab-Event-UUID': '7b1f4c2e-8f0a-4d79-a2f3-6a3b1c9d5e10',
  'X-Gitlab-Instance': 'https://gitlab.example.com',
  'X-Forwarded-For': '34.74.226.14',
};

// One event as the server holds it: its reading (the address is `event:<signal>#<n>`) with what arrived. The list carries it
// without `event` (listed); its page reads it whole (eventGet).
const TITLES: Record<ActionKind, string> = { tell: 'Tell the Chair', start: 'Start a chain', run: 'Run a job', wake: 'Wake', hire: 'Hire', end: 'End', deadline: 'A mentor\'s deadline', assist: 'Put to the Assistant', workflow: 'Resume a workflow step' };
export const firingOf = (action: ActionKind, outcome: string, at: number, subscriptionId: string, key = 'event:gitlab'): FiringLine => (firingLine({ subscriptionId, at: iso(at), title: TITLES[action], key, action, outcome, expired: false }));
export const eventSeeded = (n: number, signal: string, ageMin: number, firings: Omit<FiringLine, 'at'>[] = [], came: Partial<InboundEvent> = {}): EventRecord => {
  const at = Date.now() - ageMin * MIN;
  return {
    id: `event:${signal}#${n}`, signal, rev: n, at: iso(at), via: came.via ?? 'door',
    event: inboundEvent({ via: 'door', header: HEADERS, body: signal === 'sentry' ? sentryBody : gitlabBody(n), ...came }),
    firings: firings.map((f) => ({ ...f, at: iso(at + 2_000) })),
  };
};
const fire = (action: ActionKind, outcome: string, key = 'event:gitlab', title = TITLES[action]): Omit<FiringLine, 'at'> => ({ subscriptionId: `sub_${action}_${key.slice(6)}`, title, key, chainId: '', cardId: '', action, outcome, expired: false });
// An event as a list carries it: no `event`, which is only on its page.
export const listed = ({ event: _arrived, ...record }: EventRecord): EventRecord => record;

// What the poller writes of a GitHub issue and of a Datadog monitor (bundled/github_poll.go githubPollBody, source/datadog.go): JSON
// Remit made, with no header, since nothing was posted.
const issueBody = (n: number, title: string, body: string) =>
  JSON.stringify({ repo: 'acme/shop', number: n, title, state: 'open', labels: ['bug', 'customer'], user: 'nhung-cs', author_association: 'MEMBER', html_url: `https://github.com/acme/shop/issues/${n}`, created_at: '2026-10-31T23:12:04Z', updated_at: '2026-11-01T00:40:51Z', body });
const monitorBody = JSON.stringify({ id: 41822, name: 'checkout-api 5xx rate above 2%', status: 'Alert', priority: 2, tags: ['service:checkout-api', 'env:production'], query: 'sum(last_5m):sum:trace.http.request.errors{service:checkout-api}.as_rate() > 0.02', overall_state_modified: '2026-11-01T00:38:00Z', url: 'https://app.datadoghq.com/monitors/41822' });
// What a deploy script sends with `remit emit`: the CLI says it is the CLI, and sets the type of what it posts.
const CLI_HEADERS: Record<string, string> = { 'Content-Type': 'application/json', 'User-Agent': 'remit-emit', 'X-Remit-Via': 'cli' };
const deployBody = JSON.stringify({ status: 'success', environment: 'production', ref: 'fix/billing-retry-429', merge_request_url: MR_URL, version: 'v2026.11.01-3', took_seconds: 212 });

// The chains the events reached: what the list links to.
export const hookChains: Chain[] = [
  ['ch_deploy', 'Deploy of MR !41 to staging'],
  ['ch_billing', 'Fix the failing pipeline of acme/billing'],
  ['ch_sentry', 'Sentry: TypeError reading "shopId" in save-ad-insights'],
  ['ch_docs', 'Update the event signal guide for subscriptions'],
  ['ch_lint', 'Lint failures in acme/billing after the Go 1.26 bump: three packages fail golangci-lint on the same shadowed error, and the CI image has to be rebuilt before the fix can be checked'],
  ['ch_cache', 'Clear the stale ad-insights cache in staging'],
].map(([id, title]) => chainRecord({ id, title, goal: `${title}. Done means the cause is found and fixed in an MR the owner can review, with the evidence quoted in its description.`, status: 'running', chairId: `${id}-chair`, workdir: '/', origin: 'event', createdAt: iso(Date.now() - 90 * MIN), updatedAt: iso(Date.now() - 5 * MIN) }));

// Where those chains stand, as the server says it: a running one, one whose change waits in its merge queue, a finished one, one that checks back.
export const hookStandings: ChainStanding[] = [
  stands('ch_deploy', 'Running', 'running', { working: true, why: 'Waiting for the staging deploy to report' }),
  stands('ch_billing', 'Waiting in the merge queue of acme/billing', 'waiting'),
  stands('ch_sentry', 'Done', 'done', {}),
  stands('ch_docs', 'Checks back at 15:30', 'waiting', { why: 'Waiting for the reviewer to answer' }),
  stands('ch_lint', 'Running', 'running', { working: true }),
  stands('ch_cache', 'Done', 'done', { why: 'The cache was cleared and the dashboards load again' }),
];

// Newest first, the way the server lists them. Numbers are the events' revs; `eventOf(n)` finds one.
export function eventsSeed(rows = 64): EventRecord[] {
  const hand: EventRecord[] = [
    eventSeeded(1, 'gitlab', 0.5),
    eventSeeded(2, 'sentry', 1.5),
    eventSeeded(3, 'gitlab-merge-requests-of-the-billing-group-with-a-very-long-signal-name-that-keeps-going', 2, [], { query: 'source=group-hooks&attempt=2', bodyCut: true, body: gitlabBody(3).repeat(40) }),
    eventSeeded(4, 'jira', 3, [], { header: {}, body: '' }),
    eventSeeded(5, 'gitlab', 6, [fire('tell', 'told the Chair of ch_billing: pipeline 9105 failed on main')]),
    eventSeeded(6, 'sentry', 14, [fire('start', 'started the chain ch_sentry: Sentry: TypeError reading "shopId" in save-ad-insights', 'event:sentry')]),
    // One event, four subscriptions that held: two told a Chair, one recorded the forge's answer and a plugin's processor
    // (`plugin:<id>.<name>`, ADR-0049) posted to a channel.
    eventSeeded(7, 'gitlab', 25, [fire('tell', 'told the Chair of ch_deploy: alexp closed its merge request'), fire('tell', 'told the Chair of ch_billing: alexp closed its merge request'), fire('run', 'recorded MR !41 in billing: closed', 'event:gitlab', 'Follow the merge request'), fire('plugin:acme.notify', 'posted to #release-train: alexp closed MR !41', 'event:gitlab', 'Notify the release channel')], { header: { ...HEADERS, 'X-Gitlab-Event': 'Merge Request Hook' }, body: mergeBody('close') }),
    eventSeeded(8, 'pagerduty', 41),
    eventSeeded(9, 'gitlab', 70, [{ ...fire('start', 'the subscription started a chain a short while ago and waits for its cooldown (30 minutes) before the next'), expired: false }]),
    // Told nobody: the comment is Remit's own, and a chain the catalog lost.
    eventSeeded(10, 'gitlab-billing', 110, [fire('tell', 'the account Remit works as wrote it, so nobody was told', 'event:gitlab-billing')], { header: { ...HEADERS, 'X-Gitlab-Event': 'Note Hook' }, body: noteBody }),
    eventSeeded(11, 'gitlab-billing', 130, [fire('tell', 'no chain carries its merge request or branch, so nobody was told', 'event:gitlab-billing')], { body: passedBody }),
    // A merge: its Chair told and the change recorded in the same event, an outcome that runs long.
    eventSeeded(12, 'gitlab-billing', 150, [
      fire('tell', 'told the Chair of ch_billing: alexp merged its merge request', 'event:gitlab-billing'),
      fire('start', 'started the chain ch_docs: MR !52 merged into main with six follow-ups in its description (docs, lint, cache, billing, the Sentry alert and the staging deploy); the next waits for the start quota of 20 a day, of which 19 are used', 'event:gitlab-billing', 'Start the follow-ups'),
    ], { header: { ...HEADERS, 'X-Gitlab-Event': 'Merge Request Hook' }, body: mergeBody('merge') }),
    // Read by the sensor of GitHub issues (no header: nothing was posted), and one of a Datadog monitor that alerts.
    eventSeeded(21, 'shop-issues', 8, [fire('start', 'started the chain ch_cache: Clear the stale ad-insights cache in staging', 'event:shop-issues')], { via: 'poll', header: undefined, body: issueBody(318, 'Dashboard shows yesterday\'s ad spend for every store', 'Since this morning every store I open shows the ad spend of yesterday, even after a refresh.\n\nSteps: open Ads → Overview.') }),
    eventSeeded(22, 'checkout-alerts', 12, [], { via: 'poll', header: undefined, body: monitorBody }),
    // Sent by the deploy script with `remit emit`: the shipped job marks the change live.
    eventSeeded(23, 'deploy-bot', 18, [fire('run', 'marked the change of ch_deploy live: deployed fix/billing-retry-429 to production', 'event:deploy-bot', 'Mark the change live')], { via: 'cli', header: CLI_HEADERS, body: deployBody }),
    // Written by a Chair with emit_event: words, not JSON.
    eventSeeded(24, 'deploy-bot', 33, [], { via: 'agent', header: undefined, body: 'The CI image of acme/billing is rebuilt with Go 1.26.1; golangci-lint passes on main.' }),
  ];
  // Many short events of a busy signal, from the other signals now and then.
  const signals = ['gitlab', 'gitlab', 'sentry', 'jira', 'pagerduty', 'shop-issues', 'gitlab'];
  const filler: EventRecord[] = Array.from({ length: Math.max(0, rows - hand.length) }, (_, i) => {
    const signal = signals[i % signals.length];
    return eventSeeded(100 + i, signal, 120 + i * 3, i % 3 === 0 ? [fire('tell', 'told the Chair of ch_billing: what changed on its merge request', `event:${signal}`)] : [],
      signal === 'shop-issues' ? { via: 'poll', header: undefined, body: issueBody(200 + i, 'Export to CSV is slow', 'It takes a minute.') } : { body: i % 2 ? passedBody : gitlabBody(100 + i) });
  });
  return [...hand, ...filler].slice(0, Math.max(rows, 0));
}
export const eventOf = (n: number, from: readonly EventRecord[] = eventsSeed(24)): EventRecord => from.find((e) => e.rev === n)!;

const HOUR = 60 * MIN;
const ASSISTANT: Actor = actorOf('assistant', 'c_assistant');
const URL_SAFE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
// An event signal's key as the server makes them: `evk_` and 43 URL-safe characters. Made up from the signal's id, so the same one every time.
export function eventSignalKeyOf(id: string): string {
  let h = 2166136261;
  for (const c of id) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  let key = 'evk_';
  for (let i = 0; i < 43; i++) {
    h = (Math.imul(h ^ (h >>> 15), 2246822519) + i) | 0;
    key += URL_SAFE[(h >>> 7) & 63];
  }
  return key;
}

const LONG_NOTE = 'Merge requests, their comments and the pipelines of every project in the billing group, for the Chairs that watch the invoices service; nothing from the other groups, and nothing from forks.';
// The first `count` (at most 9) of the event signals, newest made first, as the server lists them: one with a note, one with a very
// long name and note, a paused one that he paused himself, older ones with none, one a sensor fills with GitHub issues, one a
// sensor fills with Datadog monitors and has gone silent, the deploy script's that a CLI writes, and the builtin gitlab.
export function eventSignalsSeed(count = 3): EventSignal[] {
  const signal = (id: string, name: string, madeAgo: number, extra: Partial<EventSignal> = {}): EventSignal => eventSignalRecord({
    id, name, by: ASSISTANT, createdAt: iso(Date.now() - madeAgo), updatedAt: iso(Date.now() - madeAgo), rev: 1, ...extra,
  });
  const made: EventSignal[] = [
    signal('es_g1', 'gitlab-billing', 14 * MIN, { note: 'Merge requests, comments and pipelines of acme/billing' }),
    signal('es_s2', 'gitlab-merge-requests-of-the-billing-group-with-a-very-long-signal-name-that-keeps-going', 3 * HOUR, {
      note: LONG_NOTE,
    }),
    signal('es_j3', 'jira', 5 * 24 * HOUR, { note: 'Tickets of the BILL project', paused: true, rev: 3, updatedAt: iso(Date.now() - 2 * 24 * HOUR), by: actorOf('owner', 'owner@example.com', 'web') }),
    signal('es_i4', 'shop-issues', 6 * 24 * HOUR, { note: 'Issues customers open on acme/shop', kept: true, rev: 2, by: actorOf('owner', 'owner@example.com', 'web') }),
    signal('es_m5', 'checkout-alerts', 8 * 24 * HOUR, { note: 'Datadog monitors of checkout-api in production' }),
    signal('es_p6', 'pagerduty', 9 * 24 * HOUR),
    signal('es_a7', 'grafana-alerts', 12 * 24 * HOUR, { note: 'Alerts of the staging cluster' }),
    signal('es_d8', 'deploy-bot', 30 * 24 * HOUR, { note: 'Our own deploy script, through remit emit' }),
    // Ships with Remit: made by it, not by the Assistant, and not the owner's to edit (open it with ?signals=9).
    signal('es_b9', 'gitlab', 40 * 24 * HOUR, { builtin: true, by: actorOf('remit', 'seed'), note: 'GitLab: tells the Chair of a chain what happens on its merge request. Part of Remit.' }),
  ];
  return made.slice(0, Math.min(Math.max(count, 0), made.length));
}

// What the server says of a signal whose sensor has failed its last reads (signalboard.go), by the signal's name.
export const SILENT_SENSORS: Record<string, string> = {
  'checkout-alerts': 'its sensor has failed 3 reads in a row (last error: datadog: 403 Forbidden: the API key of connection prod was revoked)',
  pagerduty: 'the plugin gone is not enabled, so its sensor kind watcher is not sensed',
};

// A script the owner's sensor of Grafana alerts runs: long enough that the folded script has to scroll, with a line wider than the card.
const GRAFANA_SCRIPT = [
  '#!/usr/bin/env python3',
  '# Reads the firing alerts of the staging cluster and prints one JSON object per alert, newest first.',
  'import json, os, sys, urllib.request',
  '',
  'BASE = os.environ["GRAFANA_URL"].rstrip("/")',
  'TOKEN = os.environ["GRAFANA_TOKEN"]',
  'req = urllib.request.Request(BASE + "/api/prometheus/grafana/api/v1/rules?state=firing&group_limit=50&rule_limit=200&matcher=cluster%3Dstaging-eu-west-1", headers={"Authorization": "Bearer " + TOKEN})',
  'with urllib.request.urlopen(req, timeout=20) as res:',
  '    groups = json.load(res)["data"]["groups"]',
  'for group in groups:',
  '    for rule in group["rules"]:',
  '        for alert in rule.get("alerts", []):',
  '            print(json.dumps({"id": alert["labels"].get("alertname", rule["name"]) + "/" + alert["labels"].get("instance", ""), "state": alert["state"], "since": alert["activeAt"], "summary": alert["annotations"].get("summary", ""), "labels": alert["labels"]}))',
].join('\n');

// The sensors, newest made first, as the server stores them (ADR-0047, ADR-0049): shop-issues is read from GitHub, checkout-alerts from Datadog by
// two sensors (the second paused, which is why it reads nothing), grafana-alerts by a script, and the staging deploy of chain k5 by the
// script its Chair made.
export function sensorsSeed(): Sensor[] {
  const sensor = (id: string, name: string, madeAgo: number, signal: string, extra: Partial<Sensor>): Sensor => sensorRecord({
    id, kind: 'poll', name, signal, source: 'github', connection: 'default', everyMinutes: 5, by: ASSISTANT, createdAt: iso(Date.now() - madeAgo), updatedAt: iso(Date.now() - madeAgo), rev: 1, ...extra,
  });
  return [
    sensor('sn_k5', 'deploy', 110 * MIN, 'chain:k5.sensor.deploy', { chain: 'k5', source: 'script', note: 'The state of the staging deploy of MR !41', script: { language: 'python3', code: 'import json, urllib.request\nprint(json.dumps({"state": "success"}))' } }),
    sensor('sn_i4', 'shop-issues', 6 * 24 * HOUR, 'event:shop-issues', { source: 'github', connection: 'acme', note: 'Issues customers open on acme/shop', query: 'acme/shop label:customer state:open', everyMinutes: 5, rev: 2, by: actorOf('owner', 'owner@example.com', 'web') }),
    sensor('sn_m5b', 'checkout-alerts-staging', 8 * 24 * HOUR, 'event:checkout-alerts', { source: 'datadog', connection: 'staging', query: 'tag:service:checkout-api status:alert', everyMinutes: 10, paused: true, by: actorOf('owner', 'owner@example.com', 'web'), updatedAt: iso(Date.now() - 2 * HOUR), rev: 2 }),
    sensor('sn_m5a', 'checkout-alerts', 8 * 24 * HOUR, 'event:checkout-alerts', { source: 'datadog', connection: 'prod', note: 'Datadog monitors of checkout-api in production', query: 'tag:service:checkout-api status:alert', everyMinutes: 1 }),
    // One signal with two sensors of a plugin's kind: one sensed with the settings it was given, one whose plugin is not enabled, so nothing senses it.
    sensor('sn_p6b', 'watcher', 9 * 24 * HOUR, 'event:pagerduty', { kind: 'plugin:gone.watcher', source: undefined, connection: undefined, everyMinutes: undefined, note: 'Left by a plugin that was switched off' }),
    sensor('sn_p6a', 'incidents', 9 * 24 * HOUR, 'event:pagerduty', { kind: 'plugin:pagerduty.incidents', source: undefined, connection: undefined, everyMinutes: undefined, note: 'Open incidents of the checkout service', settings: { service: 'checkout', urgency: 'high', window: 30, tags: ['prod', 'eu'] } }),
    sensor('sn_a7', 'grafana-alerts', 12 * 24 * HOUR, 'event:grafana-alerts', { source: 'script', note: 'Firing alerts of the staging cluster', query: GRAFANA_SCRIPT, everyMinutes: 10 }),
  ];
}
