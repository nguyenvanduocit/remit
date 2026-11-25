//! How memories are ranked for what an agent looks for (ADR-0117): pure. Retrieval casts wide and a judge decides:
//!
//! 1. **Queries.** A search is its words. A turn is three (`turn_queries`): the message, the work in one line (the card's
//!    brief), and the subjects the message names (paths, backticked names, crates, tools), looked up as what memories are
//!    about.
//! 2. **Candidates.** Each query's words (FTS5's rank) and meaning (the cosine of its vector with each memory's) give an
//!    order, the subjects one more, and links one step over the graph from the best of them. Reciprocal rank fusion joins
//!    the orders, so no signal's scale outweighs another's.
//! 3. **Relevance.** A cross-encoder reads the query beside each memory and says how relevant it is, from 0 to 1: the one
//!    signal that reads both together. It reads every memory the agent may recall while they are few (`JUDGE_ALL`), else
//!    the best `JUDGED` candidates. A turn is given only what clears `TURN_FLOOR`; a search shows what clears
//!    `SEARCH_FLOOR`. With no judge (no key, or it failed) a turn is given only what words and meaning agree on, its
//!    meaning at `MEANING_FLOOR` or nearer, or what links to such a one; a search shows the fused order.
//! 4. **Weight.** A person's confirmation and the memory's strength (what agents said of it, fading with disuse) weigh the
//!    score.
//!
//! A memory is never recalled for being near nothing.

use std::collections::{HashMap, HashSet};

use model::{Card, Memory, MemoryLinkKind, MemoryTrust, Time, subject_of};
use store::{ABOUT, MemoryEdge};

/// The constant of reciprocal rank fusion: how fast a lower place counts less (the value the method was published with).
const RRF_K: f64 = 60.0;

/// How many of the meaning's nearest memories are candidates, per query. Cosine scales differ between models, so the cut
/// is a count, and the judge decides what is near enough.
pub const MEANING_TOP: usize = 24;

/// How many fused candidates the judge reads when the memories an agent may recall are more than `JUDGE_ALL`; while they
/// are that few or fewer it reads every one, so nothing the signals missed is missed.
pub const JUDGED: usize = 32;
pub const JUDGE_ALL: usize = 256;

/// How much of a strong hit a linked memory takes: a direct link, a subject they share.
const LINKED_SHARE: f64 = 0.5;
const SUBJECT_SHARE: f64 = 0.3;

/// How many of the fused hits the graph is walked from.
const WALK_FROM: usize = 8;

/// How much more a memory a person confirmed weighs.
const CONFIRMED_WEIGHT: f64 = 1.25;

/// The cross-encoder that judges relevance, and the relevance a memory needs to be given to a turn unasked or shown for a
/// search: a pair, measured together. On the owner's 45 memories (2026-11-19, 12 queries in English and Vietnamese with
/// known answers, 5 about nothing they hold), every query about nothing scored every memory 0.000, the best answer of each
/// other query 0.843 or more, and every known answer but one 0.65 or more. Of eight rerankers on OpenRouter it was the only
/// one with no overlap (cohere/rerank-4-pro gave nothing 0.517, mongodb/rerank-3 0.294). Another model is measured again
/// (recall_eval.rs). A turn's note costs the agent's attention on every message, so its bar is high; a search was asked
/// for, so its bar only drops the unrelated.
pub const RERANK_MODEL: &str = "qwen/qwen3-reranker-8b";
pub const TURN_FLOOR: f32 = 0.5;
pub const SEARCH_FLOOR: f32 = 0.05;

/// The cosine under `EMBED_MODEL` (the query instructed) a memory needs for a turn given unasked with no judge, beside
/// its words. Measured on the owner's memories (2026-11-19): the nearest memory to a query about nothing they hold was
/// 0.371 at most, the weakest best answer 0.659. 0.55 stands 0.18 above the first and 0.11 below the second: a turn's
/// note given with no judge leans to giving less. Another model or passage is measured again (recall_eval.rs).
pub const MEANING_FLOOR: f32 = 0.55;

/// How strength bends a score: at most this factor up or down.
const STRENGTH_SPAN: f64 = 0.25;

