// Small display helpers shared by components.
import type { AgentTone, Card, CardStatus, ChainStatus, ChainTone, CheckTone, DoingTone, FindingBlocker, FindingStatus, GateState, OutcomeReads, SubscriptionState, WaitKind, WorkflowStatus } from './core/generated/types.generated';

const AGENT_LETTERS: Record<string, string> = {
  chair: 'C', research: 'R', architect: 'A', critic: 'Cr', engineer: 'E', reviewer: 'Rv', security: 'Sec',
  qa: 'Q', qc: 'QC', judge: 'J', pm: 'P', release: 'Rl', scout: 'S', human: 'You',
};
const KNOWN = new Set(Object.keys(AGENT_LETTERS));

export function agentLetter(agent: string): string {
  return AGENT_LETTERS[agent] ?? (agent.slice(0, 2).replace(/^./, (c) => c.toUpperCase()) || '?');
}

// Known agents use AgentBadge's A-<agent> class; others get a stable colour from their name.
export function agentClass(agent: string): string {
  return KNOWN.has(agent) ? `A-${agent}` : '';
}
export function agentStyle(agent: string): Record<string, string> {
  if (KNOWN.has(agent)) return {};
  let h = 0;
  for (const ch of agent) h = (h * 31 + ch.charCodeAt(0)) % 360;
  return { background: `hsl(${h} 35% 18%)`, color: `hsl(${h} 70% 72%)` };
}

export const STATUS_LABEL: Record<string, string> = {
  running: 'Running', idle: 'Turn done', permission: 'Needs permission', starting: 'Starting', stopped: 'Stopped', failed: 'Failed',
  queued: 'Queued', paused: 'Paused', waiting: 'Waiting', done: 'Done',
};

// A size in bytes, in the largest unit that keeps it short: 412 B, 3.2 KB, 150 KB, 1.0 MB.
export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const kb = n / 1024;
  if (kb < 1024) return `${kb >= 100 ? Math.round(kb) : kb.toFixed(1)} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}
export const tokens = (n: number): string => (n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e3 ? Math.round(n / 1e3) + 'k' : String(n || 0));

export function hm(iso: string | number): string {
  const d = new Date(iso);
  return d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
}

// A time of day, with the date when it is not today: when a Chair checks back, when a run is next, when a post came.
export function clockAt(iso: string | number): string {
  const d = new Date(iso);
  return d.toDateString() === new Date().toDateString() ? hm(iso) : d.toLocaleDateString('en-GB', { day: '2-digit', month: '2-digit' }) + ' ' + hm(iso);
}

// The whole date and time, for the hint under a relative one (Ago).
export function fullTime(iso: string): string {
  return new Date(iso).toLocaleString('en-GB');
}

export function ago(iso: string, now = Date.now()): string {
  const s = (now - new Date(iso).getTime()) / 1000;
  if (s < 60) return 'just now';
  if (s < 3600) return Math.floor(s / 60) + ' min ago';
  if (s < 86400) return Math.floor(s / 3600) + ' h ago';
  return Math.floor(s / 86400) + ' d ago';
}

// An agent's Markdown as one plain line of at most n characters, for a summary above the text itself: its emphasis, code
// ticks, heading and list marks and link targets are dropped, its words stay.
export function short(s: string | undefined, n: number): string {
  if (!s) return '';
  const one = s
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}(#{1,6}|[-*+]|\d+\.)\s+/gm, '')
    .replace(/(\*\*|__)(?=\S)(.+?)(?<=\S)\1/g, '$2')
    .replace(/(?<![\w*])([*_])(?=\S)([^*_]+?)(?<=\S)\1(?![\w*])/g, '$2')
    .replace(/`+([^`]+)`+/g, '$1')
    .replace(/\s+/g, ' ')
    .trim();
  return one.length > n ? one.slice(0, n - 1) + '…' : one;
}

// A count with its noun: 1 turn, 3 turns; `many` for a noun whose plural is not its singular plus s.
export const plural = (n: number, noun: string, many = noun + 's'): string => `${n} ${n === 1 ? noun : many}`;

// A model and the effort it runs at, as one label: fable · high, or the model alone at Claude Code's default effort.
export const modelEffort = (model: string, effort?: string): string => (effort ? `${model} · ${effort}` : model);

