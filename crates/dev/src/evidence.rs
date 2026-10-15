//! Where the commands find their inputs (port spec 3.4): the context of a run (cwd, environment, the program runner,
//! the clock), the evidence clone and its exit-3 refusal, the repository root and --spec's default, the OCR text
//! folder, a directory on disk as a `Tree`, and a recording named as a slug or a path.

use crate::runner::{FakeRunner, RealRunner, Runner};
use lazymkv_core::recording::{py_repr_str, Kind, Loader, MemTree, PathKey, ReadError, Tree};
pub use lazymkv_core::recording::pathlib_str;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The fixed clock of `Ctx::for_test`.
const TEST_CLOCK: &str = "2026-10-13T12:00:00";
/// --spec's default below the repository root. Py: audio_labels.SPEC.
const SPEC: &str = "docs/superpowers/specs/2026-10-04-lazymkv-design.md";

/// Everything a command reads from its surroundings, so that tests set all of it.
pub struct Ctx {
    /// The directory relative paths are taken against.
    pub cwd: PathBuf,
    /// The environment variables the commands read: LAZYMKV_EVIDENCE, LAZYMKV_OCR_TEXT, TMPDIR, TEMP, TMP.
    pub env: BTreeMap<String, String>,
    /// ffprobe and ffmpeg (and, for `now`, the clock programs).
    pub runner: Box<dyn Runner>,
    /// A fixed local time "YYYY-MM-DDTHH:MM:SS" for tests; None reads the system clock.
    pub clock: Option<String>,
}

impl Ctx {
    /// The process's cwd, environment, RealRunner, no fixed clock.
    pub fn from_process() -> Ctx {
        Ctx {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env: std::env::vars_os()
                .map(|(name, value)| (name.to_string_lossy().into_owned(), value.to_string_lossy().into_owned()))
                .collect(),
            runner: Box::new(RealRunner),
            clock: None,
        }
    }

    /// A context for tests: `cwd`, an empty environment, FakeRunner::absent(), clock "2026-10-13T12:00:00".
    pub fn for_test(cwd: &Path) -> Ctx {
        Ctx {
            cwd: cwd.to_path_buf(),
            env: BTreeMap::new(),
            runner: Box::new(FakeRunner::absent()),
            clock: Some(TEST_CLOCK.to_string()),
        }
    }

    /// Local time to the second, as datetime.now().isoformat(timespec="seconds"). std has no time zones and no date
    /// crate is allowed, so the clock program says it: `date '+%Y-%m-%dT%H:%M:%S'` (macOS, Linux) or `powershell
    /// -NoProfile -Command "Get-Date -Format s"` (the mini PC), through the runner; when that fails, UTC with a
    /// warning on stderr (02-interfaces.md, open point 5).
    pub fn now(&self) -> String {
        match self.local_time() {
            Ok(time) => time,
            Err((utc, warning)) => {
                let _ = writeln!(std::io::stderr(), "{}", warning);
                utc
            }
        }
    }

    /// The clock logic of `now`, without the printing: Ok(the local time) when the fixed clock or the clock program
    /// gives a valid local time; Err((the UTC time, the warning line `now` prints on stderr)) otherwise.
    pub fn local_time(&self) -> Result<String, (String, String)> {
        if let Some(clock) = &self.clock {
            return Ok(clock.clone());
        }
        let (program, args): (&str, Vec<String>) = if cfg!(windows) {
            ("powershell", vec!["-NoProfile".to_string(), "-Command".to_string(), "Get-Date -Format s".to_string()])
        } else {
            ("date", vec!["+%Y-%m-%dT%H:%M:%S".to_string()])
        };
        if let Ok(output) = self.runner.run(program, &args, None) {
            let text = output.stdout.trim();
            if output.status == 0 && is_local_time(text) {
                return Ok(text.to_string());
            }
        }
        Err((utc_now(), format!("warning: {} did not give the local time; the time is given in UTC", program)))
    }

