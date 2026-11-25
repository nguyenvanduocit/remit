//! The Judgments page (ADR-0118), laid out here in the owner's words so a channel shows it without deriving any of it:
//! for each question asked, what it is for (`judgment::Definition::title`), which model answers it and why and whether it
//! has settled (one sentence: the figures say nothing twice), its three figures (how often it proved right, how often it
//! decides alone and who decides the rest, what it cost), and its latest answers that proved wrong, each as what happened. What only someone tuning it reads (its key, its threshold, each model's Brier
//! score and AUC) goes under its details. The model that answers and the threshold are read from the question's
//! declaration, the standings and what the owner set (`steering`), as `Core::judge` reads them; what he set is said in
//! words where the figures it changes are, with who set it and why.

use std::collections::HashMap;

use judgment::{Acts, ActsOn, Definition, MIN_OUTCOMES, Scored, Speaks, Standing, Threshold, decider_name};
use model::{ActorKind, Judgment, JudgmentUse, OutcomeSource, Setting, Settings, SubjectKind};
use store::{JudgmentScore, JudgmentTally};
use wire::app::{
    DeciderStanding, JudgmentDetails, JudgmentFact, JudgmentFigure, JudgmentMistake, JudgmentQuestion, Judgments,
};

use crate::Result;
use crate::chain_core::Core;
use crate::judge::{on_trial, ranked_by, scored_by_decider, threshold_of};
use crate::steering;

/// How many of a question's answers that proved wrong the page shows.
const WRONG_SHOWN: i64 = 5;

/// What the page is laid out from, read in one transaction.
struct Read {
    tallies: Vec<JudgmentTally>,
    scores: Vec<JudgmentScore>,
    /// By question.
    wrong: HashMap<String, Vec<Judgment>>,
    /// What the owner set for each question.
    settings: Settings,
}

impl Core {
    /// The Judgments page as stored now.
    pub async fn judgments(&self) -> Result<Judgments> {
        let read = self
            .store
            .read(|r| {
                let tallies = r.judgment_tallies()?;
                let scores = r.judgment_scores(&[])?;
                let mut wrong = HashMap::new();
                for q in questions_of(&tallies) {
                    let w = r.wrong_judgments(&q, WRONG_SHOWN)?;
                    wrong.insert(q, w);
                }
                let settings = r.settings()?;
                Ok::<_, store::Error>(Read {
                    tallies,
                    scores,
                    wrong,
                    settings,
                })
            })
            .await?;
        Ok(lay_out(read))
    }
}

/// The questions tallies count, in their order (the questions asked last first), each once.
fn questions_of(tallies: &[JudgmentTally]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tallies {
        if !out.contains(&t.question) {
            out.push(t.question.clone());
        }
    }
    out
}

/// The page over what was read. Pure.
fn lay_out(mut read: Read) -> Judgments {
    let questions = questions_of(&read.tallies)
        .into_iter()
        .map(|key| {
            let wrong = read.wrong.remove(&key).unwrap_or_default();
            let mine: Vec<&JudgmentTally> = read.tallies.iter().filter(|t| t.question == key).collect();
            question_of(&key, &mine, &read.scores, &read.settings, wrong)
        })
        .collect();
    Judgments { questions }
}

/// What the page needs of a question's declaration and of what the owner set; a question no longer declared (an older
/// version) reads plainly.
struct Declared<'a> {
    title: String,
    acts: Acts,
    /// When it speaks, where it only informs.
    speaks: Speaks,
    hands_to: &'static str,
    /// What the question is held to now: the owner's, else its declaration's.
    target_precision: f64,
    /// The decider the owner pinned, and the bar he set, each with who and why.
    pinned: Option<&'a Setting<String>>,
    bar: Option<&'a Setting<i64>>,
    default_decider: &'static str,
    definition: Option<&'static Definition>,
}

