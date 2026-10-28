//! lazymkv-platform: the operating-system seams of spec 4.3 as traits, with in-memory fakes behind the `fakes` feature (design parts 4.1, 4.2). Pull request 2 holds no Windows API; the Windows implementations arrive in pull request 3.

use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lazymkv_core::job::Tool;

/// The in-memory fakes of every platform trait, for the tests of the crates above this one (design part 4.2).
/// Def: T8. Use: T22 to T29.
#[cfg(feature = "fakes")]
pub mod fakes;

/// The Windows implementations of the platform traits and helpers; every `windows-sys` call of the workspace sits
/// here (design parts 4.2, 4.3). Compiled only on Windows. Def: T3-1. Use: T3-2 to T3-16, T3-23.
#[cfg(windows)]
pub mod win;

/// Phase 0's local-time text, from Unix milliseconds and an offset from Coordinated Universal Time (UTC), with no
/// date-time crate (design part 4.6, R23.7). Compiled and tested on every system. Def: T3-7. Use: T3-7.
pub mod localtime;

mod process;
pub use process::StdProcessGuard;

/// The process runner the engine uses on this system (spec 4.3, design part 4.2; Ruling W7): `win::JobProcessGuard`
/// on Windows, `StdProcessGuard` elsewhere. Def: T3-5. Use: T3-5 (testkit), T3-23.
pub fn native_guard() -> Arc<dyn ProcessGuard> {
    #[cfg(windows)]
    {
        Arc::new(win::JobProcessGuard)
    }
    #[cfg(not(windows))]
    {
        Arc::new(StdProcessGuard)
    }
}

/// The pure security rules of design parts 3.2 to 3.5: the descriptor strings, the token verdict and the names.
/// Def: T3-2. Use: T3-2, T3-3, T3-4, T3-11 to T3-14, T3-23, T3-24.
pub mod security;
pub use security::{MUTEX_NAME, PIPE_NAME};

/// A change in the set of optical drives the machine holds (spec 4.3, design part 4.2): a disc arrived in `drive`,
/// whose contents are readable under `root`, or the drive's disc went away. Def: T8. Use: T22, T25.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriveEvent {
    /// A disc is readable in `drive`, under `root`.
    Arrived {
        /// The drive's identifier in MakeMKV's `D:` form, the seventh field of its `DRV` line (design part 5.4);
        /// `Ejector::eject` takes it too.
        drive: String,
        /// The folder or drive root the disc's files are read from.
        root: PathBuf,
    },
    /// The disc in `drive` is gone, ejected or pulled.
    Removed {
        /// The drive's identifier.
        drive: String,
    },
}

/// Delivers disc arrivals and removals to the engine's drive thread (spec 4.3, design part 4.2).
/// Def: T8. Use: T22, T25.
pub trait DriveWatcher: Send {
    /// Waits up to `timeout` for the next event and returns it, or None when none came in that time.
    fn next(&mut self, timeout: Duration) -> Option<DriveEvent>;
    /// The drives holding a disc right now, as (drive, root). The engine reads this at start (design part 4.2, L1),
    /// before it drains any event, and does not rely on the order.
    fn present(&self) -> Vec<(String, PathBuf)>;
}

/// Ejects the disc in a drive (spec 4.3, design part 4.2). Def: T8. Use: T23.
pub trait Ejector: Send + Sync {
    /// Opens the tray of `drive`. Err holds the reason in words for the job's notes.
    fn eject(&self, drive: &str) -> Result<(), String>;
}

/// What to start: one program with its arguments (spec 4.3, design part 4.2). Def: T8. Use: T11.
#[derive(Clone, Debug)]
pub struct Spawn {
    /// The program to run, as a full path.
    pub program: PathBuf,
    /// The arguments, one string each.
    pub args: Vec<String>,
    /// Extra environment variables for the child. The engine leaves this empty (S43).
    pub env: Vec<(String, String)>,
    /// Whether to start the child at below-normal priority. A no-op where the platform has no such class.
    pub below_normal: bool,
}

