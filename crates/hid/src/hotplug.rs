//! iCUE-style hotplug notifier (see `docs/research/icue-device-detection/A-enumeration.md` F3).
//!
//! A dedicated thread owns a hidden top-level window (not `HWND_MESSAGE`, so it also receives
//! `WM_POWERBROADCAST`) registered for HID and USB device-interface notifications. Arrivals are
//! queued and drained by a second worker thread once Windows has finished installing drivers;
//! removals are reported immediately. Per-device handle notifications (`watch_path`) let the
//! caller release a device cleanly on query-remove.
//!
//! The module only reports events; it never opens or writes to a device other than the
//! zero-access notification handle used by `watch_path`.

use std::collections::HashSet;
#[cfg(not(windows))]
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Result of the injected readiness check for a newly arrived device path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    NotReady(String),
}

/// Handle returned by [`HotplugWatcher::watch_path`]; identifies one per-device removal watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchToken(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotplugEvent {
    /// A device interface arrived and the PnP install queue has drained.
    Arrived { path: String, ready: Readiness },
    /// A device interface was removed.
    Removed { path: String },
    /// System is about to suspend.
    Suspending,
    /// System resumed automatically.
    Resumed,
    /// Windows asked to remove a watched device; its watch handle is already closed.
    QueryRemove { token: WatchToken },
    /// A pending removal was cancelled; the watch handle was reopened.
    QueryRemoveFailed { token: WatchToken },
    /// A watched device is gone.
    RemoveComplete { token: WatchToken },
}

// ---------------------------------------------------------------------------
// Pure logic (platform independent, unit-tested)
// ---------------------------------------------------------------------------

const DBT_DEVICEARRIVAL: u32 = 0x8000;
const DBT_DEVICEQUERYREMOVE: u32 = 0x8001;
const DBT_DEVICEQUERYREMOVEFAILED: u32 = 0x8002;
const DBT_DEVICEREMOVECOMPLETE: u32 = 0x8004;
const DBT_DEVTYP_DEVICEINTERFACE: u32 = 5;
const DBT_DEVTYP_HANDLE: u32 = 6;
const PBT_APMSUSPEND: u32 = 0x4;
const PBT_APMRESUMESUSPEND: u32 = 0x7;
const PBT_APMRESUMEAUTOMATIC: u32 = 0x12;
const WAIT_TIMEOUT: u32 = 258;

/// What a `WM_DEVICECHANGE` message means to us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum DeviceChange {
    InterfaceArrival,
    InterfaceRemoval,
    QueryRemove,
    QueryRemoveFailed,
    HandleRemoveComplete,
    Ignored,
}

/// Classify a `WM_DEVICECHANGE` from `wParam` and the broadcast header's `dbch_devicetype`
/// (`None` when `lParam` carried no header).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn classify_device_change(wparam: usize, devtype: Option<u32>) -> DeviceChange {
    let Some(devtype) = devtype else {
        return DeviceChange::Ignored;
    };
    match (wparam as u32, devtype) {
        (DBT_DEVICEARRIVAL, DBT_DEVTYP_DEVICEINTERFACE) => DeviceChange::InterfaceArrival,
        (DBT_DEVICEREMOVECOMPLETE, DBT_DEVTYP_DEVICEINTERFACE) => DeviceChange::InterfaceRemoval,
        (DBT_DEVICEQUERYREMOVE, DBT_DEVTYP_HANDLE) => DeviceChange::QueryRemove,
        (DBT_DEVICEQUERYREMOVEFAILED, DBT_DEVTYP_HANDLE) => DeviceChange::QueryRemoveFailed,
        (DBT_DEVICEREMOVECOMPLETE, DBT_DEVTYP_HANDLE) => DeviceChange::HandleRemoveComplete,
        _ => DeviceChange::Ignored,
    }
}

/// Map a `WM_POWERBROADCAST` `wParam` to an event.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn classify_power(wparam: usize) -> Option<HotplugEvent> {
    match wparam as u32 {
        PBT_APMSUSPEND => Some(HotplugEvent::Suspending),
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Some(HotplugEvent::Resumed),
        _ => None,
    }
}

