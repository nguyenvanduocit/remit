//! A chain starts with the capabilities of its kind (ADR-0113, kinds.rs): a `work` chain with none. Claude Code is
//! started with Bash, WebFetch and WebSearch denied for the rest. When the work turns out to need a capability the
//! chain lacks, the Chair gives it any one of the catalog that does not write (grant_capability: it knows what its agents
//! need, so it decides), and asks for one that writes (request_capability: the Assistant's Apply for the owner
//! decides). Either way the chain's own writer stores it (`CapabilityCmd`), every session of the chain open then is
//! told its tools changed and lists them again (`Listeners`, tools.rs), and every session opened from then on holds
//! them. A capability that opens a Claude Code tool (Bash, WebFetch, WebSearch) is a deny rule a session starts with, so
//! the Chair's session is opened again for it instead.

use std::sync::Arc;

use model::{
    Actor, Card, Chain, ChainOrigin, ChangeKind, ChangedField, InboxItem, InboxKind, InboxOption, InboxStatus, Origin,
    PendingChange, by_agent,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Capabilities;
use crate::capabilities::{Capability, capability_of, gift_ids, no_capability, writer_ids, writes};
use chains::apply::Records;
use chains::chain_core::gist;
use chains::chain_core::new_id;
use chains::change_item::{CHANGE_APPLY, CHANGE_NOT_NOW, code_span};
use chains::changes::doc_of;
use chains::outbox::by_remit;
use chains::policy;
use chains::step::{CapabilityCmd, ChainCmd};
use chains::tools::{Effect, SelfCard, Tools};
use chains::{Error, Result};
use reach::credentials::answered;

/// The arguments of request_capability and grant_capability.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilityInput {
    /// any id of the catalog; shell (Bash: run commands in the chain's worktrees) and web (WebFetch and WebSearch) are
    /// the ones that open a Claude Code tool
    pub capability: String,
    /// why the chain needs it, in a sentence the owner reads beside it
    pub reason: String,
}

/// What a capability change item waits on: the chain, the capability, and the Chair's reason.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct CapabilityAsk {
    chain: String,
    capability: String,
    reason: String,
}

impl CapabilityAsk {
    fn name(&self) -> String {
        format!("{}/{}", self.chain, self.capability)
    }
}

/// How a capability given reached the chain's sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Every open session of the chain was told its tools changed and lists them again.
    Listed,
    /// The Chair's session was closed to open again with the Claude Code tool the capability opens.
    Reopened,
}

impl Reach {
    /// What the Chair is told of where the capability's tools are now.
    fn words(self) -> &'static str {
        match self {
            Self::Listed => "You and every agent of the chain hold its tools now, running or not.",
            Self::Reopened => {
                "Your session and every agent opened from now on hold the tool; an agent already running holds it once it is opened again."
            }
        }
    }
}

/// Says in words that the chain holds capability id.
fn held_already(id: &str) -> String {
    format!("Nothing to do: the chain already holds the {id} capability, and every agent of the chain holds its tools.")
}

/// The field of a chain's record that a capability moves: its capabilities, as they are and as they would become.
fn capability_field(held: &[String], id: &str) -> Result<Vec<ChangedField>> {
    let before = held.to_vec();
    let mut after = before.clone();
    after.push(id.to_owned());
    after.sort();
    Ok(vec![ChangedField {
        name: "capabilities".to_owned(),
        before: doc_of(Some(&before))?,
        after: doc_of(Some(&after))?,
    }])
}

fn capability_detail(by: &str, chain: &Chain, ask: &CapabilityAsk) -> String {
    let label = capability_of(&ask.capability)
        .map(|c| c.label.as_str())
        .unwrap_or_default();
    let outside = matches!(
        chain.origin,
        ChainOrigin::Proposed | ChainOrigin::Event | ChainOrigin::Client
    );
    let origin = if !outside || writes(std::slice::from_ref(&ask.capability)).is_empty() {
        String::new()
    } else {
        format!(
            "\n\nThis chain was started by {}, not on the owner's word: its goal was written by whoever started it, so it \
             started with no capability that writes. Apply only when the goal, as written, is one the owner would give that \
             power to.",
            origin_words(&chain.origin)
        )
    };
    format!(
        "{by} asks to give the chain {} the **{}** capability: {label}. It is a {} chain, which does not start with it, so no agent of the chain can use the tool.\
         \n\nWhy, in the Chair's words: {}{origin}\
         \n\nApply gives the chain the capability: every agent of the chain holds its tools from then on. Not now gives nothing, and the Chair is told.",
        code_span(&chain.title),
        ask.capability,
        code_span(chain.kind.as_str()),
        code_span(&gist(&ask.reason, 400)),
    )
}

