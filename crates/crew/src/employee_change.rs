//! Hiring, retiring and moving an employee onto a newer template (ADR-0032) are the owner's: a hire creates a principal
//! with a ceiling, so it is a rail. The Assistant drafts the change item on his words, and only his own answer (or the
//! Assistant's for him, `answer_change`) applies it. What the item holds is an ask, an intent; apply builds the employee
//! again from the stored employee and template, so what is written is what this code makes now and never a record some
//! agent shaped. An ask that no longer fits the stored records is stale, which the apply reports as moved.

use std::collections::HashMap;
use std::sync::Arc;

use grant::Effect;
use model::{
    Accountable, Actor, Agent, Ceiling, ChangeKind, ChangedField, Employee, EmployeeStatus, InboxItem, InboxKind,
    InboxOption, InboxStatus, KeyResult, Objective, ObjectiveStatus, PendingChange, Signs, Time, Title,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use store::{Reader, Tx};

use crate::Crew;
use crate::employee_rules::judge_outside_tree;
use chains::chain_core::new_id;
use chains::chain_core::optional;
use chains::change_item::{Applied, CHANGE_APPLY, CHANGE_NOT_NOW, Tell, change_detail};
use chains::changes::{Authorship, doc_of, record_change};
use chains::owner_acts::{ACT_APPLY, for_owner};
use chains::tools::{NoInput, Tools, as_json};
use chains::{Error, Result};

/// What a change item of kind employee asks to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum EmployeeAction {
    Hire,
    /// A retired employee comes back as it was, memory included.
    Rehire,
    Retire,
    /// Onto the revision of its template the owner was shown.
    Move,
}

/// What a change item of kind employee asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmployeeAsk {
    pub action: EmployeeAction,
    /// The employee, for every action but hire.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Hire: the proposal (id, label, template, title, scope, and a ceiling when the owner set one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hire: Option<Employee>,
    /// Move: the template revision the owner was shown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub template_rev: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl EmployeeAsk {
    /// The id of the employee the ask is about.
    fn employee(&self) -> &str {
        self.hire.as_ref().map_or(&self.id, |h| &h.id)
    }
}

/// The ceiling the platform gives a judge hired with none: what a judge needs to read the work.
fn judge_ceiling() -> Ceiling {
    Ceiling {
        effect: Effect::Read.to_string(),
        tools: vec!["Read".into(), "Grep".into(), "Glob".into()],
        ..Ceiling::default()
    }
}

/// The ceiling the platform gives a Chair hired with none: what a Chair needs to make the work. A Chair's proposal for
/// an objective never carries a ceiling, so this is what its hire holds; it is also what a Chair seat needs.
pub fn chair_ceiling() -> Ceiling {
    Ceiling {
        effect: Effect::Reversible.to_string(),
        ..Ceiling::default()
    }
}

fn ceiling_set(c: &Ceiling) -> bool {
    !c.effect.is_empty() || !c.grants.is_empty() || !c.tools.is_empty()
}

/// What an ask makes of the stored records: the employee it writes, or why it no longer fits what is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fit {
    Next(Box<Employee>),
    Stale(String),
}

/// The employee ask makes of cur, the one stored (none when none is), and tmpl, the template the ask is about (none when
/// none is stored): `Fit::Stale` says why the ask no longer fits what is stored, and an error why it is refused
/// outright. Pure.
fn employee_after(ask: &EmployeeAsk, cur: Option<&Employee>, tmpl: Option<&Agent>, now: Time) -> Result<Fit> {
    match ask.action {
        EmployeeAction::Hire => {
            let Some(h) = &ask.hire else {
                return Err(Error::refused("a hire names the employee"));
            };
            if let Some(cur) = cur {
                return Ok(Fit::Stale(format!("employee {} is hired already", cur.id)));
            }
            let Some(tmpl) = tmpl else {
                return Ok(Fit::Stale(format!("template {} is not stored", h.template)));
            };
            hired_from(h.clone(), tmpl, now).map(|e| Fit::Next(Box::new(e)))
        }
        EmployeeAction::Rehire | EmployeeAction::Retire => {
            // what it must be for the ask to apply: a retirement needs an active one, a rehire a retired one
            let (want, becomes) = if ask.action == EmployeeAction::Retire {
                (EmployeeStatus::Active, EmployeeStatus::Retired)
            } else {
                (EmployeeStatus::Retired, EmployeeStatus::Active)
            };
            match cur {
                Some(cur) if cur.status == want => Ok(Fit::Next(Box::new(Employee {
                    status: becomes,
                    ..cur.clone()
                }))),
                _ => Ok(Fit::Stale(format!(
                    "employee {} is not {want}: it is {}",
                    ask.id,
                    status_of(cur)
                ))),
            }
        }
        EmployeeAction::Move => moved_onto(ask, cur, tmpl),
    }
}

