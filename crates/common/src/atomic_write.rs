//! Crash-safe atomic file writes for Windows.
//!
//! The standard `std::fs::write` truncates the target file first, then writes.
//! If the process is killed (power loss, crash, Task Manager End Task) between
//! truncate and write, the file is left empty or partially written — on next
//! startup the config loader sees a zero-byte or malformed file and falls back
//! to defaults, silently losing all user customization.
//!
//! `write_atomic` fixes this by writing to a sibling temp file, fsyncing, then
//! renaming over the target. On Windows, `std::fs::rename` uses `MoveFileExW`
//! which is atomic within a single NTFS volume. Readers see either the old
//! contents or the new contents — never a truncated prefix.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Atomically replace `path` with `contents`.
///
/// The temp file lives in the same directory as `path` so the rename is a
/// same-volume operation (atomic on Windows and Linux). On success, no temp
/// files are left behind. If the rename hits a transient lock error
/// (PermissionDenied, sharing/lock violation), it is retried a few times with a
/// short backoff (~310 ms total). The target is never deleted: on failure, the
/// temp file is cleaned up best-effort, the last rename error is returned, and
/// the original target (if any) is untouched.
///
/// Returns `io::ErrorKind::InvalidInput` if `path` has no parent directory or
/// no file name (e.g. a bare drive root like `"C:\\"`).
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no file name")
    })?;

    fs::create_dir_all(parent)?;

    // Deterministic temp name next to the target. Hidden (leading dot) so the
    // user doesn't see it in Explorer if we ever leak one.
    let mut tmp = parent.to_path_buf();
    tmp.push(format!(".{}.tmp", file_name.to_string_lossy()));

    // Write+fsync in an inner scope so the handle is dropped before the rename.
    // Windows requires the source handle to be closed before MoveFileExW.
    let write_result: io::Result<()> = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    // Transient Windows locks (antivirus, indexer) can make the rename fail
    // briefly. Retry on a short fixed schedule; never delete the target.
    let mut result = fs::rename(&tmp, path);
    for delay in RENAME_RETRY_DELAYS_MS {
        match &result {
            Err(e) if is_transient_lock_error(e) => {
                thread::sleep(Duration::from_millis(delay));
                result = fs::rename(&tmp, path);
            }
            _ => break,
        }
    }

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Backoff schedule (ms) for transient rename failures; ~310 ms total.
const RENAME_RETRY_DELAYS_MS: [u64; 5] = [10, 20, 40, 80, 160];

/// PermissionDenied, ERROR_SHARING_VIOLATION (32) or ERROR_LOCK_VIOLATION (33).
fn is_transient_lock_error(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(32 | 33))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_new_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        write_atomic(&p, b"hello").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"hello");
    }

    #[test]
    fn replaces_existing_file_atomically() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        fs::write(&p, b"old").unwrap();
        write_atomic(&p, b"new-contents").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"new-contents");
    }

    #[test]
    fn leaves_no_temp_files_on_success() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        write_atomic(&p, b"x").unwrap();
        let leftover: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".cfg.toml")
            })
            .collect();
        assert!(leftover.is_empty(), "temp file leaked: {:?}", leftover);
    }

    #[test]
    fn creates_parent_directory_if_missing() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("nested").join("deeper").join("cfg.toml");
        write_atomic(&p, b"deep").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"deep");
    }

    #[test]
    fn cleans_up_temp_on_rename_failure() {
        // Simulate a rename failure by making the target a non-empty directory
        // with the same name. fs::rename on Windows fails with a replaceable
        // target-already-exists-as-directory error; our temp cleanup should fire.
        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        fs::create_dir(&p).unwrap();
        // Put a file inside so rmdir-on-rename fails.
        fs::write(p.join("blocker"), b"stuck").unwrap();

        let res = write_atomic(&p, b"attempt");
        assert!(res.is_err(), "rename should have failed");

        // Temp should have been cleaned up regardless of the rename outcome.
        let leftover: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".cfg.toml"))
            .collect();
        assert!(leftover.is_empty(), "temp file leaked: {:?}", leftover);
    }

    #[cfg(windows)]
    fn open_exclusive(p: &Path) -> File {
        use std::os::windows::fs::OpenOptionsExt;
        // share_mode(0) = no sharing: rename/delete/open of the target by
        // anyone else fails while this handle is alive.
        fs::OpenOptions::new().read(true).share_mode(0).open(p).unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn locked_target_is_never_deleted_and_temp_cleaned() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        fs::write(&p, b"original").unwrap();

        let lock = open_exclusive(&p);
        let res = write_atomic(&p, b"new");
        drop(lock);

        assert!(res.is_err(), "rename over exclusively locked target must fail");
        assert_eq!(fs::read(&p).unwrap(), b"original", "target must be untouched");
        assert!(!dir.path().join(".cfg.toml.tmp").exists(), "temp file leaked");
    }

    #[cfg(windows)]
    #[test]
    fn retries_transient_lock_then_succeeds() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        fs::write(&p, b"original").unwrap();

        let lock = open_exclusive(&p);
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            drop(lock);
        });
        let res = write_atomic(&p, b"new");
        releaser.join().unwrap();

        res.expect("write should succeed once the transient lock is released");
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert!(!dir.path().join(".cfg.toml.tmp").exists(), "temp file leaked");
    }

    #[cfg(windows)]
    #[test]
    fn errors_on_bare_drive_root() {
        // On Windows, `Path::parent` of `"C:\\"` returns None — exercise the
        // InvalidInput branch.
        let p = Path::new("C:\\");
        let err = write_atomic(p, b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
