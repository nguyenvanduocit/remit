//! rust-embed reads each file it embeds with `include_bytes!`, so cargo rebuilds for a changed file but not for a new one.
//! Watching the directory (cargo scans it whole) rebuilds the crate when a skill gains or loses a file.
fn main() {
    println!("cargo::rerun-if-changed=skills");
}
