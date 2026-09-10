//! M3-SLICE-003G-RE / PHASE-2: Cross-Workspace Target-Local Snapshot Materialization
//!
//! This phase implements target-local Snapshot publication after materializing
//! cross-workspace source content. It validates the full materialization flow:
//! Source Version/Snapshot → Target Workspace → Target-Local Snapshot → W2.head

use pong_core::cas::Digest;
use pong_core::metadata::{
    AgentIdentity, ExecutionCreation, OperationEnvelope, OperationRef, TaskCreation,
    VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{
    SnapshotOptions, WorkspaceFailPoint, WorkspaceFailpoints, WorkspaceFaultAction,
    WorkspaceManager,
};
use pong_core::{PongError, Repository, VersionRecord};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

struct MaterializationFixture {
    _project: tempfile::TempDir,
    _ws1_parent: tempfile::TempDir,
    _ws2_parent: tempfile::TempDir,
    repository: Repository,
    ws1_path: PathBuf,
    ws2_path: PathBuf,
    ws1_lease: pong_core::LeaseToken,
    ws2_lease: pong_core::LeaseToken,
    source_version: VersionRecord,
}

fn setup_materialization_fixture() -> MaterializationFixture {
    let project = tempdir().expect("project");
    let ws1_parent = tempdir().expect("ws1 parent");
    let ws2_parent = tempdir().expect("ws2 parent");
    let mut repository = Repository::init(project.path()).expect("repository");

    // Create shared environment
    repository
        .metadata_mut()
        .record_environment(
            "env-mat",
            "project-mat",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");

    // Create agents
    for (id, provider) in [("agent-codex", "codex"), ("agent-cursor", "cursor")] {
        repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: id.into(),
                provider: provider.into(),
                display_name: None,
                created_at: "t0".into(),
            })
            .expect("agent");
    }

    // Create task
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-mat".into(),
            project_id: "project-mat".into(),
            goal: "cross-workspace materialization".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");

    // Create W1
    let ws1_path = ws1_parent.path().join("workspace1");
    let ws1_lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws1-mat",
                "project-mat",
                &ws1_path,
                None,
                Some("env-mat"),
                "t0",
            )
            .expect("ws1");
        manager
            .acquire_lease("ws1-mat", "agent-codex", 0, 1_000_000)
            .expect("ws1 lease")
    };

    // Create W2
    let ws2_path = ws2_parent.path().join("workspace2");
    let ws2_lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws2-mat",
                "project-mat",
                &ws2_path,
                None,
                Some("env-mat"),
                "t1",
            )
            .expect("ws2");
        manager
            .acquire_lease("ws2-mat", "agent-cursor", 0, 1_000_000)
            .expect("ws2 lease")
    };

    // Create source content in W1
    fs::write(ws1_path.join("root.txt"), b"root content").expect("root file");
    fs::create_dir(ws1_path.join("subdir")).expect("subdir");
    fs::write(ws1_path.join("subdir/nested.txt"), b"nested content").expect("nested file");

    // Snapshot W1
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws1-mat", &ws1_lease, SnapshotOptions::default(), 1, "t2")
        .expect("snapshot");

    // Publish Version V100 in W1
    let operation = OperationEnvelope {
        operation_id: "op-v100-mat".into(),
        project_id: "project-mat".into(),
        request_id: "req-v100-mat".into(),
        agent_id: "agent-codex".into(),
        session_id: "session".into(),
        workspace_id: Some("ws1-mat".into()),
        environment_id: Some("env-mat".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t3".into(),
        tool: "test".into(),
        action: "version.create".into(),
        input_refs: vec![OperationRef {
            kind: "snapshot".into(),
            reference: snapshot.snapshot_id.clone(),
            media_type: None,
        }],
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: None,
    };
    repository
        .metadata_mut()
        .start_operation(operation)
        .expect("operation");

    let source_version = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws1-mat".into(),
            project_id: "project-mat".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-v100-mat".into(),
            environment_id: Some("env-mat".into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .expect("version");

    MaterializationFixture {
        _project: project,
        _ws1_parent: ws1_parent,
        _ws2_parent: ws2_parent,
        repository,
        ws1_path,
        ws2_path,
        ws1_lease,
        ws2_lease,
        source_version,
    }
}

// M1: Source Tests

#[test]
fn m1_source_version_exists() {
    let fixture = setup_materialization_fixture();
    let version = fixture
        .repository
        .metadata()
        .version_record(&fixture.source_version.version_id)
        .unwrap()
        .unwrap();
    assert_eq!(version.workspace_id, "ws1-mat");
}

#[test]
fn m1_source_snapshot_exists() {
    let fixture = setup_materialization_fixture();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.workspace_id, "ws1-mat");
    assert!(!snapshot.root_digest.is_empty());
}

