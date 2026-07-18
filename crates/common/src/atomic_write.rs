//! Crash-safe atomic file writes for Windows.
//!
//! The standard `std::fs::write` truncates the target file first, then writes.
//! If the process is killed (power loss, crash, Task Manager End Task) between
//! truncate and write, the file is left empty or partially written — on next
//! startup the config loader sees a zero-byte or malformed file and falls back
//! to defaults, silently losing all user customization.
//!
//! `write_atomic` fixes this by writing to a sibling temp file, fsyncing, then
//! replacing the target with `MoveFileExW` on Windows. Readers see either the
//! old contents or the new contents, never a truncated prefix.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Atomically replace `path` with `contents`.
///
/// The temp file lives in the same directory as `path` so the rename is a
/// same-volume operation (atomic on Windows and Linux). On success, no temp
/// files are left behind. On failure, the temp file is cleaned up best-effort
/// and the original target (if any) is untouched.
///
/// Returns `io::ErrorKind::InvalidInput` if `path` has no parent directory or
/// no file name (e.g. a bare drive root like `"C:\\"`).
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
        })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;

    fs::create_dir_all(parent)?;

    let (tmp, mut file) = create_unique_temp(parent, file_name)?;

    // Write+fsync in an inner scope so the handle is dropped before the rename.
    // Windows requires the source handle to be closed before MoveFileExW.
    let write_result: io::Result<()> = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    match replace_file(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn create_unique_temp(
    parent: &Path,
    file_name: &std::ffi::OsStr,
) -> io::Result<(std::path::PathBuf, File)> {
    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tmp = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            sequence
        ));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique atomic-write temp file",
    ))
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut last_error = None;
    for attempt in 0..50 {
        // SAFETY: both paths are NUL-terminated and remain valid for the call.
        match unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt < 49 {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        }
    }
    Err(io::Error::other(format!(
        "MoveFileExW failed: {}",
        last_error.expect("replace loop always runs")
    )))
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
            .filter(|e| e.file_name().to_string_lossy().starts_with(".cfg.toml"))
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
    fn concurrent_writers_use_distinct_temp_files() {
        let dir = tempdir().unwrap();
        let path = std::sync::Arc::new(dir.path().join("cfg.toml"));
        let mut writers = Vec::new();
        for value in 0..8u8 {
            let path = std::sync::Arc::clone(&path);
            writers.push(std::thread::spawn(move || {
                write_atomic(&path, &[value; 16]).unwrap();
            }));
        }
        for writer in writers {
            writer.join().unwrap();
        }
        let contents = fs::read(&*path).unwrap();
        assert_eq!(contents.len(), 16);
        assert!(contents.iter().all(|byte| *byte == contents[0]));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    #[allow(clippy::permissions_set_readonly_false)]
    fn failed_replace_preserves_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, b"original").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions.clone()).unwrap();

        let result = write_atomic(&path, b"replacement");
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");

        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();
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
    #[test]
    fn errors_on_bare_drive_root() {
        // On Windows, `Path::parent` of `"C:\\"` returns None — exercise the
        // InvalidInput branch.
        let p = Path::new("C:\\");
        let err = write_atomic(p, b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