fn status_of(e: Option<&Employee>) -> String {
    e.map_or_else(|| "not stored".to_owned(), |e| e.status.to_string())
}

fn moved_onto(ask: &EmployeeAsk, cur: Option<&Employee>, tmpl: Option<&Agent>) -> Result<Fit> {
    let Some(cur) = cur.filter(|c| c.status == EmployeeStatus::Active) else {
        return Ok(Fit::Stale(format!(
            "employee {} is not at work: it is {}",
            ask.id,
            status_of(cur)
        )));
    };
    let Some(tmpl) = tmpl else {
        return Ok(Fit::Stale(format!("template {} is not stored", cur.template)));
    };
    if tmpl.rev == cur.template_rev {
        return Ok(Fit::Stale(format!(
            "employee {} holds the current revision ({}) of template {}",
            cur.id, tmpl.rev, tmpl.id
        )));
    }
    if tmpl.rev != ask.template_rev {
        return Ok(Fit::Stale(format!(
            "template {} moved to revision {} since the owner was shown revision {}",
            tmpl.id, tmpl.rev, ask.template_rev
        )));
    }
    template_fits(&cur.title, tmpl)?;
    Ok(Fit::Next(Box::new(Employee {
        template_rev: tmpl.rev,
        loadout: tmpl.loadout.clone(),
        ..cur.clone()
    })))
}

/// The employee the owner hires from tmpl as h proposes. What a hire is, the platform sets: standing, active,
/// reporting to the owner who hired it, the template's loadout and revision as they are now.
fn hired_from(mut h: Employee, tmpl: &Agent, now: Time) -> Result<Employee> {
    if h.id.is_empty() || h.label.trim().is_empty() || h.template.is_empty() {
        return Err(Error::refused(
            "a hire has an id, a label (the owner's name for it) and a template",
        ));
    }
    if h.title != Title::Chair && h.title != Title::Judge {
        return Err(Error::refused(format!(
            "the owner hires chairs and judges from a template, not a {}: an agent worker is hired by a Chair",
            h.title
        )));
    }
    template_fits(&h.title, tmpl)?;
    if !ceiling_set(&h.ceiling) {
        h.ceiling = if h.title == Title::Chair {
            chair_ceiling()
        } else {
            judge_ceiling()
        };
    }
    check_ceiling(&h.title, &h.ceiling)?;
    Ok(Employee {
        id: h.id,
        label: h.label.trim().to_owned(),
        template: tmpl.id.clone(),
        template_rev: tmpl.rev,
        loadout: tmpl.loadout.clone(),
        title: h.title,
        scope: h.scope,
        reports_to: Accountable::owner(),
        hired_by: Accountable::owner(),
        status: EmployeeStatus::Active,
        ceiling: h.ceiling,
        created_at: now,
        ..Employee::default()
    })
}

/// Whether tmpl is one an employee of title is made from: the title it names, if it names one, is that title, and the
/// gate it signs is one that title closes. Only a judge signs the goal; a chair signs nothing.
fn template_fits(title: &Title, tmpl: &Agent) -> Result<()> {
    if let Some(named) = tmpl.title.filter(|named| named != title) {
        return Err(Error::refused(format!(
            "template {} is titled {named}, not {title}",
            tmpl.id
        )));
    }
    if *title == Title::Chair && tmpl.signs != Signs::None {
        return Err(Error::refused(format!(
            "template {} signs {}: a chair signs nothing",
            tmpl.id, tmpl.signs
        )));
    }
    if *title == Title::Judge && tmpl.signs != Signs::None && tmpl.signs != Signs::Goal {
        return Err(Error::refused(format!(
            "template {} signs {}: a judge signs only the goal",
            tmpl.id, tmpl.signs
        )));
    }
    Ok(())
}

