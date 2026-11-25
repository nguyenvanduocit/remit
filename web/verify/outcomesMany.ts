// The Outcomes page (OutcomesView) at volume, on made-up data: the KPIs of the busy owner's chains and ?n= insights (default
// 500; 5 and 5000 are the other sizes the page has to hold), each naming a few chains, with long and short bodies and some
// saying nothing to change. No server, no sign-in. Open /verify/outcomesMany.html?n=5000. ?quiet=1 is a first week.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import OutcomesView from '../src/components/OutcomesView.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { routes } from '../src/router';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { insightsSeed, outcomesSeed } from './outcomesSeed';
import type { Decision } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, decisionRecord } from './records';

const params = new URLSearchParams(location.search);
const n = Number(params.get('n') ?? 500);
const quiet = !!params.get('quiet');
const HOUR = 3_600_000;
const iso = (ms: number) => new Date(ms).toISOString();

// Many insights, newest first, one every few hours: the seed's three first, then made-up ones of three shapes.
function many(count: number): Decision[] {
  const now = Date.now();
  const base = insightsSeed();
  const out: Decision[] = [...base];
  const shapes = [
    (i: number) => [`Wait in the merge queue rose to ${10 + (i % 9)}h on ${3 + (i % 5)} landings: the ones that waited longest were all infrastructure changes.`, `ch_${(1000 + i).toString(16)}a and ch_${(2000 + i).toString(16)}b each touched a Helm chart, which the owner reads with more care.\n\n**What to change:** have the Chair say in the MR's description what the infrastructure change does and what it leaves alone.`],
    (i: number) => [`Reviewers approved ${i % 7} of ${7 + (i % 3)} changes first time this week; nothing to change.`, `The approved ones were small: under 80 lines each (ch_${(3000 + i).toString(16)}c).`],
    (i: number) => [`Two engineers on sonnet lost their session mid-turn while a test suite ran.`, `ch_${(4000 + i).toString(16)}d and ch_${(5000 + i).toString(16)}e; both resumed by their Chair. Look at the machine's memory.\n\n**What to change:** nothing in a playbook; this is the machine.`],
  ];
  for (let i = 0; out.length < count; i++) {
    const [text, why] = shapes[i % shapes.length](i);
    out.push(decisionRecord({ id: `m${i}`, chainId: 'ch_a11a57', kind: 'insight', by: actorOf('agent', 'chair'), text, why, createdAt: iso(now - (60 + i * 7) * HOUR) }));
  }
  return out.slice(0, count);
}

fakeServer({
  appSnapshot: () => snapshotOf({ insights: quiet ? [] : many(n) }),
  outcomesGet: async (last, window) => outcomesSeed(quiet, last, window),
  analystRun: async () => ({ id: 'ch_a11a57' }) as never,
});
// The app's routes, so every link the view and its tabs name resolves; nothing renders them (no RouterView).
const router = createRouter({ history: createMemoryHistory(), routes });
await router.push('/outcomes');
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [h(OutcomesView)])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
