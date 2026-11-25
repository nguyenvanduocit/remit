# Claude Code hooks: reference (Claude Code 2.1.322)

Every hook event Claude Code 2.1.322 fires, what it means, the JSON it hands a hook, the JSON a hook answers with, and
how exit codes act. Typed in Rust by `crates/claudehooks`. Memory, subagents and wake-ups: `docs/claude-code-agents-memory-wakeups.md`.

**Sources.** Read from the bundled CLI (`hackingtool/claude/src-2.1.322`), not from the public docs:

| What | File | Where |
|---|---|---|
| Event list (33) | `chunk-mttfbqwj.js` | `rc=[...]` |
| Input schemas (zod) | `chunk-smzekmsg.js` | `hook_event_name:C("<Event>")`, byte ~240900–252500 |
| Output schemas (zod) | `chunk-smzekmsg.js` | right after the inputs (`hookEventName:C(...)`) |
| Menu text, exit codes, matcher values | `chunk-rffppytt.js` | function `E6t` |
| Hook config schema | `chunk-mttfbqwj.js` | function `Hm()` |
| Matcher | `chunk-9r1hbf9p.js` | functions `s5`, `Npt` |
| Output parser | `chunk-9r1hbf9p.js` | function `r5` |
| Where the input object is built | `chunk-9r1hbf9p.js` | `hook_event_name:"<Event>"` object literals |

**Checked against a running Claude Code 2.1.322.** The payloads of SessionStart, UserPromptSubmit, PreToolUse,
PostToolUse, PostToolUseFailure, PostToolBatch, Stop, SessionEnd, InstructionsLoaded, SubagentStart and SubagentStop
were captured from a real `claude -p` run and parsed by `crates/claudehooks`. A `PreToolUse` deny answer (`examples/guard.rs`) blocked a real `rm -rf`, and a deny held in `auto` and
`bypassPermissions` modes alike. The other events are read from source only.

## 1. Registering hooks

In `settings.json` (user, project, local, managed) or a plugin's hooks:

```jsonc
"hooks": {
  "PreToolUse": [
    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "~/bin/guard", "timeout": 10 } ] }
  ]
}
```

### Matcher

- Empty or `*` matches everything.
- Made only of `[A-Za-z0-9_|]`: an exact list of names split on `|`. Tool events also accept `,`, space and `-`.
- Anything else is a regex.
- Events without matcher values (see the tables) ignore `matcher`.

What the matcher is compared with depends on the event: the tool name for tool events, `source` for SessionStart,
`trigger` for PreCompact, and so on (column *Matcher* below).

### Handler types

| `type` | Required | Optional | What it does |
|---|---|---|---|
| `command` | `command` | `args`, `shell`, `if`, `timeout`, `onFailure`, `statusMessage`, `once`, `async`, `asyncRewake` | Runs a shell command (or, with `args`, the executable directly, no shell) with the input on stdin |
| `prompt` | `prompt` | `if`, `timeout`, `model`, `continueOnBlock`, `statusMessage`, `once` | An LLM (default: the small fast model) judges the input (`$ARGUMENTS`) and answers `{ok, reason?, impossible?}`; `ok: false` is a block |
| `agent` | `prompt` | `if`, `timeout` (default 60), `model` (default Haiku), `statusMessage`, `once` | An agent with tools verifies a condition; same answer as `prompt` |
| `http` | `url` | `headers`, `allowedEnvVars`, `if`, `timeout`, `onFailure`, `statusMessage`, `once` | POSTs the input JSON; the response body is the answer |
| `mcp_tool` | `server`, `tool` | `input`, `if`, `timeout`, `statusMessage`, `once` | Calls a tool of a configured MCP server; string values in `input` take `${tool_input.file_path}`-style paths into the input |

The shared fields:

