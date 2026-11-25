//! Whether an agent may act on a connection. An agent never holds a credential: it holds grants, the right to ask an
//! executor of Remit to act on one connection of one source with an effect (ADR-0018). [`decide`] is the one rule, a pure
//! function over the grants a chain holds and the action it asks for. It stores nothing and depends on no crate of the
//! workspace: storing grants and raising the change item that an ask needs belong to the orchestrator, and the gate that
//! calls [`decide`] for every call belongs to `source`.

use std::fmt;
use std::str::FromStr;

/// What an action does to the outside world, ranked by how far it can be undone. The variants are ordered: a grant of a
/// higher effect covers every lower one. The platform ranks a tool; its author does not. There is no "no effect": where
/// one may be absent it is an `Option<Effect>`, and text becomes an `Effect` once, with [`str::parse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    /// Looks at the connection and changes nothing.
    Read,
    /// Changes it in a way that can be put back exactly (a draft saved, a flag set).
    Reversible,
    /// Changes it in a way another act answers for (a charge and its refund).
    Compensable,
    /// Changes it and nothing puts it back (a message sent, a record destroyed).
    Irreversible,
}

impl Effect {
    /// The name an effect is stored and sent under (`grants.effect`, a source call's document, a plugin's manifest).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Reversible => "reversible",
            Self::Compensable => "compensable",
            Self::Irreversible => "irreversible",
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The text that named no effect, at the boundary where an effect arrives as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEffect(String);

impl fmt::Display for UnknownEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "effect {:?} is not one of read, reversible, compensable, irreversible",
            self.0
        )
    }
}

impl std::error::Error for UnknownEffect {}

impl FromStr for Effect {
    type Err = UnknownEffect;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        [Self::Read, Self::Reversible, Self::Compensable, Self::Irreversible]
            .into_iter()
            .find(|e| e.as_str() == s)
            .ok_or_else(|| UnknownEffect(s.to_owned()))
    }
}

/// The right to act with `effect`, or less, on one connection of one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub source: String,
    pub connection: String,
    pub effect: Effect,
}

/// What a call asks to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub source: String,
    pub connection: String,
    pub effect: Effect,
}

/// The three ways [`decide`] answers, without the words a refusal carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The call goes ahead.
    Allow,
    /// The call is refused.
    Deny,
    /// The owner decides: the call waits for a grant only he can give.
    Ask,
}

/// Which kind of refusal a deny or an ask is, so that refusals can be counted by kind and not read as prose. More kinds
/// join when the rules that make them do: a stop and a law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The chain holds no grant that covers the action: it asked for what it was never given.
    NoGrant,
    /// A rule of the platform that no grant moves turned the action away.
    Rail,
}

impl Refusal {
    /// The name a refusal is stored under in a source call's document.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoGrant => "no_grant",
            Self::Rail => "rail",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How [`decide`] answers: a verdict and, for a deny or an ask, the reason an agent is told and the kind of refusal it is.
/// An allow carries neither, so none can be read from one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny { reason: String, refusal: Refusal },
    Ask { reason: String, refusal: Refusal },
}

impl Decision {
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        match self {
            Self::Allow => Verdict::Allow,
            Self::Deny { .. } => Verdict::Deny,
            Self::Ask { .. } => Verdict::Ask,
        }
    }

    /// What an agent is told; empty for an allow.
    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            Self::Allow => "",
            Self::Deny { reason, .. } | Self::Ask { reason, .. } => reason,
        }
    }

    #[must_use]
    pub const fn refusal(&self) -> Option<Refusal> {
        match self {
            Self::Allow => None,
            Self::Deny { refusal, .. } | Self::Ask { refusal, .. } => Some(*refusal),
        }
    }
}

/// Answers one action against the grants a chain holds. The rules:
///   - a grant on the action's source and connection whose effect is at least the action's allows it;
///   - an effect above every grant on that connection asks the owner (the lowest grant is a read, so every other effect
///     can be above one);
///   - no grant on that connection denies, however many other connections of the source are granted.
#[must_use]
pub fn decide(grants: &[Grant], a: &Action) -> Decision {
    let strongest = grants
        .iter()
        .filter(|g| g.source == a.source && g.connection == a.connection)
        .map(|g| g.effect)
        .max();
    match strongest {
        Some(held) if held >= a.effect => Decision::Allow,
        Some(held) => Decision::Ask {
            reason: format!(
                "{} on {}/{} is above the {held} granted: the owner decides",
                a.effect, a.source, a.connection
            ),
            refusal: Refusal::NoGrant,
        },
        None => Decision::Deny {
            reason: format!("no grant for {}/{}", a.source, a.connection),
            refusal: Refusal::NoGrant,
        },
    }
}

/// Whether a grant may only be given by the owner: one that writes, or one on a connection the owner marked protected.
/// The Chair asks for the other grants on its own, within its chain's capabilities. It is a rule of issuing a grant, so
/// it is not part of [`decide`], which only reads what was issued.
#[must_use]
pub fn needs_owner(g: &Grant, protected: bool) -> bool {
    g.effect > Effect::Read || protected
}
