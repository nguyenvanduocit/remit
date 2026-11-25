// What the server's names for where a subscription stands say in the owner's words (ADR-0064): why one is held in place, why a
// reading it judged fired nothing, and why it does not act. One reader of each name, so every screen that shows one says the same.
import type { Control, HoldReason, SkipReason } from './core/generated/types.generated';
import type { Look } from './ui';

export const HOLD_WORDS: Record<HoldReason, string> = {
  firing_failed: 'A firing failed every attempt',
  processor_missing: 'No processor does its action',
};

export const SKIP_WORDS: Record<SkipReason, string> = {
  condition_false: 'Its condition was false',
  condition_error: 'Its condition could not be read',
  own_chain: 'Its own chain caused it',
  collapsed: 'A newer firing took its place',
  overlap: 'A firing of it already waited',
  missed_catchup: 'Past its catch-up window',
  screened: 'Judged not worth a chain',
};

// The badge of a subscription held in place on a failed firing: it waits for the owner, so it takes his orange and says so, apart
// from the state held (a next firing that only waits, ui.ts), which needs nothing of him.
export const HOLD_LOOK: Look & { label: string } = { tone: 'needs', icon: 'alert', label: 'Needs you' };

// The badge of a subscription that does not act, in place of where it stands; an active one (the empty text) has none.
export const CONTROL_LOOK: Record<Exclude<Control, ''>, Look & { label: string }> = {
  paused: { tone: 'quiet', icon: 'pause', label: 'Paused' },
  ended: { tone: 'quiet', icon: 'circle', label: 'Ended' },
};
