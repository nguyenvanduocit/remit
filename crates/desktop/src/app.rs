//! The window (ADR-0128 decision 5): the way in until a credential is held, then what the connection is doing until the
//! first snapshot is in, then the app: the top bar of the web's five areas, the link's banner while what shows may not be
//! current, and the area open. The server lays out everything drawn here; the window keeps only which area is open.

use std::rc::Rc;

use gui::prelude::*;
use gui::{Div, Element, Icon, Signal, div, icon, text};
use link::RefusalCode;
use replica::{Ask, Banner, BannerAction, BannerTone, Model, Notice, Phase, What};
use shadcn::{
    BadgeVariant, ButtonSize, ButtonVariant, EmptyMediaVariant, Toaster, badge, button, empty, empty_description,
    empty_header, empty_media, empty_media_icon, empty_title, spinner, theme, toaster,
};
use time::UtcOffset;

use crate::assistant::{self, AssistantPanel};
use crate::chain::{self, ChainScreen};
use crate::store::Store;
use crate::way_in::Credential;
use crate::{board, login, words};

/// How often the times the window shows are read again.
const TICK: std::time::Duration = std::time::Duration::from_secs(30);

/// Where the owner is: the web's five areas (`web/src/router.ts` `Area`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Work,
    Results,
    Signals,
    Crew,
    Integrations,
}

impl Area {
    const ALL: [Self; 5] = [Self::Work, Self::Results, Self::Signals, Self::Crew, Self::Integrations];

    /// The page the area opens on, by the web's name for it: where the owner is, for the Assistant.
    fn page(self) -> &'static str {
        match self {
            Self::Work => "board",
            Self::Results => "outcomes",
            Self::Signals => "signals",
            Self::Crew => "employees",
            Self::Integrations => "tools",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Work => "Work",
            Self::Results => "Results",
            Self::Signals => "Signals",
            Self::Crew => "Crew",
            Self::Integrations => "Integrations",
        }
    }
}

/// Whether the window holds a credential to come in with.
#[derive(Clone)]
pub enum Stage {
    /// The way in, saying why when the last credential did not come in.
    Out {
        why: Option<String>,
    },
    In(Store),
}

/// Starts a core for a credential and says what the window holds of it; the reason when it cannot.
pub type ComeIn = Rc<dyn Fn(&App, Credential) -> Result<Store, String>>;

/// Everything the window shows, made once; the view reads it every frame.
#[derive(Clone)]
pub struct App {
    pub stage: Signal<Stage>,
    pub area: Signal<Area>,
    /// Moves every half minute: what has waited or run for how long is read again.
    pub tick: Signal<u64>,
    /// The owner's clock: the banner's times are his.
    pub offset: UtcOffset,
    pub toaster: Toaster,
    /// What the owner pastes on the way in.
    pub link: gui::TextField,
    /// The chain open in Work, over the board.
    pub chain: Rc<ChainScreen>,
    /// The Assistant's column beside the area.
    pub assistant: Rc<AssistantPanel>,
    come_in: ComeIn,
}

impl App {
    pub fn new(offset: UtcOffset, salt: String, come_in: ComeIn) -> Self {
        let toaster = Toaster::new();
        Self {
            stage: Signal::labeled("stage", Stage::Out { why: None }),
            area: Signal::labeled("area", Area::Work),
            tick: Signal::labeled("tick", 0),
            offset,
            chain: ChainScreen::new(salt.clone(), toaster.clone()),
            assistant: AssistantPanel::new(salt, toaster.clone()),
            toaster,
            link: gui::TextField::new(""),
            come_in,
        }
    }

    /// Moves `tick` every half minute from now on.
    pub fn keep_time(&self) {
        let app = self.clone();
        gui::clock::after(TICK, move || {
            app.tick.update(|n| n.wrapping_add(1));
            app.keep_time();
        });
    }

    /// Comes in with `credential`: its core starts and dials, and the window shows what it holds.
    pub fn come_in(&self, credential: Credential) {
        match (self.come_in)(self, credential) {
            Ok(store) => {
                self.chain.open.set(None);
                self.assistant.close();
                store.ask(Ask::Connect);
                self.stage.set(Stage::In(store));
            }
            Err(why) => self.stage.set(Stage::Out { why: Some(why) }),
        }
    }

    pub fn view(&self) -> impl FnMut() -> Element + 'static {
        let app = self.clone();
        move || root(&app)
    }
}

/// Says `notice` where the owner looks: a toast, worded as the web's (`web/src/replica.ts`).
pub fn say(toaster: &Toaster, notice: Notice) {
    let said = match notice {
        Notice::ChainDone { title } => format!("Chain done: {title}"),
        Notice::TurnFinished { name } => format!("{name} finished its turn · report sent to the Chair"),
        Notice::InboxOpened { kind, title } => inbox_opened(kind, &title),
        // The duels' standings are read where they show; the window shows none yet.
        Notice::DuelsMoved => return,
        Notice::CouldNot { what, failure } => format!("{}: {}", could_not(what), failure.message),
    };
    toaster.toast(said);
}

