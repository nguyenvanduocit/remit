// What the owner's agents are made of, besides playbooks: the agents he keeps, and the skills they can hold (the
// server calls the skills its library). Like playbooks they are written by an agent, never by a form (playbook.ts);
// the lists change as it saves.
import type { Agent, ListedSkill, SkillFile, Source } from './core/generated/types.generated';

// What a Claude Code tool allowlist says, in words: no list is every tool, an empty one none (Agent.tools).
export const allowedText = (tools: readonly string[] | null): string => (tools === null ? 'every tool' : tools.length ? tools.join(', ') : 'none');

// The files of a skill, SKILL.md first (the one every skill has), then the rest in order.
const SKILL_FILE = 'SKILL.md';
export function fileNames(files: readonly SkillFile[]): string[] {
  const names = files.map((f) => f.path).sort();
  return names.includes(SKILL_FILE) ? [SKILL_FILE, ...names.filter((n) => n !== SKILL_FILE)] : names;
}
// The file of a skill that is open: the one asked for, else SKILL.md, else the first it has ('' when it has none).
export function openFile(files: readonly SkillFile[], wanted: string | undefined): string {
  return wanted !== undefined && files.some((f) => f.path === wanted) ? wanted : (fileNames(files)[0] ?? '');
}

// The agents that name a skill, in the order the server lists agents.
export const heldBy = (agents: readonly Agent[], skill: string): Agent[] => agents.filter((a) => a.loadout.skills.includes(skill));

// Where a skill came from, in words: what an agent imported from Claude Code's configuration, or what was written here.
const SOURCES: Record<string, string> = { 'claude-code': 'Claude Code' };
export const sourceText = (source: Source | undefined): string => (source ? `imported from ${SOURCES[source.kind] ?? source.kind}` : 'written here');
