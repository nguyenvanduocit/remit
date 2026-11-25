# Codex vs Remit: tools, skills, plugins, hooks

Scope: OpenAI Codex `codex-rs` (Apache-2.0, `references/codex/codex-rs`) against Remit's agent tools, capabilities,
skills, library and plugins. This is a read-only study. Codex paths are relative to `references/codex/codex-rs/`, and Remit
paths are relative to the repository root. Claude Code paths point to the minified 2.1.322 source at
`~/projects/learnSecurity/hackingtool/claude/src-2.1.322/`.

A frame for everything below: Codex runs its own model loop, so most of its tool machinery (tool specs, schema compaction,
code mode, apply-patch, file search) does the work that Claude Code already does for Remit. What carries over is what
Remit controls as an **MCP server and skill host**: the metadata on its tools, when its tool list changes, what
plugins and skills bundle, and the briefs that guide judgement.

Coverage: crates read: tools, codex-mcp (connection_manager, server, mcp/mod), skills (model, metadata, invocation,
lib, the skill-creator sample), hooks (lib, engine/mcp_runner, plugin_trust, declarations, output_spill), plugin
(manifest), core-plugins (marketplace_upgrade), agent-roles, prompts (guardian, review rubric, multi-agent),
collaboration-mode-templates (plan), context-fragments, message-board(+mcp), agent-graph-store, runtime-agent-instance,
config/mcp_types; for the second pass: context-fragments (fragment, annotated_content), guardian-context (budget,
allocation, truncation, elastic), message-board (permissions, host, tools, selection), hooks (events/pre_tool_use,
stop, dispatcher), apply-patch (seek_sequence), connectors (app_info, app_tool_policy), prompts
(incremental_tools). Skimmed by file list only: file-search, plugin-host, core-plugin-common, rmcp-client internals
(OAuth, transports), message-board storage backends. I did not read docs/agents_md.md or docs/slash_commands.md because Claude Code
covers both for Remit.

---

## Ranked ideas

### 1. Give every Remit tool the metadata Claude Code reads (annotations, alwaysLoad, searchHint): **HIGH value, S–M effort**

- **Codex.** A tool declares where it is exposed (`Direct`, `Deferred`, …), its search metadata, and whether it can
  run in parallel: `tools/src/tool_executor.rs:51-71` (`ToolExposure`) and `:112-124` (`exposure`, `search_info`,
  `supports_parallel_tool_calls`). Deferred tools are found through `tool_search` (`tools/src/tool_discovery.rs:6-7`,
  `tools/src/tool_search.rs:13-17`). HIGH.
