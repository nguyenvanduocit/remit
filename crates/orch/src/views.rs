//! Where things stand (standing.rs) is computed from the records, so it changes whenever one of them does. The views keep
//! it current for every channel: a change the orchestrator notifies marks what it may move (views_watched.rs), and at
//! most every `VIEW_EVERY` the marked views are computed again from the store and the ones that changed are pushed, each
//! at a new revision. A channel's read takes them from the same computation, so what it reads and what it is pushed are
//! one sequence of revisions, newest last. The board is laid out from the records it keeps (`BoardRecords`): read whole
//! when none are kept, and otherwise only for the chains a change moved.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

use model::{Approval, Card, CardStatus, Chain, Edge, Event, FindingBlocker, Help, InboxItem};
use model::{Landing, Time};
use store::{Catalog, Moved};
use tokio::sync::Notify;
use wire::app::{
    AgentStanding, AssistantView, Board, CARD_EVENTS_PAGE, CHAIN_EVENTS_SHOWN, ChainRead, ChainStanding, ChainView,
    CheckBack, Column, EventPage, MentorStanding, OnMessage, Snapshot, Verification, Who,
};

use crate::agentstanding::{AgentsInput, agents_of, in_turn, standalone_standing};
use crate::assistant_tools::RECENT_EVENTS_SHOWN;
use crate::orch::Orchestrator;
use crate::standing::{Held, StandingInput, board_of, column_of, standing_of};
use crate::views_changes::ChangesRead;
use crate::waiting::{ChainRecords, WaitRecords, chain_waits, waits_of};
use capability::capabilities::capability_views;
use capability::library::listed_skills;
use chains::activity::event_read;
use chains::admit::{held_by_key_result, stall_of};
use chains::assistant::started_over;
use chains::homes::is_standalone_home;
use chains::project::project_of;
use chains::seen::{away_of, ended_at, finished_unseen};
use chains::session::Run;
use chains::wait::{chain_read, wake_of, wakes_by};
use chains::{Error, Result};
use crew::coach::shown_retrospectives;
use crew::employee_name::employee_read_of;
use crew::feedback::FEEDBACK_SHOWN;
use shipping::verification::{verdict_check, verdict_state, verification_of};

/// What a read of `shown` events asks the store for: one over, to know whether there are more.
fn read_for(shown: usize) -> i64 {
    i64::try_from(shown).map_or(i64::MAX, |n| n.saturating_add(1))
}

/// How long a change waits for the others that come with it before the views are computed again.
const VIEW_EVERY: Duration = Duration::from_millis(200);

/// The views: what changes marked (`Marks`), and what the last computation found, behind one lock held while it computes,
/// so revisions follow the order the store was read in; and the chains' changes, read with git apart from that lock.
pub(crate) struct Views {
    pub marks: Arc<Marks>,
    pub(crate) compute: tokio::sync::Mutex<Computed>,
    pub(crate) changes: ChangesRead,
}

/// What a change marks, and the kick that wakes the loop computing it.
#[derive(Default)]
pub(crate) struct Marks {
    kick: Notify,
    set: Mutex<Marked>,
}

/// The views a change may have moved since the last computation.
#[derive(Default)]
struct Marked {
    /// A chain or the inbox changed: the board may have moved.
    board: bool,
    /// Chains the board reads again: their record, verdicts or inbox items may have changed.
    moved: HashSet<String>,
    /// Chains whose agents or standing may have moved.
    chains: HashSet<String>,
    /// Objectives that changed: the board reads again the chains that serve them, whose project may have moved.
    objectives: HashSet<String>,
    /// A mentor's record, device keys, apps or helps moved: their standings may have moved (mentor_standing.rs).
    mentors: bool,
    /// A playbook, agent, skill or source moved: a Crew board's columns may have moved (boards.rs).
    boards: bool,
    /// Chains and cards a help on which moved: pushed again even when the owner's standing is unchanged, as a mentor's
    /// is worded from the help (worker.rs `mentor_chain_standing`, `mentor_agent_standing`).
    helped: HashSet<String>,
    /// Repositories a landing or a release of which was written: their queue and releases moved (repo_view.rs).
    repos: HashSet<String>,
}

/// What the last computation of the views found; held under `Views::compute`.
#[derive(Default)]
pub(crate) struct Computed {
    /// The last revision given; it starts at the process's start, above any an earlier process gave.
    pub(crate) rev: i64,
    records: Option<BoardRecords>,
    shown: Board,
    /// By chain id.
    standings: HashMap<String, ChainStanding>,
    /// By chain id: what of the board its standing was last computed from.
    placed: HashMap<String, Placement>,
    /// By card id.
    agents: HashMap<String, AgentStanding>,
    /// Each queued chain's place, from the last board.
    places: HashMap<String, usize>,
    /// Why each queued chain whose start is held waits, from the last board.
    held: HashMap<String, Held>,
    /// By chain id.
    verifications: HashMap<String, Verification>,
    /// By mentor id (mentor_standing.rs).
    pub(crate) mentors: HashMap<String, MentorStanding>,
    /// The seats at work now, by home (atwork.rs).
    pub(crate) at_work: crate::atwork::Kept,
    /// The boards' columns as last pushed (boards.rs).
    pub(crate) boards: wire::app::Boards,
}

