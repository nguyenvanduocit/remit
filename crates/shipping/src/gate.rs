//! No change lands unchecked. Before the queue lands a version of a repository's change, the gates its chain names must
//! approve that exact change: an independent reviewer checks its diff and behaviour, with a separate QC gate for a distinct
//! check when requested (ADR-0131). A change that touches
//! a rail the repository declares in `.remit/rails` needs security too, on the same change (ADR-0087): which gates a
//! change needs is a pure function of the chain's gates and the paths it touches (`required_gates`), read the same way by
//! land and by the verdict that lets a landing into the queue. Verdicts are tied to the change's patch id, so
//! any edit after an approval needs a new one; rebasing or rewriting authors does not. Judges cannot edit files. The goal
//! judge (goal.rs) signs the chain's result the same way, as a verdict on the chain instead of on a change.

use std::collections::HashSet;
use std::sync::Arc;

use model::{
    Actor, Approval, Card, Cause, Chain, ChainStatus, ChangeGates, DecisionKind, Edge, EdgeKind, Finding,
    FindingBlocker, FindingStatus, Gate, InboxItem, InboxKind, InboxOption, InboxStatus, Landing, LandingState,
    ReasonCode, RepoWork, Verdict, VerdictRole, by_agent,
};
use serde::Deserialize;

use crate::Shipping;
use crate::goal::not_met;
use crate::land::{approve, last_element, not_open_text, work_named};
use capability::judge_store::store_access_error;
use chains::apply::Records;
use chains::chain_core::{first_line, new_id};
use chains::forges::{FORGE_TIMEOUT, Forge, remote_of};
use chains::gate::still_missing;
use chains::git::{git, repo_command, shell, stderr_of};
use chains::handover::HANDOVER_EDGE;
use chains::live::LandingWrite;
use chains::step::Decided;
use chains::{Error, Result};

/// Where a repository declares its rails: one path glob a line, `#` starts a comment. The file is a rail itself, so a
/// change to the list is judged as a change to what it guards.
pub const RAILS_FILE: &str = ".remit/rails";

/// How many times one gate may ask for changes to one repository before the owner is asked whether the loop should go on
/// (as `MAX_GOAL_CHECKS` limits the goal judge). Agents that cannot settle in three rounds are not converging on their own;
/// the chain keeps working while he decides, and the verdicts go on holding the landing back.
pub(crate) const MAX_GATE_ROUNDS: usize = 3;

/// Identifies the content of a branch's change, all of it committed: a verdict covers exactly what will be published.
/// base is the revision the change is measured from: the base branch's commit on the forge (`forge_base`).
pub async fn change_id(dir: &str, base: &str) -> Result<String, String> {
    if !git(dir, &["status", "--porcelain"]).await?.is_empty() {
        return Err(
            "the worktree has uncommitted changes; commit them first so the verdict covers exactly what will be \
                    published"
                .to_owned(),
        );
    }
    let id = patch_id(dir, base, "HEAD").await?;
    if id.is_empty() {
        return Err(format!("the branch changes nothing beyond its base ({base})"));
    }
    Ok(id)
}

/// The environment a read whose answer a gate rests on adds to the one childenv names. The clone's refs and git directory
/// are the agent's to write, so a replacement object or a graft (`info/grafts`, which `--no-replace-objects` does not
/// turn off) would make git read other content or other parents than the commits hold; these reads take neither.
const PLAIN_ENV: [(&str, &str); 2] = [("GIT_NO_REPLACE_OBJECTS", "1"), ("GIT_GRAFT_FILE", "/dev/null")];