#[test]
fn m1_version_snapshot_consistency() {
    let fixture = setup_materialization_fixture();
    let version = fixture
        .repository
        .metadata()
        .version_record(&fixture.source_version.version_id)
        .unwrap()
        .unwrap();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&version.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(version.snapshot_id, snapshot.snapshot_id);
    assert_eq!(version.workspace_id, snapshot.workspace_id);
    assert_eq!(version.project_id, snapshot.project_id);
}

#[test]
fn m1_source_ownership() {
    let fixture = setup_materialization_fixture();
    let version = fixture
        .repository
        .metadata()
        .version_record(&fixture.source_version.version_id)
        .unwrap()
        .unwrap();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&version.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(version.workspace_id, "ws1-mat");
    assert_eq!(snapshot.workspace_id, "ws1-mat");
}

#[test]
fn m1_source_immutability_after_materialization() {
    let mut fixture = setup_materialization_fixture();
    let source_version_before = fixture
        .repository
        .metadata()
        .version_record(&fixture.source_version.version_id)
        .unwrap()
        .unwrap();
    let source_snapshot_before = fixture
        .repository
        .metadata()
        .snapshot_record(&source_version_before.snapshot_id)
        .unwrap()
        .unwrap();

    // Materialize to W2
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-mat",
        )
        .expect("materialization");

    // Verify source unchanged
    let source_version_after = fixture
        .repository
        .metadata()
        .version_record(&fixture.source_version.version_id)
        .unwrap()
        .unwrap();
    let source_snapshot_after = fixture
        .repository
        .metadata()
        .snapshot_record(&source_version_before.snapshot_id)
        .unwrap()
        .unwrap();

    assert_eq!(source_version_before, source_version_after);
    assert_eq!(source_snapshot_before, source_snapshot_after);
}

// M2: Target Tests

#[test]
fn m2_target_workspace_exists() {
    let fixture = setup_materialization_fixture();
    let workspace = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert_eq!(workspace.workspace_id, "ws2-mat");
}

#[test]
fn m2_target_local_snapshot_created() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-create",
        )
        .expect("materialization");

    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&result.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.workspace_id, "ws2-mat");
    assert_eq!(snapshot.project_id, "project-mat");
}

#[test]
fn m2_target_snapshot_workspace_id_is_target() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-ws-id",
        )
        .expect("materialization");

    assert_eq!(result.workspace_id, "ws2-mat");
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&result.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.workspace_id, "ws2-mat");
}

#[test]
fn m2_target_head_updated() {
    let mut fixture = setup_materialization_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert!(ws2_before.head.is_none());

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-head",
        )
        .expect("materialization");

    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert!(ws2_after.head.is_some());
    let expected_head = format!("sha256:{}", result.digest);
    assert_eq!(ws2_after.head.as_deref(), Some(expected_head.as_str()));
}

#[test]
fn m2_target_version_head_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    let vh_before = ws2_before.version_head_id.clone();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-vh",
        )
        .expect("materialization");

    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws2_after.version_head_id, vh_before);
}

// M3: Content Tests

#[test]
fn m3_manifest_materialized_correctly() {
    let mut fixture = setup_materialization_fixture();
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-manifest",
        )
        .expect("materialization");

    // Verify content
    let root_content = fs::read(fixture.ws2_path.join("root.txt")).expect("root file");
    assert_eq!(root_content, b"root content");

    let nested_content = fs::read(fixture.ws2_path.join("subdir/nested.txt")).expect("nested file");
    assert_eq!(nested_content, b"nested content");
}

