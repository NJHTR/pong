//! M3-SLICE-002A Handoff / Checkpoint / Rollback / Resume contract index.
//!
//! This file is intentionally contract-only. Every case is ignored with the
//! exact NOT_IMPLEMENTED_CONTRACT_TEST marker and must not be counted as
//! runtime evidence or PASS until a separately approved implementation exists.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

// Handoff
not_implemented_contract_test!(h1_handoff_basic);
not_implemented_contract_test!(h2_codex_to_cursor);
not_implemented_contract_test!(h3_task_preserved);
not_implemented_contract_test!(h4_version_preserved);
not_implemented_contract_test!(h5_workspace_preserved);
not_implemented_contract_test!(h6_handoff_failure);
not_implemented_contract_test!(h7_handoff_crash);
not_implemented_contract_test!(h8_handoff_idempotency);
not_implemented_contract_test!(h9_handoff_lineage);
not_implemented_contract_test!(h10_nested_execution_handoff);

// Checkpoint
not_implemented_contract_test!(c1_checkpoint_basic);
not_implemented_contract_test!(c2_checkpoint_immutable);
not_implemented_contract_test!(c3_checkpoint_points_to_version);
not_implemented_contract_test!(c4_checkpoint_missing_version);
not_implemented_contract_test!(c5_checkpoint_corrupted_version);
not_implemented_contract_test!(c6_checkpoint_idempotency);
not_implemented_contract_test!(c7_task_checkpoint);
not_implemented_contract_test!(c8_execution_checkpoint);

// Rollback
not_implemented_contract_test!(r1_rollback_to_version);
not_implemented_contract_test!(r2_rollback_to_checkpoint);
not_implemented_contract_test!(r3_rollback_task_baseline);
not_implemented_contract_test!(r4_rollback_execution_baseline);
not_implemented_contract_test!(r5_rollback_preserves_history);
not_implemented_contract_test!(r6_new_version_after_rollback);
not_implemented_contract_test!(r7_selective_rollback);
not_implemented_contract_test!(r8_parallel_rollback_isolation);
not_implemented_contract_test!(r9_rollback_crash);
not_implemented_contract_test!(r10_rollback_idempotency);

// Resume
not_implemented_contract_test!(s1_resume_from_version);
not_implemented_contract_test!(s2_resume_from_checkpoint);
not_implemented_contract_test!(s3_resume_after_failure);
not_implemented_contract_test!(s4_resume_creates_new_execution);
not_implemented_contract_test!(s5_resume_preserves_old_execution);
not_implemented_contract_test!(s6_resume_idempotency);

// Multi-Agent interaction
not_implemented_contract_test!(m1_multi_agent);
not_implemented_contract_test!(m2_parallel_execution_rollback_isolation);
not_implemented_contract_test!(m3_multi_level_handoff);
not_implemented_contract_test!(m4_cross_agent_takeover);
not_implemented_contract_test!(m5_agent_quota_exhaustion);
not_implemented_contract_test!(m6_agent_crash);
not_implemented_contract_test!(m7_unknown_execution);
not_implemented_contract_test!(m8_task_baseline);
