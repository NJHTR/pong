use crate::error::PongError;
#[cfg(windows)]
use std::io;
use std::path::Path;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Replace `destination` with `temp` after `temp` has been fully written in
/// the same directory.
///
/// The implementation deliberately avoids remove-then-create sequences so the
/// namespace transition remains atomic. Callers are expected to write and sync
/// `temp` before invoking this function.
pub(crate) fn atomic_replace_file(temp: &Path, destination: &Path) -> Result<(), PongError> {
    if temp == destination {
        return Err(PongError::InvalidInput(
            "atomic replacement requires distinct temp and destination paths".into(),
        ));
    }

    let temp_parent = temp.parent().ok_or_else(|| {
        PongError::InvalidInput("atomic replacement temp path has no parent directory".into())
    })?;
    let destination_parent = destination.parent().ok_or_else(|| {
        PongError::InvalidInput(
            "atomic replacement destination path has no parent directory".into(),
        )
    })?;
    if temp_parent != destination_parent {
        return Err(PongError::InvalidInput(
            "atomic replacement requires temp and destination to be in the same directory".into(),
        ));
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };

        let temp = wide_null(temp.as_os_str());
        let destination = wide_null(destination.as_os_str());
        retry_windows_io(|| {
            let ok = unsafe {
                MoveFileExW(
                    temp.as_ptr(),
                    destination.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            };
            if ok == 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        })
        // Repository-owned marker/journal files are a protected storage
        // boundary. Classify a terminal access-denied result consistently
        // with directory sync and integrity scans.
        .map_err(PongError::from_protected_io)?;
    }

    #[cfg(not(windows))]
    {
        std::fs::rename(temp, destination)?;
    }

    sync_directory(destination_parent)?;
    Ok(())
}

/// Rename two paths in one directory, retrying only transient Windows sharing
/// errors that are commonly produced while SQLite/antivirus handles drain.
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> Result<(), PongError> {
    #[cfg(windows)]
    {
        retry_windows_io(|| std::fs::rename(from, to)).map_err(PongError::from)
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(from, to).map_err(PongError::from)
    }
}

/// Publish a newly-created directory without replacing an existing path.
///
/// This is distinct from [`rename_with_retry`], which intentionally has
/// replace semantics for repository generation commits. Workspace
/// materialization must use no-replace semantics so a concurrent destination
/// cannot be silently destroyed.
pub(crate) fn rename_new_with_retry(from: &Path, to: &Path) -> Result<(), PongError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

        let from_wide = wide_null(from.as_os_str());
        let to_wide = wide_null(to.as_os_str());
        // A directory handle is a protected repository read boundary. Map a
        // final ERROR_ACCESS_DENIED through the stricter classifier so ACL
        // failures do not surface as an ambiguous generic I/O error. Rename
        // paths keep their separate sharing/race classification.
        retry_windows_io(|| {
            let ok = unsafe {
                MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), MOVEFILE_WRITE_THROUGH)
            };
            if ok == 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        })
        .map_err(map_no_replace_error)?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let from = CString::new(from.as_os_str().as_bytes()).map_err(|_| {
            PongError::InvalidInput("workspace path contains an embedded NUL".into())
        })?;
        let to = CString::new(to.as_os_str().as_bytes()).map_err(|_| {
            PongError::InvalidInput("workspace path contains an embedded NUL".into())
        })?;
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(map_no_replace_error(std::io::Error::last_os_error()));
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let from = CString::new(from.as_os_str().as_bytes()).map_err(|_| {
            PongError::InvalidInput("workspace path contains an embedded NUL".into())
        })?;
        let to = CString::new(to.as_os_str().as_bytes()).map_err(|_| {
            PongError::InvalidInput("workspace path contains an embedded NUL".into())
        })?;
        let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        if result != 0 {
            return Err(map_no_replace_error(std::io::Error::last_os_error()));
        }
    }

    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    {
        let _ = (from, to);
        return Err(PongError::Unsupported(
            "atomic no-replace directory publication is unsupported on this platform".into(),
        ));
    }

    Ok(())
}

fn map_no_replace_error(error: std::io::Error) -> PongError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::WouldBlock
    ) {
        PongError::Conflict("materialization destination already exists".into())
    } else {
        PongError::from(error)
    }
}

/// Flush a directory entry so namespace changes are durably visible.
pub(crate) fn sync_directory(path: &Path) -> Result<(), PongError> {
    #[cfg(windows)]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

        retry_windows_io(|| {
            OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)
                .and_then(|file| file.sync_all())
        })
        .map_err(PongError::from_protected_io)?;
    }

    #[cfg(not(windows))]
    {
        std::fs::File::open(path)?.sync_all()?;
    }

    Ok(())
}

#[cfg(windows)]
fn retry_windows_io<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    // Antivirus/indexer handles and SQLite sidecars can outlive a close by
    // more than the original 660 ms window when the full test suite is
    // running in parallel. Keep retries bounded, but give transient sharing
    // and access-denied races a two-second settling window.
    const ATTEMPTS: usize = 80;
    const RETRY_DELAY: Duration = Duration::from_millis(25);
    for attempt in 0..ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error)
                if matches!(error.raw_os_error(), Some(5 | 32 | 33)) && attempt + 1 < ATTEMPTS =>
            {
                thread::sleep(RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("retry loop returns on success or terminal error")
}

#[cfg(windows)]
fn wide_null(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::iter;

    value.encode_wide().chain(iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn atomic_replace_overwrites_existing_file() {
        let directory = tempdir().expect("directory");
        let destination = directory.path().join("repository.json");
        let temp = directory.path().join("repository.json.tmp");

        fs::write(&destination, b"old").expect("destination");
        fs::write(&temp, b"new").expect("temp");

        atomic_replace_file(&temp, &destination).expect("replace");

        assert_eq!(fs::read(&destination).expect("read destination"), b"new");
        assert!(!temp.exists());
    }

    #[test]
    fn sync_directory_accepts_an_existing_directory() {
        let directory = tempdir().expect("directory");
        sync_directory(directory.path()).expect("sync");
    }

    #[test]
    fn no_replace_directory_publish_preserves_an_existing_destination() {
        let directory = tempdir().expect("directory");
        let source = directory.path().join("source");
        let destination = directory.path().join("destination");
        fs::create_dir(&source).expect("source");
        fs::write(source.join("new.txt"), b"new").expect("source file");
        fs::create_dir(&destination).expect("destination");
        fs::write(destination.join("old.txt"), b"old").expect("destination file");

        let error = rename_new_with_retry(&source, &destination)
            .expect_err("existing destination must not be replaced");
        assert_eq!(error.code(), "CONFLICT");
        assert!(destination.join("old.txt").is_file());
        assert!(source.is_dir());
    }
}