/// Whether c is a ceiling an employee of title may hold: its effect and each grant's are real, no grant is stronger than
/// the ceiling, and a judge's is read with no tool that changes files or runs commands.
fn check_ceiling(title: &Title, c: &Ceiling) -> Result<()> {
    let effect: Effect = c.effect.parse().map_err(|e| Error::refused(format!("ceiling: {e}")))?;
    for g in &c.grants {
        let ge: Effect = g
            .effect
            .parse()
            .map_err(|e| Error::refused(format!("ceiling grant on {}/{}: {e}", g.source, g.connection)))?;
        if g.source.is_empty() || g.connection.is_empty() {
            return Err(Error::refused("a ceiling grant names its source and its connection"));
        }
        if ge > effect {
            return Err(Error::refused(format!(
                "ceiling grant on {}/{} is {ge}, above the ceiling's {effect}",
                g.source, g.connection
            )));
        }
    }
    if *title != Title::Judge {
        return Ok(());
    }
    if effect != Effect::Read {
        return Err(Error::refused(format!(
            "a judge's ceiling is read, not {effect}: it cannot change what the team made"
        )));
    }
    if let Some(tool) = c
        .tools
        .iter()
        .find(|t| *t == "Bash" || agent::EDIT_TOOLS.contains(&t.as_str()))
    {
        return Err(Error::refused(format!(
            "a judge's ceiling holds no tool that changes files or runs commands, and {tool} is one"
        )));
    }
    Ok(())
}

/// Who answers for something that names a: the owner when a names no one, or no employee at work; a otherwise.
pub fn answerer(a: &Accountable, staff: &[Employee]) -> Accountable {
    if staff
        .iter()
        .any(|e| e.id == a.as_str() && e.status == EmployeeStatus::Active)
    {
        return a.clone();
    }
    Accountable::owner()
}

/// obj with who answers named everywhere it asks: the objective's owner, and each key result's owner and judge, are the
/// owner wherever they name no employee at work in staff. The stored objective keeps what it was written with; this is
/// how it is read and how a draft is filled.
pub(crate) fn answering_objective(mut obj: Objective, staff: &[Employee]) -> Objective {
    obj.owner = answerer(&obj.owner, staff);
    for kr in &mut obj.key_results {
        answering_key_result(kr, staff);
    }
    obj
}

/// kr with who answers for it and who judges it read as `answerer` reads them.
pub(crate) fn answering_key_result(kr: &mut KeyResult, staff: &[Employee]) {
    kr.owner = answerer(&kr.owner, staff);
    kr.judge = answerer(&kr.judge, staff);
}

/// Whether every employee obj names is at work in staff under the title its field asks for: a chair owns, a judge
/// judges. An empty field and the owner always do. Every field that does not is named.
pub(crate) fn names_who_answers(obj: &Objective, staff: &[Employee]) -> Result<()> {
    let mut errs: Vec<String> = answers_as(staff, "the objective's owner".to_owned(), &obj.owner, Title::Chair)
        .into_iter()
        .collect();
    for kr in &obj.key_results {
        errs.extend(key_result_answerers(kr, staff));
    }
    refused_unless_empty(errs)
}

/// Whether key result kr names who answers for it and who judges it as `names_who_answers` asks.
pub(crate) fn key_result_names_who_answers(kr: &KeyResult, staff: &[Employee]) -> Result<()> {
    refused_unless_empty(key_result_answerers(kr, staff))
}

/// Why kr's owner or judge may not stand, each named.
fn key_result_answerers(kr: &KeyResult, staff: &[Employee]) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    errs.extend(answers_as(
        staff,
        format!("key result {}'s owner", kr.id),
        &kr.owner,
        Title::Chair,
    ));
    errs.extend(answers_as(
        staff,
        format!("key result {}'s judge", kr.id),
        &kr.judge,
        Title::Judge,
    ));
    errs.extend(judge_outside_its_owners_tree(kr, staff));
    errs
}

/// Why field, naming a, does not name an employee of staff at work under title; none when it does, is empty, or is the
/// owner.
fn answers_as(staff: &[Employee], field: String, a: &Accountable, title: Title) -> Option<String> {
    if a.as_str().is_empty() || a.is_owner() {
        return None;
    }
    let Some(e) = staff.iter().find(|e| e.id == a.as_str()) else {
        return Some(format!("{field} names {a}, who is not an employee"));
    };
    if e.status != EmployeeStatus::Active {
        return Some(format!("{field} names {a}, who is {}", e.status));
    }
    if e.title != title {
        return Some(format!("{field} names {a}, a {}: it needs a {title}", e.title));
    }
    None
}

