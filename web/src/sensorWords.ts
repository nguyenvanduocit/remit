// What a sensor is, in the owner's words, whatever its kind (ADR-0049): a poll reads a source's connection on a period, a script
// runs behind one, and a plugin's kind (`plugin:<id>.<name>`) is given settings. One reader of the
// kinds, so every screen that shows a sensor says the same.
import type { Sensor } from './core/generated/types.generated';
import { plural } from './ui';

export type SensorKindLook = { noun: string; plugin?: string };

/** `poll` and `script` as nouns; a plugin's kind by its name, with the plugin that brings it. */
export function kindLook(kind: string): SensorKindLook {
  if (kind === 'poll') return { noun: 'Poll' };
  if (kind === 'script') return { noun: 'Script' };
  const rest = kind.startsWith('plugin:') ? kind.slice('plugin:'.length) : '';
  const [plugin, name] = rest.split('.');
  return plugin && name ? { noun: name, plugin } : { noun: kind };
}

/** How often a poll or a script reads: none for a kind with no period. */
export const everyWords = (s: Sensor): string | undefined => {
  if (!s.everyMinutes) return undefined;
  return s.everyMinutes === 1 ? 'Every minute' : `Every ${plural(s.everyMinutes, 'minute')}`;
};

/** The settings a plugin kind was given, as short `key: value` rows; none for anything that is not an object. */
export function settingRows(settings: unknown): [string, string][] {
  if (!settings || typeof settings !== 'object' || Array.isArray(settings)) return [];
  return Object.entries(settings).map(([k, v]) => [k, typeof v === 'string' ? v : JSON.stringify(v)]);
}
