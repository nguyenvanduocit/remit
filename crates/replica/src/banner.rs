//! What the page says of the link (ADR-0094), worded here once for every shell: whether what shows is current, since
//! when it is not, when the core dials again and what the owner can do about it. Pure: the core gives what it knows.

use link::{Millis, RefusalCode};

use crate::shell::{Banner, BannerAction, BannerTone, Phase};

/// How long the page says the link is back before the banner goes.
pub(crate) const BACK_SHOWN: Millis = 3_000;

/// What the core knows of the link, the moment it is asked.
pub(crate) struct Link<'a> {
    pub(crate) now: Millis,
    pub(crate) phase: Phase,
    pub(crate) why: &'a str,
    pub(crate) refused: Option<RefusalCode>,
    /// When the link went, once there was a page to keep showing.
    pub(crate) lost_at: Option<Millis>,
    /// Dials that failed in a row.
    pub(crate) tries: u32,
    pub(crate) next_try: Option<Millis>,
    /// Until when the page says the link is back.
    pub(crate) back_until: Option<Millis>,
    /// The owner's messages waiting for the link.
    pub(crate) waiting: usize,
}

const BLOCKED: &str = "Not connected to the server: this works again once the connection is back.";

/// Why an action that changes something cannot be taken now: with no session nothing is sent, and a message (which
/// waits for the link) is the one thing the owner still sends.
pub(crate) fn blocked(phase: Phase) -> String {
    match phase {
        Phase::Offline => BLOCKED.to_owned(),
        Phase::Syncing | Phase::Stale | Phase::Live => String::new(),
    }
}

/// What the page says, or none while everything shown is current.
pub(crate) fn banner(link: &Link) -> Option<Banner> {
    match link.phase {
        Phase::Live => link.back_until.filter(|until| *until > link.now).map(|_| Banner {
            tone: BannerTone::Back,
            text: "Connected again: what you see is current.".to_owned(),
            since: None,
            action: None,
            waits: None,
            announce: "Connected again.".to_owned(),
        }),
        Phase::Syncing => Some(said(
            BannerTone::Loading,
            "Connected, loading the latest data…".to_owned(),
        )),
        Phase::Stale => Some(said(
            BannerTone::Stale,
            format!(
                "Connected, but the latest data could not be loaded ({}), trying again. What you see may be out of date.",
                link.why
            ),
        )),
        Phase::Offline => Some(offline(link)),
    }
}

fn said(tone: BannerTone, text: String) -> Banner {
    Banner {
        tone,
        announce: text.clone(),
        text,
        since: None,
        action: None,
        waits: None,
    }
}

fn offline(link: &Link) -> Banner {
    let waits = waits(link.waiting, link.refused);
    match link.refused {
        Some(RefusalCode::NewBundle) => {
            let text = "This page and the server run different versions of Remit: reload the page to use the server’s.";
            return Banner {
                tone: BannerTone::Down,
                text: text.to_owned(),
                since: None,
                action: Some(BannerAction::Reload),
                announce: join(text, waits.as_deref()),
                waits,
            };
        }
        Some(RefusalCode::BadCredential) => {
            let text = "The server refused this browser’s owner key: sign in again with the link the server prints.";
            return Banner {
                tone: BannerTone::Down,
                text: text.to_owned(),
                since: None,
                action: None,
                announce: join(text, waits.as_deref()),
                waits,
            };
        }
        None => {}
    }
    let action = link.next_try.map(|_| BannerAction::TryNow);
    let since = link.lost_at;
    let (text, announce) = match since {
        Some(_) => (
            format!(
                "Connection lost at {{since}}: what you see is from then. {}",
                retrying(link)
            ),
            join("Connection lost.", waits.as_deref()),
        ),
        None if link.why.is_empty() => (
            "Connecting to the server…".to_owned(),
            "Connecting to the server.".to_owned(),
        ),
        None => (
            format!("Not connected yet ({}). {}", link.why, retrying(link)),
            "Connecting to the server.".to_owned(),
        ),
    };
    Banner {
        tone: BannerTone::Down,
        text,
        since,
        action,
        waits,
        announce,
    }
}

/// When the core dials again, in words: now, or in how many seconds.
fn retrying(link: &Link) -> String {
    let tries = if link.tries > 0 {
        format!(" (try {})", link.tries.saturating_add(1))
    } else {
        String::new()
    };
    match link.next_try.filter(|at| *at > link.now) {
        Some(at) => format!("Reconnecting in {} s{tries}.", seconds_left(at, link.now)),
        None => format!("Reconnecting{tries}…"),
    }
}

fn seconds_left(at: Millis, now: Millis) -> Millis {
    at.saturating_sub(now).div_ceil(1_000)
}

/// When the countdown to `next_try` shows its next number: the banner is worded again then.
pub(crate) fn next_count(next_try: Millis, now: Millis) -> Option<Millis> {
    let left = seconds_left(next_try, now);
    (left > 1).then(|| next_try.saturating_sub(left.saturating_sub(1).saturating_mul(1_000)))
}

/// How many of the owner's messages wait, and when they go.
fn waits(waiting: usize, refused: Option<RefusalCode>) -> Option<String> {
    let when = match refused {
        Some(RefusalCode::NewBundle) => " to go out once the page reloads",
        Some(RefusalCode::BadCredential) => " to go out once you sign in again",
        None => " to go out",
    };
    match waiting {
        0 => None,
        1 => Some(format!("1 message waits{when}.")),
        n => Some(format!("{n} messages wait{when}.")),
    }
}

fn join(text: &str, waits: Option<&str>) -> String {
    match waits {
        Some(waits) => format!("{text} {waits}"),
        None => text.to_owned(),
    }
}
