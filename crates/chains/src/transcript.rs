//! The transcripts Claude Code writes of the agents' sessions: the JSONL file of each, one record per line, read whole
//! where the runtime runs it.

use std::io;

use async_trait::async_trait;

/// The transcripts Claude Code writes, wherever it runs.
#[async_trait]
pub trait Transcripts: Send + Sync {
    /// Opens the transcript Claude Code wrote of session `session_id` started in `cwd`, and says which file that is (the
    /// path is given with the error too, when it is known). An error of kind `NotFound` when none is written yet.
    async fn open(&self, cwd: &str, session_id: &str) -> (String, io::Result<Box<dyn io::Read + Send>>);
}
