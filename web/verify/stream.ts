// Two cards' streams (StreamBlocks) on made-up data, side by side. No server, no sign-in. Open /verify/stream.html.
// The Chair around a landing: it calls land and ends its turn, the server's checks show as a job (running, then done or
// failed), and the result starts the next turn; ?job=running|completed|failed picks how the last job stands
// (default completed).
// An engineer: the Chair's message read in the middle of the turn, the owner's own message beside it, permission prompts
// asked and decided by each who decides one (the Chair, before and after the time limit, the owner, a rule, and the policy of an
// old event), an error and who interrupted a turn. The Chair's own prompt, with no Chair above it, is decided by the session
// itself past the limit.
// An engineer Lan helps as its mentor: the Chair's brief, Remit telling it who its mentor is, Lan's messages (headed with
// her mark and name, without the header line the agent reads), a tool call, a permission prompt the Chair decides, and the
// Chair's message.
// A deployer: a Monitor and a background command it starts and ends its turn on, one task a pause stopped, and the turns
// it took on their news, one in which it says nothing.
// A reviewer that has said its next word after a long run of steps: the run is one bar, the steps after it stay open.
// A writer and a thinker, each still writing: a message of paragraphs, a list and inline code, and a thought broken over
// lines, each ending in the caret.
// ?w=<px> sets the width of each column (420 is the drawer).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { apiKey } from '../src/core/api';
import type { EventData as TaskEventData, JobStatus, TaskOutput, ToolStatus } from '../src/core/generated/types.generated';
import StreamBlocks from '../src/components/StreamBlocks.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { blocks, dataOf } from '../src/stream';
import { fakeServer, snapshotOf } from './fake';
import type { Actor, Card, EventKind, EventRead } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { agentStands, stands } from './standingSeed';
import { asked, decided, sent, tool, turnEnd } from './activitySeed';
import { actorOf, cardRecord, chainRead, chainRecord, helpRecord, mentorRecord, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const width = params.get('w') ?? '520';
const last = (params.get('job') ?? 'completed') as JobStatus;
const MIN = 60_000;
const t0 = Date.now() - 12 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const card = (id: string, agent: string, name: string, origin: Card['origin'], extra: Partial<Card> = {}): Card =>
  cardRecord({ id, home: 'ch', agent, name, origin, model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: iso(t0), updatedAt: iso(t0), rev: 1, ...extra });
const cards = [
  card('chair', 'chair', 'Chair', 'chair', { status: last === 'running' ? 'running' : 'idle' }),
  card('engineer', 'engineer', 'Engineer · adsfns rate limits', 'hired'),
  card('deployer', 'deployer', 'Deployer · watch the rollout', 'hired', { status: 'idle' }),
  card('writer', 'engineer', 'Engineer · resubmit', 'hired'),
  card('thinker', 'engineer', 'Engineer · thinking', 'hired'),
  card('reviewer', 'qc', 'QC · race run', 'hired', { status: 'idle' }),
  card('helped', 'engineer', 'Engineer · flaky throttle test', 'hired'),
];

const chair: Actor = actorOf('agent', 'chair');
const owner: Actor = actorOf('owner', 'alexp');
const lan: Actor = actorOf('mentor', 'help_lan');
let n = 0;
const events: EventRead[] = [];

// The Chair's landing, timed in seconds from t0.
const chairEv = <K extends EventKind>(kind: K, secs: number, extra: EventSeed<K> = {}, endSecs = secs) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId: 'chair', createdAt: iso(t0 + secs * 1000), updatedAt: iso(t0 + endSecs * 1000) }, kind, extra));
const checking = 'Checking · compass/sales-engine';
const up = 'Version 2 is recorded as landing ln_68: it enters the merge queue of compass/sales-engine → develop once every gate approves it.';
chairEv('prompt', 0, { title: 'Read the report from QC', text: '[report from QC · card x · stop end_turn] — accept it', by: actorOf('agent', 'reviewer'), status: 'delivered' });
chairEv('tool', 4, { ...tool('remit', 'land', 'Remit'), status: 'completed', title: 'mcp__remit__land' });
chairEv('job', 13, { title: checking, status: 'completed', text: up }, 103);
chairEv('message', 50, { text: 'Both approved HEAD `6e60b4a`. I landed it on `develop`.' });
chairEv('stop', 51, { ...turnEnd(), status: 'end_turn' });
chairEv('prompt', 103, { title: 'Land result', text: '[land result · compass/sales-engine]\n' + up, by: actorOf('remit'), status: 'delivered' });
chairEv('stop', 112, { ...turnEnd(), status: 'end_turn' });
chairEv('prompt', 120, { title: 'Engineer · adsfns rate limits asks permission: kubectl delete pod stuck-1', text: '[permission request from Engineer · adsfns rate limits · card engineer · request p_01895130702f]\nEngineer · adsfns rate limits wants to: kubectl delete pod stuck-1\n\nIt is blocked until you answer.', by: actorOf('remit'), status: 'steered' });
chairEv('tool', 122, { ...tool('remit', 'answer_permission', 'Remit'), status: 'completed', title: 'mcp__remit__answer_permission' });
chairEv('tool', 123, { ...tool('read', 'read', 'throttle.go'), status: 'completed', title: 'Read throttle.go' });
chairEv('tool', 124, { ...sent('engineer', 'feedback', 'stress test: assert state, not log text', 'Reviewer requested changes on 753cf98 (MEDIUM). With Redis down and the fail-open log text reworded, the test passes.\n\nFix:\n1. Assert that the wait honours the throttle.\n2. Drop the log-text assertion.'), status: 'completed', title: 'mcp__remit__route', data: { input: { from_card: 'reviewer', to_card: 'engineer', kind: 'feedback', label: 'stress test: assert state, not log text', message: 'Reviewer requested changes on 753cf98 (MEDIUM). With Redis down and the fail-open log text reworded, the test passes.\n\nFix:\n1. Assert that the wait honours the throttle.\n2. Drop the log-text assertion.' } } });
chairEv('tool', 125, { ...tool('read', 'read', 'stress_test.go'), status: 'completed', title: 'Read stress_test.go' });
chairEv('tool', 126, { ...sent('reviewer', 'handoff', 'new version: 753cf98 (stress test fix) with a label long enough to need cutting at the edge of the column', 'New HEAD 753cf98 fixes the flaky test.'), status: 'completed', title: 'mcp__remit__route', data: { input: { from_card: 'engineer', to_card: 'reviewer', kind: 'handoff', label: 'new version: 753cf98 (stress test fix) with a label long enough to need cutting at the edge of the column', message: 'New HEAD 753cf98 fixes the flaky test.' } } });
chairEv('prompt', 130, { title: 'Your message', text: 'Also cover the refund case, then submit again.', by: actorOf('owner', 'owner@example.com', 'web'), status: 'delivered' });
const chairReads = events[events.length - 1].event.id;
chairEv('tool', 133, { ...tool('remit', 'land', 'Remit'), status: 'completed', title: 'mcp__remit__land' });
// The Chair's own prompt has no Chair above it: it goes to the owner, and past the limit the session decides it itself.
chairEv('system', 134, { ...asked('p_6', 'gh run view 4242 --log', 'owner'), status: 'permission_asked', title: 'gh run view 4242 --log', data: { request: 'p_6', step: 'owner' } });
chairEv('system', 136, { ...decided('gh run view 4242 --log', true, 'p_6'), status: 'permission_answered', by: chair, title: 'gh run view 4242 --log', text: 'nobody decided in 30 minutes: the session decided its own request', data: { request: 'p_6', allow: true } });
chairEv('job', 140, { title: checking, status: last, text: last === 'failed' ? 'Failed: go test ./... failed; fix it and submit again:' : last === 'running' ? '' : up }, last === 'running' ? 140 : 185);

