//! Archived chains, a page at a time with where each stands: the snapshot leaves them out, since there is no end to how
//! many the owner puts away.

use chains::Result;
use model::{Chain, Time};
use wire::app::{ChainPage, ChainRead};

use crate::orch::Orchestrator;

/// How many archived chains one read gives.
const ARCHIVED_SHOWN: usize = 50;

/// The n archived chains of chains that come after the cursor after, newest change first. The cursor is the place of the
/// last chain shown (when it changed, and its id, which breaks a tie), so a chain archived between two reads moves no
/// other one onto a page shown already or off the next. A cursor that is not one starts from the newest.
fn archived_page(chains: Vec<Chain>, after: &str, n: usize) -> ChainPage {
    let mut archived: Vec<Chain> = chains.into_iter().filter(|c| c.archived_at.is_some()).collect();
    archived.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| b.id.cmp(&a.id)));
    if let Some((at, id)) = cursor_of(after) {
        archived.retain(|c| c.updated_at < at || c.updated_at == at && c.id < id);
    }
    let more = archived.len() > n;
    archived.truncate(n);
    let next = match archived.last() {
        Some(last) if more => format!("{} {}", last.updated_at.0.unix_timestamp_nanos(), last.id),
        _ => String::new(),
    };
    ChainPage {
        chains: archived
            .into_iter()
            .map(|chain| ChainRead {
                chain,
                check_back: None,
            })
            .collect(),
        standings: Vec::new(),
        next,
    }
}

/// Reads a page's `next`: when its last chain changed, and its id.
fn cursor_of(after: &str) -> Option<(Time, String)> {
    let (nanos, id) = after.split_once(' ')?;
    let at = time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(nanos.parse::<i64>().ok()?)).ok()?;
    Some((Time(at), id.to_owned()))
}

impl Orchestrator {
    /// The page of archived chains that goes on from after (a page's `next`; empty for the newest). The snapshot leaves
    /// archived chains out: there is no end to how many the owner puts away.
    pub async fn archived_chains(&self, after: String) -> Result<ChainPage> {
        let chains = self.store.read(|r| r.chains()).await?;
        let mut p = archived_page(chains, &after, ARCHIVED_SHOWN);
        let ids: Vec<String> = p.chains.iter().map(|c| c.chain.id.clone()).collect();
        p.standings = self.standings_of(&ids).await;
        Ok(p)
    }
}
