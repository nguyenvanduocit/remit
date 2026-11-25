//! The Assistant (the web's `AssistantPanel.vue`), the owner's agent for the whole app, in a column beside whatever area is
//! open: its pane (`agent.rs`) like any agent's, its conversation asked for when the panel first opens, what he sends
//! carrying where he is, so "this one" means what he looks at. Its Decisions tab is its feed (ADR-0062): what it decided
//! for him, newest first, each answer he may overturn with his word, marked seen once he has looked at it (ADR-0079).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gui::prelude::*;
use gui::{Div, Effect, Icon, Signal, TextField, div, icon, text};
use model::{Assistant, Card, Decision};
use replica::{Ask, Model};
use serde_json::{Value, json};
use shadcn::{ButtonSize, ButtonVariant, Toaster, button, input, theme};
use time::UtcOffset;
use wire::app::{AgentStanding, AssistantFeed, FeedLine};

use crate::agent::{self, Deliver, Pane, Seat, Tab};
use crate::app::act;
use crate::core::now;
use crate::store::Store;
use crate::words;

/// How long the feed shows its new lines before they count as seen.
const SEEN_AFTER: Duration = Duration::from_secs(1);

pub struct AssistantPanel {
    pub open: Signal<bool>,
    pane: Rc<Pane>,
    /// The decision whose overturn the owner is writing, and his word.
    overturning: Signal<Option<String>>,
    word: TextField,
    /// Shows the Assistant in the pane once it is told; marks the feed seen once he has looked at it.
    effects: RefCell<Vec<Effect>>,
    toaster: Toaster,
}

impl AssistantPanel {
    pub fn new(salt: String, toaster: Toaster) -> Rc<Self> {
        Rc::new(Self {
            open: Signal::labeled("assistant open", false),
            pane: Pane::new(salt, toaster.clone()),
            overturning: Signal::labeled("overturning", None),
            word: TextField::new(""),
            effects: RefCell::new(Vec::new()),
            toaster,
        })
    }

    pub fn toggle(&self, store: &Store) {
        if self.open.get() {
            self.close();
        } else {
            self.show(store);
        }
    }

    /// Opens the panel: its conversation is asked for the first time, and its box opens on what the owner left in it.
    pub fn show(&self, store: &Store) {
        if !store.state().is_some_and(|s| s.assistant_loaded) {
            store.ask(Ask::LoadAssistant);
        }
        let following = {
            let (pane, store) = (Rc::clone(&self.pane), store.clone());
            Effect::new(move || {
                if let (Some(me), Some(card)) = (
                    store.one::<Assistant>(&Model::Assistant),
                    store.one::<Card>(&Model::AssistantCard),
                ) {
                    pane.show(&store, me.id, card.id, Model::AssistantDrafts);
                }
            })
        };
        let seen = {
            let (tab, store, marking) = (self.pane.tab.clone(), store.clone(), Rc::new(Cell::new(false)));
            Effect::new(move || {
                let unseen = store
                    .one::<AssistantFeed>(&Model::AssistantFeed)
                    .map_or(0, |f| f.unseen);
                if tab.get() != Tab::Decisions || unseen == 0 || marking.replace(true) {
                    return;
                }
                let (tab, store, marking) = (tab.clone(), store.clone(), Rc::clone(&marking));
                gui::clock::after(SEEN_AFTER, move || {
                    marking.set(false);
                    if tab.get() == Tab::Decisions {
                        store.call(
                            json!({ "method": "seen_mark", "args": { "subject": { "kind": "assistant_feed" } } }),
                            |_| {},
                        );
                    }
                });
            })
        };
        *self.effects.borrow_mut() = vec![following, seen];
        self.open.set(true);
    }

    /// Closes the panel; the Assistant keeps working.
    pub fn close(&self) {
        self.effects.borrow_mut().clear();
        self.pane.close();
        self.overturning.set(None);
        self.open.set(false);
    }
}

