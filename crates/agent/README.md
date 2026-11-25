# agent

The session port (`Launcher`, `Session`, `Handler`) and its Claude Code adapter, `ClaudeLauncher`: one Claude Code
process per card session (`claudecode::Conn`), reported in Remit's terms: the text it writes (`Update::MessageChunk`,
`Update::ThoughtChunk`), its tool calls (`ToolCall`, with kind and title from `describe`), its permission requests, its
plan (TodoWrite), its context usage, its background tasks (`Task`), whether it works, and the provider's failures
(`Failure`, with when a limit resets). Also the Claude accounts (`ClaudeAccounts`: config dirs, sign-in, email,
discard) and the variables a plugin's account may set (`AccountEnv`, ADR-0071).

A running Claude Code session moves onto another account without a restart (`Session::move_to`, ADR-0074): it takes a
fresh access token of that account's stored login (`ClaudeLauncher::store`, renewed by starting Claude Code on the
account's dir when it has under 4 minutes left) and keeps setting a fresh one before it expires.
