// The server's drafts for a verify page: the table is kept in this browser's localStorage under the page's own key, so
// a reload of the page, or a second tab of it, finds what the server would hold; each change is pushed to the page and
// to its other tabs (over a BroadcastChannel), as the server pushes it to every connection. It does what the server
// does with a draft: a save made on the revision stored changes its words at its next one, a save made on a copy another
// tab has saved over since (or a tab's first in a composer that holds a draft) is refused CONFLICT with the
// draft as it is, a sent or deleted draft is ended and a save of it after is refused.
import type { Draft, DraftGone, DraftInput } from '../src/core/generated/types.generated';
import { draftRecord } from './records';

type Push = { (name: 'draftUpdated', d: Draft): void; (name: 'draftDeleted', g: DraftGone): void };

export function fakeDrafts(key: string, push: Push, seed: Draft[] = []) {
  const table = `verify.${key}.drafts`;
  const channel = new BroadcastChannel(table);
  type Row = Draft & { ended?: boolean };
  const rows = (): Row[] => JSON.parse(localStorage.getItem(table) ?? 'null') ?? seed;
  const keep = (all: Row[]) => localStorage.setItem(table, JSON.stringify(all));
  channel.onmessage = ({ data }) => (data.gone ? push('draftDeleted', data.gone) : push('draftUpdated', data.draft));
  const tell = (m: { draft: Draft } | { gone: DraftGone }) => {
    channel.postMessage(m);
    if ('gone' in m) push('draftDeleted', m.gone);
    else push('draftUpdated', m.draft);
  };
  const end = (ids: string[]) => {
    const all = rows();
    for (const r of all) if (ids.includes(r.id) && !r.ended) (r.ended = true), tell({ gone: { id: r.id, home: r.home } });
    keep(all);
  };
  return {
    // The chain's live drafts, oldest first: what its view carries.
    live: (home: string): Draft[] => rows().filter((r) => r.home === home && !r.ended).map(({ ended: _, ...d }) => d),
    get: (id: string): Draft | undefined => rows().find((r) => r.id === id && !r.ended),
    end,
    draftSave: (d: DraftInput, baseRev: number): Draft => {
      const all = rows();
      if (all.some((r) => r.id === d.id && r.ended)) throw { code: 'ENDED', message: 'this draft was sent or deleted already' };
      // The draft as the server reads what was sent: what it leaves out is empty.
      const asked = draftRecord({ ...d, updatedAt: new Date().toISOString() });
      const box = (r: Row) => r.home === asked.home && r.cardId === asked.cardId && !r.ended;
      const held = all.find((r) => r.id === d.id) ?? all.find(box);
      if (held && (held.id !== d.id || held.rev !== baseRev)) {
        const { ended: _, ...current } = held;
        throw { code: 'CONFLICT', message: 'the draft changed on another page since this one read it: nothing was saved', detail: { kind: 'draft_conflict', current } };
      }
      const saved: Draft = held ? { ...held, text: d.text, attachments: d.attachments, rev: held.rev + 1, updatedAt: asked.updatedAt } : asked;
      keep([...all.filter((r) => r.id !== saved.id), saved]);
      tell({ draft: saved });
      return saved;
    },
    draftDelete: (id: string) => (end([id]), {}),
  };
}
