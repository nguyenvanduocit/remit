// The app's keys, on every page: ⌘K (Ctrl+K) opens the palette even while typing; and, when the owner is not typing in a box,
// `n` opens the next thing that waits for him, `g b` the board, `g a` the Assistant, `?` the list of keys, `j` and `k` move
// down and up the rows of the list he is in (listKeys.ts), Enter opens the row (its own link), Esc goes back to the row.
export type KeyAct = 'palette' | 'next' | 'board' | 'assistant' | 'keys' | 'down' | 'up' | 'back';

// The keys as the list of keys shows them, in the order it shows them.
export const KEYS: readonly { keys: string[]; says: string }[] = [
  { keys: ['⌘', 'K'], says: 'Find what waits, a chain or a page, or ask the Assistant' },
  { keys: ['n'], says: 'Open the next thing that needs you' },
  { keys: ['g', 'b'], says: 'Go to the board' },
  { keys: ['g', 'a'], says: 'Open the Assistant' },
  { keys: ['j', 'k'], says: 'Move down and up the list you are in' },
  { keys: ['↵'], says: 'Open the row you are on' },
  { keys: ['Esc'], says: 'Back to the row you opened' },
  { keys: ['?'], says: 'Show these keys' },
];

// How long after `g` its second key is waited for.
export const CHORD_MS = 1000;

type Press = Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'altKey'>;

// What a key press does, given whether the owner is typing and whether `g` was just pressed: the act, if any, and whether the
// next key completes a `g` chord.
export function keyAct(e: Press, typing: boolean, afterG: boolean): { act?: KeyAct; afterG: boolean } {
  if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === 'k') return { act: 'palette', afterG: false };
  if (typing || e.metaKey || e.ctrlKey || e.altKey) return { afterG: false };
  if (afterG && e.key === 'b') return { act: 'board', afterG: false };
  if (afterG && e.key === 'a') return { act: 'assistant', afterG: false };
  if (e.key === 'g') return { afterG: true };
  if (e.key === 'n') return { act: 'next', afterG: false };
  if (e.key === '?') return { act: 'keys', afterG: false };
  if (e.key === 'j') return { act: 'down', afterG: false };
  if (e.key === 'k') return { act: 'up', afterG: false };
  if (e.key === 'Escape') return { act: 'back', afterG: false };
  return { afterG: false };
}

// Whether a key press lands where the owner types (a box, a field, an editable area) or inside a dialog, where single keys
// are its own.
export function typingAt(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || !!target.closest('input, textarea, select, [role="dialog"], [role="combobox"]');
}
