//! The crew's service (ADR-0096): the core it runs on, and what only this crate holds. It reads as
//! the core does (`Deref`), so its code says `self.store` as the core's does.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use chains::Core;
use chains::request::Service;

pub struct Crew {
    pub core: Arc<Core>,
    /// The service itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
    /// The channels, told of what this crate changes too.
    pub notify: Arc<dyn crate::notify::Notifier>,
    /// Makes a mentor's acts on a help and its deadline decide one at a time, help by help (help.rs).
    pub help_locks: chains::keyed_locks::KeyedLocks,
    /// The models memory is searched by meaning and judged for relevance with (memory.rs); `NoModels` when the server is
    /// given none, which answers as a server with no key.
    pub models: Arc<dyn crate::memory::MemoryModels>,
    /// One embedding pass at a time (`embed_waiting`), and whether one was asked for while it ran: it runs again.
    pub(crate) embedding: tokio::sync::Mutex<()>,
    pub(crate) embed_again: std::sync::atomic::AtomicBool,
    /// One wake of the librarian at a time (`wake_librarian`), so two teachings recorded together send one message.
    pub(crate) waking_librarian: tokio::sync::Mutex<()>,
    /// The services below the crew it asks: the pipeline records a firing's outcome, capability holds the agent
    /// definitions and the library, keys a mentor's device codes.
    pub pipeline: Arc<pipeline::Pipeline>,
    pub capabilities: Arc<capability::Capabilities>,
    pub keys: Arc<keys::Keys>,
}

impl Crew {
    /// The service over core, with notify, memory's models (none: `NoModels`) and the services below it.
    pub fn new(
        core: Arc<Core>,
        notify: Arc<dyn crate::notify::Notifier>,
        models: Option<Arc<dyn crate::memory::MemoryModels>>,
        below: (Arc<pipeline::Pipeline>, Arc<capability::Capabilities>, Arc<keys::Keys>),
    ) -> Arc<Self> {
        let (pipeline, capabilities, keys) = below;
        Arc::new_cyclic(|me| Self {
            core,
            me: me.clone(),
            notify,
            help_locks: chains::keyed_locks::KeyedLocks::default(),
            models: models.unwrap_or_else(|| Arc::new(crate::memory::NoModels)),
            embedding: tokio::sync::Mutex::new(()),
            embed_again: std::sync::atomic::AtomicBool::new(false),
            waking_librarian: tokio::sync::Mutex::new(()),
            pipeline,
            capabilities,
            keys,
        })
    }

    /// The service as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Crew {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Crew {
    fn core(&self) -> &Core {
        &self.core
    }
}