/// Starts child programs so that closing the engine cannot leave them running (spec 4.3, design part 4.2).
/// Def: T8. Use: T11.
pub trait ProcessGuard: Send + Sync {
    /// Starts `spawn` with piped standard output and error. Err is the operating system's refusal, such as a program
    /// that is not there.
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>>;
}

/// A started child program (spec 4.3, design part 4.2). Def: T8. Use: T11.
pub trait GuardedChild: Send {
    /// The child's standard output, handed over once.
    fn take_stdout(&mut self) -> Box<dyn Read + Send>;
    /// The child's standard error, handed over once.
    fn take_stderr(&mut self) -> Box<dyn Read + Send>;
    /// A handle another thread can use to kill the child while this thread waits on it.
    fn killer(&self) -> KillHandle;
    /// Waits for the child to end. The exit code, or None when the child ended without one.
    fn wait(&mut self) -> io::Result<Option<i32>>;
}

/// A cloneable way to kill one child from another thread (spec 4.3, design part 4.2). Only this crate builds one
/// (Ruling T8-d), so the field is `pub(crate)`. Def: T8. Use: T11.
#[derive(Clone)]
pub struct KillHandle(pub(crate) Arc<dyn Fn() -> io::Result<()> + Send + Sync>);

impl KillHandle {
    /// Kills the child it was made for (spec 4.3, design part 4.2). Def: T8. Use: T11.
    pub fn kill(&self) -> io::Result<()> {
        (self.0)()
    }
}

/// Keeps the machine from sleeping while a job runs (spec 4.3, design part 4.2). Calls nest: the machine may sleep
/// again once every `acquire` has had its `release`. Def: T8. Use: T22.
pub trait KeepAwake: Send + Sync {
    /// Adds one holder.
    fn acquire(&self);
    /// Removes one holder.
    fn release(&self);
}

/// Whether a share answered (spec 4.3, design part 4.2). Def: T8. Use: T23, T24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// The share answered.
    Reachable,
    /// The share did not answer; the text says why, for the job's notes.
    Unreachable(String),
}

/// Reaches the network shares the staging and library folders live on (spec 4.3, design part 4.2). Roots are the
/// strings the owner's settings hold. Def: T8. Use: T23, T24.
pub trait ShareConnector: Send + Sync {
    /// Whether the share holding `root` answers now, connecting with the stored credential when it needs one.
    fn reach(&self, root: &str) -> Reach;
    /// The free bytes on the volume holding `root`.
    fn free_bytes(&self, root: &str) -> io::Result<u64>;
    /// An identifier of the volume holding `root`, equal for two roots on one volume (S11), or None when unknown.
    fn volume(&self, root: &str) -> Option<String>;
}

/// Holds the owner's secrets outside the settings files (spec 4.3, design part 4.2). Def: T8. Use: T26.
pub trait SecretStore: Send + Sync {
    /// The secret called `name`, or None when none is stored.
    fn get(&self, name: &str) -> Option<String>;
    /// Stores `value` under `name`. Err holds the reason in words.
    fn set(&self, name: &str, value: &str) -> Result<(), String>;
}

/// One accepted connection of the engine's pipe: a line reader and a writer (spec 4.3, design part 4.2).
/// Def: T8. Use: T26, T28.
pub struct IpcStream {
    /// Reads the peer's lines.
    pub reader: Box<dyn BufRead + Send>,
    /// Writes to the peer.
    pub writer: Box<dyn Write + Send>,
}

/// The engine's end of the pipe: hands out one stream per client that connects (spec 4.3, design part 4.2).
/// Def: T8. Use: T26.
pub trait IpcTransport: Send + Sync {
    /// Waits for the next client. Err when the transport is closed.
    fn accept(&self) -> io::Result<IpcStream>;
}

/// Why a client could not reach the engine (spec 4.3, design part 4.2). Def: T8. Use: T28.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectError {
    /// Nothing listens on the pipe.
    NotRunning,
    /// Something listens and turned the client away.
    Refused,
    /// The pipe exists but only an elevated client may open it.
    NotElevated,
    /// This build has no transport to the engine.
    NoTransport,
}

