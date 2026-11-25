# 0019. The top bar has four areas

Status: accepted (2026-10-30); superseded in part by ADR-0024: the Deliveries tab is the Events tab, and by ADR-0042: Crew's Tools, Plugins, Accounts and Client keys are the fifth area, Integrations. Amended three times on 2026-10-30, on the owner's word. First, Work's Scheduled tab became two,
**Signals** and **Subscriptions**: he does not think of the system as a schedule but as signals, subscriptions and firings, and
wants the signals listed and opened on a tab of their own, a column for each source, not as sections inside one column. Then
the webhooks and the deliveries moved over to the signals: a webhook is what writes the signal of its posts and a delivery is one
of those posts, so the Hooks area became the **Signals** area (Signals, Subscriptions, Webhooks, Deliveries), Work kept the
board, and Client keys, the keys outside agents hold to start chains, went to Crew. Last, the Webhooks tab went: a webhook is a source of signals like any other, so its
door (address, key, prompt, preconditions) is on the page of the signal of its posts (`delivery:<name>`) on the Signals tab, its
rules are one subscription on the Subscriptions tab with nothing a check-back or a routine lacks, and no tab has a button that
makes one: the owner tells the Assistant. He reads the Signals area as Signals, Subscriptions, Deliveries.

## Context

The top bar had six entries: Board, Outcomes, Feedback, Signals, Hooks and Crew. Three of them carried a count (Board, Feedback,
Hooks), two were already groups with a strip of tabs under them (Hooks: Deliveries, Webhooks, Client keys; Crew: Playbooks,
Agents, Skills, Tools) and four were single pages. The owner asked for one design language and a clearer information
architecture: fewer entries, each standing for one thing he comes for, with every page still reachable at its address.

Two rules bound the answer. *Agent-centric* (`AGENTS.md`): what an agent does, asks or fails at shows live only where the owner
looks for that agent, whose tab counts what waits for him, so no count may be merged into another's. *Server-driven*: the web app
keeps only view state; the counts stay the server's (`catalog.board.needsYou`, the unread feedback, `catalog.deliveryBoard`).

## Options

1. **Keep six entries.** Nothing to learn, but Signals and Feedback read as peers of the Board when one is a schedule of what
   starts chains and the other a mailbox about the chains, and a seventh page would have made a seventh entry.
2. **Three areas** (Work, Hooks, Crew), with Outcomes and Feedback under Work. One entry fewer, but the Work count would then
   have to say either what waits on a chain or what is unread, or merge the two: it breaks the counting rule, and a dashboard of
   results is not where the owner goes to act.
3. **Four areas**: Work (Board), Results (Outcomes, Feedback), Signals (Signals, Subscriptions, Deliveries), Crew
   (Playbooks, Agents, Skills, Tools, Client keys). Each area's entry keeps the count of its own first concern. **Chosen.**
4. **Webhooks and Deliveries as tabs of Work, beside Signals and Subscriptions** (weighed at the second amendment). Keeps three
   entries in their place, but Work would carry two counts that both mean "waits on you" (the board's and the deliveries'), which
   the counting rule forbids, and a Hooks area left with Client keys alone would stand for nothing the owner comes for.
5. **A Webhooks tab beside Subscriptions** (held at the second amendment, dropped at the third). Each webhook was then listed
   twice, once as a webhook and once as its line on the Subscriptions board, with two pages and two addresses for one thing.
6. **The door on the webhook's subscription page, with a New webhook button on the board** (tried at the third amendment, the
   owner refused it). It made one kind of subscription unlike the others: rows, sections and a button no check-back, watch or
   routine has, and the configuration of a source shown where what listens to it is shown.

## Decision

- The top bar reads **Work · Results · Signals · Crew**. An entry opens its area's first page and is marked on every page of the
  area. Work counts the board's `needsYou`, Results the feedback not read yet, Signals the deliveries that wait on the owner, wait
  for a Dispatcher or are being worked on, each with the tone it had; Crew counts nothing. The document title counts the board's
  `needsYou` alone, as before.
- The entry is named **Work**, not Board: it stands for the chains, the board and each chain's page, and an entry named Board over
  a tab named Board would say the same word twice for two things.
- The **Signals** area is the signal backbone as the owner reads it (ADR-0014), and its entry opens the Signals tab. Signals
  (`signals`, `/signals/:entry?`) is a column for each source of signals Remit supports, each holding its signals; a signal or
  a source opens beside its column, with the subscriptions that listen to it (for a webhook's posts, the webhook's own line). Subscriptions (`subscriptions`, `/subscriptions/:entry?`) is every subscription (check-backs, watches, the Scout,
  routines, the webhooks) in the server's groups and the firings; a subscription opens beside its group and leads back to the
  signal it listens to (Time for one on the clock). A webhook's door (who set it, its address, prompt,
  preconditions, key and how to post to it) is on the page of the signal of its posts (`/signals/signal:delivery:<name>`), as a
  source's own; its rules are its subscription (`/subscriptions/webhook:<name>`), read like every other, with Pause or Resume. Deliveries (`/deliveries`) are the posts that came and what became of them. `Where.page` names each, with its open item.
- **Crew** holds Client keys (`/client-keys`) after Tools: a client key is how an outside agent starts chains, so it sits with
  the agents and what they work with.
- **One mechanism for the grouping**: a route carries `meta.area` (`work | results | signals | crew`) and, when it is a tab of the
  area's strip, `meta.tab` (its label). `TopBar.vue` marks the entry whose area the route carries; `PageTabs.vue` draws an area's
  strip from the routes that carry its area and a tab, in route order. A chain's page carries the area and no tab: Work is marked,
  no strip shows, the chain's own tabs stand in its header.
- **The strip costs the board nothing.** `PageTabs` has an `end` slot for what a page keeps at the strip's end; the board puts its
  search there, so the strip takes the row the search had. Measured on the verify page at 1920×1080 and 1024×768: the board's
  columns start at 87 px instead of 100 px, 13 px gained, with the strip in place.
- **The keyboard keeps its place in a strip.** Each page mounts its own strip, so an arrow key that opens the next page swaps
  one strip for another; the new strip focuses its open tab when the old one had the focus.
- `/subscriptions` is added for the subscriptions; `/signals` holds the signals. A subscription's address moves from `/signals/<id>`
  to `/subscriptions/<id>`. The one stored trace of a page is a message's `data.placed`, shown as its words only
  (`StreamBlocks.vue`), never followed as a link, so nothing stored needs to move.

## Consequences

- A new page joins an area by naming it in its route's `meta`, and appears in that area's strip in route order; the top bar
  changes only when an area is added.
- Outcomes and Feedback share a strip, so the Outcomes page's own heading repeats the tab's word until that page is restyled.
- The Assistant's skills, the design document and the tool `read_signals` name the two tabs by these words.

## Revisit when

- An area's strip reaches six tabs, or a page fits no area: the areas are re-derived from what the owner comes for, not extended.
  (The second amendment was such a re-derivation: Crew's five tabs are one concern, who works and with what.)
- Two counts in one area both mean "waits on you": the area is split, never the count merged.
- The chain's page grows a need for the Work strip (moving between a chain and the schedule in one step): the `end` slot on
  `PageHead` is the place to look first, not a second row.