/// Who started a chain of origin, in words. Pure.
fn origin_words(origin: &ChainOrigin) -> &'static str {
    match origin {
        ChainOrigin::Proposed => "another chain's proposal",
        ChainOrigin::Event => "an outside event",
        ChainOrigin::Client => "an outside agent holding a client key",
        ChainOrigin::Owner => "the owner",
        ChainOrigin::Routine => "one of the owner's routines",
        ChainOrigin::Verification => "a judge, to check a result",
        ChainOrigin::Replay => "Remit, replaying a recorded goal",
    }
}

fn option(id: &str, name: &str) -> InboxOption {
    InboxOption {
        id: id.to_owned(),
        name: name.to_owned(),
        ..InboxOption::default()
    }
}

/// Gives t request_capability, for the Chair me.
fn add_capability_request_tool(svc: &Arc<Capabilities>, t: &mut Tools, me: &SelfCard) {
    let me = me.clone();
    t.serving(svc).add(
        "request_capability", Effect::Writes,
        format!(
            "Ask for your chain a capability that writes ({}), when the work cannot be done without it. The capabilities that only read or run ({}) are yours to give with grant_capability: ask here only for the ones that write. Ask here, never with ask_owner. One item is raised in the owner's inbox, where the Assistant decides it for him: Apply or Not now, and a message starting [change result tells you which. Ask once and say why; until it is answered, do not ask again and do not work around it. After Apply you and every agent of the chain hold its tools.",
            writer_ids().join(", "),
            gift_ids().join(", ")
        ),
        move |o, i: CapabilityInput| {
            let me = me.clone();
            async move {
                let card = me.read(&o).await?;
                o.request_capability(&card, i).await
            }
        },
    );
}

/// Gives t grant_capability, for the Chair me.
fn add_capability_grant_tool(svc: &Arc<Capabilities>, t: &mut Tools, me: &SelfCard) {
    let me = me.clone();
    t.serving(svc).add(
        "grant_capability", Effect::Writes,
        format!(
            "Give your chain a capability it does not hold, when the work needs it. You know what your agents have to do, so this is yours to decide: no one is asked and it takes effect at once. It covers every capability that only reads or runs: {}. shell is Claude Code's Bash, to run the build, the tests and scripts in the chain's worktrees; web is WebFetch and WebSearch. Give it as soon as the work needs it, and say why in a sentence: the owner reads your reason in the chain's history. A capability that writes ({}) is the owner's: ask with request_capability. Its tools are listed to you and to every agent of the chain at once, running or not; shell and web, which are Claude Code's own tools, open your session again with them, and reach an agent already running once it is opened again.",
            gift_ids().join(", "),
            writer_ids().join(", ")
        ),
        move |o, i: CapabilityInput| {
            let me = me.clone();
            async move {
                let card = me.read(&o).await?;
                o.grant_capability(&card, i).await
            }
        },
    );
}

impl Capabilities {
    /// Gives t the two ways a Chair gives its chain a capability: grant_capability, on its own word, and
    /// request_capability, on the owner's.
    pub fn add_capability_giving_tools(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        add_capability_request_tool(self, t, me);
        add_capability_grant_tool(self, t, me);
    }

