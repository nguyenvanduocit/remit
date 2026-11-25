//! A landing as a chain's screen reads it (ADR-0114, ADR-0116): the record with the release that carried it, as that
//! release is stored now (`wire::app::LandingRead`), read when it is shown or pushed, never stored with the landing.

use model::Landing;
use store::Reader;
use wire::app::LandingRead;

/// Landing l with the release that carried it, as r holds that release.
pub fn landing_read(r: &Reader<'_>, l: Landing) -> store::Result<LandingRead> {
    let release = l.release.as_deref().map(|id| r.release(id)).transpose()?;
    Ok(LandingRead {
        taking_back: l.taking_back().map(str::to_owned),
        landing: l,
        release,
    })
}

/// The landings of chain chain_id, the oldest first, each with the release that carried it.
pub fn chain_landings(r: &Reader<'_>, chain_id: &str) -> store::Result<Vec<LandingRead>> {
    r.chain_landings(chain_id)?
        .into_iter()
        .map(|l| landing_read(r, l))
        .collect()
}
