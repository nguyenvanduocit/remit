//! One agent on screen (the web's `AgentDrawer.vue`), the same wherever it shows: a chain's agent, the Assistant. Who it
//! is and where it stands in the server's words, Stop when the server lets the owner stop it, what waits on him with it
//! and the choices the server offers, its live stream, and his message to it with his sends on their way. What is the
//! agent's own kind of record (the Assistant's decisions) is a tab of its own beside the stream.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gui::prelude::*;
use gui::{Div, Icon, ScrollHandle, Signal, TextField, div, icon, text};
use model::{Card, EventKind, EventStatus, InboxItem, Origin};
use replica::{Ask, Model, Pending, PendingStatus};
use serde_json::{Value, json};
use shadcn::{
    BadgeVariant, ButtonSize, ButtonVariant, DialogState, Toaster, alert_dialog_action, alert_dialog_cancel,
    alert_dialog_content, alert_dialog_description, alert_dialog_footer, alert_dialog_header, alert_dialog_title,
    badge, button, collapsible, collapsible_content, collapsible_trigger, input, textarea, theme,
};
use wire::app::{AgentStanding, AgentTone, EventRead};

use crate::app::act;
use crate::composer::{Composer, DraftBox};
use crate::store::Store;

/// The call that delivers a draft of the box by its id: `card_prompt` for a chain's agent, `assistant_prompt` for the
/// Assistant.
pub type Deliver = Rc<dyn Fn(&str) -> Value>;

/// What the pane shows of one agent: its card, the name the owner reads, where it stands, and where its stream and its
/// drafts are kept.
pub struct Seat<'a> {
    pub card: &'a Card,
    pub name: String,
    pub standing: Option<&'a AgentStanding>,
    pub events: Model,
    pub deliver: Deliver,
}

/// The pane's two tabs, for an agent with decisions of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Activity,
    Decisions,
}

pub struct Pane {
    pub tab: Signal<Tab>,
    composer: Composer,
    stop: DialogState,
    answer: TextField,
    events: ScrollHandle,
    /// Which folded parts of the stream the owner opened, by event.
    folds: RefCell<HashMap<i64, Signal<bool>>>,
    /// The card shown: another one starts at its newest event, every part folded.
    shown: RefCell<Option<String>>,
    toaster: Toaster,
}

impl Pane {
    pub fn new(salt: String, toaster: Toaster) -> Rc<Self> {
        let events = ScrollHandle::new();
        // The stream sticks to its newest event unless the owner scrolls up to read; back at the bottom, it sticks again.
        let stuck = Rc::new(Cell::new(true));
        let handle = events.clone();
        events.on_layout(move |l| {
            if handle.take_wheeled() {
                stuck.set(l.offset.1 >= l.max.1 - 4.0);
            }
            let away = (l.max.1 - l.offset.1).abs() > 0.5;
            if stuck.get() && away {
                handle.scroll_to((l.offset.0, l.max.1));
            }
            stuck.get() && away
        });
        Rc::new(Self {
            tab: Signal::labeled("agent tab", Tab::Activity),
            composer: Composer::new(salt),
            stop: DialogState::new(),
            answer: TextField::new(""),
            events,
            folds: RefCell::new(HashMap::new()),
            shown: RefCell::new(None),
            toaster,
        })
    }

    /// Shows agent `card`, whose drafts its `home` keeps in `drafts`: its box opens on them.
    pub fn show(&self, store: &Store, home: String, card: String, drafts: Model) {
        let before = self.shown.borrow_mut().replace(card.clone());
        if before.as_ref() != Some(&card) {
            self.folds.borrow_mut().clear();
            self.events.scroll_to((0.0, f32::MAX));
            self.tab.set(Tab::Activity);
        }
        self.composer.open(store, DraftBox { home, card, drafts });
    }

    /// No agent shown: what its box holds stays with the server and the core.
    pub fn close(&self) {
        self.shown.borrow_mut().take();
        self.composer.close();
    }

    /// Whether the folded part of event `id` is open: closed until the owner opens it.
    fn fold(&self, id: i64) -> Signal<bool> {
        self.folds
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| Signal::labeled("fold", false))
            .clone()
    }
}

