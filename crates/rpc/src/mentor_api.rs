//! `MentorApi` answers what a mentor's session asks (ADR-0089): `wire::app::Handler`, as the mentor of the worker key the
//! session signed in with. Deny by default: every call refuses but those a mentor makes, and each of those is answered
//! by the orchestrator's mentor functions (orch `worker.rs`), which read the mentor's own helps on every call. A call
//! added to the contract does not compile here until it says what a mentor gets.

use std::sync::Arc;

use link::{ErrorCode, Failure, Request};
use model::ChangeKind;
use model::{Change, Feedback, Help, OutcomeRange, OutcomeWindow};
use orch::{Orchestrator, WorkerRequest};
use wire::Ack;
use wire::app::{
    CardContext, CardTelemetry, ChainPage, ChainView, ClientKeyValue, Delivery, DraftInput, EventPage, EventRead,
    EventRecord, EventSignalKey, Handler, Health, Judgments, LandingDiff, LinkReport, MemoryBook, NewChain, Outcomes,
    RepoView, SeenSubject, SignalDetail, SignalRow, SkillFileText, Snapshot, TaskOutput, TurnMark, WorkDuels,
    WorkerKeyValue,
};

use crate::broadcast::Showing;
use crate::convert::{done, failed, returned};
use crate::link_report::{FaultBudget, Sender};

/// The orchestrator, asked by one mentor: what one mentor's session's calls are answered by.
pub struct MentorApi {
    o: Arc<Orchestrator>,
    /// The worker key the session signed in with: every call reads its mentor anew, so a revoked key acts no more.
    key: model::WorkerKey,
    /// The homes the session shows, which what streams reaches.
    showing: Showing,
    /// The faults its pages may still send this minute.
    faults: Arc<FaultBudget>,
}

impl MentorApi {
    pub(crate) fn new(o: Arc<Orchestrator>, key: model::WorkerKey, showing: Showing) -> Self {
        Self {
            o,
            key,
            showing,
            faults: Arc::default(),
        }
    }

    /// The mentor's act on help help_id, with the page's request id.
    fn act(&self, request: Request, help_id: String) -> WorkerRequest {
        WorkerRequest {
            id: request.id,
            key_id: self.key.id.clone(),
            help_id,
        }
    }
}

/// What a mentor is told of a call they may not make.
fn denied(method: &str) -> Failure {
    Failure::new(
        ErrorCode::Forbidden,
        format!("{method}: a mentor may not make this call"),
    )
}

impl Handler for MentorApi {
    async fn app_snapshot(&self) -> Result<Snapshot, Failure> {
        returned("app_snapshot", self.o.mentor_snapshot(&self.key.id).await)
    }

    async fn chain_get(&self, id: String) -> Result<ChainView, Failure> {
        returned("chain_get", self.o.mentor_chain(&self.key.id, id).await)
    }

    async fn chain_events(&self, id: String) -> Result<EventPage, Failure> {
        returned("chain_events", self.o.mentor_chain_events(&self.key.id, id).await)
    }

    async fn homes_shown(&self, homes: Vec<String>, seq: u64) -> Result<Ack, Failure> {
        self.showing.set(homes, seq);
        Ok(Ack {})
    }

    async fn link_report(&self, report: LinkReport) -> Result<Ack, Failure> {
        // a mentor names only a chain one of his helps is on
        crate::link_report::keep(&self.o, Sender::Mentor(&self.key.id), report, &self.faults).await
    }

    /// Attachments are the owner's for now: a mentor's would be scoped to the cards of their helps first.
    async fn attachment_put(&self, _id: String, _home: String, _attachment: model::Attachment) -> Result<Ack, Failure> {
        Err(denied("attachment_put"))
    }

    async fn attachment_get(&self, _id: String) -> Result<model::Attachment, Failure> {
        Err(denied("attachment_get"))
    }

    async fn card_events(&self, card_id: String, before: i64) -> Result<EventPage, Failure> {
        returned(
            "card_events",
            self.o.mentor_card_events(&self.key.id, card_id, before).await,
        )
    }

    async fn draft_save(&self, request: Request, draft: DraftInput, base_rev: i64) -> Result<model::Draft, Failure> {
        let d = model::Draft::from(draft);
        self.o
            .mentor_save_draft(&request.id, &self.key.id, d, base_rev)
            .await
            .map_err(|e| failed("draft_save", &e))
    }

