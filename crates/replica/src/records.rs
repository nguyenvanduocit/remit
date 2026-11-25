//! A record arrives as JSON: the core reads the facts its rules need from a typed reading of it, and keeps the JSON as
//! it came for the models the page draws (`models.rs`).

use link::{ErrorCode, Failure};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::io::Tab;
use crate::models::Patch;
use crate::shell::{Notice, Ticket};

/// The typed reading of a record or an answer, as the contract defines it.
pub(crate) fn read<T: DeserializeOwned>(value: &Value) -> Result<T, Failure> {
    T::deserialize(value).map_err(|e| Failure::new(ErrorCode::InvalidArgument, format!("does not decode: {e}")))
}

/// A field of an answer, taken out of it.
pub(crate) fn take(value: &mut Value, field: &str) -> Value {
    value.get_mut(field).map(Value::take).unwrap_or(Value::Null)
}

/// The records of a list field, taken out of the answer in its order.
pub(crate) fn take_list(value: &mut Value, field: &str) -> Vec<Value> {
    match take(value, field) {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

/// A push's one argument: the record it carries (`{"push": <name>, "args": {<name>: <record>}}`).
pub(crate) fn push_arg(mut body: Value) -> Value {
    body.get_mut("args")
        .and_then(Value::as_object_mut)
        .and_then(|args| args.values_mut().next())
        .map(Value::take)
        .unwrap_or(Value::Null)
}

/// A JavaScript string's length, in UTF-16 code units: the unit a streamed chunk's offset counts in.
pub(crate) fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Who asked a call the core answers: the tab, and the ticket its answer finds the call by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Asker {
    pub(crate) tab: Tab,
    pub(crate) ticket: Ticket,
}

/// What one input changed: the patches for the shell and what the owner should notice, in order, each with the tab it
/// is for (none: whichever the owner sees).
#[derive(Debug, Default)]
pub(crate) struct Out {
    pub(crate) patches: Vec<Patch>,
    pub(crate) notices: Vec<(Option<Tab>, Notice)>,
}

impl Out {
    /// Something the owner should notice wherever he looks.
    pub(crate) fn notice(&mut self, notice: Notice) {
        self.notices.push((None, notice));
    }

    /// Something only `tab` asked about.
    pub(crate) fn notice_to(&mut self, tab: Tab, notice: Notice) {
        self.notices.push((Some(tab), notice));
    }
}
