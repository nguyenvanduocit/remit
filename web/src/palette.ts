// The palette's rows (CommandPalette, ⌘K): what the browser holds, narrowed by what the owner types. First what waits for him
// in the server's order (Board.waiting), then the places to go (the pages, the Assistant, the chains, playbooks, agents,
// skills and employees the replica holds), then what the browser itself can do (turn on notifications, turn the sound of a new
// wait on or off: sound.ts), and last, once he has
// typed something, his words handed to the Assistant. A row goes somewhere, opens the Assistant or asks the browser; none
// changes anything on the server (ADR-0078).
import type { RouteLocationRaw } from 'vue-router';
import type { Agent, Chain, EmployeeRead, Playbook, ListedSkill, Wait } from './core/generated/types.generated';

export type PaletteAct = { kind: 'wait'; wait: Wait } | { kind: 'go'; to: RouteLocationRaw } | { kind: 'assistant' } | { kind: 'ask'; words: string } | { kind: 'notify' } | { kind: 'sound'; on: boolean };
export type PaletteGroup = 'Needs you' | 'Go to' | 'Browser' | 'Assistant';
export interface PaletteRow {
  id: string;
  group: PaletteGroup;
  label: string;
  // What kind of place it is, or for a wait who it is about and in which chain.
  detail: string;
  act: PaletteAct;
}

// What the palette searches: the replica's lists as the server sent them, the app's pages, and whether the browser may still
// be asked to notify.
export interface PaletteSource {
  waiting: readonly Wait[];
  pages: readonly { name: string; label: string }[];
  chains: readonly Chain[];
  playbooks: readonly Playbook[];
  agents: readonly Agent[];
  skills: readonly ListedSkill[];
  employees: readonly EmployeeRead[];
  canAskToNotify: boolean;
  // Whether this browser can play the sound of a new wait, and whether it does.
  sound: { supported: boolean; on: boolean };
}

// How many places a search shows: the first ones in the order above; typing more narrows them.
export const PLACES_SHOWN = 12;

// Every word typed is in the row's words, in any case.
const matches = (query: string) => {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  return (...text: string[]) => terms.every((t) => text.join(' ').toLowerCase().includes(t));
};

export function paletteRows(query: string, src: PaletteSource): { rows: PaletteRow[]; placesHidden: number } {
  const has = matches(query);
  const titles = new Map(src.chains.map((c) => [c.id, c.title]));
  const waits = src.waiting
    .map((w): PaletteRow => ({ id: `wait:${w.id}`, group: 'Needs you', label: w.ask, detail: [w.who, w.chainId && titles.get(w.chainId)].filter(Boolean).join(' · '), act: { kind: 'wait', wait: w } }))
    .filter((r) => r.act.kind === 'wait' && has(r.act.wait.label, r.label, r.detail));
  const places = [
    ...src.pages.map((p): PaletteRow => ({ id: `page:${p.name}`, group: 'Go to', label: p.label, detail: 'Page', act: { kind: 'go', to: { name: p.name } } })),
    { id: 'assistant', group: 'Go to', label: 'Assistant', detail: 'Panel', act: { kind: 'assistant' } } satisfies PaletteRow,
    ...src.chains.map((c): PaletteRow => ({ id: `chain:${c.id}`, group: 'Go to', label: c.title, detail: 'Chain', act: { kind: 'go', to: { name: 'chain', params: { id: c.id } } } })),
    ...src.playbooks.map((p): PaletteRow => ({ id: `playbook:${p.id}`, group: 'Go to', label: p.name, detail: 'Playbook', act: { kind: 'go', to: { name: 'playbooks', params: { playbookId: p.id } } } })),
    ...src.agents.map((a): PaletteRow => ({ id: `agent:${a.id}`, group: 'Go to', label: a.name, detail: 'Agent', act: { kind: 'go', to: { name: 'agents', params: { agentId: a.id } } } })),
    ...src.skills.map((s): PaletteRow => ({ id: `skill:${s.name}`, group: 'Go to', label: s.name, detail: 'Skill', act: { kind: 'go', to: { name: 'skills', params: { skillName: s.name } } } })),
    ...src.employees.map((e): PaletteRow => ({ id: `employee:${e.employee.id}`, group: 'Go to', label: e.shown, detail: 'Employee', act: { kind: 'go', to: { name: 'employees', params: { id: e.employee.id } } } })),
  ].filter((r) => has(r.label, r.detail));
  const browser: PaletteRow[] = [
    ...(src.canAskToNotify ? [{ id: 'notify', group: 'Browser', label: 'Notify me when something needs me', detail: 'Notifications', act: { kind: 'notify' } } satisfies PaletteRow] : []),
    ...(src.sound.supported
      ? [src.sound.on
          ? ({ id: 'sound', group: 'Browser', label: 'Stop the sound when something needs me', detail: 'Sound · on in this browser', act: { kind: 'sound', on: false } } satisfies PaletteRow)
          : ({ id: 'sound', group: 'Browser', label: 'Play a sound when something needs me', detail: 'Sound · off in this browser', act: { kind: 'sound', on: true } } satisfies PaletteRow)]
      : []),
  ].filter((r) => has(r.label, r.detail));
  const words = query.trim();
  const ask: PaletteRow[] = words ? [{ id: 'ask', group: 'Assistant', label: `Ask the Assistant: “${words}”`, detail: 'Hands your words to the Assistant', act: { kind: 'ask', words } }] : [];
  return { rows: [...waits, ...places.slice(0, PLACES_SHOWN), ...browser, ...ask], placesHidden: Math.max(0, places.length - PLACES_SHOWN) };
}
