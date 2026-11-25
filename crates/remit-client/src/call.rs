//! A call named by its method and its args as JSON, typed by the contract it is of: what does not decode is
//! `INVALID_ARGUMENT`, and a changing call carries a `Request`, the one it came with (a retry, or a replay checked by
//! hand) or one made for it. No call is written by hand here: the contract's `Call` enum and `CALLS` list decide.

use link::{CallKind, CallSpec, ErrorCode, Failure, Outgoing, Payload, Request};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Makes the request ids of the calls that came without one: random per run, then a count, so no two repeat.
pub struct Salt {
    salt: String,
    next: u64,
}

impl Salt {
    pub fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        Self {
            salt: format!("rc-{:x}-{nanos:x}-", std::process::id()),
            next: 0,
        }
    }

    pub fn request(&mut self) -> Request {
        let id = format!("{}{}", self.salt, self.next);
        self.next = self.next.wrapping_add(1);
        Request { id }
    }
}

impl Default for Salt {
    fn default() -> Self {
        Self::new()
    }
}

/// The call `method` with `args` (an object, or null for none) of the contract whose calls are `calls`, ready to send,
/// a changing one with `request` or one `salt` makes. Pure but for that request.
pub fn outgoing<C: DeserializeOwned>(
    calls: &[CallSpec],
    method: &str,
    args: Value,
    request: Option<Request>,
    salt: &mut Salt,
) -> Result<Outgoing<C, Payload>, Failure> {
    let kind = calls
        .iter()
        .find(|spec| spec.name == method)
        .map(|spec| spec.kind)
        .ok_or_else(|| Failure::new(ErrorCode::InvalidArgument, format!("no call is named {method:?}")))?;
    let mut args = match args {
        Value::Object(args) => args,
        Value::Null => serde_json::Map::new(),
        _ => return Err(Failure::new(ErrorCode::InvalidArgument, "a call's args are an object")),
    };
    if kind == CallKind::Change {
        let request = request.unwrap_or_else(|| salt.request());
        args.insert(
            "request".to_owned(),
            serde_json::to_value(request).map_err(|e| Failure::new(ErrorCode::InternalError, e.to_string()))?,
        );
    }
    let body = serde_json::json!({ "method": method, "args": args });
    let call =
        serde_json::from_value::<C>(body).map_err(|e| Failure::new(ErrorCode::InvalidArgument, e.to_string()))?;
    Ok(Outgoing::new(call))
}
