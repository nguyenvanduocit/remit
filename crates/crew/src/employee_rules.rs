//! The rules of hiring (ADR-0032), pure: each takes the records it judges and answers, with no store, no clock and no
//! session. The orchestrator calls them where an employee is made and refuses what they refuse.

use std::collections::{HashMap, HashSet};
use std::fmt;

use grant::Effect;
use model::{Accountable, Ceiling, CeilingGrant, Employee, EmployeeStatus, Title};

use chains::hire_input::HireInput;
use chains::{Error, Result};
use wire::app::LineageStanding;

use crate::lineage_standing::standing_words;

/// The numbers a hire is held to, in one place: `headcount` is how many active employees one hirer may have hired,
/// `depth` how many levels of reporting stand under the owner (a chair is level one). They are a rail: no tool raises
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HireCaps {
    pub headcount: usize,
    pub depth: usize,
}

/// Which rule refused a hire (ADR-0027: a refusal names its kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
    /// The hire would hold more than its hirer.
    Ceiling,
    /// The hirer has hired the most it may, or the hire would stand too deep.
    Cap,
    /// The hirer does not hire, or not one of that title.
    Title,
    /// The hirer keeps an idle worker it could put to work instead.
    Reuse,
}

impl fmt::Display for RefusalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ceiling => "ceiling",
            Self::Cap => "cap",
            Self::Title => "title",
            Self::Reuse => "reuse",
        })
    }
}

/// A hire the rules refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HireRefusal {
    pub kind: RefusalKind,
    pub why: String,
}

impl fmt::Display for HireRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hire refused ({}): {}", self.kind, self.why)
    }
}

impl std::error::Error for HireRefusal {}

impl From<HireRefusal> for Error {
    fn from(r: HireRefusal) -> Self {
        Self::refused(r.to_string())
    }
}

fn refusal(kind: RefusalKind, why: impl Into<String>) -> HireRefusal {
    HireRefusal { kind, why: why.into() }
}

/// Why hirer may not hire hire, or none when it may. staff is every employee on record. The checks, in order: only a
/// chair hires, and only workers (a chair hires no chair and no judge: those are the owner's); the hire holds no more
/// than its hirer (`ceiling_excess`); the hirer has not hired as many as the headcount cap lets it, and the hire would
/// stand no deeper than the depth cap. Retired employees do not count against the headcount.
pub fn check_hire(hirer: &Employee, hire: &Employee, staff: &[Employee], caps: HireCaps) -> Option<HireRefusal> {
    if hirer.title != Title::Chair {
        return Some(refusal(
            RefusalKind::Title,
            format!("a {} does not hire: only a chair hires", hirer.title),
        ));
    }
    if hire.title != Title::Worker {
        return Some(refusal(
            RefusalKind::Title,
            format!(
                "a chair hires workers, not a {}: the owner hires chairs and judges",
                hire.title
            ),
        ));
    }
    if let Some(excess) = ceiling_excess(&hire.ceiling, &hirer.ceiling) {
        return Some(refusal(
            RefusalKind::Ceiling,
            format!("the hire would hold more than its hirer: {excess}"),
        ));
    }
    let hired = staff
        .iter()
        .filter(|e| e.hired_by.as_str() == hirer.id && e.status == EmployeeStatus::Active)
        .count();
    if hired >= caps.headcount {
        return Some(refusal(
            RefusalKind::Cap,
            format!("{} has already hired {}, the most it may", hirer.id, caps.headcount),
        ));
    }
    match depth_of(hirer, staff, caps.depth) {
        None => Some(refusal(
            RefusalKind::Cap,
            format!(
                "{} does not stand within {} levels under the owner",
                hirer.id, caps.depth
            ),
        )),
        Some(depth) if depth >= caps.depth => Some(refusal(
            RefusalKind::Cap,
            format!(
                "a hire under {} would stand {} levels deep, the most is {}",
                hirer.id,
                depth.saturating_add(1),
                caps.depth
            ),
        )),
        Some(_) => None,
    }
}

