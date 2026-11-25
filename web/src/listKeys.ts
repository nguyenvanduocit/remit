// Walking a list from the keyboard (j, k, Esc: appKeys.ts). A list is an element marked `data-rows` (a board's column, the list
// beside a page, the Assistant's feed); a row is an element in it marked `data-row` (a card's link, a side list's link, an item
// that waits on the owner). j and k move the focus to the next or the previous row shown, in the list the focus is in, else in
// the first list of the page's main region; Enter is the row's own (a link opens). Esc gives the focus back to the row last
// walked to, from wherever the owner went after opening it; when opening it drew the list anew (a chain opened from the board
// shows its column beside it), the row with the same link. Only the focus moves: what is open stays in the address.
const ROW = '[data-row]';
const LIST = '[data-rows]';

// The row last walked to, and its link, which finds it again once the list is drawn anew.
let last: HTMLElement | undefined;
let lastHref: string | null = null;

const shown = (el: Element) => el.getClientRects().length > 0;
// The row last walked to, as it is on the page now.
function lastRow(): HTMLElement | undefined {
  if (last?.isConnected) return last;
  const again = lastHref ? [...document.querySelectorAll<HTMLElement>(ROW)].find((r) => r.getAttribute('href') === lastHref && shown(r)) : undefined;
  return (last = again);
}

function focusRow(row: HTMLElement): void {
  if (!row.matches('a[href], button, [tabindex]')) row.tabIndex = -1;
  row.focus();
  row.scrollIntoView({ block: 'nearest' });
  last = row;
  lastHref = row.getAttribute('href');
}

// Moves the focus `step` rows from the one it is on; from no row, to the open one (aria-current) or the list's first or last.
export function walkRows(step: 1 | -1, at: Element | null): boolean {
  const on = at?.closest<HTMLElement>(ROW) ?? undefined;
  const before = lastRow();
  const list = at?.closest<HTMLElement>(LIST) ?? before?.closest<HTMLElement>(LIST) ?? document.querySelector<HTMLElement>(`main ${LIST}`);
  if (!list) return false;
  const rows = [...list.querySelectorAll<HTMLElement>(ROW)].filter(shown);
  if (!rows.length) return false;
  const from = on && rows.includes(on) ? on : before && rows.includes(before) && !list.contains(at) ? before : undefined;
  if (from) {
    const next = rows[rows.indexOf(from) + step];
    if (next) focusRow(next);
    return true;
  }
  const open = rows.find((r) => r.getAttribute('aria-current') === 'page');
  focusRow(open ?? (step === 1 ? rows[0] : rows[rows.length - 1]));
  return true;
}

// Gives the focus back to the row last walked to, when it is elsewhere; whether it did.
export function backToRow(at: Element | null): boolean {
  const row = lastRow();
  if (!row || !shown(row) || row.contains(at)) return false;
  focusRow(row);
  return true;
}
