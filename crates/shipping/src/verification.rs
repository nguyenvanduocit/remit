//! How a chain's result has been judged, for the owner: an independent review, separate QC when requested, and a security reviewer on a
//! change that touches a rail (gate.rs), then the goal judge on the result (goal.rs). The agents record their verdicts as approvals; these are the server's reading of them, computed
//! once here so every channel shows the same: the chip on a chain's board card (`ChainStanding::verification`) and its
//! Verification tab. A reviewer's or QC's approval is of one version of its repository's change (its patch id): once the
//! change has moved, the approval is stale and reads so, not as passed. views.rs reads each change as it is now, keeps
//! these current and pushes them. Pure.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use model::{Approval, Chain, ChainStatus, ChangeGates, FindingStatus, Gate, Landing, RepoWork, Verdict, VerdictRole};
use wire::app::{CheckLook, CheckTone, GateState, GateView, Judge, Move, VerdictView, Verification};

use crate::gate::gate_label;
use crate::goal::GOAL_JUDGE_NAME;
use chains::gate::{approved_earlier, review_needs_evidence};
use chains::goal::goal_approvals;

/// How something judged reads: `label`, in the colour `tone` names.
fn check(label: impl Into<String>, tone: CheckTone) -> CheckLook {
    CheckLook {
        label: label.into(),
        tone,
    }
}

/// How the owner reads a change gate in state s.
const fn gate_line(s: GateState) -> &'static str {
    match s {
        GateState::Approved => "Approved",
        GateState::Stale => "Approved an earlier version",
        GateState::Changes => "Changes requested",
        GateState::None => "Not asked yet",
        GateState::Running => "",
    }
}

/// A change gate's states from the best to the worst.
const fn gate_rank(s: GateState) -> u8 {
    match s {
        GateState::None | GateState::Running => 0,
        GateState::Approved => 1,
        GateState::Stale => 2,
        GateState::Changes => 3,
    }
}

/// The patch id of each repository's change as it is now, by repository; a repository missing is not known, and its
/// approvals are not judged stale.
pub type Changes = BTreeMap<String, String>;

/// How chain c's result has been judged, from approvals, its verdicts in the order they were given, against current, its
/// changes as they are now.
pub fn verification_of(
    c: &Chain,
    approvals: &[Approval],
    landings: &[Landing],
    current: Option<&Changes>,
) -> Verification {
    let mine = of_chain(c, approvals);
    let mut gates: Vec<GateView> = [Gate::Reviewer, Gate::Qc, Gate::Security]
        .into_iter()
        .filter(|&gate| needs_gate(c, landings, gate) || mine.iter().any(|a| a.role == gate.role()))
        .map(|gate| change_gate(gate, &mine, current, c, landings))
        .collect();
    gates.push(goal_gate(c, &mine));
    let mut v = Verification {
        chain_id: c.id.clone(),
        gates,
        ..Verification::default()
    };
    let repos: BTreeSet<&str> = mine.iter().map(|a| a.repo.as_str()).filter(|r| !r.is_empty()).collect();
    for role in [
        VerdictRole::Judge,
        VerdictRole::Reviewer,
        VerdictRole::Qc,
        VerdictRole::Security,
    ] {
        for j in judges_of(&mine, role) {
            // newest first: each round differs from the one after it
            let mut out = Judge {
                key: format!("{role}|{}", j.key),
                card_id: j.latest.card_id.clone(),
                role,
                name: j.name.clone(),
                repo: None,
                latest: verdict_of(j.latest, verdict_state(j.latest, current), &j.earlier),
                earlier: Vec::new(),
            };
            if repos.len() > 1
                && !j.repo.is_empty()
                && !j.name.to_lowercase().contains(&repo_name(&j.repo).to_lowercase())
            {
                out.repo = Some(repo_name(&j.repo).to_owned());
            }
            // the rounds the latest has replaced: read as they were given
            for (i, a) in j.earlier.iter().enumerate() {
                let before = j.earlier.get(i.saturating_add(1)..).unwrap_or_default();
                out.earlier.push(verdict_of(a, verdict_state(a, None), before));
            }
            v.judges.push(out);
        }
    }
    v
}

