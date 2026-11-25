//! What one session has been pushed and not yet sent, held at most once per record (ADR-0095): a newer push of a record
//! replaces the one waiting and goes to the end, so a slow session is sent fewer, larger pushes and what it is sent is
//! each record's last state, in the order those last changes came. The text an event streams merges into what waits for
//! it. A session's queue is so bounded by the records that changed, never by how long it was slow. Pure: `Broadcast`
//! holds one per session and the session's loop takes from it.

use std::collections::{BTreeMap, HashMap, HashSet};

use wire::app::{EventDelta, HomeReset, Push};

/// A record a push carries: what kind of record and which one. The pushes of one record (its update, its deletion, the
/// text it streams) share it.
type Record = (&'static str, String);

#[derive(Default)]
pub(crate) struct Pending {
    next: u64,
    /// The records waiting, in the order of their last push.
    order: BTreeMap<u64, Record>,
    by_record: HashMap<Record, (u64, Push)>,
}

impl Pending {
    /// How many records wait.
    pub(crate) fn len(&self) -> usize {
        self.by_record.len()
    }

    /// Holds push to be sent, over what waits for its record.
    pub(crate) fn put(&mut self, push: Push) {
        let record = record_of(&push);
        let push = match self.by_record.remove(&record) {
            Some((at, held)) => {
                self.order.remove(&at);
                merged(held, push)
            }
            None => push,
        };
        let at = self.next;
        self.next = self.next.wrapping_add(1);
        self.order.insert(at, record.clone());
        self.by_record.insert(record, (at, push));
    }

    /// Lets go of what streams in the home with the most records waiting, and holds `home_reset` for it in their place:
    /// that home is read again by what shows it, and every other home and the board go on (ADR-0095). A `home_shown`
    /// waiting stays where it is: the core reads the home from it, then again on the reset. Which home, if any streamed.
    pub(crate) fn reset_busiest_home(&mut self) -> Option<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (_, push) in self.by_record.values() {
            if let Some(home) = reset_home(push) {
                let n = counts.entry(home).or_default();
                *n = n.saturating_add(1);
            }
        }
        let home = counts.into_iter().max_by_key(|(_, n)| *n)?.0.to_owned();
        let gone: Vec<(u64, Record)> = self
            .by_record
            .iter()
            .filter(|(_, (_, push))| reset_home(push) == Some(home.as_str()))
            .map(|(record, (at, _))| (*at, record.clone()))
            .collect();
        for (at, record) in gone {
            self.order.remove(&at);
            self.by_record.remove(&record);
        }
        self.put(Push::home_reset {
            reset: HomeReset { home: home.clone() },
        });
        Some(home)
    }

    /// Lets go of what streams in a home not in `shown`: a session no longer showing a home reads it whole when it shows
    /// it again, and a push held from before would land on that read and undo what came after (a record deleted
    /// meanwhile put back).
    pub(crate) fn keep_homes(&mut self, shown: &HashSet<String>) {
        let gone: Vec<(u64, Record)> = self
            .by_record
            .iter()
            .filter(|(_, (_, push))| push.streamed_home().is_some_and(|home| !shown.contains(home)))
            .map(|(record, (at, _))| (*at, record.clone()))
            .collect();
        for (at, record) in gone {
            self.order.remove(&at);
            self.by_record.remove(&record);
        }
    }

    /// The push that waited longest.
    pub(crate) fn take(&mut self) -> Option<Push> {
        let (_, record) = self.order.pop_first()?;
        self.by_record.remove(&record).map(|(_, push)| push)
    }
}

/// What is sent in place of held, which waited, and newer, which came after it, for one record: newer, unless newer is
/// text streamed after what held carries, which then carries it too. Text that does not follow (a chunk missed, one held
/// already) is left, as the client leaves it: the event's next update brings the whole text.
fn merged(held: Push, newer: Push) -> Push {
    match (held, newer) {
        (Push::event_added { mut event }, Push::event_delta { delta }) if follows(&event.event.text, 0, &delta) => {
            event.event.text.push_str(&delta.text);
            Push::event_added { event }
        }
        (Push::event_updated { mut event }, Push::event_delta { delta }) if follows(&event.event.text, 0, &delta) => {
            event.event.text.push_str(&delta.text);
            Push::event_updated { event }
        }
        (Push::event_delta { delta: mut first }, Push::event_delta { delta })
            if follows(&first.text, first.offset, &delta) =>
        {
            first.text.push_str(&delta.text);
            Push::event_delta { delta: first }
        }
        (
            held @ (Push::event_added { .. } | Push::event_updated { .. } | Push::event_deleted { .. }),
            Push::event_delta { .. },
        )
        | (held @ Push::event_delta { .. }, Push::event_delta { .. }) => held,
        (_, newer) => newer,
    }
}