- **Claude Code** (Remit's runtime) reads these per MCP tool: `annotations.readOnlyHint` sets `isConcurrencySafe`
  and `isReadOnly`, and `_meta["anthropic/alwaysLoad"]` and `anthropic/searchHint` decide whether the tool skips
  deferral and how ToolSearch finds it (`chunk-f62n10eb.js`, `rg -o "anthropic/[a-zA-Z]+"` returns alwaysLoad,
  searchHint, maxResultSizeChars, delivers, requiresUserInteraction). HIGH.
- **Remit today.** `Tools::add` builds `Tool::new(name, description, schema)` with no annotations and no `_meta`
  (`crates/orch/src/tools.rs:57-71`, `:76-87`). Remit's sessions already run with deferred tools: the worker brief
  tells agents to `ToolSearch select:Monitor` (`crates/orch/src/prompts.rs:276`). About 330 `.add("…")` sites exist
  in orch (`rg -U -o '\.add\(\s*"[a-z_]+"'`), and a Chair holds dozens of tools. HIGH that no metadata is sent.
  MEDIUM that Remit's own tools are being deferred today, because that depends on the tool count Claude Code sees.
- **Gap.** Every Remit tool counts as a write, so Claude Code runs read calls (`list_chains`, `read_chain`,
  `read_signal`) one after another instead of in parallel. When ToolSearch is active, the loop's core tools
  (`submit`, `update_plan`, `accept_work`, `hire`) can be deferred, and the brief then names tools whose schemas the
  model does not hold until it searches.
- **Proposal** (`crates/orch/src/tools.rs`). `Tools::add` takes an `Effect` (Remit already has
  `wire::plugin::Effect { Read, … }`, `crates/wire/src/plugin.rs:25-29`) and sets `readOnlyHint`/`destructiveHint` from
  it. A capability's `writes` (`crates/orch/src/capabilities.rs:47-53`) and a plugin tool's `Effect` feed it. Add one
  `Exposure` value (`Core` sends `anthropic/alwaysLoad: true`, `OnDemand` sends a `searchHint`) and set it on the
  handful of tools every turn uses. A fitness test checks that every tool declares an effect. This metadata is advisory
  and never a rail: the rails stay in `rail.rs` and `policy.rs`.
- **Verify.** Count tools in a Chair session's `system/init` before and after. Watch two `read_*` calls run at once in
  the stream.

### 2. Change a live session's tools with `tools/list_changed` instead of reopening it: **HIGH value, M effort**

- **Codex.** When a server sends `list_changed`, Codex relists its tools and refreshes its catalog
  (`codex-mcp/src/connection_manager.rs:280-296`). The behaviour is configurable (`codex-mcp/src/mcp/mod.rs:208-209`).
  HIGH.
- **Claude Code** registers a `notifications/tools/list_changed` handler and refreshes tools when it fires
  (`chunk-nfet4rsf.js:5`, `chunk-d47h4m8y.js:116` "Received tools/list_changed notification, refreshing tools"). HIGH.
- **Remit today.** `CardServer` builds its tools once per connection (`crates/orch/src/tools.rs:259-270`, `server_for`)
  and declares only `enable_tools()` (`:214-222`). A granted capability therefore closes the Chair's session and
  opens it again (`crates/orch/src/capability_request.rs:269` `reopen_chair`). The Chair's brief tells it this
  (`crates/orch/src/prompts.rs:157`). Agents already running "hold them once opened again"
  (`capability_request.rs:56`, `:122`). HIGH.
- **Gap.** A reopen cuts the Chair's turn and rebuilds its context just to add tools, and workers never receive a
  capability while they run.
- **Proposal** (`crates/orch/src/tools.rs`, `capability_request.rs`). `CardServer` keeps its `Tools` behind a swap,
  rebuilt from `tools_for`, and advertises `enable_tool_list_changed()`. It also keeps each session's peer so that when
  `CapabilityCmd` commits, Remit rebuilds the tools and calls `notify_tool_list_changed` for every open session of the
  chain. `reopen_chair` remains only for `shell` and `web`: those are Claude Code launch-time deny rules
  (`crates/orch/src/capabilities.rs:34-37`), not MCP tools.
- **Falsification.** If a stream-json Claude Code session (how Remit drives it) does not list the new tool on its next
  turn after the notification, the idea is refuted for that mode. Test this first. MEDIUM until then.

### 3. External MCP servers as a source, behind the grant gate: **HIGH value, L effort (sources-and-plugins area, needs an ADR)**

- **Codex.** It is a full MCP client: stdio and HTTP transports, OAuth login, per-server tool allow and deny lists, and a
  per-tool approval policy (`config/src/mcp_types.rs:29-35` `AppToolApproval { Auto, Prompt, Writes, Approve }`,
  `config/src/types.rs:1212-1216` `enabled_tools`/`disabled_tools`; `rmcp-client/src/perform_oauth_login.rs`, 1607
  lines). HIGH.
- **Remit today.** A card holds exactly one MCP server, Remit's own (`crates/orch/src/access.rs:118-131`). The
  Assistant's manual has to say "'Bring in my MCP servers' has no answer"
  (`crates/builtin/skills/app/references/playbooks-agents-library.md:19`). Each outside system needs a plugin written
  for it (ADR-0034). HIGH.
- **Gap.** The MCP ecosystem (vendors' official servers) is out of reach, and every new source costs a plugin.
- **Proposal** (`crates/bundled`, `crates/plugin`). Add a bundled `mcp` plugin whose `Definition` connects to one
  external MCP server (rmcp, which Remit already depends on: `crates/orch/src/tools.rs:15-19`) and re-serves its tools
  as Remit tools under a capability. Every call passes the one grant gate and is recorded. A tool's `Effect` comes from
  the upstream `readOnlyHint`, defaulting to write, so writes are not proposable and move a rail. Credentials stay
  sealed in the connection's vars (`Var { secret: true }`). The proxy keeps Remit's invariant that agents hold no MCP
  server but Remit's. Revisit when: per-tool approval policy and OAuth refresh.

### 4. Plugins bundle skills: **MED-HIGH value, M effort**

- **Codex.** A plugin manifest bundles `skills`, an `onboarding_skill`, `mcp_servers` and `hooks` side by side
  (`plugin/src/manifest.rs:19-25`). Discovery reports `has_skills` (`tools/src/tool_discovery.rs:94-102`). HIGH.
- **Remit today.** `wire::plugin::Manifest` carries tools, signals, agents, sensors, processors, subscriptions, ui and
  auth, but no skills (`crates/wire/src/plugin.rs:155-171`). A plugin's `Agent` is only instructions
  (`:73-79`). Built-in skills reach the Assistant and no other card (`crates/builtin/src/lib.rs:27`,
  `for_assistant`). HIGH.
- **Gap.** A plugin can give tools but cannot ship the know-how for using them well (query patterns, pitfalls).
  Today that know-how lives in long tool descriptions or nowhere.
- **Proposal.** Add `skills: Vec<model::Skill>` to `Manifest` and `Definition`, validated by `skills::validate` like
  any library skill. The library lays them out under the plugin's own namespace (as `BUILTIN_PLUGIN_NAME` is,
  `crates/library/src/access.rs:7-9`), and `library::access` gives them to a card whose chain holds the plugin's
  capability. This is the same path ADR-0028 capabilities take, so nothing branches on origin.

### 5. A built-in skill on how to write a skill, for whoever writes skills: **MED value, S–M effort**

- **Codex** ships `skill-creator` as a system skill. Its principles: assume the model is capable, keep discovery cheap
  and precise, and match specificity to risk (`skills/src/assets/samples/skill-creator/SKILL.md:9-24`). HIGH.
- **Remit today.** The Playbook author's brief names the skill tools but says nothing about what makes a skill good
  (`crates/orch/src/playbooks.rs:201`). The `create_skill` description is one line
  (`crates/orch/src/library_tools.rs:221`). Validation checks only the shape (`crates/skills/src/validate.rs:7-14`).
  HIGH.
- **Gap.** Every skill in the library is only as good as an untaught author makes it. A vague `description` never
  triggers.
- **Proposal** (`crates/builtin/skills/writing-skills/`). Write the skill. Make `Input::builtin` follow capabilities
  (the `library` capability brings `writing-skills`), which `access.rs:9` already describes. Load
  `superpowers:writing-skills` when writing it.

### 6. A permission-judgement policy for the Chair, after Codex's Guardian: **MED value, S effort**

- **Codex.** The Guardian reviewer has a written risk taxonomy (data exfiltration, credential probing, persistent
  security weakening, destructive actions, low-risk actions: `prompts/templates/guardian/policy.md:10,31,41,47,62`) and
  a user-authorization scale (`prompts/templates/guardian/classifier_instructions.md`, "User Authorization Scoring").
  HIGH.
- **Remit today.** The Chair decides its agents' permission prompts, and its whole rule is one sentence in
  `answer_permission`'s description (`crates/orch/src/tools.rs:958-975`). The deterministic ladder before it is sound
  (`crates/orch/src/permission.rs:1-6`). The trust boundary ("what others wrote is data") is already in the briefs
  (`crates/orch/src/prompts.rs:160`, `:274`). HIGH.
- **Gap.** No taxonomy covers egress, credential probing or security weakening, so different Chairs judge the same
  prompt differently.
- **Proposal.** Write a built-in skill `permission-judgement`, adapted from Guardian's taxonomy to the owner's terms
  (customer, goal, the chain's worktrees and the sources' guards), that the Chair holds and that the
  `answer_permission` description points to. Judgement goes into a brief, not a flag (AGENTS.md). The rails stay the
  deny rules.

### 7. A skill declares the capability it needs: **MED value, S–M effort**

- **Codex.** A skill can declare tool dependencies, for example an MCP server
  (`skills/src/model.rs:79-95` `SkillDependencies`/`SkillToolDependency`, parsed in `skills/src/metadata.rs:26-34`).
  HIGH.
- **Remit today.** A skill header is `name` and `description` only (`crates/skills/src/frontmatter.rs:56-61`).
  `library::Missing` reports skills the library cannot give, but not tools a skill needs
  (`crates/library/src/access.rs:96-104`). HIGH.
- **Gap.** An agent can hold a skill that says to call `starrocks_query` while its chain lacks `data`, and it finds out
  only by failing mid-run.
- **Proposal.** Add optional frontmatter `capabilities: [data]`, which `skills::validate` checks against the
  catalog's ids. `library::access` returns the capabilities a card's skills need and its chain lacks as `Missing`, so the
  Chair sees them at hire and calls `grant_capability` (non-writing) or `request_capability`.

### 8. MCP server `instructions` carry how a card's tools fit together: **LOW-MED value, S–M effort**

- **Codex** keeps each server's `instructions` and feeds them with the catalog (`codex-mcp/src/connection_manager.rs:294`
  passes `client.server_instructions`). Claude Code places server instructions in the system prompt. HIGH: the
  "MCP Server Instructions" section of a Claude Code session shows it.
