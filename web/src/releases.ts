// A repository as the core holds it for this tab (replica.ts, ADR-0114, ADR-0116): the one a screen opens, or every one
// the merge queues show, each laid out by the server (RepoView: its queues, its newest releases with the landings each
// carried, its release pause) and replaced whole as it moves. Where each release stands and why are the server's; this
// words them. A pause is pulled and lifted by the Assistant, on the owner's word: nothing here changes one.
import { computed } from 'vue';
import type { Release, ReleaseState, RepoView } from './core/generated/types.generated';
import { inOrder, oneOf } from './core/client';
import { ask, coreState, useModel } from './replica';
import type { Look } from './ui';

const open = useModel<RepoView>({ model: 'repo' });
const every = inOrder(useModel<RepoView>({ model: 'repos' }));

// Opens repository `repo` (its catalog id, as a landing names it); empty closes it.
export function openRepo(repo: string): void {
  ask({ ask: 'openRepo', repo });
}

// Opens every repository with something in its queues or releases, or closes them.
export function openRepos(open: boolean): void {
  ask({ ask: 'openRepos', open });
}

// The repository open, once read: the server answers a view for any repository.
export const openedRepo = computed(() => oneOf(open));

export const repos = {
  // Every repository's view, in the server's order.
  get views(): RepoView[] {
    return every.value;
  },
  // They have been read since they were opened or the last snapshot.
  get read(): boolean {
    return coreState.value?.reposRead ?? false;
  },
};

// A release whose steps run turns its ring (StandingBadge's `working`); every other state wears its icon.
export const RELEASE_LOOK: Record<ReleaseState['kind'], Look & { label: string; working?: boolean }> = {
  releasing: { tone: 'info', label: 'Releasing', working: true },
  baking: { tone: 'gate', icon: 'clock', label: 'Baking' },
  live: { tone: 'success', icon: 'check', label: 'Live' },
  unchanged: { tone: 'quiet', icon: 'dot', label: 'Nothing new' },
  taken_back: { tone: 'danger', icon: 'repeat', label: 'Taken back' },
};

// A release as the owner names it: the version its procedure reported, else its commit.
export const releaseName = (r: Pick<Release, 'version' | 'commit'>): string => r.version || r.commit.slice(0, 8);
