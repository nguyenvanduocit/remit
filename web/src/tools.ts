// How a source's connection stands, and what waits for the owner on it: read from what the server sends, never decided here
// beyond choosing the words and the colour (the contract's ToolSource).
import type { BadgeVariants } from '@/components/ui/badge';
import type { InboxItem, ConnectionView } from './core/generated/types.generated';

export interface Standing {
  variant: BadgeVariants['variant'];
  icon: string;
  text: string;
}

// A connection that cannot be read says so first (its stored values do not open); else it is configured, or it lacks values.
export function connectionStanding(c: ConnectionView): Standing {
  if (c.problem) return { variant: 'danger', icon: 'alert', text: 'cannot be read' };
  if (c.configured) return { variant: 'success', icon: 'check', text: 'configured' };
  return { variant: 'needs', icon: 'key', text: `lacks ${c.missing.length} ${c.missing.length === 1 ? 'value' : 'values'}` };
}

// How many credential items of the source wait in the owner's inbox.
export const waitingFor = (inbox: readonly InboxItem[], source: string): number =>
  inbox.filter((i) => i.kind === 'credential' && i.status === 'open' && i.source === source).length;
