//! Opt-in evidence for FI-13/FI-14 against real host filesystem failures.
//!
//! These tests are ignored by the ordinary quality gate because they mutate a
//! temporary file ACL or deliberately exhaust a dedicated filesystem. See the
//! ignore messages for the required invocation contract. Synthetic Pong
//! failpoints are intentionally not used anywhere in this file.

use pong_core::Repository;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use tempfile::{tempdir_in, TempDir};

use fs2::FileExt;
#[cfg(target_os = "linux")]
use fs2::{available_space, total_space};
#[cfg(target_os = "linux")]
use pong_core::cas::digest_for;
#[cfg(target_os = "linux")]
use pong_core::metadata::{IdempotencyKey, NewEvent};
#[cfg(target_os = "linux")]
use serde_json::json;
#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(any(target_os = "linux", windows))]
use std::process::Command;

const HOST_FAULT_ROOT: &str = "PONG_HOST_FAULT_ROOT";

/// A serialized, isolated case on a caller-provided scratch filesystem.
///
/// The lock is an OS-level advisory lock rather than a process-global mutex:
/// it also serializes separate `cargo test` processes and is released by the
/// kernel if a test process is killed. The lock file itself is the only entry
/// permitted in the parent scratch directory between cases.
struct HostFaultCase {
    _lock: File,
    directory: TempDir,
}

impl HostFaultCase {
    fn path(&self) -> &Path {
        self.directory.path()
    }
}