    /// The local date, YYYY-MM-DD (the first 10 characters of now()). Py: datetime.date.today().isoformat().
    pub fn today(&self) -> String {
        self.now().chars().take(10).collect()
    }

    /// tempfile.gettempdir(): TMPDIR, TEMP, TMP from `env` (made absolute against cwd), else std::env::temp_dir().
    pub fn temp_dir(&self) -> PathBuf {
        for name in ["TMPDIR", "TEMP", "TMP"] {
            if let Some(value) = self.env.get(name).filter(|value| !value.is_empty()) {
                return self.abs(value).components().collect();
            }
        }
        std::env::temp_dir()
    }

    /// `path` taken against `cwd` when relative.
    pub fn abs(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }
}

/// "YYYY-MM-DDTHH:MM:SS", digits where digits belong.
fn is_local_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 19
        && bytes.iter().enumerate().all(|(at, b)| match at {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            _ => b.is_ascii_digit(),
        })
}

/// The time now in UTC, "YYYY-MM-DDTHH:MM:SS".
fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    civil_time(seconds)
}

/// "YYYY-MM-DDTHH:MM:SS" of a count of seconds since 1970-01-01T00:00:00, by the days-to-civil algorithm of the
/// proleptic Gregorian calendar.
fn civil_time(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", year, month, day, rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The private evidence repository's URL. Py: audio_dataset.EVIDENCE.
pub const EVIDENCE_URL: &str = "https://github.com/krcm0209/lazymkv-evidence";
/// How every report prints the evidence root, whatever --evidence says (spec 3.4).
pub const EVIDENCE_SHOWN: &str = "corpus/owner";

/// Why a slug command cannot run: the line it prints (to stderr) and its exit code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub code: i32,
    pub line: String,
}

/// The evidence directory: `flag` (--evidence), else LAZYMKV_EVIDENCE, else "corpus/owner" under the repository
/// root (else under cwd). Refused { code: 3, "<dir>: no such folder; the evidence repository lazymkv-evidence is not
/// cloned there (git clone <EVIDENCE_URL> <dir>)" } when it does not exist, code 2 "<dir>: not a folder" when it is
/// a file; <dir> is the path as given, or the default relative to cwd. Py: audio_dataset.main's corpus check.
pub fn resolve_evidence(ctx: &Ctx, flag: Option<&str>) -> Result<PathBuf, Refused> {
    let given = flag
        .map(str::to_string)
        .or_else(|| ctx.env.get("LAZYMKV_EVIDENCE").filter(|value| !value.is_empty()).cloned());
    let (path, shown) = match given {
        Some(given) => (ctx.abs(&given), given),
        None => evidence_default(&ctx.cwd, repo_root(&ctx.cwd).as_deref()),
    };
    if !path.exists() {
        return Err(Refused {
            code: 3,
            line: format!(
                "{}: no such folder; the evidence repository lazymkv-evidence is not cloned there (git clone {} {})",
                shown, EVIDENCE_URL, shown
            ),
        });
    }
    if !path.is_dir() {
        return Err(Refused { code: 2, line: format!("{}: not a folder", shown) });
    }
    Ok(path)
}

/// The default evidence folder and how it is shown: corpus/owner under the repository root, shown relative to cwd
/// ("corpus/owner" from the root, "../../corpus/owner" from crates/dev); without a root, corpus/owner under cwd.
fn evidence_default(cwd: &Path, root: Option<&Path>) -> (PathBuf, String) {
    match root {
        Some(root) => {
            let depth = cwd.strip_prefix(root).map(|rest| rest.components().count()).unwrap_or(0);
            (root.join(EVIDENCE_SHOWN), format!("{}{}", "../".repeat(depth), EVIDENCE_SHOWN))
        }
        None => (cwd.join(EVIDENCE_SHOWN), EVIDENCE_SHOWN.to_string()),
    }
}

