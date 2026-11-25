// One URL per view: /board, /at-work, /queues, /objectives/<objective>, /projects/<project>, /chains/<id>/<tab>, /outcomes, /judgments, /feedback/<feedback>, /signals/<signal>, /subscriptions/<subscription>, /events/<event:signal#rev>, /employees/<employee>, /mentors/<mentor>, /duels, /playbooks/<playbook>, /agents/<agent>, /skills/<skill>, /tools/<source>, /plugins/<plugin>, /accounts/<account>, /client-keys/<key>.
// Smaller refinements ride in the query (?card=, ?panel=, ?q= the board's search, ?project= the project the board and the objectives are narrowed to, ?col= the board's column beside a chain, ?status=, ?new=, ?assistant=, ?file= the file of a skill being read): see nav.ts. main.ts makes the app's router (appRouter); a verify page, with a
// memory history.
import { createRouter, type Router, type RouteRecordRaw, type RouterHistory } from 'vue-router';
import { carryLayers } from './nav';

// The five areas of the top bar (TopBar.vue), by what the owner comes for: Work is the chains, the merge queues their changes land
// through, the objectives they serve and the projects (products) those are about; Results is what came of the work
// and what the agents say about it; Signals is what Remit hears and what it does about it (the signals, the subscriptions to
// them, an event signal's rules among them, and the events that came in, whichever way); Crew is who works and what they are made of (the employees, the mentors, how their lineages fare in duels, the playbooks, the agents, the skills);
// Integrations is what Remit reaches out through and what reaches in (the tools, the plugins, the Claude accounts, the keys of the outside agents that start chains).
export type Area = 'work' | 'results' | 'signals' | 'crew' | 'integrations';

declare module 'vue-router' {
  interface RouteMeta {
    // The area a page is under, and its name in that area's strip of tabs (PageTabs.vue). A page with an area and no tab (a
    // chain's) marks its area's entry and shows no strip: it has tabs of its own.
    area?: Area;
    tab?: string;
    // A page opened from one tab's page, about one of its things (a repository's releases, from its merge queue): the strip
    // marks that tab (`under`, its route's name), and `title` names the page.
    under?: string;
    title?: string;
  }
}

export const routes: RouteRecordRaw[] = [
  { path: '/', redirect: { name: 'board' } },
  { path: '/board', name: 'board', meta: { area: 'work', tab: 'Board' }, component: () => import('./components/BoardView.vue') },
  { path: '/at-work', name: 'atWork', meta: { area: 'work', tab: 'At work' }, component: () => import('./components/AtWorkView.vue') },
  { path: '/queues', name: 'queues', meta: { area: 'work', tab: 'Merge queues' }, component: () => import('./components/QueuesView.vue') },
  { path: '/releases', name: 'releases', meta: { area: 'work', under: 'queues', title: 'Releases' }, component: () => import('./components/ReleasesView.vue') },
  { path: '/objectives/:id?', name: 'objectives', meta: { area: 'work', tab: 'Objectives' }, component: () => import('./components/ObjectivesView.vue') },
  { path: '/projects/:id?', name: 'projects', meta: { area: 'work', tab: 'Projects' }, component: () => import('./components/ProjectsView.vue') },
  { path: '/chains/:id/:tab(live|tree|workflows|landings|verification|overview)?', name: 'chain', meta: { area: 'work' }, component: () => import('./components/ChainsView.vue') },
  { path: '/outcomes', name: 'outcomes', meta: { area: 'results', tab: 'Outcomes' }, component: () => import('./components/OutcomesView.vue') },
  { path: '/judgments', name: 'judgments', meta: { area: 'results', tab: 'Judgments' }, component: () => import('./components/JudgmentsView.vue') },
  { path: '/feedback/:id?', name: 'feedback', meta: { area: 'results', tab: 'Feedback' }, component: () => import('./components/FeedbackView.vue') },
  { path: '/signals/:entry?', name: 'signals', meta: { area: 'signals', tab: 'Signals' }, component: () => import('./components/SignalsView.vue') },
  { path: '/subscriptions/:entry?', name: 'subscriptions', meta: { area: 'signals', tab: 'Subscriptions' }, component: () => import('./components/SubscriptionsView.vue') },
  { path: '/events/:id?', name: 'events', meta: { area: 'signals', tab: 'Events' }, component: () => import('./components/EventsView.vue') },
  { path: '/employees/:id?', name: 'employees', meta: { area: 'crew', tab: 'Employees' }, component: () => import('./components/EmployeesView.vue') },
  { path: '/mentors/:id?', name: 'mentors', meta: { area: 'crew', tab: 'Mentors' }, component: () => import('./components/MentorsView.vue') },
  { path: '/duels', name: 'duels', meta: { area: 'crew', tab: 'Duels' }, component: () => import('./components/DuelsView.vue') },
  { path: '/playbooks/:playbookId?', name: 'playbooks', meta: { area: 'crew', tab: 'Playbooks' }, component: () => import('./components/PlaybooksView.vue') },
  { path: '/agents/:agentId?', name: 'agents', meta: { area: 'crew', tab: 'Agent templates' }, component: () => import('./components/AgentsView.vue') },
  { path: '/skills/:skillName?', name: 'skills', meta: { area: 'crew', tab: 'Skills' }, component: () => import('./components/SkillsView.vue') },
  { path: '/tools/:sourceId?', name: 'tools', meta: { area: 'integrations', tab: 'Tools' }, component: () => import('./components/ToolsView.vue') },
  { path: '/plugins/:extensionId?', name: 'extensions', meta: { area: 'integrations', tab: 'Plugins' }, component: () => import('./components/ExtensionsView.vue') },
  { path: '/accounts/:accountId?', name: 'accounts', meta: { area: 'integrations', tab: 'Accounts' }, component: () => import('./components/AccountsView.vue') },
  { path: '/client-keys/:keyId?', name: 'clientKeys', meta: { area: 'integrations', tab: 'Client keys' }, component: () => import('./components/ClientKeysView.vue') },
  { path: '/:rest(.*)*', redirect: { name: 'board' } },
];

// The app's router: these routes, and the layers open over a view (nav.ts carryLayers) kept as the owner moves.
export function appRouter(history: RouterHistory): Router {
  const router = createRouter({ history, routes });
  router.beforeEach((to, from) => carryLayers(to, from) ?? true);
  return router;
}
