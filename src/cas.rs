//! Filesystem content-addressed storage for immutable Pong objects.
//!
//! Publication uses a fsynced staging file and an atomic no-replace hard-link
//! into the digest path. A hard-link is used instead of an unconditional
//! rename, because POSIX rename replaces an existing destination and would
//! violate CAS immutability under a concurrent writer. Both paths live under
//! the same repository, so link publication is atomic and the staging entry is
//! removed only after publication succeeds.

use crate::error::PongError;
use crate::redaction::Redactor;
use sha2::{Digest as ShaDigest, Sha256};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use std::time::{SystemTime, UNIX_EPOCH};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Durable boundaries at which the CAS test harness may inject an I/O fault.
///
/// The hook is deliberately explicit and instance-scoped.  Production
/// constructors install disabled [`CasFailpoints`], so no environment
/// variable or process-global switch can alter the default persistence path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CasFailPoint {
    /// Before writing and syncing a newly-created staging object.
    StagingWrite,
    /// Immediately before the atomic no-replace hard-link publication.
    Publication,
    /// Before syncing a directory after a namespace change.
    DirectorySync,
    /// Before scanning existing CAS bytes during startup.
    StartupScan,
}

impl CasFailPoint {
    fn label(self) -> &'static str {
        match self {
            Self::StagingWrite => "cas_staging_write",
            Self::Publication => "cas_publication",
            Self::DirectorySync => "cas_directory_sync",
            Self::StartupScan => "cas_startup_scan",
        }
    }
}

/// Result of a deterministic CAS fault-injection decision.
///
/// `ShortWrite(n)` writes at most the first `n` bytes and then returns an I/O
/// error.  It is meaningful at [`CasFailPoint::StagingWrite`]; at other points
/// it is treated as a regular injected failure. `QuotaExhausted(n)` has the
/// same prefix-write behavior but records a quota-specific fault reason.
/// `PermissionDenied` models an ACL/permission failure without changing host
/// permissions, which keeps Windows tests deterministic. The action is
/// intentionally small and data-free so test plans cannot accidentally persist
/// secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CasFaultAction {
    /// Continue with the real filesystem operation.
    Continue,
    /// Return an injected fault without performing the operation.
    Fail,
    /// Write a prefix, then return an injected short-write fault.
    ShortWrite(usize),
    /// Return a deterministic permission-denied fault without touching ACLs.
    PermissionDenied,
    /// Write a prefix, then return a deterministic quota-exhausted fault.
    QuotaExhausted(usize),
}

/// One-shot fault configuration owned by a [`Cas`] instance.
///
/// The default configuration is disabled.  Arming a point consumes it on the
/// first matching boundary, which lets a test retry the operation and observe
/// the normal recovery path without a process-global switch.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct CasFailpoints {
    armed: Option<(CasFailPoint, CasFaultAction)>,
}

impl CasFailpoints {
    pub const fn disabled() -> Self {
        Self { armed: None }
    }

    pub const fn once(point: CasFailPoint, action: CasFaultAction) -> Self {
        Self {
            armed: Some((point, action)),
        }
    }

    pub fn arm(&mut self, point: CasFailPoint, action: CasFaultAction) {
        self.armed = Some((point, action));
    }

    pub fn disarm(&mut self) {
        self.armed = None;
    }

    pub const fn armed(&self) -> Option<(CasFailPoint, CasFaultAction)> {
        self.armed
    }

    fn take_if(&mut self, point: CasFailPoint) -> Option<CasFaultAction> {
        if self.armed.map(|(armed, _)| armed) == Some(point) {
            self.armed.take().map(|(_, action)| action)
        } else {
            None
        }
    }
}

/// A SHA-256 content digest.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Digest([u8; 32]);

impl Digest {
    pub const LENGTH: usize = 32;

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(value: &str) -> Result<Self, PongError> {
        let bytes = hex::decode(value)
            .map_err(|error| PongError::InvalidInput(format!("invalid digest: {error}")))?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
            PongError::InvalidInput("digest must contain exactly 64 hexadecimal characters".into())
        })?;
        Ok(Self(bytes))
    }
}

