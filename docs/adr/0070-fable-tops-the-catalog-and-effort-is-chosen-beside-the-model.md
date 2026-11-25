# 0070. Fable tops the catalog, and effort is chosen beside the model

Status: accepted, 2026-11-10; (d)'s judges on `sonnet` amended by ADR-0102: every judge runs on the catalog's strongest model. Source: the owner's request of 2026-11-10 to apply an article on choosing models (raising effort one
level can move quality as much as a model upgrade; Fable at `high` matched Opus at `max`; mixing models for verifiable side
work), `AGENTS.md` (*Where we are*: quality is the only target), ADR-0006 (Claude Code is the only agent), ADR-0032 (judges of
one provider err together). Extends ADR-0006's catalog. Read against commit `090b785` and the working tree of 2026-11-10.

## Context

The customer is the owner: every result he reads was produced by an agent on the model, and at the depth of thinking, it was
started with. Two levers set that, and Remit holds one of them.

What the code does today:

- **The catalog has three models.** `provider.Default` lists `opus` (`Frontier`), `sonnet` (`Workhorse`), `haiku` (`Fast`)
  (`internal/provider/provider.go:34-40`); `Tier` is `Fast`, `Workhorse`, `Frontier` (`:14-18`). The catalog's order is the
  preference between two equal choices (`:30`).
- **The Chair is told to save.** `modelsHelp` ends with "Pick the cheapest tier that does the job" (`internal/orch/failover.go:51`),
  read by the Chair in `hire_agent`'s description (`internal/orch/tools.go:465`). `AGENTS.md` says the opposite: cost never
  outweighs quality and Remit caps none.
- **Effort exists nowhere in the product.** Claude Code takes `--effort <low|medium|high|xhigh|max>` (`claude --help`) beside
  `--model`, which `internal/claudecode/options.go:86` passes; no record, input or spec carries effort. Every session runs at
  Claude Code's default for its model.
- **The model travels one path.** A record holds it as a plain `string` (`model.Card.Model`, `internal/model/model.go:434`;
  `model.Seed.Model`, `:656`; `model.Agent.Model`, `internal/model/library.go:62`; `model.Feedback.Model`,
  `internal/model/feedback.go:37`); one orch boundary checks it against the catalog (`resolveModel`, `internal/orch/failover.go:35`);
  the hire resolves `cmp.Or(in.Model, def.Model, chair.Model)` (`internal/orch/chair_hire.go:197`); the session is told it through
  `library.Access` (`internal/library/access.go:115`), `agent.Spec.Model` (`internal/agent/agent.go:87`) and the runtime's
  `SessionSpec` (`web/src/rpc/runtime/define.ts:54`, `internal/runtimelink/convert.go:24`, `internal/airuntime/convert.go:147`).
- **Defaults.** The Chair and the Assistant run on `opus` (`cmd/remit/main.go:117-118`); the intake, goal, check and duel judges
  on `sonnet` (`cmd/remit/main.go:41`), as does the judge template (`internal/orch/agents.go:30`), kept off the Chair's model by
  ADR-0032. A health probe runs on the lowest tier (`Catalog.Cheapest`, `internal/provider/provider.go:62`;
  `probeModel`, `internal/orch/health.go:327`).
- **The Assistant's card is made once** (`makeAssistant`, `internal/orch/assistant.go:60`): `-assistant-model` reaches it only
  at the first start, so changing the default leaves the owner's Assistant on the model it was made with.
- **Checked on this machine, 2026-11-10:** `claude -p --model fable --effort high` answered on `claude-fable-5-1` with the
  machine's login; `claude -p --model haiku --effort high` answered too. Claude Code takes every level on every catalog model.

## Options

**(a) How Fable ranks.**

1. **A fourth tier above `Frontier` (chosen).** `Apex`: `fable` (apex), `opus` (frontier), `sonnet` (workhorse), `haiku`
   (fast). The Chair reads that `fable` is the stronger of the two top models from the words it already reads.
2. **Fable and Opus both `Frontier`.** The catalog's order would be the only sign of which is stronger, and `Describe` does not
   show order. Rejected: the Chair loses the one fact it needs.
