//! Formal local Repository/bootstrap initialization entry point.

use pong_core::bootstrap;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: pong-bootstrap initialize PATH"
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let command = arguments.next().and_then(|value| value.into_string().ok());
    let path = arguments.next().map(PathBuf::from);
    if command.as_deref() != Some("initialize") || path.is_none() || arguments.next().is_some() {
        return Err(usage().into());
    }

    let path = path.expect("path checked above");
    let resolution =
        bootstrap::initialize(&path).map_err(|error| format!("{} ({})", error, error.code()))?;
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
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pong-bootstrap: {message}");
            ExitCode::from(2)
        }
    }
}