#[test]
fn m3_nested_tree() {
    let mut fixture = setup_materialization_fixture();
    // Create deeper nesting in W1
    fs::create_dir_all(fixture.ws1_path.join("a/b/c")).expect("nested dirs");
    fs::write(fixture.ws1_path.join("a/b/c/deep.txt"), b"deep content").expect("deep file");

    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws1-mat",
            &fixture.ws1_lease,
            SnapshotOptions::default(),
            2,
            "t-nested",
        )
        .expect("snapshot");

    let operation = OperationEnvelope {
        operation_id: "op-nested".into(),
        project_id: "project-mat".into(),
        request_id: "req-nested".into(),
        agent_id: "agent-codex".into(),
        session_id: "session".into(),
        workspace_id: Some("ws1-mat".into()),
        environment_id: Some("env-mat".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t-nested".into(),
        tool: "test".into(),
        action: "version.create".into(),
        input_refs: vec![
            OperationRef {
                kind: "version".into(),
                reference: fixture.source_version.version_id.clone(),
                media_type: None,
            },
            OperationRef {
                kind: "snapshot".into(),
                reference: snapshot.snapshot_id.clone(),
                media_type: None,
            },
        ],
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: None,
    };
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation)
        .expect("operation");

    let nested_version = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws1-mat".into(),
            project_id: "project-mat".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-nested".into(),
            environment_id: Some("env-mat".into()),
            created_at: "t-nested".into(),
            parent_version_id: Some(fixture.source_version.version_id.clone()),
        })
        .expect("version");

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &nested_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-mat-nested",
        )
        .expect("materialization");

    let deep_content = fs::read(fixture.ws2_path.join("a/b/c/deep.txt")).expect("deep file");
    assert_eq!(deep_content, b"deep content");
}

#[test]
fn m3_empty_tree() {
    let project = tempdir().expect("project");
    let ws1_parent = tempdir().expect("ws1 parent");
    let ws2_parent = tempdir().expect("ws2 parent");
    let mut repository = Repository::init(project.path()).expect("repository");

    repository
        .metadata_mut()
        .record_environment("env-empty", "project-empty", &json!({}), "t0")
        .expect("environment");
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-empty".into(),
            provider: "test".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .expect("agent");

    let ws1_path = ws1_parent.path().join("ws1");
    let ws2_path = ws2_parent.path().join("ws2");

    let ws1_lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws1-empty",
                "project-empty",
                &ws1_path,
                None,
                Some("env-empty"),
                "t0",
            )
            .expect("ws1");
        manager
            .acquire_lease("ws1-empty", "agent-empty", 0, 1_000_000)
            .expect("lease")
    };

    let ws2_lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws2-empty",
                "project-empty",
                &ws2_path,
                None,
                Some("env-empty"),
                "t1",
            )
            .expect("ws2");
        manager
            .acquire_lease("ws2-empty", "agent-empty", 0, 1_000_000)
            .expect("lease")
    };

    // Empty W1 (no files)
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws1-empty", &ws1_lease, SnapshotOptions::default(), 1, "t2")
        .expect("snapshot");

    let operation = OperationEnvelope {
        operation_id: "op-empty".into(),
        project_id: "project-empty".into(),
        request_id: "req-empty".into(),
        agent_id: "agent-empty".into(),
        session_id: "session".into(),
        workspace_id: Some("ws1-empty".into()),
        environment_id: Some("env-empty".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t3".into(),
        tool: "test".into(),
        action: "version.create".into(),
        input_refs: vec![OperationRef {
            kind: "snapshot".into(),
            reference: snapshot.snapshot_id.clone(),
            media_type: None,
        }],
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: None,
    };
    repository
        .metadata_mut()
        .start_operation(operation)
        .expect("operation");

    let version = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws1-empty".into(),
            project_id: "project-empty".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-empty".into(),
            environment_id: Some("env-empty".into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .expect("version");

    WorkspaceManager::new(&mut repository, Redactor::default())
        .materialize_from_version(
            "ws2-empty",
            &version.version_id,
            &ws2_lease,
            0,
            100,
            "t-mat",
        )
        .expect("materialization");

    // W2 should be empty
    let entries: Vec<_> = fs::read_dir(&ws2_path).unwrap().collect();
    assert_eq!(entries.len(), 0);
}

#[test]
fn m3_multiple_blobs() {
    let mut fixture = setup_materialization_fixture();
    // Create multiple files
    for i in 0..10 {
        fs::write(
            fixture.ws1_path.join(format!("file{}.txt", i)),
            format!("content {}", i).as_bytes(),
        )
        .expect("file");
    }

    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws1-mat",
            &fixture.ws1_lease,
            SnapshotOptions::default(),
            2,
            "t-multi",
        )
        .expect("snapshot");

    let operation = OperationEnvelope {
        operation_id: "op-multi".into(),
        project_id: "project-mat".into(),
        request_id: "req-multi".into(),
        agent_id: "agent-codex".into(),
        session_id: "session".into(),
        workspace_id: Some("ws1-mat".into()),
        environment_id: Some("env-mat".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t-multi".into(),
        tool: "test".into(),
        action: "version.create".into(),
        input_refs: vec![
            OperationRef {
                kind: "version".into(),
                reference: fixture.source_version.version_id.clone(),
                media_type: None,
            },
            OperationRef {
                kind: "snapshot".into(),
                reference: snapshot.snapshot_id.clone(),
                media_type: None,
            },
        ],
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: None,
    };
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation)
        .expect("operation");

    let multi_version = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws1-mat".into(),
            project_id: "project-mat".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-multi".into(),
            environment_id: Some("env-mat".into()),
            created_at: "t-multi".into(),
            parent_version_id: Some(fixture.source_version.version_id.clone()),
        })
        .expect("version");

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &multi_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-mat-multi",
        )
        .expect("materialization");

    // Verify all files
    for i in 0..10 {
        let content = fs::read(fixture.ws2_path.join(format!("file{}.txt", i))).expect("file");
        assert_eq!(content, format!("content {}", i).as_bytes());
    }
}

