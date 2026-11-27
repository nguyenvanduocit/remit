//! The tests that drive the whole application (ADR-0096): orch's, and those of each crate below it that need every
//! domain at once. One binary, so the application is built and linked once.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers outside a #[test] fn, where a failed unwrap or panic is the failed assertion"
)]

mod disk_floor_tests;
mod gate_depth_tests;
mod hire_choice_tests;
mod inbox_prior_tests;
mod judge_tests;
mod memory_tests;
mod page_fault_tests;
mod permission_tests;
mod screen_tests;
mod shipping_folder;
mod shipping_land;
mod shipping_release;
mod shipping_round;
mod source_tests;
mod tests;
mod watchdog_tests;
