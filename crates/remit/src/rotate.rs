//! The agents' log: a file that never grows past `most` bytes. The write that would take it past moves it to
//! `<path>.1`, replacing the one before, and starts the file again. The agents' stderr runs as long as the server does,
//! so the disk holds at most twice `most`.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::PathBuf;

/// How large agents.log grows before it moves aside.
pub const AGENTS_LOG_SIZE: u64 = 64 << 20;

pub struct RotatingFile {
    path: PathBuf,
    most: u64,
    f: File,
    size: u64,
    /// The last write or move failed, and the log said so.
    failing: bool,
}

impl RotatingFile {
    /// Appends to path, the server's OS user's alone, from the size it already has.
    pub fn open(path: PathBuf, most: u64) -> io::Result<RotatingFile> {
        let (f, size) = open_append(&path)?;
        Ok(RotatingFile {
            path,
            most,
            f,
            size,
            failing: false,
        })
    }

    /// Moves the file to path.1 and opens path again; the old file stays in use until the new one is open.
    fn rotate(&mut self) -> io::Result<()> {
        let mut aside = self.path.clone().into_os_string();
        aside.push(".1");
        std::fs::rename(&self.path, aside)?;
        let (f, size) = open_append(&self.path)?;
        (self.f, self.size) = (f, size);
        Ok(())
    }
}

fn open_append(path: &PathBuf) -> io::Result<(File, u64)> {
    let f = OpenOptions::new().create(true).append(true).mode(0o600).open(path)?;
    let size = f.metadata()?.len();
    Ok((f, size))
}

impl Write for RotatingFile {
    /// Always takes all of buf: an agent's stderr is copied into it, and a write that failed would stop the copy and leave
    /// the agent blocked on a full pipe. A failure goes to the server's log once, until a write works again.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        let moved = if self.size > 0 && self.size.saturating_add(len) > self.most {
            self.rotate()
        } else {
            Ok(())
        };
        let wrote = self.f.write_all(buf);
        if wrote.is_ok() {
            self.size = self.size.saturating_add(len);
        }
        let failed = moved.err().or(wrote.err());
        if let Some(err) = &failed
            && !self.failing
        {
            tracing::error!(path = %self.path.display(), %err, "agents' log");
        }
        self.failing = failed.is_some();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.f.flush()
    }
}