/// The pane for `seat`: `actions` beside its name, and `decisions` (how many are new, and the tab's body) for an agent
/// with decisions of its own.
pub fn view(
    pane: &Rc<Pane>,
    store: &Store,
    seat: &Seat<'_>,
    actions: Div,
    decisions: Option<(i64, Box<dyn FnOnce() -> Div + '_>)>,
) -> Div {
    let t = theme();
    let a = seat.standing;
    let waiting: Vec<InboxItem> = a.map_or(Vec::new(), |a| {
        let inbox = store.records::<InboxItem>(&Model::Inbox);
        a.waiting
            .iter()
            .filter_map(|id| inbox.iter().find(|i| &i.id == id).cloned())
            .collect()
    });
    let stoppable = a.is_some_and(|a| a.asks.contains(&wire::app::Ask::Stop) && !a.stop_says.is_empty());
    let tab = if decisions.is_some() {
        pane.tab.get()
    } else {
        Tab::Activity
    };
    let (new, decided) = decisions.map_or((0, None), |(new, body)| (new, Some(body)));
    let has_decisions = decided.is_some();
    let body = match (tab, decided) {
        (Tab::Decisions, Some(body)) => div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(0.0)
            .px(5.0)
            .py(3.0)
            .overflow_y_scroll()
            .child(body()),
        _ => div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(0.0)
            .children(waiting.iter().map(|i| wait(pane, store, i)))
            .child(stream(pane, store, seat))
            .child(composer(pane, store, seat)),
    };
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .min_h(0.0)
        .child(
            div()
                .flex()
                .shrink_0()
                .items_center()
                .gap(2.0)
                .px(5.0)
                .py(3.0)
                .border_b()
                .border_color(t.border)
                .child(text(seat.name.clone()).text_sm().font_semibold())
                .when_some(a, |d, a| d.child(badge(tone(a.tone)).child(a.label.clone())))
                .when_some(a.map(doing).filter(|d| !d.is_empty()), |d, doing| {
                    d.child(
                        text(doing)
                            .text_xs()
                            .text_color(t.muted_foreground)
                            .min_w(0.0)
                            .truncate(),
                    )
                })
                .child(div().flex_1())
                .when(stoppable, |d| {
                    let stop = pane.stop.clone();
                    d.child(
                        button(ButtonVariant::Outline, ButtonSize::Sm)
                            .child(icon(Icon::OctagonX))
                            .child("Stop")
                            .on_click(move |_| stop.open()),
                    )
                })
                .child(actions),
        )
        .when(has_decisions, |d| d.child(tabs(pane, tab, new)))
        .child(body)
        .when_some(a.filter(|_| stoppable), |d, a| {
            d.child(stop_dialog(pane, store, seat, a))
        })
}

/// Activity and Decisions, Decisions counting what is new in it.
fn tabs(pane: &Rc<Pane>, on: Tab, new: i64) -> Div {
    let t = theme();
    let entry = |tab: Tab, label: &'static str, count: i64| {
        let target = pane.tab.clone();
        let variant = if tab == on {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        };
        button(variant, ButtonSize::Sm)
            .role(gui::Role::Tab)
            .aria_selected(tab == on)
            .child(label)
            .when(count > 0, |b| {
                b.child(badge(BadgeVariant::Default).child(count.to_string()))
            })
            .on_click(move |_| target.set(tab))
    };
    div()
        .flex()
        .shrink_0()
        .gap(1.0)
        .px(4.0)
        .py(1.5)
        .border_b()
        .border_color(t.border)
        .role(gui::Role::TabList)
        .aria_label("Agent views")
        .child(entry(Tab::Activity, "Activity", 0))
        .child(entry(Tab::Decisions, "Decisions", new))
}

/// The agent's stream, oldest first, kept at its newest.
fn stream(pane: &Rc<Pane>, store: &Store, seat: &Seat<'_>) -> Div {
    let t = theme();
    let events = store.records::<EventRead>(&seat.events);
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(0.0)
        .gap(3.0)
        .px(5.0)
        .py(4.0)
        .overflow_y_scroll()
        .track_scroll(&pane.events)
        .role(gui::Role::Log)
        .aria_label(format!("{}'s activity", seat.name))
        .when(events.is_empty(), |d| {
            d.child(text("Nothing yet.").text_sm().text_color(t.muted_foreground))
        })
        .children(events.iter().map(|e| event(pane, e)))
}

