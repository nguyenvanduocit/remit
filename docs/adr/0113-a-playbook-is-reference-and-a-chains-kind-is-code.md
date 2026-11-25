# 0113. A playbook is reference, and a chain's kind is code

Status: accepted, 2026-11-19; built; the kind and the capability `release` removed by ADR-0116. Supersedes ADR-0028's "capabilities a playbook lists" (a chain starts with its
kind's capabilities, and its Chair gives or asks for the rest), ADR-0048's "a playbook names who" (the Chair seat is chosen
as for a playbook that names none), ADR-0009's playbook allow-list of a client key, and ADR-0110's `Chain.gates` field (the
gates are read from the chain's kind, which ADR-0110 named as their source). Amends ADR-0001 (`owner_waits` is the kind's),
ADR-0031 and ADR-0045 (the objective and OKR playbooks become a kind and reference), ADR-0087 (the Release seat is a kind),
ADR-0102 (a trial is a kind) and ADR-0103 (`verdict_rates` by kind). Source: the owner, 2026-11-18: "playbooks seem to tie
our hands; a playbook should only be reference material, with no other meaning: no constraints, no rules, just reference
content", and his choice the same day of "reference only, fully".

## Context

A playbook (`model::Playbook`, `crates/capability/src/playbooks.rs`) is five things at once:

1. **Reference**: a name, a description, a brief for the Chair and the agents it suggests (`seeds`).
2. **Tools**: `capabilities`, copied onto the chain when it is queued (`chains::lifecycle::start_from`); a playbook write that
   adds a writing capability waits for the owner (`capability::rail::playbook_rail`).
3. **Seat**: `chair` names the employee that sits in the Chair (`orch::lifecycle` `chair_seat`, ADR-0048).
4. **Admission**: `owner_waits`, code for a built-in, copied onto the chain; such a chain takes no slot (`chains::admit`).
5. **Identity of system seats**: code starts the Release seat, the analyst and the Playbook author by playbook id
   (`RELEASE_PLAYBOOK_ID`, `ANALYST_PLAYBOOK_ID`, `AUTHOR_PLAYBOOK_ID`), and their Chair briefs live in the playbook's
   `chair_brief`. The prompt calls the playbook "a starting point, not a script", then says "the playbook wins" where it is
   narrower (`chains::prompts::start_section`).

ADR-0110 adds a sixth: a chore passes QC alone, and it left the source of `Chain.gates` to "the chain-kind change". No
code sets `Chain.gates` to `qc` today.

The owner reads these as hands tied: an agent cannot start useful work without choosing the right playbook, and a playbook
the Playbook author writes can change which tools a chain gets, who sits in its Chair and whether it holds a slot. What
code branches on has to be fixed when the chain is made and never configured by an agent; what a Chair reads for guidance
has to bind nothing.

Most of the code already keys on capabilities, not on playbook ids: `shipping::seeded::is_trial` (`seeding`),
`crew::coach::is_coaching` (`coaching`), `crew::analyst::is_analyst` (`outcomes`), the Release seat's
`shipping::release` (`release`). Two of those predicates confuse a tool with an identity: a chain granted `outcomes` reads
as an analyst to `is_analyst`, which refuses a second analyst while one runs.

## Options

1. **Capabilities only.** Every branch on a playbook id becomes a branch on a capability; admission, gates and system briefs
   read capabilities too (a `chore` capability that names QC alone, `objectives` meaning the owner waits). A capability
   would then mean a set of tools and also an identity, an admission class and a gate set. A Chair could grant itself a
   capability that lowers its own gates (ADR-0110 rejected the judged choosing its judges), and `is_analyst` shows the
   confusion is already a defect.
2. **A chain kind in code.** `model::ChainKind`, a closed `text_enum!`, set once when the chain is queued and changed by no
   tool. A code table gives each kind its starting capabilities, whether the owner waits, its gates, its Chair brief section
   and who may start it. Capabilities stay what they are: tools. A playbook keeps only reference content.
3. **System playbooks and reference playbooks.** Built-in playbooks keep code fields (capabilities, owner waits, brief) and
   the owner's are reference. "Playbook" would mean two things, and the owner asked for one.
4. **Kinds as records.** The owner or the Playbook author defines a kind with its capabilities and gates. That is today's
   playbook under another name: admission, gates and writing tools set by data an agent writes.

