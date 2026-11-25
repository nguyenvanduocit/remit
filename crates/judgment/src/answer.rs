use model::{JudgmentAnswer, JudgmentPick, JudgmentUse};
use sha2::{Digest, Sha256};

use crate::question::{Acts, Definition, Question, Shape, Speaks};
use crate::standing::Threshold;

/// What answer picks for question: yes or no, the option chosen, or the most probable level of a score (the lower of two
/// equally probable). None when it does not answer question: of another kind, a probability outside 0 to 1, an option
/// it does not offer, or not one probability for each of its levels. A decider's answer is read once here; inside, it is
/// trusted.
pub fn pick(question: &Question, answer: &JudgmentAnswer) -> Option<JudgmentPick> {
    let unit = |p: f64| (0.0..=1.0).contains(&p);
    match (question.kind().shape(), answer) {
        (Shape::Noul { .. }, JudgmentAnswer::Noul { p }) if unit(*p) => {
            Some(if *p >= 0.5 { JudgmentPick::Yes } else { JudgmentPick::No })
        }
        (
            Shape::Choice { options },
            JudgmentAnswer::Choice {
                choice, probabilities, ..
            },
        ) => {
            let offered = |name: &str| options.iter().any(|o| o.name == name);
            let fits = offered(choice) && probabilities.iter().all(|(name, p)| unit(*p) && offered(name));
            fits.then(|| JudgmentPick::Option { name: choice.clone() })
        }
        (Shape::Score { levels }, JudgmentAnswer::Score { probabilities, .. })
            if probabilities.len() == levels.len() && probabilities.iter().all(|p| unit(*p)) =>
        {
            let (index, _) = probabilities
                .iter()
                .enumerate()
                .fold(None, |best: Option<(usize, f64)>, (i, &p)| match best {
                    Some((_, b)) if b >= p => best,
                    _ => Some((i, p)),
                })?;
            Some(JudgmentPick::Level {
                index: u32::try_from(index).ok()?,
                name: levels.get(index)?.clone(),
            })
        }
        _ => None,
    }
}

/// The probability answer gives the pick `picked`: what a decider is scored on, whether `picked` is its own pick (the
/// acting judgment's) or the acting decider's (a shadow's). Zero for a pick it gives no probability.
pub fn chance(answer: &JudgmentAnswer, picked: &JudgmentPick) -> f64 {
    match (answer, picked) {
        (JudgmentAnswer::Noul { p }, JudgmentPick::Yes) => *p,
        (JudgmentAnswer::Noul { p }, JudgmentPick::No) => 1.0 - *p,
        (JudgmentAnswer::Choice { probabilities, .. }, JudgmentPick::Option { name }) => {
            probabilities.get(name).copied().unwrap_or(0.0)
        }
        (JudgmentAnswer::Score { probabilities, .. }, JudgmentPick::Level { index, .. }) => usize::try_from(*index)
            .ok()
            .and_then(|i| probabilities.get(i).copied())
            .unwrap_or(0.0),
        _ => 0.0,
    }
}

/// What the code makes of an acting answer that picked pick with probability p, for a question declared as definition: one
/// that always informs informs; one that speaks only when sure informs when its alarming pick reaches its bar and is held
/// otherwise; one that acts alone acts at or above the bar and escalates below it, or when no probability reaches its
/// target.
pub fn use_of(definition: &Definition, pick: &JudgmentPick, p: f64, threshold: Threshold) -> JudgmentUse {
    let reaches = threshold.at().is_some_and(|t| p >= t);
    match (definition.acts(), definition.speaks(), pick) {
        (Acts::Informs, Speaks::Always, _) => JudgmentUse::Informed,
        (Acts::Informs, Speaks::OnYes, JudgmentPick::Yes) | (Acts::Informs, Speaks::OnNo, JudgmentPick::No)
            if reaches =>
        {
            JudgmentUse::Informed
        }
        (Acts::Informs, ..) => JudgmentUse::Held,
        (Acts::Alone, ..) if reaches => JudgmentUse::Acted,
        (Acts::Alone, ..) => JudgmentUse::Escalated,
    }
}

/// The SHA-256 of state as it is sent, in hex: what a judgment keeps of it.
pub fn state_digest(state: &serde_json::Value) -> String {
    hex::encode(Sha256::digest(state.to_string().as_bytes()))
}
