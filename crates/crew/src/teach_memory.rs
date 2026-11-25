//! What the memory may learn from each teaching (design D3, ADR-0117), the firing of the job teach on its fact: an accepted
//! card's session whole (accepted), a Chair's when the goal judge approves its chain; a sent-back card's, or one a mentor
//! steered, only what was said to it (teaching), once the session has read it; what the owner changed after the agent's
//! last push, from the diff; the coach's retrospective of a finished chain, to its Chair; and an overturn takes back what
//! the Assistant's overturned turn taught and teaches the owner's word instead; a reply a person marked (helpful, not
//! helpful, corrected) teaches the reply with the mark and their words. A teaching that names its mentor (a steer, an
//! overturn, a marked reply) also teaches the mentor's own memory their words: what they teach is their memory. Each is
//! recorded as a `Teaching` the librarian reads (librarian.rs); these firings are the only way one is recorded, and no
//! agent but the librarian has a tool that writes a memory.
//! Pure: `reading_teaching`, `overturn_text`, `retrospective_text`, `reply_text`, `teaching_id`.

use model::{Actor, ActorKind, Card, CardStatus, Decision, LearnRole, ReportVerdict, Retrospective, Teaching};
use sha2::{Digest, Sha256};
use store::MemoryReader;
use wire::app::TurnMark;

use crate::Crew;
use chains::fact::{FactValue, fact_key, fact_value_of};
use chains::schedule::at_work;
use chains::{Error, Result};
use pipeline::processing::Delivery;

/// How much of a marked reply its teaching quotes.
const REPLY_LIMIT: usize = 8_000;

/// The id of the teaching of holder from source: the same teaching recorded again is the one already recorded. Pure.
fn teaching_id(holder: &str, source: &str) -> String {
    let sum = Sha256::digest(format!("{holder}\n{source}").as_bytes());
    format!("tch_{}", hex::encode(sum.get(..6).unwrap_or_default()))
}

/// Whether card c's session has yet to read what it was sent or finish the turn that reads it: what it is taught is in
/// its transcript once it has not. Pure.
fn reading_teaching(c: &Card) -> bool {
    at_work(c) || c.status() == &CardStatus::Idle && c.queued > 0
}

/// What the owner's overturn ov of the Assistant's decision d teaches, in words. Pure.
fn overturn_text(d: &Decision, ov: &Decision) -> String {
    format!(
        "The Assistant decided for the owner: {}, because {}\nThe owner overturned it: {}",
        d.text.strip_prefix("Decided for you: ").unwrap_or(&d.text),
        d.why,
        ov.why
    )
}

/// What the coach's retrospective r of the chain titled title teaches its Chair, in words. Pure.
fn retrospective_text(r: &Retrospective, title: &str) -> String {
    let mut t = format!("The coach's retrospective of the chain you sat in the Chair seat of, {title}:\n");
    if r.issues.is_empty() {
        t.push_str("Nothing went wrong.\n");
    } else {
        t.push_str("What went wrong:\n");
        for i in &r.issues {
            t.push_str(&format!("- ({} impact) {}\n", i.impact, i.text));
        }
    }
    t.push_str(&format!("The brief that would have served better:\n{}", r.brief));
    t
}

/// What teacher's mark (`teach.reply`) on the reply reply teaches, saying words. Pure.
fn reply_text(mark: TurnMark, teacher: &str, reply: &str, words: &str) -> String {
    let reply = cut(reply.trim(), REPLY_LIMIT);
    let found = match mark {
        TurnMark::Correct => {
            return format!(
                "{teacher} corrected this reply of yours.\nYour reply:\n{reply}\nThe correct reply:\n{words}"
            );
        }
        TurnMark::Helpful => "helpful",
        TurnMark::NotHelpful => "not helpful",
    };
    if words.is_empty() {
        format!("{teacher} found this reply of yours {found}.\nYour reply:\n{reply}")
    } else {
        format!("{teacher} found this reply of yours {found}, because: {words}\nYour reply:\n{reply}")
    }
}

/// s cut to at most limit bytes, on a character's edge.
fn cut(s: &str, limit: usize) -> &str {
    let mut end = limit.min(s.len());
    while !s.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    s.get(..end).unwrap_or_default()
}

