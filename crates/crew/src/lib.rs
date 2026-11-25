//! The organisation (ADR-0096): objectives and key results, employees, memory, mentors and helps, teaching, the coach,
//! duels, outcomes, the analyst, feedback.

pub mod analyst;
pub mod coach;
pub mod consolidate;
pub mod duel;
pub mod duel_judge;
pub mod duel_rating;
pub mod employee_change;
pub mod employee_face;
pub mod employee_name;
pub mod employee_rules;
pub mod employee_standing;
pub mod employee_strengths;
pub mod feedback;
pub mod help;
pub mod key_result_tools;
pub mod librarian;
pub mod lineage_standing;
pub mod memory;
pub mod memory_change;
mod memory_judge;
pub mod memory_tools;
pub mod mentor;
pub mod mentor_act;
pub mod notify;
pub mod objective;
pub mod objective_change;
pub mod objective_open;
pub mod objective_tools;
pub mod objective_view;
pub mod outcomes;
pub mod presence;
pub mod project;
pub mod project_tools;
pub mod recall;
pub mod recall_eval;
mod service;
pub mod teach;
pub mod teach_memory;
pub mod tool_recall;

pub use service::Crew;
