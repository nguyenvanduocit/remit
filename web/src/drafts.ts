// The owner's unsent words, browser side. The server keeps them (Draft): the replica of their home holds them (the open
// chain's, or the Assistant's), kept live by pushes, so every page shows the same words and a reload shows them as last
// saved. The core holds what the owner has typed that the server does not have yet (the `typed` model), laid over the
// server's copy until its save is answered, saves it after a pause, keeps a draft's saves and its deletion in the order
// they were made, and sends drafts only once the server has what was typed in them (replica.ts). Read and asked here.
import { computed, toValue, type MaybeRefOrGetter } from 'vue';
import type { Call } from './core/client';
import type { Draft, DraftInput, ImageRef } from './core/generated/types.generated';
import type { Pending } from './core/generated/replica/types.generated';
import { assistant } from './assistant';
import { opened } from './openChain';
import { inOrder } from './core/client';
import { ask, sendDrafts, useModel } from './replica';

const typedRows = useModel<DraftInput>({ model: 'typed' });
const typed = inOrder(typedRows);
// The owner's sends the server has not taken yet (ADR-0094): their drafts leave their box and show as pending rows.
const outbox = inOrder(useModel<Pending>({ model: 'outbox' }));
const outgoing = computed(() => new Set(outbox.value.flatMap((p) => p.drafts)));

/** The owner's sends on their way that wait for the link, for the banner's count. */
export const queued = computed(() => outbox.value.filter((p) => p.status === 'waiting').length);

// An id the page names before the server has the thing (an image being put): 128 random bits.
export const newDraftId = (): string => Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, '0')).join('');

// The words a draft shows: what the owner typed, else what the server has.
export const textOf = (d: { id: string; text: string }): string => typedRows.rows.get(d.id)?.text ?? d.text;

// What the owner writes in a draft (an empty id: a new one): shown at once, saved after a pause. A message left blank is no
// draft.
function write(d: DraftInput): void {
  ask({ ask: 'write', draft: d });
}

const NONE: readonly Draft[] = [];
// The owner's drafts the server keeps for a home, as its replica holds them: none for a home that is not held.
function draftsOf(home: string): readonly Draft[] {
  if (assistant.view?.assistant.id === home) return assistant.view.drafts;
  return opened.view?.chain.chain.id === home ? opened.view.drafts : NONE;
}

// A box that holds one draft: an agent's composer (a chain's, or the Assistant's), by the card the message is to.
export type Box = { home: string; cardId: string };
const inBox = (b: Box) => (d: DraftInput) => d.home === b.home && d.cardId === b.cardId;

// The draft in a box: the words and images typed in it, else the server's; none until the owner writes. A draft being
// sent is not in its box: it shows as a pending row (the box's `pending`).
type Held = { id?: string; text: string; images: ImageRef[] };
function boxDraft(b: Box): Held {
  const mine = (d: DraftInput) => inBox(b)(d) && !outgoing.value.has(d.id);
  for (const d of typed.value) if (mine(d)) return { id: d.id, text: d.text, images: d.images ?? [] };
  const stored = draftsOf(b.home).find(mine);
  return stored ? { id: stored.id, text: stored.text, images: stored.images } : { text: '', images: [] };
}

// Where a box's sends show: its name among the boxes of the page.
const boxKey = (b: Box): string => `${b.home}:${b.cardId}`;

// A box's sends on their way, for wherever they show: under the box, or at the end of the stream they join.
export const pendingIn = (box: MaybeRefOrGetter<Box>) => computed(() => outbox.value.filter((p) => p.at === boxKey(toValue(box))));

// A pending send tried again (it failed), or taken back to edit: its words go back in their box.
export const resend = (id: string): void => ask({ ask: 'resend', id });
export const unsend = (id: string): void => ask({ ask: 'unsend', id });

// Words put in a box for the owner to finish, as a draft the server keeps like his own: a box that holds words of his keeps them.
export function prefill(box: Box, words: string): void {
  const held = boxDraft(box);
  if (held.text.trim()) return;
  write({ ...box, id: held.id ?? '', text: words, images: held.images });
}

// The owner's words and images in a box, saved as he writes them, the send that delivers them by the draft's id (`call` names it),
// and the box's sends on their way (`pending`). `send` never holds the box: the words leave it at once for a pending
// row, and go when the link is up (the core's outbox, ADR-0094). It answers whether they went; when they did not, the
// row says why and the owner tries again or takes them back to edit.
export function useDraftBox(box: MaybeRefOrGetter<Box>, call: (draftId: string) => Call) {
  const current = computed(() => boxDraft(toValue(box)));
  const text = computed({
    get: () => current.value.text,
    set: (words: string) => write({ ...toValue(box), id: current.value.id ?? '', text: words, images: current.value.images }),
  });
  const images = computed({
    get: () => current.value.images,
    set: (images: ImageRef[]) => write({ ...toValue(box), id: current.value.id ?? '', text: current.value.text, images }),
  });
  const pending = pendingIn(box);
  function send(): Promise<boolean> {
    const id = current.value.id;
    const words = text.value.trim();
    const shown = words || (images.value.length ? `${images.value.length === 1 ? 'An image' : `${images.value.length} images`}` : '');
    if (!id || !shown) return Promise.resolve(false);
    return sendDrafts([id], call(id), boxKey(toValue(box)), shown).then(
      () => true,
      () => false,
    );
  }
  return { text, images, pending, send };
}
