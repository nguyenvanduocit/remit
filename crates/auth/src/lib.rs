//! Decides who is let in: whoever holds the owner key, a file only the owner can read (ADR-0022), or the owner signed in
//! with Google through the server's Firebase project, when it has one (ADR-0105). This is a rail, so it is plain code
//! with no judgement in it.

mod gate;
pub mod google;
mod local;
mod session;

pub use gate::{OwnerGate, SignedIn};
pub use local::{Error, LOCAL_OWNER, LocalKey};
pub use session::SESSION_FOR;
