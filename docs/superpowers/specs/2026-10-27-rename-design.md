# Rename: the product, the case, the running agent

Status: draft. Nothing is renamed yet. One name waits on the owner (`Card`, see *Decisions*), and the work waits on a clean
working tree (see *Constraints*).

## Intent

Three names no longer say what the thing is:

- **`Chain`** names the unit of work, and a chain links to nothing: `model.Chain` (`internal/model/model.go:67`) has no
  parent, next or depends. It is a goal, one Chair, a tree of agents, a budget and merge requests, which is a case. The
  name survives from the product name, and it is one letter from `Chair` (`ChairID` sits inside `Chain`).
- **`Card`** (`model.go:339`) is "one agent: one ACP session", named after how the UI draws it. The tools call the same
  thing an agent (`hire_agent` at `internal/orch/tools.go:407`, `list_agents`).
- **`Remit`** is the product name and the source of the first.

`AGENTS.md` asks for one concept under one name everywhere (Go, the contract `web/src/rpc/define.ts`, the UI, the tools
agents read), so each rename is done across all of them in one change, with the old name deleted.

## Decisions

| Now | Becomes | Status |
|---|---|---|
| `Chain` | `Case` | Decided by the owner (2026-10-26). The legal vocabulary is already in the code: Chair, judge, `record_verdict`, Decision, Approval, and `docs/design.md:69` says "One case". |
| `Remit` | `Remit` | Decided by the owner (2026-10-27): the product lives at `https://remit.example.com`. A remit is the scope the owner hands over, and to remit is to send a case back, which is the `request_changes` rework loop. |
| `Card` | open | See *Open decision: Card and Agent*. |

Remit, checked on 2026-10-26 (npm, GitHub account and repos, `.com/.dev/.app`; trademarks and other languages not
checked): npm is taken; 4,140 GitHub repos carry the name, the top one `tylerhunt/remit` with 159★. Its everyday
meaning is "send money", which payment companies' names echo. The product is served from its own subdomain and is
single-owner, so a taken npm name and a crowded search weigh little.

### Case

`case` is a keyword in Go and TypeScript, so a variable is `c` or `cs`, never `case`. QC's "test case" overlaps the word
slightly.

### Open decision: Card and Agent

Commit `ca60e61` made `Agent` the owner's definition (`internal/model/library.go:22`: instructions, tools, skills, MCP
servers, `Signs`), and `Card.Agent` is the id of the definition it runs (`model.go:344`). Renaming `Card` to `Agent`,
as first planned, would give two types one name and make the field `Agent.Agent`. The design that introduced the
definition (`2026-10-26-agent-library-design.md:110`) also made `Agent` the name in the routes (`/agents/<agent>`), the
library tools and `ReservedAgentIDs` (`library.go:16`).