fn refused_unless_empty(errs: Vec<String>) -> Result<()> {
    if errs.is_empty() {
        return Ok(());
    }
    Err(Error::refused(errs.join("\n")))
}

/// Why a key result may not stand, when its judge, both being employees, reports to its owner or through anyone who
/// does: the judge of a team stands outside it (ADR-0032).
fn judge_outside_its_owners_tree(kr: &KeyResult, staff: &[Employee]) -> Option<String> {
    let find = |a: &Accountable| staff.iter().find(|e| e.id == a.as_str());
    let (owner, judge) = (find(&kr.owner)?, find(&kr.judge)?);
    (!judge_outside_tree(judge, owner, staff)).then(|| {
        format!(
            "key result {}'s judge {} sits inside the tree of its owner {}: a judge stands outside the team it judges",
            kr.id, judge.id, owner.id
        )
    })
}

/// The fields the ask changes, as the owner reads them: every field of a new employee, the ones that differ for a
/// stored one.
fn employee_fields(cur: Option<&Employee>, next: &Employee) -> Result<Vec<ChangedField>> {
    let mut next = next.clone();
    if cur.is_none() {
        next.created_at = Time::default();
    }
    let mut fields = library::diff_fields(&doc_of(cur)?, &doc_of(Some(&next))?);
    fields.retain(|f| f.name != "createdAt");
    Ok(fields)
}

/// Reads what ask is about, from the store or a transaction, and applies it in memory: the stored employee (none when
/// none is), and what the ask makes of it.
fn built_employee(r: &Reader<'_>, ask: &EmployeeAsk, now: Time) -> Result<(Option<Employee>, Fit)> {
    let cur = optional(r.employee(ask.employee()))?;
    let tmpl = match (&ask.hire, &cur) {
        (Some(h), _) => optional(r.agent(&h.template))?,
        (None, Some(c)) if ask.action == EmployeeAction::Move => optional(r.agent(&c.template))?,
        _ => None,
    };
    let fit = employee_after(ask, cur.as_ref(), tmpl.as_ref(), now)?;
    Ok((cur, fit))
}

/// Whether w was let through by the owner, or the Assistant for him.
pub(crate) fn require_owner(w: &Authorship) -> Result<()> {
    match &w.approved_by {
        None => Err(Error::refused(ACT_APPLY.refusal())),
        Some(by) => for_owner(ACT_APPLY, by),
    }
}

/// Stores next over cur in tx with its change, at at, and returns it as stored.
fn put_employee(tx: &Tx<'_>, w: &Authorship, at: Time, cur: Option<&Employee>, next: Employee) -> Result<Employee> {
    record_change(tx, w, at, ChangeKind::Employee, &next.id, cur, Some(&next))?;
    tx.put_employee(next.clone())?;
    Ok(tx.employee(&next.id)?)
}

/// What tells the channels of the employees saved, once their transaction is committed.
fn told(saved: Vec<Employee>) -> Tell {
    Box::new(move |o: &chains::Core| {
        for e in &saved {
            o.notify.employee_updated(e);
        }
    })
}

impl Crew {
    /// Stores, in tx, the hire, the retirement or the move onto a newer template a change item asks for, if the stored
    /// employee is still as the owner was shown (`Applied::Moved` when it is not).
    pub fn apply_employee(&self, tx: &Tx<'_>, pending: &PendingChange, w: &Authorship) -> Result<Applied> {
        require_owner(w)?;
        let ask: EmployeeAsk = serde_json::from_str(&pending.record)?;
        let (cur, fit) = built_employee(tx, &ask, self.now())
            .map_err(|e| Error::refused(format!("the write is no longer one the employees accept: {e}")))?;
        let Fit::Next(next) = fit else {
            return Ok(Applied::Moved);
        };
        let next = *next;
        if employee_fields(cur.as_ref(), &next)? != pending.fields {
            return Ok(Applied::Moved);
        }
        let saved = put_employee(tx, w, self.now(), cur.as_ref(), next)?;
        Ok(Applied::Written(Some(told(vec![saved]))))
    }

