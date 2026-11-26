//! Which judge checks a chain's next finish (ADR-0132).

use model::{Card, CardStatus, JudgeKind, Time};

use super::*;

fn at(minute: i64) -> Time {
    Time(time::OffsetDateTime::UNIX_EPOCH.saturating_add(time::Duration::minutes(minute)))
}

fn judge(id: &str, minute: i64, status: CardStatus) -> Card {
    let mut c = Card::judge_of(JudgeKind::Goal);
    id.clone_into(&mut c.id);
    "sess".clone_into(&mut c.session_id);
    "opus".clone_into(&mut c.model);
    c.created_at = at(minute);
    c.set_status(status, at(minute));
    c
}

fn asked(cards: &[Card], employee: &str, model: &str) -> Option<String> {
    judge_to_ask_again(cards, employee, model).map(|c| c.id.clone())
}

// The last goal judge is asked again while it can resume and is the judge the check needs; anything else hires a new one.
#[test]
fn the_last_goal_judge_checks_again_while_it_can() {
    let mut chair = judge("c_chair", 9, CardStatus::Idle);
    chair.origin = model::Origin::Chair;
    chair.judge = None;
    let earlier = judge("c_1", 1, CardStatus::Stopped);
    let last = judge("c_2", 2, CardStatus::Stopped);
    assert_eq!(
        asked(&[chair.clone(), last.clone(), earlier.clone()], "", "opus").as_deref(),
        Some("c_2")
    );
    assert_eq!(asked(&[chair], "", "opus"), None, "no goal judge yet");

    let failed = judge("c_2", 2, CardStatus::Failed);
    assert_eq!(
        asked(&[earlier, failed], "", "opus"),
        None,
        "a failed judge is not asked, nor one before it"
    );
    let mut sessionless = last.clone();
    sessionless.session_id.clear();
    assert_eq!(asked(&[sessionless], "", "opus"), None, "nothing to resume");
    assert_eq!(
        asked(std::slice::from_ref(&last), "e_7", "opus"),
        None,
        "the key result names another judge"
    );
    assert_eq!(asked(&[last], "", "sonnet"), None, "judges run on another model");
}