/// How many days of disuse halve what agents' praise adds to a memory's strength.
const DISUSE_HALF_DAYS: f64 = 90.0;

/// What the judge is told it judges, ahead of a turn's query: Qwen3's reranker reads an instruction with the query.
const TURN_JUDGE_INSTRUCTION: &str =
    "Instruct: Judge whether this memory of earlier work bears on what the agent is about to do\nQuery: ";

/// How much of a message a turn's queries read, in characters, and of its work.
const MESSAGE_CHARS: usize = 2000;
const WORK_CHARS: usize = 300;

/// How many subjects a text names at most.
const SUBJECTS_MAX: usize = 32;

/// Why memories are ranked: to give a turn unasked, or to answer a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Turn,
    Search,
}

/// What memories are searched with: texts whose words and meaning each give an order, subjects memories may be about,
/// and the query the judge reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Queries {
    pub texts: Vec<String>,
    pub subjects: Vec<String>,
    pub judged: String,
}

impl Queries {
    /// A search for query: its words and meaning, the subjects it names, and itself for the judge.
    pub fn of(query: &str) -> Self {
        Self {
            texts: vec![query.to_owned()],
            subjects: subjects_in(query),
            judged: query.to_owned(),
        }
    }
}

/// s cut to n characters.
fn chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// The queries of card c's turn on a message reading message: (a) the message, (b) its work in one line, the card's
/// brief (its name when it has none), (c) the subjects the message names. The judge reads (b) and (a) after an
/// instruction that says what it judges. Pure.
pub fn turn_queries(c: &Card, message: &str) -> Queries {
    let message = chars(message.trim(), MESSAGE_CHARS);
    let work = c
        .brief
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(c.name.trim());
    let work = chars(work, WORK_CHARS);
    let mut texts = vec![message.clone()];
    if !work.is_empty() {
        texts.push(work.clone());
    }
    Queries {
        subjects: subjects_in(&message),
        judged: format!("{TURN_JUDGE_INSTRUCTION}{work}\n{message}"),
        texts,
    }
}

/// The subjects text names, as memories are about them (`model::subject_of`): a path (`file:`, and the crate it is in), a
/// backticked name (as a tool, a crate and a topic), a snake_case name (a tool), an MCP tool's name; each once, at most
/// `SUBJECTS_MAX`. Pure.
pub fn subjects_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |raw: String| {
        if let Some(s) = subject_of(&raw)
            && !out.contains(&s)
            && out.len() < SUBJECTS_MAX
        {
            out.push(s);
        }
    };
    for quoted in text.split('`').skip(1).step_by(2) {
        let q = quoted.trim();
        if q.is_empty() || q.chars().count() > 80 {
            continue;
        }
        if is_path(q) {
            add(format!("file:{q}"));
        } else {
            add(format!("tool:{q}"));
            add(format!("crate:{q}"));
            add(format!("topic:{q}"));
        }
    }
    for word in text.split(|c: char| c.is_whitespace() || matches!(c, '`' | '"' | '\'' | '(' | ')' | ',' | ';')) {
        let w = word.trim_matches(|c: char| matches!(c, '.' | ':' | '!' | '?'));
        if is_path(w) {
            add(format!("file:{w}"));
            if let Some(name) = w.strip_prefix("crates/").and_then(|r| r.split('/').next()) {
                add(format!("crate:{name}"));
            }
        } else if let Some(tool) = w.strip_prefix("mcp__").and_then(|r| r.split("__").nth(1)) {
            add(format!("tool:{tool}"));
        } else if w.contains('_') && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && w.len() > 3 {
            add(format!("tool:{w}"));
        }
    }
    out
}

/// Whether w reads as a path: a slash between names, or a name with a known source extension.
fn is_path(w: &str) -> bool {
    let plain = !w.is_empty() && !w.contains("://") && !w.contains(char::is_whitespace);
    let extension = w.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && matches!(
                ext,
                "rs" | "ts"
                    | "vue"
                    | "js"
                    | "go"
                    | "py"
                    | "md"
                    | "toml"
                    | "json"
                    | "sql"
                    | "sh"
                    | "yaml"
                    | "yml"
                    | "css"
            )
    });
    plain && ((w.contains('/') && w.chars().any(char::is_alphanumeric)) || extension)
}