- **Remit today.** `get_info` sets none (`crates/orch/src/tools.rs:214-222`), and the briefs enumerate capability tools
  by name (`crates/orch/src/prompts.rs:157`, `:185-190` `data_line`). HIGH.
- **Proposal.** Each capability contributes one paragraph of "how these tools work together", and `CardServer`
  concatenates the paragraphs for the tools it serves. The guidance then has one owner and follows the tool set, which
  matters once #2 makes the set change live. MEDIUM, because it moves text out of tested briefs.

### 9. Bounded tool results, plus structured content: **LOW value, S effort**

- **Codex.** Board tools cap each response at 8 000 bytes and keep it valid JSON (`message-board/src/tools.rs:45-47`).
  JSON results also carry `structuredContent` (`message-board-mcp/README.md:34`).
- **Remit today.** Answers are text, and JSON answers are pretty-printed in full (`crates/orch/src/tools.rs:233-243`,
  `as_json` at `:250`). `list_chains` already pages (`crates/orch/src/capabilities.rs:354-362`).
- **Proposal.** Set `_meta["anthropic/maxResultSizeChars"]` on the few large readers, and page any reader that can grow
  without bound, as `list_chains` does. LOW, because Claude Code already truncates large MCP output.

### 10. Remit as a hook host: **LOW-MED value, M effort (chain-runtime area)**