/// The panel, when open: `place` is where the owner is, carried by what he sends.
pub fn view(panel: &Rc<AssistantPanel>, store: &Store, place: Value, offset: UtcOffset) -> Option<Div> {
    if !panel.open.get() {
        return None;
    }
    let t = theme();
    let column = div()
        .flex()
        .flex_col()
        .w_px(520.0)
        .shrink_0()
        .min_h(0.0)
        .border_l()
        .border_color(t.border)
        .bg(t.background)
        .role(gui::Role::Region)
        .aria_label("Assistant")
        .on_key_down({
            let panel = Rc::clone(panel);
            move |k| {
                let esc = k.key == gui::Key::Named(gui::NamedKey::Escape);
                if esc {
                    panel.close();
                }
                esc
            }
        });
    let (Some(card), Some(standing)) = (
        store.one::<Card>(&Model::AssistantCard),
        store.one::<AgentStanding>(&Model::AssistantAgent),
    ) else {
        return Some(
            column.child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(t.muted_foreground)
                    .child("The Assistant is not here yet."),
            ),
        );
    };
    let feed = store.one::<AssistantFeed>(&Model::AssistantFeed);
    let new = feed.as_ref().map_or(0, |f| f.unseen);
    let deliver: Deliver = Rc::new(
        move |id| json!({ "method": "assistant_prompt", "args": { "draftId": id, "where": place, "delivery": "queue" } }),
    );
    let seat = Seat {
        card: &card,
        name: "Assistant".to_owned(),
        standing: Some(&standing),
        events: Model::AssistantEvents,
        deliver,
    };
    let decisions: Box<dyn FnOnce() -> Div + '_> = Box::new(|| decided(panel, store, feed.as_ref(), offset));
    Some(column.child(agent::view(
        &panel.pane,
        store,
        &seat,
        actions(panel, store, &card, &standing),
        Some((new, decisions)),
    )))
}

/// Start over, when the server says it can be asked, and Close.
fn actions(panel: &Rc<AssistantPanel>, store: &Store, card: &Card, standing: &AgentStanding) -> Div {
    let restart = standing.asks.contains(&wire::app::Ask::Restart);
    let (panel_, store, id) = (Rc::clone(panel), store.clone(), card.id.clone());
    let close = Rc::clone(panel);
    div()
        .flex()
        .items_center()
        .gap(1.0)
        .when(restart, |d| {
            d.child(
                button(ButtonVariant::Outline, ButtonSize::Sm)
                    .child("Start over")
                    .on_click(move |_| {
                        act(
                            &panel_.toaster,
                            &store,
                            json!({ "method": "card_restart", "args": { "cardId": id } }),
                            "Started over",
                        );
                    }),
            )
        })
        .child(
            button(ButtonVariant::Ghost, ButtonSize::IconSm)
                .aria_label("Close the Assistant")
                .child(icon(Icon::X))
                .on_click(move |_| close.close()),
        )
}

/// What the Assistant decided for the owner, newest first as the server lays it out.
fn decided(panel: &Rc<AssistantPanel>, store: &Store, feed: Option<&AssistantFeed>, offset: UtcOffset) -> Div {
    let t = theme();
    let lines = feed.map_or(&[][..], |f| &f.lines[..]);
    if lines.is_empty() {
        return div().text_sm().text_color(t.muted_foreground).py(6.0).child(text(
            "The Assistant has decided nothing for you yet. What it answers for you, or leaves to you, shows here.",
        ));
    }
    let at = now();
    div()
        .flex()
        .flex_col()
        .role(gui::Role::List)
        .aria_label("What the Assistant decided for you")
        .children(lines.iter().map(|l| line(panel, store, l, at, offset)))
}