impl AsRef<[u8]> for Digest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Digest")
            .field(&self.to_hex())
            .finish()
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// Compute sha256(domain UTF-8 bytes + NUL + exact object bytes).
pub fn digest_for(domain: &str, bytes: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    Digest(hasher.finalize().into())
}

/// Convenience form for protocol fields that store digests as lowercase hex.
pub fn digest_hex(domain: &str, bytes: &[u8]) -> String {
    digest_for(domain, bytes).to_hex()
}

pub struct Cas {
    root: PathBuf,
    redactor: Redactor,
    failpoints: Mutex<CasFailpoints>,
}

impl Cas {
    /// Open (and create) a CAS root. Objects, staging files, and quarantine
    /// entries are all kept beneath this directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PongError> {
        Self::new_with_redactor(root, Redactor::default())
    }

    /// Open a CAS with the repository's fail-closed byte policy. CAS stores
    /// exact bytes, so secret-bearing content is rejected rather than
    /// rewritten and silently changing its content identity.
    pub fn new_with_redactor(
        root: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        Self::new_with_redactor_and_failpoints(root, redactor, CasFailpoints::disabled())
    }

    /// Open a CAS with an explicit deterministic, one-shot fault plan.
    ///
    /// This constructor exists for fault-injection tests.  Normal callers
    /// should use [`Cas::new`] or [`Cas::new_with_redactor`], which always use
    /// disabled failpoints.
    pub fn new_with_failpoints(
        root: impl AsRef<Path>,
        failpoints: CasFailpoints,
    ) -> Result<Self, PongError> {
        Self::new_with_redactor_and_failpoints(root, Redactor::default(), failpoints)
    }

    /// Open a CAS with both an explicit redaction policy and fault plan.
    pub fn new_with_redactor_and_failpoints(
        root: impl AsRef<Path>,
        redactor: Redactor,
        failpoints: CasFailpoints,
    ) -> Result<Self, PongError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("objects"))?;
        fs::create_dir_all(root.join("staging"))?;
        fs::create_dir_all(root.join("quarantine"))?;
        let cas = Self {
            root,
            redactor,
            failpoints: Mutex::new(failpoints),
        };
        cas.scan_existing_bytes()?;
        Ok(cas)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Return the current test fault configuration.
    ///
    /// This is primarily useful for asserting one-shot consumption in a fault
    /// harness; normal callers observe [`CasFailpoints::disabled`].
    pub fn failpoints(&self) -> CasFailpoints {
        *self
            .failpoints
            .lock()
            .expect("CAS failpoint mutex is not poisoned")
    }

    /// Replace the instance-scoped test fault configuration.
    pub fn set_failpoints(&self, failpoints: CasFailpoints) {
        *self
            .failpoints
            .lock()
            .expect("CAS failpoint mutex is not poisoned") = failpoints;
    }

    pub fn object_path(&self, digest: Digest) -> PathBuf {
        let hex = digest.to_hex();
        self.root.join("objects").join(&hex[..2]).join(&hex[2..])
    }

    /// Store an object after checking its content-derived key. Repeated puts
    /// of identical bytes are successful no-ops; no call can overwrite a key.
    pub fn put(&self, domain: &str, bytes: &[u8]) -> Result<Digest, PongError> {
        validate_domain(domain)?;
        let digest = digest_for(domain, bytes);
        self.put_verified(domain, digest, bytes)?;
        Ok(digest)
    }

    /// Store bytes under a caller-supplied digest, rejecting mismatches before
    /// any staging file is made reachable.
    pub fn put_verified(
        &self,
        domain: &str,
        expected: Digest,
        bytes: &[u8],
    ) -> Result<(), PongError> {
        validate_domain(domain)?;
        self.redactor.assert_clean_bytes(bytes, "CAS publication")?;
        let actual = digest_for(domain, bytes);
        if actual != expected {
            return Err(PongError::Integrity(format!(
                "CAS digest mismatch: expected {expected}, got {actual}"
            )));
        }

        let destination = self.object_path(expected);
        if destination.exists() {
            return self.verify_existing(&destination, domain, expected, bytes.len() as u64);
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        let staging = self.new_staging_path();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        if let Err(error) = self.write_staging(&mut file, bytes) {
            drop(file);
            let _ = fs::remove_file(&staging);
            return Err(error);
        }
        drop(file);

        match self.take_fault(CasFailPoint::Publication) {
            CasFaultAction::Continue => {}
            action => {
                let _ = fs::remove_file(&staging);
                return Err(fault_error(CasFailPoint::Publication, action));
            }
        }

        // hard_link is atomic and never replaces an existing destination.  A
        // later directory-sync failure can therefore report an error after
        // the object is visible; retries verify this immutable object rather
        // than publishing a second value.
        match fs::hard_link(&staging, &destination) {
            Ok(()) => {
                fs::remove_file(&staging)?;
                self.sync_directory(destination.parent().expect("CAS shard has parent"))?;
                self.sync_directory(&self.root.join("objects"))?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&staging);
                self.verify_existing(&destination, domain, expected, bytes.len() as u64)
            }
            Err(error) => {
                let _ = fs::remove_file(&staging);
                Err(PongError::from(error))
            }
        }
    }

    pub fn get(&self, domain: &str, digest: Digest) -> Result<Vec<u8>, PongError> {
        validate_domain(domain)?;
        let path = self.object_path(digest);
        ensure_regular_object(&path, digest)?;
        let mut file = File::open(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                PongError::NotFound(format!("CAS object {digest} does not exist"))
            } else {
                PongError::from(error)
            }
        })?;
        let size = file.metadata()?.len();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if size != bytes.len() as u64 || digest_for(domain, &bytes) != digest {
            self.quarantine(&path, digest);
            return Err(PongError::Integrity(format!(
                "CAS object {digest} failed size or digest verification"
            )));
        }
        if self.redactor.contains_secret(&bytes) {
            self.quarantine(&path, digest);
            return Err(PongError::Integrity(
                "configured secret detected in CAS object".into(),
            ));
        }
        Ok(bytes)
    }

    pub fn exists(&self, digest: Digest) -> bool {
        fs::symlink_metadata(self.object_path(digest))
            .map(|metadata| metadata.file_type().is_file())
            .unwrap_or(false)
    }

    fn verify_existing(
        &self,
        path: &Path,
        domain: &str,
        digest: Digest,
        expected_size: u64,
    ) -> Result<(), PongError> {
        ensure_regular_object(path, digest)?;
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if size != expected_size || digest_for(domain, &bytes) != digest {
            self.quarantine(path, digest);
            return Err(PongError::Integrity(format!(
                "immutable CAS collision or corrupt object {digest}"
            )));
        }
        if self.redactor.contains_secret(&bytes) {
            self.quarantine(path, digest);
            return Err(PongError::Integrity(
                "configured secret detected in CAS object".into(),
            ));
        }
        Ok(())
    }

    fn scan_existing_bytes(&self) -> Result<(), PongError> {
        match self.take_fault(CasFailPoint::StartupScan) {
            CasFaultAction::Continue => scan_tree(&self.root, &self.redactor),
            action => Err(fault_error(CasFailPoint::StartupScan, action)),
        }
    }

    fn quarantine(&self, path: &Path, digest: Digest) {
        let suffix = unique_suffix();
        let target = self
            .root
            .join("quarantine")
            .join(format!("{}.{}", digest, suffix));
        let _ = fs::rename(path, target);
        let _ = self.sync_directory(&self.root.join("quarantine"));
    }

    fn write_staging(&self, file: &mut File, bytes: &[u8]) -> Result<(), PongError> {
        match self.take_fault(CasFailPoint::StagingWrite) {
            CasFaultAction::Continue => file
                .write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(PongError::from),
            action @ (CasFaultAction::Fail | CasFaultAction::PermissionDenied) => {
                Err(fault_error(CasFailPoint::StagingWrite, action))
            }
            action
            @ (CasFaultAction::ShortWrite(limit) | CasFaultAction::QuotaExhausted(limit)) => {
                let prefix_len = limit.min(bytes.len());
                if prefix_len > 0 {
                    file.write_all(&bytes[..prefix_len])
                        .map_err(PongError::from)?;
                }
                Err(fault_error(CasFailPoint::StagingWrite, action))
            }
        }
    }

    fn sync_directory(&self, path: &Path) -> Result<(), PongError> {
        match self.take_fault(CasFailPoint::DirectorySync) {
            CasFaultAction::Continue => sync_directory(path),
            action => Err(fault_error(CasFailPoint::DirectorySync, action)),
        }
    }

    fn take_fault(&self, point: CasFailPoint) -> CasFaultAction {
        self.failpoints
            .lock()
            .expect("CAS failpoint mutex is not poisoned")
            .take_if(point)
            .unwrap_or(CasFaultAction::Continue)
    }

    fn new_staging_path(&self) -> PathBuf {
        self.root.join("staging").join(format!(
            ".object-{}-{}",
            std::process::id(),
            unique_suffix()
        ))
    }
}

