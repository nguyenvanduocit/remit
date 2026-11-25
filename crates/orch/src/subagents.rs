//! The subagent types a session is offered (ADR-0073): the employees a card may run as subagents, each with the seat it
//! works in and the tools that seat is served. Which seats a session holds is the core's (subagents.rs in chains).

use crate::orch::Orchestrator;
use chains::Result;
use chains::policy::ceiling_read_only;
use chains::spawn::Spawnee;
use chains::subagents::SEAT_SERVER_NAME;
use chains::subagents::employee_handle;
use chains::tools::CardServer;
use chains::tools::MCP_SERVER_NAME;
use chains::tools::SelfCard;
use chains::tools::Tools;
use model::Card;
use model::Employee;
use model::EmployeeStatus;
use model::Origin;
use model::Title;
use std::collections::BTreeMap;
use std::sync::Arc;

/// How the spawning model reads employee e when it chooses a type: who it is and what it is strong in.
fn subagent_description(e: &Employee) -> String {
    let mut d = format!("{}, a standing employee", e.shown());
    if !e.scope.is_empty() {
        d.push_str(&format!(", answering for {}", e.scope));
    }
    if !e.strengths.is_empty() {
        d.push_str(&format!("; strong in: {}", e.strengths));
    }
    d + ". Give it work that fits; its report comes back to you."
}

/// What employee e reads at work as a subagent of card c: its own instructions, and where it stands.
fn seat_prompt(e: &Employee, c: &Card, instructions: &str) -> String {
    let (me, parent) = (e.shown(), &c.name);
    let place = format!(
        "You are {me}, at work as a subagent of {parent}. Remit's tools for you are mcp__{SEAT_SERVER_NAME}__*: your memory (search_memory), your record (my_work, read_my_card) and what {parent} may read. Hiring, routing, asking the owner or the Chair, accepting and finishing are {parent}'s: say what is needed in your report."
    );
    if instructions.trim().is_empty() {
        place
    } else {
        format!("{instructions}\n\n{place}")
    }
}

/// tools within parent's: none in either is every tool, so the other decides.
fn narrow_tools(tools: Option<&[String]>, parent: Option<&[String]>) -> Option<Vec<String>> {
    match (tools, parent) {
        (tools, None) => tools.map(<[String]>::to_vec),
        (None, Some(parent)) => Some(parent.to_vec()),
        (Some(tools), Some(parent)) => Some(tools.iter().filter(|t| parent.contains(t)).cloned().collect()),
    }
}

