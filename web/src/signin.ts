// How this page comes back after the server turned it away as an older build, and the owner key the link carries. Kept
// apart from the core so it can be tested alone; which refusal is final is the core's (State.refused).

const LOADED_AGAIN = 'remit.loadedAgainAt';
const LOAD_AGAIN_EVERY = 60_000; // ms

// Loads the page again for the build the server serves, unless it did so within the last minute: a server that keeps
// refusing this page (a page cached on the way) leaves the banner saying so rather than reload for ever. Whether it did.
export function loadAgain(storage: Pick<Storage, 'getItem' | 'setItem'>, now: number, reload: () => void): boolean {
  const last = Number(storage.getItem(LOADED_AGAIN) ?? 0);
  if (now - last < LOAD_AGAIN_EVERY) return false;
  storage.setItem(LOADED_AGAIN, String(now));
  reload();
  return true;
}

// The owner key the link the server prints carries (#key=<key>, ADR-0026), and the address's hash once it is taken out, so
// that the key leaves the address bar (a copied link, a screenshot) and stays in this browser alone. null: no key.
export function ownerKeyIn(hash: string): { key: string; rest: string } | null {
  const params = new URLSearchParams(hash.replace(/^#/, ''));
  const key = params.get('key');
  if (!key) return null;
  params.delete('key');
  const rest = params.toString();
  return { key, rest: rest ? `#${rest}` : '' };
}
