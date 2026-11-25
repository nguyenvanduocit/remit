//! The tools a chain's capabilities give its cards (ADR-0031): each capability's tools are its domain's, and this is
//! where they are put together for one card, since a chain's capabilities span every domain.

use crate::orch::Orchestrator;
use chains::cap_names::CAP_AGENTS;
use chains::cap_names::CAP_COACHING;
use chains::cap_names::CAP_DUELS;
use chains::cap_names::CAP_EVENTS;
use chains::cap_names::CAP_EXTENSIONS;
use chains::cap_names::CAP_FEEDBACK;
use chains::cap_names::CAP_HISTORY;
use chains::cap_names::CAP_ISSUES;
use chains::cap_names::CAP_JUDGMENT;
use chains::cap_names::CAP_LIBRARY;
use chains::cap_names::CAP_MEMORY_KEEPING;
use chains::cap_names::CAP_OBJECTIVES;
use chains::cap_names::CAP_OUTCOMES;
use chains::cap_names::CAP_PLAYBOOKS;
use chains::cap_names::CAP_PROPOSE;
use chains::cap_names::CAP_SEEDING;
use chains::cap_names::CAP_SIGNALS;
use chains::cap_names::CAP_SOURCES;
use chains::tools::Effect;
use chains::tools::SelfCard;
use chains::tools::Tools;
use model::Actor;
use model::Card;
use model::Chain;
use model::DecisionKind;
use model::Origin;
use std::sync::Arc;

