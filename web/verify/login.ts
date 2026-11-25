// The way in (Login) in each of its states, with no server: the page sets what the sign-in would have found. Open
// /verify/login.html?state=<s>: `open` (the default: this browser holds no owner key; open the link the server printed),
// `rejected` (the server refused this browser's owner key), `google` (the server offers Google sign-in: clicking it signs
// in as the owner's account) or `google-refused` (it does, and refuses the account chosen in Google's window). The
// Google states fake Google's window and the server's check; the page stays on the screen it shows after the click.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import Login from '../src/components/Login.vue';
import { offerGoogle, type Exchanged } from '../src/google';
import { fakeServer } from './fake';
import { signedIn } from '../src/session';
import '../src/theme.css';

const state = new URLSearchParams(location.search).get('state');
if (state === 'rejected') {
  signedIn();
  // The real core on a fake server that turns this browser's owner key away, as a server does a revoked key.
  fakeServer({}).refuse('bad_credential', 'bad token');
}
// Google's window takes a moment, as the owner picks an account in it.
const picked = (): Promise<string> => new Promise((resolve) => setTimeout(() => resolve('a-fake-id-token'), 400));
if (state === 'google') {
  const exchange = async (): Promise<Exchanged> => ({ ok: true, signedIn: { credential: 'gs1.fake', email: 'owner@example.com' } });
  offerGoogle({ popup: picked, exchange }, () => {});
}
if (state === 'google-refused') {
  const exchange = async (): Promise<Exchanged> => ({
    ok: false,
    why: 'Remit did not let this Google account in: someone@example.com may not come in.',
  });
  offerGoogle({ popup: picked, exchange }, () => {});
}
createApp(Login).mount('#app');
