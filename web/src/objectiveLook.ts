// How the Objectives page says where something stands and who answers, one place for the tree table and the details beside it: the
// words of each state, its tone and its icon, so colour is never all it says.
import type { Check, CheckKind, Horizon } from './core/generated/types.generated';
import type { Person, StatusFilter } from './objectiveTree';
import { timed, type Tone } from './ui';

export interface Look {
  label: string;
  tone: Tone;
  icon: string;
}
export const LOOK: Record<Exclude<StatusFilter, 'all'>, Look> = {
  draft: { label: 'Draft', tone: 'needs', icon: 'pencil' },
  active: { label: 'Active', tone: 'info', icon: 'play' },
  stale: { label: 'Stale', tone: 'warning', icon: 'clock' },
  closed: { label: 'Closed', tone: 'quiet', icon: 'archive' },
  met: { label: 'Met', tone: 'success', icon: 'check' },
  not_met: { label: 'Not met', tone: 'danger', icon: 'alert' },
  unverified: { label: 'Unverified', tone: 'warning', icon: 'eye' },
  unserved: { label: 'Unserved', tone: 'quiet', icon: 'dot' },
};
// A key result an amendment dropped says so in its status cell, in place of where it stood.
export const RETIRED: Look = { label: 'Retired', tone: 'quiet', icon: 'x' };
export const CHECK: Record<CheckKind, string> = { submit: 'landed change', reading: 'signal reading', command: 'command', unverifiable: 'unverifiable' };
/** How a key result is judged, in words; a key result with no check declared yet says so. */
export const checkText = (kind?: CheckKind) => (kind ? CHECK[kind] : 'no check yet');

export const personText = (p: Person) => (p.retired ? `${p.label} · retired` : p.label);

const dateOf = (iso?: string) => (timed(iso) ? new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' }) : '');
/** How far ahead an objective looks: the words it was given and the date they fall on. */
export const horizonText = (h?: Horizon) => [h?.text, dateOf(h?.until)].filter(Boolean).join(' · ');

/** The facts a check holds, as the label and value of each that is set: each kind holds exactly its own fields. */
export function checkFacts(check?: Check): [label: string, value: string][] {
  if (!check) return [['Kind', checkText()]];
  const facts: [string, string | undefined][] = [['Kind', CHECK[check.kind]]];
  if (check.kind === 'submit') facts.push(['How', check.how]);
  if (check.kind === 'reading') facts.push(['Signal', check.signal], ['Condition', check.condition], ['Window', check.window], ['Expires', dateOf(check.expiry)]);
  if (check.kind === 'command') facts.push(['Repository', check.repo], ['Command', check.name]);
  if (check.kind === 'unverifiable') facts.push(['Reason', check.reason]);
  return facts.flatMap(([label, value]) => (value ? [[label, value] as [string, string]] : []));
}