    /// Checks what a Chair asks of the catalog for its chain: the reason is given and the capability exists. The one
    /// reading of the arguments of grant_capability and request_capability. A capability bound to a kind is refused to a
    /// chain of another (`Capability::kind`). A chain a proposal, an event or a client started may ask for one that
    /// writes too: it started without (`kinds::may_not_start`), and the Assistant decides the ask with that origin in
    /// front of it (ADR-0087).
    async fn capability_wanted(
        &self,
        card: &Card,
        input: &CapabilityInput,
    ) -> Result<(&'static Capability, Chain, String)> {
        let reason = input.reason.trim().to_owned();
        if reason.is_empty() {
            return Err(Error::refused(
                "give the reason: the owner reads it beside the capability",
            ));
        }
        let id = input.capability.trim().to_lowercase();
        let c = capability_of(&id).ok_or_else(|| no_capability(&input.capability))?;
        let chain = self.chain(&card.home).await?;
        if let Some(kind) = c.kind.filter(|k| *k != chain.kind) {
            return Err(Error::refused(format!(
                "the {id} capability is the {kind} chain's own work, and this is a {} chain: no other kind holds it",
                chain.kind
            )));
        }
        Ok((c, chain, reason))
    }

    /// Asks the owner for the capability that writes which the Chair card of a chain needs, and says in words what came
    /// of it. A chain that holds it already raises nothing, and asking again while the owner has not answered raises no
    /// second item.
    pub(crate) async fn request_capability(&self, card: &Card, input: CapabilityInput) -> Result<String> {
        let (c, chain, reason) = self.capability_wanted(card, &input).await?;
        let id = c.id.as_str();
        if !c.writes {
            return Err(Error::refused(format!(
                "the {id} capability is yours to give: call grant_capability, it takes effect at once and needs no answer"
            )));
        }
        if chain.capabilities.iter().any(|h| h == id) {
            return Ok(held_already(id));
        }
        let ask = CapabilityAsk {
            chain: chain.id.clone(),
            capability: id.to_owned(),
            reason,
        };
        let item = InboxItem {
            id: new_id("i_"),
            home: chain.id.clone(),
            card_id: card.id.clone(),
            kind: InboxKind::Change,
            title: format!("{} asks for the {id} capability", card.name),
            detail: capability_detail(&card.name, &chain, &ask),
            options: vec![option(CHANGE_APPLY, "Apply"), option(CHANGE_NOT_NOW, "Not now")],
            status: InboxStatus::Open,
            created_at: self.now(),
            change: Some(PendingChange {
                kind: ChangeKind::Capability,
                name: ask.name(),
                record: doc_of(Some(&ask))?,
                fields: capability_field(&chain.capabilities, id)?,
            }),
            ..InboxItem::default()
        };
        let item = self.raise_change(item).await?;
        Ok(format!(
            "It waits in the owner's inbox ({}), where the Assistant decides it for him: Apply or Not now, and a message starting [change result tells you which. Until then do not ask again and do not work around it.",
            item.title
        ))
    }

    /// Gives the chain the capability a change item asks for, as the owner who answered it gave it. The decision is "this
    /// chain holds it": whatever else the chain holds by then does not change it, so two requests that wait together are
    /// both applied, in either order. The chain's own writer stores the capability with the closed item, and the chain's
    /// sessions are handed it (`hand_over_capability`).
    pub async fn apply_capability(&self, item: InboxItem, by: &Actor) -> Result<()> {
        if item.status != InboxStatus::Open {
            return Err(Error::refused(format!("item {} is {}", item.id, item.status)));
        }
        let record = item.change.as_ref().map(|c| c.record.clone()).unwrap_or_default();
        let ask: CapabilityAsk = serde_json::from_str(&record)?;
        let card_id = item.card_id.clone();
        let closed = answered(item, by.clone(), self.now(), "applied");
        let cmd = ChainCmd::Capability(CapabilityCmd {
            capability: ask.capability.clone(),
            why: ask.reason.clone(),
            card_id: card_id.clone(),
            by: by.clone(),
        });
        let w = Records {
            inbox: vec![closed.clone()],
            ..Records::default()
        };
        let a = self.apply(&ask.chain, cmd, w).await?;
        let (closed, reach) = if a.changed {
            let chair = self.store.read(move |r| r.card(&card_id)).await?;
            let reach = self.hand_over_capability(&chair, &ask.capability, "The owner").await?;
            (closed, reach)
        } else {
            // The chain held it already (another item, or its kind, gave it first) and a change that writes nothing
            // stores nothing, so the owner's answer is stored here: the item does not wait for one forever.
            let stored = self.store.update(move |tx| tx.put_inbox(closed)).await?;
            self.tell_inbox(std::slice::from_ref(&stored));
            (stored, Reach::Listed)
        };
        self.tell_owner_answered(
            &closed,
            &format!(
                "[change result · applied] The owner gave the chain the {} capability. {}",
                ask.capability,
                reach.words()
            ),
        )
        .await;
        Ok(())
    }

