//! From an agent's change to users (ADR-0096, ADR-0114, ADR-0115, ADR-0116): clones and checks, land and the merge queue
//! that lands every change itself, the rounds of competing attempts whose pick alone enters it, reverts, the gate and the
//! goal judge, verification, canary stages, and the release of what landed by each repository's own procedure.

pub mod carried;
pub mod check_rail;
pub mod checkpoint;
pub mod direct_push;
pub mod folder;
pub mod folder_repo;
pub mod gate;
pub mod goal;
pub mod hire;
pub mod install;
pub mod land;
pub mod landing;
pub mod landing_checks;
pub mod landing_view;
pub mod open_verification;
pub mod queue;
pub mod recheck;
pub mod release;
pub mod release_run;
mod release_trace;
pub mod remit_facts;
pub mod repo;
pub mod repochecks;
pub mod revert;
pub mod round;
pub mod seeded;
mod service;
pub mod shadow;
pub mod stage;
pub mod state_check;
pub mod verification;

pub use service::{Below, Outside, Shipping};
