//! The librarian's remember driven as its Chair calls it, on a real store with models that embed by what a text is about:
//! a memory is not written twice, and one that only bears on what the holder remembers is written.

use std::sync::Arc;

use async_trait::async_trait;
use crew::memory::{EMBED_MODEL, MemoryModels, ModelError};
use judgment::{Decided, Decider, DeciderError, Question};
use model::{JudgmentAnswer, Memory, MemoryScope, OutcomeSource, Proved, Teaching};
use serde_json::{Map, Value, json};

use crate::tests::{Rig, rig_of};

/// Models that place a text by what it is about (status effects, zsh, anything else) and judge every memory relevant: what
/// the owner's librarian met, where one memory bore on every lesson it wrote.
struct Topics;

#[async_trait]
impl MemoryModels for Topics {
    async fn embed(&self, _model: &str, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        Ok(texts
            .iter()
            .map(|t| {
                if t.contains("anatomy") {
                    vec![1.0, 0.0, 0.0]
                } else if t.contains("tight") {
                    // Near enough for the cosine rule (0.84) to ask the librarian to name it, whatever a decider says.
                    vec![0.92, 0.0, 0.392]
                } else if t.contains("gloss") {
                    // Near anatomy, but under the cosine rule's 0.84: only a decider can say they are one.
                    vec![0.7, 0.0, 0.714]
                } else if t.contains("zsh") {
                    vec![0.5, 0.866, 0.0]
                } else {
                    vec![0.0, 0.0, 1.0]
                }
            })
            .collect())
    }

    async fn rerank(&self, _model: &str, _query: &str, documents: &[String]) -> Result<Vec<f32>, ModelError> {
        Ok(vec![0.9; documents.len()])
    }
}

/// A librarian's chain whose holder e1 remembers mem_effects, with a teaching t1 that waits.
async fn librarian() -> (Rig, String) {
    librarian_with(None).await
}

/// The same, with decider answering what the decision layer is asked of memory pairs.
async fn librarian_with(decider: Option<Arc<dyn Decider>>) -> (Rig, String) {
    let r = rig_of(|cfg| {
        cfg.memory_models = Some(Arc::new(Topics));
        cfg.core.decider = decider;
    });
    let ch = r.create("r1", "Keep what the agents remember").await;
    let give = chains::step::ChainCmd::Capability(chains::step::CapabilityCmd {
        capability: "memory_keeping".to_owned(),
        why: "the test's librarian".to_owned(),
        card_id: String::new(),
        by: model::by_owner("owner", "web"),
    });
    r.o.apply(&ch.id, give, chains::apply::Records::default())
        .await
        .unwrap();
    let effects = Memory {
        id: "mem_effects".to_owned(),
        holder: "e1".to_owned(),
        scope: MemoryScope::Own,
        title: "pixfx status effects follow a consistent anatomy pattern".to_owned(),
        body: "Each effect has a base, a pulse and a fade.".to_owned(),
        evidence: "every effect we drew had the same three parts".to_owned(),
        source: "teaching t0".to_owned(),
        ..Memory::default()
    };
    let teaching = Teaching {
        id: "t1".to_owned(),
        chain: ch.id.clone(),
        holder: "e1".to_owned(),
        text: "the owner's notes".to_owned(),
        source: "teaching t1".to_owned(),
        ..Teaching::default()
    };
    r.o.store
        .update(move |tx| -> store::Result<()> {
            tx.put_memory(&effects)?;
            tx.put_memory_vector(
                &effects.id,
                EMBED_MODEL,
                &store::passage_hash(&effects),
                &[1.0, 0.0, 0.0],
            )?;
            tx.add_teaching(&teaching)
        })
        .await
        .unwrap();
    (r, ch.chair_id)
}

/// Calls remember as the librarian's Chair, naming distinct_from: what it answers, and whether it was refused.
async fn remember(r: &Rig, chair: &str, title: &str, distinct_from: &[&str]) -> (String, bool) {
    remember_naming(r, chair, title, json!({"distinct_from": distinct_from})).await
}

