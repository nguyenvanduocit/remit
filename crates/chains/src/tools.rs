//! Every card in a chain, and the Assistant, reaches Remit through one MCP server, `CardServer`, whose tools say what that
//! card may do. `server_for` (card_tools.rs in orch) builds it from the card and its chain: each group adds its tools with
//! an `add_*_tools` method on a `Tools`, the box a server is made of, serving them through the service they act through
//! (`Tools::serving`, ADR-0096). A tool's input is a closed `#[serde(deny_unknown_fields)]` struct whose `schemars` schema is
//! what the agent is shown; arguments that do not fit are refused before the tool runs.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use model::{Card, Decision, DecisionKind, Edge, EdgeKind, InboxItem, InboxKind, InboxOption, InboxStatus, by_agent};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, ListToolsResult, MetaObject,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, SubscriptionFilter, Tool, ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer, SubscriptionContext, SubscriptionSink};
use rmcp::{ErrorData, ServerHandler};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::chain_core::{first_line, new_id};
use crate::inbox::put_asked;
use crate::outbox::new_message;
use crate::{Error, Result};
use model::GivenTool;
use store::Tx;

/// What a tool answers: its text, or why it refused.
pub type Answer = Pin<Box<dyn Future<Output = Result<String>> + Send>>;

/// A tool's handler: its arguments read into its input, then run through the service it was added with.
type Call = Box<dyn Fn(Option<Map<String, Value>>) -> Result<Answer, ErrorData> + Send + Sync>;

/// What a tool does to what Remit holds, which every tool declares as it is added: Claude Code reads it from the tool's
/// `readOnlyHint`, runs readers side by side and handles every other tool as a write. A hint to the runtime, never a rail:
/// what an agent may do stays decided by the tools it is given and the rails (rail.rs, policy.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Looks and changes nothing another agent or the owner sees.
    Reads,
    /// Changes something: a record, a session, a message sent, a system outside.
    Writes,
}

/// The tools an agent's loop calls turn after turn, whose schemas Claude Code keeps loaded when it defers the rest behind
/// ToolSearch (`anthropic/alwaysLoad`): an agent never searches for the tool its brief tells it to call every turn.
const ALWAYS_LOAD: [&str; 13] = [
    "accept_work",
    "answer_inbox",
    "answer_permission",
    "ask_chair",
    "ask_owner",
    "finish_chain",
    "hire_agent",
    "list_inbox",
    "message_agent",
    "record_verdict",
    "report",
    "route",
    "update_plan",
];

/// tool as the MCP server lists it: with the effect it declared, and loaded always when its loop calls it every turn.
fn declared(tool: Tool, effect: Effect) -> Tool {
    let tool = tool.with_annotations(ToolAnnotations::new().read_only(effect == Effect::Reads));
    if !ALWAYS_LOAD.contains(&tool.name.as_ref()) {
        return tool;
    }
    let mut meta = MetaObject::new();
    meta.0.insert("anthropic/alwaysLoad".to_owned(), Value::Bool(true));
    tool.with_meta(meta)
}

/// The tools one server gives, each once: adding a name again replaces the tool of that name.
#[derive(Default)]
pub struct Tools {
    tools: Vec<(Tool, Call)>,
}

impl Tools {
    /// The tools added through what it returns act through svc: each handler is given it when the tool is called. Each
    /// crate serves its tools through its own service (ADR-0096).
    pub fn serving<S: Send + Sync + 'static>(&mut self, svc: &Arc<S>) -> Serving<'_, S> {
        Serving {
            tools: self,
            svc: Arc::clone(svc),
        }
    }

    fn put(&mut self, tool: Tool, call: Call) {
        match self.tools.iter_mut().find(|(t, _)| t.name == tool.name) {
            Some(slot) => *slot = (tool, call),
            None => self.tools.push((tool, call)),
        }
    }

