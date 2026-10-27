//! StartTimer's timers (design part 5.2): AwaitingNetwork's 10-minute retry and AwaitingLookup's hourly round. They
//! live in memory only; a restart recreates them through `core::job::resume` (design part 5.10).

use std::thread;
use std::time::Duration;

/// StartTimer's timers (design part 5.2): `fire` runs once after `minutes`. Def: T22. Use: T22, T24, T25.
pub trait Timers: Send + Sync {
    /// Runs `fire` once, `minutes` from now.
    fn start(&self, minutes: u32, fire: Box<dyn FnOnce() + Send>);
}

/// One sleeping thread per timer (design part 5.2). The PR 2 binary builds no Services (S28), so nothing in PR 2
/// starts one; PR 3's binary passes it. Def: T22. Use: none in PR 2.
pub struct StdTimers;

impl Timers for StdTimers {
    fn start(&self, minutes: u32, fire: Box<dyn FnOnce() + Send>) {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(u64::from(minutes) * 60));
            fire();
        });
    }
}