fn declared<'a>(key: &str, tallies: &[&JudgmentTally], settings: &'a Settings) -> Declared<'a> {
    match judgment::definition(key) {
        Some(d) => Declared {
            title: d.title().to_owned(),
            acts: d.acts(),
            speaks: d.speaks(),
            hands_to: d.hands_to(),
            target_precision: steering::target_precision(settings, d),
            pinned: steering::pinned(settings, d),
            bar: settings
                .steering
                .get(d.id())
                .and_then(|s| s.target_precision_percent.as_ref()),
            default_decider: d.default_decider(),
            definition: Some(d),
        },
        None => Declared {
            title: key.to_owned(),
            acts: if tallies
                .iter()
                .any(|t| matches!(t.used, JudgmentUse::Informed | JudgmentUse::Held))
            {
                Acts::Informs
            } else {
                Acts::Alone
            },
            speaks: Speaks::Always,
            hands_to: "its usual rule",
            target_precision: judgment::PROVISIONAL_THRESHOLD,
            pinned: None,
            bar: None,
            default_decider: judgment::DEFAULT_DECIDER,
            definition: None,
        },
    }
}

/// The counts of a question's used answers (no shadow's).
struct Used {
    used: i64,
    acted: i64,
    right: i64,
    wrong: i64,
    answers: i64,
    cost_usd: f64,
}

fn used_of(tallies: &[&JudgmentTally]) -> Used {
    let used: Vec<&&JudgmentTally> = tallies.iter().filter(|t| t.used != JudgmentUse::Shadow).collect();
    Used {
        used: used.iter().map(|t| t.judgments).sum(),
        acted: used
            .iter()
            .filter(|t| t.used == JudgmentUse::Acted)
            .map(|t| t.judgments)
            .sum(),
        right: used.iter().map(|t| t.right).sum(),
        wrong: used.iter().map(|t| t.wrong).sum(),
        answers: tallies.iter().map(|t| t.judgments).sum(),
        cost_usd: tallies.iter().map(|t| t.cost_usd).sum(),
    }
}

fn question_of(
    key: &str,
    tallies: &[&JudgmentTally],
    scores: &[JudgmentScore],
    settings: &Settings,
    wrong: Vec<Judgment>,
) -> JudgmentQuestion {
    let d = declared(key, tallies, settings);
    let scored = scored_by_decider(scores, key);
    let acting = ranked_by(d.default_decider, d.pinned.map(|p| p.value.as_str()), &scored)
        .into_iter()
        .next()
        .unwrap_or_else(|| d.default_decider.to_owned());
    let threshold = threshold_of(scores, key, acts_on_of(&d), &acting, d.target_precision);
    let trial = d
        .pinned
        .and_then(|p| on_trial(Some(&p.value), &scored).map(|pin| (p, scored.get(pin).map_or(0, Vec::len))));
    let u = used_of(tallies);
    let deciders = deciders_of(tallies, &scored, &acting);
    let shadows: Vec<&str> = deciders
        .iter()
        .filter(|s| s.decider != acting)
        .map(|s| s.name.as_str())
        .collect();
    JudgmentQuestion {
        key: key.to_owned(),
        title: d.title.clone(),
        summary: format!("{}{}", settled_words(&d, &u, threshold), bar_words(&d)),
        answered_by: answered_by(&acting, &d, &shadows, trial),
        figures: figures(&d, &u, threshold),
        mistakes: wrong
            .into_iter()
            .map(|j| mistake_of(j, d.definition, d.hands_to))
            .collect(),
        details: JudgmentDetails {
            facts: facts(key, &d, threshold, u.answers),
            deciders,
        },
    }
}

/// n of of as a whole percent, rounded half up; 0 of none.
fn percent(n: i64, of: i64) -> i64 {
    n.saturating_mul(200)
        .saturating_add(of)
        .checked_div(of.saturating_mul(2))
        .unwrap_or(0)
        .clamp(0, 100)
}

/// A probability as a whole percent.
fn chance(p: f64) -> String {
    format!("{:.0}", (p * 100.0).clamp(0.0, 100.0))
}

/// Dollars as the owner reads them: to the cent, and a fraction of one said as such.
fn usd(n: f64) -> String {
    if n <= 0.0 {
        "nothing".to_owned()
    } else if n < 0.005 {
        "under $0.01".to_owned()
    } else {
        format!("${n:.2}")
    }
}