// What an agent is called wherever it shows: the Chair by its role, any other agent by its card's name.
export const agentName = (card: Pick<Card, 'origin' | 'name'>): string => (card.origin === 'chair' ? 'Chair' : card.name);

// A length of time in its largest units: 27s, 4m, 1h 12m.
export function dur(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return s + 's';
  const m = Math.floor(s / 60);
  if (m < 60) return m + 'm';
  return Math.floor(m / 60) + 'h ' + (m % 60) + 'm';
}

// How long a step took, to the second while it is under an hour: 42s, 1m 23s, 1h 5m.
export function durSec(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  return s >= 60 && s < 3600 ? Math.floor(s / 60) + 'm ' + (s % 60) + 's' : dur(ms);
}

// A length of time in the units of dur with days beyond a day: 27s, 4m, 1h 12m, 2d 5h.
export function span(ms: number): string {
  const m = Math.floor(Math.round(ms / 1000) / 60);
  return m >= 1440 ? Math.floor(m / 1440) + 'd ' + Math.floor((m % 1440) / 60) + 'h' : dur(ms);
}

// Whether the server gave a time: one it does not have is left out (a watch with no deadline, an event signal).
export const timed = (iso?: string): iso is string => !!iso;

// How far off a time is, in the units of span: in 27s, in 1h 12m, in 2d 5h; once it has passed, how late: 5m overdue.
export function until(ms: number): string {
  if (Math.abs(ms) < 1000) return 'now';
  return ms > 0 ? 'in ' + span(ms) : span(-ms) + ' overdue';
}

// How a state reads at a glance, everywhere it shows: its tone and, beside its words, its icon. A tone is a Badge variant of
// the same name (StandingBadge) and a text colour (toneText). `needs` is what waits on the owner, orange wherever it shows;
// `warning` is something to notice that does not wait on him; `gate` is a wait on time (a check-back).
export type Tone = 'quiet' | 'info' | 'success' | 'warning' | 'gate' | 'needs' | 'danger';
export interface Look {
  tone: Tone;
  icon?: string;
}
export const toneText: Record<Tone, string> = {
  quiet: 'text-mut',
  info: 'text-info',
  success: 'text-success',
  warning: 'text-warning',
  gate: 'text-sand',
  needs: 'text-orange',
  danger: 'text-destructive',
};

// The icon of an agent's badge, by the tone the server gave it (AgentStanding.tone).
const GLYPH: Record<string, string> = { failed: 'alert', needs: 'clock', waiting: 'clock', done: 'check' };
export const badgeGlyph = (tone: string): string => GLYPH[tone] ?? '';

// The tone of where a chain stands and of a chain's, a card's or a workflow's status, each listed, so a new one fails the
// typecheck until it has a colour. A chain that `needs` the owner, and a card asking his `permission`, are in his orange; a
// chain whose first wait `failed` is red.
export type Said = ChainTone | ChainStatus | CardStatus | WorkflowStatus;
const TONE: Record<Said, Tone> = {
  running: 'info',
  done: 'success',
  idle: 'success',
  needs: 'needs',
  permission: 'needs',
  waiting: 'gate',
  watching: 'gate',
  notdone: 'warning',
  failed: 'danger',
  queued: 'quiet',
  paused: 'quiet',
  starting: 'quiet',
  stopped: 'quiet',
};
export const statusTone = (said: Said): Tone => TONE[said];

// The icon a chain's badge takes beside its words when the words are the state and colour alone must not carry it: a chain
// that watches a signal, a finished chain that is Done or Not done, one whose first wait failed. A chain that needs the owner
// wears the icon of what waits (WAIT_ICON).
const CHAIN_ICON: Partial<Record<ChainTone, string>> = { watching: 'eye', done: 'check', notdone: 'alert', failed: 'alert' };
export const chainIcon = (tone: ChainTone): string | undefined => CHAIN_ICON[tone];

// The icon of each kind of thing that waits for the owner (ADR-0078), beside the server's word for it (Wait.label), so the
// kind never rests on the word alone: a question, a permission to run something, a failure, a change to apply, a credential,
// a sign-in, an exception to decide, a held subscription.
export const WAIT_ICON: Record<WaitKind, string> = {
  asks: 'help',
  approve: 'terminal',
  failed: 'alert',
  apply: 'pencil',
  key: 'key',
  sign_in: 'person',
  decide: 'scale',
  held: 'pause',
};
// A wait's tone: a failure in red, everything else in the owner's orange.
export const waitTone = (kind: WaitKind): Tone => (kind === 'failed' ? 'danger' : 'needs');

