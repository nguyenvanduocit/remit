//! A chain's lifecycle is one pure function: `step` takes the chain as stored, the wake subscriptions it holds (waits), one
//! command and the time, and says what the chain becomes, what the owner sees decided, which messages the change sends and
//! what must happen once it is stored. `apply` (apply.rs) is its only caller and the only writer of chains: it holds the
//! chain's lock across step and one transaction that stores the chain with those decisions and messages, then runs the
//! effects. So no status is written anywhere else (`Chain::set_status` is called in this module alone), a change and the
//! message it implies are stored together, and a refused command changes nothing.
//!
//! ```text
//! state     command              next state   sends to the Chair        effects
//! queued    start                running      its first message (none when it begins with a run: it goes with the run's end)
//! not done  title                same, titled (the Chair names what it does)
//! running   pause                paused                                 cancel turns, stop background work, free slot
//! waiting   pause                paused (keeps its wake subscription)   (as running)
//! paused    resume               running      "resumed"
//! paused+w  resume               waiting                                close sessions, free slot
//! not done  take a capability    same         (nothing)                 the owner gave it: agents opened from then on hold its tools
//! running   subscribe            running+w    (replaces its wake subscription)
//! running+w turn ended           waiting                                close all sessions, free slot
//! running   turn ended, its runs wait only (a time, a signal, the owner) and nothing else works
//!                                waiting                                close all sessions, free slot
//! running   a run moves          running      the run's end, when it ended
//!           (a result, a call, its end or stop; its runs then wait only and nothing else works: waiting, close sessions, free slot)
//! waiting   a run moves          running when it puts a card to work or ended (a wait's firing is a scheduled wake), else waiting
//! paused    a run moves          paused       (its results are kept; no call is issued until the chain resumes)
//!            (something needs it) running     why the wait is cancelled
//! waiting   wake (its subscription fired)
//!                                running      the wake                  stall count reset
//! waiting   check now (owner)    running      why the owner asked       stall count reset, scheduler looks again
//! any open  send to the Chair    ends the wake subscriptions (waiting → running) and says so; a restart's continuation does not
//! done      send from the owner  running (unarchived)
//! not done  finish               done         nothing waits on it: queued messages dropped, questions and errors cancelled; end sessions, free slot
//! running   check the goal       running, finishing          the goal judge's brief
//! finishing the judge's verdict  finishing, with the verdict (refused once it has one, or from another judge)
//! finishing the check ends:
//!           approved             done         (as finish)
//!           refused              not finishing                what is left, to the Chair
//!           no verdict           done, saying the check could not run (as finish)
//!           (paused, its turn cut) finishing  "go on", to the judge, when the chain resumes
//! paused+finishing resume        running      the judge's time starts again
//! done      check again (its change landed since the verdict left a deploy or write unverified)
//!                                done, finishing (recheck)   the goal judge's brief
//!           the recheck ends     done         the new verdict, or that it could not run
//! done      archive / unarchive  done
//! done      delete               gone                                   drop sessions, free slot
//! any       mark live            same         (a decision)              refused while queued
//! any       record repo work     same         a submit's result, unless queued or done
//!           (refused as its edit says: a submit starting while one of the repository runs; a submit running
//!           refuses delete)
//! ```
//!
//! Everything else is refused with the reason, or unchanged when the chain already is what the command asks. A command step
//! was never taught cannot be written: `ChainCmd` is closed.

use std::fmt;
use std::sync::Arc;

use model::{
    Actor, Approval, Card, Chain, ChainStatus, DecisionKind, FindingBlocker, Finishing, Reading, RepoWork,
    Subscription, Time, Unjudged, Verdict,
};

use crate::chain_core::first_line;
use crate::gate::still_missing;
use crate::goal::{GOAL_JUDGE_TASK, MAX_GOAL_CHECKS, finding_lines};
use crate::goal::{goal_deadline, goal_deadline_id};
use crate::outbox::{Message, PRIORITY_OPENING, by_remit, restart_by, with_note};
use crate::plan::finish_plan;
use crate::submit::submitting;
use crate::wait::{WakeSubs, clock, wake_message};
use crate::watch::{watch_ended_text, watch_subscribed_text, watch_woke_text, watch_words};
use crate::{Error, Result};

/// Titles a chain: its Chair names what it does, first in its first turn for a chain asked for in the owner's words.
#[derive(Debug, Clone, Default)]
pub struct TitleCmd {
    pub title: String,
    pub why: String,
    /// The Chair's card.
    pub card_id: String,
    pub by: Actor,
}

/// Gives a queued chain its Chair and the Chair its first message.
#[derive(Debug, Clone, Default)]
pub struct StartCmd {
    pub chair_id: String,
    /// The Chair's first message.
    pub opening: String,
    /// The decision the owner reads: the chain started, of its kind.
    pub seed: String,
    /// Who asked for the chain.
    pub by: Actor,
}

#[derive(Debug, Clone, Default)]
pub struct PauseCmd {
    pub why: String,
    pub by: Actor,
}

#[derive(Debug, Clone, Default)]
pub struct ResumeCmd {
    pub why: String,
    pub by: Actor,
}

/// Stores the Chair's wake subscription, the plan of its return, in place of any it held; the chain keeps running until
/// the Chair's turn ends.
#[derive(Debug, Clone, Default)]
pub struct SubscribeCmd {
    /// A check-back keeps a schedule; a watch listens to a key.
    pub sub: Subscription,
}

/// What still needs the Chair when its turn ends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Need {
    #[default]
    Nothing,
    /// A message for the Chair is queued.
    MessageWaits,
    /// Other agents are at work or a submit runs: their results come to the Chair as messages.
    OthersWork,
}

