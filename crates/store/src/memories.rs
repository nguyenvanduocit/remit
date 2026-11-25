//! Memories and teachings (migrations 120 and 122, ADR-0117). A memory is a document with the columns recall filters on,
//! written with its words (`memory_text`, FTS5, keyed by the memory's row) and its edges (`memory_edges`: its links and
//! what it is about); its use (turns given it, ratings) is counted in columns beside it and never written with it; its
//! vectors are written apart, once the embedding model made them from the passage it still has. All of it goes with the
//! memory. Who may read a memory is one clause (`readable!`), so every read that recalls applies the same rule.

use model::{Memory, MemoryEnd, MemoryLinkKind, MemoryStatus, MemoryUse, Teaching, TeachingState, Time};
use rusqlite::{OptionalExtension as _, ToSql, params};
use sha2::{Digest, Sha256};

use crate::store::{Error, Reader, Result, Tx, doc, from_ms, ms};

/// The edge kind from a memory to a subject it is about.
pub const ABOUT: &str = "about";

/// The memories the reader `:holder` (hired from `:template`, at work in `:project`) may recall: its own, its project's,
/// its lineage's and everyone's, active. Another employee's own memory is never among them.
macro_rules! readable {
    ($m:literal) => {
        concat!(
            $m,
            ".status = 'active' AND (",
            $m,
            ".holder = :holder OR (",
            $m,
            ".scope = 'project' AND :project != '' AND ",
            $m,
            ".project = :project) OR (",
            $m,
            ".scope = 'lineage' AND :template != '' AND ",
            $m,
            ".template = :template) OR ",
            $m,
            ".scope = 'everyone')"
        )
    };
}

/// A memory's document as it is read from `memories!()`: the stored one with its use.
macro_rules! memory_doc {
    () => {
        concat!(
            "json_set(m.doc, '$.recalled', coalesce(u.recalled, 0), '$.helped', coalesce(u.helped, 0), '$.misled', ",
            "coalesce(u.misled, 0), '$.lastRecalledAt', CASE WHEN u.last_recalled_at IS NULL THEN NULL ",
            "ELSE strftime('%Y-%m-%dT%H:%M:%fZ', u.last_recalled_at / 1000.0, 'unixepoch') END)"
        )
    };
}

/// The memories, each `m` with its use `u`: what every read of a memory reads from.
macro_rules! memories {
    () => {
        "memories m LEFT JOIN memory_use u ON u.memory_id = m.id"
    };
}

/// The keys of a memory's document that `memory_use` holds instead.
const USE_KEYS: [&str; 4] = ["recalled", "helped", "misled", "lastRecalledAt"];

/// Who recalls: an agent's holder, the template it was hired from and the project it is at work in (empty: none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryReader {
    pub holder: String,
    pub template: String,
    pub project: String,
}

impl MemoryReader {
    fn params(&self) -> [(&str, &dyn ToSql); 3] {
        [
            (":holder", &self.holder),
            (":template", &self.template),
            (":project", &self.project),
        ]
    }
}

/// One edge of the memory graph: from memory `src` to a memory or a subject `dst`, of kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEdge {
    pub src: String,
    pub dst: String,
    pub kind: String,
}

