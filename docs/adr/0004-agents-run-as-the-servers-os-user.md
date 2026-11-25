# 0004. Agents run as the server's OS user, a time-boxed gap

Status: narrowed by 0018, 2026-10-30 (sessions start with an allowlisted environment and with Remit's secret paths hidden; the gaps that remain are listed in 0018's Consequences); changed by ADR-0058, 2026-11-04: sessions run in the AI runtime, the core's child, as the same OS user with the same HOME, and the Claude config dirs are the runtime's. Accepted as temporary 2026-10-27. Source: review S1, S2, S5 and `docs/design.md` (the Known gap on agents).

## Context

Every agent's shell runs as the server's OS user. The ask rules and the policy are a tripwire, not a boundary: an agent can
reach `secret.key` or the database in a way no rule lists, and so lift a budget cap, hide a record of who acted, or read a
credential. The rails (caps, pause and stop, authentication, merging) are code only while the code is the only path to the
data.

## Options

1. Accept the gap for good.
2. Give agents their own OS user, or a sandbox, with filtered network egress.
3. Keep tightening the tripwire (more rules).

## Decision

Option 2 is the direction; the gap is accepted only until it is built. Until then, the tripwire is kept as tight as it goes: the
key file and the database are in the ask rules from the server's flags, new files are created 0600 and
0700, and the Dispatcher holds no tool. Option 3 is not pursued past this, since a rule list cannot close a boundary.

Building option 2 needs the host (a second user or a sandbox runtime, egress filtering). It is host work outside this
repository and the owner's to schedule. The code side is prepared: the work directory, the key file and the database
paths already come from flags.

## Consequences

Until isolation exists, anyone who can steer an agent (a webhook payload, a merge request description, a page it reads) can
steer a shell with the server's rights. Webhook and import entrances stay under the threat model in each commit.

## Revisit when

Isolation is built (supersede this ADR), or an incident shows the tripwire being used.
