//! The way in (ADR-0128 decision 3): the owner pastes the link the server printed at start, or a mentor the one the
//! owner sent them, and comes in with what it carries. Typing a credential is his alone; the field masks it.

use gui::prelude::*;
use gui::{Div, InputType, Key, NamedKey, div, text};
use shadcn::{
    ButtonSize, ButtonVariant, button, card, card_content, card_description, card_footer, card_header, card_title,
    input, theme,
};

use crate::app::{App, Stage};
use crate::way_in::read_link;

pub fn screen(app: &App, why: Option<&str>) -> Div {
    let t = theme();
    let enter = {
        let app = app.clone();
        move || come_in(&app)
    };
    let on_enter = enter.clone();
    div().size_full().flex().items_center().justify_center().p(6.0).child(
        card()
            .w_px(440.0)
            .child(
                card_header().child(card_title("Come in to Remit")).child(card_description(
                    "Paste the link Remit printed when it started (http://…/#key=…), or the one the owner sent you.",
                )),
            )
            .child(
                card_content().flex_col().gap(2.0).child(
                    input(&app.link)
                        .input_type(InputType::Password)
                        .placeholder("http://127.0.0.1:4320/#key=…")
                        .aria_label("Your link to Remit")
                        .on_key_down(move |k| {
                            let pressed = k.key == Key::Named(NamedKey::Enter);
                            if pressed {
                                on_enter();
                            }
                            pressed
                        }),
                )
                .when_some(why, |d, why| {
                    d.child(text(why.to_owned()).text_sm().text_color(t.destructive))
                }),
            )
            .child(
                card_footer().child(
                    button(ButtonVariant::Default, ButtonSize::Default)
                        .w_full()
                        .child("Come in")
                        .on_click(move |_| enter()),
                ),
            ),
    )
}

/// Comes in with what was pasted; the field empties either way, as what it masks cannot be read to correct.
fn come_in(app: &App) {
    let pasted = app.link.text();
    app.link.set_text("");
    match read_link(&pasted) {
        Some(credential) => app.come_in(credential),
        None => app.stage.set(Stage::Out {
            why: Some("That is not a link to Remit: it starts with http and ends with #key=…".to_owned()),
        }),
    }
}
