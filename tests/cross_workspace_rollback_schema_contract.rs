//! M3-SLICE-003H cross-Workspace rollback schema contract index.
//!
//! This slice freezes the additive persistence proposal only. Runtime
//! migration and rollback behavior remain deferred to a later implementation
//! slice.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(rsc1_source_version);
not_implemented_contract_test!(rsc2_source_snapshot);
not_implemented_contract_test!(rsc3_target_workspace);
not_implemented_contract_test!(rsc4_previous_head);
not_implemented_contract_test!(rsc5_result_head);
not_implemented_contract_test!(rsc6_previous_version_head);
not_implemented_contract_test!(rsc7_result_version_head);
not_implemented_contract_test!(rsc8_result_version_null);
not_implemented_contract_test!(rsc9_source_target_separation);
not_implemented_contract_test!(rsc10_target_snapshot_local);
not_implemented_contract_test!(rsc11_workspace_head_local);
not_implemented_contract_test!(rsc12_foreign_source_readable);
not_implemented_contract_test!(rsc13_foreign_source_immutable);
not_implemented_contract_test!(rsc14_rollback_history_preservation);
not_implemented_contract_test!(rsc15_idempotency);
not_implemented_contract_test!(rsc16_crash_recovery);
not_implemented_contract_test!(rsc17_prepared_state);
not_implemented_contract_test!(rsc18_failed_state);
not_implemented_contract_test!(rsc19_unknown_state);
not_implemented_contract_test!(rsc20_migration);
not_implemented_contract_test!(rsc21_legacy);
not_implemented_contract_test!(rsc22_cas_failure);
not_implemented_contract_test!(rsc23_snapshot_corruption);
not_implemented_contract_test!(rsc24_source_missing);
not_implemented_contract_test!(rsc25_target_mismatch);
not_implemented_contract_test!(rsc26_project_mismatch);
not_implemented_contract_test!(rsc27_environment_mismatch);
not_implemented_contract_test!(rsc28_generation_mismatch);
not_implemented_contract_test!(rsc29_migration_mismatch);
not_implemented_contract_test!(rsc30_no_version_creation);