3. **Drop tiers, rank by position.** Tiers would leave `Describe` and the probe would take "the last listed". Rejected: an
   implicit rule in place of an explicit one, and the Chair reads no capability at all.

**(b) Where effort lives.**

1. **A field beside `Model` on every record and input that carries `Model` (chosen).** A plain string, as the model is,
   checked once by the catalog at the one orch boundary that checks the model; empty keeps Claude Code's default.
2. **Folded into the model string** (`fable@high`). One field, but two concepts in it: `Catalog.Lookup`, the importer's
   `known.Models`, `--model`, outcomes grouping and every screen would parse it, and a model id would no longer be a model id.
   Rejected (one concept, one owner).
3. **A value type `{Model, Effort}` embedded in each record.** One owner of the pair, and Go flattens an embedded struct's
   JSON. But `model` depends on nothing (`internal/fitness/imports_test.go:35`) so the type cannot be the provider's, Card and Seed
   tag `model` differently (always present, omitempty), and every `Model:` literal across orch changes shape for a pairing
   the catalog check already enforces. Rejected for now: two fields, one check.
4. **A table of effort per role or per tier in the server's config.** No per-agent choice, and the owner could steer it only
   with flags. Rejected (agent-centric: the Chair chooses per hire, the owner steers by talking to it).
5. **Effort fixed per catalog model** (`fable` always `high`). Throws away the article's lesson that effort is its own axis.
   Rejected.

**(c) What the Chair is told.** Keep a cost rule and soften it; say nothing; or say to choose for the result (chosen).

## Decision

**(a) The catalog.** `Tier` gains `Apex` above `Frontier` (`Fast`, `Workhorse`, `Frontier`, `Apex`; `String` "apex").
`provider.Default` is, in order: `{fable, Apex}`, `{opus, Frontier}`, `{sonnet, Workhorse}`, `{haiku, Fast}`. `Catalog.Cheapest`
becomes `Catalog.Fastest` (same rule: the lowest tier, first listed of two), because the probe wants the quickest answer
that proves an account answers, and the catalog carries no price; `probeModel` reads it and still runs on `haiku`.

**(b) Effort.** `internal/provider` defines `type Effort string` with `Low`, `Medium`, `High`, `XHigh` ("xhigh"), `Max`, and
`ParseEffort(s string) (Effort, error)`: empty is valid and means Claude Code's default; anything else not in the list is refused
with the list. The catalog takes every level on every model (checked above); a per-model list is not built. Records carry it as
a plain string beside `Model`, the same way the model is carried:

- `model.Card.Effort`, `model.Seed.Effort`, `model.Agent.Effort`, `model.Feedback.Effort`, all `json:"effort,omitempty"`;
  `outcomes.Model` gains `Effort` and the report groups by the pair (`fable · high` and `fable · low` are separate rows).
- `library.Input.Effort` and `library.Session.Effort` (copied by `Access` as `Model` is), `agent.Spec.Effort`,
  `claudecode.Options.Effort` → `add("--effort", o.Effort)` beside `--model`; the runtime's `SessionSpec.effort`.
- Inputs: `HireInput.Effort`, `AgentInput.Effort` (define/update an agent), `SeedInput.Effort` (a playbook seed).
- One boundary: `resolveModel(want string)` becomes `resolveModel(want, effort string) (string, string, error)` in
  `internal/orch/failover.go`, which checks the model against the catalog and the effort with `provider.ParseEffort`. Every
  place that resolves a model calls it; nothing else validates either.
- The hire resolves each field by the same rule: model `cmp.Or(in.Model, def.Model, chair.Model)`, effort
  `cmp.Or(in.Effort, def.Effort, chair.Effort)`. Empty means "as your own", for both.

**(c) What the Chair reads.** `modelsHelp(own, ownEffort)` says, with no word of cost:

> Choose each agent's model and effort with the model and effort inputs; empty is your own (fable, effort high). Models:
> fable (apex), opus (frontier), sonnet (workhorse), haiku (fast). Effort is how long the model thinks before it acts: low,
> medium, high, xhigh or max. Choose the pair whose work you would accept with the least doubt: the most capable model for
> judgement, design and work no check can prove; more effort for work that is hard to get right; less where more thinking adds
> nothing, because a check proves the result or the task is plain. A more capable model at a lower effort often does better
> than a weaker one at its highest. When the provider fails or is out of reach, an agent waits for it and goes on by itself.

`hire_agent`'s answer names the pair ("Hired … on fable · high"), and `define_agent`'s and a seed's `effort` inputs say "its
default effort (low, medium, high, xhigh, max); empty uses the hiring Chair's".

**(d) Defaults.** `-chair-model` and `-assistant-model` default to `fable`; new flags `-chair-effort` and `-assistant-effort`
default to `high`, checked at start with the model flags (`requireModels` also parses the efforts) and carried as
`orch.Config.ChairEffort` and `AssistantEffort`. A new chain's Chair card is made on `ChairModel`/`ChairEffort`
(`internal/orch/lifecycle.go:247`). The judges stay on `sonnet` with empty effort (Claude Code's default), and
`judgeTemplateModel` stays `sonnet` (ADR-0032). The flags own the Assistant's choice: `EnsureAssistant` sets its card's `Model` and
`Effort` to `AssistantModel`/`AssistantEffort` at every start when they differ, in one store update, so the owner's existing
Assistant moves to fable at high on the first start of this build and follows any later flag change. A running chain keeps the
Chair it was started with.

**(e) Stored data: no migration.** `Card`, `Agent` (table `agents`), `Playbook` with its `Seeds` (table `playbooks`) and
`Feedback` (table `feedback`) are JSON documents (`internal/store/store.go:237`, `internal/store/library.go:23`,
`internal/store/store.go:536`, `internal/store/feedback.go:23`). Each gains one `omitempty` field; a stored document without it
reads as empty effort, which is exactly today's behaviour (Claude Code's default). No document changes shape on disk until it is
next written. The Assistant's card is brought to the flags by `EnsureAssistant` (above), not by a migration, because a numbered
migration would hard-code today's default into the owner's data and could not know his `-assistant-model`.

## Threats

No new entrance: effort is a closed set of five words, parsed at the one boundary before it reaches Claude Code's command line
as a separate argument (`add`, `internal/claudecode/options.go:81-85`), so no agent can inject a flag through it.

## Consequences

- The Chair hires for the result. Nothing in its prompt points it at the weaker model; it may still choose a lighter model or
  lower effort, and has to justify it by the work, not by a price.
- The owner sees and steers the choice where he looks for an agent: the agent row, its session panel, its definition, a
  playbook's seeds, the outcomes table and feedback all show `model · effort`. He changes it by telling the Assistant or a
  playbook author to change an agent's or a seed's default; no form is built.
- Outcomes per model and effort give the Chair and the analyst the evidence the article only claims.
- Next year's models enter as one catalog line each; a new effort level is one constant. Neither adds a code path.
- **Fable must be reachable from every account.** The breaker is kept per account, not per model: an account whose login may
  not use `fable` fails every Chair turn with `access`, is taken out, answers its probe on `haiku`, and comes back, in a loop.
  This machine's login runs `fable` (checked above); the pod's must be checked (`claude -p --model fable "OK"` there) before this
  build goes live.
- **`high` may sit below Claude Code's own default.** Claude Code's default effort is reported as `xhigh` for current models
  (Anthropic's model notes, not checked against Claude Code's source); the owner asked for `high` on the article's evidence. Outcomes by
  model and effort will show whether `xhigh` does better.

## Implementation, in order, by owning agent

