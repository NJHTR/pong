//! M3-SLICE-003A parallel Agent Workspace / reconciliation contract index.
//!
//! These cases are deliberately ignored until M3-SLICE-003B implements the
//! runtime. They are contract coverage, not implementation evidence.

macro_rules! not_implemented_contract_test {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("NOT_IMPLEMENTED_CONTRACT_TEST");
        }
    };
}

not_implemented_contract_test!(p1_one_execution_one_workspace);
not_implemented_contract_test!(p2_multiple_executions_multiple_workspaces);
not_implemented_contract_test!(p3_shared_base_version);
not_implemented_contract_test!(p4_workspace_isolation);
not_implemented_contract_test!(p5_workspace_head_isolation);
not_implemented_contract_test!(p6_lease_isolation);
not_implemented_contract_test!(p7_concurrent_snapshot);
not_implemented_contract_test!(p8_concurrent_diff);
not_implemented_contract_test!(p9_concurrent_restore);
not_implemented_contract_test!(p10_concurrent_rollback);
not_implemented_contract_test!(p11_rollback_isolation);
not_implemented_contract_test!(p12_checkpoint_isolation);
not_implemented_contract_test!(p13_resume_isolation);
not_implemented_contract_test!(p14_same_workspace_conflict);
not_implemented_contract_test!(p15_revision_conflict);
not_implemented_contract_test!(p16_lease_conflict);
not_implemented_contract_test!(p17_version_head_conflict);
not_implemented_contract_test!(p18_handoff_conflict);
not_implemented_contract_test!(p19_reconciliation_conflict);
not_implemented_contract_test!(p20_scan_mutation);
not_implemented_contract_test!(p21_crash_recovery);
not_implemented_contract_test!(p22_partial_completion);
not_implemented_contract_test!(p23_task_state_semantics);
not_implemented_contract_test!(p24_provider_quota);
not_implemented_contract_test!(p25_multi_level_execution);
not_implemented_contract_test!(p26_nested_handoff);
not_implemented_contract_test!(p27_shared_task);
not_implemented_contract_test!(p28_different_project_isolation);
not_implemented_contract_test!(p29_same_base_version);
not_implemented_contract_test!(p30_deterministic_conflict);
not_implemented_contract_test!(p31_codex_cursor_handoff);
not_implemented_contract_test!(p32_codex_cursor_parallel);
not_implemented_contract_test!(p33_rollback_one_agent_only);
not_implemented_contract_test!(p34_checkpoint_one_agent);
not_implemented_contract_test!(p35_resume_failed_agent);
not_implemented_contract_test!(p36_multiple_subagents);