/// The repository root: the nearest ancestor of `start` (itself included) whose Cargo.toml holds a `[workspace]`
/// table (a `[workspace]` or `[workspace.<key>]` header line).
pub fn repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| {
            std::fs::read_to_string(dir.join("Cargo.toml"))
                .map(|text| {
                    text.lines().any(|line| {
                        let line = line.trim();
                        line == "[workspace]" || line.starts_with("[workspace.")
                    })
                })
                .unwrap_or(false)
        })
        .map(Path::to_path_buf)
}

/// --spec's default: docs/superpowers/specs/2026-10-04-lazymkv-design.md under repo_root; None when there is no
/// root (--spec is then required). Py: audio_labels.SPEC.
pub fn default_spec(ctx: &Ctx) -> Option<PathBuf> {
    repo_root(&ctx.cwd).map(|root| root.join(SPEC))
}

/// The OCR text folder: `flag` (--text), else LAZYMKV_OCR_TEXT, else None.
pub fn ocr_text_dir(ctx: &Ctx, flag: Option<&str>) -> Option<PathBuf> {
    match flag {
        Some(given) => Some(ctx.abs(given)),
        None => ctx.env.get("LAZYMKV_OCR_TEXT").filter(|value| !value.is_empty()).map(|value| ctx.abs(value)),
    }
}

/// A directory on disk as a Tree: walked once on open (folders and file names; `.git` left out), file bytes read on
/// first use.
pub struct DiskTree {
    root: PathBuf,
    tree: MemTree,
}

impl DiskTree {
    /// Opens `root`, printed under `display_root` (EVIDENCE_SHOWN, or the path as the user gave it).
    pub fn open(root: &Path, display_root: &str) -> std::io::Result<DiskTree> {
        let mut tree = MemTree::new(display_root, &resolved_name_of(root));
        walk(root, "", &mut tree)?;
        Ok(DiskTree { root: root.to_path_buf(), tree })
    }

    /// Like `open`, but the tree holds only the folder `sub` of `root` (one name, no '/'): a recording named by
    /// slug or path is read without walking the rest of its parent.
    pub fn open_sub(root: &Path, display_root: &str, sub: &str) -> std::io::Result<DiskTree> {
        let mut tree = MemTree::new(display_root, &resolved_name_of(root));
        tree.insert_dir(sub);
        walk(&root.join(sub), sub, &mut tree)?;
        Ok(DiskTree { root: root.to_path_buf(), tree })
    }

    /// The file system path of a key, for writing.
    pub fn path(&self, key: &PathKey) -> PathBuf {
        let mut path = self.root.clone();
        for part in key.components() {
            path.push(part);
        }
        path
    }
}

/// Adds every folder and file under `dir` to `tree`, below `prefix` ("" for the root), files as lazy loaders.
fn walk(dir: &Path, prefix: &str, tree: &mut MemTree) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        let key = if prefix.is_empty() { name } else { format!("{}/{}", prefix, name) };
        let path = entry.path();
        if path.is_dir() {
            tree.insert_dir(&key);
            walk(&path, &key, tree)?;
        } else {
            let shown = tree.display(&PathKey::new(&key));
            let load: Loader = Arc::new(move || std::fs::read(&path).map_err(|error| os_error_text(&error, &shown)));
            tree.insert_lazy(&key, load);
        }
    }
    Ok(())
}

/// Python's OSError text: "[Errno <n>] <strerror>: '<path>'" (Rust's message without its " (os error n)").
fn os_error_text(error: &std::io::Error, shown: &str) -> String {
    match error.raw_os_error() {
        Some(code) => {
            let text = error.to_string();
            let description = match text.rfind(" (os error ") {
                Some(at) => text[..at].to_string(),
                None => text,
            };
            format!("[Errno {}] {}: {}", code, description, py_repr_str(shown))
        }
        None => error.to_string(),
    }
}

