//! A change that moves a rail waits for the owner. rail.rs says which writes do. When an agent writes one, nothing is
//! stored: an inbox item of kind `change` asks the owner, who answers Apply or Not now, and the agent is told what he
//! answered. The owner, the Assistant for him and Remit itself write at once: a write of theirs is not an agent's. Only
//! the owner answers the item, or the Assistant for him (ADR-0062).

use model::{Actor, ChangeKind, ChangedField, InboxItem, InboxKind, InboxOption, InboxStatus, PendingChange};

use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::fact::put_asked_fact;
use crate::{Error, Result};

/// The answer that stores the write as it was asked.
pub const CHANGE_APPLY: &str = "apply";
/// The answer that stores nothing.
pub const CHANGE_NOT_NOW: &str = "not_now";

/// What the tool that asked tells the agent when its write waits for the owner (err is `Error::WaitingForOwner`);
/// none for any other error.
pub fn held_words(err: &Error) -> Option<String> {
    let Error::WaitingForOwner(item) = err else {
        return None;
    };
    Some(format!(
        "Nothing was written: this moves a rail, so it waits in the owner's inbox ({}), where the Assistant decides it for \
         him: Apply or Not now, and a message starting [change result tells you which. Until then do not write it \
         again; finish_chain waits for that answer, so use subscribe to come back later.",
        item.title
    ))
}

/// What a stored change did once its transaction commits: tells the channels of the record it wrote.
pub type Tell = Box<dyn FnOnce(&Core) + Send>;

/// What applying a change in its transaction came to.
pub enum Applied {
    /// The record is stored, with its change; the channels are told by the tell, if any, once the transaction commits.
    Written(Option<Tell>),
    /// The record is not the one the item was made against, so nothing is stored.
    Moved,
}

impl Core {
    /// Opens the item for the write of record (JSON) to the kind and name that moves the rail fields (`raise_change`: one
    /// that asks the same stays, one that asks differently is replaced), and says so with `Error::WaitingForOwner`. No
    /// fields: no item, and Ok.
    pub async fn hold_change(
        &self,
        by: &Actor,
        kind: ChangeKind,
        name: &str,
        record: String,
        fields: Vec<ChangedField>,
    ) -> Result<()> {
        if fields.is_empty() {
            return Ok(());
        }
        let who = by.who.clone();
        let card = match self.store.read(move |r| r.card(&who)).await {
            Ok(c) => c,
            Err(err) => {
                return Err(Error::refused(format!(
                    "a write that moves a rail waits in the owner's inbox, and the agent that asks is not one of his: {err}"
                )));
            }
        };
        let item = InboxItem {
            id: new_id("i_"),
            home: card.home.clone(),
            card_id: card.id.clone(),
            kind: InboxKind::Change,
            title: format!("{} asks to write {} {name}", card.name, kind_words(&kind)),
            detail: change_detail(&card.name, &kind, name, &fields),
            options: vec![option(CHANGE_APPLY, "Apply"), option(CHANGE_NOT_NOW, "Not now")],
            status: InboxStatus::Open,
            created_at: self.now(),
            change: Some(PendingChange {
                kind,
                name: name.to_owned(),
                record,
                fields,
            }),
            ..InboxItem::default()
        };
        let item = self.raise_change(item).await?;
        Err(Error::WaitingForOwner(Box::new(item)))
    }

    /// Stores item, a change item that asks the owner about the record named by its change, and tells the channels. An
    /// item of the same card that already asks the same about that record is returned as it is, so asking twice raises
    /// one item; one that asks about the record differently is cancelled and replaced, so the owner answers the newest.
    pub async fn raise_change(&self, item: InboxItem) -> Result<InboxItem> {
        let door = self.door();
        let (item, told) = self
            .store
            .update(move |tx| -> Result<(InboxItem, Vec<InboxItem>)> {
                let Some(asks) = item.change.clone() else {
                    return Err(Error::refused("a change item carries the write it asks about"));
                };
                let mut told = Vec::new();
                for mut it in tx.open_inbox()? {
                    let Some(other) = it.change.as_ref() else { continue };
                    if it.kind != InboxKind::Change || other.kind != asks.kind || other.name != asks.name {
                        continue;
                    }
                    if it.card_id == item.card_id && other.record == asks.record && other.fields == asks.fields {
                        return Ok((it, Vec::new()));
                    }
                    it.status = InboxStatus::Cancelled;
                    "a newer change to the same record replaced it".clone_into(&mut it.answer);
                    told.push(tx.put_inbox(it)?);
                }
                let item = tx.put_inbox(item)?;
                if !item.owner_only {
                    put_asked_fact(tx, &door, &item)?;
                }
                told.push(item.clone());
                Ok((item, told))
            })
            .await?;
        self.tell_inbox(&told);
        self.kick(); // the Assistant is told on the Tick
        Ok(item)
    }
}

