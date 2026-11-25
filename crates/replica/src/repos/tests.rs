//! The order a repository's view is held in, driven as the core drives it: a tab opens the repository (`repo_get` goes
//! out), the server's `repo_updated` pushes and the answer arrive in either order, and a snapshot reads it again. What the
//! tab is told is the model's rows after each patch the module writes.

use serde_json::{Value, json};

use super::*;
use crate::models::Patch;

/// A view of `repo` told apart by the base of its one queue.
fn view(repo: &str, base: &str) -> Value {
    json!({ "repo": repo, "queues": [{ "base": base, "landings": [] }], "releases": [] })
}

fn pushed(record: &Value) -> Push {
    Push::repo_updated {
        view: serde_json::from_value(record.clone()).unwrap(),
    }
}

/// The rows the tab holds once every patch written so far is applied, as the page applies them.
fn rows(out: &Out) -> Vec<(Key, Value)> {
    let mut rows: Vec<(Key, Value)> = Vec::new();
    for patch in &out.patches {
        match patch {
            Patch::Reset { records, .. } => {
                rows = records.iter().map(|r| (r.id.clone(), r.record.clone())).collect();
            }
            Patch::Put { id, record, .. } => match rows.iter_mut().find(|(k, _)| k == id) {
                Some(row) => row.1 = record.clone(),
                None => rows.push((id.clone(), record.clone())),
            },
            other => panic!("repos writes resets and puts only, not {other:?}"),
        }
    }
    rows
}

fn opened(repo: &str) -> (Repos, Calls, Out) {
    let mut calls = Calls::new(String::new(), || 0.0);
    let mut out = Out::default();
    let mut repos = Repos::one(1);
    repos.open_one(&mut calls, repo.to_owned(), &mut out);
    (repos, calls, out)
}

#[test]
fn a_view_pushed_before_the_answer_is_kept_over_it() {
    let (mut repos, _calls, mut out) = opened("acme/api");
    let read = repos.generation;
    let newer = view("acme/api", "pushed");
    repos.on_push(&pushed(&newer), &newer, &mut out);
    repos.answered(read, Ok(view("acme/api", "read")), &mut out);
    assert_eq!(rows(&out), vec![(ONE.to_owned(), newer)]);
}

#[test]
fn a_view_pushed_after_the_answer_replaces_it_and_another_repository_is_not_held() {
    let (mut repos, _calls, mut out) = opened("acme/api");
    let read = repos.generation;
    repos.answered(read, Ok(view("acme/api", "read")), &mut out);
    let newer = view("acme/api", "pushed");
    repos.on_push(&pushed(&newer), &newer, &mut out);
    let other = view("acme/web", "elsewhere");
    repos.on_push(&pushed(&other), &other, &mut out);
    assert_eq!(rows(&out), vec![(ONE.to_owned(), newer)]);
}

#[test]
fn an_answer_asked_before_a_snapshot_is_dropped() {
    let (mut repos, mut calls, mut out) = opened("acme/api");
    let before = repos.generation;
    repos.read_again(&mut calls, &mut out);
    let after = repos.generation;
    repos.answered(before, Ok(view("acme/api", "before the snapshot")), &mut out);
    assert_eq!(rows(&out), Vec::new());
    let read = view("acme/api", "after the snapshot");
    repos.answered(after, Ok(read.clone()), &mut out);
    assert_eq!(rows(&out), vec![(ONE.to_owned(), read)]);
}