1. **chain-runtime:** `internal/provider/provider.go` (`Apex`, `Effort`, `ParseEffort`, `Default`, `Fastest`) and its test;
   `internal/claudecode/options.go` (`Effort`, `--effort`); `internal/agent/agent.go` (`Spec.Effort`);
   `internal/runtimelink/convert.go`, `internal/airuntime/convert.go` (after step 3's gen); `internal/orch/failover.go`
   (`resolveModel`, `modelsHelp`); `internal/orch/health.go` (`Fastest`); `internal/orch/lifecycle.go` (Chair card);
   `internal/orch/access.go` (`:36` Input, `:103` Spec); `internal/orch/intake.go:350` (Spec from the intake card).
2. **store-and-migrations:** `internal/model/model.go` (`Card.Effort`, `Seed.Effort`), `internal/model/library.go`
   (`Agent.Effort`), `internal/model/feedback.go` (`Feedback.Effort`); no migration (Decision (e)); a store round trip of a
   card without `effort` reads empty.
3. **rpc-contract:** `web/src/rpc/define.ts` (`effort?: string` on `Card`, `Seed`, `Agent`, `OutcomeModel`, `Feedback`);
   `web/src/rpc/runtime/define.ts` (`SessionSpec.effort: string`); `gmake gen` (regenerates `internal/wire`, `web/src/rpc/*.generated.ts`,
   `internal/runtimewire`, `internal/runtimeclient`), then `gmake check-gen`.
4. **agent-tools:** `internal/library/access.go` (`Input.Effort`, `Session.Effort`); `internal/orch/chair_hire.go`
   (`HireInput.Effort`, the hire's resolution); `internal/orch/tools.go` (`modelsHelp` call, the hire's answer);
   `internal/orch/agent_tools.go`, `internal/orch/playbook_tools.go` (inputs and their read-back); `internal/orch/agents.go`
   (validate `Effort` beside `Model`, `:205-208`); `internal/orch/orch.go` (`ChairEffort`, `AssistantEffort`);
   `internal/orch/assistant.go` (`makeAssistant`, and `EnsureAssistant` following the flags); `internal/orch/feedback.go:48`;
   `internal/importer/agent.go` (map a definition's `effort` as `model` is mapped, else list it unmapped).
5. **objectives-and-crew:** `internal/outcomes/outcomes.go` (`Model.Effort`, group by the pair).
6. **release-and-ops:** `cmd/remit/main.go` (defaults `fable`, flags `-chair-effort`, `-assistant-effort`), `cmd/remit/agents.go`
   (`requireModels` parses efforts), `README.md` (flag table and the catalog paragraph).
7. **web-ui:** show `model · effort` in `AgentRow.vue`, `CardSession.vue`, `AgentCard.vue`, `AgentsView.vue`, `PlaybooksView.vue`,
   `OutcomeTables.vue`, `FeedbackView.vue` and the search text in `feedbackColumns.ts`; seed an effort, and one card without, in
   `web/verify`.

## Tests (written with the change, main path first)

- `TestTheCatalogRanksFableAboveOpus` (`internal/provider`): `Default` order and tiers; `Fastest` is `haiku`.
- `TestAnEffortOutsideTheFiveIsRefusedWithTheList` (`internal/provider`): `ParseEffort` takes empty and the five, refuses the rest.
- `TestAHireRunsAtTheEffortItNamesElseItsAgentsElseTheChairs` (`internal/orch`): the stored card and the `agent.Spec` carry it.
- `TestTheChairIsToldToChooseForTheResult` (`internal/orch`): `modelsHelp` names effort and contains no "cheap".
- `TestTheAssistantFollowsItsFlagsAtEveryStart` (`internal/orch`): a stored Assistant card on `opus` moves to `fable`/`high`.
- `TestEffortReachesClaudeCodesCommandLine` (`internal/claudecode`): `--effort high` beside `--model fable`; nothing when empty.

## Revisit when

- An account cannot run a catalog model: key the breaker and the probe by (account, model), and refuse at start a flag model
  the default account cannot run. The seam is `agent.Failure`'s `access` kind and `probeModel`.
- A model refuses an effort level: give `provider.Model` the list of levels it takes and let `resolveModel` check it.
- Outcomes by model and effort show a pattern: give the Chair that evidence in `modelsHelp`, or let a coach employee propose
  defaults for agent definitions.
- Claude Code lets a running session change effort (as `set_model` does for the model, `internal/claudecode/conn.go:222`): the
  Chair could raise an agent's effort mid-task instead of hiring again.
- Judges: whether a judge at higher effort rules better is not measured; their effort stays Claude Code's default until it is.