- **Codex** has a hook engine with 14 events (`hooks/src/lib.rs:29`), plugin-declared hooks
  (`hooks/src/declarations.rs`), a re-verified content hash before a plugin hook runs (`hooks/src/engine/plugin_trust.rs:18-49`),
  and a hook handler that calls an MCP tool (`hooks/src/engine/mcp_runner.rs:29-48`). HIGH.
- **Remit today.** `crates/claudehooks` types Claude Code's hook wire format, but only `examples/guard.rs` uses it
  (`rg -l "claudehooks::"`). Conventions such as "report before ending" live only in briefs. HIGH.
- **Proposal.** Register Claude Code HTTP hooks that point at Remit's door: `Stop` checks the card's plan and report
  deterministically, and `PostToolUse` records tool activity. Some brief rules then become code. LOW confidence that
  the need is real; check first which brief rules the judges most often catch broken.

### 11. Import skills from a git source with a pinned revision, and update them: **LOW-MED value, M effort**

- **Codex** adds marketplaces from git, pins them by revision and upgrades them (`core-plugins/src/marketplace_upgrade.rs:1-35`). HIGH.
- **Remit today.** The importer reads only this machine's Claude Code config (`crates/importer/src/lib.rs:1-3`,
  `SOURCE_KIND = "claude-code"`) and keeps no upstream to update from (`crates/orch/src/library_import.rs:1-4`). HIGH.