    /// Stores in tx the employees an objective's change item proposes to hire, and returns what tells the channels once
    /// the transaction is committed. The owner who applies the objective hires them: each from its template, at the
    /// platform's ceiling for its title (a Chair's proposal sets none), reporting to him. The objective may then name
    /// only the owner and employees at work under the right title, hired now or before.
    pub(crate) fn apply_proposed_hires(
        &self,
        tx: &Tx<'_>,
        applied: &Objective,
        hires: &[Employee],
        w: &Authorship,
    ) -> Result<Option<Tell>> {
        require_owner(w)?;
        let mut saved = Vec::with_capacity(hires.len());
        for h in hires {
            if ceiling_set(&h.ceiling) {
                return Err(Error::refused(format!(
                    "the proposal to hire {} sets a ceiling: the platform sets it",
                    h.id
                )));
            }
            let ask = EmployeeAsk {
                action: EmployeeAction::Hire,
                id: String::new(),
                hire: Some(h.clone()),
                template_rev: 0,
            };
            let (cur, fit) =
                built_employee(tx, &ask, self.now()).map_err(|e| Error::refused(format!("hire {}: {e}", h.id)))?;
            let next = match fit {
                Fit::Next(next) => *next,
                Fit::Stale(why) => return Err(Error::refused(format!("hire {}: {why}", h.id))),
            };
            saved.push(put_employee(tx, w, self.now(), cur.as_ref(), next)?);
        }
        names_who_answers(applied, &tx.employees()?)
            .map_err(|e| Error::refused(format!("objective {}: {e}", applied.id)))?;
        Ok(Some(told(saved)))
    }
}

/// The input of draft_hire.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DraftHireInput {
    /// the id of the agent definition to hire from (list_library or the Agents tab)
    #[serde(default)]
    template: String,
    /// chair or judge, the only employees the owner hires (a Chair hires its workers)
    #[serde(default)]
    title: String,
    /// the owner's name for the employee, for example login chair
    #[serde(default)]
    label: String,
    /// the area it answers for, in the owner's words
    #[serde(default)]
    scope: String,
    /// the most it may do ({effect, grants: [{source, connection, effect}], tools}); leave it out unless he named one:
    /// a judge is hired at read, a chair at reversible
    #[serde(default)]
    #[schemars(with = "Option<serde_json::Map<String, serde_json::Value>>")]
    ceiling: Option<Ceiling>,
    /// the id of a retired employee to bring back as it was, with its memory; leave every other field out
    #[serde(default)]
    again: String,
}

/// The input of draft_retire and draft_move.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmployeeIdInput {
    /// the employee's id (e_…), as list_employees shows it
    id: String,
}

/// The ask for the owner's words: a hire of a new employee, or the return of a retired one.
fn hire_ask_of(input: DraftHireInput, new_employee_id: String) -> Result<EmployeeAsk> {
    if !input.again.is_empty() {
        if !input.template.is_empty()
            || !input.title.is_empty()
            || !input.label.is_empty()
            || !input.scope.is_empty()
            || input.ceiling.is_some()
        {
            return Err(Error::refused(
                "again brings a retired employee back as it was: leave template, title, label, scope and ceiling out",
            ));
        }
        return Ok(EmployeeAsk {
            action: EmployeeAction::Rehire,
            id: input.again,
            hire: None,
            template_rev: 0,
        });
    }
    let title = input
        .title
        .parse()
        .map_err(|e: model::UnknownValue| Error::refused(format!("title: {e}")))?;
    let h = Employee {
        id: new_employee_id,
        label: input.label,
        template: input.template,
        title,
        scope: input.scope,
        ceiling: input.ceiling.unwrap_or_default(),
        ..Employee::default()
    };
    Ok(EmployeeAsk {
        action: EmployeeAction::Hire,
        id: String::new(),
        hire: Some(h),
        template_rev: 0,
    })
}

const LIST_EMPLOYEES: &str = "The employees the owner has hired, each with its title, label, scope, status, the template it \
     was hired from and the revision it holds against the template's revision now (a newer one can be moved onto). \
     People are mentors, not employees: list_mentors shows them.";