impl Views {
    /// Views whose revisions start at now.
    pub(crate) fn new(now: Time) -> Self {
        let rev = now
            .0
            .unix_timestamp_nanos()
            .checked_div(1000)
            .and_then(|us| i64::try_from(us).ok());
        Self {
            marks: Arc::default(),
            compute: tokio::sync::Mutex::new(Computed {
                rev: rev.unwrap_or_default(),
                ..Computed::default()
            }),
            changes: ChangesRead::default(),
        }
    }
}

impl Marks {
    fn marked(&self) -> std::sync::MutexGuard<'_, Marked> {
        self.set.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Marks the views a change may move: chain_id's agents and standing, and with board its place on the board.
    pub(crate) fn touch(&self, chain_id: &str, board: bool) {
        {
            let mut m = self.marked();
            if !chain_id.is_empty() {
                m.chains.insert(chain_id.to_owned());
            }
            if board {
                m.move_chain(chain_id);
            }
        }
        self.kick.notify_one();
    }

    /// Marks the board after chain chain_id was deleted: the board reads it again and finds it gone. No view of it is
    /// computed again.
    pub(crate) fn gone(&self, chain_id: &str) {
        self.marked().move_chain(chain_id);
        self.kick.notify_one();
    }

    /// Marks help h's chain and card to be pushed again, and its chain's views and the board as `touch` does.
    pub(crate) fn touch_help(&self, h: &Help) {
        {
            let mut m = self.marked();
            m.helped.insert(h.chain.clone());
            m.helped.insert(h.card.clone());
        }
        self.touch(&h.chain, true);
    }

    /// Marks the chains that serve objective id: the board reads them again.
    pub(crate) fn touch_objective(&self, id: &str) {
        {
            let mut m = self.marked();
            m.board = true;
            m.objectives.insert(id.to_owned());
        }
        self.kick.notify_one();
    }

    /// Marks the mentors' standings.
    pub(crate) fn touch_mentors(&self) {
        self.marked().mentors = true;
        self.kick.notify_one();
    }

    /// Marks the boards' columns.
    pub(crate) fn touch_boards(&self) {
        self.marked().boards = true;
        self.kick.notify_one();
    }

    /// Marks repository repo's queue and releases.
    pub(crate) fn touch_repo(&self, repo: &str) {
        self.marked().repos.insert(repo.to_owned());
        self.kick.notify_one();
    }

    /// What was marked since the last take, the marks cleared.
    fn take(&self) -> Marked {
        std::mem::take(&mut *self.marked())
    }

    /// Marks again what a computation could not compute, without a kick: the next computation a change, or the Tick,
    /// kicks takes it up, so a store that keeps failing is not read again every `VIEW_EVERY`.
    fn restore(&self, failed: Marked) {
        self.marked().merge(failed);
    }
}

impl Marked {
    fn move_chain(&mut self, chain_id: &str) {
        self.board = true;
        if !chain_id.is_empty() {
            self.moved.insert(chain_id.to_owned());
        }
    }

    fn merge(&mut self, m: Self) {
        self.board |= m.board;
        self.moved.extend(m.moved);
        self.chains.extend(m.chains);
        self.objectives.extend(m.objectives);
        self.mentors |= m.mentors;
        self.boards |= m.boards;
        self.helped.extend(m.helped);
        self.repos.extend(m.repos);
    }
}

/// What the board is laid out from, as last read: every chain, each chain's verdicts (Catalog's: no notes nor findings),
/// what is the owner's to settle of each finished chain, and the open inbox.
#[derive(Default)]
struct BoardRecords {
    /// By id.
    chains: HashMap<String, Chain>,
    /// What each waiting chain waits for, by chain id.
    wakes: HashMap<String, CheckBack>,
    /// By chain id, oldest first.
    verdicts: HashMap<String, Vec<Approval>>,
    /// By chain id, oldest first.
    landings: HashMap<String, Vec<Landing>>,
    /// What is the owner's to settle of each finished chain (`owners_of_finished`), by chain id.
    owners: HashMap<String, Vec<FindingBlocker>>,
    /// When each finished chain last finished (`ended_at`), by chain id.
    ended: HashMap<String, Option<Time>>,
    /// Each chain's project (`project_of`), by chain id.
    projects: HashMap<String, Option<String>>,
    /// The owner's seen marks, by home: when he last viewed it.
    marks: HashMap<String, Time>,
    inbox: Vec<InboxItem>,
    /// Each objective's horizon, which orders the queue (`queue_of`).
    horizons: chains::admit::Horizons,
}

impl BoardRecords {
    fn of(c: &Catalog) -> Self {
        let mut r = Self::default();
        r.take(
            &[],
            Moved {
                chains: c.chains.clone(),
                waits: c.waits.clone(),
                verdicts: c.verdicts.clone(),
                landings: c.landings.clone(),
                inbox: c.inbox.clone(),
            },
        );
        r
    }

    /// Keeps m, read again for the chains ids names: one of them m does not hold was deleted.
    fn take(&mut self, ids: &[String], m: Moved) {
        for id in ids {
            self.chains.remove(id);
            self.wakes.remove(id);
            self.verdicts.remove(id);
            self.landings.remove(id);
            self.owners.remove(id);
            self.ended.remove(id);
            self.projects.remove(id);
        }
        self.wakes.extend(wakes_by(m.waits));
        for c in m.chains {
            self.chains.insert(c.id.clone(), c);
        }
        for a in m.verdicts {
            self.verdicts.entry(a.chain_id.clone()).or_default().push(a);
        }
        for l in m.landings {
            let mine = self.landings.entry(l.chain_id.clone()).or_default();
            mine.retain(|x| x.id != l.id);
            mine.push(l);
        }
        self.inbox = m.inbox;
    }