    async fn draft_delete(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "draft_delete",
            self.o.mentor_delete_draft(&request.id, &self.key.id, id).await,
        )
    }

    async fn card_prompt(&self, request: Request, draft_id: String, delivery: Delivery) -> Result<EventRead, Failure> {
        self.o
            .mentor_prompt(&request.id, &self.key.id, draft_id, delivery)
            .await
            .map(orch::event_read)
            .map_err(|e| failed("card_prompt", &e))
    }

    /// Scoped by the orchestrator: a reply on a card one of the mentor's own helps is on, once they read it.
    async fn turn_teach(&self, request: Request, event_id: i64, mark: TurnMark, words: String) -> Result<Ack, Failure> {
        done(
            "turn_teach",
            self.o
                .crew
                .mentor_teach_turn(&request.id, &self.key.id, event_id, mark, words)
                .await,
        )
    }

    async fn help_accept(&self, request: Request, help_id: String) -> Result<Help, Failure> {
        returned(
            "help_accept",
            self.o.crew.accept_help(&self.act(request, help_id)).await,
        )
    }

    async fn help_decline(&self, request: Request, help_id: String, why: String) -> Result<Help, Failure> {
        returned(
            "help_decline",
            self.o.crew.decline_help(&self.act(request, help_id), &why).await,
        )
    }

    async fn health_check(&self) -> Result<Health, Failure> {
        Err(denied("health_check"))
    }

    async fn chains_archived(&self, _after: String) -> Result<ChainPage, Failure> {
        Err(denied("chains_archived"))
    }

    async fn event_get(&self, _id: String) -> Result<EventRecord, Failure> {
        Err(denied("event_get"))
    }

    async fn signal_get(&self, _address: String) -> Result<SignalDetail, Failure> {
        Err(denied("signal_get"))
    }

    async fn signal_rows(&self, _pattern: String, _limit: i64) -> Result<Vec<SignalRow>, Failure> {
        Err(denied("signal_rows"))
    }

    async fn skill_file(&self, _name: String, _path: String) -> Result<SkillFileText, Failure> {
        Err(denied("skill_file"))
    }

    async fn changes_get(&self, _kind: ChangeKind, _name: String) -> Result<Vec<Change>, Failure> {
        Err(denied("changes_get"))
    }

    async fn card_context(&self, _card_id: String) -> Result<CardContext, Failure> {
        Err(denied("card_context"))
    }

    async fn card_processes(&self, _card_id: String) -> Result<wire::app::CardProcesses, Failure> {
        Err(denied("card_processes"))
    }

    async fn card_telemetry(&self, _card_id: String) -> Result<CardTelemetry, Failure> {
        Err(denied("card_telemetry"))
    }

    async fn card_task_output(&self, _event_id: i64) -> Result<TaskOutput, Failure> {
        Err(denied("card_task_output"))
    }

    async fn landing_diff(&self, _landing_id: String) -> Result<LandingDiff, Failure> {
        Err(denied("landing_diff"))
    }

    async fn repo_views(&self) -> Result<Vec<RepoView>, Failure> {
        Err(denied("repo_views"))
    }

    async fn repo_get(&self, _repo: String) -> Result<RepoView, Failure> {
        Err(denied("repo_get"))
    }

    async fn event_signal_key(&self, _id: String) -> Result<EventSignalKey, Failure> {
        Err(denied("event_signal_key"))
    }

    async fn client_key(&self, _id: String) -> Result<ClientKeyValue, Failure> {
        Err(denied("client_key"))
    }

    async fn worker_key(&self, _id: String) -> Result<WorkerKeyValue, Failure> {
        Err(denied("worker_key"))
    }

    async fn outcomes_get(&self, _last: OutcomeRange, _window: OutcomeWindow) -> Result<Outcomes, Failure> {
        Err(denied("outcomes_get"))
    }

    async fn judgments_get(&self) -> Result<Judgments, Failure> {
        Err(denied("judgments_get"))
    }

    async fn employee_memory(&self, _id: String) -> Result<MemoryBook, Failure> {
        Err(denied("employee_memory"))
    }

    async fn card_memory(&self, _card_id: String) -> Result<MemoryBook, Failure> {
        Err(denied("card_memory"))
    }

    async fn mentor_memory(&self, _id: String) -> Result<MemoryBook, Failure> {
        Err(denied("mentor_memory"))
    }

    async fn duel_standings(&self) -> Result<Vec<WorkDuels>, Failure> {
        Err(denied("duel_standings"))
    }

    async fn at_work(&self) -> Result<wire::app::AtWork, Failure> {
        Err(denied("at_work"))
    }

    async fn employee_runs(&self, _id: String) -> Result<wire::app::EmployeeRuns, Failure> {
        Err(denied("employee_runs"))
    }

    async fn account_add(&self, _request: Request) -> Result<model::InboxItem, Failure> {
        Err(denied("account_add"))
    }

    async fn account_remove(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("account_remove"))
    }

    async fn chain_create(&self, _request: Request, _input: NewChain) -> Result<model::Chain, Failure> {
        Err(denied("chain_create"))
    }

    async fn chain_pause(&self, _request: Request, _id: String, _why: String) -> Result<Ack, Failure> {
        Err(denied("chain_pause"))
    }

    async fn chain_resume(&self, _request: Request, _id: String, _why: String) -> Result<Ack, Failure> {
        Err(denied("chain_resume"))
    }

    async fn chain_check_back_now(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("chain_check_back_now"))
    }

    async fn chain_finish(&self, _request: Request, _id: String, _summary: String) -> Result<Ack, Failure> {
        Err(denied("chain_finish"))
    }

    async fn chain_archive(&self, _request: Request, _id: String, _why: String) -> Result<Ack, Failure> {
        Err(denied("chain_archive"))
    }

    async fn chain_unarchive(&self, _request: Request, _id: String, _why: String) -> Result<Ack, Failure> {
        Err(denied("chain_unarchive"))
    }

    async fn chain_delete(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("chain_delete"))
    }

    async fn card_prompt_withdraw(&self, _request: Request, _event_id: i64) -> Result<model::Draft, Failure> {
        Err(denied("card_prompt_withdraw"))
    }

    async fn card_cancel(&self, _request: Request, _card_id: String) -> Result<Ack, Failure> {
        Err(denied("card_cancel"))
    }

    async fn card_restart(&self, _request: Request, _card_id: String) -> Result<Ack, Failure> {
        Err(denied("card_restart"))
    }

    async fn card_stop(&self, _request: Request, _card_id: String, _why: String) -> Result<model::Card, Failure> {
        Err(denied("card_stop"))
    }

    async fn card_stop_explain(&self, _request: Request, _card_id: String, _why: String) -> Result<Ack, Failure> {
        Err(denied("card_stop_explain"))
    }

    async fn card_process_kill(
        &self,
        _request: Request,
        _card_id: String,
        _kill_token: String,
        _why: String,
    ) -> Result<Ack, Failure> {
        Err(denied("card_process_kill"))
    }

    async fn card_task_stop(&self, _request: Request, _event_id: i64) -> Result<Ack, Failure> {
        Err(denied("card_task_stop"))
    }

    // ADR-0091: a block's pick tells the agent the owner's words; a mentor is not named for it (ADR-0089)
    async fn block_pick(
        &self,
        _request: Request,
        _event_id: i64,
        _part: String,
        _picked: Vec<String>,
    ) -> Result<EventRead, Failure> {
        Err(denied("block_pick"))
    }

    async fn assistant_prompt(
        &self,
        _request: Request,
        _draft_id: String,
        _where: model::Where,
        _delivery: Delivery,
    ) -> Result<EventRead, Failure> {
        Err(denied("assistant_prompt"))
    }

    async fn routine_run(&self, _request: Request, _id: String) -> Result<model::Chain, Failure> {
        Err(denied("routine_run"))
    }

    async fn routine_pause(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("routine_pause"))
    }

    async fn routine_resume(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("routine_resume"))
    }

    async fn event_signal_pause(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("event_signal_pause"))
    }

    async fn event_signal_resume(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("event_signal_resume"))
    }

    async fn client_key_pause(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("client_key_pause"))
    }

    async fn client_key_resume(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("client_key_resume"))
    }

    async fn extension_disable(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("extension_disable"))
    }

    async fn extension_enable(&self, _request: Request, _id: String) -> Result<Ack, Failure> {
        Err(denied("extension_enable"))
    }

    async fn feedback_read(&self, _request: Request, _id: String) -> Result<Feedback, Failure> {
        Err(denied("feedback_read"))
    }

    async fn analyst_run(&self, _request: Request) -> Result<model::Chain, Failure> {
        Err(denied("analyst_run"))
    }

    async fn inbox_answer(
        &self,
        _request: Request,
        _item_id: String,
        _option_id: String,
        _text: String,
    ) -> Result<Ack, Failure> {
        Err(denied("inbox_answer"))
    }

    async fn overturn(&self, _request: Request, _decision_id: String, _word: String) -> Result<Ack, Failure> {
        Err(denied("overturn"))
    }

    async fn seen_mark(&self, _request: Request, _subject: SeenSubject) -> Result<Ack, Failure> {
        Err(denied("seen_mark"))
    }

    async fn workflow_stop(&self, _request: Request, _workflow_id: String, _why: String) -> Result<Ack, Failure> {
        Err(denied("workflow_stop"))
    }
}
