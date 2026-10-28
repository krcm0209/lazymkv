//! The Windows `DriveWatcher` (spec 4.3; design parts 4.2, 5.4; L1a; Ruling W8): a hidden window on a thread of its
//! own, told of device changes, and a poll every `DRIVE_POLL_EVERY`, both feeding one `DriveQueue`. The drive id, the
//! scan's filter, whether a volume answers and the queue sit in `drives`, compiled and tested on every system.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};
use windows_sys::Win32::System::Diagnostics::Debug::{SetThreadErrorMode, SEM_FAILCRITICALERRORS};
use windows_sys::Win32::System::Ioctl::GUID_DEVINTERFACE_CDROM;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterDeviceNotificationW, SetTimer, UnregisterClassW,
    UnregisterDeviceNotification, CW_USEDEFAULT, DBT_DEVTYP_DEVICEINTERFACE, DEVICE_NOTIFY_WINDOW_HANDLE,
    DEV_BROADCAST_DEVICEINTERFACE_W, HDEVNOTIFY, MSG, WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE, WM_TIMER, WNDCLASSW,
};

use super::wide;
use crate::drives::{drive_id, ready_drives, volume_answers, DriveQueue, DRIVE_POLL_EVERY};
use crate::{DriveEvent, DriveWatcher};

/// The window class of every watcher's window. A class belongs to the process, so the watchers share it.
const CLASS: &str = "lazymkv-drive-watcher";
/// The title of a watcher's window, which is never shown.
const TITLE: &str = "lazymkv drive watcher";
/// The id of the poll's timer on a watcher's window.
const TIMER: usize = 1;

/// The queue `next` and `present` read and the window thread feeds, with the condition variable `next` waits on.
type Shared = Arc<(Mutex<DriveQueue>, Condvar)>;

/// Held while a watcher registers the class and creates its window, and while a watcher's thread unregisters the
/// class at its end, so the class is never unregistered between another watcher's registration and its window.
static CLASS_LOCK: Mutex<()> = Mutex::new(());

/// What the window procedure of this thread's window reaches (Ruling W8).
struct Watch {
    /// The queue the scans feed.
    queue: Shared,
    /// The last scan, which a locked volume keeps its answer from (Ruling T3-16-a).
    last: BTreeMap<String, PathBuf>,
    /// The device notification WM_DESTROY unregisters; null when Windows refused it or once it is unregistered.
    notification: HDEVNOTIFY,
}

thread_local! {
    /// The watch of the window this thread owns: each watcher's window lives on a thread of its own.
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
}

/// The Windows `DriveWatcher` (spec 4.3; design parts 4.2, 5.4; L1a; Ruling W8): a thread of its own owns a hidden
/// top-level window, which receives `WM_DEVICECHANGE` broadcasts and CD-ROM device interface notifications
/// (message-only windows receive no broadcasts), and scans the drives on each of them and every `DRIVE_POLL_EVERY`,
/// feeding one `DriveQueue` that `next` and `present` read. The message loop runs on this thread, not on the engine's
/// drive thread (W8, Owner decision 11). It holds no handle on any volume, so the `Ejector`'s lock is never refused
/// because of it (design part 5.6 step 8). Def: T3-16. Use: T3-23.
pub struct WinDrives {
    /// The queue the window thread feeds.
    queue: Shared,
    /// The window (its HWND as a number, so the watcher can move to another thread), which drop closes.
    window: usize,
    /// The window thread, joined on drop.
    thread: Option<JoinHandle<()>>,
}

impl WinDrives {
    /// Starts the window thread and returns once the window is registered for notifications and its first scan is the
    /// queue's state, so `present` is the state at start and no change after that scan is lost (L1a). Err is Windows'
    /// error for the window class, the window or the timer. Def: T3-16. Use: T3-23.
    pub fn new() -> io::Result<WinDrives> {
        let queue: Shared = Arc::new((Mutex::new(DriveQueue::new(BTreeMap::new())), Condvar::new()));
        let (report, started) = mpsc::channel();
        let fed = Arc::clone(&queue);
        let thread = std::thread::Builder::new()
            .name("lazymkv-drive-watcher".into())
            .spawn(move || watch(fed, report))?;
        match started.recv() {
            Ok(Ok(window)) => Ok(WinDrives { queue, window, thread: Some(thread) }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err(io::Error::other("the drive watcher's thread ended before its window was ready"))
            }
        }
    }
}