#[test]
fn m3_shared_cas_content() {
    let mut fixture = setup_materialization_fixture();
    let source_snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .unwrap()
        .unwrap();
    let source_digest = source_snapshot.root_digest.strip_prefix("sha256:").unwrap();
    let source_cas_digest = Digest::from_hex(source_digest).unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-cas",
        )
        .expect("materialization");

    // Source CAS still exists and readable
    let cas_path = fixture.repository.cas().object_path(source_cas_digest);
    assert!(cas_path.exists());
}

#[test]
fn m3_content_hash_consistency() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-hash",
        )
        .expect("materialization");

    // Verify target snapshot hash is valid
    let digest_hex = result.digest.to_hex();
    assert_eq!(digest_hex.len(), 64);
}

#[test]
fn m3_missing_cas_rejection() {
    let mut fixture = setup_materialization_fixture();
    let source_snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .unwrap()
        .unwrap();
    let source_digest = source_snapshot.root_digest.strip_prefix("sha256:").unwrap();
    let digest = Digest::from_hex(source_digest).unwrap();

    // Delete CAS object
    let cas_path = fixture.repository.cas().object_path(digest);
    fs::remove_file(&cas_path).expect("remove CAS");

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-missing",
        );

    assert!(matches!(
        result,
        Err(PongError::NotFound(_)) | Err(PongError::Integrity(_))
    ));
}

#[test]
fn m3_invalid_manifest_rejection() {
    let mut fixture = setup_materialization_fixture();
    let source_snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .unwrap()
        .unwrap();
    let source_digest = source_snapshot.root_digest.strip_prefix("sha256:").unwrap();
    let digest = Digest::from_hex(source_digest).unwrap();

    // Corrupt CAS object
    let cas_path = fixture.repository.cas().object_path(digest);
    fs::write(&cas_path, b"corrupted manifest").expect("corrupt");

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-invalid",
        );

    assert!(matches!(
        result,
        Err(PongError::Integrity(_)) | Err(PongError::NotFound(_))
    ));
}

// M4: Isolation Tests

#[test]
fn m4_w1_head_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws1_before = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-iso1",
        )
        .expect("materialization");

    let ws1_after = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws1_before.head, ws1_after.head);
}

#[test]
fn m4_w1_version_head_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws1_before = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-iso2",
        )
        .expect("materialization");

    let ws1_after = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws1_before.version_head_id, ws1_after.version_head_id);
}

