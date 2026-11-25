import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'vitest';
import { cn } from './utils';

// Every name theme.css adds to Tailwind's spacing, font-size and radius scales merges as that scale: a later utility of the
// same property replaces an earlier one, and one of another property stays. A name added to theme.css and not taught to
// cn fails here.
const css = readFileSync(new URL('../theme.css', import.meta.url), 'utf8');
const DEFAULT = new Set(['xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl']);
const named = (scale: string) => [...css.matchAll(new RegExp(`--${scale}-([a-z0-9-]+?):`, 'g'))].map((m) => m[1]!).filter((n) => !n.includes('--') && !DEFAULT.has(n));

describe('cn knows the names theme.css adds', () => {
  test.each(named('spacing'))('spacing %s', (n) => {
    expect(cn('h-0 px-2', `h-${n} px-${n}`)).toBe(`h-${n} px-${n}`);
  });
  test.each(named('text'))('font size %s', (n) => {
    expect(cn('text-xs text-mut', `text-${n}`)).toBe(`text-mut text-${n}`);
  });
  test.each(named('radius'))('radius %s', (n) => {
    expect(cn('rounded-md', `rounded-${n}`)).toBe(`rounded-${n}`);
  });
  test('theme.css adds names to each scale', () => {
    expect([named('spacing').length, named('text').length, named('radius').length].every((l) => l > 0)).toBe(true);
  });
});