impl Crew {
    /// The effect of the run of teach on a teaching fact: what it teaches is recorded for the librarian, then the firing
    /// is stored. A card whose session has not read its teaching yet holds the firing (`Error::NotAdmitted`), and it is done
    /// on a later pass; done again after a crash, the same teaching is recorded once (`teaching_id`).
    pub async fn fire_teach(&self, d: &Delivery) -> Result<()> {
        let Some(r) = d.readings.first() else {
            return self
                .pipeline
                .done_saying(d, "the firing holds no teaching, so nothing was learned")
                .await;
        };
        let v = match fact_value_of(r) {
            Ok(v) if !v.card.is_empty() => v,
            _ => {
                return self
                    .pipeline
                    .done_saying(d, "the fact names no card taught, so nothing was learned")
                    .await;
            }
        };
        let id = v.card.clone();
        let c = match self.store.read(move |r| r.card(&id)).await {
            Ok(c) => c,
            Err(store::Error::NotFound) => {
                return self
                    .pipeline
                    .done_saying(d, &format!("card {} is gone, so nothing was learned", v.card))
                    .await;
            }
            Err(err) => return Err(err.into()),
        };
        let teacher = self.teacher_name(&r.cause.actor).await;
        let did = match self.reader_of(&c).await? {
            None => format!("card {} is no employee's, so it learned nothing", c.id),
            Some(rd) if r.key == fact_key("teach", "verdict") => {
                self.teach_verdict(&c, &rd, v.verdict, &teacher).await?
            }
            Some(rd) if r.key == fact_key("teach", "steer") => self.teach_steer(&c, &rd, &teacher, v.event).await?,
            Some(rd) if r.key == fact_key("teach", "overturn") => self.teach_overturn(&c, &rd, &v.decision).await?,
            Some(rd) if r.key == fact_key("teach", "retrospective") => {
                self.teach_retrospective(&c, &rd, &v, &teacher).await?
            }
            Some(rd) if r.key == fact_key("teach", "reply") => self.teach_reply(&c, &rd, &v, &teacher).await?,
            Some(_) => format!("{} is no teaching the memory learns from", r.key),
        };
        let did = if v.mentor.is_empty() {
            did
        } else {
            format!("{did}; {}", self.teach_mentor(&c, &r.key, &v, &teacher).await?)
        };
        self.pipeline.done_saying(d, &did).await
    }

