//! What the owner can ask of a chain as a whole, and who a chain came from as he reads it.

use model::{Chain, ChainOrigin, Time};
use wire::app::ChainAsk;

use chains::step::{ArchiveCmd, ChainCmd, FinishCmd, PauseCmd, ResumeCmd, Step, UnarchiveCmd, step};
use chains::wait::WakeSubs;

/// The asks in the order the owner is offered them, each with the command that carries it out.
fn chain_asks() -> [(ChainAsk, ChainCmd); 6] {
    [
        (ChainAsk::Pause, ChainCmd::Pause(PauseCmd::default())),
        (ChainAsk::Resume, ChainCmd::Resume(ResumeCmd::default())),
        (ChainAsk::Finish, ChainCmd::Finish(FinishCmd::default())),
        (ChainAsk::Archive, ChainCmd::Archive(ArchiveCmd::default())),
        (ChainAsk::Unarchive, ChainCmd::Unarchive(UnarchiveCmd::default())),
        (ChainAsk::Delete, ChainCmd::Delete),
    ]
}

/// What the owner can ask of chain c now: what step would take and change, so the rule that refuses a command is the one
/// that decides what is offered; a command that would leave the chain as it is (unarchiving a chain not archived) is not
/// offered, as the step refuses it (`Halt::Unchanged`).
pub fn chain_asks_of(c: &Chain) -> Vec<ChainAsk> {
    let w = WakeSubs::default();
    // what is offered does not hang on the time: any time asks the step the same
    chain_asks()
        .into_iter()
        .filter(|(_, cmd)| matches!(step(c, &w, cmd, Time::default()), Ok(Step::Changed(_))))
        .map(|(ask, _)| ask)
        .collect()
}

/// Who a chain of origin o came from, as the owner reads it.
pub fn origin_label(o: &ChainOrigin) -> &'static str {
    match o {
        ChainOrigin::Owner => "you",
        ChainOrigin::Proposed => "a chain's proposal",
        ChainOrigin::Routine => "a routine",
        ChainOrigin::Event => "an event",
        ChainOrigin::Client => "an outside agent",
        ChainOrigin::Verification => "a judge's verification",
        ChainOrigin::Replay => "",
    }
}
