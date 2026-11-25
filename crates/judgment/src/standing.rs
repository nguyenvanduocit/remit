use crate::decider::SHADOWS;

/// How many judgments with a known outcome a decider needs on a question before its numbers count: before, a question's
/// default decider acts and its act threshold is `PROVISIONAL_THRESHOLD`.
pub const MIN_OUTCOMES: usize = 30;

/// The act threshold of a question with fewer than `MIN_OUTCOMES` outcomes: where the measurement for ADR-0118 found
/// every duplicate memory with no false alarm.
pub const PROVISIONAL_THRESHOLD: f64 = 0.9;

/// One judgment with a known outcome, as a decider is scored on it: its probability of the asking's pick, and whether
/// the pick proved right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub p: f64,
    pub right: bool,
}

/// How well a decider has answered one question.
#[derive(Debug, Clone, PartialEq)]
pub struct Standing {
    pub decider: String,
    /// Its judgments whose outcome is right or wrong.
    pub outcomes: usize,
    /// The mean squared distance of its probabilities from what happened: 0 is perfect, 0.25 a coin's; none without an
    /// outcome.
    pub brier: Option<f64>,
    /// How often it gives a pick that proved right a higher probability than one that proved wrong: 1 separates them
    /// perfectly, 0.5 not at all; none until it has one of each.
    pub auc: Option<f64>,
}

impl Standing {
    /// decider's standing over its judgments with a known outcome.
    pub fn of(decider: &str, scored: &[Scored]) -> Self {
        Self {
            decider: decider.to_owned(),
            outcomes: scored.len(),
            brier: brier(scored),
            auc: auc(scored),
        }
    }
}

/// The probability at or above which an acting answer acts alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Threshold {
    /// Fewer than `MIN_OUTCOMES` outcomes: `PROVISIONAL_THRESHOLD`.
    Provisional,
    /// The lowest probability whose measured precision reaches the question's target.
    Measured(f64),
    /// No probability's measured precision reaches the target: no answer acts alone.
    Unreached,
}

