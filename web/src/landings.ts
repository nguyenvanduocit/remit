// How a landing reads (ADR-0114): the look of each state beside its word, so colour never says it alone, and the words for
// why a landing went back to its Chair, what took it back and how soon its Chair says to read it. Where a landing stands, why
// and in which order the queue takes it are the server's (Landing, RepoView); this only words them. Pure.
import type { Checks, Gate, Landing, LandingState, Priority, ReturnReason, RevertCause } from './core/generated/types.generated';
import { span, type Look } from './ui';

// A landing being combined onto the base or checked turns its ring (StandingBadge's `working`); every other state wears its icon.
export const LANDING_LOOK: Record<LandingState['kind'], Look & { label: string; working?: boolean }> = {
  judging: { tone: 'quiet', icon: 'judge', label: 'Being judged' },
  contending: { tone: 'gate', icon: 'scale', label: 'Awaiting the pick' },
  ready: { tone: 'info', icon: 'clock', label: 'Ready' },
  combining: { tone: 'info', label: 'Combining', working: true },
  checking: { tone: 'info', label: 'Checking', working: true },
  landed: { tone: 'success', icon: 'merge', label: 'Landed' },
  returned: { tone: 'warning', icon: 'arrow-left', label: 'Returned' },
  reverted: { tone: 'danger', icon: 'repeat', label: 'Taken back' },
};

export const RETURN_SAYS: Record<ReturnReason, string> = {
  conflict: 'It conflicts with the base: the queue never resolves a conflict, its Chair does',
  checks: 'The checks failed on it combined with the base',
  empty: 'Combined with the base it changes nothing: its work is there already, or lost',
  replaced: 'A newer version of the change replaced it before it landed',
  refused: 'The forge refused the push',
  withdrawn: 'Withdrawn: its chain was deleted',
  not_picked: 'Not picked in its round',
};

export const REVERT_SAYS: Record<RevertCause, string> = {
  pipeline: "The forge's pipeline failed on the push that carried it, where the base before it passed",
  guardrail: 'A guardrail of its stage breached while it baked',
  overturn: 'On the owner’s word',
  conflict: 'A revert of an earlier landing conflicted with it, so it was taken back first',
};

export const GATE_NAME: Record<Gate, string> = { reviewer: 'Reviewer', qc: 'QC', security: 'Security' };

export const PRIORITY_SAYS: Record<Priority, string> = { high: 'High priority', medium: 'Medium priority', low: 'Low priority' };

// How the forge's checks on a commit stand, in words; a commit it has said nothing of, or runs none on, has none.
export function checksText(c?: Checks): string {
  if (!c) return 'None reported';
  if (c.state === 'failed') return c.failed ? `Failed: ${c.failed}` : 'Failed';
  return c.state === 'passed' ? 'Passed' : 'Running';
}

// The time that says most about where a landing stands: when it was taken back, went back (its last change), landed, became
// ready, else made.
export const landingAt = (l: Landing): string =>
  l.revertedAt ?? (l.state.kind === 'returned' ? l.updatedAt : undefined) ?? l.landedAt ?? l.readyAt ?? l.createdAt;

// A commit as the owner reads it beside its message: its first 8 characters.
export const shortSha = (sha: string): string => sha.slice(0, 8);

// How long a stage bakes, from the contract's nanoseconds.
export const bakeFor = (ns: number): string => span(ns / 1e6);

// A queue, of a repository (its catalog id, as a landing names it) and a base: its name, and its id in the address
// (?queue=) and on the page.
type QueueOf = Pick<Landing, 'repo' | 'base'>;
export const queueName = (q: QueueOf): string => `${q.repo} → ${q.base}`;
export const queueKey = (q: QueueOf): string => `${q.repo}:${q.base}`;