/// Why hirer, a Chair, may not put emp to work as input asks, or none. A Chair puts to work an active worker who holds
/// no more than the Chair does (`ceiling_excess`), named in place of an agent: every employee is kept until the owner
/// retires it (ADR-0061), so "standing" is any kept one.
pub fn standing_refusal(hirer: &Employee, emp: &Employee, input: &HireInput) -> Result<()> {
    if emp.status != EmployeeStatus::Active {
        return Err(Error::refused(format!(
            "{} ({}) is {}: put an active employee to work",
            emp.shown(),
            emp.id,
            emp.status
        )));
    }
    if emp.title != Title::Worker {
        return Err(Error::refused(format!(
            "{} ({}) is a {}: a Chair puts only workers to work",
            emp.shown(),
            emp.id,
            emp.title
        )));
    }
    // the ceiling bounds what the hired card holds
    if let Some(excess) = ceiling_excess(&emp.ceiling, &hirer.ceiling) {
        return Err(refusal(
            RefusalKind::Ceiling,
            format!("{} holds more than you: {excess}", emp.shown()),
        )
        .into());
    }
    if !input.agent.trim().is_empty() {
        // the employee's own record decides what it is
        return Err(Error::refused("employee names who works: leave out agent"));
    }
    Ok(())
}

/// Why hirer, a Chair, may not hire a new worker, or none when it may (ADR-0062). The Chair reuses before it hires,
/// whatever agent a kept worker was hired from: while an idle kept worker exists that it could put to work (one
/// `standing_refusal` passes, holding no card open in any chain: busy names those that do), a new one is refused unless
/// why names one of them by id, saying why its strengths do not fit. A worker whose work the hire builds on (authors) is
/// never the one: it does not judge its own work. The refusal names every candidate, with how its lineage stands in the
/// chain's kind of work when it has a standing (standings, by template).
pub fn check_reuse(
    hirer: &Employee,
    why: &str,
    staff: &[Employee],
    busy: &HashSet<String>,
    authors: &HashSet<String>,
    standings: &HashMap<String, LineageStanding>,
) -> Option<HireRefusal> {
    let mut candidates = Vec::new();
    for e in staff {
        let idle = HireInput {
            employee: e.id.clone(),
            ..HireInput::default()
        };
        if busy.contains(&e.id) || authors.contains(&e.id) || standing_refusal(hirer, e, &idle).is_err() {
            continue;
        }
        if why.contains(&e.id) {
            return None;
        }
        let mut candidate = format!("{}, {}", e.id, e.shown());
        if let Some(l) = standings.get(&e.template) {
            candidate.push_str(&format!(" ({})", standing_words(l)));
        }
        candidates.push(candidate);
    }
    if candidates.is_empty() {
        return None;
    }
    Some(refusal(
        RefusalKind::Reuse,
        format!(
            "you keep idle workers you can put to work: {}. Give one's id as employee, or say in why which one you do \
             not choose, by its id, and why its strengths do not fit",
            candidates.join(", ")
        ),
    ))
}

/// The employee that takes the Chair seat of a chain no key result's Chair sits in: the oldest Chair at work whose
/// ceiling fits, whatever agent it was hired from (ADR-0062). None when no Chair sits.
pub fn standing_chair(staff: &[Employee], fits: impl Fn(&Ceiling) -> bool) -> Option<&Employee> {
    let sits = |e: &Employee| e.title == Title::Chair && e.status == EmployeeStatus::Active && fits(&e.ceiling);
    let mut seat: Option<&Employee> = None;
    for e in staff.iter().filter(|e| sits(e)) {
        if seat.is_none_or(|s| e.created_at < s.created_at) {
            seat = Some(e);
        }
    }
    seat
}

/// The employee e reports to in staff, when it is one.
fn superior<'a>(e: &Employee, staff: &'a [Employee]) -> Option<&'a Employee> {
    staff.iter().find(|s| Accountable(s.id.clone()) == e.reports_to)
}

