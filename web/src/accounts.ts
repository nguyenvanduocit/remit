// What the owner reads of the Claude accounts agents run on. A login is added and removed with buttons on the Accounts page
// (the owner's rails: they sign it in and out) and a plugin's account comes and goes with the plugin's connection; the
// Assistant only reads them.

import type { AccountHealth, AccountKind } from './core/generated/types.generated';

// What the owner calls an account: a plugin's by the plugin and the connection of it the account is, any other by the address
// it signed in as, else its id (the machine's login has no address until it names one).
export const accountLabel = (a: Pick<AccountHealth, 'id' | 'email' | 'plugin' | 'connection'>): string =>
  a.plugin && a.connection ? `${a.plugin} · ${a.connection}` : a.email || a.id;

// What an account says of itself: it takes work (and whether the server says a limit of it is near), or why it does not.
export const accountState = (a: AccountHealth): string => (!a.ok ? (a.error ?? 'Out') : a.nearLimit ? 'Takes work · near its limit' : 'Takes work');

// What each kind of account is, and how it comes and goes.
export const ACCOUNT_KIND: Record<AccountKind, string> = {
  machine: "The default account: this machine's own Claude Code login, always there, never removed",
  login: 'A login you added',
  plugin: "A plugin's account: one of its connections. It leaves when the connection's credential is removed or the plugin is disabled",
};