impl Threshold {
    /// The probability itself; none when no answer acts alone.
    pub const fn at(self) -> Option<f64> {
        match self {
            Self::Provisional => Some(PROVISIONAL_THRESHOLD),
            Self::Measured(t) => Some(t),
            Self::Unreached => None,
        }
    }
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

fn truth(right: bool) -> f64 {
    if right { 1.0 } else { 0.0 }
}

/// The Brier score of scored: the mean of (p − outcome)², outcome 1 for right and 0 for wrong. None for none.
pub fn brier(scored: &[Scored]) -> Option<f64> {
    if scored.is_empty() {
        return None;
    }
    let sum: f64 = scored.iter().map(|s| (s.p - truth(s.right)).powi(2)).sum();
    Some(sum / count(scored.len()))
}

/// The area under the ROC curve of scored: the chance that a pick which proved right was given a higher probability
/// than one which proved wrong, a tie counting half (Mann–Whitney, ranks averaged over ties). None until scored holds
/// one of each.
pub fn auc(scored: &[Scored]) -> Option<f64> {
    let rights = scored.iter().filter(|s| s.right).count();
    let wrongs = scored.len().saturating_sub(rights);
    if rights == 0 || wrongs == 0 {
        return None;
    }
    let mut sorted = scored.to_vec();
    sorted.sort_by(|a, b| a.p.total_cmp(&b.p));
    let mut rank_sum = 0.0;
    let mut start = 0;
    while let Some(first) = sorted.get(start) {
        let tied = sorted.iter().skip(start).take_while(|s| s.p == first.p).count();
        let end = start.saturating_add(tied);
        // Ranks start at 1: the group holds ranks start+1 ..= end, whose mean is (start + 1 + end) / 2.
        let rank = (count(start) + 1.0 + count(end)) / 2.0;
        let right_here = sorted.iter().skip(start).take(tied).filter(|s| s.right).count();
        rank_sum += rank * count(right_here);
        start = end;
    }
    let (r, w) = (count(rights), count(wrongs));
    Some((rank_sum - r * (r + 1.0) / 2.0) / (r * w))
}

/// The act threshold of the acting decider's judgments scored, for a question whose acts must prove right at least
/// target of the time: the lowest probability p such that, of the judgments given p or more, that share proved right.
/// Provisional with fewer than `MIN_OUTCOMES`.
pub fn act_threshold(scored: &[Scored], target: f64) -> Threshold {
    if scored.len() < MIN_OUTCOMES {
        return Threshold::Provisional;
    }
    let mut sorted = scored.to_vec();
    sorted.sort_by(|a, b| b.p.total_cmp(&a.p));
    let (mut seen, mut right, mut lowest) = (0_usize, 0_usize, None);
    let mut start = 0;
    while let Some(first) = sorted.get(start) {
        let group: Vec<&Scored> = sorted.iter().skip(start).take_while(|s| s.p == first.p).collect();
        seen = seen.saturating_add(group.len());
        right = right.saturating_add(group.iter().filter(|s| s.right).count());
        if count(right) / count(seen) >= target {
            lowest = Some(first.p);
        }
        start = start.saturating_add(group.len());
    }
    lowest.map_or(Threshold::Unreached, Threshold::Measured)
}

/// The deciders a question is asked of, in order: those with `MIN_OUTCOMES` outcomes or more by their Brier score, the
/// best (lowest) first, then default when it is not among them. The first acts; a later one acts when those before it
/// gave no answer, so a decider that stopped answering does not hold its question in System Two.
pub fn ranked<'a>(default: &'a str, standings: &'a [Standing]) -> Vec<&'a str> {
    let mut measured: Vec<(&str, f64)> = standings
        .iter()
        .filter(|s| s.outcomes >= MIN_OUTCOMES)
        .filter_map(|s| s.brier.map(|b| (s.decider.as_str(), b)))
        .collect();
    measured.sort_by(|(_, a), (_, b)| a.total_cmp(b));
    let mut out: Vec<&str> = measured.into_iter().map(|(d, _)| d).collect();
    if !out.contains(&default) {
        out.push(default);
    }
    out
}

/// The deciders asked in shadow when acting acts for a question whose default is default: the default and `SHADOWS`,
/// each once and none of them the acting one, at most two.
pub fn shadows(default: &str, acting: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(2);
    for d in std::iter::once(default).chain(SHADOWS) {
        if d != acting && !out.iter().any(|o| o == d) && out.len() < 2 {
            out.push(d.to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(p: f64, right: bool) -> Scored {
        Scored { p, right }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn brier_is_the_mean_squared_distance_from_what_happened() {
        assert_eq!(brier(&[]), None);
        assert!(close(brier(&[s(1.0, true), s(0.0, false)]).unwrap(), 0.0));
        // (0.2)² for a right at 0.8, (0.6)² for a wrong at 0.6: (0.04 + 0.36) / 2.
        assert!(close(brier(&[s(0.8, true), s(0.6, false)]).unwrap(), 0.2));
    }

    #[test]
    fn auc_counts_the_pairs_a_decider_orders_right_and_half_a_tie() {
        assert_eq!(auc(&[s(0.9, true), s(0.8, true)]), None, "no wrong to order against");
        assert!(close(auc(&[s(0.9, true), s(0.1, false)]).unwrap(), 1.0));
        assert!(close(auc(&[s(0.1, true), s(0.9, false)]).unwrap(), 0.0));
        assert!(close(auc(&[s(0.5, true), s(0.5, false)]).unwrap(), 0.5));
        // Rights at 0.9, 0.6 and 0.4 against wrongs at 0.6 and 0.2: of six pairs, 0.9 beats both, 0.6 beats 0.2 and ties
        // 0.6, 0.4 beats 0.2: 4.5 / 6.
        let mixed = [s(0.9, true), s(0.6, true), s(0.4, true), s(0.6, false), s(0.2, false)];
        assert!(close(auc(&mixed).unwrap(), 0.75));
    }

    #[test]
    fn the_threshold_is_provisional_until_thirty_outcomes() {
        let few: Vec<Scored> = (0..29).map(|_| s(0.95, true)).collect();
        assert_eq!(act_threshold(&few, 0.9), Threshold::Provisional);
        assert_eq!(Threshold::Provisional.at(), Some(PROVISIONAL_THRESHOLD));
    }

    #[test]
    fn the_threshold_is_the_lowest_probability_whose_precision_reaches_the_target() {
        // 20 right at 0.95, then at 0.7 ten more of which 2 are wrong (precision 28/30), then at 0.4 ten of which 6 are
        // wrong (32/40).
        let mut scored: Vec<Scored> = (0..20).map(|_| s(0.95, true)).collect();
        scored.extend((0..8).map(|_| s(0.7, true)));
        scored.extend((0..2).map(|_| s(0.7, false)));
        scored.extend((0..4).map(|_| s(0.4, true)));
        scored.extend((0..6).map(|_| s(0.4, false)));
        assert_eq!(act_threshold(&scored, 0.9), Threshold::Measured(0.7));
        assert_eq!(act_threshold(&scored, 0.8), Threshold::Measured(0.4));
        assert_eq!(act_threshold(&scored, 0.99), Threshold::Measured(0.95));
        let wrong: Vec<Scored> = (0..30).map(|_| s(0.9, false)).collect();
        assert_eq!(act_threshold(&wrong, 0.5), Threshold::Unreached);
        assert_eq!(Threshold::Unreached.at(), None);
    }
}