| Field | Meaning |
|---|---|
| `if` | A permission rule such as `"Bash(git *)"`. The hook starts only when the tool call matches it, which saves spawning a process |
| `timeout` | Seconds |
| `onFailure` | `continue` (default) or `block`. With `block`, a crash, timeout, exit code other than 0 or 2, or invalid JSON counts as exit 2. Ignored for async hooks and on Stop, SubagentStop, TaskCompleted and TeammateIdle |
| `once` | Run once, then remove |
| `async` | Run in the background; Claude Code does not wait |
| `asyncRewake` | Run in the background and wake the model when it exits 2. Implies `async` |
| `shell` | `bash` (your `$SHELL`) or `powershell` |
| `headers` / `allowedEnvVars` | `$VAR` or `${VAR}` in a header value is filled in only for names listed in `allowedEnvVars`; others become empty |
| `statusMessage` | Text shown in the spinner while the hook runs |

### Environment of a hook process

| Variable | Set for |
|---|---|
| `CLAUDE_PROJECT_DIR` | the project root; `${CLAUDE_PROJECT_DIR}` is also expanded in hook paths |
| `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA` | plugin hooks |
| `CLAUDE_EFFORT` | the effort level of the turn |
| `CLAUDE_ENV_FILE` | CwdChanged, FileChanged: `export` lines written there apply to later Bash tool calls |
| `CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS` | read by Claude Code: the time SessionEnd hooks get |

## 2. Input every event receives

```jsonc
{
  "session_id": "c149632b-…",
  "transcript_path": "~/.claude/projects/<dir>/<session>.jsonl",
  "cwd": "/tmp/proj",
  "hook_event_name": "PreToolUse",
  "prompt_id": "10d7f4ab-…",          // optional: joins a prompt with every event until the next one (= OTel prompt.id)
  "permission_mode": "auto",          // optional
  "agent_id": "…",                    // optional: only inside a subagent; tells subagent calls from main-thread ones
  "agent_type": "Explore",            // optional: the subagent type, or the main thread's --agent
  "effort": { "level": "medium" }     // optional: on tool-context events with a model that has effort
}
```

Tool events also carry `mcp_server: {name, source}` for an `mcp__*` tool. `source` (`sdk`, `plugin`, `user`, `project`,
`local`, `dynamic`, `managed`, `enterprise`, `claudeai`, `agent`, open set) is the field to trust, not `name`.

## 3. What a hook answers

### Exit code

| Exit | Meaning |
|---|---|
| 0 | Success. stdout is parsed as JSON if it is JSON; otherwise it is plain text, used as the event says |
| 2 | Blocking error. stderr goes where the event's *Exit 2* column says |
| other | Non-blocking error: stderr is shown to the user and the action goes on (unless `onFailure: block`) |

### JSON on stdout (exit 0)

```ts
{
  continue?: boolean,          // false: stop after the hooks; stopReason is shown to the user
  stopReason?: string,
  suppressOutput?: boolean,    // hide stdout from the transcript
  decision?: "approve" | "block",
  reason?: string,             // the message of a block
  systemMessage?: string,      // a warning shown to the user
  terminalSequence?: string,   // OSC 0, 1, 2, 9, 99, 777 or BEL emitted for the hook; anything else dropped
  hookSpecificOutput?: { hookEventName: "<the event>", ... }
}
// or, to detach:
{ async: true, asyncTimeout?: number }
```

How Claude Code reads it (parser `r5`, `chunk-9r1hbf9p.js`):

- **`decision: "block"`** works on every event and every handler type. It is treated exactly like exit 2, and `reason`
  is the message ("Blocked by hook" when absent). What a block does is the event's *Exit 2* column.
- **`decision: "approve"`** means allow; it matters only where the event decides a permission.
- **PreToolUse:** `hookSpecificOutput.permissionDecision` is read after `decision` and replaces it. For `deny` the
  message is `permissionDecisionReason`, then `reason`.
- **`continue: false`** prevents the conversation from continuing; `stopReason` is the message.
- **A `hookEventName`** other than the event the hook ran for is an error ("Hook returned incorrect event name"), and
  any other `decision` value is an error ("Unknown hook decision type").
- Several hooks on one PostToolUse run in parallel on the original output, so two `updatedToolOutput` rewrites race,
  last write wins (stated in the `classifierContext` schema).

## 4. The 33 events

### Tools and permissions

