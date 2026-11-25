//! The transcripts Claude Code writes of the sessions the runtime runs, served a part at a time.

use std::io::{self, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use regex::Regex;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use wire::runtime::TranscriptChunk;

/// The most of a transcript one `session_transcript` answer carries.
const PART: usize = 512 << 10;

/// What Claude Code turns into "-" when it names a project's transcript folder after its directory.
static NOT_PATH_CHAR: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new("[^a-zA-Z0-9]").ok());

/// Where Claude Code writes the transcripts of the sessions the runtime runs: `<the runtime's claude config
/// dir>/projects`. Every account's config dir links its projects/ there (`agent::ClaudeAccounts::prepare`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptDir(pub PathBuf);

impl TranscriptDir {
    /// Where Claude Code writes the transcript of session `session_id` started in `cwd`: in a folder named after `cwd`.
    pub fn path(&self, cwd: &str, session_id: &str) -> Result<PathBuf, String> {
        let one_name = Path::new(session_id).file_name().is_some_and(|name| name == session_id);
        if session_id.is_empty() || !one_name || session_id.contains("..") {
            return Err("no session to read the transcript of".to_owned());
        }
        let folder = NOT_PATH_CHAR
            .as_ref()
            .map(|re| re.replace_all(cwd, "-").into_owned())
            .ok_or("the pattern of a transcript's folder does not compile")?;
        Ok(self.0.join(folder).join(format!("{session_id}.jsonl")))
    }

    /// At most `PART` of the transcript from `offset`.
    pub(crate) async fn part(&self, cwd: &str, session_id: &str, offset: i64) -> Result<TranscriptChunk, String> {
        let path = self.path(cwd, session_id)?;
        let shown = path.display().to_string();
        let start = u64::try_from(offset).map_err(|_| "a transcript is read from an offset of 0 or more")?;
        let mut f = match tokio::fs::File::open(&path).await {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(TranscriptChunk {
                    path: shown,
                    data: String::new(),
                    next: offset,
                    eof: true,
                    missing: true,
                });
            }
            Err(e) => return Err(format!("open {shown}: {e}")),
        };
        f.seek(SeekFrom::Start(start))
            .await
            .map_err(|e| format!("read {shown}: {e}"))?;
        let mut buf = Vec::with_capacity(PART);
        let n = f
            .take(u64::try_from(PART).unwrap_or(u64::MAX))
            .read_to_end(&mut buf)
            .await
            .map_err(|e| format!("read {shown}: {e}"))?;
        let read = i64::try_from(n).map_err(|_| format!("read {shown}: a part past what an offset holds"))?;
        Ok(TranscriptChunk {
            path: shown,
            data: STANDARD.encode(&buf),
            next: offset.saturating_add(read),
            eof: n < PART,
            missing: false,
        })
    }
}
