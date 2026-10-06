//! Minimal provider-neutral recovery entry point.

use pong_core::recovery;
use pong_core::PongError;
use serde_json::json;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const ROOT_HELP: &str =
    "usage: pong recovery <inspect|resume> --repository PATH --checkpoint ID [--json]";
const RECOVERY_HELP: &str = "usage: pong recovery <inspect|resume> --repository PATH --checkpoint ID [--agent-id ID] [--json]";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help(&'static str),
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
        Some("recovery") => parse_recovery(args),
        Some(_) => Err(ROOT_HELP.into()),
    }
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
                Command::Inspect { json: true, .. } | Command::Resume { json: true, .. }
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
