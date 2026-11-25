//! The capability crate's service (ADR-0096): the core it runs on, and what only this crate holds. It reads as
//! the core does (`Deref`), so its code says `self.store` as the core's does.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use chains::Core;
use chains::request::Service;

pub struct Capabilities {
    pub core: Arc<Core>,
    /// The service itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
    /// The channels, told of what this crate changes too.
    pub notify: Arc<dyn crate::notify::Notifier>,
    /// The renderings sessions load skills from, and the lock that orders the skills' changes (library.rs).
    pub library: crate::library::LibraryState,
}

impl Capabilities {
    /// The service over core, with notify.
    pub fn new(core: Arc<Core>, notify: Arc<dyn crate::notify::Notifier>) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            core,
            me: me.clone(),
            notify,
            library: crate::library::LibraryState::default(),
        })
    }

    /// The service as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Capabilities {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Capabilities {
    fn core(&self) -> &Core {
        &self.core
    }
}