/// Something only the owner settles that waits with this agent: its question and the choices the server offers.
fn wait(pane: &Rc<Pane>, store: &Store, i: &InboxItem) -> Div {
    let t = theme();
    let typed = matches!(i.kind, model::InboxKind::Credential | model::InboxKind::Login);
    let answers = div().flex().flex_wrap().gap(2.0);
    let answers = if typed {
        // A credential or a sign-in code is typed into a write-only field, which the web holds.
        answers.child(
            text("Type it in the web app: its field is write-only.")
                .text_xs()
                .text_color(t.muted_foreground),
        )
    } else if i.options.is_empty() {
        let (pane_, store_, id) = (Rc::clone(pane), store.clone(), i.id.clone());
        answers
            .child(input(&pane.answer).placeholder("Your answer").aria_label("Your answer").min_w_px(240.0))
            .child(button(ButtonVariant::Default, ButtonSize::Sm).child("Answer").on_click(move |_| {
                let words = pane_.answer.text();
                if words.trim().is_empty() {
                    return;
                }
                pane_.answer.set_text("");
                act(
                    &pane_.toaster,
                    &store_,
                    json!({ "method": "inbox_answer", "args": { "itemId": id, "optionId": "", "text": words } }),
                    "Answered",
                );
            }))
    } else {
        answers.children(i.options.iter().map(|o| {
            let (pane, store, item_id, option) = (Rc::clone(pane), store.clone(), i.id.clone(), o.id.clone());
            let variant = if o.kind.starts_with("reject") {
                ButtonVariant::Outline
            } else {
                ButtonVariant::Default
            };
            button(variant, ButtonSize::Sm).child(o.name.clone()).on_click(move |_| {
                act(
                    &pane.toaster,
                    &store,
                    json!({ "method": "inbox_answer", "args": { "itemId": item_id, "optionId": option, "text": "" } }),
                    "Answered",
                );
            })
        }))
    };
    div()
        .flex()
        .flex_col()
        .shrink_0()
        .gap(2.0)
        .mx(5.0)
        .mt(3.0)
        .p(3.0)
        .rounded_lg()
        .bg(t.muted)
        .role(gui::Role::Alert)
        .child(
            div()
                .flex()
                .items_center()
                .gap(2.0)
                .child(icon(Icon::CircleAlert))
                .child(text(i.title.clone()).text_sm().font_medium().min_w(0.0)),
        )
        .when(!i.detail.is_empty(), |d| {
            d.child(
                text(i.detail.clone())
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .line_clamp(6),
            )
        })
        .child(answers)
}

/// One entry of an agent's stream, as it reads: his messages and the agent's words whole, a message from another or its
/// thinking folded under its first line, a tool call by its title and how it went, the end of a turn as a rule.
fn event(pane: &Pane, read: &EventRead) -> Div {
    let t = theme();
    let e = &read.event;
    let row = div().key(gui::key_of(&e.id.to_string())).flex().flex_col().gap(1.0);
    let owners = e.by.as_ref().is_some_and(|by| by.kind == model::ActorKind::Owner);
    match e.kind() {
        EventKind::Prompt if !owners => row.child(fold(pane, e.id, format!("From {}", who(e)), &e.text)),
        EventKind::Thought => row.child(fold(pane, e.id, "Thought".to_owned(), &e.text)),
        EventKind::Prompt => row
            .self_end()
            .max_w_px(640.0)
            .p(3.0)
            .rounded_lg()
            .bg(t.muted)
            .child(text(said_by(e)).text_xs().font_medium().text_color(t.muted_foreground))
            .child(text(e.text.clone()).text_sm()),
        EventKind::Message => row.child(text(e.text.clone()).text_sm()),
        EventKind::Tool | EventKind::Job | EventKind::Task => row.child(
            div()
                .flex()
                .items_center()
                .gap(2.0)
                .text_xs()
                .text_color(t.muted_foreground)
                .child(icon(Icon::SquareTerminal))
                .child(text(tool_title(e)).min_w(0.0).truncate())
                .child(div().ml_auto().child(badge(ran_tone(e.status)).child(ran(e.status)))),
        ),
        EventKind::Stop => row.child(
            div()
                .flex()
                .items_center()
                .gap(2.0)
                .text_xs()
                .text_color(t.muted_foreground)
                .child(div().h_px(1.0).flex_1().bg(t.border))
                .child("Turn ended")
                .child(div().h_px(1.0).flex_1().bg(t.border)),
        ),
        EventKind::System => row.child(text(e.text.clone()).text_xs().text_color(t.muted_foreground)),
    }
}