/// The cosine of a and b; 0 when either is empty or their lengths differ.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// The `MEANING_TOP` vectors nearest query, nearest first, each with its cosine.
pub fn nearest(query: &[f32], vectors: &[(String, Vec<f32>)]) -> Vec<(String, f32)> {
    let mut near: Vec<(f32, &String)> = vectors.iter().map(|(id, v)| (cosine(query, v), id)).collect();
    near.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    near.into_iter()
        .take(MEANING_TOP)
        .map(|(c, id)| (id.clone(), c))
        .collect()
}

/// What a place in an order is worth.
fn at(place: usize) -> f64 {
    let place = u32::try_from(place).unwrap_or(u32::MAX);
    1.0 / (RRF_K + f64::from(place) + 1.0)
}

/// The fused score of each memory the orders name: the sum over orders of what its place is worth.
fn fuse(orders: &[Vec<String>]) -> HashMap<String, f64> {
    let mut score: HashMap<String, f64> = HashMap::new();
    for order in orders {
        for (place, id) in order.iter().enumerate() {
            *score.entry(id.clone()).or_default() += at(place);
        }
    }
    score
}

/// score raised by one step over edges from its `WALK_FROM` best: a memory linked to one of them (either way, but a
/// supersede: the old one is not recalled) takes `LINKED_SHARE` of it, one sharing a subject with it `SUBJECT_SHARE`; a
/// memory reached from several keeps the most it was given. Returns the ids it reached.
fn walk(score: &mut HashMap<String, f64>, edges: &[MemoryEdge]) -> HashSet<String> {
    let mut best: Vec<(&String, f64)> = score.iter().map(|(id, s)| (id, *s)).collect();
    best.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let from: HashMap<String, f64> = best
        .into_iter()
        .take(WALK_FROM)
        .map(|(id, s)| (id.clone(), s))
        .collect();
    let mut subjects: HashMap<&str, f64> = HashMap::new();
    for e in edges.iter().filter(|e| e.kind == ABOUT) {
        if let Some(s) = from.get(&e.src) {
            let held = subjects.entry(e.dst.as_str()).or_default();
            *held = held.max(*s);
        }
    }
    let mut given: HashMap<String, f64> = HashMap::new();
    let mut give = |id: &str, s: f64| {
        let held = given.entry(id.to_owned()).or_default();
        *held = held.max(s);
    };
    for e in edges {
        if e.kind == ABOUT {
            if let Some(s) = subjects.get(e.dst.as_str()).filter(|_| !from.contains_key(&e.src)) {
                give(&e.src, s * SUBJECT_SHARE);
            }
            continue;
        }
        if e.kind == MemoryLinkKind::Supersedes.as_str() {
            continue;
        }
        if let Some(s) = from.get(&e.src) {
            give(&e.dst, s * LINKED_SHARE);
        }
        if let Some(s) = from.get(&e.dst) {
            give(&e.src, s * LINKED_SHARE);
        }
    }
    let reached: HashSet<String> = given.keys().cloned().collect();
    for (id, s) in given {
        *score.entry(id).or_default() += s;
    }
    reached
}

/// What the signals found for queries: per text, the order of its words and of its meaning (each with its cosine), and
/// the order of the memories about the subjects named.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub words: Vec<Vec<String>>,
    pub meaning: Vec<Vec<(String, f32)>>,
    pub subjects: Vec<String>,
}

impl Found {
    /// The ids the graph is walked from: every one a signal found.
    pub fn seeds(&self) -> Vec<String> {
        let mut out: Vec<String> = self.words.iter().flatten().cloned().collect();
        out.extend(self.meaning.iter().flatten().map(|(id, _)| id.clone()));
        out.extend(self.subjects.iter().cloned());
        out.sort();
        out.dedup();
        out
    }
}