    /// The chains newest first, as the store lists them; chains made in the same instant, by id.
    fn ordered(&self) -> Vec<Chain> {
        let mut out: Vec<Chain> = self.chains.values().cloned().collect();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
        out
    }

    fn verdicts_of(&self, id: &str) -> &[Approval] {
        self.verdicts.get(id).map_or(&[], Vec::as_slice)
    }

    fn landings_of(&self, id: &str) -> &[Landing] {
        self.landings.get(id).map_or(&[], Vec::as_slice)
    }

    /// The chains whose change is on its base.
    fn landed(&self) -> HashSet<String> {
        self.landings
            .iter()
            .filter(|(_, ls)| chains::live::landed_at(ls).is_some())
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn owners_of(&self, id: &str) -> &[FindingBlocker] {
        self.owners.get(id).map_or(&[], Vec::as_slice)
    }

    fn project_of(&self, id: &str) -> Option<&str> {
        self.projects.get(id).and_then(Option::as_deref)
    }

    /// Whether chain c finished after the owner last viewed it.
    fn unseen(&self, c: &Chain) -> bool {
        let ended = self.ended.get(&c.id).copied().flatten();
        finished_unseen(ended, self.marks.get(&c.id).copied())
    }
}

/// What of the board a chain's standing is computed from: the chain at its revision, how many verdicts it has (a verdict
/// is never changed), what is the owner's to settle of it (a live mark settles it, the chain unchanged), what of it waits
/// for the owner, whether it finished unseen, where the board puts it, and its project (its objective's, which changes with
/// no change to the chain). A standing computed from a placement holds while the placement
/// does; what else it reads (the changes, the mentors helping) moves with the chain's own views (`chain_of`).
#[derive(Debug, Clone, PartialEq)]
struct Placement {
    rev: i64,
    wake: String,
    verdicts: usize,
    /// Each landing's id and revision: a landing moves with no change to its chain.
    landings: Vec<(String, i64)>,
    owners: Vec<FindingBlocker>,
    waits: Vec<wire::app::Wait>,
    unseen: bool,
    column: Column,
    place: usize,
    held: Option<Held>,
    project: Option<String>,
}

/// Names the wake a chain waits for, so a standing computed from it holds while the wake does.
fn wake_key(wake: Option<&CheckBack>) -> String {
    wake.map_or_else(String::new, |w| {
        format!("{} {} {}", w.at.0.unix_timestamp_nanos(), w.why, w.watch.is_some())
    })
}

/// The newest `shown` of events (read one over, to know whether there are more), with the message or thought each
/// of runs is streaming as it stands: its stored copy is written in batches, and lags behind what the Notifier has already
/// sent, which a channel goes on from.
fn page_of(mut events: Vec<Event>, shown: usize, runs: &[Arc<Run>]) -> EventPage {
    let over = events.len().saturating_sub(shown);
    events.drain(..over);
    for r in runs {
        let st = r.state();
        if let Some(stream) = &st.stream
            && let Some(e) = events.iter_mut().find(|e| e.id == stream.id)
        {
            e.clone_from(stream);
        }
    }
    EventPage {
        events: events.into_iter().map(event_read).collect(),
        truncated: over > 0,
    }
}

impl Orchestrator {
    fn datadog_links(&self, chain: &str) -> Vec<wire::app::DatadogLink> {
        let Some(dd) = &self.datadog else {
            return Vec::new();
        };
        dd.chain_links(chain)
            .into_iter()
            .map(|(label, url)| wire::app::DatadogLink {
                label: label.to_owned(),
                url,
            })
            .collect()
    }

    /// Everything that draws one chain.
    pub async fn view(&self, chain_id: String) -> Result<ChainView> {
        let v = self.store.view(chain_id.clone()).await?;
        let id = chain_id.clone();
        let retros = v.retrospectives;
        let (helps, mark, retrospectives, landings) = self
            .store
            .read(move |r| {
                Ok::<_, store::Error>((
                    r.chain_helps(&id)?,
                    r.seen(&id)?,
                    shown_retrospectives(r, retros)?,
                    chains::landings::chain_landings(r, &id)?,
                ))
            })
            .await?;
        let view = self.chain_of(&chain_id).await?;
        Ok(ChainView {
            away: away_of(&v.decisions, mark.map(|m| m.at)),
            cards: self.live_cards(v.cards),
            edges: v.edges,
            decisions: v
                .decisions
                .into_iter()
                .map(chains::shown_decision::shown_decision)
                .collect(),
            drafts: v.drafts.into_iter().filter(|d| d.mentor.is_empty()).collect(),
            helps,
            workflows: v.workflows,
            retrospectives,
            landings,
            datadog: self.datadog_links(&chain_id),
            chain: ChainRead {
                chain: v.chain,
                ..view.chain
            },
            ..view
        })
    }

