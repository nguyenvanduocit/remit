//! `Watched` is the Notifier orch calls: it passes every change on to the channels' and marks the views it may move. It
//! forwards every method of Notifier, those with a default body too, so a kind of change added to Notifier is forwarded
//! here as well, or the channels never hear of it. It tells and marks only: what a change means for the domain is stored
//! with the change.

use std::sync::Arc;
use wire::app::CardShown;

use model::{Card, ClientKey, Decision, Draft, Edge, Employee, Event, Help, InboxItem, Mentor};

use crate::notify::Notifier;
use crate::views::Marks;
use wire::app::{AgentStanding, Board, ChainRead, ChainStanding};

pub(crate) struct Watched {
    /// The channels'.
    pub inner: Arc<dyn Notifier>,
    pub marks: Arc<Marks>,
}

impl chains::Notifier for Watched {
    fn chain_updated(&self, c: &ChainRead) {
        self.inner.chain_updated(c);
        self.marks.touch(&c.chain.id, true);
    }

    fn chain_deleted(&self, chain_id: &str) {
        self.inner.chain_deleted(chain_id);
        self.marks.gone(chain_id);
    }

    fn card_updated(&self, c: &Card) {
        self.inner.card_updated(c);
        self.marks.touch(&c.home, false);
    }

    fn card_shown(&self, s: &CardShown) {
        self.inner.card_shown(s);
        self.marks.touch(&s.home, false);
    }

    fn event_delta(&self, d: &wire::app::EventDelta) {
        self.inner.event_delta(d);
    }

    fn edge_added(&self, e: &Edge) {
        self.inner.edge_added(e);
        self.marks.touch(&e.chain_id, false);
    }

    fn decision_added(&self, d: &Decision) {
        self.inner.decision_added(d);
    }

    fn event_added(&self, e: &Event) {
        self.inner.event_added(e);
        self.marks.touch(&e.home, false);
    }

    fn event_updated(&self, e: &Event) {
        self.inner.event_updated(e);
        self.marks.touch(&e.home, false);
    }

    fn event_deleted(&self, e: &Event) {
        self.inner.event_deleted(e);
        self.marks.touch(&e.home, false);
    }

    fn inbox_updated(&self, it: &InboxItem) {
        self.inner.inbox_updated(it);
        self.marks.touch(&it.home, true);
    }

    fn employee_updated(&self, e: &Employee) {
        self.inner.employee_updated(e);
    }

    fn client_key_updated(&self, k: &ClientKey) {
        self.inner.client_key_updated(k);
    }

    fn client_key_deleted(&self, id: &str) {
        self.inner.client_key_deleted(id);
    }

    fn worker_key_deleted(&self, id: &str) {
        self.inner.worker_key_deleted(id);
    }

    fn workflow_updated(&self, run: &model::Workflow) {
        self.inner.workflow_updated(run);
        self.marks.touch(&run.home, false);
    }

    fn draft_updated(&self, d: &Draft) {
        self.inner.draft_updated(d);
    }

    fn draft_deleted(&self, d: &Draft) {
        self.inner.draft_deleted(d);
    }

    fn health_updated(&self, h: &wire::app::Health) {
        self.inner.health_updated(h);
    }

    fn mentor_updated(&self, m: &Mentor) {
        self.inner.mentor_updated(m);
        self.marks.touch_mentors();
    }

    /// Moves its chain's views (a chain waits on its mentors) and its mentor's standing (their open helps).
    fn help_updated(&self, h: &Help) {
        self.inner.help_updated(h);
        self.marks.touch_help(h);
        self.marks.touch_mentors();
    }

    fn workload_updated(&self, w: &wire::app::Workload) {
        self.inner.workload_updated(w);
    }

    fn at_work_updated(&self, v: &wire::app::AtWork) {
        self.inner.at_work_updated(v);
    }

    fn boards_updated(&self, b: &wire::app::Boards) {
        self.inner.boards_updated(b);
    }

