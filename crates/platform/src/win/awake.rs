//! The Windows `KeepAwake` (design part 4.2; Ruling W9): the thread execution state belongs to the thread that set it,
//! so one thread of its own sets it for every caller, and a change is applied before `acquire` or `release` returns
//! (Ruling T3-6-a).

use std::io;
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

use crate::KeepAwake;

/// The Windows `KeepAwake` (spec 4.3, design part 4.2; Ruling W9). A thread of its own, `lazymkv-keep-awake`, sets the
/// thread execution state: `ES_CONTINUOUS | ES_SYSTEM_REQUIRED` when the holders go from 0 to 1, `ES_CONTINUOUS` when
/// they go back to 0. The state belongs to the thread that set it, so it never depends on which thread calls `acquire`
/// or `release`, or on that thread living on. Calls nest; a release with no holder stays at zero (Ruling T8-g).
/// `acquire` and `release` return once the thread has applied a change (Ruling T3-6-a). Dropping it lets the state go
/// and ends the thread. Def: T3-6. Use: T3-23.
pub struct WinKeepAwake {
    holders: Mutex<Holders>,
    thread: Option<JoinHandle<()>>,
}

/// The count and the way to the thread, under one lock, so changes reach the thread in the order they were made.
struct Holders {
    count: usize,
    to_thread: Option<mpsc::Sender<(bool, mpsc::Sender<()>)>>, // (hold, done)
}

impl Holders {
    /// Sends one change to the thread and waits for its reply, so the system state matches the count on return
    /// (Ruling T3-6-a). A thread that is gone, a send or a reply that fails, is ignored: the trait gives `acquire` and
    /// `release` no error to return.
    fn apply(&self, hold: bool) {
        let Some(to_thread) = &self.to_thread else {
            return;
        };
        let (done, applied) = mpsc::channel();
        if to_thread.send((hold, done)).is_ok() {
            let _ = applied.recv();
        }
    }
}

/// The keeper's thread: sets the state for each change it is sent, answers on `done`, and lets the state go when the
/// channel closes. The calls' results (the previous state, or 0 on failure) are not read: design part 4.2 names no
/// report and the trait no error.
fn keep(changes: mpsc::Receiver<(bool, mpsc::Sender<()>)>) {
    for (hold, done) in changes {
        let flags = if hold { ES_CONTINUOUS | ES_SYSTEM_REQUIRED } else { ES_CONTINUOUS };
        // SAFETY: `SetThreadExecutionState` takes flags only, and affects only the calling thread.
        unsafe { SetThreadExecutionState(flags) };
        let _ = done.send(());
    }
    // SAFETY: as above; `ES_CONTINUOUS` alone clears what this thread required.
    unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
}

/// The lock, taken through a poisoned state, so a panic elsewhere does not poison the next call.
fn lock(holders: &Mutex<Holders>) -> MutexGuard<'_, Holders> {
    holders.lock().unwrap_or_else(PoisonError::into_inner)
}

impl WinKeepAwake {
    /// Starts the keeper's thread. Err when the thread cannot start (design part 4.2). Def: T3-6. Use: T3-23.
    pub fn new() -> io::Result<WinKeepAwake> {
        let (to_thread, changes) = mpsc::channel::<(bool, mpsc::Sender<()>)>();
        let thread = std::thread::Builder::new().name("lazymkv-keep-awake".into()).spawn(move || keep(changes))?;
        Ok(WinKeepAwake { holders: Mutex::new(Holders { count: 0, to_thread: Some(to_thread) }), thread: Some(thread) })
    }

    /// How many `acquire` calls have no `release` yet (design part 4.2). Def: T3-6. Use: T3-6.
    pub fn holders(&self) -> usize {
        lock(&self.holders).count
    }
}

impl KeepAwake for WinKeepAwake {
    fn acquire(&self) {
        // The lock is held through the wait, so changes reach the thread in the order they were made.
        let mut holders = lock(&self.holders);
        holders.count += 1;
        if holders.count == 1 {
            holders.apply(true);
        }
    }

