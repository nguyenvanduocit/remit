# 0107. Each record is defined once, in `model`, and every contract and the store carry it typed

Status: accepted, 2026-11-17; built in the steps of *Plan* below. Supersedes, of ADR-0072, decision 6's rule that documents
keep the JSON Go wrote (the escapes, the float form, the zero time, `null` lists) and decision 4's rule that `replica`
depends on no `model`; amends ADR-0095 decision 3 (`model`'s records become link types under its postcard rules). Every
other part of ADR-0072, ADR-0095 and ADR-0096 stands. Source: the review of 2026-11-17 ("improve toàn bộ": improve the
architecture and codebase of the Rust stack), phase 2: one contract and typed domain values.

## Context

AGENTS.md says the contract is the one definition every side compiles or generates from, and a type is never copied by
hand. The Rust stack holds four definitions of much of the domain.

1. **Records twice, bridged by JSON at run time.** `crates/model` (what the store keeps) and `crates/wire/src/app.rs`
   (what the web app reads) both declare 113 types by the same name: `Chain`, `Card`, `Edge`, `Event`, `Decision`,
   `InboxItem`, `Employee`, `Objective`, `Sensor`, … and 39 string enums. A record crosses by being written as JSON and
   read back as the other type (`crates/rpc/src/convert.rs:10-19` `wire_of`; pushes in `crates/rpc/src/broadcast.rs:195-204`,
   which logs and drops one that does not fit). The one guard reads each enum's values into the contract, one direction
   (`crates/rpc/tests/contract.rs:15` `every_model_value_fits_the_contract`). The copies have drifted already: `model::Chain`
   has `replay` and `unjudged` (`crates/model/src/model.rs:73,96`) that `wire::app::Chain` (`app.rs:343-384`) drops;
   `Decision.stance` (ADR-0103, `model.rs:967`) never reaches the page; `InboxItem.change`, `.owner_only`, `.replay`,
   `.workflow`, `Card.handover`, `Card.loadout` and `Draft.mentor` are dropped too. The contract writes every optional
   field `Option<T>` (`app.rs:348-382`), the model a zero value skipped by `is_zero` (251 fields) and `Time` where wire
   holds `String`.
2. **Views twice.** orch and the domain crates compute what the owner sees into their own types, and the page reads
   wire's copies: `Column`, `Board`, `ChainStanding`, `AgentStanding`, `Snapshot`, `ChainView`, `AssistantFeed`,
   `MentorStanding` (`crates/orch/src/standing.rs:26,81,201`, `agentstanding.rs:29`, `views.rs:64,129`,
   `assist_feed.rs:20`, `mentor_standing.rs:36` against `wire/src/app.rs:1883,1907,1954,2007,2764,1706,3071,3574`), and
   about 60 more (`chains::health::Health` `health.rs:22` against `app.rs:1414`; `shipping::verification::{Verdict, Gate}`
   `verification.rs:51,121` against `app.rs:1822,1846`; pipeline's signal board; crew's lineage standings). They differ:
   `ChainStanding.wake_at: Option<Time>` renamed `checkBackAt` (`standing.rs:208-209`) is `check_back_at: Option<String>`
   in wire (`app.rs:1960`), `place: usize` skipped at zero is `Option<i64>`. Values the server derives for a record ride
   on it by `#[serde(flatten)]` (`chains::wait::ChainRead`, `crates/chains/src/wait.rs:156-161`) or by `serialize_with`
   (`chains::shown_decision`, used at `orch/src/views.rs:88,133`, `assist_feed.rs:29-39`). One builder already writes the
   contract's type directly: `orch/src/boards.rs` builds `wire::app::Boards`.
3. **Runtime types three times.** `agent` (the session port) declares `Task`, `ToolCall`, `Note`, `Limit`, `Failure`
   and a `Decision` enum (`crates/agent/src/tasks.rs:12`, `toolcall.rs:60`, `agent.rs:59,117,182`, `failure.rs:35`);
   `wire::runtime` declares them again (`crates/wire/src/runtime.rs:35,56,148,156,201,239`), with strings where `agent`
   has enums (`ToolKind`, `ToolStatus`, `NoteKind`, `agent/src/toolcall.rs:7,39`, `agent.rs:136`); `model` declares
   `SessionInit` and `SessionContext` (`model.rs:1173,1203`) which `wire::runtime` (`runtime.rs:285,303`) and `wire::app`
   (`app.rs:2244,2298`) copy. `crates/airuntime/src/convert.rs:228-339` and `crates/runtimelink/src/convert.rs:197-320`
   map them field by field, the latter with `..model::SessionContext::default()` and `..model::SessionInit::default()`
   (`runtimelink/src/convert.rs:255,310`), so a field forgotten in the map is silently empty. `wire::runtime::Decision`
   (`{allow, message}`, `runtime.rs:148`) is a permission answer named as the domain's `Decision`, and allows the
   illegal pair `allow: true` with a message that `agent::Decision` (`Allow | Deny { message }`) cannot hold.
4. **Three enum mechanisms, and tolerance in the type.** `model`'s `text_enum!` (`crates/model/src/text_enum.rs:18-40`)
   gives every enum an implicit `Unset` (`""`) and an `Unknown(any text)` kept and written back; `wire` derives serde
   enums with no such variants; `agent` writes `as_str` by hand. A stored value `model` reads as `Unknown` fails the
   contract's read, so one bad value in one record fails every snapshot that carries it. `crates/model/src/stored_json.rs`
   (237 lines) and the crate header (`crates/model/src/lib.rs:1-14`) keep documents byte-compatible with Go's
   `encoding/json` (HTML escapes, Go's float form, the zero time `0001-01-01T00:00:00Z`, `null` for an unset list read
   through `nullable`), ADR-0072 decision 6's rule for the cutover; no Go reads the database since that release.
5. **Domain values as strings, and state read back from display text.**
   - A gate verdict (`Approval.verdict`, "approve" | "request_changes") and the role that signs it (`Approval.role`,
     "reviewer" | "qc" | "security" | "judge", `model.rs:266-281`) are `String`, compared as literals across chains
     (`step.rs:934,998`, `gate.rs:42,63`), shipping (`gate.rs:246,459`), outcomes (`outcomes.rs:171`) and crew
     (`lineage_standing.rs:278`); the gate names are `&'static str` constants (`chains/src/gate.rs:10,13`,
     `chains/src/goal.rs:69`) and `MRRef.gates` is `Vec<String>` (`model.rs:472`). A Chair's ruling on a report in a
     fact is `"accept"` | `"rework"` text (`chains/src/fact.rs:104-106`, read at `crew/src/coach.rs:65`).
     `model::Stance` (`model.rs:972-981`) is the Assistant's answer, a different concept.
   - `Event.status` (`model.rs:799-800`; `Option<String>` in wire, `app.rs:1025`) holds a different set per event kind:
     a prompt's queue state (`queued`, `delivered`, `steered`, `dropped`), a tool call's (`pending`, `in_progress`,
     `completed`, `failed`), a job's or task's (`running`, `stopped`, …), and for a system event what happened
     (`new_session`, `permission_asked`, `permission_answered`, `moved`, `error`, `retrying`, a block). The store writes
     them as SQL literals (`crates/store/src/events.rs:127,220,233,253,293,385,392,445,460,475,520,533`).
   - What role a card has is decided in several places from `origin` plus the `agent` text: `is_goal_judge`
     (`chains/src/goal.rs:73-75`, `origin == Judge && agent == "judge"`), `is_duel_judge` (`duel.rs:12-14`),
     `is_second_judge` (`second_opinion.rs:14-16`), while `verdict_role` reads `signs` (`chains/src/gate.rs:28-33`): two
     definitions of a goal judge. Sites branch on them in turn (`session.rs:981-1016`, `turn_end.rs:128-136,245-250`,
     `lifecycle.rs:195-211`, `outbox.rs:214-218`).
   - State recovered from text written for display: a task cut because its chain slept is found by its text
     (`chains/src/wait.rs:392-395`); the owner's words are recovered by stripping the "[message from …]" line the agent
     reads (`chains/src/outbox.rs:114-118`, `owner_said`); whom a permission waits on is read from the card's activity
     line (`orch/src/agentstanding.rs:635-647`, prefixes `ASKING_CHAIR`, `ASKING_OWNER`, "waiting for ").
   - `crew::employee_rules::HireRefusal.kind` is `&'static str` ("ceiling", "cap", "title", "reuse",
     `crates/crew/src/employee_rules.rs:23-27`).

The customer is the owner, through every agent that changes Remit: a field added to a record should be one edit that the
compiler carries to the store, the server, the link, the client core and the page, and a value that can only be one of
a set should be a type the compiler checks at every branch.

## Options

### a. Which crate owns the records

1. **Keep two definitions; replace the JSON bridge with typed `From` impls and a round-trip test per record.** The drift
   becomes a compile error at the `From`, but every field is still written twice (113 types) and once more in the map.
   AGENTS.md forbids the copy. Rejected.
2. **`wire` owns every record; `model` is deleted into it.** One crate for everything that crosses or is stored. 327
   files rename `model::` to `wire::`; the store and the pure crates (`signal`, `library`, `rollout`, …) come to depend on
   the protocol crate and on `link` through it; protocol (calls, pushes) and records share one crate and one rebuild.
3. **`model` owns every record; `wire` carries `model`'s types in its calls, pushes and views (chosen).** `model` stays the
   leaf that names the domain (serde, serde_json, schemars, time: all build for wasm32), and adopts the contract's rules
   (postcard-safe, tsgen-readable). `wire` keeps what only a contract has: calls, pushes, envelopes, and the view types the
   server computes. tsgen already reads the crates a module reaches (`crates/tsgen/src/lib.rs:29-31`), so the page's
   TypeScript is generated from `model` too. `replica` and `client-web` depend on `model`.
4. **`model` owns, `wire` is generated from it (a Rust-to-Rust generator).** A generator to maintain, for nothing option
   3 lacks. Rejected.

Option 2 against 3: both give one definition. 3 keeps `model` a leaf, keeps the store off the protocol, and keeps 327
imports as they are; 2's only gain is AGENTS.md's wording, which this record amends (*Consequences*). The rule that
separates them is one line: **a type the store keeps, or that more than one contract carries, is `model`'s; a type only one
contract carries (a call's arguments, a push, a view the server computes) is that contract's module of `wire`.**

### a′. How a stored value this build does not name is read

1. **Keep `Unknown(text)` and implicit `Unset` in every enum, and carry them to the page.** Every `match` on every side
   keeps an arm for a value nobody named; the page renders "unknown". The type no longer says what is legal.
2. **Strict enums; a record that does not parse fails its read.** One row written by a newer build fails every list it is
   in, so a rollback across an added enum value breaks the app the rollback was meant to restore.
3. **Strict enums; the store parses each row once at its boundary, and a row it cannot read is left out of the list,
   logged once and counted (chosen).** The type says what is legal; a rolled-back build shows everything else and says how
   many records it cannot read. A row left out is never written by this build (it was never read), so nothing is lost.

### b. View types

1. **Keep orch's types and convert each to wire's by hand.** A third definition. Rejected.
2. **orch's pure builders return `wire::app`'s types (chosen),** as `boards.rs` does; orch's copies are deleted. A value the
   server derives for a record goes on a read type that holds the record as a field (`ChainRead { chain, check_back }`,
   `EventRead { event, activity }`, `DecisionRead`, `EmployeeRead`), never `flatten` (postcard cannot carry it, ADR-0095
   decision 3) and never `serialize_with` (the type then says one thing and the JSON another).

### c. Runtime types

1. **`agent` owns them and `wire::runtime` re-exports them.** `wire` would depend on `agent`, a shell crate that starts
   processes; `wire` builds for wasm32. Rejected.
2. **`wire::runtime` owns what only the runtime contract carries; `agent`'s port speaks those types; what the core stores
   (`SessionInit`, `SessionContext`) is `model`'s (chosen).** `agent` gains the edge to `wire` (`airuntime` and
   `runtimelink` have it already). The enums `agent` has (`ToolKind`, `ToolStatus`, `NoteKind`) replace wire's strings;
   `agent::Decision` replaces `wire::runtime::Decision` as `PermissionAnswer`.

### d. Typed values

1. **Constants beside the strings** (`const APPROVE: &str`). The compiler still cannot see a branch that misses a value.
   Rejected.
2. **An enum per value set, in the crate that owns the record that holds it, text unchanged (chosen).** A stored text
   stays the same, so most values need no data migration. Where a value is read back from display text, the fact is
   stored as its own typed field and the text is rendered from it.

## Decision

### 1. `model` owns the records, `wire` carries them

- Every record the store keeps, and every type two contracts carry, is declared once in `model`. `wire::app`,
  `wire::runtime` and `wire::plugin` name `model`'s types in their calls, pushes and views; their copies are deleted. Server
  code keeps writing `model::Chain`; client code writes the same. `wire` re-exports nothing.
- `rpc::convert::wire_of` and `broadcast::wire` are deleted for records and views: a channel answers with the value. What
  stays in `convert.rs` is the error mapping (`failed`, `done`).
- `make gen` passes `crates/model/src` to tsgen for `wire::app` (`Makefile:9`). tsgen learns `text_enum!` (as it learned
  `json_tagged!`, `crates/tsgen/src/lower.rs:210,280`) and a newtype that serializes as a string or a number through
  `#[serde(into = "String", try_from = "String")]` (`Time`) or `into = "i64"` (`Duration`).
- `replica` and `client-web` depend on `link`, `wire` and `model`. `model` holds records and their own invariants (a
  constructor, a private field's setter, a check returning `model::Invalid`), never a rule that weighs one record against
  another or against policy: such a rule lives in the domain crate that owns it, so the client core still cannot decide
  (ADR-0072 decision 4's intent, held now by this rule and by review instead of the missing edge).
- A field that must never reach a channel is not on a record: it is kept apart, as `ClientKey`'s key already is
  (`crates/model/src/client_key.rs:6-8`).

### 2. `model` follows the contract's rules

- **One enum mechanism.** `text_enum!` is the only way to declare a string enum, in `model`, `wire`, `agent` and the
  domain crates (`#[macro_export]` from `model`). It loses `Unknown`. It loses the implicit `Unset`: a field that may be
  absent is `Option<E>`; an enum whose empty text is a named value says so (`Stance::Neither = ""`). Its `JsonSchema` lists
  its values, so an agent's tool input that names a value outside the set is refused by the schema.
- **Fields.** `skip_serializing_if` is `Option::is_none` only (tsgen's closed set, `crates/tsgen/src/attrs.rs:88-93`, and
  ADR-0095's postcard rule); every other field is always written, and records keep `#[serde(default)]` so a document
  stored before a field existed reads. A time that may be unset is `Option<Time>`; `Time::ZERO` and `Time::is_zero` go.
  `nullable` and `present` go. `model::RawMessage` and `wire::ForeignJson` become one type in `model` with
  `ForeignJson`'s postcard form (text on the link, JSON elsewhere).
- **The writer.** `stored_json::to_string`'s formatter (Go's HTML escapes and float form) is deleted: documents are written
  by `serde_json`. Every document either writer produced reads with `serde_json`. Times keep their text (RFC 3339 with
  the offset written, trimmed fraction: `accounts` orders by `$.createdAt` text, `crates/store/src/accounts.rs:35`) and a
  `Duration` stays integer nanoseconds.
- **Data migrates once.** One numbered migration per convention rewrites the documents an earlier build wrote: an enum
  key holding `""` removed where the field became `Option`; a time holding `0001-01-01T00:00:00Z` removed; a `null` list
  removed (it reads as empty through `default`). Each names its tables and JSON paths (enumerated from the 41 `nullable`
  and the `is_zero` `Time` fields), so no foreign JSON a record carries is touched. Events are not rewritten (*Constraints*,
  `docs/design.md`: a migration never rewrites events); their enum texts are unchanged.
- **The store's boundary parses once.** A row reader that cannot parse a record (an enum text this build does not name, a
  malformed document) leaves the row out of its result, logs `table`, `id` and the error once per process, and counts it;
  the count is a field of `Health` that the owner's Health view and the Assistant read. A read of one record by id that
  cannot parse returns a typed `store::Error::Unreadable`.

### 3. Views are `wire::app` types

orch's and the domain crates' view types that `wire::app` copies are deleted, and their pure builders return the
contract's type: the standings (`Column`, `Board`, `BoardColumn`, `ChainStanding`, `AgentStanding`, `MentorStanding`,
`ChainTone`, `CheckTone`), `Health`, `Workload`, verification (`Verification`, `GateView`, `VerdictView`), the signal
board's lines, lineage standings, `Snapshot`, `ChainView`, `AssistantFeed`, `AssistantView`, `CardContext`, the pages. A
count is `u32` and a time `Time` in the contract as in the domain. Derived values sit on read types that hold the record
(*Options b*); `chains::shown_decision` becomes the builder of `DecisionRead`.

### 4. Runtime vocabulary has one owner

`wire::runtime` declares `Task`, `ToolCall` (with `kind: ToolKind`, `status: ToolStatus`), `Note` (`kind: NoteKind`),
`Limit`, `Failure` and `PermissionAnswer` (`Allow | Deny { message }`, `json_tagged!`), and `agent`'s port (`Handler`,
`Session`) takes and gives them; `agent`'s copies are deleted. What Claude Code reports at a session's start and rest is
`model::SessionInit` and `model::SessionContext` as reported (no `card_id`, `home`, `at`); the store keeps them keyed by
card with those three beside the report, so no field is left for a handler to fill. `airuntime/src/convert.rs` and
`runtimelink/src/convert.rs` keep only what is not a copy: cutting long strings (`cut`, `FIELD_LIMIT`) and the session
spec and exit mapping.