/// Settles a planned wait once a turn has ended (`settle_wait`): the chain waits, or, when something still needs the
/// Chair, the wait is cancelled. With no wait planned, a chain whose only work left is its mentors' helps waits on them
/// (mentors, by name): a mentor's message wakes it.
#[derive(Debug, Clone, Default)]
pub struct TurnEndedCmd {
    /// Whose turn ended.
    pub card: String,
    pub need: Need,
    pub mentors: Vec<String>,
    /// With nothing else to wait for, every run of the chain waits only on a time, a signal or the owner
    /// (`runs_resting`): the chain waits on them, until `rest_until` (none: no time is known).
    pub runs_rest: bool,
    pub rest_until: Option<Time>,
}

/// Stores a change of one of the chain's runs of a script (workflow_run.rs): a call's result, the calls the run made, its
/// end or its stop, built under the chain's runs lock and stored with records. `issues` says it puts a card to work,
/// `tell` is what the Chair is told when the run ended or was stopped, `scheduled` that a wait's firing moved it (a
/// scheduled wake), `card` the card whose report it records. `rest` is observed: the run waits only (`after`), and so
/// does every other run of the chain, with nothing else at work, so the chain waits until `until`.
#[derive(Debug, Clone, Default)]
pub struct WorkflowCmd {
    pub run: String,
    pub card: String,
    pub after: model::WorkflowStatus,
    pub issues: bool,
    pub tell: Option<Message>,
    pub scheduled: bool,
    pub until: Option<Time>,
    pub rest: bool,
}

/// The firing of a wake subscription of a waiting chain: `sub` is the subscription that came due, `now` the instant the
/// scheduler judged it at, and `ended` the background tasks that ended when the chain waited (observed).
#[derive(Debug, Clone, Default)]
pub struct WakeCmd {
    pub sub: Subscription,
    pub now: Time,
    pub ended: Vec<String>,
    /// Its deadline fired it, with no reading.
    pub expired: bool,
    /// The readings of a watch that met its condition.
    pub readings: Vec<Reading>,
}

/// Gives the chain a capability it did not hold, on the owner's word: `why` is the Chair's reason, `card_id` the
/// Chair that asked, `by` the owner who gave it.
#[derive(Debug, Clone, Default)]
pub struct CapabilityCmd {
    pub capability: String,
    pub why: String,
    pub card_id: String,
    pub by: Actor,
}

/// The owner waking a waiting chain's Chair before its wake subscription is due.
#[derive(Debug, Clone, Default)]
pub struct CheckNowCmd {
    pub by: Actor,
}

/// Sends a message to a card of the chain: what reaches the Chair first ends its wake subscriptions, and the owner's
/// message reopens a finished chain.
#[derive(Debug, Clone, Default)]
pub struct SendCmd {
    pub msg: Message,
    /// Who sends it, for the owner (`Core::sender`).
    pub sender: String,
}

/// Finishes the chain with no ruling of the goal judge, `unjudged` saying why (a judged finish ends the goal check,
/// `EndGoalCheckCmd`); `text` and `why` are the decision the owner reads, by who finished it, of `kind` (finish when
/// unset). `plan_done` says the Chair asked to finish: the last step of its plan, finishing, is done.
#[derive(Debug, Clone, Default)]
pub struct FinishCmd {
    pub summary: String,
    pub text: String,
    pub why: String,
    pub card_id: String,
    pub by: Actor,
    /// Finish when none is named.
    pub kind: Option<DecisionKind>,
    pub unjudged: Option<Unjudged>,
    pub plan_done: bool,
}

/// Records the Chair's finish for a goal check: the summary waits on the chain for a judge's verdict, and the check's
/// deadline subscription is stored with it (ADR-0035).
#[derive(Debug, Clone, Default)]
pub struct RequestGoalCheckCmd {
    pub summary: String,
    pub attempt: i64,
    pub deadline: Subscription,
}

/// Has the goal judge (a card stored with the change) check the finish the chain waits on.
#[derive(Debug, Clone, Default)]
pub struct HireJudgeCmd {
    pub judge: String,
    pub name: String,
    pub brief: String,
    pub attempt: i64,
}

/// Has a goal judge look again at a finished chain whose change landed or was marked live since its last verdict
/// (ADR-0039, ADR-0067).
#[derive(Debug, Clone, Default)]
pub struct HireRecheckCmd {
    pub judge: String,
    pub name: String,
    pub brief: String,
    /// What the judge looks again after: the merge, or the live mark.
    pub after: String,
    pub deadline: Subscription,
}

/// Claims the finish the judge checks for its verdict, the approval stored with the change.
#[derive(Debug, Clone, Default)]
pub struct GoalVerdictCmd {
    pub judge: String,
    pub approval_id: String,
}

/// What of the goal judge's work came to an end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum JudgeEnd {
    /// Its turn on the check (its brief, or the word to go on) ended.
    Turn,
    /// Its turn on the owner's question to it ended: the judge of a chain checks each of its finishes (ADR-0132), so a
    /// new check may wait in its queue behind the answer.
    Question,
    /// It stopped, failed or ran out of time, or no judge was hired.
    #[default]
    Stopped,
}

