//! The plugin's poll: the issues of a repository opened or changed since the cursor, oldest change first. An item is an issue,
//! never a pull request; its id is owner/name#number and its version the issue's updated_at, so an issue edited, commented on,
//! labelled, closed or reopened is a new event. An empty cursor answers no issue and marks the newest change among the issues
//! that match (none: the zero stamp, after which every issue that appears is new). The cursor moves over the pull requests it
//! skips too, so a busy stream of them never holds the issues back.

use jiff::{SignedDuration, Timestamp};
use plugin_sdk::{Error, PollAnswer, PollItem};
use serde::Serialize;
use source::{POLL_TEXT_MAX, PollMark, Query, cut_bytes, redact_text, render, rfc3339};

use super::{Filter, Github, Issue, PAGE, repo_path, set};

/// The most requests one poll makes: up to 1000 issues looked at; a longer backlog is read by the next.
const POLL_REQUESTS: usize = 10;

/// What a sensor reads: one repository's issues (`owner/name`), narrowed by state and labels (GitHub's query parameters). The
/// grammar is `owner/name`, then any of `label:<name>` (repeated, all must match; a name with spaces in double quotes) and
/// `state:open|closed|all` (default open), separated by spaces.
pub(super) fn parse(query: &str) -> Result<(String, Query), String> {
    const GRAMMAR: &str = "owner/name, then label:<name> (repeatable) and state:open|closed|all";
    let terms = terms(query)?;
    let (repo, rest) = terms
        .split_first()
        .ok_or_else(|| format!("the query is empty: it is {GRAMMAR}"))?;
    repo_path(repo).map_err(|e| format!("{e}; the query is {GRAMMAR}"))?;
    let mut f = Filter::default();
    let mut stated = false;
    for term in rest {
        let (name, value) = term.split_once(':').unwrap_or((term.as_str(), ""));
        match name {
            "label" => f.labels.push(value.to_owned()),
            "state" if !stated => {
                value.clone_into(&mut f.state);
                stated = true;
            }
            "state" => return Err("the query names state twice".to_owned()),
            _ => {
                return Err(format!(
                    "the term {:?} is not one the query takes: it is {GRAMMAR}",
                    source::shorten(term, 40)
                ));
            }
        }
    }
    Ok((repo.clone(), f.values()?))
}

/// The space-separated terms of `q`; double quotes keep spaces inside a term and are dropped.
fn terms(q: &str) -> Result<Vec<String>, String> {
    let mut terms = Vec::new();
    let mut term = String::new();
    let (mut quoted, mut started) = (false, false);
    for c in q.chars() {
        if c == '"' {
            quoted = !quoted;
            started = true;
        } else if c.is_whitespace() && !quoted {
            if started {
                terms.push(std::mem::take(&mut term));
                started = false;
            }
        } else {
            term.push(c);
            started = true;
        }
    }
    if quoted {
        return Err("the query has a quote that is not closed".to_owned());
    }
    if started {
        terms.push(term);
    }
    Ok(terms)
}

impl Github {
    pub(super) async fn poll(&self, query: &str, cursor: &str, limit: i64) -> Result<PollAnswer, Error> {
        let (name, mut params) = parse(query)?;
        let repo = format!("/repos/{name}");
        set(&mut params, "sort", "updated".into());
        set(&mut params, "per_page", PAGE.to_string());
        if cursor.is_empty() {
            set(&mut params, "direction", "desc".into());
            let mut mark = PollMark::default();
            for is in self.list_issues(&repo, &params).await? {
                mark.take(updated(&is), &item_id(&name, &is));
            }
            return Ok(PollAnswer {
                items: Vec::new(),
                cursor: mark.to_string(),
            });
        }
        let mut mark = PollMark::parse(cursor)?;
        set(&mut params, "direction", "asc".into());
        let limit = usize::try_from(limit).ok().filter(|n| *n > 0);
        let mut items = Vec::new();
        let mut page = 1_i64;
        for _ in 0..POLL_REQUESTS {
            // The read starts a second before the mark, whether GitHub's since counts the instant itself or not; what was handed
            // over is skipped by the mark. It pages by time, so an issue changed while the poll reads moves ahead and is not
            // skipped.
            if !mark.is_zero() {
                let since = mark.at().checked_sub(SignedDuration::from_secs(1)).unwrap_or(mark.at());
                set(&mut params, "since", rfc3339(since));
            }
            set(&mut params, "page", page.to_string());
            let list = self.list_issues(&repo, &params).await?;
            let mut fresh = 0_usize;
            for is in &list {
                let id = item_id(&name, is);
                if mark.handed(updated(is), &id) {
                    continue;
                }
                fresh = fresh.saturating_add(1);
                mark.take(updated(is), &id);
                if is.pull_request.is_some() {
                    continue;
                }
                items.push(item(&name, id, is)?);
                if limit == Some(items.len()) {
                    return Ok(PollAnswer {
                        items,
                        cursor: mark.to_string(),
                    });
                }
            }
            if i64::try_from(list.len()).unwrap_or(i64::MAX) < PAGE {
                break;
            }
            // A full page of what was handed over (many changes in one second) reads on in it; otherwise the next read starts
            // again from the moved mark.
            page = if fresh == 0 { page.saturating_add(1) } else { 1 };
        }
        Ok(PollAnswer {
            items,
            cursor: mark.to_string(),
        })
    }
}

/// When the issue last changed; an issue without the stamp is as old as the zero mark.
fn updated(is: &Issue) -> Timestamp {
    is.updated_at.unwrap_or(PollMark::default().at())
}

fn item_id(repo: &str, is: &Issue) -> String {
    format!("{repo}#{}", is.number)
}

/// What an event of an issue carries: enough to decide and to link, the body cut at `POLL_TEXT_MAX`.
#[derive(Debug, Serialize)]
struct Body {
    repo: String,
    number: i64,
    title: String,
    state: String,
    labels: Vec<String>,
    user: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    author_association: String,
    html_url: String,
    created_at: String,
    updated_at: String,
    body: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    body_cut: bool,
}

fn item(repo: &str, id: String, is: &Issue) -> Result<PollItem, Error> {
    let text = redact_text(&is.body);
    let body = cut_bytes(&text, POLL_TEXT_MAX);
    let stamp = |t: Option<Timestamp>| rfc3339(t.unwrap_or(PollMark::default().at()));
    let raw = render(&Body {
        repo: repo.to_owned(),
        number: is.number,
        title: is.title.clone(),
        state: is.state.clone(),
        labels: is.label_names(),
        user: is.user.login.clone(),
        author_association: is.association.clone(),
        html_url: is.html_url.clone(),
        created_at: stamp(is.created_at),
        updated_at: stamp(is.updated_at),
        body: body.to_owned(),
        body_cut: body.len() < text.len(),
    })?;
    Ok(PollItem {
        id,
        version: stamp(is.updated_at),
        body: serde_json::from_str(&raw)?,
    })
}
