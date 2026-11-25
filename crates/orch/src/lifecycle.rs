//! The owner's chain actions that need the whole application: create a chain of its kind and start it, its Chair hired from the staff (ADR-0048, ADR-0052). The rest of a chain's lifecycle is the core's
//! (lifecycle.rs in chains).

/// The owner asking for a chain: its goal (his words from the box, or the goal his Assistant wrote from them), and its
/// kind (`work` from the box; the Assistant names another). The rest is Remit's: its Chair titles it in its first turn (set_title); the
/// directory is made for the chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateChain {
    pub intent: String,
    pub kind: model::ChainKind,
    /// The key result of an active objective it works toward (the Assistant sets it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serves: Option<Serves>,
    /// The id of the project the owner named (his board narrowed to it, or the Assistant on his words): set on the chain's
    /// objective as it is stored, refused when no active project has it (ADR-0108).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// The agent a Chair is made from when no other is named (agents.rs).
const DEFAULT_CHAIR: &str = "chair";
use crate::orch::Orchestrator;
use chains::Result;
use chains::apply::Records;
use chains::chain_core::new_id;
use chains::error::Error;
use chains::lifecycle::NewChain;
use chains::lifecycle::or_default_actor;
use chains::request::Request;
use chains::request::Service;
use chains::step::ChainCmd;
use chains::step::StartCmd;
use model::Actor;
use model::Card;
use model::CardStatus;
use model::Chain;
use model::ChainOrigin;
use model::ChainStatus;
use model::Origin;
use model::PlanEntry;
use model::Serves;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The numbers a Chair's hire is held to: a rail, raised by no tool.
const CHAIR_HIRE_CAPS: crew::employee_rules::HireCaps = crew::employee_rules::HireCaps {
    headcount: 20,
    depth: 3,
};

