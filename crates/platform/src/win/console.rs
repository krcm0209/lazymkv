//! The console's stop events (design part 3.8; Ruling W19): Ctrl+C, Ctrl+Break, and the console window's close, logoff
//! and shutdown reach the one handler the engine installs, through a routine registered once with
//! `SetConsoleCtrlHandler`. Windows runs the routine on a thread it creates for each event.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use windows_sys::core::BOOL;
use windows_sys::Win32::System::Console::{
    SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
};

/// The handler `routine` calls; None until `on_console_stop` sets one.
static HANDLER: Mutex<Option<Box<dyn Fn() + Send + Sync>>> = Mutex::new(None);
/// Whether `routine` is registered with Windows; `on_console_stop` registers it the first time only.
static REGISTERED: AtomicBool = AtomicBool::new(false);
/// How long the routine sleeps at a time while it waits for the process to end.
const EXIT_WAIT: Duration = Duration::from_millis(100);

/// What the routine does once the handler ran (design part 3.8; Ruling W19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopKind {
    /// Ctrl+C or Ctrl+Break: the routine returns at once, and the engine stops on its own.
    Return,
    /// The console window's close, a logoff or a shutdown: Windows ends the process when the routine returns, so the
    /// routine waits for the process to end instead.
    WaitForExit,
}

/// The kind of the console event `code`; None for a code that is not a stop event.
fn stop_kind(code: u32) -> Option<StopKind> {
    match code {
        CTRL_C_EVENT | CTRL_BREAK_EVENT => Some(StopKind::Return),
        CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => Some(StopKind::WaitForExit),
        _ => None,
    }
}

/// The handler's lock, taken through a poisoned state, so the routine never panics on it.
fn lock() -> MutexGuard<'static, Option<Box<dyn Fn() + Send + Sync>>> {
    HANDLER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Replaces the handler `routine` calls; None removes it.
fn set_handler(handler: Option<Box<dyn Fn() + Send + Sync>>) {
    *lock() = handler;
}

/// The routine registered with `SetConsoleCtrlHandler`. For a stop event with a handler set, it calls the handler under
/// the lock and releases the lock; then, for `WaitForExit`, it sleeps until Windows or the engine's own exit ends the
/// process, and for `Return` it returns TRUE (1). Another code, or no handler, returns FALSE (0), so the next routine,
/// in the end the default one, handles the event.
extern "system" fn routine(code: u32) -> BOOL {
    let Some(kind) = stop_kind(code) else {
        return 0;
    };
    {
        let handler = lock();
        let Some(handler) = handler.as_ref() else {
            return 0;
        };
        handler();
    }
    if kind == StopKind::WaitForExit {
        loop {
            thread::sleep(EXIT_WAIT);
        }
    }
    1
}

/// Calls `handler` on the console's stop events through `SetConsoleCtrlHandler` (design part 3.8; Ruling W19):
/// Ctrl+C and Ctrl+Break, after which the handler routine returns at once, and the console window's close, logoff
/// and shutdown, after which it waits for the process to end, since Windows ends the process when the routine
/// returns. A second call replaces the handler. Err is `SetConsoleCtrlHandler`'s. Def: T3-23. Use: T3-23.
pub fn on_console_stop(handler: Box<dyn Fn() + Send + Sync>) -> io::Result<()> {
    set_handler(Some(handler));
    if REGISTERED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let registered: unsafe extern "system" fn(u32) -> BOOL = routine;
    // SAFETY: `registered` is `routine`, a function of this module with the signature `PHANDLER_ROUTINE` names, so it
    // lives as long as the process; it touches only the static `HANDLER`, and a panic inside it aborts the process
    // instead of unwinding into Windows, since its ABI is `system`, not `system-unwind`.
    if unsafe { SetConsoleCtrlHandler(Some(registered), 1) } == 0 {
        let error = io::Error::last_os_error();
        REGISTERED.store(false, Ordering::SeqCst);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
    };

    #[test]
    fn ctrl_c_and_ctrl_break_return_at_once_and_close_logoff_and_shutdown_wait() {
        assert_eq!(stop_kind(CTRL_C_EVENT), Some(StopKind::Return));
        assert_eq!(stop_kind(CTRL_BREAK_EVENT), Some(StopKind::Return));
        for code in [CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT] {
            assert_eq!(stop_kind(code), Some(StopKind::WaitForExit), "{code}");
        }
        assert_eq!((stop_kind(3), stop_kind(4)), (None, None));
    }

    #[test]
    fn the_routine_calls_the_handler_for_ctrl_c_and_declines_another_code() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        set_handler(Some(Box::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        })));
        let (ctrl_c, other) = (routine(CTRL_C_EVENT), routine(3));
        set_handler(None);
        assert_eq!((ctrl_c, other, calls.load(Ordering::SeqCst)), (1, 0, 1));
        assert_eq!(routine(CTRL_C_EVENT), 0, "with no handler the routine declines");
    }
}