/// The candidates for what found holds: every memory a signal found or a link reached, fused, best first, none of given
/// and only what is in readable. Each says whether words and meaning agree on it, its meaning at `MEANING_FLOOR` or
/// nearer (or it links to such a one): what a turn is given when no judge reads them.
pub fn candidates(
    readable: &HashMap<String, Memory>,
    found: &Found,
    edges: &[MemoryEdge],
    given: &HashSet<String>,
) -> Vec<Candidate> {
    let mut orders: Vec<Vec<String>> = found.words.clone();
    orders.extend(
        found
            .meaning
            .iter()
            .map(|o| o.iter().map(|(id, _)| id.clone()).collect()),
    );
    orders.push(found.subjects.clone());
    let mut score = fuse(&orders);
    let close: HashSet<&String> = found
        .meaning
        .iter()
        .flatten()
        .filter(|(_, c)| *c >= MEANING_FLOOR)
        .map(|(id, _)| id)
        .collect();
    let agreed: HashSet<&String> = found.words.iter().flatten().filter(|id| close.contains(id)).collect();
    let reached = walk(&mut score, edges);
    let linked_to_agreed = |id: &String| {
        edges.iter().any(|e| {
            e.kind != ABOUT
                && e.kind != MemoryLinkKind::Supersedes.as_str()
                && ((&e.src == id && agreed.contains(&e.dst)) || (&e.dst == id && agreed.contains(&e.src)))
        })
    };
    let mut out: Vec<Candidate> = score
        .into_iter()
        .filter(|(id, _)| readable.contains_key(id) && !given.contains(id))
        .map(|(id, fused)| Candidate {
            agreed: agreed.contains(&id) || (reached.contains(&id) && linked_to_agreed(&id)),
            id,
            fused,
        })
        .collect();
    out.sort_by(|a, b| b.fused.total_cmp(&a.fused).then_with(|| a.id.cmp(&b.id)));
    out
}

/// One memory to rank for a query: its fused score (0 when no signal found it) and whether the signals agree on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub fused: f64,
    pub agreed: bool,
}

/// What the judge reads of candidates: every memory of readable but given while they are `JUDGE_ALL` or fewer (the
/// candidates first, then the rest, unfound), else the best `JUDGED` candidates. Pure.
pub fn to_judge(
    candidates: &[Candidate],
    readable: &HashMap<String, Memory>,
    given: &HashSet<String>,
) -> Vec<Candidate> {
    let open = readable.keys().filter(|id| !given.contains(*id)).count();
    if open > JUDGE_ALL {
        return candidates.iter().take(JUDGED).cloned().collect();
    }
    let mut out = candidates.to_vec();
    let mut rest: Vec<&String> = readable
        .keys()
        .filter(|id| !given.contains(*id) && !candidates.iter().any(|c| &c.id == *id))
        .collect();
    rest.sort();
    out.extend(rest.into_iter().map(|id| Candidate {
        id: id.clone(),
        fused: 0.0,
        agreed: false,
    }));
    out
}

/// n as a float: counts stay far under where f64 loses whole numbers.
fn count(n: u64) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// How strength bends m's score at now: a factor in 1 ± `STRENGTH_SPAN`, from what agents said of it. The mean of its
/// Beta posterior, (helped + 1) / (helped + misled + 2), centred on 0.5 (no word yet is no change); praise fades over
/// `DISUSE_HALF_DAYS` of disuse, blame does not. How often it was given counts for nothing: being recalled is no proof it
/// helped.
pub fn strength(m: &Memory, now: Time) -> f64 {
    let p = (count(m.helped) + 1.0) / (count(m.helped) + count(m.misled) + 2.0);
    let lean = 2.0 * p - 1.0;
    let lean = if lean > 0.0 {
        let last = m.last_recalled_at.unwrap_or(m.created_at);
        let idle = now.0.unix_timestamp().saturating_sub(last.0.unix_timestamp()).max(0);
        let idle_days = f64::from(u32::try_from(idle).unwrap_or(u32::MAX)) / 86_400.0;
        lean * 0.5_f64.powf(idle_days / DISUSE_HALF_DAYS)
    } else {
        lean
    };
    1.0 + STRENGTH_SPAN * lean
}