impl Orchestrator {
    /// The employees card c's session is offered as subagent types: a Chair's are those it may put to work
    /// (`standing_staff`), the Assistant's every active worker, as it holds the owner's say; a worker and a judge are
    /// offered none.
    async fn subagent_staff(&self, c: &Card) -> Result<Vec<Employee>> {
        match c.origin {
            Origin::Chair => self.crew.standing_staff(c).await,
            Origin::Assistant => {
                let mut staff = self.store.read(|r| r.employees()).await?;
                staff.retain(|e| e.status == EmployeeStatus::Active && e.title == Title::Worker);
                Ok(staff)
            }
            _ => Ok(Vec::new()),
        }
    }
    /// The types card c's session, holding acc and reaching Remit with run token token, is offered beside the built-ins
    /// it keeps, by handle, and the plugins their skills come from: each employee `subagent_staff` names, with a seat token
    /// made for it under token. An empty token (a spec read for show) makes none and gives the types no server.
    pub(crate) async fn subagent_types(
        &self,
        c: &Card,
        acc: &library::Session,
        token: &str,
    ) -> Result<(BTreeMap<String, agent::AgentType>, Vec<String>)> {
        let staff = self.subagent_staff(c).await?;
        if staff.is_empty() {
            return Ok((BTreeMap::new(), Vec::new()));
        }
        let mut input = library::Input {
            kind: library::CardKind::Worker,
            library: self.capabilities.library_of().await?,
            ..library::Input::default()
        };
        if c.origin != Origin::Assistant {
            input.builtin = self.capabilities.builtin_of_chain(c).await?;
        }
        let offered: BTreeMap<String, Spawnee> = staff
            .iter()
            .map(|e| {
                let spawnee = Spawnee {
                    employee: e.id.clone(),
                    agent: e.template.clone(),
                    loadout: Some(e.loadout.clone()),
                };
                (employee_handle(e), spawnee)
            })
            .collect();
        let tokens = if token.is_empty() {
            BTreeMap::new()
        } else {
            self.shared().seats.offer(token, offered)
        };
        let mut types = BTreeMap::new();
        let mut plugins: Vec<String> = Vec::new();
        for e in &staff {
            e.loadout.clone_into(&mut input.loadout);
            let as_worker = library::access(&input);
            let handle = employee_handle(e);
            let seat_token = tokens.get(&handle).map(String::as_str).unwrap_or_default();
            let t = self.subagent_type(c, e, &as_worker, acc.tools.as_deref(), seat_token);
            types.insert(handle, t);
            for p in as_worker.plugins {
                if !plugins.contains(&p) {
                    plugins.push(p);
                }
            }
        }
        Ok((types, plugins))
    }
    /// Employee e as a type card c's session may spawn: what it holds as a worker card (as_worker), its tools narrowed to
    /// the card's (parent_tools, none: every tool), and, given a seat token, the seat's server in place of the card's.
    fn subagent_type(
        &self,
        c: &Card,
        e: &Employee,
        as_worker: &library::Session,
        parent_tools: Option<&[String]>,
        seat_token: &str,
    ) -> agent::AgentType {
        let mut t = agent::AgentType {
            description: subagent_description(e),
            prompt: seat_prompt(e, c, &as_worker.instructions),
            tools: narrow_tools(as_worker.tools.as_deref(), parent_tools),
            skills: as_worker.skills.clone(),
            deny_tools: vec![format!("mcp__{MCP_SERVER_NAME}")],
            ..agent::AgentType::default()
        };
        if ceiling_read_only(&e.ceiling) {
            t.deny_tools.extend(agent::EDIT_TOOLS.map(str::to_owned));
            t.deny_tools.push("Bash".to_owned());
        }
        if !seat_token.is_empty() {
            t.mcp = vec![agent::MCPServer {
                name: SEAT_SERVER_NAME.to_owned(),
                url: self.cfg.mcp_url.clone(),
                headers: [("Authorization".to_owned(), format!("Bearer {seat_token}"))].into(),
            }];
        }
        t
    }
    /// The MCP server of employee at work as a subagent of card parent_id: the employee's own tools (its memory, its
    /// record and its feedback, on its newest child card) and the reads parent's chain gives a worker of it, nothing that
    /// acts for parent. A call to any other tool is refused with where it belongs. None when the card or its chain cannot
    /// be read. The MCP door serves it to the bearer of a seat token (`caller_for_token`).
    pub async fn seat_server_for(self: &Arc<Self>, parent_id: &str, employee: &str) -> Option<CardServer> {
        match self.seat_tools(parent_id, employee).await {
            Ok((t, parent)) => Some(CardServer::of_seat(&self.core, parent_id, t, &parent)),
            Err(err) => {
                tracing::error!(card = %parent_id, employee, %err, "seat mcp tools");
                None
            }
        }
    }
    /// The tools of employee's seat in the session of card parent_id, and the parent's name.
    async fn seat_tools(self: &Arc<Self>, parent_id: &str, employee: &str) -> Result<(Tools, String)> {
        let id = parent_id.to_owned();
        let parent = self.store.read(move |r| r.card(&id)).await?;
        let mut t = Tools::default();
        let current = SelfCard::of_seat(parent_id, employee);
        self.crew.add_feedback_tool(&mut t, &current);
        self.capabilities.add_my_work_tools(&mut t, employee, &current);
        self.crew.add_memory_session_tools(&mut t, employee, &current);
        if parent.origin != Origin::Assistant {
            let home = parent.home.clone();
            let chain = self.store.read(move |r| r.chain(&home)).await?;
            self.add_every_agent_capability_tools(&mut t, &parent, &chain);
        }
        Ok((t, parent.name))
    }
}
