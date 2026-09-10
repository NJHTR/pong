//! M3-SLICE-002C Rollback Result Semantics contract index.
//!
//! These cases are intentionally ignored until the separately approved
//! materialization implementation exists. They are contract coverage, not
//! runtime evidence and must not be counted as PASS.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(rr1_target_version);
not_implemented_contract_test!(rr2_target_checkpoint);
not_implemented_contract_test!(rr3_workspace_head_changes);
not_implemented_contract_test!(rr4_version_head_changes);
not_implemented_contract_test!(rr5_history_preserved);
not_implemented_contract_test!(rr6_version_immutable);
not_implemented_contract_test!(rr7_parent_immutable);
not_implemented_contract_test!(rr8_rollback_creates_no_version);
not_implemented_contract_test!(rr9_resume_creates_new_execution);
not_implemented_contract_test!(rr10_resume_later_creates_new_version);
not_implemented_contract_test!(rr11_rollback_idempotency);
not_implemented_contract_test!(rr12_rollback_crash_recovery);
not_implemented_contract_test!(rr13_physical_metadata_consistency);
not_implemented_contract_test!(rr14_parallel_isolation);
not_implemented_contract_test!(rr15_codex_cursor_rollback_resume);
not_implemented_contract_test!(rr16_failed_materialization);
not_implemented_contract_test!(rr17_missing_cas);
not_implemented_contract_test!(rr18_corrupted_snapshot);
not_implemented_contract_test!(rr19_stale_lease);
not_implemented_contract_test!(rr20_stale_revision);
not_implemented_contract_test!(rr21_no_phantom_success);
not_implemented_contract_test!(rr22_legacy_compatibility);
