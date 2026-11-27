//! Grants are the shell around the rule of grant_access.rs: they load what the rule reads (a chain's capabilities and
//! stored grants, whether a connection is protected), answer the gate of the source crate as its Policy, and carry a
//! Chair's request for a grant the owner must give (ADR-0018). Who calls is named to the gate by the card's server
//! (sources.rs `add_source_tools`), never by the call's own input.

use std::collections::BTreeMap;
use wire::app::ToolSource;

use grant::{Decision, Effect, Grant, Refusal};
use model::{
    Card, ChangeKind, Control, GrantHolder, InboxItem, InboxKind, InboxOption, InboxStatus, KIND_POLL, OwnerKind,
    PLUGIN_ACTION_PREFIX, PendingChange, SourceConnection,
};
use schemars::JsonSchema;
use serde::Deserialize;
use source::{Action, BoxError, BoxFuture, Caller, DEFAULT_CONNECTION, Form, RUN_TOOL, SCRIPT_ENFORCED, SCRIPT_TOOL};
use store::Tx;

use crate::Reach;
use crate::credentials::connection_name;
use crate::grant_access::{
    ConnectionRef, GrantAsk, access_on, decide_on, decide_program, grant_field, none_because, program_words, strongest,
};
use crate::sources::Bound;
use chains::chain_core::gist;
use chains::chain_core::new_id;
use chains::change_item::{CHANGE_APPLY, CHANGE_NOT_NOW, code_span};
use chains::changes::{Authorship, record_change};
use chains::ingest::PROCESSOR_AGENT;
use chains::ingest::SENSOR_AGENT;
use chains::owner_acts::{ACT_APPLY, for_owner};
use chains::plugin_accounts::ACCOUNT_AGENT;
use chains::{Error, Result};

/// What decides a chain's access: the capabilities it holds and the grants stored for it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChainAccess {
    pub capabilities: Vec<String>,
    pub stored: Vec<Grant>,
}

/// The stored grants as the rule reads them. The store holds only effects the rule knows (its CHECK), so one that does not
/// parse is dropped: it grants nothing.
pub(crate) fn grants_of(rows: &[model::Grant]) -> Vec<Grant> {
    rows.iter()
        .filter_map(|r| {
            r.effect.parse::<Effect>().ok().map(|effect| Grant {
                source: r.source.clone(),
                connection: r.connection.clone(),
                effect,
            })
        })
        .collect()
}

/// The decision for an action in a form. The form decides how near the credential comes to the agent's code, so it decides
/// what grants count.
pub(crate) fn decide_as(
    form: Form,
    capabilities: &[String],
    c: &ConnectionRef,
    stored: &[Grant],
    e: Effect,
) -> Decision {
    match form {
        Form::Tool => decide_on(capabilities, c, stored, e),
        // A script behind the proxy holds no credential and the proxy confines it to reads in code, so it is a read like a
        // typed tool's: the capability implies it, and a protected connection needs a stored read grant. A grant is stored
        // without a form, so this form asks for none of its own (`grant_form` takes none for it).
        Form::Proxy => decide_on(capabilities, c, stored, e),
        Form::Env => decide_program(capabilities, c, stored, e),
    }
}

/// What a chain holds on c in a form.
fn words_as(form: Form, capabilities: &[String], c: &ConnectionRef, stored: &[Grant]) -> String {
    if form == Form::Env {
        return program_words(capabilities, c, stored);
    }
    access_on(capabilities, c, stored).words(c)
}

/// What a chain may do with the script tool on c, as an agent reads it in list_sources: the rule's own answer for the
/// form, and the enforcement the form has.
fn script_words(capabilities: &[String], c: &ConnectionRef, stored: &[Grant]) -> String {
    if decide_as(Form::Proxy, capabilities, c, stored, Effect::Read) == Decision::Allow {
        return format!("script allowed (the read above covers it; {SCRIPT_ENFORCED})");
    }
    format!(
        "script none (it needs a read on the connection, and {})",
        none_because(c)
    )
}

