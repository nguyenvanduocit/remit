//! What a chain may do on a connection (ADR-0018), decided in one place. The gate asks it for every call (grants.rs),
//! list_sources shows its answer and request_grant asks it before raising anything, so none of the three can disagree with
//! another.
//!
//! A chain holds the grants stored for it (`grants`), and, beyond them, a read on every connection of a source whose
//! capability it holds, unless the owner marked that connection protected. That read is derived here, never
//! stored: a connection made later, or a capability changed, needs no data fix. A protected connection is read through a
//! stored grant.

use grant::{Action, Decision, Effect, Grant, Refusal};
use model::ChangedField;
use serde::{Deserialize, Serialize};

/// A connection as the rule sees it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ConnectionRef {
    pub source: String,
    pub name: String,
    /// The capability whose chains read the source.
    pub capability: String,
    pub protected: bool,
}

impl ConnectionRef {
    /// "<source>/<name>", as an agent and the owner read it.
    pub(crate) fn place(&self) -> String {
        format!("{}/{}", self.source, self.name)
    }
}

/// Every effect, the strongest first.
const STRONGEST_FIRST: [Effect; 4] = [
    Effect::Irreversible,
    Effect::Compensable,
    Effect::Reversible,
    Effect::Read,
];

/// The grants a chain holds on c: the read its capabilities imply, and the stored ones.
fn grants_on(capabilities: &[String], c: &ConnectionRef, stored: &[Grant]) -> Vec<Grant> {
    let mut out = stored.to_vec();
    if !c.protected && capabilities.contains(&c.capability) {
        out.push(Grant {
            source: c.source.clone(),
            connection: c.name.clone(),
            effect: Effect::Read,
        });
    }
    out
}

/// The action of effect e on c.
fn action_on(c: &ConnectionRef, e: Effect) -> Action {
    Action {
        source: c.source.clone(),
        connection: c.name.clone(),
        effect: e,
    }
}

/// `grant::decide` over everything the chain holds on c.
pub(crate) fn decide_on(capabilities: &[String], c: &ConnectionRef, stored: &[Grant], e: Effect) -> Decision {
    grant::decide(&grants_on(capabilities, c, stored), &action_on(c, e))
}

/// The decision for a program run with the connection's credential in its environment (form env). Such a program holds
/// the credential's value, so the read a capability implies does not reach it: only a grant the owner stored does, and
/// only on a connection he did not mark protected. That the credential is read-only is the owner's word in giving the
/// grant.
pub(crate) fn decide_program(capabilities: &[String], c: &ConnectionRef, stored: &[Grant], e: Effect) -> Decision {
    if !capabilities.contains(&c.capability) {
        return Decision::Deny {
            reason: format!("this chain holds no capability of {}", c.source),
            refusal: Refusal::NoGrant,
        };
    }
    if c.protected {
        return Decision::Deny {
            reason: format!(
                "{} is protected: a program is never run with the credential of a protected connection",
                c.place()
            ),
            refusal: Refusal::Rail,
        };
    }
    match grant::decide(stored, &action_on(c, e)) {
        Decision::Deny { refusal, .. } => Decision::Deny {
            reason: format!(
                "a program holds the credential of {}, so it needs a read grant the owner gave for programs (request_grant with form env)",
                c.place()
            ),
            refusal,
        },
        d => d,
    }
}

/// What a chain may do with the run tool on c, as an agent reads it in list_sources: the rule's own answer.
pub(crate) fn program_words(capabilities: &[String], c: &ConnectionRef, stored: &[Grant]) -> String {
    match decide_program(capabilities, c, stored, Effect::Read) {
        Decision::Allow => {
            "run allowed (a read grant the owner gave for programs; enforced: credential, so the key must be read-only)"
                .to_owned()
        }
        d => format!("run none ({})", d.reason()),
    }
}

/// The strongest effect grants allow on c: none when they allow nothing.
pub(crate) fn strongest(grants: &[Grant], c: &ConnectionRef) -> Option<Effect> {
    STRONGEST_FIRST
        .into_iter()
        .find(|e| grant::decide(grants, &action_on(c, *e)) == Decision::Allow)
}

/// The strongest effect a chain may take on a connection, and whether only its capabilities give it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Access {
    /// None when the chain may do nothing on the connection.
    pub effect: Option<Effect>,
    /// The effect comes from the capability, not from a grant.
    pub implied: bool,
}

/// The strongest effect the chain holds on c, from what its capabilities imply and what is stored.
pub(crate) fn access_on(capabilities: &[String], c: &ConnectionRef, stored: &[Grant]) -> Access {
    let held = strongest(&grants_on(capabilities, c, stored), c);
    Access {
        effect: held,
        implied: held.is_some() && strongest(stored, c) < held,
    }
}

impl Access {
    /// What a chain may do on c, as an agent reads it in list_sources.
    pub(crate) fn words(&self, c: &ConnectionRef) -> String {
        match self.effect {
            None => format!("none ({})", none_because(c)),
            Some(e) if self.implied => format!("{e} (implied by the {} capability)", c.capability),
            Some(e) => format!("{e} (granted by the owner)"),
        }
    }
}

/// Why a chain may do nothing on c.
pub(crate) fn none_because(c: &ConnectionRef) -> String {
    if c.protected {
        return "protected: the Chair asks the owner for a grant with request_grant".to_owned();
    }
    format!("this chain holds no capability of {}", c.source)
}

/// What a Chair's request_grant puts to the owner: one effect on one connection for the chain, in a form.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrantAsk {
    /// The chain that asks.
    pub chain: String,
    /// The repository the grant is for, by its catalog id, for the steps of its release (ADR-0116); empty: the grant is
    /// the chain's own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repository: String,
    pub source: String,
    pub connection: String,
    pub effect: String,
    /// Empty: the typed tools; env: the run tool.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub form: String,
    pub reason: String,
}

impl GrantAsk {
    /// The record a pending grant is about, as the change item names it: the same chain, source and connection are the
    /// same record, whatever effect is asked.
    pub fn name(&self) -> String {
        if self.repository.is_empty() {
            format!("{}/{}/{}", self.chain, self.source, self.connection)
        } else {
            format!("repository:{}/{}/{}", self.repository, self.source, self.connection)
        }
    }

    /// Who holds the grant once it is given: the repository it names, else the chain that asks.
    pub fn holder(&self) -> model::GrantHolder {
        if self.repository.is_empty() {
            model::GrantHolder::chain(&self.chain)
        } else {
            model::GrantHolder {
                kind: model::HolderKind::Repository,
                id: self.repository.clone(),
            }
        }
    }
}

/// The one field of a grant that moves a rail: the effect the chain holds on the connection, and the one it would hold.
/// A grant applies only while the effect held is the one it was asked against.
pub fn grant_field(held: Option<Effect>, asked: Effect) -> Vec<ChangedField> {
    vec![ChangedField {
        name: "effect".to_owned(),
        before: held.map(|e| format!("{:?}", e.as_str())).unwrap_or_default(),
        after: format!("{:?}", asked.as_str()),
    }]
}
