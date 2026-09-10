//! M3-SLICE-003E source/materialization contract index.
//!
//! These cases remain ignored until a successor runtime slice explicitly
//! resolves target-local Snapshot publication and cross-Workspace rollback.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(s1_source_version_belongs_w1);
not_implemented_contract_test!(s2_source_snapshot_belongs_w1);
not_implemented_contract_test!(s3_immutable_source);
not_implemented_contract_test!(s4_w2_can_reference_source);
not_implemented_contract_test!(s5_source_cannot_be_mutated);
not_implemented_contract_test!(s6_w1_unchanged);
not_implemented_contract_test!(s7_w2_independent);
not_implemented_contract_test!(s8_local_materialization);
not_implemented_contract_test!(s9_result_snapshot_local);
not_implemented_contract_test!(s10_workspace_head_local);
not_implemented_contract_test!(s11_version_head_local);
not_implemented_contract_test!(s12_cross_workspace_diff);
not_implemented_contract_test!(s13_cross_workspace_restore);
not_implemented_contract_test!(s14_cross_workspace_rollback);
not_implemented_contract_test!(s15_rollback_source_result_separation);
not_implemented_contract_test!(s16_source_corruption);
not_implemented_contract_test!(s17_source_missing);
not_implemented_contract_test!(s18_cas_missing);
not_implemented_contract_test!(s19_generation_mismatch);
not_implemented_contract_test!(s20_environment_mismatch);
not_implemented_contract_test!(s21_project_mismatch);
not_implemented_contract_test!(s22_lease_isolation);
not_implemented_contract_test!(s23_revision_isolation);
not_implemented_contract_test!(s24_parallel_source_use);
not_implemented_contract_test!(s25_idempotency);
not_implemented_contract_test!(s26_crash_recovery);
not_implemented_contract_test!(s27_legacy);
not_implemented_contract_test!(s28_no_phantom_success);
not_implemented_contract_test!(s29_codex_w1_cursor_w2);
not_implemented_contract_test!(s30_codex_cursor_restore_resume);
