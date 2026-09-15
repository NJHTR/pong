//! Minimal local JSON Lines transport for the external Agent protocol.

use pong_core::protocol::{
    ExternalAgentProtocol, ProtocolRequest, ProtocolResponse, WorkspaceBindingError,
    WorkspaceBindingResolver,
};
use pong_core::Repository;
use serde_json::Value;
use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

struct LocalBindings {
    root: PathBuf,
}

impl LocalBindings {
    fn new(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|_| "workspace binding root is unavailable".to_string())?;
        if !root.is_dir() {
            return Err("workspace binding root is not a directory".into());
        }
        Ok(Self { root })
    }
}

impl WorkspaceBindingResolver for LocalBindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        if binding_ref.is_empty()
            || binding_ref == "."
            || binding_ref == ".."
            || !binding_ref
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(WorkspaceBindingError::Invalid);
        }
        Ok(self.root.join(binding_ref))
    }
}

struct Arguments {
    repository: PathBuf,
    workspace_root: PathBuf,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut repository = None;
    let mut workspace_root = None;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--repository") => {
                repository = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or_else(|| "--repository requires a path".to_string())?,
                );
            }
            Some("--workspace-root") => {
                workspace_root = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or_else(|| "--workspace-root requires a path".to_string())?,
                );
            }
            _ => {
                return Err(
                    "usage: pong-agent-protocol --repository PATH --workspace-root PATH".into(),
                )
            }
        }
    }
    Ok(Arguments {
        repository: repository.ok_or_else(|| "--repository is required".to_string())?,
        workspace_root: workspace_root.ok_or_else(|| "--workspace-root is required".to_string())?,
    })
}

fn host_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

fn request_id_from_invalid_json(line: &str) -> Option<String> {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| value.get("request_id")?.as_str().map(Into::into))
        .filter(|request_id: &String| !request_id.trim().is_empty())
}

fn run(arguments: Arguments) -> Result<(), String> {
    let mut repository =
        Repository::open_as_core_owner(&arguments.repository).map_err(|error| {
            format!(
                "Pong repository Core ownership could not be acquired ({})",
                error.code()
            )
        })?;
    let bindings = LocalBindings::new(&arguments.workspace_root)?;
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line.map_err(|_| "protocol input could not be read".to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<ProtocolRequest>(&line) {
            Ok(request) => ExternalAgentProtocol::new(&mut repository, &bindings)
                .handle(request, host_now_ms()),
            Err(_) => ProtocolResponse::invalid_envelope(request_id_from_invalid_json(&line)),
        };
        serde_json::to_writer(&mut stdout, &response)
            .map_err(|_| "protocol response could not be serialized".to_string())?;
        stdout
            .write_all(b"\n")
            .and_then(|_| stdout.flush())
            .map_err(|_| "protocol response could not be written".to_string())?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_arguments().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pong-agent-protocol: {message}");
            ExitCode::from(2)
        }
    }
}
