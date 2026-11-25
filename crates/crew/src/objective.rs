//! The pure rules of an objective (ADR-0031): where a key result stands, and what code can find wrong with a draft. Neither
//! reads a store or a clock; the orchestrator hands them what it read, and now.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use model::{
    Approval, Check, CheckKind, Finding, FindingStatus, Guardrail, KRStatus, KeyResult, Objective, Reading, ReasonCode,
    Time,
};

use model::SignalKey;

use chains::Result;
use chains::chain_core::optional;
use chains::goal::verdict_on;
use chains::wait::rfc3339;

/// One chain that works toward a key result, as its status reads it: the chain's goal-judge verdicts, oldest first. A
/// chain with no verdict (no goal check yet, an open watch, work still running) is one with no words.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServedChain {
    pub approvals: Vec<Approval>,
}

/// Where kr stands, from the chains that serve it, by the last-word rule of standing: each chain speaks through its latest
/// verdict. Any chain whose latest verdict confirms kr makes it met; else a latest verdict with a not_met finding for kr
/// makes it not_met; else it is unverified, whatever holds it back (an open watch, an unverified finding, no goal check
/// yet); with no chain serving it, it is unserved.
pub(crate) fn key_result_status(kr: &KeyResult, served: &[ServedChain]) -> KRStatus {
    if served.is_empty() {
        return KRStatus::Unserved;
    }
    let mut status = KRStatus::Unverified;
    for ch in served {
        match verdict_on(&kr.id, &ch.approvals) {
            KRStatus::Met => return KRStatus::Met,
            KRStatus::NotMet => status = KRStatus::NotMet,
            _ => {}
        }
    }
    status
}

/// Where each key result of one objective's tree (krs, parents before children) stands, by id (ADR-0115): one with a
/// check stands as its check does (own); one with no check is met when every child not retired is, not_met when one is,
/// unserved when all are, unverified otherwise, and stands as own says while it has no live child. A child retires with
/// its parent and when its parent is met (`retired_in`). Pure.
pub(crate) fn tree_status(krs: &[KeyResult], own: impl Fn(&KeyResult) -> KRStatus) -> BTreeMap<String, KRStatus> {
    let mut status: BTreeMap<String, KRStatus> = BTreeMap::new();
    // children before parents: a parent reads what its children came to
    for kr in krs.iter().rev() {
        let children: Vec<KRStatus> = krs
            .iter()
            .filter(|c| c.parent.as_deref() == Some(kr.id.as_str()) && !c.retired)
            .filter_map(|c| status.get(&c.id).copied())
            .collect();
        let s = if kr.check.is_some() || children.is_empty() {
            own(kr)
        } else if children.iter().all(|s| *s == KRStatus::Met) {
            KRStatus::Met
        } else if children.contains(&KRStatus::NotMet) {
            KRStatus::NotMet
        } else if children.iter().all(|s| *s == KRStatus::Unserved) {
            KRStatus::Unserved
        } else {
            KRStatus::Unverified
        };
        status.insert(kr.id.clone(), s);
    }
    status
}

/// The ids of the key results of one objective's tree (krs, parents before children) that are retired: by the owner's
/// amendment, or by refinement's rule, with their parent or once their parent is met (status). Pure.
pub fn retired_in(krs: &[KeyResult], status: &BTreeMap<String, KRStatus>) -> BTreeSet<String> {
    let mut retired = BTreeSet::new();
    for kr in krs {
        let by_parent = kr
            .parent
            .as_ref()
            .is_some_and(|p| retired.contains(p) || status.get(p) == Some(&KRStatus::Met));
        if kr.retired || by_parent {
            retired.insert(kr.id.clone());
        }
    }
    retired
}

/// c with the spaces an agent leaves around its words cut, so that a blank field is an empty one.
pub fn trimmed_check(mut c: Check) -> Check {
    for f in [
        &mut c.how,
        &mut c.signal,
        &mut c.condition,
        &mut c.window,
        &mut c.repo,
        &mut c.name,
        &mut c.reason,
        &mut c.telemetry,
        &mut c.incident,
    ] {
        *f = f.trim().to_owned();
    }
    c
}

