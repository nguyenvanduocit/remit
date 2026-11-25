# 0084. Hooks hold the rails mid-turn and show what Claude Code does to a session

Status: accepted, 2026-11-12. Extends ADR-0083 (the first hook Remit answers) and ADR-0021 (what a session may not change).

## Context

Claude Code 2.1.322 fires 33 hook events (`docs/claude-code-hooks.md`); a session registered one, `PermissionDenied`
(ADR-0083). Three gaps were found by reading the code and the dev instance's data:

- **Pause ends a turn by interrupting it** (`Effect::CancelTurns`, `cut_turn_of`). Until the interrupt lands, or when it
  never does, the agent starts tool calls, and auto mode allows most of them without asking Remit (`can_use_tool` only
  sees a call that needs a permission).
- **Agents make their own worktrees.** Four cards of the dev instance ran `git worktree add --detach` into Claude Code's
  scratchpad to look at master or an MR's head; two entries stayed in the registry of the clone every chain shares
  (`git worktree list`). `EnterWorktree` was allowed too.
- **A worktree's `.claude/settings.json` was writable.** Sessions read project settings (`setting_sources: project`), so an
  agent could change what its next session may do and run (permissions, hooks, `env`).
- **Some of what Claude Code does to a session is not shown**: a request to the model retried (`system/api_retry`), a
  compaction or a failed one (`compact_boundary`, `status`), which instructions files it read, a model switch it made on
  its own. The stream dropped the first two (`agent/src/claude.rs`'s `system`).

The hook crate parsed closed sets strictly, so one value a newer Claude Code adds failed the whole callback; and a hook
could not say what its failure does.

## Options

1. **Hooks for what only a hook can do or see (chosen).** Rails answer `PreToolUse` and `ConfigChange`, registered to
   fail closed; observation reads the stream where the stream says it and a hook only where it does not
   (`InstructionsLoaded`, `PostModelSwitch`).
2. **Every event as a hook.** Most repeat the stream (tool calls, results and their times, subagents as tasks, the
   assistant message's typed error) or need nothing from Remit; two owners for one fact.
3. **Telemetry (Claude Code's OpenTelemetry export).** A second pipeline beside the stream for the same facts, and it
   cannot refuse anything.

## Decision

- **`claudehooks` reads a newer Claude Code.** Every set has an `Other` variant (`Unknown` for `ApiError`, and
  `Event::Unknown`), so a new value parses. `tests/payloads` holds what a real 2.1.322 sent for 11 events;
  `tests/contract.rs` parses them and the newer-value cases.
- **A hook says what its failure does.** `claudecode::Options::hooks` maps each event to `OnFailure`, sent as the
  matcher's `onFailure`. Rails `Block`, the rest `Continue`. A process asked for a `Block` hook starts only when its
  initialize answer says `hooks_applied: true` and `hook_on_failure_supported: true` (`Options::unheld_hooks`); a
  Claude Code that dropped the hooks or ignores `onFailure` would run the session with its rails failing open, so
  `Conn::start` ends it with `Error::Unguarded`. The real 2.1.328 answers both `true`.
- **A tool call starts only when the rails let it.** Every session answers `PreToolUse` where it runs, in the AI
  runtime's process (`agent/src/claude.rs`), without a round trip to the core: from the core's `cancel` (a pause, a
  stop, the owner's interrupt) until the session next goes to work or to rest, or takes a prompt, every call is denied.
  The deny holds in `auto` and `bypassPermissions` (checked against the real 2.1.322). The owner sees a `refused`
  note.
- **Settings change between sessions only.** `ConfigChange` of any settings source is blocked (skills still load), and
  `orch::policy::controls` makes `.claude/settings.json` and `.claude/settings.local.json` of every clone and worktree
  read-only to every session, beside the git files of ADR-0021.
- **A worktree is Remit's to make.** `EnterWorktree`, `ExitWorktree` and `Bash(git worktree add:*)` are deny rules. Every
  session's brief says how to get a tree at another commit: `git clone --shared` into its scratchpad, which leaves
  nothing in the shared clone.
- **What Claude Code does to a session is a note.** `agent::Update::Note` carries a `NoteKind` (`retrying`, `compacted`,
  `instructions_read`, `model_switched`, `refused`) and its words, crosses the runtime link as `EntryBody::Note`, and the
  core records it as a system event of that status, which the stream shows with its icon.

## Consequences

- Pause and stop hold mid-turn even when the interrupt is slow to land.
- A Claude Code update that adds a value degrades to "unknown" instead of failing every hook; one that drops a required
  field fails the contract test before it ships.
- If Remit cannot answer `PreToolUse` (the callback errors), the call is refused: a hook failure stops tool calls until
  it is fixed. A Claude Code too old to say it holds those rails starts no session.
- An agent cannot change a repository's Claude Code settings, even when that is the customer's ask.
- The shared clone's registry holds only Remit's worktrees.

## Revisit when

- **Linux and missing files.** The Linux jail skips a read-only path that does not exist (`mounts_linux.rs`
  `bind_into`), so an agent there can create a `.claude/settings.json` its repository lacks; its next session reads it.
  The control: bind an empty read-only file, or start sessions with `setting_sources` empty and pass the repository's
  settings Remit reviewed.
- **`git worktree add` past the rule.** The deny rule is a tripwire on the command's words; a script still adds one. The
  wall: the jail refuses new entries under `.git/worktrees/`.
- **A customer needs settings changed.** A capability that lifts the read-only settings for one chain.
- **More to follow.** A new `NoteKind` is a variant and an icon; `Notification`, `SubagentStart` context and `PreCompact`
  instructions wait for a need the stream and the brief cannot meet.