/// Ends the goal check of `judge`: `why` says why when it gave no verdict, `end` what of its work ended. Observed under
/// the chain's lock: the verdict claimed on the check and how many finishes the goal judge has refused, that one
/// included.
#[derive(Debug, Clone, Default)]
pub struct EndGoalCheckCmd {
    pub judge: String,
    pub why: String,
    pub end: JudgeEnd,
    pub verdict: Option<Approval>,
    pub refusals: i64,
    /// What of the chain is the owner's to settle (`owners_to_settle`).
    pub owners: Vec<FindingBlocker>,
    /// What kept them from being observed.
    pub err: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ArchiveCmd {
    pub why: String,
    pub by: Actor,
}

#[derive(Debug, Clone, Default)]
pub struct UnarchiveCmd {
    pub why: String,
    pub by: Actor,
}

/// The edit a `WorkCmd` makes on the latest copy of a repository's record, or its refusal.
pub type RepoEdit = Arc<dyn Fn(&mut RepoWork) -> Result<(), String> + Send + Sync>;

/// Changes what the chain records of its work on one repository: `edit` makes the change on the latest copy, a new record
/// when there is none yet, or refuses it. `tell`, set when a submit ends, is how it went, for the Chair.
#[derive(Clone)]
pub struct WorkCmd {
    pub repo: String,
    pub edit: RepoEdit,
    pub tell: String,
}

/// The edit that makes the record rw: for the one writer of a record at that moment (open_repo, or the submit running on
/// it).
pub fn replace_with(rw: RepoWork) -> RepoEdit {
    Arc::new(move |x: &mut RepoWork| {
        *x = rw.clone();
        Ok(())
    })
}

/// Records that the change of a chain is live, with who said so and a note (live.rs): `already`, observed under the
/// chain's lock, says a change already marked live for its newest merge is marked once.
#[derive(Debug, Clone, Default)]
pub struct MarkLiveCmd {
    pub by: Actor,
    pub note: String,
    pub already: bool,
}

/// One command of the lifecycle.
#[derive(Clone)]
pub enum ChainCmd {
    Title(TitleCmd),
    Start(StartCmd),
    Pause(PauseCmd),
    Resume(ResumeCmd),
    Subscribe(SubscribeCmd),
    TurnEnded(TurnEndedCmd),
    Wake(WakeCmd),
    CheckNow(CheckNowCmd),
    Capability(CapabilityCmd),
    Send(SendCmd),
    Finish(FinishCmd),
    RequestGoalCheck(RequestGoalCheckCmd),
    HireJudge(HireJudgeCmd),
    HireRecheck(HireRecheckCmd),
    GoalVerdict(GoalVerdictCmd),
    EndGoalCheck(EndGoalCheckCmd),
    Archive(ArchiveCmd),
    Unarchive(UnarchiveCmd),
    /// Removes a finished chain.
    Delete,
    Work(WorkCmd),
    MarkLive(MarkLiveCmd),
    Workflow(WorkflowCmd),
}

impl fmt::Display for ChainCmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Title(_) => "title",
            Self::Start(_) => "start",
            Self::Pause(_) => "pause",
            Self::Resume(_) => "resume",
            Self::Subscribe(_) => "subscribe",
            Self::TurnEnded(_) => "settle the wait at a turn's end",
            Self::Wake(_) => "wake",
            Self::CheckNow(_) => "check back now",
            Self::Capability(_) => "take a capability",
            Self::Send(_) => "send",
            Self::Finish(_) => "finish",
            Self::RequestGoalCheck(_) => "request a goal check",
            Self::HireJudge(_) => "hire the goal judge",
            Self::HireRecheck(_) => "hire the goal judge to check again",
            Self::GoalVerdict(_) => "take the goal judge's verdict",
            Self::EndGoalCheck(_) => "end the goal check",
            Self::Archive(_) => "archive",
            Self::Unarchive(_) => "unarchive",
            Self::Delete => "delete",
            Self::Work(_) => "record repository work",
            Self::MarkLive(_) => "mark the change live",
            Self::Workflow(_) => "move a workflow",
        })
    }
}

/// Something that happened in the chain: what, why, the card it is about, and who did it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decided {
    pub kind: DecisionKind,
    pub text: String,
    pub why: String,
    pub card_id: String,
    pub by: Actor,
}

fn decided(kind: DecisionKind, text: impl Into<String>, why: impl Into<String>, card_id: &str, by: Actor) -> Decided {
    Decided {
        kind,
        text: text.into(),
        why: why.into(),
        card_id: card_id.to_owned(),
        by,
    }
}

/// What must happen once a change is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// The turns in progress end: the chain paused.
    CancelTurns,
    /// Every session closes until the wake subscription fires, and what the agents ran in the background ends with them.
    SleepSessions,
    /// The sessions whose agents run background work close: the chain paused, and holds all its work.
    StopBackground,
    /// Every session closes for good: the chain finished.
    EndSessions,
    /// Every session closes and is forgotten: the chain is deleted.
    DropSessions,
    /// A slot may be free: the scheduler looks again.
    FreeSlot,
    /// The chain's wait ended: its stall count starts over.
    ResetNudges,
    /// A wake subscription left the schedule: the scheduler looks again, so the schedule shows it.
    Reschedule,
    /// The worktrees collected while the chain was finished come back: it was reopened.
    RestoreWork,
}

/// A change to a card stored with the chain's, made on the card's latest copy.
#[derive(Clone)]
pub struct CardEdit {
    pub id: String,
    pub edit: Arc<dyn Fn(&mut Card) + Send + Sync>,
}

impl CardEdit {
    pub fn new(id: &str, edit: impl Fn(&mut Card) + Send + Sync + 'static) -> Self {
        Self {
            id: id.to_owned(),
            edit: Arc::new(edit),
        }
    }
}

/// What step decided: the chain's next state and what goes with it.
#[derive(Clone, Default)]
pub struct Transition {
    pub next: Chain,
    /// The chain's check-backs end with the change.
    pub end_waits: bool,
    /// The watches (by id) that end with it.
    pub end_watches: Vec<String>,
    /// The subscription it holds after it.
    pub new_wait: Option<Subscription>,
    /// When the sessions `SleepSessions` closes are woken; none when a mentor's message wakes them.
    pub sleep_until: Option<Time>,
    /// Deleted with everything under it.
    pub gone: bool,
    /// Who finished it, when the change does: nothing waits on it any more, so its queued messages are dropped, its open
    /// questions and errors cancelled, and the fact `chain.finished` is recorded with them.
    pub closed_out_by: Option<Actor>,
    pub decisions: Vec<Decided>,
    pub messages: Vec<Message>,
    pub effects: Vec<Effect>,
    /// Changes to cards stored with the chain's: the Chair's plan when it finishes.
    pub cards: Vec<CardEdit>,
    /// The subscriptions of Remit's own the change makes for the chain (a goal check's deadline).
    pub subs: Vec<Subscription>,
    /// The ones it ends, by id. A chain that is closed out or deleted ends them all.
    pub end_subs: Vec<String>,
}