/// Calls the server for an action of the owner's: a success says `done`, a failure says why, where he looks.
pub fn act(toaster: &Toaster, store: &Store, call: serde_json::Value, done: &'static str) {
    let toaster = toaster.clone();
    store.call(call, move |answer| match answer {
        Ok(_) => {
            toaster.success(done);
        }
        Err(f) => {
            toaster.error(format!("{done} failed: {}", f.message));
        }
    });
}

fn inbox_opened(kind: model::InboxKind, title: &str) -> String {
    use model::InboxKind as K;
    match kind {
        K::Question => format!("The Chair asks you: {title}"),
        K::Error => title.to_owned(),
        K::Credential => format!("A credential is needed: {title}"),
        K::Change => format!("A change waits for you: {title}"),
        K::Exception => format!("An exception waits for you: {title}"),
        _ => format!("Agent requests permission: {title}"),
    }
}

fn could_not(what: What) -> &'static str {
    match what {
        What::OpenChain => "Could not open the chain",
        What::LoadStream => "Could not load the agent’s activity",
        What::LoadEarlier => "Could not load the agent’s earlier activity",
        What::LoadAssistant => "Could not load the Assistant’s conversation",
        What::LoadAtWork => "Could not load who is at work",
        What::OpenEvent => "Could not open the event",
        What::ReadArchived => "Could not load the archived chains",
        What::ReadRepos => "Could not load the repositories",
        What::SaveDraft => "Your words were not saved",
        What::DeleteDraft => "Could not delete the draft",
    }
}

fn root(app: &App) -> Element {
    let screen = match app.stage.get() {
        Stage::Out { why } => login::screen(app, why.as_deref()),
        Stage::In(store) => signed_in(app, &store),
    };
    div()
        .size_full()
        .bg(theme().background)
        .text_color(theme().foreground)
        .child(screen)
        .child(toaster(&app.toaster))
        .into_element()
}

