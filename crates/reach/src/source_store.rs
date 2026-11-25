//! What the sources read the owner's credentials through (`source::Store`): the connections he made, and the values of one
//! connection with its secrets opened. It is the only reader of a sealed value.

use std::collections::BTreeMap;
use wire::app::SourceVarState;

use model::InboxItem;
use source::{BoxError, BoxFuture, ConnectionInfo};
use store::SourceValue;

use crate::Reach;
use crate::credentials::{credential_item, secret_aad};
use crate::sources::Bound;
use chains::sealing::Unseal;
use chains::{Error, Result};

/// The owner's stored credentials, as the sources read them.
pub(crate) struct SourceStore(pub(crate) Bound);

impl source::Store for SourceStore {
    fn connections<'a>(&'a self, src: &'a str) -> BoxFuture<'a, std::result::Result<Vec<ConnectionInfo>, BoxError>> {
        Box::pin(async move {
            let o = self.0.get()?;
            let src = src.to_owned();
            let rows = o.store.read(move |r| r.source_connections(&src)).await?;
            Ok(rows
                .into_iter()
                .map(|c| ConnectionInfo {
                    name: c.name,
                    description: c.description,
                })
                .collect())
        })
    }

    fn values<'a>(
        &'a self,
        src: &'a str,
        connection: &'a str,
    ) -> BoxFuture<'a, std::result::Result<BTreeMap<String, String>, BoxError>> {
        Box::pin(async move { Ok(self.0.get()?.connection_values(src, connection).await?) })
    }
}

impl Reach {
    /// The values of a connection with its secrets opened. A secret that does not open under the key that sealed it is
    /// lost: leaving it marked as entered would fail every call on the connection, for ever, with nobody asked, so it is
    /// cleared, the owner is asked again, and this call fails once. A key that is missing or another clears nothing: the
    /// value opens again once its key is back (`unseal`).
    pub(crate) async fn connection_values(&self, src: &str, connection: &str) -> Result<BTreeMap<String, String>> {
        let (s, c) = (src.to_owned(), connection.to_owned());
        let stored = self.store.read(move |r| r.source_values(&s, &c)).await?;
        let mut out = BTreeMap::new();
        for (name, v) in stored {
            let sealed = match v {
                SourceValue::Plain(plain) => {
                    out.insert(name, plain);
                    continue;
                }
                SourceValue::Secret(sealed) => sealed,
            };
            match self.unseal(&secret_aad(src, connection, &name), &sealed) {
                Ok(plain) => {
                    let plain = String::from_utf8(plain).map_err(|_| {
                        Error::refused(format!("{src}/{connection}: the credential for {name} is not text"))
                    })?;
                    out.insert(name, plain);
                }
                Err(Unseal::Damaged(why)) => {
                    if let Err(e) = self.forget_connection(src, connection).await {
                        return Err(Error::refused(format!(
                            "{src}/{connection} {name} {why}; and clearing it failed: {e}"
                        )));
                    }
                    return Err(Error::refused(format!(
                        "{src}/{connection} {name} {why}: the credentials of the connection were cleared and the owner is asked again in his inbox"
                    )));
                }
                Err(Unseal::Refused(why)) => {
                    return Err(Error::refused(format!(
                        "{src}/{connection}: the credential for {name} {why}"
                    )));
                }
            }
        }
        Ok(out)
    }

    /// Clears every sealed value of a connection and asks the owner for them again, in one transaction.
    pub(crate) async fn forget_connection(&self, src: &str, connection: &str) -> Result<()> {
        let assistant = self.the_assistant()?;
        let (sv, mut cv) = self
            .sources
            .blank(src)
            .ok_or_else(|| Error::refused(format!("no source {src:?}")))?;
        let (s, c, now) = (src.to_owned(), connection.to_owned(), self.now());
        let asked = self
            .store
            .update(move |tx| -> Result<Vec<InboxItem>> {
                let stored = tx.source_values(&s, &c)?;
                // What the connection knows now, for the items to say where a secret goes: the plain values it keeps.
                for v in &mut cv.vars {
                    if let Some(SourceValue::Plain(plain)) = stored.get(&v.name) {
                        (v.state, v.value) = (SourceVarState::Set, plain.clone());
                    }
                }
                let mut asked = Vec::new();
                for v in &cv.vars {
                    if !matches!(stored.get(&v.name), Some(SourceValue::Secret(_))) {
                        continue;
                    }
                    tx.delete_source_value(&s, &c, &v.name)?;
                    asked.push(tx.put_inbox(credential_item(&assistant, now, &sv, &c, &cv, v))?);
                }
                Ok(asked)
            })
            .await?;
        self.sources.invalidate(src, connection);
        self.tell_inbox(&asked);
        Ok(())
    }
}