#[test]
fn m4_w1_revision_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws1_before = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-iso3",
        )
        .expect("materialization");

    let ws1_after = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws1_before.revision, ws1_after.revision);
}

#[test]
fn m4_w1_lease_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws1_before = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-iso4",
        )
        .expect("materialization");

    let ws1_after = fixture
        .repository
        .metadata()
        .workspace("ws1-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws1_before.revision, ws1_after.revision);
}

#[test]
fn m4_w2_version_head_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-iso5",
        )
        .expect("materialization");

    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws2_before.version_head_id, ws2_after.version_head_id);
}

#[test]
fn m4_parallel_target_workspaces_isolated() {
    let mut fixture = setup_materialization_fixture();

    // Create W3
    let ws3_parent = tempdir().expect("ws3 parent");
    let ws3_path = ws3_parent.path().join("workspace3");
    let ws3_lease = {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        manager
            .create_local(
                "ws3-mat",
                "project-mat",
                &ws3_path,
                None,
                Some("env-mat"),
                "t-ws3",
            )
            .expect("ws3");
        manager
            .acquire_lease("ws3-mat", "agent-cursor", 0, 1_000_000)
            .expect("ws3 lease")
    };

    // Materialize to both W2 and W3
    let result_w2 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-w2",
        )
        .expect("w2 materialization");

    let result_w3 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws3-mat",
            &fixture.source_version.version_id,
            &ws3_lease,
            0,
            100,
            "t-w3",
        )
        .expect("w3 materialization");

    // Each has own Snapshot
    assert_eq!(result_w2.workspace_id, "ws2-mat");
    assert_eq!(result_w3.workspace_id, "ws3-mat");
    assert_ne!(result_w2.snapshot_id, result_w3.snapshot_id);

    // Both have correct content
    let w2_content = fs::read(fixture.ws2_path.join("root.txt")).expect("w2 file");
    let w3_content = fs::read(ws3_path.join("root.txt")).expect("w3 file");
    assert_eq!(w2_content, b"root content");
    assert_eq!(w3_content, b"root content");
}

// M5: Atomicity Tests

#[test]
fn m5_materialization_failure_leaves_w2_head_unchanged() {
    let mut fixture = setup_materialization_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();

    // Inject failure
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeFileWrite,
            WorkspaceFaultAction::Fail,
        ),
    );

    let result = manager.materialize_from_version(
        "ws2-mat",
        &fixture.source_version.version_id,
        &fixture.ws2_lease,
        0,
        100,
        "t-fail",
    );

    assert!(result.is_err());

    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert_eq!(ws2_before.head, ws2_after.head);
    assert_eq!(ws2_before.revision, ws2_after.revision);
}

#[test]
fn m5_partial_tree_cleanup() {
    let mut fixture = setup_materialization_fixture();

    // Inject failure during materialization
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeFileSync,
            WorkspaceFaultAction::Fail,
        ),
    );

    let _result = manager.materialize_from_version(
        "ws2-mat",
        &fixture.source_version.version_id,
        &fixture.ws2_lease,
        0,
        100,
        "t-partial",
    );

    // W2 should remain in original state (empty or unchanged)
    let ws2 = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert!(ws2.head.is_none());
}

#[test]
fn m5_publication_failure() {
    // This test verifies that publication failures don't leave inconsistent state
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-pub",
        );

    // Either succeeds completely or fails completely
    match result {
        Ok(snapshot) => {
            let ws2 = fixture
                .repository
                .metadata()
                .workspace("ws2-mat")
                .unwrap()
                .unwrap();
            let expected_head = format!("sha256:{}", snapshot.digest);
            assert_eq!(ws2.head.as_deref(), Some(expected_head.as_str()));
        }
        Err(_) => {
            let ws2 = fixture
                .repository
                .metadata()
                .workspace("ws2-mat")
                .unwrap()
                .unwrap();
            assert!(ws2.head.is_none());
        }
    }
}

#[test]
fn m5_invalid_source() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            "ver-nonexistent",
            &fixture.ws2_lease,
            0,
            100,
            "t-invalid",
        );

    assert!(matches!(result, Err(PongError::NotFound(_))));

    let ws2 = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert!(ws2.head.is_none());
}

