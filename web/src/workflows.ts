// A workflow run's steps as the owner reads them (WorkflowRun): what each call asked for, read from its input. The input is the
// script's own value (ADR-0073: wait's {at} or {key, condition, until}, askOwner's {question}, agent's {prompt}, …), stored as
// the script gave it, so it is narrowed here, once, at the edge of the screen. Nothing here decides anything. Pure.
import type { WorkflowEngine, WorkflowStatus, WorkflowStep, WorkflowVerb } from './core/generated/types.generated';
import type { Look } from './ui';

export const ENGINE_LABEL: Record<WorkflowEngine, string> = { claude: 'Claude Code', remit: 'Remit' };
export const ENGINE_TIP: Record<WorkflowEngine, string> = {
  claude: 'Claude Code runs this script inside the process of the card that started it; the run ends with that process.',
  remit: 'Remit runs this script itself: it waits on times, signals and you, and goes on after a restart.',
};

export const VERB_LABEL: Record<WorkflowVerb, string> = {
  agent: 'Agent', hire: 'Hire', wait: 'Wait', ask_owner: 'Asks you', accept: 'Accept', send_back: 'Send back', log: 'Log', phase: 'Phase', workflow: 'Script',
};

/** A run that has not ended: the owner can still stop it. */
export const open = (s: WorkflowStatus): boolean => s === 'running' || s === 'waiting' || s === 'paused';

// Where a step stands, by its mark and the words a screen reader says: done, failed, or still pending, which reads by what it
// waits on (a time or a signal, the owner, a card at work) and by its run (a paused run's steps wait for it).
export function stepLook(step: WorkflowStep, run: WorkflowStatus): Look & { label: string; working?: boolean } {
  if (step.status === 'done') return { tone: 'success', icon: 'check', label: 'Done' };
  if (step.status === 'failed') return { tone: 'danger', icon: 'alert', label: 'Failed' };
  if (run === 'paused') return { tone: 'quiet', icon: 'pause', label: 'Paused' };
  if (step.verb === 'wait') return { tone: 'gate', icon: 'clock', label: 'Waiting' };
  if (step.verb === 'ask_owner') return { tone: 'needs', icon: 'help', label: 'Waits for you' };
  return { tone: 'info', working: true, label: 'At work' };
}

/** What a step asked for: words to read, the time a wait comes due, or the signal it waits on. */
export interface StepAsks {
  text: string;
  at?: string;
  signal?: { key: string; condition: string; until?: string };
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const str = (r: Record<string, unknown>, k: string): string => (typeof r[k] === 'string' ? r[k] : '');

// The field of each verb's input that says what it asked for.
const SAYS: Partial<Record<WorkflowVerb, string>> = { agent: 'prompt', hire: 'brief', ask_owner: 'question', accept: 'notes', send_back: 'feedback', log: 'message', phase: 'title' };

export function stepAsks(step: WorkflowStep): StepAsks {
  const input = isRecord(step.input) ? step.input : {};
  if (step.verb === 'wait') {
    const at = str(input, 'at');
    if (at) return { text: '', at };
    const key = str(input, 'key');
    if (key) return { text: '', signal: { key, condition: str(input, 'condition'), until: str(input, 'until') || undefined } };
  }
  const field = SAYS[step.verb];
  return { text: field ? str(input, field) : '' };
}

/** A run's result as text: a string as it is, any other value as indented JSON. */
export const resultText = (v: unknown): string => (typeof v === 'string' ? v : JSON.stringify(v, null, 2));
