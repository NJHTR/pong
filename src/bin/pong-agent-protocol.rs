//! Minimal local JSON Lines transport for the external Agent protocol.

use pong_core::protocol::{
    ExternalAgentProtocol, ProtocolRequest, ProtocolResponse, WorkspaceBindingError,
    WorkspaceBindingResolver,
};
use pong_core::{bootstrap, Repository};
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
    repository: Option<PathBuf>,
    workspace_root: Option<PathBuf>,
    project_root: Option<PathBuf>,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut repository = None;
    let mut workspace_root = None;
    let mut project_root = None;
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
            Some("--project-root") => {
                project_root = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or_else(|| "--project-root requires a path".to_string())?,
                );
            }
            _ => return Err(usage()),
        }
    }
    if project_root.is_some() && (repository.is_some() || workspace_root.is_some()) {
        return Err(usage());
    }
    if repository.is_some() != workspace_root.is_some() {
        return Err("--repository and --workspace-root must be provided together".into());
    }
    Ok(Arguments {
        repository,
        workspace_root,
        project_root,
    })
}

fn usage() -> String {
    "usage: pong-agent-protocol [--project-root PATH] | [--repository PATH --workspace-root PATH]"
        .into()
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
    let (repository_path, workspace_root) = match (
        arguments.repository,
        arguments.workspace_root,
        arguments.project_root,
    ) {
        (Some(repository), Some(workspace_root), None) => (repository, workspace_root),
        (None, None, project_root) => {
            let resolution = match project_root {
                Some(project_root) => bootstrap::discover(project_root),
                None => bootstrap::discover_from_cwd(),
            }
            .map_err(|error| format!("{} ({})", error, error.code()))?;
            (resolution.repository_root, resolution.workspace_root)
        }
        _ => return Err(usage()),
    };
    let mut repository = Repository::open_as_core_owner(&repository_path).map_err(|error| {
        format!(
            "Pong repository Core ownership could not be acquired ({})",
            error.code()
        )
    })?;
    let bindings = LocalBindings::new(&workspace_root)?;
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
