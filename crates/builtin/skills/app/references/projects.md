# Projects

A **project** is one product the owner's work is on: its name, who it is for (its customer), his notes on it (what it is, where it runs, what matters), its repositories and the connections of sources that are its own. One Remit works on several products at once; the project is how each chain knows which one it is on. The owner reads them on the **Projects** tab of Work, each with its open chains and its history; nothing there edits one: he tells you.

A project is context. It grants nothing: a chain reads a connection only through the capabilities and the grants it holds, whatever a project lists. It limits nothing either: a chain may open any repository.

## Your tools

1. **Read first.** `list_projects` lists them, archived ones too, each with its id (p_…), revision, name, status, customer and the first line of his notes. `read_project` gives one whole, as `update_project` takes it. Its history is `list_changes` with kind `project` and its id.
2. **Make one.** `create_project` on his words: a name he calls the product by (unique), the customer in his words, his notes (Markdown, at most 4000 characters: what a repository says of itself, its `AGENTS.md` and checks, stays there), its repositories by their remotes (`github.com/owner/name`, `gitlab.example.com/group/project` or a remote URL) or, when he points at code on this machine, by the path of its checkout (`/Users/me/code/shop`, `~/code/shop`), which is read once for its origin and stored as that remote (they need not be cloned yet), and its connections, each `{source, connection}` as `list_sources` shows them (each must exist). Check `list_projects` first so a product is not made twice.
3. **Change one.** `read_project`, then `update_project` with the whole project and the revision you read, changing only what he said. A project changed since you read it is refused: read it again.
4. **Archive one** with `archive_project` when he is done with the product: no Chair is offered it from then on, and the objectives and chains that name it keep it. Nothing is deleted. `restore_project` brings one back when he asks.

## A chain's project

A chain's project is its objective's. An objective he applied carries its own (its Chair sets `project` in `draft_objective`; to move its chains, the objective is amended). A request of his own gets one by whoever reads it first: you, with `create_chain`'s `project` when he named the product or asked from a board narrowed to one (`where_is_owner` names it); else its Chair, in its first turn. A chain started by a routine, an event or another chain has its Chair choose.

`set_project` (a chain's id, a project's id or empty for none, and his why) moves any chain onto a project or off every one, when he says a chain is about another product. It is recorded in the chain's history as yours. A Chair at work reads the project when its session next starts: when it should work from it now, tell it with `message_agent`.
