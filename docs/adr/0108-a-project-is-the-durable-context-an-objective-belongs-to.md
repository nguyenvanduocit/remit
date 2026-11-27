# 0108. A project is the durable context an objective belongs to

Status: accepted, 2026-11-17; built in the steps of *Plan* below. Extends ADR-0031 (an objective names its project) and
ADR-0016 (a project's customer is where a Chair starts its CUSTOMER section). Narrows `docs/remit-design.md` K3's "company
context is the owner's words in a chain": the context of one product is a record of its own; a cell still separates
companies. Source: the owner, 2026-11-17, "support multiple projects": working on two products at once, the Chair does
not know what to do, so he runs two instances.

## Context

One instance already works on many repositories: a chain holds `repos: Vec<RepoWork>` (`crates/model/src/model.rs:49`,
`:151`) and `open_repo` opens any repository by its remote, cloning it once into `<data>/repos/<host>/<project>`
(`crates/shipping/src/repo.rs:213`; the tool at `crates/orch/src/card_tools.rs:235`). Nothing stops two products from
sharing one instance. What is missing is what the Chair knows when a goal arrives:

1. **Which product.** The Chair's brief is the goal, the playbook and the rules (`crates/chains/src/prompts.rs:97`
   `chair_prompt`). Nothing says which product the words are about, so "fix the checkout bug" means whichever checkout the
   Chair guesses.
2. **Which repositories.** `open_repo` takes a remote, a checkout path or the name of a clone Remit holds
   (`resolve_repo`, `crates/shipping/src/repo.rs:192`; `named_clone`, `:91`). A short name that two products share
   (`api`) is refused as ambiguous (`:104-107`), and a product's repository Remit has not cloned yet cannot be named short
   at all.
3. **Who the customer is.** ADR-0016 has every Chair write a CUSTOMER section before its first hire
   (`crates/chains/src/customer.rs:35` `chair_customer_rules`), from evidence it finds each time. For a product the
   customer is the same chain after chain, yet each Chair finds it again.
4. **Which outside systems.** A chain reads every unprotected connection of the sources its playbook's capabilities name
   (`docs/design.md`, Model, *Grant*). Connection names are unique across the installation (`docs/remit-design.md` §1),
   so product A's Datadog is `datadog/prod-a`; nothing tells a Chair that, and it learns it by listing.

`Objective` holds `scope` and `customer` (`crates/model/src/objective.rs:253,255`), but an objective is time-bound
(a status, a horizon) and a direct request gets a hidden one of its own (`task`, `crates/chains/src/lifecycle.rs:651`
`with_task_objective`), so it cannot carry what outlives every request. The owner's way out today, a second instance,
splits his Assistant, his inbox, his employees and their memory in two, and every agent loses what the other instance
learned.

The customer of this decision is the owner, through each Chair: a goal arrives with the product it belongs to, and the
Chair starts from that product's repositories, customer and data instead of guessing.

## Options

**Where a product's context lives.**