    /// The newest events of every card of a chain.
    pub async fn chain_events(&self, chain_id: String) -> Result<EventPage> {
        let id = chain_id.clone();
        let events = self
            .store
            .read(move |r| r.chain_events(&id, read_for(CHAIN_EVENTS_SHOWN)))
            .await?;
        Ok(page_of(events, CHAIN_EVENTS_SHOWN, &self.chain_runs(&chain_id)))
    }

    /// One card's newest events or, before an event (its id; 0: none), the newest of those older than it: the page before
    /// the one a channel holds. A page begins at the newest Start over in it (`started_over`): the conversation before it
    /// is the page before.
    pub async fn card_events(&self, card_id: String, before: i64) -> Result<EventPage> {
        let before = if before <= 0 { i64::MAX } else { before };
        let id = card_id.clone();
        let events = self
            .store
            .read(move |r| r.card_events_before(&id, before, read_for(CARD_EVENTS_PAGE)))
            .await?;
        let runs: Vec<Arc<Run>> = self.run_of(&card_id).into_iter().collect();
        let mut p = page_of(events, CARD_EVENTS_PAGE, &runs);
        if let Some(i) = p.events.iter().rposition(|e| started_over(&e.event)) {
            p.events.drain(..i);
            p.truncated = true;
        }
        Ok(p)
    }

    /// Starts the loop that computes the marked views again, on the background work, until Shutdown begins. It holds the
    /// orchestrator weakly: one dropped stops it.
    pub(crate) fn start_views(&self) {
        if tokio::runtime::Handle::try_current().is_err() {
            tracing::warn!("no async runtime: the views are not kept current");
            return;
        }
        let (o, marks, stopped) = (
            self.me().as_ref().map(Arc::downgrade).unwrap_or_default(),
            Arc::clone(&self.views.marks),
            self.stopped.clone(),
        );
        self.background(move |_| views_loop(o, marks, stopped));
    }

    /// Computes again what m marked, and gives back what it could not: marked again, it is computed on a later pass.
    async fn refresh_views(&self, mut m: Marked) -> Marked {
        let mut failed = Marked::default();
        if !m.objectives.is_empty() {
            let comp = self.views.compute.lock().await;
            // with none kept, the board reads every chain anyway
            let serving = comp
                .records
                .iter()
                .flat_map(|rec| rec.chains.values())
                .filter(|c| c.serves.as_ref().is_some_and(|s| m.objectives.contains(&s.objective)));
            let ids: Vec<String> = serving.map(|c| c.id.clone()).collect();
            drop(comp);
            m.moved.extend(ids);
        }
        if !m.helped.is_empty() {
            let mut comp = self.views.compute.lock().await;
            for id in &m.helped {
                comp.standings.remove(id);
                comp.agents.remove(id);
            }
        }
        if m.mentors
            && let Err(err) = self.mentors_of().await
        {
            tracing::warn!(%err, "mentors' standings not computed");
            failed.mentors = true;
        }
        if m.boards
            && let Err(err) = self.boards().await
        {
            tracing::warn!(%err, "boards' columns not computed");
            failed.boards = true;
        }
        for repo in m.repos {
            match self.repo_view(repo.clone()).await {
                Ok(v) => self.notify.repo_updated(&v),
                Err(err) => {
                    tracing::warn!(%repo, %err, "repository's view not computed");
                    failed.repos.insert(repo);
                }
            }
        }
        if m.board {
            let moved: Vec<String> = m.moved.into_iter().collect();
            if let Err(err) = self.refresh_board(moved.clone()).await {
                tracing::warn!(%err, "board not computed");
                failed.board = true;
                failed.moved.extend(moved);
            }
        }
        for id in m.chains {
            let computed = if self.is_assistant_home(&id) {
                self.assistant_of().await.map(|_| ())
            } else {
                self.chain_of(&id).await.map(|_| ())
            };
            match computed {
                Ok(()) | Err(Error::Store(store::Error::NotFound) | Error::ChainDeleted) => {}
                Err(err) => {
                    tracing::warn!(home = %id, %err, "views not computed");
                    failed.chains.insert(id);
                }
            }
        }
        failed
    }

    /// The catalog as the store has it now (its chains newest first), the board laid out from it and where each chain
    /// stands, in the catalog's order: a standing whose placement moved is computed again, and the others are as the views
    /// last computed them, which every change that moves one marks.
    pub async fn reload(&self) -> Result<(Catalog, Board, Vec<ChainStanding>)> {
        let mut comp = self.views.compute.lock().await;
        let c = self.reload_locked(&mut comp).await?;
        let standings = c
            .chains
            .iter()
            .filter_map(|ch| comp.standings.get(&ch.id).cloned())
            .collect();
        Ok((c, comp.shown.clone(), standings))
    }

