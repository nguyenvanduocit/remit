// Fonts ship with the app (Vietnamese included), so it reads the same on every machine.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, watch } from 'vue';
import { createWebHistory } from 'vue-router';
import App from './App.vue';
import { apiKey } from './core/api';
import { keepIn, overPort, randomHex } from './core/client';
import { api, coreState, startCore } from './replica';
import { signedIn } from './session';
import { loadAgain } from './signin';
import { comeIn, takeKeyFromAddress } from './wayIn';
import { appRouter } from './router';
import './theme.css';

// The core runs in a SharedWorker every tab of this browser shares (ADR-0095 decision 4): one session however many tabs
// are open, which dials, and dials again, by itself. The first tab's start starts it, its salt naming the request ids;
// each tab's key is the credential of the next dial.
function connect(key: string): void {
  const salt = randomHex();
  const start = { nodeId: `page-${salt}`, publicKey: '', bundle: __BUNDLE__ || undefined, salt };
  const worker = new SharedWorker(new URL('./core/worker.ts', import.meta.url), { type: 'module', name: 'remit-core' });
  startCore(overPort(worker.port, JSON.stringify(start), key), keepIn(sessionStorage, 'remit.outbox'));
}

takeKeyFromAddress();
// The server serves another build than this page runs (a page left open across a deploy): it loads again. The owner's
// messages that wait for the link are kept (keepIn) and go from the page loaded again, with their request ids.
watch(
  () => coreState.value?.refused,
  (refused) => refused === 'new_bundle' && loadAgain(sessionStorage, Date.now(), () => location.reload()),
);
createApp(App).use(appRouter(createWebHistory())).provide(apiKey, api).mount('#app');
// The owner key this browser holds is the credential of every session. Nothing held: the app shows the way in instead.
const key = comeIn();
if (key) {
  signedIn();
  connect(key);
}
