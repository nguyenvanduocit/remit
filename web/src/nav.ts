// View state that lives in the address, so reload, back and shared links land on the same screen.
import { computed, watch, type ComputedRef, type WritableComputedRef } from 'vue';
import { useRoute, useRouter, type LocationQuery, type RouteLocationNormalizedGeneric, type RouteLocationRaw } from 'vue-router';
import { listedChain } from './catalog';
import type { Where } from './core/generated/types.generated';

// A query parameter as a ref. Setting it replaces the history entry (a refinement of the screen, not a new
// screen) and drops the key when it is back at its default.
export function useQueryRef<T extends string>(key: string, fallback: T, valid?: readonly T[]): WritableComputedRef<T> {
  const route = useRoute();
  const router = useRouter();
  return computed({
    get: () => {
      const v = route.query[key];
      return (typeof v === 'string' && (!valid || (valid as readonly string[]).includes(v)) ? v : fallback) as T;
    },
    set: (v) => {
      const query = { ...route.query, [key]: v === fallback ? undefined : v };
      void router.replace({ query });
    },
  });
}

// The project a view of the work is narrowed to (?project=, ADR-0108): the board, the column beside an open chain (carried from
// the board) and the Objectives tab. View state only: what is of a project is the server's to say.
export const useProjectRef = (): WritableComputedRef<string> => useQueryRef('project', '');

// A page that lists what exists and shows one (the signals, the events) has the one open in the address, under
// `param`. An address that names none settles on the first of the list as soon as it has one, so what the owner reads stays
// put while the list changes under it.
export function useSettle(param: string, first: () => string | undefined): void {
  const route = useRoute();
  const router = useRouter();
  watch(
    [() => route.params[param], first],
    ([named, id]) => {
      if (!named && id) void router.replace({ params: { ...route.params, [param]: id }, query: route.query });
    },
    { immediate: true },
  );
}

// The card open in the drawer: 'chair' (the default) or a card id. Picking one starts its drawer on the first tab.
export function useSelected(): WritableComputedRef<string> {
  const route = useRoute();
  const router = useRouter();
  return computed({
    get: () => (typeof route.query.card === 'string' ? route.query.card : 'chair'),
    set: (id) => void router.replace({ query: { ...route.query, card: id === 'chair' ? undefined : id, panel: undefined } }),
  });
}

// The new-chain box is a layer over whatever view is open: ?new=1.
export function useNewChain() {
  const route = useRoute();
  const router = useRouter();
  return {
    open: computed(() => route.query.new !== undefined),
    close: () => void router.replace({ query: { ...route.query, new: undefined } }),
  };
}

// The Assistant is a layer over whatever view is open, and stays open as the owner moves between views: ?assistant=1, and
// ?assistantPanel= the panel it shows (its activity unless named; carryLayers keeps both on every other page).
export function useAssistant() {
  const route = useRoute();
  const router = useRouter();
  const open = computed(() => route.query.assistant !== undefined);
  const show = () => void router.push({ query: { ...route.query, assistant: '1' } });
  const close = () => void router.replace({ query: { ...route.query, assistant: undefined, assistantPanel: undefined } });
  return { open, show, close, toggle: () => (open.value ? close() : show()) };
}

// The layers that stay open over every view (the query keys that open them, and that say what they show).
const LAYERS = ['assistant', 'assistantPanel'] as const;
// A move to another page keeps the layers open on the one it leaves: a link names only where it goes (a nav link, a chain
// id in an agent's words), so without this a layer would close at every move. A change on the same page (closing the
// layer, a refinement of the screen) is taken as it is. The location to go to instead, or undefined to go as asked.
type At = Pick<RouteLocationNormalizedGeneric, 'path' | 'query' | 'hash'>;
export function carryLayers(to: At, from: At): RouteLocationRaw | undefined {
  if (to.path === from.path) return undefined;
  const carried: LocationQuery = {};
  for (const key of LAYERS) if (from.query[key] !== undefined && to.query[key] === undefined) carried[key] = from.query[key];
  if (!Object.keys(carried).length) return undefined;
  return { path: to.path, query: { ...to.query, ...carried }, hash: to.hash };
}

// Where the owner is, as a channel tells the Assistant (the contract's Where): the page and, on a chain's page, the chain,
// its tab and the agent open in its drawer (the Chair unless ?card= names another, `chairOf` says whose it is); on the
// Playbooks, Agents, Skills, Tools, Plugins, Client keys, Feedback, Objectives, Projects, Employees and Mentors pages, the one
// open (on Skills, and the file of it being read); on the Signals and Subscriptions pages the item open (a subscription's id)
// and on the Events page the event, by its address. Elsewhere, the project the view is narrowed to (?project=), so "this
// project" in his words is the one he is looking at. The Assistant reads it through its tool where_is_owner.
type Route = Pick<RouteLocationNormalizedGeneric, 'name' | 'params' | 'query'>;
const given = (v: unknown): string | undefined => (typeof v === 'string' && v ? v : undefined);
export function whereOf(at: Route, chairOf: (chainId: string) => string | undefined): Where {
  const page = typeof at.name === 'string' ? at.name : 'board';
  const where: Where = { page };
  if (page === 'chain') {
    where.chainId = given(at.params.id);
    where.tab = given(at.params.tab);
    where.cardId = given(at.query.card) ?? (where.chainId && chairOf(where.chainId));
  }
  if (page === 'playbooks') where.playbookId = given(at.params.playbookId);
  if (page === 'agents') where.agentId = given(at.params.agentId);
  if (page === 'skills') {
    where.skillName = given(at.params.skillName);
    where.file = given(at.query.file);
  }
  if (page === 'tools') where.sourceId = given(at.params.sourceId);
  if (page === 'accounts') where.accountId = given(at.params.accountId);
  if (page === 'clientKeys') where.clientKeyId = given(at.params.keyId);
  if (page === 'extensions') where.extensionId = given(at.params.extensionId);
  if (page === 'signals' || page === 'subscriptions') where.entry = given(at.params.entry);
  if (page === 'events') where.eventId = given(at.params.id);
  if (page === 'feedback') where.feedbackId = given(at.params.id);
  if (page === 'objectives') where.objectiveId = given(at.params.id);
  where.projectId = page === 'projects' ? given(at.params.id) : given(at.query.project);
  if (page === 'employees') where.employeeId = given(at.params.id);
  if (page === 'mentors') where.mentorId = given(at.params.id);
  return Object.fromEntries(Object.entries(where).filter(([, v]) => v !== undefined)) as Where;
}
export function useWhere(): ComputedRef<Where> {
  const route = useRoute();
  return computed(() => whereOf(route, (id) => listedChain(id)?.chairId));
}

// The tab a chain opens on, whatever its status: its Overview.
export const defaultTab = 'overview';

// What a list page shows of the item its address names (`wanted`), given the one open and the one being opened: the open
// one when it is that one, and while another is on its way the open one still, stepping back (`switching`) under a running
// bar. Once the one named could not be opened, the old one does not stand under its address.
export function onScreen(open: string | undefined, wanted: string, opening: string): { shows: boolean; switching: boolean } {
  const switching = !!opening && opening !== open;
  return { shows: !!open && (open === wanted || switching), switching };
}
