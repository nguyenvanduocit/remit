//! What goes into the client core and what comes out of it (ADR-0072 decision 10, ADR-0095 decision 4): a shell hands
//! it what its transport saw, the time and each tab's asks; it hands back what to do on the transport, when to wake it,
//! and what each tab is told (the models it shows and their changes, the answers to its calls, what the owner should
//! notice). The part a tab asks and is told with is `crate::shell`'s; what is here carries the transport and the tabs.

use link::{Closing, Millis, StreamId};

use crate::shell::{Ask, Told};

/// Names one of the browser's tabs the core serves, as its shell numbers them: what it opens and shows is its own, and
/// what it is told goes to it alone.
pub type Tab = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// What a tab asks; the first ask of a tab the core does not know starts serving it.
    Ask {
        tab: Tab,
        ask: Ask,
    },
    /// The tab is gone (closed, reloaded, crashed): what it opened and showed is let go, and the session with the last.
    Left {
        tab: Tab,
    },
    /// The shell's session is open (after `Output::Dial`), with the credential this attempt carries.
    Opened {
        credential: String,
    },
    /// The shell could not open a session.
    DialFailed {
        message: String,
    },
    Received {
        stream: StreamId,
        bytes: Vec<u8>,
    },
    Written {
        stream: StreamId,
        n: usize,
    },
    Closed(Closing),
    /// The time `Output::Timer` asked for came.
    Tick,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Open a session to the server; then `Opened` or `DialFailed`.
    Dial,
    Open(StreamId),
    Send(StreamId, Vec<u8>),
    Finish(StreamId),
    Reset(StreamId),
    Close(Closing),
    /// Send `Tick` at this time, in place of any time asked before; none: no timer.
    Timer(Option<Millis>),
    /// What `tab` is told, in the order the core said it.
    Tell {
        tab: Tab,
        told: Told,
    },
}