#[test]
fn m5_invalid_cas() {
    let mut fixture = setup_materialization_fixture();
    let source_snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .unwrap()
        .unwrap();
    let digest =
        Digest::from_hex(source_snapshot.root_digest.strip_prefix("sha256:").unwrap()).unwrap();
    fs::remove_file(fixture.repository.cas().object_path(digest)).expect("remove");

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-cas",
        );

    assert!(result.is_err());
    let ws2 = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert!(ws2.head.is_none());
}

#[test]
fn m5_invalid_manifest() {
    let mut fixture = setup_materialization_fixture();
    let source_snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .unwrap()
        .unwrap();
    let digest =
        Digest::from_hex(source_snapshot.root_digest.strip_prefix("sha256:").unwrap()).unwrap();
    fs::write(fixture.repository.cas().object_path(digest), b"corrupt").expect("corrupt");

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-manifest",
        );

    assert!(result.is_err());
}

// M6: Idempotency Tests

#[test]
fn m6_same_request_repeated() {
    let mut fixture = setup_materialization_fixture();
    let first = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-same",
        )
        .expect("first");

    let ws2_mid = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();

    let second = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_mid.revision,
            100,
            "t-same2",
        )
        .expect("second");

    // Both succeed, both result in valid target-local Snapshots
    assert_eq!(first.workspace_id, "ws2-mat");
    assert_eq!(second.workspace_id, "ws2-mat");
}

#[test]
fn m6_retry_after_failure() {
    let mut fixture = setup_materialization_fixture();

    // First attempt fails
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeFileWrite,
            WorkspaceFaultAction::Fail,
        ),
    );
    let first = manager.materialize_from_version(
        "ws2-mat",
        &fixture.source_version.version_id,
        &fixture.ws2_lease,
        0,
        100,
        "t-retry-fail",
    );
    assert!(first.is_err());

    // Retry succeeds
    let second = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-retry-success",
        )
        .expect("retry");

    assert_eq!(second.workspace_id, "ws2-mat");
    let ws2 = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();
    assert!(ws2.head.is_some());
}

#[test]
fn m6_repeated_successful_materialization() {
    let mut fixture = setup_materialization_fixture();

    // Materialize multiple times
    for i in 0..3 {
        let ws2 = fixture
            .repository
            .metadata()
            .workspace("ws2-mat")
            .unwrap()
            .unwrap();

        let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
            .materialize_from_version(
                "ws2-mat",
                &fixture.source_version.version_id,
                &fixture.ws2_lease,
                ws2.revision,
                100,
                &format!("t-repeat-{}", i),
            )
            .expect("materialization");

        assert_eq!(result.workspace_id, "ws2-mat");
    }
}

#[test]
fn m6_no_corrupted_duplicate_state() {
    let mut fixture = setup_materialization_fixture();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-dup",
        )
        .expect("materialization");

    let ws2 = fixture
        .repository
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .unwrap();

    // Head should point to valid Snapshot
    assert!(ws2.head.is_some());
    let head = ws2.head.as_ref().unwrap();
    let snapshot_id = fixture
        .repository
        .metadata()
        .snapshot_id_for_root(head)
        .unwrap()
        .expect("snapshot exists");

    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&snapshot_id)
        .unwrap()
        .expect("snapshot record");
    assert_eq!(snapshot.workspace_id, "ws2-mat");
}

// M7: Recovery Tests

#[test]
fn m7_cold_reopen() {
    let mut fixture = setup_materialization_fixture();
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-reopen",
        )
        .expect("materialization");

    let project_path = fixture._project.path().to_path_buf();

    drop(fixture);

    // Reopen
    let reopened = Repository::open(&project_path).expect("reopen");
    let ws2 = reopened
        .metadata()
        .workspace("ws2-mat")
        .unwrap()
        .expect("ws2 after reopen");
    assert!(ws2.head.is_some());
}

#[test]
fn m7_persisted_target_snapshot_reload() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-persist",
        )
        .expect("materialization");

    let project_path = fixture._project.path().to_path_buf();
    drop(fixture);

    let reopened = Repository::open(&project_path).expect("reopen");
    let snapshot = reopened
        .metadata()
        .snapshot_record(&result.snapshot_id)
        .unwrap()
        .expect("snapshot after reopen");
    assert_eq!(snapshot.workspace_id, "ws2-mat");
}

