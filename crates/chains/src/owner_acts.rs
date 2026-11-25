//! The acts the owner holds over Remit (ADR-0062): a decision, which the Assistant makes for him and he may overturn, or a
//! secret, which only he handles, since no agent ever reads one, the Assistant included. Every owner check calls
//! `for_owner`; who decides an inbox item at all is `may_decide`'s.

use model::{Actor, ActorKind, InboxItem, InboxKind};

use crate::homes::is_standalone_home;
use crate::{Error, Result};

/// An act the owner holds: whether it is a secret, and what anyone it is not theirs is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerAct {
    secret: bool,
    refusal: &'static str,
}

impl OwnerAct {
    /// What anyone the act is not theirs is told.
    pub const fn refusal(self) -> &'static str {
        self.refusal
    }
}

/// Lets a change item through: a hire, a retirement or a move onto a newer template, an objective, a grant, an
/// extension, a forget, a write that moves a rail. The Assistant lets through any, its own included: it holds the
/// owner's say (ADR-0062).
pub const ACT_APPLY: OwnerAct = OwnerAct {
    secret: false,
    refusal: "a change item is let through by the owner, or the Assistant for him: an agent never lets its own through",
};

/// Turns an installed extension off or on again: off, it is no source and its process ends.
pub const ACT_EXTENSION_STATE: OwnerAct = OwnerAct {
    secret: false,
    refusal: "an extension is turned off or on by the owner, or the Assistant for him",
};

/// Enters or removes the value of a credential, in the field of its inbox item.
pub const ACT_CREDENTIAL: OwnerAct = OwnerAct {
    secret: true,
    refusal: "a credential is entered and removed by the owner himself, in the field of its inbox item: an agent never \
              takes one, so do not ask him to say it here",
};

/// Types the code of a provider sign-in, in the field of its inbox item.
pub const ACT_SIGN_IN_CODE: OwnerAct = OwnerAct {
    secret: true,
    refusal: "the code of a sign-in is typed by the owner himself, in the field of its inbox item: an agent never takes \
              one, so do not ask him to say it here",
};

/// Reads the value of a key or a code (a client key, a mentor's code, an event signal's key) to pass it on.
pub const ACT_SEE_KEY: OwnerAct = OwnerAct {
    secret: true,
    refusal: "only the owner is shown a key's or a code's value: he opens its row on the page that lists it",
};

/// Whether item it asks for a secret, a credential's value or a sign-in's code: what only the owner types, and what no
/// agent, decider or judge is put. The one rule `waits_on_owner`, the deciders' prior and the second judge follow. Pure.
pub fn holds_a_secret(it: &InboxItem) -> bool {
    matches!(it.kind, InboxKind::Credential | InboxKind::Login)
}

/// Whether by may do act: the owner always; whoever acts for him (`Actor::for_owner`: the Assistant) every act but a
/// secret; nobody else. Pure.
pub fn for_owner(act: OwnerAct, by: &Actor) -> Result<()> {
    if by.kind == ActorKind::Owner || (!act.secret && by.for_owner()) {
        return Ok(());
    }
    Err(Error::refused(act.refusal))
}

/// Whether an item raised in home was raised by the Assistant: its home holds its card and the subagents its process
/// spawned (ADR-0073), so their questions, failures and permissions are its own. The one test of "the Assistant raised
/// it", at raise time (`owner_only`) and at decision (`may_decide`). Pure.
pub fn raised_by_assistant(home: &str) -> bool {
    is_standalone_home(home)
}

/// Whether by may decide inbox item it (ADR-0087): the owner any; anyone else none that waits only for him
/// (`owner_only`), nor one the Assistant raised (`raised_by_assistant`), since the asker never decides its own
/// question, failure or permission. A change item it raised is the exception: it holds the owner's say over a write
/// (`ACT_APPLY`, ADR-0062). What the act itself asks of by (`for_owner`) is checked by the item's kind. Pure.
pub fn may_decide(it: &InboxItem, by: &Actor) -> Result<()> {
    let own = raised_by_assistant(&it.home) && it.kind != InboxKind::Change;
    if by.kind == ActorKind::Owner || !(it.owner_only || own) {
        return Ok(());
    }
    Err(Error::refused(format!(
        "item {} waits for the owner: the Assistant raised it, or no agent decides it for him, and the asker never \
         decides its own. Tell him it waits.",
        it.id
    )))
}
