//! Sensors (docs/adr/0047): the readers that write readings of a signal. A standing one has no chain; a script belongs
//! to the chain it reads for, and ends with it.

use model::{Sensor, SignalKey};
use rusqlite::params;

use crate::store::{Error, Reader, Result, Tx, doc, over_read_rev, unique_failed};

impl Tx<'_> {
    /// Writes sn over the revision it was read at, sn.rev (0 for a new sensor), and returns it at the revision stored.
    /// `Error::SensorName` when another of the same owner has its name; `Error::Conflict` when the stored sensor is at
    /// another revision, or a new one's id is taken.
    pub fn put_sensor(&self, mut sn: Sensor) -> Result<Sensor> {
        sn.rev = sn.rev.saturating_add(1);
        let rev = self.put_doc(
            concat!(
                "INSERT INTO sensors(id, chain, name, signal, doc) VALUES(?,?,?,?,?)
ON CONFLICT(id) DO UPDATE SET chain=excluded.chain, name=excluded.name, signal=excluded.signal, doc=excluded.doc",
                over_read_rev!("sensors"),
                "
RETURNING json_extract(doc, '$.rev')"
            ),
            params![sn.id, sn.chain, sn.name, sn.signal.0, doc(&sn)?],
        );
        match rev {
            Err(Error::Sqlite(e)) if unique_failed(&e, "sensors.chain, sensors.name") => Err(Error::SensorName),
            Err(e) => Err(e),
            Ok(rev) => {
                sn.rev = rev;
                Ok(sn)
            }
        }
    }

    pub fn delete_sensor(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM sensors WHERE id=?", [id])?;
        Ok(())
    }

    /// Ends every sensor that belongs to chain: nothing waits on what it read once the chain is gone.
    pub fn delete_sensors_of(&self, chain: &str) -> Result<()> {
        self.exec("DELETE FROM sensors WHERE chain=?", [chain])?;
        Ok(())
    }

    /// Ends every sensor that writes signal: a signal that is deleted has no writer left.
    pub fn delete_sensors_on(&self, signal: &SignalKey) -> Result<()> {
        self.exec("DELETE FROM sensors WHERE signal=?", [signal.0.as_str()])?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn sensor(&self, id: &str) -> Result<Sensor> {
        self.one("SELECT doc FROM sensors WHERE id=?", [id])
    }

    /// The sensor name of chain (empty: the standing sensors).
    pub fn sensor_named(&self, chain: &str, name: &str) -> Result<Sensor> {
        self.one("SELECT doc FROM sensors WHERE chain=? AND name=?", [chain, name])
    }

    /// Every sensor, by id.
    pub fn sensors(&self) -> Result<Vec<Sensor>> {
        self.list("SELECT doc FROM sensors ORDER BY id", [])
    }
}
