use crate::store::{Reader, Result};

impl Reader<'_> {
    /// The ids of the keys the stored sealed values were sealed with, the sources', the event signals', the client
    /// keys', and the worker keys', each once and in order. None: nothing sealed is stored, so a new key loses nothing.
    pub fn sealed_key_ids(&self) -> Result<Vec<String>> {
        self.column(
            "SELECT key_id FROM source_values WHERE sealed IS NOT NULL UNION SELECT key_id FROM event_signals UNION SELECT key_id FROM client_keys UNION SELECT key_id FROM worker_keys ORDER BY 1",
            [],
        )
    }
}