fn checks(n: i64) -> String {
    if n == 1 {
        "1 check".to_owned()
    } else {
        format!("{n} checks")
    }
}

/// The side of its answers a question acts on: either, for one the page does not know.
fn acts_on_of(d: &Declared<'_>) -> ActsOn {
    d.definition.map_or(ActsOn::Either, Definition::acts_on)
}

/// Whether enough of its answers have a known outcome to say how often it is right: as many as a model's own numbers
/// need to count (`MIN_OUTCOMES`).
fn known(u: &Used) -> bool {
    usize::try_from(u.right.saturating_add(u.wrong)).is_ok_and(|n| n >= MIN_OUTCOMES)
}

/// Whether the question has settled, which the three figures do not say: that the level of certainty it decides alone at
/// has been measured (or, for one that only advises, that enough results are known to trust how it does). The figures
/// carry how often it is right, how often it decides alone and what it cost, so none of that is said here.
fn settled_words(d: &Declared<'_>, u: &Used, threshold: Threshold) -> String {
    match (d.acts, threshold) {
        (Acts::Informs, _) if known(u) => {
            let checked = u.right.saturating_add(u.wrong);
            format!(
                "{} it has been right {}% of the {checked} times we could check.",
                advises(d, threshold),
                percent(u.right, checked)
            )
        }
        (Acts::Informs, _) => {
            format!(
                "{} How often it is right is not known until {MIN_OUTCOMES} results are in.",
                advises(d, threshold)
            )
        }
        (Acts::Alone, Threshold::Measured(t)) => format!(
            "Settled: it decides alone when at least {}% sure, a level that has proved right at least {}% of the time.",
            chance(t),
            chance(d.target_precision)
        ),
        (Acts::Alone, Threshold::Provisional) => format!(
            "Not settled yet: until {MIN_OUTCOMES} {} are known it decides alone only when at least {}% sure.",
            acts_on_of(d).results_words(),
            chance(judgment::PROVISIONAL_THRESHOLD)
        ),
        (Acts::Alone, Threshold::Unreached) => format!(
            "Not settled: no level of certainty has proved right {}% of the time, so {} decides every time.",
            chance(d.target_precision),
            d.hands_to
        ),
    }
}

/// How a question that only informs speaks: always, or only when it reads the card as it warns of and is as sure as its
/// bar. Said with the bar, so the sentence stands on its own.
fn advises(d: &Declared<'_>, threshold: Threshold) -> String {
    match (d.speaks, threshold.at()) {
        (Speaks::Always, _) => "It only advises;".to_owned(),
        (_, Some(t)) => format!(
            "It only advises, and speaks to {} only when at least {}% sure;",
            d.hands_to,
            chance(t)
        ),
        (_, None) => format!(
            "It only advises, and says nothing to {}: no level of certainty has proved right {}% of the time;",
            d.hands_to,
            chance(d.target_precision)
        ),
    }
}

/// Who set a value, as the owner reads it: he did, or the Assistant for him.
fn setter(by: ActorKind) -> &'static str {
    match by {
        ActorKind::Assistant => "the Assistant for you",
        _ => "you",
    }
}

/// The reason a value was set, as it was given.
fn reason(why: &str) -> String {
    format!("\u{201c}{}\u{201d}", why.trim())
}

/// What the owner set for the bar a question decides alone at, when he did: said after the sentence that says what the bar
/// is, so the bar and who set it are read together.
fn bar_words(d: &Declared<'_>) -> String {
    d.bar.map_or_else(String::new, |b| {
        format!(
            " The bar of {}% was set by {}: {}.",
            chance(d.target_precision),
            setter(b.by.kind),
            reason(&b.why)
        )
    })
}

