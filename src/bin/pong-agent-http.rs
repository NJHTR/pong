//! Development HTTP adapter for External Agent Protocol v1.0.

use pong_core::protocol::{WorkspaceBindingError, WorkspaceBindingResolver};
use pong_core::{
    AgentProtocolCore, CredentialGrant, HttpRemoteServer, HttpServerConfig, Repository,
    StaticCredentialVerifier, DEFAULT_HTTP_MAX_BODY_BYTES, DEFAULT_HTTP_MAX_RESPONSE_BYTES,
    DEFAULT_HTTP_RATE_LIMIT_REQUESTS, DEFAULT_HTTP_RATE_LIMIT_WINDOW_MS,
    DEFAULT_HTTP_SESSION_TTL_MS, DEFAULT_HTTP_WORKER_THREADS,
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
    max_response_bytes: usize,
    worker_threads: usize,
    rate_limit_requests: usize,
    rate_limit_window_ms: i64,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut repository = None;
    let mut workspace_root = None;
    let mut credentials_file = None;
    let mut listen_addr = SocketAddr::from(([127, 0, 0, 1], 8743));
    let mut allow_remote_bind = false;
    let mut max_body_bytes = DEFAULT_HTTP_MAX_BODY_BYTES;
    let mut max_response_bytes = DEFAULT_HTTP_MAX_RESPONSE_BYTES;
    let mut worker_threads = DEFAULT_HTTP_WORKER_THREADS;
    let mut rate_limit_requests = DEFAULT_HTTP_RATE_LIMIT_REQUESTS;
    let mut rate_limit_window_ms = DEFAULT_HTTP_RATE_LIMIT_WINDOW_MS;
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
            Some("--max-response-bytes") => {
                max_response_bytes = required_number(&mut arguments)?;
            }
            Some("--worker-threads") => {
                worker_threads = required_number(&mut arguments)?;
            }
            Some("--rate-limit-requests") => {
                rate_limit_requests = required_number(&mut arguments)?;
            }
            Some("--rate-limit-window-ms") => {
                rate_limit_window_ms = required_number(&mut arguments)?;
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
        max_response_bytes,
        worker_threads,
        rate_limit_requests,
        rate_limit_window_ms,
    })
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, String> {
    arguments.next().map(PathBuf::from).ok_or_else(usage)
}

fn required_number<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<T, String> {
    arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse().ok())
        .ok_or_else(usage)
}

fn usage() -> String {
    "usage: pong-agent-http --repository PATH --workspace-root PATH --credentials-file PATH [--listen IP:PORT] [--allow-remote-bind] [--max-body-bytes BYTES] [--max-response-bytes BYTES] [--worker-threads COUNT] [--rate-limit-requests COUNT] [--rate-limit-window-ms MILLISECONDS]".into()
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

const MAX_CREDENTIAL_FILE_BYTES: u64 = 1024 * 1024;

fn load_credentials(path: &Path) -> Result<Vec<CredentialGrant>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "credential source is unavailable".to_string())?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_CREDENTIAL_FILE_BYTES
    {
        return Err("credential source is invalid".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("credential source permissions are too broad".into());
        }
    }
    let bytes = fs::read(path).map_err(|_| "credential source is unavailable".to_string())?;
    let grants = file_grants_from_bytes(&bytes)?;
    StaticCredentialVerifier::new(grants.clone())
        .map_err(|error| error.to_string())
        .map(|_| ())?;
    Ok(grants)
}

fn file_grants_from_bytes(bytes: &[u8]) -> Result<Vec<CredentialGrant>, String> {
    let file: CredentialFile =
        serde_json::from_slice(bytes).map_err(|_| "credential source is invalid".to_string())?;
    Ok(file
        .credentials
        .into_iter()
        .map(|entry| CredentialGrant {
            credential: entry.credential,
            principal_id: entry.principal_id,
            agent_ids: entry.agent_ids,
            expires_at_ms: entry.expires_at_ms,
        })
        .collect())
}

fn run(arguments: Arguments) -> Result<(), String> {
    let verifier = Arc::new(
        StaticCredentialVerifier::new(load_credentials(&arguments.credentials_file)?)
            .map_err(|error| error.to_string())?,
    );
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
            worker_threads: arguments.worker_threads,
            max_response_bytes: arguments.max_response_bytes,
            rate_limit_requests: arguments.rate_limit_requests,
            rate_limit_window_ms: arguments.rate_limit_window_ms,
        },
        Arc::clone(&verifier) as Arc<dyn pong_core::CredentialVerifier>,
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
        if line.trim().eq_ignore_ascii_case("reload-credentials") {
            let status = match load_credentials(&arguments.credentials_file).and_then(|grants| {
                verifier
                    .replace_grants(grants)
                    .map_err(|error| error.to_string())
            }) {
                Ok(()) => json!({"status": "credentials_reloaded"}),
                Err(_) => json!({"status": "error", "code": "CREDENTIAL_RELOAD_FAILED"}),
            };
            serde_json::to_writer(&mut stdout, &status)
                .map_err(|_| "HTTP control response could not be written".to_string())?;
            stdout
                .write_all(b"\n")
                .and_then(|_| stdout.flush())
                .map_err(|_| "HTTP control response could not be written".to_string())?;
        }
        if line.trim().eq_ignore_ascii_case("metrics") {
            let metrics = serde_json::to_value(server.metrics())
                .map_err(|_| "HTTP metrics could not be serialized".to_string())?;
            serde_json::to_writer(&mut stdout, &metrics)
                .map_err(|_| "HTTP control response could not be written".to_string())?;
            stdout
                .write_all(b"\n")
                .and_then(|_| stdout.flush())
                .map_err(|_| "HTTP control response could not be written".to_string())?;
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
