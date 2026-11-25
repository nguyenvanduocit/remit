//! A Chair's hire (hire_agent): a kept employee put to work in the role the Chair gives, or a new worker of an agent,
//! which it justifies beside a kept worker of that agent (ADR-0032, ADR-0052, ADR-0061).

use model::{
    Agent, Card, CardStatus, ChainOrigin, ChainStatus, DecisionKind, Edge, EdgeKind, Employee, Loadout, Origin, Signs,
    Time, by_agent,
};

use crate::Orchestrator;
use capability::accept::non_empty;
use capability::agent_tools::tools_label;
use chains::apply::Records;
use chains::chain_core::new_id;
use chains::failover::{model_label, resolve_effort};
use chains::gate::ERR_GATE_INPUTS;
use chains::handover::handover_brief;
use chains::hire_choice::Picked;
use chains::hire_input::HireInput;
use chains::hire_input::HireInputRef;
use chains::hire_input::Hiring;
use chains::outbox::{Message, PRIORITY_OPENING};
use chains::plan::plan_of;
use chains::policy::named_but_withheld;
use chains::prompts::{Given, opening_title};
use chains::review_depth::{raised_by, tightened};
use chains::step::Decided;
use chains::{Error, Result};
use keys::client_keys::{CLIENT_ENGINEER, ENGINEER_AGENT};

/// A role given for a new hire, whose role is its agent's (`Agent::signs`).
const ERR_ROLE_WITHOUT_EMPLOYEE: &str = "role is given only with employee: a new hire's role comes from its agent";

/// Whom a hire puts to work and as what: a new worker of an agent, stored with its card (fresh), or an employee kept
/// standing.
#[derive(Debug, Clone, Default)]
struct Hiree {
    emp: Employee,
    fresh: bool,
    /// The card's agent: the agent hired, or a standing employee's template.
    agent: String,
    /// The card's name when the Chair gives none.
    name: String,
    signs: Signs,
    loadout: Option<Loadout>,
    model: String,
    effort: String,
}

/// The gate an employee put to work in role closes with a verdict: none for a worker, review for a reviewer, QC for a
/// qc, security for a security reviewer. Pure.
pub(crate) fn signs_of_role(role: &str) -> Result<Signs> {
    match role.trim().to_lowercase().as_str() {
        "" | "worker" => Ok(Signs::None),
        "reviewer" => Ok(Signs::Review),
        "qc" => Ok(Signs::Qc),
        "security" => Ok(Signs::Security),
        _ => Err(Error::refused(format!(
            "role is worker, reviewer, qc or security, not {role:?}"
        ))),
    }
}

/// What hired card to receives: one report given per input card, labelled as the Chair named it, and one handoff edge
/// per giver. Pure but for the edges' ids.
fn handoffs_of(chain_id: &str, to: &str, refs: &[HireInputRef], inputs: &[Card], now: Time) -> (Vec<Given>, Vec<Edge>) {
    let mut reports = Vec::with_capacity(inputs.len());
    let mut edges: Vec<Edge> = Vec::new();
    for (r, from) in refs.iter().zip(inputs) {
        let label = if r.label.is_empty() { "report" } else { r.label.as_str() };
        reports.push(Given {
            from: from.clone(),
            label: label.to_owned(),
        });
        if !edges.iter().any(|e| e.from == from.id) {
            edges.push(Edge {
                id: new_id("e_"),
                chain_id: chain_id.to_owned(),
                from: from.id.clone(),
                to: to.to_owned(),
                kind: EdgeKind::Handoff,
                label: label.to_owned(),
                created_at: now,
            });
        }
    }
    (reports, edges)
}

