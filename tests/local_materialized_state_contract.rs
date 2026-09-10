//! M3-SLICE-003F local materialized state / source-target contract index.
//!
//! These are intentionally ignored until the successor runtime slice
//! implements source validation, target-local Snapshot publication, and the
//! explicitly scoped cross-Workspace operations.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(l1_source_version_owned_by_w1);
not_implemented_contract_test!(l2_source_snapshot_owned_by_w1);
not_implemented_contract_test!(l3_source_immutable);
not_implemented_contract_test!(l4_w2_source_reference);
not_implemented_contract_test!(l5_source_no_mutation);
not_implemented_contract_test!(l6_working_state_is_not_version);
not_implemented_contract_test!(l7_target_snapshot_is_local);
not_implemented_contract_test!(l8_target_snapshot_without_version);
not_implemented_contract_test!(l9_workspace_head_points_local);
not_implemented_contract_test!(l10_version_head_remains_local);
not_implemented_contract_test!(l11_head_version_head_decoupled);
not_implemented_contract_test!(l12_cross_workspace_materialization);
not_implemented_contract_test!(l13_cross_workspace_restore);
not_implemented_contract_test!(l14_restore_source_result_separation);
not_implemented_contract_test!(l15_local_rollback_no_version);
not_implemented_contract_test!(l16_cross_workspace_rollback_scope);
not_implemented_contract_test!(l17_cross_workspace_rollback_no_version);
not_implemented_contract_test!(l18_cross_workspace_rollback_head);
not_implemented_contract_test!(l19_source_diff);
not_implemented_contract_test!(l20_local_diff);
not_implemented_contract_test!(l21_diff_read_only);
not_implemented_contract_test!(l22_shared_cas);
not_implemented_contract_test!(l23_lease_isolation);
not_implemented_contract_test!(l24_revision_isolation);
not_implemented_contract_test!(l25_parallel_materialization);
not_implemented_contract_test!(l26_resume_source_reference);
not_implemented_contract_test!(l27_handoff_source_reference);
not_implemented_contract_test!(l28_parent_separation);
not_implemented_contract_test!(l29_source_project_validation);
not_implemented_contract_test!(l30_source_environment_validation);
not_implemented_contract_test!(l31_source_generation_validation);
not_implemented_contract_test!(l32_source_migration_validation);
not_implemented_contract_test!(l33_corrupt_source_fail_closed);
not_implemented_contract_test!(l34_missing_cas_fail_closed);
not_implemented_contract_test!(l35_crash_recovery);
not_implemented_contract_test!(l36_exact_retry);
not_implemented_contract_test!(l37_no_phantom_success);
not_implemented_contract_test!(l38_legacy_compatibility);
not_implemented_contract_test!(l39_codex_w1_cursor_w2);
not_implemented_contract_test!(l40_codex_cursor_restore_resume);