/// The client's end of the pipe (spec 4.3, design part 4.2). Def: T8. Use: T28.
pub trait IpcConnector {
    /// Opens a connection to the engine.
    fn connect(&self) -> Result<IpcStream, ConnectError>;
}

/// Registers the engine to start at sign-in (spec 4.3, design part 4.2). Declared only in this pull request; the
/// Windows implementation is phase 5's. Def: T8. Use: phase 5.
pub trait Autostart {
    /// Registers the engine to start at sign-in.
    fn install(&self) -> Result<(), String>;
    /// Removes the registration.
    fn remove(&self) -> Result<(), String>;
}

/// What the engine asks of the machine it runs on (spec 4.3, design part 4.2): the clock, the calendar date and the
/// local time, MakeMKV's registration key and debug log, the running programs, a disc's label, where each tool is
/// installed by default, and whether a non-elevated process of the user could write a path.
/// Def: T8; T3-7 adds now_ms, local_time and user_writable. Use: T11, T22, T3-8, T3-9, T3-22, T3-23.
pub trait Host: Send + Sync {
    /// The current time, in seconds since the Unix epoch.
    fn now_unix(&self) -> u64;
    /// The current time in milliseconds since the Unix epoch (R23.7, Ruling W16).
    fn now_ms(&self) -> u64;
    /// Today's local date as `yyyy-MM-dd`, the form phase 0's files keep (design part 4.6).
    fn local_date(&self) -> String;
    /// The instant `unix_ms` as local time, `yyyy-MM-dd HH:mm:ss.fff`, under that instant's daylight-saving rule: the
    /// form phase 0 wrote `started` in (design part 4.6, R23.7; scripts/phase0/rip-title.ps1:103).
    fn local_time(&self, unix_ms: u64) -> String;
    /// MakeMKV's registration key, or None when none is set.
    fn makemkv_app_key(&self) -> Option<String>;
    /// MakeMKV's debug log, or None when the machine has none.
    fn makemkv_debug_log(&self) -> Option<PathBuf>;
    /// The image names of the programs running now, such as `makemkvcon64.exe`.
    fn running_processes(&self) -> Vec<String>;
    /// The volume label of the disc at `root`, or None when it cannot be read.
    fn volume_label(&self, root: &Path) -> Option<String>;
    /// Where `tool` is installed by default on this machine, or None when it has no such place.
    fn default_install_path(&self, tool: Tool) -> Option<PathBuf>;
    /// Whether a non-elevated process of the user could write `path` (design part 3.6, Ruling W17): for a folder, add a
    /// file or a folder, delete a child, delete it, or change its access list or owner; for a file, write, append,
    /// delete it, or change its access list or owner. True when it cannot be told.
    fn user_writable(&self, path: &Path) -> bool;
}

/// Everything the engine takes from the operating system, in one bundle (spec 4.3, design part 4.2). Windows fills it
/// in pull request 3 and the fakes fill it in tests. Def: T8. Use: T22.
pub struct Platform {
    /// Disc arrivals and removals.
    pub drives: Box<dyn DriveWatcher>,
    /// The tray opener.
    pub ejector: Arc<dyn Ejector>,
    /// Child programs.
    pub processes: Arc<dyn ProcessGuard>,
    /// The sleep inhibitor.
    pub awake: Arc<dyn KeepAwake>,
    /// Network shares.
    pub shares: Arc<dyn ShareConnector>,
    /// The secret store.
    pub secrets: Arc<dyn SecretStore>,
    /// The engine's end of the pipe.
    pub ipc: Arc<dyn IpcTransport>,
    /// The machine itself.
    pub host: Arc<dyn Host>,
}

/// The suffix `write_atomic` adds to a file's name for the temporary file it writes beside it (design part 7.4, S13).
/// Def: T9. Use: T9, T21.
pub const TEMP_SUFFIX: &str = ".lazymkv-tmp";

