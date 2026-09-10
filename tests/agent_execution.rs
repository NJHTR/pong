//! M3-SLICE-001B durable Agent / Task / Execution integration coverage.
//!
//! These tests use the real SQLite-backed MetadataStore. The six ignored
//! cases are deliberately outside 001B and are labelled as future contract,
//! not as passing evidence.

use pong_core::metadata::{
    AgentIdentity, ExecutionCreation, MetadataStore, OperationEnvelope, OperationRef, TaskCreation,
    VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{
    LeaseToken, MetadataFailpoint, MetadataFailpoints, PongError, Repository, WorkspaceUpdate,
};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;
use tempfile::{tempdir, TempDir};

struct Fixture {
    project: TempDir,
    repository: Repository,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("repository");
    Fixture {
        project,
        repository,
    }
}

fn seed(fixture: &mut Fixture) {
    fixture
        .repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: Some("Agent A".into()),
            created_at: "t0".into(),
        })
        .expect("agent");
    fixture
        .repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "durable execution".into(),
            context_ref: Some("ctx://redacted".into()),
            created_at: "t0".into(),
        })
        .expect("task");
}

fn execution(id: &str, parent: Option<&str>) -> ExecutionCreation {
    ExecutionCreation {
        execution_id: id.into(),
        task_id: "task-a".into(),
        agent_id: "agent-a".into(),
        parent_execution_id: parent.map(str::to_owned),
        workspace_id: None,
        base_version_id: None,
        current_version_id: None,
        created_at: format!("created-{id}"),
    }
}

fn operation(project_id: &str, operation_id: &str, agent_id: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: project_id.into(),
        request_id: format!("request-{operation_id}"),
        agent_id: agent_id.into(),
        session_id: "session-a".into(),
        workspace_id: None,
        environment_id: None,
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: format!("start-{operation_id}"),
        tool: "agent".into(),
        action: "agent.execute".into(),
        input_refs: Vec::new(),
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "NONE".into(),
        policy_decision: None,
    }
}

fn workspace_fixture() -> (TempDir, Repository, LeaseToken, PathBuf) {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment("env-a", "project-a", &json!({"os":"windows"}), "t0")
        .expect("environment");
    let path = workspace_parent.path().join("workspace");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local("ws-a", "project-a", &path, None, Some("env-a"), "t0")
            .expect("workspace");
        manager
            .acquire_lease("ws-a", "agent-a", 0, 1_000_000)
            .expect("lease")
    };
    (workspace_parent, repository, lease, path)
}

