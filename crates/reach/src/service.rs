//! The reach crate's service (ADR-0096): the core it runs on, and what only this crate holds. It reads as
//! the core does (`Deref`), so its code says `self.store` as the core's does.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use chains::Core;
use chains::request::Service;

pub struct Reach {
    pub core: Arc<Core>,
    /// The service itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
}

impl Reach {
    /// The service over core.
    pub fn new(core: Arc<Core>) -> Arc<Self> {
        Arc::new_cyclic(|me| Self { core, me: me.clone() })
    }

    /// The service as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Reach {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Reach {
    fn core(&self) -> &Core {
        &self.core
    }
}