/// Words that say nothing of what a memory is about, in English and Vietnamese (with and without their marks, as a query
/// may be typed either way): a query OR-s its words, so with these every memory would match.
const STOPWORDS: &[&str] = &[
    "a", "about", "after", "all", "also", "an", "and", "any", "are", "as", "at", "be", "been", "before", "but", "by",
    "can", "could", "did", "do", "does", "for", "from", "had", "has", "have", "how", "if", "in", "into", "is", "it",
    "its", "just", "may", "me", "more", "must", "my", "no", "not", "now", "of", "on", "or", "our", "out", "should",
    "so", "some", "than", "that", "the", "their", "them", "then", "there", "these", "they", "this", "those", "to",
    "up", "us", "was", "we", "were", "what", "when", "where", "which", "while", "who", "why", "will", "with", "would",
    "you", "your", "bạn", "ban", "bằng", "bang", "bị", "bi", "các", "cac", "cái", "cai", "cần", "cho", "có", "co",
    "của", "cua", "cũng", "cung", "đã", "da", "đang", "dang", "để", "de", "đó", "được", "duoc", "gì", "gi", "hay",
    "hoặc", "hoac", "khi", "không", "khong", "là", "la", "làm", "lam", "lên", "len", "mà", "ma", "một", "mot", "nào",
    "nao", "này", "nay", "nên", "nen", "nếu", "neu", "những", "nhung", "như", "nhu", "phải", "phai", "ra", "rồi",
    "roi", "sao", "sẽ", "se", "theo", "thế", "thì", "thi", "tôi", "toi", "trong", "từ", "tu", "và", "va", "vào", "vao",
    "về", "ve", "vì", "vi", "với", "voi",
];

/// The FTS5 query of text: each word that says something (two characters or more, no stopword) quoted, any of them
/// matching. None when text has no such word.
fn words_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() > 1 && !STOPWORDS.contains(&w.as_str()))
        .take(32)
        .map(|w| format!("\"{w}\""))
        .collect();
    (!words.is_empty()).then(|| words.join(" OR "))
}

