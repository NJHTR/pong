//! M3-SLICE-003C cross-Workspace Base Reference contract index.
//!
//! These cases remain ignored until a separately approved runtime slice adds
//! the additive Execution-level reference. They are contract coverage only.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(x1_same_workspace_base);
not_implemented_contract_test!(x2_cross_workspace_base);
not_implemented_contract_test!(x3_shared_immutable_base);
not_implemented_contract_test!(x4_base_read_only);
not_implemented_contract_test!(x5_workspace_isolation);
not_implemented_contract_test!(x6_workspace_head_isolation);
not_implemented_contract_test!(x7_version_head_isolation);
not_implemented_contract_test!(x8_lease_isolation);
not_implemented_contract_test!(x9_revision_isolation);
not_implemented_contract_test!(x10_concurrent_materialization);
not_implemented_contract_test!(x11_concurrent_diff);
not_implemented_contract_test!(x12_concurrent_restore);
not_implemented_contract_test!(x13_rollback_isolated);
not_implemented_contract_test!(x14_checkpoint_semantics);
not_implemented_contract_test!(x15_resume_semantics);
not_implemented_contract_test!(x16_handoff_semantics);
not_implemented_contract_test!(x17_version_parent_separation);
not_implemented_contract_test!(x18_graph_isolation);
not_implemented_contract_test!(x19_project_mismatch);
not_implemented_contract_test!(x20_environment_mismatch);
not_implemented_contract_test!(x21_generation_mismatch);
not_implemented_contract_test!(x22_migration_mismatch);
not_implemented_contract_test!(x23_corrupted_base);
not_implemented_contract_test!(x24_missing_base);
not_implemented_contract_test!(x25_immutable_base);
not_implemented_contract_test!(x26_no_base_mutation);
not_implemented_contract_test!(x27_crash_recovery);
not_implemented_contract_test!(x28_deterministic_retry);
not_implemented_contract_test!(x29_legacy_compatibility);
not_implemented_contract_test!(x30_codex_cursor_scenario);
