// Playbooks are written by an agent, never by a form: the owner says what he wants and an author chain does it.
import type { Chain } from './core/generated/types.generated';

// The author chains still at work, newest first: they write playbooks, agents and skills (ADR-0113).
export const authoring = (chains: readonly Chain[]): Chain[] =>
  chains.filter((c) => c.kind === 'author' && c.status !== 'done').sort((a, b) => b.createdAt.localeCompare(a.createdAt));