impl DriveWatcher for WinDrives {
    fn next(&mut self, timeout: Duration) -> Option<DriveEvent> {
        let start = Instant::now();
        let (queue, changed) = &*self.queue;
        let mut queue = lock(queue);
        loop {
            if let Some(event) = queue.pop() {
                return Some(event);
            }
            let left = timeout.checked_sub(start.elapsed()).filter(|left| !left.is_zero())?;
            queue = changed.wait_timeout(queue, left).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    fn present(&self) -> Vec<(String, PathBuf)> {
        lock(&self.queue.0).present()
    }
}

impl Drop for WinDrives {
    fn drop(&mut self) {
        // SAFETY: `window` is the window the thread created; PostMessageW only queues WM_CLOSE for it, with no
        // pointer among its parameters, and fails when the window is already gone.
        unsafe { PostMessageW(self.window as HWND, WM_CLOSE, 0, 0) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The lock, taken through a poisoned state, so a panic elsewhere does not stop the watcher.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Asks the volume of `letter`'s root for its information and reads none of it back (Ruling W8). Err is Windows'
/// error, which `volume_answers` reads.
fn volume_information(letter: char) -> Result<(), u32> {
    let root = wide(format!(r"{letter}:\"));
    // SAFETY: `root` is NUL-terminated and outlives the call, which only reads it; every buffer is null with a size of
    // zero, which the call accepts for each value, so it writes nothing.
    let read = unsafe {
        GetVolumeInformationW(root.as_ptr(), null_mut(), 0, null_mut(), null_mut(), null_mut(), null_mut(), 0)
    };
    if read != 0 {
        Ok(())
    } else {
        // SAFETY: GetLastError takes no argument and reads this thread's last error, which the failed call set.
        Err(unsafe { GetLastError() })
    }
}

/// One scan (Ruling W8): the optical drives whose volume answers, where a locked volume keeps its answer in `last`
/// (Ruling T3-16-a).
fn scan(last: &BTreeMap<String, PathBuf>) -> BTreeMap<String, PathBuf> {
    // SAFETY: GetLogicalDrives takes no argument.
    let mask = unsafe { GetLogicalDrives() };
    let kind = |letter: char| {
        let root = wide(format!(r"{letter}:\"));
        // SAFETY: `root` is NUL-terminated and outlives the call, which only reads it.
        unsafe { GetDriveTypeW(root.as_ptr()) }
    };
    ready_drives(mask, kind, |letter| volume_answers(volume_information(letter), last.contains_key(&drive_id(letter))))
}

/// Runs `work` on this thread's watch and returns its result, or None when the thread has no watch or it is in use.
fn with_watch<R>(work: impl FnOnce(&mut Watch) -> R) -> Option<R> {
    WATCH
        .try_with(|slot| {
            let mut slot = slot.try_borrow_mut().ok()?;
            slot.as_mut().map(work)
        })
        .ok()
        .flatten()
}

/// Scans against this thread's last scan, hands a copy of the result to the queue, keeps the result as the last scan
/// and wakes `next` (Ruling W8).
fn rescan() {
    with_watch(|watch| {
        let now = scan(&watch.last);
        let (queue, changed) = &*watch.queue;
        lock(queue).observe(now.clone());
        watch.last = now;
        changed.notify_all();
    });
}

/// The window procedure of every watcher's window (Ruling W8): a device change and the poll's timer scan the drives,
/// WM_CLOSE destroys the window, and WM_DESTROY lets the device notification and the timer go and ends the message
/// loop. Every other message gets Windows' default.
///
/// # Safety
///
/// Only Windows calls it, for a window of the class on this thread, with the message's own parameters.
unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_DEVICECHANGE => {
            rescan();
            // TRUE, so a query to remove a device is never denied.
            1
        }
        WM_TIMER => {
            rescan();
            0
        }
        WM_CLOSE => {
            // SAFETY: `window` is this thread's window, which the message is for.
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            let notification = with_watch(|watch| std::mem::replace(&mut watch.notification, null_mut()));
            let notification = notification.unwrap_or(null_mut());
            if !notification.is_null() {
                // SAFETY: the handle came from this window's successful RegisterDeviceNotificationW, and the watch now
                // holds null in its place, so it is unregistered once.
                unsafe { UnregisterDeviceNotification(notification) };
            }
            // SAFETY: `window` is this thread's window, still valid during WM_DESTROY; the call only fails when the
            // window has no such timer.
            unsafe { KillTimer(window, TIMER) };
            // SAFETY: PostQuitMessage takes an exit code and posts WM_QUIT to this thread's queue.
            unsafe { PostQuitMessage(0) };
            0
        }
        // SAFETY: Windows' default handling, with the parameters Windows passed for `window`, unchanged.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// The window thread (Ruling W8), in the order that loses no change between the first scan and the first
/// notification: no "no disk" dialog for this thread, the class, the window, the device notification, the timer and
/// the first scan, then the message loop until WM_QUIT. `report` gets the window, or Windows' error for the class, the
/// window or the timer, after which the thread ends. At the end the class is unregistered, which Windows refuses while
/// another watcher's window uses it.
fn watch(queue: Shared, report: mpsc::Sender<io::Result<usize>>) {
    // SAFETY: the call sets this thread's error mode only, and a null pointer for the old mode is accepted.
    unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS, null_mut()) };
    let empty = Watch { queue: Arc::clone(&queue), last: BTreeMap::new(), notification: null_mut() };
    WATCH.with(|slot| *slot.borrow_mut() = Some(empty));
    // SAFETY: a null name asks for the running program's own module, whose handle is not released.
    let module = unsafe { GetModuleHandleW(null()) };
    let class = wide(CLASS);
    match open_window(module, &class).and_then(|window| start(window, &queue)) {
        Ok(window) => {
            let _ = report.send(Ok(window as usize));
            pump(window);
        }
        Err(error) => {
            let _ = report.send(Err(error));
        }
    }
    let _class = lock(&CLASS_LOCK);
    // SAFETY: `class` is NUL-terminated and outlives the call, which only reads it; Windows refuses the call while a
    // window of the class still lives.
    unsafe { UnregisterClassW(class.as_ptr(), module) };
}

/// Steps 2 and 3, under `CLASS_LOCK`: registers the class, which another watcher may have registered already
/// (`ERROR_CLASS_ALREADY_EXISTS` counts as registered), and creates this thread's top-level window, never shown.
fn open_window(module: HINSTANCE, class: &[u16]) -> io::Result<HWND> {
    let _class = lock(&CLASS_LOCK);
    // Written field by field; WNDCLASSW needs the Win32_Graphics_Gdi feature (Ruling PF3-S15-17-1).
    let description = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(procedure),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: module,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
    };
    // SAFETY: `description` and the class name it points at outlive the call, which copies them; the procedure is a
    // function of the program, alive as long as the class.
    if unsafe { RegisterClassW(&description) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_CLASS_ALREADY_EXISTS as i32) {
            return Err(error);
        }
    }
    let title = wide(TITLE);
    // SAFETY: the class and the title are NUL-terminated and outlive the call, which only reads them; the parent, the
    // menu and the creation data are null, which a top-level window accepts.
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            module,
            null(),
        )
    };
    if window.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(window)
    }
}

/// Steps 4 to 6 on this thread's `window`: the CD-ROM device notification, whose refusal is not an error since the poll
/// covers every change; the poll's timer; and the first scan, against an empty map, which becomes both the queue and
/// the last scan. Err is Windows' error for the timer, after the window is destroyed.
fn start(window: HWND, queue: &Shared) -> io::Result<HWND> {
    let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
        dbcc_size: size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
        dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE,
        dbcc_classguid: GUID_DEVINTERFACE_CDROM,
        ..Default::default()
    };
    // SAFETY: `window` is this thread's window, and `filter` outlives the call, which only reads it.
    let notification = unsafe {
        RegisterDeviceNotificationW(window, std::ptr::from_ref(&filter).cast(), DEVICE_NOTIFY_WINDOW_HANDLE)
    };
    with_watch(|watch| watch.notification = notification);
    let every = u32::try_from(DRIVE_POLL_EVERY.as_millis()).unwrap_or(u32::MAX);
    // SAFETY: `window` is this thread's window; with no timer procedure, the timer posts WM_TIMER to it.
    if unsafe { SetTimer(window, TIMER, every, None) } == 0 {
        let error = io::Error::last_os_error();
        // SAFETY: `window` is this thread's window; its WM_DESTROY lets the device notification go.
        unsafe { DestroyWindow(window) };
        return Err(error);
    }
    let first = scan(&BTreeMap::new());
    *lock(&queue.0) = DriveQueue::new(first.clone());
    with_watch(|watch| watch.last = first);
    Ok(window)
}

/// Step 7: dispatches this thread's messages until WM_QUIT. When GetMessageW fails instead, the window is destroyed,
/// so its WM_DESTROY still lets the device notification and the timer go.
fn pump(window: HWND) {
    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a local the call writes; a null window takes every message of this thread.
        let got = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if got == 0 {
            return;
        }
        if got < 0 {
            // SAFETY: `window` is the window this thread created; when it is already gone the call fails and does
            // nothing, since a thread cannot destroy another thread's window.
            unsafe { DestroyWindow(window) };
            return;
        }
        // SAFETY: `message` is the message GetMessageW just wrote.
        unsafe { DispatchMessageW(&message) };
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::ptr::{null, null_mut};
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    use super::*;
    use crate::drives::DRIVE_POLL_EVERY;
    use crate::testutil::{ci_factor, cue, skip};
    use crate::win::wide;
    use crate::{DriveEvent, DriveWatcher};

    // The volume lock's control codes, as `win::eject` and `crates/capture/src/eject.rs` use them.
    const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
    const FSCTL_UNLOCK_VOLUME: u32 = 0x0009_001C;

    /// Runs in the SSH session and on the runner: no notification is asserted, since whether a window there receives
    /// any is not measured (design part 3.1). Two watchers at once also prove the window class is shared.
    #[test]
    fn the_watcher_starts_and_stops_its_window_thread() {
        let mut first = WinDrives::new().expect("the first watcher starts");
        let mut second = WinDrives::new().expect("a second watcher starts beside it");
        let _ = first.next(Duration::from_millis(100));
        let _ = second.next(Duration::from_millis(100));
        let started = Instant::now();
        drop(first);
        drop(second);
        let limit = Duration::from_secs(5) * ci_factor();
        assert!(started.elapsed() < limit, "the window threads took {:?} to end", started.elapsed());
    }

    /// Holds on any drive set (Ruling PF3-S15-17-4): each id is of the `D:` form with its root, the ids are strictly
    /// increasing, and two calls in a row give the same list.
    #[test]
    fn present_lists_only_ids_of_the_d_colon_form_with_their_roots() {
        let watcher = WinDrives::new().unwrap();
        let present = watcher.present();
        for (id, root) in &present {
            let bytes = id.as_bytes();
            assert!(bytes.len() == 2 && bytes[0].is_ascii_uppercase() && bytes[1] == b':', "{id}");
            assert_eq!(*root, PathBuf::from(format!(r"{id}\")));
            println!("present: {id}");
        }
        let increasing = present.windows(2).all(|pair| pair[0].0 < pair[1].0);
        assert!(increasing, "the ids are not strictly increasing: {present:?}");
        assert_eq!(watcher.present(), present, "two calls in a row gave different lists");
        if present.is_empty() {
            println!("present: none");
        }
    }

    /// The next event within `limit`, waiting through `next` in slices.
    fn event_within(watcher: &mut WinDrives, limit: Duration) -> Option<DriveEvent> {
        let deadline = Instant::now() + limit;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Some(event) = watcher.next(left.min(Duration::from_millis(500))) {
                return Some(event);
            }
        }
        None
    }

    /// A lock on `\\.\<drive>`: `CreateFileW` with `GENERIC_READ | GENERIC_WRITE`, `FILE_SHARE_READ |
    /// FILE_SHARE_WRITE` and `OPEN_EXISTING`, then `DeviceIoControl(FSCTL_LOCK_VOLUME)`; Err is `GetLastError()`.
    /// Dropping it runs `FSCTL_UNLOCK_VOLUME` and closes the handle.
    struct VolumeLock(HANDLE);

    fn lock_volume(drive: &str) -> Result<VolumeLock, u32> {
        let device = wide(format!(r"\\.\{drive}"));
        // SAFETY: `device` is NUL-terminated and outlives the call, which only reads it; the security attributes and
        // the template file are null, which `CreateFileW` accepts.
        let handle = unsafe {
            CreateFileW(
                device.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError takes no argument and reads this thread's last error, which the failed call set.
            return Err(unsafe { GetLastError() });
        }
        let mut returned = 0u32;
        // SAFETY: the handle is open; the input and output buffers and the overlapped structure are null, so the call
        // is synchronous and writes only to `returned`, a local that outlives it.
        let locked =
            unsafe { DeviceIoControl(handle, FSCTL_LOCK_VOLUME, null(), 0, null_mut(), 0, &mut returned, null_mut()) };
        if locked == 0 {
            // SAFETY: as above; read before CloseHandle can set another last error.
            let code = unsafe { GetLastError() };
            // SAFETY: the handle came from the CreateFileW above and is closed once, here.
            unsafe { CloseHandle(handle) };
            return Err(code);
        }
        Ok(VolumeLock(handle))
    }

    impl Drop for VolumeLock {
        fn drop(&mut self) {
            let mut returned = 0u32;
            // SAFETY: the handle is open and holds the volume's lock; the buffers and the overlapped structure are
            // null, so the call is synchronous and writes only to `returned`, a local that outlives it.
            unsafe {
                DeviceIoControl(self.0, FSCTL_UNLOCK_VOLUME, null(), 0, null_mut(), 0, &mut returned, null_mut())
            };
            // SAFETY: the handle came from `lock_volume`'s CreateFileW and is closed once, here.
            unsafe { CloseHandle(self.0) };
        }
    }

    #[test]
    #[ignore = "desktop session through job.sh; the owner inserts and removes a disc on cue (design part 12.4)"]
    fn an_inserted_and_removed_disc_is_reported_once_each() {
        let Ok(drive) = std::env::var("LAZYMKV_TEST_DRIVE") else {
            return skip("LAZYMKV_TEST_DRIVE is unset");
        };
        let letter = drive.chars().next().expect("a drive id such as D:");
        let owner = Duration::from_secs(300) * ci_factor();
        // A bound that proves "reported once"; waiting longer only makes it stricter, so it does not scale.
        let quiet = DRIVE_POLL_EVERY * 3;
        let arrived = DriveEvent::Arrived { drive: drive.clone(), root: PathBuf::from(format!(r"{drive}\")) };
        let removed = DriveEvent::Removed { drive: drive.clone() };
        let mut watcher = WinDrives::new().unwrap();
        if watcher.present().iter().any(|(id, _)| *id == drive) {
            cue(&format!("remove the disc from {drive}"));
            assert_eq!(event_within(&mut watcher, owner), Some(removed.clone()));
        }
        cue(&format!("insert a disc in {drive}"));
        assert_eq!(event_within(&mut watcher, owner), Some(arrived));
        assert_eq!(event_within(&mut watcher, quiet), None, "a disc seen twice is reported once");
        match lock_volume(&drive) {
            Ok(lock) => {
                println!("under the lock, GetVolumeInformationW gives {:?}", volume_information(letter));
                assert_eq!(event_within(&mut watcher, quiet), None, "the lock was reported as a change");
                drop(lock);
                assert_eq!(event_within(&mut watcher, quiet), None, "the unlock was reported as a change");
                println!("the volume lock held for {quiet:?} with no event");
            }
            Err(code) => println!("the volume lock was refused (error {code}); the lock case was not checked"),
        }
        cue(&format!("remove the disc from {drive}"));
        assert_eq!(event_within(&mut watcher, owner), Some(removed));
        println!("drive watcher: the arrival and the removal in {drive} were each reported once");
    }
}