/// Whether a source has a connection of that name: `default` always does.
pub fn connection_in(stored: &[SourceConnection], name: &str) -> bool {
    name == DEFAULT_CONNECTION || stored.iter().any(|c| c.name == name)
}

/// The connections a source has, for an error that tells an agent which exist.
pub fn connection_names(stored: &[SourceConnection]) -> String {
    let mut names = vec![DEFAULT_CONNECTION];
    names.extend(
        stored
            .iter()
            .filter(|c| c.name != DEFAULT_CONNECTION)
            .map(|c| c.name.as_str()),
    );
    names.join(", ")
}

/// The form a request names: empty is the typed tools. A program only reads: no write program exists.
fn grant_form(name: &str, effect: Effect) -> Result<Form> {
    match name {
        "" | "tool" => Ok(Form::Tool),
        "env" if effect != Effect::Read => Err(Error::refused("run only reads: ask for effect read with form env")),
        "env" => Ok(Form::Env),
        "proxy" => Err(Error::refused(
            "the script tool needs no grant of its own: the read on the connection covers it, so ask for effect read with no form (list_sources shows whether this chain holds it)",
        )),
        other => Err(Error::refused(format!(
            "form {other:?} is not one of: empty (the typed tools), env (run)"
        ))),
    }
}

/// The form as a grant ask records it: empty for the typed tools.
fn grant_form_name(form: Form) -> String {
    if form == Form::Tool {
        String::new()
    } else {
        form.as_str().to_owned()
    }
}

/// The line the owner reads in his inbox for a grant request.
fn grant_title(by: &str, ask: &GrantAsk) -> String {
    if !ask.repository.is_empty() {
        return format!(
            "{by} asks for {} on {}/{} for the releases of {}",
            ask.effect, ask.source, ask.connection, ask.repository
        );
    }
    if ask.form == Form::Env.as_str() {
        return format!("{by} asks to run programs with {}/{}", ask.source, ask.connection);
    }
    format!("{by} asks for {} on {}/{}", ask.effect, ask.source, ask.connection)
}

/// What the owner reads under a grant request: who asks, for which chain, what and why. The reason is the Chair's words,
/// shown as such.
fn grant_detail(by: &str, chain_title: &str, ask: &GrantAsk, protected: bool) -> String {
    let env = ask.form == Form::Env.as_str();
    let mut b = if !ask.repository.is_empty() {
        format!(
            "{by}, the Chair of the chain {}, asks for **{}** on the {} connection {} for the **releases of the repository {}**: \
             the steps its `.remit/release/` declares reach the connection through a proxy that adds the credential, and \
             never see it. A release runs on every landed commit of the repository, so the grant is the repository's, \
             not the chain's. Irreversible lets the steps write (publish); anything less lets them read.",
            code_span(chain_title),
            ask.effect,
            code_span(&ask.source),
            code_span(&ask.connection),
            code_span(&ask.repository)
        )
    } else if env {
        format!(
            "{by}, the Chair of the chain {}, asks to **run programs** with the credential of the {} connection {}. The program holds the credential in its environment and the agent never sees it; Remit points its HTTP clients at a gate that reaches only that connection's address, but the program can connect around it. What keeps the program from writing is the credential's own rights, not Remit's code: **Apply only if this credential is read-only.**",
            code_span(chain_title),
            code_span(&ask.source),
            code_span(&ask.connection)
        )
    } else {
        format!(
            "{by}, the Chair of the chain {}, asks for **{}** on the {} connection {}.",
            code_span(chain_title),
            ask.effect,
            code_span(&ask.source),
            code_span(&ask.connection)
        )
    };
    if protected {
        b.push_str(" You marked this connection protected: no chain reads it without a grant from you.");
    }
    b.push_str(&format!(
        "\n\nWhy, in the Chair's words: {}",
        code_span(&gist(&ask.reason, 400))
    ));
    if ask.repository.is_empty() {
        b.push_str("\n\nApply gives this chain the grant: every call it makes on the connection then passes the gate with it. Not now gives nothing, and the Chair is told.");
    } else {
        b.push_str("\n\nApply gives the repository the grant: every release of it reaches the connection with it. Not now gives nothing, the release that needs it waits, and the Chair is told.");
    }
    if env {
        b.push_str(" The grant is a read on the connection; it lets the chain use the run tool, and the typed tools of the source as well.");
    }
    b
}

