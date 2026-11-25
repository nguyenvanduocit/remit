//! Reads of the whole app: what every channel loads (Catalog), what the board reads again of the chains a change moved
//! (Moved), and what the outcomes are computed from (Record). Each reads in one read transaction, as at one moment.

use model::{
    Agent, Approval, Card, Chain, Decision, DecisionKind, InboxItem, Landing, Playbook, SeededFault, Skill,
    Subscription, ToolTally,
};
use rusqlite::params_from_iter;

use crate::store::{Reader, Result, Store, card_doc, queued_params};

/// What the store holds for the whole app: every chain, newest first, what the chains subscribed to, the open inbox, the
/// playbooks, the library (agents, skills), the newest INSIGHTS_SHOWN insights, newest first, every chain's verdicts,
/// oldest first, without their notes and findings' evidence (what the board reads of them), and every chain's landings,
/// oldest first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    pub chains: Vec<Chain>,
    /// The chains' own subscriptions, by id.
    pub waits: Vec<Subscription>,
    pub inbox: Vec<InboxItem>,
    pub playbooks: Vec<Playbook>,
    pub agents: Vec<Agent>,
    pub skills: Vec<Skill>,
    pub insights: Vec<Decision>,
    pub verdicts: Vec<Approval>,
    pub landings: Vec<Landing>,
}

/// What the board reads again of the chains a change may have moved: those of them still stored, newest first, what they
/// subscribed to, their verdicts and their landings, oldest first, as Catalog reads them, and the open inbox.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Moved {
    pub chains: Vec<Chain>,
    pub waits: Vec<Subscription>,
    pub verdicts: Vec<Approval>,
    pub landings: Vec<Landing>,
    pub inbox: Vec<InboxItem>,
}

/// Everything recorded across chains that says how the agents did: every chain and what it subscribed to, every card,
/// every verdict, the decisions of the kinds asked for, every landing, and every card's tool calls counted per tool.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Record {
    pub chains: Vec<Chain>,
    /// What the chains subscribed to.
    pub waits: Vec<Subscription>,
    pub cards: Vec<Card>,
    pub approvals: Vec<Approval>,
    pub decisions: Vec<Decision>,
    pub landings: Vec<Landing>,
    pub tool_calls: Vec<ToolTally>,
    /// The faults planted in trial chains (ADR-0102).
    pub seeded: Vec<SeededFault>,
}

/// A verdict without its notes, and its findings without their evidence: what the board reads of it is what each finding
/// checked (item), found (status) and waits on (blocker), which says whether a finished chain is Done, Waiting on you or
/// Not done, and what is not proven (ADR-0067). A part a finding lacks is left out (json_patch drops a null), not
/// written null, which a typed field refuses.
macro_rules! verdict_doc {
    () => {
        "CASE WHEN json_type(doc, '$.findings') = 'array'
	THEN json_set(json_remove(doc, '$.notes', '$.findings'), '$.findings', json((SELECT json_group_array(json_patch('{}', json_object('item', json_extract(f.value, '$.item'), 'status', json_extract(f.value, '$.status'), 'blocker', json_extract(f.value, '$.blocker')))) FROM json_each(doc, '$.findings') AS f)))
	ELSE json_remove(doc, '$.notes') END"
    };
}

/// How many of the analyst's insights the app is given: the newest, far more than a week of them.
const INSIGHTS_SHOWN: i64 = 30;

fn marks(n: usize) -> String {
    vec!["?"; n].join(",")
}

impl Store {
    /// Reads the chains ids names again, with what the board reads beside them, in one read transaction: a chain not
    /// read was deleted.
    pub async fn moved(&self, ids: Vec<String>) -> Result<Moved> {
        self.read(move |r| {
            let m = marks(ids.len());
            Ok(Moved {
                chains: r.list(
                    &format!("SELECT doc FROM chains WHERE id IN ({m}) ORDER BY created_at DESC"),
                    params_from_iter(&ids),
                )?,
                waits: r.list(
                    &format!(
                        "SELECT doc FROM subscriptions WHERE owner_kind='chain' AND owner_id IN ({m}) ORDER BY id"
                    ),
                    params_from_iter(&ids),
                )?,
                verdicts: r.list(
                    &[
                        concat!("SELECT ", verdict_doc!(), " FROM approvals WHERE chain_id IN ("),
                        &m,
                        ") ORDER BY at, rowid",
                    ]
                    .concat(),
                    params_from_iter(&ids),
                )?,
                landings: r.list(
                    &format!("SELECT doc FROM landings WHERE chain_id IN ({m}) ORDER BY created_at, rowid"),
                    params_from_iter(&ids),
                )?,
                inbox: r.open_inbox()?,
            })
        })
        .await
    }

    /// Reads the whole app's records in one read transaction, as they were at one moment.
    pub async fn catalog(&self) -> Result<Catalog> {
        self.read(|r| {
            Ok(Catalog {
                chains: r.chains()?,
                waits: r.chain_subscriptions()?,
                inbox: r.open_inbox()?,
                insights: r.recent_decisions(&DecisionKind::Insight, INSIGHTS_SHOWN)?,
                verdicts: r.list(
                    concat!("SELECT ", verdict_doc!(), " FROM approvals ORDER BY at, rowid"),
                    [],
                )?,
                landings: r.landings()?,
                playbooks: r.playbooks()?,
                agents: r.agents()?,
                skills: r.skills()?,
            })
        })
        .await
    }

    /// Reads the whole record in one read transaction, as it was at one moment, with the decisions of kinds.
    pub async fn record(&self, kinds: Vec<DecisionKind>) -> Result<Record> {
        self.read(move |r| {
            Ok(Record {
                chains: r.chains()?,
                waits: r.chain_subscriptions()?,
                cards: r.list(
                    concat!("SELECT ", card_doc!(), " FROM cards ORDER BY created_at, rowid"),
                    &*queued_params(&[]),
                )?,
                approvals: r.list("SELECT doc FROM approvals ORDER BY at, rowid", [])?,
                decisions: r.decisions_of_kinds(&kinds)?,
                landings: r.landings()?,
                tool_calls: r.tool_tallies()?,
                seeded: r.seeded_faults()?,
            })
        })
        .await
    }
}

impl Reader<'_> {
    /// Every chain's decisions of the given kinds, oldest first; none for no kinds.
    fn decisions_of_kinds(&self, kinds: &[DecisionKind]) -> Result<Vec<Decision>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        self.list(
            &format!(
                "SELECT doc FROM decisions WHERE kind IN ({}) ORDER BY created_at, rowid",
                marks(kinds.len())
            ),
            params_from_iter(kinds.iter().map(DecisionKind::as_str)),
        )
    }
}
