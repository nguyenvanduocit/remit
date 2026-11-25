//! Landings and releases (ADR-0114, ADR-0116): a landing is one version of a chain's change on its way to a repository's
//! base, a release one run of a repository's procedure on a landed commit, and a release pause holds a repository's
//! releases. Each is one document with a revision, like a
//! help; the queue a landing waits in (its host, project and base) and its state are columns generated from its
//! document, which the queue reads by, and so is the release that carried it, which a release reads its landings by.
//! Landings outlive their chain (`KEPT_PAST_HOME`): what landed is on the base whoever made it, and the queue reverts it
//! from its record.

use model::{Landing, QueueKey, Release, ReleasePause};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes l over the revision it was read at, l.rev (0 for a new landing), and returns it at the revision stored.
    /// `Error::Conflict` when the stored landing is at another revision, or a new one's id is taken.
    pub fn put_landing(&self, mut l: Landing) -> Result<Landing> {
        l.rev = l.rev.saturating_add(1);
        l.rev = self.put_doc(
            concat!(
                "INSERT INTO landings(id, chain_id, repo, created_at, doc) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("landings"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![l.id, l.chain_id, l.repo, ms(l.created_at), doc(&l)?],
        )?;
        Ok(l)
    }

    /// Writes r over the revision it was read at, r.rev (0 for a new release), and returns it at the revision stored.
    /// `Error::Conflict` as put_landing.
    pub fn put_release(&self, mut r: Release) -> Result<Release> {
        r.rev = r.rev.saturating_add(1);
        r.rev = self.put_doc(
            concat!(
                "INSERT INTO releases(id, repo, created_at, doc) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("releases"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![r.id, r.repo, ms(r.created_at), doc(&r)?],
        )?;
        Ok(r)
    }

    /// Keeps mr as migration 0123 carries one over: what a test of the close starts from.
    #[cfg(feature = "testing")]
    pub fn carry_mr(&self, mr: &CarriedMr) -> Result<()> {
        self.exec(
            "INSERT OR REPLACE INTO carried_mrs(landing_id, host, project, iid, url) VALUES(?,?,?,?,?)",
            params![mr.landing, mr.host, mr.project, mr.iid, mr.url],
        )?;
        Ok(())
    }

    /// Writes p, the pause of repository p.repo's releases, pulled or lifted; the one already there is replaced.
    pub fn put_release_pause(&self, p: &ReleasePause) -> Result<()> {
        self.exec(
            "INSERT INTO release_pauses(repo, doc) VALUES(?,?) ON CONFLICT(repo) DO UPDATE SET doc=excluded.doc",
            params![p.repo, doc(p)?],
        )?;
        Ok(())
    }

    /// Forgets the merge request migration 0123 carried over for landing, once it is closed on its forge.
    pub fn forget_carried_mr(&self, landing: &str) -> Result<()> {
        self.exec("DELETE FROM carried_mrs WHERE landing_id=?", [landing])?;
        Ok(())
    }
}

/// A draft merge request Remit opened before it landed changes itself, carried over by migration 0123 to be closed on its
/// forge once (`close_carried_mrs` in shipping).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedMr {
    pub landing: String,
    pub host: String,
    pub project: String,
    pub iid: i64,
    pub url: String,
}

impl Reader<'_> {
    pub fn landing(&self, id: &str) -> Result<Landing> {
        self.one("SELECT doc FROM landings WHERE id=?", [id])
    }

    /// The landings of chain chain_id, the oldest first.
    pub fn chain_landings(&self, chain_id: &str) -> Result<Vec<Landing>> {
        self.list(
            "SELECT doc FROM landings WHERE chain_id=? ORDER BY created_at, id",
            [chain_id],
        )
    }

    /// The landings of repository repo, the oldest first: its queue and its history.
    pub fn repo_landings(&self, repo: &str) -> Result<Vec<Landing>> {
        self.list("SELECT doc FROM landings WHERE repo=? ORDER BY created_at, id", [repo])
    }

    /// The landings of one queue, the oldest first.
    pub fn queue_landings(&self, q: &QueueKey) -> Result<Vec<Landing>> {
        self.list(
            "SELECT doc FROM landings WHERE host=? AND project=? AND base=? ORDER BY created_at, id",
            params![q.host, q.project, q.base],
        )
    }

    /// The landings of every queue still on their way (judging, contending, ready, combining or checking), the oldest
    /// first.
    pub fn open_landings(&self) -> Result<Vec<Landing>> {
        self.list(
            "SELECT doc FROM landings WHERE state IN ('judging', 'contending', 'ready', 'combining', 'checking') ORDER BY created_at, id",
            [],
        )
    }

    /// Every landing, the oldest first.
    pub fn landings(&self) -> Result<Vec<Landing>> {
        self.list("SELECT doc FROM landings ORDER BY created_at, id", [])
    }

    /// The newest landing of a stage on repository host/project that is on its base: the stage the next one is judged
    /// against.
    pub fn last_stage_on(&self, host: &str, project: &str) -> Result<Option<Landing>> {
        let mut found: Vec<Landing> = self.list(
            "SELECT doc FROM landings WHERE host=? AND project=? AND state='landed' \
             AND json_type(doc, '$.version.plan') IS NOT NULL \
             ORDER BY json_extract(doc, '$.landedAt') DESC, id DESC LIMIT 1",
            params![host, project],
        )?;
        Ok(found.pop())
    }

    /// The landings release carried, the oldest first.
    pub fn release_landings(&self, release: &str) -> Result<Vec<Landing>> {
        self.list(
            "SELECT doc FROM landings WHERE release=? ORDER BY created_at, id",
            [release],
        )
    }

    pub fn release(&self, id: &str) -> Result<Release> {
        self.one("SELECT doc FROM releases WHERE id=?", [id])
    }

    /// The releases of repository repo, the newest first: of two made in one instant, the one stored later (its rowid,
    /// which a rewrite keeps), as ids are random.
    pub fn repo_releases(&self, repo: &str) -> Result<Vec<Release>> {
        self.list(
            "SELECT doc FROM releases WHERE repo=? ORDER BY created_at DESC, rowid DESC",
            [repo],
        )
    }

    /// Every release, the oldest first.
    pub fn releases(&self) -> Result<Vec<Release>> {
        self.list("SELECT doc FROM releases ORDER BY created_at, rowid", [])
    }

    /// The releases still on their way (`Release::open`): their steps run, or they bake. One a repository at most.
    pub fn open_releases(&self) -> Result<Vec<Release>> {
        self.list(
            "SELECT doc FROM releases WHERE json_extract(doc, '$.state.kind') IN ('releasing', 'baking')",
            [],
        )
    }

    /// The last pause of repository repo's releases, held or lifted (`ReleasePause::holds`); none was ever pulled.
    pub fn release_pause(&self, repo: &str) -> Result<Option<ReleasePause>> {
        let mut found: Vec<ReleasePause> = self.list("SELECT doc FROM release_pauses WHERE repo=?", [repo])?;
        Ok(found.pop())
    }

    /// The draft merge requests migration 0123 carried over that are not closed yet.
    pub fn carried_mrs(&self) -> Result<Vec<CarriedMr>> {
        let mut st = self
            .c
            .prepare_cached("SELECT landing_id, host, project, iid, url FROM carried_mrs ORDER BY landing_id")?;
        let rows = st.query_map([], |r| {
            Ok(CarriedMr {
                landing: r.get(0)?,
                host: r.get(1)?,
                project: r.get(2)?,
                iid: r.get(3)?,
                url: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