impl Orchestrator {
    pub async fn create_chain(self: &Arc<Self>, req: Request, input: CreateChain) -> Result<Chain> {
        let by = req.by.clone();
        self.act(req, "CreateChain", move |o| async move {
            o.create_chain_as(input, by).await
        })
        .await
    }
    /// Creates the chain the owner asks for, as by (he, or his Assistant on his words).
    pub(crate) async fn create_chain_as(self: &Arc<Self>, input: CreateChain, by: Actor) -> Result<Chain> {
        let ch = self
            .queue_chain(NewChain {
                goal: input.intent,
                kind: input.kind,
                origin: ChainOrigin::Owner,
                by,
                serves: input.serves,
                project: input.project,
                ..NewChain::default()
            })
            .await?;
        self.start_ready().await; // its start begins now if admission allows
        Ok(self.chain(&ch.id).await.unwrap_or(ch))
    }
    /// Gives a queued chain its Chair and the Chair its first message (the goal and its kind's section), in one step: a
    /// chain is never running with a Chair that has nothing to do.
    pub(crate) async fn start_chain(&self, id: &str) -> Result<()> {
        let mut ch = self.chain(id).await?;
        if *ch.status() != ChainStatus::Queued {
            return Ok(());
        }
        let all = self.store.read(|r| r.chains()).await?;
        let chair_model = if self.cfg.chair_model.is_empty() {
            String::new()
        } else {
            self.resolve_model(&self.cfg.chair_model)?
        };
        let chair_effort = chains::failover::resolve_effort(&self.cfg.chair_effort)?;
        let now = self.now();
        let chair = {
            let mut built = Card::default();
            built.id = new_id("c_");
            built.home = ch.id.clone();
            built.agent = DEFAULT_CHAIR.to_owned();
            built.name = "Chair".to_owned();
            built.origin = Origin::Chair;
            built.model = chair_model;
            built.effort = chair_effort;
            built.mode = self.cfg.mode.clone();
            built.cwd = ch.workdir.clone();
            built.plan = Vec::<PlanEntry>::new();
            built.created_at = now;
            built.updated_at = now;
            built.set_status(CardStatus::Starting, now);
            built
        };
        let (chair, hired) = self.chair_seat(&ch, chair).await?;
        let mut seed = format!("Start {} chain.", ch.kind);
        if !ch.capabilities.is_empty() {
            seed.push_str(&format!(" Capabilities: {}.", ch.capabilities.join(", ")));
        }
        chair.id.clone_into(&mut ch.chair_id); // as the Chair's first message reads it
        let mut opening = self.chair_opening(&ch, &all).await?;
        if !self.queued_runs(&ch.id).await?.is_empty() {
            // it begins with a run (a routine's script): its Chair's opening goes with the run's end (end_message)
            opening.clear();
        }
        let by = or_default_actor(ch.by.as_ref());
        let cmd = ChainCmd::Start(StartCmd {
            chair_id: chair.id.clone(),
            opening,
            seed,
            by,
        });
        let w = Records {
            employees: hired.into_iter().collect(),
            created: vec![chair],
            ..Records::default()
        };
        self.apply(&ch.id, cmd, w).await?;
        self.start_queued_runs(&ch.id).await
    }
    /// chair seated: the card takes the id, agent, seat name and a copy of the loadout of the employee that sits in the
    /// Chair seat of ch (`chair_employee_of`), with that employee when it is hired for this start, for the change that
    /// starts the chain to store.
    pub(crate) async fn chair_seat(&self, ch: &Chain, mut chair: Card) -> Result<(Card, Option<model::Employee>)> {
        let (boss, hired) = self.chair_employee_of(ch, &chair).await?;
        chair.name = self.seat_name(&boss).await?;
        chair.employee.clone_from(&boss.id);
        chair.agent.clone_from(&boss.template);
        chair.loadout = Some(boss.loadout.clone()); // the card holds a copy, as a hired worker's does
        Ok((chair, hired.then_some(boss)))
    }
    /// The employee that sits in the Chair seat of ch, and whether it is new. The seat is a role: the key result's own
    /// Chair takes it when the chain serves one; else the oldest active Chair whose ceiling fits, and only when there is none is one hired, by the owner, and kept. A Chair
    /// made for a chain holds what a chair the owner hires holds when he names no ceiling; a verification chain's works at
    /// the ceiling of the judge that opened it, so only a Chair at that very ceiling sits in it.
    async fn chair_employee_of(&self, ch: &Chain, chair: &Card) -> Result<(model::Employee, bool)> {
        let staff = self.store.read(|r| r.employees()).await?;
        let seat = crew::employee_change::chair_ceiling();
        let verifying = chains::schedule::verifying(ch);
        let ceiling = if verifying {
            self.shipping.opener_ceiling(ch).await?
        } else if let Some(seated) = self.key_result_seat(ch, &staff).await {
            return Ok((seated, false));
        } else {
            seat.clone()
        };
        let fits = |c: &model::Ceiling| {
            if verifying {
                crew::employee_rules::ceiling_excess(c, &ceiling).is_none()
                    && crew::employee_rules::ceiling_excess(&ceiling, c).is_none()
            } else {
                crew::employee_rules::ceiling_excess(&seat, c).is_none() // holds what the seat needs
            }
        };
        if let Some(seated) = crew::employee_rules::standing_chair(&staff, fits) {
            return Ok((seated.clone(), false));
        }
        let def = self.shipped_chair().await?;
        let boss = model::Employee {
            id: format!("e_{}", chair.id),
            template: def.id.clone(),
            template_rev: def.rev,
            loadout: def.loadout,
            title: model::Title::Chair,
            reports_to: model::Accountable::owner(),
            hired_by: model::Accountable::owner(),
            status: model::EmployeeStatus::Active,
            ceiling,
            created_at: self.now(),
            ..model::Employee::default()
        };
        Ok((boss, true))
    }
    /// The Chair that answers for the key result ch serves, when it is an employee: it takes the Chair seat of ch.
    async fn key_result_seat(&self, ch: &Chain, staff: &[model::Employee]) -> Option<model::Employee> {
        let s = ch.serves.clone()?;
        let (objective, id) = (s.objective.clone(), s.key_result.clone());
        let kr = self.store.read(move |r| r.key_result(&objective, &id)).await;
        let owner = crew::objective_open::key_result_owner(kr, &s, staff).ok()?;
        if owner.is_owner() {
            return None;
        }
        staff
            .iter()
            .find(|e| e.id == owner.0)
            .filter(|e| e.title == model::Title::Chair)
            .cloned()
    }
    /// The name of the Chair card boss sits in: boss's own name when it has one, else the name of the agent it was
    /// hired from (Chair for the shipped one).
    async fn seat_name(&self, boss: &model::Employee) -> Result<String> {
        if !boss.label.is_empty() {
            return Ok(boss.label.clone());
        }
        Ok(self
            .capabilities
            .agent_for(&boss.template)
            .await?
            .map(|def| def.name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Chair".to_owned()))
    }
    /// The agent a Chair is hired from when no kept Chair can sit: the shipped chair, or a plain one named Chair when it
    /// is not stored.
    async fn shipped_chair(&self) -> Result<model::Agent> {
        Ok(self
            .capabilities
            .agent_for(DEFAULT_CHAIR)
            .await?
            .unwrap_or_else(|| model::Agent {
                id: DEFAULT_CHAIR.to_owned(),
                name: "Chair".to_owned(),
                ..model::Agent::default()
            }))
    }
    /// The worker the Chair chair's hire of i would make, from template def, or the refusal of the hiring rules.
    pub(crate) async fn worker_for(
        &self,
        chair: &Card,
        def: &model::Agent,
        i: &chains::hire_input::HireInput,
    ) -> Result<model::Employee> {
        let hirer_id = chair.employee.clone();
        let hirer = self
            .store
            .read(move |r| r.employee(&hirer_id))
            .await
            .map_err(|err| Error::refused(format!("the Chair is no employee on record: {err}")))?;
        let staff = self.store.read(|r| r.employees()).await?;
        let hire = model::Employee {
            template: i.agent.trim().to_lowercase(),
            template_rev: def.rev,
            title: def.title.unwrap_or(model::Title::Worker),
            reports_to: model::Accountable(hirer.id.clone()),
            hired_by: model::Accountable(hirer.id.clone()),
            status: model::EmployeeStatus::Active,
            ceiling: model::Ceiling {
                effect: hirer.ceiling.effect.clone(),
                grants: hirer.ceiling.grants.clone(),
                tools: hirer.ceiling.tools.clone(),
            },
            created_at: self.now(),
            ..model::Employee::default()
        };
        if let Some(r) = crew::employee_rules::check_hire(&hirer, &hire, &staff, CHAIR_HIRE_CAPS) {
            return Err(r.into());
        }
        let busy = self.busy_of(&staff).await?;
        let authors = self.authors_of(&i.inputs).await?;
        let home = chair.home.clone();
        let ch = self.store.read(move |r| r.chain(&home)).await?;
        let standings = crew::lineage_standing::by_template(self.crew.standings_in(&ch).await?);
        if let Some(r) = crew::employee_rules::check_reuse(&hirer, &i.why, &staff, &busy, &authors, &standings) {
            return Err(r.into());
        }
        Ok(hire)
    }
    /// The employees whose cards inputs are: the work a hire builds on. An input naming no card on record is the hire's to
    /// refuse, not this lookup's.
    async fn authors_of(
        &self,
        inputs: &[chains::hire_input::HireInputRef],
    ) -> Result<std::collections::HashSet<String>> {
        let ids: Vec<String> = inputs.iter().map(|i| i.card_id.clone()).collect();
        self.store
            .read(move |r| {
                let mut authors = std::collections::HashSet::new();
                for id in ids {
                    match r.card(&id) {
                        Ok(c) if !c.employee.is_empty() => {
                            authors.insert(c.employee);
                        }
                        Ok(_) | Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                Ok(authors)
            })
            .await
            .map_err(Into::into)
    }
    /// The employee kept standing that the Chair chair's hire of i puts to work, with why it may not
    /// (`standing_refusal`): the employee read is returned with its refusal, so the hire's other rules judge who was
    /// named, not an empty record.
    pub(crate) async fn standing_for(
        &self,
        chair: &Card,
        i: &chains::hire_input::HireInput,
    ) -> (model::Employee, Result<()>) {
        let hirer_id = chair.employee.clone();
        let hirer = match self.store.read(move |r| r.employee(&hirer_id)).await {
            Ok(h) => h,
            Err(err) => {
                let why = format!("the Chair is no employee on record: {err}");
                return (model::Employee::default(), Err(Error::refused(why)));
            }
        };
        let id = i.employee.trim().to_owned();
        let key = id.clone();
        match self.store.read(move |r| r.employee(&key)).await {
            Ok(emp) => {
                let refused = crew::employee_rules::standing_refusal(&hirer, &emp, i);
                (emp, refused)
            }
            Err(store::Error::NotFound) => (
                model::Employee::default(),
                Err(Error::refused(format!(
                    "employee {id} is not on record: name one of the standing employees list_agents shows"
                ))),
            ),
            Err(err) => (model::Employee::default(), Err(err.into())),
        }
    }
}