/// Writes `bytes` to `path` so that a reader sees the old file or the new one whole, never a part (design part 7.4,
/// S13). It writes `<file name>.lazymkv-tmp` beside `path`, after removing one a crash left there, flushes it with
/// `sync_all` (`FlushFileBuffers` on Windows), and puts it in place of `path`. On Windows that is
/// `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`, tried again while the target is held open
/// (os errors 5, 32 and 33; 10 tries, 10 ms apart, Ruling T3-4-a), and the last error comes back with its raw code,
/// which the store's state.json retry reads (RR1-2); elsewhere it is `std::fs::rename`. It creates no folders: a
/// missing parent gives `ErrorKind::NotFound`, because each caller owns its folder layout (Ruling T9-c). A path with no
/// file name gives `ErrorKind::InvalidInput`. Def: T9; T3-4 swaps in `MoveFileExW` on Windows. Use: T21 to T26.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no file name"))?;
    let mut temp_name = name.to_os_string();
    temp_name.push(TEMP_SUFFIX);
    let temp = path.parent().unwrap_or_else(|| Path::new("")).join(temp_name);
    match std::fs::remove_file(&temp) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    {
        win::move_replacing(&temp, path)
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(&temp, path)
    }
}

/// Moves `from` to `to` and fails with `ErrorKind::AlreadyExists` when `to` is there, so a finished file is never
/// overwritten (design part 6.5, LIB-8, S13). On Windows it is one `MoveFileExW(from, to, 0)`: without
/// `MOVEFILE_REPLACE_EXISTING` an existing `to` fails with os error 80 or 183, and without `MOVEFILE_COPY_ALLOWED` a
/// move to another volume fails with os error 17 instead of copying, so within a share it is the server's rename.
/// Elsewhere, where only the Linux and macOS CI runs use it, it is `hard_link` then `remove_file`, which is not atomic:
/// a stop between the two leaves both names on one file, which File then finishes (design part 6.5).
/// Def: T9; T3-4 swaps in `MoveFileExW` on Windows. Use: T24, T26, T3-14.
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        win::move_no_replace(from, to)
    }
    #[cfg(not(windows))]
    {
        std::fs::hard_link(from, to)?;
        std::fs::remove_file(from)
    }
}

#[cfg(test)]
#[allow(dead_code)] // ci_factor, skip and cue serve the Windows tests, so the other systems never call them
mod testutil;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;

    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn write_atomic_replaces_the_file_whole() {
        let dir = temp_dir("atomic");
        let path = dir.join("state.json");
        write_atomic(&path, b"{\"format\":\"lazymkv job v1\",\"longer\":true}").unwrap();
        write_atomic(&path, b"{}").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"{}");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn a_temporary_file_a_crash_left_is_removed_at_the_next_write() {
        let dir = temp_dir("atomic-left");
        fs::write(dir.join("state.json.lazymkv-tmp"), "junk").unwrap();
        write_atomic(&dir.join("state.json"), b"{}").unwrap();
        assert!(!dir.join(format!("state.json{TEMP_SUFFIX}")).exists());
        assert_eq!(fs::read_to_string(dir.join("state.json")).unwrap(), "{}");
    }

    #[test]
    fn write_atomic_into_a_missing_folder_fails_with_not_found() {
        let dir = temp_dir("atomic-missing");
        let error = write_atomic(&dir.join("absent").join("state.json"), b"{}").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!dir.join("absent").exists());
    }

    #[test]
    fn rename_no_replace_refuses_an_existing_target() {
        let dir = temp_dir("rename-taken");
        let (from, to) = (dir.join("a.mkv.partial"), dir.join("a.mkv"));
        fs::write(&from, "new").unwrap();
        fs::write(&to, "old").unwrap();
        assert_eq!(rename_no_replace(&from, &to).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&from).unwrap(), "new");
        assert_eq!(fs::read_to_string(&to).unwrap(), "old");
    }

    #[test]
    fn rename_no_replace_moves_when_the_target_is_free() {
        let dir = temp_dir("rename-free");
        let (from, to) = (dir.join("a.mkv.partial"), dir.join("a.mkv"));
        fs::write(&from, "placeholder").unwrap();
        rename_no_replace(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read_to_string(&to).unwrap(), "placeholder");
    }
}
