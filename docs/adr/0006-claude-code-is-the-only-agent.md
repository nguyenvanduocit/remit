# 0006. Claude Code is the only agent

Status: accepted, 2026-10-28; its one login extended by ADR-0040 (a pool of accounts); its catalog extended by ADR-0070 (fable, and effort beside the model). Supersedes the moves between providers in ADR-0005. Source: the owner's request of 2026-10-28,
the live database (291 of 687 cards on a Codex model), and ADR-0005's account of the failovers of 2026-10-26 and 2026-10-27.

## Context

Remit ran agents on two subscription accounts, Claude and Codex, and moved a card to the other one when its own failed
(ADR-0005). Supporting both cost the codebase a second agent process (codex-acp) with its own dialect: a Codex home and
profile of its own, a router choosing the agent by model, per-token prices for a provider that reports no cost, a second
way to deliver instructions and skills, an approval path for its MCP calls, and deny rules applied by the client because
codex-acp reads none. Every agent capability was built twice or held back to what both could do (a Dispatcher's tool list
only Claude Code applies, a read-only Codex session for an agent with no edit tool).

The moves themselves did not help when it mattered: ADR-0005 records that on 2026-10-26 the six cards moved to Codex hit its
429 thirty-four seconds later, and none of the 40 failures that day had a provider left to move to. What saved the work
was waiting, which ADR-0005 decided.

The owner asked to run every agent on Claude Code alone, signed in with the machine's own Claude Code login, not a token.

## Options

1. Keep both providers. Rejected: the owner's request, and the cost of every capability built twice.
2. Keep the provider abstraction with Claude as its only member, ready for another. Rejected: an abstraction with one
   member is a flag in disguise, and the moves it served did not save the work (above).
3. Run every agent on Claude Code, and keep ADR-0005's wait: a card whose provider fails waits for it.

## Decision

Option 3.

- Every agent runs on Claude Code. A card's model is `opus`, `sonnet` or `haiku` (`provider.Default`). A flag that names
  another model refuses to start the server.
- Sessions use the machine's Claude Code login: `-claude-config-dir`, default `$CLAUDE_CONFIG_DIR`, else `~/.claude`. No
  API key is set. They run in a config dir of their own, `<data>/claude`, that links that login and holds no plugin, skill
  or setting of the machine's user.
- When the provider fails, the card waits for it, its message kept, and goes on by itself once it answers (ADR-0005's
  wait). No card moves, so no card stores the model it came from.
- Stored cards migrate forward once (migration 37): a card on a Codex model goes back to the Claude model failover moved it
  from, or else to the Claude model of its tier (`gpt-6-astra` to `opus`, `gpt-6.1-sol` to `sonnet`, `gpt-6-luna` to
  `haiku`). Its session id stays: Claude Code finds no such session, so the card's next session is a replacement given
  the card's brief again, the path any session the agent no longer has takes (`ContextLost`). A cleared id would open a
  session without the brief. Every card, finished ones included, so runtime code reads one set of models.

## Consequences

- One agent process, one dialect: instructions, skills, tools and deny rules reach every session the same way.
- No way out of a Claude outage: work waits until the account's limit resets, which can be days for a weekly limit.
  ADR-0005's consequences on waiting hold.
- Past events and decisions keep naming Codex models: they record what happened.
- A migrated card that is opened again starts a fresh Claude session with its brief and a note that its context was
  lost: what it did before is in its worktree and its stream, not in the session.

Enforced by: TestAFlagThatNamesAModelNotOfferedFailsAtStart, TestSessionsStartOnClaudeCodeInTheAgentsConfigDir, TestAgentsConfigDirSharesTheLoginAndNothingElse, TestACardWhoseProviderFailsWaitsForIt, TestMigration37PutsEveryCardOnAClaudeModel

## Revisit when

- A Claude outage costs the owner more waiting than he will bear, and another agent can run the same capabilities.
