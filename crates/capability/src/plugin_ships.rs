//! What a plugin that ships with Remit ships besides its tools (ADR-0037) is made with what the code ships: its agent
//! templates by `seed_agents` (agents.rs). They are built in like the rest: Remit brings them up to what ships at each
//! start until the owner changes them, and his version is kept. Its event signals, their sensors and their waits are
//! read from each bundled plugin's own describe: `seed_event_signals` (event_signal_keys.rs) seeds them, paused when the
//! plugin ships them paused, with `seed_sensor` (sensor_settings.rs); `ShippedSignals::ships` (ingest.rs) says which
//! plugin ships a signal.

use model::{Agent, GROUP_GENERAL, Loadout, Title};

use crate::agents::default_agents;

/// The built-in agents of the code and the templates of the bundled plugins.
pub(crate) fn all_shipped_agents() -> Vec<Agent> {
    let mut all = default_agents();
    all.extend(plugin_agents());
    all
}

/// The agent templates the bundled plugins ship: workers that sign no gate, in the general group unless one names its own.
fn plugin_agents() -> Vec<Agent> {
    bundled::agents()
        .into_iter()
        .map(|shipped| {
            let a = shipped.item;
            Agent {
                id: a.id,
                name: a.name,
                title: Some(Title::Worker),
                description: a.description,
                group: if a.group.is_empty() {
                    GROUP_GENERAL.to_owned()
                } else {
                    a.group
                },
                loadout: Loadout {
                    instructions: a.instructions,
                    ..Loadout::default()
                },
                ..Agent::default()
            }
        })
        .collect()
}
