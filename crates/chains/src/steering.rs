//! The owner steers how Remit asks its deciders (ADR-0118, point 8), through the Assistant: which decider answers a
//! question first, and how sure an answer must be to decide alone. Each is a setting of `model::Settings::steering`
//! recorded with who set it and why, by the question's id without its version, so a new wording of a question keeps it.
//! What is unset is the question's own: the decider its standings rank best, and its declared target precision. A
//! steered decider answers first once it has `MIN_OUTCOMES` results of its own, and the others are still asked after it
//! when it gives no answer, so a retired model never holds a question in its System Two; before that it is asked beside
//! the acting one and only informs (`judge::on_trial`): a choice made on a name never decides alone. A lower precision lets
//! a decider act at less certainty only down to the question's floor (`Definition::floor`): where a wrong act loses
//! something (a lesson merged away, an event screened out) the floor is 90%, and where it is taken back (the climb of a
//! hire, a deeper review) it is the lowest precision there is. A question that always informs has no bar to set.

use judgment::{Definition, decider_name};
use model::{Actor, Setting, Settings, TargetPrecision};

use crate::chain_core::Core;
use crate::judge::{on_trial, ranked_by, scored_by_decider, threshold_results};
use crate::{Error, Result};
use store::JudgmentScore;

/// The decider the owner pinned to definition, if he did.
pub(crate) fn pinned<'a>(settings: &'a Settings, definition: &Definition) -> Option<&'a Setting<String>> {
    settings.steering.get(definition.id())?.decider.as_ref()
}

/// The share of its deciding-alone answers that must prove right for definition: the owner's, else the declaration's,
/// never below the question's floor.
pub(crate) fn target_precision(settings: &Settings, definition: &Definition) -> f64 {
    settings
        .steering
        .get(definition.id())
        .and_then(|s| s.target_precision_percent.as_ref())
        .and_then(|p| TargetPrecision::parse(p.value))
        .map_or(definition.target_precision(), |p| {
            p.share().max(f64::from(definition.floor().percent()) / 100.0)
        })
}

/// The question a name from the owner or the Assistant means: its key (`memory.same_claim@1`) or its id
/// (`memory.same_claim`). Refused, naming the questions there are, when none is.
fn declared(name: &str) -> Result<&'static Definition> {
    let id = name.split('@').next().unwrap_or_default().trim();
    judgment::QUESTIONS
        .iter()
        .copied()
        .find(|d| d.id() == id)
        .ok_or_else(|| {
            let known: Vec<String> = judgment::QUESTIONS
                .iter()
                .map(|d| format!("{} ({})", d.id(), d.title()))
                .collect();
            Error::refused(format!("no question {name:?}: the questions are {}", known.join("; ")))
        })
}

/// Whether name is a decider Remit can ask: a pinned model id, as OpenRouter lists it, never a `~latest` alias (a
/// standing belongs to the model that earned it).
fn is_decider(name: &str) -> bool {
    name.split_once('/')
        .is_some_and(|(vendor, model)| !vendor.is_empty() && !model.is_empty())
        && !name.contains('~')
        && !name.contains(char::is_whitespace)
}

fn why_given(why: &str) -> Result<String> {
    let why = why.trim();
    if why.is_empty() {
        return Err(Error::refused(
            "say why, in the owner's words: the setting is recorded with it",
        ));
    }
    Ok(why.to_owned())
}

fn owner_only(by: &Actor) -> Result<()> {
    if by.for_owner() {
        Ok(())
    } else {
        Err(Error::refused("only the owner steers the deciders"))
    }
}

impl Core {
    /// The results of every decider on definition's question: judgments whose outcome is known, and the settings.
    async fn results_of(&self, definition: &Definition) -> Result<(Vec<JudgmentScore>, Settings)> {
        let key = definition.key();
        Ok(self
            .store
            .read(move |r| Ok::<_, store::Error>((r.judgment_scores(&[key])?, r.settings()?)))
            .await?)
    }

