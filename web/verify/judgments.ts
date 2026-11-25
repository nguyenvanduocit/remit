// The Judgments page (JudgmentsView) on made-up data: four questions in the states the page has to hold (judgmentsSeed).
// No server, no sign-in. Open /verify/judgments.html; ?quiet=1 is a server that has asked nothing yet.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import JudgmentsView from '../src/components/JudgmentsView.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { routes } from '../src/router';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { judgmentsSeed } from './judgmentsSeed';
import '../src/theme.css';

const quiet = !!new URLSearchParams(location.search).get('quiet');

fakeServer({
  appSnapshot: () => snapshotOf({}),
  judgmentsGet: async () => judgmentsSeed(quiet),
});
// The app's routes, so every link the view and its tabs name resolves; nothing renders them (no RouterView).
const router = createRouter({ history: createMemoryHistory(), routes });
await router.push('/judgments');
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [h(JudgmentsView)])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
