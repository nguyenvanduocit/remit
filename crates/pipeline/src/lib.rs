//! The signal pipeline (ADR-0064, ADR-0096): readings to firings to deliveries, subscriptions, sensors, routines, watches,
//! event signals and the board. What a firing does in another domain goes through `Actions`, which `orch` implements.

pub mod action_params;
pub mod actions;
pub mod automation;
pub mod chain_jobs;
pub mod deliver;
pub mod event;
pub mod event_fields;
pub mod event_record;
pub mod event_signal;
pub mod event_signal_keys;
pub mod event_signal_tools;
pub mod forget;
pub mod jira;
pub mod jira_tools;
pub mod notify;
pub mod pack;
pub mod plugin_pipeline;
pub mod polls;
pub mod processing;
pub mod record;
pub mod routine;
pub mod routine_tools;
pub mod sensor;
pub mod sensor_settings;
pub mod sensor_tools;
mod service;
pub mod signal_rows;
pub mod signal_tools;
pub mod signalboard;
pub mod signals;
pub mod snapshot;
pub mod start_event;
pub mod subscription_inspect;
pub mod subscription_tools;
pub mod tell;

pub use service::Pipeline;
