# How Remit's AI employees learn from people: implementation design

Design with the owner's calls of 2026-11-05 (§1). The ADR is written with the first build. Evidence lives in `report.md` and
`notes/` (`notes/codebase-seams.md` for Remit today); each decision names the option it rejects and why.
Confidence: **HIGH** verified, **MEDIUM** inferred, **LOW** guess.

## 0. The thesis

Learning that works today is text a frozen model reads at the start of a task, extracted from human teaching and gated by a
verdict (`report.md` findings 1, 3, 5; HIGH). Retraining per user is not established (finding 4), nobody measures taste well
(finding 6), and memory written without a gate makes agents worse (finding 2; MINJA, SWE-Bench-CL).

The design splits along what lasts:

| Layer | Holds | Why there |
|---|---|---|
| **Core** | The record of every teaching, as signals; the rails (private memory, the gate, nobody writes their own yardstick); the memory seam; duels and ratings | What people taught and how work was judged keeps its value whatever the model; rails stay deterministic code (AGENTS.md *Agent-centric*) |
| **Plugin** | The memory backend: extraction, storage, retrieval, embeddings, consolidation | Replaceable, and next year's models may need less of it (AGENTS.md *We build for the future*); the core does only the core (ADR-0034) |
| **Agent** | The learning policy: what is promoted from employee to lineage to org, when to contest a card, what to forget | Judgement belongs to agents; the owner changes it by talking to the coach, not by a code change |

## 1. The owner's calls (2026-11-05)

| Question | Call |
|---|---|
| The design | Core records, plugin remembers, a coach decides; duels and ratings stay in the core |
| Memory backend | The owner's `~/projects/memory`, run as a Remit plugin |
| What the extractor may learn from | Accepted cards and every teaching by a person; never a sent-back card's own conclusions |
| Roles no verdict ends (Chair, Assistant, judges) | Learned from as they go; an owner's overturn retracts what that decision taught |
| Who approves a lesson inferred from the owner | The Assistant, for him; he sees it and may overturn |
| First teaching signal | The owner's overturns, with his edits after merge in the same build |
| Family memory | The lineage is a scope in memory; siblings read it |
| Org-wide memory | The global scope: the owner's principles; procedures become skills |
| Duels | AI judge, calibrated on the owner's overturns; paused when agreement drops |
| Independent development | At least two lineages per kind of work |
| The number on screen first | Repeat-correction rate |
| Notes saved before the gate | Wiped; every employee starts empty |
| Build | One change for the core and the plugin |

## 2. Where Remit stands (2026-11-04)

- Memory is Claude Code's auto memory, one directory per employee keyed by employee id (`withMemory`,
  `internal/orch/memory.go:44-60`; ADR-0061). Any session with Write saves at any moment, whatever its card's verdict
  (`writesMemory`, `:63-65`). HIGH
- The employees directory is in neither the hidden nor the read-only list (`protectedFromAgents`,
  `cmd/remit/agents.go:80-95`): any session can read and write any employee's memory. HIGH
- The core runs no Claude session; the AI runtime runs them and, on one machine, shares the core's disk
  (`docs/superpowers/specs/2026-11-04-employee-runtime-design.md:32,51,228-236`). Transcripts are in the runtime's
  `<dataDir>/claude/projects` (`:232`). HIGH
- A session already loads Claude Code plugin directories the core names (`specOf`, `internal/orch/access.go:106-109`;
  `agent.Spec.Plugins`, `internal/agent/agent.go:101-102`). HIGH
- `Template` and `TemplateRev` are recorded per employee (`internal/model/employee.go:50-51`): the lineage key exists. HIGH
- The record: `EndedCard{Work, Outcome}` (`employee_standing.go:110-120`); accept and rework are decisions
  (`internal/model/model.go:571-572`); KPIs are by role, model and playbook, never by employee (`internal/outcomes`). HIGH
- Overturns are planned (slice 4 Task 3);
  mentor steers are being built (`internal/orch/help.go`, `mentor.go:281`). Latest migration 88; 89 is reserved. HIGH

## 3. Core: the teaching record

**D1. Every teaching is a signal.** The core records, as facts in the signal pipeline (`internal/orch/fact.go`), each event in
which a person or a verdict teaches an employee:

| Fact | Carries |
|---|---|
| `teach.overturn` | The decision, its maker, the owner's reason |
| `teach.verdict` | Accept or rework, the card, the judge, the reason |
| `teach.steer` | The mentor, the card, the message |
| `teach.review` | A review thread on an MR, its author, the lines |
| `teach.edit` | The diff between the agent's last push and what was merged (`MergedAt`, `internal/orch/merge_request_sense.go:33,108`) |
| `teach.duel` | Both cards, the judge's pick, the owner's overturn if any |

Each fact names the employee taught and the card. The facts are the durable asset: any later learner, model or backend
re-reads them. Rejected: teaching that only lives inside a session's memory — it dies with the backend and cannot be
re-learned by a better model. HIGH.

## 4. Core: rails

**D2. Memory is private.** The employees directory and the memory backend's store are hidden from every session
(`protectedFromAgents`); a session reaches memory only through the plugin, which answers for its own employee and the scopes
above it. HIGH (AgentPoison: >80% attack success at <0.1% poisoned memory).

**D3. The gate.** The core tells the backend which transcripts it may learn from, and as what: an accepted card's session, its
whole transcript; a sent-back card's session, only the teaching facts about it, never its own conclusions; the Chair's, the
Assistant's and the judges' sessions as they go, with an overturn retracting what the overturned decision taught (the backend's
`supersede` on memories whose `transcript_ref` falls in that decision's turn). Rejected: memory written at any moment by the
agent (today) — SWE-Bench-CL. HIGH.

**D4. Nobody writes their own yardstick.** No agent writes a judge's brief, a rating, a verdict, a teaching fact or the coach's
metrics; the owner's path and deterministic code do (DGM edited its own hallucination detector to win). HIGH.

## 5. Core: the memory seam

**D5. Memory reaches a session through a plugin.** The core turns Claude Code's auto memory off and adds the memory plugin to
every employee's session (`spec.Plugins`), with the employee's identity in its environment. The plugin's hooks inject what the
employee should know at session start and per prompt; its MCP tools search and save. Rejected: Claude Code's auto memory with a
per-card copy merged by hash — files plus a provenance table are two owners of one concept, it breaks once the runtime leaves the
core's disk, and it puts the gate after the agent has written. Rejected: memory as Remit tables — it rebuilds a backend the
owner already runs and trusts. MEDIUM until the PoC in §9 passes.

## 6. Plugin: the owner's memory system as the backend

`~/projects/memory` already does what the research says works: extraction from transcripts by a background worker
(`DESIGN.md:126-136`), typed memories with trust separate from life cycle (`:95-96`), scopes `project | stack | global` with
promotion to global in review (`:97`), evidence and `transcript_ref` on every memory (`:100`), dedupe and supersede
(`:134-135`), hybrid recall with embeddings (hit@3 60% → 80%, `:233-234`), nightly dream (`:152-163`) and an eval gate
(`:255`).

**D6. The mapping.**

| Remit | memory |
|---|---|
| An employee | A project |
| A lineage (template) | A stack |
| The org: the owner's principles | Global scope |
| The runtime's transcripts | A scan root (`scan.roots`, `:60`) |
| One Remit instance | One `MEMORY_HOME` (`:36`) |

**D7. What the backend must gain** (changes in `~/projects/memory`, each a gap found in `DESIGN.md`):

1. **Attribution by employee, not by folder.** Today a session's project comes from its working directory (`:132`); an
   employee's cards run in repository worktrees. The hook and the scanner take the project from the session's environment or
   the core's gate list first.
2. **Private but embedded.** `isolated` both hides a project and stops embeddings (`:99`); employees need the first without the
   second: a visibility `private` (never in another project's recall, embedded as usual).
3. **Learning from an AI's own accepted work.** Extraction keeps what the human said (`min_human_messages`, `:52`); an accepted
   card's lessons come from the agent's own turns. New kinds `lesson`, `pitfall`, `preference` beside `principle | decision |
   intent | fact`, and an extraction prompt per source.
4. **The gate list.** The scanner extracts only sessions the core lists, with the role each may teach (D3).
5. **No spend cap inside Remit.** `daily_usd_cap` (`:46`) is off for a Remit home: Remit records no cost and caps
   none (AGENTS.md *Where we are*).
6. **Many projects at once.** One daemon serving every employee of an instance, on the instance's Claude accounts rather than
   the owner's login (`claude -p`, `:132`).

## 7. Agent: the coach

**D8. A coach employee holds the learning policy.** A playbook `coach` subscribes to the `teach.*` facts and runs, in its brief:
- **Promotion.** A lesson moves from an employee to its lineage when accepted cards of at least two siblings stand behind it;
  from a lineage to global through the Assistant, for the owner. Procedures it sees repeated become skills (`holdSkill`,
  `internal/orch/change_item.go:77-89`).
- **Forgetting.** It drafts forgets; the Assistant applies for the owner (ADR-0062).
- **Contesting.** It asks the Chair to contest a card where lineages disagree (D10).
- **Measuring.** It computes the repeat-correction rate per employee: a new teaching that matches an existing memory is a repeat.
The thresholds are words in its brief; the owner changes them by talking to it. Rejected: thresholds and schedules in core
code — judgement belongs to agents (AGENTS.md). MEDIUM.

## 8. Core: duels and lineages

**D9. Lineages stay independent.** At least two lineages per kind of work; employee and lineage memory never cross lineages;
only the global scope does (islands with migration, FunSearch/AlphaEvolve; Artificial Hivemind for why diversity must be built
in). A losing lineage's stack is revised from the winner's lessons, never from its raw memory. MEDIUM.

**D10. Duels.** A Chair tool contests a card: a sibling card with the same brief for an employee of another lineage. An AI judge
picks; the owner may overturn any verdict; each overturn is a `teach.duel` fact that reaches the judge's memory, and the judge's
agreement with the owner shows on its screen; when it drops, duels of that kind of work pause. A Bradley–Terry rating per
lineage per kind of work is kept and shown in `hire_agent`'s staff list (`employee_standing.go:130-201`) and to `CheckReuse`
(`employee_rules.go:83`). Both employees learn from the verdict. Rejected: evolutionary loops over templates (DGM, AlphaEvolve)
— they need a machine-checkable fitness and hack it. MEDIUM; with few cards per lineage the rating is noisy, so it is shown with
its count.

## 9. Build

One change, core and plugin, with its ADR. Migrations from 90.

**PoC first** (each a pass/fail before code):
1. A Claude Code session with auto memory off and the memory plugin loaded, its project set by environment, recalls an
   employee's memory and nothing of another employee's.
2. One memory daemon with `MEMORY_HOME` under the Remit data directory and the runtime's transcripts as its only scan
   root extracts a lesson from an accepted card's transcript, attributed to the right employee.

**Core.**
- The `teach.*` facts (D1), with the overturn from slice 4 Task 3 and the edit diff from the MR follow.
- The rail: hide `<data>/employees` and the memory home (D2).
- The gate list the backend reads (D3); retraction on overturn.
- The seam: auto memory off, the plugin in `spec.Plugins`, the employee's identity in its environment (D5).
- Duels: the contest tool, the sibling card, the rating table (migration 90), the pause rule (D10).
- Wipe: archive `<data>/employees/*/memory` to one tarball beside the database backup, then remove it. Impact → Verify: on dev,
  then the pod at go-live; the tarball's file count equals the count before, and each directory is empty after.
- Tests first: `TestASentBackCardsConclusionsAreNotLearned`, `TestAnOverturnRetractsWhatItsDecisionTaught`,
  `TestASessionRecallsOnlyItsOwnEmployee`, `TestTheOwnersEditsAfterMergeBecomeATeaching`, `TestADuelVerdictUpdatesTheRating`.

**Plugin.** The six changes of D7 in `~/projects/memory`, with its eval extended by a Remit set: replayed employee cards,
hit@3, and the repeat-correction rate.

**Agent.** The `coach` playbook and its brief (D8), hired once.

## 10. What the evidence does not settle

- No paper evaluates a private → team → org promotion pipeline; the coach's thresholds are first-principles. LOW.
- No study compares inheriting memory with inheriting only the template. LOW.
- No head-to-head of human-judged and AI-judged selection in agent evolution; D10's calibration rests on indirect evidence. MEDIUM.
- CIPHER's evidence is writing tasks with simulated users; `teach.edit` needs its own replay before it is trusted. MEDIUM.
- Whether `~/projects/memory` serves an instance's employees at once is the PoC's to show. MEDIUM.
