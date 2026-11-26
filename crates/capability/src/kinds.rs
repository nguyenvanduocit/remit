//! What each kind of chain is given in code (ADR-0113): the capabilities it starts with, the section of its Chair's brief
//! that follows the goal, and who may start it. The kind itself, its gates and whether the owner waits are model's
//! (`model::ChainKind`); this table holds what needs the catalog's names and the seats' briefs. A playbook is reference
//! and decides none of it.

use model::{ChainKind, ChainOrigin};

use crate::briefs::{
    ANALYST_BRIEF, ANALYST_CHAIR_BRIEF, AUTHOR_CHAIR_BRIEF, COACH_CHAIR_BRIEF, LIBRARIAN_CHAIR_BRIEF,
    SEEDING_CHAIR_BRIEF,
};
use crate::capabilities::writes;
use crate::okr::{MEASURE_RULES, OKR_SEAT_CRAFT};
use chains::cap_names::{
    CAP_AGENTS, CAP_COACHING, CAP_HISTORY, CAP_LIBRARY, CAP_MEMORY_KEEPING, CAP_OBJECTIVES, CAP_OKR, CAP_OUTCOMES,
    CAP_PLAYBOOKS, CAP_SEEDING, CAP_SHELL, CAP_SIGNALS,
};

/// What a `work` or `chore` Chair reads of the playbooks: they are reference, and none binds it.
const PLAYBOOK_NOTE: &str = "PLAYBOOKS — list_playbooks and read_playbook describe how kinds of work have gone well: read one when it fits this goal and take what helps. None binds you; the goal and the rules below do.";

/// What a chore's Chair adds to the note: a chore stays small.
const CHORE_NOTE: &str = " This chain is a chore: a small, well-understood change. If it turns out bigger (a design choice, several repositories, a root cause still unknown), stop growing it: finish_chain saying so, and say it needs a work chain.";

/// What an objective chain's Chair works by, after the craft of the seat and before the measure rule.
const OBJECTIVE_NOTE: &str = " The goal says what this chain does with the owner's objectives: shape a new one from a big or half-formed request, review the active ones against the measure rule, or check in on where their key results stand. The reference playbooks objective, okr-review and okr-checkin carry the procedure of each: read the one that fits with read_playbook. ";

/// What closes an objective chain's Chair section: nothing it drafts is active on its word.
const OBJECTIVE_APPLY_NOTE: &str = " Nothing is active until it is applied in the owner's inbox, where the Assistant decides it for him and he may overturn it; you never apply it.";

/// The capabilities a chain of kind starts with.
pub fn capabilities_of(kind: ChainKind) -> Vec<String> {
    let caps: &[&str] = match kind {
        ChainKind::Work | ChainKind::Chore => &[],
        ChainKind::Objective => &[CAP_OBJECTIVES, CAP_OKR, CAP_SIGNALS],
        ChainKind::Author => &[CAP_AGENTS, CAP_HISTORY, CAP_LIBRARY, CAP_PLAYBOOKS],
        ChainKind::Coach => &[CAP_COACHING, CAP_SIGNALS],
        ChainKind::Analyst => &[CAP_HISTORY, CAP_OUTCOMES],
        ChainKind::Librarian => &[CAP_MEMORY_KEEPING],
        ChainKind::Trial => &[CAP_SEEDING, CAP_SHELL],
    };
    caps.iter().map(|c| (*c).to_owned()).collect()
}

/// The section of the Chair's brief a chain of kind reads after its goal.
pub fn brief_of(kind: ChainKind) -> String {
    match kind {
        ChainKind::Work => PLAYBOOK_NOTE.to_owned(),
        ChainKind::Chore => [PLAYBOOK_NOTE, CHORE_NOTE].concat(),
        ChainKind::Objective => [OKR_SEAT_CRAFT, OBJECTIVE_NOTE, MEASURE_RULES, OBJECTIVE_APPLY_NOTE].concat(),
        ChainKind::Author => AUTHOR_CHAIR_BRIEF.to_owned(),
        ChainKind::Coach => COACH_CHAIR_BRIEF.to_owned(),
        ChainKind::Analyst => format!(
            "{ANALYST_CHAIR_BRIEF}\nTHE ANALYST'S BRIEF, which every run's hire of the agent analyst carries:\n{ANALYST_BRIEF}"
        ),
        ChainKind::Librarian => LIBRARIAN_CHAIR_BRIEF.to_owned(),
        ChainKind::Trial => SEEDING_CHAIR_BRIEF.to_owned(),
    }
}

/// Why a chain of origin may not be started as kind, or none when it may: the one rule of who starts which kind. A chain
/// a proposal, an event or a client starts runs on a goal the owner did not write, so it takes no kind whose capabilities
/// write (`work` or `chore`, then); the analyst and the librarian are Remit's own, one of each at a time, so no
/// starter names them. Its Chair may still ask for what it needs once it runs (request_capability).
pub fn may_not_start(origin: &ChainOrigin, kind: ChainKind) -> Option<String> {
    if matches!(kind, ChainKind::Analyst | ChainKind::Librarian) {
        return Some(format!(
            "a {kind} chain is Remit's own: it starts it itself, one at a time"
        ));
    }
    let outside = matches!(origin, ChainOrigin::Proposed | ChainOrigin::Event | ChainOrigin::Client);
    let w = writes(&capabilities_of(kind));
    if outside && !w.is_empty() {
        return Some(format!(
            "a {kind} chain starts with {}, which a {origin} chain does not start with: start a work or a chore chain, and \
             its Chair asks for what it needs with request_capability",
            w.join(" and ")
        ));
    }
    None
}
