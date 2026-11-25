//! The replica of what the server keeps for the whole app (`catalog.ts`): each snapshot replaces it, then each push
//! changes one record unless the copy held is newer. It holds what the server answered and pushed; nothing is decided
//! here but which copy is the newer and where a record goes in its store's order.

use link::Failure;
use model::{
    Agent, Chain, ChainStatus, ClientKey, Decision, DecisionKind, EventSignal, Extension, ExtensionOrigin, Feedback,
    InboxItem, InboxStatus, Mentor, ObjectiveView, Playbook, Project, Sensor, Time,
};
use serde_json::Value;
use wire::app::{
    AtWork, Board, Boards, ChainRead, ChainStanding, DecisionRead, EmployeeRead, EventRecord, Health, ListedSkill,
    MentorStanding, Push, SignalBoard, Snapshot, ToolSource, Workload,
};

use crate::records::read;

use crate::index::{Index, newer};
use crate::records::{Out, take, take_list};
use crate::shell::{Key, Model, Notice};

/// How many events the server lists, and so how many are kept (`Snapshot.events`).
pub(crate) const EVENTS_KEPT: usize = 200;
/// How much feedback the server lists, and so how much is kept (`Snapshot.feedback`).
pub(crate) const FEEDBACK_KEPT: usize = 200;

/// The one record of a store that holds one.
const ONE: &str = "";

#[derive(Debug, Clone, Copy)]
pub(crate) struct ChainFacts {
    rev: i64,
    done: bool,
}

impl ChainFacts {
    fn of(c: &Chain) -> Self {
        Self {
            rev: c.rev,
            done: *c.status() == ChainStatus::Done,
        }
    }
}

/// A project, kept in the order the server lists them: by name, in any case, then by id.
#[derive(Debug, Clone)]
pub(crate) struct Named {
    rev: i64,
    rank: (String, Key),
}

impl Named {
    /// SQLite's `NOCASE`, which orders `Reader::projects`, folds ASCII alone.
    fn of(p: &Project) -> Self {
        Self {
            rev: p.rev,
            rank: (p.name.to_ascii_lowercase(), p.id.clone()),
        }
    }
}

/// A record kept newest first by a time.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timed {
    rev: i64,
    at: Option<i128>,
}

impl Timed {
    fn of(rev: i64, at: Time) -> Self {
        Self {
            rev,
            at: Some(at.0.unix_timestamp_nanos()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) loaded: bool,
    who: Index<()>,
    chains: Index<ChainFacts>,
    inbox: Index<i64>,
    standings: Index<i64>,
    board: Index<i64>,
    playbooks: Index<i64>,
    agents: Index<i64>,
    skills: Index<i64>,
    sources: Index<()>,
    capabilities: Index<()>,
    health: Index<i64>,
    workload: Index<i64>,
    at_work: Index<i64>,
    signals: Index<i64>,
    boards: Index<i64>,
    insights: Index<()>,
    events: Index<Timed>,
    event_signals: Index<Timed>,
    sensors: Index<Timed>,
    client_keys: Index<Timed>,
    extensions: Index<bool>,
    feedback: Index<Timed>,
    objectives: Index<Timed>,
    projects: Index<Named>,
    employees: Index<i64>,
    mentors: Index<i64>,
    mentor_standings: Index<i64>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            loaded: false,
            who: Index::new(Model::Who),
            chains: Index::new(Model::Chains),
            inbox: Index::new(Model::Inbox),
            standings: Index::new(Model::Standings),
            board: Index::new(Model::Board),
            playbooks: Index::new(Model::Playbooks),
            agents: Index::new(Model::Agents),
            skills: Index::new(Model::Skills),
            sources: Index::new(Model::Sources),
            capabilities: Index::new(Model::Capabilities),
            health: Index::new(Model::Health),
            workload: Index::new(Model::Workload),
            at_work: Index::new(Model::AtWork),
            signals: Index::new(Model::Signals),
            boards: Index::new(Model::Boards),
            insights: Index::new(Model::Insights),
            events: Index::new(Model::Events),
            event_signals: Index::new(Model::EventSignals),
            sensors: Index::new(Model::Sensors),
            client_keys: Index::new(Model::ClientKeys),
            extensions: Index::new(Model::Extensions),
            feedback: Index::new(Model::Feedback),
            objectives: Index::new(Model::Objectives),
            projects: Index::new(Model::Projects),
            employees: Index::new(Model::Employees),
            mentors: Index::new(Model::Mentors),
            mentor_standings: Index::new(Model::MentorStandings),
        }
    }
}

