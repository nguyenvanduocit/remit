//! A processor is what a firing does (docs/adr/0049, docs/adr/0064): one for each name a subscription's action carries.
//! run, end and deadline are code; tell, wake, start and hire are an agent. They are in-process and found by action kind
//! (`processor_of`); a plugin's processor, `plugin:<id>.<name>`, is found the same way among the enabled plugins' (the
//! plugin pipeline), and nothing outside the lookup (`process`) asks which action a subscription holds. A delivery worker
//! (deliver.rs) hands a processor each firing with its Delivery; the processor stores its effect with the firing done
//! (`Delivery::done`, a Delivered) in one transaction, and says what it takes and how long a firing of it may take
//! (`ProcessorSpec`), so that a subscription it cannot do is refused when it is made.

use std::time::Duration;

use model::{ActionKind, Firing, FiringCause, HoldReason, OwnerKind, ProcessorTakes, Reading, Subscription, Time};

use crate::Pipeline;
use crate::plugin_pipeline::PROCESSOR_TIMEOUT;
use chains::admit::Room;
use chains::apply::Delivered;
use chains::apply::Records;
use chains::step::{ChainCmd, WakeCmd};
use chains::subscribe::builtin_takes;
use chains::subscribe::plugin_takes;
use chains::{Error, Result};

/// What a processor declares (docs/adr/0064): what a firing of it carries, and how long one may take. The delivery is
/// cut at `deadline`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessorSpec {
    pub takes: ProcessorTakes,
    pub deadline: Duration,
}

/// What a processor is given: the subscription as the worker read it, the firing as it is stored with the readings it
/// covers, the firing's key as text (the effect's idempotency key, the same on every attempt), the room admission looks
/// at, and the time.
#[derive(Debug, Clone)]
pub struct Delivery {
    pub sub: Subscription,
    pub firing: Firing,
    pub readings: Vec<Reading>,
    pub request_id: String,
    pub room: Room,
    pub now: Time,
}

impl Delivery {
    /// The firing of d done, saying outcome, with the chain and the card it acted on: what the processor stores with its
    /// effect.
    pub fn done(&self, outcome: &str, chain: &str, card: &str) -> Delivered {
        Delivered::new(self.firing.id(), outcome, chain, card)
    }

    /// The first reading the firing covers: a processor that takes one reading and no deadline is given one.
    pub fn reading(&self) -> Result<&Reading> {
        self.readings.first().ok_or(Error::NothingToDo)
    }
}

/// How long an in-process processor's firing may take: its writes to the store and, for a tell, a teaching or a job on
/// a chain, one call to a forge or OpenRouter, which answers in seconds.
pub(crate) const IN_PROCESS_DEADLINE: Duration = Duration::from_secs(30);

/// Says whether kind is an action that no subscription of a chain, an event signal or anyone else may hold: a hire, an
/// end, a run, a deadline and an assist reach into what Remit does on its own (ADR-0035, ADR-0062), and a plugin's
/// processor runs a program the owner applied.
pub fn remit_only(kind: &ActionKind) -> bool {
    matches!(
        kind,
        ActionKind::Hire | ActionKind::End | ActionKind::Run | ActionKind::Deadline | ActionKind::Assist
    ) || matches!(kind, ActionKind::Plugin(_))
}

impl Pipeline {
    /// The spec of the processor of kind: an in-process one's, or an enabled plugin's. None when none does it.
    pub(crate) fn processor_of(&self, kind: &ActionKind) -> Option<ProcessorSpec> {
        if let Some(takes) = builtin_takes(kind) {
            return Some(ProcessorSpec {
                takes,
                deadline: IN_PROCESS_DEADLINE,
            });
        }
        self.plugin_processing(kind.as_str()).map(|_| ProcessorSpec {
            takes: plugin_takes(),
            deadline: PROCESSOR_TIMEOUT,
        })
    }

    /// Hands the firing d names to the processor of its subscription's action, cut at that processor's deadline. Only
    /// Remit's own subscriptions hold the actions that reach into what it does on its own (`remit_only`). A missing
    /// processor and a refused action are failures no attempt mends (`Error::Permanent`): the subscription is held at
    /// once, with the reason.
    pub(crate) async fn process(&self, d: &Delivery) -> Result<()> {
        let kind = &d.sub.action.kind;
        let Some(spec) = self.processor_of(kind) else {
            return Err(Error::Permanent(
                HoldReason::ProcessorMissing,
                format!(
                    "subscription {}: no processor {:?}: the action is not one Remit or an enabled plugin does",
                    d.sub.id,
                    kind.as_str()
                ),
            ));
        };
        if remit_only(kind) && d.sub.owner.kind != OwnerKind::Remit {
            return Err(Error::Permanent(
                HoldReason::FiringFailed,
                format!("subscription {}: only Remit's own subscriptions {kind}", d.sub.id),
            ));
        }
        let deliver = async {
            match kind {
                ActionKind::Start => self.fire_start(d).await,
                ActionKind::Wake => self.fire_wake(d).await,
                ActionKind::Workflow => self.fire_workflow(d).await,
                ActionKind::Tell => self.fire_tell(d).await,
                ActionKind::Hire => self.actions()?.fire_hire(d).await,
                ActionKind::End => self.actions()?.fire_end(d).await,
                ActionKind::Run => self.fire_run(d).await,
                ActionKind::Deadline => self.actions()?.fire_deadline(d).await,
                ActionKind::Assist => self.actions()?.fire_assist(d).await,
                plugin => match self.plugin_processing(plugin.as_str()) {
                    Some(p) => self.fire_plugin(&p, d).await,
                    None => Err(Error::Permanent(
                        HoldReason::ProcessorMissing,
                        format!("subscription {}: no processor {kind}", d.sub.id),
                    )),
                },
            }
        };
        match tokio::time::timeout(spec.deadline, deliver).await {
            Ok(done) => done,
            Err(_) => Err(Error::refused(format!(
                "cut at its deadline of {}s: deadline exceeded",
                spec.deadline.as_secs()
            ))),
        }
    }

    /// The effect of a wake: the scheduler resumes the Chair of the waiting chain that sub belongs to, once its time has
    /// come. The chain runs, the Chair's wake message is queued, the wait's subscriptions end and the firing is stored, in
    /// one step through apply, whose admission weighs a wake by the clock (a slot and a provider unless the owner waits on
    /// the chain). A chain that is not waiting, or whose wait changed since the evaluation read it, is left as it is and
    /// the firing waits: it fires once the chain waits.
    async fn fire_wake(&self, d: &Delivery) -> Result<()> {
        let sub = &d.sub;
        let cmd = WakeCmd {
            sub: sub.clone(),
            now: d.now,
            expired: d.firing.cause == FiringCause::Expired,
            readings: d.readings.clone(),
            ..WakeCmd::default()
        };
        let w = Records {
            fired: Some(d.done("woke the Chair", &sub.owner.id, "")),
            ..Records::default()
        };
        let applied = {
            let _slot = self.slot_mu.lock().await; // its delivery is outside the Tick, whose queued starts take slots too
            self.apply(&sub.owner.id, ChainCmd::Wake(cmd), w).await
        };
        match applied {
            Ok(a) if !a.changed => Err(Error::NotAdmitted(format!(
                "chain {} is not waiting on it now",
                sub.owner.id
            ))),
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}
