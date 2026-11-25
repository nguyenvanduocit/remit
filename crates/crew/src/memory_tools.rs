//! The tools over an agent's memory: the Assistant reads an employee's and drafts a forget, which it applies for the owner;
//! every coaching agent reads one; each session that remembers (an employee's, the Assistant's) searches what it may
//! (search_memory) and rates what it was given (rate_memory). Each recall, a search's or what a session was given, is
//! counted on the memories and recorded as the fact memory.recalled, so the coach can say which memories helped a chain
//! and which misled it (ADR-0082). Nothing here writes a memory: only the librarian does, from what the gate let through
//! (teach_memory.rs, librarian.rs).

use std::collections::HashSet;
use std::sync::Arc;

use model::{Actor, Card, Memory, by_agent};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Crew;
use crate::memory::{MEMORY_EVIDENCE, remembers};
use crate::memory_change::DraftForgetInput;
use crate::recall::Purpose;
use chains::chain_core::page_size;
use chains::fact::{FactValue, fact_write, keys};
use chains::tools::{Effect, SelfCard, Tools, as_json};

/// What an agent reads of an employee's memory.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct MemoryRead {
    note: String,
    memories: Vec<Memory>,
}

impl MemoryRead {
    fn of(memories: Vec<Memory>) -> Self {
        Self {
            note: MEMORY_EVIDENCE.to_owned(),
            memories,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadMemoryInput {
    /// the employee's id, as the Crew tab and list_employees give it
    employee: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchMemoryInput {
    /// what you look for, in a few words: the task, a file, a tool, a mistake to avoid
    query: String,
    /// how many memories at most; 8 when not given, at most 20
    #[serde(default)]
    limit: i64,
}

/// Memories one search_memory gives when it names no limit, and at most.
const SEARCH_SHOWN: usize = 8;
const SEARCH_MAX: usize = 20;

impl Crew {
    /// Gives the Assistant the tools to read an employee's memory and to draft a forget of some of it as a change item:
    /// it drafts, and it or the owner applies.
    pub fn add_memory_assistant_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        self.add_read_memory_tool(t);
        let by = by.clone();
        t.serving(self).add(
            "draft_forget", Effect::Writes,
            "Draft the forgetting of some of an employee's memories as a change item in the owner's inbox, which the Assistant decides for him: name them by id, as read_memory shows them, and say why. Nothing is forgotten until it is applied; Apply forgets exactly the memories shown, and refuses one changed since.",
            move |o, i: DraftForgetInput| {
                let by = by.clone();
                async move { o.draft_forget(&by, i).await }
            },
        );
    }

    /// Gives the tool that reads what an employee remembers: the Assistant's, and every coaching agent's.
    pub(crate) fn add_read_memory_tool(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "read_memory", Effect::Reads,
            format!(
                "What an employee remembers: each memory with its id, holder (whom it was learned for: the employee, or the sibling whose lesson its lineage shares), kind (lesson, pitfall, preference, principle, decision, intent, fact), scope (own: the holder's alone; project: every agent in its project; lineage: the employees of its template; everyone), trust (auto: the librarian wrote it; confirmed: a person did), status (active; superseded by a later memory; retracted after an overturn), title, body, the evidence it rests on, the teaching it was learned from (source: card:<id> accepted, card:<id> sent back) and who taught it (teacher; none for an accepted card's own session), what it is about (file:, crate:, tool:, topic:), its links to other memories, the corrections that taught it again (retaught), how many turns recalled it and when it last changed. Use it to find the ids a forget or a promotion names. {MEMORY_EVIDENCE}"
            ),
            |o, i: ReadMemoryInput| async move {
                as_json(&MemoryRead::of(o.employee_memory(&i.employee).await?))
            },
        );
    }

    /// Gives card's session, working as me, its tools over what it may recall, when it remembers (`remembers`: an
    /// employee's card, or the Assistant's).
    pub fn add_memory_card_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, me: &SelfCard) {
        if remembers(card) {
            self.add_recall_tools(t, me);
        }
    }

    /// Gives the session of employee, working as card me (a seat's, a subagent's), its tools over what it may recall;
    /// none when it is no employee's.
    pub fn add_memory_session_tools(self: &Arc<Self>, t: &mut Tools, employee: &str, me: &SelfCard) {
        if !employee.is_empty() {
            self.add_recall_tools(t, me);
        }
    }

    /// Gives a session working as me its tools over what it may recall (its own memories, its project's, its lineage's
    /// and everyone's): to search it and to rate what it was given. The session names what it looks for, never whose
    /// memory: it cannot read another employee's own.
    fn add_recall_tools(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        self.add_rate_memory_tool(t, me);
        let me = me.clone();
        t.serving(self).add(
            "search_memory", Effect::Reads,
            format!(
                "Search what you remember from earlier work: the lessons, pitfalls, preferences, principles, procedures, decisions and facts you, your project, your lineage and the owner were taught, matched by their words, their meaning (another language finds them too) and what they are about, best first. Each turn already brings the few memories its message is about, judged relevant; search before work like something done before, before a choice a person may have a taste in, and when you touch a file or tool your memories may know. {MEMORY_EVIDENCE}"
            ),
            move |o, i: SearchMemoryInput| {
                let me = me.clone();
                async move {
                    let limit = page_size(i.limit, SEARCH_SHOWN, SEARCH_MAX);
                    let card = me.read(&o).await?;
                    let Some(reader) = o.reader_of(&card).await? else {
                        return as_json(&MemoryRead::of(Vec::new()));
                    };
                    let holder = reader.holder.clone();
                    let found = o.search_memory(reader, &i.query, Purpose::Search, limit, &HashSet::new()).await?;
                    o.note(&card, &holder, &found, true).await;
                    as_json(&MemoryRead::of(found))
                }
            },
        );
    }

    /// Counts the memories ids as given to card c's session unasked (`note_recall`), once what gave them is stored. A
    /// failure is logged: what was given stands.
    pub async fn count_given(&self, c: &Card, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let reader = match self.reader_of(c).await {
            Ok(Some(reader)) => reader,
            Ok(None) => return,
            Err(err) => {
                tracing::error!(card = %c.id, %err, "memory: what the turn was given is not counted");
                return;
            }
        };
        let named = ids.to_vec();
        let found = match self.store.read(move |r| r.memories_named(&named)).await {
            Ok(found) => found,
            Err(err) => {
                tracing::error!(card = %c.id, %err, "memory: what the turn was given is not counted");
                return;
            }
        };
        let mut found = found;
        found.sort_by_key(|m| ids.iter().position(|id| *id == m.id));
        self.note_recall(c, &reader.holder, &found).await;
    }

    /// Records that card, holder's session, was given found unasked (a turn's note, a tool's): each counted as recalled
    /// once more, and the fact memory.recalled. Called once what was given is stored, so what never reached the session
    /// is never counted.
    pub(crate) async fn note_recall(&self, card: &Card, holder: &str, found: &[Memory]) {
        self.note(card, holder, found, false).await;
    }

    /// Records that card, holder's session, recalled found, searched for or given: each counted as recalled once more,
    /// and the fact memory.recalled with the ids, which `read_chain` gives the coach per card, in one write. The tool's
    /// output is cut at its event's size, so the ids are recorded here, where they are whole. A recall that found nothing
    /// recalled nothing; a failure to record is logged, and the recall still answers.
    async fn note(&self, card: &Card, holder: &str, found: &[Memory], searched: bool) {
        if found.is_empty() {
            return;
        }
        let ids: Vec<String> = found.iter().map(|m| m.id.clone()).collect();
        let v = FactValue {
            chain: card.home.clone(),
            card: card.id.clone(),
            employee: holder.to_owned(),
            memories: ids.clone(),
            searched,
            ..FactValue::default()
        };
        let w = match fact_write(keys::memory_recalled(), &v, by_agent(&card.id), &card.home) {
            Ok(w) => w,
            Err(err) => {
                tracing::error!(card = %card.id, %err, "memory: the recall is not recorded");
                return;
            }
        };
        let (door, now) = (self.door(), self.now());
        let recorded = self
            .store
            .update(move |tx| -> chains::Result<()> {
                tx.count_recall(&ids, now)?;
                door.ingest(tx, w)?;
                Ok(())
            })
            .await;
        if let Err(err) = recorded {
            tracing::error!(card = %card.id, %err, "memory: the recall is not recorded");
            return;
        }
        self.kick();
    }
}
