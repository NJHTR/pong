//! M3-SLICE-003G-RE / PHASE-4: Cross-Workspace Diff and Restore
//!
//! This phase validates cross-workspace diff and restore operations:
//! - Read-only diff: W2 current state vs V100[W1]
//! - Full restore: W2 ← V100[W1] with target-local Snapshot
//! - Source immutability: W1 unchanged
//! - Target independence: W2.version_head_id preserved
//! - Parallel isolation: W2 and W3 operate independently
//! - Agent scenarios: Codex → Cursor workflows

use pong_core::metadata::{
    AgentIdentity, ExecutionCreation, OperationEnvelope, OperationRef, TaskCreation,
    VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotChangeType, SnapshotOptions, WorkspaceManager};
use pong_core::{LeaseToken, PongError, Repository, VersionRecord};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

struct CrossWorkspaceFixture {
    _project: tempfile::TempDir,
    _ws1_parent: tempfile::TempDir,
    _ws2_parent: tempfile::TempDir,
    repository: Repository,
    #[allow(dead_code)]
    ws1_path: PathBuf,
    ws2_path: PathBuf,
    #[allow(dead_code)]
    ws1_lease: LeaseToken,
    ws2_lease: LeaseToken,
    source_version: VersionRecord,
}

fn setup_cross_workspace_fixture() -> CrossWorkspaceFixture {
    let project = tempdir().expect("project");
    let ws1_parent = tempdir().expect("ws1 parent");
    let ws2_parent = tempdir().expect("ws2 parent");
    let mut repository = Repository::init(project.path()).expect("repository");

    // Create shared environment
    repository
        .metadata_mut()
        .record_environment(
            "env-diff",
            "project-diff",
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
            task_id: "task-diff".into(),
            project_id: "project-diff".into(),
            goal: "cross-workspace diff and restore".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");

    // Create W1 (source workspace)
    let ws1_path = ws1_parent.path().join("workspace1");
    let ws1_lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws1-diff",
                "project-diff",
                &ws1_path,
                None,
                Some("env-diff"),
                "t0",
            )
            .expect("ws1");
        manager
            .acquire_lease("ws1-diff", "agent-codex", 0, 1_000_000)
            .expect("ws1 lease")
    };

    // Create W2 (target workspace)
    let ws2_path = ws2_parent.path().join("workspace2");
    let ws2_lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws2-diff",
                "project-diff",
                &ws2_path,
                None,
                Some("env-diff"),
                "t1",
            )
            .expect("ws2");
        manager
            .acquire_lease("ws2-diff", "agent-cursor", 0, 1_000_000)
            .expect("ws2 lease")
    };

    // Create source content in W1
    fs::write(ws1_path.join("source.txt"), b"source content from codex").expect("source file");
    fs::create_dir(ws1_path.join("src")).expect("src dir");
    fs::write(ws1_path.join("src/lib.rs"), b"pub fn hello() {}").expect("lib.rs");

    // Snapshot W1
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws1-diff", &ws1_lease, SnapshotOptions::default(), 1, "t2")
        .expect("snapshot");

    // Publish Version V100 in W1
    let operation = OperationEnvelope {
        operation_id: "op-v100-diff".into(),
        project_id: "project-diff".into(),
        request_id: "req-v100-diff".into(),
        agent_id: "agent-codex".into(),
        session_id: "session".into(),
        workspace_id: Some("ws1-diff".into()),
        environment_id: Some("env-diff".into()),
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
            workspace_id: "ws1-diff".into(),
            project_id: "project-diff".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-v100-diff".into(),
            environment_id: Some("env-diff".into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .expect("version");

    // Create different content in W2
    fs::write(ws2_path.join("target.txt"), b"target content from cursor").expect("target file");

    // Create execution (after workspaces exist)
    repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "exec-diff-001".into(),
            task_id: "task-diff".into(),
            agent_id: "agent-cursor".into(),
            parent_execution_id: None,
            workspace_id: Some("ws2-diff".into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "t0".into(),
        })
        .expect("execution");

    CrossWorkspaceFixture {
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

// D1: Source Validation

#[test]
fn d1_foreign_version_exists() {
    let fixture = setup_cross_workspace_fixture();
    let version = fixture
        .repository
        .metadata()
        .version_record(&fixture.source_version.version_id)
        .expect("query")
        .expect("exists");
    assert_eq!(version.workspace_id, "ws1-diff");
}

#[test]
fn d1_foreign_snapshot_exists() {
    let fixture = setup_cross_workspace_fixture();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.source_version.snapshot_id)
        .expect("query")
        .expect("exists");
    assert_eq!(snapshot.workspace_id, "ws1-diff");
}

#[test]
fn d1_version_snapshot_consistency() {
    let fixture = setup_cross_workspace_fixture();
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
    assert_eq!(version.workspace_id, snapshot.workspace_id);
    assert_eq!(version.project_id, snapshot.project_id);
}

// D2: Cross-Workspace Diff

#[test]
fn d2_diff_read_only() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws1_before = fixture
        .repository
        .metadata()
        .workspace("ws1-diff")
        .unwrap()
        .unwrap();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    let _diff = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff");

    // Verify no mutations
    let ws1_after = fixture
        .repository
        .metadata()
        .workspace("ws1-diff")
        .unwrap()
        .unwrap();
    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    assert_eq!(ws1_before.head, ws1_after.head);
    assert_eq!(ws1_before.version_head_id, ws1_after.version_head_id);
    assert_eq!(ws2_before.head, ws2_after.head);
    assert_eq!(ws2_before.version_head_id, ws2_after.version_head_id);
}

