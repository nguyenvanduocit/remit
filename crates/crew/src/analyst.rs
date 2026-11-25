//! The analyst tells the owner how well the agents do, as a whole. It is made of what Remit already has: an analyst
//! chain (ADR-0113) whose Chair lives by subscribe, a fresh agent hired for each run, and a capability (outcomes) whose
//! reading tools every agent of the chain has and whose recording tool is the Chair's. What the Chair records is a
//! decision of kind insight: the chain shows it like any decision, and the Outcomes page lists it.

use std::sync::Arc;

use model::{Actor, Card, Chain, ChainKind, ChainOrigin, ChainStatus, Decision, DecisionKind, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Crew;
use chains::apply::Records;
use chains::chain_core::gist;
use chains::chain_core::new_id;
use chains::lifecycle::NewChain;
use chains::outbox::owner_message;
use chains::request::Request;
use chains::request::Service;
use chains::tools::NoInput;
use chains::tools::{Effect, SelfCard, Tools, as_json};
use chains::{Error, Result};

/// What the analyst's chain is for.
const ANALYST_GOAL: &str = "Watch how well the agents do, as a whole, and tell the owner what to change. Run an analysis now, then check back every day.";

/// What the owner's Analyze now says to the analyst's Chair.
const ANALYSIS_NOW: &str = "Run an analysis now, as on a check-back, then check back as usual.";

/// How many insights list_insights gives: far more than a few weeks of them.
const INSIGHTS_LISTED: i64 = 30;

/// The longest a headline is, in characters.
const MAX_HEADLINE: usize = 200;

/// How long the same headline is refused: the owner is not told the same thing twice in a week.
const INSIGHT_REPEATS: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// Whether a chain is an analyst: its kind says so. A chain given outcomes reads the figures and is no analyst.
pub(crate) fn is_analyst(c: &Chain) -> bool {
    c.kind == ChainKind::Analyst
}

/// One insight as list_insights shows it.
#[derive(Debug, Serialize)]
struct InsightRecord {
    at: Time,
    chain: String,
    headline: String,
    body: String,
}

/// What record_insight takes.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct InsightInput {
    /// One sentence with the figure and which way it moved, under 200 characters.
    #[serde(default)]
    pub headline: String,
    /// Markdown: the evidence (KPI, window, n of of), the cause (chain ids ch_… and a short quote), the suggested action,
    /// the confidence.
    #[serde(default)]
    pub body: String,
}

/// The chain ids text names: ch_ and twelve lowercase hex digits.
fn chain_ids_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("ch_") {
        let after = rest.get(at.saturating_add(3)..).unwrap_or_default();
        let hex = after
            .bytes()
            .take(12)
            .take_while(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
            .count();
        if hex == 12 {
            out.push(format!("ch_{}", after.get(..12).unwrap_or_default()));
            rest = after.get(12..).unwrap_or_default();
        } else {
            rest = after;
        }
    }
    out
}

impl Crew {
    /// The owner asking for an analysis now, through a channel.
    pub async fn run_analyst_now(self: &Arc<Self>, req: Request) -> Result<Chain> {
        let by = req.by.clone();
        self.act(req, "RunAnalyst", move |o| async move { o.run_analyst(&by).await })
            .await
    }

