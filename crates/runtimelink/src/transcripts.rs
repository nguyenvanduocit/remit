//! The transcripts Claude Code writes, which live where the runtime runs it: a transcript comes over the link a part at
//! a time (`session_transcript`).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use wire::runtime::{TranscriptChunk, to_runtime};

use crate::Error;
use crate::launcher::Launcher;
use crate::port::Link;

pub struct Transcripts<L> {
    pub l: Launcher<L>,
}

impl<L: Link> Transcripts<L> {
    /// Opens the transcript of the conversation `session_id` run in `cwd`. `Error::NoTranscript` says none is written yet.
    pub async fn open(&self, cwd: &str, session_id: &str) -> Result<Transcript<L>, Error> {
        let mut t = Transcript {
            l: self.l.clone(),
            cwd: cwd.to_owned(),
            session_id: session_id.to_owned(),
            path: String::new(),
            buf: Vec::new(),
            next: 0,
            eof: false,
        };
        let first = t.part(0).await?;
        if first.missing {
            return Err(Error::NoTranscript(first.path));
        }
        t.path.clone_from(&first.path);
        t.take(first)?;
        Ok(t)
    }
}

/// A transcript the runtime serves, read a part at a time.
pub struct Transcript<L> {
    l: Launcher<L>,
    cwd: String,
    session_id: String,
    path: String,
    /// What was read and not yet given out.
    buf: Vec<u8>,
    /// The offset of the part to ask for next.
    next: i64,
    /// The runtime said the file ends at `next`.
    eof: bool,
}

impl<L: Link> Transcript<L> {
    /// Where the runtime keeps it.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The next bytes of the transcript; none once it has all been read.
    pub async fn next_part(&mut self) -> Result<Option<Vec<u8>>, Error> {
        while self.buf.is_empty() {
            if self.eof {
                return Ok(None);
            }
            let part = self.part(self.next).await?;
            if part.missing {
                return Err(self.broken("it was removed while it was read"));
            }
            self.take(part)?;
            if self.buf.is_empty() && !self.eof {
                return Err(self.broken("the AI runtime sent an empty part of a transcript that goes on"));
            }
        }
        Ok(Some(std::mem::take(&mut self.buf)))
    }

    async fn part(&self, offset: i64) -> Result<TranscriptChunk, Error> {
        let read = to_runtime::client::session_transcript(self.cwd.clone(), self.session_id.clone(), offset);
        Ok(self.l.on_runtime(read).await?)
    }

    fn take(&mut self, part: TranscriptChunk) -> Result<(), Error> {
        self.buf = STANDARD.decode(&part.data).map_err(|e| self.broken(&e.to_string()))?;
        self.next = part.next;
        self.eof = part.eof;
        Ok(())
    }

    fn broken(&self, why: &str) -> Error {
        Error::Transcript {
            path: self.path.clone(),
            why: why.to_owned(),
        }
    }
}