/// Calls remember as the librarian's Chair, with naming (supersedes, links or distinct_from) beside the memory's words.
async fn remember_naming(r: &Rig, chair: &str, title: &str, naming: Value) -> (String, bool) {
    let tools = r.o.tools_for(chair).await.unwrap();
    let mut args = json!({
        "teaching": "t1",
        "kind": "lesson",
        "scope": "own",
        "title": title,
        "body": "Said in the teaching.",
        "evidence": "the owner's notes",
    });
    if let (Some(a), Some(n)) = (args.as_object_mut(), naming.as_object()) {
        a.extend(n.clone());
    }
    let args: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    match tools.call("remember", Some(args)).await {
        Ok(Ok(out)) => (out, false),
        Ok(Err(e)) => (e.to_string(), true),
        Err(e) => (e.message.to_string(), true),
    }
}

// A memory that only bears on one the holder has is written: relevance is not a claim said twice.
#[tokio::test]
async fn remember_writes_a_memory_that_only_bears_on_one_the_holder_has() {
    let (r, chair) = librarian().await;
    let (out, refused) = remember(&r, &chair, "zsh treats an unquoted ? in a grep pattern as a glob", &[]).await;
    assert!(!refused, "{out}");
    assert!(out.starts_with("remembered mem_"), "{out}");
}

// A memory that may say what one the holder has says is refused until it names it (a rail: nothing is written twice).
#[tokio::test]
async fn remember_refuses_a_memory_that_may_say_one_again_until_it_names_it() {
    let (r, chair) = librarian().await;
    let title = "status effects in pixfx share one anatomy";
    let (out, refused) = remember(&r, &chair, title, &[]).await;
    assert!(refused && out.contains("mem_effects"), "{out}");
    let (out, refused) = remember(&r, &chair, title, &["mem_effects"]).await;
    assert!(!refused && out.starts_with("remembered mem_"), "{out}");
}
/// Deciders that give the same answer to every pair: `same` the probability the two teach the same lesson, `contradicts`
/// the probability they contradict.
struct Verdicts {
    same: f64,
    contradicts: f64,
}

#[async_trait]
impl Decider for Verdicts {
    async fn decide(&self, decider: &str, _state: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        let answer = |q: &Question| {
            let p = if q.key() == judgment::MEMORY_SAME_CLAIM.key() {
                self.same
            } else {
                self.contradicts
            };
            Some(JudgmentAnswer::Noul { p })
        };
        Ok(Decided {
            model: format!("{decider}-20261116"),
            answers: questions.iter().map(answer).collect(),
            cost_usd: 0.001,
        })
    }
}

/// What the deciders were asked and said of the pairs that include memory, as the librarian's outcomes left them.
async fn judged_of(r: &Rig, memory: &str) -> Vec<model::Judgment> {
    let memory = memory.to_owned();
    let keys = vec![judgment::MEMORY_SAME_CLAIM.key(), judgment::MEMORY_CONTRADICTS.key()];
    r.o.store
        .read(move |r| r.pair_judgments_of(&memory, &keys))
        .await
        .unwrap()
}

/// What the librarian's words proved of the pair's two answers, by question.
async fn proved_of(r: &Rig) -> Vec<(String, Option<(Proved, OutcomeSource)>)> {
    let mut proved: Vec<_> = judged_of(r, "mem_effects")
        .await
        .iter()
        .map(|j| (j.question.clone(), j.outcome.as_ref().map(|o| (o.proved, o.source))))
        .collect();
    proved.sort_by(|a, b| a.0.cmp(&b.0));
    proved
}

// A pair the cosine rule lets through (0.70, under 0.84) is one the decider calls the same lesson: the librarian must name
// it, and saying the teaching told it again (mark_retaught) scores the answer right, by the librarian. The pair is asked
// once however often the librarian came back.
#[tokio::test(flavor = "multi_thread")]
async fn remember_refuses_a_pair_the_decider_calls_the_same_lesson_and_retelling_it_scores_the_answer() {
    let (r, chair) = librarian_with(Some(Arc::new(Verdicts {
        same: 0.95,
        contradicts: 0.05,
    })))
    .await;
    let title = "a gloss on how every effect is drawn";
    let (out, refused) = remember(&r, &chair, title, &[]).await;
    assert!(refused && out.contains("mem_effects"), "{out}");
    let (_, refused) = remember(&r, &chair, title, &[]).await;
    assert!(refused);
    let tools = r.o.tools_for(&chair).await.unwrap();
    let args = json!({"teaching": "t1", "memory": "mem_effects"});
    let args: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    tools.call("mark_retaught", Some(args)).await.unwrap().unwrap();
    assert_eq!(
        proved_of(&r).await,
        vec![
            // Telling it again says nothing of whether the pair contradicts.
            ("memory.contradicts@1".to_owned(), None),
            (
                "memory.same_claim@1".to_owned(),
                Some((Proved::Right, OutcomeSource::Librarian))
            ),
        ]
    );
}

