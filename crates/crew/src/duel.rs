//! Duels between lineages (docs/research-agent-learning/design.md, D9 and D10). The Chair contests a worker's card
//! (contest_card, the capability duels): a kept employee hired from another template works the same brief on a sibling
//! card. Neither report waits for the Chair while the duel stands; once both have reported, a judge Remit starts picks one
//! (duel_judge.rs), the winner's report then waits for accept_work, and both employees read the pick. The owner may
//! overturn it. Each kind of work's duels rate its lineages (duel_rating.rs), which hire_agent's staff list shows, and
//! pause when the owner overturns too many of the judge's picks there.

use std::sync::Arc;

use std::collections::HashMap;
use std::path::{MAIN_SEPARATOR, Path};

use model::{
    Card, CardRole, CardStatus, Chain, ChainStatus, Duel, DuelSide, EdgeKind, Employee, JudgeKind, Origin, by_agent,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Crew;
use crate::duel_judge::PickInput;
use crate::duel_rating::{agreement_of, work_duels_of};
use crate::lineage_standing::{by_template, rank_order, standings_of, unsettled_why};
use chains::apply::{Check, Records};
use chains::chain_core::new_id;
use chains::handover::handover_brief;
use chains::hire_input::{HireInput, HireInputRef};
use chains::lifecycle::holds_open;
use chains::outbox::{Message, PRIORITY_OPENING, by_remit, new_message};
use chains::prompts::opening_title;
use chains::schedule::at_work;
use chains::step::CardEdit;
use chains::tools::{Effect, SelfCard, Tools};
use chains::{Error, Result};
use wire::app::{LineageStanding, WorkDuels};

/// What contest_card takes.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContestInput {
    /// the worker's card to contest: its brief, deliverable, criteria and the reports it was handed go to a kept employee
    /// of another lineage
    #[serde(default)]
    pub card_id: String,
    /// one or two short sentences for the owner, in his language: why this work deserves a second lineage's attempt
    #[serde(default)]
    pub why: String,
}

/// What a contest in a chain stands on: the chain, its kind of work, every duel, and the standings of the lineages in that
/// kind of work, best first.
struct Ground {
    chain: Chain,
    work: String,
    duels: Vec<Duel>,
    standings: Vec<LineageStanding>,
}

/// A contest opened, or a rule's refusal of it in words for the Chair: a duel by default reads a refusal as why none runs.
type Contested = std::result::Result<(Card, Employee), String>;

/// The contest of card card_id by rival's lineage, checked again in the transaction that opens its duel
/// (`Records::checks`): the card as stored may be contested (`contest_refusal`), and rival holds no card open. Two
/// contests read before either was stored so open one duel.
fn contested_once(card_id: String, rival: String) -> Check {
    Arc::new(move |tx| {
        let c = tx.card(&card_id)?;
        let (ch, duels) = (tx.chain(&c.home)?, tx.chain_duels(&c.home)?);
        if let Some(refusal) = contest_refusal(&c, &ch, &duels) {
            return Err(Error::refused(refusal));
        }
        if tx.employee_cards(&rival)?.iter().any(holds_open) {
            return Err(Error::refused(format!(
                "the rival worker {rival} took a card meanwhile: contest card {card_id} again"
            )));
        }
        Ok(())
    })
}

/// Why card c of chain ch may not be contested, none when it may: only a worker's own work, not a judge's, not work on its
/// way to another employee, not a card already in a duel, and not work in a repository's worktree, where the sibling would
/// write the same files. Pure.
pub(crate) fn contest_refusal(c: &Card, ch: &Chain, duels: &[Duel]) -> Option<String> {
    if c.employee.is_empty() {
        return Some(format!(
            "card {} has no employee at work in it: only an employee's work is contested",
            c.id
        ));
    }
    if let Some(role) = c.signs.verdict_role() {
        return Some(format!(
            "card {} judges others' work as {role}: contest a worker's card",
            c.id
        ));
    }
    if c.handover.is_some() {
        return Some(format!(
            "card {} is handing its work over: contest the card that takes it",
            c.id
        ));
    }
    if duels.iter().any(|d| d.side(&c.id).is_some()) {
        return Some(format!("card {} is in a duel already: a card is contested once", c.id));
    }
    let in_worktree = |path: &str| c.cwd == path || c.cwd.starts_with(&format!("{path}{MAIN_SEPARATOR}"));
    ch.repos.iter().find(|r| !r.path.is_empty() && in_worktree(&r.path)).map(|r| {
        format!(
            "card {} works in the worktree of {}, where a sibling would write the same files: contest work done in the chain's directory",
            c.id, r.repo
        )
    })
}

