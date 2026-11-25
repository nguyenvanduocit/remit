// A chain at work through scripts and subagents (ADR-0073), made up for the verify pages: the Chair's Remit run fanning out to
// five standing employees (three reported, two at work), a Remit run waiting on a signal with a question to the owner, a Remit
// run that failed, and a Claude Code run under the engineer whose three subagents are child cards inside its process (one
// stopped); the Chair also spawned one subagent of its own. Every rule (where a card hangs, what the owner can ask of it) is the
// server's: this writes out its answers.
import type { AgentStanding, Card, Workflow, WorkflowStep } from '../src/core/generated/types.generated';
import { agentStands, placed } from './standingSeed';
import { cardRecord, chainRecord, employeeRecord, workflowRecord, workflowStepRecord } from './records';
import { wordedRead } from './employeeSeed';

const MIN = 60_000;

export function workflowSeed(now: number) {
  const iso = (ms: number) => new Date(ms).toISOString();
  const t0 = now - 180 * MIN;
  const at = (min: number) => iso(t0 + min * MIN);

  const roster: [string, number, string, string][] = [
    ['e_ana', 12, 'Ana', 'reviewer'], ['e_bao', 14, 'Bao', 'reviewer'], ['e_chi', 15, 'Chi', 'qc'], ['e_dung', 17, 'Dung', 'security'], ['e_hai', 21, 'Hai', 'reviewer'],
    ['e_kim', 7, 'Kim', 'engineer'],
  ];
  const employees = roster.map(([id, number, label, template]) => wordedRead(
    employeeRecord({ id, number, label, template, templateRev: 1, title: 'worker', reportsTo: 'owner', hiredBy: 'owner', status: 'active', createdAt: at(0) }),
    { shape: 'hexagon', hue: (number * 47) % 360, accessory: 'antenna' },
  ));

  const card = (id: string, agent: string, name: string, min: number, extra: Partial<Card> = {}): Card =>
    cardRecord({ id, home: 'ch', agent, name, origin: 'hired', model: 'sonnet', mode: 'auto', cwd: '/', status: 'stopped', plan: [], ctxUsed: 40_000, ctxSize: 200_000, turns: 6, queued: 0, sessionId: 's-' + id, createdAt: at(min), updatedAt: at(min + 10), rev: 1, ...extra });
  const spawn = (parent: string, task: string) => ({ origin: 'spawned' as const, sessionId: undefined, spawn: { parent, task, toolCall: 'tc-' + task } });
  const cards: Card[] = [
    card('chair', 'chair', 'Chair', 0, { origin: 'chair', model: 'opus', status: 'running', turns: 23 }),
    card('eng', 'engineer', 'Engineer · checkout retries', 5, { employee: 'e_kim', model: 'opus', status: 'running', turns: 31 }),
    // The fan-out of the Chair's Remit run: five reviews, one per area.
    card('rv-auth', 'reviewer', 'Review · auth module', 60, { employee: 'e_ana', status: 'idle' }),
    card('rv-pay', 'reviewer', 'Review · payment webhook', 60, { employee: 'e_bao', status: 'idle' }),
    card('rv-cart', 'qc', 'Review · cart totals', 60, { employee: 'e_chi', status: 'idle' }),
    card('rv-sec', 'security', 'Review · secrets in logs', 60, { employee: 'e_dung', status: 'running', turns: 3 }),
    card('rv-api', 'reviewer', 'Review · public API shape, every endpoint the mobile app still calls', 60, { employee: 'e_hai', status: 'running', turns: 2 }),
    // Claude Code's subagents inside the engineer's process: three of its Workflow run, one stopped.
    card('sub-tests', 'reviewer', 'Find the flaky retry tests', 120, { ...spawn('eng', 'a1'), employee: 'e_ana', status: 'stopped', turns: 4 }),
    card('sub-idem', 'security', 'Check idempotency keys', 120, { ...spawn('eng', 'a2'), employee: 'e_dung', status: 'running', turns: 2 }),
    card('sub-docs', 'reviewer', 'Read the payment provider retry docs', 121, { ...spawn('eng', 'a3'), employee: 'e_hai', status: 'running', turns: 1 }),
    // The Chair's own subagent.
    card('sub-chair', 'qc', 'Sanity-check the release plan', 150, { ...spawn('chair', 'a4'), employee: 'e_chi', status: 'running', turns: 1 }),
  ];

  const leaf = (id: string, label: string, tone: AgentStanding['tone'], doing: string, doingTone: AgentStanding['doingTone'], asks: AgentStanding['asks'] = ['message', 'stop']) =>
    agentStands('ch', id, label, tone, doing, doingTone, [], { asks });
  const agents = placed('chair', cards, [
    leaf('chair', 'Running', 'running', 'Waiting for review-fanout and the engineer', 'waiting', ['message', 'interrupt']),
    leaf('eng', 'Running', 'running', 'Running workflow fix-retries · Investigate', 'busy', ['message', 'interrupt', 'stop']),
    leaf('rv-auth', 'Awaiting acceptance', 'waiting', 'Reported · no blocking issue', 'waiting'),
    leaf('rv-pay', 'Awaiting acceptance', 'waiting', 'Reported · the retry is not idempotent', 'waiting'),
    leaf('rv-cart', 'Awaiting acceptance', 'waiting', 'Reported · totals round twice', 'waiting'),
    leaf('rv-sec', 'Running', 'running', 'Grepping the log statements', 'busy'),
    leaf('rv-api', 'Running', 'running', 'Reading internal/api/routes.go', 'busy'),
    leaf('sub-tests', 'Stopped', 'idle', 'Stopped before it reported', 'idle', []),
    leaf('sub-idem', 'Running', 'running', 'Reading payment/webhook.go', 'busy', ['stop']),
    leaf('sub-docs', 'Running', 'running', 'Fetching the provider docs', 'busy', ['stop']),
    leaf('sub-chair', 'Running', 'running', 'Reading the release plan', 'busy', ['stop']),
  ], { 'sub-tests': 'eng', 'sub-idem': 'eng', 'sub-docs': 'eng' });

  const step = (workflow: string, seq: number, verb: WorkflowStep['verb'], min: number, extra: Partial<WorkflowStep> = {}): WorkflowStep =>
    workflowStepRecord({ workflow, seq, verb, status: 'done', at: at(min), endedAt: at(min + 5), ...extra });
  const review = (seq: number, cardId: string, area: string, done: boolean): WorkflowStep =>
    step('w_fan', seq, 'agent', 60, { label: `Review ${area}`, phase: 'Review', card: cardId, input: { prompt: `Review the ${area} change on branch feat/checkout-retries. Report blocking issues only, with file and line.`, agentType: cardId }, status: done ? 'done' : 'pending', endedAt: done ? at(75) : undefined });

  const workflows: Workflow[] = [
    workflowRecord({ id: 'w_old', home: 'ch', card: 'chair', engine: 'remit', name: 'triage-flakes', script: 'export const meta = { name: "triage-flakes" }\nconst found = await agent("List the flaky tests")\nreturn found', status: 'failed',
      error: 'The script diverged from its journal at call 2: it asked for agent("Fix them all") where it had recorded wait({at}).', steps: [
        step('w_old', 0, 'agent', 10, { label: 'List the flaky tests', card: 'eng', input: { prompt: 'List the flaky tests in the checkout suite.' } }),
        step('w_old', 1, 'wait', 20, { input: { at: at(40) } }),
        step('w_old', 2, 'agent', 41, { status: 'failed', label: 'Fix them all', input: { prompt: 'Fix them all.' }, error: 'Not the call recorded at this place: the script changed while the run waited.' }),
      ], createdAt: at(10), endedAt: at(41), rev: 4 }),
    workflowRecord({ id: 'w_fan', home: 'ch', card: 'chair', engine: 'remit', name: 'review-fanout', status: 'running', phase: 'Review',
      script: 'export const meta = { name: "review-fanout", description: "Five reviews at once, then a verdict" }\nphase("Review")\nconst areas = args.areas\nconst reports = await parallel(areas.map((a) => () => agent(`Review the ${a} change`, { agentType: a })))\nphase("Verdict")\nreturn agent("Merge these reports into one verdict: " + JSON.stringify(reports))',
      args: { areas: ['auth', 'payment', 'cart', 'secrets', 'api'] }, steps: [
        step('w_fan', 0, 'phase', 60, { input: { title: 'Review' } }),
        step('w_fan', 1, 'log', 60, { input: { message: 'Five reviewers, one per area of the change.' } }),
        review(2, 'rv-auth', 'auth module', true), review(3, 'rv-pay', 'payment webhook', true), review(4, 'rv-cart', 'cart totals', true),
        review(5, 'rv-sec', 'secrets in logs', false), review(6, 'rv-api', 'public API shape', false),
      ], createdAt: at(60), rev: 9 }),
    workflowRecord({ id: 'w_wait', home: 'ch', card: 'chair', engine: 'remit', name: 'canary-then-ship', status: 'waiting', phase: 'Canary',
      script: 'export const meta = { name: "canary-then-ship" }\nphase("Canary")\nawait wait({ key: "chain:ch.sensor.checkout-error-rate", condition: "value.rate < 0.02", until: args.until })\nconst go = await askOwner("The canary held under 2% for an hour. Ship to every shop?")\nreturn go',
      steps: [
        step('w_wait', 0, 'phase', 140, { input: { title: 'Canary' } }),
        step('w_wait', 1, 'wait', 140, { input: { at: at(150) } }),
        step('w_wait', 2, 'wait', 150, { status: 'pending', endedAt: undefined, input: { key: 'chain:ch.sensor.checkout-error-rate', condition: 'value.rate < 0.02', until: iso(now + 50 * MIN) } }),
        step('w_wait', 3, 'ask_owner', 150, { status: 'pending', endedAt: undefined, input: { question: 'The canary held under 2% for an hour. Ship the retry change to every shop now, or keep it on the canary shops over the weekend?' } }),
      ], createdAt: at(140), rev: 5 }),
    workflowRecord({ id: 'w_claude', home: 'ch', card: 'eng', engine: 'claude', name: 'fix-retries', status: 'running', phase: 'Investigate',
      script: 'export const meta = { name: "fix-retries" }\nphase("Investigate")\nconst [tests, idem, docs] = await parallel([\n  () => agent("Find the flaky retry tests", { agentType: "ana-12" }),\n  () => agent("Check idempotency keys", { agentType: "dung-17" }),\n  () => agent("Read the payment provider retry docs", { agentType: "hai-21" }),\n])\nreturn { tests, idem, docs }',
      claude: { taskId: 't9', runId: 'r9', scriptPath: '.claude/workflows/fix-retries.js', transcriptDir: '~/.claude/projects/x/r9' }, steps: [
        step('w_claude', 0, 'phase', 120, { input: { title: 'Investigate' } }),
        step('w_claude', 1, 'agent', 120, { label: 'Find the flaky retry tests', card: 'sub-tests', status: 'failed', error: 'Stopped by the owner.', input: { prompt: 'Find the flaky retry tests' } }),
        step('w_claude', 2, 'agent', 120, { label: 'Check idempotency keys', card: 'sub-idem', status: 'pending', endedAt: undefined, input: { prompt: 'Check idempotency keys' } }),
        step('w_claude', 3, 'agent', 121, { label: 'Read the payment provider retry docs', card: 'sub-docs', status: 'pending', endedAt: undefined, input: { prompt: 'Read the payment provider retry docs' } }),
      ], createdAt: at(120), rev: 6 }),
    workflowRecord({ id: 'w_done', home: 'ch', card: 'chair', engine: 'remit', name: 'collect-baseline', status: 'done', script: 'export const meta = { name: "collect-baseline" }\nreturn agent("Read the error baseline")',
      result: { rate: 0.031, window: '7d', source: 'checkout-error-rate' }, steps: [step('w_done', 0, 'agent', 30, { label: 'Read the error baseline', card: 'eng', input: { prompt: 'Read the checkout error baseline for the last 7 days.' } })], createdAt: at(30), endedAt: at(35), rev: 3 }),
  ];

  workflows.sort((a, b) => a.createdAt.localeCompare(b.createdAt)); // the server lists them oldest first
  const chain = chainRecord({ id: 'ch', title: 'Make checkout retries idempotent', goal: 'Retries never charge twice.', status: 'running', chairId: 'chair', workdir: '~/.remit/work/ch', origin: 'owner', createdAt: at(0), updatedAt: iso(now) });
  return { chain, cards, agents, workflows, employees };
}