- **Proposal.** `import_skill` takes a git URL and path and records `Source { kind: git, rev }`. An `update_skill` diffs
  upstream against the stored skill and raises the usual change item, since a skill's text moves a rail.

### 12. Count skill use that bypasses the Skill tool: **LOW value, S effort**

- **Codex** detects a skill read through `cat SKILL.md` or a skill's script run from a shell command
  (`skills/src/invocation.rs:26-43`).
- **Remit** counts only `Skill` tool calls (`crates/outcomes/src/usage.rs:41-44`). An agent that `Read`s a laid-out
  skill file is not counted. Match paths under the library's laid-out dir in `Read` and `Bash` calls.

---

## Not worth taking (Claude Code or Remit already covers them)

- apply-patch, file-search, code mode, JSON-schema compaction (`tools/src/json_schema/compaction.rs:14-29`): Claude
  Code owns editing and search, and Remit already strips `title`, `format` and `default` from its schemas
  (`crates/orch/src/client_tools.rs:222-241`). HIGH.
- Plan collaboration mode (`collaboration-mode-templates/templates/plan.md`): Remit splits planning between the Chair
  and `update_plan`/`need_plan` (`crates/orch/src/tools.rs` `add_plan_tool`). MEDIUM.
- Side conversations (`context-fragments/src/side_conversation.rs`): Remit's Assistant panel and the agent's own screen
  cover this. MEDIUM.
- runtime-agent-instance and connectors: these are OpenAI account and app plumbing. HIGH.

## Where Remit is already better

- **Capability rails with an owner-visible record.** A capability that writes is not proposable and waits on the owner's
  Apply as a change item. A non-writing one is granted by the Chair with a reason the owner reads
  (`crates/orch/src/capabilities.rs:1-7`, `crates/orch/src/capability_request.rs:1-5`). Codex's
  `request_plugin_install` is an elicitation form with `persist: "always"` and no audit entity
  (`tools/src/request_plugin_install.rs:11-13,53-72`). HIGH.
- **Typed, closed tool inputs.** `#[serde(deny_unknown_fields)]` structs whose schemars schema is the contract
  (`crates/orch/src/tools.rs:1-5`), against Codex's sanitizer for arbitrary schemas (`tools/src/json_schema.rs`, 429
  lines). HIGH.
- **Scoped seats.** A subagent's server refuses tools it was not served and names whose they are
  (`crates/orch/src/tools.rs:203-209`). HIGH.
- **Library hygiene.** Size limits, Claude Code name-canonicalisation clash detection, a change history with
  before/after diffs, and per-skill usage tallies (`crates/skills/src/validate.rs:7-14`,
  `crates/library/src/access.rs:40-53`, `crates/library/src/diff.rs`, `crates/outcomes/src/usage.rs:1-20`). Codex has
  no equivalent of the change history. MEDIUM, from the files read.
- **Permission ladder.** Deny rules, then a secret check, then the Chair, then the owner, with a time limit and the
  decider recorded (`crates/orch/src/permission.rs:1-6`). Codex's Guardian is a richer judge but not a richer
  structure. HIGH.
- **Reviewer brief.** It starts from the goal, searches consumers, checks tests sceptically and applies the owner's
  principles (`crates/orch/src/gate_instructions.rs:6-30`). That goes further than Codex's generic review rubric
  (`prompts/templates/review/rubric.md`). One thing to borrow: the rubric's P0–P3 priority tag. MEDIUM.
- **Work graph.** Cards, edges, chains and decisions as entities, against Codex's parent/child spawn edge with
  open/closed status (`agent-graph-store/src/types.rs:4-12`). HIGH.
- **Plugin breadth.** Remit plugins ship signals, sensors, processors, subscriptions, a UI, auth and Claude accounts
  (`crates/wire/src/plugin.rs:155-171`). Codex plugins ship skills, MCP servers, hooks and apps. Each side lacks what
  the other has; #3 and #4 close Remit's half.

