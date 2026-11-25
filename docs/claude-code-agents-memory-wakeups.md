# Claude Code internals: memory, subagents, wake-ups (Claude Code 2.1.322)

How Claude Code 2.1.322 remembers across sessions, runs subagents and talks to them, and keeps the model from being
woken too often. Companion to `docs/claude-code-hooks.md`.

**Sources.** The minified bundle `hackingtool/claude/src-2.1.322`. Each fact names its chunk and a string or constant
to grep for; identifiers are minified and only help finding the spot. Confidence: **HIGH** = read in source, **MEDIUM**
= inferred from the code around it. Server-side feature flags (`tengu_*`) are named where they gate behaviour. The
values one account gets are cached by the CLI in `cachedGrowthBookFeatures` of `<config dir>/.claude.json`; the values
quoted as "this machine" were read there on 2026-11-11 (two config dirs, same values).

---

## Part 1: Memory

Claude Code has two kinds of memory: instruction files (CLAUDE.md and rules) that a person writes, and **auto memory**,
a directory of Markdown files the model writes about the user and the project.

### 1.1 Instruction files (CLAUDE.md) — HIGH

Load order (`chunk-9r1hbf9p.js`, `Rsr()`):

1. Managed `CLAUDE.md` and its `.claude/rules/`, then the policy setting `claudeMd`.
2. User: `~/.claude/CLAUDE.md`, `~/.claude/rules/`.
3. Every directory from the filesystem root down to the cwd: `CLAUDE.md`, `.claude/CLAUDE.md`, `AGENTS.md`, then
   `.claude/rules/`, then `CLAUDE.local.md`.
4. The auto-memory index `MEMORY.md` (type `AutoMem`, see 1.3).

- `@path` includes recurse up to depth 5 (`sdo=5`); files outside the project need approval.
- A rule with `paths:` frontmatter loads only when Claude touches a matching file (`load_reason: path_glob_match`).
- Files in subdirectories below the cwd load when Claude works there (`nested_traversal`).
- `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD` adds the CLAUDE.md of `--add-dir` directories;
  `CLAUDE_CODE_DISABLE_CLAUDE_MDS` turns nested loading off.
- Each load fires the `InstructionsLoaded` hook.

### 1.2 Auto memory: on or off — HIGH

On by default. The setting `autoMemoryEnabled` defaults to `true` (`chunk-yqezv9gr.js`:
`if(e.autoMemoryEnabled!==void 0)return e.autoMemoryEnabled;return!0`). It is off when any of these holds:

| Switch | Effect |
|---|---|
| `autoMemoryEnabled: false` (settings, or the `/memory` dialog's Auto-memory toggle) | Off for the project |
| `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` | Off; set to a falsy value it forces auto memory on |
| `--safe-mode` / `CLAUDE_CODE_SAFE_MODE` | Off |
| `--restricted` / `CLAUDE_CODE_RESTRICTED` | Off (restricted mode also drops code-running tools and user/project/local settings) |
| `CLAUDE_CODE_SIMPLE`; a diskless cloud session; `CLAUDE_CODE_REMOTE` without a remote memory dir | Off |
| Per-session pause flags | Off for that session |

### 1.3 Where it lives and how it reaches the model — HIGH

- **Directory** (`chunk-yqezv9gr.js`, class `Mc`): `CLAUDE_COWORK_MEMORY_PATH_OVERRIDE`, else the setting
  `autoMemoryDirectory` (ignored in a checked-in project settings file, for security), else
  `~/.claude/projects/<key>/memory/`. `<key>` is the git or project root with every non-alphanumeric character turned
  into `-` (`replace(/[^a-zA-Z0-9]/g,"-")`), cut at 200 characters plus a hash. `CLAUDE_CODE_REMOTE_MEMORY_DIR`
  replaces `~/.claude`.
- **Index `MEMORY.md`** is loaded every session like a CLAUDE.md file, capped at **200 lines and 25,000 characters**
  (`chunk-a0mg7a7m.js`: `Em="MEMORY.md",DF=200,qte=25000`). Past the cap Claude Code cuts it and appends a warning:
  "Only part of it was loaded … Keep index entries to one line under ~200 chars; move detail into topic files."
- **Index backfill:** if `MEMORY.md` is missing but topic files exist, Claude Code writes the index itself, one
  `- [title](file.md) — description` line per file, filling at most 70% of the caps.
- **Topic files** are not in context by default. The model sees the index and reads a file with the Read tool when the
  index line looks relevant.
- **The instructions** the model gets are in the system prompt (`chunk-t5vyx4jc.js`). Newer models get the short
  ("lean") version, `Tl()`: one fact per file, frontmatter `name`/`description`/`metadata.type`, the four types
  `user`, `feedback`, `project`, `reference`, a one-line pointer in `MEMORY.md`, `[[name]]` links, and "Before saving,
  check for an existing file that already covers it. Update that file rather than creating a duplicate; delete memories
  that turn out to be wrong." Each file is kept under 4 KB because recall shows only the first 4 KB (`Xbe=4096`).

### 1.4 How memory is written — HIGH

Three writers, of which only the first is always on. **On this machine only the first runs** (HIGH, read from the
flag cache and the gate code below).

1. **The main model, with Write and Edit.** The memory directory is a permitted write target ("auto memory files are
   allowed for writing"). It saves when the user states a preference, corrects it, or a decision is made, following the
   system-prompt instructions above.
2. **Background extraction after each turn** (`chunk-80j7ffdw.js`, `executeExtractMemories`). Gate: flag
   `tengu_passport_quail` (default false) or `CLAUDE_CODE_POST_TURN_MEMORY_CONFIG`.
   - Runs after each main-thread turn (every N turns, `tengu_bramble_lintel`, default 1), never for subagents.
   - It is skipped when the conversation already wrote to memory in that span, when the user wrote fewer than 3 words,
     or while a pass is running (the request is kept for one trailing run).
   - It forks the conversation (`querySource: "extract_memories"`, `maxTurns: 5`, no transcript) with a restricted
     tool set: read-only tools plus Write/Edit inside the memory directory only; MCP and Agent are denied.
   - Its prompt: "You are now acting as the memory extraction subagent. Analyze the most recent ~N messages above and
     use them to update your persistent memory systems … turn 1 — issue all Read calls in parallel; turn 2 — issue all
     Write/Edit calls in parallel … no grepping source files … If nothing is worth saving, output only 'Nothing to
     save.'"
   - A headless run waits up to 60 s at exit for passes still running.
   - Gate code (`chunk-yqezv9gr.js`): `function MBt(){if(Ztt()!==null)return!0;if(!k("tengu_passport_quail",!1))return!1;…}`
     (`Ztt()` is the remote `CLAUDE_CODE_POST_TURN_MEMORY_CONFIG`). This machine: `tengu_passport_quail: false`, env var
     unset, so **off**. A second path runs extraction when a cloud session drives this machine (`isLaptopExtractActive`,
     MEDIUM). The cached cadence is `tengu_bramble_lintel: 7`, i.e. every 7 turns when it is on.
3. **"Dream" consolidation** (`chunk-yfsne8xx.js`, `chunk-sjmxrx3b.js`). Gate: flag `tengu_onyx_plover`, overridden
   by the setting `autoDreamEnabled` (the `/memory` dialog's Auto-dream toggle).
   - Runs at turn end only when at least **24 h** passed since the last dream and at least **5** other sessions ran
     since (`minHours:24,minSessions:5`). It scans at most once per 10 min and holds a lock file
     `<memdir>/.consolidate-lock` (stale after 1 h, rolled back on failure).
   - A forked agent (`querySource: "auto_dream"`) with the same restricted tools reads the memory files and greps
     recent session transcripts. Its prompt has four phases: Orient, Gather recent signal, Consolidate, Prune and
     index. `MEMORY.md` stays under 200 lines and ~25 KB, and the dream does not edit CLAUDE.md.
   - The user sees "Improved N memory file(s)".
   - Gate code (`chunk-yfsne8xx.js`): `function DSn(){let e=K();return e?.enabled===!0||e?.available===!0}` and
     `function FTt(){if(!DSn())return!1;let e=St().autoDreamEnabled;if(e!==void 0)return e;return K()?.enabled===!0}`.
     The setting only chooses among accounts the flag already offers it to: with neither `enabled` nor `available` set,
     `autoDreamEnabled: true` does nothing.
   - This machine: the flag is `{"enabled": false, "minHours": 24, "minSessions": 3, "remoteEnabled": false}` and the
     seat's `settings.json` has `"autoDreamEnabled": true`. `DSn()` is false, so dream is **off** despite the setting.
     The flag also lowers `minSessions` to 3 for accounts that get it.

### 1.5 Recall (memories injected per prompt) — HIGH that it exists, gated

Gate: flag `tengu_moth_copse` or `CLAUDE_MEMORY_STORES` (`chunk-9r1hbf9p.js`, "memdir prefetch").

- **The index leaves context.** `MEMORY.md` is removed, and the prompt drops the "add a pointer to `MEMORY.md`" step.
- **A selector picks the files.** On each user prompt of more than one word, a side call to **Sonnet**
  (`querySource: "memdir_relevance"`, 512 tokens, JSON schema `{selected_memories: [...]}`) gets the user's message.
  It also gets up to 200 candidate files, each listed as `- [type] filename (mtime): description`.
- **Selector prompt:** "Return a list of filenames … (up to 5). Only include memories that you are certain will be
  helpful … Be especially conservative with user-profile and project-overview memories."
- **How they are injected:** up to 5 files, each cut at 200 lines or 4 KB, go into a system reminder headed
  "Retrieved for possible relevance — use only if it actually applies to what the user asked."
- **Budget:** 60 KB per session (`MAX_SESSION_BYTES: 61440`).

When the gate is off, there is no selector: the model sees the index only. Gate code (`chunk-yqezv9gr.js`):
`function Rwe(){if(k("tengu_moth_copse",!1))return!0;return w$s()}`, where `w$s()` tests `CLAUDE_MEMORY_STORES`. This
machine: `tengu_moth_copse: false`, `CLAUDE_MEMORY_STORES` unset, so recall is **off** and `MEMORY.md` is loaded whole
(HIGH; this session's own context carries the `MEMORY.md` index, not a "Retrieved for possible relevance" block).

### 1.6 Subagent memory — HIGH

- **Opt-in per agent.** An agent definition's `memory` field gives that agent type its own directory:
  - `user` → `~/.claude/agent-memory/<agentType>/`
  - `project` → `.claude/agent-memory/<agentType>/`
  - `local` → `.claude/agent-memory-local/<agentType>/`
- **Loading.** Its `MEMORY.md` is loaded into the agent's system prompt with the same 200-line and 25,000-character
  caps, under a "Persistent Agent Memory" section.
- **Tools.** Read, Write and Edit are added to the agent's tools.
- **Writing.** The subagent writes these files itself. Background extraction and dream never touch them.

### 1.7 Not present in 2.1.322

There is no `/remember` or `/dream` command, no `#`-to-memory shortcut, and no separate end-of-session memory writer.
`/pause-memory` exists but is disabled (`isEnabled:()=>!1`).

---

## Part 2: Subagents and how they talk to the main agent

### 2.1 What a subagent is — HIGH

A subagent is an **async generator in the same process**, with its own message list, system prompt and query loop
(`chunk-6yy7c9r1.js`, `async function*DI`). It is not a child process. Two exceptions:

- cloud agents (`isolation: "remote"`) run elsewhere;
- teammates in tmux or iTerm2 panes are separate `claude` processes.

| Aspect | Fresh agent (any `subagent_type` but fork) | Fork (`subagent_type: "fork"`) |
|---|---|---|
| System prompt | The definition's prompt plus environment info | The parent's rendered system prompt, byte for byte |
| Conversation | Only the delegation prompt | The parent's whole message history |
| Tools | The parent's tools, filtered by the definition (`tools`, `disallowedTools`) and the deny lists below | The parent's exact tool pool |
| Model | The definition's, the call's `model`, or the default subagent model | Always the parent's; `model` is ignored |
| CLAUDE.md | Loaded, unless the definition sets `omitClaudeMd` (built-in `Explore` does) | Inherited with the history |
| Prompt cache | New prefix | Same prefix as the parent, so the cache is reused (MEDIUM) |
| `maxTurns` | The call's or the definition's | 200 |

- **Abort.** A foreground agent shares the parent's abort controller, so interrupting the parent stops it. A
  background agent has its own.
- **Transcript.** Each subagent has one at `~/.claude/projects/<dir>/<sessionId>/subagents/agent-<agentId>.jsonl`.
  This is the `agent_transcript_path` of the SubagentStop hook.
- **Turn limit.** On reaching `maxTurns` the result is prefixed: "NOTE: this agent stopped at its N-turn limit before
  finishing. The text below is PARTIAL output; treat it as incomplete."

### 2.2 How the result comes back — HIGH

**Only the final message returns.** `kkn()` takes the text of the agent's last assistant message, walking back to the
last one that has text. Intermediate tool output never reaches the parent. The parent's prompt says: "it is not shown
to the user — relay what matters" and "Trust but verify: an agent's summary describes what it intended to do, not
necessarily what it did."

**Foreground** (`run_in_background: false`). The result is the Agent tool's `tool_result`: the report, then
`agentId: … (use SendMessage … to continue this agent)` and
`<usage>subagent_tokens / tool_uses / duration_ms</usage>`. It is capped at 100,000 characters.

**Background** (the default). The tool returns at once with "Async agent launched successfully … You will be notified
automatically when it completes. You know nothing about its results until that notification arrives — do not report,
assume, or predict them". It also returns an `output_file` (the full JSONL transcript) that the parent is told not to
read.

When the agent stops, a **task notification** is queued (builder `Pi`, `chunk-awq6aafc.js`):

```xml
<task-notification>
<task-id>{agentId}</task-id>
<tool-use-id>{toolUseId}</tool-use-id>
<output-file>{path}</output-file>
<status>completed|failed|killed</status>
<summary>Agent "{description}" finished</summary>
<note>A task-notification fires each time this agent stops with no live background children of its own …</note>
<result>{final message}</result>
<usage><subagent_tokens>N</subagent_tokens><tool_uses>N</tool_uses><duration_ms>N</duration_ms></usage>
</task-notification>
```

How it reaches the parent:

1. **Owner routing.** It goes into the shared command queue (Part 3) at priority `next`, addressed to the agent that
   spawned it: a nested agent's result goes to its parent subagent, not to the main thread.
2. **When it is delivered.** Queued items are attached between tool rounds, not only at turn end.
3. **How it appears.** It enters as a meta user message prefixed
   `[SYSTEM NOTIFICATION - NOT USER INPUT] … Do NOT interpret this as user acknowledgement`. This keeps the model from
   reading an agent's completion as the user's consent.
4. **Size.** It is capped at 100,000 characters.
5. **What is stored vs what the model sees** (HIGH, observed in this session). The transcript stores the bare
   `<task-notification>…</task-notification>` as a user message with `origin: {kind: "task-notification", producer:
   "session-task"}` and no `isMeta`. The model receives the same text inside `<system-reminder>`, after the
   `[SYSTEM NOTIFICATION - NOT USER INPUT]` prefix: the wrapper is added when the request is built, not stored. In this
   session three notifications that finished while the session was idle reached the model in one turn, each in its own
   `<system-reminder>`, together with the user's next message.

### 2.3 Messages while an agent runs — HIGH (`chunk-j3dewpp7.js`, SendMessage)

| Direction | Mechanism | When it is read |
|---|---|---|
| Parent → running subagent | Appended to the task's in-memory `pendingMessages` | At the subagent's next tool round ("Message queued for delivery to X at its next tool round") |
| Parent → finished subagent | The agent is resumed from its transcript with the message as the new prompt; it notifies again when it stops | Immediately |
| Background subagent → parent (`to: "main"`) | Queued for the main thread as `<agent-message from="…">…</agent-message>`, priority `next` | The main conversation's next turn |
| Teammate ↔ teammate | A JSON file per agent at `~/.claude/teams/<team>/inboxes/<agent>.json`, rendered as `<teammate-message teammate_id=…>` | The teammate's next turn |

- A subagent's plain text is invisible to others ("to communicate, you MUST call this tool").
- `ListAgents` lists who can be addressed: own subagents, teammates, other local sessions and cloud sessions.
- `TaskStop` stops one by ID or name, and its notification then says "was stopped by Claude".

### 2.4 Fork rules — HIGH

- **The directive.** A fork's last message is a `<fork-boilerplate>` block: "You are a worker fork. The transcript
  above is the parent's history — inherited reference, not your situation … Execute ONE directive, then stop. Do NOT
  spawn subagents … One shot: report once and stop."
- **No re-delegation, enforced three ways:** the parent's prompt, the boilerplate, and a hard check. An Agent call
  from a fork fails with "Fork is not available inside a forked worker." A remote fork is refused, because a remote
  session cannot inherit the conversation.

### 2.5 Teams and Workflow — HIGH

- **Teams** are gated by `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` or `--agent-teams` plus the flag `tengu_amber_flint`.
  - A teammate is spawned by giving the Agent tool a `name` inside a team. It runs as a separate process in a tmux or
    iTerm2 pane, or in-process.
  - Teammates share a task list (TaskCreate/TaskUpdate/TaskList) and talk through the file mailbox.
  - The roster is flat: teammates cannot spawn teammates.
  - Hooks: `TeammateIdle` (exit 2 keeps the teammate working), `TaskCreated` and `TaskCompleted` (exit 2 blocks).
- **Workflow** runs a JavaScript script that orchestrates `agent()`, `parallel()`, `pipeline()` and `phase()`. It
  returns a run ID at once and sends one task notification when the run ends. It needs explicit opt-in, and subagents
  cannot use it.

### 2.6 Guardrails — HIGH

| Limit | Value | Source |
|---|---|---|
| Nesting depth | 3; `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH` or flag `tengu_hazel_trellis` override. At the cap the Agent tool is removed | `chunk-az40rghp.js` `PA()`, `n=3` |
| Concurrent subagents | 20; `CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS` overrides | `chunk-9r1hbf9p.js` `Ros=20` |
| Skills preloaded per agent | 32 | `Ms=32` |
| Agent with zero tools | Refused | "would be spawned with zero tools — refusing" |

- **Tools no subagent gets:** AskUserQuestion, Enter/ExitPlanMode (ExitPlanMode is allowed in plan mode),
  ScheduleWakeup, Workflow, ReadNotifications, the memory tools, and a few UI-only tools.
- **Permission prompts:**
  - A background agent cannot show a prompt: an "ask" becomes a deny, unless the agent runs in `bubble` mode (forks),
    which sends the prompt up to the parent.
  - A subagent declaring `bypassPermissions` keeps the parent's mode unless the parent already bypasses.

---

## Part 3: Waking the model without thrashing

### 3.1 One queue, three priorities — HIGH

Every source that wakes the model without a user message goes through one command queue (`chunk-9r1hbf9p.js`, queue
factory `Jco`).

| Priority | Delivery |
|---|---|
| `now`, `next` | Can be folded into a running turn between tool rounds |
| `later` | Waits until the current turn ends; never interrupts work |

| Wake source | Priority |
|---|---|
| Subagent, workflow, background Bash completion | `next` |
| Monitor output | `next`, batched and rate-limited (3.3) |
| Message from a background subagent to `main` | `next` |
| ScheduleWakeup, `/loop` wakeups, CronCreate fires | `later` |
| Poll events | `next` if `wake` is true, else `later` (they never wake the model alone) |
| Peer sessions | Rate-limited at admission (3.5) |

The scheduler ticks every 1 s and **fires nothing while a query is running**. Since a due fire then waits at `later`,
a busy session absorbs wake-ups instead of being interrupted by them.

### 3.2 ScheduleWakeup — HIGH (`chunk-mdwnbxvp.js`)

- **Clamp.** The delay is clamped to **[60, 3600] s** (`L=60,Tve=3600`). NaN becomes 60, ±Infinity becomes 3600 or
  60. The fire time is rounded up to a whole minute.
- **One pending wakeup at a time.** A new call cancels the previous one.
- **Cache-aware snapping.**
  - A delay of 300 s or less is pulled back so the request lands before the 5-minute prompt cache expires (15 s lead).
  - If the last API request was within 10 min, the fire is pulled back to land before that request's 1-hour cache
    expires.
- **End the turn.** The tool result says: "If you are reading this mid-turn, end the turn now — the harness
  re-invokes you when the wakeup fires or a task-notification arrives."
- **Prompt guidance, picked by cache TTL:**
  - 5-minute TTL: "Don't pick 300s … either drop to 270s (stay in cache) or commit to 1200s+".
  - 1-hour TTL: "scheduling extra wakeups just to keep the cache warm is pure waste".
  - Both: "Do NOT schedule a short-interval wakeup to poll for background work you started", and a CI run of ~8 min
    deserves "one ~480s check, not eight 60s ones".
- **Loop lifetime.** A dynamic `/loop` chain ends after **7 days** ("The loop reached its maximum duration — do not
  re-issue").
  - If the model ends a tick without rescheduling, the loop ends.
  - An optional keepalive (`CLAUDE_CODE_LOOP_KEEPALIVE`, default off) arms one 1200 s fallback, then stops ("model
    declined to reschedule twice").
  - Esc cancels pending wakeups.
- **No-op ticks.** A tick reported with `noop: true` is folded: the next wakeup header reads "N no-op ticks since …",
  with a note "[N prior /loop wakeups found nothing actionable; loop is healthy.]". The fold is vetoed when user input,
  a tool denial, or reported work happened in between.
- **Prompt-cache TTL** (`chunk-9r1hbf9p.js`, `bMe()`, which `fV()` tests for `"1h"`). The first rule that matches wins:
  1. `FORCE_PROMPT_CACHING_5M` → 5 min.
  2. `CLAUDE_CODE_PROMPT_CACHE_TTL` (main thread) or `CLAUDE_CODE_SUBAGENT_PROMPT_CACHE_TTL`; then the settings
     `promptCacheTtl` / `subagentPromptCacheTtl`; then an agent definition's TTL.
  3. `ENABLE_PROMPT_CACHING_1H` (or `ENABLE_PROMPT_CACHING_1H_BEDROCK` on Bedrock) → 1 h.
  4. Not a Claude.ai subscriber login, or the account is in usage overage (`isUsingOverage`) → 5 min.
  5. The request's query source is not on the allowlist `tengu_prompt_cache_1h_config` → 5 min. Built-in default
     `["repl_main_thread*","sdk","auto_mode","memdir_relevance"]`; this machine's cached list adds `rolling_compact`,
     `agent_classifier`, `prompt_suggestion`, `away_summary`, `extract_memories`.
  6. Flag `tengu_tidy_frost` with a live 5-minute cache on the model → 5 min.
  7. Otherwise 1 h (`reason: "subscriber"`).

  So a subscriber's main thread gets 1 h, and drops to 5 min in overage; subagent requests (`agent:*` sources) are not
  on the allowlist and get 5 min unless an env var, a setting or the agent definition says otherwise (HIGH).
- **Short repeated prompt.** The `<<autonomous-loop…>>` sentinels expand to the full preamble only on the first tick.
  Later ticks get a short reminder, so the long text stays in the cached prefix.

### 3.3 Monitor — HIGH (`chunk-18ctswc9.js`, `chunk-v818tayn.js`)

| Mechanism | Value |
|---|---|
| Batch window: lines within it become one notification | 200 ms (`Se=200`) |
| Per-line / per-batch size | 500 / 3,000 characters |
| Token bucket | 10 notifications burst, refill 1 per 2 s (`fMe=10,pMe=2000`), so about 0.5 wake-ups per second sustained |
| Over the bucket | Batches are counted and one notice sent: "[N events suppressed — output rate too high. Consider using TaskStop …]" |
| Suppression longer than 30 s | The monitor is killed: "[Monitor stopped — too much output …]" |
| Monitor timeout | 5 min default, 30 min max |

### 3.4 Blocking busy-waits — HIGH (`chunk-9r1hbf9p.js`)

- **Long sleeps.** A foreground Bash call that is a bare `sleep N` with N ≥ **25 s** (`Sio=25`), or begins with one,
  is refused. The error says: "To wait for a condition, use Monitor with an until-loop … Do not chain shorter sleeps
  to work around this block."
- **Duplicate commands.** Starting the same command in the same directory while it still runs in the background is
  refused.
- **Bash prompt:** "you will be notified when it completes — do not poll."

### 3.5 Cron, peers, poll events — HIGH

- **Cron** (`chunk-c078hbr6.js`, `Bee`):
  - Each recurring job gets a fixed jitter of up to half its period (cap 30 min).
  - A one-shot set on a :00 or :30 minute fires up to 90 s early; the prompt says to avoid those marks because "the
    user will not notice, and the fleet will".
  - Periods missed while busy collapse into one fire, since the next fire is computed from now.
  - Recurring jobs expire after 7 days.
  - At most 50 jobs, minimum interval 1 minute.
- **Peer messages** (`chunk-gc71ge1r.js`):
  - Token bucket of 30, refilling 0.5 per second.
  - A duplicate body within 30 s is dropped.
  - Message chains are cut by hop limits (10 self-hops, chain length 28).
- **Poll events** (`chunk-8vkz2fhk.js`): delivered in groups of at most 20 events or 32 KB, the rest in the next
  delivery; at most 1,000 queued.
- **Self-wakes of an attached computer:** at least 60 s apart, at most 2 in a row, at most 10 per 24 h, reset when a
  person writes.

### 3.6 The "user hasn't heard from you" reminder — HIGH (`chunk-9r1hbf9p.js`)

This one is the reverse problem: the model working too long without saying anything.

- **What counts as silent.** An API turn with no visible text, visible thinking, or user-visible tool.
- **When it fires.** After **5** such turns (`EWn=5`, `tengu_hushed_lark`, `CLAUDE_CODE_SILENT_TURN_REMINDER_TURNS`),
  at most **3** times per silent stretch (`CWn=3`).
- **What it says:** "The user hasn't heard from you in a while. As you continue, keep them updated when there's
  something to tell — a finding, a change of plan."
- **Time mode.** Optionally it is time-based instead: 60 s (`$5r=60`) via `CLAUDE_CODE_SILENT_TURN_REMINDER_SECONDS`.

### 3.7 Summary: the five layers

1. **Priority queue.** Scheduled wakes wait for the turn to end, and nothing fires mid-query.
2. **Hard limits.** The 60–3600 s clamp, the Monitor token bucket and kill switch, the long-sleep block, the peer and
   poll limits.
3. **Jitter and cache snapping.** Fires are spread across the fleet and land inside a warm prompt cache.
4. **Expiry and budgets.** 7-day loops, one pending wakeup, a keepalive budget of 1, self-wake caps.
5. **Prompt guidance.** Don't poll what the harness tracks; pick delays by what you wait for; 1200–1800 s for idle
   ticks.

### 3.8 Not found

- No per-second cap on background Bash completion notifications; they are deduped per task and capped at 100,000
  characters.
- No global "wake-ups per session" counter for ScheduleWakeup.
- How the cache TTL is chosen is traced in 3.2.