/// An option of a change item.
fn option(id: &str, name: &str) -> InboxOption {
    InboxOption {
        id: id.to_owned(),
        name: name.to_owned(),
        ..InboxOption::default()
    }
}

/// A record's kind, named for the owner.
pub fn kind_words(kind: &ChangeKind) -> String {
    match kind {
        ChangeKind::Agent => "agent",
        ChangeKind::Skill => "skill",
        ChangeKind::Playbook => "playbook",
        ChangeKind::Grant => "grant",
        ChangeKind::Sensor => "sensor",
        ChangeKind::EventSignal => "event signal",
        ChangeKind::Extension => "extension",
        ChangeKind::Objective => "objective",
        ChangeKind::Employee => "employee",
        ChangeKind::Forget => "forget of memory",
        ChangeKind::Promote => "sharing of a memory",
        ChangeKind::Capability => "capability",
        ChangeKind::Project => "project",
        ChangeKind::KeyResult => "key result",
    }
    .to_owned()
}

/// What the owner reads under a change item: who asks, for what, and the fields that move a rail, each as it is and as
/// it would be. A value shown as a block ends on its closing fence, and a fence with words after it does not close, so
/// the arrow after a block starts a line of its own.
pub fn change_detail(by: &str, kind: &ChangeKind, name: &str, fields: &[ChangedField]) -> String {
    let mut b = format!(
        "{by} asks to write {} {}. These fields move a rail, so it is not written until you answer:\n",
        kind_words(kind),
        code_span(name)
    );
    for f in fields {
        if f.before.is_empty() {
            b.push_str(&format!("\n- **{}**: {}", f.name, shown(&f.after)));
            continue;
        }
        let before = shown(&f.before);
        let arrow = if before.contains('\n') { "\n\n  → " } else { " → " };
        b.push_str(&format!("\n- **{}**: {before}{arrow}{}", f.name, shown(&f.after)));
    }
    b.push_str("\n\nApply writes it as shown. Not now writes nothing, and the agent is told.");
    b
}

/// A field's value for the owner to read: in a code span, in a code block when it is several lines (a program is read
/// line by line, not on one), or "nothing" for a value that is empty.
pub fn shown(v: &str) -> String {
    let p = plain_json(v);
    if p.is_empty() {
        return "nothing".to_owned();
    }
    if p.contains('\n') {
        return code_block(&p);
    }
    code_span(&p)
}

/// s as a fenced Markdown block under a list item, whatever backticks it holds.
fn code_block(s: &str) -> String {
    let mut fence = "```".to_owned();
    while s.contains(&fence) {
        fence.push('`');
    }
    let mut b = format!("\n\n  {fence}\n");
    for line in s.trim_end_matches('\n').split('\n') {
        b.push_str(&format!("  {line}\n"));
    }
    b.push_str(&format!("  {fence}"));
    b
}

/// A value as a person reads it: a JSON string without its quotes, anything else as it is.
fn plain_json(v: &str) -> String {
    serde_json::from_str::<String>(v).unwrap_or_else(|_| v.to_owned())
}

/// s in a Markdown code span, whatever backticks it holds.
pub fn code_span(s: &str) -> String {
    let mut fence = "`".to_owned();
    while s.contains(&fence) {
        fence.push('`');
    }
    if s.starts_with('`') || s.ends_with('`') {
        return format!("{fence} {s} {fence}");
    }
    format!("{fence}{s}{fence}")
}