    /// Tells the Chair that asked what the owner answered.
    pub(crate) async fn tell_owner_answered(&self, item: &InboxItem, words: &str) {
        if let Err(err) = self
            .deliver(&item.card_id, "The owner answered a change", words, by_remit())
            .await
        {
            tracing::warn!(item = %item.id, %err, "tell the Chair what the owner answered");
        }
    }

    /// Hands the sessions of chair's chain capability id, which giver has just given it. One that opens a Claude Code
    /// tool changes the deny rules a session starts with (`policy::deny_for`), so the Chair's session is closed for the
    /// message that tells it to open it again with the tool. Any other is tools of Remit's own: every open session of the
    /// chain, the Chair's and its running agents', is told its tools changed and lists them again.
    async fn hand_over_capability(&self, chair: &Card, id: &str, giver: &str) -> Result<Reach> {
        let after = self.chain(&chair.home).await?;
        let mut before = after.clone();
        before.capabilities.retain(|c| c != id);
        if policy::deny_for(chair, Some(&before)) != policy::deny_for(chair, Some(&after)) {
            if let Some(r) = self.run_of(&chair.id) {
                self.close_sessions(
                    vec![r],
                    "",
                    &model::Cut::CapabilityGiven {
                        giver: giver.to_owned(),
                        capability: id.to_owned(),
                    },
                )
                .await;
            }
            return Ok(Reach::Reopened);
        }
        let home = chair.home.clone();
        let cards: Vec<String> = self
            .store
            .read(move |r| r.cards(&home))
            .await?
            .into_iter()
            .map(|c| c.id)
            .collect();
        self.listeners.tools_changed(&cards).await;
        Ok(Reach::Listed)
    }

    /// The Chair card of a chain giving its chain a capability that does not write, on its own word: the chain's writer
    /// stores it with a decision that names the Chair and its reason, and the chain's sessions are handed it. A chain that
    /// holds it already changes nothing. It runs to its end on the orchestrator's tracker: for shell or web the session the
    /// call came from is closed on the way, which may cut the call itself short.
    pub(crate) async fn grant_capability(self: &Arc<Self>, card: &Card, input: CapabilityInput) -> Result<String> {
        if card.origin != Origin::Chair {
            return Err(Error::refused("only the Chair gives its chain a capability"));
        }
        let (o, card) = (Arc::clone(self), card.clone());
        self.acting
            .spawn(async move { o.grant_capability_now(&card, input).await })
            .await
            .map_err(|e| Error::refused(format!("grant_capability: {e}")))?
    }

    async fn grant_capability_now(&self, card: &Card, input: CapabilityInput) -> Result<String> {
        let (c, chain, reason) = self.capability_wanted(card, &input).await?;
        if c.writes {
            return Err(Error::refused(format!(
                "the {} capability writes, so it is the owner's to give: ask with request_capability",
                c.id
            )));
        }
        if chain.capabilities.contains(&c.id) {
            return Ok(held_already(&c.id));
        }
        let cmd = ChainCmd::Capability(CapabilityCmd {
            capability: c.id.clone(),
            why: reason,
            card_id: card.id.clone(),
            by: by_agent(&card.id),
        });
        let a = self.apply(&chain.id, cmd, Records::default()).await?;
        if !a.changed {
            return Ok(held_already(&c.id));
        }
        let reach = self.hand_over_capability(card, &c.id, "The Chair").await?;
        if reach == Reach::Listed {
            return Ok(format!("The chain holds the {} capability. {}", c.id, reach.words()));
        }
        // The session this call came from is closed: the message below opens it again, with the tools.
        let words = format!(
            "[capability given] The chain holds the {} capability. {}",
            c.id,
            reach.words()
        );
        if let Err(err) = self
            .deliver(&card.id, "You gave the chain a capability", &words, by_remit())
            .await
        {
            tracing::warn!(card = %card.id, %err, "tell the Chair it gave a capability");
        }
        Ok(format!("The chain holds the {} capability.", c.id))
    }
}