/// Which model answers, and why: the one the owner chose, the one its question starts with, or the one that took over by
/// proving more reliable; and the models that answer beside it to be measured.
fn answered_by(acting: &str, d: &Declared<'_>, shadows: &[&str], trial: Option<(&Setting<String>, usize)>) -> String {
    let default = d.default_decider;
    let mut out = if let Some(pin) = d.pinned.filter(|p| p.value == acting) {
        format!(
            "Answered by {}, chosen by {}: {}.",
            decider_name(acting),
            setter(pin.by.kind),
            reason(&pin.why)
        )
    } else if acting == default {
        format!("Answered by {}.", decider_name(acting))
    } else {
        format!(
            "Answered by {} \u{2014} switched from {} because its answers proved more reliable.",
            decider_name(acting),
            decider_name(default)
        )
    };
    if let Some((pin, results)) = trial {
        out.push_str(&format!(
            " You chose {} ({}: {}); it has {results} of the {MIN_OUTCOMES} results it needs, so for now it only informs, and takes over once it has them.",
            decider_name(&pin.value),
            setter(pin.by.kind),
            reason(&pin.why)
        ));
    }
    match shadows {
        [] => {}
        [one] => out.push_str(&format!(
            " {one} answers beside it, to show whether it would do better."
        )),
        [first @ .., last] => out.push_str(&format!(
            " {} and {last} answer beside it, to show whether one would do better.",
            first.join(", ")
        )),
    }
    out
}

fn figures(d: &Declared<'_>, u: &Used, threshold: Threshold) -> Vec<JudgmentFigure> {
    let checked = u.right.saturating_add(u.wrong);
    let right = JudgmentFigure {
        label: "Right".to_owned(),
        hint: format!(
            "Of its answers whose result is known, the share that proved right. It counts once {MIN_OUTCOMES} results are known."
        ),
        value: if known(u) {
            format!("{}%", percent(u.right, checked))
        } else {
            "\u{2014}".to_owned()
        },
        over: if known(u) {
            format!("{} of {checked} checked", u.right)
        } else {
            format!("{checked} checked, too few to tell")
        },
    };
    let alone = JudgmentFigure {
        label: "Decides alone".to_owned(),
        hint: format!(
            "How often its answer was sure enough to act on without {}, who decides the rest.",
            d.hands_to
        ),
        value: match d.acts {
            Acts::Informs => "Never".to_owned(),
            Acts::Alone => format!("{}%", percent(u.acted, u.used)),
        },
        over: match (d.acts, threshold.at()) {
            (Acts::Informs, _) => format!("it advises {}", d.hands_to),
            (Acts::Alone, None) => "no certainty is safe yet".to_owned(),
            (Acts::Alone, Some(_)) => format!("{} of {}", u.acted, u.used),
        },
    };
    let cost = JudgmentFigure {
        label: "Cost".to_owned(),
        hint: "What every model's answers to it cost, those of the models measured beside it included.".to_owned(),
        value: match usd(u.cost_usd).as_str() {
            "nothing" => "$0".to_owned(),
            "under $0.01" => "<$0.01".to_owned(),
            other => other.to_owned(),
        },
        over: checks(u.used),
    };
    vec![right, alone, cost]
}

/// Who said an answer was wrong, as the owner reads it.
const fn who(source: OutcomeSource) -> &'static str {
    match source {
        OutcomeSource::Librarian => "the librarian",
        OutcomeSource::Gate => "the landing",
        OutcomeSource::Owner => "you",
        OutcomeSource::SecondOpinion | OutcomeSource::SecondOpinionTargeted => "a second opinion",
        OutcomeSource::Card => "the card",
        OutcomeSource::Chain => "the chain it started",
    }
}

fn mistake_of(j: Judgment, definition: Option<&'static Definition>, hands_to: &str) -> JudgmentMistake {
    let outcome = j.outcome.unwrap_or_default();
    let said = match definition {
        Some(d) => d.wrong(&j.pick, who(outcome.source)),
        None => format!("answered wrongly; {} said so", who(outcome.source)),
    };
    let mut chars = said.chars();
    let text = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    let p = chance(j.p);
    let sure = match j.used {
        JudgmentUse::Acted => format!("was {p}% sure"),
        JudgmentUse::Escalated => format!("was only {p}% sure, so {hands_to} decided"),
        JudgmentUse::Informed | JudgmentUse::Shadow => format!("was {p}% sure; it only advised"),
        JudgmentUse::Held => format!("was {p}% sure; it said nothing to {hands_to}"),
    };
    JudgmentMistake {
        id: j.id,
        text,
        sure,
        at: outcome.at,
        card: match j.subject.kind {
            SubjectKind::Card => j.subject.id,
            _ => String::new(),
        },
        home: j.subject.home,
    }
}

