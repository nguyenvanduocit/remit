# 0090. One job per hue, and a state shows in a chip, never on a card's surface or border

Status: accepted, 2026-11-13.
Source: the owner's words of 2026-11-13 (translated): "Now implement every idea you just suggested", the ideas being those
of `docs/research-agentes-chat/report.md`, idea 16 ("blue does four jobs today; callouts stop being tinted, colour moves to
a chip") from the colour-role review.

## Context

Colour is how the owner reads a dense board at a glance, and it keeps a meaning only while each hue says one thing.

- **Blue did four jobs** (notes F2): the main button (`--primary`), the focus ring (`--ring`), the owner himself (his
  message bubble, his "You" badge) and information (`--info`: a running chain, a live dot, a "new" badge). On one screen a
  blue badge could mean "the owner did this" or "this is running", and a blue fill "press me".
- **Callouts were tinted** (notes F5): a box that waits on the owner was an orange surface inside an orange border, a decided
  one a sand surface inside a sand border, an error a red one. AGENTS.md already says a card's state shows "as its surface, a
  badge, text or an icon, never as a ring or glow on its border"; a coloured border on every callout is the ring it forbids,
  and a whole tinted surface leaves the hue nothing stronger to say when something matters more.
- The other hues already had one job each, written beside `statusTone` in `web/src/ui.ts`: orange waits on the owner, amber
  a warning, green done, red failed, sand a gate, violet the Assistant. An agent's badge colour (`AgentBadge`) is that
  agent's identity, a letter on its own pair, and reuses hues.

## Options

1. **Ink for the action** (agentes.chat: the main button is ink, blue belongs to agents). Blue then means one thing, but
   Remit's links, focus and every `variant="default"` button change at once, and a dark console with a near-white main
   button reads as a warning more than an invitation. The owner's identity would still need a hue.
2. **Blue is what the owner can act on; the owner himself is ink; information moves to cyan.** Blue keeps the main button,
   links and the focus ring, which are one job: "you can act here". The owner, a person among agents, is drawn in ink (a
   raised neutral surface, the text colour), as agentes.chat draws people. Work going on that needs no one (running, in
   progress, new) takes cyan, unused for any state until now. **Chosen.**
3. **Give the owner his own hue** (a teal or a warm white). One more hue to keep apart from the eight there are, for a
   person the screen is already built around.

## Decision

**One job per hue.** Components name the role (the shadcn names in `theme.css`, the badge variants, the tones of
`statusTone`), never the hue, so a role can move hue in one place.

| Role | Hue | Token (where it is named) | Where it shows |
|---|---|---|---|
| What the owner can act on: the main button, a link, the focus ring | blue | `--primary`, `--ring` (theme.css), `text-blue-text` on links | one solid button per view, links, `:focus-visible` |
| Work going on that needs no one: running, in progress, new | cyan | `--info` (theme.css), `--cyan-*` (style.css), badge `info` | a running chain, a live dot, "new", "edited" |
| Waits on the owner | orange | `--orange-*`, badge `needs`, `Callout tone="ask"` | the needs badge, the waiting chip |
| A warning, something to watch | amber | `--warning`, badge `warning` | waiting on time, a check-back |
| Done, allowed | green | `--success`, badge `success` | done, Allow |
| Failed, refused, destructive | red | `--destructive`, badge `danger`, `Callout tone="error"` | failed, Refuse, an error |
| A gate, a Chair's note, decided | sand | `--sand-*`, badge `gate`, `Callout tone="note"` | a gate, a note, an item decided |
| The Assistant | violet | `--violet-*`, badge `assistant` | the Assistant's tag and bubble |
| The owner, a person | ink (no hue) | badge `owner` (`border-line2 bg-panel3 text-text`), `owner-bubble` (`--panel3`, `--line2`) | his messages, "You", his overturn |
| An agent | its own pair | `--agent-*-bg` (style.css), `AgentBadge` | the agent's letter; never a state |

**A state shows in a chip, never on the surface or the border.** A callout (`Callout.vue`) is a card like the board's: the
`--panel` surface, the card's half-pixel ring (`--shadow-card`), no coloured border. Its tone is a chip (`IconChip`: an
icon on the tone's `-tint`, the glyph in the hue) beside its heading, or beside its body when it has none, and the heading's
words say the state too, so it never rests on colour alone. The owner's overturn in a decided item (`InboxDecided.vue`) is
his act and is drawn in ink, sunk below the card as a record (`--bg`, `--line`). The same holds for any card: an open card
or a chosen row is pressed into the page (`--well`, `--shadow-pressed`), a surface, never a ring.

## Consequences

- `theme.css`: `--info` is cyan; `owner-bubble` is ink; `--blue-bubble` is gone. `style.css` gains `--cyan-tint`, `-bg`,
  `-line`, `-text`, `--sand-tint` and `--well`; the hue comment there states the roles. The badge `owner` variant is ink.
- `Callout.vue` draws its tone as a chip with a default icon per tone (`waits`, `comment`, `alert`) and takes `icon` to
  name another (`InboxCallout` passes the kind's). Its callers lose the icon they put in the heading themselves.
- Running, live and "new" read cyan everywhere through `--info`, with no component edited.
- **Still on blue though their role is not "act"**, for the screens wave to move to their role, each named here so none is
  missed: column-head icons drawn in `text-blue-mark` (`BoardColumn`, `CrewColumn`, `DuelsView`, `SubscriptionColumn`'s
  "coming"), which are decoration and go neutral; the kind tints `bg-blue-bg text-blue-text` of an open MR
  (`ChainChanges` `MR_LOOK.opened`), a `check` subscription (`SubscriptionKindIcon`) and a `practice` feedback
  (`FeedbackKindIcon`), which are kinds and take neutral or cyan; the syncing banner (`ConnectionBanner`), which is
  information and takes cyan; `ChainAway`'s blue panel ("While you were away"), a callout by another name that takes the
  chip; the caret and running bar in `Markdown` and `ListPage`, which are live and take cyan; the bars of `OutcomeKpi` and
  `CardTelemetry` and the `--tree-line` and `--meter-on` of outlines and meters, which are data and keep their own scale.
- A light theme, if one comes, changes values under the same roles; this ADR fixes the roles, not the values.

## Revisit when

- The owner, looking at the board and a chain with these colours, finds cyan running chains louder than what waits on him:
  `--info` moves back toward a quieter hue, and the orange of `needs` stays the loudest thing on the board.
- agentes.chat's ink action proves itself on a dark console Remit can look at: option 1 is a change of `--primary` alone.