/// How a tool call, job or task went, in a word.
fn ran(status: EventStatus) -> &'static str {
    use model::{JobStatus as J, TaskStatus as T, ToolStatus as U};
    match status {
        EventStatus::Tool(U::Pending) => "waiting",
        EventStatus::Tool(U::InProgress) | EventStatus::Job(J::Running) | EventStatus::Task(T::Running) => "running",
        EventStatus::Tool(U::Failed) | EventStatus::Job(J::Failed) | EventStatus::Task(T::Failed) => "failed",
        EventStatus::Task(T::Stopped) => "stopped",
        _ => "done",
    }
}

fn ran_tone(status: EventStatus) -> BadgeVariant {
    if ran(status) == "failed" {
        BadgeVariant::Destructive
    } else {
        BadgeVariant::Outline
    }
}

/// A part of the stream folded under one line, `what` and its first words; a click or the keyboard opens it whole.
fn fold(pane: &Pane, id: i64, what: String, words: &str) -> Div {
    let t = theme();
    let open = pane.fold(id);
    let first = words
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .to_owned();
    collapsible()
        .flex()
        .flex_col()
        .gap(1.0)
        .text_xs()
        .text_color(t.muted_foreground)
        .child(
            collapsible_trigger(&open)
                .flex()
                .items_center()
                .gap(2.0)
                .role(gui::Role::Button)
                .aria_label(format!("{what}: {first}"))
                .aria_expanded(open.get())
                .child(icon(if open.get() {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                }))
                .child(text(what).font_medium().shrink_0())
                .when(!open.get(), |d| d.child(text(first).min_w(0.0).truncate())),
        )
        .when_some(collapsible_content(&open), |d, c| {
            d.child(
                c.pl(6.0)
                    .child(text(words.to_owned()).text_sm().text_color(t.foreground)),
            )
        })
}

/// Who sent the owner's message, and where it is when the agent has not read it yet.
fn said_by(e: &model::Event) -> String {
    match e.status {
        EventStatus::Prompt(model::PromptStatus::Queued) => format!("{} · waits for the turn to end", who(e)),
        EventStatus::Prompt(model::PromptStatus::Dropped) => format!("{} · dropped", who(e)),
        _ => who(e),
    }
}

/// Who sent a prompt: the owner, another agent, or Remit.
fn who(e: &model::Event) -> String {
    match &e.by {
        Some(by) if by.kind == model::ActorKind::Owner => "You".to_owned(),
        Some(by) if !by.who.is_empty() => by.who.clone(),
        _ => "Remit".to_owned(),
    }
}

fn tool_title(e: &model::Event) -> String {
    match (e.tool_name.is_empty(), e.title.is_empty()) {
        (false, false) => format!("{}: {}", e.tool_name, e.title),
        (true, false) => e.title.clone(),
        _ => e.tool_name.clone(),
    }
}

/// The owner's message to the agent, and his sends on their way under it.
fn composer(pane: &Rc<Pane>, store: &Store, seat: &Seat<'_>) -> Div {
    let t = theme();
    let pending = pane.composer.pending(store);
    let send = {
        let (pane, store, deliver) = (Rc::clone(pane), store.clone(), Rc::clone(&seat.deliver));
        move || pane.composer.send(&store, &*deliver)
    };
    let on_key = send.clone();
    div()
        .flex()
        .flex_col()
        .shrink_0()
        .gap(2.0)
        .px(5.0)
        .py(3.0)
        .border_t()
        .border_color(t.border)
        .children(pending.iter().map(|p| pending_row(pane, store, p)))
        .child(
            div()
                .flex()
                .items_end()
                .gap(2.0)
                .child(
                    textarea(&pane.composer.field)
                        .flex_1()
                        .placeholder(format!("Message {}… (⌘↩ sends)", seat.name))
                        .aria_label(format!("Message to {}", seat.name))
                        .on_key_down(move |k| {
                            let sends = k.key == gui::Key::Named(gui::NamedKey::Enter) && k.modifiers.primary();
                            if sends {
                                on_key();
                            }
                            sends
                        }),
                )
                .child(
                    button(ButtonVariant::Default, ButtonSize::Default)
                        .child(icon(Icon::Send))
                        .child("Send")
                        .on_click(move |_| send()),
                ),
        )
}

