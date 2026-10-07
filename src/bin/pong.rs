//! Minimal provider-neutral recovery entry point.

#[path = "../cli_finish.rs"]
mod cli_finish;
#[path = "../cli_start.rs"]
mod cli_start;
#[path = "../cli_status.rs"]
mod cli_status;
#[path = "../recovery.rs"]
mod recovery;

use pong_core::bootstrap;
use pong_core::PongError;
use serde_json::json;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const ROOT_HELP: &str = "usage: pong <init|start|finish|status|recovery> [OPTIONS]";
const INIT_HELP: &str = "usage: pong init [PROJECT_ROOT]";
const START_HELP: &str = "usage: pong start <GOAL> [--project-root PATH] [--from-project] [--json]";
const FINISH_HELP: &str = "usage: pong finish --execution-id ID [--project-root PATH] [--state completed|failed|interrupted] [--outcome TEXT] [--json]";
const STATUS_HELP: &str = "usage: pong status [--project-root PATH] [--json]";
const RECOVERY_HELP: &str = "usage: pong recovery <inspect|resume> --repository PATH --checkpoint ID [--agent-id ID] [--json]";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help(&'static str),
    Init {
        project_root: PathBuf,
    },
    Start {
        goal: String,
        project_root: Option<PathBuf>,
        from_project: bool,
        json: bool,
    },
    Finish {
        execution_id: String,
        project_root: Option<PathBuf>,
        state: String,
        outcome: Option<String>,
        json: bool,
    },
    Status {
        project_root: Option<PathBuf>,
        json: bool,
    },
    Inspect {
        repository: PathBuf,
        checkpoint: String,
        json: bool,
    },
    Resume {
        repository: PathBuf,
        checkpoint: String,
        agent_id: Option<String>,
        json: bool,
    },
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        None | Some("--help") | Some("-h") => Ok(Command::Help(ROOT_HELP)),
        Some("init") => parse_init(args),
        Some("start") => parse_start(args),
        Some("finish") => parse_finish(args),
        Some("status") => parse_status(args),
        Some("recovery") => parse_recovery(args),
        Some(_) => Err(ROOT_HELP.into()),
    }
}

fn parse_finish(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut execution_id = None;
    let mut project_root = None;
    let mut state = "completed".to_string();
    let mut outcome = None;
    let mut json = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--execution-id" => {
                execution_id = Some(
                    args.next()
                        .ok_or_else(|| "--execution-id requires an id".to_string())?,
                )
            }
            "--project-root" => {
                project_root = Some(
                    args.next()
                        .ok_or_else(|| "--project-root requires a path".to_string())?
                        .into(),
                )
            }
            "--state" => {
                state = args
                    .next()
                    .ok_or_else(|| "--state requires a value".to_string())?
            }
            "--outcome" => {
                outcome = Some(
                    args.next()
                        .ok_or_else(|| "--outcome requires text".to_string())?,
                )
            }
            "--json" => json = true,
            "--help" | "-h" => return Ok(Command::Help(FINISH_HELP)),
            _ => return Err(FINISH_HELP.into()),
        }
    }
    Ok(Command::Finish {
        execution_id: execution_id.ok_or_else(|| "--execution-id is required".to_string())?,
        project_root,
        state,
        outcome,
        json,
    })
}

fn parse_start(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut goal = None;
    let mut project_root = None;
    let mut from_project = false;
    let mut json = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--project-root" => {
                project_root = Some(
                    args.next()
                        .ok_or_else(|| "--project-root requires a path".to_string())?
                        .into(),
                )
            }
            "--json" => json = true,
            "--from-project" => from_project = true,
            "--help" | "-h" => return Ok(Command::Help(START_HELP)),
            _ if goal.is_none() => goal = Some(argument),
            _ => return Err(START_HELP.into()),
        }
    }
    let goal = goal.ok_or_else(|| "start requires a task goal".to_string())?;
    if goal.trim().is_empty() {
        return Err("start requires a non-empty task goal".into());
    }
    Ok(Command::Start {
        goal,
        project_root,
        from_project,
        json,
    })
}

fn parse_init(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    match args.next() {
        Some(argument) if argument == "--help" || argument == "-h" => Ok(Command::Help(INIT_HELP)),
        Some(_) if args.next().is_some() => Err(INIT_HELP.into()),
        Some(path) => Ok(Command::Init {
            project_root: path.into(),
        }),
        None => Ok(Command::Init {
            project_root: env::current_dir().map_err(|error| error.to_string())?,
        }),
    }
}

fn parse_status(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut project_root = None;
    let mut json = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--project-root" => {
                project_root = Some(
                    args.next()
                        .ok_or_else(|| "--project-root requires a path".to_string())?
                        .into(),
                )
            }
            "--json" => json = true,
            "--help" | "-h" => return Ok(Command::Help(STATUS_HELP)),
            _ => return Err(STATUS_HELP.into()),
        }
    }
    Ok(Command::Status { project_root, json })
}