/// A vector as the blob it is stored as, and back.
fn vec_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn blob_vec(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

/// What memory m is embedded from, as a hash: a vector is m's while the hash of its passage is the one it was made of.
pub fn passage_hash(m: &Memory) -> String {
    let sum = Sha256::digest(m.passage().as_bytes());
    sum.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// m's document as it is stored: without its use, which its columns hold.
fn stored_doc(m: &Memory) -> Result<String> {
    let mut v = serde_json::to_value(m)?;
    if let Some(o) = v.as_object_mut() {
        for k in USE_KEYS {
            o.remove(k);
        }
    }
    doc(&v)
}

/// What m's words index holds of it.
fn words_of(m: &Memory) -> (&str, &str, &str, String) {
    (&m.title, &m.body, &m.evidence, m.about.join(" "))
}

impl Tx<'_> {
    /// Writes m, its words and its edges (its links and what it is about), over what was stored of it. Its use is not
    /// written: it is counted (`count_recall`, `count_use`). Its words and edges are written again only when they changed,
    /// and its vectors stay only while its passage is the one they were made of.
    pub fn put_memory(&self, m: &Memory) -> Result<()> {
        let before = match self.memory(&m.id) {
            Ok(b) => Some(b),
            Err(Error::NotFound) => None,
            Err(err) => return Err(err),
        };
        let n: i64 = self.value(
            "INSERT INTO memories(id, holder, template, project, scope, status, created_at, updated_at, doc)
             VALUES(?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET holder=excluded.holder, template=excluded.template, project=excluded.project,
             scope=excluded.scope, status=excluded.status, updated_at=excluded.updated_at, doc=excluded.doc
             RETURNING n",
            params![
                m.id,
                m.holder,
                m.template,
                m.project,
                m.scope.as_str(),
                m.status().as_str(),
                ms(m.created_at),
                ms(m.updated_at),
                stored_doc(m)?
            ],
        )?;
        if before.as_ref().is_none_or(|b| words_of(b) != words_of(m)) {
            self.exec("DELETE FROM memory_text WHERE rowid=?", [n])?;
            let (title, body, evidence, about) = words_of(m);
            self.exec(
                "INSERT INTO memory_text(rowid, title, body, evidence, about) VALUES(?,?,?,?,?)",
                params![n, title, body, evidence, about],
            )?;
        }
        if before
            .as_ref()
            .is_none_or(|b| (&b.links, &b.about) != (&m.links, &m.about))
        {
            self.exec("DELETE FROM memory_edges WHERE src=?", [&m.id])?;
            for l in &m.links {
                self.exec(
                    "INSERT OR IGNORE INTO memory_edges(src, dst, kind) VALUES(?,?,?)",
                    params![m.id, l.to, l.kind.as_str()],
                )?;
            }
            for subject in &m.about {
                self.exec(
                    "INSERT OR IGNORE INTO memory_edges(src, dst, kind) VALUES(?,?,?)",
                    params![m.id, subject, ABOUT],
                )?;
            }
        }
        let hash = passage_hash(m);
        self.exec(
            "DELETE FROM memory_embeddings WHERE memory_id=? AND passage_hash!=?",
            params![m.id, hash],
        )?;
        self.exec(
            "DELETE FROM memory_unembedded WHERE memory_id=? AND passage_hash!=?",
            params![m.id, hash],
        )?;
        Ok(())
    }

    /// Removes the memories ids names, with their words, vectors and edges, both ways; what other memories said of them
    /// goes too: a link to one, and the successor a memory it superseded names (that one stays superseded, saying so).
    pub fn delete_memories(&self, ids: &[String], now: Time) -> Result<()> {
        for id in ids {
            let Some(n) = self
                .c
                .prepare_cached("SELECT n FROM memories WHERE id=?")?
                .query_row([id], |r| r.get::<_, i64>(0))
                .optional()?
            else {
                continue;
            };
            let naming: Vec<String> = self.column(
                "SELECT DISTINCT src FROM memory_edges WHERE dst=?1 AND kind!='about' AND src!=?1
                 UNION SELECT dst FROM memory_edges WHERE src=?1 AND kind=?2",
                params![id, MemoryLinkKind::Supersedes.as_str()],
            )?;
            self.exec("DELETE FROM memories WHERE n=?", [n])?;
            self.exec("DELETE FROM memory_text WHERE rowid=?", [n])?;
            self.exec("DELETE FROM memory_embeddings WHERE memory_id=?", [id])?;
            self.exec("DELETE FROM memory_unembedded WHERE memory_id=?", [id])?;
            self.exec("DELETE FROM memory_use WHERE memory_id=?", [id])?;
            self.exec("DELETE FROM memory_edges WHERE src=?1 OR dst=?1", [id])?;
            for other in naming {
                let mut m = match self.memory(&other) {
                    Ok(m) => m,
                    Err(Error::NotFound) => continue,
                    Err(err) => return Err(err),
                };
                m.links.retain(|l| &l.to != id);
                if let MemoryEnd::Superseded { by, why, .. } = &mut m.end
                    && by == id
                {
                    by.clear();
                    why.push_str(&format!(" (forgotten since, by the owner, on {})", now.rfc3339()));
                }
                self.put_memory(&m)?;
            }
        }
        Ok(())
    }

    /// Counts a turn given each of ids, at at: the active ones among them.
    pub fn count_recall(&self, ids: &[String], at: Time) -> Result<()> {
        self.exec(
            "INSERT INTO memory_use(memory_id, recalled, last_recalled_at)
             SELECT id, 1, ?1 FROM memories WHERE status = 'active' AND id IN (SELECT value FROM json_each(?2))
             ON CONFLICT(memory_id) DO UPDATE SET recalled = recalled + 1, last_recalled_at = excluded.last_recalled_at",
            params![ms(at), serde_json::to_string(ids)?],
        )?;
        Ok(())
    }

    /// Counts that an agent said memory id was of use u.
    pub fn count_use(&self, id: &str, u: MemoryUse) -> Result<()> {
        let query = match u {
            MemoryUse::Helped => {
                "INSERT INTO memory_use(memory_id, helped) SELECT id, 1 FROM memories WHERE id = ?
                 ON CONFLICT(memory_id) DO UPDATE SET helped = helped + 1"
            }
            MemoryUse::Misled => {
                "INSERT INTO memory_use(memory_id, misled) SELECT id, 1 FROM memories WHERE id = ?
                 ON CONFLICT(memory_id) DO UPDATE SET misled = misled + 1"
            }
        };
        self.exec(query, [id])?;
        Ok(())
    }

    /// Stores v as memory id's vector under model, made of the passage whose hash is hash: only while the memory still has
    /// that passage, so a vector made before it changed is never stored over the change. Whether it stored it.
    pub fn put_memory_vector(&self, id: &str, model: &str, hash: &str, v: &[f32]) -> Result<bool> {
        let m = match self.memory(id) {
            Ok(m) => m,
            Err(Error::NotFound) => return Ok(false),
            Err(err) => return Err(err),
        };
        if passage_hash(&m) != hash {
            return Ok(false);
        }
        self.exec(
            "INSERT INTO memory_embeddings(memory_id, model, passage_hash, vec) VALUES(?,?,?,?)
             ON CONFLICT(memory_id, model) DO UPDATE SET passage_hash=excluded.passage_hash, vec=excluded.vec",
            params![id, model, hash, vec_blob(v)],
        )?;
        self.exec(
            "DELETE FROM memory_unembedded WHERE memory_id=? AND model=?",
            params![id, model],
        )?;
        Ok(true)
    }

    /// Records that model refused to embed memory id's passage of hash hash at at, for why: the embedding job leaves it
    /// until a day after, or until the passage changes.
    pub fn put_memory_unembedded(&self, id: &str, model: &str, hash: &str, why: &str, at: Time) -> Result<()> {
        self.exec(
            "INSERT INTO memory_unembedded(memory_id, model, passage_hash, at, why) SELECT id, ?2, ?3, ?4, ?5 FROM memories
             WHERE id=?1 ON CONFLICT(memory_id, model) DO UPDATE SET passage_hash=excluded.passage_hash, at=excluded.at,
             why=excluded.why",
            params![id, model, hash, ms(at), why],
        )?;
        Ok(())
    }

    /// Records t when no teaching of its id is. A teaching recorded again stays as it stands.
    pub fn add_teaching(&self, t: &Teaching) -> Result<()> {
        self.exec(
            "INSERT OR IGNORE INTO teachings(id, state, created_at, holder, card, doc) VALUES(?,?,?,?,?,?)",
            params![t.id, t.state.as_str(), ms(t.created_at), t.holder, t.card, doc(t)?],
        )?;
        Ok(())
    }

    /// Writes t over what was stored of it.
    pub fn put_teaching(&self, t: &Teaching) -> Result<()> {
        self.exec(
            "INSERT INTO teachings(id, state, created_at, holder, card, doc) VALUES(?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET state=excluded.state, holder=excluded.holder, card=excluded.card,
             doc=excluded.doc",
            params![t.id, t.state.as_str(), ms(t.created_at), t.holder, t.card, doc(t)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The memory id names.
    pub fn memory(&self, id: &str) -> Result<Memory> {
        self.one(
            concat!("SELECT ", memory_doc!(), " FROM ", memories!(), " WHERE m.id=?"),
            [id],
        )
    }

    /// The memories ids names that are stored, in no order.
    pub fn memories_named(&self, ids: &[String]) -> Result<Vec<Memory>> {
        self.list(
            concat!(
                "SELECT ",
                memory_doc!(),
                " FROM ",
                memories!(),
                " WHERE m.id IN (SELECT value FROM json_each(?))"
            ),
            [serde_json::to_string(ids)?],
        )
    }

    /// Every memory holder holds, whatever its status, the latest changed first.
    pub fn memories_of(&self, holder: &str) -> Result<Vec<Memory>> {
        self.list(
            concat!(
                "SELECT ",
                memory_doc!(),
                " FROM ",
                memories!(),
                " WHERE m.holder=? ORDER BY m.updated_at DESC, m.id"
            ),
            [holder],
        )
    }

    /// What reader may recall, the latest changed first: its own active memories, its project's, its lineage's and
    /// everyone's.
    pub fn readable_memories(&self, reader: &MemoryReader) -> Result<Vec<Memory>> {
        self.list(
            concat!(
                "SELECT ",
                memory_doc!(),
                " FROM ",
                memories!(),
                " WHERE ",
                readable!("m"),
                " ORDER BY m.updated_at DESC, m.id"
            ),
            &reader.params()[..],
        )
    }

    /// The memories reader may recall whose words match text, best first, at most limit, each with its rank (FTS5's
    /// bm25 over title, body, evidence and subjects, weighed 4, 2, 1 and 2: lower is better).
    pub fn memories_matching(&self, reader: &MemoryReader, text: &str, limit: i64) -> Result<Vec<(String, f64)>> {
        let Some(q) = words_query(text) else {
            return Ok(Vec::new());
        };
        let mut all: Vec<(&str, &dyn ToSql)> = reader.params().to_vec();
        all.push((":q", &q));
        all.push((":limit", &limit));
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT m.id, bm25(memory_text, 4.0, 2.0, 1.0, 2.0) AS rank FROM memory_text t JOIN memories m ON m.n = t.rowid
             WHERE memory_text MATCH :q AND ",
            readable!("m"),
            " ORDER BY rank LIMIT :limit"
        ))?;
        let rows = stmt.query_map(&all[..], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The memories reader may recall that are about any of subjects, those about the most of them first, then the latest
    /// changed.
    pub fn memories_about(&self, reader: &MemoryReader, subjects: &[String]) -> Result<Vec<Memory>> {
        if subjects.is_empty() {
            return Ok(Vec::new());
        }
        let subjects = serde_json::to_string(subjects)?;
        let mut all: Vec<(&str, &dyn ToSql)> = reader.params().to_vec();
        all.push((":subjects", &subjects));
        self.list(
            concat!(
                "SELECT ",
                memory_doc!(),
                " FROM ",
                memories!(),
                " JOIN (SELECT src, count(*) AS n FROM memory_edges WHERE kind = 'about'
                   AND dst IN (SELECT value FROM json_each(:subjects)) GROUP BY src) a ON a.src = m.id WHERE ",
                readable!("m"),
                " ORDER BY a.n DESC, m.updated_at DESC, m.id"
            ),
            &all[..],
        )
    }

    /// The vectors under model of the memories reader may recall.
    pub fn readable_vectors(&self, reader: &MemoryReader, model: &str) -> Result<Vec<(String, Vec<f32>)>> {
        let mut all: Vec<(&str, &dyn ToSql)> = reader.params().to_vec();
        all.push((":model", &model));
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT v.memory_id, v.vec FROM memory_embeddings v JOIN memories m ON m.id = v.memory_id WHERE v.model = :model AND ",
            readable!("m")
        ))?;
        let rows = stmt.query_map(&all[..], |r| {
            let b: Vec<u8> = r.get(1)?;
            Ok((r.get(0)?, blob_vec(&b)))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The edges that touch ids: from them, to them, and from any memory to a subject one of them is about.
    pub fn memory_edges_near(&self, ids: &[String]) -> Result<Vec<MemoryEdge>> {
        let ids = serde_json::to_string(ids)?;
        let mut stmt = self.c.prepare_cached(
            "SELECT src, dst, kind FROM memory_edges WHERE src IN (SELECT value FROM json_each(?1)) OR dst IN (SELECT value FROM json_each(?1))
             UNION SELECT e.src, e.dst, e.kind FROM memory_edges e WHERE e.kind = 'about' AND e.dst IN
               (SELECT dst FROM memory_edges WHERE kind = 'about' AND src IN (SELECT value FROM json_each(?1)))",
        )?;
        let rows = stmt.query_map([ids], |r| {
            Ok(MemoryEdge {
                src: r.get(0)?,
                dst: r.get(1)?,
                kind: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The active memories with no vector under model made of the passage they have now, the oldest first, at most
    /// limit; one model refused after refused_before is left out until then, or until its passage changes.
    pub fn memories_to_embed(&self, model: &str, refused_before: Time, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT ",
            memory_doc!(),
            ", v.passage_hash, x.passage_hash, x.at, m.id FROM ",
            memories!(),
            " LEFT JOIN memory_embeddings v ON v.memory_id = m.id AND v.model = ?1
             LEFT JOIN memory_unembedded x ON x.memory_id = m.id AND x.model = ?1
             WHERE m.status = 'active' ORDER BY m.created_at, m.id"
        ))?;
        let mut rows = stmt.query([model])?;
        let mut out = Vec::new();
        while out.len() < limit
            && let Some(row) = rows.next()?
        {
            let text: String = row.get(0)?;
            let (vector, refused, at): (Option<String>, Option<String>, Option<i64>) =
                (row.get(1)?, row.get(2)?, row.get(3)?);
            let m = match serde_json::from_str::<Memory>(&text) {
                Ok(m) => m,
                Err(why) => {
                    let id: String = row.get(4)?;
                    let _left_out = self.left_out("Memory", &id, &why);
                    continue;
                }
            };
            let hash = passage_hash(&m);
            let held = refused.as_deref() == Some(hash.as_str()) && at.is_some_and(|at| from_ms(at) > refused_before);
            if vector.as_deref() != Some(hash.as_str()) && !held {
                out.push(m);
            }
        }
        Ok(out)
    }

    /// Every active memory, with who holds each and whom it reaches, and its vector under model when it has one: what
    /// consolidation reviews.
    pub fn active_memories(&self, model: &str) -> Result<Vec<(Memory, Option<Vec<f32>>)>> {
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT ",
            memory_doc!(),
            ", v.vec FROM ",
            memories!(),
            " LEFT JOIN memory_embeddings v ON v.memory_id = m.id AND v.model = ?1 WHERE m.status = ?2 ORDER BY m.id"
        ))?;
        let rows = stmt.query_map(params![model, MemoryStatus::Active.as_str()], |r| {
            let d: String = r.get(0)?;
            let b: Option<Vec<u8>> = r.get(1)?;
            Ok((d, b.map(|b| blob_vec(&b))))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (d, v) = row?;
            out.push((serde_json::from_str(&d)?, v));
        }
        Ok(out)
    }

    /// The teachings of holder's memory, the latest first, at most limit: what its memory screen lists beside what it
    /// learned.
    pub fn teachings_of(&self, holder: &str, limit: i64) -> Result<Vec<Teaching>> {
        self.list(
            "SELECT doc FROM teachings WHERE holder = ? ORDER BY created_at DESC, id LIMIT ?",
            params![holder, limit],
        )
    }

    /// The teaching id names.
    pub fn teaching(&self, id: &str) -> Result<Teaching> {
        self.one("SELECT doc FROM teachings WHERE id=?", [id])
    }

    /// The teachings that wait for the librarian, the oldest first, at most limit.
    pub fn waiting_teachings(&self, limit: i64) -> Result<Vec<Teaching>> {
        self.list(
            "SELECT doc FROM teachings WHERE state=? ORDER BY created_at, id LIMIT ?",
            params![TeachingState::Waiting.as_str(), limit],
        )
    }

    /// The teachings of card card_id, any state, the oldest first.
    pub fn teachings_of_card(&self, card_id: &str) -> Result<Vec<Teaching>> {
        self.list(
            "SELECT doc FROM teachings WHERE card = ? ORDER BY created_at, id",
            [card_id],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{blob_vec, vec_blob, words_query};

    #[test]
    fn a_query_quotes_each_word_that_says_something_and_a_vector_survives_its_blob() {
        assert_eq!(
            words_query("Mục tiêu của tôi: rõ ràng!").as_deref(),
            Some("\"mục\" OR \"tiêu\" OR \"rõ\" OR \"ràng\"")
        );
        assert_eq!(
            words_query("how do I fix the build"),
            Some("\"fix\" OR \"build\"".to_owned())
        );
        assert_eq!(words_query("a ! ? and the"), None);
        let v = vec![0.5_f32, -1.25, 3.0];
        assert_eq!(blob_vec(&vec_blob(&v)), v);
    }
}