fn facts(key: &str, d: &Declared<'_>, threshold: Threshold, answers: i64) -> Vec<JudgmentFact> {
    let fact = |label: &str, value: String| JudgmentFact {
        label: label.to_owned(),
        value,
    };
    let target = chance(d.target_precision);
    let mut set: Vec<JudgmentFact> = Vec::new();
    if let Some(p) = d.pinned {
        set.push(fact(
            "Model chosen by",
            format!("{}, {}: {}", setter(p.by.kind), p.at.0.date(), reason(&p.why)),
        ));
    }
    if let Some(b) = d.bar {
        set.push(fact(
            "Bar set by",
            format!("{}, {}: {}", setter(b.by.kind), b.at.0.date(), reason(&b.why)),
        ));
    }
    let alone = match (d.acts, threshold) {
        (Acts::Informs, Threshold::Measured(_) | Threshold::Provisional) if !matches!(d.speaks, Speaks::Always) => {
            format!(
                "it warns {} and is at least {}% sure",
                d.hands_to,
                chance(threshold.at().unwrap_or(judgment::PROVISIONAL_THRESHOLD))
            )
        }
        (Acts::Informs, _) => "never: it only advises".to_owned(),
        (Acts::Alone, Threshold::Provisional) => format!(
            "at least {}% sure, until {MIN_OUTCOMES} {} are known",
            chance(judgment::PROVISIONAL_THRESHOLD),
            acts_on_of(d).results_words()
        ),
        (Acts::Alone, Threshold::Measured(t)) => {
            format!("at least {}% sure", chance(t))
        }
        (Acts::Alone, Threshold::Unreached) => {
            format!("never: no level of certainty has proved right {target}% of the time")
        }
    };
    let mut facts = vec![
        fact("Question", key.to_owned()),
        fact(
            if matches!(d.speaks, Speaks::Always) {
                "Decides alone when"
            } else {
                "Speaks when"
            },
            alone,
        ),
        fact(
            "Must be right",
            format!("{target}% of the times it decides alone or speaks"),
        ),
        fact("Answers in all", format!("{answers}, the measured models' included")),
    ];
    facts.extend(set);
    facts
}

/// Every model asked the question, with its counts and standing: the one that answers it first, then the best Brier
/// score first.
fn deciders_of(
    tallies: &[&JudgmentTally],
    scored: &std::collections::BTreeMap<String, Vec<Scored>>,
    acting: &str,
) -> Vec<DeciderStanding> {
    let mut out: Vec<DeciderStanding> = Vec::new();
    for t in tallies {
        if let Some(d) = out.iter_mut().find(|d| d.decider == t.decider) {
            d.judgments = d.judgments.saturating_add(t.judgments);
            d.unknown = d.unknown.saturating_add(t.unknown);
            d.cost_usd += t.cost_usd;
            continue;
        }
        let standing = Standing::of(&t.decider, scored.get(&t.decider).map_or(&[][..], Vec::as_slice));
        out.push(DeciderStanding {
            name: decider_name(&t.decider).to_owned(),
            decider: t.decider.clone(),
            role: if t.decider == acting {
                "Answers it"
            } else {
                "Measured beside it"
            }
            .to_owned(),
            judgments: t.judgments,
            outcomes: i64::try_from(standing.outcomes).unwrap_or(i64::MAX),
            unknown: t.unknown,
            brier: standing.brier,
            auc: standing.auc,
            cost_usd: t.cost_usd,
        });
    }
    out.sort_by(|a, b| {
        (b.decider == acting)
            .cmp(&(a.decider == acting))
            .then_with(|| match (a.brier, b.brier) {
                (Some(x), Some(y)) => x.total_cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.decider.cmp(&b.decider))
    });
    out
}
