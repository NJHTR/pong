//! Contract index for M3-SLICE-001B.
//!
//! The executable A1-A15 and A22-A35 coverage lives in
//! `tests/agent_execution.rs`, which uses the durable SQLite path. The only
//! ignored tests below are explicitly outside 001B. They are OPEN / FUTURE
//! CONTRACT and are never counted as PASS.

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a16_handoff_is_deferred() {}

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a17_handoff_task_context_is_deferred() {}

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a18_handoff_version_context_is_deferred() {}

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a19_checkpoint_is_deferred() {}

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a20_rollback_is_deferred() {}

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a21_resume_from_version_is_deferred() {}