const DRAFT_HIRE: &str = "Draft the hire of a chair or a judge from an agent definition, or the return of a retired \
     employee (again), as a change item in the owner's inbox. A worker is hired by a Chair, never here; a person is added \
     as a mentor (add_mentor), never hired. Nothing is hired until it is applied: apply it yourself with answer_inbox \
     when it is what he asked for, and tell him what you did.";
const DRAFT_RETIRE: &str = "Draft the retirement of an employee as a change item. Retiring keeps its record and its \
     memory; the key results it owns or judges fall to the owner. Nothing changes until it is applied, which is yours \
     to do for him.";
const DRAFT_MOVE: &str = "Draft the move of an employee onto the newer revision of the template it was hired from, as a \
     change item: it takes the template's loadout as it is now. Nothing changes until it is applied, which is yours to \
     do for him.";

/// An employee as the Assistant reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EmployeeRow {
    pub id: String,
    /// Its name when it has one, its code and what it was hired from.
    pub shown: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scope: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub strengths: String,
    pub status: String,
    pub template: String,
    pub template_rev: i64,
    /// 0 when the template is gone.
    #[serde(rename = "template_rev_now", skip_serializing_if = "is_zero")]
    pub template_now: i64,
}

impl Crew {
    /// Gives the Assistant, acting as by, the tools that draft a hire, a retirement or a move onto a newer template as a
    /// change item.
    pub fn add_employee_assistant_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        t.serving(self).add(
            "list_employees",
            chains::tools::Effect::Reads,
            LIST_EMPLOYEES,
            |o: Arc<Crew>, _: NoInput| async move { as_json(&o.employee_rows().await?) },
        );
        let who = by.clone();
        t.serving(self).add(
            "draft_hire",
            chains::tools::Effect::Writes,
            DRAFT_HIRE,
            move |o: Arc<Crew>, input: DraftHireInput| {
                let by = who.clone();
                async move {
                    let ask = hire_ask_of(input, new_id("e_"))?;
                    o.drafted_employee(&by, ask).await
                }
            },
        );
        let who = by.clone();
        t.serving(self).add(
            "draft_retire",
            chains::tools::Effect::Writes,
            DRAFT_RETIRE,
            move |o: Arc<Crew>, input: EmployeeIdInput| {
                let by = who.clone();
                async move {
                    let ask = EmployeeAsk {
                        action: EmployeeAction::Retire,
                        id: input.id,
                        hire: None,
                        template_rev: 0,
                    };
                    o.drafted_employee(&by, ask).await
                }
            },
        );
        let who = by.clone();
        t.serving(self).add(
            "draft_move",
            chains::tools::Effect::Writes,
            DRAFT_MOVE,
            move |o: Arc<Crew>, input: EmployeeIdInput| {
                let by = who.clone();
                async move {
                    let ask = o.move_ask(input.id).await?;
                    o.drafted_employee(&by, ask).await
                }
            },
        );
    }

    /// The ask to move employee id onto its template's revision now. A template may be gone: `moved_onto` refuses it,
    /// saying why.
    async fn move_ask(&self, id: String) -> Result<EmployeeAsk> {
        self.store
            .read(move |r| -> Result<EmployeeAsk> {
                let cur = r
                    .employee(&id)
                    .map_err(|e| Error::refused(format!("employee {id:?}: {e}")))?;
                let tmpl = optional(r.agent(&cur.template))
                    .map_err(|e| Error::refused(format!("template {:?} of {}: {e}", cur.template, cur.id)))?;
                Ok(EmployeeAsk {
                    action: EmployeeAction::Move,
                    id,
                    hire: None,
                    template_rev: tmpl.map_or(0, |t| t.rev),
                })
            })
            .await
    }

    /// Every employee as the Assistant reads it.
    pub(crate) async fn employee_rows(&self) -> Result<Vec<EmployeeRow>> {
        self.store
            .read(|r| -> Result<Vec<EmployeeRow>> {
                let mut rows = Vec::new();
                for e in r.employees()? {
                    let now = optional(r.agent(&e.template))?.map_or(0, |t| t.rev);
                    rows.push(EmployeeRow {
                        id: e.id.clone(),
                        shown: e.shown(),
                        title: e.title.to_string(),
                        scope: e.scope,
                        strengths: e.strengths,
                        status: e.status.to_string(),
                        template: e.template,
                        template_rev: e.template_rev,
                        template_now: now,
                    });
                }
                Ok(rows)
            })
            .await
    }

    /// Raises the change item that asks the owner for ask, as the Assistant by, and says what waits.
    async fn drafted_employee(&self, by: &Actor, ask: EmployeeAsk) -> Result<String> {
        let (item, falls) = self.employee_item(by, ask).await?;
        let item = self.raise_change(item).await?;
        let name = item.change.as_ref().map(|c| c.name.clone()).unwrap_or_default();
        Ok(format!(
            "{} waits in the owner's inbox (employee {name}); nothing is written until it is applied. Apply it with \
             answer_inbox when it is what he asked for, and tell him what you did.\n{falls}",
            item.title
        ))
    }

    /// The change item that asks the owner for ask, as the Assistant by, and what it says of the key results that fall
    /// to the owner: refused when the ask does not fit the stored records.
    async fn employee_item(&self, by: &Actor, ask: EmployeeAsk) -> Result<(InboxItem, String)> {
        let (who, now) = (by.who.clone(), self.now());
        self.store
            .read(move |r| -> Result<(InboxItem, String)> {
                let card = r.card(&who)?;
                let (cur, fit) = built_employee(r, &ask, now)?;
                let next = match fit {
                    Fit::Next(next) => *next,
                    Fit::Stale(why) => return Err(Error::refused(why)),
                };
                let record = serde_json::to_string(&ask)?;
                let fields = employee_fields(cur.as_ref(), &next)?;
                let falls = answers_that_fall_to(&r.objectives()?, &r.every_key_result()?, &ask, &next);
                let mut detail = change_detail(&card.name, &ChangeKind::Employee, &next.id, &fields);
                if !falls.is_empty() {
                    detail.push_str(&format!("\n\n{falls}"));
                }
                let item = InboxItem {
                    id: new_id("i_"),
                    home: card.home.clone(),
                    card_id: card.id,
                    kind: InboxKind::Change,
                    title: employee_title(&ask, &next),
                    detail,
                    options: vec![
                        InboxOption {
                            id: CHANGE_APPLY.into(),
                            name: "Apply".into(),
                            ..InboxOption::default()
                        },
                        InboxOption {
                            id: CHANGE_NOT_NOW.into(),
                            name: "Not now".into(),
                            ..InboxOption::default()
                        },
                    ],
                    status: InboxStatus::Open,
                    created_at: now,
                    change: Some(PendingChange {
                        kind: ChangeKind::Employee,
                        name: next.id,
                        record,
                        fields,
                    }),
                    ..InboxItem::default()
                };
                Ok((item, falls))
            })
            .await
    }
}