/// What a Chair asks for: an effect on one connection of one source, for its chain.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GrantInput {
    /// the id of the source, as list_sources names it (mongo, bytebase, grafana...)
    pub source: String,
    /// the name of the connection, as list_sources names it; empty is the default connection
    #[serde(default)]
    pub connection: String,
    /// read, reversible, compensable or irreversible: how far the act can be undone
    pub effect: String,
    /// empty for the typed tools of the source; env to run programs with the run tool, which hold the connection's
    /// credential in their environment (a read only)
    #[serde(default)]
    pub form: String,
    /// why the chain needs it, in a sentence the owner reads under the request
    pub reason: String,
    /// the repository (as open_repo named it, a repository this chain has open) whose releases need the grant: the
    /// steps of its .remit/release/ reach the connection; empty for a grant of this chain's own
    #[serde(default)]
    pub repository: String,
}

/// What the owner tells the Assistant about a connection.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetConnectionProtectedInput {
    /// the id of the source, as list_sources names it
    pub source: String,
    /// the name of the connection; empty is the default connection
    #[serde(default)]
    pub connection: String,
    /// true: no chain reads the connection without a grant the owner gave; false: chains that hold the
    /// source's capability read it again
    pub protected: bool,
}

/// The Policy of the gate: it asks the rule what the calling chain may do on the connection a call names. The callers in
/// no chain it answers are Remit's own (the poller, a processor, a session's opener), each read only for what stands for
/// it. Any other call that names no chain holds no grant and is refused: the Assistant holds no source tool, so such a
/// caller is one whose identity was lost on the way, and letting it read would hide that loss and open a protected
/// connection to it.
pub(crate) struct GrantPolicy(pub(crate) Bound);

impl source::Policy for GrantPolicy {
    fn decide<'a>(&'a self, who: &'a Caller, a: &'a Action) -> BoxFuture<'a, std::result::Result<Decision, BoxError>> {
        Box::pin(async move { Ok(self.0.get()?.policy_decision(who, a).await?) })
    }
}

/// Whether who is Remit's own caller named agent, in no chain.
fn remit_caller(who: &Caller, agent: &str) -> bool {
    who.chain.is_empty() && who.agent == agent
}

impl Reach {
    /// The gate's decision for who taking action a.
    pub async fn policy_decision(&self, who: &Caller, a: &Action) -> Result<Decision> {
        if remit_caller(who, SENSOR_AGENT) {
            return self.sensor_verdict(a).await;
        }
        if remit_caller(who, PROCESSOR_AGENT) {
            return self.processor_verdict(a).await;
        }
        if remit_caller(who, ACCOUNT_AGENT) {
            return Ok(self.account_verdict(a));
        }
        if who.chain.is_empty() {
            return Ok(Decision::Deny {
                reason: "the call names no chain, and only a chain holds grants".to_owned(),
                refusal: Refusal::Rail,
            });
        }
        let held = self.access_of_chain(&who.chain).await?;
        let refs = self
            .connection_refs(&a.source, std::slice::from_ref(&a.connection))
            .await?;
        let c = refs
            .first()
            .ok_or_else(|| Error::refused("no connection was asked about"))?;
        Ok(decide_as(a.form, &held.capabilities, c, &held.stored, a.effect))
    }