impl Transition {
    /// Whether the change ends wake subscription s: every one when the chain is closed out, else the check-backs when
    /// `end_waits`, and the watches named in `end_watches`.
    pub fn ends(&self, s: &Subscription) -> bool {
        self.closed_out_by.is_some() || self.end_waits && s.schedule.is_some() || self.end_watches.contains(&s.id)
    }
}

/// What step answers a command it does not refuse.
pub enum Step {
    /// The chain already is what the command asks for: nothing is written.
    Unchanged,
    Changed(Box<Transition>),
}

/// Why a part of step stops: unchanged, or refused with the reason.
enum Halt {
    Unchanged,
    Refused(String),
}

type Stepped = std::result::Result<(), Halt>;

fn refused(c: &Chain, cmd: &ChainCmd) -> Halt {
    Halt::Refused(format!("the chain is {}: it cannot {cmd}", c.status()))
}

fn refuse(why: impl Into<String>) -> Halt {
    Halt::Refused(why.into())
}

/// A new chain, entering the lifecycle queued: the one status a chain is made with.
pub fn queued(mut c: Chain) -> Chain {
    c.set_status(ChainStatus::Queued);
    c
}

/// The chain's lifecycle; see the table at the top of this module.
pub fn step(c: &Chain, w: &WakeSubs, cmd: &ChainCmd, now: Time) -> Result<Step> {
    let mut t = Transition {
        next: c.clone(),
        ..Transition::default()
    };
    match step_to(&mut t, c, w, cmd, now) {
        Ok(()) => {
            t.next.updated_at = now;
            Ok(Step::Changed(Box::new(t)))
        }
        Err(Halt::Unchanged) => Ok(Step::Unchanged),
        Err(Halt::Refused(why)) => Err(Error::Refused(why)),
    }
}

fn step_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &ChainCmd, now: Time) -> Stepped {
    let status = c.status();
    match cmd {
        ChainCmd::Title(title) => {
            if c.title == title.title {
                return Err(Halt::Unchanged);
            }
            if matches!(status, ChainStatus::Queued | ChainStatus::Done) {
                return Err(refused(c, cmd));
            }
            t.next.title.clone_from(&title.title);
            t.decisions.push(decided(
                DecisionKind::Title,
                format!("Title the chain {:?}.", title.title),
                title.why.clone(),
                &title.card_id,
                title.by.clone(),
            ));
        }
        ChainCmd::Capability(capability) => {
            if c.capabilities.contains(&capability.capability) {
                return Err(Halt::Unchanged);
            }
            if *status == ChainStatus::Done {
                return Err(refused(c, cmd));
            }
            t.next.capabilities.push(capability.capability.clone());
            t.next.capabilities.sort();
            t.decisions.push(decided(
                DecisionKind::Capability,
                format!("Give the chain {}.", capability.capability),
                capability.why.clone(),
                &capability.card_id,
                capability.by.clone(),
            ));
        }
        ChainCmd::Start(cmd) => {
            if *status != ChainStatus::Queued {
                return Err(Halt::Unchanged); // started meanwhile, or finished while it waited
            }
            t.next.set_status(ChainStatus::Running);
            t.next.chair_id.clone_from(&cmd.chair_id);
            t.decisions.push(decided(
                DecisionKind::Seed,
                cmd.seed.clone(),
                "",
                &cmd.chair_id,
                cmd.by.clone(),
            ));
            if !cmd.opening.is_empty() {
                // none when it begins with a run: it goes with the run's end
                t.messages.push(Message {
                    to: cmd.chair_id.clone(),
                    title: "Start the chain".to_owned(),
                    text: cmd.opening.clone(),
                    by: by_remit(),
                    priority: PRIORITY_OPENING,
                    ..Message::default()
                });
            }
        }
        ChainCmd::Pause(pause) => {
            match status {
                ChainStatus::Paused => return Err(Halt::Unchanged),
                ChainStatus::Running | ChainStatus::Waiting => {}
                _ => return Err(refused(c, cmd)),
            }
            pause_to(
                t,
                decided(
                    DecisionKind::Pause,
                    "Pause chain.",
                    pause.why.clone(),
                    "",
                    pause.by.clone(),
                ),
            );
        }
        ChainCmd::Resume(resume) => {
            match status {
                ChainStatus::Running | ChainStatus::Waiting => return Err(Halt::Unchanged),
                ChainStatus::Paused => {}
                _ => return Err(refused(c, cmd)),
            }
            resume_to(t, c, w, resume, now);
        }
        ChainCmd::Subscribe(sub) => subscribe_to(t, c, cmd, sub, now)?,
        ChainCmd::TurnEnded(ended) => turn_ended_to(t, c, w, ended, now)?,
        ChainCmd::Wake(wake) => wake_to(t, c, w, wake, now)?,
        ChainCmd::Workflow(cmd) => workflow_to(t, c, w, cmd, now)?,
        ChainCmd::CheckNow(check) => {
            let soonest = w.earliest();
            match (status, &soonest) {
                (ChainStatus::Running, _) => return Err(Halt::Unchanged), // awake already
                (ChainStatus::Waiting, Some(_)) => {}
                _ => return Err(refused(c, cmd)),
            }
            let note = end_wait(
                t,
                c,
                w,
                "Checked back early: you asked.",
                "the owner asked you to check back now",
                check.by.clone(),
                now,
            );
            let planned = soonest.map(|s| s.sub.action.why).unwrap_or_default();
            t.messages.push(Message {
                to: c.chair_id.clone(),
                title: "Check back now".to_owned(),
                text: format!("{note}\nWhat you planned to look at: {planned}"),
                by: by_remit(),
                ..Message::default()
            });
        }
        ChainCmd::Send(send) => {
            let m = send.msg.clone();
            match status {
                ChainStatus::Queued => return Err(refuse("the chain has not started yet")),
                ChainStatus::Done => {
                    if !m.by.for_owner() {
                        return Err(refuse("the chain is finished"));
                    }
                    reopen_to(t, &m, &send.sender); // only the owner (himself, or the Assistant on his words) reopens it
                }
                _ => {}
            }
            queue(t, c, w, m, &send.sender, now);
        }
        ChainCmd::Finish(finish) => {
            if *status == ChainStatus::Done {
                return Err(refuse("the chain is already finished"));
            }
            let kind = finish.kind.unwrap_or(DecisionKind::Finish);
            finish_to(
                t,
                &finish.summary,
                finish.plan_done,
                finish.unjudged,
                decided(
                    kind,
                    finish.text.clone(),
                    finish.why.clone(),
                    &finish.card_id,
                    finish.by.clone(),
                ),
            );
        }
        ChainCmd::RequestGoalCheck(_)
        | ChainCmd::HireJudge(_)
        | ChainCmd::HireRecheck(_)
        | ChainCmd::GoalVerdict(_)
        | ChainCmd::EndGoalCheck(_) => goal_check_to(t, c, w, cmd, now)?,
        ChainCmd::Archive(archive) => {
            if *status != ChainStatus::Done {
                return Err(refuse("only a finished chain can be archived: finish it first"));
            }
            if c.archived_at.is_some() {
                return Err(Halt::Unchanged);
            }
            t.next.archived_at = Some(now);
            t.decisions.push(decided(
                DecisionKind::Archive,
                "Archive chain.",
                archive.why.clone(),
                "",
                archive.by.clone(),
            ));
        }
        ChainCmd::Unarchive(unarchive) => {
            if c.archived_at.is_none() {
                return Err(Halt::Unchanged);
            }
            t.next.archived_at = None;
            t.decisions.push(decided(
                DecisionKind::Unarchive,
                "Unarchive chain.",
                unarchive.why.clone(),
                "",
                unarchive.by.clone(),
            ));
        }
        ChainCmd::Delete => {
            if *status != ChainStatus::Done {
                return Err(refuse("only a finished chain can be deleted: finish it first"));
            }
            if submitting(c) {
                return Err(refuse(
                    "a change of this chain is being checked to land: try again when it is done",
                ));
            }
            t.gone = true;
            t.effects.extend([Effect::DropSessions, Effect::FreeSlot]);
        }
        ChainCmd::MarkLive(live) => {
            if *status == ChainStatus::Queued {
                return Err(refused(c, cmd));
            }
            if live.already {
                return Err(Halt::Unchanged);
            }
            t.decisions.push(decided(
                DecisionKind::Live,
                "The change is live.",
                live.note.clone(),
                "",
                live.by.clone(),
            ));
        }
        ChainCmd::Work(work) => {
            let at = c.repos.iter().position(|rw| rw.repo == work.repo);
            let mut rw = at.and_then(|i| c.repos.get(i)).cloned().unwrap_or_else(|| RepoWork {
                repo: work.repo.clone(),
                ..RepoWork::default()
            });
            (work.edit)(&mut rw).map_err(Halt::Refused)?;
            if work.tell.is_empty() && at.and_then(|i| c.repos.get(i)) == Some(&rw) {
                return Err(Halt::Unchanged); // the record stands as the edit leaves it
            }
            match at.and_then(|i| t.next.repos.get_mut(i)) {
                Some(slot) => *slot = rw,
                None => t.next.repos.push(rw),
            }
            if !work.tell.is_empty() && *status != ChainStatus::Queued && *status != ChainStatus::Done {
                // a finished chain's Chair is not woken by it
                let m = Message {
                    to: c.chair_id.clone(),
                    title: "Land result".to_owned(),
                    text: work.tell.clone(),
                    by: by_remit(),
                    ..Message::default()
                };
                queue(t, c, w, m, &format!("the submit of {} ended", work.repo), now);
            }
        }
    }
    Ok(())
}

