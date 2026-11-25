//! The Assistant's feed is what it decided for the owner (ADR-0062), on its own screen: each answer or item it left to him,
//! newest first, with a second judge's dissent from it (ADR-0103) and his overturn when there is one, and how many came
//! since he last looked (his mark on the feed, seen.rs). The server lays it out whole and pushes it whole
//! (`assistant_feed_updated`) whenever it moves: a decision, a dissent, an overturn, his look.

use std::collections::HashMap;

use model::{Decision, DecisionKind, InboxItem, InboxStatus, SEEN_ASSISTANT_FEED, Time};
use wire::app::{AssistantFeed, FeedLine};

use crate::orch::Orchestrator;
use chains::Result;
use chains::inbox::answered_by_assistant;

/// How many of the Assistant's decisions the feed shows: the newest, well over a week of them.
const FEED_SHOWN: i64 = 50;

/// What the feed shows of the item a decision answered: whether the Assistant answered it, and when its answer takes
/// effect while it waits out the overturn window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Answered {
    by_assistant: bool,
    takes_effect_at: Option<Time>,
}

impl Answered {
    /// Item it as its feed line reads it. Pure.
    fn of(it: &InboxItem) -> Self {
        Self {
            by_assistant: answered_by_assistant(it),
            takes_effect_at: it.takes_effect_at.filter(|_| it.status == InboxStatus::Decided),
        }
    }
}

/// Lays out the feed from the Assistant's newest decisions, the second judges' newest dissents and the owner's newest
/// overturns (all newest first), the items it answered for him (by id), as he last looked at seen_at (none: never): a
/// dissent he has not seen counts as new too. Pure.
fn feed_of(
    assists: Vec<Decision>,
    dissents: Vec<Decision>,
    overturns: Vec<Decision>,
    answered: &HashMap<String, Answered>,
    seen_at: Option<Time>,
) -> AssistantFeed {
    let mut by: HashMap<String, Decision> = HashMap::new();
    for ov in overturns {
        by.insert(ov.overturns.clone(), ov);
    }
    let mut dissent_on: HashMap<String, Decision> = HashMap::new();
    for d in dissents {
        dissent_on.insert(d.item.clone(), d);
    }
    let mut f = AssistantFeed {
        lines: Vec::with_capacity(assists.len()),
        unseen: 0,
    };
    for d in assists {
        let a = answered.get(&d.item).copied().unwrap_or_default();
        let dissent = if a.by_assistant {
            dissent_on.remove(&d.item)
        } else {
            None
        };
        let new = std::iter::once(d.created_at).chain(dissent.as_ref().map(|x| x.created_at));
        f.unseen = f.unseen.saturating_add(
            i64::try_from(new.filter(|at| seen_at.is_none_or(|seen| *at > seen)).count()).unwrap_or(i64::MAX),
        );
        f.lines.push(FeedLine {
            answered: a.by_assistant,
            takes_effect_at: a.takes_effect_at,
            dissent,
            overturn: by.remove(&d.id),
            decision: d,
        });
    }
    f
}

impl Orchestrator {
    /// The feed as stored now.
    pub async fn assistant_feed(&self) -> Result<AssistantFeed> {
        let (seen_at, assists, dissents, overturns, answered) = self
            .store
            .read(|r| {
                let seen_at = r.seen(SEEN_ASSISTANT_FEED)?.map(|s| s.at);
                let assists = r.recent_decisions(&DecisionKind::Assist, FEED_SHOWN)?;
                let dissents = r.recent_decisions(&DecisionKind::Dissent, FEED_SHOWN)?;
                let overturns = r.recent_decisions(&DecisionKind::Overturn, FEED_SHOWN)?;
                let mut answered = HashMap::new();
                for d in &assists {
                    match r.inbox_item(&d.item) {
                        Ok(it) => {
                            answered.insert(it.id.clone(), Answered::of(&it));
                        }
                        Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                Ok::<_, store::Error>((seen_at, assists, dissents, overturns, answered))
            })
            .await?;
        Ok(feed_of(assists, dissents, overturns, &answered, seen_at))
    }

    /// Tells the channels the feed as it stands; a read that fails is logged, and the next move pushes it again.
    pub(crate) async fn push_feed(&self) {
        match self.assistant_feed().await {
            Ok(f) => self.notify.assistant_feed_updated(&f),
            Err(err) => tracing::warn!(%err, "the Assistant's feed not read"),
        }
    }
}