/// A resume is reported by both `WM_POWERBROADCAST` and the suspend/resume notification
/// registration, and by both resume codes after a user wake; collapse them into one event.
const RESUME_DEDUPE_WINDOW: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct ResumeDedupe {
    last: Option<Instant>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl ResumeDedupe {
    /// `true` if a resume seen at `now` should be reported, `false` if it repeats one
    /// reported within the last 5 seconds.
    pub(crate) fn should_emit(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last
            && now.saturating_duration_since(last) < RESUME_DEDUPE_WINDOW
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// Read a NUL-terminated UTF-16 name out of a bounded buffer. Stops at the first NUL or the
/// end of `units`, whichever comes first.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn wide_name(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// Whether the arrival worker should poll the PnP install queue again.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn keep_waiting_for_install(wait_result: u32, stopped: bool) -> bool {
    wait_result == WAIT_TIMEOUT && !stopped
}

/// Pending device arrivals, coalesced so one wake drains everything queued.
#[derive(Debug, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct ArrivalQueue {
    paths: Vec<String>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl ArrivalQueue {
    pub(crate) fn push(&mut self, path: String) {
        self.paths.push(path);
    }

    /// Drop a queued arrival for a device that has since been removed.
    pub(crate) fn cancel(&mut self, path: &str) {
        self.paths.retain(|p| !p.eq_ignore_ascii_case(path));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Take every queued path, first-seen order, deduplicated. Device paths are
    /// case-insensitive on Windows, so duplicates are matched ignoring ASCII case.
    pub(crate) fn drain_deduped(&mut self) -> Vec<String> {
        let mut seen = HashSet::new();
        std::mem::take(&mut self.paths)
            .into_iter()
            .filter(|p| seen.insert(p.to_ascii_lowercase()))
            .collect()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
type ReadinessFn = Box<dyn Fn(&str) -> Readiness + Send>;

// ---------------------------------------------------------------------------
// Non-Windows stub
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
pub struct HotplugWatcher;

#[cfg(not(windows))]
impl HotplugWatcher {
    pub fn start(
        _tx: Sender<HotplugEvent>,
        _readiness: Box<dyn Fn(&str) -> Readiness + Send>,
    ) -> anyhow::Result<Self> {
        anyhow::bail!("hotplug notifications are only supported on Windows")
    }

    pub fn watch_path(&self, _path: &str) -> anyhow::Result<WatchToken> {
        anyhow::bail!("hotplug notifications are only supported on Windows")
    }

    pub fn unwatch(&self, _token: WatchToken) {}
}

#[cfg(windows)]
pub use win::HotplugWatcher;

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    use super::*;
    use anyhow::{Context, Result, anyhow};
    use std::collections::HashMap;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
    use std::sync::mpsc::{self, Sender};
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};
    use std::thread::JoinHandle;
    use tracing::{debug, error, warn};
    use windows::Win32::Devices::DeviceAndDriverInstallation::CMP_WaitNoPendingInstallEvents;
    use windows::Win32::Devices::HumanInterfaceDevice::HidD_GetHidGuid;
    use windows::Win32::Devices::Usb::GUID_DEVINTERFACE_USB_DEVICE;
    use windows::Win32::Foundation::{
        CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Power::{
        RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CREATESTRUCTW, CreateWindowExW, DBT_DEVTYP_DEVICEINTERFACE, DBT_DEVTYP_HANDLE,
        DEV_BROADCAST_DEVICEINTERFACE_W, DEV_BROADCAST_HANDLE, DEV_BROADCAST_HDR,
        DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetMessageW,
        GetWindowLongPtrW, HDEVNOTIFY, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
        RegisterDeviceNotificationW, SetWindowLongPtrW, UnregisterClassW,
        UnregisterDeviceNotification, WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE,
        WM_NCCREATE, WM_NCDESTROY, WM_POWERBROADCAST, WNDCLASSEXW, WS_OVERLAPPED,
    };
    use windows::core::{GUID, PCWSTR};

    /// Raw OS handles are not `Send`; these wrappers carry them between our own threads.
    #[derive(Clone, Copy)]
    struct SendHwnd(isize);

    #[derive(Clone, Copy)]
    struct SendNotify(*mut core::ffi::c_void);
    // SAFETY: an HDEVNOTIFY is an opaque registration token; it may be passed to
    // UnregisterDeviceNotification from any thread.
    unsafe impl Send for SendNotify {}

    #[derive(Clone, Copy)]
    struct SendHandle(*mut core::ffi::c_void);
    // SAFETY: a kernel file handle may be used and closed from any thread.
    unsafe impl Send for SendHandle {}

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// State shared by the API object, the window thread and the arrival worker.
    struct Shared {
        stop: AtomicBool,
        queue: Mutex<ArrivalQueue>,
        wake: Condvar,
        tx: Sender<HotplugEvent>,
    }

    impl Shared {
        fn send(&self, event: HotplugEvent) {
            // A dropped receiver just means nobody is listening any more.
            let _ = self.tx.send(event);
        }
    }

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// One per-device removal watch.
    struct WatchEntry {
        path: Vec<u16>,
        /// `None` between `QUERYREMOVE` (handle closed) and `QUERYREMOVEFAILED` (reopened).
        handle: Option<SendHandle>,
        notify: Option<SendNotify>,
    }

    struct Watches {
        /// Set once the window exists; 0 before that.
        hwnd: AtomicIsize,
        next_token: AtomicU64,
        entries: Mutex<HashMap<u64, WatchEntry>>,
    }

    /// Everything the window procedure needs, reachable through `GWLP_USERDATA`.
    struct WndContext {
        shared: Arc<Shared>,
        watches: Arc<Watches>,
        resume: Mutex<ResumeDedupe>,
    }

    pub struct HotplugWatcher {
        shared: Arc<Shared>,
        watches: Arc<Watches>,
        hwnd: SendHwnd,
        window_thread: Option<JoinHandle<()>>,
        worker_thread: Option<JoinHandle<()>>,
    }

    impl HotplugWatcher {
        /// Start the notifier. `readiness` runs on the arrival worker thread for each
        /// distinct arrived path once the PnP install queue is idle.
        pub fn start(tx: Sender<HotplugEvent>, readiness: ReadinessFn) -> Result<Self> {
            let shared = Arc::new(Shared {
                stop: AtomicBool::new(false),
                queue: Mutex::new(ArrivalQueue::default()),
                wake: Condvar::new(),
                tx,
            });
            // The window thread hands back its HWND (or the setup error) once it is live.
            let (ready_tx, ready_rx) = mpsc::channel::<Result<(SendHwnd, Arc<Watches>), String>>();

            let window_thread = {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name("hotplug-window".into())
                    .spawn(move || window_thread_main(shared, ready_tx))
                    .context("spawning hotplug window thread")?
            };

            let (hwnd, watches) = match ready_rx.recv() {
                Ok(Ok(ok)) => ok,
                Ok(Err(msg)) => {
                    let _ = window_thread.join();
                    return Err(anyhow!(msg));
                }
                Err(_) => {
                    let _ = window_thread.join();
                    return Err(anyhow!("hotplug window thread exited during setup"));
                }
            };

            let worker_thread = {
                let worker_shared = Arc::clone(&shared);
                match std::thread::Builder::new()
                    .name("hotplug-arrivals".into())
                    .spawn(move || arrival_worker_main(worker_shared, readiness))
                {
                    Ok(h) => h,
                    Err(e) => {
                        shutdown_window(&shared, hwnd);
                        let _ = window_thread.join();
                        return Err(anyhow!(e).context("spawning hotplug arrival worker"));
                    }
                }
            };

            Ok(Self {
                shared,
                watches,
                hwnd,
                window_thread: Some(window_thread),
                worker_thread: Some(worker_thread),
            })
        }

        /// Watch one device path for query-remove / remove-complete. Opens a zero-access
        /// handle that does not block removal; the handle is closed on `QUERYREMOVE`.
        pub fn watch_path(&self, path: &str) -> Result<WatchToken> {
            let wide = to_wide(path);
            let handle = open_notification_handle(&wide)
                .with_context(|| format!("opening notification handle for {path}"))?;
            let notify = match register_handle_notification(self.hwnd, handle) {
                Ok(n) => n,
                Err(e) => {
                    close_handle(handle);
                    return Err(e).context("registering handle notification");
                }
            };
            let id = self.watches.next_token.fetch_add(1, Ordering::Relaxed);
            lock(&self.watches.entries).insert(
                id,
                WatchEntry {
                    path: wide,
                    handle: Some(handle),
                    notify: Some(notify),
                },
            );
            Ok(WatchToken(id))
        }

        /// Stop watching a device. Unknown or already-completed tokens are ignored.
        pub fn unwatch(&self, token: WatchToken) {
            let entry = lock(&self.watches.entries).remove(&token.0);
            if let Some(entry) = entry {
                release_entry(entry);
            }
        }
    }

    impl Drop for HotplugWatcher {
        fn drop(&mut self) {
            shutdown_window(&self.shared, self.hwnd);
            if let Some(t) = self.window_thread.take()
                && t.join().is_err()
            {
                error!("hotplug window thread panicked");
            }
            if let Some(t) = self.worker_thread.take()
                && t.join().is_err()
            {
                error!("hotplug arrival worker panicked");
            }
            // The window thread releases watches on WM_DESTROY; sweep any registered after.
            let leftovers: Vec<_> = lock(&self.watches.entries)
                .drain()
                .map(|(_, e)| e)
                .collect();
            for entry in leftovers {
                release_entry(entry);
            }
        }
    }

    /// Ask the worker to stop and the window thread to leave its message loop.
    fn shutdown_window(shared: &Shared, hwnd: SendHwnd) {
        {
            // Hold the queue lock while flagging so the worker can't miss the wakeup.
            let _q = lock(&shared.queue);
            shared.stop.store(true, Ordering::SeqCst);
        }
        shared.wake.notify_all();
        // SAFETY: PostMessageW is thread-safe; an already-destroyed window just returns an error.
        let _ =
            unsafe { PostMessageW(Some(HWND(hwnd.0 as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }

    fn close_handle(h: SendHandle) {
        // SAFETY: `h` came from CreateFileW and is closed exactly once by its owning entry.
        let _ = unsafe { CloseHandle(HANDLE(h.0)) };
    }

    fn release_entry(entry: WatchEntry) {
        if let Some(n) = entry.notify {
            // SAFETY: `n` came from RegisterDeviceNotificationW and is unregistered once.
            let _ = unsafe { UnregisterDeviceNotification(HDEVNOTIFY(n.0)) };
        }
        if let Some(h) = entry.handle {
            close_handle(h);
        }
    }

    fn open_notification_handle(wide_path: &[u16]) -> Result<SendHandle> {
        // SAFETY: `wide_path` is NUL-terminated and outlives the call; access 0 means the
        // handle can't read or write the device and doesn't block removal.
        let h = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }?;
        Ok(SendHandle(h.0))
    }

    fn register_handle_notification(hwnd: SendHwnd, handle: SendHandle) -> Result<SendNotify> {
        let filter = DEV_BROADCAST_HANDLE {
            dbch_size: size_of::<DEV_BROADCAST_HANDLE>() as u32,
            dbch_devicetype: DBT_DEVTYP_HANDLE.0,
            dbch_handle: HANDLE(handle.0),
            ..Default::default()
        };
        // SAFETY: `filter` is a fully initialised DEV_BROADCAST_HANDLE that lives across the
        // call; the window handle stays valid until the watcher is dropped.
        let n = unsafe {
            RegisterDeviceNotificationW(
                HANDLE(hwnd.0 as *mut _),
                &filter as *const _ as *const core::ffi::c_void,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }?;
        Ok(SendNotify(n.0))
    }

    fn register_interface_notification(hwnd: HWND, guid: GUID) -> Result<SendNotify> {
        let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
            dbcc_classguid: guid,
            ..Default::default()
        };
        // SAFETY: `filter` is fully initialised and lives across the call; `hwnd` is the live
        // window created by this thread.
        let n = unsafe {
            RegisterDeviceNotificationW(
                HANDLE(hwnd.0),
                &filter as *const _ as *const core::ffi::c_void,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }?;
        Ok(SendNotify(n.0))
    }

    // -- window thread ------------------------------------------------------

    static CLASS_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn window_thread_main(
        shared: Arc<Shared>,
        ready: mpsc::Sender<Result<(SendHwnd, Arc<Watches>), String>>,
    ) {
        let class_name = to_wide(&format!(
            "CorsairHubHotplug_{}_{}",
            std::process::id(),
            CLASS_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        // SAFETY: GetModuleHandleW(None) returns the current module and has no preconditions.
        let hinstance = match unsafe { GetModuleHandleW(PCWSTR::null()) } {
            Ok(h) => HINSTANCE(h.0),
            Err(e) => {
                let _ = ready.send(Err(format!("GetModuleHandleW failed: {e}")));
                return;
            }
        };

        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised; the class-name buffer outlives the window.
        if unsafe { RegisterClassExW(&class) } == 0 {
            let _ = ready.send(Err(format!(
                "RegisterClassExW failed: {}",
                windows::core::Error::from_win32()
            )));
            return;
        }

        // The context lives on this stack frame, which outlives the window and its messages.
        let watches = Arc::new(Watches {
            hwnd: AtomicIsize::new(0),
            next_token: AtomicU64::new(1),
            entries: Mutex::new(HashMap::new()),
        });
        let ctx = WndContext {
            shared: Arc::clone(&shared),
            watches: Arc::clone(&watches),
            resume: Mutex::new(ResumeDedupe::default()),
        };

        // SAFETY: top-level (parentless), never shown window; `ctx` outlives it and is only
        // read through the pointer stashed in GWLP_USERDATA during WM_NCCREATE.
        let hwnd = match unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(class_name.as_ptr()),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinstance),
                Some(&ctx as *const WndContext as *const core::ffi::c_void),
            )
        } {
            Ok(h) => h,
            Err(e) => {
                // SAFETY: the class was registered above by this module and has no windows.
                let _ = unsafe { UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(hinstance)) };
                let _ = ready.send(Err(format!("CreateWindowExW failed: {e}")));
                return;
            }
        };

        // SAFETY: HidD_GetHidGuid has no preconditions.
        let hid_guid = unsafe { HidD_GetHidGuid() };
        let mut interface_notifies = Vec::new();
        for guid in [hid_guid, GUID_DEVINTERFACE_USB_DEVICE] {
            match register_interface_notification(hwnd, guid) {
                Ok(n) => interface_notifies.push(n),
                Err(e) => {
                    for n in interface_notifies {
                        // SAFETY: registered above, unregistered once.
                        let _ = unsafe { UnregisterDeviceNotification(HDEVNOTIFY(n.0)) };
                    }
                    // SAFETY: window and class were created above on this thread.
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                        let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(hinstance));
                    }
                    let _ = ready.send(Err(format!("RegisterDeviceNotificationW failed: {e}")));
                    return;
                }
            }
        }

        // Complements WM_POWERBROADCAST; failure only costs the redundant resume signal.
        // SAFETY: `hwnd` is the live window created above on this thread.
        let power_notify = match unsafe {
            RegisterSuspendResumeNotification(HANDLE(hwnd.0), DEVICE_NOTIFY_WINDOW_HANDLE)
        } {
            Ok(n) => Some(n),
            Err(e) => {
                warn!(error = %e, "RegisterSuspendResumeNotification failed");
                None
            }
        };

        watches.hwnd.store(hwnd.0 as isize, Ordering::SeqCst);

        let _ = ready.send(Ok((SendHwnd(hwnd.0 as isize), Arc::clone(&watches))));

        // Message loop: ends when WM_DESTROY posts WM_QUIT.
        let mut msg = MSG::default();
        // SAFETY: standard message pump on the thread that owns the window.
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                DispatchMessageW(&msg);
            }
        }

        // The window is gone (WM_NCDESTROY cleared GWLP_USERDATA) before `ctx` drops.
        if let Some(n) = power_notify {
            // SAFETY: registered above, unregistered once.
            let _ = unsafe { UnregisterSuspendResumeNotification(n) };
        }
        for n in interface_notifies {
            // SAFETY: registered above, unregistered once (idempotent failure is ignored).
            let _ = unsafe { UnregisterDeviceNotification(HDEVNOTIFY(n.0)) };
        }
        release_all(&watches);
        // SAFETY: all windows of this class are destroyed.
        let _ = unsafe { UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(hinstance)) };
    }

    fn release_all(watches: &Watches) {
        let all: Vec<_> = lock(&watches.entries).drain().map(|(_, e)| e).collect();
        for entry in all {
            release_entry(entry);
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_NCCREATE {
            // SAFETY: for WM_NCCREATE, lParam points to the CREATESTRUCTW whose lpCreateParams
            // is the WndContext pointer we passed to CreateWindowExW.
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                }
            }
        }
        // SAFETY: GWLP_USERDATA is either 0 or a pointer to the WndContext, which the window
        // thread keeps alive until after the message loop ends.
        let ctx = unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WndContext).as_ref() };

        let handled = catch_unwind(AssertUnwindSafe(|| {
            ctx.and_then(|ctx| handle_message(ctx, hwnd, msg, wparam, lparam))
        }));
        match handled {
            Ok(Some(result)) => result,
            Ok(None) => {
                if msg == WM_DESTROY {
                    // SAFETY: ends the message loop of this thread.
                    unsafe { PostQuitMessage(0) };
                }
                if msg == WM_NCDESTROY {
                    // SAFETY: the window is going away; stop handing out the context.
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
                }
                // SAFETY: forwarding the unmodified message.
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            Err(_) => {
                error!(msg, "panic in hotplug window procedure");
                if msg == WM_DESTROY {
                    // SAFETY: still must end the loop so Drop can join.
                    unsafe { PostQuitMessage(0) };
                }
                // SAFETY: forwarding the unmodified message.
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
    }

    /// `Some(result)` when fully handled, `None` to fall through to `DefWindowProcW`.
    fn handle_message(
        ctx: &WndContext,
        _hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match msg {
            WM_DEVICECHANGE => {
                on_device_change(ctx, wparam.0, lparam.0 as *const DEV_BROADCAST_HDR);
                // TRUE grants query-remove and is ignored for other notifications.
                Some(LRESULT(1))
            }
            WM_POWERBROADCAST => {
                if let Some(event) = classify_power(wparam.0)
                    && (event != HotplugEvent::Resumed
                        || lock(&ctx.resume).should_emit(Instant::now()))
                {
                    ctx.shared.send(event);
                }
                Some(LRESULT(1))
            }
            WM_DESTROY => {
                release_all(&ctx.watches);
                None
            }
            _ => None,
        }
    }

    fn on_device_change(ctx: &WndContext, wparam: usize, header: *const DEV_BROADCAST_HDR) {
        // SAFETY: lParam is null or points to a DEV_BROADCAST_HDR valid for the duration of the
        // message; the size is validated before reading anything beyond the header.
        let hdr = unsafe { header.as_ref() };
        let devtype = hdr
            .filter(|h| h.dbch_size as usize >= size_of::<DEV_BROADCAST_HDR>())
            .map(|h| h.dbch_devicetype.0);
        match classify_device_change(wparam, devtype) {
            DeviceChange::InterfaceArrival => {
                if let Some(path) = interface_path(header) {
                    debug!(%path, "device interface arrival");
                    lock(&ctx.shared.queue).push(path);
                    ctx.shared.wake.notify_one();
                }
            }
            DeviceChange::InterfaceRemoval => {
                if let Some(path) = interface_path(header) {
                    debug!(%path, "device interface removal");
                    lock(&ctx.shared.queue).cancel(&path);
                    ctx.shared.send(HotplugEvent::Removed { path });
                }
            }
            DeviceChange::QueryRemove => {
                if let Some(notify) = handle_notify(header) {
                    on_query_remove(ctx, notify);
                }
            }
            DeviceChange::QueryRemoveFailed => {
                if let Some(notify) = handle_notify(header) {
                    on_query_remove_failed(ctx, notify);
                }
            }
            DeviceChange::HandleRemoveComplete => {
                if let Some(notify) = handle_notify(header) {
                    on_remove_complete(ctx, notify);
                }
            }
            DeviceChange::Ignored => {}
        }
    }

    /// Extract the NUL-terminated device path from a `DBT_DEVTYP_DEVICEINTERFACE` broadcast.
    fn interface_path(header: *const DEV_BROADCAST_HDR) -> Option<String> {
        let name_offset = std::mem::offset_of!(DEV_BROADCAST_DEVICEINTERFACE_W, dbcc_name);
        // SAFETY: classify_device_change confirmed a device-interface broadcast, so the buffer
        // is a DEV_BROADCAST_DEVICEINTERFACE_W of `dbch_size` bytes; the name slice is bounded
        // by that size.
        unsafe {
            let size = (*header).dbch_size as usize;
            if size <= name_offset {
                return None;
            }
            let name = (header as *const u8).add(name_offset) as *const u16;
            let units = std::slice::from_raw_parts(name, (size - name_offset) / 2);
            Some(wide_name(units))
        }
    }

    fn handle_notify(header: *const DEV_BROADCAST_HDR) -> Option<*mut core::ffi::c_void> {
        // SAFETY: classify_device_change confirmed a DBT_DEVTYP_HANDLE broadcast of
        // `dbch_size` bytes (>= the header, validated by the caller).
        unsafe { read_handle_notify(header as *const u8, (*header).dbch_size as usize) }
    }

    /// Read only `dbch_hdevnotify` from a `DEV_BROADCAST_HANDLE` of `size` bytes at `base`,
    /// provided `size` covers it (packed layout, so read unaligned).
    ///
    /// # Safety
    /// `base` must be valid for reads of `size` bytes.
    pub(super) unsafe fn read_handle_notify(
        base: *const u8,
        size: usize,
    ) -> Option<*mut core::ffi::c_void> {
        let offset = std::mem::offset_of!(DEV_BROADCAST_HANDLE, dbch_hdevnotify);
        if size < offset + size_of::<HDEVNOTIFY>() {
            return None;
        }
        // SAFETY: the check above keeps the field inside the `size` readable bytes.
        let n = unsafe { (base.add(offset) as *const HDEVNOTIFY).read_unaligned() };
        Some(n.0)
    }

    fn token_for(
        entries: &HashMap<u64, WatchEntry>,
        notify: *mut core::ffi::c_void,
    ) -> Option<u64> {
        entries
            .iter()
            .find(|(_, e)| e.notify.is_some_and(|n| n.0 == notify))
            .map(|(id, _)| *id)
    }

    fn on_query_remove(ctx: &WndContext, notify: *mut core::ffi::c_void) {
        let token = {
            let mut entries = lock(&ctx.watches.entries);
            let Some(id) = token_for(&entries, notify) else {
                return;
            };
            // Close immediately so Windows can proceed with the removal.
            if let Some(h) = entries.get_mut(&id).and_then(|e| e.handle.take()) {
                close_handle(h);
            }
            WatchToken(id)
        };
        ctx.shared.send(HotplugEvent::QueryRemove { token });
    }

    fn on_query_remove_failed(ctx: &WndContext, notify: *mut core::ffi::c_void) {
        let hwnd = SendHwnd(ctx.watches.hwnd.load(Ordering::SeqCst));
        let token = {
            let mut entries = lock(&ctx.watches.entries);
            let Some(id) = token_for(&entries, notify) else {
                return;
            };
            let Some(entry) = entries.get_mut(&id) else {
                return;
            };
            if let Some(n) = entry.notify.take() {
                // SAFETY: registered by us, unregistered once; re-registered below.
                let _ = unsafe { UnregisterDeviceNotification(HDEVNOTIFY(n.0)) };
            }
            if let Some(h) = entry.handle.take() {
                close_handle(h);
            }
            match open_notification_handle(&entry.path)
                .and_then(|h| register_handle_notification(hwnd, h).map(|n| (h, n)))
            {
                Ok((h, n)) => {
                    entry.handle = Some(h);
                    entry.notify = Some(n);
                }
                Err(e) => warn!(error = %e, "reopening watch after query-remove failure"),
            }
            WatchToken(id)
        };
        ctx.shared.send(HotplugEvent::QueryRemoveFailed { token });
    }

    fn on_remove_complete(ctx: &WndContext, notify: *mut core::ffi::c_void) {
        let (token, entry) = {
            let mut entries = lock(&ctx.watches.entries);
            let Some(id) = token_for(&entries, notify) else {
                return;
            };
            (WatchToken(id), entries.remove(&id))
        };
        if let Some(entry) = entry {
            release_entry(entry);
        }
        ctx.shared.send(HotplugEvent::RemoveComplete { token });
    }

    // -- arrival worker -----------------------------------------------------

    fn arrival_worker_main(shared: Arc<Shared>, readiness: ReadinessFn) {
        loop {
            let batch = {
                let mut q = lock(&shared.queue);
                while q.is_empty() && !shared.stop.load(Ordering::SeqCst) {
                    q = shared.wake.wait(q).unwrap_or_else(|e| e.into_inner());
                }
                if shared.stop.load(Ordering::SeqCst) {
                    return;
                }
                // One wake drains everything queued so far.
                q.drain_deduped()
            };

            // Wait for Windows to finish installing drivers for the new devices.
            loop {
                // SAFETY: CMP_WaitNoPendingInstallEvents only blocks for the given timeout.
                let r = unsafe { CMP_WaitNoPendingInstallEvents(1000) };
                if !keep_waiting_for_install(r, shared.stop.load(Ordering::SeqCst)) {
                    break;
                }
            }
            if shared.stop.load(Ordering::SeqCst) {
                return;
            }

            for path in batch {
                let ready = catch_unwind(AssertUnwindSafe(|| readiness(&path)))
                    .unwrap_or_else(|_| Readiness::NotReady("readiness check panicked".into()));
                shared.send(HotplugEvent::Arrived { path, ready });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_arrival_and_removal_of_interfaces() {
        assert_eq!(
            classify_device_change(0x8000, Some(5)),
            DeviceChange::InterfaceArrival
        );
        assert_eq!(
            classify_device_change(0x8004, Some(5)),
            DeviceChange::InterfaceRemoval
        );
    }

    #[test]
    fn classify_handle_notifications() {
        assert_eq!(
            classify_device_change(0x8001, Some(6)),
            DeviceChange::QueryRemove
        );
        assert_eq!(
            classify_device_change(0x8002, Some(6)),
            DeviceChange::QueryRemoveFailed
        );
        assert_eq!(
            classify_device_change(0x8004, Some(6)),
            DeviceChange::HandleRemoveComplete
        );
    }

    #[test]
    fn classify_ignores_other_combinations() {
        // Arrival of a handle, query-remove of an interface, volumes (type 2), unknown codes.
        assert_eq!(
            classify_device_change(0x8000, Some(6)),
            DeviceChange::Ignored
        );
        assert_eq!(
            classify_device_change(0x8001, Some(5)),
            DeviceChange::Ignored
        );
        assert_eq!(
            classify_device_change(0x8000, Some(2)),
            DeviceChange::Ignored
        );
        assert_eq!(
            classify_device_change(0x0007, Some(5)),
            DeviceChange::Ignored
        );
        // DBT_DEVNODES_CHANGED carries no header.
        assert_eq!(classify_device_change(0x0007, None), DeviceChange::Ignored);
        assert_eq!(classify_device_change(0x8000, None), DeviceChange::Ignored);
    }

    #[test]
    fn classify_ignores_high_wparam_bits() {
        // wParam is pointer-sized; only the low 32 bits carry the DBT code.
        let wparam = (1usize << 40) | 0x8000;
        assert_eq!(
            classify_device_change(wparam, Some(5)),
            DeviceChange::InterfaceArrival
        );
    }

    #[test]
    fn classify_power_events() {
        assert_eq!(classify_power(0x4), Some(HotplugEvent::Suspending));
        assert_eq!(classify_power(0x12), Some(HotplugEvent::Resumed));
        assert_eq!(classify_power(0x7), Some(HotplugEvent::Resumed));
        // PBT_POWERSETTINGCHANGE (0x8013) is not reported.
        assert_eq!(classify_power(0x8013), None);
    }

    #[test]
    fn resume_dedupe_ignores_repeat_within_window() {
        let t0 = Instant::now();
        let mut d = ResumeDedupe::default();
        assert!(d.should_emit(t0));
        assert!(!d.should_emit(t0 + Duration::from_millis(10)));
        assert!(!d.should_emit(t0 + Duration::from_millis(4999)));
        // The window is anchored on the reported resume, not on suppressed repeats.
        assert!(d.should_emit(t0 + Duration::from_secs(5)));
        assert!(!d.should_emit(t0 + Duration::from_secs(6)));
    }

    #[test]
    fn resume_dedupe_tolerates_backwards_clock() {
        let t0 = Instant::now() + Duration::from_secs(10);
        let mut d = ResumeDedupe::default();
        assert!(d.should_emit(t0));
        assert!(!d.should_emit(t0 - Duration::from_secs(1)));
    }

    #[test]
    fn wide_name_stops_at_nul_and_end() {
        let with_nul: Vec<u16> = "\\\\?\\hid#vid_1b1c\0garbage".encode_utf16().collect();
        assert_eq!(wide_name(&with_nul), "\\\\?\\hid#vid_1b1c");
        let no_nul: Vec<u16> = "abc".encode_utf16().collect();
        assert_eq!(wide_name(&no_nul), "abc");
        assert_eq!(wide_name(&[]), "");
        assert_eq!(wide_name(&[0]), "");
    }

    #[test]
    fn install_wait_loop_condition() {
        assert!(keep_waiting_for_install(258, false));
        assert!(!keep_waiting_for_install(258, true));
        assert!(!keep_waiting_for_install(0, false));
    }

    #[test]
    fn queue_drain_coalesces_and_dedupes() {
        let mut q = ArrivalQueue::default();
        assert!(q.is_empty());
        q.push("\\\\?\\hid#a".into());
        q.push("\\\\?\\hid#b".into());
        q.push("\\\\?\\HID#A".into());
        q.push("\\\\?\\hid#a".into());
        q.push("\\\\?\\hid#c".into());
        assert!(!q.is_empty());
        assert_eq!(
            q.drain_deduped(),
            vec!["\\\\?\\hid#a", "\\\\?\\hid#b", "\\\\?\\hid#c"]
        );
        assert!(q.is_empty());
        assert!(q.drain_deduped().is_empty());
    }

    #[test]
    fn queue_cancel_drops_removed_device() {
        let mut q = ArrivalQueue::default();
        q.push("\\\\?\\hid#a".into());
        q.push("\\\\?\\hid#b".into());
        q.cancel("\\\\?\\HID#A");
        assert_eq!(q.drain_deduped(), vec!["\\\\?\\hid#b"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn start_fails_off_windows() {
        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(HotplugWatcher::start(tx, Box::new(|_| Readiness::Ready)).is_err());
    }

    /// A broadcast too short to hold `dbch_hdevnotify` must not be read past its end.
    #[cfg(windows)]
    #[test]
    fn handle_notify_respects_buffer_size() {
        use windows::Win32::UI::WindowsAndMessaging::{DEV_BROADCAST_HANDLE, HDEVNOTIFY};
        let offset = std::mem::offset_of!(DEV_BROADCAST_HANDLE, dbch_hdevnotify);
        let field = size_of::<HDEVNOTIFY>();
        let mut buf = vec![0u8; offset + field];
        let marker = 0x1234usize;
        buf[offset..].copy_from_slice(&marker.to_ne_bytes());
        // SAFETY: `buf` is valid for `buf.len()` bytes; shorter sizes only read less.
        unsafe {
            assert_eq!(
                win::read_handle_notify(buf.as_ptr(), buf.len()).map(|p| p as usize),
                Some(marker)
            );
            assert!(win::read_handle_notify(buf.as_ptr(), buf.len() - 1).is_none());
            assert!(win::read_handle_notify(buf.as_ptr(), 0).is_none());
        }
    }

    /// Creates a real (hidden) window; run explicitly with `-- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "creates a real OS window and registers device notifications"]
    fn start_and_drop_within_two_seconds() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let began = std::time::Instant::now();
        let watcher = HotplugWatcher::start(tx, Box::new(|_| Readiness::Ready)).expect("start");
        drop(watcher);
        assert!(
            began.elapsed() < std::time::Duration::from_secs(2),
            "start+drop took {:?}",
            began.elapsed()
        );
    }
}