/// Says, for the owner's record of a hire, which lists the Chair gave in place of its agent's: nothing when the hire holds
/// what its agent holds. Pure.
fn own_choices(i: &HireInput) -> String {
    let mut parts = Vec::new();
    if let Some(tools) = &i.tools {
        parts.push(format!("tools: {}", tools_label(Some(tools))));
    }
    if let Some(skills) = &i.skills {
        parts.push(format!("skills: {}", names_label(skills)));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!(" holding {}", parts.join("; "))
}

/// What the decision says of the model code chose and the effort a decider suggested, after the hire: ", model chosen by
/// Jev 1.14, 82% sure". Pure.
fn chosen_by(p: &Picked) -> String {
    if p.said.is_empty() {
        String::new()
    } else {
        format!(", {}", p.said)
    }
}

/// names joined, or "none". Pure.
fn names_label(names: &[String]) -> String {
    if names.is_empty() {
        return "none".to_owned();
    }
    names.join(", ")
}

/// a, else b when a is empty.
fn or<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.is_empty() { b } else { a }
}

impl Orchestrator {
    /// Gives work to a new card in chair's chain: a new worker of the agent i names, stored with the card, or the standing
    /// employee i names, put to work in the role i gives with no employee made. The card, its employee when fresh, its
    /// handoffs, the decision and its brief are stored in one change.
    pub async fn hire(&self, chair: &Card, i: HireInput) -> Result<Card> {
        let h = self.hiring(chair, i, true).await?;
        let c = h.card.clone();
        let brief = Message {
            title: opening_title(&c),
            text: handover_brief(
                &h.chain,
                &c,
                &h.reports,
                &h.decided,
                "",
                &self.cfg.principles,
                &self.cfg.language,
            ),
            by: by_agent(&chair.id),
            priority: PRIORITY_OPENING,
            ..Message::default()
        };
        let w = Records {
            employees: h.employees,
            created: vec![c.clone()],
            edges: h.edges,
            decisions: vec![h.decision],
            ..Records::default()
        };
        if let Err(err) = self.send(&c, brief, w).await {
            if let Some(chosen) = &c.chosen {
                self.hire_unused(chosen).await;
            }
            return Err(err);
        }
        Ok(c)
    }

    /// The hire of i, built and checked against every hiring rule, nothing stored, with the model and effort as i, the
    /// agent and the Chair leave them: no decider is asked, so no judgment is stored. A hand-over, a workflow's hire and
    /// a duel's two sides are built so; only the Chair's own `hire` asks the deciders.
    pub(crate) async fn hiring_of(&self, chair: &Card, i: HireInput) -> Result<Hiring> {
        self.hiring(chair, i, false).await
    }

