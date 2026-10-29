//! The engine binary on Windows (design parts 3.1 to 3.8, 4.2; S28; Rulings W1 to W5, W19, W26, W27): the start-up
//! sequence, the native `Platform`, and the wait for a stop.
//!
//! The start-up texts, each one line on standard error with exit 1 (Rulings W2 to W4, T3-23-d, PF3-S23-24-2):
//! `NOT_ELEVATED`; no `--data` and no ProgramData folder in the registry, `NO_PROGRAM_DATA`; no `--data` and a
//! `ProgramData` variable that names another folder, `lazymkv-engine: the ProgramData variable (<variable>) differs
//! from the registry's ProgramData (<registry>); give --data DIR (design part 3.5)` (whole-branch review F2);
//! `ENGINE_RUNNING`; a squatter, `lazymkv-engine: <mutex> is held by <account>, which is not an engine (design part
//! 3.3)`; a mutex error, `lazymkv-engine: <mutex> could not be taken: <error>`; a refused data folder,
//! `lazymkv-engine: ` and `protect_data_folder`'s text; a held pipe, `lazymkv-engine: another process holds the pipe
//! <pipe> (design part 3.3)`; any other start failure, `lazymkv-engine: the engine could not start: <error>`; a stop
//! that failed, `lazymkv-engine: the engine did not stop cleanly: <error>`; and `STOPPED_STARTING` when the console
//! stops a start.

use std::ffi::OsString;
use std::fmt::Display;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lazymkv_clients::{HttpTransport, OfflineTransport};
use lazymkv_platform::security::TokenFacts;
use lazymkv_platform::win::{
    on_console_stop, own_token, program_data, protect_data_folder, share_connector, CredentialStore, EngineMutex,
    MutexRefusal, PipeServer, WinDrives, WinEjector, WinHost, WinKeepAwake,
};
use lazymkv_platform::{
    native_guard, DriveWatcher, Ejector, Platform, ProcessGuard, SecretStore, MUTEX_NAME, PIPE_NAME,
};

use crate::{data_arg, start, EngineConfig, NoDelivery, Services, ShutdownHandle, StdTimers, USAGE};

/// A start whose token is not elevated (design part 3.2; Ruling W3). Def: T3-23. Use: T3-23.
pub const NOT_ELEVATED: &str = "lazymkv-engine: run lazymkv-engine elevated (design part 3.2)";
/// A start that met another engine's mutex (design part 3.3). Def: T3-23. Use: T3-23.
pub const ENGINE_RUNNING: &str = "lazymkv-engine: an engine is already running (design part 3.3)";
/// The console's stop event before `start` returned (design part 3.8; Ruling W19). Def: T3-23. Use: T3-23.
pub const STOPPED_STARTING: &str = "lazymkv-engine: stopped by the console while starting (design part 3.8)";

/// A start with no `--data` whose registry gives no ProgramData folder (design part 3.1; Ruling W1, amended after the
/// whole-branch review (F2)).
const NO_PROGRAM_DATA: &str = "lazymkv-engine: the registry's ProgramData could not be read; give --data DIR";

/// What `run_with` builds the engine from; each None takes the production part (Rulings W26, W27). Def: T3-23.
/// Use: T3-23.
pub struct NativeOptions {
    /// The mutex's name: `MUTEX_NAME`, or a test's own.
    pub mutex: String,
    /// The pipe's name: `PIPE_NAME`, or a test's own.
    pub pipe: String,
    /// None: `WinDrives::new()`.
    pub drives: Option<Box<dyn DriveWatcher>>,
    /// None: `WinEjector`.
    pub ejector: Option<Arc<dyn Ejector>>,
    /// None: `native_guard()`, the Job Object runner (Ruling T3-23-a).
    pub processes: Option<Arc<dyn ProcessGuard>>,
    /// None: `CredentialStore::new()`.
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// None: `OfflineTransport` (Ruling W26).
    pub http: Option<Arc<dyn HttpTransport>>,
    /// None: standard error (design part 3.5; Ruling W26).
    pub console: Option<Box<dyn Write + Send>>,
    /// None: `win::on_console_stop`.
    pub console_stop: Option<Box<dyn FnOnce(Box<dyn Fn() + Send + Sync>) -> io::Result<()> + Send>>,
    /// None: `win::own_token()` (design part 3.2).
    pub token: Option<TokenFacts>,
    /// The `ProgramData` variable compared with the registry's folder when no `--data` is given (design part 3.5;
    /// whole-branch review F2). None: the process's own variable.
    pub program_data_var: Option<OsString>,
}

