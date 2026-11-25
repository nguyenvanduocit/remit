//! Workflows
//!
//! A run of a script and the steps it made (ADR-0073), carried in its home's view and pushed whole: a run is read with its
//! steps filled in from their table, in seq order, and stored without them. A step is a change of its run, so each write of
//! one moves the run to its next revision, and the run a channel is pushed after it is the newer copy.

use model::{Workflow, WorkflowStatus, WorkflowStep};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

/// A run's document with its steps filled in.
macro_rules! workflow_doc {
    () => {
        "json_set(doc, '$.steps', json((SELECT json_group_array(json(s.doc) ORDER BY s.seq) FROM workflow_steps s WHERE s.workflow_id = workflows.id)))"
    };
}

impl Tx<'_> {
    /// Writes run over the revision it was read at, run.rev (0 for a new run), its steps left out (`put_step` writes
    /// them), and returns it at the revision stored with its steps as stored. `Error::Conflict` when the stored run is at
    /// another revision, or a new one's id is taken.
    pub fn put_workflow(&self, mut run: Workflow) -> Result<Workflow> {
        run.rev = run.rev.saturating_add(1);
        run.rev = self.put_doc(
            concat!(
                "INSERT INTO workflows(id, home, created_at, doc) VALUES(?,?,?,json_remove(?, '$.steps'))",
                " ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("workflows"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![run.id, run.home, ms(run.created_at), doc(&run)?],
        )?;
        run.steps = self.steps(&run.id)?;
        Ok(run)
    }

    /// Writes a step of a stored run, under the run's home, moves the run to its next revision and returns the run as
    /// stored, with its steps. `Error::NotFound` when no run has the step's workflow id.
    pub fn put_step(&self, s: &WorkflowStep) -> Result<Workflow> {
        let run = self.workflow(&s.workflow)?;
        self.exec(
            "INSERT INTO workflow_steps(workflow_id, seq, home, doc) VALUES(?,?,?,?) ON CONFLICT(workflow_id, seq) DO UPDATE SET doc=excluded.doc",
            params![s.workflow, s.seq, run.home, doc(s)?],
        )?;
        self.exec(
            "UPDATE workflows SET doc=json_set(doc, '$.rev', json_extract(doc, '$.rev') + 1) WHERE id=?",
            [&s.workflow],
        )?;
        self.workflow(&s.workflow)
    }
}

impl Reader<'_> {
    /// The run id, with its steps.
    pub fn workflow(&self, id: &str) -> Result<Workflow> {
        self.one(concat!("SELECT ", workflow_doc!(), " FROM workflows WHERE id=?"), [id])
    }

    /// The runs of a home, each with its steps, oldest first.
    pub fn workflows_of(&self, home: &str) -> Result<Vec<Workflow>> {
        self.list(
            concat!(
                "SELECT ",
                workflow_doc!(),
                " FROM workflows WHERE home=? ORDER BY created_at, rowid"
            ),
            [home],
        )
    }

    /// The runs of every home that have not ended (running, waiting or paused), each with its steps, oldest first: what
    /// a restart takes up again.
    pub fn running_workflows(&self) -> Result<Vec<Workflow>> {
        self.list(
            concat!(
                "SELECT ",
                workflow_doc!(),
                " FROM workflows WHERE status IN (?,?,?) ORDER BY created_at, rowid"
            ),
            [
                WorkflowStatus::Running.as_str(),
                WorkflowStatus::Waiting.as_str(),
                WorkflowStatus::Paused.as_str(),
            ],
        )
    }

    /// The steps of run id, by seq.
    pub fn steps(&self, id: &str) -> Result<Vec<WorkflowStep>> {
        self.list("SELECT doc FROM workflow_steps WHERE workflow_id=? ORDER BY seq", [id])
    }

    /// The newest step that names card: the call that put it to work, or a later one about it (accept, send back).
    /// `Error::NotFound` when no step names it.
    pub fn step_of_card(&self, card: &str) -> Result<WorkflowStep> {
        self.one(
            "SELECT doc FROM workflow_steps WHERE card_id=? ORDER BY seq DESC LIMIT 1",
            [card],
        )
    }
}
