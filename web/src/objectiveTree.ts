// The Objectives page as rows: each objective the owner approved, under it the key results the owner committed to, under
// each the key results that refine it (ADR-0115) and the chains that work toward it, as the tree table lists them top to
// bottom (ObjectivesView). The server holds every fact (the objectives, where
// each key result stands, which key result a chain serves); this nests them, narrows them by the owner's filters and says
// which rows are open. Nothing here is stored or decides a result.
import type { Accountable, Chain, EmployeeRead, KeyResult, KRStatus, ObjectiveStatus, ObjectiveView } from './core/generated/types.generated';
import type { CheckKind } from './core/generated/types.generated';

/** Where an objective stands as the page reads it: its own status, and `stale` for an active one whose horizon has passed (ADR-0031: it closes nothing by itself). */
export type ObjectiveState = ObjectiveStatus | 'stale';
export const OBJECTIVE_STATES: readonly ObjectiveState[] = ['draft', 'active', 'stale', 'closed'];
export const KEY_RESULT_STATES: readonly KRStatus[] = ['met', 'not_met', 'unverified', 'unserved'];
/** What `?status=` takes: nothing narrowed, an objective's state or a key result's. */
export type StatusFilter = 'all' | ObjectiveState | KRStatus;
export const STATUS_FILTERS: readonly StatusFilter[] = ['all', ...OBJECTIVE_STATES, ...KEY_RESULT_STATES];
const isKeyResultState = (s: StatusFilter): s is KRStatus => (KEY_RESULT_STATES as readonly string[]).includes(s);

/** Who answers for or judges something: the owner, or an employee (retired ones still show). */
export interface Person {
  label: string;
  retired: boolean;
}
export type People = ReadonlyMap<string, Person>;
export const peopleOf = (employees: readonly EmployeeRead[]): People => new Map(employees.map((e) => [e.employee.id, { label: e.shown, retired: e.employee.status === 'retired' }]));
const OWNER: Person = { label: 'Owner', retired: false };
export const personOf = (people: People, who: Accountable): Person => (who === 'owner' ? OWNER : (people.get(who) ?? { label: who, retired: false }));

interface Place {
  key: string;
  /** How far in the row stands: an objective 0, a key result one more than what it refines (1 for the owner's), a chain one more than its key result. */
  level: number;
  /** The row it hangs from, in the list (-1 for an objective). */
  parent: number;
  /** Its place among the rows it shares a parent with, and how many those are (what a screen reader says of a row the open ones hide). */
  posinset: number;
  setsize: number;
  /** Whether it has rows under it, and whether they are listed. */
  expandable: boolean;
  expanded: boolean;
  objectiveId: string;
}
export interface ObjectiveRow extends Place {
  kind: 'objective';
  text: string;
  /** The id of the project it belongs to, or '' for none. */
  project: string;
  customer: string;
  state: ObjectiveState;
  owner: Person;
  horizon: { text: string; until: string };
  /** How many of its key results are met, and how many it has (a retired one is neither). */
  met: number;
  total: number;
}
export interface KeyResultRow extends Place {
  kind: 'keyResult';
  /** Its id in its objective, the `?kr=` that picks it. */
  keyResultId: string;
  text: string;
  state: KRStatus;
  /** An amendment dropped it, or what it refines is retired or met: its chains finish and no new one opens. It stays in its objective, counts for nothing and matches no status. */
  retired: boolean;
  /** Why it takes no new chain, its last chains having made no progress; '' when it is not stalled. */
  stalled: string;
  owner: Person;
  judge: Person;
  check?: CheckKind;
}
export interface ChainRow extends Place {
  kind: 'chain';
  chainId: string;
  title: string;
}
export type Row = ObjectiveRow | KeyResultRow | ChainRow;

export interface Filters {
  q: string;
  status: StatusFilter;
  /** A scope label, or '' for every scope. */
  scope: string;
  /** A project's id (Objective.project), or '' for every project and none. */
  project: string;
}

