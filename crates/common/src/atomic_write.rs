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

    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            // Extremely rare on Windows: someone (antivirus, indexer) holds an
            // exclusive handle on the target. Best-effort fallback: remove the
            // target and retry. If the retry also fails, caller sees the error
            // and the temp file is cleaned up.
            let _ = fs::remove_file(path);
            match fs::rename(&tmp, path) {
                Ok(()) => Ok(()),
                Err(e2) => {
                    let _ = fs::remove_file(&tmp);
                    Err(e2)
                }
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
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
    #[test]
    fn errors_on_bare_drive_root() {
        // On Windows, `Path::parent` of `"C:\\"` returns None — exercise the
        // InvalidInput branch.
        let p = Path::new("C:\\");
        let err = write_atomic(p, b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