/// git with args in dir, read as `PLAIN_ENV` says, and what it printed on its output, trimmed.
async fn plain_git(dir: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = repo_command(dir, "git", args);
    cmd.envs(PLAIN_ENV);
    let out = cmd
        .output()
        .await
        .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}{}",
            args.join(" "),
            out.status,
            stderr_of(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Identifies the content of the change commit rev makes on its branch: the stable patch id of its diff from where it left
/// base (a revision: the forge's base commit, `forge_base`), "" when it changes nothing.
pub async fn patch_id(dir: &str, base: &str, rev: &str) -> Result<String, String> {
    let mb = plain_git(dir, &["merge-base", base, rev]).await?;
    let out = shell(
        dir,
        &format!(
            "{plain} git diff --no-ext-diff --no-textconv {mb} {rev} | git patch-id --stable",
            plain = PLAIN_ENV.map(|(k, v)| format!("{k}={v}")).join(" ")
        ),
    )
    .output()
    .await
    .map_err(|err| format!("patch id: {err}"))?;
    if !out.status.success() {
        return Err(format!("patch id: {}{}", out.status, stderr_of(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned())
}

/// Where a repository declares how what lands is released (ADR-0116): every path under it is a rail by code, as the
/// rails file is, since a procedure publishes under the owner's accounts.
pub const RELEASE_DIR: &str = ".remit/release/";

/// The gates a change of a chain whose gates are base needs, from the paths it touches and the text of the repository's
/// rails file: base always, and security when a path matches a rail, is the rails file itself or is under the release
/// procedure. Pure.
pub(crate) fn required_gates(base: ChangeGates, changed: &[&str], rails: &str) -> Vec<Gate> {
    let globs: Vec<&str> = rails
        .lines()
        .map(|l| l.split_once('#').map_or(l, |(before, _)| before).trim())
        .filter(|l| !l.is_empty())
        .collect();
    let touches_rail = changed
        .iter()
        .any(|p| *p == RAILS_FILE || p.starts_with(RELEASE_DIR) || globs.iter().any(|g| rail_matches(g, p)));
    let mut gates = base.gates().to_vec();
    if touches_rail {
        gates.push(Gate::Security);
    }
    gates
}

/// Whether path is under rail glob g: `*` matches within one path element, `**` across elements, and a glob that names a
/// directory (`crates/auth`, `crates/auth/`) covers everything beneath it. Pure.
fn rail_matches(g: &str, path: &str) -> bool {
    let g = g.trim_start_matches("./").trim_end_matches('/');
    glob_path(g, path) || glob_path(&format!("{g}/**"), path)
}

/// The whole of s against path glob p.
fn glob_path(p: &str, s: &str) -> bool {
    if let Some(rest) = p.strip_prefix("**") {
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        return s
            .char_indices()
            .map(|(i, _)| i)
            .chain([s.len()])
            .filter(|i| rest.is_empty() || *i == 0 || s.get(..*i).is_some_and(|b| b.ends_with('/')))
            .any(|i| s.get(i..).is_some_and(|tail| glob_path(rest, tail)));
    }
    if let Some(rest) = p.strip_prefix('*') {
        let run = s.find('/').unwrap_or(s.len());
        return (0..=run).any(|i| s.get(i..).is_some_and(|tail| glob_path(rest, tail)));
    }
    let mut pc = p.chars();
    let Some(first) = pc.next() else { return s.is_empty() };
    let matched = if first == '?' {
        s.chars().next().filter(|c| *c != '/')
    } else {
        s.chars().next().filter(|c| *c == first)
    };
    matched.is_some_and(|c| glob_path(pc.as_str(), s.get(c.len_utf8()..).unwrap_or_default()))
}

/// The text of the rails file base holds, empty when it holds none. Read from the base so a change cannot loosen the list
/// that judges it.
pub async fn rails_of(dir: &str, base: &str) -> Result<String, String> {
    let rails_at = format!("{base}:{RAILS_FILE}");
    if plain_git(dir, &["cat-file", "-e", &rails_at]).await.is_ok() {
        plain_git(dir, &["cat-file", "blob", &rails_at]).await
    } else {
        Ok(String::new())
    }
}

/// The gates the change rev makes on its branch needs, of a chain whose gates are gates (`required_gates`): the paths it touches since it left base (the
/// forge's base commit, `forge_base`), against the rails (`rails_of`) the base declares. The rails file itself always
/// counts. A file moved counts at both its paths, so a rail's file moved out of the rail is a change to the rail.
pub async fn gates_of(gates: ChangeGates, dir: &str, base: &str, rev: &str, rails: &str) -> Result<Vec<Gate>, String> {
    let mb = plain_git(dir, &["merge-base", base, rev]).await?;
    let changed = plain_git(dir, &["diff", "--no-renames", "-z", "--name-only", &mb, rev]).await?;
    let paths: Vec<&str> = changed.split('\0').filter(|p| !p.is_empty()).collect();
    Ok(required_gates(gates, &paths, rails))
}

/// Whether s is written as a commit id: 40 or 64 hex digits. Pure.
fn commit_id(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// How long `base_on` lets the clone fetch its base: the clone's origin is the agent's to point anywhere, so a fetch that
/// never ends would hold land and a verdict with it.
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The commit rw's base branch stands at on its forge f, as the forge says it: never what the clone's origin says, whose
/// URL, fetch rule and refs the agent can write. It is fetched into the clone when the clone lacks it, within
/// FETCH_TIMEOUT; a clone whose origin does not bring it is refused, as what it fetches is not the base. A commit is named
/// by its content, so once the clone holds it, what git reads of it is the forge's.
pub(crate) async fn base_on(f: &Arc<dyn Forge>, rw: &RepoWork) -> Result<String, String> {
    let at = match tokio::time::timeout(FORGE_TIMEOUT, f.branch_head(&remote_of(rw), &rw.base)).await {
        Ok(Ok(at)) => at,
        Ok(Err(err)) => return Err(format!("{} did not say where {} stands: {err}", f.name(), rw.base)),
        Err(_) => {
            return Err(format!(
                "{} did not say where {} stands in {}s",
                f.name(),
                rw.base,
                FORGE_TIMEOUT.as_secs()
            ));
        }
    };
    if !commit_id(&at) {
        return Err(format!("{} named {at:?} as the commit {} stands at", f.name(), rw.base));
    }
    let commit = format!("{at}^{{commit}}");
    if plain_git(&rw.path, &["cat-file", "-e", &commit]).await.is_ok() {
        return Ok(at);
    }
    // git runs in a process group of its own, so the fetch dropped at the deadline ends with everything it started
    let fetched = tokio::time::timeout(FETCH_TIMEOUT, git(&rw.path, &["fetch", "-q", "origin", &rw.base]))
        .await
        .unwrap_or_else(|_| {
            Err(format!(
                "the fetch of {} took more than {}s",
                rw.base,
                FETCH_TIMEOUT.as_secs()
            ))
        });
    if plain_git(&rw.path, &["cat-file", "-e", &commit]).await.is_ok() {
        return Ok(at);
    }
    Err(format!(
        "{} stands at {at} on {}, and the clone's origin does not bring that commit, so the clone's origin is not {}{}",
        rw.base,
        f.name(),
        rw.project,
        fetched.err().map(|e| format!(": {e}")).unwrap_or_default()
    ))
}

/// What a gate is called where the owner reads it.
pub(crate) const fn gate_label(g: Gate) -> &'static str {
    match g {
        Gate::Reviewer => "Review",
        Gate::Qc => "QC",
        Gate::Security => "Security",
    }
}

/// A verdict as record_verdict takes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VerdictInput {
    /// repository as the Chair opened it; empty when the chain has exactly one repository open, and for the goal judge
    #[serde(default)]
    pub repo: String,
    /// approve, or request_changes
    pub verdict: Verdict,
    /// the HEAD you judged, then only what is pre-existing and what you did not check, one line each (the findings carry
    /// the proof); for request_changes, exactly what must change. A reviewer's and QC's are named by the commit the
    /// change lands as.
    #[serde(default)]
    pub notes: String,
    /// reviewer approval: the command and result or independent check evidence proving behaviour at this version
    pub behaviour_evidence: Option<String>,
    /// one per key result of the goal, and one per defect class you ask changes for: keyResult (the id of the key result it
    /// rolls up to, when your brief lists them), item (the sub-point you checked), status (met, not_met or unverified),
    /// evidence (the one proof in a line: file:line, a command or a query and what it returned, or why it could not be checked) and, for
    /// unverified only, blocker: write, deploy, person, access or watch. When you object, code is why
    /// (uncovered_key_result, unverifiable_key_result, unknown_metric, overlap, guardrail_breach, evidence_contradicts,
    /// process_constraint, duplicate, untried or other); evidence_contradicts also needs refs, the platform records that
    /// contradict the evidence, each {kind: source_call, reading or event, id}. Every verdict gives findings: approve means
    /// none is not_met, request_changes needs one that is
    #[serde(default)]
    #[schemars(with = "Vec<serde_json::Value>")]
    pub findings: Vec<Finding>,
}

/// What is wrong with findings: each names its item and its proof, an unverified one alone its blocker, and its references
/// the records they point at.
pub(crate) fn findings_error(fs: &[Finding]) -> Result<(), String> {
    for (i, f) in (1_usize..).zip(fs) {
        let item = &f.item;
        if f.item.trim().is_empty() {
            return Err(format!(
                "finding {i} names no item: say which sub-point of the key result you checked"
            ));
        }
        if f.evidence.trim().is_empty() {
            return Err(format!(
                "finding {i} ({item}) has no evidence: give file:line, command output, or why it could not be checked"
            ));
        }
        if f.status == FindingStatus::Unverified && f.blocker.is_none() {
            return Err(format!(
                "finding {i} ({item}) is unverified: name its blocker, write (a landing, a migration, a write to a shared \
                 store), deploy, person (a product or owner decision) access (data or a system the team cannot reach) or \
                 watch (a watch on a signal rules on it and has not met its condition)"
            ));
        }
        if f.status != FindingStatus::Unverified && f.blocker.is_some() {
            return Err(format!(
                "finding {i} ({item}) is {}: only an unverified finding names a blocker",
                f.status
            ));
        }
        if f.refs.iter().any(|r| !r.valid()) {
            return Err(format!(
                "finding {i} ({item}): refs must each be {{kind: source_call, reading or event, id: the platform record's id}}"
            ));
        }
        if f.code == Some(ReasonCode::EvidenceContradicts) && f.refs.is_empty() {
            return Err(format!(
                "finding {i} ({item}) is coded evidence_contradicts: give the refs, the platform records (a source call, a \
                 reading or an event) that contradict the evidence"
            ));
        }
    }
    Ok(())
}

/// Whether a verdict follows from its findings, for a gate's and the goal judge's alike: a change or a goal is approved only
/// when nothing the team could do is left, and a refusal names what is.
pub(crate) fn verdict_findings_error(
    verdict: Verdict,
    findings: &[Finding],
    data_tools: &[String],
) -> Result<(), String> {
    judged_findings_error(findings, data_tools)?;
    let missing = not_met(findings).len();
    if verdict == Verdict::Approve && missing > 0 {
        return Err(
            "approve means every key result is met or cannot be checked by the team, but a finding is not_met: \
                    request_changes instead"
                .to_owned(),
        );
    }
    if verdict == Verdict::RequestChanges && missing == 0 {
        return Err("request_changes needs a finding that is not_met: name what the team must still do".to_owned());
    }
    Ok(())
}

/// Whether a verdict's findings are well formed and checked with what the judge holds: a chain that reads data with
/// data_tools is never unverified for access, since its judge holds them too.
pub(crate) fn judged_findings_error(findings: &[Finding], data_tools: &[String]) -> Result<(), String> {
    findings_error(findings)?;
    if findings.is_empty() {
        return Err("findings are required: one per key result of the goal, with its evidence".to_owned());
    }
    if data_tools.is_empty() {
        return Ok(());
    }
    if let Some((i, f)) = (1_usize..)
        .zip(findings)
        .find(|(_, f)| f.blocker == Some(FindingBlocker::Access))
    {
        return Err(format!(
            "finding {i} ({}) is unverified for access, but this chain reads data with {}, and so do you: run the query \
             and mark it met with what it returned, or not_met naming the query the team must run",
            f.item,
            data_tools.join(", ")
        ));
    }
    Ok(())
}

/// Whether the goal judge's verdict follows from its findings (ADR-0067). Its approve ends the chain, so it means the goal
/// is done, or waits only on what owners names, the landing or the deploy of the chain's change: what is left unproven
/// otherwise is the Chair's to settle, a request_changes. data_tools and store_tools are the readers the judge holds: what
/// they reach is never unverified for access.
pub(crate) fn goal_verdict_error(
    verdict: Verdict,
    findings: &[Finding],
    data_tools: &[String],
    store_tools: &[&str],
    owners: &[FindingBlocker],
) -> Result<(), String> {
    judged_findings_error(findings, data_tools)?;
    store_access_error(findings, store_tools)?;
    let missing = still_missing(findings, owners);
    if verdict == Verdict::Approve
        && let Some(f) = missing.first()
    {
        if f.status == FindingStatus::NotMet {
            return Err(format!(
                "approve ends the chain, but finding {:?} is not_met: request_changes instead",
                f.item
            ));
        }
        let blocker = f.blocker.map_or("", |b| b.as_str());
        if f.blocker.is_some_and(|b| merge_or_deploy(&b)) {
            return Err(format!(
                "approve ends the chain, but finding {:?} waits on {}, and nothing of the chain is left to land or deploy \
                 (it lands no change, or its change is marked live): prove it with your tools and mark it met or \
                 not_met, or request_changes with it",
                f.item, blocker
            ));
        }
        return Err(format!(
            "approve ends the chain, so it leaves nothing unproven but the landing or the deploy of the chain's change; \
             finding {:?} is unverified on {}: prove it with your tools and mark it met or not_met, or request_changes with \
             it, and the Chair settles it (proves it, or asks the owner and subscribes)",
            f.item, blocker
        ));
    }
    if verdict == Verdict::RequestChanges && missing.is_empty() {
        return Err(
            "request_changes needs a finding the Chair must still settle: not_met, or unverified on what is not the \
                    landing or the deploy of the chain's change"
                .to_owned(),
        );
    }
    Ok(())
}

/// Whether blocker b is the landing (write) or a deploy: settled after the chain ends, while it holds a landing.
pub(crate) fn merge_or_deploy(b: &FindingBlocker) -> bool {
    matches!(b, FindingBlocker::Write | FindingBlocker::Deploy)
}

/// Whether a chain whose landings are landings has a change on its way to its base, or on it.
pub(crate) fn holds_landing(landings: &[Landing]) -> bool {
    landings
        .iter()
        .any(|l| l.reverts.is_none() && (l.state.open() || l.on_base()))
}

/// How many times a change by one card is sent back before the card is raised a tier (ADR-0118, the climb): a card that
/// two rounds of a gate did not bring to approval may have been given less than its work needs. Fewer than
/// `MAX_GATE_ROUNDS`, which asks the owner, so the climb is tried first.
pub(crate) const CLIMB_AFTER_ROUNDS: usize = 2;

/// How many distinct changes the role has asked changes to in the repository since card `author` began: the verdicts of
/// the reviewers that judge its work (the cards its handoff edges reach), counted once for each reviewer and change, so
/// a verdict recorded again by a retry is not another round, and a card raised from another starts with none. Pure.
pub(crate) fn sent_back(approvals: &[Approval], edges: &[Edge], repo: &str, role: VerdictRole, author: &Card) -> usize {
    let judges: HashSet<&str> = edges
        .iter()
        .filter(|e| e.from == author.id && e.kind == EdgeKind::Handoff && e.label != HANDOVER_EDGE)
        .map(|e| e.to.as_str())
        .collect();
    approvals
        .iter()
        .filter(|a| a.repo == repo && a.role == role && a.verdict == Verdict::RequestChanges)
        .filter(|a| a.at >= author.created_at && judges.contains(a.card_id.as_str()))
        .map(|a| (a.card_id.as_str(), a.change.as_str()))
        .collect::<HashSet<_>>()
        .len()
}

/// How many times the role has asked for changes to the repository so far.
pub(crate) fn gate_rounds(approvals: &[Approval], repo: &str, role: VerdictRole) -> usize {
    approvals
        .iter()
        .filter(|a| a.repo == repo && a.role == role && a.verdict == Verdict::RequestChanges)
        .count()
}

impl Shipping {
    /// The commit rw's base branch stands at on its forge (`base_on`).
    pub(crate) async fn forge_base(&self, rw: &RepoWork) -> Result<String, String> {
        let f = self.forge_on(&rw.host).map_err(|e| e.to_string())?;
        base_on(&f, rw).await
    }

    /// What is settled after chain ch ends: the landing (write) and the deploy of its change, until its change is marked
    /// live; nothing for a chain that lands none.
    pub(crate) async fn owners_to_settle(&self, ch: &Chain) -> Result<Vec<FindingBlocker>> {
        let id = ch.id.clone();
        let landings = self.store.read(move |r| r.chain_landings(&id)).await?;
        if !holds_landing(&landings) || self.marked_live(&ch.id).await? {
            return Ok(Vec::new());
        }
        Ok(vec![FindingBlocker::Write, FindingBlocker::Deploy])
    }

    /// What is the owner's to settle of chain ch once it has finished (`owners_to_settle`); nothing while it runs.
    pub async fn owners_of_finished(&self, ch: &Chain) -> Result<Vec<FindingBlocker>> {
        if *ch.status() != ChainStatus::Done {
            return Ok(Vec::new());
        }
        self.owners_to_settle(ch).await
    }

    /// Stores a reviewer's or QC's verdict on the current version of a repository's change, or the goal judge's verdict on
    /// the chain's result.
    pub async fn record_verdict(&self, card: &Card, i: VerdictInput) -> Result<Approval> {
        let Some(role) = card.signs.verdict_role() else {
            return Err(Error::refused(format!(
                "only a reviewer, a QC, a security reviewer or the goal judge records verdicts; {} is {:?}",
                card.name, card.agent
            )));
        };
        if i.notes.trim().is_empty() {
            return Err(Error::refused(
                "notes are required: what you checked and what you found",
            ));
        }
        if role == VerdictRole::Judge {
            return self.record_goal_verdict(card, i).await;
        }
        let home = card.home.clone();
        let chain = self.store.read(move |r| r.chain(&home)).await?;
        let Some(rw) = work_named(&chain.repos, &i.repo)
            .and_then(|n| chain.repos.get(n))
            .cloned()
        else {
            return Err(Error::refused(not_open_text(&chain.repos, &i.repo)));
        };
        let data_tools = self.reach.data_tools(&chain).await?;
        verdict_findings_error(i.verdict, &i.findings, &data_tools).map_err(Error::refused)?;
        let change = tokio::time::timeout(std::time::Duration::from_secs(120), async {
            let base = self.forge_base(&rw).await?;
            change_id(&rw.path, &base).await
        })
        .await
        .map_err(|_| Error::refused("reading the change took more than 2 minutes"))?
        .map_err(Error::refused)?;
        let a = Approval {
            id: new_id("ap_"),
            chain_id: chain.id.clone(),
            repo: rw.repo.clone(),
            change,
            card_id: card.id.clone(),
            name: card.name.clone(),
            role,
            verdict: i.verdict,
            notes: i.notes.trim().to_owned(),
            behaviour_evidence: i
                .behaviour_evidence
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            findings: i.findings,
            at: self.now(),
        };
        let what = if a.verdict == Verdict::Approve {
            "approved the change"
        } else {
            "requested changes"
        };
        let d = Decided {
            kind: DecisionKind::Verdict,
            text: format!("{} ({role}) {what} in {}.", card.name, last_element(&rw.repo)),
            why: first_line(&a.notes, 200),
            card_id: card.id.clone(),
            by: by_agent(&card.id),
        };
        let mut rec = Records {
            approvals: vec![a.clone()],
            decisions: vec![d],
            ..Records::default()
        };
        let id = chain.id.clone();
        let approvals = self.store.read(move |r| r.approvals(&id)).await?;
        let rounds = gate_rounds(&approvals, &rw.repo, role).saturating_add(1);
        if a.verdict == Verdict::RequestChanges && rounds == MAX_GATE_ROUNDS {
            rec.asked.push(self.gate_loop_question(&chain, &a));
        }
        self.commit_verdict(&chain.id, &a, by_agent(&card.id), rec).await?;
        if a.verdict == Verdict::RequestChanges {
            let mut all = approvals;
            all.push(a.clone());
            if let Err(err) = self.climb_sent_back(card, &a, &all).await {
                tracing::error!(card = %card.id, %err, "raise the cards a gate sent back");
            }
        }
        Ok(a)
    }

    /// Raises the author of the change that verdict a, which reviewer card requested changes to, once its gate has sent
    /// work of its back `CLIMB_AFTER_ROUNDS` times (ADR-0118): the one card whose model code chose among the employees
    /// whose accepted work makes up the landed version of that change. With two such authors, or none, nothing is
    /// raised: the weak model cannot be told from the change. `approvals` are the chain's, a included.
    async fn climb_sent_back(&self, reviewer: &Card, a: &Approval, approvals: &[Approval]) -> Result<()> {
        let id = a.chain_id.clone();
        let (edges, landings, cards) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.edges(&id)?, r.chain_landings(&id)?, r.cards(&id)?)))
            .await?;
        let Some(made_by) = landings
            .iter()
            .find(|l| l.repo == a.repo && l.version.change == a.change)
            .map(|l| &l.version.by)
        else {
            return Ok(());
        };
        let chosen: Vec<&Card> = cards
            .iter()
            .filter(|c| c.chosen.is_some() && made_by.contains(&c.employee))
            .collect();
        let [author] = chosen.as_slice() else {
            return Ok(());
        };
        let judged_by_reviewer = edges.iter().any(|e| {
            e.from == author.id && e.to == reviewer.id && e.kind == EdgeKind::Handoff && e.label != HANDOVER_EDGE
        });
        if !judged_by_reviewer
            || !self.core.can_climb(author)
            || sent_back(approvals, &edges, &a.repo, a.role, author) < CLIMB_AFTER_ROUNDS
        {
            return Ok(());
        }
        let why = format!(
            "its {} asked for changes {CLIMB_AFTER_ROUNDS} times: {}",
            a.role,
            first_line(&a.notes, 200)
        );
        self.core.climb_cards(vec![author.id.clone()], &why);
        Ok(())
    }

    /// Stores a verdict's records. The judging landing of the change the verdict is on is a reading moved by it, and it
    /// enters the queue when the verdict is the last its gates needed (or contends in its round, for an attempt's: its
    /// round is then moved on). The chain's lock is held from reading the
    /// landing and the verdicts to storing them, so a land in between cannot be overwritten by it.
    pub(crate) async fn commit_verdict(&self, chain_id: &str, a: &Approval, by: Actor, mut rec: Records) -> Result<()> {
        let cs = self.state(chain_id).await?;
        let held = cs.held.lock().await;
        if held.gone {
            return Err(Error::ChainDeleted);
        }
        let id = chain_id.to_owned();
        let (mut approvals, landings) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.approvals(&id)?, r.chain_landings(&id)?)))
            .await?;
        approvals.push(a.clone());
        let round = self.round_of(&held.chain).await?;
        let judged = landings
            .into_iter()
            .find(|l| l.state == LandingState::Judging && l.repo == a.repo && l.version.change == a.change);
        let (mut ready, mut contends) = (None, false);
        if let Some(mut l) = judged {
            if approve(&mut l, &approvals, self.cfg.require_approvals, round.as_ref(), a.at) {
                contends = l.state == LandingState::Contending;
                ready = (l.state == LandingState::Ready).then(|| l.queue());
            }
            l.updated_at = a.at;
            rec.landings.push(LandingWrite {
                landing: l,
                cause: Cause {
                    actor: by,
                    chain_id: chain_id.to_owned(),
                },
            });
        }
        self.commit(None, None, rec).await?;
        drop(held);
        self.kick(); // the watches on the landing judge the verdict on the next pass
        if let Some(q) = ready {
            self.kick_queue(q);
        }
        if contends {
            self.judge_rounds().await;
        }
        Ok(())
    }

    /// Asks the owner whether a gate that has asked for changes `MAX_GATE_ROUNDS` times should go on, on the Chair's card:
    /// the Assistant answers it for him (ADR-0087), and the Chair carries the answer back.
    pub(crate) fn gate_loop_question(&self, chain: &Chain, last: &Approval) -> InboxItem {
        let question = format!(
            "{} ({}) has asked for changes to {} {MAX_GATE_ROUNDS} times",
            last.name,
            last.role,
            last_element(&last.repo)
        );
        InboxItem {
            id: new_id("i_"),
            home: chain.id.clone(),
            card_id: chain.chair_id.clone(),
            kind: InboxKind::Question,
            title: question.clone(),
            detail: format!(
                "{question}. Its last words: {}\n\nAgents that cannot settle a change in this many rounds are not \
                 converging on their own. Say whether they should keep going, or what should change: the goal, the \
                 approach, or the standard the gate holds.",
                first_line(&last.notes, 300)
            ),
            options: vec![
                InboxOption {
                    id: "o1".to_owned(),
                    name: "Keep going".to_owned(),
                    ..InboxOption::default()
                },
                InboxOption {
                    id: "o2".to_owned(),
                    name: "Stop and tell me what is in the way".to_owned(),
                    ..InboxOption::default()
                },
            ],
            status: InboxStatus::Open,
            created_at: self.now(),
            ..InboxItem::default()
        }
    }
}