    /// The gate's verdict for a call of Remit's poller: a read, by a typed call, of exactly the source and connection that
    /// a live poll names. A poll is set by the owner, or let through by him when anyone else asks, so the poller reads
    /// nothing he did not approve.
    async fn sensor_verdict(&self, a: &Action) -> Result<Decision> {
        if a.effect != Effect::Read || a.form != Form::Tool {
            return Ok(Decision::Deny {
                reason: "Remit's poller only reads, by a typed call".to_owned(),
                refusal: Refusal::Rail,
            });
        }
        let all = self.store.read(|r| r.sensors()).await?;
        let plugin_kind = format!("{PLUGIN_ACTION_PREFIX}{}.", a.source);
        let reads = all.iter().filter(|s| !s.paused).any(|s| {
            s.kind == KIND_POLL && s.source == a.source && s.connection == a.connection
                // a plugin's sensor reads the connection its settings name
                || s.kind.starts_with(&plugin_kind)
                    && connection_of_settings(&s.settings.as_ref().map(model::ForeignJson::text).unwrap_or_default()) == a.connection
        });
        if reads {
            return Ok(Decision::Allow);
        }
        Ok(Decision::Deny {
            reason: format!("no sensor polls {}/{}", a.source, a.connection),
            refusal: Refusal::NoGrant,
        })
    }

    /// The gate's verdict for a plugin's processor: lent its connection by a typed call, when a subscription of Remit's
    /// that is active runs a processor of that plugin on that connection.
    async fn processor_verdict(&self, a: &Action) -> Result<Decision> {
        if a.form != Form::Tool {
            return Ok(Decision::Deny {
                reason: "a plugin's processor is lent its connection by a typed call".to_owned(),
                refusal: Refusal::Rail,
            });
        }
        let subs = self.store.read(|r| r.subscriptions()).await?;
        let plugin_kind = format!("{PLUGIN_ACTION_PREFIX}{}.", a.source);
        if subs.iter().any(|s| {
            s.control == Control::Active
                && s.owner.kind == OwnerKind::Remit
                && s.action.kind.as_str().starts_with(&plugin_kind)
                && connection_of_settings(&s.action.params) == a.connection
        }) {
            return Ok(Decision::Allow);
        }
        Ok(Decision::Deny {
            reason: format!("no subscription runs a processor of {} on {}", a.source, a.connection),
            refusal: Refusal::NoGrant,
        })
    }

    /// The capabilities chain chain_id holds and the grants stored for it.
    pub(crate) async fn access_of_chain(&self, chain_id: &str) -> Result<ChainAccess> {
        let id = chain_id.to_owned();
        let (ch, rows) = self
            .store
            .read(move |r| -> store::Result<_> { Ok((r.chain(&id)?, r.grants(&GrantHolder::chain(&id))?)) })
            .await
            .map_err(|e| Error::refused(format!("chain {chain_id}: {e}")))?;
        Ok(ChainAccess {
            capabilities: ch.capabilities,
            stored: grants_of(&rows),
        })
    }

    /// The connections the rule needs to know of a source: its capability, and which are protected. The names are the
    /// ones asked about; none is looked up that the source does not have.
    pub(crate) async fn connection_refs(&self, src: &str, names: &[String]) -> Result<Vec<ConnectionRef>> {
        // reads nothing: no value is opened to know a source's capability
        let Some((sv, _)) = self.sources.blank(src) else {
            return Err(Error::refused(format!(
                "no source {src:?}: the ones that exist are {}",
                self.sources.source_ids().join(", ")
            )));
        };
        let s = src.to_owned();
        let protected = self.store.read(move |r| r.protected_connections(&s)).await?;
        Ok(names
            .iter()
            .map(|name| ConnectionRef {
                source: src.to_owned(),
                name: name.clone(),
                capability: sv.capability.clone(),
                protected: protected.contains(name),
            })
            .collect())
    }