/// Pairs each typed record of a list with its JSON, keyed and with its facts.
fn rows<T, M>(
    typed: Vec<T>,
    json: Vec<Value>,
    key: impl Fn(&T) -> Key,
    facts: impl Fn(&T) -> M,
) -> Vec<(Key, M, Value)> {
    typed.iter().zip(json).map(|(t, v)| (key(t), facts(t), v)).collect()
}

fn one<M>(facts: M, json: Value) -> Vec<(Key, M, Value)> {
    vec![(ONE.to_owned(), facts, json)]
}

/// Puts a record unless the copy held is newer; a new one goes before `before` (none: last).
fn put_newer(ix: &mut Index<i64>, id: Key, rev: i64, record: Value, before: Option<Key>, out: &mut Out) {
    if newer(ix.get(&id).copied(), rev) {
        ix.put(id, rev, record, before, &mut out.patches);
    }
}

/// A record kept in the order the server lists it, newest first by its time: a new one goes where its time puts it,
/// and one held keeps its place when it changes, unless the copy held is newer.
fn put_newest_first(ix: &mut Index<Timed>, id: Key, facts: Timed, record: Value, out: &mut Out) {
    if let Some(held) = ix.get(&id) {
        if newer(Some(held.rev), facts.rev) {
            ix.put(id, facts, record, None, &mut out.patches);
        }
        return;
    }
    let before = before_time(ix, facts.at);
    ix.put(id, facts, record, before, &mut out.patches);
}

/// The first record older than `at`; a time that does not parse goes last, as `Date.parse`'s NaN did.
fn before_time(ix: &Index<Timed>, at: Option<i128>) -> Option<Key> {
    let at = at?;
    ix.keys()
        .find(|(_, m)| m.at.is_some_and(|held| held < at))
        .map(|(k, _)| k.clone())
}

/// A list the server keeps ordered by key (playbooks and agents by id, skills by name): a record saved goes where its
/// key puts it, replacing the one that has it, unless the copy held is newer.
fn put_ordered(ix: &mut Index<i64>, id: Key, rev: i64, record: Value, out: &mut Out) {
    let before = ix.before_by(|k, _| k.clone(), &id);
    put_newer(ix, id, rev, record, before, out);
}