impl Crew {
    /// contest_card: a sibling of chair's team card i.card_id, with its brief, deliverable, criteria and handed reports,
    /// for the free kept worker of another lineage that stands best in the chain's kind of work (the oldest among equals),
    /// and the duel between them, stored with the hire. The contested card's report, if it waits, waits for the judge
    /// instead.
    pub(crate) async fn contest_card(&self, chair: &Card, i: &ContestInput) -> Result<(Card, Employee)> {
        if chair.origin != Origin::Chair {
            return Err(Error::refused("only the Chair contests a card"));
        }
        if i.why.trim().is_empty() {
            return Err(Error::refused(
                "why is required: why this work deserves a second lineage's attempt",
            ));
        }
        let c = self.team_card(&chair.home, &i.card_id).await?;
        let g = self.ground_of(chair).await?;
        self.contest(chair, &c, &g, &i.why).await?.map_err(Error::refused)
    }

    /// The ground of a contest in chair's chain.
    async fn ground_of(&self, chair: &Card) -> Result<Ground> {
        let home = chair.home.clone();
        let chain = self.store.read(move |r| r.chain(&home)).await?;
        let work = Self::work_of(&chain);
        let (outsides, duels) = self.standings_all().await?;
        let standings = standings_of(&outsides, &duels, &work, self.now());
        Ok(Ground {
            chain,
            work,
            duels,
            standings,
        })
    }