    /// `hiring_of`, asking the deciders for the model i leaves unnamed only when choose.
    async fn hiring(&self, chair: &Card, i: HireInput, choose: bool) -> Result<Hiring> {
        if chair.origin != Origin::Chair {
            return Err(Error::refused("only the Chair can hire"));
        }
        if i.brief.trim().is_empty() || (i.agent.trim().is_empty() && i.employee.trim().is_empty()) {
            return Err(Error::refused("brief is required, and agent (or employee)"));
        }
        let (mut h, def, broken) = self.hiree_of(chair, &i).await?;
        let def = def.unwrap_or_default();
        if !broken.is_empty() {
            return Err(Error::refused(broken.join("\n")));
        }
        let home = chair.home.clone();
        let chain = self.store.read(move |r| r.chain(&home)).await?;
        if *chain.status() == ChainStatus::Done {
            return Err(Error::refused("the chain is finished"));
        }
        if chain.origin == ChainOrigin::Client && h.agent == ENGINEER_AGENT {
            return Err(Error::refused(CLIENT_ENGINEER));
        }
        let mut inputs = Vec::with_capacity(i.inputs.len());
        for r in &i.inputs {
            inputs.push(self.team_card(&chain.id, &r.card_id).await?);
        }
        let name = or(&i.name, &h.name).to_owned();
        let now = self.now();
        let card_id = new_id("c_");
        let decided = self.chain_decided(&chain.id).await?;
        // Last, once nothing can refuse the hire: a judgment is stored for it. A gate signer runs on the judges' model
        // (hiree_of): a gate is a rail, no decider lowers it.
        let picked = if choose && h.signs == Signs::None {
            self.pick_for_hire(&card_id, &chain, &def, &i).await
        } else {
            Picked::default()
        };
        if let Some(m) = &picked.model {
            h.model.clone_from(m);
        }
        let mut c = {
            let mut built = Card::default();
            built.id = card_id.clone();
            built.home = chain.id.clone();
            built.employee = h.emp.id.clone();
            built.agent = h.agent.clone();
            built.signs = h.signs;
            built.loadout = h.loadout.clone();
            built.name = name.clone();
            built.origin = Origin::Hired;
            built.why = i.why.clone();
            built.brief = i.brief.clone();
            built.model = h.model.clone();
            built.effort = h.effort.clone();
            built.chosen = picked.chosen.clone();
            built.mode = self.cfg.mode.clone();
            built.cwd = or(&i.cwd, &chain.workdir).to_owned();
            built.plan = plan_of(&i.plan);
            built.deliverable = i.deliverable.trim().to_owned();
            built.criteria = non_empty(&i.criteria);
            built.created_at = now;
            built.updated_at = now;
            built.set_status(CardStatus::Starting, now);
            built
        };
        let mut hired = Vec::new();
        let what = if h.fresh {
            let mut emp = h.emp.clone();
            emp.id = format!("e_{card_id}");
            emp.loadout = h.loadout.clone().unwrap_or_default();
            c.employee.clone_from(&emp.id);
            hired.push(emp);
            format!(
                "Hire {name} ({}, {}){}{}.",
                c.agent,
                model_label(&c.model, &c.effort),
                own_choices(&i),
                chosen_by(&picked)
            )
        } else {
            // The card's name is the Chair's label for the work; the owner's record names who does it.
            format!(
                "Put {} to work ({}, {}, {}){}{}.",
                h.emp.shown(),
                c.agent,
                c.signs.gate().map_or("worker", |g| g.as_str()),
                model_label(&c.model, &c.effort),
                own_choices(&i),
                chosen_by(&picked)
            )
        };
        // A hire is the Chair's decision; only what the new agent receives is drawn, as handoffs, one per giver. The card,
        // its brief, the decision and the handoffs are stored together: a restart's brief is rebuilt from the handoffs.
        let (reports, edges) = handoffs_of(&chain.id, &c.id, &i.inputs, &inputs, now);
        let decision = Decided {
            kind: DecisionKind::Hire,
            text: what,
            why: i.why.clone(),
            card_id: c.id.clone(),
            by: by_agent(&chair.id),
        };
        Ok(Hiring {
            card: c,
            employees: hired,
            reports,
            edges,
            decision,
            chain,
            decided,
        })
    }

    /// Every hiring rule the Chair's hire of i breaks (`hiree_of`), and a tool it lists that its chain withholds, so one
    /// retry of hire_agent fixes them all.
    pub(crate) async fn hire_refusals(&self, chair: &Card, i: &HireInput) -> Result<Vec<String>> {
        let (h, _, mut broken) = self.hiree_of(chair, i).await?;
        if let Some(l) = &h.loadout {
            let home = self.chain(&chair.home).await?;
            let refusal = named_but_withheld(Some(&home), l.tools.as_deref());
            if !refusal.is_empty() {
                broken.push(refusal);
            }
        }
        Ok(broken)
    }