/// The home whose reset lets go of push: where it streams, unless it is that home's `home_shown`, which the core reads
/// the home from.
fn reset_home(push: &Push) -> Option<&str> {
    match push {
        Push::home_shown { .. } => None,
        _ => push.streamed_home(),
    }
}

/// Whether delta starts where text, which starts at offset, ends; offsets count UTF-16 code units, as the page does.
fn follows(text: &str, offset: usize, delta: &EventDelta) -> bool {
    offset.checked_add(text.encode_utf16().count()) == Some(delta.offset)
}

/// The record a push carries. Every push names one, so a kind of push added later says how it conflates.
fn record_of(push: &Push) -> Record {
    let (kind, id): (&'static str, String) = match push {
        Push::chain_updated { chain } => ("chain", chain.chain.id.clone()),
        Push::chain_deleted { gone } => ("chain", gone.id.clone()),
        Push::card_updated { card } => ("card", card.id.clone()),
        Push::card_shown { shown } => ("card_shown", shown.id.clone()),
        Push::edge_added { edge } => ("edge", edge.id.clone()),
        Push::decision_added { decision } => ("decision", decision.decision.id.clone()),
        Push::landing_updated { landing } => ("landing", landing.landing.id.clone()),
        Push::repo_updated { view } => ("repo", view.repo.clone()),
        Push::workflow_updated { workflow } => ("workflow", workflow.id.clone()),
        Push::insight_added { insight } => ("insight", insight.id.clone()),
        Push::assistant_feed_updated { .. } => ("assistant_feed", String::new()),
        Push::draft_updated { draft } => ("draft", draft.id.clone()),
        Push::draft_deleted { gone } => ("draft", gone.id.clone()),
        Push::event_added { event } | Push::event_updated { event } => ("event", event.event.id.to_string()),
        Push::event_deleted { gone } => ("event", gone.id.to_string()),
        Push::event_delta { delta } => ("event", delta.id.to_string()),
        Push::health_updated { .. } => ("health", String::new()),
        Push::workload_updated { .. } => ("workload", String::new()),
        Push::signal_board_updated { .. } => ("signal_board", String::new()),
        Push::boards_updated { .. } => ("boards", String::new()),
        Push::event_read { event } => ("event_read", event.id.clone()),
        Push::event_signal_updated { event_signal } => ("event_signal", event_signal.id.clone()),
        Push::event_signal_deleted { gone } => ("event_signal", gone.id.clone()),
        Push::sensor_updated { sensor } => ("sensor", sensor.id.clone()),
        Push::sensor_deleted { gone } => ("sensor", gone.id.clone()),
        Push::client_key_updated { key } => ("client_key", key.id.clone()),
        Push::client_key_deleted { gone } => ("client_key", gone.id.clone()),
        Push::extension_updated { extension } => ("extension", extension.id.clone()),
        Push::extension_deleted { gone } => ("extension", gone.id.clone()),
        Push::feedback_updated { feedback } => ("feedback", feedback.id.clone()),
        Push::objective_updated { view } => ("objective", view.objective.id.clone()),
        Push::project_updated { project } => ("project", project.id.clone()),
        Push::employee_updated { employee } => ("employee", employee.employee.id.clone()),
        Push::mentor_updated { mentor } => ("mentor", mentor.id.clone()),
        Push::help_updated { help } => ("help", help.id.clone()),
        Push::inbox_updated { item } => ("inbox", item.id.clone()),
        Push::playbook_updated { playbook } => ("playbook", playbook.id.clone()),
        Push::playbook_deleted { gone } => ("playbook", gone.id.clone()),
        Push::agent_updated { agent } => ("agent", agent.id.clone()),
        Push::agent_deleted { gone } => ("agent", gone.id.clone()),
        Push::skill_updated { skill } => ("skill", skill.name.clone()),
        Push::skill_deleted { gone } => ("skill", gone.name.clone()),
        Push::source_updated { source } => ("source", source.id.clone()),
        Push::board_updated { .. } => ("board", String::new()),
        Push::chain_standing_updated { standing } => ("chain_standing", standing.chain_id.clone()),
        Push::agent_standing_updated { agent } => ("agent_standing", agent.card_id.clone()),
        Push::at_work_updated { .. } => ("at_work", String::new()),
        Push::mentor_standing_updated { mentor } => ("mentor_standing", mentor.mentor.clone()),
        Push::verification_updated { verification } => ("verification", verification.chain_id.clone()),
        Push::retrospective_added { retrospective } => ("retrospective", retrospective.retrospective.id.clone()),
        Push::home_reset { reset } => ("home_reset", reset.home.clone()),
        Push::home_shown { mark } => ("home_shown", mark.home.clone()),
    };
    (kind, id)
}
