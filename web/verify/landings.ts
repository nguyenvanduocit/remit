// A chain's landings (ChainLandings), the merge queues (QueuesView) and a repository's releases (ReleasesView) on made-up
// data (landingsSeed.ts, releasesSeed.ts), in the whole signed-in app through the real client core over its fake server.
// ?at= is the route, its own query escaped (?at=/releases%3Frepo%3Dacme/api).
// - /queues (the default): acme/api's busy queue with every state, acme/web's canary stages, a protected release branch.
// - /chains/ch_billing/landings: a version sent back for a conflict, one taken back by the pipeline and its revert, a canary
//   stage being checked, a version on the web app still being judged (?landing=<id> opens one, ?file=<path> a file);
//   /chains/ch_web/landings?landing=ln_wb: an attempt waiting for its round's pick.
// - /chains/ch_notes/landings?landing=ln_n1: a folder worked in place that is not Git: approved, with no queue, branch or base;
//   ?landing=ln_n2: a version of it committed on the folder's own branch (ADR-0139).
// - /chains/ch_cache/landings?landing=ln_k1: a landed change being taken back by a revert still being combined (ln_rk1).
// - /releases?repo=acme/api: a release baking, one live, one taken back by a guardrail, one with nothing new, the first;
//   ?repo=acme/web: a release waiting on a credential under a held pause.
// ?empty=1 has no queue; ?fail=1 makes the queues, the releases and the diffs fail to read; ?live=<ms> lands the stage being
// checked that long after load, as the `landing_updated` and `repo_updated` pushes say it; ?released=<ms> makes the baking
// release live and lifts the site's pause, as `repo_updated` pushes each repository whole. window.push plays a push, window.asked holds the calls the page made.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { createMemoryHistory } from 'vue-router';
import Shell from '../src/components/Shell.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { signedIn } from '../src/session';
import { appRouter } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { cardRecord, chainRead, landingRead } from './records';
import { stands } from './standingSeed';
import { filesOf, landingsSeed } from './landingsSeed';
import { releasesSeed } from './releasesSeed';
import type { ChainView, Landing, LandingRead, RepoView } from '../src/core/generated/types.generated';
import '../src/theme.css';

const params = new URLSearchParams(location.search);
const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();
const seed = landingsSeed(now);
const releases = releasesSeed(now, seed.all);
const fail = !!params.get('fail');

// A repository as the server lays it out (RepoView), and every repository.
const repoOf = (repo: string): RepoView => ({ queues: seed.queues[repo] ?? [], ...(releases[repo] ?? { releases: [] }), repo });
const repoViews = () => (params.get('empty') ? [] : [...new Set([...Object.keys(seed.queues), ...Object.keys(releases)])].map(repoOf));
// A landing as a chain's screen reads it: with the release that carried it, and the landing it takes back when it is a revert
// on its way.
const reading = (l: Landing): LandingRead => {
  const release = Object.values(releases).flatMap((v) => v.releases).find((r) => r.release.id === l.release)?.release;
  const takingBack = seed.takingBack[l.id];
  return landingRead(l, { ...(release && { release }), ...(takingBack && { takingBack }) });
};
const of = (chainId: string): LandingRead[] => seed.all.filter((l) => l.chainId === chainId).sort((a, b) => a.createdAt.localeCompare(b.createdAt)).map(reading);

const viewOf = (id: string): ChainView => {
  const c = seed.chains.find((x) => x.id === id);
  if (!c) throw new Error(`chain ${id} not found`);
  const chair = cardRecord({ id: c.chairId, home: id, agent: 'chair', name: 'Chair', origin: 'chair', status: 'idle', createdAt: c.createdAt, updatedAt: iso(now) });
  return {
    chain: chainRead(c), cards: [chair], edges: [], decisions: [], datadog: [], landings: of(id), drafts: [], workflows: [], helps: [], agents: [],
    verification: { chainId: id, gates: [], judges: [], rev: 1 }, standing: stands(id, c.status === 'done' ? 'Done' : 'Running', c.status === 'done' ? 'done' : 'running'), retrospectives: [],
  };
};

const fake = fakeServer({
  appSnapshot: async () => structuredClone(snapshotOf({ chains: seed.chains.map((c) => chainRead(c)) })),
  chainGet: async (id) => structuredClone(viewOf(id)),
  chainEvents: async () => ({ events: [], truncated: false }),
  chainsArchived: async () => ({ chains: [], standings: [], next: '' }),
  repoViews: async () => {
    if (fail) throw new Error('the store is busy');
    return structuredClone(repoViews());
  },
  repoGet: async (repo) => {
    if (fail) throw new Error('the store is busy');
    return structuredClone(repoOf(repo));
  },
  landingDiff: async (landingId) => {
    if (fail) throw new Error('the clone of acme/api is being fetched');
    const l = seed.all.find((x) => x.id === landingId);
    if (!l) throw new Error(`no landing ${landingId}`);
    return { landingId, from: l.version.forked, files: filesOf(landingId) };
  },
});

// The stage being checked lands: its record moves, pushed as the server pushes each change of a landing and its repository.
if (params.get('live')) {
  setTimeout(() => {
    const i = seed.all.findIndex((x) => x.id === 'ln_b3');
    const l = seed.all[i];
    const landed: Landing = { ...l, state: { kind: 'landed' }, commit: 'b3c0ffee9a1b2c3d', push: 'b3c0ffee9a1b2c3d', checks: { state: 'pending', failed: '' }, landedAt: iso(Date.now()), updatedAt: iso(Date.now()), rev: l.rev + 1 };
    seed.all[i] = landed;
    for (const q of seed.queues['acme/api']) q.landings = q.landings.map((x) => (x.id === landed.id ? landed : x));
    fake.push('landingUpdated', reading(landed));
    fake.push('repoUpdated', repoOf('acme/api'));
  }, Number(params.get('live')));
}

// The release baking goes live, and the site's pause is lifted, as the server pushes each.
if (params.get('released')) {
  setTimeout(() => {
    const read = releases['acme/api'].releases[0];
    read.release = { ...read.release, state: { kind: 'live' }, endedAt: iso(Date.now()), rev: read.release.rev + 1 };
    fake.push('repoUpdated', repoOf('acme/api'));
    const web = releases['acme/web'];
    web.pause = { ...web.pause!, lifted: { by: { kind: 'assistant', who: '', via: '' }, why: 'Monday came.', at: iso(Date.now()) } };
    fake.push('repoUpdated', repoOf('acme/web'));
  }, Number(params.get('released')));
}

signedIn();
const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/queues');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
Object.assign(window, { router, push: fake.push, asked: fake.asked });