#[test]
fn d2_diff_detects_added_file() {
    let mut fixture = setup_cross_workspace_fixture();

    let diff = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff");

    // W2 has target.txt that W1 doesn't have
    let added: Vec<_> = diff
        .entries
        .iter()
        .filter(|e| matches!(e.change_type, SnapshotChangeType::Added))
        .map(|e| e.path.as_str())
        .collect();
    assert!(added.contains(&"target.txt"));
}

#[test]
fn d2_diff_detects_deleted_file() {
    let mut fixture = setup_cross_workspace_fixture();

    let diff = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff");

    // W1 has source.txt that W2 doesn't have
    let deleted: Vec<_> = diff
        .entries
        .iter()
        .filter(|e| matches!(e.change_type, SnapshotChangeType::Removed))
        .map(|e| e.path.as_str())
        .collect();
    assert!(deleted.contains(&"source.txt"));
}

#[test]
fn d2_diff_nested_paths() {
    let mut fixture = setup_cross_workspace_fixture();

    let diff = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff");

    let deleted: Vec<_> = diff
        .entries
        .iter()
        .filter(|e| matches!(e.change_type, SnapshotChangeType::Removed))
        .map(|e| e.path.as_str())
        .collect();
    assert!(deleted.contains(&"src/lib.rs"));
}

#[test]
fn d2_diff_deterministic() {
    let mut fixture = setup_cross_workspace_fixture();

    let diff1 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff1");

    let diff2 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff2");

    assert_eq!(diff1.entries, diff2.entries);
}

// D3: Cross-Workspace Restore

#[test]
fn d3_restore_creates_target_local_snapshot() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    // Result Snapshot must be owned by W2, not W1
    assert_eq!(result.workspace_id, "ws2-diff");
    assert_ne!(result.snapshot_id, fixture.source_version.snapshot_id);
}

#[test]
fn d3_restore_updates_w2_head() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();
    let head_before = ws2_before.head.clone();

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    assert_ne!(ws2_after.head, head_before);
    // Workspace.head stores "sha256:..." format
    let expected_head = format!("sha256:{}", result.digest);
    assert_eq!(ws2_after.head.as_deref(), Some(expected_head.as_str()));
}

#[test]
fn d3_restore_preserves_w2_version_head() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();
    let version_head_before = ws2_before.version_head_id.clone();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    let ws2_after = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    // Version Head must remain unchanged
    assert_eq!(ws2_after.version_head_id, version_head_before);
}

#[test]
fn d3_restore_leaves_w1_unchanged() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws1_before = fixture
        .repository
        .metadata()
        .workspace("ws1-diff")
        .unwrap()
        .unwrap();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    let ws1_after = fixture
        .repository
        .metadata()
        .workspace("ws1-diff")
        .unwrap()
        .unwrap();

    // W1 completely unchanged
    assert_eq!(ws1_before.head, ws1_after.head);
    assert_eq!(ws1_before.version_head_id, ws1_after.version_head_id);
    assert_eq!(ws1_before.revision, ws1_after.revision);
}

#[test]
fn d3_restore_materializes_content() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    // W2 physical state should now match W1 source
    let content = fs::read(fixture.ws2_path.join("source.txt")).expect("source.txt");
    assert_eq!(content, b"source content from codex");

    let lib_content = fs::read(fixture.ws2_path.join("src/lib.rs")).expect("lib.rs");
    assert_eq!(lib_content, b"pub fn hello() {}");

    // Old W2 file should be gone
    assert!(!fixture.ws2_path.join("target.txt").exists());
}

// D4: Failure Cases