## Codex does better / Remit lacks (read on Codex's own terms)

This section walks Codex's crates on their own terms and asks one question: is their engineering better? It is not
limited to ideas that fit a slot Remit already has. Items already ranked above (#1 tool exposure, #2 live tool list,
#3 MCP client, #4 plugin skills, #6 Guardian policy) are not repeated here.

### A. Edit records by context, not by whole replacement: **MED-HIGH value, S–M effort**

- **Codex.** apply-patch edits a file by quoting the lines around the change. Matching relaxes step by step (exact,
  then trailing whitespace, then both ends), and a patch whose context is gone fails loudly instead of overwriting
  (`apply-patch/src/seek_sequence.rs:1-30`, grammar in `apply-patch/src/parser.rs`). The failure is an optimistic
  concurrency check for free. HIGH.
- **Remit.** `update_playbook`, `update_agent_definition` and `update_skill` replace the whole record. The Playbook
  author's brief has to warn "an update replaces the whole playbook, so start from what read_playbook returned and keep
  every other field" (`crates/orch/src/playbooks.rs:201`). The skill test reads the skill, edits it in JSON and writes
  every file back (`crates/orch/src/library_tools.rs:443-454`). `rev` exists on every record (`crates/model/src/library.rs:76-77`)
  but no write tool checks it. HIGH.
- **Why it matters.** The model re-types up to 1 MiB of skill files (`crates/skills/src/validate.rs:9-13`) to change
  one line. Every re-type can drift, and an owner's edit made between the read and the write is silently lost.
- **Proposal** (`crates/orch/src/library_tools.rs`, `playbook_tools.rs`, `agent_tools.rs`). Add `edit_skill_file
  {name, path, old, new}` with Claude Code `Edit` semantics: `old` must occur exactly once, or the call is refused with
  the reason. Playbook and agent updates take the fields to change plus the `rev` they read. A stale `rev` is refused
  and names the newer one. Whole replacement remains for `create_*`.

### B. Cut long text at both ends and say how much was cut: **MED value, S effort**

- **Codex.** Guardian truncation keeps a prefix and a suffix and binary-searches the cut to fit the budget
  (`guardian-context/src/truncation.rs:13-30`). It leaves `<truncated omitted_approx_tokens="…" />`
  (`:55-57`), and the reader's instructions say a marker means omitted data that must not be assumed benign
  (`prompts/templates/guardian/classifier_instructions.md`, Evidence Handling). HIGH.
- **Remit.** `inputs_text` keeps the first 8 000 bytes of a report and appends `…` (`crates/orch/src/prompts.rs:220-228`,
  test `:586-597`). `EVENT_BODY_MAX` also keeps "the start of a longer one" (`crates/orch/src/event.rs:25-26`). HIGH.
- **Why it matters.** A report ends with its conclusion and verification, and the head-only cut drops exactly that. The
  marker does not say how much is missing or how to read the rest.
- **Proposal.** Write one `cut(text, max)` in orch that keeps head and tail and puts a marker in the middle naming the
  bytes omitted and the tool that reads the whole (for example `read_card <id>`). Both call sites use it.

### C. Typed context fragments with provenance, not prose prefixes: **MED value, M effort**

- **Codex.** Every piece of context the harness injects is a `ContextualUserFragment`. It has a role, a stable
  `<feature>.<name>` kind and attribution metadata "supplied when the fragment is produced, never inferred from
  rendered text", plus markers that recognise injected context later (`context-fragments/src/fragment.rs:45-90`).
  Content, kind and attribution survive edits and truncation together (`context-fragments/src/annotated_content.rs:10-16`).
  HIGH.
- **Remit.** Who wrote a message is a prose prefix the model is taught to read: "[Remit:", "[event]", "[client key",
  "[message from the owner]" (`crates/orch/src/prompts.rs:160`). Bracketed `"[…` literals are built in 39 orch files
  (`rg -l '"\[[a-z]'`). Two separate fence helpers exist (`crates/orch/src/event.rs:31-38` `fenced`,
  `crates/orch/src/threads.rs:619-630` `fence_for`), and workers' reports in `inputs_text` are not fenced at all
  (`prompts.rs:229-231`). HIGH for the facts. MEDIUM that a report could pass for a Remit message in practice.
- **Proposal** (`crates/orch/src/`, chain-runtime area). One `Fragment { kind, from, body }` type renders every
  message Remit puts into a session. The header comes from `kind` and `from`, and the body is always fenced by one
  helper, so no payload can carry a header of its own. The two fence helpers become one. The trust rule in the brief
  then names fragment kinds, not string prefixes.

### D. A message board between agents (preview pushed, content pulled): **MED value, M–L effort, LOW confidence on need**

- **Codex.** Agents post to channels and threads and subscribe to them. A post notifies subscribers with a bounded
  preview, and the full content is fetched through bounded reads (`message-board/src/host.rs:15-37`). `Append` writes
  evidence without notifying anyone (`message-board/src/tools/selection.rs:5-21`). The backend returns *effective*
  permissions to the caller (`message-board/src/permissions.rs:11-26`), and a denial is a structured answer bounded
  under 1k tokens (`:56-60`). Every tool response is capped at 8 000 bytes and stays valid JSON
  (`message-board/src/tools.rs:45-47`). HIGH.
- **Remit.** Messages go up and down only: workers have `ask_chair`, the Chair sends down, and the Assistant has
  `message_agent` (`rg '\.add\(\s*"(ask|message)…'`). Between chains, the only link is `overlap_note`, which lists other
  chains' goals (`crates/orch/src/prompts.rs:301-319`). Hub-and-spoke through the Chair is deliberate. HIGH.
- **What is better there.** Peer discovery: chains on the same system can share a finding ("staging DB is read-only")
  without the owner relaying it. And preview-then-pull keeps a long message from flooding a session.
- **Proposal.** Do not build a board beside signals. A chain's finding becomes an event signal that other Chairs
  `subscribe` to, which reuses Remit's own pipeline. Separately, adopt preview-then-pull for long messages queued to a
  session (`outbox.rs`). Revisit when chains are seen redoing each other's discoveries.

### E. Hooks as a lifecycle a plugin contributes, with every run recorded: **MED value, M effort (extends #10)**

- **Codex** implements Claude Code's hook protocol as its own engine. `PreToolUse` can block, add context or rewrite
  the input (`hooks/src/events/pre_tool_use.rs:43-49`). Each run emits `HookRunSummary`/`HookCompletedEvent`
  (`hooks/src/engine/dispatcher.rs:8-13`), so the user sees what a hook did. Plugins declare hooks
  (`hooks/src/declarations.rs:5-31`), and a plugin hook's content hash is re-verified before every run, so a changed
  bundle needs approval again (`hooks/src/engine/plugin_trust.rs:18-49`). Oversized hook output spills to a file under
  a 2 500-token limit (`hooks/src/output_spill.rs:12-13`). HIGH.
- **Remit.** `crates/claudehooks` types the wire format, but only an example uses it. A plugin `Manifest` has no hooks
  (`crates/wire/src/plugin.rs:155-171`). HIGH.
- **Proposal.** If #10 lands, hooks are a plugin contribution (`Manifest.hooks`). Each run is recorded on the card the
  way a decision is, so the owner sees it. An applied extension's hooks are pinned to the content hash the owner
  applied.

### F. A non-blocking risk scorer that turns blocking review on: **MED value, L effort, LOW confidence**

- **Codex.** The Guardian classifier is "a first-step asynchronous security reviewer. Your decisions are non-blocking,
  but we will run a blocking reviewer on future actions based on your risk level", and it scores both the action and
  the *trajectory* (`prompts/templates/guardian/classifier_instructions.md:1-6`). HIGH.
- **Remit.** Review is either blocking (the permission ladder, `crates/orch/src/permission.rs:1-6`) or none. The Chair
  sees only what Claude Code asks about. HIGH.
- **Proposal.** Score a chain's tool stream (already recorded for usage, `crates/outcomes/src/usage.rs`) after the
  fact. A high score moves that chain onto stricter ask rules (`policy::ask_rules`) and records why, so the owner sees
  it. This is a rail change and needs an ADR.

### G. A role can set reasoning effort: **LOW-MED value, S effort**

- **Codex.** An agent role is a full config layer (`agent-roles/src/agent_role_config.rs:20-28` flattens `ConfigToml`),
  including `model_reasoning_effort` (`config/src/config_toml.rs:402`). HIGH.
- **Remit.** An agent definition has `model` and no effort (`crates/model/src/library.rs:48-78`), yet the runtime
  already carries one (`crates/agent/src/agent.rs:161`, `crates/claudecode/src/options.rs:101`). ADR-0070's effort is
  "not built yet" (`crates/orch/src/workflow_run.rs:243`). HIGH.
- **Proposal.** Add `effort` to `model::Agent` beside `model`, passed through `library::Session` to `agent::Spec`.
  A reviewer or goal judge can then run at high effort and a summariser at low.

### H. Typed tool results: output schema plus structured content from one type: **LOW-MED value, S–M effort**

- **Codex** carries a tool's output schema next to its input schema (`tools/src/output_schema.rs:7-20`), and its MCP
  adapter returns JSON text with matching `structuredContent` (`message-board-mcp/README.md:34`). HIGH.
- **Remit** answers text, and JSON answers are a pretty-printed string (`crates/orch/src/tools.rs:233-243`, `:250`).
- **Proposal.** `Tools::add_typed<In, Out: Serialize + JsonSchema>` derives `outputSchema` from `Out` and returns
  `structuredContent`. Tests then assert on the typed result instead of on substrings, and the result contract has
  one owner, the Rust type.

### I. Replay safety on MCP writes: **MED value, S–M effort, LOW confidence on occurrence**

- **Codex.** The board MCP adapter keeps request deduplication per session, so a reused JSON-RPC id does not post
  twice (`message-board-mcp/README.md:30-32`). HIGH.
- **Remit.** `call_tool` ignores the request context (`crates/orch/src/tools.rs:233-237`, `_: RequestContext`). A
  write tool (`hire`, `submit_mr`, `ask_owner`) replayed after an HTTP reconnect would act twice. AGENTS.md
  (*Server-driven*) asks that "a call replayed after a reconnect does not act twice". HIGH that no dedup exists. LOW
  that Claude Code replays tool calls.
- **Proposal.** Add a falsification test first: drop the MCP connection in the middle of a call and watch whether
  Claude Code resends. If it does, key write tools by (session, request id) in `CardServer`.

### Codex engineering judged and not worth taking

- **apply-patch's own grammar** (`apply-patch/src/parser.rs`): Claude Code's `Edit`/`Write` is the editing surface of
  Remit's agents, so A takes only the idea (edit by quoted context and fail loudly). HIGH.
- **Context budgeting for a judge** (`guardian-context/src/allocation.rs`, `budget.rs`, 1 100 lines): it pushes a
  budgeted transcript to the reviewer. Remit's judges pull evidence with tools (`read_check`, `read_chain`), which
  scales with model capability and needs no allocator. That fits "we build for the future" (AGENTS.md). MEDIUM.
- **connectors** (`connectors/src/app_tool_policy.rs:14-60`): the per-tool policy from `destructive_hint` and
  `open_world_hint` is the part worth taking, and it is already in #3. The rest is ChatGPT app directory plumbing. HIGH.
- **agent-graph-store**: one parent/child spawn edge with open or closed status (`agent-graph-store/src/types.rs:4-12`).
  Remit's cards, edges and chains already go further. HIGH.

## Suggested order

1 and 2 go together: one change to `tools.rs` covering tool metadata and a live tool set, in the agent-tools area. H
and I fit the same change if wanted. Next come A and B: small, and they fix silent loss in what agents write and read.
Then 5 and 6, two built-in skills that are cheap and raise judgement quality. Then 4 and 7, which connect skills to
plugins and capabilities, and G. C, E and F touch rails or the runtime: ADRs first (`architect` with `chain-runtime` or
`rails-and-sandbox`). 3 needs an ADR from `architect` and `sources-and-plugins`.
