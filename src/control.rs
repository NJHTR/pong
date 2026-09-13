//! Provider-neutral Agent control facade.
//!
//! This module is deliberately thin. It composes the durable Agent, Task,
//! Execution, Workspace, Version, Checkpoint, Handoff, and Resume APIs without
//! exposing SQLite connections or making provider names part of core behavior.

use crate::metadata::{
    AgentIdentity, CheckpointCreation, CheckpointRecord, ExecutionCreation,
    ExecutionOperationRecord, ExecutionRecord, HandoffCreation, HandoffRecord, LeaseToken,
    OperationEnvelope, OperationOutcome, OperationRecord, OperationRef, ResumeCreation,
    ResumeRecord, SnapshotRecord, TaskCreation, TaskRecord, VersionPublication, VersionRecord,
    WorkspaceRecord,
};
use crate::redaction::Redactor;
use crate::workspace::{SnapshotDiff, SnapshotOptions, WorkspaceManager};
use crate::{PongError, Repository};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterAgentRequest {
    pub agent_id: String,
    pub provider: String,
    pub display_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub task_id: String,
    pub project_id: String,
    pub goal: String,
    pub context_ref: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateExecutionRequest {
    pub execution_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub parent_execution_id: Option<String>,
    pub workspace_id: Option<String>,
    pub base_version_id: Option<String>,
    pub current_version_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub workspace_id: String,
    pub project_id: String,
    pub path: PathBuf,
    pub branch_ref: Option<String>,
    pub environment_id: Option<String>,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquireWorkspaceRequest {
    pub workspace_id: String,
    pub agent_id: String,
    pub now_ms: i64,
    pub ttl_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenewWorkspaceRequest {
    pub lease: LeaseToken,
    pub now_ms: i64,
    pub ttl_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseWorkspaceRequest {
    pub lease: LeaseToken,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishVersionRequest {
    pub workspace_id: String,
    pub lease: LeaseToken,
    pub expected_workspace_revision: i64,
    pub now_ms: i64,
    pub created_at: String,
    pub operation_id: String,
    pub request_id: String,
    pub session_id: Option<String>,
    pub tool: Option<String>,
    pub parent_version_id: Option<String>,
    pub update_version_head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceView {
    pub workspace_id: String,
    pub project_id: String,
    pub driver: String,
    pub locator: String,
    pub branch_ref: Option<String>,
    pub head: Option<String>,
    pub version_head_id: Option<String>,
    pub environment_id: Option<String>,
    pub status: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseView {
    pub workspace_id: String,
    pub agent_id: Option<String>,
    pub epoch: i64,
    pub expires_at_ms: i64,
    pub acquired_at_ms: i64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotView {
    pub snapshot_id: String,
    pub root_digest: String,
    pub workspace_id: String,
    pub project_id: String,
    pub environment_id: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishVersionResult {
    pub snapshot: SnapshotView,
    pub version: VersionRecord,
    pub workspace: WorkspaceView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationBeforeState {
    workspace_revision: i64,
    workspace_head: Option<String>,
    version_head_id: Option<String>,
}

impl From<&WorkspaceRecord> for PublicationBeforeState {
    fn from(workspace: &WorkspaceRecord) -> Self {
        Self {
            workspace_revision: workspace.revision,
            workspace_head: workspace.head.clone(),
            version_head_id: workspace.version_head_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateView {
    pub agent: AgentIdentity,
    pub task: TaskRecord,
    pub execution: ExecutionRecord,
    pub workspace: Option<WorkspaceView>,
    pub lease: Option<LeaseView>,
    pub base_version: Option<VersionRecord>,
    pub current_version: Option<VersionRecord>,
    pub checkpoints: Vec<CheckpointRecord>,
    pub handoffs: Vec<HandoffRecord>,
    pub resume: Option<ResumeRecord>,
}

/// Thin provider-neutral composition layer over the existing durable core.
pub struct AgentControl<'a> {
    repository: &'a mut Repository,
    redactor: Redactor,
}

impl<'a> AgentControl<'a> {
    pub fn new(repository: &'a mut Repository) -> Self {
        Self::with_redactor(repository, Redactor::default())
    }

    pub fn with_redactor(repository: &'a mut Repository, redactor: Redactor) -> Self {
        Self {
            repository,
            redactor,
        }
    }

    pub fn register_agent(
        &mut self,
        request: RegisterAgentRequest,
    ) -> Result<AgentIdentity, PongError> {
        self.repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: request.agent_id,
                provider: request.provider,
                display_name: request.display_name,
                created_at: request.created_at,
            })
    }

    pub fn agent(&self, agent_id: &str) -> Result<Option<AgentIdentity>, PongError> {
        self.repository.metadata().agent_identity(agent_id)
    }

    pub fn create_task(&mut self, request: CreateTaskRequest) -> Result<TaskRecord, PongError> {
        self.repository.metadata_mut().create_task(&TaskCreation {
            task_id: request.task_id,
            project_id: request.project_id,
            goal: request.goal,
            context_ref: request.context_ref,
            created_at: request.created_at,
        })
    }

    pub fn task(&self, task_id: &str) -> Result<Option<TaskRecord>, PongError> {
        self.repository.metadata().task(task_id)
    }

    pub fn create_execution(
        &mut self,
        request: CreateExecutionRequest,
    ) -> Result<ExecutionRecord, PongError> {
        self.repository
            .metadata_mut()
            .create_execution(&ExecutionCreation {
                execution_id: request.execution_id,
                task_id: request.task_id,
                agent_id: request.agent_id,
                parent_execution_id: request.parent_execution_id,
                workspace_id: request.workspace_id,
                base_version_id: request.base_version_id,
                current_version_id: request.current_version_id,
                created_at: request.created_at,
            })
    }

    pub fn execution(&self, execution_id: &str) -> Result<Option<ExecutionRecord>, PongError> {
        self.repository.metadata().execution(execution_id)
    }

    pub fn start_execution(
        &mut self,
        execution_id: &str,
        expected_revision: i64,
        updated_at: &str,
    ) -> Result<ExecutionRecord, PongError> {
        self.repository
            .metadata_mut()
            .start_execution(execution_id, expected_revision, updated_at)
    }

    pub fn transition_execution(
        &mut self,
        execution_id: &str,
        next_state: &str,
        outcome: Option<&str>,
        expected_revision: i64,
        updated_at: &str,
    ) -> Result<ExecutionRecord, PongError> {
        self.repository.metadata_mut().transition_execution(
            execution_id,
            next_state,
            outcome,
            expected_revision,
            updated_at,
        )
    }

    pub fn finish_execution(
        &mut self,
        execution_id: &str,
        next_state: &str,
        outcome: Option<&str>,
        expected_revision: i64,
        updated_at: &str,
    ) -> Result<ExecutionRecord, PongError> {
        self.repository.metadata_mut().finish_execution(
            execution_id,
            next_state,
            outcome,
            expected_revision,
            updated_at,
        )
    }

    /// Record an Execution's explicit current Version using the existing
    /// dual execution/workspace revision and lease guards.
    #[allow(clippy::too_many_arguments)]
    pub fn set_execution_current_version(
        &mut self,
        execution_id: &str,
        version_id: &str,
        lease: &LeaseToken,
        expected_execution_revision: i64,
        expected_workspace_revision: i64,
        updated_at: &str,
        now_ms: i64,
    ) -> Result<ExecutionRecord, PongError> {
        self.repository
            .metadata_mut()
            .set_execution_current_version(
                execution_id,
                version_id,
                lease,
                expected_execution_revision,
                expected_workspace_revision,
                updated_at,
                now_ms,
            )
    }

    pub fn create_workspace(
        &mut self,
        request: CreateWorkspaceRequest,
    ) -> Result<WorkspaceView, PongError> {
        let record = WorkspaceManager::new(self.repository, self.redactor.clone()).create_local(
            &request.workspace_id,
            &request.project_id,
            request.path,
            request.branch_ref.as_deref(),
            request.environment_id.as_deref(),
            &request.now,
        )?;
        Ok(WorkspaceView::from(record))
    }

    pub fn workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceView>, PongError> {
        self.repository
            .metadata()
            .workspace(workspace_id)
            .map(|record| record.map(WorkspaceView::from))
    }

    pub fn acquire_workspace(
        &mut self,
        request: AcquireWorkspaceRequest,
    ) -> Result<LeaseToken, PongError> {
        WorkspaceManager::new(self.repository, self.redactor.clone()).acquire_lease(
            &request.workspace_id,
            &request.agent_id,
            request.now_ms,
            request.ttl_ms,
        )
    }

    pub fn renew_workspace(
        &mut self,
        request: RenewWorkspaceRequest,
    ) -> Result<LeaseToken, PongError> {
        WorkspaceManager::new(self.repository, self.redactor.clone()).renew_lease(
            &request.lease,
            request.now_ms,
            request.ttl_ms,
        )
    }

    pub fn release_workspace(&mut self, request: ReleaseWorkspaceRequest) -> Result<(), PongError> {
        WorkspaceManager::new(self.repository, self.redactor.clone())
            .release_lease(&request.lease, request.now_ms)
    }

    /// Snapshot the target Workspace and create a workspace-local immutable
    /// Version through the existing operation and Version persistence rules.
    pub fn publish_version(
        &mut self,
        request: PublishVersionRequest,
    ) -> Result<PublishVersionResult, PongError> {
        validate_publish_request(&request)?;
        let workspace_before = self
            .repository
            .metadata()
            .workspace(&request.workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        self.validate_publish_lease(&request)?;
        self.validate_publish_parent(&workspace_before, request.parent_version_id.as_deref())?;

        let existing_operation = self.publish_operation_for_request(&workspace_before, &request)?;
        let (snapshot_record, before_state) = if let Some(operation) = existing_operation {
            let before_state = publication_before_state(&operation)?;
            if before_state.workspace_revision != request.expected_workspace_revision {
                return Err(PongError::IdempotencyKeyReuse(request.request_id.clone()));
            }
            let snapshot_id = publication_snapshot_reference(&operation)?;
            let snapshot = self
                .repository
                .metadata()
                .snapshot_record(snapshot_id)?
                .ok_or_else(|| {
                    PongError::Integrity(
                        "Version publication operation references a missing Snapshot".into(),
                    )
                })?;
            let envelope =
                publication_operation(&request, &workspace_before, &snapshot, &before_state)?;
            self.repository.metadata_mut().start_operation(envelope)?;
            (snapshot, before_state)
        } else {
            if workspace_before.revision != request.expected_workspace_revision {
                return Err(PongError::Conflict("workspace revision is stale".into()));
            }
            let snapshot = WorkspaceManager::new(self.repository, self.redactor.clone())
                .snapshot_local_at_revision(
                    &request.workspace_id,
                    &request.lease,
                    SnapshotOptions::default(),
                    request.expected_workspace_revision,
                    request.now_ms,
                    &request.created_at,
                )?;
            let snapshot_record = self
                .repository
                .metadata()
                .snapshot_record(&snapshot.snapshot_id)?
                .ok_or_else(|| {
                    PongError::Integrity("published snapshot metadata is missing".into())
                })?;
            let before_state = PublicationBeforeState::from(&workspace_before);
            let envelope = publication_operation(
                &request,
                &workspace_before,
                &snapshot_record,
                &before_state,
            )?;
            self.repository.metadata_mut().start_operation(envelope)?;
            (snapshot_record, before_state)
        };

        self.complete_version_publication(request, snapshot_record, before_state)
    }

    fn complete_version_publication(
        &mut self,
        request: PublishVersionRequest,
        snapshot: SnapshotRecord,
        before_state: PublicationBeforeState,
    ) -> Result<PublishVersionResult, PongError> {
        let snapshot_revision = publication_snapshot_revision(&before_state, &snapshot)?;
        let existing_version = self
            .repository
            .metadata()
            .list_versions(&request.workspace_id)?
            .into_iter()
            .find(|version| version.creation_operation_id == request.operation_id);
        let workspace = self
            .repository
            .metadata()
            .workspace(&request.workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        validate_publication_stage(
            &workspace,
            &snapshot,
            &before_state,
            snapshot_revision,
            existing_version.as_ref(),
            request.update_version_head,
        )?;

        let version = self
            .repository
            .metadata_mut()
            .create_version(VersionPublication {
                workspace_id: workspace.workspace_id.clone(),
                project_id: workspace.project_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
                creation_operation_id: request.operation_id,
                environment_id: workspace.environment_id.clone(),
                created_at: request.created_at.clone(),
                parent_version_id: request.parent_version_id,
            })?;
        let workspace = self
            .repository
            .metadata()
            .workspace(&request.workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        let workspace = if request.update_version_head {
            if workspace.version_head_id.as_deref() == Some(version.version_id.as_str()) {
                let completed_revision = snapshot_revision.checked_add(1).ok_or_else(|| {
                    PongError::ResourceExhausted("workspace revision overflow".into())
                })?;
                if workspace.revision != completed_revision {
                    return Err(PongError::Conflict("workspace revision is stale".into()));
                }
                workspace
            } else {
                if workspace.revision != snapshot_revision
                    || workspace.version_head_id != before_state.version_head_id
                {
                    return Err(PongError::Conflict("workspace revision is stale".into()));
                }
                self.repository.metadata_mut().set_version_head(
                    &workspace.workspace_id,
                    Some(&version.version_id),
                    &request.lease,
                    snapshot_revision,
                    &request.created_at,
                    request.now_ms,
                )?
            }
        } else {
            if workspace.revision != snapshot_revision
                || workspace.version_head_id != before_state.version_head_id
            {
                return Err(PongError::Conflict("workspace revision is stale".into()));
            }
            workspace
        };
        Ok(PublishVersionResult {
            snapshot: SnapshotView::from(snapshot),
            version,
            workspace: WorkspaceView::from(workspace),
        })
    }

    fn validate_publish_lease(&self, request: &PublishVersionRequest) -> Result<(), PongError> {
        let lease = self
            .repository
            .metadata()
            .workspace_lease(&request.workspace_id)?
            .ok_or_else(|| PongError::Conflict("workspace lease is missing".into()))?;
        if request.lease.workspace_id != request.workspace_id
            || lease.agent_id.as_deref() != Some(request.lease.agent_id.as_str())
            || lease.epoch != request.lease.epoch
            || lease.expires_at_ms <= request.now_ms
        {
            return Err(PongError::Conflict(
                "workspace lease is stale or expired".into(),
            ));
        }
        Ok(())
    }

    fn validate_publish_parent(
        &self,
        workspace: &WorkspaceRecord,
        parent_version_id: Option<&str>,
    ) -> Result<(), PongError> {
        let Some(parent_version_id) = parent_version_id else {
            return Ok(());
        };
        let parent = self
            .repository
            .metadata()
            .version_record(parent_version_id)?
            .ok_or_else(|| PongError::NotFound("version parent does not exist".into()))?;
        if parent.workspace_id != workspace.workspace_id
            || parent.project_id != workspace.project_id
            || parent.environment_id != workspace.environment_id
        {
            return Err(PongError::Conflict(
                "version parent scope does not match workspace".into(),
            ));
        }
        Ok(())
    }

    fn publish_operation_for_request(
        &self,
        workspace: &WorkspaceRecord,
        request: &PublishVersionRequest,
    ) -> Result<Option<OperationRecord>, PongError> {
        let by_id = self
            .repository
            .metadata()
            .operation_record(&request.operation_id)?;
        let by_request = self.repository.metadata().operation_record_for_request(
            &workspace.project_id,
            &request.lease.agent_id,
            &request.request_id,
        )?;
        match (by_id, by_request) {
            (Some(by_id), Some(by_request)) if by_id.operation_id == by_request.operation_id => {
                Ok(Some(by_id))
            }
            (Some(_), Some(_)) => Err(PongError::Integrity(
                "Version publication operation indexes disagree".into(),
            )),
            (None, Some(_)) => Err(PongError::IdempotencyKeyReuse(request.request_id.clone())),
            (Some(_), None) => Err(PongError::Conflict(
                "operation identity is already used by another request".into(),
            )),
            (None, None) => Ok(None),
        }
    }

    pub fn operation(&self, operation_id: &str) -> Result<Option<OperationRecord>, PongError> {
        self.repository.metadata().operation_record(operation_id)
    }

    pub fn operation_for_request(
        &self,
        project_id: &str,
        agent_id: &str,
        request_id: &str,
    ) -> Result<Option<OperationRecord>, PongError> {
        self.repository
            .metadata()
            .operation_record_for_request(project_id, agent_id, request_id)
    }

    pub fn execution_for_operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<ExecutionOperationRecord>, PongError> {
        self.repository
            .metadata()
            .execution_for_operation(operation_id)
    }

    pub fn start_execution_operation(
        &mut self,
        execution_id: &str,
        operation: OperationEnvelope,
        associated_at: &str,
    ) -> Result<OperationRecord, PongError> {
        let operation_id = operation.operation_id.clone();
        let operation = self.repository.metadata_mut().start_operation(operation)?;
        self.repository
            .metadata_mut()
            .attach_operation_to_execution(execution_id, &operation_id, associated_at)?;
        Ok(operation)
    }

    pub fn finish_execution_operation(
        &mut self,
        execution_id: &str,
        operation_id: &str,
        outcome: OperationOutcome,
    ) -> Result<OperationRecord, PongError> {
        if self
            .repository
            .metadata()
            .execution_operation(execution_id, operation_id)?
            .is_none()
        {
            return Err(PongError::NotFound(
                "Execution Operation association does not exist".into(),
            ));
        }
        self.repository
            .metadata_mut()
            .finish_operation(operation_id, outcome)
    }

    pub fn attach_operation_to_execution(
        &mut self,
        execution_id: &str,
        operation_id: &str,
        created_at: &str,
    ) -> Result<ExecutionOperationRecord, PongError> {
        self.repository
            .metadata_mut()
            .attach_operation_to_execution(execution_id, operation_id, created_at)
    }

    pub fn operations_for_execution(
        &self,
        execution_id: &str,
    ) -> Result<Vec<OperationRecord>, PongError> {
        self.repository
            .metadata()
            .operations_for_execution(execution_id)?
            .into_iter()
            .map(|ownership| {
                self.repository
                    .metadata()
                    .operation_record(&ownership.operation_id)?
                    .ok_or_else(|| {
                        PongError::Integrity(
                            "execution references a missing durable Operation".into(),
                        )
                    })
            })
            .collect()
    }

    pub fn create_checkpoint(
        &mut self,
        request: CheckpointCreation,
    ) -> Result<CheckpointRecord, PongError> {
        self.repository.metadata_mut().create_checkpoint(&request)
    }

    pub fn checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRecord>, PongError> {
        self.repository.metadata().checkpoint(checkpoint_id)
    }

    pub fn list_checkpoints(&self, task_id: &str) -> Result<Vec<CheckpointRecord>, PongError> {
        self.repository.metadata().list_checkpoints(task_id)
    }

    pub fn create_handoff(&mut self, request: HandoffCreation) -> Result<HandoffRecord, PongError> {
        self.repository.metadata_mut().create_handoff(&request)
    }

    pub fn handoff(&self, handoff_id: &str) -> Result<Option<HandoffRecord>, PongError> {
        self.repository.metadata().handoff(handoff_id)
    }

    pub fn list_handoffs(&self, task_id: &str) -> Result<Vec<HandoffRecord>, PongError> {
        self.repository.metadata().list_handoffs(task_id)
    }

    pub fn resume_from_checkpoint(
        &mut self,
        request: ResumeCreation,
    ) -> Result<ResumeRecord, PongError> {
        self.repository
            .metadata_mut()
            .resume_from_checkpoint(&request)
    }

    pub fn resume_from_version(
        &mut self,
        request: ResumeCreation,
    ) -> Result<ResumeRecord, PongError> {
        self.repository.metadata_mut().resume_from_version(&request)
    }

    pub fn resume_record(&self, execution_id: &str) -> Result<Option<ResumeRecord>, PongError> {
        self.repository.metadata().resume_record(execution_id)
    }

    pub fn materialize_from_version(
        &mut self,
        target_workspace_id: &str,
        source_version_id: &str,
        lease: &LeaseToken,
        expected_revision: i64,
        now_ms: i64,
        now: &str,
    ) -> Result<SnapshotView, PongError> {
        let snapshot = WorkspaceManager::new(self.repository, self.redactor.clone())
            .materialize_from_version(
                target_workspace_id,
                source_version_id,
                lease,
                expected_revision,
                now_ms,
                now,
            )?;
        let record = self
            .repository
            .metadata()
            .snapshot_record(&snapshot.snapshot_id)?
            .ok_or_else(|| {
                PongError::Integrity("materialized snapshot metadata is missing".into())
            })?;
        Ok(SnapshotView::from(record))
    }

    pub fn restore_from_version(
        &mut self,
        target_workspace_id: &str,
        source_version_id: &str,
        lease: &LeaseToken,
        expected_revision: i64,
        now_ms: i64,
        now: &str,
    ) -> Result<SnapshotView, PongError> {
        let snapshot = WorkspaceManager::new(self.repository, self.redactor.clone())
            .restore_from_version(
                target_workspace_id,
                source_version_id,
                lease,
                expected_revision,
                now_ms,
                now,
            )?;
        let record = self
            .repository
            .metadata()
            .snapshot_record(&snapshot.snapshot_id)?
            .ok_or_else(|| PongError::Integrity("restored snapshot metadata is missing".into()))?;
        Ok(SnapshotView::from(record))
    }

    pub fn diff_against_version(
        &mut self,
        target_workspace_id: &str,
        source_version_id: &str,
    ) -> Result<SnapshotDiff, PongError> {
        WorkspaceManager::new(self.repository, self.redactor.clone())
            .diff_workspace_against_version(target_workspace_id, source_version_id)
    }

    pub fn rollback(
        &mut self,
        request: &crate::metadata::RollbackCreation,
    ) -> Result<crate::metadata::RollbackRecord, PongError> {
        WorkspaceManager::new(self.repository, self.redactor.clone()).rollback_local(request)
    }

    pub fn lease(&self, workspace_id: &str, now_ms: i64) -> Result<Option<LeaseView>, PongError> {
        self.repository
            .metadata()
            .workspace_lease(workspace_id)
            .map(|lease| lease.map(|lease| LeaseView::from_record(lease, now_ms)))
    }

    pub fn state(&self, execution_id: &str, now_ms: i64) -> Result<StateView, PongError> {
        let execution = self
            .repository
            .metadata()
            .execution(execution_id)?
            .ok_or_else(|| PongError::NotFound("execution does not exist".into()))?;
        let task = self
            .repository
            .metadata()
            .task(&execution.task_id)?
            .ok_or_else(|| PongError::Integrity("execution task is missing".into()))?;
        let agent = self
            .repository
            .metadata()
            .agent_identity(&execution.agent_id)?
            .ok_or_else(|| PongError::Integrity("execution agent is missing".into()))?;
        let workspace = execution
            .workspace_id
            .as_deref()
            .map(|id| self.workspace(id))
            .transpose()?
            .flatten();
        let lease = execution
            .workspace_id
            .as_deref()
            .map(|id| self.lease(id, now_ms))
            .transpose()?
            .flatten();
        let base_version = execution
            .base_version_id
            .as_deref()
            .map(|id| self.repository.metadata().version_record(id))
            .transpose()?
            .flatten();
        let current_version = execution
            .current_version_id
            .as_deref()
            .map(|id| self.repository.metadata().version_record(id))
            .transpose()?
            .flatten();
        Ok(StateView {
            agent,
            task: task.clone(),
            execution,
            workspace,
            lease,
            base_version,
            current_version,
            checkpoints: self.repository.metadata().list_checkpoints(&task.task_id)?,
            handoffs: self.repository.metadata().list_handoffs(&task.task_id)?,
            resume: self.repository.metadata().resume_record(execution_id)?,
        })
    }
}

impl From<WorkspaceRecord> for WorkspaceView {
    fn from(record: WorkspaceRecord) -> Self {
        Self {
            workspace_id: record.workspace_id,
            project_id: record.project_id,
            driver: record.driver,
            locator: record.locator,
            branch_ref: record.branch_ref,
            head: record.head,
            version_head_id: record.version_head_id,
            environment_id: record.environment_id,
            status: record.status,
            revision: record.revision,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl LeaseView {
    fn from_record(record: crate::metadata::LeaseRecord, now_ms: i64) -> Self {
        let active = record.agent_id.is_some() && record.expires_at_ms > now_ms;
        Self {
            workspace_id: record.workspace_id,
            agent_id: record.agent_id,
            epoch: record.epoch,
            expires_at_ms: record.expires_at_ms,
            acquired_at_ms: record.acquired_at_ms,
            active,
        }
    }
}

impl From<SnapshotRecord> for SnapshotView {
    fn from(record: SnapshotRecord) -> Self {
        Self {
            snapshot_id: record.snapshot_id,
            root_digest: record.root_digest,
            workspace_id: record.workspace_id,
            project_id: record.project_id,
            environment_id: record.environment_id,
            file_count: record.file_count,
            total_bytes: record.total_bytes,
            created_at: record.created_at,
        }
    }
}

fn validate_publish_request(request: &PublishVersionRequest) -> Result<(), PongError> {
    for (value, label) in [
        (request.workspace_id.as_str(), "workspace id"),
        (request.lease.workspace_id.as_str(), "lease workspace id"),
        (request.lease.agent_id.as_str(), "lease agent id"),
        (request.created_at.as_str(), "version created_at"),
        (request.operation_id.as_str(), "operation id"),
        (request.request_id.as_str(), "request id"),
    ] {
        if value.trim().is_empty() {
            return Err(PongError::InvalidInput(format!(
                "{label} must not be empty"
            )));
        }
    }
    for (value, label) in [
        (request.session_id.as_deref(), "session id"),
        (request.tool.as_deref(), "tool"),
        (request.parent_version_id.as_deref(), "parent version id"),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(PongError::InvalidInput(format!(
                "{label} must not be empty"
            )));
        }
    }
    if request.expected_workspace_revision < 0 {
        return Err(PongError::InvalidInput(
            "workspace revision must not be negative".into(),
        ));
    }
    Ok(())
}

fn publication_before_state(
    operation: &OperationRecord,
) -> Result<PublicationBeforeState, PongError> {
    if operation.action != "version.create" {
        return Err(PongError::Integrity(
            "Version publication operation has an invalid action".into(),
        ));
    }
    let value = operation.before_state.clone().ok_or_else(|| {
        PongError::Integrity("Version publication operation has no before state".into())
    })?;
    let state: PublicationBeforeState = serde_json::from_value(value).map_err(|error| {
        PongError::Integrity(format!(
            "Version publication before state is invalid: {error}"
        ))
    })?;
    if state.workspace_revision < 0 {
        return Err(PongError::Integrity(
            "Version publication before state has an invalid revision".into(),
        ));
    }
    Ok(state)
}

fn publication_snapshot_reference(operation: &OperationRecord) -> Result<&str, PongError> {
    let mut snapshots = operation
        .input_refs
        .iter()
        .filter(|reference| reference.kind == "snapshot");
    let snapshot = snapshots.next().ok_or_else(|| {
        PongError::Integrity("Version publication operation has no Snapshot input".into())
    })?;
    if snapshots.next().is_some() || snapshot.reference.trim().is_empty() {
        return Err(PongError::Integrity(
            "Version publication operation has invalid Snapshot inputs".into(),
        ));
    }
    Ok(&snapshot.reference)
}

fn publication_operation(
    request: &PublishVersionRequest,
    workspace: &WorkspaceRecord,
    snapshot: &SnapshotRecord,
    before_state: &PublicationBeforeState,
) -> Result<OperationEnvelope, PongError> {
    let expected_workspace_revision = before_state.workspace_revision;
    let mut input_refs = vec![OperationRef {
        kind: "snapshot".into(),
        reference: snapshot.snapshot_id.clone(),
        media_type: Some("application/vnd.pong.snapshot".into()),
    }];
    if let Some(parent_version_id) = request.parent_version_id.as_ref() {
        input_refs.push(OperationRef {
            kind: "version".into(),
            reference: parent_version_id.clone(),
            media_type: Some("application/vnd.pong.version".into()),
        });
    }
    let before_state = serde_json::to_value(before_state).map_err(|error| {
        PongError::Serialization(format!(
            "cannot encode Version publication before state: {error}"
        ))
    })?;
    Ok(OperationEnvelope {
        operation_id: request.operation_id.clone(),
        project_id: workspace.project_id.clone(),
        request_id: request.request_id.clone(),
        agent_id: request.lease.agent_id.clone(),
        session_id: request
            .session_id
            .clone()
            .unwrap_or_else(|| "agent-control".into()),
        workspace_id: Some(workspace.workspace_id.clone()),
        environment_id: workspace.environment_id.clone(),
        parent_operation_id: None,
        schema_version: crate::metadata::OPERATION_SCHEMA_VERSION.into(),
        started_at: request.created_at.clone(),
        tool: request
            .tool
            .clone()
            .unwrap_or_else(|| "agent-control".into()),
        action: "version.create".into(),
        input_refs,
        output_refs: Vec::new(),
        resource: Some(json!({
            "workspace_id": workspace.workspace_id,
            "expected_workspace_revision": expected_workspace_revision,
            "update_version_head": request.update_version_head,
        })),
        before_state: Some(before_state),
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: None,
    })
}

fn publication_snapshot_revision(
    before_state: &PublicationBeforeState,
    snapshot: &SnapshotRecord,
) -> Result<i64, PongError> {
    if before_state.workspace_head.as_deref() == Some(snapshot.root_digest.as_str()) {
        Ok(before_state.workspace_revision)
    } else {
        before_state
            .workspace_revision
            .checked_add(1)
            .ok_or_else(|| PongError::ResourceExhausted("workspace revision overflow".into()))
    }
}

fn validate_publication_stage(
    workspace: &WorkspaceRecord,
    snapshot: &SnapshotRecord,
    before_state: &PublicationBeforeState,
    snapshot_revision: i64,
    existing_version: Option<&VersionRecord>,
    update_version_head: bool,
) -> Result<(), PongError> {
    if snapshot.workspace_id != workspace.workspace_id
        || snapshot.project_id != workspace.project_id
        || snapshot.environment_id != workspace.environment_id
    {
        return Err(PongError::Integrity(
            "Version publication Snapshot scope is inconsistent".into(),
        ));
    }
    if workspace.head.as_deref() != Some(snapshot.root_digest.as_str()) {
        return Err(PongError::Conflict(
            "workspace head changed during Version publication".into(),
        ));
    }
    if let Some(version) = existing_version {
        if version.workspace_id != workspace.workspace_id
            || version.project_id != workspace.project_id
            || version.snapshot_id != snapshot.snapshot_id
            || version.environment_id != workspace.environment_id
        {
            return Err(PongError::Integrity(
                "durable Version publication result is inconsistent".into(),
            ));
        }
        if update_version_head
            && workspace.version_head_id.as_deref() == Some(version.version_id.as_str())
        {
            let completed_revision = snapshot_revision.checked_add(1).ok_or_else(|| {
                PongError::ResourceExhausted("workspace revision overflow".into())
            })?;
            if workspace.revision == completed_revision {
                return Ok(());
            }
        }
    }
    if workspace.revision != snapshot_revision
        || workspace.version_head_id != before_state.version_head_id
    {
        return Err(PongError::Conflict(
            "workspace revision is stale during Version publication".into(),
        ));
    }
    Ok(())
}
