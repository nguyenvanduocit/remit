// The keys of every box the owner writes in to an agent (a message, a review comment, a reply, a new chain): Enter sends,
// except the Enter that confirms an IME composition; Shift+Enter starts a new line; Esc leaves the box and keeps what is
// typed, as the server keeps a draft.
export function onComposerKey(e: KeyboardEvent, send: () => void, leave: () => void): void {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    send();
  } else if (e.key === 'Escape') {
    leave();
  }
}