/// How many levels e stands under the owner (an employee reporting to the owner is level one), walking `reports_to`
/// through staff, at most limit steps. None when the walk loops or does not end within limit.
fn depth_of(e: &Employee, staff: &[Employee], limit: usize) -> Option<usize> {
    let mut seen = HashSet::from([e.id.as_str()]);
    let mut e = e;
    for depth in 1..=limit {
        let Some(up) = superior(e, staff) else {
            return Some(depth);
        };
        if !seen.insert(up.id.as_str()) {
            return None;
        }
        e = up;
    }
    None
}

/// What inner holds that outer does not, in a phrase, or none when inner is within outer: its effect is no stronger, each
/// of its grants is covered by a grant of outer on the same connection of the same source with an effect no weaker (a
/// higher effect covers every lower one, ADR-0018), and its tools are a subset. An effect nobody can rank is within
/// nothing, and nothing is within it.
pub fn ceiling_excess(inner: &Ceiling, outer: &Ceiling) -> Option<String> {
    let inner_effect = match rank_of(&inner.effect) {
        Ok(e) => e,
        Err(err) => return Some(err.to_string()),
    };
    let outer_effect = match rank_of(&outer.effect) {
        Ok(e) => e,
        Err(err) => return Some(err.to_string()),
    };
    if inner_effect > outer_effect {
        return Some(format!("effect {:?} is stronger than {:?}", inner.effect, outer.effect));
    }
    for g in &inner.grants {
        match grant_covered(g, &outer.grants) {
            Err(err) => return Some(err.to_string()),
            Ok(false) => {
                return Some(format!(
                    "grant on {}/{} at {} is not one its hirer holds",
                    g.source, g.connection, g.effect
                ));
            }
            Ok(true) => {}
        }
    }
    tools_excess(&inner.tools, &outer.tools)
}

/// What inner's tools hold that outer's do not. An empty list puts no restriction on Claude Code tools (the template's
/// loadout decides), so anything is within an empty outer; a non-empty outer is the allowed set, and an inner that names
/// none would be unrestricted, so it is not within it.
fn tools_excess(inner: &[String], outer: &[String]) -> Option<String> {
    if outer.is_empty() {
        return None;
    }
    if inner.is_empty() {
        return Some("it names no tools, so it is unrestricted where its hirer is not".to_owned());
    }
    inner
        .iter()
        .find(|tool| !outer.contains(tool))
        .map(|tool| format!("tool {tool} is not one its hirer holds"))
}

/// An effect of a ceiling as a rank: the empty effect is none (below read), the rest are parsed once, here.
fn rank_of(effect: &str) -> Result<Option<Effect>, grant::UnknownEffect> {
    if effect.is_empty() {
        return Ok(None);
    }
    effect.parse().map(Some)
}

/// Whether one of outer is on g's connection with an effect at least g's.
fn grant_covered(g: &CeilingGrant, outer: &[CeilingGrant]) -> Result<bool, grant::UnknownEffect> {
    let want: Effect = g.effect.parse()?;
    for o in outer
        .iter()
        .filter(|o| o.source == g.source && o.connection == g.connection)
    {
        let have: Effect = o.effect.parse()?;
        if have >= want {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether judge stands outside the tree of chair: it is not the chair and does not report to it, directly or through
/// any employee that does. The walk goes up from the judge through `reports_to` within all, and a visited set ends it on
/// a loop (a loop that never meets the chair leaves the judge outside).
pub(crate) fn judge_outside_tree(judge: &Employee, chair: &Employee, all: &[Employee]) -> bool {
    let mut seen = HashSet::new();
    let mut e = judge;
    while seen.insert(e.id.as_str()) {
        if e.id == chair.id {
            return false;
        }
        match superior(e, all) {
            Some(up) => e = up,
            None => return true,
        }
    }
    true
}
