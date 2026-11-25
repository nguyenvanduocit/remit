//! A chain open on screen (ADR-0128 decision 5, the web's `ChainPage.vue`): the chain's title and where it stands in the
//! server's words, what the owner can do to it (pause, resume, as the server takes it: `ChainStanding.asks`), its agents
//! with where each stands, and the agent picked in its pane (`agent.rs`). Every rule is the server's; the window keeps
//! which chain and which agent are open.

use std::cell::RefCell;
use std::rc::Rc;

use gui::prelude::*;
use gui::{Div, Effect, Icon, Signal, TextField, div, icon, text};
use model::Card;
use replica::{Ask, Model};
use serde_json::json;
use shadcn::{
    ButtonSize, ButtonVariant, DialogState, ItemSize, ItemVariant, Toaster, alert_dialog_action, alert_dialog_cancel,
    alert_dialog_content, alert_dialog_description, alert_dialog_footer, alert_dialog_header, alert_dialog_title,
    badge, button, input, item, item_content, spinner, theme,
};
use wire::app::{AgentStanding, ChainAsk, ChainRead, ChainStanding};

use crate::agent::{self, Deliver, Pane, Seat, doing, name};
use crate::app::act;
use crate::board;
use crate::store::Store;

/// The chain open, and the agent picked in it (none: its Chair, once the chain is told).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub chain: String,
    pub card: Option<String>,
}

pub struct ChainScreen {
    pub open: Signal<Option<Open>>,
    pane: Rc<Pane>,
    pause: DialogState,
    why: TextField,
    /// The card whose stream the window reads.
    streamed: RefCell<Option<String>>,
    /// Picks the Chair once a chain opened with no agent picked is told.
    settle: RefCell<Option<Effect>>,
    toaster: Toaster,
}

impl ChainScreen {
    pub fn new(salt: String, toaster: Toaster) -> Rc<Self> {
        Rc::new(Self {
            open: Signal::labeled("open chain", None),
            pane: Pane::new(salt, toaster.clone()),
            pause: DialogState::new(),
            why: TextField::new(""),
            streamed: RefCell::new(None),
            settle: RefCell::new(None),
            toaster,
        })
    }

    /// Opens chain `id`: the core reads it and keeps it live; its Chair is picked once it is told.
    pub fn open_chain(self: &Rc<Self>, store: &Store, id: String) {
        store.ask(Ask::OpenChain { id: id.clone() });
        self.open.set(Some(Open { chain: id, card: None }));
        let (screen, store) = (Rc::clone(self), store.clone());
        *self.settle.borrow_mut() = Some(Effect::new(move || {
            let Some(open) = screen.open.get() else { return };
            if open.card.is_some() {
                return;
            }
            let Some(chain) = store.one::<ChainRead>(&Model::OpenChain) else {
                return;
            };
            if chain.chain.id == open.chain && !chain.chain.chair_id.is_empty() {
                screen.pick(&store, chain.chain.chair_id);
            }
        }));
    }

    /// Back to the board: the chain and its agent's stream are let go, so the core keeps them no longer.
    pub fn close(&self, store: &Store) {
        self.settle.borrow_mut().take();
        self.pane.close();
        if let Some(card) = self.streamed.borrow_mut().take() {
            store.hide(&Model::CardEvents(card));
        }
        for model in [
            Model::OpenChain,
            Model::Cards,
            Model::AgentStandings,
            Model::ChainDrafts,
        ] {
            store.hide(&model);
        }
        self.open.set(None);
    }

    /// Picks agent `card` of the open chain: its whole stream is read, and its pane shows it.
    pub fn pick(&self, store: &Store, card: String) {
        let Some(open) = self.open.get() else { return };
        let before = self.streamed.borrow_mut().replace(card.clone());
        if before.as_ref() != Some(&card) {
            if let Some(before) = before {
                store.hide(&Model::CardEvents(before));
            }
            store.ask(Ask::LoadStream { card: card.clone() });
        }
        self.pane
            .show(store, open.chain.clone(), card.clone(), Model::ChainDrafts);
        self.open.set(Some(Open {
            chain: open.chain,
            card: Some(card),
        }));
    }
}

pub fn view(screen: &Rc<ChainScreen>, store: &Store) -> Div {
    let Some(open) = screen.open.get() else {
        return div();
    };
    let chain = store
        .one::<ChainRead>(&Model::OpenChain)
        .filter(|c| c.chain.id == open.chain);
    let Some(chain) = chain else {
        return div()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap(2.0)
            .text_sm()
            .text_color(theme().muted_foreground)
            .child(spinner())
            .child("Opening the chain…");
    };
    let standing = store
        .records::<ChainStanding>(&Model::Standings)
        .into_iter()
        .find(|s| s.chain_id == open.chain);
    let cards = store.records::<Card>(&Model::Cards);
    let agents = store.records::<AgentStanding>(&Model::AgentStandings);
    let picked = open.card.as_ref().and_then(|id| cards.iter().find(|c| &c.id == id));
    let deliver: Deliver =
        Rc::new(|id| json!({ "method": "card_prompt", "args": { "draftId": id, "delivery": "queue" } }));
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h(0.0)
        .child(head(screen, store, &chain, standing.as_ref()))
        .child(
            div()
                .flex()
                .flex_1()
                .min_h(0.0)
                .child(agent_list(screen, store, &cards, &agents, open.card.as_deref()))
                .child(match picked {
                    Some(card) => agent::view(
                        &screen.pane,
                        store,
                        &Seat {
                            card,
                            name: name(card),
                            standing: agents.iter().find(|a| a.card_id == card.id),
                            events: Model::CardEvents(card.id.clone()),
                            deliver,
                        },
                        div(),
                        None,
                    ),
                    None => div().flex_1(),
                }),
        )
        .child(pause_dialog(screen, store, &chain.chain.id))
}

