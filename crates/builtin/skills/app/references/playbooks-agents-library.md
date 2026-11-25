# Playbooks, agents and the library

He changes all of these by telling you; you do not write them. Start an author chain with his words, as they are: `create_chain` with kind `author`, the intent naming what is open on his screen when he points at it (`where_is_owner`). Its Chair reads and writes playbooks and agents, writes skills, and imports them from this machine's Claude Code configuration. He can watch it. When it finishes, read what changed (below) and tell him in a sentence.

## What exists (read, never write)

- `list_playbooks`, `read_playbook`: playbooks, each a brief and the agents it suggests. A playbook is reference: it gives a chain no tool, no Chair and no rule. Every Chair can read them and takes what helps; a starter who wants one followed names it in the goal ("use the bug-sweep playbook").
- `self-development` describes a change in Remit's own repository: the agents it suggests are told they edit the system that runs them and which documents to read first, and the chain finishes at a change approved by the reviewer, QC and, when it touches a path the repository's `.remit/rails` names, the Security reviewer, whose threat model the change carries; Remit lands it once approved. A chain that proposes may name it in a goal. It never restarts anything.
- No chain releases: what lands on a repository is released by Remit's code, by the procedure the repository's `.remit/release/` declares (see SKILL.md). A chain writes that procedure in its change like any file; it is a rail, so its change needs the Security reviewer's verdict. The capability `feedback` gives a chain the feedback tool, read only.
- `list_agent_definitions`, `read_agent_definition`: the agents he can have hired. Each has instructions (what every session of it starts with, added to its system prompt; not the brief a Chair gives it for a task), a default model, the Claude Code tools it may use (every tool, none, or a list), the library skills it holds, and the gate it signs (`reviewer`, `qc` and `security` are built in and sign the review, QC and security gates; security is needed on a change that touches a rail).
- `list_library`, `read_skill`: the skills agents can hold. The tools agents read the outside world through are not in the library: see `tools-and-connections.md`.
- `list_importable`: what this machine's Claude Code configuration holds that the library can import, and why an entry cannot be imported.
- `list_changes`, `read_change`: what was written to one agent, skill or playbook, by whom and when, and the record before and after. Use them when he asks why something is as it is, or wants it back as it was: an author chain writes the old record back.

## How they fit

- A hire holds a copy of what its definition names, taken when the Chair hires it; the Chair may give it other tools or skills of the library instead. What he changes in a definition therefore reaches the agents hired afterwards and no chain already at work: tell him so when it matters, and that a Chair can hire anew. A skill reaches an agent only when the library has it and the agent holds it; a role with no definition is a plain agent: every Claude Code tool, no skill. Sessions read no skill of the machine's own Claude Code configuration unless it was imported, and hold no MCP server but Remit's own.
- The library starts empty. "Bring in my skills" is work for an author chain: tell it what he named, or that he wants all of what `list_importable` shows.
- "Bring in my MCP servers" is yours, not the library's: `list_importable_mcp_servers` shows the ones this machine's Claude Code has, and `propose_mcp_server` raises one change item per server he wants. An applied server is a source, its tools reached through grants like any other; a session never holds the server itself. See `tools-and-connections.md`. A skill that tells an agent to run a command with a token cannot be followed.

## What waits for him

A write that moves a rail waits in the inbox for you to decide it for him: an agent definition's instructions, a skill's text, a Chair's request for a grant (see the inbox reference). An import by the author waits the same way. A playbook write moves no rail and takes effect at once. The author chain tells you what waits when it finishes; decide each item (`answer_inbox`) and say, in a sentence, what you applied. Everything else it writes takes effect at once.

## What you say

Say which agent or skill changed, and what is left for him: an agent still to be given a skill. Do not paraphrase his words into a different request when you hand them over.
