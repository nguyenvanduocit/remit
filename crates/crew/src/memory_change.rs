//! Forget: the one way Remit removes an employee's memory. The Assistant, or the coach (coach.rs), drafts a change item
//! naming memories the employee holds (whatever their scope or status: the employee's to forget, never what it only
//! reads of its project, lineage or everyone) it shows the owner; only his answer forgets them, exactly those, in the
//! transaction that stores it (`apply_forget`), and the links to them go with them.
//! Pure: `claim_of`, `forget_fields`, `named_memories`, `forget_list`.

use model::{
    Actor, ActorKind, ChangeKind, ChangedField, Employee, InboxItem, InboxKind, InboxOption, InboxStatus, Memory,
    Origin, PendingChange,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use store::Tx;

use crate::Crew;
use crate::coach::is_coaching;
use crate::employee_change::require_owner;
use chains::chain_core::gist;
use chains::chain_core::new_id;
use chains::change_item::{Applied, CHANGE_APPLY, CHANGE_NOT_NOW, code_span};
use chains::changes::{Authorship, doc_of};
use chains::{Error, Result};

/// What draft_forget takes: whose memories, which, and why they go.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DraftForgetInput {
    /// the id of the employee whose memory it is
    pub employee: String,
    /// the memories to forget, by id, as read_memory shows them
    pub ids: Vec<String>,
    /// why they go, in the owner's words, in a line he reads
    pub why: String,
}

/// The record of a forget item: whose memory, the memories the owner was shown, and why they go.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ForgetRecord {
    employee: String,
    ids: Vec<String>,
    why: String,
}

/// What a memory claims, as a change item shows it and its apply compares it: its title, body, scope and status. A turn
/// recalling it or rating it changes none of them, so an item stays the one the owner was shown while the memory is used.
#[derive(Serialize)]
struct Claim<'a> {
    title: &'a str,
    body: &'a str,
    scope: model::MemoryScope,
    status: model::MemoryStatus,
}

/// m's claim (`Claim`), as a change item's field holds it. Pure.
pub(crate) fn claim_of(m: &Memory) -> Result<String> {
    doc_of(Some(&Claim {
        title: &m.title,
        body: &m.body,
        scope: m.scope,
        status: m.status(),
    }))
}

/// What the owner is shown: each memory, by id and title, as it claims, going to nothing. They are also how apply knows
/// the memories still claim what he saw.
pub(crate) fn forget_fields(mems: &[Memory]) -> Result<Vec<ChangedField>> {
    mems.iter()
        .map(|m| {
            Ok(ChangedField {
                name: format!("{} {}", m.id, m.title),
                before: claim_of(m)?,
                after: String::new(),
            })
        })
        .collect()
}

/// The memories among mems that ids name, in the order of ids; an error names an id that is not one of them.
pub(crate) fn named_memories(mems: &[Memory], ids: &[String]) -> Result<Vec<Memory>> {
    ids.iter()
        .map(|id| {
            mems.iter().find(|m| &m.id == id).cloned().ok_or_else(|| {
                Error::refused(format!(
                    "{id:?} is not a memory this employee holds: read_memory lists them, each with its holder; what it reads of its lineage or everyone is forgotten from the employee that holds it"
                ))
            })
        })
        .collect()
}