    /// Everything the app shows, as it is now: a channel that loads it and then follows the Notifier shows what the
    /// server has. Archived chains are left out; `archived_chains` pages them.
    pub async fn snapshot(&self) -> Result<Snapshot> {
        let (c, board, mut standings) = self.reload().await?;
        let assistant = self.assistant_of().await?;
        let assistant_feed = self.assistant_feed().await?;
        let events = self.pipeline.recent_events(RECENT_EVENTS_SHOWN).await?;
        let (event_signals, sensors, client_keys, feedback, staff, mentors, projects) = self
            .store
            .read(|r| {
                Ok::<_, store::Error>((
                    r.event_signals()?,
                    r.sensors()?,
                    r.client_keys()?,
                    r.recent_feedback(FEEDBACK_SHOWN)?,
                    r.employees()?,
                    r.mentors()?,
                    r.projects()?,
                ))
            })
            .await?;
        let extensions = self.reach.extensions().await?;
        let objectives = self.crew.objective_views().await?;
        let sources = self.reach.source_entries().await?;
        let mentor_standings = self.mentors_of().await?;
        let boards = self.boards().await?;
        let archived: HashSet<&str> = c
            .chains
            .iter()
            .filter(|ch| ch.archived_at.is_some())
            .map(|ch| ch.id.as_str())
            .collect();
        standings.retain(|st| !archived.contains(st.chain_id.as_str()));
        let wakes = wakes_by(c.waits);
        let chains = c
            .chains
            .iter()
            .filter(|ch| ch.archived_at.is_none())
            .map(|ch| chain_read(ch.clone(), &wakes))
            .collect();
        Ok(Snapshot {
            who: Who::Owner,
            chains,
            inbox: c.inbox,
            board,
            standings,
            playbooks: c.playbooks,
            agents: c.agents,
            skills: listed_skills(&c.skills),
            sources,
            capabilities: capability_views(),
            health: self.health(),
            workload: self.workload(),
            signals: self.pipeline.signal_board_now(),
            boards,
            insights: c.insights,
            assistant: Some(assistant),
            assistant_feed,
            events,
            event_signals,
            sensors,
            client_keys,
            extensions,
            feedback,
            objectives,
            projects,
            employees: staff.iter().map(employee_read_of).collect(),
            mentors,
            mentor_standings,
        })
    }

    /// Where each of the chains ids names stands, as the views last computed it, in ids' order; one they keep none of is
    /// left out.
    pub(crate) async fn standings_of(&self, ids: &[String]) -> Vec<ChainStanding> {
        let comp = self.views.compute.lock().await;
        ids.iter().filter_map(|id| comp.standings.get(id).cloned()).collect()
    }

