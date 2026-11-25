//! How a card's agent session is set up: the access its library grants it and the spec its session starts from.

use model::{Card, CardRole, Chain, Origin};

use crate::orch::Orchestrator;
use chains::subagents::{SEAT_SERVER_NAME, builtin_agents_of};
use chains::tools::MCP_SERVER_NAME;
use chains::{Error, Result, policy};

/// How every agent writes what another agent reads. The heading of a routed message is `route`'s kind and label; this
/// is the body's language.
const TALK_BRIEF: &str = "## Agent-to-agent text\n\nReader is an agent (routed message, question to your Chair, answer, hire brief, handover note, report, verdict): write caveman-terse. Fragments, never full sentences. No articles, filler, greetings, hedges, recap. One fact per line. Symbols: → ✓ ✗ ? ≠. Why only when not obvious. Keep exact: path:line, ids, links, commands, output, numbers. Question: one ask + what you know.\n\nFeedback, e.g.:\napi/users.rs:88 ✗ no email → 500, want 400. validate before DB\napi/users.rs:102 ✗ SQL text in error body. log server-side\ntests ✗ both cases. fail before fix\nlint ✓";

/// How a session gets a tree at another commit, now that it may not add a git worktree (policy.rs): a clone of its own in
/// the scratchpad, which shares the clone's objects and leaves nothing in the clone every chain shares.
const OTHER_COMMIT_BRIEF: &str = "## A tree at another commit\n\nYour working directory is the worktree Remit gave you. For a tree at another commit (a base to compare with, a commit to check), clone into your scratchpad: `git clone -q --shared . <scratchpad>/<name> && git -C <scratchpad>/<name> checkout -q <commit>`. Never add a git worktree.";

/// What a card is for, as `library::access` reads it: the cards Remit starts for itself are told apart by their role,
/// and every other card is a worker, run by the agent its role names.
pub(crate) fn card_kind(c: &Card) -> library::CardKind {
    match CardRole::of(c) {
        CardRole::Chair => library::CardKind::Chair,
        CardRole::Judge(_) => library::CardKind::Judge,
        CardRole::Assistant => library::CardKind::Assistant,
        CardRole::Worker | CardRole::Spawned => library::CardKind::Worker,
    }
}

impl Orchestrator {
    /// What card c's session is given: `library::access` over the loadout the card took when it was hired or seated and
    /// the library as stored. The agent the card was hired from is not read again: what it holds now is for later hires. A
    /// worker with no loadout is refused, never read as a plain agent: that would give it every tool.
    pub(crate) async fn access(&self, c: &Card) -> Result<library::Session> {
        use library::CardKind::{Assistant, Chair, Judge, Worker};
        let kind = card_kind(c);
        let mut input = library::Input {
            kind,
            model: c.model.clone(),
            effort: c.effort.clone(),
            ..library::Input::default()
        };
        input.builtin = if kind == Assistant {
            self.capabilities.builtin_for()
        } else {
            self.capabilities.builtin_of_chain(c).await?
        };
        if kind == Worker && c.loadout.is_none() {
            return Err(Error::refused(format!(
                "card {} holds no loadout: it was not hired through the Chair",
                c.id
            )));
        }
        // a Chair with none holds a plain agent's
        if let Some(loadout) = &c.loadout
            && matches!(kind, Worker | Chair)
        {
            input.loadout = loadout.clone();
        }
        if matches!(kind, Worker | Chair | Judge) {
            input.library = self.capabilities.library_of().await?;
        }
        Ok(library::access(&input))
    }

    /// What card c's session is started with: resuming session resume (none: a new one), and reaching Remit's MCP server
    /// with token. What the card holds of the library is the access returned beside it.
    pub async fn spec(&self, c: &Card, resume: &str, token: &str) -> Result<(agent::Spec, library::Session)> {
        let acc = self.access(c).await?;
        let chain = self.chain_of_card(c).await?;
        let mut spec = self.spec_of(c, &acc, chain.as_ref(), resume, token);
        let (types, plugins) = self
            .subagent_types(c, &acc, token)
            .await
            .map_err(|e| Error::refused(format!("the subagent types of card {}: {e}", c.id)))?;
        spec.agents = types;
        // a type's skills come from the plugins its session loads
        for plugin in plugins {
            let dir = self.capabilities.plugin_dir(&plugin);
            if !dir.is_empty() && !spec.plugins.contains(&dir) {
                spec.plugins.push(dir);
            }
        }
        let mut emp = None;
        if !c.employee.is_empty() {
            let id = c.employee.clone();
            let e = self
                .store
                .read(move |r| r.employee(&id))
                .await
                .map_err(|e: store::Error| Error::refused(format!("the employee of card {}: {e}", c.id)))?;
            if policy::ceiling_read_only(&e.ceiling) {
                spec.read_only = policy::read_only_worktrees(c, chain.as_ref());
                spec.deny_tools
                    .extend(agent::EDIT_TOOLS.iter().map(|t| (*t).to_owned()));
                spec.deny_tools.push("Bash".to_owned());
            }
            emp = Some(e);
        }
        spec.read_only.extend(self.controls());
        let spec = self.crew.with_record_brief(emp.as_ref(), spec);
        Ok((spec, acc))
    }

    /// The chain card c works in, or none when its home is not a chain (the Assistant's is not).
    pub(crate) async fn chain_of_card(&self, c: &Card) -> Result<Option<Chain>> {
        if c.origin == Origin::Assistant {
            return Ok(None);
        }
        let home = c.home.clone();
        match self.store.read(move |r| r.chain(&home)).await {
            Ok(chain) => Ok(Some(chain)),
            Err(store::Error::NotFound) => Ok(None),
            Err(e) => Err(Error::refused(format!("the chain of card {}: {e}", c.id))),
        }
    }

    /// Builds the launch spec from what a card is given.
    pub(crate) fn spec_of(
        &self,
        c: &Card,
        acc: &library::Session,
        chain: Option<&Chain>,
        resume: &str,
        token: &str,
    ) -> agent::Spec {
        let mcp = agent::MCPServer {
            name: MCP_SERVER_NAME.to_owned(),
            url: self.cfg.mcp_url.clone(),
            headers: [("Authorization".to_owned(), format!("Bearer {token}"))].into(),
        };
        let mut spec = agent::Spec {
            card_id: c.id.clone(),
            cwd: c.cwd.clone(),
            model: acc.model.clone(),
            effort: acc.effort.clone(),
            account: c.account.clone(),
            mode: c.mode.clone(),
            resume: resume.to_owned(),
            mcp: vec![mcp],
            allowed_tools: vec![format!("mcp__{MCP_SERVER_NAME}"), format!("mcp__{SEAT_SERVER_NAME}")],
            deny_tools: policy::deny_for(c, chain),
            ask_tools: policy::ask_rules(&self.own_secrets()),
            tools: acc.tools.clone(),
            skills: acc.skills.clone(),
            system_append: format!("{}\n\n{TALK_BRIEF}\n\n{OTHER_COMMIT_BRIEF}", acc.instructions)
                .trim()
                .to_owned(),
            builtin_agents: Some(builtin_agents_of(c)),
            telemetry: (!self.cfg.otlp_url.is_empty()).then(|| agent::Telemetry {
                endpoint: self.cfg.otlp_url.clone(),
                token: token.to_owned(),
            }),
            ..agent::Spec::default()
        };
        for plugin in &acc.plugins {
            let dir = self.capabilities.plugin_dir(plugin);
            if !dir.is_empty() {
                spec.plugins.push(dir);
            }
        }
        spec
    }
}