    /// Records, for the librarian, what of card c's session it may read for the holder rd: as role, from event from on to
    /// the last the session holds now (what it taught; a later turn is a teaching of its own), taught by teacher, named
    /// source and said as label; and wakes the librarian. Recorded once per holder and source.
    #[expect(
        clippy::too_many_arguments,
        reason = "one teaching's fields, each named at the call site"
    )]
    async fn teach_session(
        &self,
        c: &Card,
        rd: &MemoryReader,
        role: LearnRole,
        from: i64,
        teacher: &str,
        source: &str,
        label: &str,
    ) -> Result<String> {
        let card = c.id.clone();
        let to = self
            .store
            .read(move |r| r.card_events(&card, 1))
            .await?
            .last()
            .map_or(0, |e| e.id);
        let t = Teaching {
            id: teaching_id(&rd.holder, source),
            holder: rd.holder.clone(),
            template: rd.template.clone(),
            project: rd.project.clone(),
            chain: c.home.clone(),
            card: c.id.clone(),
            role,
            from_event: from,
            to_event: to,
            text: String::new(),
            teacher: teacher.to_owned(),
            source: source.to_owned(),
            source_label: label.to_owned(),
            created_at: self.now(),
            ..Teaching::default()
        };
        self.record_teaching(t).await?;
        Ok(format!("recorded card {}'s session for {} as {role}", c.id, rd.holder))
    }

    /// Records, for the librarian, text that teacher taught the holder rd outside any session, named source and said as
    /// label, about card c.
    async fn teach_text(
        &self,
        c: &Card,
        rd: &MemoryReader,
        teacher: &str,
        text: &str,
        source: &str,
        label: &str,
    ) -> Result<String> {
        let t = Teaching {
            id: teaching_id(&rd.holder, source),
            holder: rd.holder.clone(),
            template: rd.template.clone(),
            project: rd.project.clone(),
            chain: c.home.clone(),
            card: c.id.clone(),
            role: LearnRole::Teaching,
            text: text.to_owned(),
            teacher: teacher.to_owned(),
            source: source.to_owned(),
            source_label: label.to_owned(),
            created_at: self.now(),
            ..Teaching::default()
        };
        self.record_teaching(t).await?;
        Ok(format!("recorded what {teacher} taught {} ({source})", rd.holder))
    }

    /// Teaches mentor v.mentor's own memory what they taught in teaching key about card c: a steer's message, an
    /// overturn's word, a reply's mark.
    async fn teach_mentor(&self, c: &Card, key: &model::SignalKey, v: &FactValue, teacher: &str) -> Result<String> {
        let text = if *key == fact_key("teach", "steer") {
            let id = v.event;
            match self.store.read(move |r| r.event(id)).await {
                Ok(e) => e.text,
                Err(store::Error::NotFound) => {
                    return Ok(format!("message {} is gone, so {teacher} learned nothing", v.event));
                }
                Err(err) => return Err(err.into()),
            }
        } else if *key == fact_key("teach", "overturn") {
            let id = v.decision.clone();
            let (d, ov) = self
                .store
                .read(move |r| -> store::Result<_> {
                    let ov = r.decision(&id)?;
                    Ok((r.decision(&ov.overturns)?, ov))
                })
                .await?;
            overturn_text(&d, &ov)
        } else if *key == fact_key("teach", "reply") {
            v.why.clone() // why they marked it, or the correct reply
        } else {
            return Ok(format!("{key} teaches no mentor"));
        };
        if text.trim().is_empty() {
            return Ok(format!("{teacher} said nothing to learn from"));
        }
        let (source, label) = if v.event == 0 {
            (
                format!("decision:{}", v.decision),
                format!("{teacher} overturned decision {}", v.decision),
            )
        } else {
            (
                format!("card:{} event:{}", v.card, v.event),
                format!("what {teacher} told {}", c.name),
            )
        };
        let rd = MemoryReader {
            holder: v.mentor.clone(),
            template: String::new(),
            project: String::new(),
        };
        self.teach_text(c, &rd, teacher, &text, &source, &label).await
    }

    /// Teaches the holder rd teacher's mark on reply v.event of card c: the reply quoted, with the mark and its words.
    async fn teach_reply(&self, c: &Card, rd: &MemoryReader, v: &FactValue, teacher: &str) -> Result<String> {
        let id = v.event;
        let e = match self.store.read(move |r| r.event(id)).await {
            Ok(e) => e,
            Err(store::Error::NotFound) => return Ok(format!("reply {} is gone, so nothing was learned", v.event)),
            Err(err) => return Err(err.into()),
        };
        let Some(mark) = v.mark else {
            return Ok(format!(
                "the teaching on reply {} names no mark, so nothing was learned",
                v.event
            ));
        };
        let text = reply_text(mark, teacher, &e.text, &v.why);
        self.teach_text(
            c,
            rd,
            teacher,
            &text,
            &format!("card:{} event:{}", v.card, v.event),
            &format!("{teacher} marked a reply of {}", c.name),
        )
        .await
    }

    /// Teaches the holder rd (the Chair's) the coach's retrospective v.retrospective of chain v.chain, as teacher said it.
    async fn teach_retrospective(&self, c: &Card, rd: &MemoryReader, v: &FactValue, teacher: &str) -> Result<String> {
        let (chain, id) = (v.chain.clone(), v.retrospective.clone());
        let found = self
            .store
            .read(move |r| -> store::Result<_> {
                let title = r.chain(&chain)?.title;
                Ok(r.retrospectives_of(&chain)?
                    .into_iter()
                    .find(|x| x.id == id)
                    .map(|x| (x, title)))
            })
            .await;
        let (retro, title) = match found {
            Ok(Some(found)) => found,
            Ok(None) | Err(store::Error::NotFound) => {
                return Ok(format!(
                    "retrospective {} is gone, so nothing was learned",
                    v.retrospective
                ));
            }
            Err(err) => return Err(err.into()),
        };
        self.teach_text(
            c,
            rd,
            teacher,
            &retrospective_text(&retro, &title),
            &format!("retrospective:{}", retro.id),
            &format!("the coach's retrospective of {title}"),
        )
        .await
    }

    /// Records card c's session for the holder rd, as role, from event from on, once the session has read what it was
    /// taught.
    #[expect(
        clippy::too_many_arguments,
        reason = "one teaching's fields, each named at the call site"
    )]
    async fn admit(
        &self,
        c: &Card,
        rd: &MemoryReader,
        role: LearnRole,
        from: i64,
        teacher: &str,
        source: &str,
        label: &str,
    ) -> Result<String> {
        if reading_teaching(c) {
            return Err(Error::NotAdmitted(format!(
                "card {}'s session has not read what it was taught yet",
                c.id
            )));
        }
        self.teach_session(c, rd, role, from, teacher, source, label).await
    }

    /// Teaches from the verdict on card c, given by teacher: all of an accepted card's session, and of a card sent back
    /// only what was said to it.
    async fn teach_verdict(
        &self,
        c: &Card,
        rd: &MemoryReader,
        verdict: Option<ReportVerdict>,
        teacher: &str,
    ) -> Result<String> {
        if verdict == Some(ReportVerdict::Accept) {
            return self
                .admit(
                    c,
                    rd,
                    LearnRole::Accepted,
                    0,
                    "",
                    &format!("card:{} accepted", c.id),
                    &format!("an accepted card: {}", c.name),
                )
                .await;
        }
        self.admit(
            c,
            rd,
            LearnRole::Teaching,
            0,
            teacher,
            &format!("card:{} sent back", c.id),
            &format!("a card {teacher} sent back: {}", c.name),
        )
        .await
    }

    /// Teaches from card c's session what its mentor, teacher, said in message event and after.
    async fn teach_steer(&self, c: &Card, rd: &MemoryReader, teacher: &str, event: i64) -> Result<String> {
        let source = format!("card:{} steered by {teacher} (event {event})", c.id);
        let label = format!("{teacher} steered {}", c.name);
        self.admit(c, rd, LearnRole::Teaching, event, teacher, &source, &label)
            .await
    }

    /// The name of the one who taught, by, as the memory says who taught it: a mentor by their name ("a mentor" when it is
    /// not on record), the owner, an agent by its card's name (a Chair, a judge, the coach).
    async fn teacher_name(&self, by: &Actor) -> String {
        let who = by.who.clone();
        match by.kind {
            ActorKind::Owner => "the owner".to_owned(),
            ActorKind::Mentor => self
                .store
                .read(move |r| -> store::Result<String> { Ok(r.mentor(&r.help(&who)?.mentor)?.label) })
                .await
                .ok()
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| "a mentor".to_owned()),
            ActorKind::Agent => self
                .store
                .read(move |r| r.card(&who))
                .await
                .ok()
                .map(|c| c.name)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "an agent".to_owned()),
            _ => "Remit".to_owned(),
        }
    }

    /// Takes back what the Assistant's card c learned from the turn of the decision the owner's overturn ov_id
    /// overturns, and teaches the holder rd his word, citing both.
    async fn teach_overturn(&self, c: &Card, rd: &MemoryReader, ov_id: &str) -> Result<String> {
        let id = ov_id.to_owned();
        let (ov, d) = self
            .store
            .read(move |r| -> store::Result<_> {
                let ov = r.decision(&id)?;
                let d = r.decision(&ov.overturns)?;
                Ok((ov, d))
            })
            .await?;
        let retracted = self
            .retract_turn(
                c,
                &rd.holder,
                &d.item,
                &format!("the owner overturned decision {}", d.id),
            )
            .await?;
        let taught = self
            .teach_text(
                c,
                rd,
                "the owner",
                &overturn_text(&d, &ov),
                &format!("decision:{} overturned by decision:{}", d.id, ov.id),
                &format!("the owner overturned decision {}", d.id),
            )
            .await?;
        Ok(format!("retracted {retracted} memories its turn taught; {taught}"))
    }
}