fn validate_domain(domain: &str) -> Result<(), PongError> {
    if domain.is_empty() {
        return Err(PongError::InvalidInput("domain must not be empty".into()));
    }
    if domain.as_bytes().contains(&0) {
        return Err(PongError::InvalidInput(
            "domain must not contain NUL".into(),
        ));
    }
    Ok(())
}

fn ensure_regular_object(path: &Path, digest: Digest) -> Result<(), PongError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PongError::NotFound(format!("CAS object {digest} does not exist"))
        } else {
            PongError::from(error)
        }
    })?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err(PongError::Integrity(format!(
            "CAS object {digest} is not a regular file"
        )));
    }
    Ok(())
}

fn scan_tree(path: &Path, redactor: &Redactor) -> Result<(), PongError> {
    for entry in fs::read_dir(path).map_err(PongError::from_protected_io)? {
        let entry = entry.map_err(PongError::from_protected_io)?;
        let child = entry.path();
        if child.file_name().and_then(|name| name.to_str()) == Some("repository.lock")
            && path.file_name().and_then(|name| name.to_str()) == Some("locks")
        {
            continue;
        }
        let metadata = fs::symlink_metadata(&child).map_err(PongError::from_protected_io)?;
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "CAS tree contains a symlink or reparse point".into(),
            ));
        }
        if metadata.is_dir() {
            scan_tree(&child, redactor)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&child).map_err(PongError::from_protected_io)?;
            redactor.assert_clean_bytes(&bytes, "CAS-owned bytes")?;
        } else {
            return Err(PongError::Integrity(format!(
                "CAS tree contains a non-regular entry: {}",
                child.display()
            )));
        }
    }
    Ok(())
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn unique_suffix() -> String {
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}-{sequence:x}")
}

