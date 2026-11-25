// This page's side of the connection, for as long as the page is open: whether the owner is signed in (and if the server
// turned the sign-in away, why) and the link to the server, with what the page says of it, as the core words it (its
// state, replica.ts; ADR-0094). Only whether this browser holds an owner key is the page's.
import { ref } from 'vue';
import type { Banner, Phase, RefusalCode } from './core/generated/replica/types.generated';
import { coreState } from './replica';

// offline: no session (`why` is the latest reason, empty before the first try). syncing: the session is up and the
// snapshot is on its way, so what is shown is from before. stale: the snapshot failed, so what is shown may be old.
// live: the snapshot is applied and changes arrive as pushes. `refused`: the server turned this page away.
export type Link = { phase: Phase; why: string; refused?: RefusalCode };

// 'ok' once this browser holds an owner key, 'missing' until then, 'rejected' when the server refuses the key.
export type Auth = 'ok' | 'missing' | 'rejected';

const holdsKey = ref(false);
const OFFLINE: Link = { phase: 'offline', why: '' };

export const session = {
  get auth(): Auth {
    if (!holdsKey.value) return 'missing';
    return coreState.value?.refused === 'bad_credential' ? 'rejected' : 'ok';
  },
  get link(): Readonly<Link> {
    const s = coreState.value;
    return s ? { phase: s.phase, why: s.why, refused: s.refused } : OFFLINE;
  },
  // What the page says of the link, none while what is shown is current or before the core has said anything.
  get banner(): Banner | undefined {
    return coreState.value?.banner;
  },
  // Why an action that changes something cannot be taken now, or null when it can.
  get blocked(): string | null {
    return coreState.value?.blocked || null;
  },
  // The link is up: what the owner asks is sent (the core refuses it otherwise), even while the latest data is still loading.
  get connected(): boolean {
    return this.link.phase !== 'offline';
  },
};

export function signedIn(): void {
  holdsKey.value = true;
}
