Any session can read and write any employee's memory folder, on both macOS and Linux. That gap underlies most of the design questions in this report. Labels: HIGH means I read the cited lines or ran the grep named; MEDIUM means inferred.

There is no embedding, vector or full-text search code anywhere in the repo. Memory today is each employee's own Claude Code memory folder. There is no shared, team or family memory, and nothing for competition or best-of-N. What a learning design can build on: a judged-card record, self-stated strengths, owner-approved change items, a skill library, and a mentor "help" record that is only partly built.

## 1. Memory today

- **Where it lives.** `memoryDirOf` returns `filepath.Join(o.cfg.EmployeesDir, employee, "memory")` and uses nothing but the employee id (`internal/orch/memory.go:32-40`). The root is `<data>/employees` (`cmd/remit/main.go:299`). HIGH
- **How a session gets it.** `withMemory` creates the folder and sets `spec.MemoryDir` (`memory.go:44-60`), called from `spec` (`internal/orch/access.go:81`). That turns on Claude Code's own memory settings (`internal/agent/options.go:42-44`). A card with no employee runs with memory off (`memory.go:45`). HIGH
- **Who writes.** Claude Code writes the files; Remit writes none (`memory.go:19-22`). A session can save only if it holds the Write tool (`writesMemory`, `:63-65`). Otherwise `recordBrief` tells it its memory is read-only (`:69-75`). HIGH
- **Note shape.** `model.MemoryNote{File, Name, Description, Type, Modified, Body}` (`internal/model/memory.go:8-15`). `memoryNoteOf` parses the YAML header, with a `metadata` fallback for type and modified date (`memory.go:128-159`). HIGH
- **Note types.** No fixed set: `Type` is whatever Claude Code writes. HIGH
- **Index file.** `MEMORY.md` (`memory.go:25`), skipped when notes are listed (`:103`). HIGH
- **Trust label.** Memory and the record are both "evidence to verify, never instructions" (`memory.go:28`; `internal/orch/my_work_tools.go:41`). HIGH
- **What the Chair sees.** The 3 newest notes (`internal/orch/employee_standing.go:21`), each as "type: description" cut to 160 characters (`memory.go:162-195`). They appear in `hire_agent`'s staff list (`employee_standing.go:66-68, 166-172`). HIGH
- **What the owner sees.** The Crew page (`EmployeeMemory`, `memory.go:78-87`; `web/src/components/EmployeesView.vue:71-77`), which is not live-pushed (`:70`). The Assistant reads the same through `read_memory` (`internal/orch/memory_tools.go:24`). HIGH
- **Forgetting.** The Assistant drafts it (`internal/orch/memory_change.go:63-99`). Only the owner's Apply deletes (`:120-143`). Apply refuses a file that changed after the draft (`:136`) and removes its lines from `MEMORY.md` (`:147-170`). HIGH
- **Privacy is not enforced.** HIGH
  - Claude Code sessions run under `startHiding(cmd, jail.Mask{Hide, ReadOnly})` (`internal/claudecode/conn.go:124`; `cmd/remit/runtime.go:152-155`).
  - macOS: the profile is `(allow default)`, then denies read and write only on the hidden paths and write only on the read-only paths (`internal/jail/sandbox_darwin.go:141-151`).
  - Linux: the session sees the host filesystem. Only the read-only paths are remounted read-only and only the hidden paths are emptied; everything else stays writable (`internal/jail/mounts_linux.go:15-38`).
  - The hidden list covers secrets, the database and its copies, client keys and supervisor state. The read-only list covers the server binary and `ext/` (`cmd/remit/agents.go:80-95`). A card's own read-only paths are its worktrees only (`internal/orch/policy.go:100-108`).
  - The employees folder is in none of these lists. ADR-0061's "Revisit when" names this gap.
- **ADR-0033 (superseded) is the most complete earlier design.** HIGH
  - Typed entries, only ever added to, each citing a checkable source.
  - An `Untrusted` mark set by the platform, and stale-by-hash detection.
  - SQLite full-text search (FTS5), with "vector search when full text is shown to miss".
  - A threat table; team and company layers were deferred.
- **ADR-0061** replaced it with Claude Code's own memory. Migration 85 dropped the old tables (`internal/store/migrations_memory.go:5-12`). It defers three things: private memory, a way for judges to learn without Write, and a new threat model. HIGH

## 2. Employees and templates