/// The part of the lifecycle that is a finish's goal check (ADR-0035): asked for, given its judge, handed the judge's
/// verdict, and ended.
fn goal_check_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &ChainCmd, now: Time) -> Stepped {
    match cmd {
        ChainCmd::RequestGoalCheck(request) => {
            if *c.status() != ChainStatus::Running {
                return Err(refused(c, cmd));
            }
            if c.finishing.is_some() {
                return Err(refuse("the goal check is already judging a finish"));
            }
            t.next.finishing = Some(Finishing {
                summary: request.summary.clone(),
                at: now,
                ..Finishing::default()
            });
            t.subs.push(request.deadline.clone());
            t.decisions.push(decided(
                DecisionKind::Goal,
                format!("Goal check {}/{MAX_GOAL_CHECKS}: requested.", request.attempt),
                "",
                &c.chair_id,
                model::by_agent(&c.chair_id),
            ));
        }
        ChainCmd::HireJudge(hire) => {
            let Some(f) = c
                .finishing
                .as_ref()
                .filter(|f| *c.status() == ChainStatus::Running && f.judge.is_empty())
            else {
                return Err(Halt::Unchanged); // finished, ended or judged already
            };
            t.next.finishing = Some(Finishing {
                judge: hire.judge.clone(),
                ..f.clone()
            });
            t.decisions.push(decided(
                DecisionKind::Goal,
                format!(
                    "Goal check {}/{MAX_GOAL_CHECKS}: {} judges the result.",
                    hire.attempt, hire.name
                ),
                "",
                &hire.judge,
                by_remit(),
            ));
            t.messages.push(judge_brief(&hire.judge, &hire.brief));
        }
        ChainCmd::HireRecheck(recheck) => {
            if *c.status() != ChainStatus::Done || c.finishing.is_some() {
                return Err(Halt::Unchanged); // reopened since, or checked already
            }
            t.next.finishing = Some(Finishing {
                summary: c.summary.clone(),
                judge: recheck.judge.clone(),
                at: now,
                recheck: true,
                ..Finishing::default()
            });
            t.subs.push(recheck.deadline.clone());
            t.decisions.push(decided(
                DecisionKind::Goal,
                format!(
                    "Goal check after {}: {} judges the result again.",
                    recheck.after, recheck.name
                ),
                "",
                &recheck.judge,
                by_remit(),
            ));
            t.messages.push(judge_brief(&recheck.judge, &recheck.brief));
        }
        ChainCmd::GoalVerdict(verdict) => {
            let Some(f) = c.finishing.as_ref().filter(|f| f.judge == verdict.judge) else {
                return Err(refuse("no goal check is waiting for your verdict"));
            };
            if f.approval_id.is_some() {
                return Err(refuse("this goal check has your verdict already"));
            }
            t.next.finishing = Some(Finishing {
                approval_id: Some(verdict.approval_id.clone()),
                ..f.clone()
            });
        }
        ChainCmd::EndGoalCheck(end) => end_goal_check_to(t, c, w, end, now)?,
        _ => {}
    }
    Ok(())
}

