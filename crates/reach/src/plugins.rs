//! Where the plugins run, as Remit needs them: the plugins that ship inside Remit are called in-process (ADR-0072, decision 7),
//! each granted `forge.read` through the one forge service (forge_service.rs); an applied extension is a `RemotePlugin`, a
//! child process that starts only with the hash the owner applied. The gate of the source crate reaches both through
//! `PluginCalls`, the `source::Plugins` it is given.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use bundled::FORGE_PLUGIN;
use plugin::{RemotePlugin, Spec};
use plugin_sdk::{Forge, Plugin};
use source::{BoxError, BoxFuture, Extension, PluginRef, PollItem, PollResult};
use wire::plugin::PollAsk;

use crate::Reach;
use crate::extensions::manifest_of;
use crate::sources::Bound;
use chains::chain_core::new_id;
use chains::ports::{Bundled, PluginHost};

/// The plugins that ship inside Remit, each built once; the forge plugin is granted `forge.read` through `forge`.
pub struct BundledPlugins(BTreeMap<String, Arc<dyn Plugin>>);

impl BundledPlugins {
    /// Every bundled plugin on the clock and HTTP client of `shared`, the forge plugin granted forge: what it asks of a forge is
    /// answered by Remit's forge service, which answers only for what an unpaused sensor's settings name.
    #[must_use]
    pub fn new(forge: &Arc<dyn Forge>, shared: &source::Shared) -> BundledPlugins {
        BundledPlugins(
            bundled::plugins(shared)
                .into_iter()
                .map(|(id, mut p)| {
                    // only the forge plugin asks a forge anything: no other is granted forge.read
                    if id == FORGE_PLUGIN {
                        p.grant_forge(Arc::clone(forge));
                    }
                    let p: Arc<dyn Plugin> = Arc::new(p);
                    (id.to_owned(), p)
                })
                .collect(),
        )
    }
}

impl Bundled for BundledPlugins {
    fn lookup(&self, id: &str) -> Option<Arc<dyn Plugin>> {
        self.0.get(id).cloned()
    }

    fn ids(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
}

/// The processes of the applied extensions, one per id: a spec with another hash or file replaces the one that ran, so a
/// program runs only with the hash the owner applied last.
#[derive(Default)]
pub struct PluginProcesses(Mutex<HashMap<String, (Spec, Arc<RemotePlugin>)>>);

impl PluginProcesses {
    fn take(&self, id: Option<&str>) -> Vec<Arc<RemotePlugin>> {
        let mut running = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match id {
            Some(id) => running.remove(id).into_iter().map(|(_, p)| p).collect(),
            None => running.drain().map(|(_, (_, p))| p).collect(),
        }
    }
}

/// Ends the processes of ps, on the runtime when there is one; without one, dropping a RemotePlugin kills its process.
fn stop_all(ps: Vec<Arc<RemotePlugin>>) {
    if let Ok(rt) = tokio::runtime::Handle::try_current() {
        for p in ps {
            rt.spawn(async move { p.stop().await });
        }
    }
}

impl PluginHost for PluginProcesses {
    fn plugin(&self, spec: &Spec) -> Arc<dyn Plugin> {
        let mut running = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, p)) = running.get(&spec.id).filter(|(s, _)| s == spec) {
            let p: Arc<dyn Plugin> = Arc::<RemotePlugin>::clone(p);
            return p;
        }
        let fresh = Arc::new(RemotePlugin::new(spec.clone()));
        if let Some((_, old)) = running.insert(spec.id.clone(), (spec.clone(), Arc::clone(&fresh))) {
            drop(running);
            stop_all(vec![old]);
        }
        fresh
    }

    fn stop(&self, id: &str) {
        stop_all(self.take(Some(id)));
    }

    fn close(&self) {
        stop_all(self.take(None));
    }
}

/// The `source::Plugins` the gate calls an extension's tools and polls through, once the gate let the call by.
pub(crate) struct PluginCalls(pub(crate) Bound);

impl source::Plugins for PluginCalls {
    fn call<'a>(
        &'a self,
        p: &'a PluginRef,
        tool: &'a str,
        arguments: &'a str,
        vars: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, std::result::Result<String, BoxError>> {
        Box::pin(async move {
            let o = self.0.get()?;
            let plugin = o.plugin_of_ref(p)?;
            let arguments = if arguments.trim().is_empty() {
                serde_json::Value::Object(serde_json::Map::new())
            } else {
                serde_json::from_str(arguments)?
            };
            let request = link::Request { id: new_id("rq_") };
            Ok(plugin.call(request, tool.to_owned(), arguments, vars.clone()).await?)
        })
    }

    fn poll<'a>(
        &'a self,
        p: &'a PluginRef,
        query: &'a str,
        cursor: &'a str,
        limit: usize,
        vars: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, std::result::Result<PollResult, BoxError>> {
        Box::pin(async move {
            let o = self.0.get()?;
            let plugin = o.plugin_of_ref(p)?;
            let ask = PollAsk {
                query: query.to_owned(),
                cursor: cursor.to_owned(),
                limit: i64::try_from(limit)?,
                vars: vars.clone(),
            };
            let answer = plugin.poll(ask).await?;
            Ok(PollResult {
                cursor: answer.cursor,
                items: answer
                    .items
                    .into_iter()
                    .map(|it| PollItem {
                        id: it.id,
                        version: it.version,
                        body: it.body.0.to_string(),
                    })
                    .collect(),
            })
        })
    }
}

impl Reach {
    /// The manifests of the plugins that ship with Remit, read from the plugins themselves. One that does not validate is
    /// logged and left out.
    pub(crate) async fn bundled_extensions(&self) -> Vec<Extension> {
        let mut out = Vec::new();
        for id in self.bundled.ids() {
            let Some(p) = self.bundled.lookup(&id) else { continue };
            let manifest = match p.describe().await {
                Ok(m) => m,
                Err(err) => {
                    tracing::error!(id, %err, "a bundled plugin did not describe itself");
                    continue;
                }
            };
            let mut ext = manifest_of(&id, &manifest);
            ext.bundled = true;
            if let Err(err) = ext.validate() {
                tracing::error!(id, %err, "a bundled plugin's manifest is not accepted");
                continue;
            }
            out.push(ext);
        }
        out
    }
}
