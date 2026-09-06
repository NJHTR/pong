//! M2-SLICE-012A proposal tests.
//!
//! These names document the contract boundary only. Version Head production
//! behavior is deliberately not implemented in this slice, so every test is
//! ignored and must not be counted as evidence.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(r1_snapshot_head_is_distinct_from_version_head);
not_implemented_contract_test!(r2_version_head_points_to_durable_version);
not_implemented_contract_test!(r3_version_head_snapshot_consistency);
not_implemented_contract_test!(r4_detached_version_remains_durable);
not_implemented_contract_test!(r5_same_snapshot_multiple_version_policy);
not_implemented_contract_test!(r6_current_version_selection_is_explicit);
not_implemented_contract_test!(r7_version_head_workspace_scope);
not_implemented_contract_test!(r8_version_head_generation_scope);
not_implemented_contract_test!(r9_version_head_crash_recovery);
not_implemented_contract_test!(r10_version_head_deletion_constraint);
not_implemented_contract_test!(r11_legacy_v01_compatibility);
not_implemented_contract_test!(r12_version_id_is_unchanged);
not_implemented_contract_test!(r13_workspace_head_keeps_snapshot_meaning);
not_implemented_contract_test!(r14_parent_is_not_version_head);
not_implemented_contract_test!(r15_human_label_does_not_change_identity);