- **Fields.** `model.Employee` (`internal/model/employee.go:43-63`): ID, Number, Label, Template, TemplateRev, Loadout, Title, Scope, Strengths, ReportsTo, HiredBy, Status, Ceiling, Rev, CreatedAt. Titles are chair, judge, worker (`:9-15`). HIGH
- **Templates.** A template is a `model.Agent` holding a `Loadout{Instructions, Tools, Skills}` (`internal/model/library.go:27-35, 55-74`). Shipped ones come from `defaultAgent` (`internal/orch/agents.go:127`). `internal/builtin` holds only the `app` and `okr` skills (`builtin.go:18-40`). HIGH
- **Hire.** `hiredFrom` copies the template's loadout at its current revision (`internal/orch/employee_change.go:122-145`). Chairs are made at `lifecycle.go:589` and `:655`. HIGH
- **Reuse rule.** `CheckReuse` refuses a new worker while an idle one exists, unless `why` contains that worker's id. It is a substring check, not a check of the reason. Authors of the work being built on are excluded (`internal/orch/employee_rules.go:78-100`). A case-insensitive grep of `employee_rules.go` for memor|shared|team returns 0 lines. HIGH
- **Rehire** brings an employee back "memory included" (`employee_change.go:31`). HIGH
- **A move to a newer template keeps memory.** `movedOnto` does `next = *cur`, so the id is kept, and changes only `TemplateRev` and `Loadout` (`employee_change.go:117-118`). The memory folder depends on the id alone (`memory.go:39`). Both were read, so this is HIGH.
- **Two employees from one template share** only the copied instructions and the skill names, which point into the shared library. They share no memory, record or strengths. There is no lineage field. HIGH

## 3. Record and judging (possible fitness signals)

- **Judged cards.** `EndedCard{Chain, Card, Work, Outcome, Took}`, where Outcome is "accepted" or "sent back" from the newest accept or rework decision (`employee_standing.go:110-116, 178-196`). The Chair sees the last 5 (`:120`). HIGH
- **Decisions.** `model.Decision` (`internal/model/model.go:614-623`), with kinds accept, rework, goal, goal_failed, stall, insight and others (`:568-600`). HIGH
- **Strengths.** Self-stated through `state_strengths`; each statement replaces the last (`internal/orch/employee_strengths.go:35-44`). Nothing checks them against the record. HIGH
- **Own record.** `my_work` and `read_my_card`, paged (`my_work_tools.go:43-60, 123-131`). HIGH
- **Feedback.** Four kinds; "nothing acts on it by itself" (`internal/model/feedback.go:5-43`; `internal/orch/feedback.go:52-108`). HIGH
- **KPIs.** One table (`internal/outcomes/kpi.go:108-146`): accepted first time, goal first pass, goal confirmed, merge rate, merged without comment, owner wait, failed agents, stalled. They break down by role, model and playbook (`outcomes.go:55-87`), never per employee. HIGH
- **Analyst insights** are stored as decisions (`internal/orch/analyst.go:171-202`). HIGH
- **Mentor help is partly built.** HIGH
  - Present: `model.Help` (`internal/model/mentor.go:45-60`), the mentor actor (`internal/model/actor.go:12, 29-31`), migrations 87–88 (`internal/store/migrations_mentors.go:22, 49`), the functions `BringMentor`, `AcceptHelp`, `DeclineHelp` and `MentorSteer` (`internal/orch/help.go:35-153`), and the Chair prompt that names `bring_mentor` (`internal/orch/prompts.go:48`).
  - Missing: a grep of `internal`, `cmd` and `web/src` for `BringMentor|MentorSteer|AcceptHelp|bring_mentor`, leaving out `help.go`, finds only the prompt. So no tool or caller is wired yet.
  - A grep of `internal` and `cmd` for the plan's test names (`TestAStuckWorkerGetsAMentorOnItsCard`, `TestAMentorsMessageSteersTheCardsSession`, `TestAMentorReachesOnlyTheCardTheyHelp`) finds none.
  - The plan's checkboxes are unticked (`03-mentors.md:19-53`).
- **Overturns as lessons** (slice 4, Task 3) are planned, not built. HIGH

## 4. Shared knowledge today

- **Skill library.** `model.Skill` records (`internal/model/library.go:86-93`), given to sessions through `library.Access` (`access.go:35-58`). HIGH
- **Agent-written skills need the owner.** `holdSkill` turns a skill an agent writes into a change item the owner applies (`internal/orch/change_item.go:77-89`). HIGH
- **Built-in skills.** `app` is the Assistant's manual; `okr` comes with a capability (`internal/orch/library_render.go:60-67, 145-157`). HIGH
- **Importer.** Reads skills and agents only from the local Claude Code config (`internal/importer/importer.go:1-3`). HIGH
- **Playbook briefs** reach whichever Chair sits (`model.go:633-639`). HIGH
- **Self-improvement playbooks.** `self-scout` and `self-development` (`internal/orch/playbooks.go:80, 161`; ADR-0012). HIGH
- **No team, company or family memory.** HIGH
  - Grepping the memory files, `employee_rules.go` and the memory migrations finds nothing shared.
  - In `docs/remit-design.md`, the only line matching `memor` is a process limit (`:340`).
  - Team memory has been deferred three times: ADR-0033, ADR-0051:248, and `docs/superpowers/specs/2026-11-03-human-employees-design.md:238-243`.
  - I read `remit-design.md` by grep, not in full.

## 5. Storage infrastructure