impl Catalog {
    pub(crate) fn apply_snapshot(&mut self, snap: Snapshot, mut json: Value, out: &mut Out) {
        let p = &mut out.patches;
        self.who.reset(one((), take(&mut json, "who")), p);
        self.chains.reset(
            rows(
                snap.chains,
                take_list(&mut json, "chains"),
                |c| c.chain.id.clone(),
                |c| ChainFacts::of(&c.chain),
            ),
            p,
        );
        self.inbox.reset(
            rows(snap.inbox, take_list(&mut json, "inbox"), |i| i.id.clone(), |i| i.rev),
            p,
        );
        self.board.reset(one(snap.board.rev, take(&mut json, "board")), p);
        self.standings.reset(
            rows(
                snap.standings,
                take_list(&mut json, "standings"),
                |s| s.chain_id.clone(),
                |s| s.rev,
            ),
            p,
        );
        self.playbooks.reset(
            rows(
                snap.playbooks,
                take_list(&mut json, "playbooks"),
                |x| x.id.clone(),
                |x| x.rev,
            ),
            p,
        );
        self.agents.reset(
            rows(snap.agents, take_list(&mut json, "agents"), |x| x.id.clone(), |x| x.rev),
            p,
        );
        self.skills.reset(
            rows(
                snap.skills,
                take_list(&mut json, "skills"),
                |x| x.name.clone(),
                |x| x.rev,
            ),
            p,
        );
        self.sources.reset(
            rows(snap.sources, take_list(&mut json, "sources"), |x| x.id.clone(), |_| ()),
            p,
        );
        self.capabilities.reset(
            rows(
                snap.capabilities,
                take_list(&mut json, "capabilities"),
                |x| x.id.clone(),
                |_| (),
            ),
            p,
        );
        self.health.reset(one(snap.health.rev, take(&mut json, "health")), p);
        self.workload
            .reset(one(snap.workload.rev, take(&mut json, "workload")), p);
        self.signals.reset(one(snap.signals.rev, take(&mut json, "signals")), p);
        self.boards.reset(one(snap.boards.rev, take(&mut json, "boards")), p);
        self.insights.reset(
            rows(
                snap.insights,
                take_list(&mut json, "insights"),
                |x| x.id.clone(),
                |_| (),
            ),
            p,
        );
        self.events.reset(
            rows(
                snap.events,
                take_list(&mut json, "events"),
                |x| x.id.clone(),
                |x| Timed::of(x.rev, x.at),
            ),
            p,
        );
        self.event_signals.reset(
            rows(
                snap.event_signals,
                take_list(&mut json, "eventSignals"),
                |x| x.id.clone(),
                |x| Timed::of(x.rev, x.created_at),
            ),
            p,
        );
        self.sensors.reset(
            rows(
                snap.sensors,
                take_list(&mut json, "sensors"),
                |x| x.id.clone(),
                |x| Timed::of(x.rev, x.created_at),
            ),
            p,
        );
        self.client_keys.reset(
            rows(
                snap.client_keys,
                take_list(&mut json, "clientKeys"),
                |x| x.id.clone(),
                |x| Timed::of(x.rev, x.created_at),
            ),
            p,
        );
        self.extensions.reset(
            rows(
                snap.extensions,
                take_list(&mut json, "extensions"),
                |x| x.id.clone(),
                |x| x.origin == ExtensionOrigin::Bundled,
            ),
            p,
        );
        self.feedback.reset(
            rows(
                snap.feedback,
                take_list(&mut json, "feedback"),
                |x| x.id.clone(),
                |x| Timed::of(x.rev, x.created_at),
            ),
            p,
        );
        self.objectives.reset(
            rows(
                snap.objectives,
                take_list(&mut json, "objectives"),
                |x| x.objective.id.clone(),
                |x| Timed::of(x.objective.rev, x.objective.created_at),
            ),
            p,
        );
        self.projects.reset(
            rows(
                snap.projects,
                take_list(&mut json, "projects"),
                |x| x.id.clone(),
                Named::of,
            ),
            p,
        );
        self.employees.reset(
            rows(
                snap.employees,
                take_list(&mut json, "employees"),
                |x| x.employee.id.clone(),
                |x| x.employee.rev,
            ),
            p,
        );
        self.mentors.reset(
            rows(
                snap.mentors,
                take_list(&mut json, "mentors"),
                |x| x.id.clone(),
                |x| x.rev,
            ),
            p,
        );
        self.mentor_standings.reset(
            rows(
                snap.mentor_standings,
                take_list(&mut json, "mentorStandings"),
                |x| x.mentor.clone(),
                |x| x.rev,
            ),
            p,
        );
        self.loaded = true;
    }

    /// The server lists the chain: it exists. One it does not list was deleted, archived, or never was.
    pub(crate) fn listed(&self, chain: &str) -> bool {
        self.chains.has(chain)
    }