/// The name of `root`'s resolved absolute path ("" when it cannot be resolved). Py: Path.resolve().name.
fn resolved_name_of(root: &Path) -> String {
    std::fs::canonicalize(root)
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

impl Tree for DiskTree {
    fn kind(&self, path: &PathKey) -> Option<Kind> {
        self.tree.kind(path)
    }

    fn list(&self, dir: &PathKey) -> Vec<PathKey> {
        self.tree.list(dir)
    }

    fn glob(&self, base: &PathKey, pattern: &str) -> Vec<PathKey> {
        self.tree.glob(base, pattern)
    }

    fn read(&self, path: &PathKey) -> Result<Arc<[u8]>, ReadError> {
        self.tree.read(path)
    }

    fn display(&self, path: &PathKey) -> String {
        self.tree.display(path)
    }

    fn resolved_name(&self, path: &PathKey) -> String {
        self.tree.resolved_name(path)
    }
}

/// A recording named on the command line, as a slug under the evidence or as a directory path.
pub struct Recording {
    /// The evidence root (slug form) or the recording's parent directory (path form).
    pub tree: DiskTree,
    /// The recording folder inside `tree`.
    pub dir: PathKey,
    /// Its folder name.
    pub slug: String,
}

/// `arg` is a path when it names an existing directory (relative to cwd), else a slug under
/// resolve_evidence(flag) (exit 3 as there; code 2 "corpus/owner/<arg>: no such recording (neither a recording
/// folder nor a slug under corpus/owner)" when the slug is not a folder there). The path form prints paths as given
/// and makes no evidence check (spec 3.4).
pub fn resolve_recording(ctx: &Ctx, evidence_flag: Option<&str>, arg: &str) -> Result<Recording, Refused> {
    let given = ctx.abs(arg);
    if given.is_dir() {
        let (parent, parent_shown, name) = match split_recording_path(ctx, arg) {
            Some(parts) => parts,
            None => {
                let canonical = std::fs::canonicalize(&given)
                    .map_err(|error| Refused { code: 2, line: format!("{}: {}", arg, error) })?;
                let parent = canonical
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| Refused { code: 2, line: format!("{}: not a recording folder", arg) })?;
                let name = canonical.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                let shown = parent.display().to_string();
                (parent, shown, name)
            }
        };
        let tree = DiskTree::open_sub(&parent, &parent_shown, &name)
            .map_err(|error| Refused { code: 2, line: format!("{}: {}", arg, error) })?;
        return Ok(Recording { tree, dir: PathKey::new(&name), slug: name });
    }
    let evidence = resolve_evidence(ctx, evidence_flag)?;
    let simple = !arg.is_empty() && arg != "." && arg != ".." && !arg.contains('/') && !arg.contains('\\');
    if !simple || !evidence.join(arg).is_dir() {
        return Err(Refused {
            code: 2,
            line: format!(
                "{}/{}: no such recording (neither a recording folder nor a slug under {})",
                EVIDENCE_SHOWN, arg, EVIDENCE_SHOWN
            ),
        });
    }
    let tree = DiskTree::open_sub(&evidence, EVIDENCE_SHOWN, arg)
        .map_err(|error| Refused { code: 2, line: format!("{}/{}: {}", EVIDENCE_SHOWN, arg, error) })?;
    Ok(Recording { tree, dir: PathKey::new(arg), slug: arg.to_string() })
}

/// (the parent folder on disk, the parent as the user wrote it, the recording's name) for a recording path, split
/// as pathlib splits str(Path(arg)); None for ".", "..", "/" or a path ending in "..", which are resolved instead.
fn split_recording_path(ctx: &Ctx, arg: &str) -> Option<(PathBuf, String, String)> {
    let normal = if cfg!(windows) { arg.replace('\\', "/") } else { arg.to_string() };
    let shown = pathlib_str(&normal);
    let (head, name) = match shown.rsplit_once('/') {
        Some(("", name)) => ("/".to_string(), name.to_string()),
        Some((head, name)) => (head.to_string(), name.to_string()),
        None => (".".to_string(), shown.clone()),
    };
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let parent = ctx.abs(arg).parent()?.to_path_buf();
    Some((parent, head, name))
}