impl NativeOptions {
    /// `MUTEX_NAME`, `PIPE_NAME`, and every other field None (design parts 3.3, 4.2). Def: T3-23. Use: T3-23.
    pub fn production() -> NativeOptions {
        NativeOptions {
            mutex: MUTEX_NAME.to_string(),
            pipe: PIPE_NAME.to_string(),
            drives: None,
            ejector: None,
            processes: None,
            secrets: None,
            http: None,
            console: None,
            console_stop: None,
            token: None,
            program_data_var: None,
        }
    }
}

/// Runs the engine on Windows until a `shutdown` request or the console's stop event stops it (design parts 3.1 to
/// 3.8; Rulings W1 to W5, W19, W26): the arguments, the elevation check, the default data folder's place when no
/// `--data` is given (whole-branch review F2), the mutex, the data folder, the first pipe instance, the console
/// handler, then `start`. Returns 0 after a clean stop, 1 for a refusal or a failed start, and 2 for a usage error;
/// every refusal's text goes to `err` (Step 3 lists them). Def: T3-23. Use: T3-23.
pub fn run_with(args: &[String], err: &mut dyn Write, options: NativeOptions) -> i32 {
    let data = match data_arg(args) {
        Ok(data) => data,
        Err(message) => {
            let _ = writeln!(err, "{USAGE}\nlazymkv-engine: error: {message}");
            return 2;
        }
    };
    match run(data, options) {
        Ok(()) => 0,
        Err(text) => {
            let _ = writeln!(err, "{text}");
            1
        }
    }
}