    /// Applies a push that is the catalog's; false for one that is not.
    pub(crate) fn on_push(&mut self, push: &Push, record: &Value, out: &mut Out) -> bool {
        let r = || record.clone();
        let p = &mut out.patches;
        match push {
            Push::chain_updated { chain } => self.put_chain(chain, r(), out),
            Push::chain_deleted { gone } => {
                self.chains.remove(&gone.id, p);
                self.standings.remove(&gone.id, p);
            }
            Push::board_updated { board } => self.set_board(board, r(), out),
            Push::chain_standing_updated { standing } => self.put_standing(standing, r(), out),
            Push::insight_added { insight } => self.put_insight(insight, r(), out),
            Push::health_updated { health } => self.set_health(health, r(), out),
            Push::workload_updated { workload } => self.set_workload(workload, r(), out),
            Push::at_work_updated { at_work } => self.set_at_work(at_work, r(), out),
            Push::signal_board_updated { board } => self.set_signals(board, r(), out),
            Push::boards_updated { boards } => self.set_boards(boards, r(), out),
            Push::event_read { event } => self.put_event_record(event, r(), out),
            Push::event_signal_updated { event_signal } => self.put_event_signal(event_signal, r(), out),
            Push::event_signal_deleted { gone } => {
                self.event_signals.remove(&gone.id, p);
            }
            Push::sensor_updated { sensor } => self.put_sensor(sensor, r(), out),
            Push::sensor_deleted { gone } => {
                self.sensors.remove(&gone.id, p);
            }
            Push::client_key_updated { key } => self.put_client_key(key, r(), out),
            Push::client_key_deleted { gone } => {
                self.client_keys.remove(&gone.id, p);
            }
            Push::extension_updated { extension } => self.put_extension(extension, r(), out),
            Push::extension_deleted { gone } => {
                self.extensions.remove(&gone.id, p);
            }
            Push::feedback_updated { feedback } => self.put_feedback(feedback, r(), out),
            Push::objective_updated { view } => self.put_objective(view, r(), out),
            Push::project_updated { project } => self.put_project(project, r(), out),
            Push::employee_updated { employee } => self.put_employee(employee, r(), out),
            Push::mentor_updated { mentor } => self.put_mentor(mentor, r(), out),
            Push::mentor_standing_updated { mentor } => self.put_mentor_standing(mentor, r(), out),
            Push::playbook_updated { playbook } => self.put_playbook(playbook, r(), out),
            Push::playbook_deleted { gone } => {
                self.playbooks.remove(&gone.id, p);
            }
            Push::agent_updated { agent } => self.put_agent(agent, r(), out),
            Push::agent_deleted { gone } => {
                self.agents.remove(&gone.id, p);
            }
            Push::skill_updated { skill } => self.put_skill(skill, r(), out),
            Push::skill_deleted { gone } => {
                self.skills.remove(&gone.name, p);
            }
            Push::source_updated { source } => self.put_source(source, r(), out),
            Push::inbox_updated { item } => self.put_inbox_item(item, r(), out),
            Push::decision_added { decision } => note_decision(decision, out),
            _ => return false,
        }
        true
    }

    /// A chain done is worth the owner's notice; a new chain goes first.
    fn put_chain(&mut self, read: &ChainRead, record: Value, out: &mut Out) {
        let c = &read.chain;
        let facts = ChainFacts::of(c);
        let held = self.chains.get(&c.id).copied();
        if let Some(prev) = held
            && prev.rev < c.rev
            && !prev.done
            && facts.done
        {
            out.notice(Notice::ChainDone { title: c.title.clone() });
        }
        if !newer(held.map(|h| h.rev), c.rev) {
            return;
        }
        let before = self.chains.first().cloned();
        self.chains.put(c.id.clone(), facts, record, before, &mut out.patches);
    }

    fn set_board(&mut self, b: &Board, record: Value, out: &mut Out) {
        put_newer(&mut self.board, ONE.to_owned(), b.rev, record, None, out);
    }

    pub(crate) fn put_standing(&mut self, st: &ChainStanding, record: Value, out: &mut Out) {
        put_newer(&mut self.standings, st.chain_id.clone(), st.rev, record, None, out);
    }