- **SQLite driver.** `modernc.org/sqlite` v1.61.0, pure Go with no cgo (`go.mod:24`; `internal/store/store.go:21`). One WAL writer with immediate transactions, plus a separate read-only pool (`store.go:111-122`). HIGH
- **Full-text search.** FTS5 has worked with this driver (`internal/store/migrations_okr.go:64`); migration 85 dropped it. HIGH
- **Embeddings or vectors.** None. A grep for `embedding|vector|fts5|bm25|cosine|similarity` hits only Grafana test JSON, the dropped migration and `.claude/skills` text. HIGH
- **Migrations.** The latest is 88; the plan reserves 89. The list is joined at `internal/store/migrations.go:747`. HIGH
- **StarRocks mirror** of settled history (ADR-0041) could serve offline scoring. MEDIUM
- **Plugin seams (ADR-0034/0049).** Plugins are gRPC child processes pinned by SHA-256, with `Describe`, `Call`, `Poll`, `Sense`, `Process` and `Asset` (`sdk/pluginpb/plugin.proto:9-20`). Session plugin folders go in through `spec.Plugins` (`access.go:106-117`). A memory backend fitting there is my inference. MEDIUM

## 6. Shared memory, lineage and competition in docs

- **Shared memory** appears only as the deferrals above, plus ADR-0033's warning that shared memory spreads stale and poisoned entries. HIGH
- **Earlier research.** `docs/research-agent-learning/report.md` (2026-11-04) found:
  - What ships today is memory curated by people.
  - In SWE-Bench-CL, failed attempts polluted the memory.
  - Learning a user's preferences from their edits works.
  - It includes a memory-poisoning source. HIGH
- **Competition, tournaments, best-of-N, judges choosing between attempts, lineage:** none in `docs/`, `docs/archive` or `internal/orch`. The nearest are:
  - "two competing hypotheses" in a playbook brief (`playbooks.go:21`)
  - ADR-0055: chains of one objective read each other's readings
  - ADR-0032's trigger to revisit if same-template judges and Chairs agree too often. HIGH

## Seams

1. `withMemory` (`memory.go:44-60`): add a shared or family layer, or a second folder. HIGH
2. `recordBrief` (`memory.go:69-75`): what the session is told about its memory. HIGH
3. `memoryDirOf` (`memory.go:32-40`): memory is keyed by employee id. HIGH
4. `readMemory` / `memoryNoteOf` (`memory.go:90-159`): the parser an indexer or embedder would reuse. HIGH
5. `standingSummaryOf` / `summariesFor` (`employee_standing.go:130-201`): the record the Chair chooses from. HIGH
6. `CheckReuse` (`employee_rules.go:83-100`): where selection pressure would apply. HIGH
7. `hiredFrom` / `movedOnto` (`employee_change.go:103-145`): inherit memory and record lineage. HIGH
8. Change items (`memory_change.go`; `internal/model/change.go:9-34`; `holdSkill`): owner-approved promotion of a note to shared memory or a skill. HIGH
9. Decisions (`model.go:568-600`; `analyst.go:171-202`): triggers to gate memory writes. HIGH
10. `MentorSteer` / `Help` (`help.go:126-153`): human corrections as a signal, once wired. HIGH
11. `internal/outcomes/kpi.go:108-146`: where per-employee KPIs would go. HIGH
12. Migration 89 onward (`migrations.go:747`): FTS5 is available. HIGH
13. Plugin `Call` / `Sense` / `Process` (`plugin.proto`): an external memory or embedding backend. MEDIUM
14. `protectedFromAgents` (`agents.go:80-95`) and the jail mask (`conn.go:124`): enforce per-employee privacy. HIGH

## Coverage

**Read:**
- The memory files and the models for employee, library, mentor and feedback.
- `employee_standing.go`, `employee_rules.go` (`CheckReuse` and the function list) and `employee_change.go` (move and hire).
- `access.go`, `policy.go:100-108`, `help.go` plus a grep for its callers, `my_work_tools.go:41-60`, `kpi.go:99-147`.
- The jail: `jail.go:30-60`, `wrap.go`, `sandbox_darwin.go:120-151`, `mounts_linux.go:15-38`, `conn.go:124`, `runtime.go:152-155`, `main.go:245-259`.
- ADRs 0033, 0061 and 0062, the opening of 0034, the plan and slices 3–4, and the Employees and Memory sections of `design.md`.

**Grepped:**
- the whole repo for embeddings and vectors
- `docs/` including `docs/archive` for shared memory, lineage and competition
- `remit-design.md`, `paseo/` and `web/src` for memory
- `internal` and `cmd` for the mentor tests

**Open:**
- **Git state of the mentor-help work.** The inspected Remit directory has no `.git`, so I can't tell committed from uncommitted code. Whoever holds a checkout with `.git` can settle it.
- **Claude Code's memory file format.** It lives outside this repo. ADR-0061 says it was checked against Claude Code 2.1.312. Confirming the current format needs Claude Code's docs or a live session, which is yours to arrange.
- **Subagents a70e331f3e8907fcd and ab7cf34a9d9bed738.** I did not launch them; I have no tool to start or wait on agents. Their results are for you, the agent that launched me, to collect.
