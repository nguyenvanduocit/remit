// How the owner comes in with Google, on a server that offers it (ADR-0105): the server says so, and which Firebase
// project to sign in through (GET /auth/config); Google's sign-in window hands this page an ID token; the server checks it
// against Google's keys and the accounts it lets in, and answers the credential this browser keeps, as it keeps the owner
// key (wayIn.ts). Nothing here decides who is let in. Firebase keeps nothing of the sign-in (in-memory persistence):
// Remit's credential is the one this browser holds.
import { reactive } from 'vue';
import type { FirebaseWeb, GoogleSignIn, GoogleSignedIn, SignInConfig } from './core/generated/types.generated';
import { comeInWith } from './wayIn';

// What signing in with Google needs from outside the page: Google's window, which hands an ID token, and the server's check
// of that token. The verify page hands in a fake of both.
export interface GoogleWay {
  popup(): Promise<string>;
  exchange(idToken: string): Promise<Exchanged>;
}

export type Exchanged = { ok: true; signedIn: GoogleSignedIn } | { ok: false; why: string };

// offered: the server offers Google sign-in and Firebase is ready. busy: the window is open or the server is checking.
// error: why the last try did not let the owner in. signedIn: the account that came in, while the page loads again.
export const google = reactive({ offered: false, busy: false, error: '', signedIn: '' });

let way: GoogleWay | null = null;
let enter: (credential: string) => void = comeInWith;

/** Offers Google sign-in through `w`; `onIn` keeps the credential the server answered (by default: and comes in). */
export function offerGoogle(w: GoogleWay, onIn: (credential: string) => void = comeInWith): void {
  way = w;
  enter = onIn;
  google.offered = true;
}

/** Asks the server whether it offers Google sign-in and, when it does, sets Firebase up before the owner clicks, so
 * Google's window opens on the click itself (a window opened later is blocked). A server that does not answer offers
 * none: the owner key's way in stays. */
export async function loadGoogle(): Promise<void> {
  if (google.offered) return;
  let config: SignInConfig;
  try {
    const res = await fetch('/auth/config', { cache: 'no-store' });
    if (!res.ok) return;
    config = (await res.json()) as SignInConfig;
  } catch {
    return;
  }
  if (!config.google) return;
  try {
    offerGoogle(await firebaseWay(config.google));
  } catch (e) {
    google.error = `Google sign-in could not be set up: ${e instanceof Error ? e.message : String(e)}`;
  }
}

/** Opens Google's window and, once the server lets the account in, keeps its credential. */
export async function signInWithGoogle(): Promise<void> {
  if (!way || google.busy) return;
  google.busy = true;
  google.error = '';
  try {
    const got = await way.exchange(await way.popup());
    if (got.ok) {
      google.signedIn = got.signedIn.email;
      enter(got.signedIn.credential);
    } else {
      google.error = got.why;
    }
  } catch (e) {
    google.error = popupFailure(e);
  } finally {
    google.busy = false;
  }
}

// What the owner is told when Google's window did not hand a token; nothing when he closed it himself.
function popupFailure(e: unknown): string {
  const code = typeof e === 'object' && e !== null && 'code' in e ? String(e.code) : '';
  switch (code) {
    case 'auth/popup-closed-by-user':
    case 'auth/cancelled-popup-request':
      return '';
    case 'auth/popup-blocked':
      return "The browser blocked Google's sign-in window: allow pop-ups for this page and try again.";
    case 'auth/unauthorized-domain':
      return `${location.hostname} is not among the Firebase project's authorized domains: add it in the Firebase console (Authentication, Settings), then try again.`;
    default:
      return `Google sign-in failed: ${code || (e instanceof Error ? e.message : String(e))}`;
  }
}

// Google's window through Firebase, and the server's check of what it hands.
async function firebaseWay(config: FirebaseWeb): Promise<GoogleWay> {
  const [{ initializeApp }, fa] = await Promise.all([import('firebase/app'), import('firebase/auth')]);
  const auth = fa.initializeAuth(initializeApp(config), {
    persistence: fa.inMemoryPersistence,
    popupRedirectResolver: fa.browserPopupRedirectResolver,
  });
  return {
    async popup() {
      const provider = new fa.GoogleAuthProvider();
      provider.setCustomParameters({ prompt: 'select_account' });
      const { user } = await fa.signInWithPopup(auth, provider);
      const token = await user.getIdToken();
      await fa.signOut(auth);
      return token;
    },
    exchange,
  };
}

// The server's check of an ID token: the credential it mints, or its reason for refusing.
async function exchange(idToken: string): Promise<Exchanged> {
  const body: GoogleSignIn = { idToken };
  const res = await fetch('/auth/google', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  if (res.ok) return { ok: true, signedIn: (await res.json()) as GoogleSignedIn };
  const why = (await res.text()).trim() || `the server answered ${res.status}`;
  return { ok: false, why: `Remit did not let this Google account in: ${why}.` };
}
