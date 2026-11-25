// The browser tab's title: what waits for the owner, the page he is on, the app.
export function pageTitle(waiting: number, page: string | undefined): string {
  return (waiting ? `(${waiting}) ` : '') + (page ? `${page} · ` : '') + 'Remit';
}
