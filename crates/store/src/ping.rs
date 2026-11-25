use crate::store::{Result, Store};

impl Store {
    /// Asks whether the database answers a read. It leaves the write connection alone: there is only one, so a caller of
    /// ping (an unauthenticated health probe) would queue behind writers and make them wait in turn. A writer that is
    /// stuck shows in the scheduler, which writes on every Tick.
    pub async fn ping(&self) -> Result<()> {
        self.read(|r| r.value::<i64>("SELECT 1", []).map(drop)).await
    }
}
