//! The pure core of the signal backbone (docs/adr/0014, docs/adr/0064): given one subscription, the readings it has not
//! judged and the time, it says what the subscription does, as a closed set of effects. It reads no clock, touches no
//! store and starts nothing; the orchestrator records what it answers and does what it says.

mod check;
mod condition;
mod cron;
mod evaluate;
mod instant;
mod probe;
mod replay;
mod schedule;
mod screen;
mod settle;

pub use condition::{ExprError, check, check_text, check_texts, eval_text, eval_texts};
pub use cron::{CronError, cron_next};
pub use evaluate::{ChainFacts, Effect, Fire, Hold, Input, Skip, apply, asks, evaluate};
pub use probe::{ProbeError, probe};
pub use replay::{Step, replay};
pub use schedule::{schedule_first, schedule_next};
pub use screen::{Screen, Verdict};
pub use settle::{DELIVERY_ATTEMPTS, DeliveryError, permanent, redo, retry, settle, skip_firing};