    fn release(&self) {
        let mut holders = lock(&self.holders);
        // Stops at zero (Ruling T8-g).
        if holders.count == 0 {
            return;
        }
        holders.count -= 1;
        if holders.count == 0 {
            holders.apply(false);
        }
    }
}

impl Drop for WinKeepAwake {
    fn drop(&mut self) {
        // Closing the channel makes the thread set `ES_CONTINUOUS` and return; the join waits for that.
        drop(self.holders.get_mut().unwrap_or_else(PoisonError::into_inner).to_thread.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;
    use std::sync::{mpsc, Arc, Mutex, PoisonError};
    use std::time::Duration;

    use windows_sys::Win32::System::Power::{CallNtPowerInformation, SystemExecutionState, ES_SYSTEM_REQUIRED};

    use super::WinKeepAwake;
    use crate::testutil::skip;
    use crate::KeepAwake;

    /// The tests read one system-wide state, so they run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// Whether the system's execution state holds `ES_SYSTEM_REQUIRED`.
    fn system_required() -> bool {
        let mut state: u32 = 0;
        // SAFETY: no input buffer; the output buffer is `state`, a u32 the call writes, and its size is given.
        let status = unsafe {
            CallNtPowerInformation(SystemExecutionState, std::ptr::null(), 0, (&mut state as *mut u32).cast::<c_void>(), 4)
        };
        assert_eq!(status, 0, "CallNtPowerInformation returned {status:#x}");
        state & ES_SYSTEM_REQUIRED != 0
    }

    /// Ruling W9: the thread that called `acquire` has ended, and the state is still held.
    #[test]
    fn acquire_on_one_thread_and_release_on_another_keeps_the_count() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let keeper = Arc::new(WinKeepAwake::new().unwrap());
        let acquiring = Arc::clone(&keeper);
        std::thread::spawn(move || acquiring.acquire()).join().unwrap();
        assert_eq!(keeper.holders(), 1);
        assert!(system_required(), "the state ended with the thread that called acquire");
        let releasing = Arc::clone(&keeper);
        std::thread::spawn(move || releasing.release()).join().unwrap();
        assert_eq!(keeper.holders(), 0);
    }

    /// Releasing the last holder lets the system sleep again (Ruling PF3-S4-6-1). Another process may hold the state
    /// too (MakeMKV during a rip on the mini PC), and then the release cannot be read back.
    #[test]
    fn the_system_state_is_required_while_a_holder_remains() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let held_elsewhere = system_required();
        let keeper = WinKeepAwake::new().unwrap();
        keeper.acquire();
        keeper.acquire();
        keeper.release();
        assert_eq!(keeper.holders(), 1);
        assert!(system_required());
        keeper.release();
        assert_eq!(keeper.holders(), 0);
        if held_elsewhere {
            skip("another process holds ES_SYSTEM_REQUIRED, so the release cannot be read back");
        } else {
            assert!(!system_required(), "the state stayed required after the last release");
        }
    }

    /// Ruling T8-g, then a normal hold.
    #[test]
    fn a_release_with_no_holder_stays_at_zero() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let keeper = WinKeepAwake::new().unwrap();
        keeper.release();
        keeper.release();
        assert_eq!(keeper.holders(), 0);
        keeper.acquire();
        assert_eq!(keeper.holders(), 1);
        assert!(system_required());
        keeper.release();
        assert_eq!(keeper.holders(), 0);
    }

    /// Dropping a keeper that still has a holder joins its thread, and the state goes with it. The 5 s bound catches a
    /// hang, so it does not scale. Another process may hold the state too (MakeMKV during a rip on the mini PC), and
    /// then the release cannot be read back.
    #[test]
    fn dropping_the_keeper_ends_its_thread() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let held_elsewhere = system_required();
        let keeper = WinKeepAwake::new().unwrap();
        keeper.acquire();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            drop(keeper);
            tx.send(()).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(5)).expect("dropping the keeper did not join its thread within 5 s");
        if held_elsewhere {
            skip("another process holds ES_SYSTEM_REQUIRED, so the release cannot be read back");
        } else {
            assert!(!system_required(), "the state stayed required after the keeper's thread ended");
        }
    }
}