// The engineer's turn, 20 s apart.
let at = t0;
const ev = <K extends EventKind>(kind: K, extra: EventSeed<K> = {}) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId: 'engineer', createdAt: iso((at += 20_000)), updatedAt: iso(at) }, kind, extra));
// A tool call the engineer made, with what the server types it as.
const call = (title: string, typed: Pick<EventRead, 'activity'>, status: ToolStatus = 'completed') => ev('tool', { title, ...typed, status, toolCallId: 'tc' + n });

ev('prompt', { title: 'Fix the retry loop', text: '[handoff from the Chair]\nStop the ad-details sync hammering Meta: back off on every rate-limit code.', by: chair, status: 'delivered' });
ev('thought', { text: 'The handler drops code 17 as an OAuth error, so the producer calls again.' });
call('Read handler.go', tool('read', 'read', 'handler.go'));
call('Read facebook.go', tool('read', 'read', 'facebook.go'));
ev('message', { text: 'Reading how the handler treats a Graph error before I change it.' });
ev('prompt', { title: 'Your message', text: 'Also check whether the cron producer loops.', by: actorOf('owner', 'alexp'), status: 'delivered' });
const engineerReads = events[events.length - 1].event.id; // the steered answer after it opens no turn: the engineer is still on this one
call('go test ./...', tool('execute', 'go', 'test ./...'));
ev('tool', { title: 'mcp__remit__ask_chair', ...sent(undefined, 'question', 'Where does the per-account cool-off live: the producer or import-async-ad-details?', 'Where does the per-account cool-off live: the producer or import-async-ad-details?'), status: 'completed', data: { input: { question: 'Where does the per-account cool-off live: the producer or import-async-ad-details?' } } });
ev('prompt', { title: 'The Chair\'s answer', text: '[answer from the Chair · to your question · cool-off]\nApproved: put the per-account cool-off in import-async-ad-details, not in the producer. Log the skip at Warn.', by: chair, status: 'steered' });
ev('message', { text: 'The Chair approved the cool-off in the consumer; moving it there and logging the skip at Warn.' });
// A Codex-era call stored its input and no result; a Claude Code call keeps both; a running one has its input only.
ev('tool', { title: 'mcp.remit.list_agents', ...tool('execute', 'mcp.remit.list_agents', 'mcp.remit.list_agents'), status: 'completed', toolCallId: 'legacy', data: { input: { server: 'remit', tool: 'list_agents', arguments: {} } } });
ev('tool', { title: 'mcp__remit__list_agents', ...tool('remit', 'list_agents', 'Remit'), status: 'completed', toolCallId: 'result', data: { input: {}, output: '[\n  { "id": "c_0a03227d4e31", "status": "stopped" }\n]' } });
ev('tool', { title: 'mcp__remit__subscribe', ...tool('remit', 'subscribe', 'Remit', 'running'), status: 'in_progress', toolCallId: 'running', data: { input: { at: '2026-10-29T09:00:00+00:00', why: 'Wait for the owner' } } });
call('kubectl delete pod stuck-1', tool('execute', 'kubectl', 'delete pod stuck-1'));
ev('system', { ...asked('p_1', 'kubectl delete pod stuck-1', 'chair'), status: 'permission_asked', title: 'kubectl delete pod stuck-1', data: { request: 'p_1', step: 'chair' } });
ev('system', { ...decided('kubectl delete pod stuck-1', true, 'p_1'), status: 'permission_answered', by: chair, title: 'kubectl delete pod stuck-1', text: 'the pod is stuck and the goal needs it cleared', data: { request: 'p_1', allow: true } });
const grafana = `S=/tmp/remit-verify/scratchpad; source /workspace/example/secret/devops/grafana.env; jq -n --rawfile sql $S/q3.sql '{queries:[{refId:"A",datasource:{uid:"coroot-clickhouse",type:"grafana-clickhouse-datasource"},rawSql:$sql,format:1,queryType:"sql"}],from:"now-7d",to:"now"}' > $S/q.json; curl -s -u "$GRAFANA_USER:$GRAFANA_PASSWORD" -H 'Content-Type: application/json' -X POST "$GRAFANA_URL/api/ds/query" -d @$S/q.json -w '\\nHTTP %{http_code}\\n' | head -c 600`;
call(grafana, tool('execute', 'jq', grafana.slice(grafana.indexOf('jq ') + 3)));
ev('system', { ...asked('p_2', grafana.slice(grafana.indexOf('jq ')), 'owner'), status: 'permission_asked', title: grafana, data: { request: 'p_2', step: 'owner' } });
ev('system', { ...decided(grafana.slice(grafana.indexOf('jq ')), true, 'p_2'), status: 'permission_answered', by: owner, title: grafana, text: 'Re-runs the same read-only count query to debug its response; nothing is written.', data: { request: 'p_2', allow: true } });
call('git stash list', tool('execute', 'git', 'stash list'));
// Past the owner's time limit an AI employee's prompt falls to its Chair, the session's driver.
ev('system', { ...asked('p_3', 'git stash list', 'chair'), status: 'permission_asked', title: 'git stash list', data: { request: 'p_3', step: 'chair' } });
ev('system', { ...decided('git stash list', true, 'p_3'), status: 'permission_answered', by: chair, title: 'git stash list', text: 'nobody decided within the limit: a read of its own worktree', data: { request: 'p_3', allow: true } });
call('cat ~/.aws/credentials', tool('execute', 'cat', '~/.aws/credentials', 'failed'), 'failed');
ev('system', { ...decided('cat ~/.aws/credentials', false), status: 'permission_answered', by: actorOf('remit', 'rules'), title: 'cat ~/.aws/credentials', text: 'a secret is never read: ~/.aws/credentials', data: { request: '', allow: false } });
call('aws s3 rm s3://bucket/key', tool('execute', 'aws', 's3 rm s3://bucket/key', 'failed'), 'failed');
// An old event the store reads as permission_answered By the policy (store/events.go answeredNow).
ev('system', { ...decided('aws s3 rm s3://bucket/key', false), status: 'permission_answered', by: actorOf('remit', 'policy'), title: 'aws s3 rm s3://bucket/key', text: 'this reaches outside the chain (push, deploy, delete or write to a shared system)', data: { allow: false } });
const nextest = 'export S=/tmp/remit-verify/scratchpad; cd $S/head && export CARGO_TARGET_DIR=$S/target && cargo nextest run -p jail -p agent -p airuntime --locked --no-fail-fast 2>&1 | grep -E "^\\s+(PASS|FAIL)|Summary" | sort -u | cut -c1-170';
call(nextest, tool('execute', 'cargo', 'nextest run -p jail -p agent -p airuntime --locked --no-fail-fast', 'running'), 'in_progress');
ev('system', { ...asked('p_7', 'cargo nextest run -p jail -p agent -p airuntime --locked --no-fail-fast 2>&1 | grep -E "^\\s+(PASS|FAIL)|Summary"', 'chair'), status: 'permission_asked', title: nextest, data: { request: 'p_7', step: 'chair' } });
ev('system', { text: 'The owner interrupted the turn.', by: actorOf('owner', 'alexp') });
ev('system', { status: 'error', text: 'agent process exited: signal: killed' });
ev('system', { status: 'waiting', text: 'The provider cannot run this agent now (claude: You\'ve hit your session limit · resets 9:40am (UTC)): Remit sends it its message again once it can.' });
ev('system', { status: 'new_session', text: 'Replaced session 0ee7b51d-bc42-42d6-8283-b4d6025db50f with 93a4756c-3bbb-423b-a388-2087e662c744 because its saved context was not found. The next prompt repeats its brief and asks it to check git status and git log.' });
ev('prompt', { title: 'Rework after feedback', text: '[feedback from the Chair · rework]\nThe core retry work meets its criteria. Fix each point: add ad_account_id to the error line.', by: chair, status: 'queued' });

