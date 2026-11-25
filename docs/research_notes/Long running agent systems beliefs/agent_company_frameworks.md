# Beliefs and design principles of "AI company" and long-running multi-agent orchestration systems (state as of 2026-11-13)

Confidence labels: **HIGH** = read in the primary source this session (repo file via `gh api`, the project's own page, or the paper). **MEDIUM** = taken from a secondary source, or from a primary source known from earlier reading that was not re-fetched this session. **LOW** = a single unverified or promotional source.

Repo metadata was read with `gh api repos/<repo>` on 2026-11-13 (HIGH):
- paperclipai/paperclip: 113k stars, created 2026-03-02, active.
- multica-ai/multica: 56k stars, created 2026-01-13, active.
- BloopAI/vibe-kanban: 28.7k stars, created 2025-06, "sunsetting".
- getpaseo/paseo: 23.9k stars, created 2025-10-13, active.
- smtg-ai/claude-squad: 8.8k stars, last push 2026-08-20.
- imbue-ai/sculptor: 246 stars, active.
- terragon-labs/terragon-oss: snapshot taken at shutdown.

---

## Q0. What is each project's core thesis, what are its key concepts, and which design choices follow?

### Takeaway
There are two camps. The **"company" camp** (Paperclip, Polsia, MetaGPT/MGX, ChatDev 1.0) maps agents onto a human corporation: CEO, org chart, goals, budgets, a board. The **"workbench/team" camp** (Multica, Vibe Kanban, Conductor, Sculptor, Claude Squad, Paseo, Terragon) treats agents as parallel workers in isolated worktrees, with a human planning and reviewing. Framework vendors (AutoGen, Agent Framework, CrewAI, OpenHands SDK) have converged on one design: a deterministic workflow or event log as the backbone, with bounded LLM autonomy inside it.

### Cited Findings

#### Paperclip (paperclipai/paperclip): "If OpenClaw is an employee, Paperclip is the company"
- **Thesis.** The project calls itself "the backbone of the autonomous economy". Its goal is for "Paperclip-powered companies to collectively generate economic output that rivals the GDP of the world's largest countries." It sees itself as "the control plane, the nervous system" of autonomous companies, not as a company itself. — [doc/GOAL.md](https://github.com/paperclipai/paperclip/blob/master/doc/GOAL.md) (HIGH)
- **Core principle.** "You should be able to look at Paperclip and understand your entire company at a glance — who's doing what, how much it costs, and whether it's working." — [doc/GOAL.md](https://github.com/paperclipai/paperclip/blob/master/doc/GOAL.md) (HIGH)
- **Five product principles.** — [doc/PRODUCT.md §Principles](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)
  1. Unopinionated about the agent runtime.
  2. The company is the unit of organization.
  3. The adapter config defines the agent: "the minimum contract is just 'be callable'".
  4. **All work traces to the goal**: "If you can't explain why a task matters to the company goal, it shouldn't exist."
  5. **Control plane, not execution plane**: "Agents run wherever they run and phone home."
- **Goal ancestry.** Every task has a parent chain that ends at the company goal, e.g. "researching Facebook ads … because → grow signups … because → $2,000 revenue this week … because → #1 AI note-taking app to $1M MRR in 3 months". This is "what keeps autonomous agents aligned — they can always answer 'why am I doing this?'" — [doc/PRODUCT.md §Task Management](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)
- **Org chart.** "Every employee is an agent". A company starts with a CEO, then CTO, CMO and so on, each with its own adapter config, a capabilities paragraph (so other agents can discover who to ask) and a reporting line. Example CEO heartbeat instruction: "review what your executives are doing, check company metrics, reprioritize if needed, assign new strategic initiatives". — [doc/PRODUCT.md](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)
- **Heartbeat as the hiring contract.** "If it can receive a heartbeat, it's hired." Heartbeat execution is a "DB-backed wakeup queue with coalescing, budget checks, workspace resolution, secret injection, skill loading, and adapter invocation". Agents wake for assigned work, follow-ups or optional timers. "A mention alone does not assign work or wake another agent." — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH)
- **Execution correctness.** The README lists these as the hard parts it gets right — [README §Why Paperclip is special](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH):
  - atomic task checkout ("single assignee and execution locks prevent competing runs");
  - persistent work context with session resume;
  - runtime skill injection;
  - "Governance with rollback" (revisioned config);
  - goal-aware execution.
- **Budgets.** "Token salary budgets per agent" — [GOAL.md](https://github.com/paperclipai/paperclip/blob/master/doc/GOAL.md). Budgets are scoped by company, agent, project, goal, issue, provider and model, with warning thresholds and hard stops. The README itself admits that "Enforcement uses recorded spend; usage reporting and in-flight work can delay a stop." — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH)
- **The "board".** The human acts as a board that approves strategy and hires. Dream flow: "CEO proposes strategic breakdown → board approves … Set budgets … Hit go — agents start their heartbeats and the company runs." — [PRODUCT.md §User Flow](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)
- **UI stance.** "Board-level abstraction always wins". The default view answers: "what is the company doing, who is doing it, why does it matter, what did it cost, and what needs my approval". "Conversation stays attached to work objects". "Output-first: work is not done until the user can see the result". Progressive disclosure runs from summary to steps to raw logs. — [PRODUCT.md §Specific design goals](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)
- **Explicit non-goals.** Not a chatbot, not an agent framework ("We don't tell you how to build agents. We tell you how to run a company made of them"), not a Jira replacement, and enterprise RBAC is not the first priority. — [README §What Paperclip is not](https://github.com/paperclipai/paperclip/blob/master/README.md); [PRODUCT.md](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)
- **The four pillars today.** Agentic Task Manager, Org Chart for Agents, Agent Employee Training (Skill Studio, evals, "performance reviews for agents") and Agentic OS. — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH)
- **Repositioning (dated).** The repo description now reads "The open-source app everyone uses to manage agents at work" (`gh api`, 2026-11-13, HIGH). HN launch posts on 2026-03-06 used "Open-source orchestration for zero-human companies" ([HN 47277096](https://news.ycombinator.com/item?id=47277096), HIGH). A secondary source also reports the change of tagline — [web summary of mindstudio/levelup coverage](https://www.mindstudio.ai/blog/what-is-paperclip-zero-human-ai-company-framework) (MEDIUM).

#### Multica (multica-ai/multica): "Make humans and AI agents work as one team"
- **Thesis.** The name stands for "Multiplexed Information and Computing Agent", a nod to Multics time-sharing: "two engineers and a fleet of agents can move like twenty." Agents are "first-class teammates" that are assigned issues, report progress, raise blockers and ship code. — [VISION.md](https://github.com/multica-ai/multica/blob/main/VISION.md) (HIGH)
- **Explicitly not the Paperclip thesis.** "Multica is not an autonomous company running beyond human control. It is the shared operating system for people and agents doing consequential work together. People set direction and remain accountable. Agents keep the work moving." — [VISION.md](https://github.com/multica-ai/multica/blob/main/VISION.md) (HIGH)
- **Escalation rule.** "If the task is clear, agents can move it forward. If it changes product behaviour, introduces risk, or depends on a trade-off, the right person is brought in before the work continues." "The team does not review status theatre. It reviews the work itself." — [VISION.md](https://github.com/multica-ai/multica/blob/main/VISION.md) (HIGH)
- **Concepts** — [README](https://github.com/multica-ai/multica/blob/main/README.md) (HIGH):
  - agents as named assignees;
  - Squads, where "the leader routes the work";
  - Skills ("turn a solved problem into a playbook");
  - a daemon runtime on the user's own machine;
  - Autopilots (cron standups, audits, reports);
  - mid-run steering ("message lands in the current run");
  - review gates ("Work lands in review, not in main. You decide what ships");
  - an Inbox ("pinged when an agent needs a call, not for every step");
  - automatic retries and timeouts;
  - an audit trail "that includes the robots".
- **Promise.** "Nothing ships without a human saying so." — [README](https://github.com/multica-ai/multica/blob/main/README.md) (HIGH)

#### Vibe Kanban (BloopAI): planning and review are the bottleneck
- **Thesis.** "In a world where software engineers spend most of their time planning and reviewing coding agents, the most impactful way to ship more is to get faster at planning and review." It combines kanban issues, a workspace per issue (branch, terminal, dev server), inline diff comments sent back to the agent, and PRs. — [README](https://github.com/BloopAI/vibe-kanban/blob/main/README.md) (HIGH)
- **Status.** Sunset. The company shut down on 2026-04-10 (post by Louis Knight-Webb): most of the "thousands of daily users are free users" and the team "couldn't find a business model that we could get excited about". The repo continues as community-maintained open source. — [vibekanban.com/blog/shutdown](https://vibekanban.com/blog/shutdown) (HIGH)

#### Conductor (conductor.build, Melty Labs, YC)
- **Thesis.** Run many Claude Code and Codex agents in parallel, one isolated git worktree per independent task, with review, diff and merge on top. — [conductor.build docs, parallel agents](https://www.conductor.build/docs/core/parallel-agents) (HIGH, page fetched; undated)
- **Its one rule for parallel work.** "Parallel agent work starts with one decision: should these agents share a workspace, or should they move independently?" "Use multiple workspaces when tasks should land independently. Use multiple agents in one workspace when the work shares the same branch, code state, and context." "Review comments should apply to the same diff." The human "review[s], merge[s], or archive[s] each stream of work on its own schedule." — [conductor.build docs, parallel agents](https://www.conductor.build/docs/core/parallel-agents) (HIGH)
- **No org layer.** The docs have no goals, budgets or hierarchy. The site shows "We've raised a $22m Series A" and "Introducing Conductor Cloud" as banners — [conductor.build docs](https://www.conductor.build/docs/core/parallel-agents) (MEDIUM, banner text from search results)

#### Anthropic Claude Code "agent view" (2026-05-11)
- **Stance.** Agent view is "one place to manage all your Claude Code sessions". You "kick off new agents, send them to the background, and jump in only when Claude needs you". A session "waiting on a decision" is flagged; you "answer inline and the session picks back up". You can "steer many all at once", and the typical output is a list of pull requests ready for review. — [claude.com/blog/agent-view-in-claude-code](https://claude.com/blog/agent-view-in-claude-code) (HIGH)
- **Mechanics.** `claude agents` lists sessions grouped by state, with those that need you at the top. `claude --bg` starts work in the background. The view runs locally, while web sessions run in Anthropic's cloud. — [code.claude.com/docs/en/agent-view](https://code.claude.com/docs/en/agent-view) (MEDIUM, from the search summary of the official docs)
- **Philosophy.** Anthropic frames the human as the decider and reviewer and Claude as the parallel executor. It adds no org chart, goals or budgets.
- **Claude Projects.** A search restricted to anthropic.com and claude.com returned no orchestration philosophy for "Claude Projects" (the claude.ai feature). Anthropic's stated beliefs about long-running and team agents live in the engineering posts listed below, which I cite instead. (HIGH that the search found none)

#### Imbue Sculptor
- **Thesis.** A "desktop app for running coding agents in parallel" with "grounded" agents. It is an "experimental research preview. We are still learning what rigorous engineering looks like with agents." — [README](https://github.com/imbue-ai/sculptor/blob/main/README.md) (HIGH)
- **Beliefs.** Software development should stay open; the industry hype "rarely delve[s] into the nuance and tradeoffs"; "agents that stay accountable to the people they serve". — [README §Why we made Sculptor / About Imbue](https://github.com/imbue-ai/sculptor/blob/main/README.md) (HIGH)
- **Design.** Isolated workspaces, review-then-merge, and bundled workflow skills. Example: `/sculptor-workflow:fix-bug` "runs a short reproduction interview, writes failing tests, then makes them pass", which builds rigor into skills rather than into an org chart. — [README](https://github.com/imbue-ai/sculptor/blob/main/README.md) (HIGH)

#### Terragon (terragon-labs)
- **Thesis.** "Delegate work to coding agents in the cloud": a sandbox container per task, a branch per task, work "checkpointed and pushed to GitHub", local takeover via the `terry` CLI, automations (recurring and event-triggered), and @-mentions from Slack and GitHub. — [terragon-oss README](https://github.com/terragon-labs/terragon-oss/blob/main/README.md) (HIGH)
- **Status.** Shut down. The open-source snapshot is dated 2026-01-16. — [README](https://github.com/terragon-labs/terragon-oss/blob/main/README.md) (HIGH). The only reason found is a third-party opinion: the moat was thin, model providers would "cannibalize their UX", and it was a side project that exploded. — [jmduke.com/posts/failed-experiments](https://jmduke.com/posts/failed-experiments/) (LOW: opinion, not Terragon's statement)

#### Claude Squad (smtg-ai)
- **Thesis.** A minimal terminal multiplexer: tmux sessions plus a git worktree per task. It completes "tasks in the background (including yolo / auto-accept mode!)", lets you "review changes before applying them", and "pause" a session, which commits it and frees the worktree. It has no org, goals or budgets. — [README](https://github.com/smtg-ai/claude-squad/blob/main/README.md) (HIGH)

#### Paseo (getpaseo)
- **Thesis.** "For agent-driven development, Paseo replaces your IDE"; "Paseo manages other agents, it doesn't ship one". A self-hosted daemon "owns the running agents and workspaces", so work continues across devices or on a server. Apache-2.0, "no telemetry, tracking, or required Paseo account". — [paseo.sh/docs/why](https://paseo.sh/docs/why); [README](https://github.com/getpaseo/paseo/blob/main/README.md) (HIGH)
- **Orchestration.** Agents get MCP tools to "create worktrees, launch other agents, send prompts, and collect results" across providers. A launched worker returns immediately and the parent is notified when it finishes. A **heartbeat** "prompt[s] the same agent periodically to continue its task". A **schedule** runs "a prompt in a new agent at specified times". — [paseo.sh/docs/orchestration](https://paseo.sh/docs/orchestration) (HIGH)

#### OpenHands Software Agent SDK (All Hands AI)
- **Paper** (arXiv 2511.03690, published 2025-11-05). The SDK is "a complete architectural redesign of the agent components" of OpenHands. Compared with the OpenAI, Claude and Google SDKs, it claims to uniquely combine "native sandboxed execution, lifecycle control, model-agnostic multi-LLM routing, and built-in security analysis", with "seamless local-to-remote execution portability". — [arXiv abstract](https://arxiv.org/abs/2511.03690) (HIGH, abstract read via the arXiv API)
- **Design** (MEDIUM: from the search summary of the paper body, not a full read):
  - optional isolation (local-first, sandbox on demand);
  - immutable components (agent, tools, LLM) validated at construction;
  - **event-sourced state**: one `ConversationState` holds an append-only event log;
  - resume by replaying events from `base_state.json`;
  - LLM-visible events kept separate from internal events (condensation, pause).
  — [arXiv 2511.03690 html](https://arxiv.org/html/2511.03690v2); [MLSys 2026 oral](https://mlsys.org/virtual/2026/oral/3749)

#### MetaGPT / MGX (FoundationAgents)
- **Thesis** (paper published 2023-08-01). Naively chained LLMs suffer "logic inconsistencies due to cascading hallucinations". MetaGPT "encodes Standardized Operating Procedures (SOPs) into prompt sequences … allowing agents with human-like domain expertise to verify intermediate results". It uses "an assembly line paradigm to assign diverse roles to various agents". — [arXiv 2308.00352 abstract](https://arxiv.org/abs/2308.00352) (HIGH)
- **Mechanics.** The structured documents (PRDs, designs) and the publish-subscribe message pool come from the paper body, not the abstract. Slogan: `Code = SOP(Team)`. — [noze.it summary](https://www.noze.it/en/insights/metagpt-open-source/) (MEDIUM)
- MGX (MetaGPT X) is the commercial product, "the first multi-agent AI dev team based on real software SOPs", which the user chats with like a team leader (LOW: third-party listing wording).
- **MGX is now "Atoms".** `https://mgx.dev` redirects to `https://atoms.dev` (checked with curl, 2026-11-13). The site's description reads: "Atoms is your team's website and app creator. Use Atoms AI employees to validate ideas, build products, and acquire customers in minutes, no code needed." — [atoms.dev](https://atoms.dev) (HIGH). The product moved from "a software company of agents" to "AI employees" that run the whole build-and-acquire loop for non-coders. No 2026 design document was found.

#### ChatDev (OpenBMB)
- **1.0 (paper 2023-07-16).** Agents are "guided in what to communicate (via chat chain) and how to communicate (via communicative dehallucination)" across design, coding and testing. The paper establishes "language as a unifying bridge for autonomous task-solving". — [arXiv 2307.07924 abstract](https://arxiv.org/abs/2307.07924) (HIGH). The README describes 1.0 as a "Virtual Software Company" with CEO, CTO and Programmer agents in "specialized functional seminars" — [README](https://github.com/OpenBMB/ChatDev) (HIGH)
- **2.0 "DevAll" (2026-01-07).** "ChatDev has evolved from a specialized software development multi-agent system into a comprehensive multi-agent orchestration platform": a "Zero-Code Multi-Agent Platform for 'Developing Everything'". "The classic ChatDev (v1.x) has been moved to the `chatdev1.0` branch for maintenance." — [README](https://github.com/OpenBMB/ChatDev) (HIGH). OpenBMB thus **dropped the fixed software-company metaphor**.
- **Between the two:** MacNet, DAG topologies across "more than a thousand agents" (2024-06), and a "puppeteer" orchestrator "optimized with reinforcement learning" that "dynamically activates and sequences agents" (NeurIPS 2025, [arXiv 2505.19591](https://arxiv.org/abs/2505.19591)). — [README news](https://github.com/OpenBMB/ChatDev) (HIGH)

#### CrewAI
- **Thesis** (João Moura, 2025-12-15). "Structure where you need it, intelligence where it matters." Flows set the sequence and rules; Crews (role, goal and backstory agents) are invoked at specific steps and hand control back. "The Flow enforces business logic that can't be negotiated, the agents don't decide whether to do these steps." Production systems must be "observable (trace every decision), governable (enforce policies)", plus cost-controlled, auditable and maintainable. — [blog.crewai.com/agentic-systems-with-crewai](https://blog.crewai.com/agentic-systems-with-crewai/) (HIGH)
- The docs advise starting with a Flow when going to production. — [docs production-architecture](https://docs.crewai.com/v1.14.3/ko/concepts/production-architecture) (MEDIUM, Korean page via search summary)

#### Microsoft AutoGen, Magentic-One and Agent Framework
- **Magentic-One (2024-11-07).** "A lead agent, the Orchestrator, plans, tracks progress, and re-plans to recover from errors", directing agents that operate a browser, navigate files, or write and run code. "Agents [can] be added or removed from the team without additional prompt tuning". — [arXiv 2411.04468 abstract](https://arxiv.org/abs/2411.04468) (HIGH)
- The Orchestrator's two ledgers come from the paper body, not the abstract: a Task Ledger (facts, guesses, plan) and a Progress Ledger (per step: done? stalled? who acts next?). — [arXiv 2411.04468](https://arxiv.org/abs/2411.04468) (MEDIUM, from earlier reading)
- **Status, from AutoGen's own README** (read 2026-11-13) — [microsoft/autogen README](https://github.com/microsoft/autogen) (HIGH):
  - "AutoGen is now archived and no longer maintained by Microsoft; the architecture described below remains available for historical/reference purposes."
  - "Microsoft Agent Framework (MAF) is the enterprise‑ready successor to AutoGen … Microsoft Agent Framework 1.0 gives you enterprise-grade multi-agent orchestration, multi-provider model support, and cross-runtime interoperability via A2A and MCP."
  - "AutoGen was developed for research and experimentation … we continue to advise against production use."
  - Magentic-One remains in the repo as a package built on AgentChat.
  - The GitHub flag `archived` is still false (last push 2026-11-12), while the README says "archived" and "maintenance mode".
- **Merger dates.** MAF merges AutoGen with Semantic Kernel and reached 1.0 GA on 2 or 3 April 2026. — [VentureBeat](https://venturebeat.com/ai/microsoft-retires-autogen-and-debuts-agent-framework-to-unify-and-govern); [techstrong.ai](https://techstrong.ai/features/microsoft-ships-agent-framework-1-0-a-production-ready-foundation-for-multi-agent-ai/) (MEDIUM)

#### Anthropic: stated philosophy and the Project Vend experiment
- **"Building effective agents" (2024-12-19, Erik S. and Barry Zhang)** — [anthropic.com/engineering/building-effective-agents](https://www.anthropic.com/engineering/building-effective-agents) (HIGH):
  - Workflows are "systems where LLMs and tools are orchestrated through predefined code paths"; agents "dynamically direct their own processes and tool usage".
  - "We recommend finding the simplest solution possible, and only increasing complexity when needed."
  - Principles: simplicity, transparency ("explicitly showing the agent's planning steps"), and a carefully crafted agent-computer interface.
  - Agents pause at checkpoints or blockers, with "stopping conditions (such as a maximum number of iterations) to maintain control".
- **"Effective harnesses for long-running agents" (2025-11-26)** — [anthropic.com/engineering/effective-harnesses-for-long-running-agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) (HIGH):
  - Work persists in files and git, not in the context window.
  - An initializer agent writes `init.sh`, a `claude-progress.txt` log, an initial commit, and a JSON feature list with every feature "marked as 'failing'".
  - Each later session reads the git log and the progress file, then picks one unfinished feature. The key insight was "finding a way for agents to quickly understand the state of work when starting with a fresh context window."
  - Observed failures: overreach that left "a feature half-implemented and undocumented"; later agents declaring the project finished too early; features marked complete without end-to-end tests.
- **"Building effective human-agent teams" (2026-06-24)** — [claude.com/blog/building-effective-human-agent-teams](https://claude.com/blog/building-effective-human-agent-teams) (HIGH). This is Anthropic's closest stated philosophy for a company of agents:
  - "Grant agents autonomy in proportion to demonstrated reliability, then expand it deliberately."
  - "Humans always set the north star."
  - "At the beginning, humans reviewed every decision made by an agent." Later, "decisions with hard tradeoffs always had a human in the loop."
  - "Clearly defined roles and responsibilities set human-agent teams up for success … Each owns a clear task and works on its own schedule; people set the goals and review output."
  - "Engineers have successfully dispatched agents on their team to handle 500 bug fixes independently."
- **Related posts not read** (titles from search, LOW): "Scaling Managed Agents with stable interfaces", "Harness design for long-running apps" (a planner, generator and evaluator architecture), and "Introducing dynamic workflows" in Claude Code. — [anthropic.com/engineering/managed-agents](https://www.anthropic.com/engineering/managed-agents); [harness-design-long-running-apps](https://www.anthropic.com/research/harness-design-long-running-apps); [dynamic workflows](https://claude.com/resources/articles/introducing-dynamic-workflows-in-claude-code)
- **Project Vend phase 2 (2025-12-18).** Andon Labs and Anthropic added a **CEO agent, "Seymour Cash"**, above the shopkeeper agent Claudius. Findings — [anthropic.com/research/project-vend-2](https://www.anthropic.com/research/project-vend-2) (HIGH):
  - The CEO set revenue targets and a 50% minimum margin, but "shared many of the deficiencies and blind spots of Claudius" (same model).
  - The shop's profit "may have been in spite of the CEO, rather than because of it".
  - "Bureaucracy matters": mandated checklists and verification acted as institutional memory.
  - A **narrowly scoped, separate-role agent (Clothius)** worked better than hierarchy.
  - Humans still did the physical work and dispute resolution, and purchases needed a human check.
  - A staffer manipulated a naming vote into making Claudius declare a new "CEO".
- "Claude Projects" has no published orchestration philosophy that I found (see Gaps).

#### Polsia (an "autonomous company" startup)
- **Self-description.** The homepage title is "Polsia — AI That Runs Your Company While You Sleep". The page is a JavaScript app; nothing else is readable without a browser, and Polsia publishes no design document. — [polsia.com](https://polsia.com) (HIGH for the title)
- **Model.** AI runs many small businesses for founders, priced at $49/month plus a 20% revenue share. It reportedly raised $30M at a $250M valuation (May 2026), with a solo founder and "zero employees". — [aiweekly.co](https://aiweekly.co/alerts/polsia-solo-founder-raises-30m-at-250m-valuation); [dev.to](https://dev.to/andrew-ooo/polsia-how-one-solo-founder-hit-15m-arr-in-30-days-with-zero-employees-15n7) (LOW: press and founder claims; no primary source exists)
- **Disputed revenue.** Revenue figures conflict across sources ($1.4M earned, a $4.5M run rate, $6.3M ARR, $10M forecast). Most revenue reportedly comes from founders paying for the service, not from customers of the AI-run businesses. — [search summary of Enterprise DNA / runtimewire / thenextnewthing](https://runtimewire.com/article/polsia-ben-broca-10-million-revenue-zero-employees) (LOW)

### Inferences
- The "company" metaphor gives a legible control surface (goal tree, budget, approvals). Its own critics, and Anthropic's Vend data, say the metaphor's *hierarchy* adds little when every layer runs the same model. What helps is role separation, checklists and verification tools. (MEDIUM)
- Every system that survived (Paperclip, Multica, Paseo, Agent Framework, CrewAI) has moved toward a durable, server-side record of work (issue, event log, DB queue), with LLM autonomy inside bounded steps. (MEDIUM)
- ChatDev's 2.0 pivot and Paperclip's tagline change both move away from the literal "zero-human software company" toward general orchestration or management. (MEDIUM)

### Gaps
- No Paperclip "manifesto" file beyond `doc/GOAL.md` and `doc/PRODUCT.md` was found. `DESIGN.md` is a UI design-language document, not a philosophy.
- The `paperclip.ing/blog/where-autonomous-companies-go-next/` post returns 404 on the site and has no Wayback snapshot (checked via the archive.org availability API). It is absent from the current blog index ([paperclip.ing/blog](https://paperclip.ing/blog)), so its claim is dropped. The closest live post, "AI agent governance" (2026-11-10), is cited under Q2.
- For MetaGPT, ChatDev, Magentic-One, OpenHands SDK and TheAgentCompany, only the arXiv abstracts were read (HIGH for the abstract). Body-level mechanics stay MEDIUM.
- MGX/Atoms has no 2026 primary design document; only a Product Hunt listing exists.
- Anthropic "Claude Projects" has no orchestration philosophy that I could find.

---

## Q1. What does each believe a company of agents needs: hierarchy, goals/OKRs, budgets, approvals, heartbeats, tickets?

### Takeaway
Only Paperclip (and MetaGPT/ChatDev 1.0 in research form) asks for the full corporate kit: CEO-rooted org chart, goal tree, per-agent "salary" budgets, board approvals and heartbeats. The workbench tools need only tickets or issues, an isolated workspace and a review gate. Multica sits between the two (squads with a leader, autopilots, an inbox) and is being pushed by users toward explicit org structure.

### Cited Findings

| Need | Paperclip | Multica | Vibe Kanban / Conductor / Sculptor / Claude Squad | Paseo | Frameworks |
|---|---|---|---|---|---|
| Hierarchy | CEO→CTO/CMO→ICs, reporting lines, delegation "up and down the org chart" ([PRODUCT.md](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md), HIGH) | Squads with a leader that "routes the work" ([README](https://github.com/multica-ai/multica/blob/main/README.md), HIGH) | None: flat parallel workers ([VK](https://github.com/BloopAI/vibe-kanban/blob/main/README.md), [CS](https://github.com/smtg-ai/claude-squad/blob/main/README.md), HIGH) | Parent agent launches workers ([docs/orchestration](https://paseo.sh/docs/orchestration), HIGH) | MetaGPT fixed SOP roles; Magentic-One one Orchestrator (MEDIUM) |
| Goals | Goal ancestry on every task; "if you can't explain why … it shouldn't exist" (HIGH) | Projects group work; no goal tree (README, HIGH) | Issue / prompt only | None | None |
| Budgets | Company/agent/project/goal/issue budgets, thresholds, hard stops (README, HIGH) | Usage analytics per agent and issue; no caps stated (README, HIGH) | None | None stated | None built in |
| Approvals | Board approves strategy and hires; review/approval stages; gateway actions "Allowed / Ask first / Off" (README, HIGH) | Review gates; human brought in on risk or trade-off (VISION, HIGH) | Human reviews diff and merges (HIGH) | Human decides "what to merge" ([why](https://paseo.sh/docs/why), HIGH) | Human-in-loop optional |
| Heartbeats | Core primitive: wake on assignment, follow-up, timer (HIGH) | Assignment-triggered runs + Autopilots (cron) (HIGH) | None | Heartbeat re-prompts the same agent; schedules start new agents (HIGH) | None |
| Tickets | Issues with atomic checkout, single assignee, blockers, documents (HIGH) | Issues as the unit; agents set status via CLI (HIGH) | Kanban issues (VK) / workspaces (others) | Workspaces/agents, not tickets | None |

- **Anthropic counter-evidence on hierarchy.** A same-model CEO "shared many of the deficiencies and blind spots" of its report. A separate, narrowly scoped role helped more. — [Project Vend 2](https://www.anthropic.com/research/project-vend-2) (HIGH)
- **A user asks for explicit org structure in Multica.** A fintech CMO runs two marketing agents. Delegation and review encoded in free-text instructions "works *sometimes* — but it's brittle, opaque, and not introspectable. Neither agent can answer 'who are my reports?'" The maintainer replied: "Maybe Multica needs something like an organization structure chart for agent." — [multica#815 comments](https://github.com/multica-ai/multica/issues/815) (HIGH)
- **Paperclip skill permissions are restrictions, not grants.** "Skill permissions are opt-in restrictions, not opt-in capabilities … ordinary skill work does not require board confirmation." — [PRODUCT.md](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md) (HIGH)

### Inferences
- The users' recurring demand is *introspectable* structure: who reviews whom, who decides the next stage. They are not asking for a CEO persona. A typed relation (reviewer of, reports to) answers "who are my reports?", and free-text instructions do not. (MEDIUM)
- Budgets as a first-class concept appear only where agents run unattended for long periods (Paperclip). Workbench tools that assume a human is present skip them. (MEDIUM)

### Gaps
- No project models OKRs with measurable key results tied to live metrics. Paperclip's goals are text at the root of a task tree. I found no evidence of metric-backed goals in any project.

---

## Q2. How do they keep the human in or out of the loop, and what decides for the human when he is absent?

### Takeaway
No project has an agent that holds the owner's authority while he is away. When the human is absent, the systems fall back to **deterministic rules**: budget hard stops, approval gates that block, gateway action policies, retries and timeouts. Work simply waits in review. Paperclip's direction admits this does not scale to the volume of a real company's decisions. Multica's community treats human-in-the-loop as the feature.

### Cited Findings
- **Paperclip.** Governance means board approvals, "execution policies with review/approval stages, decision tracking, budget hard-stops, and agent pause/resume/terminate". Connection actions go through a tool gateway: "Approve a call once or save a revocable rule." — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH). Paperclip has many idle-time decision docs (`idle-decision-expiry.md`, `idle-approval-queue-seeding.md`, `idle-tool-review-expiry.md`, …), which shows that expiring or re-routing pending decisions is an active design area — [doc/ listing](https://github.com/paperclipai/paperclip/tree/master/doc) (HIGH for existence; contents not read).
- **Paperclip's governance checklist** ("AI agent governance", Paperclip Labs, 2026-11-10) — [paperclip.ing/blog/ai-agent-governance](https://paperclip.ing/blog/ai-agent-governance/) (HIGH). It does not say how work proceeds when no approver is present. It says:
  - "List the actions that need a person's approval" (merge to main, a published post, a payment, a customer message); everything else can "run, or turn them off".
  - Each connection action is "Allowed, Ask first or Off".
  - "Approve a call once, or save a rule that you can revoke later."
  - "An alert without a pause doesn't stop a loop that runs at 3 a.m."
  - "Any authorized person must be able to pause, reassign or stop an agent right away."
- **Paperclip's task watchdog: an agent that second-guesses stopped work.** An opt-in watchdog agent is woken "when every leaf in that subtree comes to rest — done, cancelled, blocked, in review, or waiting". Its purpose is to verify the stop and put the work "back into motion when stopping was a mistake" — [doc/TASK-WATCHDOG.md](https://github.com/paperclipai/paperclip/blob/master/doc/TASK-WATCHDOG.md) (HIGH).
  - Why it exists: agents stop for wrong reasons, such as "misreading a blocker, accepting a stale plan confirmation, declaring 'done' without proof, leaving an issue `in_review` with no real reviewer … None of those failures wake anyone on their own — the tree just sits."
  - Its mandate: "Treat every stopped leaf as a **claim** that must be verified … Do not accept 'I could not' or 'waiting for approval' as automatically valid." A real blocker needs "a valid waiting disposition that names the unblock owner and the next action".
  - Its authority limit is enforced by the server, and custom instructions cannot widen it. It must not approve or reject rows in `approvals` ("hiring, CEO strategy, spend, budget override, or `request_board_approval` decisions"), must not touch budgets, secrets, agents or settings, and must not "bypass typed participants".
  - Paperclip therefore lets an agent restore a work path without the human, but never decide what is the human's.
- **Paperclip decisions expire instead of waiting forever.** Open decisions carry an `expiresAt` deadline settled by a worker. Startup "reconstructs saved deadlines, including already-overdue work" — [doc/idle-decision-expiry.md](https://github.com/paperclipai/paperclip/blob/master/doc/idle-decision-expiry.md) (HIGH).
  - A decision whose target task is gone or completed expires with `target_gone` or `target_completed` — [doc/idle-decision-targets.md](https://github.com/paperclipai/paperclip/blob/master/doc/idle-decision-targets.md) (HIGH).
  - A terminal decision can owe a wake to the agent that asked (`continuationPolicy = wake_origin_agent`), stored on the decision row itself and delivered by a recovery worker — [doc/idle-decision-continuations.md](https://github.com/paperclipai/paperclip/blob/master/doc/idle-decision-continuations.md) (HIGH).
  - So the default for an absent human is **expiry plus waking the asker**, not a delegate deciding.
- **Paperclip "decision models" (small machine decisions).** A company can configure one API connection for "optional Paperclip features" that ask a model typed questions (boolean, choice, score), e.g. classifying a task. "Allow company-sponsored background decisions defaults on", which "authorizes internal background features to charge the selected connection without a responsible human". Every call is recorded in `decision_invocations` with the actor, responsible user and cost, but not the prompt or answer. — [doc/decision-models.md](https://github.com/paperclipai/paperclip/blob/master/doc/decision-models.md) (HIGH)
- **How Paperclip waits on a human** — [doc/execution-semantics.md](https://github.com/paperclipai/paperclip/blob/master/doc/execution-semantics.md) (HIGH):
  - **A wait must be routable.** "Entering `blocked` requires a routable waiting path": a first-class blocker, "a pending issue-thread interaction or linked approval that names the responder", or "a structured unblock descriptor naming `{owner, action}`". The named owner is notified at once: an agent gets a wake, a user or the board gets an inbox item. "Prose-only blocked … routes to nobody. It is rejected at the API or auto-classified as `needs_attention`."
  - **Agents keep going around unanswered questions.** A newer human message makes an earlier question historical. "Agents must continue authorized work that does not need the missing information, withdraw obsolete questions when evidence satisfies them, and name any input that still prevents the current work."
  - **Approvals never lapse into consent.** "Dismissal never authorizes a tool, secret, connection, document approval … or configured review stage." "Approvals, governed tool/credential/connection requests, and configured review stages retain their gates; a later message does not grant approval."
  - **Tool approvals have no implicit deadline.** "Ordinary tool reviews have no implicit one-hour deadline. A person may approve the saved action after the requesting run ends or the server restarts."
  - **What expiry does.** A connection request made obsolete by an already-authorized connection is retired "as expired, and record[s] a system decision without a fabricated human approval".
  - Net rule: **expiry closes a request; it never approves one.** The gated work stays blocked until a person decides, or the request becomes moot.
- **Paperclip separates system-owned recovery from human decisions.** "Infrastructure recovery belongs to the system … Approval, credentials, assignment, budget and deliberate pause remain separate user decisions." — [doc/task-recovery.md](https://github.com/paperclipai/paperclip/blob/master/doc/task-recovery.md) (HIGH)
- **Multica.** "Get pinged when an agent needs a call, not for every step." Failed runs "retry on their own, or stop and tell you why". "Nothing ships without a human saying so." — [README](https://github.com/multica-ai/multica/blob/main/README.md) (HIGH)
- **Multica's maintainer on direction.** "Over time, the human role will decrease — most work will be executed autonomously by AI. But at critical decision points, humans need to set direction and make key calls." Also: "agent A write code and mention agent B to review shouldn't involve human." — [multica#815](https://github.com/multica-ai/multica/issues/815) (HIGH)
- **Critique: the human is still the orchestrator.** In Multica "the answer [to who advances the workflow] is still mostly 'the human'". The assignment workflow "is still basically a hardcoded default prompt, not a user-definable workflow engine". "Agents manage issue status themselves via the CLI", with "No status gate". — [multica#815 issue body](https://github.com/multica-ai/multica/issues/815) (HIGH)
- **Workbench tools.** The human is always the merge gate: Claude Squad "review changes before applying them" with optional `--autoyes`; Vibe Kanban "review diffs and leave inline comments"; Paseo "what to merge". — READMEs (HIGH)
- **Anthropic Vend.** Purchases were withheld from the agent so it "always checked with a human before making purchases". A human staffer, not the CEO agent, caught the onion-futures contract, which was illegal under the 1958 Onion Futures Act. — [Project Vend 2](https://www.anthropic.com/research/project-vend-2) (HIGH)
- **Magentic-One.** The Orchestrator decides for the human inside a task through its Progress Ledger: is the task done, is it stalled, who acts next. — [arXiv 2411.04468](https://arxiv.org/abs/2411.04468) (MEDIUM)

### Inferences
- The design gap across the field is a **delegate that decides for the absent owner, with a record of who decided and why**. Paperclip comes closest:
  - saved revocable rules;
  - decisions that expire and wake the asker;
  - a scoped watchdog agent that may reopen or reassign work but is barred from approvals;
  - attributed background model calls.

  Paperclip explicitly withholds approval authority from every agent (HIGH, TASK-WATCHDOG.md authority contract). Multica offers only an inbox. Claude Code's agent view flags waiting sessions for the human. (MEDIUM, as the overall conclusion)
- Vend shows a same-model supervisor is a weak substitute for the human. A delegate that decides for the owner needs different information, tools or checks (verification, checklists), not just a higher title. (MEDIUM)

### Gaps
- Read: `idle-decision-expiry`, `-targets`, `-continuations`, `idle-approval-queue-seeding`, `decision-models`, `task-recovery`, `TASK-WATCHDOG`, `idle-execution-control`, and the wait, approval and expiry rules of `execution-semantics.md` (found by grep in a 6,066-line file, not read in full).
- **All 35 `idle-*` docs** were skimmed by their first lines; the four most relevant were grepped in more depth. They sort into three groups:
  - **Q2, human absent (approval and decision lifecycle):**
    - `idle-decision-expiry`, `-targets`, `-continuations`, `-effect-recovery`, `-archive-delivery`;
    - `idle-approval-retention`, `idle-approval-queue-seeding`;
    - `idle-tool-review-expiry`, `idle-tool-review-projection`, `idle-approved-tool-recovery`, `idle-interrupted-tool-recovery`;
    - `idle-secret-proposal-expiry`, `idle-join-request-retention`.

    Pattern: each pending request saves its own `expiresAt`, and one owning worker settles it. In `idle-secret-proposal-expiry.md`, "Approval still rejects an expired" proposal, so an approval that arrives late cannot act on an expired request. In `idle-tool-review-expiry.md`, an unsigned pending tool request "creates no expiry obligation yet". This confirms the `execution-semantics.md` rule: expiry closes a request and never approves it.
  - **Q3, persistence across idle periods and restarts:**
    - `idle-agent-timers`: an agent's next timer deadline is rebuilt from agent, issue, run and cost records, including rejection through a saved hire approval;
    - `idle-issue-monitors`: "Paperclip reconstructs monitor deadlines from saved issue rows at server startup";
    - `idle-execution-control`, `idle-native-status-reassessment`, `idle-environment-cleanup`, `idle-terminal-workspace-pagination`;
    - `idle-domain-deadlines`, `idle-incoming-wake`, `idle-sleep-safety` (Paperclip Cloud lets a tenant's host sleep when it is idle, with a safety report of what blocks sleep), `idle-plugin-jobs`.

    Pattern: no recurring polling. Durable deadlines are rebuilt at startup and fired by one shared delivery coordinator.
  - **Channel and credential housekeeping, outside these questions:** `idle-voice-*` (5 docs), `idle-teams-*` (2), `idle-discord-command-registration`, `idle-grok-dispatch`, `idle-login-cleanup`, `idle-gateway-credential-expiry`, `idle-tool-connection-health`.

  — [doc/ listing](https://github.com/paperclipai/paperclip/tree/master/doc) (HIGH, first lines read via `gh api`)

---

## Q3. How do they persist work over hours or days (scheduling, durable state, worktrees, restarts)?

### Takeaway
Durable state lives in a server-side database or event log, with the ticket or issue as the persistent unit. Work isolation is a git worktree, or a sandbox container per task. Long-running continuity comes from re-waking agents (heartbeats, cron routines, schedules) and resuming saved provider sessions, not from one session running forever.

### Cited Findings
- **Paperclip.**
  - The pitch: "On reboot you lose everything" is what it fixes, since "sessions persist across reboots". — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH)
  - Mechanisms: a DB-backed wakeup queue; "supporting adapters resume saved sessions across runs"; "bounded recovery handles supported failures and surfaces cases that need human action"; optional git worktrees per execution; routines (cron, webhook, API) with "concurrency and catch-up policies", each run creating a tracked issue. — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH)
  - Embedded Postgres runs locally. — [README](https://github.com/paperclipai/paperclip/blob/master/README.md) (HIGH)
- **Multica.** A daemon runtime runs on the user's machine ("Code never leaves it"), with Autopilots on cron and automatic retries and timeouts. — [README](https://github.com/multica-ai/multica/blob/main/README.md) (HIGH)
- **Paseo.** "The daemon owns the running agents and workspaces", so clients can disconnect. A heartbeat re-prompts the same agent and a schedule spawns a new one. — [docs/why](https://paseo.sh/docs/why); [docs/orchestration](https://paseo.sh/docs/orchestration) (HIGH)
- **Terragon.** A sandbox container per task; "agent work is checkpointed and pushed to GitHub"; local takeover via `terry`. — [README](https://github.com/terragon-labs/terragon-oss/blob/main/README.md) (HIGH)
- **Claude Squad.** tmux keeps sessions alive. "Checkout" commits changes and pauses the session; resume later. — [README](https://github.com/smtg-ai/claude-squad/blob/main/README.md) (HIGH)
- **Vibe Kanban.** Worktree cleanup includes "orphan and expired workspace cleanup" (`DISABLE_WORKTREE_CLEANUP`). — [README](https://github.com/BloopAI/vibe-kanban/blob/main/README.md) (HIGH)
- **OpenHands SDK.** An append-only event log; resume by replaying events from `base_state.json`; incomplete conversations are detected and continued. — [arXiv 2511.03690](https://arxiv.org/html/2511.03690v2) (MEDIUM)

### Inferences
- The ticket plus the wake queue is the dominant durability pattern for "company" systems. The event log is the dominant pattern for SDKs. Both make the LLM session disposable. (MEDIUM)

- **Paperclip retry keeps the work.** `POST /api/issues/:id/retry` takes `{ idempotencyKey, observedRunId }`, so a repeated HTTP request cannot start a second attempt. "A Retry never silently drops the agent's work":
  - If the task's own worktree is safe to reuse, the retry continues there with a fresh session, and "uncommitted files, Git history and the task branch stay in place".
  - Otherwise a fresh attempt is seeded from committed files, and the previous directory is named in `priorWorkspace` with its uncommitted files.

  — [doc/task-recovery.md](https://github.com/paperclipai/paperclip/blob/master/doc/task-recovery.md) (HIGH)
- **Paperclip recovery workers.** They are driven by committed writes, not unconditional polling. "Each worker runs once on startup to restore durable obligations". Duties cover abandoned finalization, safe native replacement, continuation delivery, execution status and automatic disposition. — [doc/idle-execution-control.md](https://github.com/paperclipai/paperclip/blob/master/doc/idle-execution-control.md) (HIGH)
- **Three distinct liveness mechanisms in Paperclip:**
  - the task watchdog, when a whole subtree stops;
  - the silent active-run watchdog, when a process goes quiet;
  - liveness recovery, when `in_progress` work has no live path.

  — [doc/TASK-WATCHDOG.md §Mental model](https://github.com/paperclipai/paperclip/blob/master/doc/TASK-WATCHDOG.md) (HIGH)

### Gaps
- Paperclip's `dependency-wake-recovery.md` was not read. `execution-semantics.md` was read only for its wait, approval and expiry rules (by grep), not in full.

---

## Q4. What do critics and users say fails in practice?

### Takeaway
The recurring failures fall into four groups:
- **Coordination overhead and token burn** in corporate-hierarchy systems.
- **Recovery and stuck states** in long-running execution: unrecoverable tasks, budget stops that misfire, sessions whose auth expires.
- **Vendor dependency**: subscription and billing changes from Anthropic, and labs absorbing the UX. This killed Terragon and Vibe Kanban as businesses.
- **Credibility gaps** in "autonomous company" revenue claims.

### Cited Findings
- **"Too chatty."** One user moved to Multica "because paperclip and its clones were too bloated, too chaotic, too chatty, spending millions of tokens just negotiating and delegating between themselves. Often for little benefit." — [multica#815 comment by darktempla, 2026-04-25](https://github.com/multica-ai/multica/issues/815) (HIGH: verbatim user opinion)
- **"Managing AI the way it manages people."** Teammate, assignment and comment metaphors fit humans, but the agent-first problem is "who decomposes … who decides the next stage … who enforces approval / acceptance gates … who drives the pace". — [multica#815](https://github.com/multica-ai/multica/issues/815) (HIGH). This is the mirror of the critique that Paperclip "copies traditional human corporate structures … mapping roles like CEO, CTO, and engineers onto AI agents", which "fundamentally misunderstands how autonomous systems function" — [stork.ai via search summary](https://www.stork.ai/blog/ai-wont-run-your-company-heres-why) (LOW: page 403'd on fetch)
- **Hierarchy does not fix blind spots.** Vend's same-model CEO authorized lenient requests "about eight times as often as it denied them". It drifted into "unproductive late-night conversations" with its report and was socially engineered. Profit came "in spite of the CEO". — [Project Vend 2](https://www.anthropic.com/research/project-vend-2) (HIGH)
- **Paperclip long-running bugs** (issue titles read via GitHub API, HIGH; bodies not read):
  - [#13640](https://github.com/paperclipai/paperclip/issues/13640) (2026-09-18): "Task permanently unrecoverable: pre-dispatch cancellation leaves a `replay: blocked` hold with no board, API, or comment path out".
  - [#18106](https://github.com/paperclipai/paperclip/issues/18106) (2026-10-28): "Budget hard stop fires with observed spend 0 when runs finish concurrently; agents then get stuck in `pausing`".
  - [#14182](https://github.com/paperclipai/paperclip/issues/14182) (2026-09-26): "Anthropic subscription AI connections expire after ~8h: only the access token is stored, env-injected, never refreshed".
  - [#13959](https://github.com/paperclipai/paperclip/issues/13959) (2026-09-24): an unbounded `executionContinuation` exceeds `MAX_ARG_STRLEN`, so "spawn E2BIG on every local adapter".
  - [#8723](https://github.com/paperclipai/paperclip/issues/8723): "false-positive orphan reaps".
  - [#5534](https://github.com/paperclipai/paperclip/issues/5534): a regex "mislabels successful runs as claude_auth_required when agent output contains words like 'Unauthorized'".
- **"Why We Killed Paperclip"** (Alien Brain Trust, 2026-04-05) — [abt-blog](https://abt-blog.pages.dev/ai-learning/2026-04-05-why-we-killed-paperclip) (HIGH that the post says this; it is a single operator's account):
  - Setup: CEO, CTO and CFO agents on EC2 with GitHub access and shared memory, on a Claude subscription.
  - The agents acted faster than one part-time person could review. Each morning brought unwanted branches and low-priority tickets, and decisions "internally logical" drifted from that week's needs, leaving "a week of work to unwind".
  - "We spent more time reviewing Paperclip's work than it would have taken to do the work ourselves." Output quality was fine ("weren't broken"); the failure was alignment with priorities.
  - Running persistent agents on a subscription risked the account being flagged, and the API cost too much because of shared-memory reads.
  - Replacement: narrow single-purpose agents (a QA bot, a content bot) behind a human approval gate.
- **Independent test of Paperclip defaults** (Implicator, 2026-09-29, tests run 2026-09-28) — [implicator.ai](https://www.implicator.ai/paperclip-agents-hire-agents-by-default/) (HIGH that the article says this; the tests used stand-in agents):
  - "A new agent could hire another until the owner changed a setting."
  - "Claude Code agents started with permission prompts switched off."
  - A subscription-classed run "reported $5.00 and counted $0.00 against a $1.00 cap".
  - What held: agents could not raise their own budgets, read secrets or approve requests, and a human pause stopped a job within a second.
  - Co-founder Dotta named the biggest problems: "Quality and governance permissions."
- **Reddit (secondhand).** A review quotes an r/aisolobusinesses user: "bunch of Codex credits burned; nothing tangible so far. CEO, CTO and CMO keep checking in and playing ping-pong with certain tickets." It also reports setups taking about 10 days, and a CEO agent writing objectives the engineer agent could not parse. — [preuve.ai/blog/paperclip-ai-review](https://preuve.ai/blog/paperclip-ai-review) (LOW: the Reddit thread itself was not located; a direct search for r/ClaudeAI threads returned only blog reviews)
- **Paperclip issues on token waste** (titles and bodies read via GitHub API, HIGH):
  - [#390](https://github.com/paperclipai/paperclip/issues/390) "Agent circuit breaker" (open since 2026-03-09). The budget hard-stop acts only "by then the damage is done". Users in #373 describe "entire teams hemorrhaging tokens on no-progress heartbeat cycles". It proposes pausing on N runs with no status change, N consecutive failures, or a token-velocity spike, and calls this "the #1 pain point across all multi-agent orchestration tools".
  - [#3406](https://github.com/paperclipai/paperclip/issues/3406) (open): under Claude Max subscriptions "budget policies become fictional guardrails".
  - [#13662](https://github.com/paperclipai/paperclip/issues/13662): "A long comment thread makes an issue permanently un-wakeable … (139 silent dead runs, no board-visible signal)".
  - [#3817](https://github.com/paperclipai/paperclip/issues/3817): agent completion comments attributed to the user "retrigger the assignee (causing status drift, duplicate comments, and agent run loops)".
- **What Paperclip users ask for most.** Local LLMs via Ollama (#187, 82 reactions), chat with agents (#49), more CLI adapters, and a "Company-level knowledge layer (wiki, role context, shared strategy docs)" (#1858). — [GitHub search, sorted by reactions](https://github.com/paperclipai/paperclip/issues?q=sort%3Areactions-%2B1-desc) (HIGH). Cost and lock-in drive the top request, and missing shared company memory is the knowledge gap.
- **What Multica users ask for most.** "Workflow Orchestration" (#1943), a plugin system (#4490), Jira integration, and the issue "Claude Code -p (headless) usage not covered by subscription — affects all daemon tasks" (#2563, closed). — [GitHub search](https://github.com/multica-ai/multica/issues/2563) (HIGH for titles)
- **Paseo's billing question.** "Question: Claude provider roadmap after Agent SDK billing changes" (#1029, 2026-05-14). — [paseo#1029](https://github.com/getpaseo/paseo/issues/1029) (HIGH for title). Together with multica#2563 and paperclip#14182, this shows that orchestrators wrapping subscription CLIs are exposed to provider auth and billing policy.
- **Business fragility.**
  - Vibe Kanban: mostly free users, "couldn't find a business model" — [shutdown post](https://vibekanban.com/blog/shutdown) (HIGH).
  - Terragon: shut down January 2026 — [README](https://github.com/terragon-labs/terragon-oss/blob/main/README.md) (HIGH).
  - Plandex Cloud wound down, citing competition from labs' own agents — [plandex.ai/blog/winding-down](https://plandex.ai/blog/winding-down) (MEDIUM, via search summary).
- **Benchmark reality.** TheAgentCompany (paper 2024-12-18) is "a self-contained environment with internal web sites and data that mimics a small software company"; "the most competitive agent can complete 30% of tasks autonomously". — [arXiv 2412.14161 abstract](https://arxiv.org/abs/2412.14161) (HIGH). The details come from the search summary of v3, not the abstract — [arXiv 2412.14161v3](https://arxiv.org/html/2412.14161v3) (MEDIUM):
  - 175 tasks;
  - GitLab, RocketChat, Plane and ownCloud;
  - Gemini 2.5 Pro at 30.3% full and 39.3% partial completion;
  - an earlier version reported 24%.
- **Polsia credibility (HN, 2026-05-23).** A thread titled "Polsia raised $30M; source map: fake ARR, dead users, god-mode over your company" (35 points, 15 comments). Commenters split between "crypto scams all over again" and "their business model is real … 100% vendor lock-in and take 20% off the top". An earlier HN post (2026-03-22) was titled "Polsia Leaking Customers Secrets". — [HN 48252194](https://news.ycombinator.com/item?id=48252194); [HN 47483387](https://news.ycombinator.com/item?id=47483387) (HIGH that the threads exist; the claims are unverified, LOW)
- **Reviews are mostly promotional.** Blog reviews of Paperclip are SEO-style. One titled "I Tried to Build a Zero-Human Company in a Weekend" reports no actual trial results and repeats README warnings, such as runaway loops that "waste hundreds of dollars of tokens". — [kunalganglani.com](https://www.kunalganglani.com/blog/paperclip-ai-review-zero-human-company.md) (HIGH that the review lacks evidence)
- **Claude Code agent view cost.** Each session draws on rate limits on its own, so 10 parallel agents use roughly 10 times the quota. — [buildfastwithai guide](https://www.buildfastwithai.com/blogs/claude-code-agent-view-guide) (LOW)
- **CrewAI.** Independent critique: crews get "overlapping roles, loops, and model cost" unless roles are narrow, outputs explicit, and there are timeouts, evaluation and cost limits. Crews-inside-Flows is "the sensible architecture for serious work". — [yespress.io review](https://yespress.io/crewai) (LOW-MEDIUM)
- **Field fatigue.** An HN commenter: "all these 'ADE's are mostly the exact same". — via search summary (LOW, thread not located)

### Inferences
- The most-cited real failure is **agents talking to agents** (delegation chatter, same-model supervision). The fixes the evidence supports are fewer handoffs, a deterministic workflow spine, role separation, checklists and verification tools. (MEDIUM)
- Long-running orchestrators fail at their **recovery edges** (cancellation holds, concurrent budget stops, token refresh, unbounded continuation payloads). These are the places to invest tests and telemetry. (MEDIUM, from Paperclip issue titles)
- A wrapper whose only value is UX over a lab's CLI is commercially exposed. The survivors differentiate on the org/governance layer (Paperclip, Multica) or on self-hosted ownership (Paseo). (MEDIUM)

### Gaps
- HN discussion of Paperclip itself is thin. The launch posts had 0–1 comments ([HN 47277096](https://news.ycombinator.com/item?id=47277096)), so most community critique comes from GitHub issues and blogs.
- **Reddit is not reachable from this environment.**
  - A web search restricted to reddit.com fails with "The following domains are not accessible to our user agent: ['reddit.com']".
  - Reddit's anonymous `search.json` API returns an HTML page instead of JSON, for the queries "paperclip agents", "paperclip.ing", "vibe kanban" and "multica".
  - `WebFetch` of a Reddit page (`reddit.com/r/ClaudeAI/search/?q=paperclip`) fails with "Claude Code is unable to fetch from www.reddit.com".
  - Two open web searches found only blog reviews that quote Reddit.

  First-hand Reddit critique needs a logged-in browser session, which this research environment does not have. The one Reddit quote above is secondhand (LOW).
- Polsia's revenue and the "fake ARR" allegations cannot be verified. The public dashboard API that HN commenters cited (`polsia.com/api/public/live/dashboard`) now returns `{"success":false,"message":"Not found"}` (curl, 2026-11-13), and Polsia publishes no financials.
- Paperclip issue bodies: #390 and #3406 were read in full. #13640, #13662, #13725, #14182, #18106, #5534, #8723 and #3817 are cited by title only.

---
Coverage note: about 65 tool calls in total.
- **Primary repo files read:**
  - Paperclip: README, GOAL.md, PRODUCT.md (partial), DESIGN.md (head), TASK-WATCHDOG.md, task-recovery.md, decision-models.md, idle-decision-expiry/-targets/-continuations, idle-approval-queue-seeding, idle-execution-control (heads), and issues #390 and #3406.
  - Multica: README, VISION.md and the #815 thread.
  - READMEs of Vibe Kanban, Claude Squad, Paseo, Sculptor, Terragon and ChatDev.
- **Primary pages fetched:**
  - Paseo docs (why, orchestration) and Conductor's parallel-agents doc;
  - Claude Code agent view post, Project Vend 2, Vibe Kanban shutdown post;
  - Paperclip "AI agent governance" post and CrewAI's agentic-systems post;
  - Implicator and ABT critiques, the HN Polsia thread, the polsia.com title;
  - arXiv abstracts of MetaGPT, ChatDev, Magentic-One, OpenHands SDK and TheAgentCompany.
- **Also read from primary sources in the last pass:** AutoGen's README; Paperclip `execution-semantics.md` (by grep); Anthropic's "Building effective agents", "Effective harnesses for long-running agents" and "Building effective human-agent teams".
- **Still LOW, because no primary source exists or is reachable:**
  - MGX's original "SOP dev team" wording: a third-party listing (its current Atoms positioning is HIGH, from atoms.dev).
  - Polsia's revenue: press and founder claims only. polsia.com shows only its title without JavaScript, and its public dashboard API returns "Not found".
  - The Reddit quote: Reddit is blocked from this environment (search, JSON API and WebFetch all refused).
  - The MAF 1.0 GA date: press only, MEDIUM.