    /// An inbox item the server still has open is kept; one it closed goes. One newly open is worth the owner's notice.
    fn put_inbox_item(&mut self, it: &InboxItem, record: Value, out: &mut Out) {
        let held = self.inbox.get(&it.id).copied();
        if it.status == InboxStatus::Open && held.is_none() {
            out.notice(Notice::InboxOpened {
                kind: it.kind,
                title: it.title.clone(),
            });
        }
        if !newer(held, it.rev) {
            return;
        }
        if it.status == InboxStatus::Open {
            self.inbox.put(it.id.clone(), it.rev, record, None, &mut out.patches);
        } else {
            self.inbox.remove(&it.id, &mut out.patches);
        }
    }

    fn put_playbook(&mut self, p: &Playbook, record: Value, out: &mut Out) {
        put_ordered(&mut self.playbooks, p.id.clone(), p.rev, record, out);
    }

    fn put_agent(&mut self, a: &Agent, record: Value, out: &mut Out) {
        put_ordered(&mut self.agents, a.id.clone(), a.rev, record, out);
    }

    fn put_skill(&mut self, s: &ListedSkill, record: Value, out: &mut Out) {
        put_ordered(&mut self.skills, s.name.clone(), s.rev, record, out);
    }

    /// A source is pushed whole, as it stands now, so it takes the place of the one held.
    fn put_source(&mut self, s: &ToolSource, record: Value, out: &mut Out) {
        self.sources.put(s.id.clone(), (), record, None, &mut out.patches);
    }

    fn set_health(&mut self, h: &Health, record: Value, out: &mut Out) {
        put_newer(&mut self.health, ONE.to_owned(), h.rev, record, None, out);
    }

    fn set_boards(&mut self, b: &Boards, record: Value, out: &mut Out) {
        put_newer(&mut self.boards, ONE.to_owned(), b.rev, record, None, out);
    }

    fn set_workload(&mut self, w: &Workload, record: Value, out: &mut Out) {
        put_newer(&mut self.workload, ONE.to_owned(), w.rev, record, None, out);
    }

    fn set_at_work(&mut self, a: &AtWork, record: Value, out: &mut Out) {
        put_newer(&mut self.at_work, ONE.to_owned(), a.rev, record, None, out);
    }

    /// The seats at work as the `at_work` read a (re)connect asked answered: kept unless a push brought a newer copy.
    pub(crate) fn at_work_answered(&mut self, result: Result<Value, Failure>, out: &mut Out) -> Result<(), Failure> {
        let json = result?;
        let at_work: AtWork = read(&json)?;
        self.set_at_work(&at_work, json, out);
        Ok(())
    }

    /// A page with no seats to read (a mentor's) holds none.
    pub(crate) fn forget_at_work(&mut self, out: &mut Out) {
        self.at_work.reset(Vec::new(), &mut out.patches);
    }

    fn set_signals(&mut self, b: &SignalBoard, record: Value, out: &mut Out) {
        put_newer(&mut self.signals, ONE.to_owned(), b.rev, record, None, out);
    }

    /// A new insight comes first; one held already (an insight never changes) is not added twice.
    fn put_insight(&mut self, d: &Decision, record: Value, out: &mut Out) {
        if !self.insights.has(&d.id) {
            let before = self.insights.first().cloned();
            self.insights.put(d.id.clone(), (), record, before, &mut out.patches);
        }
    }

    /// An event's `rev` is its reading's, not a count of its changes, so the push that arrives last is the newer: a
    /// firing added to an event brings it again. One that falls beyond the newest 200 is let go.
    fn put_event_record(&mut self, e: &EventRecord, record: Value, out: &mut Out) {
        let facts = Timed::of(e.rev, e.at);
        let before = if self.events.has(&e.id) {
            None
        } else {
            before_time(&self.events, facts.at)
        };
        self.events.put(e.id.clone(), facts, record, before, &mut out.patches);
        self.events.truncate(EVENTS_KEPT, &mut out.patches);
    }

    fn put_event_signal(&mut self, s: &EventSignal, record: Value, out: &mut Out) {
        put_newest_first(
            &mut self.event_signals,
            s.id.clone(),
            Timed::of(s.rev, s.created_at),
            record,
            out,
        );
    }

