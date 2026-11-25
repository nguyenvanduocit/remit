//! Finding a bare program name along the PATH of the program's own environment.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Finds a bare program name along the PATH of `env`, among the files `visible` says the jail shows. The helper's own
/// PATH is not consulted: it is the caller's.
pub(crate) fn look_path(name: &str, env: &[String], visible: impl Fn(&str) -> bool) -> Result<String, String> {
    if name.contains('/') {
        return Ok(name.to_owned());
    }
    let path = env
        .iter()
        .filter_map(|e| e.strip_prefix("PATH="))
        .next_back()
        .unwrap_or("");
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let candidate = Path::new(dir).join(name).to_string_lossy().into_owned();
        let runnable = std::fs::metadata(&candidate).is_ok_and(|m| !m.is_dir() && m.permissions().mode() & 0o111 != 0);
        if runnable && visible(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!("{name}: not found in the jail's PATH {path:?}"))
}
