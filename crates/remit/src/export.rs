//! `remit export` writes everything the server holds of the owner's work to a new directory, one JSON Lines file per
//! table (`store::export`): his to take, in a form any program reads. It reads the database as one moment, beside a
//! server that is running, and writes nothing to it. What is sealed under the server's key is not in it.

use std::fmt::Write as _;
use std::path::Path;

use crate::dash_flags::{self, Flag, Kind};

/// `remit export` with args, the words after export; home gives the default data directory. Returns what it wrote.
pub fn run(args: &[String], home: &Path) -> Result<String, String> {
    let defs = vec![
        Flag {
            name: "data",
            kind: Kind::Str,
            default: home.join(".remit").display().to_string(),
            usage: "the server's data directory, whose remit.db is read",
        },
        Flag {
            name: "out",
            kind: Kind::Str,
            default: String::new(),
            usage: "the directory to write: it must not exist, or be empty",
        },
    ];
    let p = dash_flags::parse(&defs, args).map_err(|e| match e {
        dash_flags::Error::Help => format!(
            "usage: remit export -out <directory> [-data <data directory>]\n{}",
            dash_flags::usage(&defs)
        ),
        dash_flags::Error::Bad(why) => why,
    })?;
    let out = p.str("out");
    if out.is_empty() {
        return Err("-out names the directory to write".to_owned());
    }
    if std::fs::read_dir(&out).is_ok_and(|mut d| d.next().is_some()) {
        return Err(format!(
            "{out} holds files already: an export is written to a directory of its own"
        ));
    }
    let db = Path::new(&p.str("data")).join("remit.db");
    std::fs::metadata(&db).map_err(|e| format!("no database at {}: {e}", db.display()))?;
    let got = store::export(&db, Path::new(&out)).map_err(|e| e.to_string())?;
    let mut said = format!(
        "exported {} tables of schema version {} to {out}\n",
        got.tables.len(),
        got.version
    );
    for (t, n) in &got.tables {
        writeln!(said, "  {t:<20} {n}").ok();
    }
    said.push_str("sealed values (credentials, keys) are not in it: their key stays with the server");
    Ok(said)
}
