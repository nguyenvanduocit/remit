# 0101. What cannot be undone takes effect after an overturn window

Status: accepted, 2026-11-13. Amends ADR-0087 (*No agent waits on the owner*, *Threats*, *Revisit when*). Source: the
owner's choice of option (b) of R1 in `docs/reports/Long running agent systems beliefs.md`, in his words: "The Assistant
decides immediately, but an irreversible action takes effect only after a fixed window in which you can overturn it. No
agent sits idle: the agent that asked keeps working on other parts."

## Context

ADR-0087 routes every item that would wait for the owner to the Assistant, which decides it at once; he reads after and
overturns. For most items an overturn puts things back: a playbook is written again, a capability taken back, a question
answered again. For the top rung of the effect ladder it does not. `grant::Effect` (`crates/grant/src/lib.rs:13-26`)
ranks what an action does to the outside world by how far it can be undone: read, reversible, compensable, and
irreversible ("a message sent, a record destroyed"). A grant at `irreversible` the Assistant applies lets a chain do that
at once, and an overturn afterwards reaches only what was not done yet. The report's evidence (R1): a same-model supervisor
is lenient (Vend's CEO said yes about eight times as often as no), the Assistant acts on text other parties wrote, and
Paperclip's watchdog may not approve irreversible items. `AGENTS.md` takes a risk "when its loss is bounded and recovery is
cheap"; an irreversible effect has neither.

Which items let something irreversible happen, read from what already ranks effects (one concept, one owner):

| Item the Assistant decides | Rung | Why |
|---|---|---|
| A change item of kind `grant` | the grant's `effect` | The gate (`grant_access.rs`) lets every source call through only under a grant of its effect or above, so the grant is the one door to an effect on a connection |
| A change item of kind `extension` (an outside MCP server included) | irreversible | Its program's tools are ranked by its author; the ladder's rule is that the platform ranks a tool, never its author (`crates/grant/src/lib.rs:13`), so what the platform did not rank counts as the top rung |
| Any other change item (playbook, agent, skill, sensor, objective, employee, forget, promote, capability) | none | Each writes a record Remit holds, which a later write or a stop puts back; what a capability lets a chain do outside passes its own gates (grants, `merge_mr`'s verdicts and canary, the release's bake) |
| A question, an exception, an error, a held subscription | none | The answer is words or a retry; what follows from it passes the gates above |
| A permission request a Chair passed up | not held | See *Options*: the Chair decides the same request at once, so a window on the Assistant's answer alone guards nothing |

## Options

**The window's state.**

1. **A new entity, a held decision** (its table, its notification). A second record of what the item already says.
2. **A status of the item, `decided`, with `takes_effect_at` (chosen).** The item is the decision's subject, already
   carries the answer, who gave it and when, and its status already says where it stands; the effect is the answer the
   owner's Apply calls (`answer_change`), run when the time comes. No table, no migration: `inbox.status` is text and
   `takes_effect_at` lives in the item's document.

**When it takes effect.**

1. **A timer per item.** Lost on restart.
2. **The Tick (chosen).** `take_effect_due` runs on every Tick (at least once a minute): each decided item whose time has
   come is applied through `answer_change`, whose transaction stores the write and closes the item together
   (`store_answer` accepts a decided item only once its time has passed, `answerable`). A restart mid-window resumes it, and
   two Ticks never write it twice: the second finds it answered.

**Permission requests.**

1. **Hold the Assistant's Allow of a call with the `irreversible` trait** (`permission_ask.rs`): refuse the call now, store
   an allowance for that exact call from `takes_effect_at`, and let the agent call again. The Chair decides the same
   requests at once and passes up only what it cannot judge, and once the owner's time limit passes the Chair decides
   alone; a window on the Assistant's answer would only teach Chairs not to pass irreversible calls up.
2. **Leave permission requests as ADR-0059 and ADR-0087 have them (chosen).** Named under *Revisit when*.

**The window's length.**

1. **A flag or setting.** No flag sets a comparable duration; `permission_wait` is the owner's setting for a different wait
   (the lead's answer), and a second knob before anyone asks for one is a flag in disguise.
2. **A constant of 60 minutes (chosen), `OVERTURN_WINDOW` in `crates/orch/src/overturn_window.rs`.** It is the bake ADR-0087
   gives a release before it is called healthy: the time Remit already allows a person or a signal to catch a bad change.
   Only the irreversible step waits; the asker works on meanwhile.

Models applied. **Inversion**: what guarantees an irreversible harm with this window in place? The window passing while
the owner is away (bounded by his feed's count and the digest, and by the fact that the effect is the one an owner would
have approved by hand), an overturn racing the effect (one transaction decides: `stop_decided` and `store_answer` both
read the status inside their write), and a decided item lost to a restart (it is the stored item). **Reversibility**:
the window converts a one-way door into a two-way one for its length, at the price of latency on that one step.

## Decision

- **Decided now, effective after the window.** When the Assistant answers Apply on a change item whose write lets
  something on the top rung happen (`waits_out_window`), the item is stored `decided` with the answer, who gave it, when,
  and `takes_effect_at` = now + 60 minutes, and nothing is written. The owner's own Apply takes effect at once: the window
  exists for his overturn. The Assistant's `assist` decision is recorded as for any answer.
- **No agent idles.** The agent that asked is told at once (`[change result · decided]`): nothing is written yet, when it
  will be, and to go on with what does not need it; at the time it is told `[change result · applied]` as for any Apply,
  or `[change result · overturned]`. `finish_chain` still waits for a decided item, as it waits for an open one.
- **Takes effect once.** On each Tick, `take_effect_due` applies each decided item whose time has come through
  `answer_change`, as the Assistant that decided it, so the change records who wrote it and who let it through. A record
  that changed meanwhile closes the item unwritten and the author is told, as for any Apply.
- **Overturn within the window.** His overturn of a decided item closes it, cancelled, in the transaction that records the
  overturn (`stop_decided`): nothing is ever written, the asker reads his word, and the Assistant is told it was within the
  window. After the window his overturn is ADR-0087's: the asker undoes what followed.
- **How he sees it.** The item and its feed line carry `takes_effect_at` while it is decided (`InboxItem`, `FeedLine` in
  `crates/wire/src/app.rs`); the Assistant's feed shows the time it takes effect, counting down, beside Overturn.

## Threats

| Threat | Control |
|---|---|
| Text in an item steers the Assistant to apply an irreversible grant or an extension | It is decided as before, but nothing is written for 60 minutes, during which the line shows in his feed with Overturn |
| An overturn and the effect race | Both read the item's status inside their own transaction; whichever writes first wins, and the other sees it |
| A restart loses or repeats the effect | The decided item is stored; `answer_change` closes it in the transaction that writes, so a second run is refused |
| A Chair passes an irreversible call up to dodge nothing | Permission requests are not held (see *Options*) |

## Consequences

- An irreversible grant or an extension the Assistant applies arrives up to 61 minutes later; the chain that asked works on
  around it or waits by subscribing, and its slot is free while it waits.
- The inbox has a fourth status, `decided`; `Board.waiting` and Needs you do not list it (nothing waits for him).
- The owner's own Apply is unchanged.
- New words: the status `decided`, `takes_effect_at`, `OVERTURN_WINDOW`, `waits_out_window`, `take_effect_due`,
  `stop_decided`.

## Revisit when

- **The owner asks for a window on permission requests:** hold the Allow of a call with the `irreversible` trait on both
  rungs of the ladder (the Chair's and the Assistant's) through an allowance the exact call consumes once; the seam is
  `permission_ask::traits_of` and `on_permission`'s first step.
- **The window should be his to set:** a setting beside `permission_wait`, set by the Assistant on his words; the seam is
  `OVERTURN_WINDOW`.
- **The Assistant's record earns it** (ADR-0098's approve and decline ratios): a shorter window for a kind of item he has
  not overturned in a long run, or a longer one for a kind he overturns often.
- **He wants to be told** when an item enters its window, outside the panel (a channel of its own).
- **Another rung needs it:** a compensable effect whose compensation is costly; `rung_of` is the one place.