/// What a chain's board card says about its checking against current, its changes as they are now, or none when nobody
/// has checked it. The most recent word wins: a refusal by the goal judge stands only until a gate judges the team again.
pub fn verification_check(
    c: &Chain,
    approvals: &[Approval],
    landings: &[Landing],
    current: Option<&Changes>,
) -> Option<CheckLook> {
    if c.finishing.is_some() {
        return Some(check("Goal check running", CheckTone::Running));
    }
    let mine = of_chain(c, approvals);
    let goal = goal_approvals(mine.iter().copied());
    if let Some(last) = goal.last() {
        let newer_gate = mine.iter().any(|a| a.role.gate().is_some() && a.at > last.at);
        if !newer_gate {
            return Some(if last.verdict == Verdict::Approve {
                check("Goal met", CheckTone::Done)
            } else if *c.status() == ChainStatus::Done {
                check("Finished, goal not met", CheckTone::Failed)
            } else {
                check(format!("Goal not met · check {}", goal.len()), CheckTone::Needs)
            });
        }
    }
    gates_check(c, &mine, landings, current)
}

/// Where one repository's change stands by the latest verdict of each gate, from the worst to the best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stand {
    Refused,
    Stale,
    Half,
    Passed,
}

/// One word on where the changes stand, by the latest verdict of each gate on each repository, the ones that decide
/// whether its change may go out, against current: the worst repository speaks.
fn gates_check(
    c: &Chain,
    approvals: &[&Approval],
    landings: &[Landing],
    current: Option<&Changes>,
) -> Option<CheckLook> {
    let (mut worst, mut seen, mut awaiting) = (Stand::Passed, false, None);
    for rw in &c.repos {
        let required = required_of(landings, rw);
        let latest = latest_gates(approvals, &rw.repo, required);
        if latest.is_empty() {
            continue;
        }
        seen = true;
        let missing = required.iter().find(|g| {
            !latest
                .iter()
                .any(|a| a.role == g.role() && !review_needs_evidence(a, required))
        });
        let stand = if latest.iter().any(|a| a.verdict != Verdict::Approve) {
            Stand::Refused
        } else if latest.iter().any(|a| verdict_state(a, current) == GateState::Stale) {
            Stand::Stale
        } else if let Some(&g) = missing {
            awaiting = awaiting.or(Some(g));
            Stand::Half
        } else {
            Stand::Passed
        };
        worst = worst.min(stand);
    }
    if !seen {
        return None;
    }
    Some(match worst {
        Stand::Refused => check("Changes requested", CheckTone::Needs),
        Stand::Stale => check("Changed since approval", CheckTone::Needs),
        Stand::Passed => check("Required reviews passed", CheckTone::Done),
        Stand::Half => check(
            format!("Awaiting {}", awaiting.map_or("review", awaited)),
            CheckTone::Needs,
        ),
    })
}

/// The gates rw's change needs: the ones its newest landing recorded when it was checked (`required_gates`), the chain
/// default before it has submitted any.
fn required_of<'a>(landings: &'a [Landing], rw: &RepoWork) -> &'a [Gate] {
    match landings
        .iter()
        .rfind(|l| l.repo == rw.repo && l.reverts.is_none() && !l.version.gates.is_empty())
    {
        Some(l) => &l.version.gates,
        None => ChangeGates::default().gates(),
    }
}

/// What the board says a change awaits while gate g has not judged it.
const fn awaited(g: Gate) -> &'static str {
    match g {
        Gate::Reviewer => "review",
        Gate::Qc => "QC",
        Gate::Security => "security",
    }
}

/// Whether a repository in the chain requires gate, as its newest landing recorded.
fn needs_gate(c: &Chain, landings: &[Landing], gate: Gate) -> bool {
    if c.repos.is_empty() {
        return ChangeGates::default().gates().contains(&gate);
    }
    c.repos.iter().any(|rw| required_of(landings, rw).contains(&gate))
}

/// The latest verdict of each required gate on repo.
fn latest_gates<'a>(approvals: &[&'a Approval], repo: &str, required: &[Gate]) -> Vec<&'a Approval> {
    required
        .iter()
        .filter_map(|g| {
            approvals
                .iter()
                .rfind(|a| a.repo == repo && a.role == g.role())
                .copied()
        })
        .collect()
}

/// Where a reviewer's or QC's verdict a leaves its gate against current: changes requested, approved, or stale, an
/// approval of an earlier version of its repository's change than the one it has now.
pub fn verdict_state(a: &Approval, current: Option<&Changes>) -> GateState {
    if a.verdict != Verdict::Approve {
        return GateState::Changes;
    }
    match current.and_then(|c| c.get(&a.repo)) {
        Some(now) if approved_earlier(a, now) => GateState::Stale,
        _ => GateState::Approved,
    }
}