    /// Whom the Chair's hire of i puts to work, with every hiring rule the hire breaks: who works, in which role, with the
    /// inputs a reviewer or QC judges, holding what and on which model and effort. An error is a failure to read what the rules need.
    async fn hiree_of(&self, chair: &Card, i: &HireInput) -> Result<(Hiree, Option<Agent>, Vec<String>)> {
        let (mut h, def, mut broken) = self.who_works(chair, i).await?;
        if h.signs != Signs::None && i.inputs.is_empty() {
            // A reviewer or QC judges someone's work; naming it draws that link and hands the author's report over.
            broken.push(ERR_GATE_INPUTS.to_owned());
        }
        let Some(def) = def else {
            return Ok((h, None, broken));
        };
        let base = if h.fresh { &def.loadout } else { &h.emp.loadout };
        match self.loadout_for(base, i).await {
            Ok(l) => h.loadout = Some(l),
            Err(err) => broken.push(err.to_string()),
        }
        // a card that signs a gate grades the team's work, so it runs on the judges' model, the strongest (ADR-0102)
        let want = if h.signs != Signs::None && !self.cfg.judge_model.is_empty() {
            self.cfg.judge_model.clone()
        } else {
            or(i.named_model(&def), &chair.model).to_owned()
        };
        match self.resolve_model(&want) {
            Ok(m) => h.model = m,
            Err(err) => broken.push(err.to_string()),
        }
        match resolve_effort(or(i.named_effort(&def), &chair.effort)) {
            Ok(e) => h.effort = e,
            Err(err) => broken.push(err.to_string()),
        }
        if h.signs != Signs::None {
            // a decider may have found the change risky: its judges think at least as hard as that asks (ADR-0118)
            let home = chair.home.clone();
            let landings = self.store.read(move |r| r.chain_landings(&home)).await?;
            // a judge hired in a worktree judges that repository's change; any other judges the highest asked
            let repo = self
                .chain(&chair.home)
                .await?
                .repos
                .iter()
                .find(|rw| !i.cwd.is_empty() && rw.path == i.cwd)
                .map(|rw| rw.repo.clone());
            h.effort = tightened(&h.effort, raised_by(&landings, repo.as_deref()));
        }
        Ok((h, Some(def), broken))
    }

    /// Whom the Chair's hire of i puts to work, the agent its card runs (none for an employee who may not be put to
    /// work) and the rules who it is breaks: the standing employee i names, in the role i gives, else a new worker of the
    /// agent i names, whose role comes from that agent.
    async fn who_works(&self, chair: &Card, i: &HireInput) -> Result<(Hiree, Option<Agent>, Vec<String>)> {
        let mut broken = Vec::new();
        if i.employee.trim().is_empty() {
            let def = self.capabilities.agent_for(&i.agent).await?.unwrap_or_default();
            if !i.role.is_empty() {
                broken.push(ERR_ROLE_WITHOUT_EMPLOYEE.to_owned());
            }
            let worker = match self.worker_for(chair, &def, i).await {
                Ok(w) => w,
                Err(err) => {
                    broken.push(err.to_string());
                    Employee::default()
                }
            };
            let h = Hiree {
                emp: worker,
                fresh: true,
                agent: i.agent.trim().to_lowercase(),
                name: i.agent.clone(),
                signs: def.signs,
                ..Hiree::default()
            };
            return Ok((h, Some(def), broken));
        }
        let (emp, refused) = self.standing_for(chair, i).await;
        let refused = match refused {
            Ok(()) => false,
            Err(err) => {
                broken.push(err.to_string());
                true
            }
        };
        let signs = signs_of_role(&i.role).unwrap_or_else(|err| {
            broken.push(err.to_string());
            Signs::None
        });
        let name = if emp.label.is_empty() {
            emp.shown()
        } else {
            emp.label.clone()
        };
        let h = Hiree {
            agent: emp.template.clone(),
            name,
            signs,
            emp,
            ..Hiree::default()
        };
        if refused {
            return Ok((h, None, broken));
        }
        let def = self.capabilities.agent_for(&h.emp.template).await?.unwrap_or_default();
        Ok((h, Some(def), broken))
    }

    /// The loadout a hire holding base gets, or what is wrong with it: a copy of base (its agent's, or a standing
    /// employee's), with each list the Chair gave in place of base's. It is a copy, so what base holds later reaches only
    /// later hires.
    async fn loadout_for(&self, base: &Loadout, i: &HireInput) -> Result<Loadout> {
        let mut l = base.clone();
        if let Some(tools) = &i.tools {
            l.tools = Some(tools.clone());
        }
        if let Some(skills) = &i.skills {
            l.skills.clone_from(skills);
        }
        let lib = self.capabilities.library_of().await?;
        ::library::validate_loadout(&l, &lib).map_err(|p| Error::refused(format!("what the agent holds: {p}")))?;
        Ok(l)
    }
}
