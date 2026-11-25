// A question the Chair asked, as the owner reads it in his inbox (InboxCallout): the suggested answers and Other, which swaps
// every button for a field and Send; a question with no suggestions offers Other alone. Beside them, permissions as the server
// words them (PermissionAsk): a high-risk one whose Allow is held, a low-risk read, a long command that folds, and one sent
// before the server put its calls in words. Pressing Send or an answer logs it to the console.
// Open /verify/inboxquestion.html; ?fail=<ms> fails each answer that late; ?width=<px> narrows the column the items sit in.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import InboxCallout from '../src/components/InboxCallout.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { fakeServer, snapshotOf } from './fake';
import type { InboxItem } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { inboxRecord, optionOf, permissionAsk } from './records';

const asked = (id: string, extra: Partial<InboxItem>): InboxItem => inboxRecord({ id, home: 'ch_1', cardId: 'c_chair', kind: 'question', title: 'How long should old events be kept?', detail: 'The table holds **4 GB**. Keep 30 days, 90 days, or everything?', status: 'open', createdAt: new Date().toISOString(), ...extra });
const allowRefuse = [optionOf('a', 'Allow', 'allow_once'), optionOf('r', 'Refuse', 'reject_once')];
const items = [
  asked('i_q1', { options: [optionOf('o1', '30 days'), optionOf('o2', '90 days'), optionOf('o3', 'Keep everything')] }),
  asked('i_q2', { title: 'Which region should the replica run in?', detail: 'No suggestion: the Chair has no basis to choose.' }),
  asked('i_p1', { kind: 'permission', cardId: 'c_eng', title: 'Drop the events_old table on production', detail: '', options: allowRefuse,
    permission: permissionAsk({ action: 'Drop the events_old table on production', reason: 'It removes what cannot be brought back and touches production, so it runs only if it is allowed.', traits: [{ kind: 'irreversible', says: 'cannot be undone' }, { kind: 'production', says: 'production' }, { kind: 'network', says: 'network' }], risk: { kind: 'high', says: 'High risk' }, tool: 'Bash',
      command: 'psql "host=prod-db.internal dbname=events user=remit_writer sslmode=require" -c "begin; drop table if exists events_old cascade; delete from schema_migrations where version = \'20261101_events_old\'; commit;"' }) }),
  asked('i_p2', { kind: 'permission', cardId: 'c_eng', title: 'Read the nginx config', detail: '', options: allowRefuse,
    permission: permissionAsk({ action: 'Read /etc/nginx/nginx.conf', reason: 'It reads outside its working directory, so it runs only if it is allowed.', traits: [], risk: { kind: 'low', says: 'Low risk' }, tool: 'Read', command: '/etc/nginx/nginx.conf' }) }),
  asked('i_p3', { kind: 'permission', cardId: 'c_eng', title: 'Run npm publish', detail: 'npm publish --access public', options: allowRefuse }),
];
const width = new URLSearchParams(location.search).get('width') ?? '440';
// ?fail=<ms> answers every answer with a failure that late, to see a held Allow come back after it fails.
const fail = Number(new URLSearchParams(location.search).get('fail') ?? 0);
fakeServer({
  appSnapshot: () => snapshotOf({ inbox: items }),
  inboxAnswer: async (...args) => {
    console.log('inboxAnswer', JSON.stringify(args));
    if (!fail) return {};
    await new Promise((r) => setTimeout(r, fail));
    throw new Error('the server is restarting');
  },
});
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { style: { width: `${width}px`, padding: '16px', display: 'grid', gap: '12px' } }, items.map((item) => h(InboxCallout, { item, key: item.id })))) })
  .use(createRouter({ history: createMemoryHistory(), routes }))
  .provide(apiKey, api)
  .mount('#app');