fn signed_in(app: &App, store: &Store) -> Div {
    let state = store.state();
    if state.as_ref().and_then(|s| s.refused) == Some(RefusalCode::BadCredential) {
        return login::screen(
            app,
            Some("The server did not take that key: paste the link it printed again."),
        );
    }
    let loaded = state.as_ref().is_some_and(|s| s.loaded);
    let banner = state.as_ref().and_then(|s| s.banner.clone());
    // Nothing to show yet: no snapshot had. Only what the connection is doing, centred.
    if !loaded && state.as_ref().is_none_or(|s| s.phase == Phase::Offline) {
        return connecting(app, store, banner.as_ref());
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(top_bar(app, store))
        .when_some(banner, |d, b| d.child(link_banner(app, store, &b)))
        .child(
            div()
                .flex()
                .flex_1()
                .min_h(0.0)
                .child(area(app, store, loaded))
                .when_some(
                    loaded
                        .then(|| assistant::view(&app.assistant, store, place(app), app.offset))
                        .flatten(),
                    |d, panel| d.child(panel),
                ),
        )
}

/// Where the owner is, as the Assistant reads it (`wire::app::Where`): a chain open and its agent picked, else the
/// page of the area open.
fn place(app: &App) -> serde_json::Value {
    match (app.area.get(), app.chain.open.get()) {
        (Area::Work, Some(open)) => match open.card {
            Some(card) => serde_json::json!({ "page": "chain", "chainId": open.chain, "cardId": card }),
            None => serde_json::json!({ "page": "chain", "chainId": open.chain }),
        },
        (area, _) => serde_json::json!({ "page": area.page() }),
    }
}

fn connecting(app: &App, store: &Store, banner: Option<&Banner>) -> Div {
    let says = banner.map_or_else(|| "Connecting…".to_owned(), |b| said(app, b));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(3.0)
        .text_sm()
        .text_color(theme().muted_foreground)
        .child(div().flex().items_center().gap(2.0).child(spinner()).child(text(says)))
        .when(banner.and_then(|b| b.action) == Some(BannerAction::TryNow), |d| {
            d.child(try_now(store, ButtonSize::Sm))
        })
}

/// The banner's words, `{since}` in the owner's clock.
fn said(app: &App, banner: &Banner) -> String {
    match banner.since {
        Some(since) => banner.text.replace("{since}", &words::clock(since, app.offset)),
        None => banner.text.clone(),
    }
}

fn try_now(store: &Store, size: ButtonSize) -> Div {
    let store = store.clone();
    button(ButtonVariant::Outline, size)
        .child(icon(Icon::RefreshCw))
        .child("Try now")
        .on_click(move |_| store.ask(Ask::Connect))
}

/// Under the top bar whenever what shows may not be current (ADR-0094): loading, could not load, or the link is down
/// since when and when it tries again; once it is back, it says so for a moment.
fn link_banner(app: &App, store: &Store, banner: &Banner) -> Div {
    let t = theme();
    let (bg, fg) = match banner.tone {
        BannerTone::Back => (t.secondary, t.secondary_foreground),
        BannerTone::Loading => (t.muted, t.muted_foreground),
        BannerTone::Stale | BannerTone::Down => (t.accent, t.accent_foreground),
    };
    let words = match &banner.waits {
        Some(waits) => format!("{} {waits}", said(app, banner)),
        None => said(app, banner),
    };
    div()
        .flex()
        .shrink_0()
        .items_center()
        .gap(2.0)
        .min_h(9.0)
        .px(4.0)
        .py(1.5)
        .border_b()
        .border_color(t.border)
        .bg(bg)
        .text_color(fg)
        .text_xs()
        .when(banner.tone == BannerTone::Down, |d| d.child(icon(Icon::TriangleAlert)))
        .when(banner.tone == BannerTone::Back, |d| d.child(icon(Icon::Check)))
        .child(text(words).min_w(0.0))
        .when(banner.action == Some(BannerAction::TryNow), |d| {
            d.child(div().ml_auto().child(try_now(store, ButtonSize::Xs)))
        })
}

/// The bar over every area: where to go, each area counting what waits for the owner in it alone (Work: what the
/// server's board says needs him).
fn top_bar(app: &App, store: &Store) -> Div {
    let t = theme();
    let here = app.area.get();
    let needs = store.one::<wire::app::Board>(&Model::Board).map_or(0, |b| b.needs_you);
    div()
        .flex()
        .h(11.0)
        .shrink_0()
        .items_center()
        .gap(3.5)
        .px(4.0)
        .border_b()
        .border_color(t.border)
        .bg(t.sidebar)
        .child(
            div()
                .flex()
                .gap(0.5)
                .role(gui::Role::TabList)
                .aria_label("Views")
                .children(Area::ALL.map(|a| area_entry(app, store, a, a == here, (a == Area::Work).then_some(needs)))),
        )
        .child(div().flex_1())
        .child(assistant_entry(app, store))
}

/// Opens and closes the Assistant's column, counting what it decided for the owner that he has not seen.
fn assistant_entry(app: &App, store: &Store) -> Div {
    let open = app.assistant.open.get();
    let new = store
        .one::<wire::app::AssistantFeed>(&Model::AssistantFeed)
        .map_or(0, |f| f.unseen);
    let (panel, store) = (Rc::clone(&app.assistant), store.clone());
    button(
        if open {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        },
        ButtonSize::Sm,
    )
    .aria_expanded(open)
    .child(icon(Icon::Sparkles))
    .child("Assistant")
    .when(new > 0, |b| {
        b.child(badge(BadgeVariant::Default).child(new.to_string()))
    })
    .on_click(move |_| panel.toggle(&store))
}

/// An area's tab; Work's, from a chain, goes back to the board.
fn area_entry(app: &App, store: &Store, area: Area, on: bool, count: Option<usize>) -> Div {
    let (target, screen, store) = (app.area.clone(), Rc::clone(&app.chain), store.clone());
    let variant = if on {
        ButtonVariant::Secondary
    } else {
        ButtonVariant::Ghost
    };
    button(variant, ButtonSize::Sm)
        .role(gui::Role::Tab)
        .aria_selected(on)
        .child(area.label())
        .when_some(count, |b, n| {
            let tone = if n > 0 {
                BadgeVariant::Destructive
            } else {
                BadgeVariant::Outline
            };
            b.child(badge(tone).child(n.to_string()))
        })
        .on_click(move |_| {
            if area == Area::Work && screen.open.get().is_some() {
                screen.close(&store);
            }
            target.set(area);
        })
}

fn area(app: &App, store: &Store, loaded: bool) -> Div {
    let body = if loaded {
        match app.area.get() {
            Area::Work => {
                // Read so the board's times move with the clock.
                app.tick.get();
                if app.chain.open.get().is_some() {
                    chain::view(&app.chain, store)
                } else {
                    let (screen, store_) = (Rc::clone(&app.chain), store.clone());
                    let open: board::OpenChain = Rc::new(move |id| screen.open_chain(&store_, id));
                    board::view(store, &open)
                }
            }
            other => elsewhere(other),
        }
    } else {
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(2.0)
            .size_full()
            .text_sm()
            .text_color(theme().muted_foreground)
            .child(spinner())
            .child("Loading…")
    };
    div().flex().flex_1().min_h(0.0).child(body)
}

/// An area the window does not draw yet (ADR-0128 decision 5): it says so, and where it is.
fn elsewhere(area: Area) -> Div {
    div().flex().size_full().items_center().justify_center().child(
        empty().child(
            empty_header()
                .child(empty_media(EmptyMediaVariant::Icon).child(empty_media_icon(Icon::Globe)))
                .child(empty_title(format!("{} is in the web app for now", area.label())))
                .child(empty_description(
                    "The desktop draws the board first; this area follows. Open Remit in the browser to see it.",
                )),
        ),
    )
}