export type ChainFacts = Pick<Chain, 'id' | 'title' | 'serves' | 'archivedAt'>;

/** The chains that work toward each key result, by `<objective>/<key result>`: an archived chain, and one that serves none, are left out. */
export function chainsByKeyResult(chains: readonly ChainFacts[]): Map<string, ChainFacts[]> {
  const byKeyResult = new Map<string, ChainFacts[]>();
  for (const c of chains) {
    if (!c.serves || c.archivedAt) continue;
    const at = `${c.serves.objective}/${c.serves.keyResult}`;
    byKeyResult.set(at, [...(byKeyResult.get(at) ?? []), c]);
  }
  return byKeyResult;
}

export const stateOf = (view: ObjectiveView, now: number): ObjectiveState => {
  const { status, horizon } = view.objective;
  const until = horizon?.until ? Date.parse(horizon.until) : NaN;
  return status === 'active' && until < now ? 'stale' : status;
};

const has = (text: string, terms: readonly string[]) => terms.every((t) => text.includes(t));

interface KeyResultNode {
  kr: KeyResult;
  state: KRStatus;
  /** The key results that refine it, as nodes of their own. */
  children: KeyResultNode[];
  /** The chains listed under it: all it has, or (a search that matched only a chain's title) the ones that matched. */
  chains: ChainFacts[];
  /** It passes the filters, or something under it does. */
  listed: boolean;
  /** It is listed for what is under it (a chain that matched, a key result below), so that shows without a click. */
  viaBelow: boolean;
}

// The objectives the owner reads, newest made first as the server lists them; task objectives (the one of a direct request)
// are not listed. A search keeps a row that matches and the rows above it; an objective or key result that matched on its own
// lists all it holds. A search, or a key result's status, opens the path to what it found.
export function objectiveRows(views: readonly ObjectiveView[], chains: readonly ChainFacts[], people: People, open: ReadonlySet<string>, filters: Filters, now: number): Row[] {
  const terms = filters.q.toLowerCase().split(/\s+/).filter(Boolean);
  const matches = (...parts: (string | undefined)[]) => has(parts.join(' ').toLowerCase(), terms);
  const byKeyResult = chainsByKeyResult(chains);
  const keyResultFilter = isKeyResultState(filters.status) ? filters.status : null;
  const objectiveFilter = filters.status !== 'all' && !keyResultFilter ? filters.status : null;

  const rows: Row[] = [];
  const add = (row: Row) => rows.push(row) - 1;
  for (const view of views) {
    const o = view.objective;
    if (o.task) continue;
    const state = stateOf(view, now);
    if ((objectiveFilter && state !== objectiveFilter) || (filters.scope && (o.scope ?? '') !== filters.scope) || (filters.project && o.project !== filters.project)) continue;
    const owner = personOf(people, o.owner);
    const own = matches(o.outcome, o.customer, o.northStar, o.scope, o.id, owner.label);

    // The view's tree lists parents before children; a node is built from the key results that name it as parent.
    const nodesUnder = (parent: string | undefined, matched: boolean): KeyResultNode[] =>
      view.keyResults
        .filter((kr) => kr.parent === parent)
        .map((kr) => {
          const served = byKeyResult.get(`${o.id}/${kr.id}`) ?? [];
          const krState = view.status[kr.id] ?? 'unserved';
          const stands = !keyResultFilter || (!kr.retired && krState === keyResultFilter);
          const self = matched || matches(kr.text, personOf(people, kr.owner).label, personOf(people, kr.judge).label);
          const children = nodesUnder(kr.id, self);
          const below = children.some((c) => c.listed);
          if (self) return { kr, state: krState, children, chains: served, listed: stands || below, viaBelow: below && keyResultFilter !== null };
          const hit = served.filter((c) => matches(c.title));
          return { kr, state: krState, children, chains: hit, listed: (stands && hit.length > 0) || below, viaBelow: hit.length > 0 || below };
        });
    const nodes = nodesUnder(undefined, own);
    const shown = nodes.filter((n) => n.listed);
    const counted = nodes.filter((n) => !n.kr.retired);
    if (shown.length === 0 && !(own && !keyResultFilter)) continue;

    const objectiveOpen = open.has(o.id) || (shown.length > 0 && (keyResultFilter !== null || !own));
    const at = add({
      kind: 'objective', key: o.id, level: 0, parent: -1, posinset: 0, setsize: 0, objectiveId: o.id,
      expandable: shown.length > 0, expanded: objectiveOpen && shown.length > 0,
      text: o.outcome || o.id, project: o.project ?? '', customer: o.customer ?? '', state, owner,
      horizon: { text: o.horizon?.text ?? '', until: o.horizon?.until ?? '' },
      met: counted.filter((n) => n.state === 'met').length, total: counted.length,
    });
    if (!objectiveOpen) continue;
    const addNodes = (list: KeyResultNode[], parent: number, level: number) => {
      list.forEach((n, i) => {
        const key = `${o.id}/${n.kr.id}`;
        const children = n.children.filter((c) => c.listed);
        const under = children.length + n.chains.length;
        const krOpen = open.has(key) || n.viaBelow;
        const krAt = add({
          kind: 'keyResult', key, level, parent, posinset: i + 1, setsize: list.length, objectiveId: o.id,
          expandable: under > 0, expanded: krOpen && under > 0,
          keyResultId: n.kr.id, text: n.kr.text, state: n.state, retired: !!n.kr.retired, stalled: view.stalled[n.kr.id] ?? '',
          owner: personOf(people, n.kr.owner), judge: personOf(people, n.kr.judge), check: n.kr.check?.kind,
        });
        if (!krOpen) return;
        addNodes(children, krAt, level + 1);
        n.chains.forEach((c, j) =>
          add({ kind: 'chain', key: `chain:${c.id}`, level: level + 1, parent: krAt, posinset: children.length + j + 1, setsize: under, objectiveId: o.id, expandable: false, expanded: false, chainId: c.id, title: c.title }),
        );
      });
    };
    addNodes(shown, at, 1);
  }
  // The objectives are siblings: their place among them is known once all are listed.
  const objectives = rows.filter((r) => r.level === 0);
  objectives.forEach((r, i) => Object.assign(r, { posinset: i + 1, setsize: objectives.length }));
  return rows;
}