/// A change gate against current. As the gate that lets land queue the change, the latest verdict of its role in each
/// repository stands, and the gate is as bad as its worst repository.
fn change_gate(
    gate: Gate,
    approvals: &[&Approval],
    current: Option<&Changes>,
    c: &Chain,
    landings: &[Landing],
) -> GateView {
    let role = gate.role();
    let mut g = GateView {
        key: gate_key(gate).to_owned(),
        label: gate_label(gate).to_owned(),
        ..GateView::default()
    };
    let mut standing: HashMap<&str, &Approval> = HashMap::new();
    for a in approvals.iter().filter(|a| a.role == role) {
        standing.insert(a.repo.as_str(), a);
        g.rounds.push(a.verdict == Verdict::Approve);
    }
    let mut missing_evidence = false;
    for a in standing.values() {
        let required = c
            .repos
            .iter()
            .find(|rw| rw.repo == a.repo)
            .map_or(ChangeGates::default().gates(), |rw| required_of(landings, rw));
        let state = verdict_state(a, current);
        let st = if state == GateState::Approved && review_needs_evidence(a, required) {
            missing_evidence = true;
            GateState::None
        } else {
            state
        };
        if gate_rank(st) > gate_rank(g.state) {
            g.state = st;
        }
    }
    if missing_evidence && gate_rank(g.state) <= gate_rank(GateState::Approved) {
        g.state = GateState::None;
        g.line = "Needs behaviour evidence".to_owned();
        return g;
    }
    g.line = gate_line(g.state).to_owned();
    let sided: Vec<String> = judges_of(approvals, role)
        .into_iter()
        .filter(|j| verdict_state(j.latest, current) == g.state)
        .map(|j| j.name)
        .collect();
    if let Some(first) = sided.first() {
        g.who.clone_from(first);
        if sided.len() > 1 {
            g.who = format!("{first} +{}", sided.len().saturating_sub(1));
        }
    }
    g
}

/// The key the owner's page knows a change gate by.
const fn gate_key(g: Gate) -> &'static str {
    match g {
        Gate::Reviewer => "review",
        Gate::Qc => "qc",
        Gate::Security => "security",
    }
}

/// The goal judge's gate: running while it works, then met or not by the latest check.
fn goal_gate(c: &Chain, approvals: &[&Approval]) -> GateView {
    let mut g = GateView {
        key: "goal".to_owned(),
        label: "Goal check".to_owned(),
        ..GateView::default()
    };
    let mut last = None;
    for a in approvals.iter().filter(|a| a.role == VerdictRole::Judge) {
        g.rounds.push(a.verdict == Verdict::Approve);
        last = Some(*a);
    }
    let (state, line, who) = match (&c.finishing, last) {
        (Some(f), _) => {
            g.watch = (!f.judge.is_empty()).then(|| f.judge.clone());
            (GateState::Running, "Checking the result", "")
        }
        (None, None) if *c.status() == ChainStatus::Done => (GateState::None, "Not run", ""),
        (None, None) => (GateState::None, "Not asked yet", ""),
        (None, Some(a)) if a.verdict == Verdict::Approve => (GateState::Approved, "Goal met", a.name.as_str()),
        (None, Some(a)) => (GateState::Changes, "Not met", a.name.as_str()),
    };
    g.state = state;
    line.clone_into(&mut g.line);
    who.clone_into(&mut g.who);
    g
}

/// One judge's verdicts: the latest, and the earlier rounds newest first.
struct Judged<'a> {
    key: String,
    name: String,
    repo: String,
    latest: &'a Approval,
    earlier: Vec<&'a Approval>,
}

/// Who gave verdicts as role, newest first. A reviewer or QC is one judge per repository, its earlier verdicts the rounds
/// of the same review; two reviewers working side by side are two judges. The goal checks are one judge, the rounds of one
/// question asked again at every finish, though each is a fresh agent.
fn judges_of<'a>(approvals: &[&'a Approval], role: VerdictRole) -> Vec<Judged<'a>> {
    let mut keys: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&'a Approval>> = HashMap::new();
    for a in approvals.iter().filter(|a| a.role == role) {
        let key = if role == VerdictRole::Judge {
            "checks".to_owned()
        } else {
            format!("{}|{}", a.repo, a.card_id)
        };
        if !groups.contains_key(&key) {
            keys.push(key.clone());
        }
        groups.entry(key).or_default().push(a);
    }
    let mut out: Vec<Judged<'a>> = keys
        .into_iter()
        .filter_map(|k| {
            let list = groups.remove(&k)?;
            let (latest, before) = list.split_last()?;
            let first = list.first()?;
            let name = if role == VerdictRole::Judge {
                GOAL_JUDGE_NAME.to_owned() // each check is named by its number; the judge of them all is not
            } else {
                latest.name.clone()
            };
            Some(Judged {
                key: k,
                name,
                repo: first.repo.clone(),
                latest,
                earlier: before.iter().rev().copied().collect(),
            })
        })
        .collect();
    out.sort_by_key(|x| std::cmp::Reverse(x.latest.at)); // stable: judges given at one instant keep their order
    out
}