fn judge_brief(judge: &str, brief: &str) -> Message {
    Message {
        to: judge.to_owned(),
        title: GOAL_JUDGE_TASK.to_owned(),
        text: brief.to_owned(),
        by: by_remit(),
        priority: PRIORITY_OPENING,
        ..Message::default()
    }
}

fn end_goal_check_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &EndGoalCheckCmd, now: Time) -> Stepped {
    let Some(f) = c.finishing.as_ref().filter(|f| f.judge == cmd.judge) else {
        return Err(Halt::Unchanged); // ended already, or another judge's
    };
    if let Some(err) = &cmd.err {
        return Err(refuse(err.clone()));
    }
    if cmd.end == JudgeEnd::Question && cmd.verdict.is_none() {
        return Err(Halt::Unchanged); // the check's own turn comes next, on its brief
    }
    let paused = *c.status() == ChainStatus::Paused;
    if cmd.verdict.is_some() || cmd.end != JudgeEnd::Turn || !paused {
        t.end_subs.push(goal_deadline_id(&c.id)); // the check is over: its deadline with it
    }
    let judged = model::by_agent(&cmd.judge);
    match &cmd.verdict {
        v if f.recheck => recheck_ended(t, v.as_ref(), cmd),
        None if cmd.end == JudgeEnd::Turn && paused => {
            // The pause cut its turn: the check waits with the chain, and the judge goes on when it resumes.
            t.messages.push(Message {
                to: cmd.judge.clone(),
                by: by_remit(),
                title: "Continue the goal check".to_owned(),
                text: "[the chain was paused while you judged, and has resumed] Continue your check where you stopped, then call record_verdict and end your turn.".to_owned(),
                ..Message::default()
            });
        }
        None => finish_to(
            t,
            &format!(
                "(The goal check could not run: {}.)\n\n{}",
                first_line(&cmd.why, 200),
                f.summary
            ),
            true,
            Some(Unjudged::Failed),
            decided(
                DecisionKind::GoalFailed,
                "Goal check failed: the chain finishes without it.",
                cmd.why.clone(),
                &cmd.judge,
                by_remit(),
            ),
        ),
        Some(v) if v.verdict == Verdict::Approve => {
            t.decisions.push(decided(
                DecisionKind::Goal,
                "Goal check: met.",
                first_line(&v.notes, 300),
                &cmd.judge,
                judged,
            ));
            finish_to(
                t,
                &f.summary,
                true,
                None,
                decided(
                    DecisionKind::Finish,
                    "Finish chain.",
                    finish_why(&f.summary),
                    &c.chair_id,
                    model::by_agent(&c.chair_id),
                ),
            );
        }
        Some(v) => {
            let missing = finding_lines(&still_missing(&v.findings, &cmd.owners));
            t.next.finishing = None;
            t.decisions.push(decided(
                DecisionKind::Goal,
                format!("Goal check: not met (attempt {}/{MAX_GOAL_CHECKS}).", cmd.refusals),
                missing.join("; "),
                &cmd.judge,
                judged,
            ));
            let m = Message {
                to: c.chair_id.clone(),
                by: by_remit(),
                title: "Goal check: something is missing".to_owned(),
                text: format!(
                    "[goal check: not met, {} of {MAX_GOAL_CHECKS}] An independent judge checked your result against the goal. Still to do:\n- {}\nDo these (hire, open another repository, verify), then finish_chain again. If one cannot be done because something outside the chain blocks it, ask the owner with ask_owner and subscribe; finish_chain again would only be judged the same. A change reaches its base only through land: once every gate approves the version, the queue lands it, and what it says on the chain is the landing's state.",
                    cmd.refusals,
                    missing.join("\n- ")
                ),
                ..Message::default()
            };
            queue(t, c, w, m, "the goal check", now);
        }
    }
    Ok(())
}

/// Ends the second look at a finished chain: its verdict is the chain's new one and nothing finishes; with none, the
/// earlier verdict stands.
fn recheck_ended(t: &mut Transition, v: Option<&Approval>, cmd: &EndGoalCheckCmd) {
    t.next.finishing = None;
    let Some(v) = v else {
        t.decisions.push(decided(
            DecisionKind::GoalFailed,
            "Goal check, looked at again, failed: the earlier verdict stands.",
            cmd.why.clone(),
            &cmd.judge,
            by_remit(),
        ));
        return;
    };
    t.next.unjudged = None; // the judge has ruled on the result as it is now
    let text = if v.verdict == Verdict::Approve {
        "Goal check, looked at again: met."
    } else {
        "Goal check, looked at again: not met."
    };
    t.decisions.push(decided(
        DecisionKind::Goal,
        text,
        first_line(&v.notes, 300),
        &cmd.judge,
        model::by_agent(&cmd.judge),
    ));
}

