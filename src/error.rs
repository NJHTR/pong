use thiserror::Error;

/// Stable domain and storage failures exposed by the Core primitive ports.
#[derive(Debug, Error)]
pub enum PongError {
    #[error("I/O error: {0}")]
    Io(#[source] std::io::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[source] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("integrity error: {0}")]
    Integrity(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("idempotency key reuse: {0}")]
    IdempotencyKeyReuse(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("recovery required: {0}")]
    RecoveryRequired(String),

    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// A permission failure that retains the host OS error for diagnostics.
    ///
    /// The stable protocol code remains `PERMISSION_DENIED`; callers must not
    /// branch on the display text. This is used at protected repository
    /// boundaries where Windows may otherwise collapse a sharing/ACL error
    /// into the same domain status.
    #[error("permission denied: {message} (raw_os_error={raw_os_error})")]
    PermissionDeniedWithOsError { message: String, raw_os_error: i32 },

    /// A deterministic test fault was requested at a durable boundary.
    ///
    /// This variant is intentionally distinct from I/O and SQLite failures:
    /// callers exercising crash/recovery behavior must be able to tell that
    /// the fault was injected, while still treating the operation as
    /// unconfirmed when it was raised after a transaction commit.
    #[error("fault injected at {0}")]
    FaultInjected(String),
}

impl From<std::io::Error> for PongError {
    fn from(error: std::io::Error) -> Self {
        if is_filesystem_permission_error(&error) {
            Self::PermissionDenied("filesystem operation was denied".into())
        } else if is_filesystem_capacity_error(&error) {
            Self::ResourceExhausted("filesystem capacity is exhausted".into())
        } else {
            Self::Io(error)
        }
    }
}

impl PongError {
    /// Map an error from a protected repository read/write boundary. On
    /// Windows, ERROR_ACCESS_DENIED is also used for some rename/share races,
    /// so the ordinary `From<io::Error>` conversion deliberately leaves that
    /// code as IO_ERROR; repository publication, directory sync, and integrity
    /// scans opt into the stricter ACL mapping.
    pub(crate) fn from_protected_io(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::PermissionDenied
            || cfg!(windows) && error.raw_os_error() == Some(5)
        {
            if let Some(raw_os_error) = error.raw_os_error() {
                Self::PermissionDeniedWithOsError {
                    message: "protected repository access was denied".into(),
                    raw_os_error,
                }
            } else {
                Self::PermissionDenied("protected repository access was denied".into())
            }
        } else if is_filesystem_capacity_error(&error) {
            Self::ResourceExhausted("filesystem capacity is exhausted".into())
        } else {
            Self::from(error)
        }
    }
}

fn is_filesystem_permission_error(error: &std::io::Error) -> bool {
    if error.kind() != std::io::ErrorKind::PermissionDenied {
        return false;
    }

    #[cfg(windows)]
    {
        // Windows also reports sharing and lock collisions as
        // PermissionDenied/ERROR_ACCESS_DENIED. Callers that are reading a
        // protected repository boundary use `from_protected_io` to opt into
        // the stricter ACL classification without misclassifying rename races.
        let _ = error;
        false
    }
    #[cfg(not(windows))]
    {
        true
    }
}

impl From<rusqlite::Error> for PongError {
    fn from(error: rusqlite::Error) -> Self {
        match &error {
            rusqlite::Error::SqliteFailure(failure, _)
                if failure.code == rusqlite::ErrorCode::DiskFull =>
            {
                Self::ResourceExhausted("SQLite storage capacity is exhausted".into())
            }
            rusqlite::Error::SqliteFailure(failure, _)
                if matches!(
                    failure.code,
                    rusqlite::ErrorCode::PermissionDenied
                        | rusqlite::ErrorCode::ReadOnly
                        | rusqlite::ErrorCode::AuthorizationForStatementDenied
                ) =>
            {
                Self::PermissionDenied("SQLite storage access was denied".into())
            }
            _ => Self::Sqlite(error),
        }
    }
}

fn is_filesystem_capacity_error(error: &std::io::Error) -> bool {
    let Some(code) = error.raw_os_error() else {
        return false;
    };

    #[cfg(windows)]
    {
        // ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL, and
        // ERROR_DISK_QUOTA_EXCEEDED.
        matches!(code, 39 | 112 | 1295)
    }
    #[cfg(target_os = "linux")]
    {
        // ENOSPC and EDQUOT.
        matches!(code, 28 | 122)
    }
    #[cfg(target_os = "macos")]
    {
        // ENOSPC and EDQUOT.
        matches!(code, 28 | 69)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = code;
        false
    }
}

impl PongError {
    /// Return the host OS error retained by a storage boundary, when one is
    /// available. Domain errors without a host error return `None`.
    pub fn raw_os_error(&self) -> Option<i32> {
        match self {
            Self::Io(error) => error.raw_os_error(),
            Self::PermissionDeniedWithOsError { raw_os_error, .. } => Some(*raw_os_error),
            _ => None,
        }
    }

    /// Machine-readable protocol code; callers must not branch on display text.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO_ERROR",
            Self::Sqlite(_) => "STORAGE_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::Integrity(_) => "INTEGRITY_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::IdempotencyKeyReuse(_) => "IDEMPOTENCY_KEY_REUSE",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::RecoveryRequired(_) => "RECOVERY_REQUIRED",
            Self::ResourceExhausted(_) => "RESOURCE_EXHAUSTED",
            Self::PermissionDenied(_) | Self::PermissionDeniedWithOsError { .. } => {
                "PERMISSION_DENIED"
            }
            Self::FaultInjected(_) => "FAULT_INJECTED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_denied_io_has_a_stable_domain_code() {
        #[cfg(windows)]
        {
            let error = PongError::from_protected_io(std::io::Error::from_raw_os_error(5));
            assert_eq!(error.code(), "PERMISSION_DENIED");
            assert_eq!(error.raw_os_error(), Some(5));
        }
        #[cfg(not(windows))]
        assert_eq!(
            PongError::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied)).code(),
            "PERMISSION_DENIED"
        );
    }

    #[test]
    fn protected_permission_retains_an_available_raw_os_error() {
        let raw_os_error = if cfg!(windows) { 5 } else { 13 };
        let error = PongError::from_protected_io(std::io::Error::from_raw_os_error(raw_os_error));
        assert_eq!(error.code(), "PERMISSION_DENIED");
        assert_eq!(error.raw_os_error(), Some(raw_os_error));
    }

    #[cfg(windows)]
    #[test]
    fn windows_sharing_violation_is_not_misreported_as_acl_denial() {
        let error = PongError::from(std::io::Error::from_raw_os_error(32));
        assert_eq!(error.code(), "IO_ERROR");
    }

    #[test]
    fn sqlite_full_has_a_stable_domain_code() {
        let failure = rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_FULL);
        let error = PongError::from(rusqlite::Error::SqliteFailure(failure, None));
        assert_eq!(error.code(), "RESOURCE_EXHAUSTED");
    }

    #[test]
    fn sqlite_read_only_has_a_stable_domain_code() {
        let failure = rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_READONLY);
        let error = PongError::from(rusqlite::Error::SqliteFailure(failure, None));
        assert_eq!(error.code(), "PERMISSION_DENIED");
    }
}