fn fault_error(point: CasFailPoint, action: CasFaultAction) -> PongError {
    let reason = match action {
        CasFaultAction::Continue => return PongError::FaultInjected(point.label().into()),
        CasFaultAction::Fail => None,
        CasFaultAction::ShortWrite(_) => Some("short_write"),
        CasFaultAction::PermissionDenied => Some("permission_denied"),
        CasFaultAction::QuotaExhausted(_) => Some("quota_exhausted"),
    };
    let label = match reason {
        Some(reason) => format!("{}:{reason}", point.label()),
        None => point.label().to_owned(),
    };
    match action {
        CasFaultAction::PermissionDenied => PongError::PermissionDenied(label),
        CasFaultAction::QuotaExhausted(_) => PongError::ResourceExhausted(label),
        CasFaultAction::Continue | CasFaultAction::Fail | CasFaultAction::ShortWrite(_) => {
            PongError::FaultInjected(label)
        }
    }
}

fn sync_directory(path: &Path) -> Result<(), PongError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_BACKUP_SEMANTICS is required to obtain a directory handle
        // on Windows; a plain File::open returns ERROR_ACCESS_DENIED.
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)?
            .sync_all()?;
    }
    #[cfg(not(windows))]
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redaction::Redactor;
    use tempfile::tempdir;

    #[test]
    fn digest_includes_domain_separator_and_exact_bytes() {
        assert_ne!(digest_for("a", b"bc"), digest_for("ab", b"c"));
        assert_ne!(digest_for("x", b""), digest_for("x", b"\0"));
    }

    #[test]
    fn put_get_and_repeat_are_verified() {
        let directory = tempdir().unwrap();
        let cas = Cas::new(directory.path()).unwrap();
        let digest = cas.put("snapshot/v1", b"hello").unwrap();
        assert_eq!(cas.get("snapshot/v1", digest).unwrap(), b"hello");
        assert_eq!(cas.put("snapshot/v1", b"hello").unwrap(), digest);
        assert!(cas.object_path(digest).is_file());
        assert!(cas
            .root()
            .join("staging")
            .read_dir()
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn wrong_expected_digest_is_rejected_before_publication() {
        let directory = tempdir().unwrap();
        let cas = Cas::new(directory.path()).unwrap();
        let wrong = digest_for("other", b"hello");
        assert!(matches!(
            cas.put_verified("snapshot/v1", wrong, b"hello"),
            Err(PongError::Integrity(_))
        ));
        assert_eq!(cas.root().join("objects").read_dir().unwrap().count(), 0);
    }

    #[test]
    fn tampering_is_integrity_error_and_quarantined() {
        let directory = tempdir().unwrap();
        let cas = Cas::new(directory.path()).unwrap();
        let digest = cas.put("snapshot/v1", b"hello").unwrap();
        fs::write(cas.object_path(digest), b"tampered").unwrap();
        assert!(matches!(
            cas.get("snapshot/v1", digest),
            Err(PongError::Integrity(_))
        ));
        assert_eq!(cas.root().join("quarantine").read_dir().unwrap().count(), 1);
    }

    #[test]
    fn malformed_digest_and_domain_are_rejected() {
        assert!(Digest::from_hex("00").is_err());
        let directory = tempdir().unwrap();
        let cas = Cas::new(directory.path()).unwrap();
        assert!(matches!(
            cas.put("bad\0domain", b"x"),
            Err(PongError::InvalidInput(_))
        ));
    }

    #[test]
    fn configured_secret_is_rejected_before_cas_publication() {
        let directory = tempdir().unwrap();
        let mut redactor = Redactor::default();
        redactor.register_secret("cas-secret").unwrap();
        let cas = Cas::new_with_redactor(directory.path(), redactor).unwrap();
        let error = cas
            .put("snapshot/v1", b"payload with cas-secret")
            .expect_err("secret-bearing CAS object must fail closed");
        assert_eq!(error.code(), "INTEGRITY_ERROR");
        assert_eq!(cas.root().join("objects").read_dir().unwrap().count(), 0);
    }

    #[test]
    fn existing_secret_bytes_block_cas_startup() {
        let directory = tempdir().unwrap();
        let cas = Cas::new(directory.path()).unwrap();
        let digest = cas.put("snapshot/v1", b"payload with cas-secret").unwrap();
        drop(cas);
        let mut redactor = Redactor::default();
        redactor.register_secret("cas-secret").unwrap();
        let error = match Cas::new_with_redactor(directory.path(), redactor) {
            Ok(_) => panic!("existing secret object must block startup"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "INTEGRITY_ERROR");
        assert!(directory
            .path()
            .join("objects")
            .join(&digest.to_hex()[..2])
            .join(&digest.to_hex()[2..])
            .is_file());
    }

    #[test]
    fn injected_short_write_cleans_staging_and_publishes_nothing() {
        let directory = tempdir().unwrap();
        let failpoints =
            CasFailpoints::once(CasFailPoint::StagingWrite, CasFaultAction::ShortWrite(2));
        let cas = Cas::new_with_failpoints(directory.path(), failpoints).unwrap();

        let error = cas
            .put("snapshot/v1", b"payload")
            .expect_err("short write must fail closed");
        assert_eq!(error.code(), "FAULT_INJECTED");
        assert_eq!(cas.failpoints().armed(), None);
        assert!(!cas
            .object_path(digest_for("snapshot/v1", b"payload"))
            .exists());
        assert_eq!(cas.root().join("staging").read_dir().unwrap().count(), 0);
    }

    #[test]
    fn injected_publication_failure_cleans_staging() {
        let directory = tempdir().unwrap();
        let failpoints = CasFailpoints::once(CasFailPoint::Publication, CasFaultAction::Fail);
        let cas = Cas::new_with_failpoints(directory.path(), failpoints).unwrap();
        let digest = digest_for("snapshot/v1", b"payload");

        let error = cas
            .put("snapshot/v1", b"payload")
            .expect_err("publication failure must be reported");
        assert_eq!(error.code(), "FAULT_INJECTED");
        assert_eq!(cas.failpoints().armed(), None);
        assert!(!cas.object_path(digest).exists());
        assert_eq!(cas.root().join("staging").read_dir().unwrap().count(), 0);
    }

    #[test]
    fn injected_permission_failure_is_stable_without_changing_host_acl() {
        let directory = tempdir().unwrap();
        let failpoints =
            CasFailpoints::once(CasFailPoint::StagingWrite, CasFaultAction::PermissionDenied);
        let cas = Cas::new_with_failpoints(directory.path(), failpoints).unwrap();

        let error = cas
            .put("snapshot/v1", b"payload")
            .expect_err("permission fault must fail closed");
        assert_eq!(error.code(), "PERMISSION_DENIED");
        assert!(error.to_string().contains("permission_denied"));
        assert_eq!(cas.root().join("staging").read_dir().unwrap().count(), 0);
    }

    #[test]
    fn injected_quota_failure_cleans_partial_staging_bytes() {
        let directory = tempdir().unwrap();
        let failpoints = CasFailpoints::once(
            CasFailPoint::StagingWrite,
            CasFaultAction::QuotaExhausted(2),
        );
        let cas = Cas::new_with_failpoints(directory.path(), failpoints).unwrap();

        let error = cas
            .put("snapshot/v1", b"payload")
            .expect_err("quota fault must fail closed");
        assert_eq!(error.code(), "RESOURCE_EXHAUSTED");
        assert!(error.to_string().contains("quota_exhausted"));
        assert!(!cas
            .object_path(digest_for("snapshot/v1", b"payload"))
            .exists());
        assert_eq!(cas.root().join("staging").read_dir().unwrap().count(), 0);
    }

    #[test]
    fn injected_directory_sync_failure_leaves_a_retryable_published_object() {
        let directory = tempdir().unwrap();
        let failpoints = CasFailpoints::once(CasFailPoint::DirectorySync, CasFaultAction::Fail);
        let cas = Cas::new_with_failpoints(directory.path(), failpoints).unwrap();

        let error = cas
            .put("snapshot/v1", b"payload")
            .expect_err("directory sync failure must be reported");
        assert_eq!(error.code(), "FAULT_INJECTED");
        assert_eq!(cas.root().join("staging").read_dir().unwrap().count(), 0);
        let digest = digest_for("snapshot/v1", b"payload");
        assert!(cas.object_path(digest).is_file());
        assert_eq!(cas.put("snapshot/v1", b"payload").unwrap(), digest);
    }

    #[test]
    fn injected_startup_scan_failure_does_not_disable_normal_reopen() {
        let directory = tempdir().unwrap();
        let cas = Cas::new(directory.path()).unwrap();
        let digest = cas.put("snapshot/v1", b"payload").unwrap();
        drop(cas);

        let failpoints = CasFailpoints::once(CasFailPoint::StartupScan, CasFaultAction::Fail);
        let error = match Cas::new_with_failpoints(directory.path(), failpoints) {
            Ok(_) => panic!("startup scan fault must fail startup"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "FAULT_INJECTED");
        assert!(Cas::new(directory.path())
            .unwrap()
            .get("snapshot/v1", digest)
            .is_ok());
    }
}