    /// The view of the Assistant, its standing pushed when it changed.
    pub(crate) async fn assistant_of(&self) -> Result<AssistantView> {
        let mut comp = self.views.compute.lock().await;
        let a = self.the_assistant()?;
        let (card_id, home) = (a.card_id.clone(), a.id.clone());
        let (k, waiting, on_message, last, drafts) = self
            .store
            .read(move |r| {
                let k = r.card(&card_id)?;
                let waiting: Vec<InboxItem> = r.open_inbox()?.into_iter().filter(|it| it.home == home).collect();
                let (mut on_message, mut last) = (None, None);
                if in_turn(&k) {
                    match r.last_taken(&k.id) {
                        Ok((event_id, answering, at)) => {
                            on_message = Some(OnMessage {
                                event_id,
                                answering,
                                at,
                            })
                        }
                        Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                    match r.last_done(&k.id) {
                        Ok(e) => last = Some(e),
                        Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                // the owner's: a mentor's are theirs alone
                let drafts: Vec<_> = r.drafts(&home)?.into_iter().filter(|d| d.mentor.is_empty()).collect();
                Ok::<_, store::Error>((k, waiting, on_message, last, drafts))
            })
            .await?;
        let card = self.live_cards(vec![k]).pop().unwrap_or_default();
        let doing = |tool: &str| self.reach.tool_doing_of(tool);
        let st = standalone_standing(&card, &waiting, last.as_ref(), on_message, &doing);
        let agent = put_agent(self, &mut comp, st);
        let seats = crate::atwork::seats_of(None, std::slice::from_ref(&card), std::slice::from_ref(&agent));
        let kept = &mut *comp;
        let seen = kept.at_work.put(&agent.home, seats, &mut kept.rev);
        self.push_at_work(seen);
        Ok(AssistantView {
            assistant: a,
            card,
            agent,
            drafts,
        })
    }

    /// Reads every record the board is laid out from, and lays it out as the store has it now; comp is held.
    async fn reload_locked(&self, comp: &mut Computed) -> Result<Catalog> {
        comp.records = None;
        let c = self.store.catalog().await?;
        let mut rec = BoardRecords::of(&c);
        self.settle_waits(&mut rec, &c.chains).await?;
        self.lay_out(comp, &rec).await;
        comp.records = Some(rec);
        Ok(c)
    }

    /// Lays the board out again once the chains moved names may have moved: from what is read again of them, or from
    /// every record, read whole, when none are kept. A read that fails leaves none kept, so the next reads them all.
    async fn refresh_board(&self, moved: Vec<String>) -> Result<()> {
        let mut comp = self.views.compute.lock().await;
        let Some(mut rec) = comp.records.take() else {
            return self.reload_locked(&mut comp).await.map(|_| ());
        };
        let m = self.store.moved(moved.clone()).await?;
        let chains = m.chains.clone();
        rec.take(&moved, m);
        self.settle_waits(&mut rec, &chains).await?;
        self.lay_out(&mut comp, &rec).await;
        comp.records = Some(rec);
        Ok(())
    }

    /// Why the key result each queued chain of chains serves is stalled (`stall_of`), by chain id; one not stalled is not
    /// named. A read that fails is logged and names none: the board says less until the next lay-out.
    async fn stalls_of_queued(&self, chains: &[Chain]) -> HashMap<String, String> {
        let all = chains.to_vec();
        let read = self
            .store
            .read(move |r| {
                let mut out = HashMap::new();
                for c in all.iter().filter(|c| *c.status() == model::ChainStatus::Queued) {
                    if let Some(serves) = &c.serves
                        && let Some(why) = stall_of(r, &all, serves)?
                    {
                        out.insert(c.id.clone(), why);
                    }
                }
                Ok::<_, store::Error>(out)
            })
            .await;
        read.unwrap_or_else(|err| {
            tracing::error!(%err, "the stalled key results of the queue");
            HashMap::new()
        })
    }

    /// Reads what of chains their waits and standings read beside the catalog: what is the owner's to settle of each that
    /// finished, as the goal gate reads it (what their outcome reads), when they finished and their projects; and
    /// the owner's seen marks, all of them, as one may have moved with no chain changing.
    async fn settle_waits(&self, rec: &mut BoardRecords, chains: &[Chain]) -> Result<()> {
        for c in chains {
            let owners = self.shipping.owners_of_finished(c).await?;
            rec.owners.insert(c.id.clone(), owners);
        }
        let read: Vec<Chain> = chains.to_vec();
        let (reads, marks, objectives) = self
            .store
            .read(move |r| {
                let mut reads = Vec::with_capacity(read.len());
                for c in &read {
                    reads.push((c.id.clone(), ended_at(r, c)?, project_of(r, c)?));
                }
                Ok::<_, store::Error>((reads, r.seen_all()?, r.objectives()?))
            })
            .await?;
        for (id, ended, project) in reads {
            rec.ended.insert(id.clone(), ended);
            rec.projects.insert(id, project);
        }
        rec.marks = marks.into_iter().map(|m| (m.subject, m.at)).collect();
        rec.horizons = chains::admit::horizons_of(&objectives);
        Ok(())
    }

    /// Lays the board out from rec, pushing it when it changed, and every chain's standing on it: one whose placement moved
    /// is computed again and pushed when it changed, the others are as they were; comp is held.
    async fn lay_out(&self, comp: &mut Computed, rec: &BoardRecords) {
        let chains = rec.ordered();
        let waiting = waits_of(&WaitRecords {
            chains: &chains,
            inbox: &rec.inbox,
            marks: &rec.marks,
        });
        let mut waits: HashMap<&str, Vec<wire::app::Wait>> = HashMap::new();
        for w in &waiting {
            if let Some(id) = w.chain_id.as_deref() {
                waits.entry(id).or_default().push(w.clone());
            }
        }
        let mut b = board_of(
            &chains,
            &rec.wakes,
            &rec.landed(),
            waiting.clone(),
            &rec.inbox,
            &rec.horizons,
        );
        b.rev = comp.shown.rev;
        if b != comp.shown {
            comp.rev = comp.rev.saturating_add(1);
            b.rev = comp.rev;
            comp.shown = b.clone();
            self.notify.board_updated(&b);
        }
        let mut col: HashMap<&str, Column> = HashMap::new();
        for c in &b.columns {
            for id in &c.chains {
                col.insert(id, c.key);
            }
            if c.key == Column::Queued {
                comp.places.clear();
                for (i, id) in c.chains.iter().enumerate() {
                    comp.places.insert(id.clone(), i.saturating_add(1));
                }
            }
        }
        comp.held.clear();
        let stalls = self.stalls_of_queued(&chains).await;
        let all: Vec<Landing> = rec.landings.values().flatten().cloned().collect();
        for c in &chains {
            let stalled = stalls.get(&c.id).map_or("", String::as_str);
            let held = Held {
                line: "Held for its key result",
                why: held_by_key_result(c, &chains, &all, stalled),
            };
            if !held.why.is_empty() {
                comp.held.insert(c.id.clone(), held);
            }
        }
        for c in &chains {
            let its = waits.get(c.id.as_str()).map_or(&[][..], Vec::as_slice);
            let owners = rec.owners_of(&c.id);
            let wake = rec.wakes.get(&c.id);
            // an archived chain is on no column
            let column = col.get(c.id.as_str()).copied().unwrap_or_else(|| {
                let landed = chains::live::landed_at(rec.landings_of(&c.id)).is_some();
                column_of(c, landed, !its.is_empty(), wake.is_some_and(|w| w.watch.is_some()))
            });
            let at = Placement {
                rev: c.rev,
                wake: wake_key(wake),
                verdicts: rec.verdicts_of(&c.id).len(),
                landings: rec.landings_of(&c.id).iter().map(|l| (l.id.clone(), l.rev)).collect(),
                owners: owners.to_vec(),
                waits: its.to_vec(),
                unseen: rec.unseen(c),
                column,
                place: comp.places.get(&c.id).copied().unwrap_or_default(),
                held: comp.held.get(&c.id).cloned(),
                project: rec.project_of(&c.id).map(str::to_owned),
            };
            let holds = comp.standings.contains_key(&c.id) && comp.placed.get(&c.id) == Some(&at);
            if !holds {
                self.standing_on_board(comp, rec, c, &at).await;
                comp.placed.insert(c.id.clone(), at);
            }
        }
        forget(comp, &chains);
        self.views.changes.keep_only(&chains);
        let held: HashSet<&str> = chains.iter().map(|c| c.id.as_str()).collect();
        let gone = comp
            .at_work
            .forget(|h| held.contains(h) || is_standalone_home(h), &mut comp.rev);
        self.push_at_work(gone);
    }

    /// Chain c's standing where at places it, pushed when it changed; comp is held.
    async fn standing_on_board(&self, comp: &mut Computed, rec: &BoardRecords, c: &Chain, at: &Placement) {
        let verdicts = rec.verdicts_of(&c.id);
        let mentors = self.mentors_waited_on(c, at.column).await;
        let current = self.views.changes.current(c, verdicts);
        let s = standing_of(
            c,
            StandingInput {
                col: at.column,
                place: comp.places.get(&c.id).copied().unwrap_or_default(),
                held: comp.held.get(&c.id),
                wake: rec.wakes.get(&c.id),
                waits: &at.waits,
                verdicts,
                landings: rec.landings_of(&c.id),
                owners: rec.owners_of(&c.id),
                mentors: &mentors,
                current: Some(&current),
                unseen: at.unseen,
                project: at.project.as_deref(),
            },
        );
        put_standing(self, comp, s);
    }

    /// Reads chain chain_id's changes again from its worktrees (views_changes.rs), for its views to judge its verdicts
    /// against; the views' lock is not held, as git may take seconds.
    async fn read_changes(&self, chain_id: &str) -> Result<()> {
        let id = chain_id.to_owned();
        let (c, approvals, landings) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.chain(&id)?, r.approvals(&id)?, r.chain_landings(&id)?)))
            .await?;
        self.views
            .changes
            .read(&c, &approvals, &landings, &self.cfg.folders_dir)
            .await;
        Ok(())
    }

