//! Cross-process ownership for direct Corsair HID access.

use std::marker::PhantomData;
use std::os::windows::ffi::OsStrExt;
use std::rc::Rc;

use thiserror::Error;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, HANDLE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::{Error as WindowsError, PCWSTR};

/// Shared by the GUI and service so only one process directly controls the hardware.
pub const HARDWARE_OWNERSHIP_MUTEX_NAME: &str = "Global\\ai-corsair-hub-hardware-owner-v1";

#[derive(Debug, Error)]
pub enum HardwareOwnershipError {
    #[error("Corsair hardware is already controlled by another ai-corsair-hub process")]
    AlreadyOwned,
    #[error("failed to acquire the Corsair hardware ownership mutex: {0}")]
    Windows(#[source] WindowsError),
    #[error("hardware ownership mutex returned unexpected wait result {0:#x}")]
    UnexpectedWaitResult(u32),
}

/// Releases the process-wide hardware lease when dropped on its owning thread.
#[derive(Debug)]
pub struct HardwareOwnershipGuard {
    handle: HANDLE,
    // Windows mutexes are thread-affine: only the acquiring thread may release them.
    _not_send_or_sync: PhantomData<Rc<()>>,
}

/// Tries to acquire exclusive direct access to Corsair HID devices without blocking.
pub fn try_acquire_hardware_ownership() -> Result<HardwareOwnershipGuard, HardwareOwnershipError> {
    try_acquire_named_mutex(HARDWARE_OWNERSHIP_MUTEX_NAME)
}

fn try_acquire_named_mutex(name: &str) -> Result<HardwareOwnershipGuard, HardwareOwnershipError> {
    let wide_name: Vec<u16> = std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle =
        unsafe { CreateMutexW(None, false, PCWSTR(wide_name.as_ptr())) }.map_err(|err| {
            if err.code() == ERROR_ACCESS_DENIED.to_hresult() {
                HardwareOwnershipError::AlreadyOwned
            } else {
                HardwareOwnershipError::Windows(err)
            }
        })?;

    let wait_result = unsafe { WaitForSingleObject(handle, 0) };
    match wait_result {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(HardwareOwnershipGuard {
            handle,
            _not_send_or_sync: PhantomData,
        }),
        WAIT_TIMEOUT => {
            close_handle(handle);
            Err(HardwareOwnershipError::AlreadyOwned)
        }
        WAIT_FAILED => {
            let err = WindowsError::from_win32();
            close_handle(handle);
            if err.code() == ERROR_ACCESS_DENIED.to_hresult() {
                Err(HardwareOwnershipError::AlreadyOwned)
            } else {
                Err(HardwareOwnershipError::Windows(err))
            }
        }
        other => {
            close_handle(handle);
            Err(HardwareOwnershipError::UnexpectedWaitResult(other.0))
        }
    }
}

impl Drop for HardwareOwnershipGuard {
    fn drop(&mut self) {
        if let Err(err) = unsafe { ReleaseMutex(self.handle) } {
            tracing::warn!(error = %err, "failed to release hardware ownership mutex");
        }
        close_handle(self.handle);
    }
}

fn close_handle(handle: HANDLE) {
    if let Err(err) = unsafe { CloseHandle(handle) } {
        tracing::warn!(error = %err, "failed to close hardware ownership mutex handle");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;

    use super::*;

    static NEXT_MUTEX_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_mutex_name() -> String {
        format!(
            "Local\\ai-corsair-hub-test-{}-{}",
            std::process::id(),
            NEXT_MUTEX_ID.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[test]
    fn guard_release_allows_reacquisition() {
        let name = unique_mutex_name();
        drop(try_acquire_named_mutex(&name).unwrap());
        let _guard = try_acquire_named_mutex(&name).unwrap();
    }

    #[test]
    fn another_thread_cannot_acquire_owned_mutex() {
        let name = unique_mutex_name();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let owner = thread::spawn({
            let name = name.clone();
            move || {
                let _guard = try_acquire_named_mutex(&name).unwrap();
                acquired_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
        });

        acquired_rx.recv().unwrap();
        assert!(matches!(
            try_acquire_named_mutex(&name),
            Err(HardwareOwnershipError::AlreadyOwned)
        ));
        release_tx.send(()).unwrap();
        owner.join().unwrap();

        let _guard = try_acquire_named_mutex(&name).unwrap();
    }
}