/// How one verdict reads, in state (`verdict_state`'s; the goal judge's verdicts are on the chain, not on a version of a
/// change): the goal judge's approval means the goal is met, a gate's that its change passes, unless the change has moved
/// since.
pub fn verdict_check(a: &Approval, state: GateState) -> CheckLook {
    let ok = a.verdict == Verdict::Approve;
    if a.role == VerdictRole::Judge {
        return if ok {
            check("Goal met", CheckTone::Done)
        } else {
            check("Goal not met", CheckTone::Needs)
        };
    }
    if state == GateState::Stale {
        return check(gate_line(GateState::Stale), CheckTone::Needs);
    }
    if ok {
        return check("Approved", CheckTone::Done);
    }
    check("Changes requested", CheckTone::Needs)
}

/// How verdict a reads, in state, against the rounds the same judge gave before it, newest first.
fn verdict_of(a: &Approval, state: GateState, before: &[&Approval]) -> VerdictView {
    let (headline, mut rest) = split_notes(&a.notes);
    if let Some(evidence) = &a.behaviour_evidence {
        if !rest.is_empty() {
            rest.push_str("\n\n");
        }
        rest.push_str("Behaviour evidence\n");
        rest.push_str(evidence);
    }
    let more = rest.split('\n').filter(|l| !l.trim().is_empty()).count();
    VerdictView {
        id: a.id.clone(),
        card_id: a.card_id.clone(),
        check: verdict_check(a, state),
        headline,
        rest,
        more,
        findings: a.findings.clone(),
        moves: moves_of(a, before),
        at: a.at,
    }
}

/// The items a differs on from the round just before it (`before[0]`): the statuses that changed, the items that are new,
/// and the ones it no longer lists. The first round differs from nothing. Items are compared by their words, which a judge
/// keeps from one check to the next (its brief says so).
fn moves_of(a: &Approval, before: &[&Approval]) -> Vec<Move> {
    let Some(prev) = before.first() else { return Vec::new() };
    let was: HashMap<&str, FindingStatus> = prev.findings.iter().map(|f| (f.item.as_str(), f.status)).collect();
    let now: BTreeSet<&str> = a.findings.iter().map(|f| f.item.as_str()).collect();
    let mut moves = Vec::new();
    for f in &a.findings {
        match was.get(f.item.as_str()) {
            None => moves.push(Move {
                item: f.item.clone(),
                from: None,
                to: Some(f.status),
            }),
            Some(from) if *from != f.status => {
                moves.push(Move {
                    item: f.item.clone(),
                    from: Some(*from),
                    to: Some(f.status),
                });
            }
            Some(_) => {}
        }
    }
    for f in prev.findings.iter().filter(|f| !now.contains(f.item.as_str())) {
        moves.push(Move {
            item: f.item.clone(),
            from: Some(f.status),
            to: None,
        });
    }
    moves
}

/// Whether line is a Markdown heading: one to six `#` and a space.
fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && line.chars().nth(hashes).is_some_and(|c| c.is_ascii_whitespace())
}

/// What a verdict says first, and the proof behind it. The first line is the verdict; when that is only a heading, the
/// line after it comes too. Agents often write no blank lines, so the split is by line, not by paragraph.
fn split_notes(notes: &str) -> (String, String) {
    let lines: Vec<&str> = notes.trim().split('\n').collect();
    let mut end = 1;
    if lines.first().is_some_and(|l| is_heading(l))
        && let Some(i) = lines.iter().skip(1).position(|l| !l.trim().is_empty())
    {
        end = i.saturating_add(2);
    }
    let (head, rest) = lines.split_at(end.min(lines.len()));
    (head.join("\n").trim().to_owned(), rest.join("\n").trim().to_owned())
}

/// The approvals of chain c.
pub fn of_chain<'a>(c: &Chain, approvals: &'a [Approval]) -> Vec<&'a Approval> {
    approvals.iter().filter(|a| a.chain_id == c.id).collect()
}

/// A repository's last path segment, as the owner names it.
fn repo_name(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}
