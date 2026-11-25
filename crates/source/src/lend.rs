//! A plugin that senses or processes on its own (ADR-0049), or whose connection is a Claude account (ADR-0071), needs the
//! credential variables of the connection it works through, as a tool and a poll of its source have them. `Set::lend` is the
//! one way to them: it passes the same gate (the connection, the policy with the effect, the recorder), and answers where the
//! program is and the variables of the connection, to be handed to the program's call.

use std::collections::BTreeMap;

use grant::Effect;
use serde_json::Value;

use crate::connection::{Resolved, Set, key, no_source};
use crate::error::{Result, fail};
use crate::extension::PluginRef;
use crate::gate::{Caller, Form, Gate, Resolution};
use crate::source::Kind;

// The names a lend goes by where a tool's would be: the Call a Recorder is told of and the gate's errors.

/// The stream of a plugin's sensor kind.
pub const SENSE_CALL: &str = "sense";
/// One firing run through a plugin's processor.
pub const PROCESS_CALL: &str = "process";
/// The environment of a session that opens on the account a plugin's connection is.
pub const ACCOUNT_CALL: &str = "account";

impl Set {
    /// The program of `source` and the variables of its connection, for a call of the given name and effect. It is refused
    /// as any call is: the policy decides it for `caller`, and the recorder is told of it once. `connection` is a name, or
    /// empty for the only configured one.
    pub async fn lend(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        call: &str,
        effect: Effect,
    ) -> Result<(PluginRef, BTreeMap<String, String>)> {
        let Some(k) = self.kind(source) else {
            return Err(no_source(source));
        };
        let gate = Gate {
            source: k.id.clone(),
            how: Resolution::Lend(k, effect),
            set: self.clone(),
        };
        gate.pass(caller, call, connection, &Value::Null, |res| async move {
            match res.lend {
                Some(lent) => Ok(lent),
                None => fail!("{} lends nothing", res.label),
            }
        })
        .await
    }

    /// The connection a lend names, as `resolve_call` finds it for a tool, and what it lends.
    pub(crate) async fn resolve_lend(&self, k: &Kind, effect: Effect, connection: &str) -> Result<Resolved> {
        let name = self.pick(k, connection).await?;
        let p = self.provider(k, &name).await?;
        let Some(lent) = p.lend() else {
            return fail!("{} runs no program on this server", p.name());
        };
        let m = p.missing();
        if !m.is_empty() {
            return fail!(
                "{} is not configured on this server (connection {name}): set {}",
                p.name(),
                m.join(", ")
            );
        }
        let mut res = Resolved::new(key(&k.id, &name), &name, effect, Form::Tool);
        res.lend = Some(lent);
        Ok(res)
    }
}
