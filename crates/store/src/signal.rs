//! Signals (docs/adr/0014): the subscriptions to them, their readings, and the firings that record a subscription acting
//! on a reading.

use model::{
    Control, Firing, FiringKey, NearMiss, OwnerKind, Reading, SignalKey, SignalPattern, Subscription,
    SubscriptionOwner, Time,
};
use rusqlite::{ToSql, params};
use serde::Deserialize;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

/// When sub can fire with no new reading (`Subscription::due`). NULL when it has no time, so the index of due
/// subscriptions holds only the ones the tick must look at.
fn due_at(sub: &Subscription) -> Option<i64> {
    sub.due().map(ms)
}

/// sub's control as the column spells it, so a query reads it by name: active, paused or ended.
fn control_column(sub: &Subscription) -> &str {
    if sub.control == Control::Active {
        "active"
    } else {
        sub.control.as_str()
    }
}

/// One stored row of the signal backbone: a reading of a signal, or a firing of a subscription on one.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SignalRow {
    pub reading: Option<Reading>,
    pub firing: Option<Firing>,
}

/// Narrows the firings read: those of one subscription, those on one key, both, or neither (every firing).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FiringFilter {
    pub subscription: String,
    pub key: SignalKey,
}

/// True of a row of readings no one needs any more (docs/adr/0064, Start position and retention): no live subscription
/// (not ended) whose pattern lets its key through has yet to judge it, and no firing still to be done or held on covers
/// it. A held or paused subscription so keeps every reading it has not judged, and the readings of its held firing.
macro_rules! unneeded {
    () => {
        "NOT EXISTS (SELECT 1 FROM subscriptions AS s WHERE s.control <> 'ended' AND s.seen_seq < readings.seq AND readings.key GLOB s.match)
AND NOT EXISTS (SELECT 1 FROM firings AS f WHERE f.status IN ('pending', 'failed') AND f.key = readings.key
  AND EXISTS (SELECT 1 FROM json_each(f.doc, '$.seqs') WHERE value = readings.seq))"
    };
}

