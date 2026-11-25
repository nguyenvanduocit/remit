//! Remit desktop (ADR-0128): the owner's Remit in a native window, drawn with shadcn-rs over the client core the web runs.
//!
//! `remit-desktop [link]` comes in with the link the server printed (`http://host:port/#key=…`), else with the one kept
//! from before, else opens on the way in. `--trace <dir>` drives the window headless against a fake server and records
//! each frame; `--dark` uses the dark theme; `--reduce-motion` stills transitions and animations; `--hud` opens with the
//! frame HUD (F10 toggles it); `--debug` logs each frame and outlines its boxes.

mod agent;
mod app;
mod assistant;
mod board;
mod chain;
mod composer;
mod core;
mod fake;
mod login;
mod quic;
mod store;
mod trace;
mod way_in;
mod words;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gui::{DebugTools, RunOptions, Signal};
use replica::{Ask, Kept, Told};
use shadcn::{Theme, provide_theme};
use time::UtcOffset;

use crate::app::{App, ComeIn};
use crate::store::{Store, WeakStore};
use crate::way_in::Credential;

/// The heap counted, for the frame HUD and `--debug`'s frame reports.
#[global_allocator]
static HEAP: &gui::heap::Counting = gui::heap::COUNTING;

/// Where the window keeps the owner's sends the server has not taken, across a restart (`Told::Keep`).
fn outbox_at(home: &Path) -> PathBuf {
    home.join(".remit").join("desktop-outbox.json")
}

/// Starts the core for `credential` on the shell thread: kept for the next start, its sends from before restored.
fn come_in_live(app: &App, credential: Credential, home: &Path) -> Result<Store, String> {
    let kept_at = way_in::kept_at(home);
    way_in::keep(&kept_at, &credential).map_err(|e| format!("Could not keep the key at {}: {e}", kept_at.display()))?;
    // The mailbox takes what the core tells the window; it holds the store weakly, so coming in again lets the old go.
    let slot: Rc<RefCell<Option<WeakStore>>> = Rc::new(RefCell::new(None));
    let to = Rc::clone(&slot);
    let told = gui::mailbox(move |told: Vec<Told>| {
        if let Some(store) = to.borrow().as_ref().and_then(WeakStore::upgrade) {
            store.apply(told);
        }
    });
    let asks = quic::start(credential, core::options(core::salt()), told)
        .map_err(|e| format!("Could not start the connection: {e}"))?;
    let outbox = outbox_at(home);
    let toaster = app.toaster.clone();
    let store = Store::new(
        Rc::new(move |ask| {
            if asks.send(ask).is_err() {
                eprintln!("remit-desktop: the connection thread is gone");
            }
        }),
        Rc::new(move |kept: &Kept| keep_outbox(&outbox, kept)),
        Rc::new(move |notice| app::say(&toaster, notice)),
    );
    *slot.borrow_mut() = Some(store.downgrade());
    if let Some(kept) = std::fs::read(outbox_at(home))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Kept>(&bytes).ok())
    {
        store.ask(Ask::Restore { kept });
    }
    Ok(store)
}

fn keep_outbox(path: &Path, kept: &Kept) {
    let written = serde_json::to_vec(kept)
        .map_err(std::io::Error::other)
        .and_then(|bytes| way_in::write_private(path, &bytes));
    if let Err(e) = written {
        eprintln!("remit-desktop: keep the sends that wait at {}: {e}", path.display());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| args.iter().skip_while(|a| *a != name).nth(1);
    provide_theme(Signal::labeled(
        "theme",
        if flag("--dark") { Theme::dark() } else { Theme::light() },
    ));
    // Read before any thread starts: the platform's offset is read safely only then.
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);

    if let Some(dir) = value("--trace") {
        if let Err(e) = trace::run(Path::new(dir), offset) {
            eprintln!("trace failed: {e}");
            std::process::exit(1);
        }
        println!(
            "{}",
            std::fs::read_to_string(Path::new(dir).join("trace.txt")).unwrap_or_default()
        );
        return;
    }

    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let first = args
        .iter()
        .find_map(|a| way_in::read_link(a))
        .or_else(|| way_in::kept(&way_in::kept_at(&home)));
    let come_in: ComeIn = Rc::new(move |app: &App, credential| come_in_live(app, credential, &home));
    let app = App::new(offset, core::salt(), come_in);
    app.keep_time();
    if let Some(credential) = first {
        app.come_in(credential);
    }
    let options = RunOptions {
        debug: DebugTools {
            log: flag("--debug"),
            overlay: flag("--debug"),
            hud: flag("--hud"),
        },
        reduced_motion: flag("--reduce-motion"),
    };
    gui::run("Remit", app.view(), options);
}