/// A send of the owner's on its way: its words, and where it is; one that failed says why, to try again or take back.
fn pending_row(pane: &Rc<Pane>, store: &Store, p: &Pending) -> Div {
    let t = theme();
    let status = match p.status {
        PendingStatus::Waiting => "Waits for the connection".to_owned(),
        PendingStatus::Going => "Sending…".to_owned(),
        PendingStatus::Slow => "Still sending…".to_owned(),
        PendingStatus::Sent => "Sent".to_owned(),
        PendingStatus::Failed => format!("Not sent: {}", p.why),
    };
    let failed = p.status == PendingStatus::Failed;
    let resend = (store.clone(), p.id.clone());
    let unsend = (Rc::clone(pane), store.clone(), p.clone());
    div()
        .flex()
        .items_center()
        .gap(2.0)
        .text_xs()
        .child(text(p.text.clone()).min_w(0.0).truncate())
        .child(
            text(status)
                .ml_auto()
                .text_color(if failed { t.destructive } else { t.muted_foreground }),
        )
        .when(failed, |d| {
            d.child(
                button(ButtonVariant::Outline, ButtonSize::Xs)
                    .child("Try again")
                    .on_click(move |_| resend.0.ask(Ask::Resend { id: resend.1.clone() })),
            )
            .child(
                button(ButtonVariant::Ghost, ButtonSize::Xs)
                    .child("Edit")
                    .on_click(move |_| unsend.0.composer.unsend(&unsend.1, &unsend.2)),
            )
        })
}

/// Stopping an agent asks first, saying in the server's words what stopping it does.
fn stop_dialog(pane: &Rc<Pane>, store: &Store, seat: &Seat<'_>, a: &AgentStanding) -> gui::Element {
    let (pane_, store, id, says, who) = (
        Rc::clone(pane),
        store.clone(),
        seat.card.id.clone(),
        a.stop_says.clone(),
        seat.name.clone(),
    );
    alert_dialog_content(&pane.stop, move |d| {
        d.child(
            alert_dialog_header()
                .child(alert_dialog_title(format!("Stop {who}?")))
                .child(alert_dialog_description(says)),
        )
        .child(
            alert_dialog_footer()
                .child(alert_dialog_cancel(&pane_.stop).child("Cancel"))
                .child({
                    let pane = Rc::clone(&pane_);
                    alert_dialog_action(&pane_.stop, move || {
                        act(
                            &pane.toaster,
                            &store,
                            json!({ "method": "card_stop", "args": { "cardId": id, "why": "" } }),
                            "Agent stopped",
                        );
                    })
                    .child("Stop")
                }),
        )
    })
}

/// An agent's name as the owner reads it: the Chair as such, any other by its own.
pub fn name(card: &Card) -> String {
    if card.origin == Origin::Chair {
        "Chair".to_owned()
    } else {
        card.name.clone()
    }
}

/// What an agent does, as its line reads: its work, then what it does now.
pub fn doing(a: &AgentStanding) -> String {
    match (a.work.is_empty(), a.doing.is_empty()) {
        (false, false) => format!("{} · {}", a.work, a.doing),
        (false, true) => a.work.clone(),
        _ => a.doing.clone(),
    }
}

pub fn tone(tone: AgentTone) -> BadgeVariant {
    match tone {
        AgentTone::Needs | AgentTone::Failed => BadgeVariant::Destructive,
        AgentTone::Running => BadgeVariant::Default,
        AgentTone::Done => BadgeVariant::Secondary,
        AgentTone::Waiting | AgentTone::Idle => BadgeVariant::Outline,
    }
}
