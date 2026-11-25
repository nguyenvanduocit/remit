//! The tools of grants (ADR-0018). request_grant is the Chair's, in the sources capability: it names a connection and an
//! effect and never handles a value. set_connection_protected is the Assistant's: the owner marks a connection by telling
//! it.

use std::sync::Arc;

use crate::Reach;
use crate::grants::{GrantInput, SetConnectionProtectedInput};
use chains::tools::{Effect, SelfCard, Tools};

impl Reach {
    /// Gives t request_grant, for the Chair me.
    pub fn add_grant_request_tool(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let me = me.clone();
        t.serving(self).add(
        "request_grant", Effect::Writes,
        "Ask for the right of your chain to act on one connection of a source with an effect (list_sources names the sources and connections, and says what the chain already may do on each). A read of a connection that is not protected, by a chain that holds the source's capability, is already held: nothing is asked. Any effect above read (reversible: can be put back exactly; compensable: another act answers for it; irreversible: nothing puts it back), or a read of a connection the owner marked protected, is given on the owner's side: one request is raised in his inbox, the Assistant decides it for him, and you are told the answer; ask once, then wait. To run programs on a connection with the run tool (a command-line program that holds the connection's credential in its environment, in a jail), ask for effect read with form env: it is always asked for, because only the credential's own rights keep the program from writing, and it is never given on a protected connection; the read a capability implies does not cover it. A source your chain's capabilities do not reach is refused. For a repository's release (ADR-0116), name the repository too (as open_repo returned it): the grant is the repository's, so the steps of its .remit/release/ reach that connection behind Remit's proxy, which adds the credential; a step publishes (writes) only through an irreversible grant. Give it form tool, and ask when your change adds the connection to .remit/release/connections. Give the reason in a sentence read under the request. You name the source, connection and effect, never a value.",
        move |o, i: GrantInput| {
            let me = me.clone();
            async move {
                let card = me.read(&o).await?;
                o.request_grant(&card, i).await
            }
        },
    );
    }

    /// Gives t set_connection_protected.
    pub fn add_connection_protect_tool(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
        "set_connection_protected", Effect::Writes,
        "Mark a connection of a source protected, or clear the mark, on the owner's word. A protected connection (production, usually) is read only by a chain holding a grant for it; every other connection of a source is read by the chains that hold the source's capability. A chain at work loses the read at once. A Chair asks for a grant with request_grant, and you answer it for the owner in his inbox.",
        |o, i: SetConnectionProtectedInput| async move {
            let (source, protected) = (i.source.clone(), i.protected);
            let connection = o.set_connection_protected(i).await?;
            Ok(if protected {
                format!("{source}/{connection} is protected: a chain reads it only through a grant the owner gives.")
            } else {
                format!(
                    "{source}/{connection} is not protected: chains that hold the source's capability read it."
                )
            })
        },
    );
    }
}