    fn event_signal_updated(&self, es: &model::EventSignal) {
        self.inner.event_signal_updated(es);
    }

    fn event_signal_deleted(&self, id: &str) {
        self.inner.event_signal_deleted(id);
    }

    fn sensor_updated(&self, s: &model::Sensor) {
        self.inner.sensor_updated(s);
    }

    fn sensor_deleted(&self, id: &str) {
        self.inner.sensor_deleted(id);
    }

    fn source_updated(&self, s: &wire::app::ToolSource) {
        self.inner.source_updated(s);
        self.marks.touch_boards();
    }

    fn extension_updated(&self, e: &model::Extension) {
        self.inner.extension_updated(e);
        self.marks.touch_boards();
    }

    fn extension_deleted(&self, id: &str) {
        self.inner.extension_deleted(id);
        self.marks.touch_boards();
    }

    fn playbook_updated(&self, p: &model::Playbook) {
        self.inner.playbook_updated(p);
        self.marks.touch_boards();
    }

    fn playbook_deleted(&self, id: &str) {
        self.inner.playbook_deleted(id);
        self.marks.touch_boards();
    }

    fn agent_updated(&self, a: &model::Agent) {
        self.inner.agent_updated(a);
        self.marks.touch_boards();
    }

    fn agent_deleted(&self, id: &str) {
        self.inner.agent_deleted(id);
        self.marks.touch_boards();
    }

    fn skill_deleted(&self, name: &str) {
        self.inner.skill_deleted(name);
        self.marks.touch_boards();
    }

    fn feedback_updated(&self, f: &model::Feedback) {
        self.inner.feedback_updated(f);
    }

    fn insight_added(&self, d: &Decision) {
        self.inner.insight_added(d);
    }

    /// Moves the standings of its chains, which name its project.
    fn objective_updated(&self, v: &model::ObjectiveView) {
        self.inner.objective_updated(v);
        self.marks.touch_objective(&v.objective.id);
    }

    fn project_updated(&self, p: &model::Project) {
        self.inner.project_updated(p);
    }

    fn views_moved(&self, chain_id: &str) {
        self.marks.touch(chain_id, true);
    }

    fn landing_updated(&self, l: &wire::app::LandingRead) {
        self.inner.landing_updated(l);
    }

    fn repo_moved(&self, repo: &str) {
        self.marks.touch_repo(repo);
    }

    fn mentors_moved(&self) {
        self.marks.touch_mentors();
    }
}

impl pipeline::notify::Notifier for Watched {
    fn event_read(&self, e: &wire::app::EventRecord) {
        self.inner.event_read(e);
    }

    fn signal_board_updated(&self, b: &wire::app::SignalBoard) {
        self.inner.signal_board_updated(b);
    }
}

impl capability::notify::Notifier for Watched {
    fn skill_updated(&self, s: &wire::app::ListedSkill) {
        self.inner.skill_updated(s);
        self.marks.touch_boards();
    }
}

impl crew::notify::Notifier for Watched {
    fn retrospective_added(&self, r: &wire::app::RetrospectiveRead) {
        self.inner.retrospective_added(r);
    }
}

impl crate::notify::Notifier for Watched {
    fn assistant_feed_updated(&self, f: &wire::app::AssistantFeed) {
        self.inner.assistant_feed_updated(f);
    }

    fn mentor_standing_updated(&self, s: &wire::app::MentorStanding) {
        self.inner.mentor_standing_updated(s);
    }

    fn board_updated(&self, b: &Board) {
        self.inner.board_updated(b);
    }

    fn chain_standing_updated(&self, s: &ChainStanding) {
        self.inner.chain_standing_updated(s);
    }

    fn verification_updated(&self, v: &wire::app::Verification) {
        self.inner.verification_updated(v);
    }

    fn agent_standing_updated(&self, a: &AgentStanding) {
        self.inner.agent_standing_updated(a);
    }

    fn repo_updated(&self, v: &wire::app::RepoView) {
        self.inner.repo_updated(v);
    }
}