/// `run_with` after the arguments (Ruling W3's order): Err is the refusal's or the failure's start-up text. The mutex
/// is held until this returns, on every path, so the engine releases it on exit (design part 3.8).
fn run(data: Option<String>, options: NativeOptions) -> Result<(), String> {
    let NativeOptions {
        mutex,
        pipe,
        drives,
        ejector,
        processes,
        secrets,
        http,
        console,
        console_stop,
        token,
        program_data_var,
    } = options;
    // Design part 3.2; Ruling W3: the check only reads the token, so it comes before the mutex, the first step with an
    // effect.
    let token = match token {
        Some(token) => token,
        None => own_token().map_err(|error| could_not_start(&error))?,
    };
    if !token.elevated {
        return Err(NOT_ELEVATED.to_string());
    }
    // Whole-branch review F2: the default folder's check, like the elevation check, only reads the process's own
    // inputs, so it too comes before the mutex.
    let data = match data {
        Some(data) => PathBuf::from(data),
        None => default_data(program_data_var.or_else(|| std::env::var_os("ProgramData")))?,
    };
    let _mutex = EngineMutex::take(&mutex).map_err(|refusal| mutex_refusal_text(&mutex, &refusal))?;
    protect_data_folder(&data, &token.user_sid).map_err(|text| format!("lazymkv-engine: {text}"))?;
    let ipc = PipeServer::bind(&pipe).map_err(|error| match error.kind() {
        io::ErrorKind::AddrInUse => {
            format!("lazymkv-engine: another process holds the pipe {pipe} (design part 3.3)")
        }
        _ => could_not_start(&error),
    })?;
    // Ruling W19: installed before `start`, whose MakeMKV check a stop event then ends with the process.
    let slot: Arc<Mutex<Option<ShutdownHandle>>> = Arc::default();
    let handler = console_handler(Arc::clone(&slot));
    let installed = match console_stop {
        Some(install) => install(handler),
        None => on_console_stop(handler),
    };
    installed.map_err(|error| could_not_start(&error))?;
    let drives: Box<dyn DriveWatcher> = match drives {
        Some(drives) => drives,
        None => Box::new(WinDrives::new().map_err(|error| could_not_start(&error))?),
    };
    let ejector: Arc<dyn Ejector> = ejector.unwrap_or_else(|| Arc::new(WinEjector));
    let processes = processes.unwrap_or_else(native_guard);
    let awake = WinKeepAwake::new().map_err(|error| could_not_start(&error))?;
    let secrets: Arc<dyn SecretStore> = secrets.unwrap_or_else(|| Arc::new(CredentialStore::new()));
    // The one share connector of this run: its memory of refused credentials is its own (Review Focus 5), so every
    // consumer, the steps, the checks and the self-test, reads it through `Platform.shares`.
    let shares = share_connector(Arc::clone(&secrets));
    let platform = Platform {
        drives,
        ejector,
        processes,
        awake: Arc::new(awake),
        shares,
        secrets,
        ipc: Arc::new(ipc),
        host: Arc::new(WinHost::new()),
    };
    let http: Arc<dyn HttpTransport> = http.unwrap_or_else(|| Arc::new(OfflineTransport));
    let services = Services { platform, http, notify: Box::new(NoDelivery), timers: Arc::new(StdTimers), steps: None };
    let (resources, ctl) = match std::env::current_exe() {
        Ok(exe) => {
            let (resources, ctl) = binary_paths(&exe);
            (Some(resources), Some(ctl))
        }
        Err(_) => (None, None),
    };
    let console: Box<dyn Write + Send> = match console {
        Some(console) => console,
        None => Box::new(io::stderr()),
    };
    let config = EngineConfig {
        data,
        resources,
        limits_override: None,
        path_var: std::env::var_os("PATH"),
        ctl,
        console: Some(console),
    };
    let engine = start(config, services).map_err(|error| could_not_start(&error))?;
    *lock(&slot) = Some(engine.shutdown_handle());
    engine.wait_for_exit(None);
    engine.stop().map_err(|error| format!("lazymkv-engine: the engine did not stop cleanly: {error}"))
}

/// The data folder of a start with no `--data` (design part 3.1; Ruling W1, amended after the whole-branch review
/// (F2)): `program_data()`, the registry's ProgramData folder joined with `lazymkv`. Err is `NO_PROGRAM_DATA` when the
/// registry gives none, and the refusal naming both folders when `variable`, the `ProgramData` variable, is set and
/// is not the registry's folder (`same_folder`): a non-elevated process of the user can set the variable, and the
/// engine would otherwise trust a folder it chose (design part 3.5).
fn default_data(variable: Option<OsString>) -> Result<PathBuf, String> {
    let data = program_data().ok_or(NO_PROGRAM_DATA)?;
    let registry = data.parent().ok_or(NO_PROGRAM_DATA)?;
    match variable {
        Some(variable) if !same_folder(Path::new(&variable), registry) => Err(format!(
            "lazymkv-engine: the ProgramData variable ({}) differs from the registry's ProgramData ({}); give --data \
             DIR (design part 3.5)",
            Path::new(&variable).display(),
            registry.display()
        )),
        _ => Ok(data),
    }
}

/// Whether `a` and `b` name the same folder, compared part by part and case-insensitively, as Windows compares names,
/// so a trailing separator or a forward slash makes no difference.
fn same_folder(a: &Path, b: &Path) -> bool {
    let parts = |path: &Path| -> Vec<String> {
        path.components().map(|part| part.as_os_str().to_string_lossy().to_lowercase()).collect()
    };
    parts(a) == parts(b)
}

/// A start failure's text (Ruling T3-23-d).
fn could_not_start(error: &dyn Display) -> String {
    format!("lazymkv-engine: the engine could not start: {error}")
}

