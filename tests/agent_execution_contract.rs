//! M3-SLICE-001A contract placeholders.
//!
//! These tests intentionally remain ignored. They name the contract surface
//! for the future Agent Execution Core and are not implementation evidence.

const NOT_IMPLEMENTED_CONTRACT_TEST: &str = "NOT_IMPLEMENTED_CONTRACT_TEST";

macro_rules! contract_placeholder {
    ($name:ident) => {
        #[test]
        #[ignore = "NOT_IMPLEMENTED_CONTRACT_TEST"]
        fn $name() {
            panic!("{NOT_IMPLEMENTED_CONTRACT_TEST}");
        }
    };
}

contract_placeholder!(a1_agent_identity);
contract_placeholder!(a2_provider_separation);
contract_placeholder!(a3_task_identity);
contract_placeholder!(a4_execution_identity);
contract_placeholder!(a5_execution_belongs_to_task);
contract_placeholder!(a6_execution_belongs_to_agent);
contract_placeholder!(a7_execution_workspace_attachment);
contract_placeholder!(a8_execution_base_version_attachment);
contract_placeholder!(a9_execution_current_version_attachment);
contract_placeholder!(a10_parent_execution);
contract_placeholder!(a11_multi_level_subagent);
contract_placeholder!(a12_execution_cycle_rejection);
contract_placeholder!(a13_parallel_execution);
contract_placeholder!(a14_workspace_isolation);
contract_placeholder!(a15_operation_ownership);
contract_placeholder!(a16_handoff);
contract_placeholder!(a17_handoff_preserves_task);
contract_placeholder!(a18_handoff_preserves_version_context);
contract_placeholder!(a19_checkpoint);
contract_placeholder!(a20_rollback_preserves_history);
contract_placeholder!(a21_resume_from_version);
contract_placeholder!(a22_provider_failure);
contract_placeholder!(a23_execution_crash);
contract_placeholder!(a24_core_persistence_failure);
contract_placeholder!(a25_unknown_outcome);
contract_placeholder!(a26_lease_enforcement);
contract_placeholder!(a27_stale_execution);
contract_placeholder!(a28_legacy_v01_compatibility);
contract_placeholder!(a29_agent_identity_has_no_secret);
contract_placeholder!(a30_context_transfer);
contract_placeholder!(a31_parent_execution_recovery);
contract_placeholder!(a32_concurrent_child_execution);
contract_placeholder!(a33_multiple_agent_providers);
contract_placeholder!(a34_execution_lineage);
contract_placeholder!(a35_task_vs_execution_separation);