// Keeping a pair as another lesson (distinct_from) proves a yes wrong for the same lesson and says nothing of whether the
// pair contradicts: a contradiction kept apart still reaches the review.
#[tokio::test(flavor = "multi_thread")]
async fn naming_a_pair_distinct_proves_the_same_lesson_wrong_and_leaves_its_contradiction_for_review() {
    let (r, chair) = librarian_with(Some(Arc::new(Verdicts {
        same: 0.95,
        contradicts: 0.95,
    })))
    .await;
    let title = "a gloss on how every effect is drawn";
    let (out, refused) = remember_naming(&r, &chair, title, json!({"distinct_from": ["mem_effects"]})).await;
    assert!(!refused && out.starts_with("remembered mem_"), "{out}");
    assert_eq!(
        proved_of(&r).await,
        vec![
            ("memory.contradicts@1".to_owned(), None),
            (
                "memory.same_claim@1".to_owned(),
                Some((Proved::Wrong, OutcomeSource::Librarian))
            ),
        ]
    );
    let key = judgment::MEMORY_CONTRADICTS.key();
    let open = r.o.store.read(move |r| r.open_yes_judgments(&key)).await.unwrap();
    assert_eq!(open.len(), 1, "the review still reads it: {open:?}");
}

/// Deciders that refuse every question, as when OpenRouter is down.
struct Down;

#[async_trait]
impl Decider for Down {
    async fn decide(&self, _: &str, _: &Value, _: &[Question]) -> Result<Decided, DeciderError> {
        Err(DeciderError::Failed("OpenRouter answered 502".to_owned()))
    }
}

// Where the decider gave no answer, or one too unsure to act on, the cosine rule still makes the librarian name a pair
// at 0.92: an unsure decider does not lift the guard.
#[tokio::test(flavor = "multi_thread")]
async fn a_decider_that_fails_or_is_unsure_leaves_the_cosine_rule_refusing() {
    let title = "a tight restatement of how every effect is drawn";
    let unsure: Arc<dyn Decider> = Arc::new(Verdicts {
        same: 0.7,
        contradicts: 0.1,
    });
    let down: Arc<dyn Decider> = Arc::new(Down);
    for decider in [down, unsure] {
        let (r, chair) = librarian_with(Some(decider)).await;
        let (out, refused) = remember(&r, &chair, title, &[]).await;
        assert!(refused && out.contains("mem_effects"), "{out}");
    }
}

// A pair the decider calls a different lesson is written without naming it, though near; and a pair it finds
// contradicting waits for the daily review.
#[tokio::test(flavor = "multi_thread")]
async fn remember_writes_beside_a_pair_the_decider_calls_different_and_keeps_a_contradiction_for_review() {
    let (r, chair) = librarian_with(Some(Arc::new(Verdicts {
        same: 0.05,
        contradicts: 0.95,
    })))
    .await;
    let (out, refused) = remember(&r, &chair, "a gloss on how every effect is drawn", &[]).await;
    assert!(!refused && out.starts_with("remembered mem_"), "{out}");
    let key = judgment::MEMORY_CONTRADICTS.key();
    let open = r.o.store.read(move |r| r.open_yes_judgments(&key)).await.unwrap();
    let pairs: Vec<_> = open
        .iter()
        .filter_map(|j| j.subject.pair().map(|(_, old)| old.to_owned()))
        .collect();
    assert_eq!(pairs, vec!["mem_effects".to_owned()], "the review reads it: {open:?}");
}
