// What the core says of the link (State.banner, ADR-0094), written out: the core words it; the page only writes the time
// the link went in the owner's own clock.
import type { Banner } from './core/generated/replica/types.generated';

const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

export const said = (banner: Banner): string => (banner.since === undefined ? banner.text : banner.text.replace('{since}', clock(banner.since)));