    /// Asks for the grant the Chair of chain card.home needs, and says in words what came of it. A read the chain's
    /// capabilities already imply, or a grant it already holds, raises nothing. A write, or a read of a protected
    /// connection, is the owner's to give: one change item asks him, and asking again while it waits raises no second.
    /// What the chain's capabilities do not reach is refused. It takes names, never a value.
    pub(crate) async fn request_grant(&self, card: &Card, input: GrantInput) -> Result<String> {
        let connection = connection_name(&input.connection)?;
        let effect: Effect = input
            .effect
            .parse()
            .map_err(|e: grant::UnknownEffect| Error::refused(e.to_string()))?;
        let form = grant_form(&input.form, effect)?;
        let reason = input.reason.trim().to_owned();
        if reason.is_empty() {
            return Err(Error::refused("give the reason: the owner reads it under the request"));
        }
        let refs = self
            .connection_refs(&input.source, std::slice::from_ref(&connection))
            .await?; // it says which sources exist
        let src = input.source.clone();
        let conns = self.store.read(move |r| r.source_connections(&src)).await?;
        if !connection_in(&conns, &connection) {
            return Err(Error::refused(format!(
                "{} has no connection {connection:?}: the ones that exist are {}",
                input.source,
                connection_names(&conns)
            )));
        }
        let c = refs
            .into_iter()
            .next()
            .ok_or_else(|| Error::refused("no connection was asked about"))?;
        if !input.repository.trim().is_empty() {
            return self
                .request_repository_grant(card, &input, &c, effect, form, reason)
                .await;
        }
        let held = self.access_of_chain(&card.home).await?;
        let place = c.place();
        if !held.capabilities.contains(&c.capability) {
            return Err(Error::refused(format!(
                "this chain holds no {} capability, so it holds no tool of {} and a grant on {place} is not the Chair's to ask: ask for the capability first with request_capability",
                c.capability, c.source
            )));
        }
        let d = decide_as(form, &held.capabilities, &c, &held.stored, effect);
        if d == Decision::Allow {
            return Ok(format!(
                "Nothing to ask: the chain already holds {effect} on {place}, {}.",
                words_as(form, &held.capabilities, &c, &held.stored)
            ));
        }
        if c.protected && form == Form::Env {
            return Err(Error::refused(format!("{place}: {}", d.reason()))); // no one grants it: the owner marked it protected
        }
        let g = Grant {
            source: c.source.clone(),
            connection: c.name.clone(),
            effect,
        };
        if !grant::needs_owner(&g, c.protected) && form != Form::Env {
            return Err(Error::refused(format!(
                "a grant of {effect} on {place} is not the Chair's to widen: {}",
                d.reason()
            )));
        }
        let ask = GrantAsk {
            chain: card.home.clone(),
            repository: String::new(),
            source: c.source.clone(),
            connection: c.name.clone(),
            effect: effect.as_str().to_owned(),
            form: grant_form_name(form),
            reason,
        };
        let item = self
            .raise_grant(card, &ask, effect, &c, strongest(&held.stored, &c))
            .await?;
        Ok(format!(
            "It waits in the owner's inbox ({}), where the Assistant decides it for him: Apply or Not now, and a message starting [change result tells you which. Until then do not ask again and do not work around it: finish_chain waits for that answer, so use subscribe to come back later.",
            item.title
        ))
    }

