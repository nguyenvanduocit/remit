// The products the made-up owner works on (ADR-0108): two active projects, one with long notes, three repositories and
// connections, the other with a long name and nothing but a customer, and an archived one an old chain still names. Which
// chain is on which, as the server would name it in ChainStanding.project (a chain not listed is on none), and the history
// of the first, as changes_get would answer it. The Projects tab, the board's project filter, the chain cards and the
// Objectives tab read them (verify/app.ts, verify/objectivesMany.ts).
import type { Change, Project } from '../src/core/generated/types.generated';
import { actorOf, changeRecord, projectRecord } from './records';

const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();

export const STOREFRONT = 'p_storefront';
export const GAMES = 'p_games';
export const BILLING = 'p_billing';

const BRIEF = `The storefront is where shop owners sell: product pages, the cart, the checkout and the coupon field.

- **Runs on** Kubernetes in \`eu-west\`, deployed by \`prod-build-deploy\` on every merge to \`main\`.
- **What matters**: checkout must never charge twice, and a page loads in under two seconds on a phone.
- **Who to ask**: the payments team owns the checkout; anything about coupons goes through Lan.

Read the error rate in Grafana before and after a change to the checkout.`;

export const projectsSeed = (now: number): Project[] => [
  projectRecord({ id: BILLING, name: 'Billing (old invoices)', status: 'archived', customer: 'accountants of the shops', repos: ['gitlab.com/acme/billing'], connections: [], by: actorOf('assistant', 'assistant'), createdAt: iso(now - 90 * 24 * 60 * MIN), updatedAt: iso(now - 20 * 24 * 60 * MIN), rev: 3 }),
  projectRecord({ id: GAMES, name: 'Game factory player refunds and support tooling', status: 'active', customer: 'players of the game factory', repos: [], connections: [], by: actorOf('owner', 'owner@example.com', 'web'), createdAt: iso(now - 9 * 24 * 60 * MIN), updatedAt: iso(now - 9 * 24 * 60 * MIN), rev: 1 }),
  projectRecord({ id: STOREFRONT, name: 'Storefront', status: 'active', customer: 'shop owners who sell on the storefront, and their buyers at checkout', brief: BRIEF,
    repos: ['github.com/acme/storefront', 'github.com/acme/checkout', 'gitlab.com/acme/platform/sales-engine', '/Users/remit/Projects/Storefront experiments'],
    connections: [{ source: 'grafana', connection: 'eu-west' }, { source: 'starrocks', connection: 'prod-read' }],
    by: actorOf('assistant', 'assistant'), createdAt: iso(now - 30 * 24 * 60 * MIN), updatedAt: iso(now - 2 * 60 * MIN), rev: 4 }),
];

// The project each chain is on, by chain id.
export const chainProjects: Record<string, string> = {
  busy: STOREFRONT, mr: STOREFRONT, watch: STOREFRONT, ok: STOREFRONT, q2: STOREFRONT,
  ask: GAMES, wait: GAMES, done1: GAMES, q1: GAMES,
  arch: BILLING,
};

export const projectChangesSeed = (now: number): Record<string, Change[]> => ({
  [`project/${STOREFRONT}`]: [
    changeRecord({ id: 'chp_3', kind: 'project', name: STOREFRONT, action: 'changed', by: actorOf('assistant', 'assistant'), at: iso(now - 2 * 60 * MIN),
      fields: [{ name: 'connections', before: '[{"source":"grafana","connection":"eu-west"}]', after: '[{"source":"grafana","connection":"eu-west"},{"source":"starrocks","connection":"prod-read"}]' }] }),
    changeRecord({ id: 'chp_2', kind: 'project', name: STOREFRONT, action: 'changed', by: actorOf('assistant', 'assistant'), at: iso(now - 3 * 24 * 60 * MIN),
      fields: [{ name: 'customer', before: '"shop owners"', after: '"shop owners who sell on the storefront, and their buyers at checkout"' }] }),
    changeRecord({ id: 'chp_1', kind: 'project', name: STOREFRONT, action: 'created', by: actorOf('assistant', 'assistant'), at: iso(now - 30 * 24 * 60 * MIN), fields: [] }),
  ],
});
