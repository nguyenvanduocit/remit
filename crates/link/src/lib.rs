//! The link: one symmetric protocol between Remit's nodes and every client core (ADR-0072 decisions 2, 3 and 12). Pure:
//! frames in, frames out, the time given by the driver; it builds for wasm32 and natively, and a driver maps it onto
//! WebTransport (or any transport with streams) through `Transport`.

mod conn;
mod contract;
pub mod envelope;
pub mod memory;
mod written;

pub use conn::{CallId, Closing, Config, Conn, Event, Incoming, Millis, Side, StreamId, StreamKind, Transport};
pub use contract::{
    CallKind, CallSpec, Contract, Outgoing, Pending, Pushes, decode_answer_part, decode_call, decode_part, decode_push,
    encode_answer, first, unknown,
};
pub use envelope::{ErrorCode, Failure, Hello, Node, Payload, Refusal, RefusalCode, Request, Welcome};