/// Where a reading key result stands, from what its signal says and nothing else (ADR-0045): met when the newest reading
/// meets the check's condition, not_met when it does not, and unverified while the signal has no reading or the condition
/// cannot be read on it. No chain and no judge decides it.
pub(crate) fn reading_status(c: &Check, head: Option<&Reading>, now: Time) -> KRStatus {
    let Some(head) = head else {
        return KRStatus::Unverified;
    };
    match signal::probe(&c.condition, head, now) {
        Err(_) => KRStatus::Unverified,
        Ok(true) => KRStatus::Met,
        Ok(false) => KRStatus::NotMet,
    }
}

/// What a draft's references are checked against: the surrogate ids of the outcomes the platform has, and the newest
/// reading of each signal the draft reads, which is what its measures are dry-run on. A signal with no entry has no
/// reading.
#[derive(Debug, Clone, Default)]
pub(crate) struct Known {
    pub surrogates: BTreeSet<String>,
    pub readings: HashMap<String, Reading>,
}

/// The findings a check collects, each not_met and coded.
#[derive(Default)]
pub(crate) struct Findings(Vec<Finding>);

impl Findings {
    fn add(&mut self, kr: &str, item: &str, code: ReasonCode, why: String) {
        self.0.push(Finding {
            key_result: kr.to_owned(),
            item: item.to_owned(),
            status: FindingStatus::NotMet,
            evidence: why,
            code: Some(code),
            ..Finding::default()
        });
    }

    pub(crate) fn into_vec(self) -> Vec<Finding> {
        self.0
    }
}

/// What code can find wrong with a draft objective, as findings coded by reason: no key result or an empty one (other), a
/// check that is no measure or cannot be read (unverifiable_key_result), a text another repeats (duplicate), a surrogate
/// or a signal the platform does not know (unknown_metric), and a measure whose reading already passes a guardrail's bound
/// (guardrail_breach). Every guardrail and every reading key result is dry-run on the reading its signal made, so a
/// measure counts only when a signal reads it (ADR-0045). It is pure and returns no error: the owner cannot apply a draft
/// while it finds anything (apply_objective). What needs reading (is the outcome covered, does the evidence contradict) is
/// the hired judge's.
pub(crate) fn check_objective(o: &Objective, now: Time, known: &Known) -> Vec<Finding> {
    let mut out = Findings::default();
    // A retired key result is kept for the record and demands nothing: its check and text are no longer the author's to
    // answer.
    let key_results: Vec<&KeyResult> = o.key_results.iter().filter(|kr| !kr.retired).collect();
    if key_results.is_empty() {
        out.add(
            "",
            "key results",
            ReasonCode::Other,
            "the objective has no key result: nothing says what reaching it means".to_owned(),
        );
    }
    for s in &o.surrogates {
        if !known.surrogates.contains(&s.kpi) {
            out.add(
                "",
                &format!("surrogate {}", s.kpi),
                ReasonCode::UnknownMetric,
                format!("the platform has no outcome {:?} to read", s.kpi),
            );
        }
    }
    for g in &o.guardrails {
        check_guardrail(g, now, known, &mut out);
    }
    let mut first_of: BTreeMap<String, String> = BTreeMap::new();
    for kr in key_results {
        if kr.text.trim().is_empty() {
            out.add(
                &kr.id,
                "text",
                ReasonCode::Other,
                "the key result has no text".to_owned(),
            );
        } else if let Some(first) = first_of.get(&normalised(&kr.text)) {
            out.add(
                &kr.id,
                "text",
                ReasonCode::Duplicate,
                format!("it says what key result {first} says"),
            );
        } else {
            first_of.insert(normalised(&kr.text), kr.id.clone());
        }
        check_key_result(kr, &o.guardrails, now, known, &mut out);
    }
    out.into_vec()
}

/// A key result's text as two texts are compared: case, spacing and a closing stop do not tell them apart.
fn normalised(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .trim_end_matches(['.', '!', ';', ':', ','])
        .to_owned()
}