// The tone of where an agent stands (AgentStanding.tone). One that `needs` the owner is in his colour; one `waiting` waits on
// something else (a provider, the Chair's acceptance), in the colour of a wait, as a chain waiting on its check-back is.
const AGENT_TONE: Record<AgentTone, Tone> = { needs: 'needs', waiting: 'gate', failed: 'danger', running: 'info', done: 'success', idle: 'quiet' };
export const agentTone = (tone: AgentTone): Tone => AGENT_TONE[tone];

// The colour of the line saying what an agent is doing (AgentStanding.doingTone): waiting on the owner in his colour, on
// anything else in the colour of a wait.
const DOING: Partial<Record<DoingTone, string>> = { needs: toneText.needs, waiting: toneText.gate, error: toneText.danger };
export const doingText = (doingTone: DoingTone): string => DOING[doingTone] ?? '';

// A subscription the owner has to notice; one that is waiting or paused reads from its time. One that is over has no next time.
export const SUBSCRIPTION_LOOK: Partial<Record<SubscriptionState, Look & { label: string }>> = {
  held: { tone: 'warning', icon: 'alert', label: 'Held' },
  due: { tone: 'info', label: 'Due now' },
  underway: { tone: 'info', label: 'In progress' },
  over: { tone: 'quiet', icon: 'circle', label: 'No next run' },
};

// A gate of a chain's verification (ChainVerification); `running` shows its ring.
export const GATE_LOOK: Record<GateState, Look> = {
  approved: { tone: 'success', icon: 'check' },
  stale: { tone: 'warning', icon: 'clock' },
  changes: { tone: 'warning', icon: 'x' },
  running: { tone: 'info' },
  none: { tone: 'quiet', icon: 'circle' },
};

// Where a judge's check stands: passed, asking for changes (as a gate does), failed to run, or running.
export const CHECK_LOOK: Record<CheckTone, Look> = {
  done: { tone: 'success', icon: 'check' },
  needs: { tone: 'warning', icon: 'x' },
  failed: { tone: 'danger', icon: 'alert' },
  running: { tone: 'info' },
};

// A finding of a verdict, and how a screen reader says it.
export const FINDING_LOOK: Record<FindingStatus, Look & { label: string }> = {
  met: { tone: 'success', icon: 'check', label: 'met' },
  not_met: { tone: 'danger', icon: 'x', label: 'not met' },
  unverified: { tone: 'warning', icon: 'alert', label: 'not verified' },
};

// What an unverified key result waits on, as the owner reads it beside the finding.
export const BLOCKER_SAYS: Record<FindingBlocker, string> = {
  write: 'waits on a write',
  deploy: 'waits on a deploy',
  person: 'waits on a decision',
  access: 'waits on access',
  watch: 'waits on its watch',
};

// How an outcome's last period reads against the one before; `text` stands in for a change too small or too new to show,
// and a change of none is said against the period before, in the span's words.
export const OUTCOME_LOOK: Record<OutcomeReads, Look & { text?: string }> = {
  better: { tone: 'success', icon: 'check' },
  worse: { tone: 'danger', icon: 'alert' },
  moved: { tone: 'quiet' },
  same: { tone: 'quiet' },
  few: { tone: 'quiet', text: 'Too few to compare yet' },
  none: { tone: 'quiet', text: 'Nothing to compare yet' },
};

// Where an MCP server an agent was given stands, as its provider reports it; a status it does not name shows as itself.
const MCP_LOOK: Record<string, Look & { label: string }> = {
  connected: { tone: 'success', icon: 'check', label: 'Connected' },
  failed: { tone: 'danger', icon: 'alert', label: 'Failed' },
  'needs-auth': { tone: 'needs', icon: 'key', label: 'Needs sign-in' },
  pending: { tone: 'warning', icon: 'clock', label: 'Pending' },
};
export const mcpLook = (status: string): Look & { label: string } => MCP_LOOK[status] ?? { tone: 'quiet', label: status };