/// The line `skip` writes: "skipped: <reason>" and a newline.
pub fn skip_line(reason: &str) -> String {
    format!("skipped: {}\n", reason)
}

/// A test whose input is absent: writes `skip_line(reason)` straight to std::io::stderr() (not through eprintln!,
/// which the test harness captures), so `cargo test` shows it; the test then returns and passes.
pub fn skip(reason: &str) {
    let _ = std::io::stderr().write_all(skip_line(reason).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::Output;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lazymkv-dev-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A repository: a workspace manifest, a crates/dev folder, and with `clone` an evidence clone holding one
    /// recording, a golden folder and a .git folder.
    fn repository(name: &str, clone: bool) -> PathBuf {
        let root = scratch(name);
        write(&root.join("Cargo.toml"), "[workspace]\nmembers = []\n");
        write(&root.join("crates/dev/Cargo.toml"), "[package]\nname = \"x\"\n");
        if clone {
            write(&root.join("corpus/owner/disc-a/scan.txt"), "MSG:1005,0,1,\"made-up line\"\n");
            write(&root.join("corpus/owner/disc-a/all-tracks/rip-summary.json"), "{\"ok\": true, \"title\": 0}");
            write(&root.join("corpus/owner/disc-a/golden/scan.json"), "{}");
            write(&root.join("corpus/owner/.git/HEAD"), "ref: refs/heads/main\n");
        }
        root
    }

    #[test]
    fn missing_evidence_refuses_with_exit_3_and_the_clone_line() {
        let root = repository("no-clone", false);
        let ctx = Ctx::for_test(&root.join("crates/dev"));
        assert_eq!(
            resolve_evidence(&ctx, None),
            Err(Refused {
                code: 3,
                line: "../../corpus/owner: no such folder; the evidence repository lazymkv-evidence is not cloned there \
                       (git clone https://github.com/krcm0209/lazymkv-evidence ../../corpus/owner)"
                    .to_string(),
            })
        );
        let ctx = Ctx::for_test(&root);
        assert_eq!(
            resolve_evidence(&ctx, None).unwrap_err().line,
            "corpus/owner: no such folder; the evidence repository lazymkv-evidence is not cloned there \
             (git clone https://github.com/krcm0209/lazymkv-evidence corpus/owner)"
        );
        assert_eq!(
            resolve_evidence(&ctx, Some("elsewhere")),
            Err(Refused {
                code: 3,
                line: "elsewhere: no such folder; the evidence repository lazymkv-evidence is not cloned there \
                       (git clone https://github.com/krcm0209/lazymkv-evidence elsewhere)"
                    .to_string(),
            })
        );
        write(&root.join("a-file"), "");
        assert_eq!(
            resolve_evidence(&ctx, Some("a-file")),
            Err(Refused { code: 2, line: "a-file: not a folder".to_string() })
        );
    }

    #[test]
    fn evidence_comes_from_the_flag_then_the_environment_then_the_repository() {
        let root = repository("with-clone", true);
        let mut ctx = Ctx::for_test(&root.join("crates/dev"));
        let evidence = root.join("corpus/owner");
        assert_eq!(resolve_evidence(&ctx, None).unwrap(), root.join("corpus").join("owner"));
        ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), "../../corpus".to_string());
        assert_eq!(resolve_evidence(&ctx, None).unwrap(), root.join("crates/dev").join("../../corpus"));
        assert_eq!(resolve_evidence(&ctx, Some(evidence.to_str().unwrap())).unwrap(), evidence);
    }

    #[test]
    fn the_default_evidence_is_shown_relative_to_cwd() {
        let (path, shown) = evidence_default(Path::new("/work/here"), None);
        assert_eq!((path, shown.as_str()), (PathBuf::from("/work/here/corpus/owner"), "corpus/owner"));
        let (path, shown) = evidence_default(Path::new("/work/repo/crates/dev"), Some(Path::new("/work/repo")));
        assert_eq!((path, shown.as_str()), (PathBuf::from("/work/repo/corpus/owner"), "../../corpus/owner"));
        let (path, shown) = evidence_default(Path::new("/work/repo"), Some(Path::new("/work/repo")));
        assert_eq!((path, shown.as_str()), (PathBuf::from("/work/repo/corpus/owner"), "corpus/owner"));
    }

    #[test]
    fn repo_root_is_the_nearest_folder_with_a_workspace_manifest() {
        let root = repository("root-search", false);
        assert_eq!(repo_root(&root.join("crates/dev")), Some(root.clone()));
        assert_eq!(repo_root(&root), Some(root.clone()));
        let ctx = Ctx::for_test(&root.join("crates/dev"));
        assert_eq!(default_spec(&ctx), Some(root.join("docs/superpowers/specs/2026-10-04-lazymkv-design.md")));
        write(&root.join("crates/dev/inner/Cargo.toml"), "[workspace.package]\nedition = \"2021\"\n");
        assert_eq!(repo_root(&root.join("crates/dev/inner")), Some(root.join("crates/dev/inner")));
    }

    #[test]
    fn ocr_text_comes_from_the_flag_then_the_environment() {
        let mut ctx = Ctx::for_test(Path::new("/work"));
        assert_eq!(ocr_text_dir(&ctx, None), None);
        ctx.env.insert("LAZYMKV_OCR_TEXT".to_string(), "text".to_string());
        assert_eq!(ocr_text_dir(&ctx, None), Some(PathBuf::from("/work/text")));
        assert_eq!(ocr_text_dir(&ctx, Some("/data/ocr")), Some(PathBuf::from("/data/ocr")));
    }

    #[test]
    fn the_test_context_has_a_fixed_clock_and_takes_paths_against_cwd() {
        let mut ctx = Ctx::for_test(Path::new("/work"));
        assert_eq!(ctx.now(), "2026-10-13T12:00:00");
        assert_eq!(ctx.today(), "2026-10-13");
        assert_eq!(ctx.abs("a/b"), PathBuf::from("/work/a/b"));
        ctx.env.insert("TMP".to_string(), "tmp-c".to_string());
        assert_eq!(ctx.temp_dir(), PathBuf::from("/work/tmp-c"));
        ctx.env.insert("TEMP".to_string(), "tmp-b/".to_string());
        assert_eq!(ctx.temp_dir(), PathBuf::from("/work/tmp-b"));
        ctx.env.insert("TMPDIR".to_string(), "/tmp-a".to_string());
        assert_eq!(ctx.temp_dir(), PathBuf::from("/tmp-a"));
    }

    #[test]
    fn local_time_asks_the_runner_and_falls_back_to_utc_with_a_warning() {
        let mut ctx = Ctx::for_test(Path::new("/work"));
        assert_eq!(ctx.local_time(), Ok("2026-10-13T12:00:00".to_string()));
        ctx.clock = None;
        ctx.runner = Box::new(FakeRunner::scripted(vec![Output {
            status: 0,
            stdout: "2026-10-13T21:30:05\n".to_string(),
            stderr: String::new(),
        }]));
        assert_eq!(ctx.local_time(), Ok("2026-10-13T21:30:05".to_string()));
        ctx.runner = Box::new(FakeRunner::absent());
        let (utc, warning) = ctx.local_time().unwrap_err();
        assert!(is_local_time(&utc), "{}", utc);
        let program = if cfg!(windows) { "powershell" } else { "date" };
        assert_eq!(warning, format!("warning: {} did not give the local time; the time is given in UTC", program));
        ctx.runner = Box::new(FakeRunner::scripted(vec![Output {
            status: 0,
            stdout: "not a time\n".to_string(),
            stderr: String::new(),
        }]));
        assert_eq!(ctx.local_time().unwrap_err().1, warning);
    }

    #[test]
    fn civil_time_counts_days_as_the_gregorian_calendar() {
        assert_eq!(civil_time(0), "1970-01-01T00:00:00");
        assert_eq!(civil_time(951_868_799), "2000-02-29T23:59:59");
        assert_eq!(civil_time(1_792_065_600), "2026-10-15T12:00:00");
        assert_eq!(civil_time(4_102_444_800), "2100-01-01T00:00:00");
        assert_eq!(civil_time(1_700_000_000), "2023-11-14T22:13:20");
    }

    #[test]
    fn disk_tree_walks_names_once_leaves_out_git_and_reads_on_first_use() {
        let root = repository("disk-tree", true);
        let evidence = root.join("corpus/owner");
        let tree = DiskTree::open(&evidence, EVIDENCE_SHOWN).unwrap();
        assert_eq!(tree.list(&PathKey::root()), vec![PathKey::new("disc-a")]);
        assert_eq!(
            tree.list(&PathKey::new("disc-a")),
            vec![PathKey::new("disc-a/all-tracks"), PathKey::new("disc-a/scan.txt")]
        );
        assert!(!tree.exists(&PathKey::new(".git")));
        assert!(tree.is_dir(&PathKey::new("disc-a/golden")));
        assert_eq!(tree.display(&PathKey::new("disc-a/scan.txt")), "corpus/owner/disc-a/scan.txt");
        assert_eq!(tree.resolved_name(&PathKey::root()), "owner");
        assert_eq!(tree.path(&PathKey::new("disc-a/scan.txt")), evidence.join("disc-a").join("scan.txt"));
        fs::write(evidence.join("disc-a/scan.txt"), "changed after the walk\n").unwrap();
        assert_eq!(&*tree.read(&PathKey::new("disc-a/scan.txt")).unwrap(), b"changed after the walk\n");
        fs::remove_file(evidence.join("disc-a/all-tracks/rip-summary.json")).unwrap();
        let error = tree.read(&PathKey::new("disc-a/all-tracks/rip-summary.json")).unwrap_err().to_string();
        assert!(error.starts_with("[Errno 2] "), "{}", error);
        assert!(error.ends_with(": 'corpus/owner/disc-a/all-tracks/rip-summary.json'"), "{}", error);
    }

    #[test]
    fn a_recording_is_a_folder_path_or_a_slug_under_the_evidence() {
        let root = repository("recording", true);
        write(&root.join("recs/disc-b/scan.txt"), "");
        write(&root.join("recs/other/notes.txt"), "not walked");
        let ctx = Ctx::for_test(&root);
        let by_path = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        assert_eq!((by_path.slug.as_str(), by_path.dir.clone()), ("disc-b", PathKey::new("disc-b")));
        assert_eq!(by_path.tree.display(&by_path.dir.join("scan.txt")), "recs/disc-b/scan.txt");
        assert!(!by_path.tree.exists(&PathKey::new("other")));
        let by_slug = resolve_recording(&ctx, None, "disc-a").unwrap();
        assert_eq!(by_slug.slug, "disc-a");
        assert_eq!(by_slug.tree.display(&by_slug.dir.join("scan.txt")), "corpus/owner/disc-a/scan.txt");
        assert_eq!(
            by_slug.tree.list(&by_slug.dir),
            vec![PathKey::new("disc-a/all-tracks"), PathKey::new("disc-a/scan.txt")]
        );
        assert_eq!(
            resolve_recording(&ctx, None, "disc-z").err().unwrap(),
            Refused {
                code: 2,
                line: "corpus/owner/disc-z: no such recording (neither a recording folder nor a slug under corpus/owner)"
                    .to_string(),
            }
        );
        let bare = Ctx::for_test(&repository("recording-no-clone", false));
        assert_eq!(resolve_recording(&bare, None, "disc-a").err().unwrap().code, 3);
    }

    #[test]
    fn skip_line_is_the_reason_after_skipped_and_ends_in_a_newline() {
        assert_eq!(skip_line("no clone"), "skipped: no clone\n");
        assert_eq!(skip_line(""), "skipped: \n");
        // `skip` itself writes to the real stderr, so no test calls it; this only checks that it takes a &str.
        let _: fn(&str) = skip;
    }
}
