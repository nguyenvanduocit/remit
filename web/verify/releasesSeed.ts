// Releases for the verify pages (ADR-0116), as a repository's view (RepoView) holds them: each repository's newest first, each
// with the landings it carried, oldest first, and the repository's last release pause. The awkward cases: a release baking under two guards, one taken back by a guardrail with what its way back
// ran, one whose procedure said nothing new, a repository's first release, a release waiting on a connection with no
// credential whose step exited 75, a pause held by the Assistant on the owner's word and one lifted. Each landing names the
// release that carried it (landingsSeed.ts).
import type { Landing, Release, RepoView } from '../src/core/generated/types.generated';
import { actorOf } from './records';

const MIN = 60_000;
const HOUR = 60 * MIN;

export function releasesSeed(now: number, landings: readonly Landing[]): Record<string, Omit<RepoView, 'queues'>> {
  const iso = (ago: number) => new Date(now - ago).toISOString();
  const sha = (n: number) => `${n.toString(16).padStart(4, '0')}c0ffee9a1b2c3d4e5f60718293a4b5c6d7e8f9`;
  const api = { repo: 'acme/api', host: 'gitlab.example.com', project: 'acme/api', base: 'main' };
  const web = { repo: 'acme/web', host: 'github.com', project: 'acme/web', base: 'main' };
  const remit = actorOf('remit', 'release');
  const release = (fields: Pick<Release, 'id' | 'repo' | 'host' | 'project' | 'base' | 'commit' | 'top' | 'state' | 'createdAt'> & Partial<Release>): Release => ({
    version: '', links: [], steps: [], tries: 1, back: [], guards: [], waits: '', rev: 1, ...fields,
  });
  const step = (name: string, exit: number | undefined, output: string, ago: number, took = 40_000) =>
    ({ name, ...(exit === undefined ? {} : { exit }), output, startedAt: iso(ago), endedAt: iso(ago - took) });
  const guard = { signal: 'chain:ch_billing.sensor.webhook-5xx', breach: 'value.rate > 0.01', bake: 2 * HOUR * 1e6 };

  const baking = release({ ...api, id: 'rel_5', commit: sha(108), previous: sha(6), top: 'ln_k1', state: { kind: 'baking' }, version: 'v2026.11.19-3',
    links: ['https://api.acme.example/v1', 'https://gitlab.example.com/acme/api/-/releases/v2026.11.19-3'],
    steps: [step('10-build', 0, 'go build ./... ok (41s)\nimage registry.acme.example/api:v2026.11.19-3 pushed', 2 * HOUR), step('20-deploy', 0, 'helm upgrade api ./deploy/helm --set image.tag=v2026.11.19-3\nrelease "api" upgraded: 6/6 pods ready', 2 * HOUR - 60_000)],
    guards: [guard, { signal: 'chain:ch_cache.sensor.currency-mismatch', breach: 'value.count > 0', bake: 30 * MIN * 1e6 }],
    createdAt: iso(2 * HOUR), bakeUntil: new Date(now + 50 * MIN).toISOString() });
  const live = release({ ...api, id: 'rel_4', commit: sha(6), previous: sha(3), top: 'ln_r2', state: { kind: 'live' }, version: 'v2026.11.19-2',
    links: ['https://api.acme.example/v1'], steps: [step('10-build', 0, 'go build ./... ok', 4 * HOUR + 50 * MIN), step('20-deploy', 0, 'release "api" upgraded', 4 * HOUR + 49 * MIN)],
    guards: [guard], createdAt: iso(4 * HOUR + 50 * MIN), bakeUntil: iso(2 * HOUR + 50 * MIN), endedAt: iso(2 * HOUR + 50 * MIN), rev: 3 });
  const takenBack = release({ ...api, id: 'rel_3', commit: sha(3), previous: sha(102), top: 'ln_b2',
    state: { kind: 'taken_back', by: remit, why: 'chain:ch_billing.sensor.webhook-5xx read 0.034 (> 0.01) for 6 minutes while it baked: its way back ran, its landings were reverted and the release before it put back.' },
    version: 'v2026.11.19-1', links: ['https://api.acme.example/v1'],
    steps: [step('10-build', 0, 'go build ./... ok', 5 * HOUR + 35 * MIN), step('20-deploy', 0, 'release "api" upgraded', 5 * HOUR + 34 * MIN)],
    back: [step('10-rollback', 0, 'helm rollback api 41\nRollback was a success! 6/6 pods ready on v2026.11.18-4', 5 * HOUR + 2 * MIN, 25_000)],
    guards: [guard], createdAt: iso(5 * HOUR + 35 * MIN), bakeUntil: iso(3 * HOUR + 35 * MIN), endedAt: iso(5 * HOUR), rev: 4 });
  const unchanged = release({ ...api, id: 'rel_2', commit: sha(102), previous: sha(98), top: 'ln_c1', state: { kind: 'unchanged' },
    steps: [step('10-build', 0, 'nothing to publish: the image for this tree is registry.acme.example/api:v2026.11.18-4 already', 8 * HOUR + 48 * MIN)],
    createdAt: iso(8 * HOUR + 48 * MIN), endedAt: iso(8 * HOUR + 47 * MIN), rev: 2 });
  const first = release({ ...api, id: 'rel_1', commit: sha(98), top: 'ln_m1', state: { kind: 'live' }, version: 'v2026.11.18-4',
    steps: [step('10-build', 0, 'go build ./... ok', 20 * HOUR)], createdAt: iso(20 * HOUR), endedAt: iso(19 * HOUR), rev: 2 });

  const waiting = release({ ...web, id: 'rel_w1', commit: sha(129), previous: sha(127), top: 'ln_w0', state: { kind: 'releasing' },
    waits: 'The connection vercel has no credential yet: the release runs again once one is typed in the inbox.',
    steps: [step('10-deploy', 75, 'vercel deploy --prebuilt\nError: no token for api.vercel.com (exit 75: try again later)', 2 * HOUR + 55 * MIN, 3000)],
    tries: 3, createdAt: iso(2 * HOUR + 56 * MIN), rev: 4 });

  // A release with the landings that name it (landingsSeed.ts), oldest first.
  const read = (release: Release) => ({ release, landings: landings.filter((l) => l.release === release.id).sort((a, b) => a.createdAt.localeCompare(b.createdAt)) });
  return {
    'acme/api': { repo: 'acme/api', releases: [baking, live, takenBack, unchanged, first].map(read),
      pause: { repo: 'acme/api', by: actorOf('assistant'), why: 'The owner: hold the API while finance closes the month.', at: iso(30 * HOUR),
        lifted: { by: actorOf('owner'), why: 'Month closed.', at: iso(22 * HOUR) } } },
    'acme/web': { repo: 'acme/web', releases: [waiting].map(read),
      pause: { repo: 'acme/web', by: actorOf('assistant'), why: 'The owner: hold releases of the site until Monday; the checkout copy is still being reviewed.', at: iso(90 * MIN) } },
  };
}