fn parse_recovery(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let Some(action) = args.next() else {
        return Ok(Command::Help(RECOVERY_HELP));
    };
    if action == "--help" || action == "-h" {
        return Ok(Command::Help(RECOVERY_HELP));
    }
    if action != "inspect" && action != "resume" {
        return Err(RECOVERY_HELP.into());
    }
    let mut repository = None;
    let mut checkpoint = None;
    let mut agent_id = None;
    let mut json = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--repository" => {
                repository = Some(
                    args.next()
                        .ok_or_else(|| "--repository requires a path".to_string())?
                        .into(),
                )
            }
            "--checkpoint" => {
                checkpoint = Some(
                    args.next()
                        .ok_or_else(|| "--checkpoint requires an id".to_string())?,
                )
            }
            "--agent-id" if action == "resume" => {
                agent_id = Some(
                    args.next()
                        .ok_or_else(|| "--agent-id requires an id".to_string())?,
                )
            }
            "--json" => json = true,
            "--help" | "-h" => return Ok(Command::Help(RECOVERY_HELP)),
            _ => return Err(RECOVERY_HELP.into()),
        }
    }
    let repository = repository.ok_or_else(|| "--repository is required".to_string())?;
    let checkpoint = checkpoint.ok_or_else(|| "--checkpoint is required".to_string())?;
    if action == "inspect" {
        Ok(Command::Inspect {
            repository,
            checkpoint,
            json,
        })
    } else {
        Ok(Command::Resume {
            repository,
            checkpoint,
            agent_id,
            json,
        })
    }
}

struct CliFailure {
    status: u8,
    code: &'static str,
    message: String,
}

fn run(command: Command) -> Result<(), CliFailure> {
    match command {
        Command::Help(help) => {
            println!("{help}");
            Ok(())
        }
        Command::Init { project_root } => cli_status::initialize(&project_root)
            .map(|resolution| {
                println!("status: ready");
                println!("project_root: {}", resolution.project_root.display());
                println!("repository_root: {}", resolution.repository_root.display());
                println!("workspace_root: {}", resolution.workspace_root.display());
                println!(
                    "bootstrap: {}",
                    bootstrap::BootstrapMetadata::path(&resolution.project_root).display()
                );
                println!(
                    "core_endpoint: {}",
                    resolution.core_endpoint.as_deref().unwrap_or("null")
                );
            })
            .map_err(|error| CliFailure {
                status: 2,
                code: error.code(),
                message: error.to_string(),
            }),
        Command::Start {
            goal,
            project_root,
            from_project,
            json,
        } => cli_start::start(project_root.as_deref(), &goal, from_project)
            .and_then(|value| {
                if json {
                    serde_json::to_string_pretty(&value)
                        .map(|output| println!("{output}"))
                        .map_err(|error| format!("start serialization failed: {error}"))
                } else {
                    println!("status: {}", value.status);
                    println!("project_root: {}", value.project_root);
                    println!("project_id: {}", value.project_id);
                    println!("task_id: {}", value.task_id);
                    println!("workspace_id: {}", value.workspace_id);
                    println!("execution_id: {}", value.execution_id);
                    println!("agent_id: {}", value.agent_id);
                    println!("workspace_root: {}", value.workspace_root);
                    if let Some(version_id) = value.initial_version_id {
                        println!("initial_version_id: {version_id}");
                    }
                    if let Some(snapshot_id) = value.initial_snapshot_id {
                        println!("initial_snapshot_id: {snapshot_id}");
                    }
                    Ok(())
                }
            })
            .map_err(|message| CliFailure {
                status: 1,
                code: "START_ERROR",
                message,
            }),
        Command::Finish {
            execution_id,
            project_root,
            state,
            outcome,
            json,
        } => cli_finish::finish(
            project_root.as_deref(),
            &execution_id,
            &state,
            outcome.as_deref(),
        )
        .and_then(|value| {
            if json {
                serde_json::to_string_pretty(&value)
                    .map(|output| println!("{output}"))
                    .map_err(|error| format!("finish serialization failed: {error}"))
            } else {
                println!("status: {}", value.status);
                println!("execution_id: {}", value.execution_id);
                println!("workspace_id: {}", value.workspace_id);
                println!("version_id: {}", value.version_id);
                println!("snapshot_id: {}", value.snapshot_id);
                println!("root_digest: {}", value.root_digest);
                println!("execution_state: {}", value.execution_state);
                Ok(())
            }
        })
        .map_err(|message| CliFailure {
            status: 1,
            code: "FINISH_ERROR",
            message,
        }),
        Command::Status { project_root, json } => cli_status::inspect(project_root.as_deref())
            .and_then(|value| {
                if json {
                    serde_json::to_string_pretty(&value)
                        .map(|output| println!("{output}"))
                        .map_err(|error| format!("status serialization failed: {error}"))
                } else {
                    println!("{}", format_human_status(&value));
                    Ok(())
                }
            })
            .map_err(|message| CliFailure {
                status: 1,
                code: "STATUS_ERROR",
                message,
            }),
        Command::Inspect {
            repository,
            checkpoint,
            json,
        } => recovery::inspect(&repository, &checkpoint)
            .and_then(|value| {
                if json {
                    println!("{}", recovery::as_json(&value)?);
                } else {
                    println!("{}", recovery::as_human_inspection(&value));
                }
                Ok(())
            })
            .map_err(classify),
        Command::Resume {
            repository,
            checkpoint,
            agent_id,
            json,
        } => recovery::resume(&repository, &checkpoint, agent_id.as_deref())
            .and_then(|value| {
                if json {
                    println!("{}", recovery::as_json(&value)?);
                } else {
                    println!("{}", recovery::as_human_resume(&value));
                }
                Ok(())
            })
            .map_err(classify),
    }
}

