# claudecode

Runs one Claude Code process and speaks its SDK protocol over the `claude` CLI's stdin and stdout. It is our own client:
no SDK or Claude API library. The process inherits only the environment `childenv` names, runs jailed from the paths
`Options::hide` and `Options::read_only` list (`jail`), and leads its own process group, which ends whole on `close`
(`procgroup`, ADR-0018).

- `Conn` (conn.rs): start, initialize, send a user message, interrupt, set model and permission mode, close, done.
- `Options` (options.rs): the command line and the initialize request.
- `start_login`, `Login` (login.rs): `claude auth login` for a config dir, its link and code.
- `Auth`, `Status` (status.rs): `claude auth status` and `logout` for a config dir.
- `AccessToken`, `Store`, `Auth::fresh_token` (token.rs): the access token of a config dir's stored login (macOS Keychain
  item `Claude Code-credentials-<sha256(dir)[:8]>`, elsewhere `.credentials.json`), renewed by Claude Code itself when a
  process starts on the dir (ADR-0074).

## The protocol as implemented

Newline-delimited JSON both ways; the CLI runs with `--input-format stream-json --output-format stream-json --verbose
--permission-prompt-tool=stdio --include-partial-messages`.

Written by us:

- `{"type":"user","message":{"role":"user","content":[{"type":"text","text":…}]},"parent_tool_use_id":null,"uuid"?,"priority"?}`
  (`priority` is `now`, `next` or `later`).
- `{"type":"control_request","request_id":"req_N","request":{"subtype":…}}` with subtype `initialize`
  (`appendSystemPrompt`, `systemPromptSnapshot`, `skills`), `interrupt`, `set_model`, `set_permission_mode`.
- `{"type":"control_response","response":{"subtype":"success","request_id":…,"response":{"behavior":"allow","updatedInput":…,"toolUseID":…}}}`,
  or `"behavior":"deny","message":…`, answering `can_use_tool`; `{"subtype":"error","request_id":…,"error":…}` for any
  other control request (we register no SDK MCP servers or dialogs).
- `initialize` also registers the hooks of `Options::hooks`, one callback per event named after it, each with its
  `onFailure`; `{"type":"control_response","response":{"subtype":"success","request_id":…,"response":<HookOutput>}}`
  answers each `hook_callback`.

- `{"type":"update_environment_variables","request_id":"req_N","variables":{"CLAUDE_CODE_OAUTH_TOKEN":…}}` (`set_oauth_token`):
  not a control request, yet answered by a `control_response` of its id. The next request to the model, in the same turn,
  goes out with that token (docs/research-claude-account-hot-swap/).

Read from the CLI:

- `control_response`: settles our request of that id (`success` with its `response`, or `error`).
- `control_request` with subtype `can_use_tool`: handed to `Handler::can_use_tool` on a task of its own.
- `control_cancel_request`: withdraws that permission request (its handler future is dropped).
- `keep_alive`: ignored.
- Anything else is a `Message` for `Handler::message`, in order, on one task: `system`, `assistant`, `user`,
  `stream_event`, `result`, `rate_limit_event`, and any kind this build does not know, kept as `Kind::Unknown` with its
  raw JSON. The errors of a `result` with `is_error` are what `start` fails with when the process ends before it answers
  `initialize` (a session to resume that it does not have).
- A line that is not JSON is written to stderr and skipped.
