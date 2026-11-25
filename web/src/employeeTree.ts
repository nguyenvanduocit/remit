// Who answers to whom among the employees, as the Crew page's outline lists them. `reportsTo` is the whole structure (the
// owner, or the id of an employee); nothing here decides a place, this nests what the server stored and lists it as the
// outline draws it (Outline, TreeRow).
import type { EmployeeRead, Title } from './core/generated/types.generated';

/** One line of the outline: an employee, how far in it stands, the row of its manager (-1 for a root) and how many employees hang under it, folded or not. */
export interface EmployeeRow {
  employee: EmployeeRead;
  level: number;
  parent: number;
  under: number;
}

interface Branch {
  employee: EmployeeRead;
  reports: Branch[];
  under: number;
}

// Each employee is listed once, in the order the server lists them (oldest hired first), under its manager. A root is one
// that reports to the owner or to a manager the replica does not hold; one caught in a loop of managers (the server never
// stores one) is listed as a root rather than lost. A retired manager keeps its place, with its reports under it. With
// `hideRetired`, a retired employee stays only where an active one hangs under it; with a search (`q`, every word in its label,
// area, id or title), one that does not match stays only where one that does hangs under it.
export function employeeOutline(employees: readonly EmployeeRead[], view: { folded: ReadonlySet<string>; hideRetired: boolean; q?: string }): EmployeeRow[] {
  const terms = (view.q ?? '').toLowerCase().split(/\s+/).filter(Boolean);
  const matches = ({ employee: e }: EmployeeRead) => {
    const text = [e.label, e.scope, e.id, TITLE[e.title].label].join(' ').toLowerCase();
    return terms.every((t) => text.includes(t));
  };
  const byId = new Map(employees.map((e) => [e.employee.id, e]));
  const reportsOf = new Map<string, EmployeeRead[]>();
  for (const e of employees) {
    const list = reportsOf.get(e.employee.reportsTo);
    if (list) list.push(e);
    else reportsOf.set(e.employee.reportsTo, [e]);
  }
  const listed = new Set<string>();
  const grow = (employee: EmployeeRead): Branch | null => {
    listed.add(employee.employee.id);
    const reports = (reportsOf.get(employee.employee.id) ?? []).flatMap((r) => (listed.has(r.employee.id) ? [] : (grow(r) ?? [])));
    if (!reports.length && ((view.hideRetired && employee.employee.status === 'retired') || !matches(employee))) return null;
    return { employee, reports, under: reports.reduce((n, r) => n + 1 + r.under, 0) };
  };
  const isRoot = ({ employee: e }: EmployeeRead) => e.reportsTo === e.id || !byId.has(e.reportsTo);
  const roots = employees.filter(isRoot).flatMap((e) => grow(e) ?? []);
  const lost = employees.filter((e) => !listed.has(e.employee.id)).flatMap((e) => (listed.has(e.employee.id) ? [] : (grow(e) ?? [])));

  const rows: EmployeeRow[] = [];
  const add = (b: Branch, level: number, parent: number) => {
    const at = rows.push({ employee: b.employee, level, parent, under: b.under }) - 1;
    if (!view.folded.has(b.employee.employee.id)) b.reports.forEach((r) => add(r, level + 1, at));
  };
  [...roots, ...lost].forEach((b) => add(b, 0, -1));
  return rows;
}

/** What each title is called and drawn as: always text beside the icon, never a colour alone. */
export const TITLE: Readonly<Record<Title, { label: string; icon: string }>> = {
  chair: { label: 'Chair', icon: 'key' },
  judge: { label: 'Judge', icon: 'eye' },
  worker: { label: 'Worker', icon: 'gear' },
};
