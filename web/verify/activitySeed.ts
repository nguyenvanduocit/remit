// What the server types an event of an agent's work as (`orch::activity`, the contract's `Activity`), for a seed to send
// it as the server would: a seed states it, and nothing on this side works it out from a title or a tool's input.
import type { Asker, Block, EventRead, ToolGroup, ToolState } from '../src/core/generated/types.generated';

export const tool = (group: ToolGroup, name: string, summary: string, status: ToolState = 'completed'): Pick<EventRead, 'activity'> => ({
  activity: { type: 'tool', group, name, summary, status },
});

/** A message sent through a tool: `to` is the card, none for the Chair. */
export const sent = (to: string | undefined, kind: string, label: string, text: string): Pick<EventRead, 'activity'> => ({
  activity: { type: 'sent', ...(to === undefined ? {} : { to }), kind, label, text },
});

export const turnEnd = (ms?: number): Pick<EventRead, 'activity'> => ({ activity: { type: 'turn_end', ...(ms === undefined ? {} : { ms }) } });

/** A permission prompt asked: `id` is its request id, `of` who is asked first. */
export const asked = (id: string, request: string, of: Asker): Pick<EventRead, 'activity'> => ({ activity: { type: 'asked', id, request, of } });

/** A permission prompt decided: `id` is its request id, none for a refusal no prompt was asked for. */
export const decided = (request: string, allowed: boolean, id?: string): Pick<EventRead, 'activity'> => ({
  activity: { type: 'decided', ...(id === undefined ? {} : { id }), request, allowed },
});

/** The owner's Start over: the session break the conversation shown begins at. */
export const startedOver = (): Pick<EventRead, 'activity'> => ({ activity: { type: 'started_over' } });

/** A block the agent showed (`show_block`, ADR-0091), with its picks as they stand. */
export const shownBlock = (block: Block): Pick<EventRead, 'activity'> => ({ activity: { type: 'block', block } });