    /// The tools by name, as the MCP server lists them: sorted.
    pub fn listed(&self) -> Vec<Tool> {
        let mut out: Vec<Tool> = self.tools.iter().map(|(t, _)| t.clone()).collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// The tools as the owner reads what a card was given.
    pub fn given(&self) -> Vec<GivenTool> {
        self.listed()
            .into_iter()
            .map(|t| GivenTool {
                name: t.name.into_owned(),
                description: t.description.map(std::borrow::Cow::into_owned).unwrap_or_default(),
            })
            .collect()
    }

    /// Runs the tool name on args.
    pub async fn call(&self, name: &str, args: Option<Map<String, Value>>) -> Result<Result<String>, ErrorData> {
        let Some((_, call)) = self.tools.iter().find(|(t, _)| t.name == name) else {
            return Err(ErrorData::invalid_params(format!("unknown tool {name:?}"), None));
        };
        Ok(call(args)?.await)
    }
}

/// The tools being added to a `Tools`, each acting through the service S (`Tools::serving`).
pub struct Serving<'a, S> {
    tools: &'a mut Tools,
    svc: Arc<S>,
}

impl<S: Send + Sync + 'static> Serving<'_, S> {
    /// Adds the tool name, which has effect, described by description, whose input is In and whose handler is run.
    pub fn add<In, F, Fut>(&mut self, name: &'static str, effect: Effect, description: impl Into<String>, run: F)
    where
        In: JsonSchema + DeserializeOwned + Send + 'static,
        F: Fn(Arc<S>, In) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        let tool = declared(Tool::new(name, description.into(), schema_of::<In>()), effect);
        let svc = Arc::clone(&self.svc);
        let call: Call = Box::new(move |args| {
            let i: In = input(name, args)?;
            Ok(Box::pin(run(Arc::clone(&svc), i)))
        });
        self.tools.put(tool, call);
    }

    /// Adds a tool whose schema is not a Rust type's: a source's, which the source crate builds (sources.rs). Its handler
    /// takes the arguments as the agent wrote them, a JSON object.
    pub fn add_schema<F>(
        &mut self,
        name: String,
        effect: Effect,
        description: String,
        schema: Map<String, Value>,
        run: F,
    ) where
        F: Fn(Arc<S>, Value) -> Answer + Send + Sync + 'static,
    {
        let tool = declared(Tool::new(name, description, Arc::new(schema)), effect);
        let svc = Arc::clone(&self.svc);
        let call: Call = Box::new(move |args| Ok(run(Arc::clone(&svc), Value::Object(args.unwrap_or_default()))));
        self.tools.put(tool, call);
    }
}

/// What reads the card a tool acts as: read anew on each call, as update_plan and the like change it while
/// the session is open. A card's own tools read it by id; a seat's (subagents.rs) read the seat it works as.
#[derive(Clone)]
pub struct SelfCard(Arc<dyn Fn(&Core) -> CardRead + Send + Sync>);

/// A read of the card a tool acts as.
pub type CardRead = Pin<Box<dyn Future<Output = Result<Card>> + Send>>;

impl SelfCard {
    /// The card a seat works as: employee's newest child card in the process of card parent (subagents.rs's
    /// `seat_card`).
    pub fn of_seat(parent: &str, employee: &str) -> Self {
        let (parent, employee) = (parent.to_owned(), employee.to_owned());
        Self(Arc::new(move |o| {
            let card = o.seat_card(&parent, &employee);
            Box::pin(async move { card })
        }))
    }

    /// The card card_id, read from the store.
    pub fn of_id(card_id: &str) -> Self {
        let id = card_id.to_owned();
        Self(Arc::new(move |o| {
            let (id, store) = (id.clone(), Arc::clone(&o.store));
            Box::pin(async move { Ok(store.read(move |r| r.card(&id)).await?) })
        }))
    }

    /// The card read, as o has it now.
    pub async fn read(&self, o: &Core) -> Result<Card> {
        (self.0)(o).await
    }
}

/// A tool that takes no argument.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoInput {}