impl Tx<'_> {
    /// Writes sub over the revision it was read at, sub.rev (0 for a new subscription), and returns it at the revision
    /// stored. `Error::Conflict` when the stored subscription is at another revision, or a new one's id is taken.
    pub fn put_subscription(&self, mut sub: Subscription) -> Result<Subscription> {
        sub.rev = sub.rev.saturating_add(1);
        sub.rev = self.put_doc(
            concat!(
                "INSERT INTO subscriptions(id, owner_kind, owner_id, match, control, due_at, seen_seq, doc) VALUES(?,?,?,?,?,?,?,?)
ON CONFLICT(id) DO UPDATE SET owner_kind=excluded.owner_kind, owner_id=excluded.owner_id, match=excluded.match, control=excluded.control, due_at=excluded.due_at, seen_seq=excluded.seen_seq, doc=excluded.doc",
                over_read_rev!("subscriptions"),
                "
RETURNING json_extract(doc, '$.rev')"
            ),
            params![
                sub.id,
                sub.owner.kind.as_str(),
                sub.owner.id,
                sub.r#match.0,
                control_column(&sub),
                due_at(&sub),
                sub.seen_seq,
                doc(&sub)?
            ],
        )?;
        Ok(sub)
    }

    /// Ends the subscription id and its near-misses. Its firings stay, as the record of what it did; the ones still to
    /// be done, or held on, are cancelled: nothing will do them, and so is the owner's item that it is held.
    pub fn delete_subscription(&self, id: &str) -> Result<()> {
        self.delete_subscriptions("SELECT id FROM subscriptions WHERE id=?1", &[&id])
    }

    /// Ends every subscription that belongs to owner, as delete_subscription ends one.
    pub fn delete_subscriptions_of(&self, owner: &SubscriptionOwner) -> Result<()> {
        self.delete_subscriptions(
            "SELECT id FROM subscriptions WHERE owner_kind=?1 AND owner_id=?2",
            &[&owner.kind.as_str(), &owner.id],
        )
    }

    /// Deletes the subscriptions whose ids ids selects, from args, with what goes with them.
    fn delete_subscriptions(&self, ids: &str, args: &[&dyn ToSql]) -> Result<()> {
        for q in [
            format!(
                "UPDATE firings SET status='cancelled', doc=json_set(doc, '$.status', 'cancelled') WHERE status IN ('pending', 'failed') AND subscription_id IN ({ids})"
            ),
            format!("DELETE FROM near_misses WHERE subscription_id IN ({ids})"),
            // The owner's item that a deleted subscription is held is answered by no one now.
            format!(
                "UPDATE inbox SET status='cancelled', doc=json_set(doc, '$.status', 'cancelled', '$.answer', 'the subscription is gone', '$.rev', json_extract(doc, '$.rev')+1)
WHERE status='open' AND json_extract(doc, '$.subscription') IN ({ids})"
            ),
            format!("DELETE FROM subscriptions WHERE id IN ({ids})"),
        ] {
            self.exec(&q, args)?;
        }
        Ok(())
    }

    /// The next place in the log: the counter is moved on in the write itself, so two readings never share one, and a
    /// number given is never given again.
    fn next_seq(&self) -> Result<i64> {
        self.value("UPDATE log_seq SET last = last + 1 WHERE id = 1 RETURNING last", [])
    }

    /// Stores r as the next reading of its key, at the next place in the log, and returns it with the revision and seq
    /// it got. The revision is counted in the write itself, so two readings of a key never share one.
    pub fn add_reading(&self, mut r: Reading) -> Result<Reading> {
        r.seq = self.next_seq()?;
        r.rev = self.value(
            "INSERT INTO readings(key, rev, at, seq, doc) SELECT ?, COALESCE(MAX(rev), 0)+1, ?, ?, json_set(?, '$.rev', COALESCE(MAX(rev), 0)+1) FROM readings WHERE key=? RETURNING rev",
            params![r.key.0, ms(r.at), r.seq, doc(&r)?, r.key.0],
        )?;
        Ok(r)
    }

    /// Drops the readings past their kind's history that no one needs (unneeded; docs/adr/0064, Start position and
    /// retention): of a stream (`model::stream_key_prefixes`), those stored before streams_since; of a state, all but its
    /// newest states_kept. Never the newest of a key: a revision counts on from the newest, and a key emptied would start
    /// again at 1 under subscriptions that have judged far past it. It answers how many it dropped.
    pub fn forget_readings(&self, streams_since: Time, states_kept: i64) -> Result<i64> {
        let n = self.exec(
            concat!(
                "DELETE FROM readings WHERE rev < (SELECT MAX(newest.rev) FROM readings AS newest WHERE newest.key = readings.key)
AND CASE WHEN EXISTS (SELECT 1 FROM json_each(?1) AS family WHERE substr(readings.key, 1, length(family.value)) = family.value)
  THEN at < ?2
  ELSE rev <= (SELECT MAX(newest.rev) FROM readings AS newest WHERE newest.key = readings.key) - ?3 END
AND ",
                unneeded!()
            ),
            params![doc(&model::stream_key_prefixes())?, ms(streams_since), states_kept],
        )?;
        Ok(i64::try_from(n).unwrap_or(i64::MAX))
    }

    /// Drops the reading of key at revision rev, unless it is the newest: a revision counts on from the newest.
    pub fn delete_reading(&self, key: &SignalKey, rev: i64) -> Result<()> {
        self.exec(
            "DELETE FROM readings WHERE key=?1 AND rev=?2 AND rev < (SELECT MAX(rev) FROM readings WHERE key=?1)",
            params![key.0, rev],
        )?;
        Ok(())
    }

    /// Stores f unless a firing of the subscription on that key and revision is stored already, which a retry or a
    /// restart is: false says nothing was added. Part of the Tx that makes the firing's effect, so both are stored or
    /// neither.
    pub fn add_firing(&self, f: &Firing) -> Result<bool> {
        let n = self.exec(
            "INSERT INTO firings(subscription_id, key, rev, at, owner_kind, owner_id, status, doc) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT DO NOTHING",
            params![f.subscription_id, f.key.0, f.rev, ms(f.at), f.owner.kind.as_str(), f.owner.id, f.status.as_str(), doc(f)?],
        )?;
        Ok(n == 1)
    }

    /// Writes f over the stored firing of its key: where a firing stored before stands now. `Error::NotFound` when none
    /// is stored.
    pub fn update_firing(&self, f: &Firing) -> Result<()> {
        let n = self.exec(
            "UPDATE firings SET at=?, status=?, doc=? WHERE subscription_id=? AND key=? AND rev=?",
            params![ms(f.at), f.status.as_str(), doc(f)?, f.subscription_id, f.key.0, f.rev],
        )?;
        if n == 0 { Err(crate::Error::NotFound) } else { Ok(()) }
    }

    /// Cancels the firings of subscription id still to be done: a wait set again answers the instants of its new
    /// schedule, not the ones recorded before it was set. A failed one stays, as the record its hold names.
    pub fn cancel_pending_firings(&self, id: &str) -> Result<()> {
        self.exec(
            "UPDATE firings SET status='cancelled', doc=json_set(doc, '$.status', 'cancelled') WHERE status IN ('pending', 'failed') AND status='pending' AND subscription_id=?",
            [id],
        )?;
        Ok(())
    }

    /// Drops the firings recorded before t but the ones still to be done or held on: what a subscription will still do
    /// is never forgotten.
    pub fn forget_firings_before(&self, t: Time) -> Result<()> {
        self.exec(
            "DELETE FROM firings WHERE at<? AND status NOT IN ('pending', 'failed')",
            [ms(t)],
        )?;
        Ok(())
    }

    /// Stores the near-misses of subscription id, each once, and keeps only its newest keep.
    pub fn add_near_misses(&self, id: &str, misses: &[NearMiss], keep: i64) -> Result<()> {
        if misses.is_empty() {
            return Ok(());
        }
        for m in misses {
            self.exec(
                "INSERT OR IGNORE INTO near_misses(subscription_id, seq, reason, at, doc) VALUES(?,?,?,?,?)",
                params![id, m.seq, m.reason.as_str(), ms(m.at), doc(m)?],
            )?;
        }
        self.exec(
            "DELETE FROM near_misses WHERE subscription_id=?1 AND seq NOT IN (SELECT seq FROM near_misses WHERE subscription_id=?1 ORDER BY seq DESC LIMIT ?2)",
            params![id, keep],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn subscription(&self, id: &str) -> Result<Subscription> {
        self.one("SELECT doc FROM subscriptions WHERE id=?", [id])
    }

    /// Every subscription, by id.
    pub fn subscriptions(&self) -> Result<Vec<Subscription>> {
        self.list("SELECT doc FROM subscriptions ORDER BY id", [])
    }

    /// The subscriptions that belong to owner, by id.
    pub fn subscriptions_of(&self, owner: &SubscriptionOwner) -> Result<Vec<Subscription>> {
        self.list(
            "SELECT doc FROM subscriptions WHERE owner_kind=? AND owner_id=? ORDER BY id",
            [owner.kind.as_str(), owner.id.as_str()],
        )
    }

    /// The subscriptions every chain holds, by id.
    pub fn chain_subscriptions(&self) -> Result<Vec<Subscription>> {
        self.list(
            "SELECT doc FROM subscriptions WHERE owner_kind=? ORDER BY id",
            [OwnerKind::Chain.as_str()],
        )
    }

    /// The active subscriptions whose pattern matches key, by id.
    pub fn subscriptions_on_key(&self, key: &SignalKey) -> Result<Vec<Subscription>> {
        self.list(
            "SELECT doc FROM subscriptions WHERE control='active' AND ? GLOB match ORDER BY id",
            [key.0.as_str()],
        )
    }

    /// The active subscriptions whose due time has come by now, the earliest first.
    pub fn due_subscriptions(&self, now: Time) -> Result<Vec<Subscription>> {
        self.list(
            "SELECT doc FROM subscriptions WHERE control='active' AND due_at IS NOT NULL AND due_at<=? ORDER BY due_at, id",
            [ms(now)],
        )
    }

    /// The active subscriptions with a stored reading past their place in the log that their pattern matches, by id.
    pub fn subscriptions_with_unread(&self) -> Result<Vec<Subscription>> {
        self.list(
            "SELECT doc FROM subscriptions s WHERE s.control='active' AND EXISTS (SELECT 1 FROM readings r WHERE r.seq>s.seen_seq AND r.key GLOB s.match) ORDER BY s.id",
            [],
        )
    }

    /// The newest place in the log given so far; zero before any.
    pub fn last_seq(&self) -> Result<i64> {
        self.value("SELECT last FROM log_seq WHERE id = 1", [])
    }

    /// The place in the log just before the first reading stored at or after t: where a subscription that judges from t
    /// stands. The newest place when none is.
    pub fn seq_before(&self, t: Time) -> Result<i64> {
        self.value(
            "SELECT COALESCE((SELECT MIN(seq) FROM readings WHERE at >= ?) - 1, (SELECT last FROM log_seq WHERE id = 1))",
            [ms(t)],
        )
    }

    /// The reading of key at revision rev; `Error::NotFound` for one that is not stored.
    pub fn reading(&self, key: &SignalKey, rev: i64) -> Result<Reading> {
        self.one("SELECT doc FROM readings WHERE key=? AND rev=?", params![key.0, rev])
    }

    /// The reading of key whose dedupe is dedupe: the event a writer's retry is the same as.
    pub fn reading_deduped(&self, key: &SignalKey, dedupe: &str) -> Result<Reading> {
        self.one(
            "SELECT doc FROM readings WHERE key=? AND json_extract(doc, '$.dedupe')=?",
            [key.0.as_str(), dedupe],
        )
    }

    /// The newest n readings of any event signal, the newest first.
    pub fn newest_event_readings(&self, n: i64) -> Result<Vec<Reading>> {
        self.list(
            "SELECT doc FROM readings WHERE substr(key, 1, length(?1))=?1 ORDER BY at DESC, rev DESC, key LIMIT ?2",
            params![model::EVENT_KEY_PREFIX, n],
        )
    }

    /// The newest reading of key; `Error::NotFound` while it has none.
    pub fn last_reading(&self, key: &SignalKey) -> Result<Reading> {
        self.one(
            "SELECT doc FROM readings WHERE key=? ORDER BY rev DESC LIMIT 1",
            [key.0.as_str()],
        )
    }

    /// The newest n readings of key, newest first.
    pub fn newest_readings(&self, key: &SignalKey, n: i64) -> Result<Vec<Reading>> {
        self.list(
            "SELECT doc FROM readings WHERE key=? ORDER BY rev DESC LIMIT ?",
            params![key.0, n],
        )
    }

    /// The newest limit readings and firings whose key matches pattern, a GLOB (`fact:*`, `chain:*.live`, `*` for
    /// all), newest first. The rows are picked on their keys and times, and only those picked are read whole.
    pub fn signal_rows(&self, pattern: &str, limit: i64) -> Result<Vec<SignalRow>> {
        self.list(
            "WITH top AS (
	SELECT 'reading' AS kind, key, rev, '' AS sub, at FROM readings WHERE key GLOB ?1
	UNION ALL
	SELECT 'firing', key, rev, subscription_id, at FROM firings WHERE key GLOB ?1
	ORDER BY at DESC, kind, key, rev DESC LIMIT ?2)
SELECT CASE top.kind WHEN 'reading' THEN json_object('reading', json(r.doc)) ELSE json_object('firing', json(f.doc)) END
FROM top
LEFT JOIN readings r ON top.kind = 'reading' AND r.key = top.key AND r.rev = top.rev
LEFT JOIN firings f ON top.kind = 'firing' AND f.subscription_id = top.sub AND f.key = top.key AND f.rev = top.rev
ORDER BY top.at DESC, top.kind, top.key, top.rev DESC",
            params![pattern, limit],
        )
    }

    /// The newest reading of each key that has readings, without its value, newest first: the signals as the owner lists
    /// them. A reading's value can be large and the list needs only who made it and when.
    pub fn signal_heads(&self) -> Result<Vec<Reading>> {
        self.list(
            "SELECT json_remove(doc, '$.value') FROM readings r WHERE rev=(SELECT MAX(rev) FROM readings WHERE key=r.key) ORDER BY at DESC, key",
            [],
        )
    }

    /// The keys with a stored reading that begin with prefix, by name.
    pub fn reading_keys_with_prefix(&self, prefix: &str) -> Result<Vec<SignalKey>> {
        let keys: Vec<String> = self.column(
            "SELECT DISTINCT key FROM readings WHERE substr(key, 1, length(?1))=?1 ORDER BY key",
            [prefix],
        )?;
        Ok(keys.into_iter().map(SignalKey).collect())
    }

    /// The exact keys, each once and by name, that an active subscription listens to and that begin with prefix: the
    /// signals a chain watches (list_signals). A pattern with a wildcard names no one key and is not one of them.
    /// (substr, not LIKE: a prefix is text, and `_` in it is no wildcard.)
    pub fn active_keys_with_prefix(&self, prefix: &str) -> Result<Vec<SignalKey>> {
        let keys: Vec<String> = self.column(
            "SELECT DISTINCT match FROM subscriptions WHERE control='active' AND substr(match, 1, length(?1))=?1 AND match NOT GLOB '*[*?[]*' ORDER BY match",
            [prefix],
        )?;
        Ok(keys.into_iter().map(SignalKey).collect())
    }

    /// The stored readings whose key the pattern matches, after place after_seq in the log, oldest first, at most limit.
    pub fn unread_readings(&self, pattern: &SignalPattern, after_seq: i64, limit: i64) -> Result<Vec<Reading>> {
        self.list(
            "SELECT doc FROM readings WHERE seq>? AND key GLOB ? ORDER BY seq LIMIT ?",
            params![after_seq, pattern.0, limit],
        )
    }

    /// The reading at place seq in the log; `Error::NotFound` when none is stored there (never given, or forgotten).
    pub fn reading_at(&self, seq: i64) -> Result<Reading> {
        self.one("SELECT doc FROM readings WHERE seq=?", [seq])
    }

    /// How many stored readings whose key the pattern matches come after place after_seq in the log: what a
    /// subscription standing there has not judged.
    pub fn count_unread(&self, pattern: &SignalPattern, after_seq: i64) -> Result<i64> {
        self.value(
            "SELECT COUNT(*) FROM readings WHERE seq>? AND key GLOB ?",
            params![after_seq, pattern.0],
        )
    }

    /// The readings of key at the places seqs in the log, ascending: those a firing covers. A reading forgotten is not
    /// among them; none are kept past what a pending or failed firing covers (unneeded), so a firing still to be
    /// delivered has all of its own.
    pub fn readings_of(&self, key: &SignalKey, seqs: &[i64]) -> Result<Vec<Reading>> {
        if seqs.is_empty() {
            return Ok(Vec::new());
        }
        self.list(
            "SELECT doc FROM readings WHERE key=? AND seq IN (SELECT value FROM json_each(?)) ORDER BY seq",
            params![key.0, doc(seqs)?],
        )
    }

    /// The firings recorded and not yet done of subscription id, every subscription's when id is "", by subscription in
    /// the order they are delivered (docs/adr/0064): by the time they were decided, then by the place in the log of the
    /// first reading each covers, an expiry, which covers none, after the readings decided with it. (The status is
    /// named as the index firings_open names it, so the query reads that index and not every firing.)
    pub fn pending_firings(&self, id: &str) -> Result<Vec<Firing>> {
        self.list(
            "SELECT doc FROM firings WHERE status IN ('pending', 'failed') AND status='pending' AND (?1='' OR subscription_id=?1)
ORDER BY subscription_id, at, COALESCE(json_extract(doc, '$.seqs[0]'), 9223372036854775807), key, rev",
            [id],
        )
    }

    /// The firing of key; `Error::NotFound` when none is stored.
    pub fn firing(&self, key: &FiringKey) -> Result<Firing> {
        self.one(
            "SELECT doc FROM firings WHERE subscription_id=? AND key=? AND rev=?",
            params![key.subscription, key.key.0, key.rev],
        )
    }

    /// The newest limit firings that match f, newest first.
    pub fn firings(&self, f: &FiringFilter, limit: i64) -> Result<Vec<Firing>> {
        self.list(
            "SELECT doc FROM firings WHERE (?1='' OR subscription_id=?1) AND (?2='' OR key=?2) ORDER BY at DESC, rev DESC LIMIT ?3",
            params![f.subscription, f.key.0, limit],
        )
    }

    /// The firings that covered the reading of key at place seq in the log, oldest first: one for each subscription
    /// that held for it, whether it fired for it alone or for it with others.
    pub fn firings_on(&self, key: &SignalKey, seq: i64) -> Result<Vec<Firing>> {
        self.list(
            "SELECT doc FROM firings WHERE key=?1 AND EXISTS (SELECT 1 FROM json_each(doc, '$.seqs') WHERE value=?2) ORDER BY at, subscription_id",
            params![key.0, seq],
        )
    }

    /// How many times subscription id fired since t: its firings done.
    pub fn count_firings(&self, id: &str, since: Time) -> Result<i64> {
        self.value(
            "SELECT COUNT(*) FROM firings WHERE subscription_id=? AND at>=? AND status='done'",
            params![id, ms(since)],
        )
    }

    /// The newest n near-misses of each subscription, by subscription id and the newest first: what the Subscriptions
    /// tab shows on each line.
    pub fn recent_near_misses(&self, n: i64) -> Result<Vec<NearMiss>> {
        self.list(
            "SELECT doc FROM (SELECT doc, subscription_id, seq, ROW_NUMBER() OVER (PARTITION BY subscription_id ORDER BY seq DESC) AS nth FROM near_misses)
WHERE nth<=? ORDER BY subscription_id, seq DESC",
            [n],
        )
    }

    /// The newest n near-misses of subscription id, the newest first.
    pub fn near_misses(&self, id: &str, n: i64) -> Result<Vec<NearMiss>> {
        self.list(
            "SELECT doc FROM near_misses WHERE subscription_id=? ORDER BY seq DESC LIMIT ?",
            params![id, n],
        )
    }
}
