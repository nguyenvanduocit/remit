//! Every chain on one board, laid out by the server (`Board`): its columns in its order, each with what waits for the
//! owner from no chain (in the first) and its chains, each card saying where the chain stands in the server's words
//! (`ChainStanding`) and how long it has waited for him, has run, or since when it finished. The web's twin is
//! `BoardView.vue`.

use std::collections::HashMap;
use std::rc::Rc;

use gui::prelude::*;
use gui::{Div, div, text};
use model::{InboxItem, Project};
use replica::Model;
use shadcn::{BadgeVariant, ItemSize, ItemVariant, badge, item, item_content, theme};
use wire::app::{Board, BoardColumn, ChainRead, ChainStanding, ChainTone, Column};

use crate::core::now;
use crate::store::Store;
use crate::words;

/// What a column needs to draw its cards: the records its ids name.
struct Held {
    chains: HashMap<String, ChainRead>,
    standings: HashMap<String, ChainStanding>,
    inbox: HashMap<String, InboxItem>,
    projects: HashMap<String, String>,
    /// The server's word for each thing that waits for the owner, by its inbox item's id.
    labels: HashMap<String, String>,
    now: link::Millis,
}

/// Opens a chain by its id.
pub type OpenChain = Rc<dyn Fn(String)>;

pub fn view(store: &Store, open: &OpenChain) -> Div {
    let Some(board) = store.one::<Board>(&Model::Board) else {
        return div();
    };
    let held = Held {
        chains: by_id(store.records::<ChainRead>(&Model::Chains), |c| c.chain.id.clone()),
        standings: by_id(store.records::<ChainStanding>(&Model::Standings), |s| {
            s.chain_id.clone()
        }),
        inbox: by_id(store.records::<InboxItem>(&Model::Inbox), |i| i.id.clone()),
        projects: store
            .records::<Project>(&Model::Projects)
            .into_iter()
            .map(|p| (p.id, p.name))
            .collect(),
        labels: board.waiting.iter().map(|w| (w.id.clone(), w.label.clone())).collect(),
        now: now(),
    };
    div()
        .flex()
        .flex_1()
        .min_w(0.0)
        .gap(3.0)
        .px(4.0)
        .pt(2.5)
        .pb(3.5)
        .overflow_x_scroll()
        .children(board.columns.iter().map(|col| column(col, &held, open)))
}

fn by_id<T>(records: Vec<T>, id: impl Fn(&T) -> String) -> HashMap<String, T> {
    records.into_iter().map(|r| (id(&r), r)).collect()
}