/// The open `subscriptions/listen` streams of agents' sessions, each with the card whose session opened it (a seat's is
/// its parent's). MCP 2026-07-28, which Claude Code speaks, has no sessions: this stream is the one way a server tells a
/// session that its tools changed, and Claude Code lists them again when told.
#[derive(Default)]
pub struct Listeners {
    open: Mutex<Vec<(u64, String, SubscriptionSink)>>,
    next: AtomicU64,
}

impl Listeners {
    /// Keeps sink, the stream card's session opened, until `close` is called with the number returned.
    fn open(&self, card_id: &str, sink: SubscriptionSink) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.lock().push((id, card_id.to_owned(), sink));
        id
    }

    fn close(&self, id: u64) {
        self.lock().retain(|(n, _, _)| *n != id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(u64, String, SubscriptionSink)>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells every open session of the cards card_ids that its tools changed, so it lists them again; the number told.
    pub async fn tools_changed(&self, card_ids: &[String]) -> usize {
        let sinks: Vec<(u64, SubscriptionSink)> = self
            .lock()
            .iter()
            .filter(|(_, card, _)| card_ids.contains(card))
            .map(|(id, _, sink)| (*id, sink.clone()))
            .collect();
        let mut told = 0_usize;
        for (id, sink) in sinks {
            match sink.notify_tool_list_changed().await {
                Ok(()) => told = told.saturating_add(1),
                Err(err) => {
                    tracing::debug!(%err, "a tool-list stream closed");
                    self.close(id);
                }
            }
        }
        told
    }
}

/// One card's MCP server, or one seat's in a card's session: the tools `server_for` (or subagents.rs's
/// `seat_server_for`) gave it.
pub struct CardServer {
    o: Arc<Core>,
    /// The card whose session is served (a seat's parent): told when its tools change (`Listeners`).
    card_id: String,
    tools: Arc<Tools>,
    /// The card a seat works for, by name: a call to a tool the seat is not served is that card's to make. None for a
    /// card's own server.
    seat_of: Option<String>,
}

impl CardServer {
    /// The server of the tools t, card card_id's own.
    pub fn of_card(o: &Arc<Core>, card_id: &str, t: Tools) -> Self {
        Self {
            o: Arc::clone(o),
            card_id: card_id.to_owned(),
            tools: Arc::new(t),
            seat_of: None,
        }
    }

    /// The server of the tools t, a seat's in the session of card card_id, named parent.
    pub fn of_seat(o: &Arc<Core>, card_id: &str, t: Tools, parent: &str) -> Self {
        Self {
            seat_of: Some(parent.to_owned()),
            ..Self::of_card(o, card_id, t)
        }
    }

    /// Runs the tool name on args. A seat asking for a tool it is not served is told whose it is: a subagent asks for
    /// its parent's acts in its report.
    pub async fn answer(&self, name: &str, args: Option<Map<String, Value>>) -> Result<Result<String>, ErrorData> {
        if let Some(parent) = &self.seat_of
            && !self.tools.listed().iter().any(|t| t.name == name)
        {
            return Ok(Err(Error::refused(format!(
                "{name} is not yours: a subagent works for {parent}: say it in your report."
            ))));
        }
        self.tools.call(name, args).await
    }
}

impl ServerHandler for CardServer {
    fn get_info(&self) -> ServerConfig {
        let name = if self.seat_of.is_some() {
            crate::subagents::SEAT_SERVER_NAME
        } else {
            MCP_SERVER_NAME
        };
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_server_info(Implementation::new(name, "0.1"))
    }

    fn accepted_subscription_filter(&self, requested: &SubscriptionFilter) -> Option<SubscriptionFilter> {
        Some(requested.clone()) // the SDK keeps of it what get_info advertises: the tool list
    }

    async fn listen(&self, ctx: SubscriptionContext) -> Result<(), ErrorData> {
        let id = self.o.listeners.open(&self.card_id, ctx.sink().clone());
        ctx.cancelled().await;
        self.o.listeners.close(id);
        Ok(())
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.listed()))
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let answer = self.answer(req.name.as_ref(), req.arguments).await?;
        Ok(match answer {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(err) => CallToolResult::error(vec![ContentBlock::text(err.to_string())]),
        }
        .into())
    }
}

