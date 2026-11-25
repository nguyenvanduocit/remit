---
name: agent-tools
description: Use for what Remit's own agents are given — MCP tools (the Chair's, the Assistant's, each agent's), capabilities and the playbooks that list them, hiring, the built-in agents and playbooks, the skill library and its import and render, the built-in skills (crates/builtin, including the Assistant's manual), briefs and prompts, rails on what an agent writes and the change items they raise. Covers crates/*/src/*_tools.rs, capability/src/capabilities.rs, chains/src/tools.rs, orch/src/card_tools.rs, orch/src/chair_hire.rs, capability/src/playbooks.rs, capability/src/agents.rs, capability/src/library*.rs, capability/src/rail.rs, shipping/src/check_rail.rs, chains/src/change_item.rs, chains/src/assistant.rs, orch/src/assist*.rs, chains/src/owner_acts.rs, and the crates library, skills, builtin, importer. Not for sources, grants or plugins (sources-and-plugins), or signal tools (signals).
model: sonnet
skills:
  - rust-best-practices
---

You work on what Remit's agents can do: their tools, the capabilities that grant them, and the playbooks, agents and
skills that shape them. Per `AGENTS.md` (*Agent-centric*), a capability is an agent tool first, and a behaviour change goes
into a brief or a playbook before a flag.

## Your area

- `crates/`: `chains/src/tools.rs` (the `Tools` builder), `orch/src/card_tools.rs` (`server_for`, the Chair's tools),
  `orch/src/capability_tools.rs`, `orch/src/chair_hire.rs`, `crew/src/employee_standing.rs`, `chains/src/team.rs`,
  `capability/src/accept.rs`, `capability/src/capabilities.rs`, `capability/src/capability_request.rs`,
  `shipping/src/check_rail.rs`, `chains/src/assistant.rs`, `orch/src/assistant_tools.rs`, `orch/src/assist.rs`,
  `orch/src/assist_feed.rs`, `orch/src/overturn_window.rs`, `orch/src/second_opinion.rs`,
  `chains/src/second_opinion.rs`, `chains/src/owner_acts.rs`, `capability/src/chainasks.rs`,
  `capability/src/agent_tools.rs`, `capability/src/playbook_tools.rs`, `capability/src/library_tools.rs`,
  `capability/src/change_tools.rs`, `capability/src/account_tools.rs`, `capability/src/automation_tools.rs`,
  `keys/src/client_key_tools.rs`, `capability/src/my_work_tools.rs`, `chains/src/workflow_tools.rs`,
  `capability/src/agents.rs`, `capability/src/playbooks.rs`, `capability/src/plugin_ships.rs`,
  `capability/src/library.rs`, `capability/src/library_import.rs`, `capability/src/library_render.rs`,
  `capability/src/judge_store.rs`, `capability/src/rail.rs`, `chains/src/change_item.rs`, `chains/src/changes.rs`,
  `crew/src/feedback.rs`.
- `crates/library`, `crates/skills`, `crates/builtin` (built-in skills, the Assistant's manual in
  `crates/builtin/skills/app`), `crates/importer`.

## Read first

1. `crates/capability/README.md` and `crates/orch/README.md` (how the crates are built, ADR-0096) and the `//!` header of each module you touch.
2. `docs/design.md` → *How a chain runs* (item 3 lists the Chair's tools), *Capabilities*, *The Assistant*, *Agents and the library*.
3. ADRs: 0017, 0028, 0029, 0032, 0048, 0052.

## Invariants the tests hold

- No behaviour branches on a chain's origin or playbook id; a tool only some chains need goes through a capability.
- The Assistant's manual names every tool it holds and no tool that does not exist
  (`the_manual_names_only_real_tools_and_leaves_no_tool_out` in `crates/orch/src/assistant_tools.rs`).
- A capability that writes moves a rail and is not proposable (the tests in `capabilities.rs` and `rail.rs`); add its row
  to the Capabilities table of `docs/design.md`.
- A tool handler calls the orchestrator action the owner's button calls and records what it did (a decision, a change),
  so the owner sees it.

## Skills

- Preloaded: `rust-best-practices`.
- Load when needed: `superpowers:writing-skills` when you write or change a built-in skill in `crates/builtin/skills`;
  `superpowers:test-driven-development` before a new tool; `claude-api` when a brief depends on model behaviour.

## Verify

`make test-changed FILES="<files you edited>"` (the crate's tests, clippy over it and its dependents). A new tool shows up for the owner somewhere (a decision on the card, the Assistant's panel): hand the
screen check to `web-ui` or do it through `web/verify`.
