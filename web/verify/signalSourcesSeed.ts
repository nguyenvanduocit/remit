// The sources of signals the server lists first on the Signals board (crates/orch/src/signal_sources.rs), in its words and order,
// counted from made-up signals and subscriptions the way the server counts them, for the verify pages and the tests.
import type { SignalLine, SignalSource, SignalSourceLine, SubscriptionLine } from '../src/core/generated/types.generated';

const SPECS: Omit<SignalSourceLine, 'signals' | 'subscriptions'>[] = [
  { id: 'source:clock', source: 'clock', title: 'Time', pattern: 'schedule:<subscription>', what: 'Time itself: a subscription keeps its own schedule (a cron, an interval or a moment), and each instant that comes fires it, recorded under its key at that instant. Nothing is stored before: time is no signal to read, only what it fired. An instant missed while the server was down fires once when it is back, then the next counts from then.', who: 'A Chair, to check back on its chain at a time; each routine, to start a chain.' },
  { id: 'source:event', source: 'event', title: 'Events', pattern: 'event:<name>', what: "One reading for each event that comes in, whichever way: a post to the signal's door, `remit emit`, a sensor reading a source, an agent's emit_event. It is stored before its writer is answered; its value is what arrived and what Remit found in it.", who: "The signal's own rules, in order: skip an event, tell a chain's Chair, or hand it to a Dispatcher. A chain cannot subscribe to one." },
  { id: 'source:landing', source: 'landing', title: 'Landings', pattern: 'chain:<chain>.landing.<repository>', what: "How a chain's change stands on its way onto its base: the landing, its state (judging, ready, checking, landed, returned, reverted), the version land submitted, the commit it landed as, the forge's checks on that commit, the latest reviewer and QC verdict on it and who made the version; a reading each time one of them moves, with what moved it.", who: 'The Chair of that chain, and of each chain that serves the same objective, to wake when the change lands or comes back.' },
  { id: 'source:live', source: 'live', title: 'Change is live', pattern: 'chain:<chain>.live', what: 'The change of a chain is live, recorded with who said so: the owner through the Assistant, or a rule of the event signal his deploys write.', who: 'The Chair of that chain only, to verify the effect once the change is live.' },
  { id: 'source:sensor', source: 'sensor', title: 'Sensors', pattern: 'chain:<chain>.sensor.<name>', what: "The state a sensor reads for a chain: a script the Chair made, run every few minutes behind a source's connection while a watch on its key is open, or the checks a forge's pipeline ran on what a chain landed, which Remit reads from its forge (`landing`). Each reading is a state that differs from the one before.", who: 'The Chair of that chain only, which makes a script sensor (create_sensor) and watches its key, to wake when the state meets its condition.' },
  { id: 'source:fact', source: 'fact', title: 'Facts', pattern: 'fact:<noun>.<verb>', what: "One reading for each change of Remit's own state that something answers: a chain queued, a finish requested, a check stated. The value names what it is about, never words of the work; the step that changes the state stores it.", who: 'The subscriptions that ship with Remit, to hire a judge; the owner, through the Assistant, to start a chain or tell a Chair.' },
];

// The source of a key, as the server reads it.
export function sourceOfKey(key: string): SignalSource {
  if (key.startsWith('schedule:')) return 'clock';
  if (key.startsWith('fact:')) return 'fact';
  if (key.startsWith('event:')) return 'event';
  if (/^chain:[^.]+\.landing\./.test(key)) return 'landing';
  if (/^chain:[^.]+\.live$/.test(key)) return 'live';
  if (/^chain:[^.]+\.sensor\./.test(key)) return 'sensor';
  return '';
}

// The sources, counting signals (every one, listed or not: `unlisted` adds the ones a capped board leaves out, by source) and
// the lines that listen to each.
export function signalSourcesSeed(signals: readonly SignalLine[], lines: readonly SubscriptionLine[], opts: { unlisted?: Partial<Record<SignalSource, number>> } = {}): SignalSourceLine[] {
  return SPECS.map((spec): SignalSourceLine => {
    const own = signals.filter((s) => s.source === spec.source);
    const subscriptions = lines.filter((l) => l.source === spec.source).length;
    const lastRead = own.map((s) => s.at).filter((at): at is string => !!at).sort().at(-1);
    return {
      ...spec,
      signals: own.length + (opts.unlisted?.[spec.source] ?? 0),
      subscriptions,
      ...(lastRead ? { lastRead } : {}),
    };
  });
}