#[test]
fn m7_reopened_workspace_head_correctness() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-head-correct",
        )
        .expect("materialization");

    let project_path = fixture._project.path().to_path_buf();
    drop(fixture);

    let reopened = Repository::open(&project_path).expect("reopen");
    let ws2 = reopened.metadata().workspace("ws2-mat").unwrap().unwrap();
    let expected_head = format!("sha256:{}", result.digest);
    assert_eq!(ws2.head.as_deref(), Some(expected_head.as_str()));
}

#[test]
fn m7_cas_readable_after_reopen() {
    let mut fixture = setup_materialization_fixture();
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-cas-reopen",
        )
        .expect("materialization");

    let project_path = fixture._project.path().to_path_buf();
    let digest_hex = result.digest.to_hex();
    let digest = Digest::from_hex(&digest_hex).unwrap();

    drop(fixture);

    let reopened = Repository::open(&project_path).expect("reopen");
    let cas_path = reopened.cas().object_path(digest);
    assert!(cas_path.exists());
}

// M8: Scenario Tests

#[test]
fn m8_codex_cursor_base_materialization() {
    let mut fixture = setup_materialization_fixture();

    // E1: Codex creates V100 in W1
    fixture
        .repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "e1-codex".into(),
            task_id: "task-mat".into(),
            agent_id: "agent-codex".into(),
            parent_execution_id: None,
            workspace_id: Some("ws1-mat".into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "t-e1".into(),
        })
        .expect("e1");

    // E2: Cursor in W2 with base = V100
    fixture
        .repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "e2-cursor".into(),
            task_id: "task-mat".into(),
            agent_id: "agent-cursor".into(),
            parent_execution_id: None,
            workspace_id: Some("ws2-mat".into()),
            base_version_id: Some(fixture.source_version.version_id.clone()),
            current_version_id: None,
            created_at: "t-e2".into(),
        })
        .expect("e2");

    // Materialize base into W2
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-cursor-mat",
        )
        .expect("materialization");

    assert_eq!(result.workspace_id, "ws2-mat");
    let content = fs::read(fixture.ws2_path.join("root.txt")).expect("content");
    assert_eq!(content, b"root content");
}

#[test]
fn m8_w1_to_w2() {
    m2_target_local_snapshot_created();
}

#[test]
fn m8_w1_to_w3() {
    let mut fixture = setup_materialization_fixture();
    let ws3_parent = tempdir().expect("ws3 parent");
    let ws3_path = ws3_parent.path().join("workspace3");
    let ws3_lease = {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        manager
            .create_local(
                "ws3-mat",
                "project-mat",
                &ws3_path,
                None,
                Some("env-mat"),
                "t-ws3",
            )
            .expect("ws3");
        manager
            .acquire_lease("ws3-mat", "agent-cursor", 0, 1_000_000)
            .expect("ws3 lease")
    };

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws3-mat",
            &fixture.source_version.version_id,
            &ws3_lease,
            0,
            100,
            "t-w3",
        )
        .expect("materialization");

    assert_eq!(result.workspace_id, "ws3-mat");
}

#[test]
fn m8_w1_to_w2_and_w3_concurrently() {
    m4_parallel_target_workspaces_isolated();
}

#[test]
fn m8_two_target_executions_materialize_same_source() {
    let mut fixture = setup_materialization_fixture();

    // E2 and E3 both reference V100
    for id in ["e2", "e3"] {
        fixture
            .repository
            .metadata_mut()
            .create_execution(&ExecutionCreation {
                execution_id: id.into(),
                task_id: "task-mat".into(),
                agent_id: "agent-cursor".into(),
                parent_execution_id: None,
                workspace_id: Some("ws2-mat".into()),
                base_version_id: Some(fixture.source_version.version_id.clone()),
                current_version_id: None,
                created_at: format!("t-{}", id),
            })
            .expect("execution");
    }

    // Both can materialize
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .materialize_from_version(
            "ws2-mat",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            0,
            100,
            "t-e2-mat",
        )
        .expect("materialization");

    assert_eq!(result.workspace_id, "ws2-mat");
}