## Decision

Option 2.

### The kind

- `model::ChainKind` with `work` (the default), `chore`, `objective`, `author`, `release`, `coach`, `analyst`, `trial`.
  `Chain.kind` is stored; `Chain.playbook`, `Chain.playbook_id`, `Chain.owner_waits` and `Chain.gates` go.
- Pure methods on the kind in `model`: `gates()` (`qc` for `chore`, `review-and-qc` for every other) and `owner_waits()`
  (`objective`, `author`). Admission reads `c.kind.owner_waits()`; `required_gates`, `merge_mr`, the merge queue, the
  verification view and `gates_rule` read `c.kind.gates()`. A stored value cannot disagree with its kind.
- The code table `capability::kinds` gives each kind what needs the capability crate's names and briefs, reached by
  `chains` through the `Domains` seam that `playbook_for` and `owner_waits` use today:

  | Kind | Starts with | Owner waits | Gates | Chair brief section | Started by |
  |---|---|---|---|---|---|
  | `work` | none | no | review and QC | the playbook note (below) | anyone |
  | `chore` | none | no | QC | the playbook note; move a task that grows to a `work` chain | anyone |
  | `objective` | `objectives`, `okr`, `signals` | yes | review and QC | `OKR_SEAT_CRAFT` and `MEASURE_RULES`: nothing is active until applied in the inbox | owner, Assistant, his routines |
  | `author` | `agents`, `history`, `library`, `playbooks` | yes | review and QC | the Playbook author's brief | owner, Assistant |
  | `release` | `history`, `propose`, `release`, `ship`, `signals` | no | review and QC | `RELEASE_CHAIR_BRIEF` | Remit (`automation.rs`) |
  | `coach` | `coaching`, `signals` | no | review and QC | `COACH_CHAIR_BRIEF` | owner, Assistant |
  | `analyst` | `history`, `outcomes` | no | review and QC | `ANALYST_CHAIR_BRIEF` | Remit (`crew::analyst`) |
  | `trial` | `seeding`, `shell` | no | review and QC | `SEEDING_CHAIR_BRIEF` | owner, Assistant |

- Who may start a kind is one rule, `may_not_start(origin, kind)`, which replaces `may_not_have(origin, caps)`: a chain a
  proposal, an event or a client starts takes no kind whose capabilities write (so `work` or `chore`), and `release` and
  `analyst` are Remit's alone (one seat per instance, one analyst at a time).
- Identity predicates read the kind: `is_trial` is `kind == trial`, `is_coaching` is `kind == coach`, `is_analyst` is
  `kind == analyst`, the Release seat is `kind == release`. The tools stay behind capabilities. `release`, `coaching` and
  `seeding` are bound to their kind in the catalog (`Capability.kind`): `grant_capability` and `request_capability` refuse
  them to a chain of another kind, so tool and identity cannot part.
