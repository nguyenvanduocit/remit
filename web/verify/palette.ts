// The palette (⌘K) over the whole signed-in app, on app.ts's made-up data: what needs the owner first (every kind the seed
// holds: a permission, failures, a question, changes to apply, the Assistant's credentials, the archived chain's question;
// ?key=1 leaves a credential alone), then the pages and the chains, playbooks, agents and
// skills to go to, then the browser's own row, and the words typed handed to the Assistant last. Open /verify/palette.html; it
// opens the palette as ⌘K would. ?q=<words> types them; ?shut=1 leaves it closed (to press the keys yourself); the app's own
// parameters (?at=, ?assistant=1, ?wait=<ms>) pass through to app.ts.
import './app';

const params = new URLSearchParams(location.search);
if (!params.get('shut')) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true, bubbles: true }));
  const q = params.get('q');
  if (q) {
    // The palette's box once the dialog has drawn it.
    const box = await new Promise<HTMLInputElement>((found) => {
      const look = () => {
        const el = document.querySelector<HTMLInputElement>('[data-palette-input]');
        if (el) found(el);
        else requestAnimationFrame(look);
      };
      look();
    });
    box.value = q;
    box.dispatchEvent(new Event('input', { bubbles: true }));
  }
}