// The deployer, timed in seconds from t0.
const dep = <K extends EventKind>(kind: K, secs: number, extra: EventSeed<K> = {}, endSecs = secs) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId: 'deployer', createdAt: iso(t0 + secs * 1000), updatedAt: iso(t0 + endSecs * 1000) }, kind, extra));
const own = actorOf('agent', 'deployer') as Actor;
// Its background tasks, as the server records them: what started each, what it runs, and the end of what an ended one wrote.
// Typed by the Rust contract, whose server records them (crates/orch background.rs).
const shell = (command: string, extra: Partial<TaskEventData> = {}) => {
  const data: TaskEventData = { taskId: 'b' + n, kind: 'local_bash', tool: 'Bash', command, readable: true, stoppable: true, ...extra };
  return { data };
};
dep('task', 0, { title: 'tail ingest logs until the backfill batch lands', status: 'stopped', text: 'The chain was paused before it finished.', ...shell('tail -f /var/log/ingest.log', { output: 'batch 41 written\nbatch 42 written\n' }) }, 95);
dep('prompt', 100, { title: 'Watch the production rollout', text: '[handoff from the Chair]\nRoll out MR !68 and watch it until every pod is ready.', by: chair, status: 'delivered' });
dep('tool', 104, { ...tool('other', 'Monitor', ''), status: 'completed', title: 'Monitor', toolCallId: 'mon' });
dep('task', 105, { title: 'watch the rollout (kubectl rollout status, every 30 s)', status: 'running', ...shell('while true; do kubectl rollout status deploy/ingest -n prod --timeout=30s 2>&1 | grep --line-buffered -E "ready|error|failed"; sleep 30; done', { tool: 'Monitor' }) });
dep('tool', 106, { ...tool('execute', 'go', 'run ./cmd/migrate-zones'), status: 'completed', title: 'go run ./cmd/migrate-zones', toolCallId: 'mig' });
dep('task', 107, { title: 'run the zone migration', status: 'completed', text: 'Background command "run the zone migration" completed (exit code 0)', ...shell('go run ./cmd/migrate-zones', { output: 'zone 1/53 backfilled\n…\nzone 53/53 backfilled\n0 errors\n\n[exited with code 0]\n' }) }, 347);
dep('message', 110, { text: 'Watching the rollout and the zone migration; I will report when either has news.' });
dep('stop', 111, { ...turnEnd(11_000), status: 'end_turn', data: { durationMs: 11_000 } });
dep('prompt', 347, { title: 'News from its background work', text: 'Background command "run the zone migration" completed (exit code 0)', by: own, status: 'delivered' });
dep('tool', 349, { ...tool('read', 'read', 'migrate-zones.output'), status: 'completed', title: 'Read migrate-zones.output', toolCallId: 'out' });
dep('message', 360, { text: 'The zone migration finished: **53 zones** backfilled, 0 errors. The rollout is still at 4 of 6 pods.' });
dep('stop', 361, { ...turnEnd(14_000), status: 'end_turn', data: { durationMs: 14_000 } });
dep('prompt', 420, { title: 'News from its background work', text: 'What it watches: watch the rollout (kubectl rollout status, every 30 s)', by: own, status: 'delivered' });
dep('stop', 424, { ...turnEnd(4_000), status: 'end_turn', data: { durationMs: 4_000 } });