/// One column: its name and how many it holds, what waits from no chain, then its chains in the server's order.
fn column(col: &BoardColumn, held: &Held, open: &OpenChain) -> Div {
    let t = theme();
    let items: Vec<&InboxItem> = col.items.iter().filter_map(|id| held.inbox.get(id)).collect();
    let chains: Vec<&ChainRead> = col.chains.iter().filter_map(|id| held.chains.get(id)).collect();
    let count = col.chains.len().saturating_add(col.items.len());
    let finished = col.key == Column::Finished;
    div()
        .flex()
        .flex_col()
        // The columns share the board's width, each at least 220px; a narrower window scrolls them.
        .flex_1()
        .min_w_px(220.0)
        .min_h(0.0)
        .rounded_lg()
        .bg(t.muted)
        .p(2.0)
        .role(gui::Role::Group)
        .aria_label(col.label.clone())
        .child(
            div()
                .flex()
                .items_center()
                .gap(2.0)
                .px(1.5)
                .pt(1.0)
                .pb(2.5)
                .child(
                    text(col.label.to_uppercase())
                        .text_xs()
                        .font_semibold()
                        .text_color(t.muted_foreground),
                )
                .child(
                    div()
                        .ml_auto()
                        .child(badge(BadgeVariant::Outline).child(count.to_string())),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(0.0)
                .gap(2.0)
                .overflow_y_scroll()
                .children(items.into_iter().map(|i| inbox_card(i, held)))
                .children(chains.into_iter().map(|c| chain_card(c, held, finished, open)))
                .when(count == 0, |d| {
                    d.child(
                        text(col.empty.clone())
                            .text_xs()
                            .text_color(t.muted_foreground)
                            .px(2.0)
                            .py(4.0)
                            .text_center(),
                    )
                }),
        )
}

/// Something that waits for the owner from no chain (files a deleted chain left, the idle watchdog).
fn inbox_card(i: &InboxItem, held: &Held) -> Div {
    item(ItemVariant::Outline, ItemSize::Sm)
        .key(gui::key_of(&i.id))
        .bg(theme().background)
        .child(
            item_content()
                .min_w(0.0)
                .child(title(&i.title))
                .when_some(held.labels.get(&i.id), |d, label| {
                    d.child(div().child(badge(BadgeVariant::Destructive).child(label.clone())))
                }),
        )
}

/// A chain: its title, then its state in one word, what the server says beside it, its project, and its time.
/// A click opens the chain.
fn chain_card(c: &ChainRead, held: &Held, finished: bool, open: &OpenChain) -> Div {
    let t = theme();
    let chain = &c.chain;
    let st = held.standings.get(&chain.id);
    let wait = st.and_then(|s| s.wait.as_ref());
    let words: Vec<&str> = st.map_or(Vec::new(), |s| s.text.split(" · ").collect());
    let state = wait.map_or_else(
        || words.first().copied().unwrap_or_default().to_owned(),
        |w| w.label.clone(),
    );
    let rest = wait.map_or_else(|| words.get(1..).unwrap_or_default().join(" · "), |w| w.ask.clone());
    let more = st.map_or(0, |s| s.waits.saturating_sub(1));
    let project = st.and_then(|s| s.project.as_ref()).and_then(|p| held.projects.get(p));
    let when = if finished {
        format!("{} ago", words::span_since(chain.updated_at.0, held.now))
    } else if let Some(w) = wait {
        format!("waiting {}", words::span_since(w.since.0, held.now))
    } else {
        words::span_since(chain.created_at.0, held.now)
    };
    item(ItemVariant::Outline, ItemSize::Sm)
        .key(gui::key_of(&chain.id))
        .bg(t.background)
        .on_click({
            let (open, id) = (Rc::clone(open), chain.id.clone());
            move |_| open(id.clone())
        })
        .child(
            item_content().min_w(0.0).gap(1.5).child(title(&chain.title)).child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(1.5)
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .when(!state.is_empty(), |d| {
                        d.child(badge(tone(st.map(|s| s.tone))).child(state))
                    })
                    .when(finished && st.is_some_and(|s| s.unseen), |d| {
                        d.child(badge(BadgeVariant::Secondary).child("new"))
                    })
                    .when_some(project, |d, p| d.child(text(p.clone()).truncate()))
                    .when(!rest.is_empty(), |d| d.child(text(rest).min_w(0.0)))
                    .when(more > 0, |d| {
                        d.child(text(if more == 1 {
                            "+1 more wait".to_owned()
                        } else {
                            format!("+{more} more waits")
                        }))
                    })
                    .child(div().ml_auto().child(text(when))),
            ),
        )
}

/// A card's title, `item_title`'s type, as a line of the card's column so it wraps rather than clips.
fn title(words: &str) -> gui::Text {
    text(words.to_owned()).text_sm().leading_snug().font_medium()
}

/// A standing's tone as a badge: what needs the owner or failed stands out, what runs is marked, the rest is quiet.
pub fn tone(tone: Option<ChainTone>) -> BadgeVariant {
    match tone {
        Some(ChainTone::Needs | ChainTone::Failed) => BadgeVariant::Destructive,
        Some(ChainTone::Running) => BadgeVariant::Default,
        Some(ChainTone::Done) => BadgeVariant::Secondary,
        _ => BadgeVariant::Outline,
    }
}