- A chain's `capabilities` are what it holds: its kind's when it is queued, then what its Chair gives (`grant_capability`,
  non-writing) or the Assistant decides (`request_capability`, writing). `NewChain.capabilities` stays for the chains Remit
  opens for its own job (verification, whose capabilities are its parent's reading ones).
- The Chair seat is chosen as for a playbook that names none: the employee that answers for the key result the chain
  serves, else the oldest standing Chair that fits the seat, else the shipped chair. `standing_chair` loses its `named`
  argument; `check_named_chair` and `named_chair_refusal` go.

### The playbook

- `model::Playbook` keeps `id`, `name`, `description`, `group` (its column on the Playbooks page), `brief` (renamed from
  `chair_brief`: it is the playbook's text, read by whoever chooses to), `seeds` (the agents it suggests), `builtin`,
  `edited` and `rev`. `chair`, `capabilities` and `owner_waits` go. A playbook write moves no rail, so `playbook_rail` and
  `hold_playbook` go, and a playbook change item is applied as written.
- Every Chair holds `list_playbooks` and `read_playbook`, as every Chair holds the reads of the agent definitions and the
  library. The writes stay with the `playbooks` capability (the `author` kind).
- The Chair's opening carries no playbook. For `work` and `chore` it says: playbooks describe how kinds of work have gone
  well; read one when it fits and take what helps; none binds you. The rules that said "a decision your playbook leaves to
  people" and "as far as your playbook allows" (`chains::prompts`, lines 38 and 39) are rewritten without the playbook.
- A starter who wants a playbook followed says so in the goal ("use the bug-sweep playbook"); the Chair reads it. No start
  carries a playbook id: `CreateChain`, `wire::app::NewChain`, `ProposeInput`, `StartChainInput`, `StartSpec`,
  `RoutineInput` and the subscription's start take a `kind` where their starter may choose one, and nothing otherwise.
- The built-in playbooks that were system seats (`release`, `playbook-author`, `coach`, `seeded-fault`, `analyst`) stop
  shipping: their briefs move into the kind table, and `retire_playbooks` removes them unless the owner edited one, which
  stays his as reference. The others ship as reference without capabilities: `discovery-fix`, `chore`, `feature`,
  `canary-release`, `bug-sweep`, `wrong-number`, `backfill`, `self-development`, `prd-stories`, `objective`, `okr-review`,
  `okr-checkin`, `okr-instrument`.
- `Origin::Seed` and `HireInput.from_playbook` go: a card is the Chair's, a hire or a judge; it does not remember where the
  Chair read its brief.

### Starting each kind

| Work | Who starts it, with what |
|---|---|
| An ordinary request | The owner's new-chain box (`chain_create`, `work`), or the Assistant's `create_chain` |
| A chore | The Assistant's `create_chain` with `kind: chore`, a routine or event start with it, or `propose_chain` with it |
| Shaping or amending an objective | The Assistant's `create_chain` with `kind: objective`; the goal names the shaping, review or check-in, and the okr skill and the reference playbooks carry the procedure |
| Playbooks, agents, skills | The Assistant's `create_chain` with `kind: author`, the playbook on screen named in the intent; `ask_playbook_author` folds into it and goes |
| The Release seat | `pipeline::automation`'s `RELEASE_SEAT_SUB`, its start naming `kind: release` |
| The analyst | `crew::analyst`, queueing `kind: analyst` |
| The coach | The Assistant's `create_chain` with `kind: coach` |
| A seeded-fault trial | The Assistant's `create_chain` with `kind: trial` |
| A client key's chain | `start_chain` on `/mcp/client`, always `work`, while the key's `chainsPerDay` is above zero |

### Client keys

`ClientKey.playbooks` goes, and with it `has_engineer`: a client's chain already hires no engineer
(`orch::chair_hire`, origin `client`) and submits nothing (`shipping::submit`, origin `client`). What the allow-list also
did, a key that starts nothing until the owner says otherwise, moves to `chainsPerDay`: zero starts none, and the Assistant's
input leaves it out to take the default of 3. No flag is added.

### Outcomes and standings

- `Outcomes.playbooks` (`OutcomePlaybook`) becomes `Outcomes.kinds`; the Outcomes page reads "By kind". `verdict_rates`
  groups the Chairs' verdicts by kind: the comparison ADR-0110 asks for (chore against work) is that table.
- A duel's `work` (`model::Duel.work`) and the standings' kind of work are the chain's kind, in place of the group of the
  chain's playbook (`Crew::work_of`, `standings_all`). Work stays a fact of the chain: `ground_of` reads it from the chain
  before any card is named, and `outsides_of` scores a chain once for every lineage in it. A chain with no playbook read
  `general` before, as most owner chains do since ADR-0109; they read `work` now.

### Stored data

Migration 0118 (the next free number when this lands), each statement idempotent:

- `chains`: `$.kind` set from `$.playbookId` (`chore` → `chore`; `objective`, `okr-review` and `okr-checkin` → `objective`;
  `playbook-author` → `author`; `release`, `coach`, `analyst` → the same; `seeded-fault` → `trial`; any other, or none →
  `work`); `$.playbook`, `$.playbookId`, `$.ownerWaits` and `$.gates` removed. `$.capabilities` stays as stored: it is what
  the chain holds.
- `cards`: `$.origin` `seed` → `hired`.
- `playbooks`: `$.chairBrief` renamed `$.brief`; `$.chair` and `$.capabilities` removed. The same in an `inbox` item
  still open or decided (`status` `open` or `decided`) whose `$.change.kind` is `playbook`: its `$.change.record` is JSON
  held as text (`PendingChange.record: String`), so it is read with `json()`,
  edited, and written back as text. Settled `changes` rows keep the before and after they recorded: they are history.
- `subscriptions`: `$.action.start.playbookId` removed; a start that named `chore`, `objective`, `okr-review` or
  `okr-checkin` takes that kind (`objective` for the last three: a check-in reads objectives with `read_objective`, a
  tool of the `objectives` capability); a start that named another playbook keeps its intent in its goal, which gains
  the line "The playbook <name> (<id>) fits this work: read it with read_playbook."
- `client_keys`: `$.chainsPerDay` set to 0 where `$.playbooks` is empty or null; `$.playbooks` removed.
- `duels`: `$.work` set to the new `$.kind` of the duel's chain, so the agreement of a kind of work (`agreement_of`,
  which filters the documents `Reader::duels` returns) reads the same names as new duels; the `work` column, written only
  when a duel is inserted, is set to the same.

## Threats

- **A proposed or event-started chore lowers the gates.** A Chair that proposes a chain, or an event whose start the owner's
  Assistant made a chore, gets a QC and no reviewer. The goal judge still rules, the check still has to fail first, and a
  change to a path `.remit/rails` names still needs `security` (`required_gates`), whatever the kind. No tool changes a
  running chain's kind.
- **A playbook is no longer held.** An Author chain steered by injected text can write a misleading playbook, and a Chair
  that reads it can be misled. It gives no tool, no seat and no slot; it arrives as a tool result the Chair asked for, and
  every write is a `Change` the owner can read and put back. The seam is `hold_playbook`'s place in `create_playbook` and
  `update_playbook`.
- **A client key starts chains by its count.** The allow-list of playbooks is gone; a key whose count is above zero starts
  `work` chains, which hire no engineer, submit nothing and start with no capability. Keys that could start nothing are
  migrated to zero.

## Consequences

- One concept, one owner: content is the playbook's, tools are the capabilities', and what code branches on is the kind's.
  A playbook id is never compared in code; a capability names a tool, never a seat.
- The Playbook author's writes no longer touch rails, so its chains finish without waiting on the inbox for a capability.
- The system seats' briefs are code, changed by a chain on Remit itself and released by the Release seat, not by the
  Playbook author. The owner shapes a seat through its employee's memory and mentoring (ADR-0062).
- A `work` chain starts with no capability, as a playbook-less chain does today; its Chair gives itself `shell`, `web`,
  `signals`, `data` and the other reads, and asks the Assistant for `ship` and other writers. A chain that used
  `canary-release` for `ship` now asks once.
- Kinds are few and closed. A new kind is a code change, made when a seat needs different admission, gates, a fixed brief
  or its own identity; never for a brief alone, which is a playbook.
- Not decided here and left to the change that builds it: no new test is named. The rails it moves keep their tests: the
  migration test `documents_an_earlier_build_wrote_read_after_migrating` (crates/store/tests/store.rs) gains a chain of each
  mapped playbook id, and the tests that build a chain through a playbook's capabilities (`chain_with_mr` in
  crates/orch/tests/app/shipping_ship.rs, the `chain` helper in crates/orch/tests/app/source_tests.rs) build it through its
  kind and grants.

## Revisit when

- Outcomes' tool usage shows Chairs rarely call `read_playbook`, and goal-judge first-pass rates by kind fall against those
  before this change: the seam is the Chair's opening, where the most fitting playbook could be named as a suggestion (still
  binding nothing).
- `verdict_rates` shows `chore` chains sent back by the goal judge or reverted more often than `work` ones: `ChainKind::gates`
  is the one place to change (ADR-0110's own revisit).
- A plugin needs a seat of its own (its own admission, gates or brief): kinds become something a plugin manifest lists
  (ADR-0034), with the same table shape.
- Chairs spend turns granting the same non-writing capabilities on most `work` chains: the seam is `work`'s row in the table.
- The owner wants to steer a system seat's brief himself: the seam is the brief section, which could load a library skill
  the seat holds.
- A client needs more than `work` chains: an allow-list of kinds on the key.
- A misleading playbook is found steering a chain: hold a playbook write again (`hold_playbook`), or mark `read_playbook`'s
  answer as data in the way ADR-0112 frames a plugin's context.
