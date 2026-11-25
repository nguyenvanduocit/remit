use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Time};

text_enum! {
    /// Which record a Change is of: the records the owner has agents write for him.
    #[derive(Default)]
    pub enum ChangeKind {
        #[default]
        Agent = "agent",
        Skill = "skill",
        Playbook = "playbook",
        /// A chain's grant on a connection (ADR-0018), named "chain/source/connection": the owner's answer stores the
        /// grant and a Change of this kind records who asked and who allowed it.
        Grant = "grant",
        /// An event signal's settings, named by the signal's name. Only history carries it: what moved a rail on a signal
        /// was its pull, which is a sensor since ADR-0047.
        EventSignal = "event_signal",
        /// A standing sensor (ADR-0047), named by the sensor's name: one that reads a source, a connection, a query or a
        /// script it did not read before is a standing use of a credential or of the host, so the owner's answer stores it.
        Sensor = "sensor",
        /// A source the owner applies as a program, named by its id: the program, the commands it may run, the addresses
        /// it may reach and the variables of its credential are a standing use of a credential, so the owner's answer
        /// stores them.
        Extension = "extension",
        /// An objective the Assistant drafted (ADR-0031), named by its id: applying it opens the work.
        Objective = "objective",
        /// A hire, a retirement or a move onto a newer template (ADR-0032), named by the employee's id: a hire creates a
        /// principal with a ceiling, so the owner's answer stores it.
        Employee = "employee",
        /// Memories of an employee to forget, named by the employee's id: only the owner's answer forgets them.
        Forget = "forget",
        /// A memory of an employee to share with every employee (the global scope), named by the employee's id: only the
        /// owner's answer promotes it.
        Promote = "promote",
        /// A capability a running chain asks for (ADR-0028), named "chain/capability": the owner's answer gives it,
        /// through the chain's own writer, and the chain's decisions record who asked and who allowed it.
        Capability = "capability",
        /// A project (ADR-0108), named by its id: the Assistant writes it on the owner's words, and no answer waits, as
        /// a project moves no rail.
        Project = "project",
        /// A key result that refines another (ADR-0115), named "objective/key result": whoever answers for its parent
        /// writes it, and no answer waits, as it changes how a result is reached and not what was promised.
        KeyResult = "key_result",
    }
}

text_enum! {
    /// What a write did to its record.
    #[derive(Default)]
    pub enum ChangeAction {
        #[default]
        Created = "created",
        Changed = "changed",
        Deleted = "deleted",
    }
}

/// One top-level field of a record that a write made different: what it was and what it became, each as compact JSON,
/// empty when the field was not there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChangedField {
    pub name: String,
    pub before: String,
    pub after: String,
}

/// One write of an agent, a skill or a playbook: the record before and after, as JSON ("" when it did not exist yet, or
/// does not any more), and the fields that differ. It is stored in the transaction that stores the record, so no record
/// changes without one, and it is never changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    pub kind: ChangeKind,
    /// The record's name, or its id for an agent or a playbook.
    pub name: String,
    pub action: ChangeAction,
    pub before: String,
    pub after: String,
    pub fields: Vec<ChangedField>,
    /// Who wrote it: the Chair that held the capability, or Remit itself when a new version replaced a built-in.
    pub by: Actor,
    /// Who let it through, when it moved a rail and waited for the owner's answer; none for any other write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<Actor>,
    pub at: Time,
}

/// What a `change` inbox item asks the owner to allow: the record as it would be written (JSON), and the fields that
/// move a rail with what they are and would become. It is kept with the item and never shown as it is: the item's detail
/// says it in words. Applying it takes the record only if the stored one still has the fields as they were.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PendingChange {
    pub kind: ChangeKind,
    pub name: String,
    pub record: String,
    pub fields: Vec<ChangedField>,
}