### 5. Typed values

| Value | Type, owner | Replaces | Stored text | Migration |
|---|---|---|---|---|
| A gate verdict | `model::Verdict { Approve, RequestChanges }` on `Approval.verdict` | `String` (`model.rs:276`) | same | none |
| Who signs a verdict | `model::VerdictRole { Reviewer, Qc, Security, Judge, CheckReview }` on `Approval.role`; `model::Gate { Reviewer, Qc, Security }` on `MRRef.gates` and `Submit.gates`, `VerdictRole::gate()`, `Gate::role()`; what a card signs as, `Signs::gate()` and `Signs::verdict_role()`; the gates every change needs, `chains::gate::GATES: [Gate; 2]` (a rule, so not `model`'s) | `String`, `&str` `GATES`, `SECURITY`, `CHECK_REVIEW_ROLE`, `GOAL_JUDGE_ROLE` as a verdict's role (it stays the goal judge's agent until 7b), `gate_closed_by`, `verdict_role` | same | none |
| The approval a finish waits on | `Finishing.approval_id` | `Finishing.verdict: String` holding an id (`model.rs:255`) | renamed key | documents: `$.finishing.verdict` → `$.finishing.approvalId` |
| A Chair's ruling on a report | `model::ReportVerdict { Accept, Rework }` on the `teach.verdict` fact | `FactValue.verdict: String` (`fact.rs:106`) | same | none |
| A judge's kind | `model::JudgeKind { Intake, Goal, Check, Duel, Second }` on `Card.judge: Option<JudgeKind>`, set where Remit starts the judge | `agent` text compared to role constants | new key | cards: `$.judge` from `$.agent` (and `signs`) where `origin = 'judge'` |
| A card's role | `model::CardRole::of(&Card)`: `Chair`, `Assistant`, `Worker`, `Spawned`, `Judge(JudgeKind)`, one function from `origin`, `judge`, `spawn` | `is_goal_judge`, `is_duel_judge`, `is_second_judge`, the `Origin` tests at each site | derived | none |
| An event's status | `model::EventStatus`, one variant per kind with its own enum: `PromptStatus`, `ToolStatus`, `JobStatus`, `TaskStatus`, `SystemNote`; parsed with its kind in one place (`EventStatus::read(kind, text)`), written as its text | `Event.status: String` and its constants (`EVENT_PERMISSION_ASKED`, `NEW_SESSION`, `EVENT_BLOCK`) | same | none; every pair an earlier build wrote is named (*Plan* 7c) |
| Whom a permission waits on | `model::PermissionWait { Chair, Owner, Driver { who } }` on `Card.permission_wait: Option<_>` | prefixes of `Card.activity` | new key | cards in status `permission`: derived from the prefix |
| Why a task was cut | `model::Cut` in the stopped task event's data (`cut`) | `e.text == cut_short(Cut::Slept)` | new key in new events | none: older events converted on read (below) |
| The owner's words | a prompt stores what he wrote, with `by`, marked as his in its data (`ownerWords`); the "[message from …]" line is written when the prompt is delivered to the session | `owner_said` stripping the line back off | new events hold his words only | none: older events converted on read (below) |
| A refused hire | `crew::RefusalKind { Ceiling, Cap, Title, Reuse }` | `HireRefusal.kind: &'static str` | not stored | none |

- SQL takes an enum's text as a bound parameter (`?` with `PromptStatus::Delivered.as_str()`), never as a literal in the
  statement, so the store's queries name values the compiler checks.
- Events written before 7e have the old shape. A migration records where the new shape starts (a one-row-per-shape table,
  `event_shapes (shape TEXT PRIMARY KEY, from_id INTEGER NOT NULL)`, `INSERT OR IGNORE` of the next event id), and the
  store's event reader alone converts an event below it (the voice line stripped, the cut read from its text): the old
  shape is parsed once, at the boundary, by id, never guessed from content.
- The view types `shipping::verification::{Verdict, Gate}` and `wire::app::{Verdict, Gate}` become `VerdictView` and
  `GateView`, and `chains::repeats::Verdict` becomes `Repetition`, so `Verdict` and `Gate` name one concept each.

## Threats

A field the contract's narrower copy dropped reaches every channel once the record crosses as itself, a mentor's session
included (ADR-0089): `Card.loadout` (the agent's instructions and tools), `Card.handover`, `InboxItem.change`, `.replay`,
`.workflow`, `.owner_only`, `Draft.mentor`, `Chain.replay`, `Decision.stance`. None is a credential (credentials are kept
off records, *Decision 1*). Step 4 lists every such field per record and decides each: sent, or moved off the record.
`MentorApi` still chooses which records a mentor reads; it cannot choose fields.

Decided in step 4b, for the records that cross from it (`Chain`, `Card`, `Edge`, `Decision` and what they hold): every
field newly reaching a channel, a mentor's session included, is sent. None is a credential, and each is what the owner or
a mentor helping the chain reads of its work.

| Field | Why it is sent |
|---|---|
| `Chain.replay` | names the change item a replay chain tries; the item's own fields stay the owner's |
| `Chain.unjudged` | why a finish had no goal ruling: the result a mentor helps judge |
| `RepoWork.host`, `.pushed_from`, `.earlier_mrs`, `.submit` (its title, description, body, gates); `MRRef.decided_at`, `.by`, `.version`, `.change`, `.gates`, `.checks` | the chain's own code work, already public on its forge to whoever reads the MR |
| `Finishing.recheck` | whether the finish is a recheck after merge |
| `Card.loadout` (instructions, tools, skills) | what the agent works with: a mentor coaching it reads it; a secret never goes in instructions (credentials stay off records, *Decision 1*) |
| `Card.handover` (employee, agent, why, by, until) | who takes the card's work and why, which the chain's stream already says |
| `Card.ended_at` | when its work ended |
| `Decision.stance` | the Assistant's answer to an item, already in the decision's words |
| `Actor.who`, `.via` written empty instead of absent | no new fact |

A decision's `outcome` rides on `DecisionRead` where a chain's stream reads it (`ChainView.decisions`, `decision_added`);
the analyst's insights and the Assistant's feed lines carry `Decision` alone, as no page reads an outcome there.

Decided in step 4c, for `Event`, `InboxItem` and `Draft` and what they hold: every field newly reaching a channel is sent.
An event reaches every session that shows its home, a mentor's included; an inbox item and the owner's drafts reach the
owner alone (`Audience::Owner`), a mentor's draft that mentor alone (`Audience::Writer`).

| Field | Why it is sent |
|---|---|
| `Event.tool_kind`, `.tool_name` | the tool a call used, which its title already names |
| `Event.data`, whole (a prompt's `note`, a recovered session's `lead`, a block's pick's `block`, a turn's `turn`, a reply's `marks`, beside the keys `EventData` names) | what the agent read or the stream shows of the event; a lead is the recap and brief the session reads, which a mentor coaching it reads in `Card.loadout` already; no credential is written into an event |
| `InboxItem.change` (the record a change item would write, with its fields) | owner only: the write he or the Assistant for him allows; a record holds no credential (*Decision 1*) |
| `InboxItem.replay`, `.workflow`, `.owner_only`; `Retry.card_id`, `.prompt_id`, `.reopen` | owner only: how the item was replayed, the workflow step it answers, whether it waits for him, what its Retry does |
| `Draft.mentor` | to its own writer only |
| `Event.priority` | not sent: it orders a card's queue in the store (`#[serde(skip)]`) |

Decided in step 4d, for the rest of the records (the library's, the organisation's, the signal side's, a chain's workflow
runs, the Assistant, what a session was given): every field newly reaching a channel is sent. A mentor's session reads,
of these, only their own `Mentor`, the `Help`s offered to them, and a helped chain's view; the rest reach the owner alone.

| Field | Why it is sent |
|---|---|
| `Change.before`, `.after` (the record before and after a write, as JSON) | owner only (`changes_get` refuses a mentor): the record's own history, which its fields already show; a record holds no credential (*Decision 1*) |
| `Sensor.cursor` | owner only: where a poll has read up to, beside the query he already sees |
| `WorkflowStep.hash`, `.arrived` | a run's own step: which call it was and how many results came back, which the step's verdict already says |
| `KeyResult.check.telemetry`, `.incident` | owner only: how a result is seen and taken back in production, which the Chair stated for him |
| Fields written empty or `false` instead of absent (`Playbook.chair`, `Agent.model`, `Mentor.strengths`, `Help.why`, …) | no new fact |

## Consequences

- A field is added once: `model`, then `make gen`. The compiler finds every Rust use and `vue-tsc` every page use. The
  runtime JSON bridge and the enum-values test go, because the drift they guarded cannot be written.
- `model` is a shared kernel of the server, the runtime, the client core and the page. Vernon's warning against a large
  shared kernel (coordination between teams, `aio-architect-reference` *Context Mapping*) assumes separately released
  sides; here every side is built from one tree in one release, and a page of another build reloads (ADR-0095 decision
  3, `new_bundle`). A translation layer is for a model one cannot change (*Anti-Corruption Layer*, *When NOT to Use*: "When
  you control both systems ... align the domain models"); Remit controls all of them.
- A change to a record's shape changes the generated TypeScript in the same commit; a stored shape change is still a
  numbered migration. Storage and contract move together on purpose.
- A rolled-back build hides the records it cannot read and shows how many, instead of `Unknown` values the page cannot
  draw. A record hidden that way is untouched until a build that reads it comes back.
- Documents written from step 3 lack Go's escapes; an idempotent act asked before and replayed after it with `<`, `>` or
  `&` in its arguments hashes differently and is refused as another action under the same id, never done twice (the
  same case as ADR-0095 decision 3).
- AGENTS.md's sentence on the contract changes to: "The records (`crates/model`) and the contracts that carry them
  (`crates/wire`) are the one definition every side compiles or generates from: never copy a type by hand." Made in the
  commit of step 4a.

## Plan

Each step is one commit (≤800 lines changed outside generated files, or a `Large-change:` line saying why,
`scripts/commit_size.sh`). Its guard is written first: seen failing before the change where the step fixes a defect, seen
passing before and after where it protects stored data. Tests are added only for stored data, the contract read across
versions and the rails (AGENTS.md *Few tests*); the compiler carries the rest. Steps marked ∥ run at the same time, each
in its own worktree, and land in the order listed. Migration numbers are taken when a step lands, after the store
write-discipline series (0109 onward).

1. **tsgen reads `text_enum!` and text newtypes.** Crates: `tsgen`, `model` (`#[macro_export]`). Guard: tsgen unit tests
   for a `text_enum!` (a union of its texts) and a `#[serde(into = "String")]` newtype (`string`); `make check-gen`
   output unchanged. tsgen also follows glob `use` items (`model`'s root re-exports each module with one), and
   `Duration` is declared `into = "i64", from = "i64"` (the JSON and postcard of `transparent`). `Time` keeps its
   hand-written serde, whose read turns `null` into the zero time, until 3b drops the zero time and declares it
   `into = "String", try_from = "String"`. Group A, ∥ step 2.
2. **`RefusalKind`.** Crates: `crew`. Guard: none (the compiler). Group A.
3. **`model` follows the contract's rules** (*Decision 2*), in three commits, alone (it touches every crate that matches
   on `Unknown` or uses a zero time). After the write-discipline series lands.
   - 3a. Strict `text_enum!`, `Option<E>` for absent values, the store's row quarantine and its `Health` count; migration
     removing `""` enum keys. Crates: `model`, `store`, `chains`, `orch`, `capability`, `crew`, `library`, `claudecode`,
     `agent`. Guards: `store` tests `documents_an_earlier_build_wrote_read_after_migrating` (old-shape documents seeded by
     raw SQL in each document table: `""` enums, zero times, `null` lists, Go escapes, local offsets; passes before and
     after) and `a_record_this_build_cannot_read_is_left_out_and_counted` (fails before: today it reads as `Unknown`).
   - 3b. Fields: `Option::is_none` the only skip, `default` on records, `Option<Time>` (and `Time` declared
     `into = "String", try_from = "String"`), `nullable` and `present` gone;
     migrations removing zero times and `null` lists at their named paths. Crates: `model`, `store`, every caller of a
     changed field. Guard: 3a's first test, extended to each path; `wire/tests/round_trip.rs`
     `each_shape_keeps_its_json_and_crosses_postcard` extended to a filled value of every `model` record (fails before:
     `is_zero` and `RawMessage` do not cross postcard).
   - 3c. The writer: the Go formatter deleted; `RawMessage` and `ForeignJson` one type in `model`. Crates: `model`, `wire`,
     `store`. Guard: 3a's first test.
4. **Records cross as themselves** (*Decision 1*), four commits in order, each deleting `wire::app`'s copies, regenerating
   the TypeScript and fixing the page. Crates: `wire`, `model`, `rpc`, `chains`, `orch`, `replica`, `client-web`, `web`,
   `Makefile`. Guard, written in 4a: `rpc/tests/contract.rs` `a_filled_record_crosses_to_the_client_unchanged` (a filled
   record answered by `rpc`, decoded by `replica`, equal; fails before for `Chain`, `Decision`, `InboxItem`, `Card`).
   `every_model_value_fits_the_contract` and `a_mentor_snapshot_fits_the_contract` are deleted as their records cross.
   - 4a. The 39 shared enums and the rest of `wire::app`'s enum copies; AGENTS.md's sentence.
   - 4b. `Chain` with `ChainRead`, `Card`, `Edge`, `Decision` with `DecisionRead`, `Approval`, `Finding`, `Finishing`; the
     *Threats* list decided.
   - 4c. `Event` with `EventRead`, `InboxItem`, `Draft`. Built: `EventRead { event, activity }` (`chains::activity::
     event_read`) replaces `rpc::convert::event_of`; `Activity`, `Asker` and `ToolState` (the channel's running,
     completed or failed, apart from `model::ToolStatus`) are the contract's, built by `chains::activity`; the owner's
     Start over is computed once, by the server, as `Activity::StartedOver`; `FailureDetail` (stored with a request) is
     `model`'s, and `json_tagged!` moved to `model::tagged` with `json_paired!`, which writes `Event`'s status as the two
     texts `kind` and `status` and which tsgen reads as the union of each kind with its statuses; tsgen refuses a
     hand-written serde wherever the type is reached, but a newtype over `serde_json::Value` (`ForeignJson`, `unknown`).
     An event's data stays JSON its writer per kind defines; the keys a channel reads are `wire::app::EventData`, read so
     by `chains::activity` and the page.
   - 4d. The remaining shared records (`Employee` with `EmployeeRead`, `Mentor`, `Help`, `Objective`, `Sensor`, …);
     `wire_of` and `broadcast::wire` deleted. Built: `wire::app`'s copies of `Playbook`, `Seed`, `Agent`, `Source`,
     `Change`, `Sensor`, `EventSignal`, `ClientKey`, `Extension`, `Feedback`, `InboundEvent`, `FiringKey`, `NearMiss`,
     `Workflow` with its steps, `ContextGiven`, `Objective` with what it holds, `Project`, `Employee` with its ceiling,
     `Mentor`, `Help`, `Retrospective` and `Assistant` are gone, and the contract carries `model`'s;
     `EmployeeRead { employee, shown, tag, face }` and `RetrospectiveRead { retrospective, by_name }` are the contract's,
     nesting the record, as `ChainRead` does. `model::Agent` keeps its loadout under `loadout` (migration 0129 moves the
     keys an earlier build flattened among the agent's own, in `agents` and in a change item's record), and a horizon an
     agent writes as a bare day is read once, by the objective tools (`HorizonDraft`), so `Horizon` derives its serde.
     Every push and answer of a record is sent as it is (`rpc::convert::returned`); `wire_of` and `broadcast::wire`
     remain for the views alone (`Health`, the standings, boards, `ToolSource`), which steps 5a and 5b type. Guards:
     `wire` test `an_agent_a_change_and_an_employee_cross_the_link_unchanged` (fails with the loadout flattened: the
     link's decode panics) and `store`'s test of documents an earlier build wrote (agents and a change item flattened).
5. **Views are `wire::app` types** (*Decision 3*). Crates: `orch`, `chains`, `shipping`, `crew`, `pipeline`, `capability`,
   `wire`, `rpc`, `web`. Guard: 4a's test over a snapshot and a chain view built by orch.
   - 5a. The standings, `Health`, `Workload`, verification, signal board and lineage views. ∥ 4, after the orch
     views-lock refactor lands (both edit `orch/src/views.rs`).
     Built, first part: `Health` with `AccountHealth`, `Workload`, `Verification` with `GateView`, `GateState`, `Judge`,
     `VerdictView`, `Move` and `CheckLook`, `ChainStanding`, `AgentStanding`, `MentorStanding`, `Board` with its columns
     and the tones (`chains::tones` deleted) are declared once in `wire::app`; chains, shipping, capability and orch build
     them, and their own copies are gone; rpc pushes them as they are. Counts are the server's `usize` and times typed
     (`Time`); an account's texts are sent empty rather than absent (*Decision 2*). Left for the second part: the signal
     board (`SignalBoard`, `SignalDetail`, `SignalRow`) and the lineage views (`WorkDuels`, `LineageStanding`,
     `LineageRating`, `OutsideEvidence`, `OutsideResult`), whose methods and Chair's words become crew's functions.
     Built, second part: the signal board (`SignalBoard`, `SignalDetail`, `SignalLine`, `SubscriptionLine` and their
     parts, `SignalRow`, `SignalSource`, `SignalSourceLine`) and the lineage views (`WorkDuels`, `LineageStanding`,
     `LineageRating`, `OutsideEvidence`, `OutsideResult`) are declared once in `wire::app`; pipeline, chains and crew
     build them, crew's methods on them are its functions (`standing_words`), and rpc pushes and answers them as they
     are. `SignalSource` names a line with no source `None` (sent as `""`), and the board's texts and counts are always
     sent, empty or zero, so no `skip_serializing_if` but `Option::is_none` is left in `wire::app`; the subscription's
     hold is `model::Hold` (wire's `SubscriptionHold` deleted). The verify seeds build these through `verify/records.ts`
     and the fake snapshot is no longer cast, so tsc checks it against the contract.
   - 5b. `Snapshot`, `ChainView`, `AssistantFeed`, `AssistantView`, `CardContext`, the pages; after 4d and 5a.
     Built, first part: `EventRecord`, `SkillFileText`, `EmployeeRuns` with `Run`, `ChainPage`, `CardContext`, and
     `CardTelemetry` with `ModelUse`, `ToolUse` and `ApiError` are declared once in `wire::app` and answered as they are;
     `Boards` and `AtWork` are pushed as they are. The telemetry store gives a card's events and `chains` sums them
     (`telemetry::summarize`), so the store names no view. Times are typed, so the replica's own RFC 3339 parser
     (`replica::time`) is gone.
     Built, second part: the outcomes report is `model::OutcomeReport` with its parts (`OutcomeSpan`, `OutcomePeriod`,
     the KPIs and the usages), since the owner's channel and the analyst's and a client's tools all carry it; `outcomes`
     keeps only the computation (`Span` lays out the `OutcomeSpan` it shows, `describe` words a report). `wire::app::
     Outcomes` carries `report` beside `analyst` (`OutcomeAnalyst`, built by crew) rather than flattening it, and
     `outcomes_get`'s range and window reach crew typed, so their parsing from words is gone.
     Built, third part: every push is sent as it is, so `broadcast::publish` and its JSON bridge are gone. `CardShown`,
     `EventDelta` (the core's `Delta`, named as its push), `AssistantFeed` with `FeedLine`, `ListedSkill` with
     `SkillFile`, and a source's view `ToolSource` with `SourceTool`, `ConnectionView`, `SourceVariable` and
     `SourceVarState` (the source crate's `Entry` and its parts) are declared once in `wire::app`. A view and a record
     of one thing are named apart: `ListedSkill` lists a `model::Skill`, `ConnectionView` shows a
     `model::SourceConnection`. A streamed chunk's offset is a `usize` on every side. `FeedLine` crosses in camelCase:
     its `takesEffectAt` never reached the page through the bridge, which read the server's snake_case key as absent.
     Built, fourth part: `Snapshot`, `ChainView`, `AssistantView`, `EventPage`, `Who`, `Away` with `AwayLine`, and the
     catalog's `CapabilityView` are declared once in `wire::app`; the store's records of one chain are
     `model::StoredChain`, which orch lays out as the `ChainView`. Where the owner is as he writes to the Assistant is
     one `model::Where` (his prompt event stores it), so the `chains` copy with empty strings for none is gone. The
     calls a channel makes reach the orchestrator typed: `changes_get` passes its `ChangeKind` (the tool alone parses
     words), `chain_create` builds its `CreateChain`, and `draft_save`'s `DraftInput` becomes a `model::Draft` by
     `From`. `rpc::convert` keeps only `returned`, `failed` and `done`: no value crosses by way of JSON.
6. **Runtime vocabulary** (*Decision 4*). Crates: `wire` (`runtime.rs`), `agent`, `airuntime`, `runtimelink`, `model`,
   `chains`, `orch`, `store` (the session table, migration if its document changes shape). Guard: a filled `Task`,
   `ToolCall`, `Note`, `SessionInit` and `SessionContext` sent by `airuntime` over `runtimelink::memory` arrive equal at
   the core's handler (fails before if a map forgot a field). ∥ 4 and 5a, after 3.
7. **Typed values** (*Decision 5*), one commit each.
   - 7a. `Verdict`, `VerdictRole`, `Gate`, `ReportVerdict`, `Finishing.approval_id` (migration), the view renames.
     Crates: `model`, `chains`, `shipping`, `outcomes`, `crew`, `orch`, `capability`, `wire`, `web`. Guard: 3a's first
     test seeded with a finishing chain. ∥ 4, 5a, 6, after 3.
   - 7b. `JudgeKind`, `CardRole::of` (migration). Crates: `model`, `chains`, `crew`, `orch`. Guard: 3a's first test seeded
     with a card of each judge kind; the chain main path tests of `orch/tests/app`. After 4b.
   - 7c. `EventStatus` per kind, SQL parameters. Crates: `model`, `store`, `chains`, `orch`, `agent`. Guard: `store` test
     `every_event_kind_and_status_an_earlier_build_wrote_reads`, its pairs taken from `SELECT kind, status, count(*) FROM
     events GROUP BY 1, 2` on the dev database and a copy of the owner's. After 6.
   - 7d. `PermissionWait` (migration). Crates: `model`, `chains`, `orch`. Guard: `orch/tests/app/orch_waiting.rs` for each
     of the three waits. After 7b. Built: `Card.permission_wait` is private, set with status `permission`
     (`Card::ask_permission`) and cleared by `set_status` to any other; `chains::permission` writes the card's wait from its
     request due first, and `Core::asked_of` (which read the live run) is gone. With it, ADR-0059 decision 3 is amended: at
     the limit a session with no Chair above it is refused by the rule, never its own decider. The migration takes the
     wait of a card left in status `permission` from its activity's prefix (`waiting for the Chair: ` → chair, else owner) (migration 0128). Guards as built: `orch/tests/app/permission_tests.rs` (the refusal at the limit, a rail) and
     `store`'s test of documents an earlier build wrote (cards left asking).
   - 7e. The cut cause and the owner's words, `event_shapes` (migration). Crates: `chains`, `store`, `orch`. Guard: a
     `store` test reading an old-shape and a new-shape owner's prompt and slept task each as the new shape. After 7c.
     Built: `model::Cut` (moved from `chains::session`) is written into a stopped task's data by `Cut::stop_task`, and
     the wake reads it (`Cut::of`); `model::cut_short` keeps the words. `by` alone does not say a prompt is the owner's
     words (a question's answer or an exception sent back is by him and Remit words it), so `owner_message` marks its
     prompt `ownerWords` in its data (`model::with_owner_words`, `Message.owner_words`), and `outbox::read_of` heads it
     with his voice (`model::text_as_read`); `owner_said` and the channels' strips are gone. Migration 0130 records
     `owner_words` and `task_cut` at the next event id, and `store::events::scan_event` converts an event below them
     (`model::owner_words_under_head`, `Cut::read_from_words`), leaving one already in today's shape as it is. Guard as
     built: `store` test
     `an_owners_message_and_a_slept_task_read_in_the_new_shape_whichever_build_wrote_them`.

Each step updates `docs/design.md` where it changes what stands (*Constraints*: the sentence on the contract, `make gen`;
*Model*: Card's role and judge kind, Event's status).

## Revisit when

- A side of the contract is released apart from the server (a third-party client, a plugin built elsewhere against
  records): then records it reads need versioned shapes and a published schema, and `model` may need a narrower kernel
  for it.
- Quarantined records are seen outside a rollback (the `Health` count above zero on a build that wrote them): that is a
  writer's defect, and the count becomes an item for the Assistant.
- A field must reach the owner and not a mentor: then a mentor's channel gets view types of its own for that record.
- Compile time of every crate after an edit to `model` becomes the slowest part of an agent's loop: split `model` along
  the domains of ADR-0096, each record in the lowest crate that owns it.