    /// Asks for the grant the steps of a repository's release need on connection c (ADR-0116), for the Chair card of a
    /// chain that has the repository open: any effect is the owner's to give, so the Assistant decides it for him, an
    /// irreversible one after the overturn window. A grant the repository already holds raises nothing.
    async fn request_repository_grant(
        &self,
        card: &Card,
        input: &GrantInput,
        c: &ConnectionRef,
        effect: Effect,
        form: Form,
        reason: String,
    ) -> Result<String> {
        let repository = input.repository.trim().to_owned();
        if form != Form::Tool {
            return Err(Error::refused(
                "a release step reaches its connections through the proxy: ask with no form",
            ));
        }
        let id = card.home.clone();
        let chain = self.store.read(move |r| r.chain(&id)).await?;
        if !chain.repos.iter().any(|rw| rw.repo == repository) {
            return Err(Error::refused(format!(
                "this chain has no repository {repository:?} open: name it as open_repo returned it ({})",
                chain
                    .repos
                    .iter()
                    .map(|rw| rw.repo.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let ask = GrantAsk {
            chain: card.home.clone(),
            repository,
            source: c.source.clone(),
            connection: c.name.clone(),
            effect: effect.as_str().to_owned(),
            form: String::new(),
            reason,
        };
        let holder = ask.holder();
        let rows = self.store.read(move |r| r.grants(&holder)).await?;
        let held = strongest(&grants_of(&rows), c);
        if held.is_some_and(|h| h >= effect) {
            return Ok(format!(
                "Nothing to ask: repository {} already holds {} on {}.",
                ask.repository,
                held.map_or("", Effect::as_str),
                c.place()
            ));
        }
        let item = self.raise_grant(card, &ask, effect, c, held).await?;
        Ok(format!(
            "It waits in the owner's inbox ({}), where the Assistant decides it for him; an irreversible grant takes effect an hour after it applies it, unless he overturns it. A release that needs it waits meanwhile, and you work on: do not ask again.",
            item.title
        ))
    }

    /// Opens the change item that asks the owner for the grant, as a change to the chain's grant on that connection.
    async fn raise_grant(
        &self,
        card: &Card,
        ask: &GrantAsk,
        asked: Effect,
        c: &ConnectionRef,
        held: Option<Effect>,
    ) -> Result<InboxItem> {
        let id = ask.chain.clone();
        let chain = self.store.read(move |r| r.chain(&id)).await?;
        let item = InboxItem {
            id: new_id("i_"),
            home: card.home.clone(),
            card_id: card.id.clone(),
            kind: InboxKind::Change,
            title: grant_title(&card.name, ask),
            detail: grant_detail(&card.name, &chain.title, ask, c.protected),
            options: vec![
                InboxOption {
                    id: CHANGE_APPLY.to_owned(),
                    name: "Apply".to_owned(),
                    ..InboxOption::default()
                },
                InboxOption {
                    id: CHANGE_NOT_NOW.to_owned(),
                    name: "Not now".to_owned(),
                    ..InboxOption::default()
                },
            ],
            status: InboxStatus::Open,
            created_at: self.now(),
            change: Some(PendingChange {
                kind: ChangeKind::Grant,
                name: ask.name(),
                record: serde_json::to_string(ask)?,
                fields: grant_field(held, asked),
            }),
            ..InboxItem::default()
        };
        self.raise_change(item).await
    }

    /// Stores the grant a change item asks for, as the owner who answered it gave it, if the chain, the connection and the
    /// effect the chain holds on it are still what they were when the owner was asked (true, moved, when they are not, and
    /// nothing is stored).
    pub fn apply_grant(&self, tx: &Tx<'_>, pending: &PendingChange, w: &Authorship) -> Result<bool> {
        let approved = match &w.approved_by {
            Some(by) if for_owner(ACT_APPLY, by).is_ok() => by.clone(),
            _ => return Err(Error::refused(ACT_APPLY.refusal())),
        };
        let ask: GrantAsk = serde_json::from_str(&pending.record)?;
        let effect: Effect = ask
            .effect
            .parse()
            .map_err(|e: grant::UnknownEffect| Error::refused(e.to_string()))?;
        match tx.chain(&ask.chain) {
            Err(store::Error::NotFound) => return Ok(true),
            other => other?,
        };
        if !connection_in(&tx.source_connections(&ask.source)?, &ask.connection) {
            return Ok(true);
        }
        let rows = tx.grants(&ask.holder())?;
        let c = ConnectionRef {
            source: ask.source.clone(),
            name: ask.connection.clone(),
            ..ConnectionRef::default()
        };
        if grant_field(strongest(&grants_of(&rows), &c), effect) != pending.fields {
            return Ok(true);
        }
        // the owner was told the connection was not protected
        if ask.form == Form::Env.as_str() && tx.protected_connections(&ask.source)?.contains(&ask.connection) {
            return Ok(true);
        }
        let given = model::Grant {
            holder: ask.holder(),
            source: ask.source.clone(),
            connection: ask.connection.clone(),
            effect: ask.effect.clone(),
            granted_by: approved,
            granted_at: self.now(),
        };
        let before = rows
            .iter()
            .rfind(|r| r.source == ask.source && r.connection == ask.connection);
        // The grant is written with the record of who asked and who allowed it, as every write that moves a rail is.
        record_change(tx, w, self.now(), ChangeKind::Grant, &ask.name(), before, Some(&given))?;
        tx.put_grant(&given)?;
        Ok(false)
    }

    /// What list_sources says of each connection beyond its state, by "<source>/<name>": that the owner marked it protected
    /// and, to the agent of a chain (who), what the chain may do on it. The words come from the rule the gate asks.
    pub(crate) async fn connection_notes(
        &self,
        views: &[ToolSource],
        who: &Caller,
    ) -> Result<BTreeMap<String, String>> {
        let held = if who.chain.is_empty() {
            None
        } else {
            Some(self.access_of_chain(&who.chain).await?)
        };
        let mut notes = BTreeMap::new();
        for v in views {
            let names: Vec<String> = v.connections.iter().map(|c| c.name.clone()).collect();
            for c in self.connection_refs(&v.id, &names).await? {
                let mut parts = Vec::new();
                if c.protected {
                    parts.push("protected".to_owned());
                }
                if let Some(h) = &held {
                    parts.push(format!(
                        "this chain may: {}",
                        access_on(&h.capabilities, &c, &h.stored).words(&c)
                    ));
                    if offers_tool(v, RUN_TOOL) {
                        parts.push(program_words(&h.capabilities, &c, &h.stored));
                    }
                    if offers_tool(v, SCRIPT_TOOL) {
                        parts.push(script_words(&h.capabilities, &c, &h.stored));
                    }
                }
                notes.insert(c.place(), parts.join("; "));
            }
        }
        Ok(notes)
    }

    /// Marks a connection protected or clears the mark, for the owner. A protected connection is read only by a chain
    /// holding a grant for it; the read a capability implies stops there, for a chain at work too, because the gate asks
    /// at every call. It returns the connection's name.
    pub async fn set_connection_protected(&self, input: SetConnectionProtectedInput) -> Result<String> {
        let connection = connection_name(&input.connection)?;
        self.connection_refs(&input.source, &[]).await?; // says which sources exist
        let (src, conn, protected) = (input.source.clone(), connection.clone(), input.protected);
        self.store
            .update(move |tx| -> Result<()> {
                let conns = tx.source_connections(&src)?;
                if !connection_in(&conns, &conn) {
                    return Err(Error::refused(format!(
                        "{src} has no connection {conn:?}: the ones that exist are {}",
                        connection_names(&conns)
                    )));
                }
                Ok(tx.set_connection_protected(&src, &conn, protected)?)
            })
            .await?;
        self.tell_source(&input.source).await;
        Ok(connection)
    }
}

/// Whether a source's tools include the tool of that name (run, script).
fn offers_tool(v: &ToolSource, name: &str) -> bool {
    v.tools.iter().any(|t| t.name == name)
}

/// The connection a plugin's sensor reads, or its processor acts, through: its settings' `connection`, else `default`.
pub fn connection_of_settings(settings: &str) -> String {
    serde_json::from_str::<serde_json::Value>(settings)
        .ok()
        .and_then(|v| v.get("connection").and_then(|c| c.as_str()).map(str::to_owned))
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| DEFAULT_CONNECTION.to_owned())
}