/** The scope labels the objectives carry, for the filter that narrows by one. Task objectives have none worth a filter. */
export const scopesOf = (views: readonly ObjectiveView[]): string[] => [...new Set(views.filter((v) => !v.objective.task && v.objective.scope).map((v) => v.objective.scope as string))].sort();

/** What a key does to a row of the tree table: move the focus to a row, open or close one, or do what clicking it does (open a chain, pick an objective or key result). */
export type KeyMove = { focus: string } | { open: string } | { close: string } | { activate: string };

// The treegrid's keys (WAI-ARIA authoring practices): Up and Down walk the rows; Right opens a closed row, else steps into it; Left
// closes an open row, else steps out to the row it hangs from; Home and End go to the first and last row.
export function keyMove(rows: readonly Row[], at: number, key: string): KeyMove | null {
  const row = rows[at];
  if (!row) return null;
  const focus = (i: number): KeyMove | null => (rows[i] && i !== at ? { focus: rows[i].key } : null);
  switch (key) {
    case 'ArrowDown': return focus(at + 1);
    case 'ArrowUp': return focus(at - 1);
    case 'Home': return focus(0);
    case 'End': return focus(rows.length - 1);
    case 'ArrowRight':
      if (row.expandable && !row.expanded) return { open: row.key };
      return row.expanded ? focus(at + 1) : null;
    case 'ArrowLeft':
      if (row.expanded) return { close: row.key };
      return row.parent >= 0 ? focus(row.parent) : null;
    case 'Enter':
    case ' ':
      return { activate: row.key };
    default: return null;
  }
}
