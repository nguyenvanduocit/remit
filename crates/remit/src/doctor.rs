//! `remit doctor [-addr <address>] [-data <dir>]`: read-only checks of a running server, from the machine it runs on,
//! written as one JSON object with a row per check, `{name, ok, detail, fix}`; exit status 1 when one fails. It asks the
//! server what a supervisor and a browser ask (`probe`) and changes nothing:
//! - `health`: `/healthz` answers 200;
//! - `build`: the server runs the commit of this binary (the release `releases/current` names, when this is it);
//! - `certificate`: `/link.json` names the hash of the link's certificate, and that certificate is not about to expire
//!   (it is made at each start, valid 13 days: a server up longer shuts browsers out);
//! - `link`: a hello on the link over UDP is welcomed with the owner key.
//!
//! The verdicts are pure (`*_row`); `run` reads.

use std::path::Path;

use model::Time;
use remit_client::dial::{Down, LinkJson};
use serde::Serialize;

use crate::dash_flags;
use crate::health::{Build, this_build};
use crate::probe::{self, Healthz};

/// A certificate with less time left than this fails the check: the server should restart before it expires.
const CERTIFICATE_MARGIN: time::Duration = time::Duration::days(1);

/// One check: what it looked at, whether it holds, what it saw (no credential, no path to one) and how to fix it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fix: String,
}

fn row(name: &'static str, ok: bool, detail: String, fix: &str) -> Check {
    Check {
        name,
        ok,
        detail,
        fix: if ok { String::new() } else { fix.to_owned() },
    }
}

/// `/healthz`: healthy, unhealthy with its reason, or no answer.
pub fn health_row(h: &Result<Healthz, String>) -> Check {
    let fix = "read the server's log; a server that does not answer is not running at -addr";
    match h {
        Ok(h) if h.ok() => row("health", true, format!("{} ok", h.status), fix),
        Ok(h) => row("health", false, format!("{}: {}", h.status, h.reason()), fix),
        Err(why) => row("health", false, why.clone(), fix),
    }
}

/// Whether the server runs the commit this binary was built from.
pub fn build_row(h: &Result<Healthz, String>, this: &Build) -> Check {
    let fix = "restart the server on this release (remit update), or run the doctor of the release it runs";
    let Ok(h) = h else {
        return row(
            "build",
            false,
            "the server did not say which commit it runs".to_owned(),
            fix,
        );
    };
    let modified = |m: bool| if m { " (modified)" } else { "" };
    let detail = format!(
        "the server runs {}{}, this binary is {}{}",
        h.revision(),
        modified(h.modified()),
        this.revision,
        modified(this.modified)
    );
    row("build", h.revision() == this.revision, detail, fix)
}

/// Whether `/link.json` names a certificate hash, and how long the certificate has left at now.
pub fn certificate_row(link: &Result<LinkJson, String>, now: Time) -> Check {
    let fix = "restart the server: it makes a new certificate at each start";
    let link = match link {
        Ok(l) => l,
        Err(why) => return row("certificate", false, why.clone(), fix),
    };
    let hashes_ok = !link.certificate_hashes.is_empty()
        && link
            .certificate_hashes
            .iter()
            .all(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    if !hashes_ok {
        return row(
            "certificate",
            false,
            format!("/link.json names no SHA-256 hash: {:?}", link.certificate_hashes),
            fix,
        );
    }
    let text = link.certificate_expires.as_deref().unwrap_or_default();
    let expires = time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339).ok();
    let Some(expires) = expires else {
        return row(
            "certificate",
            false,
            "/link.json does not say when the certificate expires".to_owned(),
            fix,
        );
    };
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the difference of two dates, far inside OffsetDateTime's range"
    )]
    let left = expires - now.0;
    let detail = format!("expires {text}, in {}h", left.whole_hours());
    row("certificate", left > CERTIFICATE_MARGIN, detail, fix)
}

/// The hello on the link: welcomed with the owner key; refused when no key could be read still shows UDP gets through.
pub fn link_row(hello: &Result<(), Down>, keyed: bool) -> Check {
    let fix = "the link is WebTransport over UDP at -addr's port: let UDP to it through, and present the owner key in <data>/owner.key";
    match hello {
        Ok(()) => row("link", true, "welcomed with the owner key".to_owned(), fix),
        Err(Down::Refused(r)) if !keyed => row(
            "link",
            true,
            format!("reachable; refused without the owner key ({:?})", r.code),
            fix,
        ),
        Err(Down::Refused(r)) => row(
            "link",
            false,
            format!("refused the owner key ({:?}): {}", r.code, r.reason),
            fix,
        ),
        Err(Down::Unreachable(why)) => row("link", false, why.clone(), fix),
    }
}

/// The whole report.
#[derive(Debug, Serialize)]
struct Report {
    ok: bool,
    checks: Vec<Check>,
}

/// Runs every check of the server `args` name and prints the report; an error when a check failed.
pub async fn run(args: &[String], home: &Path) -> Result<(), String> {
    let defs = crate::flags::subcommand_defs(home, &["addr", "data"], Vec::new());
    let parsed = match dash_flags::parse(&defs, args) {
        Ok(p) => p,
        Err(dash_flags::Error::Help) => {
            eprintln!("Usage of remit doctor:\n{}", dash_flags::usage(&defs));
            return Ok(());
        }
        Err(dash_flags::Error::Bad(why)) => return Err(why),
    };
    let (addr, data) = (parsed.str("addr"), parsed.str("data"));
    let health = probe::healthz(&addr).await;
    let link = probe::link_json(&addr).await;
    let key = probe::owner_key(Path::new(&data));
    let hello = match &link {
        Ok(l) => probe::hello(l, key.as_deref().unwrap_or_default()).await,
        Err(why) => Err(Down::Unreachable(why.clone())),
    };
    let checks = vec![
        health_row(&health),
        build_row(&health, &this_build()),
        certificate_row(&link, Time(time::OffsetDateTime::now_utc())),
        link_row(&hello, key.is_some()),
    ];
    let report = Report {
        ok: checks.iter().all(|c| c.ok),
        checks,
    };
    println!("{}", serde_json::to_string(&report).map_err(|e| e.to_string())?);
    if report.ok {
        Ok(())
    } else {
        Err("a check failed: its row says what to do".to_owned())
    }
}