| Option | Result | Cost |
|---|---|---|
| A. Keep `Card` | `Agent` is the definition, `Card` stays the running one | None, but it leaves the instance named after its drawing, the defect this rename set out to remove. |
| B. A new name for the running one | `Agent` stays the definition. Candidates: `Hire` (the Chair calls `hire_agent`; `EdgeHire` and `DecisionHire` at `model.go:427,481` name the act, so the noun shares a word with them) or `Member`. Not usable: `Session` (`SessionID`, `SessionInit`), `Worker` (`Agent.Model` says "the worker model"), `Run` (`Usage.RunID` is the agent process, `model.go:377`), `Brief` (the task a card is given, `library.go:27`), `Task` (`EventTask` is an agent's background work, `model.go:452`; the Scout reads PRD tasks from the vault, `docs/design.md:237`), `Thread` (a review thread). | `Card` renames across the same surface as `Chain` (see *Surface*). Nothing shipped is reversed. |
| C. The running one becomes `Agent` | The definition gets another name (`Profile`, `Kind`) | Reverses the library design: its routes, tools, file names, spec and `ReservedAgentIDs`. The largest change. |

Recommendation: **B**, MEDIUM (a judgement from the evidence above, not a measurement). A contradicts the reason for the
rename, and C undoes a design that shipped yesterday (`ca60e61`). The name for B is still to be chosen.

## Surface

Measured on the working tree of 2026-10-27, uncommitted changes included, excluding `node_modules` and `web/dist`:

| Name | Files | Lines | Method |
|---|---|---|---|
| `chain` | 375 | 6,471 | `rg -i chain`: substring, so an upper bound (`toolchain`, "supply chain" match) |
| `card` | 270 | 3,036 | `rg -w -i "card\|cards\|cardId\|card_id"`: whole word, so a lower bound (`CardStatus` does not match) |
| `remit` | 265 | 975 | `rg -i remit`: substring |
| module path | 197 | | files carrying `remit` |

Where a name is data, not code, and needs a deliberate step:

- **SQLite.** Tables `chains` and `cards` (`internal/store/migrate.go:15-16`). Migration 18 (`migrate.go:243-262`)
  already renamed `chain_id` to `home` where the Assistant owns a card too; what only a chain has (edges, decisions,
  review threads, approvals, cleanups) keeps `chain_id` (`migrate.go:245`). Indexes carry the name (`cards_chain`,
  `edges_chain`, `decisions_chain`, `approvals_chain`, `usage_card`, `events_card`, ...). The latest migration is 26
  (`migrate.go:322`), so each rename is the next entry of `migrations` (`migrate.go:11`).
- **Stored actor kind.** `ActorRemit = "remit"` (`model.go:17`) is a stored value (`by_kind = 'remit'`
  and JSON `"kind": "remit"`, `migrate.go:141-152`). The generated `ActorKindRemit`
  (`internal/wire/types.generated.go:1918`) follows the contract.
- **Persisted agent sessions.** The MCP server name `remit` (`internal/orch/mcpapproval.go:8`,
  `tools.go:332`) and the `[Remit: ...]` markers agents see (`schedule.go:471`, `checkback.go:181`, `plan.go:81`,
  `assistant.go:246`, and the migration that matches them at `migrate.go:213-216`) live inside sessions that resume.
  `mcp__remit__*` tool names are in old transcripts.
- **Tools agents read.** `finish_chain` (12 occurrences in `internal/orch/*.go`), `propose_chain` (11), `read_chain` (13),
  `list_chains` (13), and the `card_id`, `to_card`, `from_card` parameters. Briefs in `prompts.go` and `playbooks.go`
  name them.
- **Text the owner wrote.** `playbooks` stores `doc TEXT` (`migrate.go:26`) and may say "chain" in prose ("supply chain"
  too). Code does not rewrite it. The Playbook author agent rewrites it, because judgement belongs to agents.
- **Contract and UI.** `web/src/rpc/define.ts` (159 of 1,358 lines mention chain or card; `make gen` regenerates both
  sides), the router (`web/src/router.ts:1,17,18`: `/chains`, `/chains/:id/:tab`, `?card=`).
- **Ids.** `ch_` (`internal/orch/lifecycle.go:78`) and `c_` (`tools.go:88`). Ids are stored values: they name the working
  directory `<data>/work/<id>` and Claude Code's transcript path, so renaming them loses resumable sessions.
- **Branches.** `feature/mc-<id>-<slug>` (`internal/orch/repo.go:304`) is built from the old initials and the `ch_`
  id. `docs/design.md:31` says the repositories' push rules accept `feature/mc-*`; whether they match `mc-` or only
  `feature/` is unread. Pushed branches and open MRs keep their names.
- **The address.** The product is served at `https://remit.example.com`, the only host the repo names (`allowedHosts`,
  `web/vite.config.ts:11`). Sign-in goes through Firebase, which refuses a sign-in from a domain missing in the project's
  authorised domains; `remit.example.com` is listed there (read from the project's public config, `getProjectConfig`,
  2026-10-27). The flag's default is `127.0.0.1:4320` (`cmd/remit/main.go:40`); the pm2 apps run with `0.0.0.0`
  (`~/.remit/ecosystem.config.cjs`, outside the repository), and the agents reach the MCP endpoint on loopback
  whatever the listen address is (`cmd/remit/mcpurl.go`). The name resolves to `192.168.10.5`; which port that
  address forwards to is outside this repository and not yet known.
- **Outward-facing.** The GitHub repository and Go module path, the `REMIT_*` variables (`README.md:24-27`,
  `cmd/remit/google.go`), the data directory `~/.remit` (`cmd/remit/main.go:41`), the Codex permission
  profile name (`cmd/remit/codexhome.go:36`), the deny globs (`internal/orch/policy.go:90,99`) and the MR label
  (`internal/orch/submit.go:484`).

## Constraints

- **Stored ids keep their value and prefix.** An id is a value, not a name.
- **No redirect for the old URLs and no alias for the old tool names.** Nothing of this was released.
- **Structure, not text.** Go renames with `gopls rename` or `sg`, TypeScript with `tsc`; a regex over the tree would also
  change "supply chain" and `discard`.
- **One concept per change.** One commit per rename with `make test` green in each, and a test for the migration written
  first.
- **Outward-facing steps ask the owner first, each one.** The repository, the module path, the variables and the data
  directory move only on a yes.
- **The working tree is clean before the first rename.** The tree holds uncommitted work in `internal/model`,
  `internal/orch` and `internal/store` (`git status`, 2026-10-27), the core that every rename touches; a rename beside it
  creates a large merge to verify. Peers' dirty files are theirs, so the owner commits or releases them.

## Order

1. The owner settles the open decision (`Card`).
2. **Card** (when option B or C): migration, then Go, contract and UI.
3. **Chain to Case**: migration, tools, a request to the Playbook author to rewrite stored playbooks, contract and UI.
4. **Remit to Remit**, with each outward-facing step asked first.
5. **Real data.** Copy the SQLite file before the new binary first opens it. Check the new table and column names exist
   and the row counts did not drop. Resume an old session and confirm it continues.

Each step is sequential, because all of them touch the same files. Parallel agents help with review and with looking at
the UI.

## Verification

- A migration test per rename: the old schema in, the new names present, row counts equal.
- The old session resume: open a transcript written before the rename and send a prompt.
- Every screen the rename touches is rendered in a browser (`cd web && npm run verify`, `node web/verify/shot.mjs`), as
  `AGENTS.md` requires, including a busy case and a finished one.
- `rg` for the old term on every surface returns only the stored values kept on purpose (ids) and history (migrations,
  commit messages).

## Not verified

- Trademarks, and what each name means in Vietnamese or other languages.
- Whether the repositories' push rules accept a branch prefix other than `feature/mc-*`.
- Whether SQLite `RENAME TABLE` and `RENAME COLUMN` behave under the repository's `modernc.org/sqlite` driver for tables
  with indexes. Migration 18 already uses `RENAME COLUMN` (`migrate.go:247`), so `RENAME TABLE` is the one to test first.
- Web searches that found no existing AI tool named Remit are weak evidence.
- Which port `192.168.10.5`, where `remit.example.com` resolves, forwards to: the pod cannot reach that address.
- Sign-in by popup on `remit.example.com`: the domain is authorised, a sign-in from that domain has not been run.

## Origin

The owner asked for a better product name on 2026-10-26, then asked whether `Chain` is still right and chose `Case`. On
2026-10-27 the owner gave the product's address, `https://remit.example.com`. The discussion stayed in the chat and
this file records it, with the surface re-measured against the tree of 2026-10-27.
