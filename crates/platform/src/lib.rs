//! lazymkv-platform: the operating-system seams of spec 4.3 as traits, with in-memory fakes behind the `fakes` feature (design parts 4.1, 4.2). Pull request 2 holds no Windows API; the Windows implementations arrive in pull request 3.

use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lazymkv_core::job::Tool;

#[cfg(feature = "fakes")]
pub mod fakes;

/// A change in the set of optical drives the machine holds (spec 4.3, design part 4.2): a disc arrived in `drive`,
/// whose contents are readable under `root`, or the drive's disc went away. Def: T8. Use: T22, T25.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriveEvent {
    /// A disc is readable in `drive`, under `root`.
    Arrived {
        /// The drive's identifier, the one MakeMKV's `disc:` argument and `Ejector::eject` take.
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

/// Delivers disc arrivals and removals to the engine's drive thread (spec 4.3, design part 4.2). Def: T8. Use: T22,
/// T25.
pub trait DriveWatcher: Send {
    /// Waits up to `timeout` for the next event and returns it, or None when none came in that time.
    fn next(&mut self, timeout: Duration) -> Option<DriveEvent>;
    /// The drives holding a disc right now, as (drive, root), sorted by drive. The engine reads this at start (design
    /// part 4.2, L1), before it drains any event.
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

/// Starts child programs so that closing the engine cannot leave them running (spec 4.3, design part 4.2). Def: T8.
/// Use: T11.
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
    /// Kills the child it was made for.
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

/// One accepted connection of the engine's pipe: a line reader and a writer (spec 4.3, design part 4.2). Def: T8.
/// Use: T26, T28.
pub struct IpcStream {
    /// Reads the peer's lines.
    pub reader: Box<dyn BufRead + Send>,
    /// Writes to the peer.
    pub writer: Box<dyn Write + Send>,
}

/// The engine's end of the pipe: hands out one stream per client that connects (spec 4.3, design part 4.2). Def: T8.
/// Use: T26.
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

/// What the engine asks of the machine it runs on (spec 4.3, design part 4.2): the clock, the calendar date, MakeMKV's
/// registration key and debug log, the running programs, a disc's label and where each tool is installed by default.
/// Def: T8. Use: T11, T22.
pub trait Host: Send + Sync {
    /// The current time, in seconds since the Unix epoch.
    fn now_unix(&self) -> u64;
    /// Today's local date as `yyyy-MM-dd`, the form phase 0's files keep (design part 4.6).
    fn local_date(&self) -> String;
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