// The reviewer: the run between its two messages folds; the two steps after the second stay open.
const rv = <K extends EventKind>(kind: K, secs: number, extra: EventSeed<K> = {}, endSecs = secs) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId: 'reviewer', createdAt: iso(t0 + secs * 1000), updatedAt: iso(t0 + endSecs * 1000) }, kind, extra));
rv('message', 0, { text: 'Now the diff and test names.' });
rv('tool', 2, { ...tool('execute', 'git', 'diff 9ee3199 --stat'), status: 'completed', title: '`git diff 9ee3199 --stat`' }, 4);
rv('thought', 5, { text: "I'm noting that the SQS handler change moves Timeout to the outermost wrapper, since the previous run timed out first." });
rv('thought', 6, { text: '**Evaluating route assertions**\n\nI should read `route.go` first.' });
rv('prompt', 7, { title: 'Nudged for standing still', text: '[nudge]\nYou have not reported for 10 minutes. Say where you are.', by: actorOf('remit'), status: 'delivered' });
rv('tool', 8, { ...tool('execute', 'go', 'test ./internal/sqs/...'), status: 'completed', title: '`go test ./internal/sqs/...`' }, 108);
rv('tool', 9, { ...tool('read', 'read', 'handler_test.go'), status: 'completed', title: 'Read handler_test.go' }, 10);
rv('prompt', 110, { title: 'add -race and strict stress run', text: '[message from the Chair]\nadd -race and strict stress run', by: chair, status: 'steered' });
rv('tool', 112, { ...tool('execute', 'go', 'test -race -count=5 ./...'), status: 'completed', title: '`go test -race -count=5 ./...`' }, 225);
rv('thought', 226, { text: 'There is a race failure in the stress test at line 508 under -race. I should check whether this fails on base too.' });
rv('tool', 228, { ...tool('execute', 'git', 'worktree add /tmp/base 9ee3199'), status: 'completed', title: '`git worktree add /tmp/base 9ee3199`' }, 278);
rv('thought', 280, { text: 'The flakiness under race conditions seems to affect both HEAD and base, so it looks like a pre-existing issue.' });
rv('message', 282, { text: 'Race run: `TestStress_ThrottleContention` fails under `-race` on HEAD and on 9ee3199, so it isn\'t caused by this change. Checking the `disputeingest` build failure on base next.' });
rv('tool', 284, { ...tool('execute', 'go', 'build ./cmd/disputeingest'), status: 'completed', title: '`go build ./cmd/disputeingest`' }, 290);
rv('thought', 291, { text: 'The build fails on base as well.' });