/// The chain's title, where it stands (its first wait, when one holds it up for the owner) and what he can do to it.
fn head(screen: &Rc<ChainScreen>, store: &Store, chain: &ChainRead, standing: Option<&ChainStanding>) -> Div {
    let t = theme();
    let back = {
        let (screen, store) = (Rc::clone(screen), store.clone());
        button(ButtonVariant::Ghost, ButtonSize::Sm)
            .child(icon(Icon::ArrowLeft))
            .child("Board")
            .on_click(move |_| screen.close(&store))
    };
    let state = standing.map(|s| {
        let word = s.wait.as_ref().map_or_else(
            || s.text.split(" · ").next().unwrap_or_default().to_owned(),
            |w| w.label.clone(),
        );
        badge(board::tone(Some(s.tone))).child(word)
    });
    let said = standing.and_then(|s| match &s.wait {
        Some(w) => Some(format!("{}: {}", w.label, w.ask)),
        None => s.text.split_once(" · ").map(|(_, rest)| rest.to_owned()),
    });
    let asks = standing.map_or(Vec::new(), |s| s.asks.clone());
    let id = chain.chain.id.clone();
    div()
        .flex()
        .shrink_0()
        .items_center()
        .gap(3.0)
        .px(4.0)
        .py(2.5)
        .border_b()
        .border_color(t.border)
        .child(back)
        .child(
            text(chain.chain.title.clone())
                .text_base()
                .font_semibold()
                .min_w(0.0)
                .truncate(),
        )
        .when_some(state, |d, b| d.child(b))
        .when_some(said, |d, said| {
            d.child(
                text(said)
                    .text_sm()
                    .text_color(t.muted_foreground)
                    .min_w(0.0)
                    .truncate(),
            )
        })
        .child(div().flex_1())
        .when(asks.contains(&ChainAsk::Pause), |d| {
            let pause = screen.pause.clone();
            d.child(
                button(ButtonVariant::Outline, ButtonSize::Sm)
                    .child("Pause")
                    .on_click(move |_| pause.open()),
            )
        })
        .when(asks.contains(&ChainAsk::Resume), |d| {
            let (screen, store) = (Rc::clone(screen), store.clone());
            d.child(
                button(ButtonVariant::Outline, ButtonSize::Sm)
                    .child("Resume")
                    .on_click(move |_| {
                        act(
                            &screen.toaster,
                            &store,
                            json!({ "method": "chain_resume", "args": { "id": id, "why": "" } }),
                            "Chain resumed",
                        );
                    }),
            )
        })
}

/// The chain's agents, each with where it stands in the server's word and what it does; a click picks one.
fn agent_list(
    screen: &Rc<ChainScreen>,
    store: &Store,
    cards: &[Card],
    agents: &[AgentStanding],
    picked: Option<&str>,
) -> Div {
    let t = theme();
    div()
        .flex()
        .flex_col()
        .w_px(300.0)
        .shrink_0()
        .gap(1.5)
        .p(3.0)
        .border_r()
        .border_color(t.border)
        .overflow_y_scroll()
        .role(gui::Role::List)
        .aria_label("Agents")
        .children(cards.iter().map(|card| {
            let a = agents.iter().find(|a| a.card_id == card.id);
            let on = picked == Some(card.id.as_str());
            let (screen, store, id) = (Rc::clone(screen), store.clone(), card.id.clone());
            item(ItemVariant::Outline, ItemSize::Sm)
                .key(gui::key_of(&card.id))
                .bg(if on { t.accent } else { t.background })
                .aria_selected(on)
                .on_click(move |_| screen.pick(&store, id.clone()))
                .child(
                    item_content()
                        .min_w(0.0)
                        .gap(1.0)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(2.0)
                                .child(text(name(card)).text_sm().font_medium().min_w(0.0).truncate())
                                .when_some(a, |d, a| {
                                    d.child(div().ml_auto().child(badge(agent::tone(a.tone)).child(a.label.clone())))
                                }),
                        )
                        .when_some(a.map(doing).filter(|d| !d.is_empty()), |d, doing| {
                            d.child(text(doing).text_xs().text_color(t.muted_foreground).line_clamp(2))
                        }),
                )
        }))
}

/// Pausing asks first, taking a why in a sentence, which the chain's stream shows with who paused it (ADR-0080).
fn pause_dialog(screen: &Rc<ChainScreen>, store: &Store, chain: &str) -> gui::Element {
    let (screen_, store, id) = (Rc::clone(screen), store.clone(), chain.to_owned());
    alert_dialog_content(&screen.pause, move |d| {
        d.child(
            alert_dialog_header()
                .child(alert_dialog_title("Pause this chain?"))
                .child(alert_dialog_description(
                    "Its agents stop at once and nothing new starts until you resume it. Say why, if you like.",
                )),
        )
        .child(
            input(&screen_.why)
                .placeholder("Why (optional)")
                .aria_label("Why you pause it"),
        )
        .child(
            alert_dialog_footer()
                .child(alert_dialog_cancel(&screen_.pause).child("Cancel"))
                .child({
                    let screen = Rc::clone(&screen_);
                    alert_dialog_action(&screen_.pause, move || {
                        let why = screen.why.text();
                        screen.why.set_text("");
                        act(
                            &screen.toaster,
                            &store,
                            json!({ "method": "chain_pause", "args": { "id": id, "why": why } }),
                            "Chain paused",
                        );
                    })
                    .child("Pause")
                }),
        )
    })
}