#[test]
fn d4_diff_missing_source_version() {
    let mut fixture = setup_cross_workspace_fixture();

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", "ver-nonexistent");

    assert!(matches!(result, Err(PongError::NotFound(_))));
}

#[test]
fn d4_restore_stale_revision() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision + 999, // stale revision
            100,
            "t-restore",
        );

    assert!(matches!(result, Err(PongError::Conflict(_))));
}

// D5: Parallel Isolation

#[test]
fn d5_parallel_diff_w2_w3() {
    let mut fixture = setup_cross_workspace_fixture();

    // Create W3
    let ws3_parent = tempdir().expect("ws3 parent");
    let ws3_path = ws3_parent.path().join("workspace3");
    {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        manager
            .create_local(
                "ws3-diff",
                "project-diff",
                &ws3_path,
                None,
                Some("env-diff"),
                "t5",
            )
            .expect("ws3");
    }

    // Both W2 and W3 diff against V100[W1]
    let diff2 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff2");

    let diff3 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws3-diff", &fixture.source_version.version_id)
        .expect("diff3");

    // Both succeed independently
    assert!(!diff2.entries.is_empty());
    assert!(!diff3.entries.is_empty());
}

#[test]
fn d5_parallel_restore_w2_w3() {
    let mut fixture = setup_cross_workspace_fixture();

    // Create W3
    let ws3_parent = tempdir().expect("ws3 parent");
    let ws3_path = ws3_parent.path().join("workspace3");
    let ws3_lease = {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        manager
            .create_local(
                "ws3-diff",
                "project-diff",
                &ws3_path,
                None,
                Some("env-diff"),
                "t5",
            )
            .expect("ws3");
        manager
            .acquire_lease("ws3-diff", "agent-claude", 0, 1_000_000)
            .expect("ws3 lease")
    };

    fs::write(ws3_path.join("w3.txt"), b"w3 content").expect("w3.txt");

    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();
    let ws3_before = fixture
        .repository
        .metadata()
        .workspace("ws3-diff")
        .unwrap()
        .unwrap();

    // Restore both W2 and W3 from V100[W1]
    let result2 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore-w2",
        )
        .expect("restore w2");

    let result3 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws3-diff",
            &fixture.source_version.version_id,
            &ws3_lease,
            ws3_before.revision,
            101,
            "t-restore-w3",
        )
        .expect("restore w3");

    // Both have target-local Snapshots
    assert_eq!(result2.workspace_id, "ws2-diff");
    assert_eq!(result3.workspace_id, "ws3-diff");
    assert_ne!(result2.snapshot_id, result3.snapshot_id);

    // Both have identical content from source
    let w2_content = fs::read(fixture.ws2_path.join("source.txt")).expect("w2 source.txt");
    let w3_content = fs::read(ws3_path.join("source.txt")).expect("w3 source.txt");
    assert_eq!(w2_content, w3_content);
    assert_eq!(w2_content, b"source content from codex");
}

// D6: Agent Scenarios

#[test]
fn d6_codex_to_cursor_workflow() {
    let mut fixture = setup_cross_workspace_fixture();

    // Codex created V100 in W1 (done in fixture setup)
    // Cursor now diffs W2 against V100
    let diff = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version("ws2-diff", &fixture.source_version.version_id)
        .expect("diff");

    let deleted_count = diff
        .entries
        .iter()
        .filter(|e| matches!(e.change_type, SnapshotChangeType::Removed))
        .count();
    assert!(deleted_count > 0); // Cursor sees what Codex had

    // Cursor restores V100 into W2
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    // Cursor can now continue from Codex's state
    let content = fs::read(fixture.ws2_path.join("source.txt")).expect("source.txt");
    assert_eq!(content, b"source content from codex");
}

// D7: Recovery

#[test]
fn d7_cold_reopen_after_restore() {
    let mut fixture = setup_cross_workspace_fixture();
    let ws2_before = fixture
        .repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();

    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_from_version(
            "ws2-diff",
            &fixture.source_version.version_id,
            &fixture.ws2_lease,
            ws2_before.revision,
            100,
            "t-restore",
        )
        .expect("restore");

    let snapshot_id = result.snapshot_id.clone();
    let project_path = fixture._project.path().to_path_buf();

    // Close repository
    drop(fixture);

    // Reopen
    let repository = Repository::open(&project_path).expect("reopen");

    // Snapshot remains durable
    let snapshot = repository
        .metadata()
        .snapshot_record(&snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.workspace_id, "ws2-diff");

    // Workspace head points to result
    let ws2 = repository
        .metadata()
        .workspace("ws2-diff")
        .unwrap()
        .unwrap();
    assert_eq!(ws2.head.as_deref(), Some(snapshot.root_digest.as_str()));
}
