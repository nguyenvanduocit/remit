//! How the helper tells the parent that the program runs, or why it could not: a unix socket in a directory only this
//! user can enter. The helper connects once, writes nothing and closes once the program runs, or writes why it could not.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::Error;

/// How often the parent looks again while the helper has neither connected nor exited.
const POLL: Duration = Duration::from_millis(2);

/// The parent's end: the socket the helper reports on.
pub(crate) struct Status {
    dir: PathBuf,
    path: String,
    listener: UnixListener,
}

impl Status {
    pub(crate) fn new() -> Result<Self, Error> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("remit-jail-{}-{n}", std::process::id()));
            match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
                Ok(()) => break dir,
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Error::new(format!("status directory: {e}"))),
            }
        };
        let path = dir.join("s");
        let listener = UnixListener::bind(&path)
            .and_then(|l| l.set_nonblocking(true).map(|()| l))
            .map_err(|e| Error::new(format!("status socket: {e}")))?;
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            dir,
            listener,
        })
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    /// Returns once the program runs, or with why the helper could not run it. `exited` says whether the helper has ended
    /// without connecting, which no later connection can follow.
    pub(crate) fn wait(&self, mut exited: impl FnMut() -> Option<String>) -> Result<(), Error> {
        let mut conn = loop {
            match self.listener.accept() {
                Ok((conn, _)) => break conn,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    if let Some(how) = exited() {
                        return Err(Error::new(format!("the helper ended ({how}) before it reported")));
                    }
                    std::thread::sleep(POLL);
                }
                Err(e) => return Err(Error::new(format!("status: {e}"))),
            }
        };
        conn.set_nonblocking(false)
            .map_err(|e| Error::new(format!("status: {e}")))?;
        let mut msg = String::new();
        conn.read_to_string(&mut msg)
            .map_err(|e| Error::new(format!("status: {e}")))?;
        if msg.is_empty() { Ok(()) } else { Err(Error(msg)) }
    }
}

impl Drop for Status {
    fn drop(&mut self) {
        // A directory left behind holds nothing but a dead socket: there is nothing more to do on failure.
        std::fs::remove_file(&self.path).ok();
        std::fs::remove_dir(&self.dir).ok();
    }
}

/// The helper's end. It is opened by the standard library close-on-exec, so the program never holds it.
pub(crate) struct Report(UnixStream);

impl Report {
    pub(crate) fn connect(path: &str) -> std::io::Result<Self> {
        UnixStream::connect(path).map(Self)
    }

    /// Tells the parent the program runs.
    pub(crate) fn running(self) {}

    /// Tells the parent why the program did not run. A socket that cannot be written leaves the parent with a closed
    /// status and the helper's exit: nothing more to do.
    pub(crate) fn failed(mut self, why: &str) {
        self.0.write_all(why.as_bytes()).ok();
    }
}