/// Reads what signal_key says now and runs condition on it, for the measure item: what is wrong with the measure when it
/// cannot be read is added to out and None returned, else whether the condition held on the reading.
pub(crate) fn dry_run(
    kr: &str,
    item: &str,
    signal_key: &str,
    condition: &str,
    now: Time,
    known: &Known,
    out: &mut Findings,
) -> Option<bool> {
    if signal_key.is_empty() {
        out.add(
            kr,
            item,
            ReasonCode::UnknownMetric,
            "it names no signal: a measure is read from a signal, and nothing else counts".to_owned(),
        );
        return None;
    }
    let Some(head) = known.readings.get(signal_key) else {
        out.add(
            kr,
            item,
            ReasonCode::UnknownMetric,
            format!(
                "the signal {signal_key:?} has no reading to run the measure on: build the signal first, and draft the \
                 measure once it reads"
            ),
        );
        return None;
    };
    match signal::probe(condition, head, now) {
        Ok(held) => Some(held),
        Err(err) => {
            out.add(
                kr,
                item,
                ReasonCode::UnverifiableKeyResult,
                format!(
                    "dry run on the reading of {signal_key} made {}: {err}",
                    rfc3339(head.at)
                ),
            );
            None
        }
    }
}

/// Dry-runs g on its signal's reading: a guardrail that cannot be read, or that the reading already passes, is a finding.
fn check_guardrail(g: &Guardrail, now: Time, known: &Known, out: &mut Findings) {
    let item = format!("guardrail {}", g.kpi);
    if dry_run("", &item, &g.signal, &g.condition, now, known, out) == Some(false) {
        out.add(
            "",
            &item,
            ReasonCode::GuardrailBreach,
            format!(
                "the reading of {} already fails {:?}: the work would start outside the bound",
                g.signal, g.condition
            ),
        );
    }
}

/// Reports what is wrong with kr's check: one that is no measure (a person's opinion, or nothing), one no one can run, a
/// reading with no incident plan or that cannot be read now, a reading that can only be met by breaching a guardrail.
fn check_key_result(kr: &KeyResult, guardrails: &[Guardrail], now: Time, known: &Known, out: &mut Findings) {
    let Some(c) = &kr.check else {
        out.add(
            &kr.id,
            "check",
            ReasonCode::UnverifiableKeyResult,
            "it has no check: name one of submit, reading, command, unverifiable".to_owned(),
        );
        return;
    };
    if let Err(err) = c.valid() {
        out.add(&kr.id, "check", ReasonCode::UnverifiableKeyResult, err.to_string());
        return;
    }
    match c.kind {
        CheckKind::Unverifiable => {
            out.add(
                &kr.id,
                "check",
                ReasonCode::UnverifiableKeyResult,
                format!("it is marked unverifiable: {}", c.reason),
            );
            return;
        }
        CheckKind::Submit => {
            out.add(
                &kr.id,
                "check",
                ReasonCode::UnverifiableKeyResult,
                "a submit check is a person's opinion, and no measure: read a signal, or run a command that passes"
                    .to_owned(),
            );
            return;
        }
        CheckKind::Reading => {}
        CheckKind::Command => return,
    }
    if c.expiry.is_some_and(|expiry| now >= expiry) {
        out.add(
            &kr.id,
            "check",
            ReasonCode::UnverifiableKeyResult,
            "the reading's window has already ended, so no reading can confirm it".to_owned(),
        );
        return;
    }
    if c.incident.trim().is_empty() {
        out.add(
            &kr.id,
            "incident",
            ReasonCode::UnverifiableKeyResult,
            "a reading reads production, and it says nothing of what happens when it breaks: name what breaks, how it is \
             seen, who reverses it and how"
                .to_owned(),
        );
    }
    if dry_run(&kr.id, "check", &c.signal, &c.condition, now, known, out).is_none() {
        return;
    }
    for g in guardrails {
        if g.signal == c.signal && breaches(&c.condition, &g.condition) {
            out.add(
                &kr.id,
                &format!("guardrail {}", g.kpi),
                ReasonCode::GuardrailBreach,
                format!(
                    "it needs {:?} while the guardrail holds the measure to {:?}",
                    c.condition, g.condition
                ),
            );
        }
    }
}

/// One side of a range: value op limit, with op one of < <= > >=.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bound {
    op: &'static str,
    limit: f64,
}

impl Bound {
    fn upper(self) -> bool {
        self.op == "<" || self.op == "<="
    }
}

