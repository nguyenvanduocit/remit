//! Employees: agents hired from a template and kept until retired (ADR-0032). One document with a revision, like a
//! chain.

use model::{Card, Employee};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, card_doc, doc, ms, over_read_rev, queued_params};

impl Tx<'_> {
    /// Writes e over the revision it was read at, e.rev (0 for a new employee), and returns it as stored, at the next
    /// revision. A new employee with no number takes the next after the highest on record, in this write, so two hires
    /// never share one (ADR-0061). `Error::Conflict` when the stored employee is at another revision, or a new one's id is
    /// taken.
    pub fn put_employee(&self, mut e: Employee) -> Result<Employee> {
        e.rev = e.rev.saturating_add(1);
        let stored: String = self.put_doc(
            concat!(
                "INSERT INTO employees(id, created_at, doc) VALUES(?1, ?2, CASE WHEN json_extract(?3, '$.number') = 0
  THEN json_set(?3, '$.number', (SELECT COALESCE(max(json_extract(doc, '$.number')), 0) + 1 FROM employees)) ELSE ?3 END)
ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("employees"),
                " RETURNING doc"
            ),
            params![e.id, ms(e.created_at), doc(&e)?],
        )?;
        Ok(serde_json::from_str(&stored)?)
    }
}

impl Reader<'_> {
    pub fn employee(&self, id: &str) -> Result<Employee> {
        self.one("SELECT doc FROM employees WHERE id=?", [id])
    }

    /// Every employee, oldest first.
    pub fn employees(&self) -> Result<Vec<Employee>> {
        self.list("SELECT doc FROM employees ORDER BY created_at, id", [])
    }

    /// Every card employee has been at work in, in any chain, oldest first: what a Chair reads of the load of an
    /// employee it may put to work.
    pub fn employee_cards(&self, employee: &str) -> Result<Vec<Card>> {
        self.list(
            concat!(
                "SELECT ",
                card_doc!(),
                " FROM cards WHERE json_extract(doc, '$.employee') = :employee ORDER BY created_at, id"
            ),
            &*queued_params(&[(":employee", &employee)]),
        )
    }
}