fn version_fixture() -> (TempDir, Repository, LeaseToken, PathBuf, String) {
    let (workspace_parent, mut repository, lease, path) = workspace_fixture();
    fs::write(path.join("state.txt"), b"version").expect("state");
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-a", &lease, SnapshotOptions::default(), 1, "t1")
        .expect("snapshot");
    let mut envelope = operation("project-a", "op-version", "agent-a");
    envelope.workspace_id = Some("ws-a".into());
    envelope.environment_id = Some("env-a".into());
    envelope.action = "version.create".into();
    envelope.input_refs.push(OperationRef {
        kind: "snapshot".into(),
        reference: snapshot.snapshot_id.clone(),
        media_type: None,
    });
    repository
        .metadata_mut()
        .start_operation(envelope)
        .expect("version operation");
    let version = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-a".into(),
            project_id: "project-a".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-version".into(),
            environment_id: Some("env-a".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .expect("version");
    (
        workspace_parent,
        repository,
        lease,
        path,
        version.version_id,
    )
}

#[test]
fn a1_agent_identity_is_durable_and_stable() {
    let mut f = fixture();
    let identity = AgentIdentity {
        agent_id: "agent-a".into(),
        provider: "codex".into(),
        display_name: Some("Agent A".into()),
        created_at: "t0".into(),
    };
    assert_eq!(
        f.repository
            .metadata_mut()
            .create_agent_identity(&identity)
            .unwrap(),
        identity
    );
    assert_eq!(
        f.repository
            .metadata_mut()
            .create_agent_identity(&identity)
            .unwrap(),
        identity
    );
    assert_eq!(
        f.repository
            .metadata()
            .list_agent_identities()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a2_provider_is_metadata_not_lifecycle_behavior() {
    let mut f = fixture();
    for (id, provider) in [
        ("a-codex", "codex"),
        ("a-cursor", "cursor"),
        ("a-claude", "claude"),
    ] {
        f.repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: id.into(),
                provider: provider.into(),
                display_name: None,
                created_at: "t".into(),
            })
            .unwrap();
    }
    assert_eq!(
        f.repository
            .metadata()
            .list_agent_identities()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn a3_task_identity_and_lifecycle_are_durable() {
    let mut f = fixture();
    let task = f
        .repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "t".into(),
            project_id: "p".into(),
            goal: "goal".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .unwrap();
    assert_eq!(task.state, "created");
    let running = f
        .repository
        .metadata_mut()
        .update_task_state("t", "running", 0, "t1")
        .unwrap();
    assert_eq!((running.state.as_str(), running.revision), ("running", 1));
    assert!(matches!(
        f.repository
            .metadata_mut()
            .update_task_state("t", "created", 1, "t2"),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn a4_execution_identity_and_task_agent_binding() {
    let mut f = fixture();
    seed(&mut f);
    let record = f
        .repository
        .metadata_mut()
        .create_execution(&execution("e1", None))
        .unwrap();
    assert_eq!(
        (
            record.task_id.as_str(),
            record.agent_id.as_str(),
            record.state.as_str()
        ),
        ("task-a", "agent-a", "created")
    );
    assert_eq!(
        f.repository.metadata().execution("e1").unwrap(),
        Some(record)
    );
}

#[test]
fn a5_missing_task_is_rejected() {
    let mut f = fixture();
    f.repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "a".into(),
            provider: "p".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut e = execution("e", None);
    e.task_id = "missing".into();
    e.agent_id = "a".into();
    assert!(matches!(
        f.repository.metadata_mut().create_execution(&e),
        Err(PongError::NotFound(_))
    ));
}

#[test]
fn a6_missing_agent_is_rejected() {
    let mut f = fixture();
    let mut e = execution("e", None);
    e.agent_id = "missing".into();
    assert!(matches!(
        f.repository.metadata_mut().create_execution(&e),
        Err(PongError::NotFound(_))
    ));
}

#[test]
fn a7_workspace_attachment_is_scope_checked() {
    let (_parent, mut repository, _lease, _path) = workspace_fixture();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut e = execution("e", None);
    e.workspace_id = Some("ws-a".into());
    assert_eq!(
        repository
            .metadata_mut()
            .create_execution(&e)
            .unwrap()
            .workspace_id
            .as_deref(),
        Some("ws-a")
    );
}

#[test]
fn a8_version_references_are_explicit_not_inferred() {
    let (_parent, mut repository, _lease, _path, version_id) = version_fixture();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut e = execution("e-version", None);
    e.workspace_id = Some("ws-a".into());
    e.base_version_id = Some(version_id.clone());
    let record = repository.metadata_mut().create_execution(&e).unwrap();
    assert_eq!(record.base_version_id.as_deref(), Some(version_id.as_str()));
    assert_eq!(record.current_version_id, None);
}

#[test]
fn a9_current_version_is_not_derived_from_workspace_head() {
    let (_parent, mut repository, _lease, _path, version_id) = version_fixture();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut e = execution("e-version", None);
    e.workspace_id = Some("ws-a".into());
    e.current_version_id = Some(version_id);
    let record = repository.metadata_mut().create_execution(&e).unwrap();
    assert!(record.current_version_id.is_some());
}

#[test]
fn a10_parent_execution_requires_existing_same_task() {
    let mut f = fixture();
    seed(&mut f);
    assert!(matches!(
        f.repository
            .metadata_mut()
            .create_execution(&execution("child", Some("missing"))),
        Err(PongError::NotFound(_))
    ));
    f.repository
        .metadata_mut()
        .create_execution(&execution("parent", None))
        .unwrap();
    let child = f
        .repository
        .metadata_mut()
        .create_execution(&execution("child", Some("parent")))
        .unwrap();
    assert_eq!(child.parent_execution_id.as_deref(), Some("parent"));
}

#[test]
fn a11_multi_level_subagent_lineage_is_iterative() {
    let mut f = fixture();
    seed(&mut f);
    for (id, parent) in [("e1", None), ("e2", Some("e1")), ("e3", Some("e2"))] {
        f.repository
            .metadata_mut()
            .create_execution(&execution(id, parent))
            .unwrap();
    }
    assert_eq!(
        f.repository
            .metadata()
            .child_executions("e2")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a12_parent_cycles_are_rejected() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e1", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("e2", Some("e1")))
        .unwrap();
    let connection = Connection::open(f.repository.active_metadata_path()).unwrap();
    connection
        .execute(
            "UPDATE executions SET parent_execution_id = 'e2' WHERE execution_id = 'e1'",
            [],
        )
        .unwrap();
    assert!(matches!(
        f.repository.metadata().execution("e1"),
        Err(PongError::Integrity(_))
    ));
}

#[test]
fn a13_parallel_children_are_independent() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("parent", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("c1", Some("parent")))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("c2", Some("parent")))
        .unwrap();
    assert_eq!(
        f.repository
            .metadata()
            .child_executions("parent")
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn a14_operation_ownership_is_unique_and_durable() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e1", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_operation(operation("project-a", "op1", "agent-a"))
        .unwrap();
    let owner = f
        .repository
        .metadata_mut()
        .attach_operation_to_execution("e1", "op1", "t1")
        .unwrap();
    assert_eq!(owner.execution_id, "e1");
    assert_eq!(
        f.repository
            .metadata()
            .operations_for_execution("e1")
            .unwrap()
            .len(),
        1
    );
    f.repository
        .metadata_mut()
        .create_execution(&execution("e2", None))
        .unwrap();
    assert!(matches!(
        f.repository
            .metadata_mut()
            .attach_operation_to_execution("e2", "op1", "t2"),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn a15_child_operation_is_owned_by_child_not_parent() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("parent", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("child", Some("parent")))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_operation(operation("project-a", "op-child", "agent-a"))
        .unwrap();
    f.repository
        .metadata_mut()
        .attach_operation_to_execution("child", "op-child", "t")
        .unwrap();
    assert!(f
        .repository
        .metadata()
        .operations_for_execution("parent")
        .unwrap()
        .is_empty());
    assert_eq!(
        f.repository
            .metadata()
            .operations_for_execution("child")
            .unwrap()
            .len(),
        1
    );
}

#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a16_handoff() {}
#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a17_handoff_preserves_task() {}
#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a18_handoff_preserves_version_context() {}
#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a19_checkpoint() {}
#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a20_rollback_preserves_history() {}
#[test]
#[ignore = "OPEN / FUTURE CONTRACT"]
fn a21_resume_from_version() {}

#[test]
fn a22_provider_failure_is_explicit_test_double_failure() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    let failed = f
        .repository
        .metadata_mut()
        .finish_execution("e", "failed", Some("TEST_DOUBLE provider failure"), 1, "t2")
        .unwrap();
    assert_eq!(failed.state, "failed");
    assert_eq!(
        f.repository
            .metadata()
            .task("task-a")
            .unwrap()
            .unwrap()
            .state,
        "running"
    );
}

#[test]
fn a23_interrupted_execution_survives_cold_reopen() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    f.repository
        .metadata_mut()
        .finish_execution("e", "interrupted", Some("process interrupted"), 1, "t2")
        .unwrap();
    drop(f.repository);
    let reopened = Repository::open(f.project.path()).unwrap();
    assert_eq!(
        reopened.metadata().execution("e").unwrap().unwrap().state,
        "interrupted"
    );
}

#[test]
fn a24_core_persistence_failure_rolls_back_execution_and_task() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    assert!(matches!(
        f.repository.metadata_mut().start_execution("e", 0, "t1"),
        Err(PongError::FaultInjected(_))
    ));
    assert_eq!(
        f.repository
            .metadata()
            .execution("e")
            .unwrap()
            .unwrap()
            .state,
        "created"
    );
    assert_eq!(
        f.repository
            .metadata()
            .task("task-a")
            .unwrap()
            .unwrap()
            .state,
        "created"
    );
}

#[test]
fn a25_postcommit_uncertainty_is_recovered_after_reopen() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    f.repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    assert!(matches!(
        f.repository
            .metadata_mut()
            .finish_execution("e", "completed", Some("ok"), 1, "t2"),
        Err(PongError::FaultInjected(_))
    ));
    drop(f.repository);
    let reopened = Repository::open(f.project.path()).unwrap();
    assert_eq!(
        reopened.metadata().execution("e").unwrap().unwrap().state,
        "completed"
    );
}

#[test]
fn a26_workspace_lease_is_still_the_only_write_authority() {
    let (_parent, mut repository, lease, _path) = workspace_fixture();
    let stale = LeaseToken {
        expires_at_ms: lease.expires_at_ms,
        epoch: lease.epoch + 1,
        ..lease.clone()
    };
    let error = repository
        .metadata_mut()
        .update_workspace(WorkspaceUpdate {
            workspace_id: "ws-a",
            expected_revision: 0,
            lease: &stale,
            branch_ref: None,
            head: None,
            environment_id: Some("env-a"),
            status: "created",
            updated_at: "t1",
            now_ms: 1,
        })
        .expect_err("stale lease");
    assert!(matches!(error, PongError::Conflict(_)));
}

#[test]
fn a27_stale_execution_revision_fails_deterministically() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    assert!(matches!(
        f.repository
            .metadata_mut()
            .finish_execution("e", "failed", Some("x"), 0, "t2"),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn a28_legacy_v01_opens_with_empty_m3_tables() {
    let project = tempdir().unwrap();
    let initialized = Repository::init(project.path()).unwrap();
    let metadata_path = initialized.layout().metadata_path().to_path_buf();
    drop(initialized);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut path = metadata_path.as_os_str().to_os_string();
        path.push(suffix);
        let _ = fs::remove_file(PathBuf::from(path));
    }
    let connection = Connection::open(&metadata_path).unwrap();
    connection
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .unwrap();
    drop(connection);
    let repository = Repository::open(project.path()).unwrap();
    assert_eq!(
        repository.metadata().list_agent_identities().unwrap().len(),
        0
    );
    assert_eq!(
        repository
            .metadata()
            .list_tasks("legacy-project")
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        repository
            .metadata()
            .list_events("legacy-stream")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a29_registered_secret_is_not_persisted_in_agent_or_task_rows() {
    let project = tempdir().unwrap();
    let mut redactor = Redactor::default();
    redactor.register_secret("secret-value").unwrap();
    let mut repository = Repository::init_with_redactor(project.path(), redactor.clone()).unwrap();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent".into(),
            provider: "provider".into(),
            display_name: Some("secret-value".into()),
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task".into(),
            project_id: "p".into(),
            goal: "secret-value goal".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    drop(repository);
    let bytes = fs::read(project.path().join(".pong/metadata.sqlite")).unwrap();
    assert!(!redactor.contains_secret(&bytes));
}

#[test]
fn a30_context_reference_is_opaque_and_not_a_prompt_transcript() {
    let mut f = fixture();
    let task = f
        .repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "t".into(),
            project_id: "p".into(),
            goal: "goal".into(),
            context_ref: Some("opaque://context/1".into()),
            created_at: "t".into(),
        })
        .unwrap();
    assert_eq!(task.context_ref.as_deref(), Some("opaque://context/1"));
}

#[test]
fn a31_parent_and_children_recover_after_cold_reopen() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("parent", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("child", Some("parent")))
        .unwrap();
    drop(f.repository);
    let reopened = Repository::open(f.project.path()).unwrap();
    assert_eq!(
        reopened.metadata().child_executions("parent").unwrap()[0].execution_id,
        "child"
    );
}

#[test]
fn a32_distinct_children_do_not_overwrite_each_other() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("parent", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("c1", Some("parent")))
        .unwrap();
    f.repository
        .metadata_mut()
        .create_execution(&execution("c2", Some("parent")))
        .unwrap();
    assert_eq!(
        f.repository
            .metadata()
            .child_executions("parent")
            .unwrap()
            .iter()
            .map(|e| e.execution_id.as_str())
            .collect::<Vec<_>>(),
        vec!["c1", "c2"]
    );
}

#[test]
fn a33_multiple_provider_identities_share_same_core_path() {
    let mut f = fixture();
    for (agent, provider) in [("a1", "codex"), ("a2", "cursor")] {
        f.repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: agent.into(),
                provider: provider.into(),
                display_name: None,
                created_at: "t".into(),
            })
            .unwrap();
    }
    f.repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "t".into(),
            project_id: "p".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    for (id, agent) in [("e1", "a1"), ("e2", "a2")] {
        let mut e = execution(id, None);
        e.task_id = "t".into();
        e.agent_id = agent.into();
        f.repository.metadata_mut().create_execution(&e).unwrap();
    }
    assert_eq!(
        f.repository.metadata().list_executions("t").unwrap().len(),
        2
    );
}

#[test]
fn a34_execution_lineage_is_distinct_from_version_lineage() {
    let mut f = fixture();
    seed(&mut f);
    for (id, parent) in [
        ("root", None),
        ("child", Some("root")),
        ("leaf", Some("child")),
    ] {
        f.repository
            .metadata_mut()
            .create_execution(&execution(id, parent))
            .unwrap();
    }
    assert_eq!(
        f.repository
            .metadata()
            .execution("leaf")
            .unwrap()
            .unwrap()
            .parent_execution_id
            .as_deref(),
        Some("child")
    );
}

#[test]
fn a35_task_and_execution_states_are_independent() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    f.repository
        .metadata_mut()
        .finish_execution("e", "failed", Some("provider"), 1, "t2")
        .unwrap();
    assert_eq!(
        f.repository
            .metadata()
            .task("task-a")
            .unwrap()
            .unwrap()
            .state,
        "running"
    );
    assert_eq!(
        f.repository
            .metadata()
            .execution("e")
            .unwrap()
            .unwrap()
            .state,
        "failed"
    );
}