    /// The owner asking for an analysis now: the analyst is started when none is open, and one open is asked to run now
    /// (one at a time). It is Remit's own chain, but the owner starts it, and its Chair keeps it going by checking back.
    pub async fn run_analyst(&self, by: &Actor) -> Result<Chain> {
        let chains = self.store.read(|r| r.chains()).await?;
        if let Some(c) = chains
            .into_iter()
            .find(|c| is_analyst(c) && *c.status() != ChainStatus::Done)
        {
            self.ask_for_analysis(&c, by).await?;
            return Ok(c);
        }
        // one at a time, held in the transaction that queues it: of two asks at once, one starts it
        let one = |tx: &store::Tx<'_>| -> Result<()> {
            if tx
                .chains()?
                .iter()
                .any(|c| is_analyst(c) && *c.status() != ChainStatus::Done)
            {
                return Err(Error::refused("an analyst started meanwhile: it runs the analysis"));
            }
            Ok(())
        };
        let ch = self
            .queue_chain_if(
                NewChain {
                    goal: ANALYST_GOAL.to_owned(),
                    kind: ChainKind::Analyst,
                    origin: ChainOrigin::Owner,
                    by: by.clone(),
                    ..NewChain::default()
                },
                one,
            )
            .await?;
        self.kick();
        Ok(ch)
    }

    /// Asks analyst c's Chair to run an analysis now, as the owner's message, which wakes a waiting Chair. A queued
    /// analyst is not asked: it runs one when it starts.
    async fn ask_for_analysis(&self, c: &Chain, by: &Actor) -> Result<()> {
        if *c.status() == ChainStatus::Queued {
            return Ok(());
        }
        let chair_id = c.chair_id.clone();
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        self.send(
            &chair,
            owner_message(&chair.id, "Your message", ANALYSIS_NOW, by.clone()),
            Records::default(),
        )
        .await?;
        Ok(())
    }

    /// What every agent of an analyst chain reads with: outcome_metrics and list_insights, on t.
    pub fn add_outcome_read_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "outcome_metrics", Effect::Reads,
            "How well the agents are doing, as a whole: every KPI with its definition, the last 7 days against the 7 days before and all time, what that move reads as, a weekly point for each of the last 13 weeks, and tables by role, by model and by playbook. The figures the owner's Outcomes page shows. Read only.",
            |o, _: NoInput| async move {
                let r = o.outcome_report().await?;
                let whole = serde_json::to_string_pretty(&r)?;
                Ok(format!("THE KPIS IN WORDS\n{}\nTHE WHOLE REPORT\n{whole}", outcomes::describe(&r)))
            },
        );
        t.serving(self).add(
            "list_insights", Effect::Reads,
            "What the owner has already been told: the latest insights across analyst chains, newest first, with their date, headline and body. Read it before you write, so nothing is said twice.",
            |o, _: NoInput| async move {
                let ds = o.store.read(|r| r.recent_decisions(&DecisionKind::Insight, INSIGHTS_LISTED)).await?;
                let out: Vec<InsightRecord> = ds
                    .into_iter()
                    .map(|d| InsightRecord { at: d.created_at, chain: d.chain_id, headline: d.text, body: d.why })
                    .collect();
                as_json(&out)
            },
        );
    }

    /// Records what the Chair accepted from the analyst, as a decision of the chain the owner reads at once. Two rails
    /// keep it honest: every chain it names exists, and the owner is not told the same thing twice in a week.
    pub(crate) async fn record_insight(&self, chair: &Card, i: InsightInput) -> Result<()> {
        let (headline, body) = (i.headline.trim().to_owned(), i.body.trim().to_owned());
        if headline.is_empty() {
            return Err(Error::refused(
                "an insight needs a headline: one sentence with the figure and which way it moved",
            ));
        }
        if headline.chars().count() > MAX_HEADLINE {
            return Err(Error::refused(format!(
                "the headline is one sentence, under {MAX_HEADLINE} characters; the evidence goes in the body"
            )));
        }
        if body.is_empty() {
            return Err(Error::refused(
                "an insight needs a body: the evidence, the cause, the suggested action and the confidence",
            ));
        }
        for id in chain_ids_in(&format!("{headline} {body}")) {
            let key = id.clone();
            if self
                .store
                .read(move |r| r.chain(&key))
                .await
                .map_err(|e: store::Error| e)
                .is_err()
            {
                return Err(Error::refused(format!(
                    "the insight names chain {id}, which does not exist: cite only chains list_chains shows"
                )));
            }
        }
        let now = self.now();
        let recent = self
            .store
            .read(|r| r.recent_decisions(&DecisionKind::Insight, INSIGHTS_LISTED))
            .await?;
        if recent.iter().any(|d| {
            d.text.trim().to_lowercase() == headline.to_lowercase()
                && chains::health::elapsed(d.created_at, now) < INSIGHT_REPEATS
        }) {
            return Err(Error::refused(
                "the owner was told this within the week: record it again only when its figures moved, and say how in the headline",
            ));
        }
        let d = Decision {
            id: new_id("d_"),
            chain_id: chair.home.clone(),
            kind: DecisionKind::Insight,
            by: Some(model::by_agent(&chair.id)),
            text: headline,
            why: body,
            card_id: chair.id.clone(),
            created_at: now,
            ..Decision::default()
        };
        let put = d.clone();
        self.store.update(move |tx| tx.put_decision(&put)).await?;
        self.notify.decision_added(&d);
        self.notify.insight_added(&d);
        Ok(())
    }

    /// record_insight, the Chair's: what the owner reads is decided by the Chair, after judging the analyst's report.
    pub fn add_insight_tool(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let me = me.clone();
        t.serving(self).add(
            "record_insight", Effect::Writes,
            "Record one insight for the owner, who reads it on the Outcomes page. Copy the headline and the body from the analyst's report once you accepted it. Refused when it names a chain that does not exist, or when the owner was told the same headline within the week.",
            move |o, i: InsightInput| {
                let me = me.clone();
                async move {
                    let chair = me.read(&o).await?;
                    let said = gist(&i.headline, 80);
                    o.record_insight(&chair, i).await?;
                    Ok(format!("Recorded: {said}"))
                }
            },
        );
    }
}