/// The input of a tool about one agent.
#[derive(Debug, Default, serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CardIdInput {
    /// the agent's card id (c_…)
    pub card_id: String,
}

/// Whether a record's revision is none: a record a tool shows before it is stored, or one an agent creates.
pub fn is_zero_rev(rev: &i64) -> bool {
    *rev == 0
}

/// v as JSON indented by two spaces.
pub fn as_json(v: &impl Serialize) -> Result<String> {
    Ok(serde_json::to_string_pretty(v)?)
}

impl Core {
    /// Records that two cards of chain chain_id work together, once per pair: later messages between them are not edges.
    pub async fn add_edge(&self, chain_id: &str, from: &str, to: &str, kind: EdgeKind, label: &str) {
        let key = chain_id.to_owned();
        if let Ok(existing) = self.store.read(move |r| r.edges(&key)).await
            && existing
                .iter()
                .any(|e| (e.from == from && e.to == to) || (e.from == to && e.to == from))
        {
            return;
        }
        let e = Edge {
            id: new_id("e_"),
            chain_id: chain_id.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            kind,
            label: label.to_owned(),
            created_at: self.now(),
        };
        let put = e.clone();
        if let Err(err) = self.store.update(move |tx| tx.put_edge(&put)).await {
            tracing::error!(chain = %chain_id, %err, "edge");
            return;
        }
        self.notify.edge_added(&e);
    }

    /// The Chair chair passing work between the agents of its team, or answering one's question.
    pub async fn route(&self, chair: &Card, i: RouteInput) -> Result<()> {
        let answer = i.kind == ROUTE_ANSWER;
        let kind = if answer {
            EdgeKind::Question
        } else {
            i.kind
                .parse()
                .map_err(|e: model::UnknownValue| Error::refused(e.to_string()))?
        };
        if ![EdgeKind::Handoff, EdgeKind::Feedback, EdgeKind::Question].contains(&kind) {
            return Err(Error::refused(format!(
                "kind must be handoff, feedback, question or answer, not {:?}",
                i.kind
            )));
        }
        let to = self.team_card(&chair.home, &i.to_card).await?;
        let mut from_name = "the Chair".to_owned();
        if !i.from_card.is_empty() && i.from_card != chair.id {
            let from = self.team_card(&chair.home, &i.from_card).await?;
            from_name.clone_from(&from.name);
            let home = chair.home.clone();
            let decisions = self.store.read(move |r| r.decisions(&home)).await?;
            let label = relation_label(&kind, edge_cause(&decisions, &from.id));
            self.add_edge(&chair.home, &from.id, &to.id, kind, &label).await;
        }
        let text = if answer {
            format!(
                "[answer from the Chair · to your question · {}]\n{}",
                i.label, i.message
            )
        } else {
            format!(
                "[{kind} from the Chair · about {from_name}'s work · {}]\n{}",
                i.label, i.message
            )
        };
        self.deliver_now(&to.id, &route_title(&i.kind, &i.label), &text, by_agent(&chair.id))
            .await?;
        self.decide(
            &chair.home,
            DecisionKind::Route,
            &format!("{from_name} → {}: {} ({}).", to.name, i.label, i.kind),
            "",
            &to.id,
            by_agent(&chair.id),
        )
        .await;
        Ok(())
    }

