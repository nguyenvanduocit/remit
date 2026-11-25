# 0042. Crew is split into Crew and Integrations

Status: accepted, 2026-11-02. Amends ADR-0019 (the top bar has four areas). Source: the owner's request that the Crew tab, with
too many unrelated pages in it, be split into one more tab.

## Context

ADR-0019 listed under *Revisit when*: "An area's strip reaches six tabs, or a page fits no area: the areas are re-derived from
what the owner comes for, not extended." Crew had eight tabs: Employees, Playbooks, Agents, Skills, Tools, Plugins, Accounts and
Client keys. Two things were in it: the pages that say who works and what they are made of, and the pages that say what Remit
connects to and what connects to it. The owner comes for them for different reasons: to read or shape a worker, or to connect,
sign in or hand out a key.

## Options

1. **Keep one area with eight tabs.** Nothing to change, and the strip keeps growing with every new connection kind.
2. **Two areas: Crew (Employees, Playbooks, Agents, Skills) and Integrations (Tools, Plugins, Accounts, Client keys).** **Chosen.**
   Each strip has four tabs. Every page of Integrations is about a boundary: a tool source, a plugin that is a source of tools and
   events, a Claude account agents run on, a key an outside agent holds.
3. **Tools with Crew, Plugins, Accounts and Client keys apart.** Tools is what agents are made of in the Skills sense, but a tool
   source's page is mostly its connections and credentials, the same concern as Plugins and Accounts; the split would cut a concern.

## Decision

- The top bar reads **Work · Results · Signals · Crew · Integrations**. Integrations opens its first page, Tools, and counts
  nothing, as Crew and Signals do.
- A route carries `meta.area` (`work | results | signals | crew | integrations`); Tools (`/tools`), Plugins (`/plugins`),
  Accounts (`/accounts`) and Client keys (`/client-keys`) carry `integrations`. No address changes.
- `CrewPage`, the board shared by Playbooks, Agents, Skills and Tools, takes the area whose strip it draws (`group`) from its page.

## Consequences

- A new connection kind (a source of events, a provider account) joins Integrations by naming the area in its route.
- The Assistant's `Where.page` names are unchanged.

## Revisit when

- Integrations or Crew reaches six tabs again: re-derive the areas from what the owner comes for.
- A count that waits on the owner appears in Integrations (an account that needs a sign-in, a source whose credential is missing):
  the entry counts it, as Work and Results do, and the count stays one meaning.
