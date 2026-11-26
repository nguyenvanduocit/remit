//! `Api` answers what the owner's session asks: `wire::app::Handler`, over the orchestrator's application layer
//! (`orch::request`). Each call is turned into the orchestrator's types, asked of it as the owner the session signed in,
//! and its answer turned back. Every decision is the orchestrator's, so another channel calling it behaves the same.

use std::sync::Arc;

use link::{ErrorCode, Failure, Request};
use model::{Actor, ChangeKind};
use orch::Orchestrator;
use tokio_util::sync::CancellationToken;

use crate::broadcast::Showing;
use crate::convert::{done, failed, returned};
use crate::link_report::{FaultBudget, Sender};
use model::{Change, Feedback, Help, OutcomeRange, OutcomeWindow};
use wire::Ack;
use wire::app::{
    CardContext, CardTelemetry, ChainPage, ChainView, ClientKeyValue, Delivery, DraftInput, EventPage, EventRead,
    EventRecord, EventSignalKey, Handler, Health, Judgments, LandingDiff, LinkReport, MemoryBook, NewChain, Outcomes,
    RepoView, SeenSubject, SignalDetail, SignalRow, SkillFileText, Snapshot, TaskOutput, TurnMark, WorkDuels,
    WorkerKeyValue,
};

/// The orchestrator, asked by one signed-in owner: what one session's calls are answered by.
#[derive(Clone)]
pub struct Api {
    o: Arc<Orchestrator>,
    /// Who the session signed in: everything it asks is asked by him.
    by: Actor,
    /// The homes the session shows, which what streams reaches.
    showing: Showing,
    /// The faults its pages may still send this minute.
    faults: Arc<FaultBudget>,
}

impl Api {
    pub(crate) fn new(o: Arc<Orchestrator>, by: Actor, showing: Showing) -> Self {
        Self {
            o,
            by,
            showing,
            faults: Arc::default(),
        }
    }

    /// The channel's action as the orchestrator takes it: its id, by the owner the session signed in.
    fn request(&self, r: Request) -> orch::Request {
        orch::Request {
            id: r.id,
            by: self.by.clone(),
        }
    }
}

impl Handler for Api {
    async fn app_snapshot(&self) -> Result<Snapshot, Failure> {
        returned("app_snapshot", self.o.snapshot().await)
    }

    async fn health_check(&self) -> Result<Health, Failure> {
        // The probe runs to its end: nothing a channel does cuts it short.
        let h = self.o.check_agents(&CancellationToken::new()).await;
        Ok(h)
    }

    async fn chain_get(&self, id: String) -> Result<ChainView, Failure> {
        returned("chain_get", self.o.view(id).await)
    }

    async fn chain_events(&self, id: String) -> Result<EventPage, Failure> {
        returned("chain_events", self.o.chain_events(id).await)
    }

    async fn homes_shown(&self, homes: Vec<String>, seq: u64) -> Result<Ack, Failure> {
        self.showing.set(homes, seq);
        Ok(Ack {})
    }

    async fn link_report(&self, report: LinkReport) -> Result<Ack, Failure> {
        // the owner may name any chain there is
        crate::link_report::keep(&self.o, Sender::Owner, report, &self.faults).await
    }

    async fn attachment_put(&self, id: String, home: String, attachment: model::Attachment) -> Result<Ack, Failure> {
        done("attachment_put", self.o.core.put_attachment(id, home, attachment).await)
    }

    async fn attachment_get(&self, id: String) -> Result<model::Attachment, Failure> {
        returned("attachment_get", self.o.core.attachment(id).await)
    }

    async fn chains_archived(&self, after: String) -> Result<ChainPage, Failure> {
        returned("chains_archived", self.o.archived_chains(after).await)
    }

    async fn event_get(&self, id: String) -> Result<EventRecord, Failure> {
        returned("event_get", self.o.pipeline.event_get(&id).await)
    }

    async fn signal_get(&self, address: String) -> Result<SignalDetail, Failure> {
        returned("signal_get", self.o.pipeline.signal_detail(&address).await)
    }

    async fn signal_rows(&self, pattern: String, limit: i64) -> Result<Vec<SignalRow>, Failure> {
        returned("signal_rows", self.o.pipeline.signal_rows(&pattern, limit).await)
    }

    async fn skill_file(&self, name: String, path: String) -> Result<SkillFileText, Failure> {
        returned("skill_file", self.o.capabilities.skill_file(&name, &path).await)
    }