/// One decision: what it decided or left to the owner, why and when; his overturn of it, or, while it stands, a second
/// judge's dissent from it, when it takes effect, and Overturn.
fn line(panel: &Rc<AssistantPanel>, store: &Store, l: &FeedLine, now: link::Millis, offset: UtcOffset) -> Div {
    let t = theme();
    let d = &l.decision;
    div()
        .key(gui::key_of(&d.id))
        .flex()
        .gap(2.0)
        .py(2.5)
        .border_b()
        .border_color(t.border)
        .text_sm()
        .child(
            div()
                .shrink_0()
                .pt(0.5)
                .text_color(t.muted_foreground)
                .child(icon(if l.answered { Icon::Sparkles } else { Icon::CircleHelp })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(0.0)
                .gap(1.0)
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(2.0)
                        .child(
                            text(d.text.clone())
                                .flex_1()
                                .min_w(0.0)
                                .text_color(if l.overturn.is_some() {
                                    t.muted_foreground
                                } else {
                                    t.foreground
                                }),
                        )
                        .child(ago(&d.created_at, now)),
                )
                .when(!d.why.is_empty(), |c| {
                    c.child(text(d.why.clone()).text_xs().text_color(t.muted_foreground))
                })
                .when_some(l.dissent.as_ref().filter(|_| l.overturn.is_none()), |c, dissent| {
                    c.child(note(
                        Icon::TriangleAlert,
                        format!("A second judge decided otherwise: {}", dissent.text),
                    ))
                })
                .when_some(l.overturn.as_ref(), |c, o| {
                    c.child(note(Icon::RotateCcw, format!("You overturned it: {}", o.why)))
                })
                .when(l.overturn.is_none() && l.answered, |c| {
                    c.when_some(l.takes_effect_at.and_then(|t| words::at(t.0)), |c, ms| {
                        c.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(1.5)
                                .text_xs()
                                .text_color(t.muted_foreground)
                                .child(icon(Icon::Clock))
                                .child(format!(
                                    "Takes effect at {}, unless you overturn it",
                                    words::clock(ms, offset)
                                )),
                        )
                    })
                    .child(overturn(panel, store, d))
                }),
        )
}

/// A note on a decision, its icon saying what kind it is beside its words.
fn note(mark: Icon, words: String) -> Div {
    let t = theme();
    div()
        .flex()
        .items_start()
        .gap(1.5)
        .mt(1.0)
        .p(2.0)
        .rounded_md()
        .bg(t.muted)
        .text_xs()
        .child(icon(mark))
        .child(text(words).min_w(0.0))
}

fn ago(at: &model::Time, now: link::Millis) -> Div {
    div()
        .shrink_0()
        .text_xs()
        .text_color(theme().muted_foreground)
        .child(words::span_since(at.0, now))
}

/// Overturning an answer the Assistant gave for the owner: a button that opens one box for what he decides instead,
/// sent as `overturn`; the line shows the overturn once the server pushes it, and what did not go stays in the box.
fn overturn(panel: &Rc<AssistantPanel>, store: &Store, d: &Decision) -> Div {
    let t = theme();
    if panel.overturning.get().as_deref() != Some(d.id.as_str()) {
        let (overturning, id) = (panel.overturning.clone(), d.id.clone());
        return div().mt(1.0).child(
            button(ButtonVariant::Outline, ButtonSize::Xs)
                .aria_label(format!("Overturn: {}", d.text))
                .child("Overturn")
                .on_click(move |_| overturning.set(Some(id.clone()))),
        );
    }
    let send = {
        let (panel, store, id) = (Rc::clone(panel), store.clone(), d.id.clone());
        move || {
            let word = panel.word.text();
            if word.trim().is_empty() {
                return;
            }
            let (overturning, field, toaster) = (panel.overturning.clone(), panel.word.clone(), panel.toaster.clone());
            store.call(
                json!({ "method": "overturn", "args": { "decisionId": id, "word": word.trim() } }),
                move |answer| match answer {
                    Ok(_) => {
                        field.set_text("");
                        overturning.set(None);
                    }
                    Err(f) => {
                        toaster.error(format!("Could not overturn: {}", f.message));
                    }
                },
            );
        }
    };
    let on_enter = send.clone();
    let cancel = panel.overturning.clone();
    div()
        .flex()
        .flex_col()
        .gap(1.5)
        .mt(2.0)
        .child(
            text("What do you decide instead?")
                .text_xs()
                .text_color(t.muted_foreground),
        )
        .child(
            div()
                .flex()
                .gap(1.5)
                .child(
                    input(&panel.word)
                        .flex_1()
                        .placeholder("In a few words")
                        .aria_label("What you decide instead")
                        .on_key_down(move |k| {
                            let enter = k.key == gui::Key::Named(gui::NamedKey::Enter);
                            if enter {
                                on_enter();
                            }
                            enter
                        }),
                )
                .child(
                    button(ButtonVariant::Default, ButtonSize::Sm)
                        .child("Overturn")
                        .on_click(move |_| send()),
                )
                .child(
                    button(ButtonVariant::Ghost, ButtonSize::Sm)
                        .child("Cancel")
                        .on_click(move |_| cancel.set(None)),
                ),
        )
}