    /// Sets the decider that answers question first to decider (a pinned model id), or back to the one its standings
    /// rank best when decider is empty or `default`. By the owner or the Assistant on his words, with why; returns what
    /// it now is, in words: a decider with fewer than `MIN_OUTCOMES` results of its own on the question only informs
    /// until it has them.
    pub async fn set_decider(&self, by: &Actor, question: &str, decider: &str, why: &str) -> Result<String> {
        owner_only(by)?;
        let definition = declared(question)?;
        if !definition.scored() {
            return Err(Error::refused(format!(
                "{} has no results to measure, so no model is ever chosen for it: a pin would never be used",
                definition.title()
            )));
        }
        let why = why_given(why)?;
        let decider = decider.trim();
        let pin = (!decider.is_empty() && !decider.eq_ignore_ascii_case("default")).then(|| decider.to_owned());
        if let Some(d) = &pin
            && !is_decider(d)
        {
            return Err(Error::refused(format!(
                "{d:?} is not a decider: name a model as OpenRouter lists it (vendor/model, pinned, never a ~latest alias)"
            )));
        }
        let (by, at) = (by.clone(), self.now());
        let id = definition.id();
        let value = pin.clone();
        self.store
            .update(move |tx| {
                let mut s = tx.settings()?;
                let steering = s.steering.entry(id.to_owned()).or_default();
                steering.decider = value.map(|value| Setting { value, by, why, at });
                if steering.is_empty() {
                    s.steering.remove(id);
                }
                tx.put_settings(s)?;
                Ok::<_, store::Error>(())
            })
            .await?;
        let (scores, _) = self.results_of(definition).await?;
        let scored = scored_by_decider(&scores, &definition.key());
        let acting = ranked_by(definition.default_decider(), pin.as_deref(), &scored)
            .into_iter()
            .next()
            .unwrap_or_else(|| definition.default_decider().to_owned());
        Ok(match pin {
            Some(d) if on_trial(Some(&d), &scored).is_some() => format!(
                "{} has {} of the {} results it needs on {}, so for now it only informs: it is asked beside {}, which still decides, and takes over once it has them.",
                decider_name(&d),
                scored.get(&d).map_or(0, Vec::len),
                judgment::MIN_OUTCOMES,
                definition.title(),
                decider_name(&acting),
            ),
            Some(d) => format!("{} is now answered by {} first.", definition.title(), decider_name(&d)),
            None => format!(
                "{} is answered by the model its results rank best, {} until one has {} results.",
                definition.title(),
                decider_name(&acting),
                judgment::MIN_OUTCOMES
            ),
        })
    }