fn turn_ended_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &TurnEndedCmd, now: Time) -> Stepped {
    let soonest = w.earliest();
    if *c.status() != ChainStatus::Running || soonest.is_none() && cmd.mentors.is_empty() && !cmd.runs_rest {
        return Err(Halt::Unchanged); // nothing to wait for, or paused meanwhile: Resume settles it
    }
    if cmd.need != Need::Nothing && w.check_backs().is_empty() {
        return Err(Halt::Unchanged); // a watch alone has no check-back to cancel: it stays open, and the chain keeps running
    }
    match cmd.need {
        Need::Nothing => {
            t.next.set_status(ChainStatus::Waiting);
            // the chain waits on its check-back when one is held (its subscribe recorded the wait), else on the mentors
            // helping it: a message of theirs wakes it, and the owner reads whom it waits on as he reads a check-back's time
            match soonest {
                Some(s) => t.sleep_until = s.at,
                None if cmd.runs_rest => {
                    t.sleep_until = cmd.rest_until;
                    t.decisions.push(decided(
                        DecisionKind::Subscribe,
                        "Wait on its workflows.",
                        "",
                        &c.chair_id,
                        by_remit(),
                    ));
                }
                None => t.decisions.push(decided(
                    DecisionKind::Subscribe,
                    format!("Wait on {}.", cmd.mentors.join(", ")),
                    "",
                    &c.chair_id,
                    by_remit(),
                )),
            }
            t.effects.extend([Effect::SleepSessions, Effect::FreeSlot]);
        }
        Need::MessageWaits => {
            let note = end_wait(
                t,
                c,
                w,
                "Woke early: a new message arrived.",
                "a message reached you first",
                by_remit(),
                now,
            );
            t.messages.push(cancelled_check_back(c, note));
        }
        Need::OthersWork => {
            let note = end_wait(
                t,
                c,
                w,
                "Cancelled the check-back: other agents are still working.",
                "other agents are still working or a submit is running; their results come to you as messages",
                by_remit(),
                now,
            );
            t.messages.push(cancelled_check_back(c, note));
        }
    }
    Ok(())
}

fn cancelled_check_back(c: &Chain, note: String) -> Message {
    Message {
        to: c.chair_id.clone(),
        title: "Check-back cancelled".to_owned(),
        text: note,
        by: by_remit(),
        ..Message::default()
    }
}

/// Plans the Chair's wake or watch: a second wake with a time replaces the first, a watch only the same watch.
fn subscribe_to(t: &mut Transition, c: &Chain, cmd: &ChainCmd, s: &SubscribeCmd, now: Time) -> Stepped {
    if *c.status() != ChainStatus::Running {
        return Err(refused(c, cmd));
    }
    let sub = &s.sub;
    t.end_waits = sub.schedule.is_some();
    t.new_wait = Some(sub.clone());
    let text = if sub.watch() {
        watch_subscribed_text(sub, now)
    } else if let Some(at) = sub.schedule.as_ref().and(sub.due_at) {
        format!("Check back at {}.", clock(at, now))
    } else {
        "Watch.".to_owned()
    };
    t.decisions.push(decided(
        DecisionKind::Subscribe,
        text,
        sub.action.why.clone(),
        &c.chair_id,
        model::by_agent(&c.chair_id),
    ));
    Ok(())
}

/// Wakes the Chair of a waiting chain by the wake subscription that fired: the Chair plans again, so its check-backs
/// end, and a watch that fires ends with its firing.
/// Stores a change of a run: a waiting chain runs again when the run puts a card to work or its Chair is told of its end,
/// a running one waits when its runs wait only and nothing else works, and a paused one keeps what the run recorded.
fn workflow_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &WorkflowCmd, now: Time) -> Stepped {
    match c.status() {
        ChainStatus::Running => {
            if cmd.rest && cmd.tell.is_none() {
                t.next.set_status(ChainStatus::Waiting);
                t.sleep_until = cmd.until;
                t.decisions.push(decided(
                    DecisionKind::Subscribe,
                    "Wait on its workflows.",
                    "",
                    &c.chair_id,
                    by_remit(),
                ));
                t.effects.extend([Effect::SleepSessions, Effect::FreeSlot]);
            }
        }
        ChainStatus::Waiting => {
            if cmd.issues && cmd.tell.is_none() {
                t.next.set_status(ChainStatus::Running);
                t.decisions.push(decided(
                    DecisionKind::Wake,
                    "A workflow went on: it put agents to work.",
                    "",
                    &c.chair_id,
                    by_remit(),
                ));
                t.effects.extend([Effect::ResetNudges, Effect::Reschedule]);
            }
        }
        ChainStatus::Paused => {}
        _ => return Err(refused(c, &ChainCmd::Workflow(cmd.clone()))),
    }
    if let Some(m) = &cmd.tell {
        queue(t, c, w, m.clone(), "a workflow ended", now);
    }
    Ok(())
}