| Event | Fires | Matcher | Extra input fields | `hookSpecificOutput` | Exit 2 |
|---|---|---|---|---|---|
| **PreToolUse** | Before a tool runs | tool name | `tool_name`, `tool_input`, `tool_use_id`, `mcp_server?` | `permissionDecision` (`allow`/`deny`/`ask`/`defer`), `permissionDecisionReason`, `updatedInput` (replaces the input), `additionalContext` | Blocks the call; stderr goes to the model |
| **PermissionRequest** | When the permission dialog would show | tool name | `tool_name`, `tool_input`, `permission_suggestions?` (permission updates), `mcp_server?` | `decision`: `{behavior:"allow", updatedInput?, updatedPermissions?}` or `{behavior:"deny", message?, interrupt?}` | – (exit 0 applies the decision) |
| **PostToolUse** | After a tool succeeds | tool name | `tool_name`, `tool_input`, `tool_response` (the tool's structured result), `tool_use_id`, `duration_ms?` (without prompts and hooks), `mcp_server?` | `additionalContext`, `updatedToolOutput` (replaces what the model sees), `updatedMCPToolOutput` (MCP only), `classifierContext` | stderr goes to the model |
| **PostToolUseFailure** | After a tool fails | tool name | `tool_name`, `tool_input`, `tool_use_id`, `error`, `is_interrupt?`, `duration_ms?`, `mcp_server?` | `additionalContext` | stderr goes to the model |
| **PostToolBatch** | Once after every call of a batch resolved, before the next model request | – | `tool_calls[]`: `{tool_name, tool_input, tool_use_id, tool_response?}` | `additionalContext` (once for the whole batch) | Stops the agent loop |
| **PermissionDenied** | After auto mode denied a call | tool name | `tool_name`, `tool_input`, `tool_use_id`, `reason` | `retry: true` tells the model it may retry | – |

`PostToolUse.tool_response` is the tool's own result (for Bash `{stdout, stderr, interrupted, isImage,
noOutputExpected}`), while `PostToolBatch.tool_calls[].tool_response` is what the model saw (for Bash a string). Both
were seen in a real run.

`classifierContext` is a short, genuine user statement shown to the auto-mode permission classifier beside this call's
result: at most 2000 UTF-16 units shared by every hook of the call, honoured only from synchronous hooks. Never copy tool
output into it.

`permission_suggestions` and `updatedPermissions` are permission updates, tagged by `type`:
`addRules` / `replaceRules` / `removeRules` (`rules: [{toolName, ruleContent?}]`, `behavior: allow|deny|ask`,
`destination`), `setMode` (`mode`, `destination`), `addDirectories` / `removeDirectories` (`directories`,
`destination`), where `destination` is `userSettings`, `projectSettings`, `localSettings`, `session` or `cliArg`.

### Prompts and turns

| Event | Fires | Matcher | Extra input fields | `hookSpecificOutput` | Exit 2 |
|---|---|---|---|---|---|
| **UserPromptSubmit** | A prompt is submitted | – | `prompt`, `source?` (`user`/`sdk`/`system`/`loop_wakeup`/`schedule_wakeup`/`poll_event`), `session_title?` | `additionalContext`, `sessionTitle`, `suppressOriginalPrompt` (with a block, leaves the prompt out of the message) | Blocks the prompt and erases it; stderr to the user |
| **UserPromptExpansion** | A slash command or MCP prompt expands into a prompt | – | `expansion_type` (`slash_command`/`mcp_prompt`), `command_name`, `command_args`, `command_source?`, `prompt` | `additionalContext`, `suppressOriginalPrompt` | Blocks the expansion |
| **Stop** | Right before Claude ends its response | – | `stop_hook_active` (a Stop hook already blocked once: guard against loops), `last_assistant_message?`, `background_tasks?[]`, `session_crons?[]` | `additionalContext` (feedback; the model keeps working) | stderr goes to the model and the turn continues |
| **StopFailure** | In place of Stop when an API error ended the turn | error | `error`, `error_details?`, `last_assistant_message?` | – (output and exit code ignored) | – |
| **MessageDisplay** | Each batch of newly completed lines of a streaming assistant message | – | `turn_id`, `message_id`, `index` (int), `final`, `delta` | `displayContent` (shown in place of the delta; the stored message and the model are untouched) | – (any failure shows the original) |

`stdout` of UserPromptSubmit and UserPromptExpansion at exit 0 is added for Claude to read.
`source: poll_event` fires before an incoming event is acknowledged, so a block rejects the event.

`background_tasks[]`: `{id, type (shell|subagent|monitor|workflow|…), status, description, command?, agent_type?,
server?, tool?, name?}`; `session_crons[]`: `{id, schedule (cron), recurring, prompt}`. Together they tell "the session
is done" from "the session waits for background work or a scheduled wake-up".

`StopFailure.error`: `authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`, `verification_required`,
`billing_error`, `rate_limit`, `overloaded`, `invalid_request`, `model_not_found`, `server_error`, `unknown`,
`max_output_tokens`, `cloud_credential_error`.

### Session and lifecycle

| Event | Fires | Matcher | Extra input fields | `hookSpecificOutput` | Exit 2 |
|---|---|---|---|---|---|
| **SessionStart** | A session starts | `startup`/`resume`/`clear`/`compact`/`fork` | `source`, `model?`, `session_title?`; on resume/fork: `seconds_since_last_response`, `context_tokens`, `prompt_cache_likely_expired`, `estimated_cache_write_usd` | `additionalContext`, `initialUserMessage`, `sessionTitle`, `watchPaths` (for FileChanged), `reloadSkills` (re-scan skills a hook installed) | stderr to the user only |
| **SessionEnd** | A session ends | `clear`/`resume`/`logout`/`prompt_input_exit`/`other` | `reason` | – | – |
| **Setup** | Repository setup | `init`/`maintenance` | `trigger` | `additionalContext` | stderr to the user only |
| **PreCompact** | Before compaction | `manual`/`auto` | `trigger`, `custom_instructions` (string or null) | – (stdout at exit 0 is added to the compaction instructions) | Blocks compaction |
| **PostCompact** | After compaction | `manual`/`auto` | `trigger`, `compact_summary` | – (stdout shown to the user) | – |
| **PreModelSwitch** | Before `/model`, the model picker or `set_model` switches | – | the model-switch fields below; `source`: `command`/`picker`/`sdk` | `permissionDecision` (`allow` skips the cache-miss confirm, `deny` cancels, `ask` confirms), `permissionDecisionReason` | Blocks the switch |
| **PostModelSwitch** | After the model changed, for any reason | – | the model-switch fields; `source` also `auto`/`resume` | `additionalContext` (reaches the new model on its next request) | – |
| **Notification** | Claude Code sends a notification | `notification_type` | `message`, `title?`, `notification_type` | `additionalContext` | – |

Model-switch fields: `from_model`, `to_model`, `requested_model` (alias, id or null for default), `source`,
`context_tokens`, `prompt_cache_warm`, `cache_ttl` (`5m`/`1h`), `estimated_cache_write_usd`, `pricing`
(`configured`/`catalog`/`default`).

`notification_type` (open set): `permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog`,
`agent_needs_input`, `agent_completed`, `elicitation_url_dialog`, `worker_permission_prompt`, `push_notification`,
`computer_use_enter`, `computer_use_exit`, `quota_auto_resume_fired`, `quota_auto_resume_stale`,
`quota_auto_resume_disabled`, `model_refusal_fallback`, `auth_storage_failure`, `plugin_notification`,
`elicitation_complete`, `elicitation_response`.

### Subagents, teams and tasks

| Event | Fires | Matcher | Extra input fields | `hookSpecificOutput` | Exit 2 |
|---|---|---|---|---|---|
| **SubagentStart** | A subagent (Agent tool call) starts | – | `agent_id`, `agent_type` | `additionalContext` (given to the subagent) | stderr to the user only |
| **SubagentStop** | Right before a subagent ends | – | Stop's fields + `agent_id`, `agent_type`, `agent_transcript_path` | `additionalContext` | stderr goes to the subagent, which keeps running |
| **TeammateIdle** | A teammate is about to go idle | – | `teammate_name`, `team_name` (deprecated) | – | The teammate keeps working |
| **TaskCreated** | A task is being created | – | `task_id`, `task_subject`, `task_description?`, `teammate_name?`, `team_name?` | – | Prevents creation; stderr goes to the model |
| **TaskCompleted** | A task is being marked done | – | same as TaskCreated | – | Prevents completion; stderr goes to the model |

### MCP elicitation

| Event | Fires | Matcher | Extra input fields | `hookSpecificOutput` | Exit 2 |
|---|---|---|---|---|---|
| **Elicitation** | An MCP server asks the user for input | – | `mcp_server_name`, `message`, `mode?` (`form`/`url`), `url?`, `elicitation_id?`, `requested_schema?` | `action` (`accept`/`decline`/`cancel`), `content`: answers without showing the dialog | Denies the request |
| **ElicitationResult** | After the user answered | – | `mcp_server_name`, `elicitation_id?`, `mode?`, `action`, `content?` | `action`, `content`: overrides the answer before it is sent | The answer becomes `decline` |

### Configuration, files and worktrees

| Event | Fires | Matcher | Extra input fields | `hookSpecificOutput` | Exit 2 |
|---|---|---|---|---|---|
| **ConfigChange** | A settings file changes during the session | `user_settings`/`project_settings`/`local_settings`/`policy_settings`/`skills` | `source`, `file_path?` | – | Keeps the change out of the session |
| **InstructionsLoaded** | A CLAUDE.md or rule file is loaded | `session_start`/`nested_traversal`/`path_glob_match`/`include`/`compact` | `file_path`, `memory_type` (`User`/`Project`/`Local`/`Managed`), `load_reason`, `globs?`, `trigger_file_path?`, `parent_file_path?` | – (observe only, cannot block) | – |
| **CwdChanged** | The working directory changes | – | `old_cwd`, `new_cwd` | `watchPaths` | – |
| **FileChanged** | A watched file changes | file names, e.g. `.envrc\|.env` | `file_path`, `event` (`change`/`add`/`unlink`) | `watchPaths` (updates the watch list) | – |
| **DirectoryAdded** | `/add-dir` or the SDK's `register_repo_root` added a directory | `slash_command`/`register_repo_root` | `directory`, `source` | – | – |
| **WorktreeCreate** | Claude Code needs an isolated worktree (any version control system) | – | `name` (suggested slug) | `worktreePath`; a command hook prints the bare path on stdout instead | Any non-zero exit: creation failed |
| **WorktreeRemove** | A worktree is removed | – | `worktree_path` | – | – |

## 5. Corrections to the `/hooks` menu text

The menu text (`E6t`) is out of date in two places; the code that builds the input wins:

- **PostToolUse:** the menu says `"inputs"` and `"response"`. The input object
  (`chunk-9r1hbf9p.js`, `hook_event_name:"PostToolUse"`) is `{tool_name, tool_input, tool_response, tool_use_id,
  duration_ms, mcp_server?}`, which the real run confirmed.
- **PostToolUseFailure:** the menu lists `error_type` and `is_timeout`. The input object
  (`hook_event_name:"PostToolUseFailure"`) is `{tool_name, tool_input, tool_use_id, error, is_interrupt,
  duration_ms, mcp_server?}`; `rg 'is_timeout|error_type'` over the bundle finds them only in the menu text and in
  update-check telemetry. The real run sent no such fields.

Fields marked `@internal` upstream (`permissionDisplay` on PreToolUse, `cloud`, `rewakeMessage`, `rewakeSummary` in the
config) serve the desktop app, SDK hosts and cloud sessions; they are left out here and in the crate.

## 6. In Rust

```rust
use claudehooks::{parse_input, Event, HookOutput, SpecificOutput, output::PermissionDecision};

let input = parse_input(&stdin)?;                      // HookInput { common, event }
if let Event::PreToolUse { tool_input, .. } = input.event {
    let out = HookOutput {
        hook_specific_output: Some(SpecificOutput::PreToolUse {
            permission_decision: Some(PermissionDecision::Deny),
            permission_decision_reason: Some("not allowed".into()),
            updated_input: None,
            additional_context: None,
        }),
        ..HookOutput::default()
    };
    println!("{}", serde_json::to_string(&out)?);
}
```

A full hook is `crates/claudehooks/examples/guard.rs`; `claudehooks::config::Hooks` writes the `hooks` block of a
settings file.