impl Orchestrator {
    /// Gives the Chair the tools of the capabilities that concern the work it answers for (ADR-0031).
    fn add_employee_capability_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, ch: &Chain) {
        if ch.capabilities.iter().any(|c| c == CAP_OBJECTIVES) {
            self.crew.add_objective_chair_tools(t, card);
        }
    }

    /// Gives a card the tools of its chain's capabilities: those every agent holds (the reads, and `ask_decider`, a write), the rest to the Chair.
    pub(crate) async fn add_capability_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, ch: &Chain, me: &SelfCard) {
        let has = |id: &str| ch.capabilities.iter().any(|c| c == id);
        self.add_every_agent_capability_tools(t, card, ch);
        if has(CAP_DUELS) {
            self.crew.add_duel_tools(t, card, me);
        }
        if card.origin != Origin::Chair {
            return;
        }
        if has(CAP_COACHING) {
            self.crew.add_coach_chair_tools(t, me);
        }
        if has(CAP_MEMORY_KEEPING) {
            self.crew.add_librarian_tools(t, me);
        }
        self.capabilities.add_capability_giving_tools(t, me);
        self.capabilities.add_playbook_read_tools(t); // a playbook is reference every Chair may read (ADR-0113)
        if has(CAP_SIGNALS) {
            let own = pipeline::subscription_tools::own_subscription_of(&ch.id);
            self.pipeline.add_inspect_tools(t, std::sync::Arc::clone(&own));
            self.pipeline.add_chain_dry_run_tool(t, own);
            self.pipeline.add_standing_signal_tools(t, &model::by_agent(&card.id));
        }
        if has(CAP_OUTCOMES) {
            self.crew.add_insight_tool(t, me);
        }
        if has(CAP_SOURCES) {
            self.reach.add_source_request_tool(t);
            self.reach.add_grant_request_tool(t, me);
        }
        if has(CAP_EVENTS) {
            self.pipeline.add_emit_tool(t, ch, &card.id);
        }
        if has(CAP_SEEDING) {
            self.shipping.add_plant_tool(t, me);
        }
        self.add_employee_capability_tools(t, card, ch);
        if has(CAP_EXTENSIONS) {
            self.reach.add_extension_tool(t, &card.id);
            self.reach.add_extension_removal_tool(t, &card.id);
        }
        if has(CAP_PROPOSE) {
            self.add_propose_tool(t, me);
        }
        self.add_authoring_tools(t, ch, me);
    }

    /// Gives the Chair propose_chain, a chain for Remit to work on by itself, and refine_key_result, the key results
    /// such chains serve.
    fn add_propose_tool(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let m = me.clone();
        t.serving(self).add(
            "propose_chain",
            Effect::Writes,
            "Queue a new work or chore chain for Remit to work on by itself, serving one key result. The scheduler starts it when the limit has room; a key result runs as many chains at once as its share (1, growing with each of its landings that holds), and one whose last 3 chains made no progress takes none until the Assistant lifts it. A key result that is met, retired, or already has a chain queued is refused, and the refusal names that chain: message its Chair instead. For a key result whose check is a command, set attempt to open competing attempts instead, one call each, up to its share: each works its own way, their approved versions wait, and a judge runs the check on each and picks the one that lands. Name a playbook that fits in its goal (list_playbooks), and its Chair reads it. Read the reply: it lists what else refines the same key result and the chains serving each.",
            move |o, i: crate::propose::ProposeInput| {
                let m = m.clone();
                async move {
                    let card = m.read(&o).await?;
                    let (c, siblings) = o.propose(&card, i).await?;
                    let serves = c
                        .serves
                        .as_ref()
                        .map(|s| format!(", serving key result {}", s.key_result))
                        .unwrap_or_default();
                    Ok(format!("Queued chain {}: {}{serves}.\n\n{siblings}", c.id, c.title).trim_end().to_owned())
                }
            },
        );
        self.crew.add_key_result_chair_tools(t, me);
    }

    /// Gives the Chair the tools that write playbooks, agent definitions and skills, for the capabilities its chain holds.
    fn add_authoring_tools(self: &Arc<Self>, t: &mut Tools, ch: &Chain, me: &SelfCard) {
        let has = |id: &str| ch.capabilities.iter().any(|c| c == id);
        if has(CAP_PLAYBOOKS) || has(CAP_AGENTS) || has(CAP_LIBRARY) {
            self.capabilities.add_change_read_tools(t);
        }
        if has(CAP_PLAYBOOKS) {
            self.capabilities.add_playbook_tools(t, me.clone());
        }
        if has(CAP_AGENTS) {
            self.capabilities
                .add_agent_write_tools(t, &capability::Capabilities::author_of(me.clone(), DecisionKind::Agent));
        }
        if has(CAP_LIBRARY) {
            self.capabilities.add_library_write_tools(
                t,
                &capability::Capabilities::author_of(me.clone(), DecisionKind::Library),
            );
            self.capabilities.add_importable_tool(t);
        }
    }

    /// Gives the tools chain ch's capabilities give every agent in it, card or a seat of it: the reads and its sources,
    /// whose calls the grant gate decides as card's, and `ask_decider`, which records a judgment and spends on the
    /// owner's account, so its capability is the owner's to give (`writes`) though every agent of the chain then holds it.
    pub(crate) fn add_every_agent_capability_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, ch: &Chain) {
        let has = |id: &str| ch.capabilities.iter().any(|c| c == id);
        if has(CAP_HISTORY) {
            self.capabilities.add_history_tools(t);
        }
        if has(CAP_FEEDBACK) {
            self.crew.add_feedback_read_tool(t);
        }
        if has(CAP_ISSUES) {
            self.pipeline.add_issues_tool(t);
        }
        if has(CAP_JUDGMENT) {
            self.add_decider_tool(t, card);
        }
        if has(CAP_OUTCOMES) {
            self.crew.add_outcome_read_tools(t);
        }
        if has(CAP_SIGNALS) {
            self.pipeline.add_signal_tools(t, ch);
        }
        self.reach.add_source_tools(t, card, ch);
        if has(CAP_COACHING) {
            self.crew.add_coach_read_tools(t);
        }
    }

    /// Gives the Assistant the tools for every way in that the owner opens for an outside caller: an event signal for a
    /// system that tells Remit what happened (and the sensors that read the ones that cannot), a client key for an agent
    /// that acts, and the mentors with the codes they work from.
    pub(crate) fn add_entrance_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        self.pipeline.add_event_signal_tools(t, by);
        self.pipeline.add_sensor_tools(t, by);
        self.keys.add_client_key_tools(t, by);
        self.crew.add_mentor_tools(t, by);
        self.keys.add_worker_key_tools(t, by);
    }
}
