// A chain's decisions as its history draws them (CardDecisions), newest first: an overturn of a decision that is listed is drawn
// under that decision rather than as a line of its own, so `rows` is also what the history counts. `overturnOf` holds each such
// overturn by the id of the decision it overturns. Pure.
import type { Decision, DecisionKind } from './core/generated/types.generated';

// The icon a decision is drawn with, by its kind; a kind not named here is a dot.
export const decisionIcon: Partial<Record<DecisionKind, string>> = { hire: 'plus', route: 'arrow-right', ask: 'help', withdraw: 'check', pause: 'pause', resume: 'play', finish: 'stop', stop: 'stop', seed: 'dot', accept: 'check', rework: 'pencil', verdict: 'check', live: 'eye', landing: 'merge', goal: 'spark', repo: 'link', offer: 'check', handover: 'arrow-right', outside: 'check', deadline: 'clock', cancel: 'stop', dispute: 'alert', duel: 'scale', pick: 'scale', overturn: 'interrupt', dissent: 'alert', workflow: 'list', archive: 'archive', unarchive: 'archive', reopen: 'play', release: 'arrow-up' };

export function decisionRows(decisions: readonly Decision[]): { rows: Decision[]; overturnOf: Map<string, Decision> } {
  const ids = new Set(decisions.map((d) => d.id));
  const overturnOf = new Map<string, Decision>();
  for (const d of decisions) if (d.kind === 'overturn' && d.overturns && ids.has(d.overturns)) overturnOf.set(d.overturns, d);
  const rows = [...decisions].reverse().filter((d) => !(d.kind === 'overturn' && d.overturns && ids.has(d.overturns)));
  return { rows, overturnOf };
}
