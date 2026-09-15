//! Development HTTP adapter for External Agent Protocol v1.0.

use pong_core::protocol::{WorkspaceBindingError, WorkspaceBindingResolver};
use pong_core::{
    AgentProtocolCore, CredentialGrant, HttpRemoteServer, HttpServerConfig, Repository,
    StaticCredentialVerifier, DEFAULT_HTTP_MAX_BODY_BYTES, DEFAULT_HTTP_SESSION_TTL_MS,
};
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

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
    credentials_file: PathBuf,
    listen_addr: SocketAddr,
    allow_remote_bind: bool,
    max_body_bytes: usize,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut repository = None;
    let mut workspace_root = None;
    let mut credentials_file = None;
    let mut listen_addr = SocketAddr::from(([127, 0, 0, 1], 8743));
    let mut allow_remote_bind = false;
    let mut max_body_bytes = DEFAULT_HTTP_MAX_BODY_BYTES;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--repository") => repository = Some(required_path(&mut arguments)?),
            Some("--workspace-root") => workspace_root = Some(required_path(&mut arguments)?),
            Some("--credentials-file") => credentials_file = Some(required_path(&mut arguments)?),
            Some("--listen") => {
                let value = arguments
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .ok_or_else(usage)?;
                listen_addr = value.parse().map_err(|_| usage())?;
            }
            Some("--allow-remote-bind") => allow_remote_bind = true,
            Some("--max-body-bytes") => {
                let value = arguments
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .ok_or_else(usage)?;
                max_body_bytes = value.parse().map_err(|_| usage())?;
            }
            _ => return Err(usage()),
        }
    }
    Ok(Arguments {
        repository: repository.ok_or_else(usage)?,
        workspace_root: workspace_root.ok_or_else(usage)?,
        credentials_file: credentials_file.ok_or_else(usage)?,
        listen_addr,
        allow_remote_bind,
        max_body_bytes,
    })
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, String> {
    arguments.next().map(PathBuf::from).ok_or_else(usage)
}

fn usage() -> String {
    "usage: pong-agent-http --repository PATH --workspace-root PATH --credentials-file PATH [--listen IP:PORT] [--allow-remote-bind] [--max-body-bytes BYTES]".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFile {
    credentials: Vec<CredentialFileEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFileEntry {
    credential: String,
    principal_id: String,
    agent_ids: Vec<String>,
    expires_at_ms: Option<i64>,
}

fn load_credentials(path: &Path) -> Result<StaticCredentialVerifier, String> {
    let bytes = fs::read(path).map_err(|_| "credential source is unavailable".to_string())?;
    let file: CredentialFile =
        serde_json::from_slice(&bytes).map_err(|_| "credential source is invalid".to_string())?;
    let grants = file
        .credentials
        .into_iter()
        .map(|entry| CredentialGrant {
            credential: entry.credential,
            principal_id: entry.principal_id,
            agent_ids: entry.agent_ids,
            expires_at_ms: entry.expires_at_ms,
        })
        .collect();
    StaticCredentialVerifier::new(grants).map_err(|error| error.to_string())
}

fn run(arguments: Arguments) -> Result<(), String> {
    let verifier = Arc::new(load_credentials(&arguments.credentials_file)?);
    let bindings = LocalBindings::new(&arguments.workspace_root)?;
    let repository = Repository::open_as_core_owner(&arguments.repository).map_err(|error| {
        format!(
            "Pong repository Core ownership could not be acquired ({})",
            error.code()
        )
    })?;
    let dispatcher = Arc::new(AgentProtocolCore::new(repository, bindings));
    let server = HttpRemoteServer::start(
        HttpServerConfig {
            listen_addr: arguments.listen_addr,
            allow_non_loopback: arguments.allow_remote_bind,
            max_body_bytes: arguments.max_body_bytes,
            session_ttl_ms: DEFAULT_HTTP_SESSION_TTL_MS,
            worker_threads: pong_core::DEFAULT_HTTP_WORKER_THREADS,
        },
        verifier,
        dispatcher,
    )
    .map_err(|error| error.to_string())?;
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(
        &mut stdout,
        &json!({"status": "ready", "listen_addr": server.listen_addr().to_string()}),
    )
    .map_err(|_| "HTTP readiness could not be serialized".to_string())?;
    stdout
        .write_all(b"\n")
        .and_then(|_| stdout.flush())
        .map_err(|_| "HTTP readiness could not be written".to_string())?;

    for line in io::stdin().lock().lines() {
        let line = line.map_err(|_| "HTTP control input could not be read".to_string())?;
        if line.trim().eq_ignore_ascii_case("shutdown") {
            break;
        }
    }
    server.shutdown().map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    match parse_arguments().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pong-agent-http: {message}");
            ExitCode::from(2)
        }
    }
}
