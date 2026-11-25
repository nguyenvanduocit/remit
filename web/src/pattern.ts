// What a subscription listens to is a pattern over signal keys (ADR-0049), with the syntax SQLite's GLOB gives the server: `*` any
// run of characters (dots included), `?` one character, `[abc]` / `[a-c]` / `[^abc]` a class. A key with none of them is an exact
// key. One matcher here, so the list's search, a signal's listeners and the words under a subscription agree with the server's router.
import type { SignalSourceLine } from './core/generated/types.generated';

export const isPattern = (pattern: string): boolean => /[*?[]/.test(pattern);

const regexOf = (pattern: string): RegExp => {
  let out = '';
  for (let i = 0; i < pattern.length; i++) {
    const c = pattern[i];
    if (c === '*') out += '[\\s\\S]*';
    else if (c === '?') out += '[\\s\\S]';
    else if (c === '[') {
      const close = pattern.indexOf(']', i + 2);
      if (close < 0) out += '\\[';
      else {
        const body = pattern.slice(i + 1, close);
        out += body.startsWith('^') ? `[^${body.slice(1).replace(/[\\\]]/g, '\\$&')}]` : `[${body.replace(/[\\\]^]/g, '\\$&')}]`;
        i = close;
      }
    } else out += c.replace(/[.+^${}()|\\\]]/g, '\\$&');
  }
  return new RegExp(`^${out}$`);
};

/** Whether key is one the pattern matches whole. */
export const matchesKey = (pattern: string, key: string): boolean => (isPattern(pattern) ? regexOf(pattern).test(key) : pattern === key);

/** A key shape of the server as the pattern the owner types: each `<part>` any run of characters, a run of them one `*`. */
export const patternOfShape = (shape: string): string => shape.replace(/<[^>]*>/g, '*').replace(/\*(\.\*)+/g, '*');

/**
 * What a pattern listens to, in plain words: `one signal` for an exact key, `every event signal` for the shape of a source, and
 * otherwise how many of the known signals it matches now.
 */
export function patternWords(pattern: string, sources: readonly SignalSourceLine[], keys: readonly string[]): string {
  if (!isPattern(pattern)) return 'one signal';
  const source = sources.find((s) => patternOfShape(s.pattern) === pattern);
  if (source) return `every ${source.source.replace(/_/g, ' ')} signal`;
  const n = keys.filter((k) => matchesKey(pattern, k)).length;
  return n === 1 ? 'every signal it matches: 1 now' : `every signal it matches: ${n} now`;
}