    /// Sets how sure an answer to question must be to decide alone, or to speak where it speaks only when sure: the share
    /// of such answers that must prove right, percent (50 to 99, and not below the question's floor), or back to its
    /// declaration's when percent is none. By the owner or the Assistant on his words, with why; returns what it now is,
    /// in words. A question that always informs has no bar.
    pub async fn set_target_precision(
        &self,
        by: &Actor,
        question: &str,
        percent: Option<i64>,
        why: &str,
    ) -> Result<String> {
        owner_only(by)?;
        let definition = declared(question)?;
        if !definition.scored() {
            return Err(Error::refused(format!(
                "{} has no results to measure, so it has no precision to set",
                definition.title()
            )));
        }
        if !definition.has_bar() {
            return Err(Error::refused(format!(
                "{} only informs, whatever its probability: there is no level of certainty to set",
                definition.title()
            )));
        }
        let set = match percent {
            Some(p) => Some(TargetPrecision::parse(p).ok_or_else(|| {
                Error::refused(format!(
                    "a precision is {} to {} percent, not {p}",
                    TargetPrecision::MIN,
                    TargetPrecision::MAX
                ))
            })?),
            None => None,
        };
        let floor = definition.floor();
        if let Some(p) = set.filter(|p| p.percent() < floor.percent()) {
            return Err(Error::refused(format!(
                "{} cannot be held to {}%: it is held to at least {}%, because {}",
                definition.title(),
                p.percent(),
                floor.percent(),
                floor.because()
            )));
        }
        let why = why_given(why)?;
        let (by, at) = (by.clone(), self.now());
        let id = definition.id();
        self.store
            .update(move |tx| {
                let mut s = tx.settings()?;
                let steering = s.steering.entry(id.to_owned()).or_default();
                steering.target_precision_percent = set.map(|p| Setting {
                    value: i64::from(p.percent()),
                    by,
                    why,
                    at,
                });
                if steering.is_empty() {
                    s.steering.remove(id);
                }
                tx.put_settings(s)?;
                Ok::<_, store::Error>(())
            })
            .await?;
        let (scores, settings) = self.results_of(definition).await?;
        let scored = scored_by_decider(&scores, &definition.key());
        let pin = pinned(&settings, definition).map(|p| p.value.as_str());
        let acting = ranked_by(definition.default_decider(), pin, &scored)
            .into_iter()
            .next()
            .unwrap_or_else(|| definition.default_decider().to_owned());
        let known = threshold_results(&scores, &definition.key(), definition.acts_on(), &acting).len();
        let provisional = if known < judgment::MIN_OUTCOMES {
            format!(
                " {} has {known} of the {} {} it needs to measure that, so until then the provisional {:.0}% still applies.",
                decider_name(&acting),
                judgment::MIN_OUTCOMES,
                definition.acts_on().results_words(),
                judgment::PROVISIONAL_THRESHOLD * 100.0
            )
        } else {
            String::new()
        };
        let what = if matches!(definition.acts(), judgment::Acts::Alone) {
            "decides alone"
        } else {
            "speaks to the card's Chair"
        };
        Ok(match set {
            Some(p) => format!(
                "{} now {what} only at a level of certainty that has proved right {}% of the time.{provisional}",
                definition.title(),
                p.percent()
            ),
            None => format!(
                "{} is back to the usual {:.0}%: it {what} only at a level of certainty that has proved right that often.{provisional}",
                definition.title(),
                definition.target_precision() * 100.0
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::Steering;

    #[test]
    fn a_question_is_named_by_its_key_or_its_id() {
        assert_eq!(
            declared("memory.same_claim@1").map(Definition::id).ok(),
            Some("memory.same_claim")
        );
        assert_eq!(
            declared("memory.same_claim").map(Definition::id).ok(),
            Some("memory.same_claim")
        );
        assert!(declared("memory.nonsense").is_err());
    }

    #[test]
    fn only_a_pinned_model_id_is_a_decider() {
        assert!(is_decider("typesafe/jev-1.14"));
        assert!(is_decider("perplexity/pplx-decider-v1.3:free"));
        assert!(!is_decider("jev"));
        assert!(!is_decider("typesafe/jev~latest"));
        assert!(!is_decider("typesafe/ jev"));
    }

    #[test]
    fn the_owner_sets_the_precision_and_a_question_without_a_setting_keeps_its_own() {
        let mut settings = Settings::default();
        let question = &judgment::MEMORY_SAME_CLAIM;
        assert!((target_precision(&settings, question) - question.target_precision()).abs() < 1e-9);
        let said = Setting {
            value: 95,
            by: Actor::default(),
            why: "wrong merges cost more than a missed duplicate".to_owned(),
            at: model::Time::default(),
        };
        settings.steering.insert(
            question.id().to_owned(),
            Steering {
                target_precision_percent: Some(said),
                ..Steering::default()
            },
        );
        assert!((target_precision(&settings, question) - 0.95).abs() < 1e-9);
        assert!((target_precision(&settings, &judgment::HIRE_TIER_CAN) - 0.8).abs() < 1e-9);
    }
}