fn format_human_status(value: &cli_status::StatusReport) -> String {
    let mut lines = vec![
        format!("Project: {}", value.project_root),
        format!("Pong initialized: {}", value.initialized),
    ];
    if let Some(repository) = &value.repository {
        lines.push(format!("Repository: {}", repository.root));
        lines.push(format!("Repository format: {}", repository.format));
        lines.push(format!("Storage: {}", repository.storage));
    }
    if value.projects.is_empty() {
        lines.push("Projects: unavailable (no durable project state)".into());
    } else {
        for project in &value.projects {
            lines.push(format!("Project ID: {}", project.project_id));
            lines.push(format!("Workspaces: {}", project.workspaces.len()));
            for workspace in &project.workspaces {
                lines.push(format!(
                    "  Workspace {} [{}], Version: {}, Snapshot: {}",
                    workspace.id,
                    workspace.status,
                    workspace
                        .current_version_id
                        .as_deref()
                        .unwrap_or("unavailable"),
                    workspace.snapshot_id.as_deref().unwrap_or("unavailable")
                ));
            }
            lines.push(format!("Tasks: {}", project.tasks.len()));
            lines.push(format!(
                "Active executions: {}",
                project.active_executions.len()
            ));
            for execution in &project.active_executions {
                lines.push(format!(
                    "  Execution {} [{}], Task: {}",
                    execution.id, execution.state, execution.task_id
                ));
            }
            lines.push(format!(
                "Latest checkpoint: {}",
                project
                    .latest_checkpoint
                    .as_ref()
                    .map(|checkpoint| checkpoint.id.as_str())
                    .unwrap_or("unavailable")
            ));
            lines.push(format!(
                "Resumable checkpoints: {}",
                project.resumable_checkpoints.len()
            ));
            for checkpoint in &project.resumable_checkpoints {
                lines.push(format!(
                    "  Checkpoint {} [{}], Execution: {}",
                    checkpoint.id, checkpoint.reason, checkpoint.execution_id
                ));
            }
        }
    }
    if let Some(note) = &value.note {
        lines.push(format!("Note: {note}"));
    }
    lines.join("\n")
}

fn classify(error: PongError) -> CliFailure {
    let status = match error.code() {
        "INVALID_INPUT" => 2,
        "PERMISSION_DENIED" => 3,
        "NOT_FOUND" => 1,
        "INTEGRITY_ERROR" | "RECOVERY_REQUIRED" => 4,
        "CONFLICT" | "IDEMPOTENCY_KEY_REUSE" => 1,
        "IO_ERROR" | "STORAGE_ERROR" | "RESOURCE_EXHAUSTED" => 5,
        _ => 1,
    };
    CliFailure {
        status,
        code: error.code(),
        message: error.to_string(),
    }
}

fn main() -> ExitCode {
    match parse(env::args().skip(1)) {
        Ok(command) => {
            let json_output = matches!(
                &command,
                Command::Start { json: true, .. }
                    | Command::Finish { json: true, .. }
                    | Command::Inspect { json: true, .. }
                    | Command::Resume { json: true, .. }
            );
            match run(command) {
                Ok(()) => ExitCode::SUCCESS,
                Err(failure) => {
                    if json_output {
                        eprintln!(
                            "{}",
                            json!({"error": {"code": failure.code, "message": failure.message}})
                        );
                    } else {
                        eprintln!("pong: {} ({})", failure.message, failure.code);
                    }
                    ExitCode::from(failure.status)
                }
            }
        }
        Err(message) => {
            eprintln!("pong: {message}");
            ExitCode::from(2)
        }
    }
}