// The session Lan helps, 20 s apart after the engineer's turn.
const le = <K extends EventKind>(kind: K, extra: EventSeed<K> = {}) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId: 'helped', createdAt: iso((at += 20_000)), updatedAt: iso(at) }, kind, extra));
le('prompt', { title: 'Fix the flaky test', text: '[handoff from the Chair]\nTestStress_ThrottleContention fails one run in ten under -race. Find why and fix it.', by: chair, status: 'delivered' });
le('prompt', { title: 'Lan is your mentor', text: '[from the Chair] Lan, strong in Go concurrency, is now your mentor on this card, for: the flaky throttle test.', by: actorOf('remit'), status: 'delivered' });
le('prompt', { title: 'Run it 50 times first', text: '[from Lan, your mentor]\nRun the stress test 50 times under -race and tell me how often it fails.', by: lan, status: 'steered' });
le('message', { text: 'It failed 6 of 50 runs: the throttle reads `limits` before the refill goroutine writes it.' });
le('tool', { title: 'go test -race -count=50 -run TestStress_ThrottleContention ./throttle', ...tool('execute', 'go', 'test -race -count=50 -run TestStress_ThrottleContention ./throttle'), status: 'completed', toolCallId: 'lan1' });
le('system', { ...asked('p_5', 'git push origin fix/flaky-throttle', 'chair'), status: 'permission_asked', title: 'git push origin fix/flaky-throttle', data: { request: 'p_5', step: 'chair' } });
le('system', { ...decided('git push origin fix/flaky-throttle', true, 'p_5'), status: 'permission_answered', by: chair, title: 'git push origin fix/flaky-throttle', text: 'my branch, for the review', data: { request: 'p_5', allow: true } });
le('prompt', { title: 'Keep the refill lock', text: '[message from the Chair]\nKeep the lock around the refill; do not make limits atomic.', by: chair, status: 'delivered' });
le('prompt', { title: 'Looks good', text: '[from Lan, your mentor]\nLooks good. Report it to the Chair with the 50 runs.', by: lan, status: 'delivered' });
le('message', { text: 'Fixed: the refill takes the lock before it writes limits. 50 of 50 runs pass under -race.' });
le('stop', { ...turnEnd(), status: 'end_turn' });

