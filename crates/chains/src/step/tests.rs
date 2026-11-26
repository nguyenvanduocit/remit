//! The goal check's end: what of its judge's work ended decides whether the check ends.

use model::{Chain, ChainStatus, Finishing, Time};

use super::*;

fn judging() -> Chain {
    let mut c = Chain::default();
    "ch_1".clone_into(&mut c.id);
    "c_chair".clone_into(&mut c.chair_id);
    c.set_status(ChainStatus::Running);
    c.finishing = Some(Finishing {
        summary: "done".to_owned(),
        judge: "c_judge".to_owned(),
        ..Finishing::default()
    });
    c
}

fn ended(end: JudgeEnd) -> Result<Step> {
    let cmd = ChainCmd::EndGoalCheck(EndGoalCheckCmd {
        judge: "c_judge".to_owned(),
        why: "the judge ended its turn without a verdict".to_owned(),
        end,
        ..EndGoalCheckCmd::default()
    });
    step(&judging(), &WakeSubs::default(), &cmd, Time::default())
}

// A judge asked again (ADR-0132) may answer the owner's question while the check's brief waits behind it: that turn's
// end leaves the check pending, while the end of its turn on the check, or its stop, ends it.
#[test]
fn only_the_judges_work_on_the_check_ends_it() {
    assert!(matches!(ended(JudgeEnd::Question), Ok(Step::Unchanged)));
    for end in [JudgeEnd::Turn, JudgeEnd::Stopped] {
        let Ok(Step::Changed(t)) = ended(end) else {
            panic!("{end:?} leaves the check pending");
        };
        assert!(t.next.finishing.is_none(), "{end:?} leaves the check pending");
    }
}
