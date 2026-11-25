//! The keys crate's service (ADR-0096): the core it runs on, and what only this crate holds. It reads as
//! the core does (`Deref`), so its code says `self.store` as the core's does.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use chains::Core;
use chains::request::Service;

pub struct Keys {
    pub core: Arc<Core>,
    /// The service itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
    /// Makes a client key's chain starts one at a time, so two cannot take the last of its day (client_tools.rs).
    pub client_mu: tokio::sync::Mutex<()>,
}

impl Keys {
    /// The service over core.
    pub fn new(core: Arc<Core>) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            core,
            me: me.clone(),
            client_mu: tokio::sync::Mutex::new(()),
        })
    }

    /// The service as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Keys {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Keys {
    fn core(&self) -> &Core {
        &self.core
    }
}