    /// Opens a duel on chair's team card c on ground g, for why; a rule that refuses one says why in the inner error.
    async fn contest(&self, chair: &Card, c: &Card, g: &Ground, why: &str) -> Result<Contested> {
        if let Some(refusal) = contest_refusal(c, &g.chain, &g.duels) {
            return Ok(Err(refusal));
        }
        let settings = self.store.read(|r| r.settings()).await?;
        if let Some(paused) = agreement_of(&g.duels, &g.work).paused_why(settings.duel_agreement()) {
            return Ok(Err(paused));
        }
        let rival = match self.rival_for(chair, c, &by_template(g.standings.clone())).await? {
            Ok(rival) => rival,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let d = Duel {
            id: new_id("du_"),
            chain: g.chain.id.clone(),
            work: g.work.clone(),
            created_at: self.now(),
            ..Duel::default()
        };
        let sibling = self.open_sibling(chair, c, &rival, why, d).await?;
        Ok(Ok((sibling, rival)))
    }

    /// Runs a second lineage on card c, just hired by chair, when the chain's kind of work is unsettled (`unsettled_why`):
    /// variation by default, without the Chair asking (ADR-0069). It says what it did for the Chair, or why no second
    /// lineage runs; "" in a settled kind of work. The hire stands whatever happens here, so an error is said to the
    /// Chair in what it reads back (and the owner sees it there), not returned.
    pub async fn duel_by_default(&self, chair: &Card, c: &Card) -> String {
        let g = match self.ground_of(chair).await {
            Ok(g) => g,
            Err(err) => {
                return format!(
                    " Whether a second lineage runs on card {} could not be weighed: {err}.",
                    c.id
                );
            }
        };
        let Some(unsettled) = unsettled_why(&g.work, &g.standings) else {
            return String::new();
        };
        let why = format!("Variation by default: {unsettled}.");
        match self.contest(chair, c, &g, &why).await {
            Ok(Ok((sibling, rival))) => format!(
                " The kind of work {} is unsettled ({unsettled}), so {} works the same brief on card {}, a second lineage: a judge picks one of the two reports once both have reported, and the winner's then waits for accept_work.",
                g.work,
                rival.shown(),
                sibling.id
            ),
            Ok(Err(refused)) => format!(
                " The kind of work {} is unsettled ({unsettled}), but no second lineage runs on card {}: {refused}.",
                g.work, c.id
            ),
            Err(err) => format!(
                " The kind of work {} is unsettled ({unsettled}), but the second lineage on card {} could not start: {err}.",
                g.work, c.id
            ),
        }
    }

    /// Where lineages stand in every kind of work: each lineage's standing from the outside with its evidence and its duel
    /// rating, whether the kind of work is settled, and the judge's agreement with the owner with the pause it sets, for
    /// the owner's screens and the coach.
    pub async fn duel_standings(&self) -> Result<Vec<WorkDuels>> {
        let (outsides, duels) = self.standings_all().await?;
        let settings = self.store.read(|r| r.settings()).await?;
        Ok(work_duels_of(&outsides, &duels, settings.duel_agreement(), self.now()))
    }

    /// The kept worker chair may put to work against card c: hired from another template, holding no card open, the one
    /// whose lineage stands best (`rank_order`; one with no standing reads as even), the oldest among equals. None names
    /// why in the inner error.
    async fn rival_for(
        &self,
        chair: &Card,
        c: &Card,
        standings: &HashMap<String, LineageStanding>,
    ) -> Result<std::result::Result<Employee, String>> {
        let mut others = self.standing_staff(chair).await?;
        others.retain(|e| e.template != c.agent);
        if others.is_empty() {
            return Ok(Err(format!(
                "no kept worker hired from an agent other than {} can take card {}: hire a worker of another agent with hire_agent, then contest",
                c.agent, c.id
            )));
        }
        let busy = self.busy_of(&others).await?;
        let mut free: Vec<Employee> = others.iter().filter(|e| !busy.contains(&e.id)).cloned().collect();
        if free.is_empty() {
            let names: Vec<String> = others.iter().map(Employee::shown).collect();
            return Ok(Err(format!(
                "every kept worker of another lineage holds a card open ({}): contest card {} once one is free",
                names.join(", "),
                c.id
            )));
        }
        let even = LineageStanding::default();
        free.sort_by(|a, b| {
            rank_order(
                standings.get(&a.template).unwrap_or(&even),
                standings.get(&b.template).unwrap_or(&even),
            )
        });
        Ok(free.into_iter().next().ok_or_else(|| "no free worker".to_owned()))
    }

    /// Hires rival onto a sibling of card c, in a directory of its own under the chain's, and stores duel d between them
    /// with the hire: the sibling's card, brief, handoffs and decision, the duel, and the contested report no longer
    /// waiting.
    async fn open_sibling(&self, chair: &Card, c: &Card, rival: &Employee, why: &str, mut d: Duel) -> Result<Card> {
        let (home, to) = (c.home.clone(), c.id.clone());
        let (edges, ch) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.edges(&home)?, r.chain(&home)?)))
            .await?;
        let inputs = edges
            .into_iter()
            .filter(|e| e.to == to && e.kind == EdgeKind::Handoff)
            .map(|e| HireInputRef {
                card_id: e.from,
                label: e.label,
            })
            .collect();
        let dir = Path::new(&ch.workdir).join(&d.id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| Error::refused(format!("the sibling's directory: {e}")))?;
        let i = HireInput {
            employee: rival.id.clone(),
            name: format!("{} · contest", c.name),
            brief: c.brief.clone(),
            why: why.to_owned(),
            deliverable: c.deliverable.clone(),
            criteria: c.criteria.clone(),
            cwd: dir.to_string_lossy().into_owned(),
            inputs,
            ..HireInput::default()
        };
        let mut h = self.domains()?.hiring_of(chair, i).await?;
        let s = h.card.clone();
        d.sides = [
            DuelSide {
                card: c.id.clone(),
                employee: c.employee.clone(),
                template: c.agent.clone(),
            },
            DuelSide {
                card: s.id.clone(),
                employee: rival.id.clone(),
                template: rival.template.clone(),
            },
        ];
        h.decision.text = format!("Contest {}: {} works the same brief.", c.name, rival.shown());
        let brief = Message {
            title: opening_title(&s),
            text: handover_brief(
                &h.chain,
                &s,
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
            checks: vec![contested_once(c.id.clone(), rival.id.clone())],
            employees: h.employees,
            created: vec![s.clone()],
            edges: h.edges,
            decisions: vec![h.decision],
            duels: vec![d],
            cards: vec![CardEdit::new(&c.id, |c| c.awaiting_review = false)],
            ..Records::default()
        };
        self.send(&s, brief, w).await?;
        Ok(s)
    }

    /// Moves duel d on after one of its sides reported: once both have, a judge is hired; a side that ended without a
    /// report ends the duel without one. A duel moved on already is left as it is.
    pub async fn duel_reported(&self, d: &Duel) -> Result<()> {
        let id = d.id.clone();
        let d = self.duel_of(&d.chain, move |x| x.id == id).await?;
        if !d.judge.is_empty() || !d.ended.is_empty() {
            return Ok(());
        }
        let ids = d.sides.clone().map(|s| s.card);
        let sides = self
            .store
            .read(move |r| -> Result<[Card; 2]> {
                let [a, b] = &ids;
                Ok([r.card(a)?, r.card(b)?])
            })
            .await?;
        if sides.iter().any(|c| holds_open(c) || at_work(c)) {
            return Ok(()); // the other is still at it
        }
        let moved = match sides
            .iter()
            .find(|c| c.status() == &CardStatus::Failed || c.output.is_empty())
        {
            Some(c) => {
                let why = format!("card {} ({}) ended without a report", c.id, c.name);
                self.end_duel(d, &why, Records::default()).await
            }
            None => self.hire_duel_judge(d, sides).await,
        };
        match moved {
            // the duel is written over the revision read: the other side's report moved it on first
            Err(err) if err.is_conflict() => Ok(()),
            other => other,
        }
    }

    /// The duel of chain chain_id that is matches.
    pub(crate) async fn duel_of(&self, chain_id: &str, is: impl Fn(&Duel) -> bool + Send + 'static) -> Result<Duel> {
        let id = chain_id.to_owned();
        let duels = self.store.read(move |r| r.chain_duels(&id)).await?;
        duels
            .into_iter()
            .find(|d| is(d))
            .ok_or_else(|| Error::refused(format!("no such duel in chain {chain_id}: not found")))
    }

    /// Ends duel d without a pick, for why, with w: each side that reported then waits for the Chair's accept_work, and
    /// the Chair hears why. A finished chain is told nothing.
    pub(crate) async fn end_duel(&self, mut d: Duel, why: &str, mut w: Records) -> Result<()> {
        why.clone_into(&mut d.ended);
        let ids: Vec<String> = d.sides.iter().map(|s| s.card.clone()).collect();
        let chain_id = d.chain.clone();
        let (sides, ch) = self
            .store
            .read(move |r| -> Result<(Vec<Card>, Chain)> {
                let sides = ids.iter().map(|id| r.card(id)).collect::<store::Result<Vec<_>>>()?;
                Ok((sides, r.chain(&chain_id)?))
            })
            .await?;
        let mut waiting = Vec::new();
        for c in sides
            .iter()
            .filter(|c| !c.output.is_empty() && c.status() != &CardStatus::Failed)
        {
            w.cards.push(CardEdit::new(&c.id, |c| c.awaiting_review = true));
            waiting.push(c.id.clone());
        }
        let duel_id = d.id.clone();
        w.duels.push(d);
        if *ch.status() == ChainStatus::Done {
            self.commit(None, None, w).await?;
            return Ok(());
        }
        let chair_id = ch.chair_id.clone();
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        let mut text = format!("[duel {duel_id} ends without a pick] {why}.");
        if !waiting.is_empty() {
            text.push_str(&format!(
                " Judge the report(s) yourself with accept_work: {}.",
                waiting.join(", ")
            ));
        }
        let m = new_message(&chair.id, "The duel ends without a pick", &text, by_remit());
        self.send(&chair, m, w).await?;
        Ok(())
    }

    /// Gives the tools of the capability duels: contest_card to the Chair, pick_report to a duel's judge.
    pub fn add_duel_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, me: &SelfCard) {
        match CardRole::of(card) {
            CardRole::Chair => {
                let me = me.clone();
                t.serving(self).add(
                    "contest_card", Effect::Writes,
                    "Contest a worker's card: a kept worker hired from another agent (another lineage) gets the same brief, deliverable, criteria and handed reports on a card of its own, and once both have reported a judge picks the better report. Neither report waits for accept_work until the pick; then the winner's does. The owner may overturn the pick. Each lineage's rating in this kind of work moves with the pick, and hire_agent's list shows it. Refused when no kept worker of another agent is free, when the card works in a repository's worktree, and while duels of this kind of work are paused because the owner overturned too many picks.",
                    move |o, i: ContestInput| {
                        let me = me.clone();
                        async move {
                            let chair = me.read(&o).await?;
                            let (sibling, rival) = o.contest_card(&chair, &i).await?;
                            Ok(format!(
                                "Contested: {} works the same brief as card {} (card {}, in {}). A judge picks one once both have reported.",
                                rival.shown(),
                                i.card_id,
                                sibling.id,
                                sibling.cwd
                            ))
                        }
                    },
                );
            }
            CardRole::Judge(JudgeKind::Duel) => {
                let me = me.clone();
                t.serving(self).add(
                    "pick_report", Effect::Writes,
                    "Record your pick of the duel: winner is A or B, as your brief names the reports, and why says what the winner gives the customer that the other does not, with the evidence you checked. One pick per duel.",
                    move |o, i: PickInput| {
                        let me = me.clone();
                        async move {
                            let judge = me.read(&o).await?;
                            o.pick_report(&judge, i).await?;
                            Ok("Recorded: your pick. End your turn.".to_owned())
                        }
                    },
                );
            }
            CardRole::Worker
            | CardRole::Spawned
            | CardRole::Assistant
            | CardRole::Judge(JudgeKind::Goal | JudgeKind::Second | JudgeKind::Round) => {}
        }
    }
}
