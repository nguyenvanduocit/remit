//! The agents' Claude Code config dir: Remit's own, under its data, holding nothing of the machine's user's config dir,
//! whose plugins, skills and settings an agent that ran there would load.

use std::path::Path;

/// On macOS Claude Code keeps a login in the Keychain under its config dir's own name, which no file links, so the
/// default account is signed in for home itself (`CLAUDE_CONFIG_DIR=<home> claude auth login`). Without it the default
/// account is one that is signed out: its probe fails and the agents run on the other accounts (failover), as for any
/// account out of use, and the Health bar names it.
#[cfg(target_os = "macos")]
pub fn prepare(home: &Path, _login: &Path) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| format!("agents' claude config dir: {e}"))?;
    restrict(home)
}

/// On Linux Claude Code keeps the login in a file, which is linked from login's config dir when the machine has one. The
/// link follows a token the machine refreshes; a file Claude Code wrote into home itself (it replaces the link when it
/// refreshes) is the freshest there is, and stays. With no login on the machine the default account is one that is
/// signed out, and the agents run on the other accounts (failover).
#[cfg(target_os = "linux")]
pub fn prepare(home: &Path, login: &Path) -> Result<(), String> {
    const CREDENTIALS: &str = ".credentials.json";
    std::fs::create_dir_all(home).map_err(|e| format!("agents' claude config dir: {e}"))?;
    restrict(home)?;
    let cwd = std::env::current_dir().map_err(|e| format!("agents' claude login: {e}"))?;
    let source = cwd.join(login).join(CREDENTIALS);
    if !source.exists() {
        return Ok(());
    }
    let link = home.join(CREDENTIALS);
    match std::fs::symlink_metadata(&link) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("agents' claude login: {e}")),
        Ok(info) if !info.file_type().is_symlink() => return Ok(()),
        Ok(_) => {
            if std::fs::read_link(&link).is_ok_and(|to| to == source) {
                return Ok(());
            }
            std::fs::remove_file(&link).map_err(|e| format!("agents' claude login: {e}"))?;
        }
    }
    std::os::unix::fs::symlink(&source, &link).map_err(|e| format!("agents' claude login: {e}"))
}

/// The config dir is the server's OS user's alone (mode 0700).
fn restrict(home: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    let meta = std::fs::metadata(home).map_err(|e| format!("agents' claude config dir: {e}"))?;
    if meta.permissions().mode() & 0o077 == 0 {
        return Ok(());
    }
    std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("agents' claude config dir: {e}"))
}