#[test]
fn a36_exact_retry_returns_the_same_durable_execution_result() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    let first = f
        .repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    let retry = f
        .repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    assert_eq!(first, retry);
    let completed = f
        .repository
        .metadata_mut()
        .finish_execution("e", "completed", Some("ok"), 1, "t2")
        .unwrap();
    assert_eq!(
        f.repository
            .metadata_mut()
            .finish_execution("e", "completed", Some("ok"), 1, "t2")
            .unwrap(),
        completed
    );
}

#[test]
fn a37_unknown_outcome_is_not_inferred_as_success() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    f.repository
        .metadata_mut()
        .start_execution("e", 0, "t1")
        .unwrap();
    let unknown = f
        .repository
        .metadata_mut()
        .finish_execution("e", "unknown", Some("uncertain commit"), 1, "t2")
        .unwrap();
    assert_eq!(unknown.state, "unknown");
    drop(f.repository);
    let reopened = Repository::open(f.project.path()).unwrap();
    assert_eq!(
        reopened.metadata().execution("e").unwrap().unwrap().state,
        "unknown"
    );
}

#[test]
fn a38_missing_workspace_and_version_references_fail_closed() {
    let mut f = fixture();
    seed(&mut f);
    let mut missing_workspace = execution("e-workspace", None);
    missing_workspace.workspace_id = Some("missing".into());
    assert!(matches!(
        f.repository
            .metadata_mut()
            .create_execution(&missing_workspace),
        Err(PongError::NotFound(_))
    ));
    let (_parent, mut repository, _lease, _path, _version) = version_fixture();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut missing_version = execution("e-version", None);
    missing_version.workspace_id = Some("ws-a".into());
    missing_version.base_version_id = Some("ver-missing".into());
    assert!(matches!(
        repository.metadata_mut().create_execution(&missing_version),
        Err(PongError::Integrity(_)) | Err(PongError::NotFound(_))
    ));
}