    async fn changes_get(&self, kind: ChangeKind, name: String) -> Result<Vec<Change>, Failure> {
        returned("changes_get", self.o.capabilities.changes(kind, &name).await)
    }

    async fn card_events(&self, card_id: String, before: i64) -> Result<EventPage, Failure> {
        returned("card_events", self.o.card_events(card_id, before).await)
    }

    async fn card_context(&self, card_id: String) -> Result<CardContext, Failure> {
        returned("card_context", self.o.card_context(&card_id).await)
    }

    async fn card_processes(&self, card_id: String) -> Result<wire::app::CardProcesses, Failure> {
        returned("card_processes", self.o.card_processes(&card_id).await)
    }

    async fn card_telemetry(&self, card_id: String) -> Result<CardTelemetry, Failure> {
        returned("card_telemetry", self.o.card_telemetry(&card_id).await)
    }

    async fn card_task_output(&self, event_id: i64) -> Result<TaskOutput, Failure> {
        // wire::runtime::TaskOutput is the runtime's own, with no serde shape: its three fields cross by name.
        let out = self
            .o
            .task_output(event_id)
            .await
            .map_err(|e| failed("card_task_output", &e))?;
        Ok(TaskOutput {
            text: out.text,
            total_bytes: out.total_bytes,
            truncated: out.truncated,
        })
    }

    async fn landing_diff(&self, landing_id: String) -> Result<LandingDiff, Failure> {
        returned("landing_diff", self.o.shipping.landing_diff(&landing_id).await)
    }

    async fn repo_views(&self) -> Result<Vec<RepoView>, Failure> {
        self.o.repo_views().await.map_err(|e| failed("repo_views", &e))
    }

    async fn repo_get(&self, repo: String) -> Result<RepoView, Failure> {
        self.o.repo_view(repo).await.map_err(|e| failed("repo_get", &e))
    }

    async fn event_signal_key(&self, id: String) -> Result<EventSignalKey, Failure> {
        let key = self.o.pipeline.event_signal_key(&id, &self.by).await;
        Ok(EventSignalKey {
            key: key.map_err(|e| failed("event_signal_key", &e))?,
        })
    }

    async fn client_key(&self, id: String) -> Result<ClientKeyValue, Failure> {
        let key = self.o.keys.client_key(&id, &self.by).await;
        Ok(ClientKeyValue {
            key: key.map_err(|e| failed("client_key", &e))?,
        })
    }

    async fn worker_key(&self, id: String) -> Result<WorkerKeyValue, Failure> {
        let key = self.o.keys.worker_key(&id, &self.by).await;
        Ok(WorkerKeyValue {
            key: key.map_err(|e| failed("worker_key", &e))?,
        })
    }

    async fn outcomes_get(&self, last: OutcomeRange, window: OutcomeWindow) -> Result<Outcomes, Failure> {
        returned("outcomes_get", self.o.crew.outcomes(last, window).await)
    }

    async fn judgments_get(&self) -> Result<Judgments, Failure> {
        self.o.judgments().await.map_err(|e| failed("judgments_get", &e))
    }

    /// What an employee remembers, read when its page opens.
    async fn employee_memory(&self, id: String) -> Result<MemoryBook, Failure> {
        returned("employee_memory", self.o.crew.employee_book(&id).await)
    }

    /// What an agent remembers, read when its drawer's Memory panel opens.
    async fn card_memory(&self, card_id: String) -> Result<MemoryBook, Failure> {
        returned("card_memory", self.o.crew.card_book(&card_id).await)
    }

    /// What a mentor taught, read when their page opens.
    async fn mentor_memory(&self, id: String) -> Result<MemoryBook, Failure> {
        returned("mentor_memory", self.o.crew.mentor_book(&id).await)
    }

    /// Where lineages stand in every kind of work, read when the Crew page of duels opens.
    async fn duel_standings(&self) -> Result<Vec<WorkDuels>, Failure> {
        returned("duel_standings", self.o.crew.duel_standings().await)
    }

    async fn at_work(&self) -> Result<wire::app::AtWork, Failure> {
        returned("at_work", self.o.at_work().await)
    }

    async fn employee_runs(&self, id: String) -> Result<wire::app::EmployeeRuns, Failure> {
        returned("employee_runs", self.o.employee_runs(id).await)
    }

    async fn account_add(&self, request: Request) -> Result<model::InboxItem, Failure> {
        self.o
            .core
            .add_account(self.request(request))
            .await
            .map_err(|e| failed("account_add", &e))
    }