    fn put_sensor(&mut self, s: &Sensor, record: Value, out: &mut Out) {
        put_newest_first(
            &mut self.sensors,
            s.id.clone(),
            Timed::of(s.rev, s.created_at),
            record,
            out,
        );
    }

    fn put_client_key(&mut self, k: &ClientKey, record: Value, out: &mut Out) {
        put_newest_first(
            &mut self.client_keys,
            k.id.clone(),
            Timed::of(k.rev, k.created_at),
            record,
            out,
        );
    }

    /// A plugin keeps the place the server lists it in: bundled first, then by id.
    fn put_extension(&mut self, e: &Extension, record: Value, out: &mut Out) {
        let bundled = e.origin == ExtensionOrigin::Bundled;
        let rank = |k: &Key, b: &bool| (!*b, k.clone());
        if self.extensions.get(&e.id).is_some_and(|held| *held != bundled) {
            self.extensions.remove(&e.id, &mut out.patches);
        }
        let before = self.extensions.before_by(rank, &(!bundled, e.id.clone()));
        self.extensions
            .put(e.id.clone(), bundled, record, before, &mut out.patches);
    }

    /// What falls beyond the newest 200 is let go.
    fn put_feedback(&mut self, f: &Feedback, record: Value, out: &mut Out) {
        put_newest_first(
            &mut self.feedback,
            f.id.clone(),
            Timed::of(f.rev, f.created_at),
            record,
            out,
        );
        self.feedback.truncate(FEEDBACK_KEPT, &mut out.patches);
    }

    /// An objective is pushed whole with where its key results stand, which changes without its revision changing, so a
    /// copy at the revision held replaces it; only a lower revision is stale.
    fn put_objective(&mut self, v: &ObjectiveView, record: Value, out: &mut Out) {
        let facts = Timed::of(v.objective.rev, v.objective.created_at);
        match self.objectives.get(&v.objective.id) {
            Some(held) if v.objective.rev < held.rev => {}
            Some(_) => self
                .objectives
                .put(v.objective.id.clone(), facts, record, None, &mut out.patches),
            None => {
                let before = before_time(&self.objectives, facts.at);
                self.objectives
                    .put(v.objective.id.clone(), facts, record, before, &mut out.patches);
            }
        }
    }

    /// A project goes where its name puts it, moving when it is renamed, unless the copy held is newer.
    fn put_project(&mut self, p: &Project, record: Value, out: &mut Out) {
        let facts = Named::of(p);
        if let Some(held) = self.projects.get(&p.id) {
            if !newer(Some(held.rev), facts.rev) {
                return;
            }
            if held.rank != facts.rank {
                self.projects.remove(&p.id, &mut out.patches);
            }
        }
        let before = self.projects.before_by(|_, m| m.rank.clone(), &facts.rank);
        self.projects.put(p.id.clone(), facts, record, before, &mut out.patches);
    }

    /// The employees stay in the order the server lists them, oldest first: a hire comes last.
    fn put_employee(&mut self, e: &EmployeeRead, record: Value, out: &mut Out) {
        let e = &e.employee;
        put_newer(&mut self.employees, e.id.clone(), e.rev, record, None, out);
    }

    /// The mentors stay in the order the server lists them, oldest added first: a new one comes last.
    fn put_mentor(&mut self, m: &Mentor, record: Value, out: &mut Out) {
        put_newer(&mut self.mentors, m.id.clone(), m.rev, record, None, out);
    }

    fn put_mentor_standing(&mut self, s: &MentorStanding, record: Value, out: &mut Out) {
        put_newer(&mut self.mentor_standings, s.mentor.clone(), s.rev, record, None, out);
    }
}

/// A duel judged or overturned moves where duels stand (`duels.ts`).
fn note_decision(d: &DecisionRead, out: &mut Out) {
    if matches!(d.decision.kind, DecisionKind::Duel | DecisionKind::Overturn) {
        out.notice(Notice::DuelsMoved);
    }
}