/// The console's stop handler (design part 3.8; Ruling W19): a forced shutdown through the handle in `slot` once
/// `start` has returned; before that, `STOPPED_STARTING` on the process's standard error and exit 1 at once, the Job
/// Objects ending the MakeMKV check's listing.
fn console_handler(slot: Arc<Mutex<Option<ShutdownHandle>>>) -> Box<dyn Fn() + Send + Sync> {
    Box::new(move || {
        let handle = lock(&slot).clone();
        match handle {
            Some(handle) => handle.shutdown(true),
            None => {
                let _ = writeln!(io::stderr(), "{STOPPED_STARTING}");
                std::process::exit(1);
            }
        }
    })
}

/// The handle's slot, taken through a poisoned state.
fn lock(slot: &Mutex<Option<ShutdownHandle>>) -> MutexGuard<'_, Option<ShutdownHandle>> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The start-up text of a mutex refusal (design part 3.3; Rulings W4, W5): `EngineRunning` is `ENGINE_RUNNING`;
/// `Squatter` is `lazymkv-engine: <mutex> is held by <account>, which is not an engine (design part 3.3)`; `Os` is
/// `lazymkv-engine: <mutex> could not be taken: <error>`. Def: T3-23. Use: T3-23 (`run_with` step 3).
pub(crate) fn mutex_refusal_text(mutex: &str, refusal: &MutexRefusal) -> String {
    match refusal {
        MutexRefusal::EngineRunning => ENGINE_RUNNING.to_string(),
        MutexRefusal::Squatter { account } => {
            format!("lazymkv-engine: {mutex} is held by {account}, which is not an engine (design part 3.3)")
        }
        MutexRefusal::Os(error) => format!("lazymkv-engine: {mutex} could not be taken: {error}"),
    }
}

/// The binary's `resources` folder and its sibling `lazymkvctl` from the path of the running executable (design parts
/// 9.5, 11.5; Ruling W26): `(<exe folder>\..\resources, <exe folder>\lazymkvctl<EXE_SUFFIX>)`. Def: T3-23.
/// Use: T3-23 (`run_with` step 7).
pub(crate) fn binary_paths(exe: &Path) -> (PathBuf, PathBuf) {
    let folder = exe.parent().unwrap_or(Path::new(""));
    let ctl = folder.join(format!("lazymkvctl{}", std::env::consts::EXE_SUFFIX));
    (folder.join("..").join("resources"), ctl)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::{Path, PathBuf};

    use lazymkv_platform::win::MutexRefusal;

    use super::*;

    #[test]
    fn each_mutex_refusal_has_its_start_up_text() {
        assert_eq!(mutex_refusal_text(r"Global\x", &MutexRefusal::EngineRunning), ENGINE_RUNNING);
        let squatter = MutexRefusal::Squatter { account: r"HOST\account".to_string() };
        assert_eq!(mutex_refusal_text(r"Global\x", &squatter),
                   r"lazymkv-engine: Global\x is held by HOST\account, which is not an engine (design part 3.3)");
        let error = io::Error::from_raw_os_error(5);
        let expected = format!(r"lazymkv-engine: Global\x could not be taken: {error}");
        assert_eq!(mutex_refusal_text(r"Global\x", &MutexRefusal::Os(error)), expected);
    }

    #[test]
    fn a_program_data_variable_is_the_registrys_folder_whatever_its_case_or_separators() {
        let registry = Path::new(r"C:\ProgramData");
        for same in [r"C:\ProgramData", r"c:\programdata", r"C:\PROGRAMDATA\", "C:/ProgramData"] {
            assert!(same_folder(Path::new(same), registry), "{same}");
        }
        for other in [r"D:\ProgramData", r"C:\ProgramData\lazymkv", r"C:\Data\ProgramData", r"C:\", "ProgramData", ""] {
            assert!(!same_folder(Path::new(other), registry), "{other}");
        }
    }

    #[test]
    fn the_run_folder_gives_resources_and_lazymkvctl() {
        assert_eq!(binary_paths(Path::new(r"C:\lazymkv\run\lazymkv-engine.exe")),
                   (PathBuf::from(r"C:\lazymkv\run\..\resources"), PathBuf::from(r"C:\lazymkv\run\lazymkvctl.exe")));
    }
}