/// Every comparison text states: an operator (<=, >=, <, >), spaces, and a number (-?digits, an optional fraction).
fn comparisons(text: &str) -> Vec<Bound> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(['<', '>']) {
        let (_, from) = rest.split_at(at);
        let op: &'static str = match (from.starts_with('<'), from.get(1..2) == Some("=")) {
            (true, true) => "<=",
            (true, false) => "<",
            (false, true) => ">=",
            (false, false) => ">",
        };
        let after = from.get(op.len()..).unwrap_or_default();
        match number_at(after.trim_start()) {
            Some((limit, len)) => {
                out.push(Bound { op, limit });
                let skipped = after.len().saturating_sub(after.trim_start().len());
                rest = after.get(skipped.saturating_add(len)..).unwrap_or_default();
            }
            None => rest = from.get(1..).unwrap_or_default(),
        }
    }
    out
}

/// The number that opens s (-?digits, then a fraction when digits follow the point) and its length in bytes.
fn number_at(s: &str) -> Option<(f64, usize)> {
    let digits = |t: &str| t.bytes().take_while(u8::is_ascii_digit).count();
    let sign = usize::from(s.starts_with('-'));
    let whole = digits(s.get(sign..)?);
    if whole == 0 {
        return None;
    }
    let mut len = sign.saturating_add(whole);
    if let Some(frac) = s.get(len..).and_then(|t| t.strip_prefix('.')).map(digits)
        && frac > 0
    {
        len = len.saturating_add(1).saturating_add(frac);
    }
    s.get(..len)?.parse().ok().map(|n| (n, len))
}

/// The one comparison text states, and None when it states none or several (code does not guess which rules).
fn bound_of(text: &str) -> Option<Bound> {
    match comparisons(text).as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// Whether no value can satisfy both condition (a key result's reading) and guardrail (its bound): meeting the key result
/// then passes the bound. Text that is not one plain comparison each is not decided here.
pub(crate) fn breaches(condition: &str, guardrail: &str) -> bool {
    let (Some(kr), Some(g)) = (bound_of(condition), bound_of(guardrail)) else {
        return false;
    };
    if kr.upper() == g.upper() {
        return false;
    }
    let (lower, upper) = if g.upper() { (kr, g) } else { (g, kr) };
    let same = lower.limit.total_cmp(&upper.limit).is_eq();
    let meet = same && lower.op == ">=" && upper.op == "<=";
    lower.limit > upper.limit || (same && !meet)
}

/// What the checker finds in obj, with its measures dry-run on the newest reading of each signal they name, as r (the
/// store, or a transaction) holds them.
pub fn checked_objective(r: &store::Reader<'_>, obj: &Objective, now: Time) -> Result<Vec<Finding>> {
    let mut known = Known::default();
    let report = outcomes::compute(
        &outcomes::Input {
            now,
            ..outcomes::Input::default()
        },
        &outcomes::Span::default(),
    );
    for g in report.groups {
        known.surrogates.extend(g.kpis.into_iter().map(|k| k.key));
    }
    for key in measured_signals(obj) {
        if let Some(reading) = optional(r.last_reading(&SignalKey(key.clone())))? {
            known.readings.insert(key, reading);
        }
    }
    Ok(check_objective(obj, now, &known))
}

/// The signals obj's guardrails and live reading key results name, each once.
fn measured_signals(obj: &Objective) -> BTreeSet<String> {
    let guardrails = obj.guardrails.iter().map(|g| g.signal.clone());
    let readings = obj
        .key_results
        .iter()
        .filter(|kr| !kr.retired)
        .filter_map(|kr| kr.check.as_ref())
        .filter(|c| c.kind == CheckKind::Reading)
        .map(|c| c.signal.clone());
    guardrails.chain(readings).filter(|k| !k.is_empty()).collect()
}

/// The findings as the author reads them under a refusal: empty for none.
pub(crate) fn findings_words(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return String::new();
    }
    let mut b = "\n\nThe checker found:".to_owned();
    for f in findings {
        let on = if f.key_result.is_empty() {
            "the objective".to_owned()
        } else {
            format!("key result {}", f.key_result)
        };
        b.push_str(&format!(
            "\n- {} on {on}: {}",
            f.code.map_or("", |c| c.as_str()),
            format!("{} {}", f.item, f.evidence).trim()
        ));
    }
    b
}
