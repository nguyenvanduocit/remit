// How a person comes in: the owner with the owner key that the link the server printed at start carries (ADR-0026), a
// mentor with the code the owner made for them (ADR-0060), from the link the owner passed on or pasted on the way in.
// Nothing here decides who is let in or what they see: the server checks the credential the core's hello carries and says
// who it is (Snapshot.who).
import { ownerKeyIn } from './signin';

// Where this browser keeps its credential, the owner key or a mentor's code: it is this browser's, not view state.
const KEY = 'remit.key';
// Where it was kept before the browser could hold a mentor's code; moved to KEY once.
const OWNER_KEY = 'remit.ownerKey';

// The credential the address carried when the page loaded (takeKeyFromAddress), until comeIn keeps it.
let linkKey: string | null = null;

// Takes the credential out of the address the page loaded at, if it holds one (#key=, the link the server prints or the
// owner passes on), so that it is in no copied link or screenshot. It runs before the router reads the address, which
// would put it back.
export function takeKeyFromAddress(): void {
  const found = ownerKeyIn(location.hash);
  if (!found) return;
  linkKey = found.key;
  history.replaceState(history.state, '', location.pathname + location.search + found.rest);
}

// Keeps the credential the address carried, if it did, in place of the one kept; then the one kept: the credential each
// session's hello carries. None: this browser holds none to come in with.
export function comeIn(): string | null {
  const before = localStorage.getItem(OWNER_KEY);
  if (before !== null) {
    if (localStorage.getItem(KEY) === null) localStorage.setItem(KEY, before);
    localStorage.removeItem(OWNER_KEY);
  }
  if (linkKey) localStorage.setItem(KEY, linkKey);
  return localStorage.getItem(KEY);
}

/** Keeps a pasted credential and loads the page again to come in with it. */
export function comeInWith(key: string): void {
  localStorage.setItem(KEY, key.trim());
  location.assign('/');
}

/** Lets go of the credential this browser holds and loads the page again, at the way in. */
export function signOut(): void {
  localStorage.removeItem(KEY);
  location.assign('/');
}
