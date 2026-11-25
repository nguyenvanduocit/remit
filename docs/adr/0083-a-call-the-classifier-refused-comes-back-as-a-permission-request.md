# 0083. A call the auto-mode classifier refused comes back as a permission request

Status: accepted, 2026-11-12; its waits on the owner, but a secret act, are an agent's (ADR-0087). Extends ADR-0059 (every permission prompt goes up one ladder).

## Context

Every session runs Claude Code in auto mode (`--mode`, default `auto`). There a server-side classifier answers each call no
rule decides: allow, no verdict (Claude Code asks over `can_use_tool`, and the ladder of ADR-0059 decides), or block. A
block is final: the model reads "Permission for this action was denied by the Claude Code auto mode classifier" and no
one is asked. A card of chain `ch_f5bf8d7fcffa` lost a `cargo check` run that way (`[Safety Bypass Flag]`, a Bash call with
`dangerouslyDisableSandbox`), and neither its Chair nor the owner could allow it. The owner asked that the Chair decide
such calls as it decides a permission request.

What Claude Code 2.1.322 offers, read in its source and proved against the real binary on 2026-11-12:

- A `PermissionDenied` hook runs after a block, with the tool, its input and the classifier's reason. An SDK host
  registers it in `initialize` (`hooks: {Event: [{hookCallbackIds}]}`) and answers `hook_callback` control requests. Its
  answer `retry: true` tells the model it may retry; the model sent the same input again at once.
- In auto mode a `PreToolUse` hook's `allow` is ignored (`U3t`: `case "allow": break`) and its `ask` still meets the
  classifier: the retried call was blocked again both ways.
- An exact allow rule passes the classifier (only `Bash`, `*` and an interpreter with a wildcard count as dangerous rules),
  and `apply_flag_settings` added one to a live session; but that request writes the `flagSettings` layer Remit's own
  `--settings` fills, and whether it merges or replaces it is unproven.
- `set_permission_mode default` on the live process sends the retried call to `can_use_tool`
  (`decision_reason_type: sandboxOverride`); Claude Code answers the mode request while the hook waits.

## Options

1. **Leave auto mode for the retry (chosen).** On `PermissionDenied` the session sets mode `default`, then answers
   `retry: true`; the retried call is asked over `can_use_tool` and goes up the existing ladder: deny rules, the secret
   refusal, the Chair, the owner, the time limit. The session goes back to its mode at the first permission request after
   that, before it is answered, or when the turn ends.
2. **A live allow rule.** The hook asks the Chair itself, adds an exact rule with `apply_flag_settings`, answers
   `retry: true`, and removes the rule after. A second decision path beside the ladder, a rule that outlives its one call,
   and a write over the layer that holds Remit's own settings.
3. **Sessions in `default` mode.** Every call no rule allows is asked: the Chair answers what the classifier answers now,
   for every read and edit.
4. **Only show the block.** The owner sees it, nobody can allow it: a retry meets the classifier again.

## Decision

- `claudecode` answers hooks: `Options::hooks` lists the events registered at `initialize`, each with one callback named
  after the event, and `Handler::hook` answers `hook_callback` on a task of its own, ordered and withdrawn as
  `can_use_tool` is. Its types are `claudehooks`'.
- Every session registers `PermissionDenied`. `crates/agent/src/refused.rs` holds a turn's refusals: at most one open,
  each call sent back once a turn, so a call refused again after it came back stays refused and the two never loop. The
  hook leaves the mode the hook reports (`permission_mode`) for `default` and answers `retry: true` only once the mode is
  set; a session that cannot change mode answers no retry.
- The retried call's request carries the classifier's reason as its own field (`agent::PermissionRequest::classifier`),
  and its inbox item says it in one sentence: "Claude Code's safety check stopped it (`<reason>`), so it runs only if it
  is allowed." The ladder shows every request's reason, first, to the Chair and in the owner's inbox item, beside the
  call's action, traits and risk (`orch/src/permission_ask.rs`).
- Calls other than the refused one made while the session waits in `default` are asked too: stricter than auto mode,
  never looser.

## Consequences

- The Chair, or the owner, can allow a call Claude Code's classifier refused, with the reason in front of them; the deny
  rules and the secret refusal still come first.
- A Chair may now overrule Anthropic's classifier. That is the authority ADR-0059 gives it over its agents' work.
- A subagent's call the classifier refuses comes back the same way; its request goes up its parent's ladder.
- If the model enters plan mode between the refusal and the retry, going back sets the mode the hook reported and ends
  plan mode.

## Revisit when

- A Chair allows refused calls the owner would not: a control on which refusal reasons reach the Chair and which go to
  the owner only (the seam is the reason carried on the request).
- Claude Code honours a hook's `allow` in auto mode, or offers a per-call override: the mode switch goes.
- A refused call should be retried in a later turn: the refusals are forgotten at each turn's end.
