//! What waits for the owner (ADR-0078, ADR-0087): only what no agent decides for him. That is an open item asking him to
//! type a credential or a sign-in code (`owner_acts.rs`, the secret acts), and one nobody is asked to decide for him
//! (`owner_only`: the Assistant's own, or Remit's with no agent to put it to). Everything else is decided by an agent as
//! it is raised: the Assistant for him. One list, in one order, computed here from the records; the board's column and
//! count and a chain's standing read it, so none of them can disagree. Pure.

use std::collections::HashMap;

use model::{Chain, InboxItem, InboxKind, Time};
use wire::app::{Wait, WaitKind};

use chains::homes::is_standalone_home;
use chains::owner_acts::holds_a_secret;

/// Whether open item it waits for the owner: it asks him to type a credential or a sign-in code, acts no agent does for
/// him (`ACT_CREDENTIAL`, `ACT_SIGN_IN_CODE`), or nobody is asked to decide it for him (`owner_only`). The one rule the
/// list, the board's column and its count follow. Pure.
pub fn waits_on_owner(it: &InboxItem) -> bool {
    it.owner_only || holds_a_secret(it)
}

/// The kind of wait an open inbox item is. A kind this build does not name asks the owner something.
pub(crate) fn kind_of(k: &InboxKind) -> WaitKind {
    match k {
        InboxKind::Permission => WaitKind::Approve,
        InboxKind::Error => WaitKind::Failed,
        InboxKind::Change => WaitKind::Apply,
        InboxKind::Credential => WaitKind::Key,
        InboxKind::Login => WaitKind::SignIn,
        InboxKind::Exception => WaitKind::Decide,
        InboxKind::Held => WaitKind::Held,
        InboxKind::Question => WaitKind::Asks,
    }
}

/// The server's word for a kind of wait: a chain's badge, an agent's line ("{label}: {ask}").
pub(crate) fn label_of(k: WaitKind) -> &'static str {
    match k {
        WaitKind::Asks => "Asks",
        WaitKind::Approve => "Approve",
        WaitKind::Failed => "Failed",
        WaitKind::Apply => "Apply",
        WaitKind::Key => "Key",
        WaitKind::SignIn => "Sign in",
        WaitKind::Decide => "Decide",
        WaitKind::Held => "Held",
    }
}

/// What a wait's place in the list is decided by: the oldest first; the id last, so the order is the same on every read.
type Place = (Time, String);

/// Whether something that began at since is newer than the owner's mark on its home (none: he never viewed it).
fn unseen(since: Time, mark: Option<Time>) -> bool {
    mark.is_none_or(|at| since > at)
}

/// Open inbox item it as a wait, about who, its home's mark being mark.
fn item_wait(it: &InboxItem, chain_id: Option<&str>, who: &str, mark: Option<Time>) -> (Place, Wait) {
    let kind = kind_of(&it.kind);
    let place = (it.created_at, it.id.clone());
    let w = Wait {
        id: it.id.clone(),
        kind,
        label: label_of(kind).to_owned(),
        chain_id: chain_id.map(str::to_owned),
        card_id: Some(it.card_id.clone()).filter(|id| !id.is_empty()),
        who: who.to_owned(),
        ask: it.title.clone(),
        since: it.created_at,
        unseen: unseen(it.created_at, mark),
    };
    (place, w)
}

/// What the waits of one chain are read from besides the chain: `items`, its open inbox items; `mark`, when the owner
/// last viewed it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ChainRecords<'a> {
    pub items: &'a [InboxItem],
    pub mark: Option<Time>,
}

fn placed_chain_waits(c: &Chain, f: &ChainRecords<'_>) -> Vec<(Place, Wait)> {
    f.items
        .iter()
        .filter(|it| waits_on_owner(it))
        .map(|it| item_wait(it, Some(&c.id), &c.title, f.mark))
        .collect()
}

fn ordered(mut placed: Vec<(Place, Wait)>) -> Vec<Wait> {
    placed.sort_by(|a, b| a.0.cmp(&b.0));
    placed.into_iter().map(|(_, w)| w).collect()
}

/// Every wait of chain c, in the board's order.
pub(crate) fn chain_waits(c: &Chain, f: &ChainRecords<'_>) -> Vec<Wait> {
    ordered(placed_chain_waits(c, f))
}

/// Everything the list is read from: every chain, the open inbox, and by home the owner's marks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WaitRecords<'a> {
    pub chains: &'a [Chain],
    pub inbox: &'a [InboxItem],
    pub marks: &'a HashMap<String, Time>,
}

/// Everything that waits for the owner, in the one order every surface shows.
pub(crate) fn waits_of(r: &WaitRecords<'_>) -> Vec<Wait> {
    let mut items: HashMap<&str, Vec<InboxItem>> = HashMap::new();
    for it in r.inbox.iter().filter(|it| waits_on_owner(it)) {
        items.entry(it.home.as_str()).or_default().push(it.clone());
    }
    let mut placed = Vec::new();
    for c in r.chains {
        let its = items.remove(c.id.as_str()).unwrap_or_default();
        let f = ChainRecords {
            items: &its,
            mark: r.marks.get(&c.id).copied(),
        };
        placed.extend(placed_chain_waits(c, &f));
    }
    // what waits from no chain: the Assistant's, or Remit's own (the files a deleted chain left)
    for (home, its) in items {
        let who = if is_standalone_home(home) {
            "the Assistant"
        } else {
            "Remit"
        };
        let mark = r.marks.get(home).copied();
        placed.extend(its.iter().map(|it| item_wait(it, None, who, mark)));
    }
    ordered(placed)
}