fn empty_host_fault_root() -> HostFaultCase {
    let parent = env::var_os(HOST_FAULT_ROOT)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{HOST_FAULT_ROOT} must name a dedicated scratch directory"));
    fs::create_dir_all(&parent).expect("create host-fault scratch directory");
    let parent = fs::canonicalize(parent).expect("canonical host-fault scratch directory");
    let lock_path = parent.join(".pong-host-fault.lock");
    if let Ok(metadata) = fs::symlink_metadata(&lock_path) {
        assert!(
            metadata.file_type().is_file(),
            "host-fault lock path must be a regular file, not a link or directory"
        );
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .expect("open host-fault lock file");
    FileExt::lock_exclusive(&lock).expect("lock host-fault scratch directory");

    for entry in fs::read_dir(&parent).expect("read host-fault scratch directory") {
        let entry = entry.expect("read host-fault scratch entry");
        assert_eq!(
            entry.path(),
            lock_path,
            "{HOST_FAULT_ROOT} may contain only .pong-host-fault.lock before a run"
        );
    }

    let directory = tempfile::Builder::new()
        .prefix("pong-host-fault-case-")
        .tempdir_in(parent)
        .expect("create isolated host-fault case directory");
    HostFaultCase {
        _lock: lock,
        directory,
    }
}

fn repository_in(root: &Path) -> TempDir {
    tempdir_in(root).expect("create isolated host-fault repository")
}

#[cfg(windows)]
struct WindowsDenyAce {
    path: PathBuf,
    identity: String,
    active: bool,
}

#[cfg(windows)]
impl WindowsDenyAce {
    fn read(path: impl AsRef<Path>) -> Self {
        let identity_output = Command::new("whoami")
            .output()
            .expect("run whoami for ACL identity");
        assert!(identity_output.status.success(), "whoami must succeed");
        let identity = String::from_utf8(identity_output.stdout)
            .expect("whoami output is UTF-8")
            .trim()
            .to_owned();
        assert!(!identity.is_empty(), "whoami returned an empty identity");

        let path = path.as_ref().to_path_buf();
        let output = Command::new("icacls")
            .arg(&path)
            .arg("/deny")
            .arg(format!("{identity}:(R)"))
            .output()
            .expect("run icacls deny");
        assert!(
            output.status.success(),
            "icacls deny failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Self {
            path,
            identity,
            active: true,
        }
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        let output = Command::new("icacls")
            .arg(&self.path)
            .arg("/remove:d")
            .arg(&self.identity)
            .output()
            .expect("run icacls restore");
        assert!(
            output.status.success(),
            "icacls restore failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        self.active = false;
    }
}

#[cfg(windows)]
impl Drop for WindowsDenyAce {
    fn drop(&mut self) {
        if self.active {
            let _ = Command::new("icacls")
                .arg(&self.path)
                .arg("/remove:d")
                .arg(&self.identity)
                .output();
        }
    }
}

#[cfg(windows)]
#[test]
#[ignore = "FI-13 host evidence: set PONG_HOST_FAULT_ROOT to an empty NTFS scratch directory"]
fn fi_13_real_windows_acl_read_revocation_fails_closed_and_recovers() {
    let scratch = empty_host_fault_root();
    let directory = repository_in(scratch.path());
    let mut repository = Repository::init(directory.path()).expect("initialize repository");
    let committed = repository
        .cas()
        .put("snapshot/v1", b"committed-before-acl-revocation")
        .expect("publish committed object");
    repository
        .metadata_mut()
        .compare_and_swap_ref(
            "refs/heads/main",
            None,
            &committed.to_hex(),
            "2026-08-24T00:00:00Z",
        )
        .expect("publish committed ref");
    let metadata_path = repository.active_metadata_path();
    drop(repository);

    let mut deny = WindowsDenyAce::read(&metadata_path);
    let kernel_error = fs::read(&metadata_path).expect_err("the host ACL must deny a real read");
    assert_eq!(kernel_error.kind(), std::io::ErrorKind::PermissionDenied);
    let open_error = Repository::open(directory.path()).expect_err("startup must fail closed");
    let observed_code = open_error.code();
    println!(
        "FI-13 platform=windows filesystem=NTFS kernel_error={:?} pong_code={observed_code}",
        kernel_error.raw_os_error()
    );

    deny.restore();
    let reopened = Repository::open(directory.path()).expect("cold reopen after ACL restoration");
    assert_eq!(
        reopened
            .metadata()
            .get_ref("refs/heads/main")
            .expect("read committed ref"),
        Some(committed.to_hex())
    );
    assert_eq!(
        reopened
            .cas()
            .get("snapshot/v1", committed)
            .expect("read committed object"),
        b"committed-before-acl-revocation"
    );
    assert!(
        matches!(observed_code, "PERMISSION_DENIED" | "RECOVERY_REQUIRED"),
        "FI-13 requires a stable security/recovery status, got {observed_code}"
    );
}

#[cfg(target_os = "linux")]
fn filesystem_type(path: &Path) -> String {
    let output = Command::new("stat")
        .args(["-f", "-c", "%T"])
        .arg(path)
        .output()
        .expect("run stat for filesystem type");
    assert!(
        output.status.success(),
        "stat filesystem query must succeed"
    );
    String::from_utf8(output.stdout)
        .expect("stat output is UTF-8")
        .trim()
        .to_owned()
}

#[cfg(target_os = "linux")]
struct FullFilesystem {
    _filler: tempfile::NamedTempFile,
    initial_available: u64,
    bytes_written: u64,
    kernel_error: i32,
}

#[cfg(target_os = "linux")]
impl FullFilesystem {
    fn fill(root: &Path) -> Self {
        const MAX_SAFE_CAPACITY: u64 = 64 * 1024 * 1024;
        const BLOCK_SIZE: usize = 64 * 1024;

        let filesystem = filesystem_type(root);
        assert_eq!(
            filesystem, "tmpfs",
            "FI-14 must run on a disposable tmpfs, not a host data volume"
        );
        let capacity = total_space(root).expect("query bounded filesystem capacity");
        assert!(
            capacity <= MAX_SAFE_CAPACITY,
            "refusing to exhaust a filesystem larger than {MAX_SAFE_CAPACITY} bytes"
        );
        let initial_available = available_space(root).expect("query initial available space");
        assert!(
            initial_available >= 1024 * 1024,
            "bounded filesystem needs at least 1 MiB before the schedule"
        );

        let mut filler = tempfile::Builder::new()
            .prefix(".pong-fi14-filler-")
            .tempfile_in(root)
            .expect("create filler on bounded filesystem");
        let block = vec![0xa5; BLOCK_SIZE];
        let mut bytes_written = 0_u64;
        let kernel_error = loop {
            match filler.as_file_mut().write_all(&block) {
                Ok(()) => bytes_written += BLOCK_SIZE as u64,
                Err(error) => {
                    let raw = error
                        .raw_os_error()
                        .expect("ENOSPC must expose a host OS error");
                    assert_eq!(raw, 28, "filler must stop on real Linux ENOSPC");
                    break raw;
                }
            }
            assert!(
                bytes_written <= initial_available + BLOCK_SIZE as u64,
                "filler exceeded the bounded filesystem's reported free space"
            );
        };
        assert_eq!(
            available_space(root).expect("query exhausted filesystem"),
            0,
            "the schedule did not fully exhaust the bounded filesystem"
        );
        Self {
            _filler: filler,
            initial_available,
            bytes_written,
            kernel_error,
        }
    }

    fn print_evidence(&self, row: &str, observed_code: &str) {
        println!(
            "{row} platform=linux filesystem=tmpfs initial_available={} bytes_written={} kernel_error={} pong_code={observed_code}",
            self.initial_available, self.bytes_written, self.kernel_error
        );
    }
}

#[cfg(target_os = "linux")]
fn assert_resource_exhausted(observed_code: &str) {
    assert!(
        matches!(observed_code, "RESOURCE_EXHAUSTED" | "RECOVERY_REQUIRED"),
        "FI-14 requires a stable resource/recovery status, got {observed_code}"
    );
}

#[cfg(target_os = "linux")]
fn event(id: &str, payload_bytes: usize) -> NewEvent {
    NewEvent {
        event_id: id.into(),
        project_id: "project-fi14".into(),
        stream_id: "project:fi14".into(),
        schema_version: "0.1".into(),
        actor_id: Some("agent-fi14".into()),
        request_id: Some(format!("request-{id}")),
        occurred_at: "2026-08-24T00:00:00Z".into(),
        payload: json!({"bytes": "x".repeat(payload_bytes)}),
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "FI-14 host evidence: run on an empty <=64MiB tmpfs via PONG_HOST_FAULT_ROOT"]
fn fi_14_real_enospc_during_cas_write_preserves_committed_history() {
    let scratch = empty_host_fault_root();
    let directory = repository_in(scratch.path());
    let repository = Repository::init(directory.path()).expect("initialize repository");
    let committed = repository
        .cas()
        .put("snapshot/v1", b"committed-before-enospc")
        .expect("publish committed object");
    let rejected_bytes = vec![0x5a; 256 * 1024];
    let rejected_digest = digest_for("snapshot/v1", &rejected_bytes);

    let full = FullFilesystem::fill(scratch.path());
    assert_eq!(
        repository
            .cas()
            .get("snapshot/v1", committed)
            .expect("committed history remains readable while full"),
        b"committed-before-enospc"
    );
    let mutation = repository.cas().put("snapshot/v1", &rejected_bytes);
    let observed_code = mutation
        .as_ref()
        .err()
        .map(|error| error.code())
        .unwrap_or("FALSE_SUCCESS");
    full.print_evidence("FI-14:cas", observed_code);
    assert_eq!(
        repository
            .cas()
            .get("snapshot/v1", committed)
            .expect("committed history remains readable after rejected write"),
        b"committed-before-enospc"
    );
    drop(repository);
    drop(full);

    let reopened = Repository::open(directory.path()).expect("cold reopen after freeing space");
    assert_eq!(
        reopened
            .cas()
            .get("snapshot/v1", committed)
            .expect("committed object survives cold reopen"),
        b"committed-before-enospc"
    );
    assert!(!reopened.cas().exists(rejected_digest));
    assert_eq!(
        fs::read_dir(reopened.layout().staging_dir())
            .expect("read CAS staging directory")
            .count(),
        0,
        "a real ENOSPC must not leave partial staging bytes"
    );
    assert_resource_exhausted(observed_code);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "FI-14 host evidence: run on an empty <=64MiB tmpfs via PONG_HOST_FAULT_ROOT"]
fn fi_14_real_enospc_during_metadata_write_preserves_committed_history() {
    let scratch = empty_host_fault_root();
    let directory = repository_in(scratch.path());
    let mut repository = Repository::init(directory.path()).expect("initialize repository");
    repository
        .metadata_mut()
        .append_event(event("committed", 0))
        .expect("append committed event");

    let full = FullFilesystem::fill(scratch.path());
    assert_eq!(
        repository
            .metadata()
            .list_events("project:fi14")
            .expect("committed event remains readable while full")
            .len(),
        1
    );
    let mutation = repository
        .metadata_mut()
        .append_event(event("rejected", 512 * 1024));
    let observed_code = mutation
        .as_ref()
        .err()
        .map(|error| error.code())
        .unwrap_or("FALSE_SUCCESS");
    full.print_evidence("FI-14:metadata", observed_code);
    assert_eq!(
        repository
            .metadata()
            .list_events("project:fi14")
            .expect("committed event remains readable after rejected write")
            .len(),
        1
    );
    drop(repository);
    drop(full);

    let reopened = Repository::open(directory.path()).expect("cold reopen after freeing space");
    let events = reopened
        .metadata()
        .list_events("project:fi14")
        .expect("read events after cold reopen");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_id, "committed");
    assert_resource_exhausted(observed_code);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "FI-14 host evidence: run on an empty <=64MiB tmpfs via PONG_HOST_FAULT_ROOT"]
fn fi_14_real_enospc_during_journal_write_preserves_committed_history() {
    let scratch = empty_host_fault_root();
    let directory = repository_in(scratch.path());
    let mut repository = Repository::init(directory.path()).expect("initialize repository");
    repository
        .metadata_mut()
        .record_intent(
            "project-fi14",
            "agent-fi14",
            "request-committed",
            "operation-committed",
            &json!({"status": "committed"}),
            "2026-08-24T00:00:00Z",
        )
        .expect("append committed intent");
    let committed_key = IdempotencyKey {
        project_id: "project-fi14".into(),
        actor_id: "agent-fi14".into(),
        request_id: "request-committed".into(),
    };
    let rejected_key = IdempotencyKey {
        project_id: "project-fi14".into(),
        actor_id: "agent-fi14".into(),
        request_id: "request-rejected".into(),
    };

    let full = FullFilesystem::fill(scratch.path());
    assert!(repository
        .metadata()
        .operation(&committed_key)
        .expect("committed journal remains readable while full")
        .is_some());
    let mutation = repository.metadata_mut().record_intent(
        "project-fi14",
        "agent-fi14",
        "request-rejected",
        "operation-rejected",
        &json!({"bytes": "x".repeat(512 * 1024)}),
        "2026-08-24T00:00:01Z",
    );
    let observed_code = mutation
        .as_ref()
        .err()
        .map(|error| error.code())
        .unwrap_or("FALSE_SUCCESS");
    full.print_evidence("FI-14:journal", observed_code);
    assert!(repository
        .metadata()
        .operation(&committed_key)
        .expect("committed journal remains readable after rejected write")
        .is_some());
    drop(repository);
    drop(full);

    let reopened = Repository::open(directory.path()).expect("cold reopen after freeing space");
    assert!(reopened
        .metadata()
        .operation(&committed_key)
        .expect("read committed journal after cold reopen")
        .is_some());
    assert!(
        reopened
            .metadata()
            .operation(&rejected_key)
            .expect("inspect rejected journal after cold reopen")
            .is_none(),
        "disk-full journal mutation must not be reported as committed"
    );
    assert_resource_exhausted(observed_code);
}
