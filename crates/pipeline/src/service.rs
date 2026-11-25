//! The signal pipeline's service (ADR-0096): the core it runs on, and what only this crate holds. It reads as
//! the core does (`Deref`), so its code says `self.store` as the core's does.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use chains::Core;
use chains::request::Service;

pub struct Pipeline {
    pub core: Arc<Core>,
    /// The service itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
    /// The channels, told of what this crate changes too.
    pub notify: Arc<dyn crate::notify::Notifier>,
    /// What the signal pipeline holds in memory (signals.rs).
    pub signals: crate::signals::SignalState,
    /// Customer issues for the issues capability (jira.rs); none when not configured.
    pub jira: Option<Arc<crate::jira::Jira>>,
    /// What a firing does in another domain (actions.rs): the application, which holds this service.
    pub actions: Weak<dyn crate::actions::Actions>,
}

impl Pipeline {
    /// The service over core, with notify, jira, actions.
    pub fn new(
        core: Arc<Core>,
        notify: Arc<dyn crate::notify::Notifier>,
        jira: Option<Arc<crate::jira::Jira>>,
        actions: Weak<dyn crate::actions::Actions>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            core,
            me: me.clone(),
            notify,
            signals: crate::signals::SignalState::default(),
            jira,
            actions,
        })
    }

    /// What a firing does in another domain; `Stopping` once the application is dropped.
    pub fn actions(&self) -> chains::Result<Arc<dyn crate::actions::Actions>> {
        self.actions.upgrade().ok_or(chains::Error::Stopping)
    }

    /// The service as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Pipeline {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Pipeline {
    fn core(&self) -> &Core {
        &self.core
    }
}
