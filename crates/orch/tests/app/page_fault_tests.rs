//! Which chain a mentor's page may name on the faults it reports (ADR-0119): one of his helps is on, as the store holds
//! them, never one his page says it shows.

use model::{Help, HelpState, Mentor, MentorStatus, by_owner};

use crate::tests::rig;

#[tokio::test]
async fn a_mentors_page_names_only_a_chain_one_of_his_helps_is_on() {
    let r = rig();
    r.o.store
        .update(|tx| -> store::Result<()> {
            tx.put_mentor(Mentor {
                id: "m_po".to_owned(),
                label: "the PO".to_owned(),
                status: MentorStatus::Active,
                ..Mentor::default()
            })?;
            tx.put_help(Help {
                id: "help_1".to_owned(),
                chain: "ch_helped".to_owned(),
                card: "card_1".to_owned(),
                mentor: "m_po".to_owned(),
                state: HelpState::Accepted,
                ..Help::default()
            })?;
            Ok(())
        })
        .await
        .unwrap();
    let key =
        r.o.keys
            .create_worker_key("m_po", "laptop", &by_owner("owner", "web"))
            .await
            .unwrap();

    assert!(r.o.mentor_names(&key.id, "ch_helped").await);
    assert!(
        !r.o.mentor_names(&key.id, "ch_victim").await,
        "a chain none of his helps is on is not his to name"
    );
    assert!(
        !r.o.mentor_names("wk_revoked", "ch_helped").await,
        "a key the store does not hold names nothing"
    );
}