    async fn account_remove(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "account_remove",
            self.o.core.remove_account_by(self.request(request), id).await,
        )
    }

    async fn chain_create(&self, request: Request, input: NewChain) -> Result<model::Chain, Failure> {
        let input = orch::CreateChain {
            intent: input.intent,
            kind: model::ChainKind::Work,
            serves: None,
            project: input.project,
        };
        let made = self.o.create_chain(self.request(request), input).await;
        made.map_err(|e| failed("chain_create", &e))
    }

    async fn chain_pause(&self, request: Request, id: String, why: String) -> Result<Ack, Failure> {
        done("chain_pause", self.o.core.pause(self.request(request), id, why).await)
    }

    async fn chain_resume(&self, request: Request, id: String, why: String) -> Result<Ack, Failure> {
        done("chain_resume", self.o.core.resume(self.request(request), id, why).await)
    }

    async fn chain_check_back_now(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "chain_check_back_now",
            self.o.core.wake_now(self.request(request), id).await,
        )
    }

    async fn chain_finish(&self, request: Request, id: String, summary: String) -> Result<Ack, Failure> {
        done(
            "chain_finish",
            self.o.core.finish(self.request(request), id, summary).await,
        )
    }

    async fn chain_archive(&self, request: Request, id: String, why: String) -> Result<Ack, Failure> {
        done(
            "chain_archive",
            self.o.core.archive(self.request(request), id, why).await,
        )
    }

    async fn chain_unarchive(&self, request: Request, id: String, why: String) -> Result<Ack, Failure> {
        done(
            "chain_unarchive",
            self.o.core.unarchive(self.request(request), id, why).await,
        )
    }

    async fn chain_delete(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done("chain_delete", self.o.core.delete(self.request(request), id).await)
    }

    async fn card_prompt(&self, request: Request, draft_id: String, delivery: Delivery) -> Result<EventRead, Failure> {
        self.o
            .core
            .prompt(self.request(request), draft_id, delivery)
            .await
            .map(orch::event_read)
            .map_err(|e| failed("card_prompt", &e))
    }

    async fn card_prompt_withdraw(&self, request: Request, event_id: i64) -> Result<model::Draft, Failure> {
        self.o
            .core
            .withdraw_prompt(self.request(request), event_id)
            .await
            .map_err(|e| failed("card_prompt_withdraw", &e))
    }

    async fn card_cancel(&self, request: Request, card_id: String) -> Result<Ack, Failure> {
        done("card_cancel", self.o.core.cancel(self.request(request), card_id).await)
    }

    async fn card_restart(&self, request: Request, card_id: String) -> Result<Ack, Failure> {
        done(
            "card_restart",
            self.o.core.restart(self.request(request), card_id).await,
        )
    }

    async fn card_stop(&self, request: Request, card_id: String, why: String) -> Result<model::Card, Failure> {
        self.o
            .core
            .stop_card(self.request(request), card_id, why)
            .await
            .map_err(|e| failed("card_stop", &e))
    }

    async fn card_stop_explain(&self, request: Request, card_id: String, why: String) -> Result<Ack, Failure> {
        done(
            "card_stop_explain",
            self.o.core.explain_stop(self.request(request), card_id, why).await,
        )
    }

    async fn turn_teach(&self, request: Request, event_id: i64, mark: TurnMark, words: String) -> Result<Ack, Failure> {
        done(
            "turn_teach",
            self.o
                .crew
                .teach_turn(self.request(request), event_id, mark, words)
                .await,
        )
    }

    async fn card_process_kill(
        &self,
        request: Request,
        card_id: String,
        kill_token: String,
        why: String,
    ) -> Result<Ack, Failure> {
        done(
            "card_process_kill",
            self.o
                .core
                .kill_card_process(self.request(request), card_id, kill_token, why)
                .await,
        )
    }

    async fn card_task_stop(&self, request: Request, event_id: i64) -> Result<Ack, Failure> {
        done(
            "card_task_stop",
            self.o.core.stop_task(self.request(request), event_id).await,
        )
    }

    async fn block_pick(
        &self,
        request: Request,
        event_id: i64,
        part: String,
        picked: Vec<String>,
    ) -> Result<EventRead, Failure> {
        self.o
            .core
            .pick_in_block(self.request(request), event_id, part, picked)
            .await
            .map(orch::event_read)
            .map_err(|e| failed("block_pick", &e))
    }

    async fn assistant_runtime_options(&self) -> Result<wire::app::AssistantRuntimeOptions, Failure> {
        Ok(self.o.core.assistant_runtime_options())
    }

    async fn assistant_set_runtime(
        &self,
        request: Request,
        model: Option<String>,
        effort: Option<String>,
    ) -> Result<Ack, Failure> {
        done(
            "assistant_set_runtime",
            self.o
                .core
                .set_assistant_runtime(self.request(request), model, effort)
                .await,
        )
    }

    async fn assistant_prompt(
        &self,
        request: Request,
        draft_id: String,
        r#where: model::Where,
        delivery: Delivery,
    ) -> Result<EventRead, Failure> {
        self.o
            .core
            .prompt_assistant(self.request(request), draft_id, r#where, delivery)
            .await
            .map(orch::event_read)
            .map_err(|e| failed("assistant_prompt", &e))
    }

    async fn draft_save(&self, request: Request, draft: DraftInput, base_rev: i64) -> Result<model::Draft, Failure> {
        let d = model::Draft::from(draft);
        self.o
            .core
            .save_draft(self.request(request), d, base_rev)
            .await
            .map_err(|e| failed("draft_save", &e))
    }

    async fn draft_delete(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "draft_delete",
            self.o.core.delete_draft(self.request(request), id).await,
        )
    }

    async fn routine_run(&self, request: Request, id: String) -> Result<model::Chain, Failure> {
        let ran = self.o.pipeline.run_routine_now(self.request(request), id).await;
        ran.map_err(|e| failed("routine_run", &e))
    }

    async fn routine_pause(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "routine_pause",
            self.o.pipeline.pause_routine_now(self.request(request), id).await,
        )
    }

    async fn routine_resume(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "routine_resume",
            self.o.pipeline.resume_routine_now(self.request(request), id).await,
        )
    }

    async fn event_signal_pause(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "event_signal_pause",
            self.o.pipeline.pause_event_signal_now(self.request(request), id).await,
        )
    }

    async fn event_signal_resume(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "event_signal_resume",
            self.o.pipeline.resume_event_signal_now(self.request(request), id).await,
        )
    }

    async fn client_key_pause(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "client_key_pause",
            self.o.keys.pause_client_key(self.request(request), id).await,
        )
    }

    async fn client_key_resume(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "client_key_resume",
            self.o.keys.resume_client_key(self.request(request), id).await,
        )
    }

    async fn extension_disable(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "extension_disable",
            self.o.reach.disable_extension(self.request(request), id).await,
        )
    }

    async fn extension_enable(&self, request: Request, id: String) -> Result<Ack, Failure> {
        done(
            "extension_enable",
            self.o.reach.enable_extension(self.request(request), id).await,
        )
    }

    async fn feedback_read(&self, request: Request, id: String) -> Result<Feedback, Failure> {
        returned(
            "feedback_read",
            self.o.crew.read_feedback(self.request(request), &id).await,
        )
    }

    async fn analyst_run(&self, request: Request) -> Result<model::Chain, Failure> {
        let ran = self.o.crew.run_analyst_now(self.request(request)).await;
        ran.map_err(|e| failed("analyst_run", &e))
    }

    async fn inbox_answer(
        &self,
        request: Request,
        item_id: String,
        option_id: String,
        text: String,
    ) -> Result<Ack, Failure> {
        let answered = self.o.answer(self.request(request), item_id, option_id, text).await;
        done("inbox_answer", answered)
    }

    async fn overturn(&self, request: Request, decision_id: String, word: String) -> Result<Ack, Failure> {
        done(
            "overturn",
            self.o.overturn(self.request(request), decision_id, word).await,
        )
    }

    async fn seen_mark(&self, request: Request, subject: SeenSubject) -> Result<Ack, Failure> {
        done("seen_mark", self.o.seen_mark(self.request(request), subject).await)
    }

    async fn workflow_stop(&self, request: Request, workflow_id: String, why: String) -> Result<Ack, Failure> {
        done(
            "workflow_stop",
            self.o.core.stop_workflow(self.request(request), workflow_id, why).await,
        )
    }

    async fn help_accept(&self, _request: Request, _help_id: String) -> Result<Help, Failure> {
        Err(only_the_mentor("help_accept"))
    }

    async fn help_decline(&self, _request: Request, _help_id: String, _why: String) -> Result<Help, Failure> {
        Err(only_the_mentor("help_decline"))
    }
}

/// What the owner is told of an answer only the mentor asked gives.
fn only_the_mentor(method: &str) -> Failure {
    Failure::new(
        ErrorCode::Forbidden,
        format!("{method}: only the mentor it is offered to answers a help"),
    )
}
