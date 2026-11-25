use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Accountable, Loadout, Time};

text_enum! {
    /// Which rules an employee works under (ADR-0032). The set is closed: a new title is a change to the rules.
    #[derive(Default)]
    pub enum Title {
        Chair = "chair",
        Judge = "judge",
        #[default]
        Worker = "worker",
    }
}

text_enum! {
    /// Whether an employee is at work. Retiring keeps the record and its memory.
    #[derive(Default)]
    pub enum EmployeeStatus {
        #[default]
        Active = "active",
        Retired = "retired",
    }
}

/// A grant of a ceiling (ADR-0018): the strongest effect on one connection of one source. Effect is a name the grant
/// crate ranks; the orchestrator parses it there where a ceiling is compared.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CeilingGrant {
    pub source: String,
    pub connection: String,
    pub effect: String,
}

/// The most an employee may do: the strongest effect (ADR-0027), a set of grants and a set of Claude Code tools. Nothing
/// an employee hires or opens holds more. Effect, like `CeilingGrant::effect`, is text here and parsed at the boundary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ceiling {
    pub effect: String,
    pub grants: Vec<CeilingGrant>,
    pub tools: Vec<String>,
}

/// An AI employee kept until the owner retires it (ADR-0032, ADR-0061): an agent's template loadout copied at
/// `template_rev`, a title, a place in the reporting tree, and a memory (ADR-0033). A card is the employee at work in one
/// chain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Employee {
    pub id: String,
    /// Its own, the next after the highest on record, given in the write that stores it and never reused.
    pub number: i64,
    /// Its name: the one the owner gave an employee he hires, or the one it chose; empty until an agent chooses one, and
    /// fixed once it has.
    pub label: String,
    pub template: String,
    /// The template's revision when it was hired: a later change to the template does not reach it.
    pub template_rev: i64,
    pub loadout: Loadout,
    pub title: Title,
    /// The area it answers for, in the owner's words.
    pub scope: String,
    /// What it says it is good at, in its own words; its record checks them.
    pub strengths: String,
    pub reports_to: Accountable,
    pub hired_by: Accountable,
    pub status: EmployeeStatus,
    pub ceiling: Ceiling,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
    pub created_at: Time,
}

impl Employee {
    /// How its number reads: E-42.
    pub fn code(&self) -> String {
        format!("E-{}", self.number)
    }

    /// Its code and the agent it was hired from, as in "E-42 (engineer)": the part of `shown` that names it when its name
    /// is cut short.
    pub fn tag(&self) -> String {
        format!("{} ({})", self.code(), self.template)
    }

    /// The employee as the owner and a Chair read it: its name when it has one, then its tag, as in
    /// "Mai · E-42 (engineer)" or "E-42 (engineer)".
    pub fn shown(&self) -> String {
        if self.label.is_empty() {
            self.tag()
        } else {
            format!("{} · {}", self.label, self.tag())
        }
    }
}
