//! What a tab says on its port to the SharedWorker that runs the browser's one core (ADR-0095 decision 4). `make gen`
//! writes it for the page (`web/src/core/generated/client_web`), whose side is `overPort` in `web/src/core/client.ts`.

use replica::Ask;
use serde::Deserialize;

/// What a tab says on its port: its hello first, then its asks, `bye` when it lets the core go.
#[derive(Deserialize)]
#[serde(tag = "say", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FromTab {
    Hello {
        /// The page's `Start`, as JSON: read only when this tab is the first.
        start: String,
        /// What the core's next dial carries.
        credential: String,
        /// The name of the Web Lock the tab holds while it is open.
        lock: String,
    },
    Ask {
        ask: Ask,
    },
    Bye,
}
