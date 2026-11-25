// What the owner reads of a mentor (ADR-0062): the mark that says Mentor, in words and an icon, where they stand with their apps,
// and how a help of theirs on a card reads. The words are the server's (MentorStanding.presence, Help.state); only the icon and
// the badge's look are chosen here.
import { catalog } from './catalog';
import { opened } from './openChain';
import type { HelpState, Mentor, MentorStanding } from './core/generated/types.generated';

export const MENTOR = { label: 'Mentor', icon: 'person' } as const;

// What stands in for a mentor's code until the owner asks to see it.
export const WORKER_KEY_MASK = 'wk_••••••••••••';

// A mentor in one of their apps now reads as present; one active before, or who has used no code yet, reads quietly: the icon
// differs with each, so the state never rests on the colour.
export function presenceLook(p: MentorStanding): { label: string; icon: 'dot' | 'clock' | 'x'; variant: 'success' | 'quiet' } {
  if (p.inAppNow) return { label: p.presence, icon: 'dot', variant: 'success' };
  return { label: p.presence, icon: p.lastActive ? 'clock' : 'x', variant: 'quiet' };
}

// The mentor of id, as the replica holds them.
export const mentorNamed = (id: string): Mentor | undefined => catalog.mentors.find((m) => m.id === id);

// The name of the mentor of help helpId, a help on the open chain, or "A mentor" when the replica holds neither.
export function mentorOfHelp(helpId: string | undefined): string {
  const h = opened.view?.helps.find((x) => x.id === helpId);
  return (h && mentorNamed(h.mentor)?.label) || 'A mentor';
}

// How each state of a help reads, always as words beside an icon.
export const HELP: Readonly<Record<HelpState, { label: string; icon: string; variant: 'info' | 'success' | 'quiet' }>> = {
  offered: { label: 'asked to help', icon: 'clock', variant: 'info' },
  accepted: { label: 'helping', icon: 'person', variant: 'success' },
  declined: { label: 'declined', icon: 'x', variant: 'quiet' },
  ended: { label: 'helped', icon: 'check', variant: 'quiet' },
};