    /// Puts the Chair's question to the owner. The Chair waits on one question at a time: a new one replaces what it asked
    /// before and he has not answered, so his answer to the latest leaves nothing open that he still owes.
    pub async fn ask_owner(&self, chair: &Card, i: AskInput) -> Result<InboxItem> {
        if i.question.trim().is_empty() {
            return Err(Error::refused("question is empty"));
        }
        let options = i
            .options
            .iter()
            .zip(1..)
            .map(|(name, n): (&String, i64)| InboxOption {
                id: format!("o{n}"),
                name: name.clone(),
                ..InboxOption::default()
            })
            .collect();
        let it = InboxItem {
            id: new_id("i_"),
            home: chair.home.clone(),
            card_id: chair.id.clone(),
            kind: InboxKind::Question,
            title: first_line(&i.question, 120),
            detail: format!("{}\n\n{}", i.question, i.why),
            options,
            status: InboxStatus::Open,
            created_at: self.now(),
            ..InboxItem::default()
        };
        let (door, chair_id) = (self.door(), chair.id.clone());
        // one transaction: the owner never sees the new question beside the ones it replaces
        let (it, cancelled) = self
            .store
            .update(move |tx| {
                let it = put_asked(tx, &door, it)?;
                let cancelled = cancel_questions(tx, &chair_id, &it.id, "the Chair asked again")?;
                Ok::<_, Error>((it, cancelled))
            })
            .await?;
        self.notify.inbox_updated(&it);
        for c in &cancelled {
            self.notify.inbox_updated(c);
        }
        self.kick(); // the Assistant is told on the Tick
        self.decide(
            &chair.home,
            DecisionKind::Ask,
            &format!("Ask you: {}", it.title),
            &i.why,
            &chair.id,
            by_agent(&chair.id),
        )
        .await;
        Ok(it)
    }

    /// Takes back every question chair asked the owner that he has not answered: what blocked it is cleared (he opened
    /// the access outside Remit) or it no longer matters. The questions leave his inbox, and the chain may finish.
    pub async fn withdraw_question(&self, chair: &Card, i: WithdrawInput) -> Result<usize> {
        if i.why.trim().is_empty() {
            return Err(Error::refused("why is empty"));
        }
        let (chair_id, why) = (chair.id.clone(), format!("the Chair withdrew it: {}", i.why));
        let cancelled = self
            .store
            .update(move |tx| cancel_questions(tx, &chair_id, "", &why))
            .await?;
        if cancelled.is_empty() {
            return Err(Error::refused("you have no open question"));
        }
        for c in &cancelled {
            self.notify.inbox_updated(c);
        }
        let titles: Vec<&str> = cancelled.iter().map(|it| it.title.as_str()).collect();
        self.decide(
            &chair.home,
            DecisionKind::Withdraw,
            &format!("Withdrew: {}", titles.join("; ")),
            &i.why,
            &chair.id,
            by_agent(&chair.id),
        )
        .await;
        Ok(titles.len())
    }

    /// A worker's question to its Chair mid-task, headed with the asking card's name.
    pub async fn ask_chair(&self, worker: &Card, question: &str) -> Result<()> {
        self.question_to_chair(worker, question, Records::default()).await
    }

    /// Sends worker's question to its Chair with w, in one change.
    pub async fn question_to_chair(&self, worker: &Card, question: &str, w: Records) -> Result<()> {
        let home = worker.home.clone();
        let chain = self.store.read(move |r| r.chain(&home)).await?;
        let chair_id = chain.chair_id.clone();
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        let head = format!("[question from {} · card {}]", worker.name, worker.id);
        let m = new_message(
            &chair.id,
            &format!("Question from {}", worker.name),
            &format!("{head}\n{question}"),
            by_agent(&worker.id),
        );
        self.send(&chair, m, w).await.map(|_| ())
    }
}

/// Closes, in tx, the open questions chair_id asked the owner, but keep, with why in the answer he reads, and gives them
/// as stored: each is read in tx, so an answer given meanwhile is never written over.
fn cancel_questions(tx: &Tx<'_>, chair_id: &str, keep: &str, why: &str) -> Result<Vec<InboxItem>> {
    let mut cancelled = Vec::new();
    for mut it in tx.open_inbox()? {
        if it.card_id != chair_id || it.kind != InboxKind::Question || it.id == keep {
            continue;
        }
        (it.status, it.answer) = (InboxStatus::Cancelled, why.to_owned());
        cancelled.push(tx.put_inbox(it)?);
    }
    Ok(cancelled)
}