    /// The mentors whose helps on chain c are open when it is in the Waiting column, the one standing that names them, and
    /// no one otherwise. A read that fails is logged and names no one: the chain's own views name them once the store
    /// answers.
    async fn mentors_waited_on(&self, c: &Chain, col: Column) -> Vec<String> {
        if col != Column::Waiting {
            return Vec::new();
        }
        self.helping_now(&c.id).await.unwrap_or_else(|err| {
            tracing::warn!(chain = %c.id, %err, "helps not read: its standing names no one it waits on");
            Vec::new()
        })
    }

    /// The views of chain chain_id: where each of its agents stands and where the chain stands, pushing what changed. Its
    /// records are left for the caller. Its changes are read again first, before the views' lock is taken.
    pub(crate) async fn chain_of(&self, chain_id: &str) -> Result<ChainView> {
        self.read_changes(chain_id).await?;
        let mut comp = self.views.compute.lock().await;
        let c = self.chain(chain_id).await?;
        let ch = c.clone();
        let read = self.store.read(move |r| read_chain_views(r, &ch)).await?;
        let cards = self.live_cards(read.cards);
        let watches = self.watching_cards(chain_id).await?;
        let helping = self.helping_now(chain_id).await?;
        let owners = self.shipping.owners_of_finished(&c).await?;
        let held = self.waits_of(chain_id).await?;
        let mut wakes = HashMap::new();
        if let Some(w) = wake_of(&held) {
            wakes.insert(chain_id.to_owned(), w);
        }
        let wake = wakes.get(chain_id);
        let doing = |tool: &str| self.reach.tool_doing_of(tool);
        let verdict_label = |a: &Approval| verdict_check(a, verdict_state(a, None)).label;
        let agents = agents_of(
            &c,
            AgentsInput {
                cards: &cards,
                edges: &read.edges,
                verdicts: &read.approvals,
                owners: &owners,
                items: &read.items,
                last: &read.last,
                on_message: &read.on_message,
                watches: &watches,
                doing: &doing,
                verdict_label: &verdict_label,
                helping: &helping,
                checks_back: wake.is_some(),
                landings: &read.landings,
            },
        )
        .into_iter()
        .map(|a| put_agent(self, &mut comp, a))
        .collect::<Vec<AgentStanding>>();
        let seats = crate::atwork::seats_of(Some(&c), &cards, &agents);
        let kept = &mut *comp;
        let seen = kept.at_work.put(&c.id, seats, &mut kept.rev);
        self.push_at_work(seen);
        let waits = chain_waits(
            &c,
            &ChainRecords {
                items: &read.items,
                mark: read.mark,
            },
        );
        let landed = chains::live::landed_at(&read.landings).is_some();
        let col = column_of(&c, landed, !waits.is_empty(), wake.is_some_and(|w| w.watch.is_some()));
        let current = self.views.changes.current(&c, &read.approvals);
        let ver = put_verification(
            self,
            &mut comp,
            verification_of(&c, &read.approvals, &read.landings, Some(&current)),
        );
        let place = comp.places.get(chain_id).copied().unwrap_or_default();
        let held = comp.held.get(chain_id).cloned();
        let st = standing_of(
            &c,
            StandingInput {
                col,
                place,
                held: held.as_ref(),
                wake,
                waits: &waits,
                verdicts: &read.approvals,
                landings: &read.landings,
                owners: &owners,
                mentors: &helping,
                current: Some(&current),
                unseen: finished_unseen(read.ended, read.mark),
                project: read.project.as_deref(),
            },
        );
        let standing = put_standing(self, &mut comp, st);
        Ok(ChainView {
            chain: chain_read(c, &wakes),
            agents,
            verification: ver,
            standing,
            ..ChainView::default()
        })
    }
}

/// The loop that computes the marked views again, at most every `VIEW_EVERY`, until stopped is cancelled or the
/// orchestrator is gone.
async fn views_loop(o: Weak<Orchestrator>, marks: Arc<Marks>, stopped: tokio_util::sync::CancellationToken) {
    loop {
        tokio::select! {
            () = stopped.cancelled() => return,
            () = marks.kick.notified() => {}
        }
        tokio::select! {
            () = stopped.cancelled() => return,
            () = tokio::time::sleep(VIEW_EVERY) => {}
        }
        let Some(o) = o.upgrade() else { return };
        let failed = o.refresh_views(marks.take()).await;
        marks.restore(failed);
    }
}

/// What the views of one chain read of the store, in one read.
struct ChainViewsRead {
    cards: Vec<Card>,
    edges: Vec<Edge>,
    approvals: Vec<Approval>,
    landings: Vec<Landing>,
    /// Its open inbox items.
    items: Vec<InboxItem>,
    /// The newest thing each running card did, by card id.
    last: HashMap<String, Event>,
    /// The message each card in a turn is on, by card id.
    on_message: HashMap<String, OnMessage>,
    /// When it last finished (`ended_at`), and when the owner last viewed it.
    ended: Option<Time>,
    mark: Option<Time>,
    /// Its project (`project_of`).
    project: Option<String>,
}

fn read_chain_views(r: &store::Reader<'_>, c: &Chain) -> store::Result<ChainViewsRead> {
    let chain_id = c.id.as_str();
    let cards = r.cards(chain_id)?;
    let items = r.open_inbox()?.into_iter().filter(|it| it.home == chain_id).collect();
    let approvals = r.approvals(chain_id)?;
    let mut last = HashMap::new();
    let mut on_message = HashMap::new();
    for k in &cards {
        if !in_turn(k) {
            continue;
        }
        if let Some((event_id, answering, at)) = found(r.last_taken(&k.id))? {
            on_message.insert(
                k.id.clone(),
                OnMessage {
                    event_id,
                    answering,
                    at,
                },
            );
        }
        if k.status() != &CardStatus::Running {
            continue;
        }
        if let Some(e) = found(r.last_done(&k.id))? {
            last.insert(k.id.clone(), e);
        }
    }
    let edges = r.edges(chain_id)?;
    Ok(ChainViewsRead {
        cards,
        edges,
        approvals,
        landings: r.chain_landings(chain_id)?,
        items,
        last,
        on_message,
        ended: ended_at(r, c)?,
        mark: r.seen(chain_id)?.map(|m| m.at),
        project: project_of(r, c)?,
    })
}

/// The record read, `None` when the store has none.
fn found<T>(read: store::Result<T>) -> store::Result<Option<T>> {
    match read {
        Ok(v) => Ok(Some(v)),
        Err(store::Error::NotFound) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Drops what is kept for the chains no longer on the board, chains (deleted); comp is held.
fn forget(comp: &mut Computed, chains: &[Chain]) {
    let held: HashSet<&str> = chains.iter().map(|c| c.id.as_str()).collect();
    comp.standings.retain(|id, _| held.contains(id.as_str()));
    comp.verifications.retain(|id, _| held.contains(id.as_str()));
    comp.placed.retain(|id, _| held.contains(id.as_str()));
    comp.agents
        .retain(|_, a| held.contains(a.home.as_str()) || is_standalone_home(&a.home));
}

/// Keeps s as the chain's standing, pushed at a new revision when it changed; comp is held.
fn put_standing(o: &Orchestrator, comp: &mut Computed, mut s: ChainStanding) -> ChainStanding {
    let was = comp.standings.get(&s.chain_id);
    s.rev = was.map_or(0, |w| w.rev);
    if let Some(was) = was
        && *was == s
    {
        return s;
    }
    comp.rev = comp.rev.saturating_add(1);
    s.rev = comp.rev;
    comp.standings.insert(s.chain_id.clone(), s.clone());
    o.notify.chain_standing_updated(&s);
    s
}

/// Keeps ver as the chain's verification, pushed at a new revision when it changed; comp is held.
fn put_verification(o: &Orchestrator, comp: &mut Computed, mut ver: Verification) -> Verification {
    let was = comp.verifications.get(&ver.chain_id);
    ver.rev = was.map_or(0, |w| w.rev);
    if let Some(was) = was
        && *was == ver
    {
        return ver;
    }
    comp.rev = comp.rev.saturating_add(1);
    ver.rev = comp.rev;
    comp.verifications.insert(ver.chain_id.clone(), ver.clone());
    o.notify.verification_updated(&ver);
    ver
}

/// Keeps a as the agent's standing, pushed at a new revision when it changed; comp is held.
fn put_agent(o: &Orchestrator, comp: &mut Computed, mut a: AgentStanding) -> AgentStanding {
    let was = comp.agents.get(&a.card_id);
    a.rev = was.map_or(0, |w| w.rev);
    if let Some(was) = was
        && *was == a
    {
        return a;
    }
    comp.rev = comp.rev.saturating_add(1);
    a.rev = comp.rev;
    comp.agents.insert(a.card_id.clone(), a.clone());
    o.notify.agent_standing_updated(&a);
    a
}