/// The memories to recall from pool, best first, at most limit: with relevance (the judge's score of each, by id), those
/// that clear the purpose's floor ranked by relevance; without, a turn takes only those the signals agreed on and a search
/// every one a signal found, ranked by the fused score. Either way weighed by trust and strength at now.
pub fn rank(
    readable: &HashMap<String, Memory>,
    pool: &[Candidate],
    relevance: Option<&HashMap<String, f32>>,
    purpose: Purpose,
    limit: usize,
    now: Time,
) -> Vec<Memory> {
    let floor = match purpose {
        Purpose::Turn => TURN_FLOOR,
        Purpose::Search => SEARCH_FLOOR,
    };
    let mut ranked: Vec<(f64, &Memory)> = pool
        .iter()
        .filter_map(|c| {
            let m = readable.get(&c.id)?;
            let base = match relevance {
                Some(r) => {
                    let r = *r.get(&c.id)?;
                    (r >= floor).then_some(f64::from(r))?
                }
                None if purpose == Purpose::Turn && !c.agreed => return None,
                None if c.fused <= 0.0 => return None,
                None => c.fused,
            };
            let trust = if m.trust == MemoryTrust::Confirmed {
                CONFIRMED_WEIGHT
            } else {
                1.0
            };
            Some((base * trust * strength(m, now), m))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| b.1.updated_at.cmp(&a.1.updated_at))
            .then_with(|| a.1.id.cmp(&b.1.id))
    });
    ranked.into_iter().take(limit).map(|(_, m)| m.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str) -> Memory {
        Memory {
            id: id.to_owned(),
            ..Memory::default()
        }
    }

    fn edge(src: &str, dst: &str, kind: &str) -> MemoryEdge {
        MemoryEdge {
            src: src.to_owned(),
            dst: dst.to_owned(),
            kind: kind.to_owned(),
        }
    }

    fn ids(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| (*x).to_owned()).collect()
    }

    // The main path of recall. Candidates: what words, meaning or subjects found, what links to them or shares their
    // subject; never what the session was given, what a supersede points back to, or what the agent may not read. A judge
    // then decides what a turn is given, by its floor, reading every memory while they are few; with no judge a turn takes
    // only what words and meaning near enough agree on, or what links to it.
    #[test]
    fn recall_gives_a_turn_what_the_judge_stands_behind_and_without_one_what_the_signals_agree_on() {
        let readable: HashMap<String, Memory> = ["a", "b", "c", "d", "e", "f", "old", "z"]
            .map(|id| (id.to_owned(), mem(id)))
            .into();
        let edges = vec![
            edge("a", "c", "because"),
            edge("a", "file:src/x.rs", ABOUT),
            edge("d", "file:src/x.rs", ABOUT),
            edge("a", "old", "supersedes"),
            edge("b", "secret", "related"),
        ];
        let given: HashSet<String> = ["f".to_owned()].into();
        let found = Found {
            words: vec![ids(&["b", "a", "e", "f"])],
            meaning: vec![vec![
                ("a".to_owned(), 0.7),
                ("e".to_owned(), 0.4),
                ("f".to_owned(), 0.9),
            ]],
            subjects: Vec::new(),
        };
        let cands = candidates(&readable, &found, &edges, &given);
        let names: Vec<&str> = cands.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(names, ["a", "e", "b", "c", "d"]);

        let pool = to_judge(&cands, &readable, &given);
        let pooled: Vec<&str> = pool.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            pooled,
            ["a", "e", "b", "c", "d", "old", "z"],
            "while few, the judge reads them all"
        );

        let now = Time::default();
        let judged: HashMap<String, f32> = [("a", 0.9), ("b", 0.2), ("c", 0.7), ("d", 0.6), ("e", 0.01), ("z", 0.8)]
            .map(|(id, r)| (id.to_owned(), r))
            .into();
        let got = |r: Option<&HashMap<String, f32>>, p: Purpose| -> Vec<String> {
            rank(&readable, &pool, r, p, 10, now)
                .into_iter()
                .map(|m| m.id)
                .collect()
        };
        assert_eq!(got(Some(&judged), Purpose::Turn), ["a", "z", "c", "d"]);
        assert_eq!(got(Some(&judged), Purpose::Search), ["a", "z", "c", "d", "b"]);
        assert_eq!(
            got(None, Purpose::Turn),
            ["a", "c"],
            "e's words agree but its meaning is far"
        );
    }
}