/// Names, in the owner's words, the act an edge records between two agents (ADR-0082), and what caused it: the Chair's
/// verdict on the giver's work, when one was its last word on it. What was passed stays in the chain's events. Pure.
pub fn relation_label(kind: &EdgeKind, cause: Option<&DecisionKind>) -> String {
    let act = match kind {
        EdgeKind::Hire => "spawned",
        EdgeKind::Feedback => "sent back",
        EdgeKind::Question => "asked",
        _ => "hand-off",
    };
    match cause {
        Some(DecisionKind::Accept) => format!("{act} on accept"),
        Some(DecisionKind::Rework) => format!("{act} on rework"),
        _ => act.to_owned(),
    }
}

/// What caused an edge from card from: the chain's newest decision about it among decisions, when that is the Chair's
/// accept or rework of its work. Pure.
pub fn edge_cause<'a>(decisions: &'a [Decision], from: &str) -> Option<&'a DecisionKind> {
    decisions
        .iter()
        .filter(|d| d.card_id == from)
        .max_by_key(|d| d.created_at)
        .map(|d| &d.kind)
        .filter(|k| matches!(k, DecisionKind::Accept | DecisionKind::Rework))
}

/// The route kind of the Chair's answer to an agent's question (ask_chair).
pub const ROUTE_ANSWER: &str = "answer";

/// What a routed message's turn is about: the Chair's label, else what its kind asks of the agent. Pure.
pub fn route_title(kind: &str, label: &str) -> String {
    let label = label.trim();
    if !label.is_empty() {
        return label.to_owned();
    }
    match kind {
        "feedback" => "Address feedback",
        "question" => "Answer a question",
        ROUTE_ANSWER => "The Chair's answer",
        _ => "Receive results",
    }
    .to_owned()
}

/// The Chair's route of work between its agents.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RouteInput {
    /// card whose work is being passed on (its id), e.g. the reviewer whose findings go to the engineer; empty when the
    /// message is yours, the Chair's
    pub from_card: String,
    /// card that receives the message
    pub to_card: String,
    /// handoff (pass work on, or a new version to the reviewer or QC who judged the last one), feedback (another agent's
    /// findings on to_card's work, to fix) question (a question to the agent) or answer (your answer to the question it
    /// asked you)
    pub kind: String,
    /// short note of what is passed, e.g. 'findings.md' or 'SR-2 missing 4xx test' (shown in the heading of the message
    /// the agent receives)
    pub label: String,
    /// what the receiving agent should do with it
    pub message: String,
}

/// The Chair's question to the owner.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AskInput {
    /// the decision or the way past a block you need from the owner, with enough context to answer it alone
    pub question: String,
    /// suggested answers, recommended one first
    pub options: Vec<String>,
    /// why only the owner can decide or clear this
    pub why: String,
}

/// The Chair taking its questions back.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct WithdrawInput {
    /// what cleared the block, or why the question no longer matters, in one line the owner reads
    pub why: String,
}

/// The name Remit's MCP server goes by in every agent's session.
pub const MCP_SERVER_NAME: &str = "remit";

/// The JSON Schema of In as the holder reads it: no meta-schema, no title, no number formats, closed to other fields.
pub fn schema_of<In: JsonSchema>() -> serde_json::Map<String, Value> {
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|s| {
            s.meta_schema = None;
            s.inline_subschemas = true;
        })
        .into_generator();
    let mut schema = match generator.into_root_schema_for::<In>().to_value() {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    schema.remove("title");
    if let Some(Value::Object(props)) = schema.get_mut("properties") {
        for p in props.values_mut().filter_map(Value::as_object_mut) {
            p.remove("format");
            p.remove("default");
        }
    }
    schema
}

/// The arguments of tool as its input, or why they are not one: they are refused before the tool runs.
pub fn input<In: DeserializeOwned>(tool: &str, args: Option<serde_json::Map<String, Value>>) -> Result<In, ErrorData> {
    serde_json::from_value(Value::Object(args.unwrap_or_default()))
        .map_err(|e| ErrorData::invalid_params(format!("{tool}: the arguments are not the tool's input: {e}"), None))
}