/// The memories as the owner reads them under the item.
pub(crate) fn forget_list(mems: &[Memory]) -> String {
    mems.iter()
        .map(|m| format!("- **{}** · {}: {}", m.title, m.kind, code_span(&gist(&m.body, 300))))
        .collect::<Vec<_>>()
        .join("\n")
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

/// Stores employee id again in tx, so its rev moves and a page showing it reads its memory again; it is returned as
/// stored.
pub(crate) fn touch_employee(tx: &Tx<'_>, id: &str) -> Result<Employee> {
    tx.put_employee(tx.employee(id)?)?;
    Ok(tx.employee(id)?)
}

impl Crew {
    /// Who by is as a drafter of a forget, as the owner reads it: the Assistant, or the Chair of a coaching chain; anyone
    /// else drafts none.
    async fn forget_drafter(&self, by: &Actor) -> Result<&'static str> {
        let refused = || Error::refused("only the Assistant and the coach draft a forget");
        match by.kind {
            ActorKind::Assistant => Ok("The Assistant"),
            ActorKind::Agent => {
                let id = by.who.clone();
                let coach = self
                    .store
                    .read(move |r| -> Result<bool> {
                        let c = r.card(&id)?;
                        Ok(c.origin == Origin::Chair && is_coaching(&r.chain(&c.home)?))
                    })
                    .await
                    .unwrap_or(false);
                if coach { Ok("The coach") } else { Err(refused()) }
            }
            _ => Err(refused()),
        }
    }

    /// Raises the change item that asks the owner to forget memories of an employee, as by: the Assistant or the coach.
    pub(crate) async fn draft_forget(&self, by: &Actor, i: DraftForgetInput) -> Result<String> {
        let drafter = self.forget_drafter(by).await?;
        let id = i.employee.clone();
        let emp = self
            .store
            .read(move |r| r.employee(&id))
            .await
            .map_err(|e: store::Error| Error::refused(format!("employee {:?}: {e}", i.employee)))?;
        let mut ids = i.ids.clone();
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Err(Error::refused(
                "name the memories to forget, by id, as read_memory shows them",
            ));
        }
        let holder = emp.id.clone();
        let mems = self.store.read(move |r| r.memories_of(&holder)).await?;
        let named = named_memories(&mems, &ids)?;
        let card_id = by.who.clone();
        let card = self.store.read(move |r| r.card(&card_id)).await?;
        let n = named.len();
        let memories = plural(n, "memory", "memories");
        let item = InboxItem {
            id: new_id("i_"),
            home: card.home.clone(),
            card_id: card.id.clone(),
            kind: InboxKind::Change,
            title: format!("Forget {n} {memories} of {}", emp.label),
            detail: format!(
                "{drafter} asks to forget {n} {memories} of {} ({}). Why, in its words: {}\n\n{}\n\nApply forgets exactly these, and the employee is no longer reminded of them. Not now keeps them.",
                emp.label,
                code_span(&emp.id),
                code_span(&gist(&i.why, 400)),
                forget_list(&named)
            ),
            options: vec![
                InboxOption {
                    id: CHANGE_APPLY.into(),
                    name: "Apply".into(),
                    ..InboxOption::default()
                },
                InboxOption {
                    id: CHANGE_NOT_NOW.into(),
                    name: "Not now".into(),
                    ..InboxOption::default()
                },
            ],
            status: InboxStatus::Open,
            created_at: self.now(),
            change: Some(PendingChange {
                kind: ChangeKind::Forget,
                name: emp.id.clone(),
                record: doc_of(Some(&ForgetRecord {
                    employee: emp.id.clone(),
                    ids,
                    why: i.why.clone(),
                }))?,
                fields: forget_fields(&named)?,
            }),
            ..InboxItem::default()
        };
        self.raise_change(item).await?;
        Ok(format!(
            "The forget of {n} {memories} waits in the owner's inbox, where the Assistant decides it for him. Nothing is forgotten until it is applied; you are told the answer."
        ))
    }

    /// Forgets, in tx, the memories the change item asks to forget, if they are still the ones the owner was shown: the one
    /// way Remit removes a memory, on his answer. The employee is stored again, so the pages showing it read its memory
    /// anew. `Applied::Moved`, and nothing forgotten, when one of them is gone or changed since.
    pub fn apply_forget(&self, tx: &Tx<'_>, pending: &PendingChange, w: &Authorship) -> Result<Applied> {
        require_owner(w)?;
        let rec: ForgetRecord = serde_json::from_str(&pending.record)?;
        let emp = tx
            .employee(&rec.employee)
            .map_err(|e| Error::refused(format!("employee {:?}: {e}", rec.employee)))?;
        let mems = tx.memories_of(&emp.id)?;
        // a memory gone or changed since the draft is a record that moved, which the owner is told
        let shown = match named_memories(&mems, &rec.ids) {
            Ok(named) => forget_fields(&named)? == pending.fields,
            Err(_) => false,
        };
        if !shown {
            return Ok(Applied::Moved);
        }
        tx.delete_memories(&rec.ids, self.now())?;
        let saved = touch_employee(tx, &rec.employee)?;
        Ok(Applied::Written(Some(Box::new(move |o: &chains::Core| {
            o.notify.employee_updated(&saved);
        }))))
    }
}
