//! The Seatbelt profiles of a jail and of a Wrap, and the real paths they are written with.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;

/// What a jail's profile lets a program do, every path real: Seatbelt matches the path a file has once every link in it
/// is followed (/tmp is /private/tmp), so a rule written with a link in it would never match.
pub(crate) struct Sandbox {
    /// This binary, which runs in the sandbox as the helper.
    pub helper: String,
    pub readable: Vec<String>,
    /// The work directory, then the other writable paths.
    pub writable: Vec<String>,
    /// The read-only paths inside a writable one, which stay read-only.
    pub kept: Vec<String>,
    pub hidden: Vec<String>,
    /// The socket and the port of its relay on the host's loopback.
    pub socket: Option<(String, u16)>,
}

/// Lets a process signal the members of its own sandbox and no other process. The helper's end of the sandbox signals
/// every process of the user and relies on it (sweep), so a refusal is not reported: it is the rule at work.
const SAME_SANDBOX_SIGNALS: &str = "(deny signal (with no-report))\n(allow signal (target same-sandbox))\n";

impl Sandbox {
    /// Denies everything but what bsd.sb, Apple's base for a command-line program, allows, and then allows: starting a
    /// program, signalling a process of the same jail, running the helper, reading the readable paths, writing the
    /// writable ones but the kept paths in them, and connecting to the socket and its relay. The hidden paths come after, as
    /// the last rule that matches wins, and then the directories above a kept or hidden path in a writable one.
    pub(crate) fn profile(&self) -> String {
        let mut b = String::from("(version 1)\n(deny default)\n(import \"bsd.sb\")\n");
        b.push_str("(allow process-fork)\n");
        b.push_str(SAME_SANDBOX_SIGNALS);
        b.push_str(&format!(
            "(allow process-exec file-read* (literal {}))\n",
            quote(&self.helper)
        ));
        b.push_str(&format!(
            "(allow process-exec file-read*{})\n",
            subpaths(&self.readable)
        ));
        b.push_str(&format!("(allow file-write*{})\n", subpaths(&self.writable)));
        if !self.kept.is_empty() {
            b.push_str(&format!("(deny file-write*{})\n", subpaths(&self.kept)));
        }
        if let Some((socket, port)) = &self.socket {
            b.push_str(&format!(
                "(allow network-outbound (remote unix-socket (path-literal {})) (remote ip \"localhost:{port}\"))\n",
                quote(socket)
            ));
        }
        if !self.hidden.is_empty() {
            b.push_str(&format!(
                "(deny process-exec file-read* file-write*{})\n",
                subpaths(&self.hidden)
            ));
        }
        // Outside the writable paths nothing can be moved.
        let movable: Vec<String> = self
            .kept
            .iter()
            .chain(
                self.hidden
                    .iter()
                    .filter(|p| self.writable.iter().any(|w| within(p, w))),
            )
            .cloned()
            .collect();
        b.push_str(&holders_stay(&movable));
        b
    }
}

/// The profile of a Wrap: everything the user may do is allowed, but reading, changing or connecting to a hidden path (a
/// socket is reached by connecting, which no file rule covers), changing a read-only one, moving a directory above
/// either, and signalling a process outside the sandbox. A rule on a directory holds for the directory itself and
/// everything under it.
pub(crate) fn hidden_profile(hide: &[String], read_only: &[String]) -> String {
    let mut b = String::from("(version 1)\n(allow default)\n");
    b.push_str(SAME_SANDBOX_SIGNALS);
    if !hide.is_empty() {
        b.push_str(&format!("(deny file-read* file-write*{})\n", subpaths(hide)));
        let mut sockets = String::new();
        for p in hide {
            write!(sockets, " (remote unix-socket (subpath {}))", quote(p)).ok();
        }
        b.push_str(&format!("(deny network-outbound{sockets})\n"));
    }
    if !read_only.is_empty() {
        b.push_str(&format!("(deny file-write*{})\n", subpaths(read_only)));
    }
    let protected: Vec<String> = hide.iter().chain(read_only).cloned().collect();
    b.push_str(&holders_stay(&protected));
    b
}

/// Refuses to rename or remove a directory that holds one of paths: Seatbelt matches a rule by path, so moving a
/// directory above a hidden or read-only path would carry it to a path no rule names. A rename is an unlink of the old
/// name. The rule comes last so that no allowance after it reopens the unlink.
fn holders_stay(paths: &[String]) -> String {
    let holders: BTreeSet<&str> = paths
        .iter()
        .flat_map(|p| Path::new(p).ancestors().skip(1))
        .filter_map(Path::to_str)
        .collect();
    if holders.is_empty() {
        return String::new();
    }
    let literals: String = holders.iter().map(|d| format!(" (literal {})", quote(d))).collect();
    format!("(deny file-write-unlink (require-all (vnode-type DIRECTORY) (require-any{literals})))\n")
}

/// The filter that matches each path and everything under it.
fn subpaths(paths: &[String]) -> String {
    paths.iter().map(|p| format!(" (subpath {})", quote(p))).collect()
}

/// p as a string of the profile's language: a backslash and a double quote are escaped, and nothing else needs it
/// (validation refuses a NUL).
pub(crate) fn quote(p: &str) -> String {
    format!("\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(crate) fn real_paths(paths: &[String]) -> Vec<String> {
    paths.iter().map(|p| real_path(p)).collect()
}

/// p with every link followed. A path that does not exist yet is its nearest existing parent's real path with the rest
/// of p after it, so that a rule on it holds once it is made.
pub(crate) fn real_path(p: &str) -> String {
    if let Ok(resolved) = std::fs::canonicalize(p) {
        return resolved.to_string_lossy().into_owned();
    }
    let path = Path::new(p);
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => Path::new(&real_path(&parent.to_string_lossy()))
            .join(name)
            .to_string_lossy()
            .into_owned(),
        _ => p.to_owned(),
    }
}

/// Says whether path is root or under it.
pub(crate) fn within(path: &str, root: &str) -> bool {
    path == root || root == "/" || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}