fn employee_title(ask: &EmployeeAsk, next: &Employee) -> String {
    match ask.action {
        EmployeeAction::Hire => format!("Hire {} as {}", next.label, next.title),
        EmployeeAction::Rehire => format!("Hire {} again", next.shown()),
        EmployeeAction::Retire => format!("Retire {}", next.shown()),
        EmployeeAction::Move => {
            format!(
                "Move {} onto revision {} of {}",
                next.shown(),
                next.template_rev,
                next.template
            )
        }
    }
}

/// Which key results of the active objectives (every key result of each in key_results, those that refine others
/// included) name the employee e a retirement lets go, since each of them is answered by the owner from then on; "" for
/// none, or for any other ask.
fn answers_that_fall_to(
    objectives: &[Objective],
    key_results: &HashMap<String, Vec<KeyResult>>,
    ask: &EmployeeAsk,
    e: &Employee,
) -> String {
    if ask.action != EmployeeAction::Retire {
        return String::new();
    }
    let me = Accountable(e.id.clone());
    let mut names = Vec::new();
    for obj in objectives.iter().filter(|o| o.status == ObjectiveStatus::Active) {
        if obj.owner == me {
            names.push(obj.id.clone());
        }
        let krs = key_results.get(&obj.id).map(Vec::as_slice).unwrap_or_default();
        for kr in krs.iter().filter(|kr| kr.owner == me || kr.judge == me) {
            names.push(format!("{}/{}", obj.id, kr.id));
        }
    }
    if names.is_empty() {
        return String::new();
    }
    format!(
        "{} answers for {}; the owner answers for them from then on.",
        e.label,
        names.join(", ")
    )
}