fn wake_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &WakeCmd, now: Time) -> Stepped {
    let Some(held) = w.by_id(&cmd.sub.id) else {
        return Err(Halt::Unchanged);
    };
    if *c.status() != ChainStatus::Waiting || held.sub.schedule.is_some() && held.at.is_some_and(|at| cmd.now < at) {
        return Err(Halt::Unchanged); // woken early meanwhile, or not due
    }
    t.next.set_status(ChainStatus::Running);
    t.end_waits = true;
    let mut text = "Checked back as scheduled.".to_owned();
    if held.sub.watch() {
        t.end_watches = vec![held.sub.id.clone()];
        text = watch_ended_text(held.sub.r#match.as_str());
        if let Some(last) = cmd.readings.last().filter(|_| !cmd.expired) {
            text = watch_woke_text(held.sub.r#match.as_str(), last.rev);
        }
    }
    let (title, body) = wake_message(&held, cmd, &cmd.ended, now);
    t.decisions.push(decided(
        DecisionKind::Wake,
        text,
        held.sub.action.why,
        &c.chair_id,
        by_remit(),
    ));
    t.messages.push(Message {
        to: c.chair_id.clone(),
        title,
        text: body,
        by: by_remit(),
        ..Message::default()
    });
    t.effects.push(Effect::ResetNudges);
    Ok(())
}

/// Pauses the chain, as d decides, holding all its work: the turns in progress end, the sessions whose agents run
/// background work close, as their news would start turns, and it gives up its slot. A waiting chain keeps its wake
/// subscription. The owner's pause is this one pause.
fn pause_to(t: &mut Transition, d: Decided) {
    t.next.set_status(ChainStatus::Paused);
    t.decisions.push(d);
    t.effects
        .extend([Effect::CancelTurns, Effect::StopBackground, Effect::FreeSlot]);
}

/// Resumes the paused chain c, which holds the wake subscriptions w, as cmd asks: a chain paused while waiting waits
/// again, with no session, until its subscription fires; any other runs again, and its Chair is told to restart the
/// background work the pause ended. A goal check it was judging starts its time again.
fn resume_to(t: &mut Transition, c: &Chain, w: &WakeSubs, cmd: &ResumeCmd, now: Time) {
    if let Some(f) = &c.finishing {
        // its judge stood still with the chain: its time starts again
        t.next.finishing = Some(Finishing { at: now, ..f.clone() });
        t.subs.push(goal_deadline(&c.id, now));
    }
    if let Some(soonest) = w.earliest() {
        // paused while waiting: wait again, with no session; its subscription wakes it when due
        t.next.set_status(ChainStatus::Waiting);
        t.sleep_until = soonest.at;
        t.decisions.push(decided(
            DecisionKind::Resume,
            "Resume waiting.",
            cmd.why.clone(),
            "",
            cmd.by.clone(),
        ));
        t.effects.extend([Effect::SleepSessions, Effect::FreeSlot]);
        return;
    }
    t.next.set_status(ChainStatus::Running);
    t.decisions.push(decided(
        DecisionKind::Resume,
        "Resume chain.",
        cmd.why.clone(),
        "",
        cmd.by.clone(),
    ));
    t.messages.push(Message {
        to: c.chair_id.clone(),
        by: by_remit(),
        title: "Resume the chain".to_owned(),
        text: format!(
            "[the chain was paused and is now resumed] {}\nThe pause ended what agents ran in the background: have them start again what they still need. Check list_agents and continue.",
            cmd.why
        ),
        ..Message::default()
    });
}

/// Puts a finished chain back to work on the owner's message m: back in the lists too, and with the worktrees collected
/// while it was finished.
fn reopen_to(t: &mut Transition, m: &Message, sender: &str) {
    t.next.set_status(ChainStatus::Running);
    t.next.archived_at = None;
    t.decisions.push(decided(
        DecisionKind::Reopen,
        format!("Reopen chain: {sender}."),
        "",
        &m.to,
        m.by.clone(),
    ));
    t.effects.push(Effect::RestoreWork);
}

/// Makes the chain done with summary, as d decides, `unjudged` saying why the goal judge did not rule on it (unset when
/// it did): nothing waits on it any more, and its sessions end. When the Chair asked for it (`plan_done`), finishing is
/// the last step of its plan, done.
/// A finish decision's why: the summary's first line, its outcome. The whole summary is the chain's result, shown beside
/// its history, so the decision does not carry it twice.
pub fn finish_why(summary: &str) -> String {
    first_line(summary, 200)
}

fn finish_to(t: &mut Transition, summary: &str, plan_done: bool, unjudged: Option<Unjudged>, d: Decided) {
    t.next.set_status(ChainStatus::Done);
    summary.clone_into(&mut t.next.summary);
    t.next.finishing = None;
    t.next.unjudged = unjudged;
    t.closed_out_by = Some(d.by.clone());
    t.decisions.push(d);
    t.effects.extend([Effect::EndSessions, Effect::FreeSlot]);
    if plan_done {
        t.cards.push(CardEdit::new(&t.next.chair_id.clone(), |c: &mut Card| {
            c.plan = finish_plan(&c.plan)
        }));
    }
}

/// Sends m: what reaches the Chair first ends the check-backs among the wake subscriptions w it holds (and wakes the
/// chain that waits), and says so in a note the Chair reads ahead of it (sender is who sent it, for the owner); a
/// restart's continuation does not.
fn queue(t: &mut Transition, c: &Chain, w: &WakeSubs, mut m: Message, sender: &str, now: Time) {
    // A watch alone is no wait a message ends while the Chair runs; it wakes the chain that waits on it.
    let check_backs = !w.check_backs().is_empty();
    let ends = check_backs || *c.status() == ChainStatus::Waiting;
    if m.to == c.chair_id && ends && m.by != restart_by() {
        let (owner_why, chair_reason) = if check_backs {
            (format!("Woke early: {sender}."), "a message reached you first")
        } else {
            // no check-back to come early to: the message, often the reply waited on, is what woke the wait
            (format!("A message woke the wait: {sender}."), "a message woke you")
        };
        let note = end_wait(t, c, w, &owner_why, chair_reason, m.by.clone(), now);
        m.data = Some(with_note(m.data.as_ref(), &note));
    }
    t.messages.push(m);
}

/// Ends the chain's check-backs, whether planned in the turn still running, waiting, or paused while waiting: a waiting
/// chain runs again, the others keep their status. A watch stays open. The owner reads `owner_why`, by who woke it; the
/// note it returns tells the Chair why.
fn end_wait(
    t: &mut Transition,
    c: &Chain,
    w: &WakeSubs,
    owner_why: &str,
    chair_reason: &str,
    by: Actor,
    now: Time,
) -> String {
    if *c.status() == ChainStatus::Waiting {
        t.next.set_status(ChainStatus::Running);
    }
    t.end_waits = true;
    t.decisions
        .push(decided(DecisionKind::Wake, owner_why, "", &c.chair_id, by));
    t.effects.extend([Effect::ResetNudges, Effect::Reschedule]);
    wait_ended_note(w, chair_reason, now)
}

/// Tells the Chair what happened to the wake subscriptions w: its check-back is cancelled, and each watch stays open.
fn wait_ended_note(w: &WakeSubs, reason: &str, now: Time) -> String {
    let mut b = match w.check_backs().earliest().and_then(|soonest| soonest.at) {
        Some(at) => format!(
            "[Remit: your check-back at {} is cancelled: {reason}. Call subscribe again if you still need to look later.",
            clock(at, now)
        ),
        None => format!("[Remit: you were waiting, and {reason}."),
    };
    for s in w.watches().iter() {
        b.push_str(&format!(
            " Your watch on {} stays open and wakes you when it fires.",
            watch_words(&s.sub, now)
        ));
    }
    b.push(']');
    b
}

#[cfg(test)]
mod tests;
