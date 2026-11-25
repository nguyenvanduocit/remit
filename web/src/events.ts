// What came in, as the owner reads it: how much the writer sent and the way it came. What became of it is its firings, in the
// server's words.
import type { EventVia, InboundEvent } from './core/generated/types.generated';
import { bytes } from './ui';

// The way an event came in, as the owner reads it: a word for its line and its own icon (so it never rests on colour), and a
// sentence for its page. Door and CLI both post to the signal's door (`posts`), so the path they posted to is part of it.
export const VIA: Record<EventVia, { word: string; icon: string; says: string; posts: boolean }> = {
  door: { word: 'Door', icon: 'arrow-right', says: 'Posted to its door', posts: true },
  cli: { word: 'CLI', icon: 'terminal', says: 'Sent with remit emit, through its door', posts: true },
  poll: { word: 'Sensor', icon: 'repeat', says: 'Read by a sensor of the signal from its source', posts: false },
  agent: { word: 'Agent', icon: 'spark', says: 'Written by an agent with emit_event', posts: false },
  plugin: { word: 'Plugin', icon: 'spark', says: 'Written by a processor of a plugin', posts: false },
};

const encoder = new TextEncoder();
// The body's size as the owner reads every size (ui.ts bytes); a body the server cut is only the start of what was sent.
export function bodySize(e: Pick<InboundEvent, 'body' | 'bodyCut'>): string {
  const size = bytes(encoder.encode(e.body).length);
  return e.bodyCut ? `${size}, cut` : size;
}

// A JSON media type: application/json or a +json suffix (application/vnd.api+json), whatever its parameters.
const JSON_TYPE = /^application\/(?:[\w.-]+\+)?json\s*(?:;|$)/i;
// The body as the owner reads it: indented when it is JSON and parses, as it was sent otherwise (a body the server cut does
// not parse). A sensor's event is JSON Remit wrote; a writer's is JSON when it said so.
export function bodyShown(e: Pick<InboundEvent, 'via' | 'header' | 'body'>): string {
  if (!e.body) return '(empty body)';
  const type = Object.entries(e.header ?? {}).find(([name]) => name.toLowerCase() === 'content-type')?.[1] ?? '';
  if (e.via !== 'poll' && !JSON_TYPE.test(type)) return e.body;
  try {
    return JSON.stringify(JSON.parse(e.body), null, 2);
  } catch {
    return e.body;
  }
}
