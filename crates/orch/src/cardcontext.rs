//! What one card's agent has in its context, read for the owner on request: what Remit gave its session as it started it,
//! what Claude Code loaded, and what fills its context as Claude Code counts it (`/context`). Each is what was recorded or
//! counted; a part with nothing recorded yet is absent, never filled in.

use model::{Card, ContextCount, ContextGiven, GivenCapability, GivenMissing, SessionLaunch};
use wire::app::CardContext;

use crate::orch::Orchestrator;
use capability::capabilities::capability_of;
use chains::homes::standalone_card;
use chains::session::CANCEL_WAIT;
use chains::{Error, Result};

impl Orchestrator {
    /// Records what Remit starts card c's session `session_id` with: spec as sent, the library's access it was built from
    /// (acc), and the tools Remit's server lists for the card. A resumed session keeps the opening recorded of it; a new
    /// one's is recorded as it is sent (`record_opening`).
    pub(crate) async fn record_launch(
        &self,
        c: &Card,
        spec: &agent::Spec,
        acc: &library::Session,
        session_id: &str,
        resumed: bool,
    ) -> Result<()> {
        let me = self.me().ok_or(Error::Stopping)?;
        let tools = me
            .tools_for(&c.id)
            .await
            .map_err(|e| Error::refused(format!("list the tools of {}: {e}", c.id)))?;
        // the Assistant is in no chain, and has no capability
        let caps = if standalone_card(c) {
            Vec::new()
        } else {
            let home = c.home.clone();
            self.store.read(move |r| r.chain(&home)).await?.capabilities
        };
        let mut l = SessionLaunch {
            card_id: c.id.clone(),
            home: c.home.clone(),
            session_id: session_id.to_owned(),
            at: self.now(),
            resumed,
            given: ContextGiven {
                skills: acc.held_skills.clone(),
                claude_tools: spec.tools.clone(),
                setting_sources: acc.setting_sources.clone(),
                missing: acc
                    .missing
                    .iter()
                    .map(|m| GivenMissing {
                        kind: m.kind.clone(),
                        name: m.name.clone(),
                        why: m.why.clone(),
                    })
                    .collect(),
                tools: tools.given(),
                capabilities: caps
                    .iter()
                    .filter_map(|id| capability_of(id))
                    .map(|cap| GivenCapability {
                        id: cap.id.clone(),
                        label: cap.label.clone(),
                        gives: cap.gives.clone(),
                    })
                    .collect(),
                allowed_tools: spec.allowed_tools.clone(),
                deny_rules: spec.deny_tools.clone(),
                principles: self.cfg.principles.clone(),
                opening: None,
            },
        };
        let id = c.id.clone();
        self.store
            .update(move |tx| {
                if resumed {
                    match tx.session_launch(&l.card_id) {
                        Ok(before) if before.session_id == l.session_id => l.given.opening = before.given.opening,
                        Ok(_) | Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                tx.put_session_launch(&l)
            })
            .await
            .map_err(|e| Error::refused(format!("record the start of {id}: {e}")))
    }

    /// Reads card `card_id`'s context. A part that cannot be read (no init yet, no count yet) is reported as such, never
    /// as the whole read failing.
    pub async fn card_context(&self, card_id: &str) -> Result<CardContext> {
        let id = card_id.to_owned();
        let c: Card = self.store.read(move |r| r.card(&id)).await?;
        let mut out = CardContext {
            card_id: c.id.clone(),
            ..CardContext::default()
        };
        let id = c.id.clone();
        match self.store.read(move |r| r.session_launch(&id)).await {
            Ok(l) if !c.session_id.is_empty() && l.session_id == c.session_id => out.given = Some(l.given),
            Ok(_) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
        let id = c.id.clone();
        match self.store.read(move |r| r.session_init(&id)).await {
            Ok(loaded) if !c.session_id.is_empty() && loaded.init.session_id == c.session_id => {
                out.loaded = Some(loaded)
            }
            Ok(_) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
        match self.count_context(&c).await {
            Some(Ok(context)) => out.context = Some(context),
            counted => {
                if let Some(Err(why)) = counted {
                    out.context_error = why;
                }
                let id = c.id.clone();
                match self.store.read(move |r| r.session_context(&id)).await {
                    Ok(stored) if stored.context.session_id == c.session_id => out.context = Some(stored),
                    Ok(_) | Err(store::Error::NotFound) => {}
                    Err(err) => return Err(err.into()),
                }
            }
        }
        Ok(out)
    }

    /// Has card c's running session count its context now, giving it `CANCEL_WAIT` to answer; none when no session of
    /// it runs.
    async fn count_context(&self, c: &Card) -> Option<std::result::Result<ContextCount, String>> {
        let sess = self
            .run_of(&c.id)
            .filter(|r| r.open())
            .and_then(|r| r.state().sess.clone())?;
        let counted = match tokio::time::timeout(CANCEL_WAIT, sess.context()).await {
            Ok(Ok(context)) => Ok(ContextCount {
                context,
                at: self.now(),
            }),
            Ok(Err(err)) => Err(err.to_string()),
            Err(_) => Err("the agent's process did not answer".to_owned()),
        };
        Some(counted)
    }
}