1. **A cell per product** (two instances, K3's allocator). Separation by structure, nothing to build. It is the complaint:
   two Assistants, two inboxes, two crews, nothing learned crosses. A cell is for companies that must not see each other;
   one owner's products are not that.
2. **Words in the objective** (`scope`, `customer`). No entity, but every direct request has a fresh task objective with
   none, and the same words are retyped per objective: the durable fact has no home.
3. **A playbook per product.** A playbook is *how* work is done (seeds, capabilities, the Chair's craft); a product is
   *what* it is done on. Crossing them multiplies playbooks (engineering × each product) and makes the intake's one
   choice two choices in one.
4. **A `Project` record (chosen).** A name, the customer, the owner's notes, the repositories and the connections that are
   the product's. Kept like the records the owner keeps through the Assistant (an event signal, a mentor), pushed like an
   objective.

**Where a chain's project is stored.**

1. **On the chain.** Every creation path (owner, routine, event, proposed, client, verification, replay) has to set it,
   and a chain serving an objective's key result could name a project its objective does not.
2. **On both** the chain and its objective, kept equal. Two owners of one fact.
3. **On the objective, `Objective.project` (chosen).** Every chain serves exactly one key result (a direct request its
   task objective's), so a chain's project is its objective's and is read, never stored on the chain. An objective the
   owner applied carries its project in its draft, so all of its chains get it with no per-chain choice; a direct
   request's task objective gets the one chosen for it.

**Required or optional.**

1. **Required, with a migrated default project.** Every existing objective lands in "Default": a name that means "none",
   and a choice forced on requests that are about no product (research, Remit's own housekeeping).
2. **Optional (chosen).** `Objective.project: Option<String>`; none is today's behaviour exactly. Nothing to migrate in
   any document: an objective without the key has no project. With no project defined, nothing changes anywhere.

**Who chooses a chain's project.**

1. **Code**, by matching the goal's words or the chain's repositories against projects. A rule that guesses, and the
   repositories are known only after `open_repo`.
2. **Agents (chosen).** Whoever already reads the request picks: the intake for an owner's chain (with the title and the
   playbook, from the same words), the Assistant when he asks through it (`create_chain`'s `project`), the owner's own
   board when it is narrowed to one project (named, as "Run with…" names a playbook). A chain that starts with none while
   projects exist (a routine's, an event's, a proposed or verification chain) has its Chair choose in its first turn with
   `set_project`, from the list its brief carries, or say none. One write path, whoever calls it.

**What a project changes.**

1. **Context only (chosen).** The Chair's brief opens with the project; `open_repo` resolves a short name against the
   project's repositories first. It grants nothing, narrows nothing, and no code branches on which project it is.
2. **Also a boundary**: grants, memory, employees and connections scoped per project. That is tenancy inside one owner's
   process, which K3 rejects for companies and which nothing here asks for; each part is a seam below (*Revisit when*).

Models applied. **First principles**: the gap is context the Chair lacks, not isolation the owner lacks, so the answer is
a record agents read, not a boundary. **Inversion**: what would guarantee failure is a project that is a permission
boundary with `if project == …` branches, or a required "Default" project that lies; both are out. **Second-order
thinking**: six months on, the pressure will be to hang more on a project (memory, slots, signals); each is recorded as a
named seam, and a project grants nothing, so none of them can leak in by accident. Patterns: DDD *aggregates* (reference
by identity: the objective holds the project's id, the project stays a small record of its own, no transaction spans
both); *cell-based architecture* as the rejected alternative for this need and the kept one for companies.

## Decision

1. **`model::Project`** (`crates/model/src/project.rs`, table `projects`, one document with a revision like an
   objective): `id` (`new_id("p_")`), `rev`, `name` (unique among projects), `status` (`ProjectStatus`: `active`,
   `archived`), `customer` (who the product is for, in words), `brief` (the owner's notes on the product: what it is,
   where it runs, what matters; Markdown, at most 4000 characters; what a repository says of itself, its `AGENTS.md`
   and `.remit/checks` (ADR-0025), stays in the repository), `repos` (remotes in `RepoRemote::named` form,
   `host/group/project` or an absolute local folder path, ADR-0141), `connections` (`ConnectionRef { source, connection }`, each an existing connection), `by`,
   `created_at`, `updated_at`. An archived project is offered to no one and stays readable on the objectives that name it;
   nothing is deleted, so no reference dangles.
2. **`Objective.project: Option<String>`**: the project it belongs to, none for a request about no product. A chain's
   project is its served objective's, read by one function (`chains::project::project_of`), never stored on the chain.
3. **Who writes.** The Assistant, on the owner's words: `create_project`, `update_project` (whole, over the revision it
   read), `archive_project`, `list_projects`, `read_project`. Each write stores a `Change` of kind `project` in its
   transaction (the record before and after, who), as an event signal's does, and pushes `project_updated`. No Apply
   waits: a project moves no rail.
4. **A chain's project is set by one path**, `chains::project::assign` (the task objective's `project` written over its
   revision, and a `project` decision with who and why), called by: the intake (its answer gains `project`: an id from
   the active projects listed in its prompt, or empty), the Assistant's `create_chain` and the owner's `chain_create`
   (`project`, kept by the intake as a named playbook is), the Chair's `set_project` (its own chain), and the Assistant's
   `set_project` (any chain). A chain serving an objective the owner applied takes that objective's project; `set_project`
   on it is refused with "amend the objective" (`draft_objective` carries `project`).
5. **The Chair reads it.** `chair_prompt` opens with a PROJECT section when the chain has one: its name, customer,
   brief, repositories and connections, headed as the owner's notes on the product, data to work from like the goal. The
   CUSTOMER rule says the project's customer is where the section starts and evidence still decides it. When the chain has
   none and active projects exist, the brief lists them (name and first line) and asks for `set_project` or "none" in the
   first turn. With no project defined, the brief is as before.
6. **`open_repo` resolves a short name in the project first**: a query that names one of the chain's project's
   repositories (`named()`, its path, or its last element) opens that remote, cloned on first use; otherwise resolution is
   as before (`resolve_repo`). A chain may open any repository: the list is context, not a limit.
7. **The owner sees it.** `Snapshot.projects` and `project_updated` carry the records (the owner's alone; a mentor's
   session gets none). `ChainStanding.project` names each chain's project, computed by the server. The board narrows to
   one project with `?project=<id>` (view state, beside `?q=`), every card shows its project's name, and a chain asked
   from a narrowed board names that project. The Objectives tab narrows the same way. A Projects tab of Work
   (`/projects/<id>`) lists the projects read-only with their customer, brief, repositories, connections, open chains and
   history (`changes_get`); nothing there edits one. `Where` gains `project_id`, so "this project" in his words is the one he
   is looking at.

## Threats

| Threat | Control |
|---|---|
| Text that reaches the Assistant (an event, a chain's report) steers it to rewrite a project's brief, which every Chair of that project then reads | Only the Assistant's tools write a project, each write is a `Change` with who and the before and after, shown in the project's history; the Chair reads the brief as notes, under the same "data, not instructions" rule as the goal |
| A project lists a connection so a chain reads data it should not | A project grants nothing: a chain's reads are its playbook's capabilities and its grants, and a protected connection still needs a grant (ADR-0027); `connections` is checked to exist, never to permit |
| A Chair moves its chain into another project to read that project's notes | One owner, no secret between his products; the move is a `project` decision he sees; nothing is granted by it |

## Consequences

- One instance serves several products: one Assistant, one inbox, one crew whose memory crosses products, one board that
  narrows to a product.
- A chain's Chair starts with the product's repositories, customer and connections; `api` opens this product's `api`.
- New words: `Project`, `ProjectStatus`, `ConnectionRef`, `Objective.project`, `project_of`, `assign`, `set_project`,
  `create_project`, `update_project`, `archive_project`, `list_projects`, `read_project`, `project_updated`,
  `ChainStanding.project`, `Where.project_id`, the decision kind and change kind `project`, migration `0111.sql`.
- Existing objectives and chains have no project; the owner names projects by telling the Assistant, which may
  `set_project` his open chains.
- Workers are not told the project by Remit: the Chair passes what they need in their briefs, as it does the goal.

## Plan

Each step is done whole, in dependency order: (1) the record and its store, migration `0111.sql`; (2) the contract and
the client core; (3) the chain core (`project_of`, `assign`, the brief, the intake, the views); (4) the tools: the
Assistant's and the Chair's, `draft_objective`'s `project`, the manual; (5) `open_repo`'s resolution and the connection
check; (6) the screens, each looked at on a `web/verify` page.

## Revisit when

- **A memory misleads across products**: a retrospective marks a recalled memory *misled* because it was another
  product's (ADR-0082). Tag what memory admits with the chain's project and recall the project's first; the seam is
  `memoryd`'s admission and the session's `memory_project`.
- **One product takes every slot**: admission is global; give each project a share. The seam is the scheduler's
  admission order, which can read `project_of`.
- **A product needs its own Chair**: a project names the Chair employee that sits in its chains, between the key result's
  owner and the playbook's (ADR-0062's seat order).
- **Agents keep naming the connection by hand**: a project's connection becomes the default `connection` of a source
  tool in its chains; the seam is the tool's argument resolution in `reach`.
- **Grants should follow the product** (the owner asks for hardening): a grant scoped to a project instead of a chain;
  the seam is `ChainGrant` and `grant::decide`.
- **A product has signals of its own** (error rate, its KPIs): a project lists its signals and its guardrails, which its
  objectives inherit (ADR-0045).
- **Two companies share one instance**: that is a cell (K3), never a project.
