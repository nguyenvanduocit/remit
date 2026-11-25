//! `tsgen <src dir> <module> <out dir> [<src dir>...]`: writes `types.generated.ts` for one module of the crate whose
//! `src` is the first directory, and `api.generated.ts` when the module has a contract, into `out dir`. The directories
//! after are the `src` of the crates its types reach. A crate is named as its directory is, `-` written `_`
//! (`crates/client-web/src` is `client_web`). `make gen` runs it for `app` of `wire`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, module, out, reached @ ..] = args.as_slice() else {
        eprintln!("usage: tsgen <src dir> <module> <out dir> [<src dir of a crate it reaches>...]");
        return ExitCode::from(2);
    };
    match run(Path::new(src), module, Path::new(out), reached) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tsgen: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(src: &Path, module: &str, out: &Path, reached: &[String]) -> Result<(), String> {
    let krate = read(src)?;
    let reached = reached
        .iter()
        .map(|dir| read(Path::new(dir)))
        .collect::<Result<Vec<_>, _>>()?;
    let generated = tsgen::generate(&krate, &reached, module).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let files = [
        ("types.generated.ts", Some(&generated.types)),
        ("api.generated.ts", generated.api.as_ref()),
    ];
    for (name, text) in files {
        if let Some(text) = text {
            let path = out.join(name);
            std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// The crate whose `src` is `src`: its name and its `.rs` files.
fn read(src: &Path) -> Result<tsgen::Source, String> {
    let full = std::fs::canonicalize(src).map_err(|e| format!("read {}: {e}", src.display()))?;
    let name = full
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{} is not the src of a crate's directory", src.display()))?
        .replace('-', "_");
    let mut files = BTreeMap::new();
    let entries = std::fs::read_dir(&full).map_err(|e| format!("read {}: {e}", src.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("read {}: {e}", src.display()))?.path();
        let Some(file) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            files.insert(file.to_owned(), text);
        }
    }
    Ok(tsgen::Source { name, files })
}