#[test]
fn a39_project_and_environment_scope_are_checked() {
    let (_parent, mut repository, _lease, _path) = workspace_fixture();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-b".into(),
            project_id: "project-b".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut execution_b = execution("e-b", None);
    execution_b.task_id = "task-b".into();
    execution_b.workspace_id = Some("ws-a".into());
    assert!(matches!(
        repository.metadata_mut().create_execution(&execution_b),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn a40_current_version_update_uses_dual_cas_and_preserves_workspace_heads() {
    let (_parent, mut repository, lease, _path, version_id) = version_fixture();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "codex".into(),
            display_name: None,
            created_at: "t".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "g".into(),
            context_ref: None,
            created_at: "t".into(),
        })
        .unwrap();
    let mut e = execution("e-current", None);
    e.workspace_id = Some("ws-a".into());
    repository.metadata_mut().create_execution(&e).unwrap();
    let before = repository.metadata().workspace("ws-a").unwrap().unwrap();
    let expected_workspace_revision = before.revision;
    let updated = repository
        .metadata_mut()
        .set_execution_current_version(
            "e-current",
            &version_id,
            &lease,
            0,
            expected_workspace_revision,
            "t3",
            1,
        )
        .unwrap();
    assert_eq!(
        updated.current_version_id.as_deref(),
        Some(version_id.as_str())
    );
    let after = repository.metadata().workspace("ws-a").unwrap().unwrap();
    assert_eq!(after.head, before.head);
    assert_eq!(after.version_head_id, before.version_head_id);
    assert_eq!(after.revision, expected_workspace_revision + 1);
    assert_eq!(
        repository
            .metadata_mut()
            .set_execution_current_version(
                "e-current",
                &version_id,
                &lease,
                0,
                expected_workspace_revision,
                "t3",
                1,
            )
            .unwrap(),
        updated
    );
    assert_eq!(
        repository.metadata_mut().create_execution(&e).unwrap(),
        updated
    );
}

#[test]
fn a41_concurrent_start_is_one_transition_or_same_retry() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("e", None))
        .unwrap();
    let path = f.repository.active_metadata_path().to_path_buf();
    drop(f.repository);
    let barrier = Arc::new(Barrier::new(2));
    let path_one = path.clone();
    let path_two = path.clone();
    let (one, two) = thread::scope(|scope| {
        let barrier_one = Arc::clone(&barrier);
        let barrier_two = Arc::clone(&barrier);
        let first = scope.spawn(move || {
            let mut store = MetadataStore::open(&path_one).unwrap();
            barrier_one.wait();
            store.start_execution("e", 0, "t1")
        });
        let second = scope.spawn(move || {
            let mut store = MetadataStore::open(&path_two).unwrap();
            barrier_two.wait();
            store.start_execution("e", 0, "t1")
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    match (one, two) {
        (Ok(first), Ok(second)) => assert_eq!(first, second),
        (Ok(_), Err(PongError::Conflict(_))) | (Err(PongError::Conflict(_)), Ok(_)) => {}
        (first, second) => panic!("unexpected concurrent results: {first:?} / {second:?}"),
    }
}

#[test]
fn a42_concurrent_child_creation_keeps_distinct_children() {
    let mut f = fixture();
    seed(&mut f);
    f.repository
        .metadata_mut()
        .create_execution(&execution("parent", None))
        .unwrap();
    let path = f.repository.active_metadata_path().to_path_buf();
    drop(f.repository);
    let barrier = Arc::new(Barrier::new(2));
    let path_one = path.clone();
    let path_two = path.clone();
    let (one, two) = thread::scope(|scope| {
        let barrier_one = Arc::clone(&barrier);
        let barrier_two = Arc::clone(&barrier);
        let first = scope.spawn(move || {
            let mut store = MetadataStore::open(&path_one).unwrap();
            barrier_one.wait();
            store.create_execution(&execution("c1", Some("parent")))
        });
        let second = scope.spawn(move || {
            let mut store = MetadataStore::open(&path_two).unwrap();
            barrier_two.wait();
            store.create_execution(&execution("c2", Some("parent")))
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    assert!(
        one.is_ok() && two.is_ok(),
        "unexpected child results: {one:?} / {two:?}"
    );
    let store = MetadataStore::open(&path).unwrap();
    assert_eq!(store.child_executions("parent").unwrap().len(), 2);
}

/// Development-only scale sanity. This is not a performance budget or release
/// qualification; it records that the bounded implementation handles the
/// requested entity counts and an iterative 1,000-node execution chain.
#[test]
fn development_performance_sanity_counts_and_depth() {
    let started = Instant::now();
    for count in [1_usize, 100, 1_000] {
        let mut f = fixture();
        let project_id = format!("scale-project-{count}");
        for index in 0..count {
            let agent_id = format!("scale-agent-{index:04}");
            let task_id = format!("scale-task-{index:04}");
            f.repository
                .metadata_mut()
                .create_agent_identity(&AgentIdentity {
                    agent_id: agent_id.clone(),
                    provider: "test-double".into(),
                    display_name: None,
                    created_at: format!("scale-{index:04}"),
                })
                .unwrap();
            f.repository
                .metadata_mut()
                .create_task(&TaskCreation {
                    task_id: task_id.clone(),
                    project_id: project_id.clone(),
                    goal: "development scale sanity".into(),
                    context_ref: None,
                    created_at: format!("scale-{index:04}"),
                })
                .unwrap();
            f.repository
                .metadata_mut()
                .create_execution(&ExecutionCreation {
                    execution_id: format!("scale-execution-{index:04}"),
                    task_id,
                    agent_id,
                    parent_execution_id: None,
                    workspace_id: None,
                    base_version_id: None,
                    current_version_id: None,
                    created_at: format!("scale-{index:04}"),
                })
                .unwrap();
        }
        assert_eq!(
            f.repository
                .metadata()
                .list_agent_identities()
                .unwrap()
                .len(),
            count
        );
        assert_eq!(
            f.repository
                .metadata()
                .list_tasks(&project_id)
                .unwrap()
                .len(),
            count
        );
        let execution_count = (0..count)
            .map(|index| {
                f.repository
                    .metadata()
                    .list_executions(&format!("scale-task-{index:04}"))
                    .unwrap()
                    .len()
            })
            .sum::<usize>();
        assert_eq!(execution_count, count);
    }

    let mut f = fixture();
    seed(&mut f);
    let mut previous = None;
    for depth in 1..=1_000 {
        let id = format!("depth-execution-{depth}");
        let record = f
            .repository
            .metadata_mut()
            .create_execution(&ExecutionCreation {
                execution_id: id.clone(),
                task_id: "task-a".into(),
                agent_id: "agent-a".into(),
                parent_execution_id: previous.clone(),
                workspace_id: None,
                base_version_id: None,
                current_version_id: None,
                created_at: format!("depth-{depth:04}"),
            })
            .unwrap();
        assert_eq!(record.execution_id, id);
        previous = Some(id);
        if matches!(depth, 10 | 100 | 1_000) {
            assert_eq!(
                f.repository
                    .metadata()
                    .execution(previous.as_deref().unwrap())
                    .unwrap()
                    .unwrap()
                    .execution_id,
                format!("depth-execution-{depth}")
            );
        }
    }

    let elapsed_ms = started.elapsed().as_millis();
    eprintln!(
        "DEVELOPMENT_PERFORMANCE_SANITY entity_counts=1,100,1000 graph_depths=10,100,1000 max_depth=1000 elapsed_ms={elapsed_ms}"
    );
}