// The writer and the thinker, each in the middle of its last block.
const writing = (cardId: string, kind: 'message' | 'thought', text: string) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId, createdAt: iso(at), updatedAt: iso(at) }, kind, { text }));
writing('writer', 'thought', 'It looks like the push did not go through, since ls-remote does not list the branch.');
writing('writer', 'message', 'The branch never reached GitLab: `git ls-remote` finds it missing after the publish run.\n\nChecked:\n\n- the remote\n- the `publish` job log\n\nResubmitting the same approved version.');
writing('thinker', 'thought', 'The handler drops code 17 as an OAuth error,\nso the producer calls again and again');

const chain = chainRecord({ id: 'ch', title: 'x', goal: 'x', status: 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(at), rev: 1 });
// What the server says each agent is on: the receipts under the owner's messages read it.
const agents = [
  agentStands('ch', 'chair', '', 'running', '', 'busy'),
  agentStands('ch', 'engineer', '', 'running', '', 'busy'),
].map((a) => ({ ...a, onMessage: a.cardId === 'engineer' ? { eventId: engineerReads, answering: true, at: iso(at) } : last === 'running' ? { eventId: chairReads, answering: true, at: iso(at) } : undefined }));
// The Rust server's calls a task row asks: a running task's output grows as the page asks again; an ended one's is what its
// event kept.
let reads = 0;
const taskOutput = (eventId: number): TaskOutput => {
  const e = events.find((x) => x.event.id === eventId);
  if (e?.event.status !== 'running') {
    const output = (e && dataOf(e).output) ?? '';
    return { text: output, totalBytes: output.length, truncated: false };
  }
  reads++;
  const lines = Array.from({ length: 24 + reads }, (_, i) => `[${String(8 + Math.floor(i / 2)).padStart(2, '0')}:${i % 2 ? '30' : '00'}] Waiting for deployment "ingest" rollout to finish: ${Math.min(6, 2 + Math.floor(i / 2))} of 6 updated replicas are available...`);
  return { text: lines.join('\n') + '\n', totalBytes: 18_400 + reads * 120, truncated: true };
};
fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], mentors: [mentorRecord({ id: 'mt_lan', label: 'Lan', strengths: 'Go concurrency', status: 'active', createdAt: iso(t0) })] }),
  chainGet: () => ({ chain: chainRead(chain), cards, edges: [], retrospectives: [], decisions: [], helps: [helpRecord({ id: 'help_lan', chain: 'ch', card: 'helped', mentor: 'mt_lan', problem: 'The throttle test fails one run in ten.', state: 'accepted', offeredAt: iso(t0), deadline: iso(t0 + 7_200_000), answeredAt: iso(t0), rev: 2 })], verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, datadog: [], landings: [], drafts: [], workflows: [], agents, standing: stands('ch', 'Running', 'running') }),
  chainEvents: () => ({ events, truncated: false }),
  cardTaskOutput: taskOutput,
});
openChain('ch');

const column = (cardId: string, live: boolean) =>
  h('div', { 'data-card': cardId, style: { width: width + 'px', flex: 'none' } }, [h(StreamBlocks, { blocks: blocks(events.filter((e) => e.event.cardId === cardId)), live })]);
createApp({
  render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex items-start gap-6 p-4' }, opened.view ? [column('chair', last === 'running'), column('engineer', true), column('helped', false), column('deployer', false), column('writer', true), column('thinker', true), column('reviewer', false)] : [])),
})
  .provide(apiKey, api)
  .mount('#app');
