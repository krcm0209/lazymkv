# Phase 2 Pull Request 2: Thinnest Whole Job Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A media-arrival event takes a synthetic movie disc to a filed placeholder file on ubuntu, macos and windows. It uses stand-in tools, in-memory platform fakes and recorded HTTP answers, with no real disc, no network and no Windows API. The PR also fixes, as serde types in `core`, the interfaces between tools, reducer actions, persistence and the IPC messages of design part 9.3 (design part 12.6, item 2).

**Architecture:**
- `core` gains the pure pieces: lower-case job IDs, the disc snapshot's pure part, a ustar reader, the `makemkvcon` line reducer, pure subtitle-event reading, and the persisted and IPC record types.
- `platform`, `tools` and `clients` hold the traits with their fakes, the recorded tool runs and the HTTP transport.
- `engine` is a library. One job-owner thread runs `core::job::reduce` and persists each transition before its actions run. It performs the no-event actions itself and hands tool steps to a `StepRunner`.
- `ctl` routes `plan`, `decide` and `retune` to `lazymkv_dev::dispatch`, and sends engine commands over an `IpcConnector`.
- `dev`'s decide splits into `read_decide_inputs` and a pure `decide_doc(&DecideInputs) -> DecideDoc`. The engine calls that renderer for each `decide.json` (K5, R-B1).

**Tech Stack:** Rust 1.99.0, edition 2021.
- Workspace crates in use: `serde`, `serde_json` (`preserve_order`, `float_roundtrip`), `md-5`, `sha1`, `regex`, `fancy-regex`, `unicode-normalization` (Cargo.toml:10-19).
- New path crates: `lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-engine` (bin `lazymkv-engine`), `lazymkv-ctl` (bin `lazymkvctl`).
- **PR 2 adds no external crate.** `windows-sys` waits for PR 3, and `ureq` with its TLS stack waits for PR 4 (design parts 4.7, 12.6).
- Task 1 adds every path edge of the PR at once, so `Cargo.lock` changes once. Task 1 prints that delta for the owner's yes.

**Spec:** `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (the design; binding with P1 to P37, R-A, R-B, K1 to K15 and L1 to L15) and `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (the spec). `docs/phase1/decision-core.md` holds the phase 1 rulings that still bind (RP19, RP21, RP23, RP24, OR13).

**Branch:** `phase-2-thin-job`, from `main` at `70ada2b` (the merge of pull request #5).

## Global Constraints

- **Toolchain and commands.** Rust 1.99.0, edition 2021. Every cargo command runs on the mini PC through `scripts/box.sh`, one at a time.
  - A task that touches dev's `decide`, `plan` or `retune`, or the RP21 guard, runs the junction form: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`.
  - `LAZYMKV_OCR_TEXT` is never set. Nobody reads or writes `corpus/owner`.
- **Dependencies.**
  - No external dependency.
  - Task 1's first build is `scripts/box.sh cargo build --workspace --offline`. Every later command uses `--locked`, and no later task edits a `Cargo.toml` dependency list.
  - No tokio, reqwest, clap, `windows`, `windows-sys`, date-time, Matroska or tar crate (design part 4.6).
- **No I/O in core (RP19).** The ustar reader, the snapshot's pure part, the line reducer, `subtitle_events_of` and the record types take bytes, strings and listings, never paths.
- **No Windows API in PR 2.** Code under `cfg(windows)` is path and suffix handling only.
- **RP21.**
  - No file under `crates/` or `corpus/scenarios/` holds a byte copy of, or a value read only from, `corpus/owner`.
  - Scenario values are synthetic, or come from `corpus/thediscdb-api` and `core::testsupport`.
  - Task 23 extends the guard to `corpus/scenarios/`.
- **Stand-ins never run inside `corpus/`.** Tests first copy `fake-tool` and the scenario folder (the base, then the variant) into a temporary folder, because the stand-in writes its response counter into the scenario folder.
- **Unchanged output.** The goldens stay byte-identical (K5), and so does `capture disc`'s output (R-B2).
- **Docs and prose.** Every `pub` item has a `///` doc naming its spec section, design part or ruling, ending "Def: T<n>. Use: T<m>.". Prose is plain sentences, no em-dashes, acronyms expanded at first use.
- **Tests.**
  - Tests go in the file's `mod tests` or a sibling `tests.rs`. Names are snake_case sentences.
  - Tests that run `fake-tool` live in `crates/fake-tools/tests/` (Ruling S12).
  - Platform fakes sit behind `lazymkv-platform`'s `fakes` feature, and the engine harness behind `lazymkv-engine`'s `testkit` feature and the engine's own `cfg(test)`. Both features are enabled only as dev-dependencies, and the engine's unit tests need no feature flag (Task 1). Task 30 counts the tests from one workspace run.
- **Persisted files.**
  - New persisted files go through `platform::write_atomic` and carry their `format` exactly: `lazymkv job v1`, `lazymkv drives v1`, `lazymkv remembered v1` (design part 7.4). Settings files have no `format` key.
  - New files write times as Unix seconds. Phase 0-shaped files keep `yyyy-MM-dd` dates (`fetched_at`) from `Host::local_date` (design part 4.6).
- **TDD cycle per task.** Write the listed tests. Run the task's **Run** command and see the named failure (compile error or FAIL). Implement. Run again for PASS. Commit.
- **Commits.** One signed commit per task, with no attribution line. `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` must show `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>`.

## Review Focus

1. **Non-UTF-8 bytes in `makemkvcon` output**, such as a Windows-1252 disc name in `CINFO`. The recorded file keeps the bytes unchanged, and the reducer and planner read them with replacement (PIPE-14, design part 5.3).
   - Tests: `a_scan_line_with_invalid_utf8_is_recorded_byte_for_byte_and_read_with_replacement` (Task 13), and on the engine `a_disc_name_in_windows_1252_reaches_ejected_with_scan_txt_unchanged` (Task 23).
2. **The drive id against the `DRV` seventh field.** Design part 5.4 says "equals", so PR 2 pins exact equality. PR 3's `DriveWatcher` must report the id in MakeMKV's `"D:"` form (see Not covered).
   - Test: `the_drive_index_requires_an_exact_match_of_the_seventh_field` (`"D:"` matches; `"D:\\"` and `"d:"` give None). Owner: Task 13.
3. **The first remux into a library root that has no `.lazymkv-partial` folder yet.**
   - Test: `the_remux_creates_the_partial_folder_when_the_library_root_has_none`. Owner: Task 24.
4. **A crash between replacing `state.json` and appending its `transitions.jsonl` line** (design part 7.3).
   - Test: `a_restart_after_a_lost_last_transition_line_resumes_from_state_json_and_appends_after_it`. Owner: Task 25.
5. **A disc pulled during its fingerprint walk.** Design part 5.4 step 2 asks for one `failed` with class `Unknown`, a null `job_id`, an `engine.log` line, and no job folder.
   - Test: `a_disc_removed_during_its_fingerprint_walk_sends_one_failed_with_a_null_job_id_and_makes_no_job`. Owner: Task 22. A regular file stands for the volume root, so the walk fails as it does for a pulled disc; the test also asserts that nothing was ejected.

## File structure

| Path | Change | Responsibility |
|---|---|---|
| `Cargo.toml:3` | Modify | Members gain `crates/platform`, `crates/tools`, `crates/clients`, `crates/engine`, `crates/ctl` |
| `Cargo.lock` | Modify | Five path packages; capture and fake-tools gain dependency names |
| `crates/{platform,tools,clients,engine,ctl}/Cargo.toml` | Create | Full dependency lists (Task 1) |
| `crates/capture/Cargo.toml`, `crates/fake-tools/Cargo.toml` | Modify | capture→tools; fake-tools dev-dependencies (Task 1) |
| `crates/core/src/lib.rs` | Modify | `pub mod ipc; pub mod record; pub mod snapshot; pub mod ustar;` |
| `crates/core/src/job.rs:1153-1163`, `job/tests.rs:2271-2316, :2409` | Modify | Lower-case `job_id` (K3) |
| `crates/core/src/snapshot.rs`, `ustar.rs`, `record.rs`, `ipc.rs` | Create | R-B2 pure part; ustar; records; IPC |
| `crates/core/src/profile.rs:5` | Modify | `FALLBACK_SELECTION`, `JOB_PROFILE_FILE`, `FALLBACK_PROFILE_FILE` (L4) |
| `crates/core/src/robot.rs:123`, `robot/lines.rs` | Modify, Create | `message` becomes `pub(crate)`; line reducer (PIPE-10) |
| `crates/core/src/verify.rs:565-600` | Modify | `PacketsFile`, `read_packets_file`, `subtitle_events_of` |
| `crates/core/src/lookup/mod.rs` | Modify | `pub fn record_to_fetch` over `:534`, `:616`, `:666` |
| `crates/core/src/testsupport/mod.rs` | Modify | `ustar_build`, `default_profile_xml` |
| `crates/platform/src/{lib.rs,process.rs,fakes.rs}` | Create | Traits, `write_atomic`, `rename_no_replace`, `StdProcessGuard`, fakes |
| `crates/tools/src/{lib.rs,record.rs,lookup.rs,snapshot.rs,makemkv.rs,mkvmerge.rs,ffprobe.rs}` | Create | Limits, runs, lookup, snapshot I/O, drivers |
| `crates/clients/src/{lib.rs,transport.rs,recorded.rs,lookups.rs,saved.rs,chain.rs}` | Create | Transport, recorded answers, lookups, slim copies, chain |
| `crates/engine/src/{lib.rs,main.rs,store.rs,owner.rs,arrival.rs,space.rs,steps.rs,file_phase.rs,restart.rs,ipc_server.rs,notify.rs,timers.rs,selftest.rs,testkit.rs}` | Create (lib.rs and main.rs in Task 1) | Engine |
| `crates/engine/tests/binary.rs` | Create (Task 21) | The binary's exit and message, one test compiled per OS |
| `crates/ctl/src/{lib.rs,main.rs,engine_cmds.rs}` | Create (lib.rs and main.rs in Task 1) | `run`, routing, engine commands |
| `crates/capture/src/disc.rs`, `src/record.rs`, `src/main.rs:23-34`, `tests/disc_snapshot.rs` | Modify, Create | Call `core::snapshot` and `tools`; characterisation test |
| `crates/dev/src/cmd/decide.rs:1-60, :231-505, :368` | Modify | K5 split, replay flags, root refusal, pub `rec_folder` |
| `crates/dev/src/cmd/decide/characterisation/*.json` | Create | Pre-split stdout of each made-up fixture |
| `crates/dev/src/cmd/plan.rs:270-283`, `src/lib.rs:23-28`, `src/lib.rs:84-207` | Modify | `pub RecordedAnswers`, `recorded_answers`; usage row; `Args::rest` (Task 27) |
| `crates/dev/tests/decide.rs:36, :179, :784`, `tests/clone_values.rs:23-24, :139` | Modify | New calls; guard over scenarios and `.json` files |
| `crates/fake-tools/src/{main.rs,scenario.rs}`, `tests/replay.rs` | Modify | Design part 12.2 features |
| `crates/fake-tools/tests/{tools_record,makemkv_driver,mkvmerge_ffprobe_driver,engine_disc_phase,engine_file_phase,engine_restart,engine_ipc,ctl_engine,whole_job}.rs` | Create | Stand-in tests |
| `corpus/scenarios/movie-direct/`, `movie-direct-*/` | Create | The synthetic disc and its variants (Tasks 23 to 26) |
| `.github/workflows/ci.yml:3-5`, `README.md:23-46`, `docs/phase2/engine.md` | Modify, Create | Stale comment, crate list, PR 2 notes |

## Interface sheet

All names below are used exactly as written. They come from design parts 4.1, 4.2, 5.2 to 5.8, 7.1 to 7.4, 8.1 to 8.6 and 9.1 to 9.6. Paths are qualified wherever core has two types with the same name.

### core

```rust
// core::job (K3)
pub fn job_id(fingerprint: &str, earlier: &[&str]) -> String; // "<fingerprint lower-cased>-<n>"; only exact lower-case earlier IDs count

// core::profile (design part 5.4, K6, L4)
pub const FALLBACK_SELECTION: &str = "+sel:all,-sel:mvcvideo";
pub const JOB_PROFILE_FILE: &str = "job.mmcp.xml";
pub const FALLBACK_PROFILE_FILE: &str = "fallback.mmcp.xml";

// core::snapshot (R-B2); field order is meta.json's key order
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DiscMeta { pub volume_label: String, pub format: Option<String>, pub fingerprint: Option<String>,
    pub disc_id: Option<String>, pub file_count: usize, pub total_bytes: u64, pub captured_unix: u64 }
pub fn is_nav_file(relative_path: &str) -> bool;                  // from crates/capture/src/disc.rs:57
pub fn unit_key_file(files: &[DiscFile]) -> Option<&DiscFile>;    // from disc.rs:66
pub fn disc_meta(files: &[DiscFile], volume_label: &str, unit_key: Option<&[u8]>, captured_unix: u64) -> DiscMeta;

// core::ustar (design part 5.4)
pub fn ustar_entry<'a>(archive: &'a [u8], name: &str) -> Result<Option<&'a [u8]>, String>; // last path part equals `name`
pub const DEFAULT_PROFILE_NAME: &str = "default.mmcp.xml";

// core::robot::lines (PIPE-10, design part 5.3)
pub enum Call { DriveList, Scan, Rip { index: u32, source: disc::TitleSource } }
pub enum LineEvent { Progress { fraction: f64 }, IndexMismatch, Licence { code: u32, text: String }, ProfileRejected { text: String } }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunSummary { pub version: Option<String>, pub java: bool, pub profile_rejected: Option<String>,
    pub licence: Option<robot::Message>, pub index_guard: Option<bool>, pub read_errors: bool, pub write_error: bool,
    pub removed: Vec<robot::Message>, pub title_saved: bool, pub title_failed: bool, pub scan_complete: bool,
    pub drives: Vec<robot::Drive> }
pub struct LineReader { /* private; holds a scratch robot::Scan so robot::message is reused */ }
impl LineReader { pub fn new(call: Call) -> LineReader; pub fn feed(&mut self, line: &str) -> Vec<LineEvent>; pub fn finish(self) -> RunSummary; }
pub const LICENCE_CODES: [u32; 5] = [5021, 5050, 5052, 5055, 5095];

// core::verify (K5, S25): one copy of the subtitle-event rule
pub struct PacketsFile { pub shown: String, pub packets: Result<Vec<recording::files::Packet>, String> }
pub fn read_packets_file(tree: &dyn Tree, subs_dir: &PathKey, n: i64) -> PacketsFile; // subtitle_events' read and error texts
pub fn subtitle_events_of(index: &[SubsIndexEntry], packets: &BTreeMap<i64, PacketsFile>, report: &VerifyReport,
    clear_max_bytes: i64) -> Result<BTreeMap<u32, Vec<f64>>, String>;              // subtitle_events (:577) calls it

// core::record (design parts 7.3, 7.4; RP23)
pub const JOB_FORMAT: &str = "lazymkv job v1";
pub const DRIVES_FORMAT: &str = "lazymkv drives v1";
pub const REMEMBERED_FORMAT: &str = "lazymkv remembered v1";
pub fn check_format(document: &serde_json::Value, expected: &str) -> Result<(), String>;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobRecord { pub format: String, pub id: String, pub fingerprint: String, pub disc_id: Option<String>,
    pub drive: String, pub created_unix: u64, pub updated_unix: u64, pub state: job::State, pub message: Option<String>,
    pub rip_again: bool, pub profile: String, pub plan: Option<plan::Plan>, pub titles: Vec<TitleRecord>,
    pub filed_unix: Option<u64>, pub collisions: Vec<String>, pub notes: Vec<review::Noted> }
pub struct TitleRecord { pub key: String, pub scan_title: u32, pub rip_index: Option<u32>, pub attempts: u32,
    pub verified: bool, pub read_errors: Vec<verify::ReadError>, pub rip_reviews: Vec<review::ReviewReason>,
    pub decision: Option<decision::FileDecision>, pub filed: Option<FiledFile> }
pub struct FiledFile { pub path: String, pub size: u64, pub mtime_unix: u64 }
pub struct Transition { pub time_unix: u64, pub event: Option<job::Event>, pub old: Option<job::State>, pub new: job::State, pub actions: Vec<job::Action> }
pub struct DrivesFile { pub format: String, pub drives: BTreeMap<String, DriveRecord>, pub drive_incapable: Vec<DriveIncapableEntry> }
pub struct DriveRecord { pub fingerprint: Option<String>, pub handled: bool }
pub struct DriveIncapableEntry { pub drive_name: String, pub format: Option<String>, pub job_id: String, pub time_unix: u64, pub message: String }
pub fn drive_name_without_serial(drv: &[String]) -> String; // field 4 cut at its last ASCII space, rest byte for byte (spec 3)

// core::ipc (design parts 9.1 to 9.4, spec 12)
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
pub const LINE_TOO_LONG: &str = "the line is longer than 4 MiB"; // parse_request's and the engine's capped reader's one text
pub const HISTORY_PAGE: usize = 50;
pub const CANCELLED: &str = "cancelled by the owner";
pub const SECRET_NAMES: [&str; 3] = ["tmdb_token", "ha_webhook_url", "webhook_url"];
pub fn is_secret_name(name: &str) -> bool;                      // also `share:\\server\share`
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Envelope { pub v: u32, #[serde(rename = "type")] pub kind: String, pub id: Option<u64>, pub payload: serde_json::Value }
pub enum Request { SettingsSet { changes: serde_json::Map<String, serde_json::Value> }, SecretSet { name: String, value: String },
    Retry { job: String }, Cancel { job: String }, Discard { job: String }, RipAgain { job: String }, RunSelftest,
    History { before: Option<String> }, Shutdown { force: bool }, Command { words: Vec<String> },
    ReviewAction { job: String, action: String, overrides: Option<serde_json::Value> }, UiQuit }
// `type` is the variant in snake_case (settings_set, secret_set, retry, cancel, discard, rip_again, run_selftest, history,
// shutdown, command, review_action, ui_quit). settings_set's whole `payload` object is `changes`; there is no `changes` key on the wire.
pub fn parse_request(line: &str) -> Result<(Option<u64>, Request), String>; // Err is the error reply's line, with the id when readable
pub fn reply_ok(id: Option<u64>, result: serde_json::Value) -> String;
pub fn reply_error(id: Option<u64>, message: &str, problems: &[settings::SettingsProblem]) -> String;
pub enum Push { Snapshot(Snapshot), JobUpdate(JobUpdate), Progress(Progress), SelftestResult(SelfTest),
    CommandOutput { stream: OutputStream, line: String } }
pub fn push_line(push: &Push, id: Option<u64>) -> String;      // id only for CommandOutput
pub fn read_reply(line: &str) -> Result<Reply, String>;
pub enum Reply { Ok { id: Option<u64>, result: serde_json::Value }, Error { id: Option<u64>, message: String, problems: Vec<settings::SettingsProblem> }, Push(Envelope) }
#[serde(rename_all = "lowercase")] pub enum OutputStream { Stdout, Stderr }
pub struct JobSummary { pub id: String, pub state: job::State, pub title: Option<String>, pub fingerprint: String, pub drive: String,
    pub created_unix: u64, pub updated_unix: u64, pub review_reasons: Vec<review::ReviewReason>, pub notes: Vec<review::Noted>,
    pub class: Option<job::FailureClass>, pub message: Option<String> }
pub fn job_summary(record: &record::JobRecord) -> JobSummary;
pub struct JobUpdate { #[serde(flatten)] pub summary: JobSummary, pub plan: Option<plan::Plan>, pub collisions: Option<Vec<String>> }
pub struct Snapshot { pub jobs: Vec<JobSummary>, pub settings: settings::Settings, pub problems: Vec<settings::SettingsProblem>,
    pub secrets: BTreeMap<String, SecretState>, pub selftest: SelfTest }
pub struct SecretState { pub set: bool, pub host: Option<String> }
pub struct Progress { pub job: String, pub step: String, pub title: Option<String>, pub fraction: f64 }
pub struct SelfTest { pub checks: Vec<SelfTestCheck> }
pub struct SelfTestCheck { pub name: String, pub ok: bool, pub detail: String, pub path: Option<String>,
    pub version: Option<String>, pub found: Option<String>, pub busy: bool }
pub struct Notification { #[serde(flatten)] pub event: job::NotifyEvent, pub job_id: Option<String>, pub fingerprint: Option<String>,
    pub title: Option<String>, pub drive: Option<String>, pub message: String, pub time: u64 }

// core::lookup (design part 8.2 items 3 and 4)
pub fn record_to_fetch(inputs: &LookupInputs<'_>) -> Option<tmdb::TmdbIdentity>;

// core::testsupport
pub fn ustar_build(entries: &[(&str, &[u8])]) -> Vec<u8>;
pub fn default_profile_xml() -> String; // hand-made, one <name> and two defaultSelection attributes
```

### platform (design part 4.2; fakes behind feature `fakes`)

```rust
pub enum DriveEvent { Arrived { drive: String, root: PathBuf }, Removed { drive: String } }
pub trait DriveWatcher: Send { fn next(&mut self, timeout: Duration) -> Option<DriveEvent>; fn present(&self) -> Vec<(String, PathBuf)>; }
pub trait Ejector: Send + Sync { fn eject(&self, drive: &str) -> Result<(), String>; }
pub struct Spawn { pub program: PathBuf, pub args: Vec<String>, pub env: Vec<(String, String)>, pub below_normal: bool } // env: extra variables; the engine passes none
pub trait ProcessGuard: Send + Sync { fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>>; }
pub trait GuardedChild: Send { fn take_stdout(&mut self) -> Box<dyn Read + Send>; fn take_stderr(&mut self) -> Box<dyn Read + Send>;
    fn killer(&self) -> KillHandle; fn wait(&mut self) -> io::Result<Option<i32>>; }
#[derive(Clone)] pub struct KillHandle(pub(crate) Arc<dyn Fn() -> io::Result<()> + Send + Sync>);
impl KillHandle { pub fn kill(&self) -> io::Result<()>; }
pub trait KeepAwake: Send + Sync { fn acquire(&self); fn release(&self); }
pub enum Reach { Reachable, Unreachable(String) }
pub trait ShareConnector: Send + Sync { fn reach(&self, root: &str) -> Reach; fn free_bytes(&self, root: &str) -> io::Result<u64>; fn volume(&self, root: &str) -> Option<String>; }
pub trait SecretStore: Send + Sync { fn get(&self, name: &str) -> Option<String>; fn set(&self, name: &str, value: &str) -> Result<(), String>; }
pub struct IpcStream { pub reader: Box<dyn BufRead + Send>, pub writer: Box<dyn Write + Send> }
pub trait IpcTransport: Send + Sync { fn accept(&self) -> io::Result<IpcStream>; }
pub enum ConnectError { NotRunning, Refused, NotElevated, NoTransport }
pub trait IpcConnector { fn connect(&self) -> Result<IpcStream, ConnectError>; }
pub trait Autostart { fn install(&self) -> Result<(), String>; fn remove(&self) -> Result<(), String>; } // declared only
pub trait Host: Send + Sync { fn now_unix(&self) -> u64; fn local_date(&self) -> String; fn makemkv_app_key(&self) -> Option<String>;
    fn makemkv_debug_log(&self) -> Option<PathBuf>; fn running_processes(&self) -> Vec<String>;
    fn volume_label(&self, root: &Path) -> Option<String>; fn default_install_path(&self, tool: job::Tool) -> Option<PathBuf>; }
pub struct Platform { pub drives: Box<dyn DriveWatcher>, pub ejector: Arc<dyn Ejector>, pub processes: Arc<dyn ProcessGuard>,
    pub awake: Arc<dyn KeepAwake>, pub shares: Arc<dyn ShareConnector>, pub secrets: Arc<dyn SecretStore>,
    pub ipc: Arc<dyn IpcTransport>, pub host: Arc<dyn Host> }
pub struct StdProcessGuard;                 // std::process; kills only the child; below_normal is a no-op
pub const TEMP_SUFFIX: &str = ".lazymkv-tmp";
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()>;   // <file name>.lazymkv-tmp beside it, sync_all, rename; removes a left temp first
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()>; // hard_link then remove_file; ErrorKind::AlreadyExists when `to` exists
pub mod fakes {
    pub struct ScriptedDrives; pub struct DriveScript;   // ScriptedDrives::new() -> (ScriptedDrives, DriveScript) sharing one queue
    // DriveScript (Clone): arrive(&self, drive: &str, root: &Path); remove(&self, drive: &str); present(&self) -> Vec<(String, PathBuf)>
    pub struct RecordingEjector;  // new(); calls() -> Vec<String>; fail_next(&self, message: &str); with_removals(self, script: DriveScript) -> Self (pushes Removed on eject)
    pub struct CountingKeepAwake; // new(); holders() -> usize
    pub struct FakeShares;        // new(); set_free(&self, root: &str, bytes: u64); set_reach(&self, root: &str, reach: Reach); same_volume(&self, a: &str, b: &str); default: each root its own volume, free u64::MAX
    pub struct MemorySecrets;     // new(); with(name, value) -> Self
    pub struct MemoryIpc; pub struct MemoryConnector; // MemoryIpc::new() -> (MemoryIpc, MemoryConnector); MemoryConnector::failing(ConnectError)
    pub struct FakeHost;          // new(local_date: &str, now_unix: u64); set_label(root, label); set_running(Vec<String>); set_app_key(key); set_debug_log(path)
}
```

### tools

```rust
pub struct Limits { pub header: Duration, pub scan: Duration, pub exit_wait: Duration, pub after_kill: Duration }
impl Limits { pub fn from_settings(settings: &Settings) -> Limits; } // 120 s, scan_timeout_min, exit_wait_min, 10 s
#[derive(Clone, Default)] pub struct CancelToken(Arc<AtomicBool>); // cancel(), is_cancelled()
pub struct ToolCtx<'a> { pub guard: &'a dyn ProcessGuard, pub host: &'a dyn Host, pub limits: &'a Limits,
    pub cancel: CancelToken, pub on_output: &'a (dyn Fn(OutputStream, &str) + Sync), pub on_progress: &'a (dyn Fn(f64) + Sync) }
pub struct RunResult { pub exit_code: Option<i32>, pub lines: u64, pub elapsed_ms: u64, pub started_unix: u64, pub timed_out: bool, pub cancelled: bool, pub stdout: Vec<u8> }
pub enum LineAction { Continue, Kill }
pub fn run_recorded(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path, limit: Option<Duration>, below_normal: bool,
    on_line: &mut dyn FnMut(&str) -> LineAction) -> io::Result<RunResult>;
pub use record::{tee_lines, with_suffix, Clock, RealClock};
#[serde(rename_all = "snake_case")] pub enum Found { Settings, Resources, InstallFolder, Path }
pub struct ToolLocation { pub tool: job::Tool, pub path: PathBuf, pub found: Found }
pub fn exe_name(tool: job::Tool) -> String; // makemkvcon64, mkvmerge, ffprobe, ffmpeg, java, plus EXE_SUFFIX
pub fn find_tool(tool: job::Tool, settings: &Settings, resources: Option<&Path>, host: &dyn Host, path_var: Option<&OsStr>) -> Option<ToolLocation>;
pub fn list_files(root: &Path) -> io::Result<Vec<DiscFile>>;
pub fn snapshot(root: &Path, volume_label: &str, out: &Path, captured_unix: u64) -> io::Result<DiscMeta>;
pub struct MakemkvFlags { pub minlength_s: u32, pub cache_mb: u32, pub profile: PathBuf, pub debug: bool }
pub fn drive_list_args(debug: bool) -> Vec<String>;   // ["-r","--cache=1","info","disc:9999"], "--debug" after "-r" when debug
pub fn scan_args(flags: &MakemkvFlags, drive: u32) -> Vec<String>;
pub fn rip_args(flags: &MakemkvFlags, drive: u32, title: u32, dir: &Path) -> Vec<String>;
pub fn drive_index(drives: &[robot::Drive], drive: &str) -> Option<u32>; // exact equality of field 6
pub struct DriveListing { pub index: Option<u32>, pub summary: RunSummary, pub run: RunResult }
pub fn list_drives(ctx: &ToolCtx, program: &Path, debug: bool, drive: &str, out: &Path) -> io::Result<DriveListing>;
pub enum ScanOutcome { Done { text: String, summary: RunSummary }, ProfileRejected { text: String }, Failed { failure: Failure, message: String } }
pub fn scan(ctx: &ToolCtx, program: &Path, flags: &MakemkvFlags, drive: u32, out: &Path) -> io::Result<ScanOutcome>;
pub struct StagingHealth { pub share_healthy: bool, pub out_of_space: bool } // classify_title_failure's first two arguments (job.rs:677)
pub enum RipOutcome { Ripped { file: PathBuf }, IndexMismatch, Licence { message: String }, Failed { failure: Failure, message: String } }
pub struct RipRun { pub outcome: RipOutcome, pub run: RunResult, pub summary: RunSummary }
pub fn rip(ctx: &ToolCtx, program: &Path, flags: &MakemkvFlags, drive: u32, title: u32, source: TitleSource,
    dir: &Path, out: &Path, staging: &dyn Fn() -> StagingHealth) -> io::Result<RipRun>;
pub fn save_debug_log(host: &dyn Host, started_unix: u64, out: &Path) -> Result<(), String>; // "[app_Key removed]"
pub fn ident_args(file: &Path) -> Vec<String>;   // ["-J", <file>]
pub fn ident(ctx: &ToolCtx, program: &Path, file: &Path, out: &Path) -> io::Result<Result<MkvmergeIdent, String>>;
pub fn remux_args(partial: &Path, layout: &Layout, staged: &VerifyReport, raw: &Path) -> Result<Vec<String>, String>;
    // `staged.pairs` maps each layout stream to the raw file's track ID (S60); Err when !staged.tracks_ok or a stream has no track
pub fn gui_progress(line: &str) -> Option<f64>;  // "#GUI#progress 45%" -> Some(0.45)
pub fn remux(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path) -> io::Result<Result<(), String>>;
pub fn probe_args(raw: &Path) -> Vec<String>;    // ["-v","error","-show_streams","-show_format","-of","json",<raw>]
pub fn probe(ctx: &ToolCtx, program: &Path, raw: &Path, out: &Path) -> io::Result<Result<FfprobeStreams, String>>;
```

### clients (design part 8; PR 2 subset)

```rust
pub struct Response { pub status: u16, pub body: Vec<u8> }
pub enum TransportError { Timeout, Unreachable(String), Unrecorded(String) }
pub trait HttpTransport: Send + Sync { fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError>; }
pub const TIMEOUT_S: u64 = 10;
pub fn user_agent() -> String;                                   // "lazymkv/" + CARGO_PKG_VERSION
pub struct RecordedTransport; // new(); answer(url, status, body); fail(url, err); from_file(path, vars: &[(&str, &str)]);
                              // gate(url) -> Gate (Gate::open() releases a held request); requests() -> Vec<(String, Vec<(String, String)>)>; unrecorded() -> Vec<String>
pub const DISCDB: &str = "https://thediscdb.com/api";
pub const TMDB: &str = "https://api.themoviedb.org/3";
pub fn dischash_url(fingerprint: &str) -> String; pub fn discid_url(disc_id: &str) -> String;
pub fn search_url(query: &str) -> String;  pub fn record_url(id: TmdbIdentity) -> String; pub fn authentication_url() -> String; // {TMDB}/authentication
pub struct Fetched { pub url: String, pub status: u16, pub body: Vec<u8> }
pub enum PendingReason { NoToken, TokenRefused, Timeout, Unreachable(String), Status(u16), Unparsed(String) }
pub enum Outcome<T> { Answered { value: T, fetched: Fetched }, Miss { fetched: Fetched }, Pending { reason: PendingReason, fetched: Option<Fetched> } }
pub fn discdb_by_fingerprint(t: &dyn HttpTransport, fingerprint: &str) -> Outcome<DiscDbRecord>; // 404, or 200 read as None: Miss
pub fn discdb_by_disc_id(t: &dyn HttpTransport, disc_id: &str) -> Outcome<Vec<DiscDbRecord>>;
pub fn tmdb_search(t: &dyn HttpTransport, token: Option<&str>, query: &str) -> Outcome<TmdbSearch>;
pub fn tmdb_record(t: &dyn HttpTransport, token: Option<&str>, id: TmdbIdentity) -> Outcome<TmdbRecord>;
pub struct SavedFile { pub name: String, pub bytes: Vec<u8> }
pub fn slim_tmdb(raw: &serde_json::Value, today: &str) -> serde_json::Value;
pub fn slim_record(raw: &serde_json::Value, kind: tmdb::TmdbKind, today: &str) -> serde_json::Value;
pub fn slim_search(raw: &serde_json::Value, query: &str, today: &str) -> serde_json::Value;
pub fn status_text(lines: &BTreeMap<String, String>) -> String;
pub enum Lookup { Dischash, Discid, Search, Record }             // Ord
pub enum LookupState { NotAsked, InFlight, Answered { status: u16 }, Pending { reason: PendingReason } }
pub struct LookupBook { pub states: BTreeMap<Lookup, LookupState>, pub answers: Answers }
pub struct Answers { pub by_fingerprint: Option<DiscDbRecord>, pub by_disc_id: Vec<DiscDbRecord>, pub search: Option<TmdbSearch>, pub record: Option<TmdbRecord> }
pub struct ChainInputs { pub fingerprint: String, pub disc_id: Option<String>, pub name: Option<DiscName> }
impl LookupBook { pub fn pending(&self) -> (bool, bool); pub fn lookup_inputs<'a>(&'a self, name: Option<&'a DiscName>) -> LookupInputs<'a>; }
pub fn run_round(t: &dyn HttpTransport, token: Option<&str>, book: &mut LookupBook, inputs: &ChainInputs, today: &str, save: &mut dyn FnMut(SavedFile));
```

### dev (K5, design parts 7.1, 9.5)

```rust
#[derive(Clone, Debug)] pub struct RipLogText { pub shown: String, pub text: String }
#[derive(Clone, Debug)] pub struct FileFacts {
    pub level_md5: Result<Vec<(Option<i64>, Option<String>)>, String>,   // (file track, trimmed digest), level_md5s' read order
    pub probes: Result<Option<FfprobeStreams>, String>,                   // None: ffprobe-streams.json absent
    pub subs_index: Result<Option<Vec<SubsIndexEntry>>, String>,          // None: subs/index.json absent
    pub packets: BTreeMap<i64, verify::PacketsFile> }                     // every index entry's file, errors held back
#[derive(Clone, Debug)] pub struct DecideInputs { pub slug: String, pub label: String, pub title: i64, pub selection: Selection,
    pub scan: Scan, pub ident: MkvmergeIdent, pub rip_log: Option<RipLogText>, pub reconcile: reconcile::Report,
    pub audio: Vec<AudioEntry>, pub subtitles: SubtitleEntry, pub plan: PlanEntry, pub answers: Result<RecordedAnswers, String>,
    pub file: FileFacts, pub library: LibraryFacts, pub overrides: Option<remembered::Overrides> }
pub fn read_decide_inputs(ctx: &Ctx, rec: &Recording, label: &str, text: Option<&Path>, selection: Option<Selection>,
    options: &PlanOptions) -> Result<DecideInputs, Refused>;
pub fn decide_doc(inputs: &DecideInputs) -> DecideDoc;          // pure, no Ctx, no Tree
pub fn no_text_entry() -> SubtitleEntry;                         // Skipped "no OCR text {TEXT_HINT}"
pub fn rec_folder(ctx: &Ctx, dir: &str) -> Result<(Recording, String), Refused>; // now pub (decide.rs:368)
pub struct EngineDecideDoc { #[serde(flatten)] pub doc: DecideDoc, pub audio_rule_version: u32, pub lazymkv_version: String }
pub struct RecordedAnswers { ... }                               // now pub, Clone
pub fn recorded_answers(rec: &Recording) -> Result<RecordedAnswers, Refused>; // now pub
pub fn replay_answers(ctx: &Ctx, rec: &Recording, options: &PlanOptions) -> Result<RecordedAnswers, String>;
    // recorded_answers with its Refused.line as Err, then --tmdb-record's record as `recorded` (S49); read_decide_inputs
    // and the engine both fill DecideInputs.answers with it (Task 20)
impl Args { pub fn rest(self) -> Result<Vec<String>, String>; } // the words left, unchanged; Err for an ambiguous prefix (Task 27)
```

### engine

```rust
pub struct EngineConfig { pub data: PathBuf, pub resources: Option<PathBuf>, pub limits_override: Option<Limits>, pub path_var: Option<OsString> } // Task 21
pub trait NotifySink: Send { fn deliver(&mut self, notification: &Notification); }
pub trait Timers: Send + Sync { fn start(&self, minutes: u32, fire: Box<dyn FnOnce() + Send>); }
pub struct StdTimers;        // impl Timers; one sleeping thread per timer; unused in PR 2, PR 3's binary passes it (Task 22)
pub struct StepResult { pub event: Option<Event>, pub message: Option<String> }
pub struct JobContext { pub record: JobRecord, pub job_dir: PathBuf, pub settings: Settings, pub drive_root: Option<PathBuf>,
    pub lookups: Arc<Mutex<LookupBook>> }   // created at the job's first RunScan (Task 23)
pub trait StepRunner: Send + Sync { fn run(&self, job: &JobContext, action: &Action, cancel: CancelToken,
    progress: &(dyn Fn(Option<&str>, f64) + Sync)) -> StepResult; }  // (title key, fraction)
pub struct Services { pub platform: Platform, pub http: Arc<dyn HttpTransport>, pub notify: Box<dyn NotifySink>, pub timers: Arc<dyn Timers>, pub steps: Option<Arc<dyn StepRunner>> } // None: the real runner
pub fn start(config: EngineConfig, services: Services) -> io::Result<EngineHandle>; // start_until(config, services, None)
pub(crate) type StopAfter = Arc<dyn Fn(&Transition) -> bool + Send + Sync>;         // the testkit's stop hook (Task 22)
pub(crate) fn start_until(config: EngineConfig, services: Services, stop_after: Option<StopAfter>) -> io::Result<EngineHandle>;
impl EngineHandle { pub fn jobs(&self) -> Vec<JobSummary>; pub fn wait_until(&self, timeout: Duration, done: &dyn Fn(&[JobSummary]) -> bool) -> bool;
    pub fn stop(self) -> io::Result<()>; } // stop cancels every running step (CheckSpace and Eject included), logs `engine stopped`, joins
pub fn main_with(args: &[String], err: &mut dyn Write) -> i32;
#[cfg(any(test, feature = "testkit"))] pub mod testkit { // Task 22 unless noted; every Harness method takes &self
    pub struct MemorySink;     // new() -> (MemorySink, Arc<Mutex<Vec<Notification>>>)
    pub struct ManualTimers;   // new(); fire_all(&self); pending() -> Vec<u32>
    pub enum StepScript { Result(StepResult), Hang, After(Duration, StepResult) } // Hang returns only when cancelled; After (Task 26) returns its result after the delay, cancelled or not
    pub struct StepSeen { pub action: String, pub state: String, pub lines: usize }
    pub struct ScriptedSteps;  // new(script: Vec<(&'static str /* Action tag */, StepScript)>); calls() -> Vec<String>; seen() -> Vec<StepSeen>
    pub struct Harness;
    impl Harness {
        pub fn new_scripted(script: Vec<(&'static str, StepScript)>, settings: serde_json::Value) -> Harness;
        pub fn new(fake_tool: &Path, scenarios: &[&str], settings: serde_json::Value) -> Harness; // Task 23: copies
            // corpus/scenarios/<first>, then each later folder over it; places tool copies and appdata.tar
        pub fn insert(&self, drive: &str, disc: &str); pub fn insert_root(&self, drive: &str, root: &Path); pub fn remove(&self, drive: &str);
            // each queues its drive event; every other method first hands the queue to the DriveScript, in order
        pub fn run_until(&self, state: &str); // stops the engine after it persists the first transition into `state`, before
            // that transition's actions run, and joins it; right after restart, a state already passed stops it at once
        pub fn restart(&self); // stop, drop pending timers, start_until over the same data folder, fakes, drive script,
            // transport, ScriptedSteps (used entries stay used) and notification list
        pub fn fingerprint(&self, disc: &str) -> String; pub fn job_ids(&self) -> Vec<String>; pub fn state_of(&self, id: &str) -> String;
            // job_ids, state_of, record, transitions, engine_log and drives_json read the data folder, so they work while stopped
        pub fn record(&self, id: &str) -> JobRecord; pub fn transitions(&self, id: &str) -> Vec<Transition>;
        pub fn notifications(&self) -> Vec<Notification>; pub fn engine_log(&self) -> String; pub fn drives_json(&self) -> serde_json::Value;
        pub fn steps(&self) -> &ScriptedSteps;
        pub fn wait(&self, what: &str, done: impl Fn(&Harness) -> bool); pub fn settle(&self);
        pub fn data(&self) -> &Path; pub fn staging(&self) -> &Path; pub fn library(&self) -> &Path; pub fn job_dir(&self, id: &str) -> PathBuf;
        pub fn ejector(&self) -> &RecordingEjector; pub fn awake(&self) -> &CountingKeepAwake; pub fn shares(&self) -> &FakeShares;
        pub fn host(&self) -> &FakeHost; pub fn http(&self) -> &RecordedTransport; pub fn timers(&self) -> &ManualTimers; pub fn fire_timers(&self);
        pub fn tools(&self) -> &Path; pub fn scan_waits_for_lookups(&self, wait: bool); // Task 23
        pub fn on_spawn(&self, needles: &[&str], action: impl Fn() + Send + Sync + 'static); // Task 24
        pub fn client(&self) -> MemoryConnector; // Task 26
    }
}
```

Owner thread versus runner (design part 5.2):
- The owner thread itself performs `Notify`, `StartTimer`, `RecordNote`, `ArmRipAgain`, `WaitForDisc` and `RefreshIdentity`. It never hands them to the `StepRunner`.
- A new job's `Fingerprint` is done by the owner's recording thread (design part 5.4 step 3: it "is already done"). Every other `Fingerprint`, which comes only from `EngineRestart` in `Detected`, goes to the `StepRunner`.
- `EmptyTitleDirectory` and `DeletePartialFile` run in the runner and return `event: None`.
- Every other action returns its event.

### ctl (design parts 9.5, 9.6)

```rust
pub struct Env<'a> { pub ctx: &'a Ctx, pub connector: Option<&'a dyn IpcConnector>, pub stdin: &'a mut dyn Read,
    pub out: &'a mut dyn Write, pub err: &'a mut dyn Write }
pub fn run(argv: &[String], env: Env) -> i32;
pub const LOCAL_COMMANDS: [&str; 3] = ["plan", "decide", "retune"];
pub const NO_TRANSPORT: &str = "lazymkvctl: no engine transport exists in this build";
pub const NOT_RUNNING: &str = "lazymkvctl: no engine is running";
pub const REFUSED: &str = "lazymkvctl: the engine refused the connection";
pub const NOT_ELEVATED: &str = "lazymkvctl: run lazymkvctl elevated to reach the engine";
pub(crate) const RETUNE_RELAY_FLAGS: [&str; 9]; // retune's eight flags plus "--jobs" (Task 27)
pub(crate) fn retune_spec(ctx: &Ctx, words: &[String]) -> Result<PathBuf, String>; // --spec, else default_spec; Task 27, used by Task 28
```

## Task index

| Task | Deliverable | Tier |
|---|---|---|
| 1 | Five crates, every edge, lock delta | cheap |
| 2 | Lower-case job IDs (K3) | cheap |
| 3 | `core::snapshot`, capture characterisation | standard |
| 4 | `core::ustar`, profile constants | cheap |
| 5 | `core::robot::lines` | standard |
| 6 | `core::record` | standard |
| 7 | `core::ipc` | standard |
| 8 | Platform traits and fakes | standard |
| 9 | `StdProcessGuard`, `write_atomic`, `rename_no_replace` | standard |
| 10 | fake-tools extensions | standard |
| 11 | tools recording, limits, lookup | standard |
| 12 | tools snapshot; capture switched | cheap |
| 13 | `makemkvcon` drivers | standard |
| 14 | `mkvmerge`, `ffprobe`, debug log | standard |
| 15 | clients transport and single lookups | standard |
| 16 | clients chain and rounds | standard |
| 17 | decide characterisation | cheap |
| 18 | Pure subtitle events in core | cheap |
| 19 | K5 split | standard |
| 20 | decide replay flags, root refusal | cheap |
| 21 | Engine store and binary | standard |
| 22 | Job-owner loop, arrivals, token | standard |
| 23 | Disc-phase steps, scenarios, guard | standard |
| 24 | File-phase steps | standard |
| 25 | Restarts | standard |
| 26 | IPC server, notifier, self-test | standard |
| 27 | ctl local commands | cheap |
| 28 | ctl engine commands | standard |
| 29 | Whole job on three systems | standard |
| 30 | Documents and count | cheap |

## Tasks

### Task 1: Five new crates, every path edge and the `Cargo.lock` delta

**Files:**
- Modify: `Cargo.toml:3` (workspace `members`)
- Modify: `crates/capture/Cargo.toml` (`[dependencies]` gains `lazymkv-tools`)
- Modify: `crates/fake-tools/Cargo.toml` (new `[dev-dependencies]`)
- Create: `crates/platform/Cargo.toml`, `crates/platform/src/lib.rs`
- Create: `crates/tools/Cargo.toml`, `crates/tools/src/lib.rs`
- Create: `crates/clients/Cargo.toml`, `crates/clients/src/lib.rs`
- Create: `crates/engine/Cargo.toml`, `crates/engine/src/lib.rs`, `crates/engine/src/main.rs`
- Create: `crates/ctl/Cargo.toml`, `crates/ctl/src/lib.rs`, `crates/ctl/src/main.rs`
- Modify: `Cargo.lock` (written by cargo on the mini PC and copied back by `scripts/box.sh`)
- Tests: none. This task adds no code, so its check is the offline build and the lock diff.

**Interfaces:**
- Consumes: the workspace dependencies `serde` and `serde_json` (Cargo.toml:13, :16), and the existing path packages `lazymkv-core` (with feature `testsupport`, crates/core/Cargo.toml:10) and `lazymkv-dev` (lib `lazymkv_dev`, crates/dev/Cargo.toml:7-9).
- Produces. These are the complete dependency lists. No later task edits a `Cargo.toml` dependency list.

| Package | Targets | Features | `[dependencies]` | `[dev-dependencies]` |
|---|---|---|---|---|
| `lazymkv-platform` | lib | `fakes = []` | `lazymkv-core` | none |
| `lazymkv-tools` | lib | none | `lazymkv-core`, `lazymkv-platform`, `serde`, `serde_json` | none |
| `lazymkv-clients` | lib | none | `lazymkv-core`, `serde`, `serde_json` | none |
| `lazymkv-engine` | lib `lazymkv_engine`, bin `lazymkv-engine` | `testkit = ["lazymkv-platform/fakes", "lazymkv-core/testsupport"]` | `lazymkv-core`, `lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-dev`, `serde`, `serde_json` | `lazymkv-core` (`testsupport`), `lazymkv-platform` (`fakes`) |
| `lazymkv-ctl` | lib `lazymkv_ctl`, bin `lazymkvctl` | none | `lazymkv-core`, `lazymkv-platform`, `lazymkv-dev`, `serde_json` | `lazymkv-core` (`testsupport`), `lazymkv-platform` (`fakes`) |
| `lazymkv-capture` | unchanged | none | gains `lazymkv-tools` | none |
| `fake-tools` | unchanged | none | unchanged | `lazymkv-core` (`testsupport`), `lazymkv-platform` (`fakes`), `lazymkv-tools`, `lazymkv-clients`, `lazymkv-engine` (`testkit`), `lazymkv-ctl`, `lazymkv-dev` |

- [ ] **Step 1: Write the workspace members and the five manifests**

`Cargo.toml:3` becomes:

```toml
members = ["crates/core", "crates/capture", "crates/fake-tools", "crates/dev", "crates/platform", "crates/tools", "crates/clients", "crates/engine", "crates/ctl"]
```

Every new manifest opens with the four package lines that `crates/capture/Cargo.toml` uses, with its own `name`:

```toml
[package]
name = "<package>"
edition.workspace = true
version.workspace = true
publish.workspace = true
```

`crates/platform/Cargo.toml`, after `[package]`:

```toml
[features]
# The in-memory fakes of design part 4.2 (platform::fakes), for the tests of the crates above this one. Enabled only
# as a dev-dependency or through lazymkv-engine's testkit feature.
fakes = []

[dependencies]
lazymkv-core = { path = "../core" }
```

`crates/tools/Cargo.toml`:

```toml
[dependencies]
lazymkv-core = { path = "../core" }
lazymkv-platform = { path = "../platform" }
serde.workspace = true
serde_json.workspace = true
```

`crates/clients/Cargo.toml`:

```toml
[dependencies]
lazymkv-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
```

`crates/engine/Cargo.toml`:

```toml
[lib]
name = "lazymkv_engine"
path = "src/lib.rs"

[[bin]]
name = "lazymkv-engine"
path = "src/main.rs"

[features]
# The engine test harness (engine::testkit) with the platform fakes and core's test helpers. Enabled only by
# fake-tools' dev-dependency and by `--features testkit`.
testkit = ["lazymkv-platform/fakes", "lazymkv-core/testsupport"]

[dependencies]
lazymkv-core = { path = "../core" }
lazymkv-platform = { path = "../platform" }
lazymkv-tools = { path = "../tools" }
lazymkv-clients = { path = "../clients" }
# The decision document's renderer (R-B1, K5).
lazymkv-dev = { path = "../dev" }
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
# The engine's own unit tests use the platform fakes, core's test helpers and engine::testkit, which is compiled
# under cfg(test) as well as under the testkit feature, so a plain `cargo test -p lazymkv-engine` builds them.
lazymkv-core = { path = "../core", features = ["testsupport"] }
lazymkv-platform = { path = "../platform", features = ["fakes"] }
```

Ruling: lazymkv-engine's dev-dependencies name only packages already in its `[dependencies]`, so the lock delta of Step 4 is unchanged, and every engine unit test compiles without `--features testkit` (Task 22 declares `#[cfg(any(test, feature = "testkit"))] pub mod testkit;`). Task 30's count then needs no feature flag.

`crates/ctl/Cargo.toml`:

```toml
[lib]
name = "lazymkv_ctl"
path = "src/lib.rs"

[[bin]]
name = "lazymkvctl"
path = "src/main.rs"

[dependencies]
lazymkv-core = { path = "../core" }
lazymkv-platform = { path = "../platform" }
# plan, decide and retune through lazymkv_dev::dispatch (design part 9.5, P23).
lazymkv-dev = { path = "../dev" }
serde_json.workspace = true

[dev-dependencies]
lazymkv-core = { path = "../core", features = ["testsupport"] }
lazymkv-platform = { path = "../platform", features = ["fakes"] }
```

`crates/capture/Cargo.toml`: add `lazymkv-tools = { path = "../tools" }` to `[dependencies]`, after `lazymkv-core`.

`crates/fake-tools/Cargo.toml`: append

```toml
[dev-dependencies]
# Pull request 2's stand-in tests (Ruling S12): this is the only crate whose tests get CARGO_BIN_EXE_fake-tool.
lazymkv-core = { path = "../core", features = ["testsupport"] }
lazymkv-platform = { path = "../platform", features = ["fakes"] }
lazymkv-tools = { path = "../tools" }
lazymkv-clients = { path = "../clients" }
lazymkv-engine = { path = "../engine", features = ["testkit"] }
lazymkv-ctl = { path = "../ctl" }
lazymkv-dev = { path = "../dev" }
```

- [ ] **Step 2: Write the crate docs and the two placeholder binaries**

Each `src/lib.rs` holds only its crate doc:
- `crates/platform/src/lib.rs`: `//! lazymkv-platform: the operating-system seams of spec 4.3 as traits, with in-memory fakes behind the `fakes` feature (design parts 4.1, 4.2). Pull request 2 holds no Windows API; the Windows implementations arrive in pull request 3.`
- `crates/tools/src/lib.rs`: `//! lazymkv-tools: the makemkvcon, mkvmerge and ffprobe drivers, their capture-style recorded runs under ProcessGuard, the tool lookup of design part 3.6, and the disc snapshot's file I/O moved from capture (design part 4.1, R-B2).`
- `crates/clients/src/lib.rs`: `//! lazymkv-clients: TheDiscDB and TMDb (The Movie Database) behind an HTTP transport trait, with a transport that replays recorded answers (design parts 4.1, 8.1). The live transport arrives in pull request 4.`
- `crates/engine/src/lib.rs`: `//! lazymkv-engine: the job-owner loop over core::job::reduce, the job store, the IPC (inter-process communication) server and the self-test (design parts 4.1, 4.4, 7.3).`
- `crates/ctl/src/lib.rs`: `//! lazymkv-ctl: lazymkvctl's commands. plan, decide and retune run locally through lazymkv_dev::dispatch; the rest go to the engine over IPC (design parts 4.1, 9.5, 9.6).`

`crates/engine/src/main.rs`:

```rust
//! lazymkv-engine's binary (design parts 3.5, 4.1). Task 21 gives it `--data DIR` through `lazymkv_engine::main_with`.
fn main() {
    eprintln!("lazymkv-engine is not built yet");
    std::process::exit(1);
}
```

`crates/ctl/src/main.rs`:

```rust
//! lazymkvctl's binary (design parts 4.1, 9.5, 9.6). Task 27 hands its arguments to `lazymkv_ctl::run`.
fn main() {
    eprintln!("lazymkvctl is not built yet");
    std::process::exit(1);
}
```

Ruling: until Tasks 21 and 27 replace them, the placeholder binaries exit 1 with a sentence. An empty `main` would exit 0, and a run of it could be mistaken for success.

- [ ] **Step 3: Build offline so that nothing can be fetched**

Run: `scripts/box.sh cargo build --workspace --offline`
Expected: `Finished` with no `Downloading` or `Updating` line. An error naming a registry or "offline mode" means a manifest pulled an external crate, which is a defect. Fix the manifest; never drop `--offline`. `box.sh` copies the updated `Cargo.lock` back to the Mac.

- [ ] **Step 4: Check the lock delta**

Run on the Mac: `git diff Cargo.lock`, `grep -c '^\[\[package\]\]' Cargo.lock` and `git diff Cargo.lock | grep -c '^+source'`.

Expected:
- The package count goes from 38 to 43, and no added line is a `source` line (count 0).
- Five blocks are added, each with `version = "0.0.0"`, no `source` and no `checksum`. Their `dependencies` lists, which cargo sorts:

  | Package | `dependencies` |
  |---|---|
  | `lazymkv-clients` | `"lazymkv-core", "serde", "serde_json"` |
  | `lazymkv-ctl` | `"lazymkv-core", "lazymkv-dev", "lazymkv-platform", "serde_json"` |
  | `lazymkv-engine` | `"lazymkv-clients", "lazymkv-core", "lazymkv-dev", "lazymkv-platform", "lazymkv-tools", "serde", "serde_json"` |
  | `lazymkv-platform` | `"lazymkv-core"` |
  | `lazymkv-tools` | `"lazymkv-core", "lazymkv-platform", "serde", "serde_json"` |

- Two existing blocks gain dependency names only:
  - `lazymkv-capture` gains `"lazymkv-tools"`.
  - `fake-tools` gains `"lazymkv-clients"`, `"lazymkv-core"`, `"lazymkv-ctl"`, `"lazymkv-dev"`, `"lazymkv-engine"`, `"lazymkv-platform"` and `"lazymkv-tools"`.
- No other line changes.

- [ ] **Step 5: Stop for the owner's yes**

Print the full `git diff Cargo.lock` for the owner and say: "Pull request 2's whole `Cargo.lock` delta: five path packages, no external crate (design part 4.7, P33)." Do not commit and do not start Task 2 until the owner answers yes in chat. Task 30 records the date of that yes.

- [ ] **Step 6: Check that every test target compiles with the locked file**

Run: `scripts/box.sh cargo test --workspace --locked --no-run`
Expected: `Finished`, with each test executable listed and no fetch. This compiles the `fakes`, `testkit` and `testsupport` feature paths through the dev-dependencies.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/platform crates/tools crates/clients crates/engine crates/ctl crates/capture/Cargo.toml crates/fake-tools/Cargo.toml
git commit -m "workspace: add the platform, tools, clients, engine and ctl crates (design part 4.1); no external crate"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, then the message alone with no attribution line.

---

### Task 2: Lower-case job IDs (K3)

**Files:**
- Modify: `crates/core/src/job.rs:1153-1163` (the `job_id` doc and body)
- Modify: `crates/core/src/job/tests.rs:2271-2316` (the job ID block), `:2320-2430` (the job lookup expectations that `earlier()` feeds), `:2409` (`a_discarded_job_starts_a_new_one`)
- Tests: 2 tests added (`a_job_id_is_a_valid_slug_and_keeps_the_fingerprints_digits`, `an_upper_case_earlier_id_does_not_count`) and 1 constant check (`fp_lower_is_fp_in_lower_case`). Seven existing tests change their expected values.

**Interfaces:**
- Consumes:
  - `fn job_number(digits: &str) -> Option<u32>` (job.rs:1167, unchanged).
  - `pub fn is_slug(text: &str) -> bool` (crates/core/src/audio/labels.rs:132).
  - `pub fn job_lookup(earlier: &[(String, State, bool)]) -> JobLookup` (job.rs:1193, unchanged: it reads only the part after the last `-`).
- Produces: `pub fn job_id(fingerprint: &str, earlier: &[&str]) -> String` with an unchanged signature (job.rs:1155). It now returns `"<fingerprint lower-cased>-<n>"`, and only exact lower-case earlier IDs count. Doc:

```rust
/// Spec 5 and K3: `<fingerprint in lower case>-<n>`, n one more than the highest n among the `earlier` IDs that
/// start with the lower-cased fingerprint and '-' (an upper-case ID, an ID of another fingerprint and a malformed ID
/// are ignored), 1 for the first. Lower case keeps the ID a valid slug (`audio::labels::is_slug`), so a job folder
/// reads as a recording (design part 7.1); the fingerprint itself stays upper case in meta.json. Def: T14 (phase 1),
/// K3 in T2. Use: T22.
```

- [ ] **Step 1: Write the failing tests**

Replace `crates/core/src/job/tests.rs:2271-2316` (the block from the comment `// Spec 5's job ID ...` through `a_job_number_may_be_as_high_as_u32_max`) with:

```rust
// Spec 5's job ID (`job_id`, lower case under K3) and spec 6.1 step 2's job lookup (`job_lookup`).

/// A made-up fingerprint in the form `fingerprint::content_hash` returns: 32 upper-case hexadecimal digits (spec 5).
const FP: &str = "0123456789ABCDEF0123456789ABCDEF";

/// FP in lower case, the form a job ID carries (K3).
const FP_LOWER: &str = "0123456789abcdef0123456789abcdef";

/// An earlier job of FP numbered `n`, its ID in lower case as `job_id` writes it.
fn earlier(n: u32, state: State, armed: bool) -> (String, State, bool) {
    (format!("{FP_LOWER}-{n}"), state, armed)
}

#[test]
fn fp_lower_is_fp_in_lower_case() {
    assert_eq!(FP_LOWER, FP.to_ascii_lowercase());
}

#[test]
fn the_first_job_of_a_fingerprint_is_numbered_1() {
    assert_eq!(job_id(FP, &[]), format!("{FP_LOWER}-1"));
}

#[test]
fn the_next_job_is_one_more_than_the_highest_earlier_number_in_any_order() {
    let ids = [format!("{FP_LOWER}-1"), format!("{FP_LOWER}-3"), format!("{FP_LOWER}-2")];
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(job_id(FP, &ids), format!("{FP_LOWER}-4"));
}

#[test]
fn the_highest_number_is_compared_as_a_number_not_as_text() {
    let ids = [format!("{FP_LOWER}-9"), format!("{FP_LOWER}-10")];
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(job_id(FP, &ids), format!("{FP_LOWER}-11"));
}

#[test]
fn ids_of_other_fingerprints_are_ignored() {
    assert_eq!(job_id("ABC", &["def-7", "abcd-9", "xabc-4"]), "abc-1");
    assert_eq!(job_id("ABC", &["def-7", "abc-2"]), "abc-3");
}

#[test]
fn malformed_ids_are_ignored() {
    let malformed = ["abc", "abc-", "abc-x", "abc-3-2", "abc3", "abc-+4", "abc- 4", "abc-4 ", "abc-4294967296"];
    assert_eq!(job_id("ABC", &malformed), "abc-1");
    assert_eq!(job_id("ABC", &["abc-3-2", "abc-5"]), "abc-6");
}

#[test]
fn a_job_number_may_be_as_high_as_u32_max() {
    assert_eq!(job_id("ABC", &["abc-4294967295"]), "abc-4294967296");
}

#[test]
fn a_job_id_is_a_valid_slug_and_keeps_the_fingerprints_digits() {
    let id = job_id("76E63EBB5B66F73B0C37BFAB59B8ABA5", &[]);
    assert_eq!(id, "76e63ebb5b66f73b0c37bfab59b8aba5-1");
    assert!(crate::audio::labels::is_slug(&id));
    assert!(!crate::audio::labels::is_slug("76E63EBB5B66F73B0C37BFAB59B8ABA5-1"), "the upper-case form K3 replaces");
}

#[test]
fn an_upper_case_earlier_id_does_not_count() {
    assert_eq!(job_id("ABC", &["ABC-4"]), "abc-1");
    assert_eq!(job_id("abc", &["abc-4"]), "abc-5", "a lower-case fingerprint is taken as it is");
}
```

Then, in `:2320-2430`, replace every `format!("{FP}-` with `format!("{FP_LOWER}-`, at :2329, :2339, :2347, :2365, :2377, :2393, :2409 (both sides), :2419, :2421, :2428 and :2429. Also replace `FP.to_string()` at :2430 with `FP_LOWER.to_string()`. Line :2409 then reads:

```rust
    assert_eq!(job_id(FP, &[format!("{FP_LOWER}-1").as_str()]), format!("{FP_LOWER}-2"));
```

The `job_id` input stays `FP`, in upper case, everywhere.

Ruling: the lower-cased fingerprint is a constant `FP_LOWER`, checked against `FP.to_ascii_lowercase()` by `fp_lower_is_fp_in_lower_case`, rather than a `let` in each test. It reads the same in every `format!`.

Ruling: the job lookup tests' expected IDs follow `earlier()` into lower case, because they compare against the IDs `earlier()` builds. The skeleton listed only :2271-2316 and :2409. `job_lookup` itself does not change.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked job::`
Expected: `test result: FAILED`, with 9 failures and left/right mismatches showing upper-case IDs, such as `left: "0123456789ABCDEF0123456789ABCDEF-1"`.
- The 9 failing tests are `the_first_job_of_a_fingerprint_is_numbered_1`, `the_next_job_is_one_more_than_the_highest_earlier_number_in_any_order`, `the_highest_number_is_compared_as_a_number_not_as_text`, `ids_of_other_fingerprints_are_ignored`, `malformed_ids_are_ignored`, `a_job_number_may_be_as_high_as_u32_max`, `a_discarded_job_starts_a_new_one`, `a_job_id_is_a_valid_slug_and_keeps_the_fingerprints_digits` and `an_upper_case_earlier_id_does_not_count`.
- `fp_lower_is_fp_in_lower_case` and every other job lookup test pass.

- [ ] **Step 3: Implement**

In `crates/core/src/job.rs:1153-1163`, replace the doc with the one under **Interfaces** and change the body:
- Its first line becomes `let fp = fingerprint.to_ascii_lowercase();`.
- The filter becomes `id.strip_prefix(fp.as_str())?.strip_prefix('-')`.
- The result becomes `format!("{fp}-{}", u64::from(highest) + 1)`.

`job_number` is unchanged.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked job::`
Expected: `test result: ok. 187 passed; 0 failed`: the 184 tests of `crates/core/src/job/tests.rs` at `70ada2b` plus the 3 new.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/job.rs crates/core/src/job/tests.rs
git commit -m "core::job: job IDs are lower case (K3)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G`, the noreply identity twice, and no attribution line.

---

### Task 3: The snapshot's pure part in core, pinned by capture's output (R-B2, part 1)

**Files:**
- Create: `crates/capture/tests/disc_snapshot.rs` (2 tests)
- Create: `crates/core/src/snapshot.rs` (4 tests in `mod tests`)
- Modify: `crates/core/src/lib.rs:17-18` (`pub mod snapshot;` between `pub mod settings;` and `pub mod subtitle;`)
- Unchanged: `crates/capture/src/disc.rs`. Capture keeps its own `list_files`, `is_nav_file`, `find_unit_key`, `snapshot` and `Meta`, and its two unit tests (:128-174), until Task 12 removes them (S9).

**Interfaces:**
- Consumes (crates/core/src/fingerprint.rs):
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct DiscFile { pub path: String, pub size: u64 }` (:9)
  - `pub enum DiscFormat { BluRay, Dvd }` (:15)
  - `pub fn disc_format(files: &[DiscFile]) -> Option<DiscFormat>` (:29)
  - `pub fn content_hash(files: &[DiscFile]) -> Option<String>` (:46)
  - `pub fn aacs_disc_id(unit_key_ro: &[u8]) -> String` (:80)
- Consumes, for the test: capture's binary `lazymkv-capture disc --root <path> --label <text> --out <dir>` (crates/capture/src/main.rs:9, :26-35). It prints `serde_json::to_string_pretty(&meta)` and a newline.
- Produces, in `crates/core/src/snapshot.rs`, with module doc `//! The pure part of a disc snapshot (R-B2, design part 4.1): which files are navigation files, which file is the AACS unit key, and meta.json's values from a listing. No I/O (RP19); tools::snapshot reads and writes the files (Task 12).`:

```rust
/// meta.json as phase 0's capture writes it (spec 15 "Recordings", R-B2), keys in this order. This is the writer's type;
/// the reader stays `recording::files::Meta`, because merging them would print `captured_unix` as a float (S9).
/// Def: T3. Use: T12, T22.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DiscMeta {
    pub volume_label: String,
    pub format: Option<String>,
    pub fingerprint: Option<String>,
    pub disc_id: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub captured_unix: u64,
}

/// True for the small unencrypted navigation files a recording keeps under `nav/`: `BDMV/PLAYLIST/*.mpls`,
/// `BDMV/index.bdmv`, `BDMV/MovieObject.bdmv`, `BDMV/META/DL/*.xml` and `VIDEO_TS/*.IFO`, ignoring ASCII case
/// (R-B2, from capture's disc.rs:57). Def: T3. Use: T12.
pub fn is_nav_file(relative_path: &str) -> bool;

/// The AACS unit key file of a listing: `AACS/Unit_Key_RO.inf`, else `AACS/DUPLICATE/Unit_Key_RO.inf`, matched
/// ignoring ASCII case (R-B2, from capture's disc.rs:66). Def: T3. Use: T12, T22.
pub fn unit_key_file(files: &[DiscFile]) -> Option<&DiscFile>;

/// meta.json's values for a listing (spec 5, R-B2):
/// - `format` is "bluray" or "dvd" from `disc_format`;
/// - `fingerprint` is `content_hash`;
/// - `disc_id` is `aacs_disc_id(unit_key)` when the caller read the unit key, else None;
/// - `file_count` and `total_bytes` count `files`;
/// - `captured_unix` is passed in, because core reads no clock (RP19).
/// Def: T3. Use: T12, T22.
pub fn disc_meta(files: &[DiscFile], volume_label: &str, unit_key: Option<&[u8]>, captured_unix: u64) -> DiscMeta;
```

- [ ] **Step 1: Write the characterisation test of capture's output**

Create `crates/capture/tests/disc_snapshot.rs`.

Ruling: expected texts are arrays of lines joined with `"\n"`, never multi-line literals. The repository has no `.gitattributes`, so a Windows checkout with `core.autocrlf` could otherwise put `\r\n` into the source.

```rust
//! Capture's `disc` output, pinned byte for byte before its snapshot moves into core and tools, and checked against
//! core's pure part (R-B2; design part 4.1, "unchanged output"). The disc is a made-up Blu-ray folder written here;
//! nothing is read from corpus/.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// The made-up disc: each file's path under the root and its bytes.
fn disc_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("BDMV/index.bdmv", b"index".to_vec()),
        ("BDMV/MovieObject.bdmv", b"movieobject".to_vec()),
        ("BDMV/PLAYLIST/00800.mpls", b"mpls".to_vec()),
        ("BDMV/META/DL/bdmt_eng.xml", b"<di:name>Synthetic</di:name>".to_vec()),
        ("BDMV/CLIPINF/00000.clpi", b"clpi".to_vec()),
        ("BDMV/STREAM/00000.m2ts", vec![0u8; 100]),
        ("BDMV/STREAM/00001.m2ts", vec![0u8; 300]),
        ("AACS/Unit_Key_RO.inf", b"abc".to_vec()),
    ]
}

/// files.json for `disc_files`: sorted by path as bytes, two-space indent, no final newline.
const FILES_JSON: &[&str] = &[
    "[",
    "  {", "    \"path\": \"AACS/Unit_Key_RO.inf\",", "    \"size\": 3", "  },",
    "  {", "    \"path\": \"BDMV/CLIPINF/00000.clpi\",", "    \"size\": 4", "  },",
    "  {", "    \"path\": \"BDMV/META/DL/bdmt_eng.xml\",", "    \"size\": 28", "  },",
    "  {", "    \"path\": \"BDMV/MovieObject.bdmv\",", "    \"size\": 11", "  },",
    "  {", "    \"path\": \"BDMV/PLAYLIST/00800.mpls\",", "    \"size\": 4", "  },",
    "  {", "    \"path\": \"BDMV/STREAM/00000.m2ts\",", "    \"size\": 100", "  },",
    "  {", "    \"path\": \"BDMV/STREAM/00001.m2ts\",", "    \"size\": 300", "  },",
    "  {", "    \"path\": \"BDMV/index.bdmv\",", "    \"size\": 5", "  }",
    "]",
];

/// meta.json for `disc_files` with `captured_unix` masked to 0. The fingerprint is the MD5 of the sizes 100 and 300 as
/// little-endian 64-bit integers; the disc ID is the SHA-1 of "abc".
const META_JSON: &[&str] = &[
    "{",
    "  \"volume_label\": \"SYNTHETIC\",",
    "  \"format\": \"bluray\",",
    "  \"fingerprint\": \"881150E67DD260B285FD74856BFEEBE2\",",
    "  \"disc_id\": \"A9993E364706816ABA3E25717850C26C9CD0D89D\",",
    "  \"file_count\": 8,",
    "  \"total_bytes\": 455,",
    "  \"captured_unix\": 0",
    "}",
];

/// The navigation files copied under `nav/`, sorted.
const NAV_FILES: [&str; 4] =
    ["BDMV/META/DL/bdmt_eng.xml", "BDMV/MovieObject.bdmv", "BDMV/PLAYLIST/00800.mpls", "BDMV/index.bdmv"];

/// A fresh, empty folder under the system's temporary directory.
fn scratch(tag: &str) -> PathBuf {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lazymkv-capture-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The made-up disc written under a fresh root.
fn disc_root() -> PathBuf {
    let root = scratch("disc");
    for (path, bytes) in disc_files() {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    root
}

/// `lazymkv-capture disc --root <root> --label SYNTHETIC --out <out>`.
fn capture_disc(root: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lazymkv-capture"))
        .arg("disc")
        .arg("--root")
        .arg(root)
        .args(["--label", "SYNTHETIC", "--out"])
        .arg(out)
        .output()
        .unwrap()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn text(path: &Path) -> String {
    String::from_utf8(fs::read(path).unwrap()).unwrap()
}

/// `text` with the digits after `"captured_unix": ` replaced by 0, and the value they held.
fn mask_captured_unix(text: &str) -> (String, u64) {
    let key = "\"captured_unix\": ";
    let start = text.find(key).expect("captured_unix is written") + key.len();
    let digits = text[start..].bytes().take_while(u8::is_ascii_digit).count();
    let value = text[start..start + digits].parse().unwrap();
    (format!("{}0{}", &text[..start], &text[start + digits..]), value)
}

/// Every file under `dir`, as `/`-joined paths relative to it, sorted.
fn relative_files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let parts: Vec<String> = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.push(parts.join("/"));
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

#[test]
fn snapshot_output_is_pinned_byte_for_byte() {
    let root = disc_root();
    let out = scratch("out").join("snapshot"); // not yet created: capture makes it
    let before = unix_now();
    let output = capture_disc(&root, &out);
    let after = unix_now();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(text(&out.join("files.json")), FILES_JSON.join("\n"));
    let (meta, captured) = mask_captured_unix(&text(&out.join("meta.json")));
    assert_eq!(meta, META_JSON.join("\n"));
    assert!((before..=after).contains(&captured), "captured_unix {captured} not in {before}..={after}");
    let (stdout, printed) = mask_captured_unix(&String::from_utf8(output.stdout).unwrap());
    assert_eq!(stdout, META_JSON.join("\n") + "\n");
    assert_eq!(printed, captured);

    let mut top: Vec<String> =
        fs::read_dir(&out).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    top.sort();
    assert_eq!(top, ["files.json", "meta.json", "nav"]);
    assert_eq!(relative_files(&out.join("nav")), NAV_FILES);
    for path in NAV_FILES {
        assert_eq!(fs::read(out.join("nav").join(path)).unwrap(), fs::read(root.join(path)).unwrap(), "{path}");
    }
}
```

- [ ] **Step 2: Run the characterisation test on the code as it stands; it must pass**

Run: `scripts/box.sh cargo test -p lazymkv-capture --locked --test disc_snapshot`
Expected: `test snapshot_output_is_pinned_byte_for_byte ... ok` and `test result: ok. 1 passed`. A failure here means the literal is wrong, not the code: correct the literal from capture's actual output, and change no capture code.

- [ ] **Step 3: Write the failing tests of the pure part**

Append to `crates/capture/tests/disc_snapshot.rs`, adding `use lazymkv_core::fingerprint::DiscFile;` to its imports:

```rust
#[test]
fn core_snapshot_functions_agree_with_captures_output() {
    use lazymkv_core::snapshot::{disc_meta, is_nav_file, unit_key_file};
    let root = disc_root();
    let out = scratch("out").join("snapshot");
    assert_eq!(capture_disc(&root, &out).status.code(), Some(0));

    let mut files: Vec<DiscFile> = disc_files()
        .into_iter()
        .map(|(path, bytes)| DiscFile { path: path.to_string(), size: bytes.len() as u64 })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let written = fs::read(out.join("meta.json")).unwrap();
    let (_, captured) = mask_captured_unix(&String::from_utf8(written.clone()).unwrap());

    let key = unit_key_file(&files).expect("the disc has a unit key");
    assert_eq!(key.path, "AACS/Unit_Key_RO.inf");
    let key_bytes = fs::read(root.join(&key.path)).unwrap();
    let meta = disc_meta(&files, "SYNTHETIC", Some(key_bytes.as_slice()), captured);
    assert_eq!(serde_json::to_vec_pretty(&meta).unwrap(), written);

    let nav: Vec<&str> = files.iter().filter(|file| is_nav_file(&file.path)).map(|file| file.path.as_str()).collect();
    assert_eq!(nav, relative_files(&out.join("nav")));
}
```

In `crates/core/src/lib.rs`, add `pub mod snapshot;` after `pub mod settings;`. Create `crates/core/src/snapshot.rs` with the module doc from **Interfaces** and this `mod tests` only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::DiscFile;

    fn file(path: &str, size: u64) -> DiscFile {
        DiscFile { path: path.into(), size }
    }

    #[test]
    fn disc_meta_gives_phase_0s_seven_keys_in_order() {
        let files = [file("AACS/Unit_Key_RO.inf", 3), file("BDMV/STREAM/00000.m2ts", 100), file("BDMV/STREAM/00001.m2ts", 300)];
        let meta = disc_meta(&files, "MY_DISC", Some(&b"abc"[..]), 7);
        assert_eq!(
            serde_json::to_string(&meta).unwrap(),
            concat!(
                r#"{"volume_label":"MY_DISC","format":"bluray","fingerprint":"881150E67DD260B285FD74856BFEEBE2","#,
                r#""disc_id":"A9993E364706816ABA3E25717850C26C9CD0D89D","file_count":3,"total_bytes":403,"captured_unix":7}"#
            )
        );
    }

    #[test]
    fn a_dvd_has_format_dvd_and_no_disc_id_without_a_unit_key() {
        let files = [file("VIDEO_TS/VIDEO_TS.IFO", 10), file("VIDEO_TS/VTS_01_1.VOB", 2000)];
        assert_eq!(
            disc_meta(&files, "DVD_DISC", None, 0),
            DiscMeta {
                volume_label: "DVD_DISC".into(),
                format: Some("dvd".into()),
                fingerprint: Some("106D7957633694DC87F528C84A4C73FA".into()),
                disc_id: None,
                file_count: 2,
                total_bytes: 2010,
                captured_unix: 0,
            }
        );
        let empty = disc_meta(&[], "", None, 0);
        assert_eq!((empty.format, empty.fingerprint, empty.file_count, empty.total_bytes), (None, None, 0, 0));
    }

    #[test]
    fn the_unit_key_is_found_in_either_aacs_folder() {
        let main = file("AACS/Unit_Key_RO.inf", 3);
        let duplicate = file("AACS/DUPLICATE/Unit_Key_RO.inf", 4);
        let found = |files: &[DiscFile]| unit_key_file(files).map(|file| file.path.clone());
        assert_eq!(found(&[duplicate.clone(), main.clone()]), Some(main.path.clone()), "the main folder wins");
        assert_eq!(found(&[duplicate.clone()]), Some(duplicate.path.clone()));
        assert_eq!(found(&[file("aacs/unit_key_ro.inf", 3)]), Some("aacs/unit_key_ro.inf".to_string()));
        assert_eq!(found(&[file("AACS/Unit_Key_RW.inf", 3)]), None);
        assert_eq!(found(&[]), None);
    }

    #[test]
    fn navigation_files_are_recognised() {
        assert!(is_nav_file("BDMV/PLAYLIST/00800.mpls"));
        assert!(is_nav_file("BDMV/index.bdmv"));
        assert!(is_nav_file("BDMV/MovieObject.bdmv"));
        assert!(is_nav_file("BDMV/META/DL/bdmt_eng.xml"));
        assert!(is_nav_file("VIDEO_TS/VTS_01_0.IFO"));
        assert!(!is_nav_file("BDMV/STREAM/00001.m2ts"));
        assert!(!is_nav_file("VIDEO_TS/VTS_01_1.VOB"));
        assert!(!is_nav_file("AACS/Unit_Key_RO.inf"));
    }
}
```

Ruling: `core_snapshot_functions_agree_with_captures_output` pins core's three functions against capture's own code while both exist. Task 12 can then switch capture over with the equality already shown. The test is added to the skeleton's list, as is `a_dvd_has_format_dvd_and_no_disc_id_without_a_unit_key`, which covers the `"dvd"` spelling and the absent unit key that the Blu-ray tests do not reach.

- [ ] **Step 4: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked snapshot::`
Expected: compile error, `error[E0425]: cannot find function `disc_meta` in this scope` (also `unit_key_file` and `is_nav_file`) and `cannot find struct `DiscMeta``.

Run: `scripts/box.sh cargo test -p lazymkv-capture --locked --test disc_snapshot`
Expected: compile error in `crates/capture/tests/disc_snapshot.rs`, `error[E0432]: unresolved imports `lazymkv_core::snapshot::disc_meta`, `lazymkv_core::snapshot::is_nav_file`, `lazymkv_core::snapshot::unit_key_file``. Core's library itself compiles, because `snapshot.rs` holds only its `cfg(test)` module so far.

- [ ] **Step 5: Implement `crates/core/src/snapshot.rs`**

Above `mod tests`, add `use crate::fingerprint::{aacs_disc_id, content_hash, disc_format, DiscFile, DiscFormat};` and the four items with the signatures and docs under **Interfaces**:
- `is_nav_file` has the body of crates/capture/src/disc.rs:57-64.
- `unit_key_file` has the body of `find_unit_key` (disc.rs:66-70).
- `disc_meta` builds `DiscMeta` as disc.rs:95-104 builds `Meta`, with `disc_id: unit_key.map(aacs_disc_id)` and `captured_unix` taken from the argument.

Leave crates/capture/src unchanged.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked snapshot::`
Expected: `test result: ok. 4 passed`.

Run: `scripts/box.sh cargo test -p lazymkv-capture --locked`
Expected: every test passes. That is `disc_snapshot`'s 2 plus capture's existing 8 on the mini PC, 10 in all, and `snapshot_output_is_pinned_byte_for_byte` passes with its literals unchanged.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/lib.rs crates/core/src/snapshot.rs crates/capture/tests/disc_snapshot.rs
git commit -m "core::snapshot: the disc snapshot's pure part (R-B2); capture's output pinned"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G`, the noreply identity twice, and no attribution line.

### Task 4: The ustar reader and the profile constants

**Files:**
- Create: `crates/core/src/ustar.rs` (7 tests in its `mod tests`)
- Modify: `crates/core/src/lib.rs` (add `pub mod ustar;`)
- Modify: `crates/core/src/profile.rs:5` (three constants after `ALL_TRACKS_SELECTION`), tests `:64-115` (the `DEFAULT` const at `:68-82` moves out; 1 new test)
- Modify: `crates/core/src/testsupport/mod.rs` (two helpers after `search_json`, `:582-594`, before `mod builder_tests` at `:596`)

**Interfaces:**
- Consumes: `profile::make_job_profile(default_profile_xml: &str, name: &str, selection: &str) -> Result<String, ProfileError>` (profile.rs:22); `profile::ALL_TRACKS_SELECTION` (profile.rs:5).
- Produces:

```rust
// crates/core/src/ustar.rs
/// MakeMKV's default profile inside its appdata.tar (design part 5.4). Def: T4. Use: T23.
pub const DEFAULT_PROFILE_NAME: &str = "default.mmcp.xml";
/// The data of the first regular-file entry whose last path part equals `name`; Ok(None) when no entry has it; Err
/// for an archive cut short or a size field that is not octal. No I/O (RP19). Design part 5.4. Def: T4. Use: T23.
pub fn ustar_entry<'a>(archive: &'a [u8], name: &str) -> Result<Option<&'a [u8]>, String>;

// crates/core/src/profile.rs
/// The 1009 fallback profile's selection: every stream but 3D video (design part 5.4, K6, L4). Def: T4. Use: T23.
pub const FALLBACK_SELECTION: &str = "+sel:all,-sel:mvcvideo";
/// The job profile's file name in the job folder (design part 7.1). Def: T4. Use: T13, T23.
pub const JOB_PROFILE_FILE: &str = "job.mmcp.xml";
/// The fallback profile's file name in the job folder (design part 7.1, K6). Def: T4. Use: T13, T23.
pub const FALLBACK_PROFILE_FILE: &str = "fallback.mmcp.xml";

// crates/core/src/testsupport/mod.rs
/// A ustar archive of `entries` (name, data), each a regular file, ended by two zero blocks: the fixture appdata.tar
/// of design part 5.4. Panics on a name over 100 bytes. Def: T4. Use: T4, T23.
pub fn ustar_build(entries: &[(&str, &[u8])]) -> Vec<u8>;
/// A hand-made profile in the shape of MakeMKV's default profile: one <name> and two defaultSelection attributes
/// (design part 5.4; the text is ours, D17). Def: T4. Use: T4, T23.
pub fn default_profile_xml() -> String;
```

- [ ] **Step 1: Write the test helpers in `testsupport/mod.rs`**

`default_profile_xml()` returns exactly the text of the `DEFAULT` const now at `crates/core/src/profile.rs:68-82`, keeping its comment "Same shape as MakeMKV's default profile; the text is ours." Remove the const from `profile.rs`'s tests. Change its two uses, `:86` and `:100`, to `&default_profile_xml()` with `use crate::testsupport::default_profile_xml;`.
Ruling: one copy of the hand-made profile, used by both profile's tests and the fixture `appdata.tar`.

`ustar_build` writes one 512-byte header per entry, then the data zero-padded to a multiple of 512, then 1024 zero bytes. The header (POSIX ustar) holds:

| Offset | Bytes | Value |
|---|---|---|
| 0 | 100 | the name, NUL-padded |
| 100 | 8 | `0000644\0` |
| 108, 116 | 8 each | `0000000\0` |
| 124 | 12 | the size as 11 octal digits, then `\0` (`format!("{:011o}\0", len)`) |
| 136 | 12 | `00000000000\0` |
| 148 | 8 | the checksum: the sum of all 512 header bytes with this field as 8 spaces, written `format!("{:06o}\0 ", sum)` |
| 156 | 1 | `0` |
| 257 | 6 | `ustar\0` |
| 263 | 2 | `00` |

All other bytes are zero.

- [ ] **Step 2: Write the failing tests**

In `crates/core/src/ustar.rs`, with `pub mod ustar;` added to `lib.rs`:

```rust
//! A reader of the ustar archive MakeMKV installs as appdata.tar, for its default.mmcp.xml (design part 5.4). Takes
//! bytes, never a path (RP19).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::ustar_build;

    #[test]
    fn the_default_profile_is_found_by_its_last_path_part() {
        let archive = ustar_build(&[("x/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(Some(&b"<a/>"[..])));
    }

    #[test]
    fn a_missing_entry_is_none() {
        let archive = ustar_build(&[("x/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, "other.xml"), Ok(None));
    }

    #[test]
    fn a_name_that_only_ends_with_the_wanted_name_does_not_match() {
        let archive = ustar_build(&[("x/my-default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(None));
    }

    #[test]
    fn the_first_of_two_matching_entries_is_returned() {
        let archive = ustar_build(&[("a/default.mmcp.xml", &b"1"[..]), ("b/default.mmcp.xml", &b"2"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(Some(&b"1"[..])));
    }

    #[test]
    fn a_truncated_archive_is_refused() {
        // An empty entry's header at 0, the profile's header at 512, its data at 1024.
        let archive = ustar_build(&[("x/empty.txt", &b""[..]), ("x/default.mmcp.xml", &b"<a/>"[..])]);
        let refused = Err("ustar: truncated entry at offset 512".to_string());
        assert_eq!(ustar_entry(&archive[..600], DEFAULT_PROFILE_NAME), refused);
        assert_eq!(ustar_entry(&archive[..1026], DEFAULT_PROFILE_NAME), refused);
    }

    #[test]
    fn a_bad_size_field_is_refused() {
        let mut archive = ustar_build(&[("default.mmcp.xml", &b"<a/>"[..])]);
        archive[124..136].copy_from_slice(b"zzzzzzzzzzz\0");
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Err("ustar: bad size at offset 0".to_string()));
    }

    #[test]
    fn ustar_build_writes_one_header_one_data_block_and_two_zero_blocks() {
        let archive = ustar_build(&[("x/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(archive.len(), 4 * 512);
        assert_eq!(&archive[0..18], b"x/default.mmcp.xml");
        assert_eq!(&archive[124..136], b"00000000004\0");
        assert_eq!(&archive[156..157], b"0");
        assert_eq!(&archive[257..263], b"ustar\0");
        assert_eq!(&archive[512..516], b"<a/>");
        assert!(archive[1024..].iter().all(|byte| *byte == 0));
    }
}
```

In `crates/core/src/profile.rs`'s `mod tests`:

```rust
    #[test]
    fn the_fallback_profile_selects_every_stream_but_mvc_video() {
        let out = make_job_profile(&default_profile_xml(), "j", FALLBACK_SELECTION).unwrap();
        assert_eq!(out.matches("defaultSelection=\"+sel:all,-sel:mvcvideo\"").count(), 2);
        assert_eq!(FALLBACK_SELECTION, "+sel:all,-sel:mvcvideo");
        assert_eq!((JOB_PROFILE_FILE, FALLBACK_PROFILE_FILE), ("job.mmcp.xml", "fallback.mmcp.xml"));
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked ustar::`
Expected: compile error, ``cannot find function `ustar_entry` `` and ``cannot find value `DEFAULT_PROFILE_NAME` ``. The profile test adds ``cannot find value `FALLBACK_SELECTION` ``.

- [ ] **Step 4: Implement `ustar_entry` and the three constants**

Add the constants to `profile.rs` after `:5`, and `DEFAULT_PROFILE_NAME` and `ustar_entry` to `ustar.rs`, with the signatures above.

`ustar_entry` walks the headers from offset 0:
1. At `offset >= archive.len()`, return `Ok(None)`. A 512-byte block of zeros also returns `Ok(None)`.
2. If `offset + 512 > len`, return `Err(format!("ustar: truncated entry at offset {offset}"))`.
3. Read the size from bytes 124..136. Leading spaces are skipped. Then come one or more ASCII octal digits up to the first NUL or space, or the end of the field. Anything else gives `Err(format!("ustar: bad size at offset {offset}"))`.
4. If `offset + 512 + size > len`, return the truncated error at the header's `offset`.
5. The entry matches when the typeflag (byte 156) is `b'0'` or `0`, and when the last `/`-separated part of the name is byte-equal to `name`. The name is bytes 0..100, cut at the first NUL. On a match, return `Ok(Some(&archive[offset + 512..offset + 512 + size]))`.
6. Otherwise, `offset += 512 + size` rounded up to 512.

Ruling: the prefix field (offset 345) is not read, because the last path part always lies in the name field, so a match never needs the join.
Ruling: the checksum is not checked, because no test or step depends on it and MakeMKV's own file is the only input.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked ustar::`
Expected: `test result: ok. 7 passed; 0 failed`.

Run: `scripts/box.sh cargo test -p lazymkv-core --locked profile::`
Expected: `test result: ok. 4 passed; 0 failed`. That is the three existing tests unchanged, plus the fallback test.

- [ ] **Step 6: Commit**

```
git add crates/core/src/ustar.rs crates/core/src/lib.rs crates/core/src/profile.rs crates/core/src/testsupport/mod.rs
git commit -m "core::ustar: read default.mmcp.xml from appdata.tar (design part 5.4); the fallback selection (L4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, and no attribution line.

---

### Task 5: The `makemkvcon` line reducer (PIPE-10)

**Files:**
- Create: `crates/core/src/robot/lines.rs` (19 tests in its `mod tests`)
- Modify: `crates/core/src/robot.rs:7` (add `pub mod lines;` after the `use` lines), `:123` (`fn message` becomes `pub(crate) fn message`)

**Interfaces:**
- Consumes:
  - `robot::Message { code: u32, text: String, params: Vec<String> }` (robot.rs:14);
  - `robot::Drive = Vec<String>` (robot.rs:21);
  - `robot::Scan` (robot.rs:25);
  - `robot::split_fields(text: &str) -> Vec<String>` (robot.rs:41);
  - `fn message(scan: &mut Scan, fields: &[String]) -> Result<(), String>` (robot.rs:123, now `pub(crate)`). It pushes one `Message` onto `scan.messages`.
  - `disc::TitleSource { File { name: String }, DvdTitle { number: u32 }, Unknown }` (disc.rs:115).
- Produces (`crates/core/src/robot/lines.rs`):

```rust
/// Which makemkvcon call the lines come from (design part 5.3, PIPE-10). Def: T5. Use: T13.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call { DriveList, Scan, Rip { index: u32, source: crate::disc::TitleSource } }

/// What one line tells the driver at once (design part 5.3). Def: T5. Use: T13.
#[derive(Clone, Debug, PartialEq)]
pub enum LineEvent { Progress { fraction: f64 }, IndexMismatch, Licence { code: u32, text: String }, ProfileRejected { text: String } }

/// What a whole run said (design part 5.3, spec 9.4). Def: T5. Use: T13, T23.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunSummary { pub version: Option<String>, pub java: bool, pub profile_rejected: Option<String>,
    pub licence: Option<super::Message>, pub index_guard: Option<bool>, pub read_errors: bool, pub write_error: bool,
    pub removed: Vec<super::Message>, pub title_saved: bool, pub title_failed: bool, pub scan_complete: bool,
    pub drives: Vec<super::Drive> }

/// Spec 9.4's licence and key messages. Def: T5. Use: T13.
pub const LICENCE_CODES: [u32; 5] = [5021, 5050, 5052, 5055, 5095];

/// The per-line reader (PIPE-10); holds a scratch `Scan` so `robot::message` builds each Message. Def: T5. Use: T13.
pub struct LineReader { /* private */ }
impl LineReader {
    pub fn new(call: Call) -> LineReader;
    /// One line, with or without its "\n" or "\r\n" and a leading byte order mark.
    pub fn feed(&mut self, line: &str) -> Vec<LineEvent>;
    pub fn finish(self) -> RunSummary;
}
```

- [ ] **Step 1: Write the failing tests**

In `crates/core/src/robot/lines.rs`, with `pub mod lines;` added to `robot.rs`:

```rust
//! The per-line reader of makemkvcon output (PIPE-10, design part 5.3, spec 9.4). The whole-text `robot::parse` still
//! reads the finished scan for the planner. No I/O (RP19).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disc::TitleSource;
    use crate::robot::split_fields;

    /// The public Pan's Labyrinth UHD scan of `corpus/scenarios/disc-a-scan` (CRLF line ends).
    const DISC_A: &[u8] = include_bytes!("../../../../corpus/scenarios/disc-a-scan/scan.txt");

    /// A MSG line as makemkvcon writes it: code, flags 0, the parameter count, the text, the text again as the format,
    /// then each parameter quoted.
    fn msg(code: u32, text: &str, params: &[&str]) -> String {
        let quoted: String = params.iter().map(|p| format!(",\"{p}\"")).collect();
        format!("MSG:{code},0,{},\"{text}\",\"{text}\"{quoted}", params.len())
    }

    fn read<S: AsRef<str>>(call: Call, lines: &[S]) -> (Vec<LineEvent>, RunSummary) {
        let mut reader = LineReader::new(call);
        let mut events = Vec::new();
        for line in lines {
            events.extend(reader.feed(line.as_ref()));
        }
        (events, reader.finish())
    }

    fn rip_file(index: u32) -> Call {
        Call::Rip { index, source: TitleSource::File { name: "00800.mpls".to_string() } }
    }

    fn added(code: u32, file: &str, n: &str) -> String {
        msg(code, "File %1 was added as title #%2", &[file, n])
    }

    fn saving() -> String {
        msg(5015, "Saving all titles to MKV files", &[])
    }

    #[test]
    fn a_scan_is_complete_only_with_5011_followed_by_a_tcount_line() {
        let text = String::from_utf8_lossy(DISC_A);
        let lines: Vec<&str> = text.lines().collect();
        let (_, summary) = read(Call::Scan, &lines);
        assert!(summary.scan_complete);
        assert_eq!(summary.version.as_deref(), Some("2.0.0"));
        assert_eq!(summary.drives.len(), 16);
        assert_eq!(summary.drives[0][6], "D:");
        let without: Vec<&str> = lines.iter().copied().filter(|line| !line.starts_with("TCOUNT")).collect();
        assert!(!read(Call::Scan, &without).1.scan_complete);
        let reversed = ["TCOUNT:1".to_string(), msg(5011, "Operation successfully completed", &[])];
        assert!(!read(Call::Scan, &reversed).1.scan_complete);
    }

    #[test]
    fn the_version_comes_from_msg_1005() {
        let line = msg(1005, "MakeMKV v1.17.0 win(x64-release) started", &["MakeMKV v1.17.0 win(x64-release)"]);
        assert_eq!(read(Call::DriveList, &[line]).1.version.as_deref(), Some("1.17.0"));
        assert_eq!(read(Call::DriveList, &[msg(1005, "started", &["something else"])]).1.version, None);
    }

    #[test]
    fn java_in_use_is_msg_3344() {
        let line = msg(3344, "Using Java runtime from %1", &["/opt/java/bin/java"]);
        assert!(read(Call::Scan, &[line]).1.java);
        assert!(!read(Call::Scan, &[msg(1005, "x", &[])]).1.java);
    }

    #[test]
    fn msg_1009_is_a_rejected_profile() {
        let (events, summary) = read(Call::Scan, &[msg(1009, "Profile parsing error: bad selection", &[])]);
        assert_eq!(events, vec![LineEvent::ProfileRejected { text: "Profile parsing error: bad selection".to_string() }]);
        assert_eq!(summary.profile_rejected.as_deref(), Some("Profile parsing error: bad selection"));
    }

    #[test]
    fn each_licence_code_is_reported() {
        assert_eq!(LICENCE_CODES, [5021, 5050, 5052, 5055, 5095]);
        for code in LICENCE_CODES {
            let (events, summary) = read(Call::Scan, &[msg(code, "key problem", &[])]);
            assert_eq!(events, vec![LineEvent::Licence { code, text: "key problem".to_string() }]);
            assert_eq!(summary.licence.map(|message| message.code), Some(code));
        }
    }

    #[test]
    fn the_index_guard_passes_when_the_planned_file_keeps_its_index() {
        let (events, summary) = read(rip_file(0), &[added(3307, "00800.mpls", "0"), saving()]);
        assert!(events.is_empty());
        assert_eq!(summary.index_guard, Some(true));
        assert_eq!(read(Call::Scan, &[added(3307, "00800.mpls", "0"), saving()]).1.index_guard, None);
    }

    #[test]
    fn the_index_guard_fails_when_3307_moves_the_planned_file() {
        let (events, summary) = read(rip_file(0), &[added(3307, "00800.mpls", "1"), saving()]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn the_angle_form_3308_counts_as_an_added_title() {
        assert_eq!(read(rip_file(0), &[added(3308, "00800.mpls", "0"), saving()]).1.index_guard, Some(true));
        let (events, summary) = read(rip_file(0), &[added(3308, "00800.mpls", "1"), saving()]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn the_dvd_guard_compares_the_3028_title_number_as_a_number() {
        let call = Call::Rip { index: 1, source: TitleSource::DvdTitle { number: 3 } };
        let dvd = |second: &str| {
            vec![
                msg(3028, "Title #%1 was added (%2 cell(s), %3)", &["1", "19", "2:01:14"]),
                msg(3028, "Title #%1 was added (%2 cell(s), %3)", &[second, "25", "1:26:26"]),
                saving(),
            ]
        };
        let (events, summary) = read(call.clone(), &dvd("3"));
        assert!(events.is_empty());
        assert_eq!(summary.index_guard, Some(true));
        assert_eq!(read(call.clone(), &dvd("03")).1.index_guard, Some(true));
        let (events, summary) = read(call, &dvd("4"));
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn a_missing_3307_before_the_saving_message_is_a_mismatch() {
        let (events, summary) = read(rip_file(0), &[saving(), added(3307, "00800.mpls", "0")]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn the_guard_verdict_is_fixed_at_the_first_saving_message_5015_or_5014() {
        let plain = msg(5014, "Saving %1 titles into directory %2", &["1", "x"]);
        let (events, summary) = read(rip_file(0), &[added(3307, "00800.mpls", "0"), plain.clone(), saving()]);
        assert!(events.is_empty());
        assert_eq!(summary.index_guard, Some(true));
        let (events, summary) = read(rip_file(0), &[plain, added(3307, "00800.mpls", "0"), saving()]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn a_5036_with_one_title_is_a_saved_title() {
        let saved = |code: u32, n: &str| read(rip_file(0), &[msg(code, "Copy complete. %1 titles saved.", &[n])]).1.title_saved;
        assert!(saved(5036, "1"));
        assert!(saved(5005, "1"));
        assert!(!saved(5036, "2"));
        assert!(!saved(5005, "0"));
    }

    #[test]
    fn msg_5003_or_5004_with_5037_is_a_failed_title() {
        let failed = |codes: &[u32]| {
            let lines: Vec<String> = codes.iter().map(|code| msg(*code, "failed", &["1"])).collect();
            read(rip_file(0), &lines).1.title_failed
        };
        assert!(failed(&[5003]));
        assert!(failed(&[5004, 5037]));
        assert!(!failed(&[5036]));
    }

    #[test]
    fn msgs_2003_and_2019_set_read_and_write_errors() {
        let read_error = msg(2003, "Error '%1' occurred while reading '%2' at offset '%3'", &["Scsi error", "BD-RE", "1048576"]);
        let summary = read(rip_file(0), &[read_error]).1;
        assert!(summary.read_errors && !summary.write_error);
        let summary = read(rip_file(0), &[msg(2019, "Error writing", &[])]).1;
        assert!(!summary.read_errors && summary.write_error);
    }

    #[test]
    fn msg_4047_is_kept_as_a_removal() {
        let line = msg(
            4047,
            "AV sync issue in stream %1 at %2 : %3",
            &["2", "0:00:00", "Forced subtitles track #2 turned out to be empty and was removed from output file"],
        );
        let summary = read(rip_file(0), &[line]).1;
        assert_eq!(summary.removed.len(), 1);
        assert_eq!(summary.removed[0].params[0], "2");
    }

    #[test]
    fn prgv_gives_total_over_max() {
        let (events, _) = read(rip_file(0), &["PRGV:10,32768,65536", "PRGV:0,0,0", "PRGV:x,1,2"]);
        assert_eq!(events, vec![LineEvent::Progress { fraction: 0.5 }]);
    }

    #[test]
    fn msg_5010_on_the_drive_listing_is_ignored() {
        let drv = r#"DRV:0,2,999,12,"BD-RE SYNTHETIC DRIVE 1.00 SERIAL","SYNTHETIC_MOVIE","tok0""#;
        let (events, summary) = read(Call::DriveList, &[drv.to_string(), msg(5010, "Failed to open disc", &[])]);
        assert!(events.is_empty());
        let fields = ["0", "2", "999", "12", "BD-RE SYNTHETIC DRIVE 1.00 SERIAL", "SYNTHETIC_MOVIE", "tok0"];
        assert_eq!(summary, RunSummary { drives: vec![fields.map(String::from).to_vec()], ..RunSummary::default() });
        assert_eq!(split_fields(&drv[4..]), summary.drives[0]);
    }

    #[test]
    fn a_line_with_its_newline_and_a_byte_order_mark_is_read() {
        let line = format!("\u{feff}{}\r\n", msg(1005, "x", &["MakeMKV v2.0.0 win(x64-release)"]));
        assert_eq!(read(Call::Scan, &[line]).1.version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn a_malformed_line_is_skipped() {
        let (events, summary) = read(Call::Scan, &["MSG:x,0,0", "no colon", "PRGV:1"]);
        assert!(events.is_empty());
        assert_eq!(summary, RunSummary::default());
    }
}
```

Ruling: the skeleton's names `5003_or_5004_with_5037_is_a_failed_title` and `2003_and_2019_set_read_and_write_errors` gain a `msg_`/`msgs_` prefix, because a Rust test name cannot start with a digit.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked robot::lines`
Expected: compile error, ``cannot find type `LineReader` `` and ``cannot find type `Call` ``.

- [ ] **Step 3: Implement the reducer in `robot/lines.rs`**

In `robot.rs:123`, change `fn message` to `pub(crate) fn message`. Add the items of the Interfaces block. `LineReader` privately holds:
- the `Call`;
- a scratch `robot::Scan`;
- the `RunSummary` being built;
- `seen_5011: bool`;
- the guard's recorded index `Option<u32>` (Blu-ray) or 3028 count and N (DVD);
- `verdict_fixed: bool`.

`feed` strips a trailing `"\n"`, then `"\r"`, then a leading `'\u{feff}'`. It splits at the first `':'`, and a line with no `':'` is skipped. The kinds below are read; any other kind is skipped.

| Kind | Handling |
|---|---|
| `DRV` | `summary.drives.push(split_fields(rest))` |
| `TCOUNT` | `scan_complete = seen_5011` |
| `PRGV` | Fields `[current, total, max]` as u64. Emit `Progress { fraction: total as f64 / max as f64 }` only when all three parse and `max > 0`. |
| `MSG` | Call `message(&mut self.scratch, &split_fields(rest))`. An Err skips the line. Otherwise take the message with `self.scratch.messages.pop()` and handle it by its code, as in the next table. |

| Code | Effect |
|---|---|
| 1005 | `version`, set once from the first 1005. Take the run of non-space characters after `"MakeMKV v"` in `params[0]`, else in `text`. If neither holds `"MakeMKV v"`, version stays None. |
| 3344 | `java = true` |
| 1009 | Emit `ProfileRejected { text }`. `profile_rejected` keeps the first text. |
| 5021, 5050, 5052, 5055, 5095 | Emit `Licence { code, text }`. `licence` keeps the first `Message`. |
| 2003 | `read_errors = true` |
| 2019 | `write_error = true` |
| 4047 | Push the `Message` onto `removed`. |
| 5005, 5036 | With `params[0] == "1"`, `title_saved = true` (S5). |
| 5003, 5004, 5037 | `title_failed = true` |
| 5011 | `seen_5011 = true` |
| 3307, 3308 | Only for `Call::Rip` with `TitleSource::File { name }`, before the verdict. The first such line whose `params[0] == name` records `params[1]` parsed as u32. |
| 3028 | Only for `Call::Rip` with `TitleSource::DvdTitle`, before the verdict. Count the lines. The (index + 1)-th records `params[0].trim().parse::<u32>()`. |
| 5014, 5015 | For `Call::Rip`, the first of either fixes the verdict. `Some(true)` holds only when the recorded value equals `index` (File) or `number` (DvdTitle). Anything else, including `TitleSource::Unknown` and nothing recorded, gives `Some(false)` and emits one `IndexMismatch`. Later saving messages change nothing. |
| 5010 and all others | No effect (PIPE-14) |

`finish` returns the summary.

Ruling: the guard's verdict is fixed at whichever of `5015` or `5014` comes first, as spec 6.2 step 6 names them ("`MSG:5015` with the job profile, else `MSG:5014`"). The reducer cannot know at a `5014` whether a `5015` will follow.
Ruling: `title_failed` is any of `5003`, `5004` or `5037`, because spec 9.4 requires "none of 5003, 5004 or 5037" for success.
Ruling: `version` is the bare `N.N.N` after `"MakeMKV v"`, so that `settings::version_triple` (settings.rs:219) reads it against the 2.0.0 minimum.
Ruling: the guard reads only lines before the verdict, so a `3307` after the saving message counts as missing (spec 6.2 step 6).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked robot::lines`
Expected: `test result: ok. 19 passed; 0 failed`.

Run: `scripts/box.sh cargo test -p lazymkv-core --locked robot::`
Expected: `0 failed`, with every existing `robot::tests` test unchanged.

- [ ] **Step 5: Commit**

```
git add crates/core/src/robot.rs crates/core/src/robot/lines.rs
git commit -m "core::robot::lines: the per-line reader of makemkvcon output (PIPE-10, design part 5.3)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, the noreply identity for author and committer, and no attribution line.

---

### Task 6: Job, transition and drive records (design parts 7.3, 7.4)

**Files:**
- Create: `crates/core/src/record.rs` (12 tests in its `mod tests`)
- Modify: `crates/core/src/lib.rs` (add `pub mod record;`)

**Interfaces:**
- Consumes:
  - `job::{State, Event, Action, FailureClass}` (job.rs:222, :454, :586, :25). All are serde, internally tagged as `state`, `event` and `action`.
  - `plan::Plan` (plan/mod.rs:171).
  - `decision::FileDecision` (decision.rs:139).
  - `review::{ReviewReason, Note, Noted}` (review.rs:16, :113, :200). `Note` is `Ord` in declaration order; `Noted` is not `Ord`.
  - `verify::ReadError` (verify.rs:160).
- Produces (`crates/core/src/record.rs`):

```rust
/// state.json's format (design part 7.4). Def: T6. Use: T21.
pub const JOB_FORMAT: &str = "lazymkv job v1";
/// drives.json's format (design part 7.4). Def: T6. Use: T21, T22.
pub const DRIVES_FORMAT: &str = "lazymkv drives v1";
/// A remembered decision's format (design part 7.4). Def: T6. Use: T21, T23.
pub const REMEMBERED_FORMAT: &str = "lazymkv remembered v1";

/// Ok when `document["format"]` is `expected` or a minor version of it (`<name> v<major>[.<minor>]`, same name and
/// major); Err naming both strings otherwise (design part 7.4: refused, reported, never overwritten). Def: T6. Use: T21.
pub fn check_format(document: &serde_json::Value, expected: &str) -> Result<(), String>;

/// state.json (design part 7.3; S1). No deny_unknown_fields, so a minor version's added key reads.
/// `titles` serialise in key order and `notes` by note (RP23). Def: T6. Use: T7, T21, T22, T25.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobRecord { pub format: String, pub id: String, pub fingerprint: String, pub disc_id: Option<String>,
    pub drive: String, pub created_unix: u64, pub updated_unix: u64, pub state: crate::job::State, pub message: Option<String>,
    pub rip_again: bool, pub profile: String, pub plan: Option<crate::plan::Plan>,
    #[serde(serialize_with = "...")] pub titles: Vec<TitleRecord>,
    pub filed_unix: Option<u64>, pub collisions: Vec<String>,
    #[serde(serialize_with = "...")] pub notes: Vec<crate::review::Noted> }

/// One planned title's progress in state.json (design part 7.3). Def: T6. Use: T23, T24.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TitleRecord { pub key: String, pub scan_title: u32, pub rip_index: Option<u32>, pub attempts: u32,
    pub verified: bool, pub read_errors: Vec<crate::verify::ReadError>, pub rip_reviews: Vec<crate::review::ReviewReason>,
    pub decision: Option<crate::decision::FileDecision>, pub filed: Option<FiledFile> }

/// A filed file's exact path, size and modification time (design part 7.3). Def: T6. Use: T24.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FiledFile { pub path: String, pub size: u64, pub mtime_unix: u64 }

/// One transitions.jsonl line (design part 7.3). Def: T6. Use: T21.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transition { pub time_unix: u64, pub event: Option<crate::job::Event>, pub old: Option<crate::job::State>,
    pub new: crate::job::State, pub actions: Vec<crate::job::Action> }

/// drives.json (design part 7.4, L1a; S3). Def: T6. Use: T21, T22, T25.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DrivesFile { pub format: String, pub drives: std::collections::BTreeMap<String, DriveRecord>,
    pub drive_incapable: Vec<DriveIncapableEntry> }
/// One drive's last disc and whether its arrival was handled (design part 7.4, L1a). Def: T6. Use: T22.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DriveRecord { pub fingerprint: Option<String>, pub handled: bool }
/// One DriveIncapable failure (design part 7.4, spec 10.2). Def: T6. Use: phase 5.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DriveIncapableEntry { pub drive_name: String, pub format: Option<String>, pub job_id: String,
    pub time_unix: u64, pub message: String }

/// A DRV line's field 4 cut at its last ASCII space (the serial dropped), the rest byte for byte; the whole field when
/// it has no space; "" when the line has no field 4 (spec 3, design part 7.4). Def: T6. Use: phase 5.
pub fn drive_name_without_serial(drv: &[String]) -> String;
```

- [ ] **Step 1: Write the failing tests**

In `crates/core/src/record.rs`, with `pub mod record;` added to `lib.rs`:

```rust
//! The engine's persisted records: state.json, transitions.jsonl and drives.json (design parts 7.3, 7.4; RP23).
//! Plain serde types with no I/O (RP19).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Action, Event, FailureClass, State};
    use crate::review::{Note, Noted};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn job() -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(),
            id: "abc-1".to_string(),
            fingerprint: "ABC".to_string(),
            disc_id: None,
            drive: "tok0".to_string(),
            created_unix: 1,
            updated_unix: 2,
            state: State::Detected,
            message: None,
            rip_again: false,
            profile: "job.mmcp.xml".to_string(),
            plan: None,
            titles: vec![],
            filed_unix: None,
            collisions: vec![],
            notes: vec![],
        }
    }

    fn title(key: &str) -> TitleRecord {
        TitleRecord {
            key: key.to_string(),
            scan_title: 0,
            rip_index: None,
            attempts: 1,
            verified: true,
            read_errors: vec![],
            rip_reviews: vec![],
            decision: None,
            filed: None,
        }
    }

    fn noted(note: Note) -> Noted {
        Noted { note, stream: None, detail: "d".to_string() }
    }

    #[test]
    fn a_job_record_round_trips_with_its_format_first() {
        let mut record = job();
        record.titles = vec![title("t00-00800")];
        record.titles[0].filed = Some(FiledFile { path: "/lib/a.mkv".to_string(), size: 16, mtime_unix: 8 });
        record.filed_unix = Some(9);
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.starts_with(r#"{"format":"lazymkv job v1","id":"abc-1","#), "{text}");
        assert!(text.contains(r#""filed":{"path":"/lib/a.mkv","size":16,"mtime_unix":8}"#), "{text}");
        assert!(text.contains(r#""filed_unix":9"#), "{text}");
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(check_format(&value, JOB_FORMAT), Ok(()));
        assert_eq!(serde_json::from_value::<JobRecord>(value).unwrap(), record);
    }

    #[test]
    fn a_record_with_no_format_is_refused() {
        let refused = r#"format is missing; expected "lazymkv job v1""#.to_string();
        assert_eq!(check_format(&json!({"id": "abc-1"}), JOB_FORMAT), Err(refused.clone()));
        assert_eq!(check_format(&json!({"format": 1}), JOB_FORMAT), Err(refused));
    }

    #[test]
    fn a_record_of_another_major_version_is_refused() {
        assert_eq!(
            check_format(&json!({"format": "lazymkv job v2"}), JOB_FORMAT),
            Err(r#"format "lazymkv job v2" is not "lazymkv job v1" or a minor version of it"#.to_string())
        );
        for found in ["lazymkv job v10", "lazymkv job v1.", "lazymkv job v1x", "lazymkv job v1.1.1", "lazymkv drives v1"] {
            assert!(check_format(&json!({ "format": found }), JOB_FORMAT).is_err(), "{found}");
        }
    }

    #[test]
    fn a_minor_version_is_accepted() {
        assert_eq!(check_format(&json!({"format": "lazymkv job v1.1"}), JOB_FORMAT), Ok(()));
        assert_eq!(check_format(&json!({"format": "lazymkv drives v1"}), DRIVES_FORMAT), Ok(()));
        assert_eq!(check_format(&json!({"format": "lazymkv remembered v1.12"}), REMEMBERED_FORMAT), Ok(()));
    }

    #[test]
    fn a_minor_versions_added_key_is_ignored_on_read() {
        let mut value = serde_json::to_value(job()).unwrap();
        value["added_in_v1_1"] = json!(true);
        assert_eq!(serde_json::from_value::<JobRecord>(value).unwrap(), job());
    }

    #[test]
    fn a_failed_state_keeps_its_class_and_from() {
        let mut record = job();
        record.state = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Scanned) };
        record.message = Some("needs whole-disc copy mode".to_string());
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains(r#""state":{"state":"Failed","class":"Unknown","from":{"state":"Scanned"}}"#), "{text}");
        assert_eq!(serde_json::from_str::<JobRecord>(&text).unwrap(), record);
    }

    #[test]
    fn the_first_transition_has_no_event_and_no_old_state() {
        let first = Transition { time_unix: 7, event: None, old: None, new: State::Detected, actions: vec![Action::Fingerprint] };
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            r#"{"time_unix":7,"event":null,"old":null,"new":{"state":"Detected"},"actions":[{"action":"Fingerprint"}]}"#
        );
        let second = Transition {
            time_unix: 8,
            event: Some(Event::FingerprintDone),
            old: Some(State::Detected),
            new: State::Fingerprinted,
            actions: vec![Action::RunScan],
        };
        let text = serde_json::to_string(&second).unwrap();
        assert_eq!(
            text,
            r#"{"time_unix":8,"event":{"event":"FingerprintDone"},"old":{"state":"Detected"},"new":{"state":"Fingerprinted"},"actions":[{"action":"RunScan"}]}"#
        );
        assert_eq!(serde_json::from_str::<Transition>(&text).unwrap(), second);
    }

    #[test]
    fn drives_are_keyed_by_drive_id() {
        let file = DrivesFile {
            format: DRIVES_FORMAT.to_string(),
            drives: BTreeMap::from([("D:".to_string(), DriveRecord { fingerprint: None, handled: false })]),
            drive_incapable: vec![],
        };
        let text = serde_json::to_string(&file).unwrap();
        assert_eq!(
            text,
            r#"{"format":"lazymkv drives v1","drives":{"D:":{"fingerprint":null,"handled":false}},"drive_incapable":[]}"#
        );
        assert_eq!(serde_json::from_str::<DrivesFile>(&text).unwrap(), file);
    }

    #[test]
    fn a_drive_incapable_entry_keeps_its_five_keys_in_order() {
        let entry = DriveIncapableEntry {
            drive_name: "BD-RE SYNTHETIC DRIVE 1.00".to_string(),
            format: Some("bluray".to_string()),
            job_id: "abc-1".to_string(),
            time_unix: 7,
            message: "m".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            r#"{"drive_name":"BD-RE SYNTHETIC DRIVE 1.00","format":"bluray","job_id":"abc-1","time_unix":7,"message":"m"}"#
        );
    }

    #[test]
    fn a_drive_name_drops_the_serial() {
        let drv = |name: &str| ["0", "2", "999", "12", name, "L", "D:"].map(String::from).to_vec();
        assert_eq!(drive_name_without_serial(&drv("BD-RE PIONEER BD-RW  BDR-XS07U 1.01 SERIAL")), "BD-RE PIONEER BD-RW  BDR-XS07U 1.01");
        assert_eq!(drive_name_without_serial(&drv("DRIVE")), "DRIVE");
        assert_eq!(drive_name_without_serial(&["0".to_string()]), "");
    }

    #[test]
    fn notes_serialise_sorted_by_note() {
        let mut record = job();
        record.notes = vec![noted(Note::LabelRuleUnavailable), noted(Note::TuningEvidencePending)];
        let text = serde_json::to_string(&record).unwrap();
        let tuning = text.find(r#""note":"tuning_evidence_pending""#).unwrap();
        let label = text.find(r#""note":"label_rule_unavailable""#).unwrap();
        assert!(tuning < label, "{text}");
    }

    #[test]
    fn titles_serialise_in_key_order() {
        let mut record = job();
        record.titles = vec![title("t01-00055"), title("t00-00800")];
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.find(r#""key":"t00-00800""#).unwrap() < text.find(r#""key":"t01-00055""#).unwrap(), "{text}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked record::`
Expected: compile error, ``cannot find struct, variant or union type `JobRecord` `` and ``cannot find function `check_format` ``.

- [ ] **Step 3: Implement the records in `record.rs`**

Write the items of the Interfaces block with the field orders shown. Field order is the key order (S1). Two private functions serve as the `serialize_with` targets:
- `sorted_titles` serialises the titles sorted by `key`.
- `sorted_notes` serialises the notes sorted by `(note, stream, detail)`.

Deserialisation keeps the file's order.

`check_format` works as follows:
1. A missing `format`, or one that is not a string, gives ``Err(format!("format is missing; expected \"{expected}\""))``.
2. Split `expected` at its last `" v"` into name and major.
3. Ok when `found` starts with `"<name> v"` and the rest is `<major>` or `<major>.<digits>`, with at least one ASCII digit after the dot.
4. Otherwise ``Err(format!("format \"{found}\" is not \"{expected}\" or a minor version of it"))``.

`drive_name_without_serial` behaves as its doc states: it cuts at `rfind(' ')`.

Ruling: notes sort by `Note`'s own order, the declaration order that core's sorted fields already use, and titles sort by key. Sorting at serialise time keeps every writer's output in RP23's sorted form without each caller sorting.
Ruling: the refusal texts are the two sentences above, each naming the expected format, so that `engine.log` can quote them (design part 7.4, "reports it").
Ruling: `JobRecord` and `DrivesFile` have no `deny_unknown_fields`, so that a `v1.<minor>` file's added keys read, as `check_format` accepts minor versions.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked record::`
Expected: `test result: ok. 12 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add crates/core/src/record.rs crates/core/src/lib.rs
git commit -m "core::record: state.json, transitions.jsonl and drives.json types (design parts 7.3, 7.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as both author and committer, and no attribution line.

### Task 7: IPC messages and the notification payload (design parts 9.1, 9.3, spec 12)

**Files:**
- Create: `crates/core/src/ipc.rs`, with 17 tests in its `mod tests`.
- Modify: `crates/core/src/lib.rs:6`. Add `pub mod ipc;` between `pub mod fingerprint;` and `pub mod job;`.

**Interfaces:**
- Consumes:
  - `job::State` (crates/core/src/job.rs:222, `#[serde(tag = "state")]`), `job::FailureClass` (:25, PascalCase), `job::NotifyEvent` (:413, `#[serde(rename_all = "snake_case", tag = "event")]`) and `job::WaitReason` (:359, lowercase).
  - `review::ReviewReason` (crates/core/src/review.rs:16), `review::Noted` (:200) and `review::sorted_unique` (:211).
  - `plan::Plan` (crates/core/src/plan/mod.rs:171), `identity.title` (crates/core/src/lookup/mod.rs:382) and `reviews`.
  - `settings::Settings` (crates/core/src/settings.rs:73) and `settings::SettingsProblem { key, problem }` (:238).
  - From Task 6: `record::{JobRecord, TitleRecord, JOB_FORMAT}`.
- Produces: every item below, in `core::ipc`.

```rust
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
/// The error text for a line over MAX_LINE_BYTES, from parse_request and from the engine's capped reader (design part
/// 9.1). Def: T7. Use: T7, T26.
pub const LINE_TOO_LONG: &str = "the line is longer than 4 MiB";
pub const HISTORY_PAGE: usize = 50;
pub const CANCELLED: &str = "cancelled by the owner";
pub const SECRET_NAMES: [&str; 3] = ["tmdb_token", "ha_webhook_url", "webhook_url"];
pub fn is_secret_name(name: &str) -> bool;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Envelope { pub v: u32, #[serde(rename = "type")] pub kind: String,
    #[serde(default)] pub id: Option<u64>, #[serde(default)] pub payload: serde_json::Value }

#[derive(Clone, Debug, PartialEq)]
pub enum Request { SettingsSet { changes: serde_json::Map<String, serde_json::Value> }, SecretSet { name: String, value: String },
    Retry { job: String }, Cancel { job: String }, Discard { job: String }, RipAgain { job: String }, RunSelftest,
    History { before: Option<String> }, Shutdown { force: bool }, Command { words: Vec<String> },
    ReviewAction { job: String, action: String, overrides: Option<serde_json::Value> }, UiQuit }
pub fn parse_request(line: &str) -> Result<(Option<u64>, Request), String>;
pub fn reply_ok(id: Option<u64>, result: serde_json::Value) -> String;
pub fn reply_error(id: Option<u64>, message: &str, problems: &[SettingsProblem]) -> String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputStream { Stdout, Stderr }
#[derive(Clone, Debug, PartialEq)]
pub enum Push { Snapshot(Snapshot), JobUpdate(JobUpdate), Progress(Progress), SelftestResult(SelfTest),
    CommandOutput { stream: OutputStream, line: String } }
pub fn push_line(push: &Push, id: Option<u64>) -> String;
#[derive(Clone, Debug, PartialEq)]
pub enum Reply { Ok { id: Option<u64>, result: serde_json::Value },
    Error { id: Option<u64>, message: String, problems: Vec<SettingsProblem> }, Push(Envelope) }
pub fn read_reply(line: &str) -> Result<Reply, String>;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobSummary { pub id: String, pub state: State, pub title: Option<String>, pub fingerprint: String, pub drive: String,
    pub created_unix: u64, pub updated_unix: u64, pub review_reasons: Vec<ReviewReason>, pub notes: Vec<Noted>,
    pub class: Option<FailureClass>, pub message: Option<String> }
pub fn job_summary(record: &JobRecord) -> JobSummary;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobUpdate { #[serde(flatten)] pub summary: JobSummary, pub plan: Option<Plan>, pub collisions: Option<Vec<String>> }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot { pub jobs: Vec<JobSummary>, pub settings: Settings, pub problems: Vec<SettingsProblem>,
    pub secrets: BTreeMap<String, SecretState>, pub selftest: SelfTest }
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecretState { pub set: bool, pub host: Option<String> }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Progress { pub job: String, pub step: String, pub title: Option<String>, pub fraction: f64 }
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelfTest { pub checks: Vec<SelfTestCheck> }
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelfTestCheck { pub name: String, pub ok: bool, pub detail: String, pub path: Option<String>,
    pub version: Option<String>, pub found: Option<String>, pub busy: bool }
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Notification { #[serde(flatten)] pub event: NotifyEvent, pub job_id: Option<String>, pub fingerprint: Option<String>,
    pub title: Option<String>, pub drive: Option<String>, pub message: String, pub time: u64 }
```

Rulings settled in this task:
- **Ruling T7-a:** every `Option` field serialises as `null` and is never omitted. Spec 12 already writes `job_id` as null, and clients then read one shape.
- **Ruling T7-b:** an `error` reply always carries `problems`, an empty list unless a `settings_set` was refused. This gives one shape for `read_reply`.
- **Ruling T7-c:** a summary's `review_reasons` is `sorted_unique` over three sources: the plan's `reviews`, each title's `rip_reviews`, and each title's `decision.reviews`. `class` and `message` are Some only when `state` is `Failed`, because design part 9.3 gives them "for a failed job".
- **Ruling T7-d:** a summary's `title` is `plan.identity.title`, and None without a plan. It is the only title the record holds before a decision.
- **Ruling T7-e:** a `null` or absent payload reads as `{}`. A request with no payload fields, such as `run_selftest`, then needs nothing, and a missing required field still fails by name.
- **Ruling T7-f:** a share secret name is exactly `share:\\<server>\<share>`, with both parts non-empty and no further backslash, because design part 9.4 names a share and not a folder.
- **Ruling T7-g:** the error texts are the ones in the tests below. Neither the design nor the spec gives any.
- **Ruling T7-h:** `reply_ok`, `reply_error` and `push_line` return the line without its `\n`, and the IPC writer appends one.
- **Ruling T7-i:** `shutdown`'s `force` is required, because design part 9.3 gives it as "true or false" and ctl always sends it.

- [ ] **Step 1: Write the failing tests in `crates/core/src/ipc.rs`**

Add `pub mod ipc;` to `crates/core/src/lib.rs`, then create `ipc.rs` holding only a placeholder module doc and these tests:

```rust
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::job::{FailureClass, NotifyEvent, State, WaitReason};
    use crate::record::{JobRecord, TitleRecord, JOB_FORMAT};
    use crate::review::ReviewReason;
    use crate::settings::{Settings, SettingsProblem};

    fn record(state: State, message: Option<&str>) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(),
            id: "abc-1".to_string(),
            fingerprint: "ABC".to_string(),
            disc_id: None,
            drive: "tok0".to_string(),
            created_unix: 1_800_000_000,
            updated_unix: 1_800_000_060,
            state,
            message: message.map(str::to_string),
            rip_again: false,
            profile: "job.mmcp.xml".to_string(),
            plan: None,
            titles: vec![],
            filed_unix: None,
            collisions: vec![],
            notes: vec![],
        }
    }

    fn title(key: &str, rip_reviews: Vec<ReviewReason>) -> TitleRecord {
        TitleRecord { key: key.to_string(), scan_title: 0, rip_index: Some(0), attempts: 2, verified: true,
            read_errors: vec![], rip_reviews, decision: None, filed: None }
    }

    /// The error reply `parse_request` refused `line` with, read back.
    fn refusal(line: &str) -> Reply {
        read_reply(&parse_request(line).unwrap_err()).unwrap()
    }

    fn error(id: Option<u64>, message: &str) -> Reply {
        Reply::Error { id, message: message.to_string(), problems: vec![] }
    }

    #[test]
    fn a_request_with_v_2_is_an_error_reply_with_its_id() {
        assert_eq!(refusal(r#"{"v":2,"type":"retry","id":7,"payload":{"job":"a-1"}}"#),
            error(Some(7), "protocol version 2 is not supported; this engine speaks version 1"));
        assert_eq!(refusal(r#"{"type":"retry","id":8,"payload":{"job":"a-1"}}"#),
            error(Some(8), "the message has no protocol version"));
    }

    #[test]
    fn an_unknown_type_is_an_error_reply() {
        assert_eq!(refusal(r#"{"v":1,"type":"frobnicate","id":5,"payload":null}"#),
            error(Some(5), "unknown message type \"frobnicate\""));
        assert_eq!(refusal(r#"{"v":1,"id":6,"payload":null}"#), error(Some(6), "the message has no type"));
    }

    #[test]
    fn a_line_over_4_mib_is_refused_before_parsing() {
        let long = format!(r#"{{"v":1,"type":"retry","id":9,"payload":{{"job":"{}"}}}}"#, "a".repeat(MAX_LINE_BYTES));
        assert_eq!(refusal(&long), error(None, LINE_TOO_LONG));
        assert_eq!(LINE_TOO_LONG, "the line is longer than 4 MiB");
        let short = r#"{"v":1,"type":"run_selftest","id":1,"payload":null}"#;
        let exact = format!("{short}{}", " ".repeat(MAX_LINE_BYTES - short.len()));
        assert_eq!(exact.len(), MAX_LINE_BYTES);
        assert_eq!(parse_request(&exact), Ok((Some(1), Request::RunSelftest)));
    }

    #[test]
    fn a_line_that_is_not_json_is_an_error_reply_with_no_id() {
        let Reply::Error { id, message, problems } = refusal("not json") else { panic!("not an error reply") };
        assert_eq!(id, None);
        assert!(message.starts_with("the line is not a JSON message: "), "{message}");
        assert!(problems.is_empty());
    }

    #[test]
    fn unknown_payload_fields_are_ignored() {
        assert_eq!(parse_request(r#"{"v":1,"type":"retry","id":3,"payload":{"job":"a-1","extra":1},"later":true}"#),
            Ok((Some(3), Request::Retry { job: "a-1".to_string() })));
    }

    #[test]
    fn a_payload_missing_a_field_is_an_error_reply_naming_the_type() {
        assert_eq!(refusal(r#"{"v":1,"type":"retry","id":4,"payload":{}}"#),
            error(Some(4), "the retry payload does not fit: missing field `job`"));
        assert_eq!(refusal(r#"{"v":1,"type":"retry","id":4,"payload":null}"#),
            error(Some(4), "the retry payload does not fit: missing field `job`"));
        assert_eq!(refusal(r#"{"v":1,"type":"settings_set","id":2,"payload":[1]}"#),
            error(Some(2), "the settings_set payload is not an object"));
    }

    #[test]
    fn a_settings_set_payload_is_the_changed_keys_themselves() {
        let (id, request) = parse_request(r#"{"v":1,"type":"settings_set","id":1,"payload":{"cache_mb":512}}"#).unwrap();
        assert_eq!(id, Some(1));
        let Request::SettingsSet { changes } = request else { panic!("not settings_set") };
        assert_eq!(serde_json::Value::Object(changes), json!({"cache_mb": 512}));
    }

    #[test]
    fn every_request_type_of_design_part_9_3_parses() {
        let job = || "a-1".to_string();
        let cases: Vec<(&str, Request)> = vec![
            (r#"{"v":1,"type":"secret_set","id":2,"payload":{"name":"tmdb_token","value":"t0k"}}"#,
                Request::SecretSet { name: "tmdb_token".to_string(), value: "t0k".to_string() }),
            (r#"{"v":1,"type":"retry","id":2,"payload":{"job":"a-1"}}"#, Request::Retry { job: job() }),
            (r#"{"v":1,"type":"cancel","id":2,"payload":{"job":"a-1"}}"#, Request::Cancel { job: job() }),
            (r#"{"v":1,"type":"discard","id":2,"payload":{"job":"a-1"}}"#, Request::Discard { job: job() }),
            (r#"{"v":1,"type":"rip_again","id":2,"payload":{"job":"a-1"}}"#, Request::RipAgain { job: job() }),
            (r#"{"v":1,"type":"run_selftest","id":2,"payload":null}"#, Request::RunSelftest),
            (r#"{"v":1,"type":"history","id":2,"payload":{"before":"a-1"}}"#, Request::History { before: Some(job()) }),
            (r#"{"v":1,"type":"history","id":2,"payload":{}}"#, Request::History { before: None }),
            (r#"{"v":1,"type":"shutdown","id":2,"payload":{"force":true}}"#, Request::Shutdown { force: true }),
            (r#"{"v":1,"type":"command","id":2,"payload":{"words":["plan","--job","a-1"]}}"#,
                Request::Command { words: vec!["plan".to_string(), "--job".to_string(), job()] }),
            (r#"{"v":1,"type":"review_action","id":2,"payload":{"job":"a-1","action":"approve"}}"#,
                Request::ReviewAction { job: job(), action: "approve".to_string(), overrides: None }),
            (r#"{"v":1,"type":"review_action","id":2,"payload":{"job":"a-1","action":"approve","overrides":{}}}"#,
                Request::ReviewAction { job: job(), action: "approve".to_string(), overrides: Some(json!({})) }),
            (r#"{"v":1,"type":"ui_quit","id":2,"payload":null}"#, Request::UiQuit),
        ];
        for (line, expected) in cases {
            assert_eq!(parse_request(line), Ok((Some(2), expected)), "{line}");
        }
        assert_eq!(parse_request(r#"{"v":1,"type":"ui_quit"}"#), Ok((None, Request::UiQuit)));
    }

    #[test]
    fn ok_and_error_replies_read_back() {
        let ok = reply_ok(Some(3), json!({"exit_code": 0}));
        assert_eq!(ok, r#"{"v":1,"type":"ok","id":3,"payload":{"result":{"exit_code":0}}}"#);
        assert_eq!(read_reply(&ok), Ok(Reply::Ok { id: Some(3), result: json!({"exit_code": 0}) }));
        assert_eq!(read_reply(&reply_ok(Some(4), serde_json::Value::Null)),
            Ok(Reply::Ok { id: Some(4), result: serde_json::Value::Null }));
        assert_eq!(read_reply(r#"{"v":2,"type":"ok","id":3,"payload":{"result":null}}"#),
            Err("the engine speaks protocol version 2; this client speaks version 1".to_string()));
    }

    #[test]
    fn pushes_carry_a_null_id_except_command_output() {
        let progress = Push::Progress(Progress { job: "abc-1".to_string(), step: "RipTitle".to_string(),
            title: Some("t00-00800".to_string()), fraction: 0.5 });
        assert_eq!(push_line(&progress, Some(9)),
            r#"{"v":1,"type":"progress","id":null,"payload":{"job":"abc-1","step":"RipTitle","title":"t00-00800","fraction":0.5}}"#);
        let output = Push::CommandOutput { stream: OutputStream::Stderr, line: "plan: refused".to_string() };
        let line = push_line(&output, Some(9));
        assert_eq!(line, r#"{"v":1,"type":"command_output","id":9,"payload":{"stream":"stderr","line":"plan: refused"}}"#);
        let Ok(Reply::Push(envelope)) = read_reply(&line) else { panic!("not a push") };
        assert_eq!((envelope.kind.as_str(), envelope.id), ("command_output", Some(9)));
        assert_eq!(push_line(&Push::SelftestResult(SelfTest::default()), Some(9)),
            r#"{"v":1,"type":"selftest_result","id":null,"payload":{"checks":[]}}"#);
    }

    #[test]
    fn a_summary_nests_the_state_as_it_serialises() {
        let failed = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Scanned) };
        let summary = job_summary(&record(failed, Some(CANCELLED)));
        assert_eq!(serde_json::to_string(&summary).unwrap(), concat!(
            r#"{"id":"abc-1","state":{"state":"Failed","class":"Unknown","from":{"state":"Scanned"}},"title":null,"#,
            r#""fingerprint":"ABC","drive":"tok0","created_unix":1800000000,"updated_unix":1800000060,"#,
            r#""review_reasons":[],"notes":[],"class":"Unknown","message":"cancelled by the owner"}"#));
        let running = job_summary(&record(State::Ripping, Some("ripping t00-00800")));
        assert_eq!((running.class, running.message), (None, None));
    }

    #[test]
    fn a_summary_gathers_the_review_reasons_sorted_and_unique() {
        let mut rec = record(State::Ripping, None);
        rec.titles = vec![
            title("t00-00800", vec![ReviewReason::ReadErrors, ReviewReason::VerifyFailed]),
            title("t01-00801", vec![ReviewReason::VerifyFailed]),
        ];
        assert_eq!(job_summary(&rec).review_reasons, vec![ReviewReason::VerifyFailed, ReviewReason::ReadErrors]);
    }

    #[test]
    fn a_job_update_flattens_its_summary_beside_plan_and_collisions() {
        let update = JobUpdate { summary: job_summary(&record(State::Verified, None)), plan: None,
            collisions: Some(vec!["/lib/Movies/A (2001)/A (2001).mkv".to_string()]) };
        let value = serde_json::to_value(&update).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "state", "title", "fingerprint", "drive", "created_unix", "updated_unix",
            "review_reasons", "notes", "class", "message", "plan", "collisions"]);
        assert_eq!(serde_json::from_value::<JobUpdate>(value).unwrap(), update);
    }

    #[test]
    fn a_snapshot_push_has_its_five_keys_in_order() {
        let snapshot = Snapshot { jobs: vec![job_summary(&record(State::Filed, None))], settings: Settings::default(),
            problems: Settings::default().validate(),
            secrets: BTreeMap::from([("tmdb_token".to_string(), SecretState { set: false, host: None })]),
            selftest: SelfTest::default() };
        let Ok(Reply::Push(envelope)) = read_reply(&push_line(&Push::Snapshot(snapshot.clone()), Some(1))) else {
            panic!("not a push")
        };
        assert_eq!((envelope.kind.as_str(), envelope.id), ("snapshot", None));
        let keys: Vec<&str> = envelope.payload.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["jobs", "settings", "problems", "secrets", "selftest"]);
        assert_eq!(serde_json::from_value::<Snapshot>(envelope.payload).unwrap(), snapshot);
    }

    #[test]
    fn a_refused_settings_set_lists_key_and_problem() {
        let problems = [SettingsProblem { key: "tuning_root".to_string(), problem: "tuning_root is not set".to_string() }];
        let line = reply_error(Some(4), "the settings were refused", &problems);
        assert_eq!(line, concat!(r#"{"v":1,"type":"error","id":4,"payload":{"message":"the settings were refused","#,
            r#""problems":[{"key":"tuning_root","problem":"tuning_root is not set"}]}}"#));
        assert_eq!(read_reply(&line), Ok(Reply::Error { id: Some(4), message: "the settings were refused".to_string(),
            problems: problems.to_vec() }));
        assert_eq!(reply_error(None, "x", &[]), r#"{"v":1,"type":"error","id":null,"payload":{"message":"x","problems":[]}}"#);
    }

    #[test]
    fn a_notification_flattens_its_event() {
        let failed = Notification { event: NotifyEvent::Failed { class: FailureClass::Unknown }, job_id: None,
            fingerprint: None, title: None, drive: Some("tok0".to_string()),
            message: "the volume could not be read".to_string(), time: 1_800_000_000 };
        let text = serde_json::to_string(&failed).unwrap();
        assert_eq!(text, concat!(r#"{"event":"failed","class":"Unknown","job_id":null,"fingerprint":null,"title":null,"#,
            r#""drive":"tok0","message":"the volume could not be read","time":1800000000}"#));
        assert_eq!(serde_json::from_str::<Notification>(&text).unwrap(), failed);
        let waiting = Notification { event: NotifyEvent::Waiting { reason: WaitReason::Lookup },
            job_id: Some("abc-1".to_string()), fingerprint: Some("ABC".to_string()), title: None,
            drive: Some("tok0".to_string()), message: "a lookup is pending".to_string(), time: 1 };
        let text = serde_json::to_string(&waiting).unwrap();
        assert!(text.starts_with(r#"{"event":"waiting","reason":"lookup","job_id":"abc-1""#), "{text}");
        assert_eq!(serde_json::from_str::<Notification>(&text).unwrap(), waiting);
    }

    #[test]
    fn share_secret_names_are_accepted() {
        for name in SECRET_NAMES {
            assert!(is_secret_name(name), "{name}");
        }
        assert!(is_secret_name(r"share:\\nas\media"));
        for name in ["tmdb", "", r"share:\\nas", r"share:\\\media", r"share:\\nas\", r"share:\\nas\media\films", r"share:nas\media"] {
            assert!(!is_secret_name(name), "{name}");
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked ipc`
Expected: a compile error, because `parse_request`, `read_reply`, `reply_ok`, `reply_error`, `push_line`, `job_summary`, `Request`, `Reply`, `Push` and the other ipc names are not found in this scope.

- [ ] **Step 3: Implement `core::ipc`**

The module doc names design parts 9.1 to 9.3, spec 4.5 and spec 12, and says the module does no I/O (RP19). Write the constants and types exactly as in **Interfaces**. Each `pub` item gets a `///` doc that names its design part and ends `Def: T7. Use: T<m>.` Use:
- T22 for `Notification`;
- T26 for the server side (`parse_request`, `reply_ok`, `reply_error`, `push_line`, `job_summary`, `is_secret_name`, `HISTORY_PAGE` and `CANCELLED`);
- T28 for `read_reply`;
- T26 and T28 for the shared types.

Do not put `deny_unknown_fields` on any type.

`parse_request(line)` refuses with the texts Step 1's tests pin, and checks in this order, which the tests leave open. Every `Err` is `reply_error(id, <text>, &[])`, with `id` None until step 3 reads it.
1. The length, against `MAX_LINE_BYTES`.
2. `serde_json::from_str::<Value>(line)`. A value that is not an object gives `the line is not a JSON message: it is not an object`, a text no test names.
3. `id = value["id"].as_u64()`, read before the version check, so a wrong version is answered with the request's id.
4. `v`, then 5. `type`.
6. `payload`, `Null` when absent. `settings_set` takes an object as `changes`. Every other type reads a `Null` payload as `{}` (T7-e) and decodes it with `serde_json::from_value` into a private `#[derive(serde::Deserialize)]` struct: `{ job }`, `{ name, value }`, `{ #[serde(default)] before: Option<String> }`, `{ force: bool }`, `{ words: Vec<String> }`, or `{ job, action, #[serde(default)] overrides: Option<Value> }`. `run_selftest` and `ui_quit` ignore their payload. An unknown type string is refused.

The output functions write the envelope shapes the tests pin, with `serde_json::to_string` of a `json!` value, whose keys stay in order through `preserve_order`, and no newline (T7-h). `push_line`'s `id` is the argument for `CommandOutput` and `null` for every other push.

`read_reply(line)`:
1. Decode `line` as an `Envelope`. On an error `e`, return `Err("the engine's line is not a message: {e}")`.
2. When `v != 1`, return `Err("the engine speaks protocol version {v}; this client speaks version 1")`.
3. For `ok`, return `Reply::Ok { id, result: payload["result"] }`, with `Null` when absent.
4. For `error`, read `message`, a required string; otherwise return `Err("the engine's error reply has no message")`. `problems` is a list of `SettingsProblem` and is empty when absent.
5. Any other type gives `Reply::Push(envelope)`.

`job_summary(record)`:
- `title`: `record.plan.as_ref().and_then(|p| p.identity.title.clone())` (T7-d).
- `review_reasons`: `sorted_unique` of the plan's `reviews`, then each title's `rip_reviews`, then each title's `decision` `reviews` (T7-c).
- `notes`: `record.notes` cloned.
- `class` and `message`: Some only when the state is `State::Failed { class, .. }` (T7-c).
- Every other field is copied from the record.

`is_secret_name(name)`: true when `SECRET_NAMES` holds `name`. It is also true when `name.strip_prefix(r"share:\\")` gives `rest` and `rest.split_once('\\')` gives a non-empty server and a non-empty share containing no `\` (T7-f).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked ipc`
Expected: PASS, `test result: ok. 17 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add crates/core/src/ipc.rs crates/core/src/lib.rs
git commit -m "core::ipc: the IPC envelope, payloads and notification (design parts 9.1, 9.3; spec 12)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>` and no attribution line.

### Task 8: Platform traits, the `Platform` bundle and the fakes (design part 4.2)

**Files:**
- Modify: `crates/platform/src/lib.rs`, which Task 1 created with only its crate doc. Add the traits and types below, and `#[cfg(feature = "fakes")] pub mod fakes;`.
- Create: `crates/platform/src/fakes.rs`, with 13 tests in its `mod tests`. They compile only with feature `fakes`, which Task 1 declared.

**Interfaces:**
- Consumes: `lazymkv_core::job::Tool` (crates/core/src/job.rs:138).
- Produces, in `crates/platform/src/lib.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriveEvent { Arrived { drive: String, root: PathBuf }, Removed { drive: String } }
pub trait DriveWatcher: Send { fn next(&mut self, timeout: Duration) -> Option<DriveEvent>; fn present(&self) -> Vec<(String, PathBuf)>; }
pub trait Ejector: Send + Sync { fn eject(&self, drive: &str) -> Result<(), String>; }
#[derive(Clone, Debug)]
pub struct Spawn { pub program: PathBuf, pub args: Vec<String>, pub env: Vec<(String, String)>, pub below_normal: bool }
pub trait ProcessGuard: Send + Sync { fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>>; }
pub trait GuardedChild: Send { fn take_stdout(&mut self) -> Box<dyn Read + Send>; fn take_stderr(&mut self) -> Box<dyn Read + Send>;
    fn killer(&self) -> KillHandle; fn wait(&mut self) -> io::Result<Option<i32>>; }
#[derive(Clone)]
pub struct KillHandle(pub(crate) Arc<dyn Fn() -> io::Result<()> + Send + Sync>);
impl KillHandle { pub fn kill(&self) -> io::Result<()>; }
pub trait KeepAwake: Send + Sync { fn acquire(&self); fn release(&self); }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach { Reachable, Unreachable(String) }
pub trait ShareConnector: Send + Sync { fn reach(&self, root: &str) -> Reach; fn free_bytes(&self, root: &str) -> io::Result<u64>; fn volume(&self, root: &str) -> Option<String>; }
pub trait SecretStore: Send + Sync { fn get(&self, name: &str) -> Option<String>; fn set(&self, name: &str, value: &str) -> Result<(), String>; }
pub struct IpcStream { pub reader: Box<dyn BufRead + Send>, pub writer: Box<dyn Write + Send> }
pub trait IpcTransport: Send + Sync { fn accept(&self) -> io::Result<IpcStream>; }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectError { NotRunning, Refused, NotElevated, NoTransport }
pub trait IpcConnector { fn connect(&self) -> Result<IpcStream, ConnectError>; }
pub trait Autostart { fn install(&self) -> Result<(), String>; fn remove(&self) -> Result<(), String>; }
pub trait Host: Send + Sync { fn now_unix(&self) -> u64; fn local_date(&self) -> String; fn makemkv_app_key(&self) -> Option<String>;
    fn makemkv_debug_log(&self) -> Option<PathBuf>; fn running_processes(&self) -> Vec<String>;
    fn volume_label(&self, root: &Path) -> Option<String>; fn default_install_path(&self, tool: Tool) -> Option<PathBuf>; }
pub struct Platform { pub drives: Box<dyn DriveWatcher>, pub ejector: Arc<dyn Ejector>, pub processes: Arc<dyn ProcessGuard>,
    pub awake: Arc<dyn KeepAwake>, pub shares: Arc<dyn ShareConnector>, pub secrets: Arc<dyn SecretStore>,
    pub ipc: Arc<dyn IpcTransport>, pub host: Arc<dyn Host> }
```

- Produces, in `crates/platform/src/fakes.rs` (feature `fakes`):

```rust
pub struct ScriptedDrives;   // impl DriveWatcher
#[derive(Clone)] pub struct DriveScript;
impl ScriptedDrives { pub fn new() -> (ScriptedDrives, DriveScript); }
impl DriveScript { pub fn arrive(&self, drive: &str, root: &Path); pub fn remove(&self, drive: &str); pub fn present(&self) -> Vec<(String, PathBuf)>; }
pub struct RecordingEjector; // impl Ejector
impl RecordingEjector { pub fn new() -> RecordingEjector; pub fn calls(&self) -> Vec<String>; pub fn fail_next(&self, message: &str);
    pub fn with_removals(self, script: DriveScript) -> RecordingEjector; }
pub struct CountingKeepAwake; // impl KeepAwake
impl CountingKeepAwake { pub fn new() -> CountingKeepAwake; pub fn holders(&self) -> usize; }
pub struct FakeShares;       // impl ShareConnector
impl FakeShares { pub fn new() -> FakeShares; pub fn set_free(&self, root: &str, bytes: u64); pub fn set_reach(&self, root: &str, reach: Reach);
    pub fn same_volume(&self, a: &str, b: &str); }
pub struct MemorySecrets;    // impl SecretStore
impl MemorySecrets { pub fn new() -> MemorySecrets; pub fn with(self, name: &str, value: &str) -> MemorySecrets; }
pub struct MemoryIpc;        // impl IpcTransport
#[derive(Clone)] pub struct MemoryConnector; // impl IpcConnector
impl MemoryIpc { pub fn new() -> (MemoryIpc, MemoryConnector); }
impl MemoryConnector { pub fn failing(error: ConnectError) -> MemoryConnector; }
pub struct FakeHost;         // impl Host
impl FakeHost { pub fn new(local_date: &str, now_unix: u64) -> FakeHost; pub fn set_label(&self, root: &Path, label: &str);
    pub fn set_running(&self, names: Vec<String>); pub fn set_app_key(&self, key: &str); pub fn set_debug_log(&self, path: &Path); }
```

(The struct bodies are private. Every fake keeps its state behind `Mutex` or atomics, so its setters take `&self`.)

Rulings settled in this task:
- **Ruling T8-a:** `FakeShares` keys free space and reach by the exact root string. Volumes start as `Some(root)`, and `same_volume(a, b)` makes `volume(b)` return `volume(a)` (S11). The engine asks each root by its own string, so no path rule runs on Linux and macOS.
- **Ruling T8-b:** `present()` on the watcher and on the script reflects `arrive` and `remove` at once, sorted by drive id, before `next` hands the event out. The engine reads `present()` at start (L1), before any event is drained.
- **Ruling T8-c:** `with_removals` pushes `Removed` only for a successful eject. A failed eject leaves the disc in.
- **Ruling T8-d:** `KillHandle`'s field is `pub(crate)`, so only `platform` builds one (Task 9).
- **Ruling T8-e:** a `MemoryIpc` write after the other end's reader is gone fails with `ErrorKind::BrokenPipe`. A reader whose writer is gone reads end of file. `accept` returns `Err(BrokenPipe)` once every `MemoryConnector` is dropped. These are the ends a real pipe gives.
- **Ruling T8-f:** `DriveScript::remove` pushes `Removed` even for a drive that is not present. `arrive` on a present drive replaces its root and pushes `Arrived`. A script then says exactly what the test wrote.
- **Ruling T8-g:** `CountingKeepAwake::release` stops at zero.

- [ ] **Step 1: Write the failing tests in `crates/platform/src/fakes.rs`**

Add `#[cfg(feature = "fakes")] pub mod fakes;` to `lib.rs`. Create `fakes.rs` with a module doc naming design part 4.2 and these tests:

```rust
#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, Write};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use lazymkv_core::job::Tool;

    use super::*;
    use crate::{ConnectError, DriveEvent, DriveWatcher, Ejector, GuardedChild, Host, IpcConnector, IpcTransport, KeepAwake,
        Platform, ProcessGuard, Reach, SecretStore, ShareConnector, Spawn};

    const SHORT: Duration = Duration::from_millis(10);

    fn arrived(drive: &str, root: &str) -> Option<DriveEvent> {
        Some(DriveEvent::Arrived { drive: drive.to_string(), root: PathBuf::from(root) })
    }

    #[test]
    fn scripted_drives_replay_arrivals_and_removals_in_order() {
        let (mut drives, script) = ScriptedDrives::new();
        script.arrive("tok0", Path::new("/discs/a"));
        assert_eq!(script.present(), vec![("tok0".to_string(), PathBuf::from("/discs/a"))]);
        assert_eq!(drives.present(), script.present());
        script.remove("tok0");
        assert!(drives.present().is_empty());
        assert_eq!(drives.next(SHORT), arrived("tok0", "/discs/a"));
        assert_eq!(drives.next(SHORT), Some(DriveEvent::Removed { drive: "tok0".to_string() }));
        assert_eq!(drives.next(SHORT), None);
    }

    #[test]
    fn a_waiting_watcher_wakes_on_an_arrival() {
        let (mut drives, script) = ScriptedDrives::new();
        let later = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            script.arrive("tok1", Path::new("/discs/b"));
        });
        assert_eq!(drives.next(Duration::from_secs(5)), arrived("tok1", "/discs/b"));
        later.join().unwrap();
    }

    #[test]
    fn the_ejector_records_each_call_and_fails_on_request() {
        let ejector = RecordingEjector::new();
        assert_eq!(ejector.eject("tok0"), Ok(()));
        ejector.fail_next("stuck");
        assert_eq!(ejector.eject("tok0"), Err("stuck".to_string()));
        assert_eq!(ejector.eject("tok1"), Ok(()));
        assert_eq!(ejector.calls(), ["tok0", "tok0", "tok1"]);
    }

    #[test]
    fn an_ejector_with_removals_pushes_the_removal_it_causes() {
        let (mut drives, script) = ScriptedDrives::new();
        script.arrive("tok0", Path::new("/discs/a"));
        assert_eq!(drives.next(SHORT), arrived("tok0", "/discs/a"));
        let ejector = RecordingEjector::new().with_removals(script.clone());
        ejector.fail_next("stuck");
        assert_eq!(ejector.eject("tok0"), Err("stuck".to_string()));
        assert_eq!(drives.next(SHORT), None);
        assert_eq!(ejector.eject("tok0"), Ok(()));
        assert_eq!(drives.next(SHORT), Some(DriveEvent::Removed { drive: "tok0".to_string() }));
        assert!(script.present().is_empty());
    }

    #[test]
    fn keep_awake_counts_its_holders() {
        let awake = CountingKeepAwake::new();
        awake.acquire();
        awake.acquire();
        awake.release();
        assert_eq!(awake.holders(), 1);
        awake.release();
        awake.release();
        assert_eq!(awake.holders(), 0);
    }

    #[test]
    fn fake_shares_report_free_space_and_switchable_reach() {
        let shares = FakeShares::new();
        assert_eq!(shares.reach("/staging"), Reach::Reachable);
        assert_eq!(shares.free_bytes("/staging").unwrap(), u64::MAX);
        shares.set_free("/staging", 1_000);
        assert_eq!(shares.free_bytes("/staging").unwrap(), 1_000);
        assert_eq!(shares.free_bytes("/library").unwrap(), u64::MAX);
        shares.set_reach("/staging", Reach::Unreachable("cable out".to_string()));
        assert_eq!(shares.reach("/staging"), Reach::Unreachable("cable out".to_string()));
        assert_eq!(shares.free_bytes("/staging").unwrap_err().to_string(), "cable out");
        shares.set_reach("/staging", Reach::Reachable);
        assert_eq!(shares.free_bytes("/staging").unwrap(), 1_000);
    }

    #[test]
    fn roots_are_on_separate_volumes_until_joined() {
        let shares = FakeShares::new();
        assert_eq!(shares.volume("/staging"), Some("/staging".to_string()));
        assert_ne!(shares.volume("/staging"), shares.volume("/library"));
        shares.same_volume("/staging", "/library");
        assert_eq!(shares.volume("/library"), shares.volume("/staging"));
        assert_ne!(shares.volume("/tuning"), shares.volume("/staging"));
    }

    #[test]
    fn memory_secrets_round_trip() {
        let secrets = MemorySecrets::new().with("tmdb_token", "t0k");
        assert_eq!(secrets.get("tmdb_token"), Some("t0k".to_string()));
        assert_eq!(secrets.get("webhook_url"), None);
        secrets.set("webhook_url", "http://ha.local/api/webhook/x").unwrap();
        assert_eq!(secrets.get("webhook_url").as_deref(), Some("http://ha.local/api/webhook/x"));
    }

    #[test]
    fn memory_ipc_is_a_duplex_line_stream() {
        let (ipc, connector) = MemoryIpc::new();
        let mut client = connector.connect().unwrap();
        let mut server = ipc.accept().unwrap();
        client.writer.write_all(b"{\"v\":1}\n").unwrap();
        client.writer.flush().unwrap();
        let mut line = String::new();
        server.reader.read_line(&mut line).unwrap();
        assert_eq!(line, "{\"v\":1}\n");
        server.writer.write_all(b"ok\n").unwrap();
        line.clear();
        client.reader.read_line(&mut line).unwrap();
        assert_eq!(line, "ok\n");
        drop(client);
        line.clear();
        assert_eq!(server.reader.read_line(&mut line).unwrap(), 0);
        assert_eq!(server.writer.write_all(b"late\n").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        drop(connector);
        assert_eq!(ipc.accept().err().map(|e| e.kind()), Some(io::ErrorKind::BrokenPipe));
    }

    #[test]
    fn a_failing_connector_returns_its_error() {
        for error in [ConnectError::NotRunning, ConnectError::Refused, ConnectError::NotElevated, ConnectError::NoTransport] {
            assert_eq!(MemoryConnector::failing(error).connect().err(), Some(error));
        }
    }

    #[test]
    fn fake_host_reports_no_gui_no_key_and_the_fake_label() {
        let host = FakeHost::new("2026-10-23", 1_800_000_000);
        assert_eq!(host.now_unix(), 1_800_000_000);
        assert!(host.running_processes().is_empty());
        assert_eq!(host.makemkv_app_key(), None);
        assert_eq!(host.makemkv_debug_log(), None);
        assert_eq!(host.default_install_path(Tool::Makemkvcon), None);
        assert_eq!(host.volume_label(Path::new("/discs/a")), None);
        host.set_label(Path::new("/discs/a"), "SYNTHETIC_MOVIE");
        assert_eq!(host.volume_label(Path::new("/discs/a")).as_deref(), Some("SYNTHETIC_MOVIE"));
        host.set_running(vec!["makemkvcon64.exe".to_string()]);
        assert_eq!(host.running_processes(), ["makemkvcon64.exe"]);
        host.set_app_key("T-KEY");
        assert_eq!(host.makemkv_app_key().as_deref(), Some("T-KEY"));
        host.set_debug_log(Path::new("/home/MakeMKV_log.txt"));
        assert_eq!(host.makemkv_debug_log(), Some(PathBuf::from("/home/MakeMKV_log.txt")));
    }

    #[test]
    fn the_fake_host_gives_a_fixed_local_date() {
        let host = FakeHost::new("2026-10-23", 1_800_000_000);
        assert_eq!(host.local_date(), "2026-10-23");
        assert_eq!(host.local_date(), "2026-10-23");
    }

    struct NoProcesses;

    impl ProcessGuard for NoProcesses {
        fn spawn(&self, _: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "no processes in this test"))
        }
    }

    #[test]
    fn the_fakes_fill_a_platform_bundle() {
        let (drives, _script) = ScriptedDrives::new();
        let (ipc, _connector) = MemoryIpc::new();
        let platform = Platform { drives: Box::new(drives), ejector: Arc::new(RecordingEjector::new()),
            processes: Arc::new(NoProcesses), awake: Arc::new(CountingKeepAwake::new()), shares: Arc::new(FakeShares::new()),
            secrets: Arc::new(MemorySecrets::new()), ipc: Arc::new(ipc),
            host: Arc::new(FakeHost::new("2026-10-23", 1_800_000_000)) };
        assert_eq!(platform.host.local_date(), "2026-10-23");
        assert!(platform.drives.present().is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked --features fakes`
Expected: a compile error, because `ScriptedDrives`, `RecordingEjector`, `DriveEvent`, `Platform` and the other names are not found.

- [ ] **Step 3: Implement the traits in `lib.rs` and the fakes in `fakes.rs`**

Write every item of **Interfaces** with a `///` doc that names spec 4.3 and design part 4.2, or the ruling above, and ends `Def: T8. Use: T<m>.` Use:
- T22 and T25 for `DriveWatcher`;
- T23 for `Ejector`;
- T11 for `ProcessGuard`, `GuardedChild` and `KillHandle`;
- T22 for `KeepAwake`;
- T23 and T24 for `ShareConnector`;
- T26 for `SecretStore` and `IpcTransport`;
- T28 for `IpcConnector`;
- phase 5 for `Autostart`;
- T11 and T22 for `Host`;
- T22 for `Platform`;
- T22 to T29 for the fakes.

`Spawn.env` holds extra variables, which the engine leaves empty (S43). `KillHandle::kill` calls the closure. `Autostart` is declared only.

How each fake works:
- **`ScriptedDrives` and `DriveScript`** share one `Arc<(Mutex<(VecDeque<DriveEvent>, BTreeMap<String, PathBuf>)>, Condvar)>`. `next` waits on the condvar up to `timeout` (T8-b, T8-f).
- **`RecordingEjector`** holds the calls, a `fail_next` message and an optional `DriveScript`. A successful eject with a script calls `script.remove(drive)` (T8-c).
- **`CountingKeepAwake`** counts with saturating subtraction (T8-g).
- **`FakeShares`** keeps maps from root to free bytes, root to `Reach` and root to volume. An unreachable root's `free_bytes` is `Err(io::Error::new(ErrorKind::Other, <message>))` (T8-a).
- **`MemorySecrets`** is a `Mutex<BTreeMap<String, String>>`.
- **`MemoryIpc`** works as follows (T8-e):
  - `MemoryIpc` holds the receiving end of an `mpsc` channel of server-side `IpcStream`s. `MemoryConnector` holds `Option<Sender>`, which is None when built by `failing(error)`.
  - `connect` builds two byte channels. Each reader is a `BufReader` over a `Read` adapter that pulls `Vec<u8>` chunks and gives end of file when the channel closes. Each writer sends one chunk per `write`, and `flush` does nothing.
  - `connect` sends the server ends and returns the client ends.
- **`FakeHost`** keeps the fixed date and time, and a `Mutex` per settable value. It answers None or empty for anything not set.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked --features fakes`
Expected: PASS, `test result: ok. 13 passed; 0 failed`.

Run: `scripts/box.sh cargo build -p lazymkv-platform --locked`
Expected: it builds without the feature. The traits do not depend on `fakes`.

- [ ] **Step 5: Commit**

```
git add crates/platform/src/lib.rs crates/platform/src/fakes.rs
git commit -m "platform: the spec 4.3 traits with in-memory fakes (design part 4.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

### Task 9: The std process runner and the file writes (design parts 4.2, 6.5, 7.4)

**Files:**
- Create: `crates/platform/src/process.rs`, with 2 tests and 1 `#[ignore]` helper in its `mod tests`.
- Modify: `crates/platform/src/lib.rs`. Add:
  - `mod process; pub use process::StdProcessGuard;`;
  - `TEMP_SUFFIX`, `write_atomic` and `rename_no_replace`;
  - a `#[cfg(test)] pub(crate) mod testutil`;
  - 5 tests in `lib.rs`'s `mod tests`.

**Interfaces:**
- Consumes (Task 8): `Spawn { program, args, env, below_normal }`, `ProcessGuard`, `GuardedChild` and `KillHandle` with its `pub(crate)` field.
- Produces:

```rust
/// Def: T9. Use: T11, T13, T23.
pub struct StdProcessGuard;              // impl ProcessGuard
/// Def: T9. Use: T9, T21.
pub const TEMP_SUFFIX: &str = ".lazymkv-tmp";
/// Def: T9. Use: T21 to T26.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()>;
/// Def: T9. Use: T24.
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()>;
// test only, as capture's crates/capture/src/main.rs:79:
pub(crate) mod testutil { pub fn temp_dir(tag: &str) -> PathBuf; } // fresh `lazymkv-<tag>-<pid>-<nanos>` folder under std::env::temp_dir()
```

Rulings settled in this task:
- **Ruling T9-a:** the child sits in an `Arc<Mutex<std::process::Child>>`. `wait` polls `try_wait` every 20 ms and releases the lock between polls, so a `KillHandle` used from another thread can always take the lock.
- **Ruling T9-b:** `KillHandle::kill` after the child has exited returns `Ok(())`, with `ErrorKind::InvalidInput` from std mapped to `Ok`. Cancel and timeout races then never surface as errors.
- **Ruling T9-c:** `write_atomic` creates no folders. A missing parent gives `ErrorKind::NotFound`, because each caller owns its folder layout.
- **Ruling T9-d:** `take_stdout` and `take_stderr` return the pipe the first time and `io::empty()` after that.
- **Ruling T9-e:** `StdProcessGuard` spawns with stdin null, stdout and stderr piped, and `env` added to the inherited environment. `below_normal` does nothing in PR 2 (S13, S43).

- [ ] **Step 1: Write the failing tests**

In `crates/platform/src/process.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::{ProcessGuard, Spawn};

    fn helper(mode: &str) -> Spawn {
        Spawn {
            program: std::env::current_exe().unwrap(),
            args: ["--exact", "process::tests::helper_child", "--ignored", "--nocapture"].map(String::from).to_vec(),
            env: vec![("LAZYMKV_TEST_HELPER".to_string(), mode.to_string())],
            below_normal: false,
        }
    }

    /// The child the runner tests start: this test binary running only this test. It sleeps until killed when
    /// LAZYMKV_TEST_HELPER is `sleep`, prints one line to each stream when it is `echo`, and returns at once otherwise.
    #[test]
    #[ignore]
    fn helper_child() {
        match std::env::var("LAZYMKV_TEST_HELPER").as_deref() {
            Ok("sleep") => loop {
                std::thread::sleep(Duration::from_secs(60));
            },
            Ok("echo") => {
                std::io::stdout().write_all(b"helper-out\n").unwrap();
                std::io::stderr().write_all(b"helper-err\n").unwrap();
            }
            _ => {}
        }
    }

    #[test]
    fn the_runner_kills_its_child() {
        let mut child = StdProcessGuard.spawn(&helper("sleep")).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let killer = child.killer();
        killer.kill().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || tx.send(child.wait().map_err(|e| e.kind())).unwrap());
        let status = rx.recv_timeout(Duration::from_secs(10)).expect("the killed child did not end within 10 s");
        assert_ne!(status, Ok(Some(0)));
        assert!(killer.kill().is_ok(), "a kill after the exit is not an error");
    }

    #[test]
    fn the_runner_pipes_the_childs_output_and_passes_its_environment() {
        let mut child = StdProcessGuard.spawn(&helper("echo")).unwrap();
        let mut out = String::new();
        child.take_stdout().read_to_string(&mut out).unwrap();
        let mut err = String::new();
        child.take_stderr().read_to_string(&mut err).unwrap();
        assert_eq!(child.wait().unwrap(), Some(0));
        assert!(out.contains("helper-out\n"), "{out}");
        assert!(err.contains("helper-err\n"), "{err}");
        let mut again = String::new();
        child.take_stdout().read_to_string(&mut again).unwrap();
        assert_eq!(again, "");
    }
}
```

In `crates/platform/src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: a compile error, because `StdProcessGuard`, `write_atomic`, `rename_no_replace`, `TEMP_SUFFIX` and `testutil` are not found.

- [ ] **Step 3: Implement `StdProcessGuard`, `write_atomic` and `rename_no_replace`**

`process.rs` holds `pub struct StdProcessGuard;` and a private `StdChild { child: Arc<Mutex<Child>>, stdout: Option<ChildStdout>, stderr: Option<ChildStderr> }`. Its module doc names design part 4.2's Linux and macOS stand-in and S13, and says that PR 3's Job Object runner replaces it on Windows.
- `ProcessGuard::spawn` follows T9-e.
- `wait` follows T9-a and returns `ExitStatus::code()`, which is None for a signal.
- `killer` returns `KillHandle(Arc::new(move || ...))`, locking the child and calling `Child::kill`, with T9-b's mapping.
- `take_stdout` and `take_stderr` follow T9-d.

In `lib.rs`:
- `write_atomic(path, bytes)` works in five steps:
  1. The temporary path is `path`'s parent joined with the file name as an `OsString` followed by `TEMP_SUFFIX`. A path with no file name gives `ErrorKind::InvalidInput`.
  2. Remove a temporary file left there, ignoring `NotFound`.
  3. `File::create` the temporary path, `write_all` the bytes, then `sync_all`.
  4. `std::fs::rename` it over `path`.
  5. Leave the folders alone (T9-c).

  Its doc cites design part 7.4 and S13: PR 3 swaps in `FlushFileBuffers` with `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`.
- `rename_no_replace(from, to)` is `std::fs::hard_link(from, to)` then `std::fs::remove_file(from)`. An existing `to` makes `hard_link` fail with `ErrorKind::AlreadyExists`, and that error is returned unchanged. Its doc cites design part 6.5 and S13: PR 3 swaps in `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` or `MOVEFILE_COPY_ALLOWED`.
- `testutil::temp_dir` is a copy of capture's `crates/capture/src/main.rs:79-87`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: PASS, `test result: ok. 7 passed; 0 failed; 1 ignored`. The ignored test is `process::tests::helper_child`.

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked --features fakes`
Expected: PASS, `test result: ok. 20 passed; 0 failed; 1 ignored`.

- [ ] **Step 5: Commit**

```
git add crates/platform/src/lib.rs crates/platform/src/process.rs
git commit -m "platform: a std process runner, write_atomic and rename_no_replace (design parts 4.2, 6.5, 7.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

### Task 10: Stand-in extensions (design part 12.2)

**Files:**
- Modify: `crates/fake-tools/src/scenario.rs:10-41` (`Response`, `Create`, `select`). Add `Stream` and `schedule`, and tests `:56-97`. The bin's unit tests go from 2 to 7.
- Modify: `crates/fake-tools/src/main.rs:10-71` (`run`).
- Modify: `crates/fake-tools/tests/replay.rs`. It adds 9 tests (1 to 10), and the existing test passes unchanged.

**Interfaces:**
- Consumes:
  - `lazymkv_core::testsupport::{Scratch, ScratchExt}` (testsupport/mod.rs:373, :400), from the dev-dependency Task 1 added.
  - The existing `parse_timing(text: &str) -> Vec<u64>` (scenario.rs:49).
- Produces (inside the `fake-tool` binary; each item gets a `///` doc that names design part 12.2 and ends "Def: T10. Use: T11, T13, T14, T23."):

```rust
// crates/fake-tools/src/scenario.rs
pub struct Response {
    pub args_contain: Vec<String>, pub stdout: Option<String>, pub timing: Option<String>, pub exit_code: i32,
    pub creates: Vec<Create>,
    #[serde(default)] pub stderr: Option<String>,          // file in the scenario folder replayed to stderr
    #[serde(default)] pub stderr_timing: Option<String>,   // its {"n","t_ms"} lines
    #[serde(default)] pub stop_after_line: Option<usize>,  // print this many stdout lines, then exit with exit_code
    #[serde(default)] pub hang_after_line: Option<usize>,  // print this many stdout lines, then sleep until killed
}
pub struct Create {
    #[serde(default)] pub dir_arg: Option<usize>,
    #[serde(default)] pub dir_arg_from_end: Option<usize>, // 1 is the last argument
    #[serde(default)] pub arg_after: Option<String>,       // the argument after the first argument equal to this flag
    #[serde(default)] pub name: Option<String>,            // required with dir_arg and dir_arg_from_end; ignored with arg_after
    pub copy_from: String,
}
impl Create { pub fn target(&self, args: &[String]) -> Result<PathBuf, String>; }
pub fn select<'a>(scenario: &'a Scenario, args: &[String], calls: &mut BTreeMap<String, u64>) -> Option<&'a Response>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Stream { Out, Err }
pub fn schedule(out_ms: &[u64], out_lines: usize, err_ms: &[u64], err_lines: usize) -> Vec<(Stream, usize, u64)>;
```

The JSON keys a scenario may use after this task: `args_contain`, `stdout`, `timing`, `exit_code`, `creates[{dir_arg|dir_arg_from_end|arg_after, name, copy_from}]`, `stderr`, `stderr_timing`, `stop_after_line` and `hang_after_line`. The counter file is `<scenario dir>/<stem>.calls.json`, shaped like `{"0":3}`.

**Rulings:**
- Ruling: a line with no timing record takes the time of the line before it in its own stream (0 for the first). The streams are merged by time, with stdout first when two times are equal. This makes "interleaved by its own timing" deterministic when one stream has no timing file.
- Ruling: `stop_after_line` and `hang_after_line` count stdout lines only. Stderr lines scheduled after the cut are not printed. A value at or above the line count prints everything, then stops or hangs.
- Ruling: a response that stops never runs its `creates`, because it stands for a run that ended without completing.
- Ruling: one response may not set both `stop_after_line` and `hang_after_line`. The stand-in exits 2 if it does.
- Ruling: with `arg_after` the stand-in does not create the target's folder, and exits 2 if that folder is missing. This makes the stand-in behave like `mkvmerge -o`, so Review Focus 3 (the engine creates `.lazymkv-partial`) is tested for real. The directory forms keep `create_dir_all`, as today.
- Ruling: the counter file is written on every run that matches, even when only one response matches. One rule is simpler than two, and tests always run in copied folders.

- [ ] **Step 1: Write the failing unit tests in `scenario.rs`**

Update the existing test `the_first_response_whose_needles_all_appear_is_selected`:
- Pass `&mut calls` to every `select` call, from one `let mut calls = BTreeMap::new();`.
- Change `assert_eq!(rip.creates[0].dir_arg, 4)` to `assert_eq!(rip.creates[0].dir_arg, Some(4))`.

Add these tests:

```rust
#[test]
fn repeated_arguments_take_the_next_matching_response_then_the_last_repeats() {
    let scenario: Scenario = serde_json::from_str(
        r#"{"responses":[
            {"args_contain":["mkv"],"exit_code":1},
            {"args_contain":["info"],"exit_code":5},
            {"args_contain":["mkv","disc:0"],"exit_code":0,"stdout":"rip.txt"}
        ]}"#,
    )
    .unwrap();
    let mut calls = BTreeMap::new();
    let rip = args(&["-r", "mkv", "disc:0", "0", "out"]);
    let codes: Vec<i32> = (0..3).map(|_| select(&scenario, &rip, &mut calls).unwrap().exit_code).collect();
    assert_eq!(codes, vec![1, 0, 0]);
    assert_eq!(select(&scenario, &args(&["info", "disc:0"]), &mut calls).unwrap().exit_code, 5);
    assert_eq!(calls, BTreeMap::from([("0".to_string(), 3), ("1".to_string(), 1)]));
}

#[test]
fn each_locator_names_its_target() {
    let create = |json: &str| serde_json::from_str::<Create>(json).unwrap();
    let rip = args(&["-r", "mkv", "disc:0", "0", "out"]);
    assert_eq!(create(r#"{"dir_arg":4,"name":"t.mkv","copy_from":"s"}"#).target(&rip), Ok(Path::new("out").join("t.mkv")));
    assert_eq!(
        create(r#"{"dir_arg_from_end":1,"name":"t.mkv","copy_from":"s"}"#).target(&rip),
        Ok(Path::new("out").join("t.mkv"))
    );
    let remux = args(&["--gui-mode", "-o", "lib/x.mkv.partial", "raw.mkv"]);
    assert_eq!(
        create(r#"{"arg_after":"-o","name":"ignored","copy_from":"s"}"#).target(&remux),
        Ok(PathBuf::from("lib/x.mkv.partial"))
    );
}

#[test]
fn a_create_with_two_locators_or_none_or_no_name_is_refused() {
    let create = |json: &str| serde_json::from_str::<Create>(json).unwrap();
    let rip = args(&["-r", "mkv", "disc:0", "0", "out"]);
    let both = "creates: give exactly one of dir_arg, dir_arg_from_end and arg_after";
    assert_eq!(create(r#"{"dir_arg":4,"arg_after":"-o","name":"t","copy_from":"s"}"#).target(&rip), Err(both.to_string()));
    assert_eq!(create(r#"{"name":"t","copy_from":"s"}"#).target(&rip), Err(both.to_string()));
    assert_eq!(
        create(r#"{"dir_arg":4,"copy_from":"s"}"#).target(&rip),
        Err("creates: name is required with dir_arg and dir_arg_from_end".to_string())
    );
    assert_eq!(
        create(r#"{"dir_arg_from_end":9,"name":"t","copy_from":"s"}"#).target(&rip),
        Err("creates: there is no argument 9 from the end".to_string())
    );
    assert_eq!(
        create(r#"{"arg_after":"-o","copy_from":"s"}"#).target(&rip),
        Err("creates: there is no argument after -o".to_string())
    );
}

#[test]
fn stdout_and_stderr_lines_are_merged_by_their_times() {
    assert_eq!(
        schedule(&[0, 10, 30], 3, &[5, 30], 2),
        vec![(Stream::Out, 0, 0), (Stream::Err, 0, 5), (Stream::Out, 1, 10), (Stream::Out, 2, 30), (Stream::Err, 1, 30)]
    );
}

#[test]
fn a_line_without_timing_keeps_the_time_of_the_line_before_it() {
    assert_eq!(
        schedule(&[0, 20], 3, &[], 1),
        vec![(Stream::Out, 0, 0), (Stream::Err, 0, 0), (Stream::Out, 1, 20), (Stream::Out, 2, 20)]
    );
}
```

The tests need `use std::collections::BTreeMap;` and `use std::path::{Path, PathBuf};`.

- [ ] **Step 2: Write the failing integration tests in `tests/replay.rs`**

Leave `replays_output_creates_files_and_returns_the_exit_code` unchanged. Add the code below. The new tests rely on the stand-in reading the scenario beside its executable, so each one runs with `LAZYMKV_FAKE_DIR` removed.

```rust
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

const SAVED: &str = "MSG:5036,0,1,\"Copy complete. 1 titles saved.\"\n";

fn stand_in(dir: &Path, stem: &str) -> PathBuf {
    let tool = dir.join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();
    tool
}

fn run_in(tool: &Path, fake_dir: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(tool);
    command.args(args);
    match fake_dir {
        Some(dir) => command.env("LAZYMKV_FAKE_DIR", dir),
        None => command.env_remove("LAZYMKV_FAKE_DIR"),
    };
    command.output().unwrap()
}

const REMUX: &str = r#"{"responses":[{"args_contain":["--gui-mode","-o"],"stdout":"remux.txt",
    "creates":[{"arg_after":"-o","copy_from":"placeholder.mkv"}]}]}"#;

#[test]
fn the_file_after_dash_o_is_created() {
    let dir = Scratch::new("fake-o");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write("mkvmerge.json", REMUX);
    dir.write("remux.txt", "#GUI#progress 45%\n#GUI#progress 100%\n");
    dir.write("placeholder.mkv", b"0123456789abcdef");
    let partial = dir.0.join("lib/.lazymkv-partial/abc-1-t00-00800.mkv.partial");
    fs::create_dir_all(partial.parent().unwrap()).unwrap();

    let remux = run_in(&tool, None, &["--gui-mode", "-o", partial.to_str().unwrap(), "--language", "1:eng", "raw.mkv"]);

    assert_eq!(remux.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&remux.stdout), "#GUI#progress 45%\n#GUI#progress 100%\n");
    assert_eq!(fs::read(&partial).unwrap(), b"0123456789abcdef");
}

#[test]
fn the_file_after_dash_o_needs_its_folder_to_exist() {
    let dir = Scratch::new("fake-o-missing");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write("mkvmerge.json", REMUX);
    dir.write("remux.txt", "#GUI#progress 100%\n");
    dir.write("placeholder.mkv", b"0123456789abcdef");
    let partial = dir.0.join("lib/.lazymkv-partial/abc-1-t00-00800.mkv.partial");

    let remux = run_in(&tool, None, &["--gui-mode", "-o", partial.to_str().unwrap(), "raw.mkv"]);

    assert_eq!(remux.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&remux.stderr).contains("does not exist"));
    assert!(!partial.parent().unwrap().exists());
}

#[test]
fn a_directory_counted_from_the_end_survives_an_added_flag() {
    let dir = Scratch::new("fake-end");
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write(
        "makemkvcon64.json",
        r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt",
            "creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"sample.mkv"}]}]}"#,
    );
    dir.write("rip.txt", SAVED);
    dir.write("sample.mkv", b"sample");
    let (plain, debug) = (dir.0.join("plain"), dir.0.join("debug"));
    let rip = |extra: &[&str], out: &Path| {
        let mut args = vec!["-r", "--messages=-stdout", "--progress=-same"];
        args.extend_from_slice(extra);
        args.extend(["mkv", "disc:0", "0", out.to_str().unwrap()]);
        run_in(&tool, None, &args)
    };

    assert_eq!(rip(&[], &plain).status.code(), Some(0));
    assert_eq!(rip(&["--debug"], &debug).status.code(), Some(0));
    assert_eq!(fs::read(plain.join("title_t00.mkv")).unwrap(), b"sample");
    assert_eq!(fs::read(debug.join("title_t00.mkv")).unwrap(), b"sample");
}

#[test]
fn stderr_is_replayed_from_its_file() {
    let dir = Scratch::new("fake-stderr");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write(
        "mkvmerge.json",
        r#"{"responses":[{"args_contain":["-J"],"stdout":"ident.json","stderr":"warn.txt",
            "stderr_timing":"warn.txt.timing.jsonl","exit_code":1}]}"#,
    );
    dir.write("ident.json", "{\"tracks\":[]}\n");
    dir.write("warn.txt", "Warning: x\nWarning: y\n");
    dir.write("warn.txt.timing.jsonl", "{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":9}\n");

    let ident = run_in(&tool, None, &["-J", "a.mkv"]);

    assert_eq!(ident.status.code(), Some(1));
    assert_eq!(ident.stdout, b"{\"tracks\":[]}\n");
    assert_eq!(ident.stderr, b"Warning: x\nWarning: y\n");
}

#[test]
fn the_same_arguments_get_the_next_response_by_a_counter_in_the_scenario_folder() {
    let dir = Scratch::new("fake-count");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write("mkvmerge.json", r#"{"responses":[{"args_contain":["-J"],"exit_code":1},{"args_contain":["-J"],"exit_code":0}]}"#);

    let codes: Vec<_> = (0..3).map(|_| run_in(&tool, None, &["-J", "title_t00.mkv"]).status.code()).collect();

    assert_eq!(codes, [Some(1), Some(0), Some(0)]);
    let calls: serde_json::Value = serde_json::from_slice(&fs::read(dir.0.join("mkvmerge.calls.json")).unwrap()).unwrap();
    assert_eq!(calls, serde_json::json!({"0": 3}));
}

#[test]
fn a_hanging_run_sleeps_until_killed() {
    let dir = Scratch::new("fake-hang");
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["mkv"],"stdout":"rip.txt","hang_after_line":1}]}"#);
    dir.write("rip.txt", format!("MSG:5015,0,0,\"Saving 1 titles\"\n{SAVED}"));
    let mut child = Command::new(&tool)
        .args(["-r", "mkv", "disc:0", "0", "out"])
        .env_remove("LAZYMKV_FAKE_DIR")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));
    assert!(child.try_wait().unwrap().is_none(), "the stand-in exited before it was killed");
    child.kill().unwrap();
    child.wait().unwrap();

    let mut printed = String::new();
    child.stdout.take().unwrap().read_to_string(&mut printed).unwrap();
    assert_eq!(printed, "MSG:5015,0,0,\"Saving 1 titles\"\n");
}

#[test]
fn output_stops_at_the_given_line_and_exits_with_the_given_code() {
    let dir = Scratch::new("fake-stop");
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write(
        "makemkvcon64.json",
        r#"{"responses":[{"args_contain":["mkv"],"stdout":"rip.txt","stop_after_line":2,"exit_code":4,
            "creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"sample.mkv"}]}]}"#,
    );
    dir.write("rip.txt", format!("MSG:5015,0,0,\"Saving 1 titles\"\nPRGV:10,32768,65536\n{SAVED}"));
    dir.write("sample.mkv", b"sample");
    let out = dir.0.join("out");

    let rip = run_in(&tool, None, &["-r", "mkv", "disc:0", "0", out.to_str().unwrap()]);

    assert_eq!(rip.status.code(), Some(4));
    assert_eq!(String::from_utf8_lossy(&rip.stdout), "MSG:5015,0,0,\"Saving 1 titles\"\nPRGV:10,32768,65536\n");
    assert!(!out.join("title_t00.mkv").exists());
}

#[test]
fn without_lazymkv_fake_dir_the_scenario_beside_the_executable_is_used() {
    let (dir, other) = (Scratch::new("fake-beside"), Scratch::new("fake-env"));
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["info"],"exit_code":3}]}"#);
    other.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["info"],"exit_code":5}]}"#);

    assert_eq!(run_in(&tool, None, &["-r", "--cache=1", "info", "disc:9999"]).status.code(), Some(3));
    assert_eq!(run_in(&tool, Some(&other.0), &["-r", "--cache=1", "info", "disc:9999"]).status.code(), Some(5));
}

#[test]
fn the_ffmpeg_stem_selects_ffmpeg_json() {
    let dir = Scratch::new("fake-ffmpeg");
    let tool = stand_in(&dir.0, "ffmpeg");
    dir.write("ffmpeg.json", r#"{"responses":[{"args_contain":["-f","streamhash"],"stdout":"hash.txt"}]}"#);
    dir.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["-f"],"exit_code":9}]}"#);
    dir.write("hash.txt", "0,a,MD5=00\n");

    let hashed = run_in(&tool, None, &["-i", "raw.mkv", "-map", "0:1", "-f", "streamhash", "-hash", "md5", "-"]);

    assert_eq!(hashed.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&hashed.stdout), "0,a,MD5=00\n");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools --locked`
Expected: compile errors in the `fake-tool` unit tests. `select` takes 2 arguments, `schedule`, `Stream` and `Create::target` are not found, and `dir_arg` has type `usize`.

- [ ] **Step 4: Implement `scenario.rs`**

Change `Response` and `Create` to the shapes above. `Create` keeps its existing `#[derive(Debug, Deserialize)]`.

`Create::target(&self, args: &[String]) -> Result<PathBuf, String>`:
- If the number of set locators is not exactly one, return the "give exactly one" error from the tests.
- `dir_arg: Some(i)` gives `args[i]` joined with `name`. A missing argument keeps today's text, `creates: there is no argument {i}`.
- `dir_arg_from_end: Some(k)` gives `args[args.len() - k]` joined with `name`. A `k` of 0, or one above `args.len()`, gives `creates: there is no argument {k} from the end`.
- `arg_after: Some(flag)` gives the argument after the first argument equal to `flag`. If there is none, the error is `creates: there is no argument after {flag}`.
- The two directory forms need `name`. Without it they return `creates: name is required with dir_arg and dir_arg_from_end`.

`select(scenario, args, calls)`:
1. `matching` is every response, in file order, whose needles all appear in the arguments, as today. If it is empty, return `None` and leave `calls` unchanged.
2. `key` is the file index of `matching[0]`, as a decimal string.
3. `n = calls[key]`, or 0 when absent. Return `matching[min(n, matching.len() - 1)]` and store `n + 1`.

`schedule(out_ms, out_lines, err_ms, err_lines)`:
- Each line's time is `ms[i]`. When `ms[i]` is absent, it is the time of line `i - 1` in the same stream, or 0 for the first line.
- The result is both streams' `(stream, index, t_ms)` items in one list, stably sorted by `t_ms`, with stdout before stderr at equal times.

- [ ] **Step 5: Implement `main.rs`**

`run()` keeps its signature and its exit 2 on any `Err`. In order:

1. The scenario folder is `LAZYMKV_FAKE_DIR` when that is set, else `exe.parent()`.
2. Read `<stem>.json`. Read `<stem>.calls.json` as `BTreeMap<String, u64>`; a missing file is an empty map. Call `select`, then write the map back with `serde_json::to_vec`. A write error returns `Err("<path>: <error>")`.
3. If both `stop_after_line` and `hang_after_line` are set, return `Err("a response may not have both stop_after_line and hang_after_line")`.
4. Read the stdout and stderr files, split each with `split_inclusive(b'\n')`, and read both timing files with `parse_timing`. Walk `schedule(..)`. Before writing the stdout line whose index equals the cut:
   - for a stop, return `Ok(exit_code)` without running `creates`;
   - for a hang, flush both streams and `loop { thread::sleep(Duration::from_secs(3600)) }`.
5. Each line waits until `t_ms * LAZYMKV_FAKE_SPEED` (as today), is written to its stream, and is flushed.
6. If the walk ends with the cut not yet reached, stop or hang the same way.
7. For each `creates` entry, `target(&args)?`.
   - The directory forms run `create_dir_all` on the target's parent.
   - `arg_after` checks that the parent exists, and otherwise returns `Err("creates: {parent} does not exist")`.
   - Then copy `copy_from` to the target.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p fake-tools --locked`
Expected: PASS. The `fake-tool` unit tests run 7 passed, and `replay` runs 10 passed, the existing test unchanged among them.

- [ ] **Step 7: Commit**

```
git add crates/fake-tools/src/scenario.rs crates/fake-tools/src/main.rs crates/fake-tools/tests/replay.rs
git commit -m "fake-tools: -o outputs, stderr, several responses, hang, early stop, scenario beside the executable (design part 12.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>` and no attribution line.

---

### Task 11: Recorded runs, limits and tool lookup (design parts 3.6, 5.3, 7.2)

**Files:**
- Create: `crates/tools/src/record.rs`, with 2 unit tests.
- Create: `crates/tools/src/lookup.rs`, with 5 unit tests.
- Create: `crates/fake-tools/tests/tools_record.rs`, with 6 tests.
- Modify: `crates/tools/src/lib.rs` (created in Task 1). It gains the modules, the re-exports and a `#[cfg(test)] pub(crate) mod testutil`.
- Modify: `crates/capture/src/record.rs:3-60`. `Clock` (:9), `RealClock` (:13-25), `tee_lines` (:29-54) and `with_suffix` (:56-60) move to tools, and capture imports them. Capture's `run_recorded` (:62-98) and its 3 tests (:128-176) stay unchanged.

**Interfaces:**
- Consumes:
  - `lazymkv_platform::{ProcessGuard, GuardedChild, KillHandle, Spawn, Host}` (Task 8) and `lazymkv_platform::StdProcessGuard` (Task 9).
  - `lazymkv_platform::fakes::FakeHost::new(local_date: &str, now_unix: u64)` (Task 8).
  - `lazymkv_core::ipc::OutputStream { Stdout, Stderr }` (Task 7).
  - `lazymkv_core::settings::{Settings, ToolPaths}` (settings.rs:55, `scan_timeout_min` :122, `exit_wait_min` :128).
  - `lazymkv_core::job::Tool { Makemkvcon, Mkvmerge, Ffprobe, Ffmpeg, Java }` (job.rs:138).
  - Task 10's `hang_after_line` and the scenario beside the executable.
- Produces. Each item gets a `///` doc naming design part 5.3, 7.2 or 3.6, ending "Def: T11. Use: T13, T14, T23." (`find_tool`, `Found` and `ToolLocation` use "Use: T23, T26."). Everything is re-exported from `lib.rs`: `pub use record::{run_recorded, tee_lines, with_suffix, CancelToken, Clock, LineAction, Limits, RealClock, RunResult, ToolCtx}; pub use lookup::{exe_name, find_tool, Found, ToolLocation};`

```rust
// crates/tools/src/record.rs
pub trait Clock { fn elapsed_ms(&self) -> u64; }                    // moved from capture record.rs:9
pub struct RealClock(Instant); impl RealClock { pub fn start() -> Self; }
pub fn tee_lines(reader: &mut dyn BufRead, out: &mut dyn Write, timing: &mut dyn Write, echo: &mut dyn Write,
    clock: &dyn Clock) -> io::Result<u64>;                            // moved unchanged
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf;            // moved unchanged
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits { pub header: Duration, pub scan: Duration, pub exit_wait: Duration, pub after_kill: Duration }
impl Limits { pub fn from_settings(settings: &Settings) -> Limits; }
#[derive(Clone, Default)] pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken { pub fn cancel(&self); pub fn is_cancelled(&self) -> bool; }
pub struct ToolCtx<'a> { pub guard: &'a dyn ProcessGuard, pub host: &'a dyn Host, pub limits: &'a Limits,
    pub cancel: CancelToken, pub on_output: &'a (dyn Fn(OutputStream, &str) + Sync), pub on_progress: &'a (dyn Fn(f64) + Sync) }
#[derive(Clone, Debug)]
pub struct RunResult { pub exit_code: Option<i32>, pub lines: u64, pub elapsed_ms: u64, pub started_unix: u64,
    pub timed_out: bool, pub cancelled: bool, pub stdout: Vec<u8> }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum LineAction { Continue, Kill }
pub fn run_recorded(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path, limit: Option<Duration>, below_normal: bool,
    on_line: &mut dyn FnMut(&str) -> LineAction) -> io::Result<RunResult>;

// crates/tools/src/lookup.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)] #[serde(rename_all = "snake_case")]
pub enum Found { Settings, Resources, InstallFolder, Path }
#[derive(Clone, Debug, PartialEq, Eq)] pub struct ToolLocation { pub tool: Tool, pub path: PathBuf, pub found: Found }
pub fn exe_name(tool: Tool) -> String;
pub fn find_tool(tool: Tool, settings: &Settings, resources: Option<&Path>, host: &dyn Host, path_var: Option<&OsStr>) -> Option<ToolLocation>;

// crates/tools/src/lib.rs
#[cfg(test)] pub(crate) mod testutil { pub fn temp_dir(tag: &str) -> PathBuf; } // fresh folder: pid + an AtomicUsize count
```

**Rulings:**
- Ruling: the line given to `on_line` and `on_output` is the line with its trailing `\n` and a `\r` before it removed, decoded with `String::from_utf8_lossy` (PIPE-14). The file on disk keeps every byte.
- Ruling: `<name>.exit.json` writes `-1` when the process has no exit code. This is capture's `unwrap_or(-1)` (capture record.rs:87), so every job folder keeps phase 0's integer shape.
- Ruling: `run_recorded` spawns before it creates any run file. A program that cannot start returns `Err` and leaves no `<name>` files, so a missing tool never looks like a run.
- Ruling: `find_tool` checks `is_file()` on the resources, install-folder and `PATH` candidates, and takes the settings path as given (design part 3.6).
- Ruling: tools has no dev-dependencies (Task 1), so its unit tests use `testutil::temp_dir` and a local `Host` implementation. Tests that need the stand-in or `FakeHost` live in `crates/fake-tools/tests/` (S12).

- [ ] **Step 1: Write the failing unit tests in `crates/tools/src/record.rs`**

First add `mod record;` to `crates/tools/src/lib.rs`, so the file compiles with its tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_from_settings_are_120s_30min_30min_10s() {
        assert_eq!(
            Limits::from_settings(&Settings::default()),
            Limits {
                header: Duration::from_secs(120),
                scan: Duration::from_secs(30 * 60),
                exit_wait: Duration::from_secs(30 * 60),
                after_kill: Duration::from_secs(10),
            }
        );
        let short = Settings { scan_timeout_min: 2, exit_wait_min: 3, ..Settings::default() };
        assert_eq!(Limits::from_settings(&short).scan, Duration::from_secs(120));
        assert_eq!(Limits::from_settings(&short).exit_wait, Duration::from_secs(180));
    }

    #[test]
    fn a_cancel_token_is_shared_by_its_clones() {
        let token = CancelToken::default();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}
```

- [ ] **Step 2: Write the failing unit tests in `crates/tools/src/lookup.rs`**

First add `mod lookup;` to `crates/tools/src/lib.rs`, and the test helper `#[cfg(test)] pub(crate) mod testutil` with `temp_dir(tag)`: `std::env::temp_dir().join(format!("lazymkv-tools-{tag}-{pid}-{n}"))`, where `n` comes from a static `AtomicUsize`; it removes any old folder of that name, then creates it.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use lazymkv_core::settings::ToolPaths;
    use std::fs;

    struct TestHost(Option<PathBuf>);

    impl Host for TestHost {
        fn now_unix(&self) -> u64 { 0 }
        fn local_date(&self) -> String { String::new() }
        fn makemkv_app_key(&self) -> Option<String> { None }
        fn makemkv_debug_log(&self) -> Option<PathBuf> { None }
        fn running_processes(&self) -> Vec<String> { Vec::new() }
        fn volume_label(&self, _: &Path) -> Option<String> { None }
        fn default_install_path(&self, _: Tool) -> Option<PathBuf> { self.0.clone() }
    }

    fn named(path: &str) -> Settings {
        Settings { tool_paths: ToolPaths { mkvmerge: Some(path.to_string()), ..ToolPaths::default() }, ..Settings::default() }
    }

    #[test]
    fn exe_names_carry_the_platform_suffix() {
        let suffix = std::env::consts::EXE_SUFFIX;
        assert_eq!(exe_name(Tool::Makemkvcon), format!("makemkvcon64{suffix}"));
        assert_eq!(exe_name(Tool::Mkvmerge), format!("mkvmerge{suffix}"));
        assert_eq!(exe_name(Tool::Ffprobe), format!("ffprobe{suffix}"));
        assert_eq!(exe_name(Tool::Ffmpeg), format!("ffmpeg{suffix}"));
        assert_eq!(exe_name(Tool::Java), format!("java{suffix}"));
    }

    #[test]
    fn the_settings_path_is_taken_as_given() {
        let found = find_tool(Tool::Mkvmerge, &named(r"C:\Tools\mkvmerge.exe"), None, &TestHost(None), None);
        assert_eq!(
            found,
            Some(ToolLocation { tool: Tool::Mkvmerge, path: PathBuf::from(r"C:\Tools\mkvmerge.exe"), found: Found::Settings })
        );
    }

    #[test]
    fn the_lookup_order_is_settings_resources_install_folder_path() {
        let root = temp_dir("lookup-order");
        let place = |folder: &str| {
            let path = root.join(folder).join(exe_name(Tool::Mkvmerge));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"x").unwrap();
            path
        };
        let (resources, install, on_path) = (place("resources"), place("install"), place("path"));
        let path_var = std::env::join_paths([root.join("empty"), root.join("path")]).unwrap();
        let find = |settings: &Settings, resources: Option<&Path>, host: &TestHost| {
            find_tool(Tool::Mkvmerge, settings, resources, host, Some(path_var.as_os_str())).map(|l| (l.path, l.found))
        };
        let host = TestHost(Some(install.clone()));
        let default = Settings::default();

        assert_eq!(
            find(&named("given.exe"), Some(&root.join("resources")), &host),
            Some((PathBuf::from("given.exe"), Found::Settings))
        );
        assert_eq!(find(&default, Some(&root.join("resources")), &host), Some((resources, Found::Resources)));
        assert_eq!(find(&default, None, &host), Some((install, Found::InstallFolder)));
        assert_eq!(find(&default, None, &TestHost(None)), Some((on_path, Found::Path)));
    }

    #[test]
    fn a_missing_tool_is_none() {
        let root = temp_dir("lookup-missing");
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("resources").join(exe_name(Tool::Ffprobe)), b"x").unwrap();
        let path_var = std::env::join_paths([root.join("path")]).unwrap();
        let host = TestHost(Some(root.join("nowhere").join(exe_name(Tool::Mkvmerge))));

        let found =
            find_tool(Tool::Mkvmerge, &Settings::default(), Some(&root.join("resources")), &host, Some(path_var.as_os_str()));

        assert_eq!(found, None);
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), None, &TestHost(None), None), None);
    }

    #[test]
    fn found_serialises_in_snake_case() {
        assert_eq!(serde_json::to_string(&Found::InstallFolder).unwrap(), "\"install_folder\"");
        assert_eq!(serde_json::to_string(&Found::Path).unwrap(), "\"path\"");
    }
}
```

- [ ] **Step 3: Write the failing stand-in tests in `crates/fake-tools/tests/tools_record.rs`**

```rust
//! tools::run_recorded against the stand-in (design parts 5.3, 7.2). The scenario sits beside the copied stand-in,
//! so no environment variable is passed (S15).

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::settings::Settings;
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{run_recorded, with_suffix, CancelToken, LineAction, Limits, RunResult, ToolCtx};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const NOW: u64 = 1_790_000_000;

fn stand_in(dir: &Scratch, scenario: &str) -> PathBuf {
    let tool = dir.0.join(format!("makemkvcon64{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();
    dir.write("makemkvcon64.json", scenario);
    tool
}

type Seen = Vec<(&'static str, String)>;

fn run(
    tool: &Path,
    out: &Path,
    limit: Option<Duration>,
    cancel: CancelToken,
    on_line: &mut dyn FnMut(&str) -> LineAction,
) -> io::Result<(RunResult, Seen)> {
    let guard = StdProcessGuard;
    let host = FakeHost::new("2026-10-23", NOW);
    let limits = Limits::from_settings(&Settings::default());
    let seen = Mutex::new(Vec::new());
    let on_output = |stream: OutputStream, line: &str| {
        let name = if matches!(stream, OutputStream::Stdout) { "stdout" } else { "stderr" };
        seen.lock().unwrap().push((name, line.to_string()));
    };
    let ctx = ToolCtx { guard: &guard, host: &host, limits: &limits, cancel, on_output: &on_output, on_progress: &|_: f64| {} };
    let result = run_recorded(&ctx, tool, &["info".to_string(), "disc:0".to_string()], out, limit, false, on_line)?;
    Ok((result, seen.into_inner().unwrap()))
}

#[test]
fn a_run_writes_stdout_stderr_timing_and_exit_files() {
    let dir = Scratch::new("rec-files");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","stderr":"err.txt","exit_code":3}]}"#);
    dir.write("out.txt", "MSG:1005,0,1,\"MakeMKV v2.0.0\"\r\nTCOUNT:0\n");
    dir.write("err.txt", "warn\n");
    let out = dir.0.join("job/scan.txt");
    let mut lines = Vec::new();

    let (result, seen) =
        run(&tool, &out, None, CancelToken::default(), &mut |line| { lines.push(line.to_string()); LineAction::Continue }).unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"MSG:1005,0,1,\"MakeMKV v2.0.0\"\r\nTCOUNT:0\n");
    assert_eq!(result.stdout, fs::read(&out).unwrap());
    assert_eq!(fs::read(with_suffix(&out, ".stderr")).unwrap(), b"warn\n");
    let timing: Vec<serde_json::Value> = fs::read_to_string(with_suffix(&out, ".timing.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(timing.len(), 2);
    assert_eq!(timing[1]["n"], 1);
    assert!(timing[1]["t_ms"].is_u64());
    let exit: serde_json::Value = serde_json::from_slice(&fs::read(with_suffix(&out, ".exit.json")).unwrap()).unwrap();
    assert_eq!(exit.as_object().unwrap().keys().collect::<Vec<_>>(), ["exit_code", "elapsed_ms", "lines"]);
    assert_eq!((exit["exit_code"].clone(), exit["lines"].clone()), (serde_json::json!(3), serde_json::json!(2)));
    assert_eq!(
        (result.exit_code, result.lines, result.started_unix, result.timed_out, result.cancelled),
        (Some(3), 2, NOW, false, false)
    );
    assert_eq!(lines, ["MSG:1005,0,1,\"MakeMKV v2.0.0\"", "TCOUNT:0"]);
    assert!(seen.contains(&("stdout", "TCOUNT:0".to_string())));
    assert!(seen.contains(&("stderr", "warn".to_string())));
}

#[test]
fn invalid_utf8_reaches_the_file_unchanged_and_the_callback_replaced() {
    let dir = Scratch::new("rec-utf8");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt"}]}"#);
    dir.write("out.txt", b"caf\xe9\n");
    let out = dir.0.join("scan.txt");
    let mut lines = Vec::new();

    let (result, seen) =
        run(&tool, &out, None, CancelToken::default(), &mut |line| { lines.push(line.to_string()); LineAction::Continue }).unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"caf\xe9\n");
    assert_eq!(result.stdout, b"caf\xe9\n");
    assert_eq!(lines, ["caf\u{fffd}"]);
    assert_eq!(seen, [("stdout", "caf\u{fffd}".to_string())]);
}

#[test]
fn a_header_call_past_its_limit_is_killed_and_reported_timed_out() {
    let dir = Scratch::new("rec-limit");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":0}]}"#);
    dir.write("out.txt", "never\n");
    let out = dir.0.join("mkvmerge.json");

    let (result, _) =
        run(&tool, &out, Some(Duration::from_millis(200)), CancelToken::default(), &mut |_| LineAction::Continue).unwrap();

    assert!(result.timed_out);
    assert!(!result.cancelled);
    assert_eq!(result.lines, 0);
    assert!(result.elapsed_ms >= 200 && result.elapsed_ms < 10_000, "{}", result.elapsed_ms);
    assert_ne!(result.exit_code, Some(0));
    assert!(with_suffix(&out, ".exit.json").exists());
}

#[test]
fn a_cancelled_run_is_killed() {
    let dir = Scratch::new("rec-cancel");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":1}]}"#);
    dir.write("out.txt", "MSG:5015,0,0,\"Saving 1 titles\"\nTCOUNT:0\n");
    let token = CancelToken::default();
    let trigger = token.clone();

    let (result, _) =
        run(&tool, &dir.0.join("rip.txt"), None, token, &mut |_| { trigger.cancel(); LineAction::Continue }).unwrap();

    assert!(result.cancelled);
    assert!(!result.timed_out);
    assert_eq!(result.lines, 1);
    assert!(result.elapsed_ms < 10_000, "{}", result.elapsed_ms);
}

#[test]
fn a_line_action_kill_stops_the_run() {
    let dir = Scratch::new("rec-kill");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":1}]}"#);
    dir.write("out.txt", "MSG:3307,0,0,\"x\",\"00800.mpls\",\"1\"\nMSG:5015,0,0,\"Saving 1 titles\"\n");
    let mut calls = 0;

    let (result, _) =
        run(&tool, &dir.0.join("rip.txt"), None, CancelToken::default(), &mut |_| { calls += 1; LineAction::Kill }).unwrap();

    assert_eq!(calls, 1);
    assert_eq!(result.lines, 1);
    assert!(!result.cancelled && !result.timed_out);
    assert_ne!(result.exit_code, Some(0));
    assert!(result.elapsed_ms < 10_000, "{}", result.elapsed_ms);
}

#[test]
fn a_program_that_cannot_start_is_an_error_and_writes_no_files() {
    let dir = Scratch::new("rec-missing");
    let out = dir.0.join("scan.txt");

    let outcome = run(&dir.0.join("no-such-tool"), &out, None, CancelToken::default(), &mut |_| LineAction::Continue);

    assert!(outcome.is_err());
    assert!(!out.exists());
    assert!(!with_suffix(&out, ".exit.json").exists());
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p fake-tools -p lazymkv-capture --locked`
Expected: compile errors. In `lazymkv-tools`, the unit tests of `record.rs` and `lookup.rs` name items not yet written (for example `cannot find function run_recorded`). In `tools_record.rs`, `Limits`, `CancelToken`, `find_tool`, `exe_name`, `Found`, `ToolLocation` and `run_recorded` are unresolved imports of `lazymkv_tools`.

- [ ] **Step 5: Implement `record.rs`, `lookup.rs` and `lib.rs`**

**`record.rs`: the moved pieces.**
- Move `Clock`, `RealClock`, `tee_lines` and `with_suffix` byte for byte from capture record.rs:9-60.
- Add the private `fn tee_each(reader: &mut dyn BufRead, out: &mut dyn Write, timing: &mut dyn Write, clock: &dyn Clock, each: &mut dyn FnMut(&[u8])) -> io::Result<u64>`. It holds today's `tee_lines` loop, with `each(&line)` in place of the echo writes.
- `tee_lines` then calls `tee_each` with a closure that writes and flushes `echo` and ignores the errors. There is one recorder (S17).

**`record.rs`: `Limits` and `CancelToken`.**
- `Limits::from_settings`: `header` 120 s, `scan` is `scan_timeout_min * 60` s, `exit_wait` is `exit_wait_min * 60` s, and `after_kill` 10 s (design part 5.3).
- `CancelToken::cancel` stores `true`, and `is_cancelled` loads it, both with `SeqCst`.

**`record.rs`: `run_recorded`.** Its body follows this order:
1. Create the parent folder of `out`.
2. Take `started_unix = ctx.host.now_unix()` and `clock = RealClock::start()`.
3. Spawn with `ctx.guard.spawn(&Spawn { program: program.to_path_buf(), args: args.to_vec(), env: Vec::new(), below_normal })?`. A spawn error returns before any file is written.
4. Create `<out>`, `<out>.timing.jsonl` and `<out>.stderr`. Take stdout, stderr and `killer()`.
5. Inside `std::thread::scope`:
   - A stderr thread copies each stderr line's bytes to `<out>.stderr` and calls `on_output(OutputStream::Stderr, line)`.
   - A watchdog thread checks every 20 ms until `done` is set:
     - if `limit` has passed since spawn, it sets `timed_out` and kills;
     - otherwise, if `ctx.cancel.is_cancelled()`, it sets `cancelled` and kills.

     Only the first reason is recorded.
   - The calling thread runs `tee_each` over stdout. For each line it appends to `RunResult.stdout`, calls `on_output(OutputStream::Stdout, line)`, and calls `on_line(line)`. A `LineAction::Kill` kills the child (the kill error is ignored), and reading continues to end of file.
   - Then `wait()`, then set `done`.
6. Write `<out>.exit.json` as `{"exit_code":<code or -1>,"elapsed_ms":<clock>,"lines":<n>}\n`, the format string of capture record.rs:90-95.
7. Return `RunResult`.

**`lookup.rs`.**
- `exe_name` returns `makemkvcon64`, `mkvmerge`, `ffprobe`, `ffmpeg` or `java`, followed by `std::env::consts::EXE_SUFFIX`.
- `find_tool` tries these sources in order and returns the first hit:
  1. The `settings.tool_paths` field for `tool`, when it is `Some`, taken as given: `Found::Settings`.
  2. `resources/<exe_name>` when it is a file: `Found::Resources`.
  3. `host.default_install_path(tool)` when it is a file: `Found::InstallFolder`.
  4. The first `std::env::split_paths(path_var)` folder holding `<exe_name>` as a file: `Found::Path`.

  The writable-folder skip is PR 3 (S18).

**`lib.rs`.**
- Add the two `pub use` lines from Interfaces (Steps 1 and 2 declared the modules and `testutil`).

- [ ] **Step 6: Switch capture to the moved pieces**

- In `crates/capture/src/record.rs`, replace lines 3-60 with these imports:
  - `use lazymkv_tools::{tee_lines, with_suffix, Clock, RealClock};`
  - `use std::fs::File;`
  - `use std::io::{self, BufReader};`
  - `use std::path::Path;`
  - `use std::process::{Command, Stdio};`
- Keep `run_recorded` (:62-98) and the tests (:100-177) as they are. `use super::*` still brings in `tee_lines`, `Clock` and `with_suffix`.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p fake-tools -p lazymkv-capture --locked`
Expected: PASS.
- `lazymkv-tools` unit tests: 7 passed.
- `tools_record`: 6 passed.
- `replay` and the `fake-tool` unit tests: unchanged from Task 10.
- `lazymkv-capture`: `tee_copies_lines_and_records_their_timing`, `run_recorded_captures_output_and_the_exit_code`, `tee_keeps_recording_when_the_echo_reader_is_gone`, `record_exits_with_the_childs_exit_code` and Task 3's `snapshot_output_is_pinned_byte_for_byte` all pass unchanged.

- [ ] **Step 8: Commit**

```
git add crates/tools/src/lib.rs crates/tools/src/record.rs crates/tools/src/lookup.rs crates/fake-tools/tests/tools_record.rs crates/capture/src/record.rs
git commit -m "tools: capture-style recorded runs under ProcessGuard, limits and tool lookup (design parts 3.6, 5.3, 7.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` twice, and no attribution line.

---

### Task 12: The snapshot's file I/O in tools; capture switched (R-B2, part 2)

**Files:**
- Create: `crates/tools/src/snapshot.rs`, with 4 unit tests.
- Modify: `crates/tools/src/lib.rs` (`mod snapshot; pub use snapshot::{list_files, snapshot};`).
- Delete: `crates/capture/src/disc.rs`.
  - Its I/O (`list_files` :28, `walk` :35 and the file half of `snapshot` :73-112) moves to `tools::snapshot`.
  - Its pure part (`is_nav_file` :57, `find_unit_key` :66 and the `Meta` build :95-109) has been in `core::snapshot` since Task 3. `Meta` (:17) is replaced by `core::snapshot::DiscMeta`.
  - Its two tests go too: `navigation_files_are_recognised` (:129) has lived in core since Task 3, and `snapshot_lists_fingerprints_and_copies_navigation_files` (:141) moves to tools below.
- Modify: `crates/capture/src/main.rs:1` (drop `mod disc;`) and `:26-35` (the `disc` arm).
- Test: `crates/capture/tests/disc_snapshot.rs` (Task 3), unchanged.

Ruling: `disc.rs` is deleted rather than left holding unused types, because every item in it has moved to core or tools. `eject.rs` stays in capture (design part 4.2).

**Interfaces:**
- Consumes:
  - `lazymkv_core::snapshot::{DiscMeta, disc_meta, is_nav_file, unit_key_file}` (Task 3). `disc_meta` has the signature `pub fn disc_meta(files: &[DiscFile], volume_label: &str, unit_key: Option<&[u8]>, captured_unix: u64) -> DiscMeta`.
  - `lazymkv_core::fingerprint::DiscFile { path: String, size: u64 }` (fingerprint.rs:9, `PartialEq`).
  - `crate::testutil::temp_dir` (Task 11).
- Produces. Each item gets a `///` doc naming R-B2 and design part 5.4 step 2, ending "Def: T12. Use: T22, T23.":

```rust
// crates/tools/src/snapshot.rs
/// Every file under `root`, with `/`-separated paths relative to it, sorted by path.
pub fn list_files(root: &Path) -> io::Result<Vec<DiscFile>>;
/// Writes `files.json`, `nav/` and `meta.json` under `out` for the disc mounted at `root`.
pub fn snapshot(root: &Path, volume_label: &str, out: &Path, captured_unix: u64) -> io::Result<DiscMeta>;
```

- [ ] **Step 1: Write the failing tests in `crates/tools/src/snapshot.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use lazymkv_core::fingerprint::content_hash;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn snapshot_lists_fingerprints_and_copies_navigation_files() {
        let (root, out) = (temp_dir("disc"), temp_dir("out"));
        write(&root, "BDMV/STREAM/00001.m2ts", &[0u8; 300]);
        write(&root, "BDMV/STREAM/00000.m2ts", &[0u8; 100]);
        write(&root, "BDMV/PLAYLIST/00001.mpls", b"mpls");
        write(&root, "BDMV/index.bdmv", b"index");
        write(&root, "AACS/Unit_Key_RO.inf", b"abc");

        let meta = snapshot(&root, "MY_DISC", &out, 7).unwrap();

        assert_eq!(meta.volume_label, "MY_DISC");
        assert_eq!(meta.format.as_deref(), Some("bluray"));
        assert_eq!(meta.file_count, 5);
        assert_eq!(meta.total_bytes, 300 + 100 + 4 + 5 + 3);
        assert_eq!(meta.captured_unix, 7);
        let expected = content_hash(&[
            DiscFile { path: "BDMV/STREAM/00000.m2ts".into(), size: 100 },
            DiscFile { path: "BDMV/STREAM/00001.m2ts".into(), size: 300 },
        ]);
        assert_eq!(meta.fingerprint, expected);
        assert_eq!(meta.disc_id.as_deref(), Some("A9993E364706816ABA3E25717850C26C9CD0D89D"));
        assert!(out.join("nav/BDMV/PLAYLIST/00001.mpls").exists());
        assert!(out.join("nav/BDMV/index.bdmv").exists());
        assert!(!out.join("nav/BDMV/STREAM/00001.m2ts").exists());
        let written: serde_json::Value = serde_json::from_slice(&fs::read(out.join("meta.json")).unwrap()).unwrap();
        assert_eq!(written["captured_unix"], 7);
        let listed: serde_json::Value = serde_json::from_slice(&fs::read(out.join("files.json")).unwrap()).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 5);
        assert_eq!(listed[0], serde_json::json!({"path": "AACS/Unit_Key_RO.inf", "size": 3}));
    }

    #[test]
    fn the_snapshot_never_copies_unit_key_ro() {
        let (root, out) = (temp_dir("disc-key"), temp_dir("out-key"));
        write(&root, "AACS/Unit_Key_RO.inf", b"abc");
        write(&root, "AACS/DUPLICATE/Unit_Key_RO.inf", b"abc");
        write(&root, "BDMV/index.bdmv", b"index");

        let meta = snapshot(&root, "KEY", &out, 7).unwrap();

        assert!(!out.join("nav/AACS").exists());
        assert_eq!(meta.disc_id.as_deref(), Some("A9993E364706816ABA3E25717850C26C9CD0D89D"));
        let listed = fs::read_to_string(out.join("files.json")).unwrap();
        assert!(listed.contains("AACS/DUPLICATE/Unit_Key_RO.inf"), "{listed}");
    }

    #[test]
    fn nav_files_keep_their_relative_paths() {
        let (root, out) = (temp_dir("disc-nav"), temp_dir("out-nav"));
        write(&root, "BDMV/META/DL/bdmt_eng.xml", b"<di:name>Synthetic Movie</di:name>");
        write(&root, "BDMV/PLAYLIST/00800.mpls", b"mpls");
        write(&root, "BDMV/STREAM/00001.m2ts", &[0u8; 64]);

        snapshot(&root, "SYNTHETIC_MOVIE", &out, 7).unwrap();

        assert_eq!(fs::read(out.join("nav/BDMV/META/DL/bdmt_eng.xml")).unwrap(), b"<di:name>Synthetic Movie</di:name>");
        assert_eq!(fs::read(out.join("nav/BDMV/PLAYLIST/00800.mpls")).unwrap(), b"mpls");
        assert!(!out.join("nav/BDMV/STREAM").exists());
    }

    #[test]
    fn list_files_gives_slash_paths_sorted_by_path() {
        let root = temp_dir("list");
        write(&root, "b/2.txt", b"22");
        write(&root, "a.txt", b"1");
        write(&root, "b/1.txt", b"333");

        assert_eq!(
            list_files(&root).unwrap(),
            vec![
                DiscFile { path: "a.txt".into(), size: 1 },
                DiscFile { path: "b/1.txt".into(), size: 3 },
                DiscFile { path: "b/2.txt".into(), size: 2 },
            ]
        );
    }
}
```

In the same step, add `mod snapshot; pub use snapshot::{list_files, snapshot};` to `crates/tools/src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-tools --locked snapshot`
Expected: compile error. `cannot find function snapshot` and `cannot find function list_files` are reported in `snapshot.rs`, and the `pub use` names nothing.

- [ ] **Step 3: Implement `crates/tools/src/snapshot.rs`**

- **`list_files` and `walk`:** move them from capture disc.rs:28-54 unchanged.
- **`snapshot` keeps capture's write order (disc.rs:73-112):**
  1. `list_files(root)`, then `create_dir_all(out)`.
  2. Write `files.json` as `serde_json::to_vec_pretty` of a private `#[derive(Serialize)] struct FileEntry<'a> { path: &'a str, size: u64 }` (disc.rs:10-14), with `fs::write`.
  3. Copy each file that `core::snapshot::is_nav_file` accepts to `out/nav/<path>`, making its folders first.
  4. The unit key bytes are `unit_key_file(&files).and_then(|file| fs::read(root.join(&file.path)).ok())`.
  5. Then `let meta = disc_meta(&files, volume_label, unit_key.as_deref(), captured_unix);`.
  6. Write `meta.json` as `serde_json::to_vec_pretty(&meta)` with `fs::write`, and return `meta`.

This file holds no clock, because `captured_unix` is a parameter.

- [ ] **Step 4: Switch capture**

- Delete `crates/capture/src/disc.rs`, and remove `mod disc;` (main.rs:1).
- Replace the `Some("disc")` arm (main.rs:26-35) with code that does the following:
  1. Compute `let captured_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0);`. This is the expression at disc.rs:105-108.
  2. Call `lazymkv_tools::snapshot(Path::new(flag(args, "--root")?), flag(args, "--label")?, Path::new(flag(args, "--out")?), captured_unix).map_err(io_error)?`.
  3. Print `serde_json::to_string_pretty(&meta)` as today, and return `Ok(0)`.

`DiscMeta` serialises the same seven keys in the same order as the old `Meta` (Task 3's `disc_meta_gives_phase_0s_seven_keys_in_order`), so stdout is unchanged.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p lazymkv-capture --locked`
Expected: PASS.
- `lazymkv-tools` gains 4 passed, for 11 unit tests in all.
- `lazymkv-capture` loses its 2 `disc.rs` tests. Task 3's `disc_snapshot::snapshot_output_is_pinned_byte_for_byte` passes unchanged, which confirms that `files.json`, `meta.json` (with `captured_unix` masked), stdout and the `nav/` list are byte-identical (R-B2).
- capture's record tests and `exit_code` test pass unchanged.

- [ ] **Step 6: Commit**

```
git add crates/tools/src/snapshot.rs crates/tools/src/lib.rs crates/capture/src/main.rs
git rm crates/capture/src/disc.rs
git commit -m "tools::snapshot: the disc snapshot's file I/O moves out of capture (R-B2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` twice, and no attribution line.

### Task 13: The `makemkvcon` drivers (design parts 5.3, 5.4, 5.6)

**Files:**
- Create: `crates/tools/src/makemkv.rs`, with 5 unit tests in its `mod tests`.
- Modify: `crates/tools/src/lib.rs`, adding the module and its re-exports.
- Create: `crates/fake-tools/tests/makemkv_driver.rs`, with 16 tests. Each test writes its scenario into its own scratch folder, never under `corpus/`.

**Interfaces:**
- Consumes:
  - Task 5 (`core::robot::lines`):
    - `pub enum Call { DriveList, Scan, Rip { index: u32, source: disc::TitleSource } }`
    - `pub enum LineEvent { Progress { fraction: f64 }, IndexMismatch, Licence { code: u32, text: String }, ProfileRejected { text: String } }`
    - `pub struct RunSummary { pub version, pub java, pub profile_rejected: Option<String>, pub licence: Option<robot::Message>, pub index_guard: Option<bool>, pub read_errors: bool, pub write_error: bool, pub removed, pub title_saved: bool, pub title_failed: bool, pub scan_complete: bool, pub drives: Vec<robot::Drive> }`
    - `impl LineReader { pub fn new(call: Call) -> LineReader; pub fn feed(&mut self, line: &str) -> Vec<LineEvent>; pub fn finish(self) -> RunSummary; }`
  - Existing core items:
    - `robot::Drive = Vec<String>` (crates/core/src/robot.rs:21) and `robot::Message { code, text, params }` (robot.rs:14).
    - `disc::TitleSource { File { name }, DvdTitle { number }, Unknown }` (crates/core/src/disc.rs:115).
    - `job::Failure { class, output }` (crates/core/src/job.rs:128), `job::FailureClass` (job.rs:25) and `job::OutputCause { ShareUnreachable, OutOfSpace, WriteError }` (job.rs:91).
    - `pub fn classify_title_failure(share_healthy: bool, out_of_space: bool, write_error: bool, read_errors: bool) -> Failure` (job.rs:677).
  - Task 7: `core::ipc::CANCELLED` (`"cancelled by the owner"`) and `core::ipc::OutputStream`.
  - Task 8: `platform::Host::running_processes(&self) -> Vec<String>`. Tests use `fakes::FakeHost::new(local_date: &str, now_unix: u64)` and `FakeHost::set_running(&self, Vec<String>)`.
  - Task 9: `platform::StdProcessGuard`.
  - Task 11:
    - `Limits { header, scan, exit_wait, after_kill }`, `CancelToken` (`cancel()`, `is_cancelled()`), `ToolCtx<'a> { guard, host, limits, cancel, on_output, on_progress }`.
    - `RunResult { exit_code, lines, elapsed_ms, started_unix, timed_out, cancelled, stdout }` and `LineAction { Continue, Kill }`.
    - `pub fn run_recorded(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path, limit: Option<Duration>, below_normal: bool, on_line: &mut dyn FnMut(&str) -> LineAction) -> io::Result<RunResult>`. This task relies on Task 11's behaviour: `on_line` gets each stdout line decoded with replacement and without its `\n`; `limit` bounds the whole run; `timed_out` and `cancelled` say why a run was killed.
  - Task 10: the stand-in's scenario keys `creates[].dir_arg_from_end` (1 is the last argument) and `hang_after_line`. Two responses with identical `args_contain` are served one per run, in order. The stand-in reads `<stem>.json` beside its own executable when `LAZYMKV_FAKE_DIR` is unset (S15).
- Produces (in `crates/tools/src/makemkv.rs`, re-exported at the crate root):

```rust
/// The flags a job fixes for every makemkvcon call (design parts 5.3, 5.4; spec 9.2). Def: T13. Use: T23.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakemkvFlags { pub minlength_s: u32, pub cache_mb: u32, pub profile: PathBuf, pub debug: bool }
/// ["-r","--cache=1","info","disc:9999"], with "--debug" after "-r" when `debug` (design part 5.4 step 1, S45). Def: T13. Use: T13, T23.
pub fn drive_list_args(debug: bool) -> Vec<String>;
/// ["-r","--messages=-stdout","--progress=-same",("--debug",)"--minlength=<m>","--cache=<c>","--profile=<p>","info","disc:<drive>"] (design part 5.4 step 4). Def: T13. Use: T13, T23.
pub fn scan_args(flags: &MakemkvFlags, drive: u32) -> Vec<String>;
/// The scan's flags, then "mkv","disc:<drive>","<title>","<dir>" (design part 5.6 step 6). Def: T13. Use: T13, T23.
pub fn rip_args(flags: &MakemkvFlags, drive: u32, title: u32, dir: &Path) -> Vec<String>;
/// Field 0, read as a number, of the first DRV line whose field 6 equals `drive` exactly (design part 5.4 step 1). Def: T13. Use: T13, T23.
pub fn drive_index(drives: &[robot::Drive], drive: &str) -> Option<u32>;
/// One drive listing: the index for the drive id, the reducer's summary and the run (design parts 3.7, 5.4). Def: T13. Use: T23, T26.
#[derive(Clone, Debug)]
pub struct DriveListing { pub index: Option<u32>, pub summary: RunSummary, pub run: RunResult }
/// Runs and records the drive listing into `out` (design part 5.4 step 1). Def: T13. Use: T23, T26.
pub fn list_drives(ctx: &ToolCtx, program: &Path, debug: bool, drive: &str, out: &Path) -> io::Result<DriveListing>;
/// How a scan ended (design part 5.4 step 4, L5). Def: T13. Use: T23.
#[derive(Clone, Debug, PartialEq)]
pub enum ScanOutcome { Done { text: String, summary: RunSummary }, ProfileRejected { text: String }, Failed { failure: Failure, message: String } }
/// Runs and records the scan into `out`, killing it past `limits.scan`; the engine lists the drives and retries once
/// (RP-5; design parts 5.3, 5.4 step 4). Def: T13. Use: T23.
pub fn scan(ctx: &ToolCtx, program: &Path, flags: &MakemkvFlags, drive: u32, out: &Path) -> io::Result<ScanOutcome>;
/// `classify_title_failure`'s first two arguments, read when a rip has failed (design part 5.6 step 6). Def: T13. Use: T23.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagingHealth { pub share_healthy: bool, pub out_of_space: bool }
/// How a rip ended (design part 5.6 step 6). Def: T13. Use: T23.
#[derive(Clone, Debug, PartialEq)]
pub enum RipOutcome { Ripped { file: PathBuf }, IndexMismatch, Licence { message: String }, Failed { failure: Failure, message: String } }
/// A rip's outcome, run and summary. Def: T13. Use: T23.
#[derive(Clone, Debug)]
pub struct RipRun { pub outcome: RipOutcome, pub run: RunResult, pub summary: RunSummary }
/// Runs and records one title's rip into `out`, with the index guard (design part 5.6 step 6). Def: T13. Use: T23.
pub fn rip(ctx: &ToolCtx, program: &Path, flags: &MakemkvFlags, drive: u32, title: u32, source: TitleSource,
    dir: &Path, out: &Path, staging: &dyn Fn() -> StagingHealth) -> io::Result<RipRun>;
```

Rulings settled in this task:
- **S55. The "one `makemkvcon` at a time" check runs inside every driver before it spawns.** It compares `host.running_processes()` against `makemkvcon64`, `makemkvcon` and `makemkv` (the GUI), lower-cased and with a trailing `.exe` removed. A match stops the call with the message `another MakeMKV process is running: <name as reported>`. `scan` and `rip` return `Failed { Unknown }` with that message. `list_drives` returns `Err` of kind `io::ErrorKind::ResourceBusy` with the same message, because `DriveListing` has no failure variant. Why: design part 5.3 fails the step as `Unknown`, naming the process.
- **S56. A driver that kills its process sleeps `limits.after_kill` before it returns.** This covers every kill: a time limit, the exit wait, a `LineAction::Kill` and a cancel. Why: design part 5.3 says no `makemkvcon` call may start within 10 s of a kill, and returning only after the wait guarantees it.
- **S57. The exit wait (`limits.exit_wait`) starts at the call's final message.** The driver recognises that message by its line prefix, only to start the clock:
  - a drive listing ends at the first `MSG:5010,`;
  - a scan ends at the first `TCOUNT:` after a `MSG:5011,`;
  - a rip ends at the first of `MSG:5004,`, `MSG:5005,`, `MSG:5036,` or `MSG:5037,`.

  Every verdict still comes from `RunSummary`. A run killed by the exit wait reports `run.timed_out == true`, and `run.cancelled` reflects only `ctx.cancel`. `scan` and `rip` tell the kills apart by the private `Kill` that `run_makemkv` returns (Step 3 item 5), never by `run.timed_out`, so an exit-wait kill never reports as a scan past its limit, which the engine retries (RP-5). Why: design part 5.3 bounds a run only after its final message, and `run_recorded`'s `limit` bounds the whole run.
- **S58. A retried scan writes `out` again.** `scan` writes `out` from the start on every call, so the engine's retry (Task 23, RP-5) replaces the killed attempt's files, because design part 7.1 says `scan.txt` is the scan the plan was made from.
- **S59. A licence message does not kill a rip.** MakeMKV ends the run itself, because design part 5.6 only says the message "is fed as `LicenceMessage`".

RP-5 settles this: (b) `scan` returns `Failed { Unknown, "the scan did not finish within scan_timeout_min and was killed" }` after the first timeout, and the engine (Task 23) lists the drives and calls `scan` once more, so the driver holds no retry and the retry and its test are in Task 23.

- [ ] **Step 1: Write the failing tests**

First add `pub mod makemkv;` to `crates/tools/src/lib.rs`, so the file compiles with its tests. Then add to the end of `crates/tools/src/makemkv.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn flags(debug: bool) -> MakemkvFlags {
        MakemkvFlags { minlength_s: 120, cache_mb: 1024, profile: PathBuf::from("p"), debug }
    }

    fn drv(fields: &[&str]) -> Vec<String> {
        fields.iter().map(|field| field.to_string()).collect()
    }

    #[test]
    fn scan_args_match_the_design() {
        assert_eq!(
            scan_args(&flags(false), 0),
            ["-r", "--messages=-stdout", "--progress=-same", "--minlength=120", "--cache=1024", "--profile=p", "info", "disc:0"]
        );
        assert_eq!(
            scan_args(&flags(true), 2),
            ["-r", "--messages=-stdout", "--progress=-same", "--debug", "--minlength=120", "--cache=1024", "--profile=p", "info", "disc:2"]
        );
    }

    #[test]
    fn rip_args_end_with_the_title_index_and_folder() {
        assert_eq!(
            rip_args(&flags(false), 1, 3, Path::new("dir")),
            ["-r", "--messages=-stdout", "--progress=-same", "--minlength=120", "--cache=1024", "--profile=p", "mkv", "disc:1", "3", "dir"]
        );
    }

    #[test]
    fn the_drive_listing_gets_debug_after_r_when_debug_is_on() {
        assert_eq!(drive_list_args(false), ["-r", "--cache=1", "info", "disc:9999"]);
        assert_eq!(drive_list_args(true), ["-r", "--debug", "--cache=1", "info", "disc:9999"]);
    }

    #[test]
    fn the_drive_index_is_the_drv_line_whose_seventh_field_is_the_drive_id() {
        let drives = vec![
            drv(&["0", "2", "999", "12", "BD-RE DRIVE A 1.00 SERIAL", "DISC", "D:"]),
            drv(&["1", "2", "999", "12", "BD-RE DRIVE B 1.00 SERIAL", "OTHER", "E:"]),
            drv(&["2", "256", "999", "0", "", "", ""]),
            drv(&["x", "2", "999", "12", "BROKEN", "", "F:"]),
            drv(&["3", "2", "999"]),
        ];
        assert_eq!(drive_index(&drives, "D:"), Some(0));
        assert_eq!(drive_index(&drives, "E:"), Some(1));
        assert_eq!(drive_index(&drives, "F:"), None);
        assert_eq!(drive_index(&drives, "G:"), None);
    }

    #[test]
    fn the_drive_index_requires_an_exact_match_of_the_seventh_field() {
        let drives = vec![drv(&["0", "2", "999", "12", "BD-RE DRIVE A 1.00 SERIAL", "DISC", "D:"])];
        assert_eq!(drive_index(&drives, "D:"), Some(0));
        assert_eq!(drive_index(&drives, "D:\\"), None);
        assert_eq!(drive_index(&drives, "d:"), None);
    }
}
```

Create `crates/fake-tools/tests/makemkv_driver.rs`:

```rust
//! The makemkvcon drivers against the stand-in (design parts 5.3, 5.4, 5.6; Ruling S12).

use lazymkv_core::disc::TitleSource;
use lazymkv_core::ipc::OutputStream;
use lazymkv_core::job::{Failure, FailureClass, OutputCause};
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{
    list_drives, rip, scan, CancelToken, Limits, MakemkvFlags, RipOutcome, RipRun, ScanOutcome, StagingHealth, ToolCtx,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const EXE: &str = std::env::consts::EXE_SUFFIX;
const VERSION: &str = "MSG:1005,0,1,\"MakeMKV v2.0.0 win(x64-release) started\",\"%1 started\",\"MakeMKV v2.0.0 win(x64-release)\"\n";
const ADDED_0: &str = "MSG:3307,0,2,\"File 00800.mpls was added as title #0\",\"File %1 was added as title #%2\",\"00800.mpls\",\"0\"\n";
const ADDED_1: &str = "MSG:3307,0,2,\"File 00800.mpls was added as title #1\",\"File %1 was added as title #%2\",\"00800.mpls\",\"1\"\n";
const TITLE: &str = "TINFO:0,9,0,\"1:59:10\"\nTINFO:0,16,0,\"00800.mpls\"\n";
const COMPLETE: &str = "MSG:5011,0,0,\"Operation successfully completed\",\"Operation successfully completed\"\nTCOUNT:1\n";
const SAVING: &str = "MSG:5015,0,0,\"Saving to MKV file\",\"Saving to MKV file\"\n";
const PROGRESS: &str = "PRGV:32768,32768,65536\n";
const SAVED: &str = "MSG:5036,0,1,\"Copy complete. 1 titles saved.\",\"Copy complete. %1 titles saved.\",\"1\"\n";
const FAILED: &str = "MSG:5037,0,2,\"Copy complete. 0 titles saved, 1 failed.\",\"Copy complete. %1 titles saved, %2 failed.\",\"0\",\"1\"\n";
const FILLER: &str = "MSG:1004,0,0,\"Synthetic debug line\",\"Synthetic debug line\"\n";
const READ_ERROR: &str = "MSG:2003,0,3,\"Error 'Scsi error' occurred while reading '/BDMV/STREAM/00001.m2ts' at offset '6144'\",\"Error '%1' occurred while reading '%2' at offset '%3'\",\"Scsi error\",\"/BDMV/STREAM/00001.m2ts\",\"6144\"\n";
const LICENCE: &str = "MSG:5050,0,0,\"Synthetic licence message\",\"Synthetic licence message\"\n";
const REJECTED: &str = "MSG:1009,0,1,\"Profile parsing error: synthetic\",\"Profile parsing error: %1\",\"synthetic\"\n";
const DRIVES: &str = "DRV:0,2,999,12,\"BD-RE SYNTHETIC DRIVE 1.00 SERIAL\",\"SYNTHETIC_MOVIE\",\"tok0\"\nDRV:1,256,999,0,\"\",\"\",\"\"\nMSG:5010,0,0,\"Failed to open disc\",\"Failed to open disc\"\n";
const HEALTHY: StagingHealth = StagingHealth { share_healthy: true, out_of_space: false };

/// A stand-in makemkvcon64 in `<scratch>/tools/` with its scenario and replay files beside it (S15).
fn stand_in(scratch: &Scratch, scenario: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let program = scratch.0.join("tools").join(format!("makemkvcon64{EXE}"));
    scratch.write("tools/makemkvcon64.json", scenario);
    for (name, bytes) in files {
        scratch.write(&format!("tools/{name}"), bytes);
    }
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &program).unwrap();
    program
}

struct Env { host: FakeHost, limits: Limits, cancel: CancelToken, progress: Mutex<Vec<f64>> }

impl Env {
    fn new() -> Env {
        Env {
            host: FakeHost::new("2026-10-23", 1_800_000_000),
            limits: Limits {
                header: Duration::from_secs(120),
                scan: Duration::from_secs(60),
                exit_wait: Duration::from_secs(60),
                after_kill: Duration::from_millis(10),
            },
            cancel: CancelToken::default(),
            progress: Mutex::new(Vec::new()),
        }
    }

    fn run<R>(&self, f: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |fraction: f64| self.progress.lock().unwrap().push(fraction);
        let ctx = ToolCtx {
            guard: &StdProcessGuard,
            host: &self.host,
            limits: &self.limits,
            cancel: self.cancel.clone(),
            on_output: &on_output,
            on_progress: &on_progress,
        };
        f(&ctx)
    }

    /// Cancels after 30 s, so that a broken kill fails the test instead of hanging it.
    fn watchdog(&self) {
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(30));
            cancel.cancel();
        });
    }
}

fn flags(scratch: &Scratch) -> MakemkvFlags {
    MakemkvFlags { minlength_s: 120, cache_mb: 1024, profile: scratch.0.join("job.mmcp.xml"), debug: false }
}

fn scan_failure(outcome: &ScanOutcome) -> (Failure, String) {
    match outcome {
        ScanOutcome::Failed { failure, message } => (*failure, message.clone()),
        other => panic!("expected Failed, got {other:?}"),
    }
}

fn rip_failure(outcome: &RipOutcome) -> (Failure, String) {
    match outcome {
        RipOutcome::Failed { failure, message } => (*failure, message.clone()),
        other => panic!("expected Failed, got {other:?}"),
    }
}

const UNKNOWN: Failure = Failure { class: FailureClass::Unknown, output: None };

/// Runs one scan of `stdout` (exit `exit_code`) and returns the outcome and the recorded scan.txt path.
fn scan_once(scratch: &Scratch, env: &Env, stdout: &[u8], exit_code: i32) -> (ScanOutcome, PathBuf) {
    let scenario = format!(r#"{{"responses":[{{"args_contain":["info","disc:0"],"stdout":"scan.txt","exit_code":{exit_code}}}]}}"#);
    let program = stand_in(scratch, &scenario, &[("scan.txt", stdout)]);
    let out = scratch.0.join("job/scan.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let outcome = env.run(|ctx| scan(ctx, &program, &flags(scratch), 0, &out)).unwrap();
    (outcome, out)
}

/// Runs one rip of title index 0 (source 00800.mpls) under `scenario`, with `rip.txt` holding `stdout`.
fn rip_once(scratch: &Scratch, env: &Env, scenario: &str, stdout: &str, staging: StagingHealth) -> (RipRun, PathBuf, PathBuf) {
    let program = stand_in(scratch, scenario, &[("rip.txt", stdout.as_bytes()), ("placeholder.mkv", b"placeholder bytes")]);
    let dir = scratch.0.join("staging/j-1/rip/t00-00800");
    let out = scratch.0.join("job/t00-00800/rip.txt");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let source = TitleSource::File { name: "00800.mpls".to_string() };
    let ran = env.run(|ctx| rip(ctx, &program, &flags(scratch), 0, 0, source, &dir, &out, &|| staging)).unwrap();
    (ran, dir, out)
}

const ONE_MKV: &str = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt",
    "creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"placeholder.mkv"}]}]}"#;

#[test]
fn a_drive_listing_finds_the_index_and_ignores_msg_5010() {
    let scratch = Scratch::new("drive-list");
    let env = Env::new();
    let program = stand_in(&scratch, r#"{"responses":[{"args_contain":["info","disc:9999"],"stdout":"drive-list.txt"}]}"#,
        &[("drive-list.txt", DRIVES.as_bytes())]);
    let out = scratch.0.join("job/drive-list.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let listing = env.run(|ctx| list_drives(ctx, &program, false, "tok0", &out)).unwrap();
    assert_eq!(listing.index, Some(0));
    assert_eq!(listing.summary.drives.len(), 2);
    assert_eq!(listing.run.exit_code, Some(0));
    assert_eq!(fs::read(&out).unwrap(), DRIVES.as_bytes());
    let other = env.run(|ctx| list_drives(ctx, &program, false, "tok1", &out)).unwrap();
    assert_eq!(other.index, None);
}

#[test]
fn a_scan_without_5011_and_tcount_fails_as_unknown_whatever_its_exit_code() {
    let scratch = Scratch::new("scan-incomplete");
    let env = Env::new();
    let text = format!("{VERSION}{ADDED_0}{TITLE}");
    let (outcome, _) = scan_once(&scratch, &env, text.as_bytes(), 0);
    let (failure, message) = scan_failure(&outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "the scan ended without MSG:5011 and a TCOUNT line (exit code 0)");
}

#[test]
fn a_complete_scan_succeeds_whatever_its_exit_code() {
    let scratch = Scratch::new("scan-exit-1");
    let env = Env::new();
    let text = format!("{VERSION}{ADDED_0}{TITLE}{COMPLETE}");
    let (outcome, out) = scan_once(&scratch, &env, text.as_bytes(), 1);
    match outcome {
        ScanOutcome::Done { text: read, summary } => {
            assert_eq!(read, text);
            assert!(summary.scan_complete);
        }
        other => panic!("expected Done, got {other:?}"),
    }
    assert_eq!(fs::read(&out).unwrap(), text.as_bytes());
    for suffix in [".stderr", ".timing.jsonl", ".exit.json"] {
        assert!(PathBuf::from(format!("{}{suffix}", out.display())).is_file(), "{suffix}");
    }
}

#[test]
fn a_scan_licence_message_fails_as_licence() {
    let scratch = Scratch::new("scan-licence");
    let env = Env::new();
    let text = format!("{VERSION}{LICENCE}");
    let (outcome, _) = scan_once(&scratch, &env, text.as_bytes(), 0);
    assert_eq!(
        outcome,
        ScanOutcome::Failed {
            failure: Failure { class: FailureClass::Licence, output: None },
            message: "Synthetic licence message".to_string()
        }
    );
}

#[test]
fn a_1009_on_the_scan_is_reported_as_a_rejected_profile() {
    let scratch = Scratch::new("scan-1009");
    let env = Env::new();
    let text = format!("{VERSION}{REJECTED}{ADDED_0}{TITLE}{COMPLETE}");
    let (outcome, _) = scan_once(&scratch, &env, text.as_bytes(), 0);
    assert_eq!(outcome, ScanOutcome::ProfileRejected { text: "Profile parsing error: synthetic".to_string() });
}

/// RP-5: the driver kills a scan past its limit and fails at once; the engine's retry is Task 23's.
#[test]
fn a_scan_past_its_limit_is_killed_and_fails_as_unknown() {
    let scratch = Scratch::new("scan-limit");
    let mut env = Env::new();
    env.limits.scan = Duration::from_millis(300);
    env.watchdog();
    let text = format!("{VERSION}{ADDED_0}");
    let program = stand_in(&scratch,
        r#"{"responses":[{"args_contain":["info","disc:0"],"stdout":"scan.txt","hang_after_line":0}]}"#,
        &[("scan.txt", text.as_bytes())]);
    let out = scratch.0.join("job/scan.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let started = Instant::now();
    let outcome = env.run(|ctx| scan(ctx, &program, &flags(&scratch), 0, &out)).unwrap();
    assert!(started.elapsed() < Duration::from_secs(20));
    let (failure, message) = scan_failure(&outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "the scan did not finish within scan_timeout_min and was killed");
    // Task 10's counter: the stand-in ran once, so the driver did not retry.
    assert_eq!(fs::read_to_string(scratch.0.join("tools/makemkvcon64.calls.json")).unwrap(), r#"{"0":1}"#);
}

#[test]
fn a_scan_line_with_invalid_utf8_is_recorded_byte_for_byte_and_read_with_replacement() {
    let scratch = Scratch::new("scan-1252");
    let env = Env::new();
    let mut bytes = VERSION.as_bytes().to_vec();
    bytes.extend_from_slice(b"CINFO:2,0,\"Caf\xe9 Disc\"\n");
    bytes.extend_from_slice(format!("{ADDED_0}{TITLE}{COMPLETE}").as_bytes());
    let (outcome, out) = scan_once(&scratch, &env, &bytes, 0);
    assert_eq!(fs::read(&out).unwrap(), bytes);
    match outcome {
        ScanOutcome::Done { text, .. } => assert!(text.contains("CINFO:2,0,\"Caf\u{fffd} Disc\"")),
        other => panic!("expected Done, got {other:?}"),
    }
}

#[test]
fn a_rip_succeeds_with_one_saved_title_and_one_mkv() {
    let scratch = Scratch::new("rip-ok");
    let env = Env::new();
    let text = format!("{VERSION}{ADDED_0}{SAVING}{PROGRESS}{SAVED}");
    let (ran, dir, out) = rip_once(&scratch, &env, ONE_MKV, &text, HEALTHY);
    assert_eq!(ran.outcome, RipOutcome::Ripped { file: dir.join("title_t00.mkv") });
    assert_eq!(ran.run.exit_code, Some(0));
    assert!(ran.summary.title_saved);
    assert_eq!(ran.summary.index_guard, Some(true));
    assert_eq!(*env.progress.lock().unwrap(), vec![0.5]);
    assert_eq!(fs::read(&out).unwrap(), text.as_bytes());
}

#[test]
fn a_rip_with_two_mkv_files_fails() {
    let scratch = Scratch::new("rip-two");
    let env = Env::new();
    let scenario = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt","creates":[
        {"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"placeholder.mkv"},
        {"dir_arg_from_end":1,"name":"title_t01.mkv","copy_from":"placeholder.mkv"}]}]}"#;
    let (ran, _, _) = rip_once(&scratch, &env, scenario, &format!("{VERSION}{ADDED_0}{SAVING}{SAVED}"), HEALTHY);
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "the title folder holds 2 .mkv files, not one");
}

#[test]
fn an_index_mismatch_kills_the_rip() {
    let scratch = Scratch::new("rip-drift");
    let mut env = Env::new();
    env.limits.after_kill = Duration::from_millis(300);
    env.watchdog();
    let scenario = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt","hang_after_line":3}]}"#;
    let started = Instant::now();
    let (ran, _, _) = rip_once(&scratch, &env, scenario, &format!("{VERSION}{ADDED_1}{SAVING}{PROGRESS}{SAVED}"), HEALTHY);
    assert!(started.elapsed() >= Duration::from_millis(300), "the driver returned before limits.after_kill (S56)");
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_eq!(ran.outcome, RipOutcome::IndexMismatch);
    assert_eq!(ran.summary.index_guard, Some(false));
    assert_ne!(ran.run.exit_code, Some(0));
}

#[test]
fn a_licence_message_during_a_rip_is_rip_outcome_licence() {
    let scratch = Scratch::new("rip-licence");
    let env = Env::new();
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{LICENCE}{FAILED}"), HEALTHY);
    assert_eq!(ran.outcome, RipOutcome::Licence { message: "Synthetic licence message".to_string() });
}

#[test]
fn a_1009_first_seen_on_a_rip_fails_the_title_as_unknown() {
    let scratch = Scratch::new("rip-1009");
    let env = Env::new();
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{REJECTED}{ADDED_0}{SAVING}{SAVED}"), HEALTHY);
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "MakeMKV rejected the profile on the rip: Profile parsing error: synthetic");
}

#[test]
fn a_rip_with_2003_and_a_healthy_share_fails_as_media_read_error() {
    let scratch = Scratch::new("rip-2003");
    let env = Env::new();
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{READ_ERROR}{FAILED}"), HEALTHY);
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, Failure { class: FailureClass::MediaReadError, output: None });
    assert_eq!(message, "MakeMKV reported the title failed (exit code 0)");
}

#[test]
fn a_rip_with_the_share_unreachable_fails_as_output_share_unreachable() {
    let scratch = Scratch::new("rip-share");
    let env = Env::new();
    let lost = StagingHealth { share_healthy: false, out_of_space: false };
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{FAILED}"), lost);
    let (failure, _) = rip_failure(&ran.outcome);
    assert_eq!(failure, Failure { class: FailureClass::Output, output: Some(OutputCause::ShareUnreachable) });
}

#[test]
fn a_rip_that_does_not_exit_within_the_exit_wait_is_killed_and_fails_saying_so() {
    let scratch = Scratch::new("rip-exit-wait");
    let mut env = Env::new();
    env.limits.exit_wait = Duration::from_millis(300);
    env.watchdog();
    let scenario = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt","hang_after_line":4}]}"#;
    let started = Instant::now();
    let (ran, _, _) = rip_once(&scratch, &env, scenario, &format!("{VERSION}{ADDED_0}{SAVING}{SAVED}{FILLER}"), HEALTHY);
    assert!(started.elapsed() < Duration::from_secs(20));
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "makemkvcon did not exit within exit_wait_min of its final message and was killed");
    assert!(ran.run.timed_out);
    assert!(!ran.run.cancelled);
}

#[test]
fn another_makemkvcon_running_fails_the_step_naming_it() {
    let scratch = Scratch::new("busy");
    let env = Env::new();
    env.host.set_running(vec!["makemkvcon64.exe".to_string()]);
    let (outcome, out) = scan_once(&scratch, &env, format!("{VERSION}{COMPLETE}").as_bytes(), 0);
    assert_eq!(scan_failure(&outcome), (UNKNOWN, "another MakeMKV process is running: makemkvcon64.exe".to_string()));
    assert!(!out.exists());
    let listed = env.run(|ctx| list_drives(ctx, &scratch.0.join(format!("tools/makemkvcon64{EXE}")), false, "tok0",
        &scratch.0.join("job/drive-list.txt")));
    let error = listed.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::ResourceBusy);
    assert_eq!(error.to_string(), "another MakeMKV process is running: makemkvcon64.exe");
    env.host.set_running(vec!["MakeMKV.exe".to_string()]);
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{SAVED}"), HEALTHY);
    assert_eq!(rip_failure(&ran.outcome), (UNKNOWN, "another MakeMKV process is running: MakeMKV.exe".to_string()));
}
```

Two scenario notes:
- Task 10 prints exactly `hang_after_line` stdout lines, then hangs (its Ruling on `stop_after_line` and `hang_after_line`). `hang_after_line` in `an_index_mismatch_kills_the_rip` and the exit-wait test is therefore the number of lines the test needs printed.
- In the exit-wait test the stand-in never reaches `creates`, so the folder holds no `.mkv`. The exit-wait verdict takes precedence over that.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p fake-tools --locked`
Expected: compile error, with `cannot find function scan_args in this scope` in `lazymkv-tools` and `unresolved imports lazymkv_tools::list_drives, lazymkv_tools::rip, lazymkv_tools::scan` in `makemkv_driver.rs`.

- [ ] **Step 3: Implement `crates/tools/src/makemkv.rs` and export it**

1. In `crates/tools/src/lib.rs`, add `pub use makemkv::{drive_index, drive_list_args, list_drives, rip, rip_args, scan, scan_args, DriveListing, MakemkvFlags, RipOutcome, RipRun, ScanOutcome, StagingHealth};`.
2. Write the Produces items above, each with its `///` doc ending "Def: T13. Use: ...".
3. Pinned values:
   - `--profile=` is followed by `flags.profile.display()`. The rip folder is `dir.display()`.
   - `drive_index` skips a DRV line with fewer than 7 fields or a field 0 that is not a `u32`.
   - Private `const MAKEMKV_PROCESSES: [&str; 3] = ["makemkvcon64", "makemkvcon", "makemkv"];` (S55).
4. Message texts (exact). `<code>` is the exit code, or `none` when the process was killed:
   - process busy: `another MakeMKV process is running: <name as reported>`
   - incomplete scan: `the scan ended without MSG:5011 and a TCOUNT line (exit code <code>)`
   - scan past its limit: `the scan did not finish within scan_timeout_min and was killed`
   - exit wait: `makemkvcon did not exit within exit_wait_min of its final message and was killed`
   - cancelled: `lazymkv_core::ipc::CANCELLED`
   - licence: the licence `Message.text`
   - 1009 on a rip: `MakeMKV rejected the profile on the rip: <text>`
   - rip without a success summary: `the rip ended without a success summary (exit code <code>)`
   - rip with a failed title: `MakeMKV reported the title failed (exit code <code>)`
   - rip folder count: `the title folder holds <n> .mkv files, not one`
5. Write a private `enum Kill { None, Limit, ExitWait, LineKill, Cancel }` and a private helper `run_makemkv(ctx, program, args, out, limit, call) -> io::Result<(RunResult, RunSummary, Kill)>`, shared by all three drivers. `Kill` says which kill ended the run: `Limit` when `run_recorded` reports the whole-run `limit` hit, `ExitWait` when the watcher below fired, `LineKill` after `on_line` returned `Kill`, `Cancel` when `ctx.cancel` was set, `None` otherwise. This is the one algorithm the tests do not fix:
   1. Run S55's process check. A match returns `Err(ResourceBusy)`.
   2. Make an inner `CancelToken` and a watcher thread inside `std::thread::scope`. Every 50 ms or less, the thread copies `ctx.cancel` into the inner token. It also cancels the inner token, and records "exit wait hit", once `limits.exit_wait` has passed since the final-message instant (S57).
   3. Call `run_recorded` with a `ToolCtx` whose `cancel` is the inner token, `below_normal: false`, and an `on_line` callback that does three things:
      - strips one trailing `\r` and feeds the line to a `LineReader::new(call)`;
      - forwards each `Progress` to `ctx.on_progress`;
      - returns `Kill` on `IndexMismatch`, and records the final-message instant.
   4. Stop the watcher. Set `run.cancelled = ctx.cancel.is_cancelled()`, and set `run.timed_out = true` when the kill is `ExitWait`.
   5. If the kill is not `Kill::None`, sleep `limits.after_kill` (S56).
6. `list_drives` calls `run_makemkv(.., limit None, Call::DriveList)` and fills `index` with `drive_index(&summary.drives, drive)`.
7. `scan` calls `run_makemkv(.., Some(limits.scan), Call::Scan)`. It decides in this order:
   1. busy gives `Failed`;
   2. `Kill::Limit` gives `Failed { Unknown }` with the past-limit text, with no retry in the driver (RP-5; Task 23 retries);
   3. `Kill::ExitWait` gives `Failed` with the exit-wait text;
   4. `Kill::Cancel` gives `Failed { Unknown, CANCELLED }`;
   5. `summary.licence` gives `Failed { Licence }`;
   6. `summary.profile_rejected` gives `ProfileRejected`;
   7. `!summary.scan_complete` gives `Failed`;
   8. otherwise `Done { text: String::from_utf8_lossy(&run.stdout).into_owned(), summary }`.
8. `rip` calls `run_makemkv(.., None, Call::Rip { index: title, source })`. It decides in this order:
   1. busy;
   2. `Kill::Cancel`;
   3. `Kill::ExitWait`;
   4. `index_guard == Some(false)` gives `IndexMismatch`;
   5. licence gives `Licence { message }`;
   6. `profile_rejected` gives `Failed { Unknown }`;
   7. otherwise count the regular files in `dir` whose extension is `mkv`, ignoring ASCII case. A missing `dir` counts 0, and other I/O errors propagate. `title_saved && !title_failed && count == 1` gives `Ripped { file }`.
   8. Anything else gives `Failed { failure: classify_title_failure(h.share_healthy, h.out_of_space, summary.write_error, summary.read_errors), message }`, where `h = staging()`. The message is chosen by these exact conditions: the failed-title text when `title_failed`; the no-summary text when `!title_saved && !title_failed`; the folder-count text when `title_saved && !title_failed && count != 1`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p fake-tools --locked`
Expected: PASS. `lazymkv-tools` shows the 5 `makemkv::tests` passing, and `Running tests/makemkv_driver.rs` reports `test result: ok. 16 passed; 0 failed`. Every earlier test is unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/makemkv.rs crates/tools/src/lib.rs crates/fake-tools/tests/makemkv_driver.rs
git commit -m "tools::makemkv: drive listing, scan with its fallback signal, and rip with the index guard (design parts 5.4, 5.6)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, the message, and no attribution line.

### Task 14: The `mkvmerge` and `ffprobe` drivers and the debug-log save

**Files:**
- Create: `crates/tools/src/mkvmerge.rs` (6 unit tests) and `crates/tools/src/ffprobe.rs` (1 unit test).
- Modify: `crates/tools/src/makemkv.rs`, adding `save_debug_log` and 3 unit tests. Modify `crates/tools/src/lib.rs` for the modules and re-exports.
- Create: `crates/fake-tools/tests/mkvmerge_ffprobe_driver.rs`, with 6 tests.

**Interfaces:**
- Consumes:
  - `core::tracks::layout::{Layout, OutputTrack}` (crates/core/src/tracks/layout.rs:40, :16). `OutputTrack` has the fields `stream`, `kind`, `language`, `name`, `default`, `forced`, `hearing_impaired` and `original`.
  - `core::disc::StreamKind` (crates/core/src/disc.rs:20).
  - `core::verify::VerifyReport` (crates/core/src/verify.rs:198). `pairs: Vec<(u32, Option<i64>)>` and `tracks_ok: bool` are the fields this task reads.
  - `core::recording::files::MkvmergeIdent` (crates/core/src/recording/files.rs:81) and `FfprobeStreams` (files.rs:127).
  - `core::ipc::CANCELLED` (Task 7) and `platform::Host` (Task 8): `makemkv_debug_log(&self) -> Option<PathBuf>` and `makemkv_app_key(&self) -> Option<String>`.
  - From Task 11: `run_recorded`, `ToolCtx`, `Limits`, `RunResult`, `LineAction` and `with_suffix(path: &Path, suffix: &str) -> PathBuf`.
- Produces:

```rust
// crates/tools/src/mkvmerge.rs
/// ["-J", <file>] (design part 5.6 step 7, 5.7 step 14). Def: T14. Use: T14, T23, T24.
pub fn ident_args(file: &Path) -> Vec<String>;
/// mkvmerge -J under limits.header, below normal; exit 0 or 1 parses stdout (PIPE-14). Def: T14. Use: T23, T24.
pub fn ident(ctx: &ToolCtx, program: &Path, file: &Path, out: &Path) -> io::Result<Result<MkvmergeIdent, String>>;
/// The remux's arguments (design part 5.7 step 13; spec 8.10); `staged` maps each layout stream to the raw file's
/// track ID (S60). Def: T14. Use: T24.
pub fn remux_args(partial: &Path, layout: &Layout, staged: &VerifyReport, raw: &Path) -> Result<Vec<String>, String>;
/// "#GUI#progress 45%" -> Some(0.45); a whole percentage 0 to 100 only (design part 5.11). Def: T14. Use: T14, T24.
pub fn gui_progress(line: &str) -> Option<f64>;
/// The remux, below normal, no limit; exit 0 or 1 is Ok, verification decides (design parts 5.3, 5.11). Def: T14. Use: T24.
pub fn remux(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path) -> io::Result<Result<(), String>>;
// crates/tools/src/ffprobe.rs
/// ["-v","error","-show_streams","-show_format","-of","json",<raw>] (design part 5.7 step 9). Def: T14. Use: T14, T24.
pub fn probe_args(raw: &Path) -> Vec<String>;
/// ffprobe under limits.header, below normal; exit 0 parses stdout. Def: T14. Use: T24.
pub fn probe(ctx: &ToolCtx, program: &Path, raw: &Path, out: &Path) -> io::Result<Result<FfprobeStreams, String>>;
// crates/tools/src/makemkv.rs
/// Saves MakeMKV's debug log as `<out>.makemkv-debug.txt` with the key replaced by "[app_Key removed]" (design part 7.2, PIPE-9). Def: T14. Use: T23.
pub fn save_debug_log(host: &dyn Host, started_unix: u64, out: &Path) -> Result<(), String>;
```

Rulings settled in this task:
- **S60. `remux_args` maps streams to track IDs through the staged report, and each layout track gets its per-track options in the layout's order.**
  - A `Layout` names scan streams (`OutputTrack.stream`, "the scan stream (0-based, within the title)"), but the options of design part 5.7 step 13 need the raw file's mkvmerge track IDs. Only the staged file's `VerifyReport.pairs` maps one to the other (`stream_of_track`, verify.rs:532). So `remux_args` takes `staged: &VerifyReport` and returns Err when `!staged.tracks_ok`, or when a layout stream has no paired track. Taking a stream's position in the selection as its track ID would be wrong after any `MSG:4047` removal or a MakeMKV reordering.
  - The options are `--language <id>:<language or und>`, `--track-name <id>:<name or empty>`, `--default-track-flag <id>:yes|no`, `--forced-display-flag <id>:yes|no`, `--hearing-impaired-flag <id>:yes|no` and `--original-flag <id>:yes|no`.
  - After all tracks come `--audio-tracks <ids>` (or `--no-audio` when there are none), `--subtitle-tracks <ids>` (or `--no-subtitles`), `--no-chapters` only when `!layout.chapters_kept`, then `--track-order 0:<id>,...`, then `<raw>`.
  - An empty name and `und` clear the names and languages MakeMKV wrote, because `verify_final` compares the absence of a name and the language through `norm_lang` (verify.rs:455-463).
  - Design part 5.7 step 13 lists no `--video-tracks`, so every raw video track is kept.
- **S61. `save_debug_log` copies bytes.** It replaces each byte occurrence of a non-empty key, and copies the log unchanged when the key is `None`. A log whose modification time in Unix seconds is below `started_unix` is older than the call. Why: design part 7.2 keeps the log "with the registration key removed", and a log need not be UTF-8.
- **S62. The debug-log tests use a private `Host` in `makemkv::tests`.** Task 1 gives `lazymkv-tools` no dev-dependency on the platform fakes.

- [ ] **Step 1: Write the failing tests**

First add `pub mod mkvmerge; pub mod ffprobe;` to `crates/tools/src/lib.rs`, so the files compile with their tests. Then, in `crates/tools/src/mkvmerge.rs`, add a `mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::disc::StreamKind;
    use lazymkv_core::tracks::layout::{Layout, OutputTrack};
    use lazymkv_core::verify::VerifyReport;
    use std::path::Path;

    fn track(stream: u32, kind: StreamKind, language: Option<&str>, name: Option<&str>, flags: [bool; 4]) -> OutputTrack {
        OutputTrack {
            stream, kind, language: language.map(str::to_string), name: name.map(str::to_string),
            default: flags[0], forced: flags[1], hearing_impaired: flags[2], original: flags[3],
        }
    }

    fn staged(pairs: Vec<(u32, Option<i64>)>, tracks_ok: bool) -> VerifyReport {
        VerifyReport {
            duration_ok: true, expected_seconds: 7150.0, file_seconds: 7150.0, chapters_ok: true, scan_chapters: 12,
            file_chapters: 12, tracks_ok, pairs, extra_tracks: vec![], unknown_removed: vec![], mismatches: vec![],
            unchecked: vec![], read_errors: vec![], ok: tracks_ok,
        }
    }

    fn layout(tracks: Vec<OutputTrack>) -> Layout {
        Layout { tracks, chapters_kept: true }
    }

    #[test]
    fn ident_args_are_dash_j_and_the_file() {
        assert_eq!(ident_args(Path::new("t.mkv")), ["-J", "t.mkv"]);
    }

    #[test]
    fn remux_args_start_with_gui_mode_and_the_partial_path() {
        let partial = Path::new("lib").join(".lazymkv-partial").join("j-1-t00-00800.mkv.partial");
        let args = remux_args(&partial, &layout(vec![track(0, StreamKind::Video, None, None, [true, false, false, false])]),
            &staged(vec![(0, Some(0))], true), Path::new("raw.mkv")).unwrap();
        assert_eq!(&args[..3], ["--gui-mode".to_string(), "-o".to_string(), partial.display().to_string()]);
        assert_eq!(args.last().unwrap(), "raw.mkv");
    }

    #[test]
    fn remux_args_write_every_flag_of_each_layout_track() {
        let tracks = vec![
            track(0, StreamKind::Video, None, None, [true, false, false, false]),
            track(1, StreamKind::Audio, Some("eng"), Some("DTS-HD MA 5.1"), [true, false, false, true]),
            track(3, StreamKind::Subtitles, Some("eng"), Some("English (Forced)"), [false, true, false, false]),
        ];
        let report = staged(vec![(0, Some(0)), (1, Some(1)), (2, Some(2)), (3, Some(4))], true);
        let args = remux_args(Path::new("p.partial"), &layout(tracks), &report, Path::new("raw.mkv")).unwrap();
        assert_eq!(args, [
            "--gui-mode", "-o", "p.partial",
            "--language", "0:und", "--track-name", "0:", "--default-track-flag", "0:yes", "--forced-display-flag", "0:no",
            "--hearing-impaired-flag", "0:no", "--original-flag", "0:no",
            "--language", "1:eng", "--track-name", "1:DTS-HD MA 5.1", "--default-track-flag", "1:yes",
            "--forced-display-flag", "1:no", "--hearing-impaired-flag", "1:no", "--original-flag", "1:yes",
            "--language", "4:eng", "--track-name", "4:English (Forced)", "--default-track-flag", "4:no",
            "--forced-display-flag", "4:yes", "--hearing-impaired-flag", "4:no", "--original-flag", "4:no",
            "--audio-tracks", "1", "--subtitle-tracks", "4", "--track-order", "0:0,0:1,0:4", "raw.mkv",
        ]);
    }

    #[test]
    fn a_title_without_subtitles_gets_no_subtitles() {
        let tracks = vec![
            track(0, StreamKind::Video, None, None, [true, false, false, false]),
            track(1, StreamKind::Audio, Some("eng"), Some("AC-3 5.1"), [true, false, false, true]),
        ];
        let args = remux_args(Path::new("p"), &layout(tracks), &staged(vec![(0, Some(0)), (1, Some(1))], true),
            Path::new("raw.mkv")).unwrap();
        assert!(args.windows(1).any(|w| w[0] == "--no-subtitles"));
        assert!(!args.iter().any(|arg| arg == "--subtitle-tracks"));
    }

    #[test]
    fn a_layout_stream_without_a_staged_track_is_refused() {
        let tracks = vec![track(2, StreamKind::Audio, Some("eng"), Some("AC-3 5.1"), [true, false, false, false])];
        assert_eq!(
            remux_args(Path::new("p"), &layout(tracks.clone()), &staged(vec![(2, None)], true), Path::new("raw.mkv")),
            Err("stream 2 has no track in the staged file".to_string())
        );
        assert_eq!(
            remux_args(Path::new("p"), &layout(tracks), &staged(vec![(2, Some(2))], false), Path::new("raw.mkv")),
            Err("the staged file's tracks did not match its scan title, so no track ID is known".to_string())
        );
    }

    #[test]
    fn gui_progress_reads_a_percentage() {
        assert_eq!(gui_progress("#GUI#progress 45%"), Some(0.45));
        assert_eq!(gui_progress("#GUI#progress 100%"), Some(1.0));
        assert_eq!(gui_progress("#GUI#progress 101%"), None);
        assert_eq!(gui_progress("#GUI#warning something"), None);
        assert_eq!(gui_progress("Progress: 45%"), None);
    }
}
```


In `crates/tools/src/ffprobe.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_args_are_v_error_show_streams_show_format_json() {
        assert_eq!(probe_args(std::path::Path::new("raw.mkv")),
            ["-v", "error", "-show_streams", "-show_format", "-of", "json", "raw.mkv"]);
    }
}
```

Add to `crates/tools/src/makemkv.rs`'s `mod tests`:

```rust
    use lazymkv_core::job::Tool;
    use lazymkv_platform::Host;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct KeyHost { log: Option<PathBuf>, key: Option<String> }

    impl Host for KeyHost {
        fn now_unix(&self) -> u64 { 0 }
        fn local_date(&self) -> String { String::new() }
        fn makemkv_app_key(&self) -> Option<String> { self.key.clone() }
        fn makemkv_debug_log(&self) -> Option<PathBuf> { self.log.clone() }
        fn running_processes(&self) -> Vec<String> { Vec::new() }
        fn volume_label(&self, _: &Path) -> Option<String> { None }
        fn default_install_path(&self, _: Tool) -> Option<PathBuf> { None }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lazymkv-tools-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    #[test]
    fn the_debug_log_is_saved_with_the_key_removed() {
        let dir = scratch("debug-key");
        let log = dir.join("MakeMKV_log.txt");
        std::fs::write(&log, b"key ABC and ABC again, caf\xe9\n").unwrap();
        let host = KeyHost { log: Some(log), key: Some("ABC".to_string()) };
        save_debug_log(&host, now() - 60, &dir.join("scan.txt")).unwrap();
        assert_eq!(std::fs::read(dir.join("scan.txt.makemkv-debug.txt")).unwrap(),
            b"key [app_Key removed] and [app_Key removed] again, caf\xe9\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_debug_log_older_than_the_call_is_not_saved() {
        let dir = scratch("debug-old");
        let log = dir.join("MakeMKV_log.txt");
        std::fs::write(&log, b"old").unwrap();
        let host = KeyHost { log: Some(log.clone()), key: None };
        assert_eq!(save_debug_log(&host, now() + 3600, &dir.join("scan.txt")),
            Err(format!("{}: the debug log is older than the call", log.display())));
        assert!(!dir.join("scan.txt.makemkv-debug.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_debug_log_is_reported_not_saved() {
        let dir = scratch("debug-missing");
        let log = dir.join("MakeMKV_log.txt");
        let host = KeyHost { log: Some(log.clone()), key: None };
        assert_eq!(save_debug_log(&host, now(), &dir.join("scan.txt")),
            Err(format!("{}: no debug log was written", log.display())));
        let none = KeyHost { log: None, key: None };
        assert_eq!(save_debug_log(&none, now(), &dir.join("scan.txt")),
            Err("MakeMKV's debug log path is not known".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
```

Create `crates/fake-tools/tests/mkvmerge_ffprobe_driver.rs`:

```rust
//! The mkvmerge and ffprobe drivers against the stand-in (design parts 5.3, 5.7, 5.11; Ruling S12).

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{ident, probe, remux, CancelToken, Limits, ToolCtx};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

const EXE: &str = std::env::consts::EXE_SUFFIX;
const IDENT: &str = r#"{"container":{"properties":{"duration":7150000000000}},"tracks":[{"id":0,"type":"video","codec":"MPEG-H/HEVC/H.265","properties":{"language":"und"}},{"id":1,"type":"audio","codec":"DTS-HD Master Audio","properties":{"language":"eng","audio_channels":6}}],"chapters":[{"num_entries":12}]}
"#;

fn stand_in(scratch: &Scratch, stem: &str, scenario: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let program = scratch.0.join("tools").join(format!("{stem}{EXE}"));
    scratch.write(&format!("tools/{stem}.json"), scenario);
    for (name, bytes) in files {
        scratch.write(&format!("tools/{name}"), bytes);
    }
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &program).unwrap();
    program
}

struct Env { host: FakeHost, limits: Limits, progress: Mutex<Vec<f64>> }

impl Env {
    fn new() -> Env {
        Env {
            host: FakeHost::new("2026-10-23", 1_800_000_000),
            limits: Limits { header: Duration::from_secs(120), scan: Duration::from_secs(60),
                exit_wait: Duration::from_secs(60), after_kill: Duration::from_millis(10) },
            progress: Mutex::new(Vec::new()),
        }
    }

    fn run<R>(&self, f: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |fraction: f64| self.progress.lock().unwrap().push(fraction);
        let ctx = ToolCtx { guard: &StdProcessGuard, host: &self.host, limits: &self.limits, cancel: CancelToken::default(),
            on_output: &on_output, on_progress: &on_progress };
        f(&ctx)
    }
}

#[test]
fn ident_accepts_exit_0_and_exit_1() {
    let scratch = Scratch::new("ident-0-1");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[
        {"args_contain":["-J","a.mkv"],"stdout":"ident.json"},
        {"args_contain":["-J","b.mkv"],"stdout":"ident.json","exit_code":1}]}"#, &[("ident.json", IDENT.as_bytes())]);
    for name in ["a.mkv", "b.mkv"] {
        let out = scratch.0.join(format!("job/{name}.json"));
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        let read = env.run(|ctx| ident(ctx, &program, &scratch.0.join(name), &out)).unwrap().unwrap();
        assert_eq!(read.tracks.len(), 2, "{name}");
        assert_eq!(read.container.properties.duration, 7_150_000_000_000);
        assert_eq!(fs::read_to_string(&out).unwrap(), IDENT);
    }
}

#[test]
fn ident_refuses_another_exit_code() {
    let scratch = Scratch::new("ident-2");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge",
        r#"{"responses":[{"args_contain":["-J"],"stdout":"ident.json","exit_code":2}]}"#, &[("ident.json", IDENT.as_bytes())]);
    let out = scratch.0.join("job/mkvmerge.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| ident(ctx, &program, &scratch.0.join("t.mkv"), &out)).unwrap().unwrap_err(),
        "mkvmerge -J exited with code 2");
}

#[test]
fn an_ident_past_the_header_limit_fails_saying_so() {
    let scratch = Scratch::new("ident-limit");
    let mut env = Env::new();
    env.limits.header = Duration::from_millis(300);
    let program = stand_in(&scratch, "mkvmerge",
        r#"{"responses":[{"args_contain":["-J"],"stdout":"ident.json","hang_after_line":0}]}"#, &[("ident.json", IDENT.as_bytes())]);
    let out = scratch.0.join("job/mkvmerge.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let result = env.run(|ctx| ident(ctx, &program, &scratch.0.join("t.mkv"), &out)).unwrap();
    assert!(result.unwrap_err().starts_with("mkvmerge -J did not finish within"));
}

#[test]
fn remux_progress_lines_reach_the_progress_callback() {
    let scratch = Scratch::new("remux");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[{"args_contain":["--gui-mode","-o"],"stdout":"remux.txt",
        "creates":[{"arg_after":"-o","copy_from":"placeholder.mkv"}]}]}"#,
        &[("remux.txt", b"#GUI#progress 45%\n#GUI#warning synthetic\n#GUI#progress 100%\n"), ("placeholder.mkv", b"placeholder bytes")]);
    let partial = scratch.0.join("lib/.lazymkv-partial/j-1-t00-00800.mkv.partial");
    fs::create_dir_all(partial.parent().unwrap()).unwrap();
    let args = vec!["--gui-mode".to_string(), "-o".to_string(), partial.display().to_string(), "raw.mkv".to_string()];
    let out = scratch.0.join("job/t00-00800/remux-mkvmerge.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| remux(ctx, &program, &args, &out)).unwrap(), Ok(()));
    assert_eq!(*env.progress.lock().unwrap(), vec![0.45, 1.0]);
    assert_eq!(fs::read(&partial).unwrap(), b"placeholder bytes");
}

#[test]
fn a_remux_accepts_exit_1_and_refuses_exit_2() {
    let scratch = Scratch::new("remux-exit");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[
        {"args_contain":["--gui-mode","one.partial"],"exit_code":1},
        {"args_contain":["--gui-mode","two.partial"],"exit_code":2}]}"#, &[]);
    let out = scratch.0.join("job/remux-mkvmerge.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let args = |name: &str| vec!["--gui-mode".to_string(), "-o".to_string(), name.to_string(), "raw.mkv".to_string()];
    assert_eq!(env.run(|ctx| remux(ctx, &program, &args("one.partial"), &out)).unwrap(), Ok(()));
    assert_eq!(env.run(|ctx| remux(ctx, &program, &args("two.partial"), &out)).unwrap(),
        Err("mkvmerge exited with code 2".to_string()));
}

#[test]
fn a_probe_parses_the_streams() {
    let scratch = Scratch::new("probe");
    let env = Env::new();
    let streams = br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc"},{"index":1,"codec_type":"audio","codec_name":"dts","profile":"DTS-HD MA"}],"format":{"duration":"7150.000000"}}
"#;
    let program = stand_in(&scratch, "ffprobe", r#"{"responses":[{"args_contain":["-show_streams"],"stdout":"streams.json"}]}"#,
        &[("streams.json", streams)]);
    let out = scratch.0.join("job/t00-00800/ffprobe-streams.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let read = env.run(|ctx| probe(ctx, &program, &scratch.0.join("raw.mkv"), &out)).unwrap().unwrap();
    assert_eq!(read.streams.len(), 2);
    assert_eq!(read.streams[1].codec_name.as_deref(), Some("dts"));
    assert_eq!(read.streams[1].profile.as_deref(), Some("DTS-HD MA"));
    assert_eq!(fs::read(&out).unwrap(), streams);
}
```

`MkvmergeIdent` derives no `PartialEq` (files.rs:80), so `ident_refuses_another_exit_code` compares the `unwrap_err()` text. `remux`'s `Result<(), String>` compares directly.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p fake-tools --locked`
Expected: compile error, with `cannot find function remux_args`, `probe_args` and `save_debug_log`, and `unresolved imports lazymkv_tools::ident, lazymkv_tools::probe, lazymkv_tools::remux` in `mkvmerge_ffprobe_driver.rs`.

- [ ] **Step 3: Implement the drivers and the debug-log save**

1. In `crates/tools/src/lib.rs`, add the re-exports `pub use mkvmerge::{gui_progress, ident, ident_args, remux, remux_args}; pub use ffprobe::{probe, probe_args};`. Add `save_debug_log` to the `makemkv` re-export.
2. Write the Produces items with their `///` docs ending "Def: T14. Use: ...".
3. `ident`: `run_recorded(ctx, program, &ident_args(file), out, Some(ctx.limits.header), true, &mut |_| LineAction::Continue)`. Then:
   - `run.cancelled` gives `Err(CANCELLED)`;
   - `run.timed_out` gives `Err(format!("mkvmerge -J did not finish within {} s and was killed", ctx.limits.header.as_secs()))`;
   - an exit code of 0 or 1 gives `serde_json::from_slice::<MkvmergeIdent>(&run.stdout)`, with the error `"{out}: {error}"` (out shown with `display()`);
   - any other code gives `Err(format!("mkvmerge -J exited with code {code}"))`, where a missing code shows as `none`.
4. `probe` follows the same pattern with `probe_args(raw)`. Exit 0 parses `FfprobeStreams`. The texts are `ffprobe did not finish within {} s and was killed` and `ffprobe exited with code {code}`.
5. `remux`: `run_recorded(ctx, program, args, out, None, true, ..)`. `on_line` forwards each `gui_progress(line)` value to `ctx.on_progress`. Cancelled gives `Err(CANCELLED)`. Exit 0 or 1 gives `Ok(())` (PIPE-14). Any other code gives `Err(format!("mkvmerge exited with code {code}"))`.
6. `remux_args` follows S60. Each track's ID is the `Some(id)` of `staged.pairs`' entry for `track.stream`. Check `staged.tracks_ok` first, then each stream, with the two error texts in the test.
7. `gui_progress`: strip the prefix `#GUI#progress ` and the suffix `%`, parse a `u32` no greater than 100, and divide by 100.
8. `save_debug_log` follows S61. The order of checks:
   1. `host.makemkv_debug_log()` is None: `MakeMKV's debug log path is not known`.
   2. The metadata is `NotFound`: `<path>: no debug log was written`.
   3. The modification time is below `started_unix`: `<path>: the debug log is older than the call`.
   4. Any other I/O error: `<path>: <error>`.

   Otherwise it writes the replaced bytes to `with_suffix(out, ".makemkv-debug.txt")` with `std::fs::write`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-tools -p fake-tools --locked`
Expected: PASS. `lazymkv-tools` shows 6 `mkvmerge::tests`, 1 `ffprobe::tests` and 3 new `makemkv::tests` passing. `Running tests/mkvmerge_ffprobe_driver.rs` reports `test result: ok. 6 passed; 0 failed`. Task 13's tests are unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/mkvmerge.rs crates/tools/src/ffprobe.rs crates/tools/src/makemkv.rs crates/tools/src/lib.rs crates/fake-tools/tests/mkvmerge_ffprobe_driver.rs
git commit -m "tools: mkvmerge and ffprobe drivers and MakeMKV's debug log (design parts 5.7, 7.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, then the noreply identity twice, and no attribution line.

### Task 15: The transport, the recorded transport and single lookups (design parts 8.1, 8.3, 8.4)

**Files:**
- Create: `crates/clients/src/transport.rs` (1 test), `crates/clients/src/recorded.rs` (4 tests), `crates/clients/src/lookups.rs` (9 tests) and `crates/clients/src/saved.rs` (4 tests).
- Modify: `crates/clients/src/lib.rs`, adding the modules and the crate-root re-exports.

**Interfaces:**
- Consumes:
  - From `crates/core/src/lookup/discdb.rs`:
    - `parse_dischash(text: &str, shown: &str) -> Result<Option<DiscDbRecord>, String>` (:328);
    - `parse_discid(text: &str, shown: &str) -> Result<Vec<DiscDbRecord>, String>` (:345);
    - `DiscDbRecord::tmdb_identity(&self) -> Option<TmdbIdentity>` (:281).
  - From `crates/core/src/lookup/tmdb.rs`:
    - `parse_search(text: &str, shown: &str) -> Result<TmdbSearch, String>` (:131);
    - `parse_movie` (:278) and `parse_tv` (:299), both `(text: &str, shown: &str) -> Result<TmdbRecord, String>`;
    - `parse_recorded` (:388);
    - `TmdbIdentity { kind, id }` (:54) and `TmdbKind { Movie, Tv }` with `as_str()` (:22).
  - Dev's `lookup-status.txt` reading (crates/core/src/recording/files.rs:309): first word is the key, last word the status. An answer file is parsed only when its line says `200` or is absent (crates/dev/src/cmd/plan.rs:300). Any word other than `200` and `404` reads as failed (crates/dev/src/cmd/plan.rs:504-511).
- Produces (crate root re-exports, signatures as in the interface sheet, with these exact forms):

```rust
// transport.rs
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Response { pub status: u16, pub body: Vec<u8> }
#[derive(Clone, Debug, PartialEq, Eq)] pub enum TransportError { Timeout, Unreachable(String), Unrecorded(String) }
pub trait HttpTransport: Send + Sync { fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError>; }
pub const TIMEOUT_S: u64 = 10;
pub fn user_agent() -> String; // "lazymkv/" + env!("CARGO_PKG_VERSION")
// recorded.rs
pub struct RecordedTransport { /* private: Mutex'd answers per URL with counters, request log, unrecorded list, gates */ }
impl RecordedTransport {
    pub fn new() -> RecordedTransport;
    pub fn answer(&self, url: &str, status: u16, body: &[u8]);
    pub fn fail(&self, url: &str, error: TransportError);
    pub fn from_file(path: &Path, vars: &[(&str, &str)]) -> Result<RecordedTransport, String>;
    pub fn gate(&self, url: &str) -> Gate;
    pub fn requests(&self) -> Vec<(String, Vec<(String, String)>)>;
    pub fn unrecorded(&self) -> Vec<String>;
}
impl Default for RecordedTransport; impl HttpTransport for RecordedTransport;
#[derive(Clone)] pub struct Gate(/* Arc<(Mutex<bool>, Condvar)> */);
impl Gate { pub fn open(&self); }
// lookups.rs
pub const DISCDB: &str = "https://thediscdb.com/api";
pub const TMDB: &str = "https://api.themoviedb.org/3";
pub fn dischash_url(fingerprint: &str) -> String;      // {DISCDB}/dischash/{fingerprint upper-cased}
pub fn discid_url(disc_id: &str) -> String;            // {DISCDB}/discid/{disc_id as given}
pub fn search_url(query: &str) -> String;              // {TMDB}/search/multi?query=<percent-encoded> (S20)
pub fn record_url(id: TmdbIdentity) -> String;         // {TMDB}/{movie|tv}/{id}?append_to_response=external_ids
pub fn authentication_url() -> String;                 // {TMDB}/authentication
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Fetched { pub url: String, pub status: u16, pub body: Vec<u8> }
#[derive(Clone, Debug, PartialEq, Eq)] pub enum PendingReason { NoToken, TokenRefused, Timeout, Unreachable(String), Status(u16), Unparsed(String) }
#[derive(Clone, Debug, PartialEq)] pub enum Outcome<T> { Answered { value: T, fetched: Fetched }, Miss { fetched: Fetched }, Pending { reason: PendingReason, fetched: Option<Fetched> } }
pub fn discdb_by_fingerprint(t: &dyn HttpTransport, fingerprint: &str) -> Outcome<DiscDbRecord>;
pub fn discdb_by_disc_id(t: &dyn HttpTransport, disc_id: &str) -> Outcome<Vec<DiscDbRecord>>;
pub fn tmdb_search(t: &dyn HttpTransport, token: Option<&str>, query: &str) -> Outcome<TmdbSearch>;
pub fn tmdb_record(t: &dyn HttpTransport, token: Option<&str>, id: TmdbIdentity) -> Outcome<TmdbRecord>;
// saved.rs
#[derive(Clone, Debug, PartialEq, Eq)] pub struct SavedFile { pub name: String, pub bytes: Vec<u8> }
pub fn slim_tmdb(raw: &serde_json::Value, today: &str) -> serde_json::Value;
pub fn slim_record(raw: &serde_json::Value, kind: TmdbKind, today: &str) -> serde_json::Value;
pub fn slim_search(raw: &serde_json::Value, query: &str, today: &str) -> serde_json::Value;
pub fn status_text(lines: &BTreeMap<String, String>) -> String; // "<key> <status>\n" per entry, in key order
pub(crate) fn status_word<T>(outcome: &Outcome<T>) -> String;  // crate-private; Task 16's chain writes it
```

Each `pub` item's `///` doc names design part 8.1, 8.3 or 8.4 (or 7.1 for the slim files) and ends "Def: T15. Use: T16" (plus T23, T26, T29 where they call it).

Rulings settled in this task:
- **S63. Answers can be added to a live transport.** `answer` and `fail` take `&self`, so a test adds answers to the transport the engine already holds (`Harness::http()` returns `&RecordedTransport`). In `http.json`, `"error":"timeout"` gives `Timeout` and any other error text gives `Unreachable(text)`. Each `{name}` in a URL is replaced from `vars`. An entry with neither `status` nor `error` is refused, naming its index. Why: design part 8.1 only says "answers keyed by URL".
- **S64. Every request is logged when it arrives, before its gate is waited on.** A test can therefore see a request in flight. Why: Task 23's `the_scan_does_not_wait_for_a_slow_lookup` needs that.
- **S65. How a fetch maps to an outcome.**
  - `Unrecorded(url)` is `Pending { Unreachable("no recorded answer for <url>"), None }`, because `PendingReason` has no recorded-only variant and design part 8.3 makes every missing answer pending.
  - A 401 is `TokenRefused` only for TMDb. TheDiscDB's 401 is `Status(401)`, because TheDiscDB needs no credential (design part 8.1).
  - A discid 200 holding `[]` is `Answered` with an empty list, as `parse_discid` reads it.
  - A body that is not UTF-8 is `Unparsed("<url>: the body is not UTF-8")`.
- **S66. Status words.** `status_word` gives:
  - `Answered` and `Miss`: the HTTP status;
  - `Pending` with `Unparsed`: `unparsed`;
  - `Pending` with a fetched answer: its status;
  - `Pending` with no fetch, including `NoToken`: `000`.

  Why: S21 makes no answer `000`, and dev reads `000` as failed, so the lookup reads as pending (design part 8.3 makes a missing token pending).
- **S67. `search_url` percent-encodes every UTF-8 byte except `A-Z a-z 0-9 - . _ ~`, as `%XX` in upper-case hex.** There is no encoding crate (design part 4.6).
- **S68. The slim shapes.**
  - `slim_tmdb` keeps phase 0's keys in phase 0's order: `id`, `imdb_id`, `title`, `original_title`, `release_date`, `name`, `original_name`, `first_air_date`, `original_language`, each when present and not null. It then writes `external_ids` with `imdb_id`, `tvdb_id` and `wikidata_id`, each only when not null, `""`, `0` or `false` (PowerShell's `if`), and only when one remains. Last comes `fetched_at` (scripts/phase0/record-disc.ps1:98-106).
  - `slim_record` is `slim_tmdb`'s keys, then `kind`, then `runtime` when present and not null. A tv record then gets `seasons`, each holding `season_number`, `episode_count` when present and `episodes[]{episode_number, runtime}` when the raw season has `episodes`. `fetched_at` comes last.
  - `slim_search` keeps only the `results` whose `media_type` is `movie` or `tv`. Each keeps `media_type`, `id` and then the kind's `title`, `original_title` and `release_date` (movie) or `name`, `original_name` and `first_air_date` (tv), each when present and not null. The page then gets `"queries":[query]` and `fetched_at`.
  - Why: design parts 7.1 and 8.4 list these fields, and P29 keeps no full answer.
- **S69. The `lazymkv-clients` tests write their bodies inline and use a private scratch folder.** Task 1 gives the crate no dev-dependency on `lazymkv-core/testsupport`. The one public TheDiscDB answer is read from `corpus/thediscdb-api`.

- [ ] **Step 1: Write the failing tests**

First add `pub mod transport; pub mod recorded; pub mod lookups; pub mod saved;` to `crates/clients/src/lib.rs`, so that each new file below compiles with its tests.

`crates/clients/src/transport.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_lazymkv_and_its_version() {
        assert_eq!(user_agent(), format!("lazymkv/{}", env!("CARGO_PKG_VERSION")));
        assert_eq!(TIMEOUT_S, 10);
    }
}
```

`crates/clients/src/recorded.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookups::{discdb_by_disc_id, discdb_by_fingerprint, dischash_url, discid_url, search_url, tmdb_search, Outcome, PendingReason};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn several_answers_for_one_url_are_served_in_order_then_the_last_repeats() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url("ABC"), 503, b"");
        t.answer(&dischash_url("ABC"), 404, b"");
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"),
            Outcome::Pending { reason: PendingReason::Status(503), fetched: Some(_) }));
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"), Outcome::Miss { .. }));
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"), Outcome::Miss { .. }));
    }

    #[test]
    fn an_unrecorded_url_is_pending_and_listed() {
        let t = RecordedTransport::new();
        let url = discid_url("D1");
        assert_eq!(discdb_by_disc_id(&t, "D1"),
            Outcome::Pending { reason: PendingReason::Unreachable(format!("no recorded answer for {url}")), fetched: None });
        assert_eq!(t.unrecorded(), vec![url]);
    }

    #[test]
    fn a_gated_request_waits_until_opened() {
        let t = std::sync::Arc::new(RecordedTransport::new());
        t.answer(&dischash_url("ABC"), 404, b"");
        let gate = t.gate(&dischash_url("ABC"));
        let (sent, got) = mpsc::channel();
        let asking = t.clone();
        std::thread::spawn(move || sent.send(discdb_by_fingerprint(&*asking, "ABC")).unwrap());
        assert!(got.recv_timeout(Duration::from_millis(200)).is_err());
        assert_eq!(t.requests().len(), 1);
        gate.open();
        assert!(matches!(got.recv_timeout(Duration::from_secs(5)).unwrap(), Outcome::Miss { .. }));
    }

    #[test]
    fn http_json_answers_load_with_body_files_and_vars() {
        let dir = std::env::temp_dir().join(format!("lazymkv-clients-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("search.json"), r#"{"page":1,"results":[{"media_type":"movie","id":900001,"title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03"}]}"#).unwrap();
        std::fs::write(dir.join("http.json"), r#"{"answers":[
            {"url":"https://thediscdb.com/api/dischash/{fingerprint}","status":404,"body":""},
            {"url":"https://thediscdb.com/api/discid/{disc_id}","error":"timeout"},
            {"url":"https://api.themoviedb.org/3/search/multi?query=SYNTHETIC%20MOVIE","status":200,"body_file":"search.json"}]}"#).unwrap();
        let t = RecordedTransport::from_file(&dir.join("http.json"), &[("fingerprint", "ABC"), ("disc_id", "D1")]).unwrap();
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"), Outcome::Miss { .. }));
        assert!(matches!(discdb_by_disc_id(&t, "D1"), Outcome::Pending { reason: PendingReason::Timeout, fetched: None }));
        match tmdb_search(&t, Some("token"), "SYNTHETIC MOVIE") {
            Outcome::Answered { value, fetched } => {
                assert_eq!(value.results.len(), 1);
                assert_eq!(fetched.url, search_url("SYNTHETIC MOVIE"));
            }
            other => panic!("expected Answered, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

`crates/clients/src/lookups.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorded::RecordedTransport;
    use crate::transport::{user_agent, TransportError};
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};

    const FP: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
    const MOVIE: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
    const MOVIE_BODY: &[u8] = br#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","runtime":119,"external_ids":{"imdb_id":"tt0900001"}}"#;

    #[test]
    fn a_404_dischash_is_a_miss() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 404, b"");
        assert_eq!(discdb_by_fingerprint(&t, FP),
            Outcome::Miss { fetched: Fetched { url: dischash_url(FP), status: 404, body: vec![] } });
    }

    #[test]
    fn a_dischash_200_with_an_empty_body_is_a_miss() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 200, b"");
        assert!(matches!(discdb_by_fingerprint(&t, FP), Outcome::Miss { fetched } if fetched.status == 200));
    }

    #[test]
    fn a_dischash_answer_parses() {
        let body = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/thediscdb-api/dischash-76E63EBB5B66F73B0C37BFAB59B8ABA5.json")).unwrap();
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 200, &body);
        match discdb_by_fingerprint(&t, FP) {
            Outcome::Answered { value, fetched } => {
                assert_eq!(value.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::Movie, id: 245891 }));
                assert_eq!(fetched.body, body);
            }
            other => panic!("expected Answered, got {other:?}"),
        }
    }

    #[test]
    fn the_dischash_url_uses_the_upper_case_fingerprint() {
        assert_eq!(dischash_url("76e63ebb5b66f73b0c37bfab59b8aba5"),
            "https://thediscdb.com/api/dischash/76E63EBB5B66F73B0C37BFAB59B8ABA5");
        assert_eq!(discid_url("D1"), "https://thediscdb.com/api/discid/D1");
        assert_eq!(record_url(MOVIE), "https://api.themoviedb.org/3/movie/900001?append_to_response=external_ids");
        assert_eq!(record_url(TmdbIdentity { kind: TmdbKind::Tv, id: 5 }),
            "https://api.themoviedb.org/3/tv/5?append_to_response=external_ids");
        assert_eq!(authentication_url(), "https://api.themoviedb.org/3/authentication");
        assert_eq!(search_url("Pan's Labyrinth: \u{d1}"),
            "https://api.themoviedb.org/3/search/multi?query=Pan%27s%20Labyrinth%3A%20%C3%91");
    }

    #[test]
    fn no_token_makes_no_tmdb_request() {
        let t = RecordedTransport::new();
        assert_eq!(tmdb_search(&t, None, "SYNTHETIC MOVIE"), Outcome::Pending { reason: PendingReason::NoToken, fetched: None });
        assert_eq!(tmdb_record(&t, None, MOVIE), Outcome::Pending { reason: PendingReason::NoToken, fetched: None });
        assert!(t.requests().is_empty());
        assert!(t.unrecorded().is_empty());
    }

    #[test]
    fn a_401_is_pending_with_token_refused() {
        let t = RecordedTransport::new();
        t.answer(&record_url(MOVIE), 401, b"{\"status_code\":7}");
        assert!(matches!(tmdb_record(&t, Some("bad"), MOVIE),
            Outcome::Pending { reason: PendingReason::TokenRefused, fetched: Some(f) } if f.status == 401));
        t.answer(&dischash_url(FP), 401, b"");
        assert!(matches!(discdb_by_fingerprint(&t, FP), Outcome::Pending { reason: PendingReason::Status(401), .. }));
    }

    #[test]
    fn a_200_that_does_not_parse_is_pending_with_the_parsers_sentence() {
        let t = RecordedTransport::new();
        t.answer(&search_url("X"), 200, b"{\"results\":5}");
        match tmdb_search(&t, Some("token"), "X") {
            Outcome::Pending { reason: PendingReason::Unparsed(sentence), fetched: Some(fetched) } => {
                assert!(sentence.starts_with(&format!("{}: ", search_url("X"))), "{sentence}");
                assert_eq!(fetched.status, 200);
            }
            other => panic!("expected Pending Unparsed, got {other:?}"),
        }
        t.fail(&record_url(MOVIE), TransportError::Timeout);
        assert_eq!(tmdb_record(&t, Some("token"), MOVIE), Outcome::Pending { reason: PendingReason::Timeout, fetched: None });
    }

    #[test]
    fn a_movie_record_parses_by_its_kind() {
        let t = RecordedTransport::new();
        t.answer(&record_url(MOVIE), 200, MOVIE_BODY);
        match tmdb_record(&t, Some("token"), MOVIE) {
            Outcome::Answered { value, .. } => {
                assert_eq!(value.title, "Synthetic Movie");
                assert_eq!(value.runtime, Some(119));
                assert_eq!(value.year, Some(2001));
            }
            other => panic!("expected Answered, got {other:?}"),
        }
    }

    #[test]
    fn every_request_carries_the_user_agent_and_tmdb_the_bearer_token() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 404, b"");
        t.answer(&record_url(MOVIE), 200, MOVIE_BODY);
        discdb_by_fingerprint(&t, FP);
        tmdb_record(&t, Some("test-token"), MOVIE);
        let requests = t.requests();
        let ua = ("User-Agent".to_string(), user_agent());
        assert_eq!(requests[0], (dischash_url(FP), vec![ua.clone()]));
        assert_eq!(requests[1], (record_url(MOVIE), vec![("Authorization".to_string(), "Bearer test-token".to_string()), ua]));
    }
}
```

`crates/clients/src/saved.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookups::{discdb_by_disc_id, discdb_by_fingerprint, discid_url, dischash_url, record_url, tmdb_record, tmdb_search};
    use crate::recorded::RecordedTransport;
    use crate::transport::TransportError;
    use lazymkv_core::lookup::tmdb::{parse_movie, parse_recorded, parse_search, parse_tv, TmdbIdentity, TmdbKind};
    use serde_json::json;

    fn movie_raw() -> serde_json::Value {
        json!({"adult": false, "id": 900001, "imdb_id": "tt0900001", "original_language": "en",
            "original_title": "Synthetic Movie", "overview": "synthetic", "release_date": "2001-02-03", "runtime": 119,
            "title": "Synthetic Movie",
            "external_ids": {"imdb_id": "tt0900001", "wikidata_id": "Q900001", "facebook_id": null, "tvdb_id": null}})
    }

    #[test]
    fn slim_tmdb_keeps_phase_0s_keys_in_order_with_fetched_at() {
        let slim = slim_tmdb(&movie_raw(), "2026-10-23");
        let text = serde_json::to_string(&slim).unwrap();
        assert_eq!(text, r#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","external_ids":{"imdb_id":"tt0900001","wikidata_id":"Q900001"},"fetched_at":"2026-10-23"}"#);
        let record = parse_recorded(&text, "tmdb.json").unwrap();
        assert_eq!((record.title.as_str(), record.year), ("Synthetic Movie", Some(2001)));
    }

    #[test]
    fn slim_record_adds_kind_and_runtime_and_parses_back() {
        let text = serde_json::to_string(&slim_record(&movie_raw(), TmdbKind::Movie, "2026-10-23")).unwrap();
        assert_eq!(text, r#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","external_ids":{"imdb_id":"tt0900001","wikidata_id":"Q900001"},"kind":"movie","runtime":119,"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_movie(&text, "tmdb-record.json").unwrap().runtime, Some(119));
        let tv = json!({"id": 5, "name": "Show", "original_name": "Show", "first_air_date": "2002-01-01",
            "original_language": "en", "seasons": [{"season_number": 1, "episode_count": 2, "name": "S1", "poster_path": "x"}]});
        let tv_text = serde_json::to_string(&slim_record(&tv, TmdbKind::Tv, "2026-10-23")).unwrap();
        assert_eq!(tv_text, r#"{"id":5,"name":"Show","original_name":"Show","first_air_date":"2002-01-01","original_language":"en","kind":"tv","seasons":[{"season_number":1,"episode_count":2}],"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_tv(&tv_text, "tmdb-record.json").unwrap().seasons.len(), 1);
    }

    #[test]
    fn slim_search_keeps_movie_and_tv_results_with_queries() {
        let raw = json!({"page": 1, "total_results": 3, "results": [
            {"media_type": "movie", "id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie",
             "release_date": "2001-02-03", "popularity": 1.5},
            {"media_type": "person", "id": 7, "name": "Someone"},
            {"media_type": "tv", "id": 5, "name": "Show", "original_name": "Show", "first_air_date": "2002-01-01"}]});
        let text = serde_json::to_string(&slim_search(&raw, "SYNTHETIC MOVIE", "2026-10-23")).unwrap();
        assert_eq!(text, r#"{"results":[{"media_type":"movie","id":900001,"title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03"},{"media_type":"tv","id":5,"name":"Show","original_name":"Show","first_air_date":"2002-01-01"}],"queries":["SYNTHETIC MOVIE"],"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_search(&text, "tmdb-search.json").unwrap().results.len(), 2);
    }

    #[test]
    fn the_status_file_writes_000_for_no_answer() {
        let movie = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
        let t = RecordedTransport::new();
        t.fail(&dischash_url("ABC"), TransportError::Timeout);
        t.answer(&discid_url("D1"), 404, b"");
        t.answer(&record_url(movie), 200, b"{}");
        let mut lines = BTreeMap::new();
        lines.insert("dischash".to_string(), status_word(&discdb_by_fingerprint(&t, "ABC")));
        lines.insert("discid".to_string(), status_word(&discdb_by_disc_id(&t, "D1")));
        lines.insert("search".to_string(), status_word(&tmdb_search(&t, None, "SYNTHETIC MOVIE")));
        lines.insert("tmdb movie/900001".to_string(), status_word(&tmdb_record(&t, Some("token"), movie)));
        assert_eq!(status_text(&lines), "dischash 000\ndiscid 404\nsearch 000\ntmdb movie/900001 unparsed\n");
    }

}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked`
Expected: compile error, with `cannot find type RecordedTransport` and `cannot find function slim_tmdb`.

- [ ] **Step 3: Implement the four modules**

1. `lib.rs`: re-export every Produces item at the crate root (`status_word` stays `pub(crate)`).
2. `transport.rs` holds the items as listed.
3. `recorded.rs`:
   - One `Mutex` holds, per URL, a `Vec<Result<Response, TransportError>>` and a served count. It also holds the request log, the unrecorded list and the gates.
   - `get` follows S64: log the request with its headers as owned pairs, wait on the URL's gate if one is set and not yet open, then serve answer `min(count, len - 1)` and increase the count.
   - A URL with no answers gives `Err(Unrecorded(url))` and is pushed to `unrecorded`.
   - `from_file` reads `{"answers":[..]}` (S63). `body` is a string's UTF-8 bytes. `body_file` is read relative to the folder of `http.json`, and a missing file gives `Err("<path>: <error>")`. With neither, the body is empty.
4. `lookups.rs`:
   - A private `fetch(t, url, headers) -> Result<Fetched, PendingReason>` maps transport errors per S65.
   - TheDiscDB headers are `[("User-Agent", ua)]`. TMDb headers are `[("Authorization", "Bearer <token>"), ("User-Agent", ua)]`. A `None` token returns `Pending { NoToken, None }` before any request.
   - The status maps:
     - 404 gives `Miss`;
     - a TMDb 401 gives `TokenRefused`;
     - another non-200 gives `Status(code)`;
     - 200 decodes the body as UTF-8 and parses it with the core parser, passing `shown` = the URL.
   - Parser results: `parse_dischash` `Ok(None)` gives `Miss` (S46). `tmdb_record` calls `parse_movie` for `TmdbKind::Movie` and `parse_tv` for `Tv`. A parser error gives `Pending { Unparsed(sentence), Some(fetched) }`.
   - The URL builders follow the comments in Produces and S67.
5. `saved.rs` follows S66 and S68. `status_text` writes `"{key} {status}\n"` per map entry, in the map's order.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked`
Expected: PASS, `test result: ok. 18 passed; 0 failed` for the `lazymkv-clients` unit tests: 1 in `transport`, 4 in `recorded`, 9 in `lookups` and 4 in `saved`.

- [ ] **Step 5: Commit**

```bash
git add crates/clients/src/lib.rs crates/clients/src/transport.rs crates/clients/src/recorded.rs crates/clients/src/lookups.rs crates/clients/src/saved.rs
git commit -m "clients: HTTP transport, recorded answers, TheDiscDB and TMDb lookups and their saved copies (design parts 8.1, 8.3, 8.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, then the noreply identity twice, and no attribution line.

### Task 16: The movie lookup chain and its rounds (design parts 8.2, 8.3)

**Files:**
- Modify: `crates/core/src/lookup/mod.rs`. Add `record_to_fetch` right after `identify`, which ends at `:453`.
- Modify: `crates/core/src/lookup/tests.rs`. Add 5 tests after `an_entry_without_a_tmdb_id_and_several_results_is_ambiguous` (`:696`).
- Create: `crates/clients/src/chain.rs`. It holds the code and its `mod tests` (12 tests).
- Modify: `crates/clients/src/lib.rs`. Add `pub mod chain;` and `pub use chain::{run_round, Answers, ChainInputs, Lookup, LookupBook, LookupState};`.

**Interfaces:**

Consumes (core, existing):
- `LookupInputs<'a> { name, discdb_by_disc_id, discdb_by_fingerprint, discdb_pending, search, record, tmdb_pending, remembered, reviewed }` (`crates/core/src/lookup/mod.rs:350`)
- `identify` (`:415`)
- The private helpers `discdb_match(inputs, evidence) -> Option<(IdentityStep, &DiscDbRecord)>` (`:534`), `plausible_by_name(search, disc_name, evidence)` (`:616`) and `search_by_entry(inputs, step, record, title, year, evidence) -> IdentityFound` (`:666`)
- `DiscDbRecord::{tmdb_identity, title_and_year}` (`crates/core/src/lookup/discdb.rs:281`, `:300`) and `DiscDbRecord.titles` (`:265`)
- `parse_discid` (`discdb.rs:345`)
- `TmdbIdentity` (`tmdb.rs:54`; it derives `Copy`, `Ord`) and `TmdbKind::as_str`
- `name::{disc_name, DiscName}` (`name.rs:367`, `:338`)

Consumes (Task 15, as it re-exports them at the `lazymkv_clients` root):
- `HttpTransport`
- `RecordedTransport`:
  - `new() -> RecordedTransport`
  - `answer(&self, url: &str, status: u16, body: &[u8])`
  - `fail(&self, url: &str, error: TransportError)`
  - `gate(&self, url: &str) -> Gate`, with `Gate::open()`
  - `requests(&self) -> Vec<(String, Vec<(String, String)>)>`
- `TransportError::Timeout`
- The URL builders `dischash_url`, `discid_url`, `search_url`, `record_url`
- `Fetched { url, status, body }`
- `PendingReason { NoToken, TokenRefused, Timeout, Unreachable(String), Status(u16), Unparsed(String) }`, which must derive `Clone, Debug, PartialEq`
- `Outcome<T> { Answered { value, fetched }, Miss { fetched }, Pending { reason, fetched } }`
- The clients `discdb_by_fingerprint`, `discdb_by_disc_id`, `tmdb_search`, `tmdb_record`
- `SavedFile { name, bytes }`
- The slim writers `slim_tmdb`, `slim_record`, `slim_search`
- `status_text(lines: &BTreeMap<String, String>) -> String`, which writes one `<key> <status>` line per entry

Produces (core):
```rust
/// The TMDb record spec 8.1's chain fetches next for the answers in `inputs` (design part 8.2 items 3 and 4), by the
/// same rule `identify` applies, so the fetched record and the plan's identity never disagree: the owner's reviewed
/// identity, else the remembered one; else the TheDiscDB match's TMDb ID (`discdb_match`); else, for a match without a
/// TMDb ID but with a title, the one result `search_by_entry` keeps after preferring the entry's kind and year, read
/// over `inputs.search`; else, for a non-generic disc name, the one result `plausible_by_name` leaves. None when
/// several results stay plausible, when no search answer is in hand, or for a generic or absent name. Equals
/// `identify(..).tmdb` except when the disc name's search leaves several plausible results. Def: T16. Use: T16.
pub fn record_to_fetch(inputs: &LookupInputs<'_>) -> Option<tmdb::TmdbIdentity>;
```

Produces (clients, in `crates/clients/src/chain.rs`):
```rust
/// One request of the movie chain (design part 8.2 items 1 to 4). Def: T16. Use: T23, T24, T25.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lookup { Dischash, Discid, Search, Record }
/// Where one lookup stands (design part 8.3). Def: T16. Use: T23, T24, T25.
#[derive(Clone, Debug, PartialEq)]
pub enum LookupState { NotAsked, InFlight, Answered { status: u16 }, Pending { reason: PendingReason } }
/// The answers in hand, parsed by core. Def: T16. Use: T23, T24.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answers { pub by_fingerprint: Option<DiscDbRecord>, pub by_disc_id: Vec<DiscDbRecord>,
    pub search: Option<TmdbSearch>, pub record: Option<TmdbRecord> }
/// A job's lookups. A lookup absent from `states` is not part of this disc's chain (a DVD's discid, a generic name's
/// search, a record with no identity to fetch). Def: T16. Use: T23, T24, T25.
#[derive(Clone, Debug, Default)]
pub struct LookupBook { pub states: BTreeMap<Lookup, LookupState>, pub answers: Answers }
/// What the chain asks with (design part 8.2). Def: T16. Use: T23.
#[derive(Clone, Debug)]
pub struct ChainInputs { pub fingerprint: String, pub disc_id: Option<String>, pub name: Option<DiscName> }
impl LookupBook {
    /// (discdb_pending, tmdb_pending) as `LookupInputs` takes them (design part 8.3). Def: T16. Use: T23, T24.
    pub fn pending(&self) -> (bool, bool);
    /// The book as core's `LookupInputs`, with the flags of `pending`. Def: T16. Use: T16, T23, T24.
    pub fn lookup_inputs<'a>(&'a self, name: Option<&'a DiscName>) -> LookupInputs<'a>;
}
/// One round of design part 8.3: asks every lookup of the chain that is not answered, in the order of design part 8.2,
/// saving each answer and the status file as they come. Def: T16. Use: T23, T24, T25, T26.
pub fn run_round(t: &dyn HttpTransport, token: Option<&str>, book: &mut LookupBook, inputs: &ChainInputs,
    today: &str, save: &mut dyn FnMut(SavedFile));
```

`lookup_inputs` spells out its lifetime, as the sheet does, because elision cannot tie `name` to the output.

Rulings for this task:
- **Ruling:** `pending()` treats an absent `Dischash` as "no round has started", so `(true, true)`. A Plan that runs before the lookup thread's first request then reads both lookups as pending, which design part 8.3 requires ("Planning never waits").
- **Ruling:** a record that answered 404 is asked again in the next round. The book keeps no identity for a miss, so this is the only way a later change of identity reaches its record, and rounds are few (design part 8.3).
- **Ruling:** the status word is `000` for `InFlight` and for `Pending` with `Timeout`, `Unreachable` or `NoToken`. It is `401` for `TokenRefused`, the code for `Status(code)`, and `unparsed` for `Unparsed`. dev reads every one of these as failed (`plan.rs:506`), so dev and the engine both see the lookup as pending (S21, S23).
- **Ruling:** in PR 2 a TheDiscDB entry with a title but no TMDb ID gets no entry-title search (S47). `record_to_fetch` then reads the name search's page as the entry's search, which is exactly how `identify` reads the same inputs at Plan, so the record and the plan stay consistent.
- **Ruling:** TheDiscDB bodies are saved as received for every `Fetched`, including a 404's empty body, as phase 0's `curl -o` did. A TMDb answer is saved only when `Answered`, so no error body is kept (P29).
- **Ruling:** saved JSON is `serde_json::to_vec_pretty` of the slim value plus a final `\n`. dev's readers accept any JSON layout.

Locking: `run_round` takes `&mut LookupBook` for the whole round, so a job's `JobContext.lookups: Arc<Mutex<LookupBook>>` stays locked while a request is held, up to 10 s each. Task 23 settles how Plan reads it (R23.1): Plan takes the book with `try_lock`, and a book held by a running round counts as an empty book, so every lookup reads as pending (design part 8.3).

- [ ] **Step 1: Write the failing core tests in `crates/core/src/lookup/tests.rs`**

Insert these after `an_entry_without_a_tmdb_id_and_several_results_is_ambiguous`. They use the file's helpers `without_tmdb_id` (`:386`), `search` (`:402`), `movie` (`:393`), `label` (`:417`), `by_fingerprint` (`:377`), `JOHN_WICK_UHD_BY_FINGERPRINT` and `JOHN_WICK_RESULTS`.

```rust
#[test]
fn record_to_fetch_prefers_the_entry_kind_and_year() {
    let fingerprint = without_tmdb_id("Example Film", 1986);
    let results = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("movie", 1002, "Example Film", "Example Film", "2021-03-01"),
        ("tv", 1003, "Example Film", "Example Film", "2020-01-24"),
    ]);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&results), ..LookupInputs::default() };
    assert_eq!(record_to_fetch(&inputs), Some(movie(1001)));
    assert_eq!(record_to_fetch(&inputs), identify(&inputs, Kind::Tv, &RuntimeFacts::default(), &Settings::default()).tmdb);
    // Two results of TheDiscDB's kind and year stay ambiguous: no record is fetched.
    let several = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("movie", 1004, "Example Film", "Example Film", "1986-11-01"),
    ]);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&several), ..LookupInputs::default() };
    assert_eq!(record_to_fetch(&inputs), None);
}

#[test]
fn record_to_fetch_takes_thediscdbs_tmdb_id_before_any_search() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let name = label("JOHN_WICK");
    // The one plausible search result is another ID: TheDiscDB's ID wins.
    let results = search(&[("movie", 900001, "John Wick", "John Wick", "2099-01-01")]);
    let inputs = LookupInputs {
        name: Some(&name),
        discdb_by_fingerprint: Some(&fingerprint),
        search: Some(&results),
        ..LookupInputs::default()
    };
    assert_eq!(record_to_fetch(&inputs), Some(movie(245891)));
}

#[test]
fn record_to_fetch_names_the_reviewed_identity_then_the_remembered_one() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let both = LookupInputs {
        discdb_by_fingerprint: Some(&fingerprint),
        reviewed: Some(movie(7)),
        remembered: Some(movie(8)),
        ..LookupInputs::default()
    };
    assert_eq!(record_to_fetch(&both), Some(movie(7)));
    assert_eq!(record_to_fetch(&LookupInputs { reviewed: None, ..both }), Some(movie(8)));
}

#[test]
fn record_to_fetch_takes_the_one_plausible_result_of_the_name_search() {
    let name = label("JOHN_WICK_UHD");
    let results = search(&JOHN_WICK_RESULTS);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    assert_eq!(record_to_fetch(&inputs), Some(movie(245891)));
    assert_eq!(record_to_fetch(&inputs), identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default()).tmdb);
}

#[test]
fn record_to_fetch_fetches_nothing_for_several_plausible_results_a_generic_name_or_no_search() {
    let name = label("EXAMPLE_FILM");
    let both = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("tv", 1003, "Example Film", "Example Film", "2020-01-24"),
    ]);
    // identify shows the movie for a movie-shaped disc, but the step stays 5 (8.1 row 5): nothing to fetch.
    assert_eq!(record_to_fetch(&LookupInputs { name: Some(&name), search: Some(&both), ..LookupInputs::default() }), None);
    let generic = label("LOGICAL_VOLUME_ID");
    assert_eq!(record_to_fetch(&LookupInputs { name: Some(&generic), search: Some(&both), ..LookupInputs::default() }), None);
    assert_eq!(record_to_fetch(&LookupInputs { name: Some(&name), ..LookupInputs::default() }), None);
}
```

- [ ] **Step 2: Run the core tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked record_to_fetch`
Expected: compile error, `cannot find function 'record_to_fetch' in this scope`.

- [ ] **Step 3: Implement `record_to_fetch` in `crates/core/src/lookup/mod.rs`**

Place it after `identify` (`:453`), with the signature and `///` doc from Interfaces. The body mirrors `find_identity` (`:484`) step for step. It uses a scratch `let mut evidence = Vec::new();` that is thrown away, and calls neither `check_runtime` nor `describe_identity`:
1. `inputs.reviewed`, then `inputs.remembered`: return it.
2. `if let Some((step, record)) = discdb_match(inputs, &mut evidence)`:
   - if `record.tmdb_identity()` is Some, return it;
   - else, if `record.title_and_year()` is `Some((title, year))`, return `search_by_entry(inputs, step, record, &title, year, &mut evidence).tmdb`;
   - else fall through to step 3.
3. Return None when `inputs.name` is None, `name.generic` is true, or `inputs.search` is None.
4. Otherwise `match plausible_by_name(search, name, &mut evidence).as_slice() { [(identity, _)] => Some(*identity), _ => None }`.

- [ ] **Step 4: Run the core tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked lookup::`
Expected: `test result: ok. 149 passed; 0 failed`: the 144 tests under `crates/core/src/lookup/` at `70ada2b`, unchanged, and the five new.

- [ ] **Step 5: Write the failing chain tests in `crates/clients/src/chain.rs`**

First make the module compile. Create `chain.rs` holding only the `#[cfg(test)] mod tests` below, then add `pub mod chain;` and the `pub use chain::{..}` line to `lib.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use lazymkv_core::lookup::discdb::{parse_discid, DiscDbRecord};
    use lazymkv_core::lookup::name::disc_name;
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use serde_json::{json, Value};

    use crate::{
        dischash_url, discid_url, record_url, search_url, PendingReason, RecordedTransport, SavedFile, TransportError,
    };

    /// John Wick's UHD disc, whose TheDiscDB answers are recorded under corpus/thediscdb-api.
    const FP: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
    const DISC_ID: &str = "8D0355F530C31D44912831F8637FF65ACBCD6886";
    const JOHN_WICK_DISCHASH: &str = "thediscdb-api/dischash-76E63EBB5B66F73B0C37BFAB59B8ABA5.json";
    const JOHN_WICK_DISCID: &str = "thediscdb-api/discid-8D0355F530C31D44912831F8637FF65ACBCD6886.json";
    const JOHN_WICK: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 245891 };
    const TOKEN: &str = "test-token";
    const TODAY: &str = "2026-10-23";

    /// A file under corpus/, as bytes.
    fn corpus(relative: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"))
    }

    /// A /search/multi first page of movies (id, title, release date) in TMDb's documented shape.
    fn search_body(movies: &[(i64, &str, &str)]) -> Vec<u8> {
        let results: Vec<Value> = movies
            .iter()
            .map(|(id, title, date)| {
                json!({"media_type": "movie", "id": id, "title": title, "original_title": title, "release_date": date})
            })
            .collect();
        json!({"page": 1, "results": results, "total_pages": 1, "total_results": movies.len()}).to_string().into_bytes()
    }

    /// A /movie/{id}?append_to_response=external_ids answer; the IMDb ID is made up.
    fn movie_body(id: i64, title: &str, date: &str) -> Vec<u8> {
        json!({"id": id, "title": title, "original_title": title, "release_date": date, "original_language": "en",
               "runtime": 101, "external_ids": {"imdb_id": format!("tt{id:07}")}})
        .to_string()
        .into_bytes()
    }

    fn movie(id: i64) -> TmdbIdentity {
        TmdbIdentity { kind: TmdbKind::Movie, id }
    }

    /// The chain's inputs for John Wick's fingerprint, an optional disc ID and an optional volume label.
    fn chain(disc_id: Option<&str>, label: Option<&str>) -> ChainInputs {
        ChainInputs {
            fingerprint: FP.to_string(),
            disc_id: disc_id.map(str::to_string),
            name: label.and_then(|label| disc_name(None, Some(label))),
        }
    }

    /// The URL of the disc name's search.
    fn search_of(inputs: &ChainInputs) -> String {
        search_url(&inputs.name.as_ref().unwrap().cleaned)
    }

    /// dischash and discid answer 404 with empty bodies (TheDiscDB's miss).
    fn misses(t: &RecordedTransport) {
        t.answer(&dischash_url(FP), 404, b"");
        t.answer(&discid_url(DISC_ID), 404, b"");
    }

    /// TheDiscDB misses, the name search with one plausible result (John Wick), and John Wick's record.
    fn tmdb_only(t: &RecordedTransport, inputs: &ChainInputs) {
        misses(t);
        t.answer(&search_of(inputs), 200, &search_body(&[(245891, "John Wick", "2014-10-22"), (324552, "John Wick: Chapter 2", "2017-02-08")]));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
    }

    /// One round with the token, returning the files saved, in order.
    fn round(t: &RecordedTransport, book: &mut LookupBook, inputs: &ChainInputs) -> Vec<SavedFile> {
        let mut saved = Vec::new();
        run_round(t, Some(TOKEN), book, inputs, TODAY, &mut |file| saved.push(file));
        saved
    }

    /// Every URL requested so far, in order.
    fn urls(t: &RecordedTransport) -> Vec<String> {
        t.requests().into_iter().map(|(url, _)| url).collect()
    }

    /// The sorted lines of the last lookup-status.txt in `saved`.
    fn last_status(saved: &[SavedFile]) -> Vec<String> {
        let file = saved.iter().rev().find(|file| file.name == "lookup-status.txt").expect("a status file");
        let mut lines: Vec<String> = String::from_utf8(file.bytes.clone()).unwrap().lines().map(str::to_string).collect();
        lines.sort();
        lines
    }

    fn answered(status: u16) -> LookupState {
        LookupState::Answered { status }
    }

    #[test]
    fn the_chain_asks_dischash_discid_search_then_record() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert_eq!(
            book.states,
            BTreeMap::from([
                (Lookup::Dischash, answered(404)),
                (Lookup::Discid, answered(404)),
                (Lookup::Search, answered(200)),
                (Lookup::Record, answered(200)),
            ])
        );
        assert_eq!(book.pending(), (false, false));
        assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(JOHN_WICK));
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200", "tmdb movie/245891 200"]);
    }

    #[test]
    fn each_answer_is_saved_when_it_arrives_between_two_status_files() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let saved = round(&t, &mut LookupBook::default(), &inputs);
        let names: Vec<&str> = saved.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "lookup-status.txt", "thediscdb-hash.json", "lookup-status.txt",
                "lookup-status.txt", "thediscdb-discid.json", "lookup-status.txt",
                "lookup-status.txt", "tmdb-search.json", "lookup-status.txt",
                "lookup-status.txt", "tmdb.json", "tmdb-record.json", "lookup-status.txt",
            ]
        );
        // TheDiscDB's 404 is kept as received: an empty body.
        assert!(saved[1].bytes.is_empty());
        let search: Value = serde_json::from_slice(&saved[7].bytes).unwrap();
        assert_eq!((&search["queries"], &search["fetched_at"]), (&json!(["JOHN WICK"]), &json!(TODAY)));
        let record: Value = serde_json::from_slice(&saved[11].bytes).unwrap();
        assert_eq!((&record["id"], &record["fetched_at"]), (&json!(245891), &json!(TODAY)));
    }

    #[test]
    fn a_dvd_skips_discid() {
        let t = RecordedTransport::new();
        let inputs = chain(None, Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert!(!book.states.contains_key(&Lookup::Discid));
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn a_generic_name_is_never_searched() {
        for label in [Some("LOGICAL_VOLUME_ID"), None] {
            let t = RecordedTransport::new();
            misses(&t);
            let mut book = LookupBook::default();
            round(&t, &mut book, &chain(Some(DISC_ID), label));
            assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID)], "{label:?}");
            assert_eq!(book.states.keys().copied().collect::<Vec<Lookup>>(), [Lookup::Dischash, Lookup::Discid]);
            assert_eq!(book.pending(), (false, false));
        }
    }

    #[test]
    fn the_record_comes_from_discdbs_tmdb_id_when_present() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        t.answer(&discid_url(DISC_ID), 404, b"");
        // The speculative name search is still asked, and its one plausible result is another ID.
        t.answer(&search_of(&inputs), 200, &search_body(&[(900002, "John Wick", "2099-01-01")]));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert_eq!(book.answers.by_fingerprint.as_ref().and_then(|record| record.tmdb_identity()), Some(JOHN_WICK));
    }

    #[test]
    fn the_record_comes_from_the_one_plausible_search_result_otherwise() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        round(&t, &mut LookupBook::default(), &inputs);
        assert_eq!(urls(&t).last(), Some(&record_url(JOHN_WICK)));
        // Two plausible results: no record is asked, and nothing is left pending.
        let two = RecordedTransport::new();
        misses(&two);
        two.answer(&search_of(&inputs), 200, &search_body(&[(245891, "John Wick", "2014-10-22"), (900003, "John Wick", "1990-01-01")]));
        let mut book = LookupBook::default();
        round(&two, &mut book, &inputs);
        assert_eq!(urls(&two), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK")]);
        assert!(!book.states.contains_key(&Lookup::Record));
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn a_round_restarts_at_the_first_unanswered_step() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.fail(&dischash_url(FP), TransportError::Timeout);
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(book.states[&Lookup::Dischash], LookupState::Pending { reason: PendingReason::Timeout });
        assert_eq!(book.pending(), (true, false));
        assert_eq!(urls(&t).len(), 4);
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t)[4..], [dischash_url(FP)]);
        assert_eq!(book.states[&Lookup::Dischash], answered(404));
        assert_eq!(book.pending(), (false, false));
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200", "tmdb movie/245891 200"]);
    }

    #[test]
    fn a_late_thediscdb_answer_leads_to_its_record_within_the_round() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.fail(&dischash_url(FP), TransportError::Timeout);
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        t.answer(&discid_url(DISC_ID), 404, b"");
        t.answer(&search_of(&inputs), 200, &search_body(&[(900004, "John Wick", "2099-01-01")]));
        t.answer(&record_url(movie(900004)), 200, &movie_body(900004, "John Wick", "2099-01-01"));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(movie(900004)));
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t)[4..], [dischash_url(FP), record_url(JOHN_WICK)]);
        assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(JOHN_WICK));
        assert_eq!(book.pending(), (false, false));
        assert_eq!(last_status(&saved), ["dischash 200", "discid 404", "search 200", "tmdb movie/245891 200"]);
    }

    #[test]
    fn the_status_line_is_000_while_a_request_is_in_flight() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let gate = t.gate(&dischash_url(FP));
        let saved: Mutex<Vec<SavedFile>> = Mutex::new(Vec::new());
        let mut book = LookupBook::default();
        std::thread::scope(|scope| {
            let running = scope.spawn(|| {
                run_round(&t, Some(TOKEN), &mut book, &inputs, TODAY, &mut |file| saved.lock().unwrap().push(file))
            });
            let deadline = Instant::now() + Duration::from_secs(10);
            while saved.lock().unwrap().is_empty() {
                assert!(Instant::now() < deadline, "no status file was saved while dischash was held");
                std::thread::sleep(Duration::from_millis(10));
            }
            {
                let held = saved.lock().unwrap();
                assert_eq!(held.len(), 1);
                assert_eq!(held[0].name, "lookup-status.txt");
                let text = String::from_utf8(held[0].bytes.clone()).unwrap();
                assert_eq!(text.lines().collect::<Vec<&str>>(), ["dischash 000"]);
            }
            gate.open();
            running.join().unwrap();
        });
        assert_eq!(book.states[&Lookup::Dischash], answered(404));
    }

    #[test]
    fn without_a_token_the_tmdb_lookups_are_pending_and_never_requested() {
        let t = RecordedTransport::new();
        misses(&t);
        let mut book = LookupBook::default();
        let mut saved = Vec::new();
        run_round(&t, None, &mut book, &chain(Some(DISC_ID), Some("JOHN_WICK")), TODAY, &mut |file| saved.push(file));
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID)]);
        assert_eq!(book.states[&Lookup::Search], LookupState::Pending { reason: PendingReason::NoToken });
        assert!(!book.states.contains_key(&Lookup::Record));
        assert_eq!(book.pending(), (false, true));
        assert!(last_status(&saved).contains(&"search 000".to_string()));
    }

    #[test]
    fn pending_flags_follow_the_book() {
        let book = |states: &[(Lookup, LookupState)]| LookupBook {
            states: states.iter().cloned().collect(),
            answers: Answers::default(),
        };
        let timeout = LookupState::Pending { reason: PendingReason::Timeout };
        assert_eq!(LookupBook::default().pending(), (true, true));
        assert_eq!(book(&[(Lookup::Dischash, answered(404)), (Lookup::Discid, timeout.clone())]).pending(), (true, false));
        assert_eq!(book(&[(Lookup::Dischash, timeout.clone())]).pending(), (true, false));
        assert_eq!(book(&[(Lookup::Dischash, answered(200)), (Lookup::Search, LookupState::InFlight)]).pending(), (false, true));
        assert_eq!(
            book(&[(Lookup::Dischash, answered(404)), (Lookup::Search, answered(200)), (Lookup::Record, LookupState::NotAsked)]).pending(),
            (false, true)
        );
        assert_eq!(book(&[(Lookup::Dischash, answered(404))]).pending(), (false, false));
        let all = book(&[
            (Lookup::Dischash, answered(404)),
            (Lookup::Discid, answered(404)),
            (Lookup::Search, answered(200)),
            (Lookup::Record, answered(200)),
        ]);
        assert_eq!(all.pending(), (false, false));
        let waiting = book(&[(Lookup::Dischash, timeout)]);
        let inputs = waiting.lookup_inputs(None);
        assert_eq!((inputs.discdb_pending, inputs.tmdb_pending), waiting.pending());
    }

    #[test]
    fn lookup_inputs_take_the_first_disc_id_record_with_titles() {
        let empty: DiscDbRecord = serde_json::from_str(r#"{"titles": []}"#).unwrap();
        let text = String::from_utf8(corpus(JOHN_WICK_DISCID)).unwrap();
        let known = parse_discid(&text, JOHN_WICK_DISCID).unwrap().remove(0);
        let book = LookupBook {
            states: BTreeMap::from([(Lookup::Dischash, answered(404)), (Lookup::Discid, answered(200))]),
            answers: Answers { by_disc_id: vec![empty, known.clone()], ..Answers::default() },
        };
        assert_eq!(book.lookup_inputs(None).discdb_by_disc_id, Some(&known));
        assert_eq!(book.lookup_inputs(None).discdb_by_fingerprint, None);
    }
}
```

- [ ] **Step 6: Run the chain tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked chain`
Expected: compile errors. `cannot find type 'LookupBook' in this scope`, and likewise for `Lookup`, `LookupState`, `Answers`, `ChainInputs` and `run_round`.

- [ ] **Step 7: Implement `crates/clients/src/chain.rs`**

The types and signatures are as in Interfaces, each with its `///` doc. Imports: `lazymkv_core::lookup::{record_to_fetch, LookupInputs}`, `lazymkv_core::lookup::discdb::DiscDbRecord`, `lazymkv_core::lookup::name::DiscName`, `lazymkv_core::lookup::tmdb::{TmdbRecord, TmdbSearch}`, and the Task 15 items listed under Consumes.

`LookupBook::pending`, where "answered" means `matches!(state, LookupState::Answered { .. })`:
- `discdb` is true when `Dischash` is absent or not answered, or `Discid` is present and not answered.
- `tmdb` is true when `Dischash` is absent, or `Search` or `Record` is present and not answered.

`LookupBook::lookup_inputs` builds:
- `name`
- `discdb_by_fingerprint: answers.by_fingerprint.as_ref()`
- `discdb_by_disc_id: answers.by_disc_id.iter().find(|record| !record.titles.is_empty())`, the rule of `crates/dev/src/cmd/plan.rs:217`
- `discdb_pending` and `tmdb_pending` from `pending()`
- `search: answers.search.as_ref()` and `record: answers.record.as_ref()`
- `..LookupInputs::default()`

`run_round`. The tests do not fix every step, so here is the body in order:
1. **Seed.** Insert `NotAsked` for each lookup that is absent:
   - `Dischash`, always;
   - `Discid`, when `inputs.disc_id` is Some;
   - `Search`, when `inputs.name` is Some and not `generic`.
2. **Ask.** For `Dischash`, `Discid`, `Search` in that order, ask each one that is present and not `Answered`:
   1. Set it `InFlight` and save the status file.
   2. Call `discdb_by_fingerprint(t, &inputs.fingerprint)`, `discdb_by_disc_id(t, disc_id)` or `tmdb_search(t, token, &name.cleaned)`.
   3. Apply the outcome:
      - `Answered { value, fetched }` stores `value` in `answers` (`by_fingerprint = Some`, `by_disc_id`, `search = Some`) and sets `Answered { status: fetched.status }`.
      - `Miss { fetched }` clears that answer (None, or an empty Vec) and sets `Answered { status: fetched.status }`.
      - `Pending { reason, .. }` sets `Pending { reason }` and leaves `answers` alone.
   4. Save the answer file:
      - `thediscdb-hash.json` or `thediscdb-discid.json` with `fetched.body`, for every `Fetched`;
      - `tmdb-search.json` with `slim_search(&raw, &name.cleaned, today)`, only on `Answered`. Here `raw` is `fetched.body` parsed as a `serde_json::Value`.
   5. Save the status file.
3. **Record.** Compute `want = record_to_fetch(&book.lookup_inputs(inputs.name.as_ref()))`.
   - With None, remove `Record` if it is not `Answered`.
   - With `Some(id)`, skip when `Record` is `Answered` and `answers.record`'s identity is `Some(id)`. Otherwise ask as in step 2 with `tmdb_record(t, token, id)`:
     - `Answered` sets `answers.record = Some(value)` and saves `tmdb.json` (`slim_tmdb(&raw, today)`), then `tmdb-record.json` (`slim_record(&raw, id.kind, today)`);
     - `Miss` sets `answers.record = None`.
4. **The status file** is `SavedFile { name: "lookup-status.txt", bytes: status_text(&lines) }`. `lines` holds one entry per present state that is not `NotAsked`:
   - The keys are `dischash`, `discid` and `search`. The record's key is `tmdb <kind>/<id>`, where the identity is `record_to_fetch` over the book as it stands, else `answers.record`'s identity. With neither, there is no record line.
   - The values follow the Ruling above.

JSON files are `serde_json::to_vec_pretty(&value)` plus `\n`.

- [ ] **Step 8: Run both crates' tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-clients -p lazymkv-core --locked`
Expected: PASS. The 12 chain tests, Task 15's clients tests and every core test pass.

- [ ] **Step 9: Commit**

```bash
git add crates/clients/src/chain.rs crates/clients/src/lib.rs crates/core/src/lookup/mod.rs crates/core/src/lookup/tests.rs
git commit -m "clients::chain: the movie lookup chain and its rounds (design parts 8.2, 8.3)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, followed by the message and no attribution line.

---

### Task 17: Characterise decide's output before the split

**Files:**
- Modify: `crates/dev/src/cmd/decide.rs`, in `mod tests`. Add 4 setup helpers, `characterisation_dir`, `masked` and 1 test after `the_command_is_registered_with_its_synopsis` (`:2398`), before the module's closing brace at `:2405`.
- Create, via generation: 8 files under `crates/dev/src/cmd/decide/characterisation/`:
  - `disc-a-eng-only.json`
  - `disc-a-eng-only-all.json`
  - `disc-a-eng-only-text.json`
  - `disc-b-all-tracks.json`
  - `disc-b-eng-only.json`
  - `disc-b-extra.json`
  - `disc-b-forced-kept.json`
  - `disc-b-all-tracks-settings.json`

**Interfaces:**
- Consumes these items of `decide.rs`'s `mod tests`:
  - `run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32` (`decide.rs:390`)
  - `run_in(root: &Path, args: &[&str]) -> (i32, String, String)` (`:949`)
  - `recording(root: &Path) -> PathBuf` (`:870`), `level_folders(label: &Path)` (`:891`) and `subtitles(label: &Path, text: Option<&Path>, missing: &[i64])` (`:905`)
  - `disc_b(root: &Path)` (`:1642`) and `forced_kept(root: &Path)` (`:1681`)
  - `discdb_answer() -> String` (`:2195`) and `tmdb_answer(id: i64) -> String` (`:2228`)
  - `write` and `Scratch` (`crate::testutil`)
- Produces: test-only items and the 8 files. The task adds no `pub` item.

Rulings for this task:
- **Ruling:** `scripts/box.sh` copies back only `Cargo.lock` (`scripts/box.sh:18-20`), not other files as the skeleton says. Step 3 therefore fetches the generated folder with `scp` before the next `box.sh` call, which wipes `C:\lazymkv\src` (`scripts/box.sh:13`).
- **Ruling:** the stored file's `\r\n` is read as `\n` before comparing. The repository has no `.gitattributes`, and `actions/checkout` on `windows-latest` checks text out with `core.autocrlf` on. decide's stdout never holds a `\r`, because JSON escapes it.
- **Ruling:** the scratch root is written as `<root>`, in both its plain and its JSON-escaped spelling, before writing or comparing, and each JSON-escaped Windows separator after it as `/`. The files are generated on the mini PC (Windows) only, so this keeps a printed path below the root from differing on ubuntu and macos. `Scratch::new` names a fresh folder per process and per call (`crates/core/src/testsupport/mod.rs:376-383`).
- **Ruling:** the expected exit code is the one `run` derives from the stored document (`decide.rs:470-474`): 0 when `reconcile.exit_code` is 0, else 1. The files thus pin it without a second file.

- [ ] **Step 1: Write the characterisation test in `decide.rs`'s `mod tests`**

```rust
    /// recs/disc-a with its eng-only rip label (`recording`).
    fn disc_a(root: &Path) {
        recording(root);
    }

    /// recs/disc-a with its level folders and both subtitle streams' OCR text under text/disc-a/eng-only/subs-ocr, as
    /// `a_rip_label_gives_reconcile_audio_subtitles_and_reviews_in_one_document` lays it out.
    fn disc_a_with_text(root: &Path) {
        let label = recording(root);
        level_folders(&label);
        subtitles(&label, Some(&root.join("text/disc-a/eng-only/subs-ocr")), &[]);
    }

    /// recs/disc-b with its forced-kept rip label.
    fn disc_b_forced_kept(root: &Path) {
        disc_b(root);
        forced_kept(root);
    }

    /// recs/disc-b with TheDiscDB's and TMDb's answers and settings.json naming /srv/media/Movies, as
    /// `a_settings_file_gives_decides_own_decision_its_final_path` lays it out.
    fn disc_b_with_settings(root: &Path) {
        disc_b(root);
        write(&root.join("recs/disc-b/thediscdb-hash.json"), discdb_answer());
        write(&root.join("recs/disc-b/tmdb.json"), tmdb_answer(4242));
        write(&root.join("settings.json"), json!({"movies_root": "/srv/media/Movies"}).to_string());
    }

    /// The folder of decide's characterised documents (Task 17, K5).
    fn characterisation_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cmd/decide/characterisation")
    }

    /// `out` with the scratch root written as `<root>`, in its JSON-escaped and its plain spelling, and then each
    /// JSON-escaped Windows separator (`\\` in the printed text) written as `/`, so the files compare equal on every OS.
    fn masked(out: &str, root: &Path) -> String {
        let plain = root.display().to_string();
        let escaped = plain.replace('\\', "\\\\");
        out.replace(&escaped, "<root>").replace(&plain, "<root>").replace("\\\\", "/")
    }

    #[test]
    fn decide_prints_the_characterised_document_for_each_made_up_fixture() {
        let cases: [(&str, fn(&Path), &[&str]); 8] = [
            ("disc-a-eng-only.json", disc_a, &["recs/disc-a", "eng-only"]),
            ("disc-a-eng-only-all.json", disc_a, &["recs/disc-a", "eng-only", "--selection", "all"]),
            ("disc-a-eng-only-text.json", disc_a_with_text, &["recs/disc-a", "eng-only", "--text", "text"]),
            ("disc-b-all-tracks.json", disc_b, &["recs/disc-b", "all-tracks"]),
            ("disc-b-eng-only.json", disc_b, &["recs/disc-b", "eng-only"]),
            ("disc-b-extra.json", disc_b, &["recs/disc-b", "extra"]),
            ("disc-b-forced-kept.json", disc_b_forced_kept, &["recs/disc-b", "forced-kept"]),
            (
                "disc-b-all-tracks-settings.json",
                disc_b_with_settings,
                &["recs/disc-b", "all-tracks", "--settings", "settings.json"],
            ),
        ];
        let folder = characterisation_dir();
        let writing = std::env::var_os("LAZYMKV_WRITE_CHARACTERISATION").is_some_and(|value| value == "1");
        let mut differing = Vec::new();
        for (name, setup, args) in cases {
            let tmp = Scratch::new("characterised");
            let root = tmp.0.clone();
            setup(&root);
            let (code, out, err) = run_in(&root, args);
            assert_eq!(err, "", "{name}");
            let printed = masked(&out, &root);
            let path = folder.join(name);
            if writing {
                fs::create_dir_all(&folder).unwrap();
                fs::write(&path, &printed).unwrap();
                continue;
            }
            let stored = fs::read_to_string(&path)
                .unwrap_or_else(|error| {
                    panic!("{}: {error}: run once with LAZYMKV_WRITE_CHARACTERISATION=1 on the pre-split code (Task 17)", path.display())
                })
                .replace("\r\n", "\n");
            let value: Value = serde_json::from_str(&stored).unwrap();
            let want = if value["reconcile"]["exit_code"] == json!(0) { 0 } else { 1 };
            if code != want || printed != stored {
                differing.push(name);
            }
        }
        assert!(differing.is_empty(), "characterisation files that differ from decide's output: {differing:?}");
    }
```

- [ ] **Step 2: Run the test to verify it fails with no stored file**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked decide_prints_the_characterised`
Expected: FAIL. The panic names `...characterisation\disc-a-eng-only.json`, then the OS's "cannot find the file" error, then `run once with LAZYMKV_WRITE_CHARACTERISATION=1 on the pre-split code (Task 17)`.

- [ ] **Step 3: Generate the files on the pre-split code and fetch them**

Run: `scripts/box.sh "set LAZYMKV_WRITE_CHARACTERISATION=1&& cargo test -p lazymkv-dev --locked decide_prints_the_characterised"`
Expected: PASS, 1 test.

Then, before any other `box.sh` call, run:
`(. scripts/phase0/box.env; scp -q -r -o BatchMode=yes "${LAZYMKV_BOX:-USER@makemkv-box}:C:/lazymkv/src/crates/dev/src/cmd/decide/characterisation" crates/dev/src/cmd/decide/)`

Check:
- `ls crates/dev/src/cmd/decide/characterisation` lists the 8 file names above.
- `head -c 40 crates/dev/src/cmd/decide/characterisation/disc-a-eng-only.json` starts `{` then a newline then ` "slug": "disc-a",`.
- `grep -c . crates/dev/src/cmd/decide/characterisation/*.json` shows no empty file.

The files hold the made-up fixtures' output only, so RP21 allows them. `crates/dev/tests/clone_values.rs` scans only `.rs` files under `crates/`.

- [ ] **Step 4: Run the test to verify it passes against the stored files**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked decide_prints_the_characterised`
Expected: PASS, 1 test.

- [ ] **Step 5: Run the full workspace with the evidence junction**

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS. Every test passes, including the 6 goldens in `crates/dev/tests/decide.rs`.

CI on ubuntu and macos must also pass this test after the push. `masked` already writes the Windows separator as `/`, so a remaining difference is a platform dependence in decide's output other than a separator: stop and report it to the owner with the differing line, and do not regenerate.

- [ ] **Step 6: Commit**

```bash
git add crates/dev/src/cmd/decide.rs crates/dev/src/cmd/decide/characterisation
git commit -m "dev::decide: pin the printed document of every made-up fixture before the K5 split"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, followed by the message and no attribution line.

---

### Task 18: Pure subtitle events in core (K5, S25)

**Files:**
- Modify: `crates/core/src/verify.rs`:
  - the import at `:21` gains `Packet`;
  - `read_packets_file` and `subtitle_events_of` are added after `subs_index` (`:565-569`);
  - `subtitle_events` (`:571-600`) is rewritten over them;
  - `mod tests` gains 2 helpers and 4 tests after `subtitle_events_prefix_an_events_error_with_the_path`.

**Interfaces:**

Consumes:
- `subs_index(tree: &dyn Tree, subs_dir: &PathKey) -> Result<Vec<SubsIndexEntry>, String>` (`verify.rs:565`)
- `subs_streams(index: &[SubsIndexEntry], report: &VerifyReport) -> BTreeMap<i64, u32>` (`:552`)
- `read_text(tree, &path, Encoding::Utf8Sig, Errors::Strict)`
- `align::load_packets(text: &str, shown: &str) -> Result<Vec<Packet>, String>` (`crates/core/src/subtitle/align.rs:206`)
- `align::events(packets: &[Packet], clear_max_bytes: i64) -> Result<Vec<f64>, String>` (`align.rs:93`)
- `align::is_text(codec: Option<&str>) -> bool` (`align.rs:142`)
- `recording::files::{Packet, SubsIndexEntry}` (`crates/core/src/recording/files.rs:166`, `:154`)

Produces:
```rust
/// One sub-<n>.packets.json as read for the subtitle-event rule: `shown` is the tree's display of its path, and
/// `packets` the packets or the error text `subtitle_events` gives for it (a missing file: the read error, which names
/// the path; any other read, decode, syntax or shape error: prefixed with `shown` exactly once). Not
/// `recording::files::PacketsFile` (files.rs:175), the file's serde shape. Spec 6.4 step 10, 8.7, K5. Def: T18. Use: T19.
#[derive(Clone, Debug)]
pub struct PacketsFile { pub shown: String, pub packets: Result<Vec<Packet>, String> }

/// `subs_dir/sub-<n>.packets.json` read strictly (UTF-8, a byte-order mark allowed) and parsed by `align::load_packets`,
/// its error held in `packets` as `PacketsFile` says. Spec 6.4 step 10, K5. Def: T18. Use: T18, T19.
pub fn read_packets_file(tree: &dyn Tree, subs_dir: &PathKey, n: i64) -> PacketsFile;

/// The one subtitle-event rule over read inputs: for each `index` entry in order that `subs_streams` maps, the
/// `align::events` of `packets[n]` at `clear_max_bytes` (0 where `align::is_text`), keyed by scan stream. Unmapped
/// entries are skipped, so their files may be broken or absent. A mapped entry's held error is returned as is, its
/// events error as "<shown>: <error>", and a mapped entry with no `packets` entry as "sub-<n>.packets.json: not read".
/// Spec 6.4 step 10, 8.7, K5, S25. Def: T18. Use: T18, T19.
pub fn subtitle_events_of(index: &[SubsIndexEntry], packets: &BTreeMap<i64, PacketsFile>, report: &VerifyReport,
    clear_max_bytes: i64) -> Result<BTreeMap<u32, Vec<f64>>, String>;
```

`subtitle_events(tree: &dyn Tree, subs_dir: &PathKey, report: &VerifyReport, clear_max_bytes: i64) -> Result<BTreeMap<u32, Vec<f64>>, String>` keeps its signature and every error text. Its doc gains "Def: T18. Use: decide (T19)", in place of nothing.

**Ruling:** a mapped entry with no `packets` entry gives `"sub-<n>.packets.json: not read"`. `subtitle_events` reads every mapped entry, so the case arises only from a caller that built the map itself, and the text names the file the caller left out.

- [ ] **Step 1: Write the failing tests in `verify.rs`'s `mod tests`**

These use the file's `INDEX`, `TEXT_PACKETS`, `IMAGE_PACKETS`, `subs_tree` and `dvd_report`.

```rust
    /// The packets files of `ns` under subs/ in `tree`, read as `subtitle_events` reads them.
    fn read_all(tree: &MemTree, ns: &[i64]) -> BTreeMap<i64, PacketsFile> {
        ns.iter().map(|&n| (n, read_packets_file(tree, &PathKey::new("subs"), n))).collect()
    }

    /// subs/index.json's entries.
    fn index_of(text: &str) -> Vec<SubsIndexEntry> {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn events_of_read_packets_equal_the_tree_form() {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        let packets = read_all(&tree, &[0, 1]);
        assert_eq!(packets[&0].shown, "rec/subs/sub-0.packets.json");
        for limit in [64, 0] {
            assert_eq!(
                subtitle_events_of(&index_of(INDEX), &packets, &dvd_report(), limit),
                subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), limit)
            );
        }
        assert_eq!(
            subtitle_events_of(&index_of(INDEX), &packets, &dvd_report(), 64),
            Ok(BTreeMap::from([(3, vec![21.0, 23.0]), (5, vec![10.0, 12.5, 15.25])]))
        );
    }

    #[test]
    fn a_broken_unmapped_packets_file_is_silent() {
        // n 1 names track 9, which no pair holds.
        let unmapped = r#"[{"n": 0, "track_id": 2, "codec": "SubRip/SRT"}, {"n": 1, "track_id": 9, "codec": "VobSub"}]"#;
        let tree = subs_tree(&[("subs/index.json", unmapped), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", "[]")]);
        let packets = read_all(&tree, &[0, 1]);
        assert_eq!(
            packets[&1].packets.as_ref().err().map(String::as_str),
            Some("rec/subs/sub-1.packets.json: 'list' object has no attribute 'get'")
        );
        assert_eq!(
            subtitle_events_of(&index_of(unmapped), &packets, &dvd_report(), 64),
            Ok(BTreeMap::from([(5, vec![10.0, 12.5, 15.25])]))
        );
    }

    #[test]
    fn a_broken_mapped_packets_file_names_its_path() {
        let missing = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        assert_eq!(
            subtitle_events_of(&index_of(INDEX), &read_all(&missing, &[0, 1]), &dvd_report(), 64),
            Err("[Errno 2] No such file or directory: 'rec/subs/sub-0.packets.json'".to_string())
        );
        for (text, error) in [
            ("[]", "rec/subs/sub-0.packets.json: 'list' object has no attribute 'get'"),
            (
                r#"{"packets": [{"pts_time": "1.0", "size": "big"}]}"#,
                "rec/subs/sub-0.packets.json: invalid literal for int() with base 10: 'big'",
            ),
        ] {
            let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", text), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
            assert_eq!(
                subtitle_events_of(&index_of(INDEX), &read_all(&tree, &[0, 1]), &dvd_report(), 64),
                Err(error.to_string()),
                "{text}"
            );
        }
    }

    #[test]
    fn a_mapped_entry_whose_file_was_not_read_is_refused_naming_it() {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        assert_eq!(
            subtitle_events_of(&index_of(INDEX), &read_all(&tree, &[0]), &dvd_report(), 64),
            Err("sub-1.packets.json: not read".to_string())
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked verify`
Expected: compile errors, `cannot find type 'PacketsFile' in this scope` and `cannot find function 'read_packets_file'` / `'subtitle_events_of'`.

- [ ] **Step 3: Implement in `verify.rs`**

Add `Packet` to `use crate::recording::files::{..}` (`:21`). Add `PacketsFile`, `read_packets_file` and `subtitle_events_of` with the signatures and docs above.

- **`read_packets_file`** moves the current per-entry read, `verify.rs:587-594`, unchanged:
  - The path is `subs_dir.join(&format!("sub-{n}.packets.json"))` and `shown = tree.display(&path)`.
  - The read error is kept bare when `!tree.is_file(&path)`, else becomes `"{shown}: {error}"`.
  - The `load_packets` error is prefixed with `shown` unless it already `starts_with(&shown)`.
- **`subtitle_events_of`** runs `subs_streams(index, report)`. Then, for each entry in `index` order that is mapped:
  - take `packets.get(&entry.n)`, or `Err(format!("sub-{}.packets.json: not read", entry.n))`;
  - take `file.packets.as_ref().map_err(Clone::clone)?`;
  - set `limit = if is_text(entry.codec.as_deref()) { 0 } else { clear_max_bytes }`;
  - compute `events(packets, limit).map_err(|error| format!("{}: {error}", file.shown))?` and insert it under the stream.
- **`subtitle_events`** becomes:
  1. `let index = subs_index(tree, subs_dir)?;`
  2. `let streams = subs_streams(&index, report);`
  3. `read_packets_file` for every entry with `streams.contains_key(&entry.n)`, collected into a `BTreeMap<i64, PacketsFile>`;
  4. `subtitle_events_of(&index, &packets, report, clear_max_bytes)`.

- [ ] **Step 4: Run the core tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked verify`
Expected: PASS. The 4 new tests and every existing `subtitle_events_*` test (`verify.rs:1878-1990`) pass with their exact error texts.

- [ ] **Step 5: Run the full workspace with the evidence junction**

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS. Task 17's `decide_prints_the_characterised_document_for_each_made_up_fixture` and the 6 goldens in `crates/dev/tests/decide.rs` pass unchanged.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/verify.rs
git commit -m "core::verify: subtitle events from read packets, the one copy the renderer and the tree share (K5)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, followed by the message and no attribution line.

### Task 19: The K5 split into readers and a pure renderer

**Files:**
- Modify: `crates/dev/src/cmd/decide.rs`:
  - module doc `:14-22`;
  - imports `:47-53`;
  - `decide_doc` at `:231`, with its doc comment above (it becomes `read_decide_inputs` plus the new `decide_doc`);
  - `subtitle_section` `:341-366` (its `None` branch calls `no_text_entry`);
  - `rec_folder` `:365-368` (becomes `pub`);
  - `run` `:455-462`;
  - `decision_doc` at `:505`, with its doc comment (it becomes the private `decision_section`);
  - `text_decisions` at `:602`;
  - `level_md5s` at `:699`, `ffprobe_profiles` at `:738` and `subtitle_starts` at `:762`, each with its doc comment (each splits into a reading half and a rendering half).
- Modify (tests in the same file):
  - `doc_of` `:940-946`, `doc_b` `:1704-1710`, `:1737`, `:1874`;
  - `decided_with` `:1972-1988`;
  - `:2349` and `:2365`;
  - 5 new tests.
- Modify: `crates/dev/src/cmd/plan.rs:266-283`. `RecordedAnswers` and `recorded_answers` become `pub`.
- Modify: `crates/dev/tests/decide.rs`:
  - the import `:35-38`;
  - `check_label` `:179-185` and `:298`;
  - `check_decision` `:686-690` and `:779-795`.

**Interfaces:**

Consumes (from the tree and earlier tasks):

```rust
// crates/core/src/verify.rs (Task 18; Clone and Debug are needed by FileFacts' derive)
pub struct PacketsFile { pub shown: String, pub packets: Result<Vec<recording::files::Packet>, String> }
pub fn read_packets_file(tree: &dyn Tree, subs_dir: &PathKey, n: i64) -> PacketsFile;
pub fn subtitle_events_of(index: &[SubsIndexEntry], packets: &BTreeMap<i64, PacketsFile>, report: &VerifyReport,
    clear_max_bytes: i64) -> Result<BTreeMap<u32, Vec<f64>>, String>;
pub fn subs_index(tree: &dyn Tree, subs_dir: &PathKey) -> Result<Vec<SubsIndexEntry>, String>;   // verify.rs:565
pub fn stream_of_track(report: &VerifyReport, track_id: i64) -> Option<u32>;                     // verify.rs:532
pub fn verify_rip(title: &Title, ident: &MkvmergeIdent, selection: &[u32], rip_log: Option<&str>, dvd: bool) -> VerifyReport; // :371
// crates/core/src/recording/files.rs
pub fn rip_label(tree: &dyn Tree, slug_dir: &PathKey, label: &str) -> Result<RipLabel, String>;  // :348; RipLabel { title: i64, selection, .. } :322
// crates/core/src/reconcile.rs:314
pub fn reconcile(scan, title: u32, ident, selection, rip_log: Option<RipLog>) -> Result<Report, String>;
// crates/core/src/audio/rule.rs:21
pub const RULE_VERSION: u32 = 1;
// crates/dev/src/cmd/plan.rs
pub fn plan_doc(ctx: &Ctx, source: &PlanSource<'_>, options: &PlanOptions) -> Result<PlanDoc, Refused>; // :175
pub struct PlanOptions { pub settings, pub tmdb_search, pub tmdb_record, pub remembered: Option<PathBuf> } // :118, Default
// crates/dev/src/evidence.rs
pub(crate) fn read_scan_in(tree: &dyn Tree, path: &PathKey, encoding: Encoding, errors: Errors) -> Result<Scan, String>; // :334
pub struct Refused { pub code: i32, pub line: String }                                           // :159, PartialEq
```

Produces (in `crates/dev/src/cmd/decide.rs` unless noted):

```rust
/// The rip's MakeMKV log as decide reads it (the first of `RIP_LOG_NAMES` beside mkvmerge.json, UTF-8 with
/// invalid bytes replaced, `text_mode` applied) and its path as printed. K5. Def: T19. Use: T19, T24.
#[derive(Clone, Debug)]
pub struct RipLogText { pub shown: String, pub text: String }

/// The file-phase reads of one rip label, each with its error held back, so the renderer surfaces only the
/// errors decision_doc surfaced and in its order (K5, design part 5.7 step 3). Def: T19. Use: T19, T24.
#[derive(Clone, Debug)]
pub struct FileFacts {
    /// (file track, trimmed digest) in level_md5s' read order: per audio-levels-20ms-* folder in PathKey order,
    /// each manifest.txt `main=` then `cand=` line's (`main`/`cand` value parsed as i64, `md5`), then each
    /// audio-proxy.json track's (`source_stream` as i64, `source.md5`). Err: the manifest read error, or
    /// "<audio-proxy.json shown>: <parse error>".
    pub level_md5: Result<Vec<(Option<i64>, Option<String>)>, String>,
    /// <label>/ffprobe-streams.json; None when absent; Err "<shown>: <serde error>".
    pub probes: Result<Option<FfprobeStreams>, String>,
    /// <label>/subs/index.json through `verify::subs_index`; None when it is not a file.
    pub subs_index: Result<Option<Vec<SubsIndexEntry>>, String>,
    /// `verify::read_packets_file` for every index entry's `n`, mapped or not; empty without an index.
    pub packets: BTreeMap<i64, PacketsFile>,
}

/// Everything `decide_doc` renders for one rip label (K5, R-B1, design part 5.7 step 3). `read_decide_inputs`
/// builds it from a recording folder; the engine builds it from what it measured (Task 24). Def: T19.
/// Use: T19, T20, T24, T25.
#[derive(Clone, Debug)]
pub struct DecideInputs {
    pub slug: String,            // rec.slug, or the job ID
    pub label: String,           // the rip label, or the title key
    pub title: i64,              // RipLabel.title as read
    pub selection: Selection,    // --selection, else RipLabel.selection
    pub scan: Scan,              // <slug>/scan.txt, Utf8, Replace
    pub ident: MkvmergeIdent,    // <label>/mkvmerge.json
    pub rip_log: Option<RipLogText>,
    pub reconcile: reconcile::Report,
    pub audio: Vec<AudioEntry>,
    pub subtitles: SubtitleEntry,
    pub plan: PlanEntry,
    pub answers: Result<RecordedAnswers, String>, // recorded_answers(rec), its Refused.line held back
    pub file: FileFacts,
    pub library: LibraryFacts,                    // the readers: LibraryFacts::default()
    pub overrides: Option<remembered::Overrides>, // the readers: None
}

/// decide's readers (spec 3.4, RP14, K5). They read the rip label, the scan, mkvmerge.json and the rip log, and run
/// reconcile; each failure is Refused { code: 2 }. Then come the audio and subtitle sections, `plan_doc` with
/// `options` (its refusal held as `PlanEntry::Skipped`), the recorded answers, and the file facts. Def: T19.
/// Use: T19, T20, T25, T27.
pub fn read_decide_inputs(ctx: &Ctx, rec: &Recording, label: &str, text: Option<&Path>, selection: Option<Selection>,
    options: &PlanOptions) -> Result<DecideInputs, Refused>;

/// decide's pure renderer (K5, R-B1). It reads no file and takes no Ctx or Tree. The reviews come from `inputs.audio`
/// and `inputs.subtitles`. The decision is the private `decision_section` when the plan is in `Results`, else
/// "no plan: <reason>". Def: T19. Use: T19, T24, T25, T29.
pub fn decide_doc(inputs: &DecideInputs) -> DecideDoc;

/// The subtitles section of a run with no OCR text: Skipped "no OCR text {TEXT_HINT}". The engine uses it in PR 2
/// (design part 5.7, S33). Def: T19. Use: T19, T24.
pub fn no_text_entry() -> SubtitleEntry;

/// `--rec DIR` resolved (doc unchanged from :365-367). Now pub for the engine (design part 7.1). Def: T19. Use: T24, T25.
pub fn rec_folder(ctx: &Ctx, dir: &str) -> Result<(Recording, String), Refused>;

/// `<title key>/decide.json` as the engine writes it (design part 7.1): decide's document, then the audio content
/// rule's version and lazymkv's version as its last two keys. Def: T19. Use: T24, T29.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct EngineDecideDoc { #[serde(flatten)] pub doc: DecideDoc, pub audio_rule_version: u32, pub lazymkv_version: String }

// crates/dev/src/cmd/plan.rs: `pub(crate)` becomes `pub`; the derives and fields are unchanged. Docs end
// "Def: T15. Use: T15, T19, T24."
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedAnswers { pub by_fingerprint: Option<DiscDbRecord>, pub by_disc_id: Vec<DiscDbRecord>, pub recorded: Option<TmdbRecord> }
pub fn recorded_answers(rec: &Recording) -> Result<RecordedAnswers, Refused>;
```

Private, replacing `decision_doc`:

```rust
fn decision_section(inputs: &DecideInputs, plan: &PlanDoc, doc: &DecideDoc) -> Result<DecisionDoc, String>;
```

Rulings:
- Ruling (S24): the reader runs `reconcile`, and `decide_doc` cannot fail, as K5 writes it.
- Ruling: `decided_with` keeps its closure type `FnOnce(&mut PlanDoc, &mut DecideDoc)` and calls `decision_section(&inputs, &plan, &doc)` directly. Its twenty closures and `with_text` then stay unchanged.
- Ruling: the reader reads the packets file of every index entry, not only the mapped ones. The verify report is the renderer's (S25), and errors are held per entry, so a broken unmapped file stays silent as today.
- Ruling: `EngineDecideDoc` is `Serialize` only. No reader reads it back as that type.

- [ ] **Step 1: Write the failing tests in `decide.rs`'s `mod tests`**

Add to the test imports: `use lazymkv_core::audio::rule::RULE_VERSION;`.

```rust
    #[test]
    fn run_prints_the_rendering_of_the_read_inputs() {
        let tmp = Scratch::new("read-then-render");
        let root = tmp.0.clone();
        let label = recording(&root);
        level_folders(&label);
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-a").unwrap();
        let inputs = read_decide_inputs(&ctx, &rec, "eng-only", None, None, &PlanOptions::default()).unwrap();
        let printed = dumps(&serde_json::to_value(decide_doc(&inputs)).unwrap(), Style::INDENT1_ASCII);
        let (code, out, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!((code, out, err.as_str()), (0, format!("{printed}\n"), ""));
    }

    #[test]
    fn the_renderer_reads_no_file() {
        let tmp = Scratch::new("render-no-file");
        let root = tmp.0.clone();
        disc_b(&root);
        forced_kept(&root);
        write(&root.join("recs/disc-b/thediscdb-hash.json"), discdb_answer());
        write(&root.join("recs/disc-b/tmdb.json"), tmdb_answer(4242));
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let inputs = read_decide_inputs(&ctx, &rec, "forced-kept", None, None, &PlanOptions::default()).unwrap();
        let before = decide_doc(&inputs);
        // The file facts reached the inputs: MD5s, profiles and subtitle events.
        let facts = &results_of(&before.decision).facts;
        assert!(
            !facts.md5_streams.is_empty() && !facts.profile_streams.is_empty() && !facts.event_streams.is_empty(),
            "{facts:?}"
        );
        fs::remove_dir_all(root.join("recs")).unwrap();
        assert_eq!(decide_doc(&inputs), before);
    }

    #[test]
    fn a_title_the_scan_lacks_is_refused_by_the_reader_with_code_2() {
        let tmp = Scratch::new("reader-refuses");
        let root = tmp.0.clone();
        let label = recording(&root);
        write(&label.join("rip-summary.json"), "{\"ok\": true, \"title\": 7}");
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-a").unwrap();
        let refused = read_decide_inputs(&ctx, &rec, "eng-only", None, None, &PlanOptions::default()).unwrap_err();
        assert_eq!(refused, Refused { code: 2, line: "the scan lists no title 7".to_string() });
    }

    #[test]
    fn the_engine_document_appends_the_rule_and_lazymkv_versions() {
        let tmp = Scratch::new("engine-doc");
        let root = tmp.0.clone();
        recording(&root);
        let doc = doc_of(&root, "eng-only", None, None).unwrap();
        let engine = EngineDecideDoc {
            doc: doc.clone(),
            audio_rule_version: RULE_VERSION,
            lazymkv_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        let value = serde_json::to_value(&engine).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            ["slug", "label", "reconcile", "audio", "subtitles", "reviews", "plan", "decision", "audio_rule_version",
             "lazymkv_version"]
        );
        assert_eq!((value["audio_rule_version"].clone(), value["lazymkv_version"].clone()), (json!(1), json!("0.0.0")));
        let mut without = value.clone();
        let map = without.as_object_mut().unwrap();
        map.remove("audio_rule_version");
        map.remove("lazymkv_version");
        assert_eq!(without, serde_json::to_value(&doc).unwrap());
    }

    #[test]
    fn the_engines_subtitles_entry_is_the_no_ocr_text_skip() {
        let expected = SubtitleEntry::Skipped { skipped: "no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)".to_string() };
        assert_eq!(no_text_entry(), expected);
        let tmp = Scratch::new("no-text-entry");
        let root = tmp.0.clone();
        recording(&root);
        assert_eq!(doc_of(&root, "eng-only", None, None).unwrap().subtitles, expected);
    }
```

Change the existing callers to the new pair:
- `doc_of` (`:945`): `read_decide_inputs(&ctx, &rec, label, text.map(Path::new), selection, &PlanOptions::default()).map(|inputs| decide_doc(&inputs))`.
- `doc_b` (`:1709`), `:1737` and `:1874`: `decide_doc(&read_decide_inputs(&ctx, &rec, <label>, None, None, &PlanOptions::default()).unwrap())`.
- `:2349` and `:2365`: the same, with `&PlanOptions { settings: Some(PathBuf::from("settings.json")), ..PlanOptions::default() }` and `"bad-settings.json"`.
- `decided_with` (`:1981-1987`):
  - `let inputs = read_decide_inputs(&ctx, &rec, label, None, None, &PlanOptions::default()).unwrap();`
  - `let mut doc = decide_doc(&inputs);`
  - the plan clone and `change(&mut plan, &mut doc)` are unchanged;
  - then `decision_section(&inputs, &plan, &doc)`.

In `crates/dev/tests/decide.rs`:
- The import (`:35-38`) drops `decision_doc` and adds `read_decide_inputs` and `DecideInputs`. Add `use lazymkv_dev::cmd::plan::PlanOptions;`.
- `check_label` (`:179`):
  - `let inputs = match read_decide_inputs(ctx, rec, label, text.as_deref(), None, &PlanOptions::default()) { Ok(inputs) => inputs, Err(refused) => { <the same failure push>; return; } };`
  - then `let doc = decide_doc(&inputs);`.
- `:298` passes `&inputs` in place of `ctx`.
- `check_decision` (`:686-690`): the first parameter `ctx: &Ctx` becomes `inputs: &DecideInputs`.
- Its tail (`:779-795`):
  - keeps the `let PlanEntry::Results(_) = &doc.plan else { return; };` guard;
  - `let mut rooted = inputs.clone();` and `if let PlanEntry::Results(plan) = &mut rooted.plan { plan.settings.movies_root = Some(MOVIES_ROOT.to_string()); plan.settings.tv_root = Some(SHOWS_ROOT.to_string()); }`;
  - `match decide_doc(&rooted).decision { DecisionEntry::Results(found) => <the same two checks>, DecisionEntry::Skipped { skipped } => failures.push(format!("{at}: with library roots: {skipped}")) }`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked cmd::decide`

Expected: compile errors:
- `error[E0425]: cannot find function `read_decide_inputs` in this scope`;
- `cannot find function `no_text_entry``;
- `cannot find struct, variant or union type `EngineDecideDoc``;
- in `tests/decide.rs`: `unresolved imports `lazymkv_dev::cmd::decide::read_decide_inputs`, `lazymkv_dev::cmd::decide::DecideInputs``.

- [ ] **Step 3: Implement the split**

1. In `plan.rs:266-283`, `RecordedAnswers` and `recorded_answers` become `pub`, with the docs above.
2. Write `read_decide_inputs` in `decide.rs`. Its body is today's `decide_doc` (`:239-261`) up to `plan`, in the same order and with the same refusals:
   1. `rip_label`;
   2. the selection;
   3. the title as `u32` (a negative one gives `NO_SUCH_TITLE`); `inputs.title` keeps the `i64`;
   4. `scan.txt`, `mkvmerge.json` and the rip log (its `(path, text)` becomes `RipLogText { shown: tree.display(&path), text }`);
   5. `reconcile`;
   6. `audio_section`, then `subtitle_section` (with `text` taken against `ctx.cwd`);
   7. `plan_doc(ctx, &PlanSource::Recording(rec), options)` into `PlanEntry`;
   8. `answers: recorded_answers(rec).map_err(|refused| refused.line)`;
   9. `file` from the three private readers below;
   10. `library: LibraryFacts::default()` and `overrides: None`.
3. Private readers, which are the reading halves of today's helpers:
   - `fn read_level_md5(tree: &dyn Tree, label_dir: &PathKey) -> Result<Vec<(Option<i64>, Option<String>)>, String>`. It is `level_md5s`' loop, recording `(fields.get(key).and_then(|text| text.trim().parse::<i64>().ok()), fields.get("md5").map(|text| text.trim().to_string()))` and `(track.source_stream.as_i64(), <source.md5>.map(|text| text.trim().to_string()))`.
   - `fn read_probes(tree: &dyn Tree, label_dir: &PathKey) -> Result<Option<FfprobeStreams>, String>`, with `ffprobe_profiles`' absence test and error text.
   - `fn read_subs(tree: &dyn Tree, label_dir: &PathKey) -> (Result<Option<Vec<SubsIndexEntry>>, String>, BTreeMap<i64, PacketsFile>)`. When `subs/index.json` is not a file it gives `Ok(None)`. Otherwise it gives `subs_index(tree, &subs)` and, on Ok, `read_packets_file(tree, &subs, entry.n)` for every entry.
4. Write the renderer `decide_doc(inputs)`. It builds the reviews exactly as `:250-259` did, from `inputs.audio` and `inputs.subtitles`. The `DecideDoc` takes clones of `slug`, `label`, `reconcile`, `audio`, `subtitles` and `plan`, with the `UNDECIDED` placeholder. The decision is then set as `:274-281` set it, calling `decision_section(inputs, found, &doc)`.
5. `decision_section` is `decision_doc` with every file read replaced by a field. The first Err gives `DecisionEntry::Skipped`, in this order:
   1. `planned_title(&plan.plan, doc.reconcile.title)`.
   2. `disc::disc(&inputs.scan)`, then `.title(number)`, then `selected_streams(&inputs.scan, number, doc.reconcile.selection)`. Then `verify_rip(.., inputs.rip_log.as_ref().map(|log| log.text.as_str()), typed.is_dvd)` and `rip_title`.
   3. `md5_by_stream(inputs.file.level_md5.as_ref().map_err(Clone::clone)?, &report)`, then `profiles_by_stream(inputs.file.probes.as_ref().map_err(Clone::clone)?.as_ref(), &report)`. These are the rendering halves of `level_md5s` and `ffprobe_profiles`: `stream_of_track` plus `first_md5`, and the audio-profile filter, unchanged.
   4. Events. A `subs_index` Err is returned. `Ok(None)` gives an empty map. `Ok(Some(index))` gives `subtitle_events_of(index, &inputs.file.packets, &report, settings.sub_clear_max_bytes)?`.
   5. `content_verdicts(&doc.audio, &report)`. Then `text_decisions(&doc.subtitles, &inputs.file.subs_index, &report, settings)?`, whose second parameter replaces `tree` and `label_dir`. In it, Err is returned, and `Ok(None)` gives an empty map (S48).
   6. `inputs.answers.as_ref().map_err(Clone::clone)?`, then `decide` with `library: &inputs.library` and `overrides: inputs.overrides.as_ref()`.
6. Delete `level_md5s`, `ffprobe_profiles`, `subtitle_starts` and the now unused imports (`subtitle_events`, and `subs_index` only if no use remains).
7. `subtitle_section`'s `None` branch returns `no_text_entry()`. `rec_folder` becomes `pub`.
8. `run` (`:455`):
   - `let options = PlanOptions { settings: settings.map(PathBuf::from), ..PlanOptions::default() };`
   - `read_decide_inputs(ctx, &rec, &label, text.as_deref(), selection, &options)`, with its Err printed and returned as today;
   - `let doc = decide_doc(&inputs);`. The rest is unchanged.
9. Rewrite the module doc `:14-22` as one paragraph. It says that the readers build `DecideInputs` and the pure `decide_doc` renders it (K5), and that the engine renders its own inputs through the same function (R-B1).

- [ ] **Step 4: Run the dev tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked cmd::decide`

Expected: PASS, including the 5 new tests and every existing decide unit test.

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked decide_prints_the_characterised`

Expected: `test cmd::decide::tests::decide_prints_the_characterised_document_for_each_made_up_fixture ... ok`. The documents of Task 17 are byte-identical.

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`

Expected:
- every binary reports `test result: ok`, with 0 failed;
- the 6 tests of `crates/dev/tests/decide.rs` pass, and their final-path comparison prints no `with library roots:` failure;
- `clone_values` passes.

- [ ] **Step 5: Commit**

```
git add crates/dev/src/cmd/decide.rs crates/dev/src/cmd/plan.rs crates/dev/tests/decide.rs
git commit -m "dev::decide: readers build DecideInputs and a pure decide_doc renders them (K5)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>` and no attribution line.

---

### Task 20: decide's replay flags and the absolute-root refusal (design parts 7.1, 9.5)

**Files:**
- Modify: `crates/dev/src/cmd/decide.rs`:
  - module doc `:1-13`, `USAGE` `:60` and `HELP` `:62-86`;
  - `run` `:390-470` (three flags);
  - `read_decide_inputs` (Task 19), which gains the root check and the `--tmdb-record` record;
  - tests: `argument_errors_are_reported_as_argparse_reports_them` `:1401-1432` and `the_command_is_registered_with_its_synopsis` `:2398-2404`, plus 3 new tests.
- Modify: `crates/dev/src/cmd/plan.rs:461` (`option_text`) and `:495` (`parse_record`), which become `pub(crate)`.
- Modify: `crates/dev/src/lib.rs:26`, decide's synopsis row.

**Interfaces:**
- Consumes:
  - `Settings::from_json(text: &str) -> Result<(Settings, Vec<SettingsProblem>), String>` (settings.rs:377);
  - `is_absolute_root(path: &str) -> bool` (settings.rs:211);
  - `SettingsProblem { key, problem }` (settings.rs:238);
  - `option_text(ctx: &Ctx, path: &Path) -> Result<(String, String), Refused>` (plan.rs:461);
  - `parse_record(text: &str, shown: &str) -> Result<TmdbRecord, String>` (plan.rs:495);
  - `PlanOptions` (plan.rs:118);
  - `read_decide_inputs` and `DecideInputs` (Task 19);
  - `RecordedAnswers` and `recorded_answers(rec: &Recording) -> Result<RecordedAnswers, Refused>` (plan.rs:270, :281, `pub` since Task 19).
- Produces, in `crates/dev/src/cmd/decide.rs`:

```rust
/// The answers decide's decision reads (S49): `recorded_answers(rec)`, its Refused.line held back as the Err, with
/// `recorded` replaced by `--tmdb-record`'s record when `options.tmdb_record` is Some and both `option_text` and
/// `parse_record` succeed. read_decide_inputs and the engine (Task 24) both fill `DecideInputs.answers` with it, so
/// the two documents of R-B1 read the answers one way. Def: T20. Use: T20, T24.
pub fn replay_answers(ctx: &Ctx, rec: &Recording, options: &PlanOptions) -> Result<RecordedAnswers, String>;
```

  - `option_text` and `parse_record` become `pub(crate)`. Their docs gain "Use: T15, T20."
  - The new `USAGE` is `"decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE] [--tmdb-search FILE] [--tmdb-record FILE] [--remembered FILE] [--evidence DIR]"`.
  - The new `COMMANDS` row is `("decide", cmd::decide::run as Command, "(<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE] [--tmdb-search FILE] [--tmdb-record FILE] [--remembered FILE]")`.

Rules:
- `read_decide_inputs` checks the roots first, before the rip label. If `options.settings` is Some, it reads the file with `option_text`. When that read succeeds and `Settings::from_json` returns Ok, it checks `movies_root` and then `tv_root`. The first that is `Some(root)` with `!is_absolute_root(root)` gives `Refused { code: 2, line: format!("{shown}: {key}: {problem}") }`. Here `problem` is the text of the first `SettingsProblem` whose `key` is that key.
- When the read or `from_json` fails, nothing is refused. `plan_doc` refuses the file and the plan is skipped, as today.
- `replay_answers` holds the `--tmdb-record` rule: when `options.tmdb_record` is Some, `recorded_answers` is Ok, and `option_text` and `parse_record` succeed, `recorded` becomes that record (S49). Any failure leaves the answers as read. `plan_doc` then refuses the file, so the plan and the decision are skipped. `read_decide_inputs` sets `answers: replay_answers(ctx, rec, options)`.

Rulings:
- Ruling: the root check runs before the recording is read, so an argument error is reported before any recording error, as argparse's are.
- Ruling: when both roots fail, `movies_root` is named, following spec 14's key order, which `validate` keeps.
- Ruling: `option_text` and `parse_record` become `pub(crate)` rather than being copied, so there is one reading of each file.
- Note: `decide`'s exit 3 for a stale TMDb copy (K15) is PR 4, together with the refetch (S40).

- [ ] **Step 1: Write the failing tests in `decide.rs`'s `mod tests`**

Add to the test imports:
- `use lazymkv_core::remembered::{Remembered, RememberedTitle, TitleRole};`
- `use lazymkv_core::testsupport::{search_json, tmdb_movie_json};` (beside the existing testsupport import).

```rust
    #[test]
    fn decide_reads_tmdb_search_tmdb_record_and_remembered_like_plan() {
        let tmp = Scratch::new("replay-flags");
        let root = tmp.0.clone();
        disc_b(&root);
        write(&root.join("recs/disc-b/thediscdb-hash.json"), discdb_answer());
        write(&root.join("recs/disc-b/tmdb.json"), tmdb_answer(4242));
        write(&root.join("settings.json"), json!({"movies_root": "/srv/media/Movies"}).to_string());
        write(&root.join("search.json"), search_json(&[("movie", 4242, "Made Up Film", "Made Up Film", "2001-05-04")]));
        // The plan's identity with another title and year than tmdb.json's: the decision must name the file from it (S49).
        write(
            &root.join("record.json"),
            tmdb_movie_json(4242, "Another Made Up Film", "Another Made Up Film", "2003-02-01", "en", Some(50)),
        );
        let remembered = Remembered {
            fingerprint: "0123456789ABCDEF0123456789ABCDEF".to_string(),
            tmdb: TmdbIdentity { kind: TmdbKind::Movie, id: 4242 },
            titles: vec![RememberedTitle {
                source_file: Some("00800.mpls".to_string()),
                dvd_title: None,
                segment_map: "1".to_string(),
                seconds: 3000,
                role: TitleRole::Feature,
                audio: Vec::new(),
                subtitles: Vec::new(),
            }],
        };
        write(&root.join("remembered.json"), serde_json::to_string(&remembered).unwrap());
        let args = [
            "recs/disc-b", "all-tracks", "--settings", "settings.json", "--tmdb-search", "search.json",
            "--tmdb-record", "record.json", "--remembered", "remembered.json",
        ];
        let (code, out, err) = run_in(&root, &args);
        assert_eq!((code, err.as_str()), (0, ""));
        let value: Value = serde_json::from_str(&out).unwrap();
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let options = PlanOptions {
            settings: Some("settings.json".into()),
            tmdb_search: Some("search.json".into()),
            tmdb_record: Some("record.json".into()),
            remembered: Some("remembered.json".into()),
        };
        let planned = plan_doc(&ctx, &PlanSource::Recording(&rec), &options).unwrap();
        assert_eq!(value["plan"], serde_json::to_value(&planned).unwrap());
        let answers = replay_answers(&ctx, &rec, &options).unwrap();
        assert_eq!(answers.recorded.map(|record| record.title), Some("Another Made Up Film".to_string()));
        let plain = replay_answers(&ctx, &rec, &PlanOptions::default()).unwrap();
        assert_eq!(plain, recorded_answers(&rec).unwrap());
        let given = (
            planned.inputs.tmdb_search.as_deref(),
            planned.inputs.tmdb_record.as_deref(),
            planned.inputs.remembered.as_deref(),
        );
        assert_eq!(given, (Some("search.json"), Some("record.json"), Some("remembered.json")));
        assert_eq!(planned.plan.identity.tmdb, Some(TmdbIdentity { kind: TmdbKind::Movie, id: 4242 }));
        let path = "/srv/media/Movies/Another Made Up Film (2003) {tmdb-4242}/Another Made Up Film (2003) {tmdb-4242} - 1080p.mkv";
        assert_eq!(value["decision"]["decision"]["path"]["path"], json!(path));
    }

    #[test]
    fn decide_refuses_a_relative_library_root_with_exit_2_naming_the_key() {
        let tmp = Scratch::new("relative-root");
        let root = tmp.0.clone();
        disc_b(&root);
        let not_absolute = |key: &str, value: &str| {
            format!(
                "settings.json: {key}: {key} {value} is not an absolute path: a library root must be a UNC path, a \
                 drive-letter path or a path from / (spec 7, 14)"
            )
        };
        let cases = [
            (json!({"movies_root": "Movies"}), not_absolute("movies_root", "Movies")),
            (json!({"tv_root": "D:Shows"}), not_absolute("tv_root", "D:Shows")),
            (
                json!({"movies_root": "  "}),
                "settings.json: movies_root: movies_root is blank: a library root must be an absolute path (spec 7, 14)"
                    .to_string(),
            ),
            (json!({"movies_root": "Movies", "tv_root": "Shows"}), not_absolute("movies_root", "Movies")),
        ];
        for (settings, line) in cases {
            write(&root.join("settings.json"), settings.to_string());
            let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks", "--settings", "settings.json"]);
            assert_eq!((code, out.as_str(), err), (2, "", format!("lazymkv-dev decide: error: {line}\n")), "{settings}");
        }
        // The reader refuses it the same way, so lazymkvctl and the engine's replay meet the same rule (OR13).
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let options = PlanOptions { settings: Some(PathBuf::from("settings.json")), ..PlanOptions::default() };
        let refused = read_decide_inputs(&ctx, &rec, "all-tracks", None, None, &options).unwrap_err();
        assert_eq!(refused, Refused { code: 2, line: not_absolute("movies_root", "Movies") });
    }

    #[test]
    fn an_absolute_posix_root_is_accepted() {
        let tmp = Scratch::new("absolute-root");
        let root = tmp.0.clone();
        disc_b(&root);
        for library in ["/srv/media/Movies", r"\\nas\media\Movies", r"D:\Movies"] {
            write(&root.join("settings.json"), json!({"movies_root": library}).to_string());
            let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks", "--settings", "settings.json"]);
            assert_eq!((code, err.as_str()), (0, ""), "{library}");
            let value: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(value["plan"]["settings"]["movies_root"], json!(library));
        }
    }
```

Update the two existing tests:
- `argument_errors_are_reported_as_argparse_reports_them` (`:1404-1405`):
  - its `usage` becomes `"usage: lazymkv-dev decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE] [--tmdb-search FILE] [--tmdb-record FILE] [--remembered FILE] [--evidence DIR]\n"`;
  - `cases` becomes `[(&[&str], &str); 9]`, with the added case `(&["recs/disc-a", "eng-only", "--tmdb-record"], "argument --tmdb-record: expected one argument")`;
  - it also asserts `out.contains("  --remembered FILE ")` on the `-h` output.
- `the_command_is_registered_with_its_synopsis` (`:2402`) expects `Some("(<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE] [--tmdb-search FILE] [--tmdb-record FILE] [--remembered FILE]")`.

The test `a_settings_file_gives_decides_own_decision_its_final_path` (`:2338-2373`) stays as it is. Its `bad-settings.json` part must still exit 0, with the plan skipped.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked cmd::decide`

Expected: compile error, `error[E0425]: cannot find function `replay_answers` in this scope`, from the body of `decide_reads_tmdb_search_tmdb_record_and_remembered_like_plan`, because Step 3 defines `replay_answers`. No test runs until then. After Step 3, `decide_refuses_a_relative_library_root_with_exit_2_naming_the_key`, `argument_errors_are_reported_as_argparse_reports_them` and `the_command_is_registered_with_its_synopsis` pass for the first time, and `an_absolute_posix_root_is_accepted`, which the old code already met, keeps passing.

- [ ] **Step 3: Implement the flags and the refusal**

1. `plan.rs:461` and `:495`: `fn` becomes `pub(crate) fn`.
2. `run`:
   - `Args::new(args, &["--rec", "--text", "--selection", "--settings", "--tmdb-search", "--tmdb-record", "--remembered", "--evidence"])`;
   - take the three new flags with `parsed.flag(..)`, each printing its Err through `usage_error` as the others do;
   - `options` becomes `PlanOptions { settings, tmdb_search, tmdb_record, remembered }`, each `.map(PathBuf::from)`.
3. `read_decide_inputs` applies the Rules above: the root check first, and `answers: replay_answers(ctx, rec, options)`. `replay_answers` is `recorded_answers(rec).map_err(|refused| refused.line)`, then the `--tmdb-record` replacement.
4. `USAGE`, the `lib.rs:26` row, and `HELP`:
   - `HELP` gains three rows after `--settings FILE`:
     - `--tmdb-search FILE    a TMDb /search/multi answer for the disc's name, for the plan section (as plan's)`
     - `--tmdb-record FILE    a TMDb /movie/{id} or /tv/{id} answer: replaces the recording's tmdb.json for the plan section and names the decision's file`
     - `--remembered FILE     a remembered decision for the disc's fingerprint, for the plan section (spec 5)`
   - Its exit-status paragraph adds to 2 "a library root in the settings file that is not absolute".
5. The module doc `:1-13` names the three replay flags (design part 7.1) and the refusal (design part 9.5, OR13) in one sentence each.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked cmd::decide`

Expected: PASS, including the 3 new tests, the two updated tests and `a_settings_file_gives_decides_own_decision_its_final_path`.

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`

Expected:
- every binary reports `test result: ok`;
- the 6 decide goldens and `decide_prints_the_characterised_document_for_each_made_up_fixture` pass unchanged.

- [ ] **Step 5: Commit**

```
git add crates/dev/src/cmd/decide.rs crates/dev/src/cmd/plan.rs crates/dev/src/lib.rs
git commit -m "dev::decide: --tmdb-search, --tmdb-record and --remembered; a relative library root exits 2 (design parts 7.1, 9.5, OR13)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>` twice, with no attribution line.

---

### Task 21: The job store, engine.log and the binary

**Files:**
- Create: `crates/engine/src/store.rs`, with 9 tests in its `mod tests`.
- Create: `crates/engine/tests/binary.rs`, with 2 tests, one compiled per OS.
- Modify: `crates/engine/src/lib.rs` (created in Task 1): `mod store;` (Step 1), `pub fn main_with`, and 1 test.
- Modify: `crates/engine/src/main.rs` (created in Task 1).

**Interfaces:**

Consumes:

```rust
// core::record (Task 6)
pub const JOB_FORMAT: &str = "lazymkv job v1";
pub const DRIVES_FORMAT: &str = "lazymkv drives v1";
pub const REMEMBERED_FORMAT: &str = "lazymkv remembered v1";
pub fn check_format(document: &serde_json::Value, expected: &str) -> Result<(), String>;
pub struct JobRecord { pub format, pub id, pub fingerprint, pub disc_id, pub drive, pub created_unix, pub updated_unix,
    pub state: job::State, pub message, pub rip_again, pub profile, pub plan, pub titles, pub filed_unix, pub collisions, pub notes }
pub struct Transition { pub time_unix: u64, pub event: Option<job::Event>, pub old: Option<job::State>, pub new: job::State, pub actions: Vec<job::Action> }
pub struct DrivesFile { pub format: String, pub drives: BTreeMap<String, DriveRecord>, pub drive_incapable: Vec<DriveIncapableEntry> }
pub struct DriveRecord { pub fingerprint: Option<String>, pub handled: bool }
// platform (Task 9)
pub const TEMP_SUFFIX: &str = ".lazymkv-tmp";
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()>;
// tools (Task 11)
pub struct Limits { pub header: Duration, pub scan: Duration, pub exit_wait: Duration, pub after_kill: Duration }
// core
pub struct Remembered { pub fingerprint: String, pub tmdb: TmdbIdentity, pub titles: Vec<RememberedTitle> } // remembered.rs:114
pub struct Settings { .. }                                                                                // settings.rs:73
pub struct SettingsProblem { pub key: String, pub problem: String }                                       // settings.rs:238
impl Settings { pub fn validate(&self) -> Vec<SettingsProblem>;                                           // :262
    pub fn from_json(text: &str) -> Result<(Settings, Vec<SettingsProblem>), String>; }                   // :377
// job.rs: State :222, Event :454, Action :586
// dev
pub struct Args;                              // lib.rs:86
impl Args { pub fn new(args: &[String], flags: &[&str]) -> Args;                 // :103
    pub fn flag(&mut self, name: &str) -> Result<Option<String>, String>;        // :109
    pub fn positionals(self) -> Result<Vec<String>, String>; }                   // :137
```

Produces:

```rust
// crates/engine/src/lib.rs
/// The engine's start-up configuration (design parts 3.5, 3.6; S44): the data folder, the resources folder the tool
/// lookup reads second, limits that replace `Limits::from_settings` (tests only), and the `PATH` the tool lookup reads
/// last. Def: T21. Use: T22 to T29.
pub struct EngineConfig { pub data: PathBuf, pub resources: Option<PathBuf>, pub limits_override: Option<Limits>, pub path_var: Option<OsString> }
/// The lazymkv-engine binary's run (design parts 3.1, 4.2, 4.6; S28). `--data DIR` is parsed with dev's `Args`. A
/// usage error prints "usage: lazymkv-engine --data DIR" and "lazymkv-engine: error: <message>" to `err` and returns
/// 2. Otherwise PR 2 prints its OS's sentence and returns 1: "lazymkv-engine runs only on Windows in this version"
/// outside Windows, and "lazymkv-engine has no Windows implementations in this build" on Windows. It creates nothing.
/// Def: T21. Use: T21.
pub fn main_with(args: &[String], err: &mut dyn Write) -> i32;

// crates/engine/src/store.rs (private module; pub(crate) items)
/// The data folder's persisted files (design parts 3.5, 7.3, 7.4): `<data>/jobs/<id>/state.json` and
/// `transitions.jsonl`, `<data>/drives.json`, `<data>/remembered/<fingerprint>.json`, `<data>/settings.json` and
/// `<data>/engine.log`. Def: T21. Use: T22 to T26.
pub(crate) struct Store { /* data: PathBuf, now: Arc<dyn Fn() -> u64 + Send + Sync>, console: Mutex<Option<Box<dyn Write + Send>>> */ }
impl Store {
    /// `now` gives the Unix seconds of engine.log lines. `console` is standard error in a console run. Touches no file.
    pub(crate) fn new(data: PathBuf, now: Arc<dyn Fn() -> u64 + Send + Sync>, console: Option<Box<dyn Write + Send>>) -> Store;
    /// `<data>/jobs/<id>`.
    pub(crate) fn job_dir(&self, id: &str) -> PathBuf;
    /// Every `<data>/jobs/*/state.json` in folder-name order. Each is read as a Value, passed through
    /// `check_format(JOB_FORMAT)` and then into JobRecord. An Err is "<state.json path>: <reason>". A folder with no
    /// state.json is skipped.
    pub(crate) fn load_jobs(&self) -> Vec<Result<JobRecord, String>>;
    /// Design part 7.3's order. It refuses (`ErrorKind::InvalidData`, the check_format text) when an existing
    /// state.json fails `check_format(JOB_FORMAT)` or is not JSON, and then writes nothing. Otherwise it writes
    /// state.json with `write_atomic` (`to_vec_pretty` plus "\n"), then appends `to_string(transition)` plus "\n" to
    /// transitions.jsonl and calls `sync_data`.
    pub(crate) fn persist(&self, record: &JobRecord, transition: &Transition) -> io::Result<()>;
    /// drives.json through `check_format(DRIVES_FORMAT)`. An absent file gives the empty record with DRIVES_FORMAT.
    pub(crate) fn drives(&self) -> Result<DrivesFile, String>;
    /// Refuses as `persist` does when the existing file fails the check; otherwise `write_atomic` of `to_vec_pretty` plus "\n".
    pub(crate) fn write_drives(&self, drives: &DrivesFile) -> io::Result<()>;
    /// `<data>/remembered/<fingerprint>.json` through `check_format(REMEMBERED_FORMAT)`, with the file's bytes for
    /// remembered.json. Absent gives Ok(None).
    pub(crate) fn remembered(&self, fingerprint: &str) -> Result<Option<(Remembered, Vec<u8>)>, String>;
    /// `<data>/settings.json` through `Settings::from_json`. Absent gives `Settings::default()` with its `validate()`.
    /// An unreadable file gives `Settings::default()` and one problem { key: "settings.json", problem: "<path>: <error>" }.
    pub(crate) fn settings(&self) -> (Settings, Vec<SettingsProblem>);
    /// `write_atomic` of `to_vec_pretty(settings)` plus "\n", with no `format` key (design part 7.4).
    pub(crate) fn write_settings(&self, settings: &Settings) -> io::Result<()>;
    /// Appends "<now> <line>\n" to engine.log, and writes the same text to `console` when one is set. Errors are ignored.
    pub(crate) fn log(&self, line: &str);
}
```

Rulings:
- Ruling (S28): the PR 2 binary exits 1 on every OS with its OS's sentence, so the CI whole-job tests drive the library.
- Ruling: `Store::new` takes `now` as a closure and the console as an optional writer, so the store needs no `Host` and its tests need no fake. The engine passes `move || host.now_unix()`.
- Ruling: an engine.log line is `<Unix seconds> <text>`. Design part 4.6 writes new times as Unix seconds, and part 3.5 names no line format.
- Ruling: the refusal is checked at each write by re-reading the existing file's `format`, so a refused file is never overwritten without the store holding any state.
- Ruling: a job folder with no `state.json` is skipped by `load_jobs`. S29 persists `Detected` first, so such a folder holds no job, and its disc's next arrival rewrites it.
- Ruling: an unreadable data-folder `settings.json` gives the defaults with one problem keyed `settings.json`. Any problem stops new jobs (design part 7.4), and the snapshot then names the file.
- Ruling: the store's tests use a temporary folder of their own in `mod tests`, so they run with no feature and no dev-dependency. The binary tests live in `crates/engine/tests/binary.rs`, the only place `CARGO_BIN_EXE_lazymkv-engine` exists, as Task 3 does for capture.
- Note: `Store` has no caller until Task 22, so dead-code warnings are expected. CI does not deny warnings (ci.yml:50-53).

- [ ] **Step 1: Write the failing tests**

First add `mod store;` to `crates/engine/src/lib.rs`, so the new file compiles with its tests.

In `crates/engine/src/store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use lazymkv_core::job::{Action, Event, State};
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, Transition, DRIVES_FORMAT, JOB_FORMAT};
    use lazymkv_core::remembered::Remembered;
    use lazymkv_core::settings::Settings;
    use lazymkv_platform::TEMP_SUFFIX;
    use serde_json::{json, Value};

    /// A temporary folder removed on drop.
    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Temp {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("lazymkv-engine-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A console that keeps what it is given.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn store_in(data: &Path) -> Store {
        Store::new(data.to_path_buf(), Arc::new(|| 7), None)
    }

    fn record(state: State, updated_unix: u64) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(), id: "abc-1".to_string(), fingerprint: "ABC".to_string(), disc_id: None,
            drive: "tok0".to_string(), created_unix: 7, updated_unix, state, message: None, rip_again: false,
            profile: "job.mmcp.xml".to_string(), plan: None, titles: Vec::new(), filed_unix: None,
            collisions: Vec::new(), notes: Vec::new(),
        }
    }

    fn first() -> Transition {
        Transition { time_unix: 7, event: None, old: None, new: State::Detected, actions: vec![Action::Fingerprint] }
    }

    fn empty_drives() -> DrivesFile {
        DrivesFile { format: DRIVES_FORMAT.to_string(), drives: BTreeMap::new(), drive_incapable: Vec::new() }
    }

    #[test]
    fn persist_replaces_state_json_then_appends_one_transition_line() {
        let tmp = Temp::new("persist");
        let store = store_in(&tmp.0);
        let second = Transition {
            time_unix: 8, event: Some(Event::FingerprintDone), old: Some(State::Detected), new: State::Fingerprinted,
            actions: vec![Action::RunScan],
        };
        store.persist(&record(State::Detected, 7), &first()).unwrap();
        store.persist(&record(State::Fingerprinted, 8), &second).unwrap();
        let dir = tmp.0.join("jobs/abc-1");
        assert_eq!(store.job_dir("abc-1"), dir);
        let state = fs::read(dir.join("state.json")).unwrap();
        assert!(
            state.starts_with(b"{\n  \"format\": \"lazymkv job v1\",\n  \"id\": \"abc-1\",\n"),
            "{}",
            String::from_utf8_lossy(&state)
        );
        assert_eq!(serde_json::from_slice::<JobRecord>(&state).unwrap(), record(State::Fingerprinted, 8));
        let lines = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
        let expected =
            format!("{}\n{}\n", serde_json::to_string(&first()).unwrap(), serde_json::to_string(&second).unwrap());
        assert_eq!(lines, expected);
        let names: Vec<String> =
            fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|name| !name.ends_with(TEMP_SUFFIX)), "{names:?}");
        assert_eq!(store.load_jobs(), vec![Ok(record(State::Fingerprinted, 8))]);
    }

    #[test]
    fn a_job_folder_with_a_refused_format_is_reported_and_never_overwritten() {
        let tmp = Temp::new("job-format");
        let store = store_in(&tmp.0);
        let refused = r#"{"format":"lazymkv job v2","id":"abc-1"}"#;
        let missing = r#"{"id":"abc-2"}"#;
        for (id, text) in [("abc-1", refused), ("abc-2", missing)] {
            fs::create_dir_all(tmp.0.join("jobs").join(id)).unwrap();
            fs::write(tmp.0.join("jobs").join(id).join("state.json"), text).unwrap();
        }
        // A folder that never persisted its first transition holds no job.
        fs::create_dir_all(tmp.0.join("jobs/abc-3")).unwrap();
        fs::write(tmp.0.join("jobs/abc-3/meta.json"), "{}").unwrap();
        let loaded = store.load_jobs();
        assert_eq!(loaded.len(), 2, "{loaded:?}");
        let first_error = loaded[0].clone().unwrap_err();
        assert!(first_error.contains("abc-1") && first_error.contains("lazymkv job v2"), "{first_error}");
        assert!(first_error.contains("lazymkv job v1"), "{first_error}");
        assert!(loaded[1].clone().unwrap_err().contains("abc-2"), "{loaded:?}");
        let error = store.persist(&record(State::Detected, 9), &first()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(tmp.0.join("jobs/abc-1/state.json")).unwrap(), refused);
        assert!(!tmp.0.join("jobs/abc-1/transitions.jsonl").exists());
    }

    #[test]
    fn an_absent_drives_json_is_an_empty_record() {
        let tmp = Temp::new("drives-absent");
        let drives = store_in(&tmp.0).drives().unwrap();
        assert_eq!(
            serde_json::to_value(&drives).unwrap(),
            json!({"format": "lazymkv drives v1", "drives": {}, "drive_incapable": []})
        );
    }

    #[test]
    fn drives_json_is_rewritten_with_its_format() {
        let tmp = Temp::new("drives");
        let store = store_in(&tmp.0);
        let mut drives = empty_drives();
        drives.drives.insert("tok0".to_string(), DriveRecord { fingerprint: Some("ABC".to_string()), handled: false });
        store.write_drives(&drives).unwrap();
        drives.drives.get_mut("tok0").unwrap().handled = true;
        store.write_drives(&drives).unwrap();
        let text = fs::read_to_string(tmp.0.join("drives.json")).unwrap();
        assert!(text.starts_with("{\n  \"format\": \"lazymkv drives v1\",\n"), "{text}");
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            json!({"format": "lazymkv drives v1", "drives": {"tok0": {"fingerprint": "ABC", "handled": true}},
                   "drive_incapable": []})
        );
        assert_eq!(serde_json::to_value(store.drives().unwrap()).unwrap(), value);
    }

    #[test]
    fn a_drives_json_with_a_refused_format_is_reported_and_never_overwritten() {
        let tmp = Temp::new("drives-format");
        let store = store_in(&tmp.0);
        let text = r#"{"format":"lazymkv drives v2","drives":{}}"#;
        fs::write(tmp.0.join("drives.json"), text).unwrap();
        let error = store.drives().unwrap_err();
        assert!(error.contains("lazymkv drives v2"), "{error}");
        assert_eq!(store.write_drives(&empty_drives()).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(tmp.0.join("drives.json")).unwrap(), text);
    }

    #[test]
    fn a_remembered_file_of_another_major_version_is_refused_by_the_store() {
        let tmp = Temp::new("remembered");
        let store = store_in(&tmp.0);
        let remembered = Remembered {
            fingerprint: "ABC".to_string(),
            tmdb: TmdbIdentity { kind: TmdbKind::Movie, id: 900001 },
            titles: Vec::new(),
        };
        let with_format = |format: &str| {
            let mut value = serde_json::to_value(&remembered).unwrap();
            value.as_object_mut().unwrap().insert("format".to_string(), json!(format));
            serde_json::to_vec(&value).unwrap()
        };
        assert_eq!(store.remembered("ABC"), Ok(None));
        fs::create_dir_all(tmp.0.join("remembered")).unwrap();
        let v1 = with_format("lazymkv remembered v1");
        fs::write(tmp.0.join("remembered/ABC.json"), &v1).unwrap();
        assert_eq!(store.remembered("ABC"), Ok(Some((remembered.clone(), v1))));
        fs::write(tmp.0.join("remembered/ABC.json"), with_format("lazymkv remembered v2")).unwrap();
        let error = store.remembered("ABC").unwrap_err();
        assert!(error.contains("lazymkv remembered v2"), "{error}");
    }

    #[test]
    fn settings_json_round_trips_without_a_format_key() {
        let tmp = Temp::new("settings");
        let store = store_in(&tmp.0);
        let (defaults, problems) = store.settings();
        assert_eq!(defaults, Settings::default());
        assert_eq!(problems.iter().map(|problem| problem.key.as_str()).collect::<Vec<_>>(), ["tuning_root"]);
        let mut changed = Settings::default();
        changed.cache_mb = 512;
        changed.keep_tuning_evidence = false;
        store.write_settings(&changed).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(tmp.0.join("settings.json")).unwrap()).unwrap();
        assert!(value.get("format").is_none(), "{value}");
        let problems = changed.validate();
        assert_eq!(store.settings(), (changed, problems));
    }

    #[test]
    fn an_unreadable_settings_json_is_one_problem_and_the_defaults() {
        let tmp = Temp::new("settings-bad");
        let store = store_in(&tmp.0);
        fs::write(tmp.0.join("settings.json"), "{").unwrap();
        let (settings, problems) = store.settings();
        assert_eq!(settings, Settings::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].key, "settings.json");
        let path = tmp.0.join("settings.json");
        assert!(problems[0].problem.starts_with(&format!("{}: ", path.display())), "{problems:?}");
    }

    #[test]
    fn engine_log_lines_also_go_to_stderr_in_a_console_run() {
        let tmp = Temp::new("log");
        let console = Shared::default();
        let store = Store::new(tmp.0.clone(), Arc::new(|| 7), Some(Box::new(console.clone())));
        store.log("engine started");
        store.log("an arrival in tok0 made no job: the volume could not be listed");
        let expected = "7 engine started\n7 an arrival in tok0 made no job: the volume could not be listed\n";
        assert_eq!(fs::read_to_string(tmp.0.join("engine.log")).unwrap(), expected);
        assert_eq!(String::from_utf8(console.0.lock().unwrap().clone()).unwrap(), expected);
        let quiet = Temp::new("log-quiet");
        Store::new(quiet.0.clone(), Arc::new(|| 8), None).log("x");
        assert_eq!(fs::read_to_string(quiet.0.join("engine.log")).unwrap(), "8 x\n");
    }
}
```

In `crates/engine/src/lib.rs`'s `mod tests`:

```rust
    #[test]
    fn a_missing_data_value_is_a_usage_error_exit_2() {
        let words = |text: &[&str]| text.iter().map(|word| word.to_string()).collect::<Vec<String>>();
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["--data"]), &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-engine --data DIR\nlazymkv-engine: error: argument --data: expected one argument\n"
        );
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["--data", "x", "extra"]), &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-engine --data DIR\nlazymkv-engine: error: unrecognized arguments: extra\n"
        );
        let mut err = Vec::new();
        assert_eq!(main_with(&[], &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-engine --data DIR\nlazymkv-engine: error: the following arguments are required: --data\n"
        );
    }
```

In `crates/engine/tests/binary.rs`:

```rust
//! The PR 2 engine binary (design part 4.2, S28): it creates nothing and exits 1 with its OS's sentence.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run_engine(data: &Path) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(data).output().unwrap();
    (output.status.code(), String::from_utf8(output.stderr).unwrap())
}

fn absent_data() -> PathBuf {
    std::env::temp_dir().join(format!("lazymkv-engine-binary-{}", std::process::id()))
}

#[cfg(not(windows))]
#[test]
fn the_binary_refuses_to_run_outside_windows_in_this_version() {
    let data = absent_data();
    assert_eq!(run_engine(&data), (Some(1), "lazymkv-engine runs only on Windows in this version\n".to_string()));
    assert!(!data.exists());
}

#[cfg(windows)]
#[test]
fn the_binary_has_no_windows_implementations_in_this_build() {
    let data = absent_data();
    assert_eq!(run_engine(&data), (Some(1), "lazymkv-engine has no Windows implementations in this build\n".to_string()));
    assert!(!data.exists());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked`

Expected: compile errors: `error[E0433]: failed to resolve: use of undeclared type `Store`` in store.rs's tests (Step 1 declared `mod store;`, so the file compiles), and `cannot find function `main_with``.

- [ ] **Step 3: Implement the store, `main_with` and `main.rs`**

1. In `crates/engine/src/store.rs`, add `Store` with the signatures and docs above, and these private constants:
   - `JOBS_DIR = "jobs"`
   - `STATE_FILE = "state.json"`
   - `TRANSITIONS_FILE = "transitions.jsonl"`
   - `DRIVES_FILE = "drives.json"`
   - `SETTINGS_FILE = "settings.json"`
   - `REMEMBERED_DIR = "remembered"`
   - `ENGINE_LOG = "engine.log"`
   
   Every write creates its parent folders with `create_dir_all`. Refusals are `io::Error::new(io::ErrorKind::InvalidData, <message>)`. Every pub(crate) item's `///` doc names design part 3.5, 7.3 or 7.4 and ends "Def: T21. Use: T22, T23, T25, T26."
2. In `crates/engine/src/lib.rs`, add `pub fn main_with` (Step 1 declared `mod store;`):
   - `Args::new(args, &["--data"])`, then `flag("--data")?`, then `positionals()?`. Either Err is written as the two usage lines and returns 2.
   - `positionals()` returns value-like words as Ok, so `main_with` builds two messages itself: a non-empty Ok gives `unrecognized arguments: <the words joined by " ">`, as decide.rs:451 forms it, and an absent `--data` gives `the following arguments are required: --data`. Both are written as the two usage lines and return 2.
   - `EngineConfig` is the struct in Produces, with its `///` doc.
   - Otherwise it writes the `cfg`-selected sentence plus "\n" and returns 1.
3. `crates/engine/src/main.rs`: `fn main() { let args: Vec<String> = std::env::args().skip(1).collect(); std::process::exit(lazymkv_engine::main_with(&args, &mut std::io::stderr())); }`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked`

Expected: PASS: the 9 store tests, `a_missing_data_value_is_a_usage_error_exit_2`, and on the mini PC `the_binary_has_no_windows_implementations_in_this_build` (`the_binary_refuses_to_run_outside_windows_in_this_version` is compiled out on Windows).

- [ ] **Step 5: Commit**

```
git add crates/engine/src/store.rs crates/engine/src/lib.rs crates/engine/src/main.rs crates/engine/tests/binary.rs
git commit -m "engine: the job store, engine.log and the --data binary (design parts 3.5, 7.3, 7.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>` twice, with no attribution line.

### Task 22: The job-owner loop, arrivals, the disc-phase token and the queue (design parts 4.4, 5.4; K8, L2)

**Files:**
- Create: `crates/engine/src/owner.rs`, `crates/engine/src/owner/tests.rs` (19 tests), `crates/engine/src/arrival.rs` (2 tests in its `mod tests`), `crates/engine/src/timers.rs`, `crates/engine/src/notify.rs`, `crates/engine/src/testkit.rs`
- Every test module here compiles in a plain `cargo test -p lazymkv-engine`: `lib.rs` declares `#[cfg(any(test, feature = "testkit"))] pub mod testkit;`, and Task 1's engine dev-dependencies enable the platform fakes and core's `testsupport` for the unit tests.
- Modify: `crates/engine/src/lib.rs` (module declarations and re-exports), `crates/engine/src/store.rs` (two `pub(crate)` methods, below)

**Interfaces:**

Consumes:
- `core::job`:
  - `State` (job.rs:222), `NotifyEvent` (:413), `Event` (:454), `Action` (:586), `Failure` (:128), `FailureClass` (:25)
  - `new_job() -> (State, Vec<Action>)` (:716), `reduce(&State, &Event) -> Result<(State, Vec<Action>), Rejected>` (:769)
  - `job_id(&str, &[&str]) -> String` (:1155, lower case after Task 2)
  - `JobLookup { Resume(String), AlreadyRipped(String), NewJob }` (:1176), `job_lookup(&[(String, State, bool)]) -> JobLookup` (:1193)
- `core::record::{JobRecord, Transition, DrivesFile, DriveRecord, JOB_FORMAT, DRIVES_FORMAT}` (Task 6), `core::ipc::{Notification, JobSummary, job_summary}` (Task 7), `core::snapshot::{disc_meta, unit_key_file, DiscMeta}` (Task 3), `core::profile::JOB_PROFILE_FILE` (Task 4)
- `core::disc::{bdmt_candidates (disc.rs:586), bdmt_name (disc.rs:505)}`, `core::lookup::name::{disc_name (name.rs:367), DiscName (name.rs:338)}`
- `core::settings::Settings` (settings.rs:73), `Settings::validate` (settings.rs:262)
- `core::review::Noted` (review.rs:200)
- `tools::{list_files, snapshot, CancelToken, Limits}` (Tasks 11, 12)
- `clients::{LookupBook, Answers, ChainInputs, RecordedTransport}` (Tasks 15, 16)
- `platform::{Platform, DriveEvent, Host, StdProcessGuard, write_atomic}` and `platform::fakes::*` (Tasks 8, 9)
- `EngineConfig` and the private `Store` (Task 21)

Produces (lib.rs re-exports `start`, `EngineHandle`, `Services`, `StepRunner`, `StepResult`, `JobContext`, `NotifySink`, `Timers`, `StdTimers`; `pub mod testkit` under `#[cfg(any(test, feature = "testkit"))]`):

```rust
// crates/engine/src/notify.rs
/// Where notifications go (design part 8.6, S36): PR 2's sink; PR 4 adds webhook delivery. Def: T22. Use: T22, T26, T29.
pub trait NotifySink: Send { fn deliver(&mut self, notification: &Notification); }
/// Spec 12's `message` for each event (pinned texts below). Def: T22. Use: T22, T24, T26.
pub(crate) fn message(event: &NotifyEvent, record: Option<&JobRecord>) -> String;
/// The notifier thread (design part 4.4): it drains the channel in order into the sink. Def: T22. Use: T22.
pub(crate) fn spawn_notifier(sink: Box<dyn NotifySink>) -> std::sync::mpsc::Sender<Notification>;

// crates/engine/src/timers.rs
/// StartTimer's timers (design part 5.2): `fire` runs once after `minutes`. Def: T22. Use: T22, T24, T25.
pub trait Timers: Send + Sync { fn start(&self, minutes: u32, fire: Box<dyn FnOnce() + Send>); }
/// One sleeping thread per timer. The PR 2 binary builds no Services (S28), so nothing in PR 2 starts one; PR 3's binary
/// passes it. Def: T22. Use: none in PR 2.
pub struct StdTimers;

// crates/engine/src/owner.rs
/// A step's outcome (design part 5.2): the event to feed, if any, and the sentence for the job record and notification.
/// Def: T22. Use: T23, T24, T26.
#[derive(Clone, Debug, PartialEq)] pub struct StepResult { pub event: Option<Event>, pub message: Option<String> }
/// What a step sees of its job: a copy of the record, the job folder, the job's own settings (ENG-19), the drive's
/// volume root while its disc is in, and the lookup book (empty until the job's first RunScan, Task 23). Def: T22. Use: T23, T24.
pub struct JobContext { pub record: JobRecord, pub job_dir: PathBuf, pub settings: Settings, pub drive_root: Option<PathBuf>,
    pub lookups: Arc<Mutex<LookupBook>> }
/// Runs every action the owner thread does not perform itself (design part 5.2, the sheet's owner/runner split).
/// Def: T22. Use: T23, T24.
pub trait StepRunner: Send + Sync { fn run(&self, job: &JobContext, action: &Action, cancel: CancelToken,
    progress: &(dyn Fn(Option<&str>, f64) + Sync)) -> StepResult; }
/// The engine's outside world. `steps: None` means the real runner (Task 23). Def: T22. Use: T22 to T29.
pub struct Services { pub platform: Platform, pub http: Arc<dyn HttpTransport>, pub notify: Box<dyn NotifySink>,
    pub timers: Arc<dyn Timers>, pub steps: Option<Arc<dyn StepRunner>> }
/// Starts the drive, owner and notifier threads over `config.data` (design part 4.4); `start_until(config, services,
/// None)`. Def: T22. Use: T22 to T29.
pub fn start(config: EngineConfig, services: Services) -> io::Result<EngineHandle>;
/// Called by the owner after each persisted transition, before its actions run; true makes the owner run none of
/// them and stop as `EngineHandle::stop` does. Only the testkit passes one (`Harness::run_until`). Def: T22. Use: T22.
pub(crate) type StopAfter = Arc<dyn Fn(&Transition) -> bool + Send + Sync>;
/// `start` with the testkit's stop hook. Def: T22. Use: T22.
pub(crate) fn start_until(config: EngineConfig, services: Services, stop_after: Option<StopAfter>) -> io::Result<EngineHandle>;
/// A running engine. Def: T22. Use: T22 to T29.
pub struct EngineHandle { /* private */ }
impl EngineHandle {
    pub fn jobs(&self) -> Vec<JobSummary>;                                                     // job_summary of each record, by id
    pub fn wait_until(&self, timeout: Duration, done: &dyn Fn(&[JobSummary]) -> bool) -> bool; // polls every 20 ms
    /// Cancels every running step's CancelToken (CheckSpace and Eject included), drops their results, writes the
    /// engine.log line `engine stopped` and joins every thread (design part 3.5).
    pub fn stop(self) -> io::Result<()>;
}
/// A step's changes to its job's record and the events it feeds before its own result, taken by the owner with the
/// step's result and persisted in that result's transition. Def: T22. Use: T23, T24.
#[derive(Clone, Default)] pub(crate) struct Outbox(/* Arc<Mutex<BTreeMap<String, Posted>>> */);
#[derive(Default)] pub(crate) struct Posted { pub(crate) edits: Vec<Box<dyn FnOnce(&mut JobRecord) + Send>>, pub(crate) first: Vec<Event> }
impl Outbox {
    pub(crate) fn edit(&self, job: &str, edit: impl FnOnce(&mut JobRecord) + Send + 'static);
    pub(crate) fn feed_first(&self, job: &str, event: Event);
    pub(crate) fn take(&self, job: &str) -> Posted;  // empties the job's entry
}
/// The disc-phase token and its one FIFO queue (design part 5.4, L2). Def: T22. Use: T22, T25, T26.
#[derive(Default)] pub(crate) struct DiscToken { /* holder: Option<String>, queue: VecDeque<Waiting> */ }
pub(crate) enum Waiting { Arrival(Arrival) }   // Task 26 adds Retry and returns
impl DiscToken {
    pub(crate) fn holder(&self) -> Option<&str>;
    pub(crate) fn take(&mut self, job: &str) -> bool;       // true when free, or already held by `job`
    pub(crate) fn wait(&mut self, waiting: Waiting);
    pub(crate) fn remove_drive(&mut self, drive: &str) -> bool;
    pub(crate) fn after(&mut self, job: &str, state: &State, actions: &[Action]) -> Option<Waiting>; // releases when !keeps_token, then pops the next
}
/// True while the job may still run a disc-phase step: a disc-phase state whose actions do not include WaitForDisc.
pub(crate) fn keeps_token(state: &State, actions: &[Action]) -> bool;

// crates/engine/src/arrival.rs
/// A fingerprinted arrival (design part 5.4 steps 1 and 2). Def: T22. Use: T22, T25.
#[derive(Clone, Debug)] pub(crate) struct Arrival { pub drive: String, pub root: PathBuf, pub number: u64, pub meta: DiscMeta, pub label: String }
/// Waits `settle`, waits up to `ready` for `root` to exist (polled every 100 ms), then `read_arrival`; sends the outcome.
/// A root still missing after `ready` fails with `the volume did not appear within drive_ready_timeout_s (<n> s)`.
pub(crate) fn spawn_arrival(drive: String, root: PathBuf, number: u64, settle: Duration, ready: Duration, host: Arc<dyn Host>,
    done: Sender<Msg>);
/// Lists the volume, reads the unit key, `disc_meta`. A failed walk gives `the disc's fingerprint could not be
/// computed: <io error>`; a fingerprint of None gives `the disc's fingerprint could not be computed: the volume holds
/// no BDMV/STREAM or VIDEO_TS files` (design part 5.4 step 2).
pub(crate) fn read_arrival(drive: &str, root: &Path, number: u64, host: &dyn Host) -> Result<Arrival, String>;
/// Writes the new job's recording with `tools::snapshot` and feeds FingerprintDone, or FingerprintFailed { Unknown }
/// when the snapshot fails or its fingerprint differs from the arrival's.
pub(crate) fn spawn_recording(job: String, generation: u64, arrival: Arrival, job_dir: PathBuf, captured_unix: u64, done: Sender<Msg>);
/// The lookup chain's inputs from a job folder: meta.json's `fingerprint`, `disc_id` and `volume_label`, and the
/// `<di:name>` of the first of `bdmt_candidates` under `nav/BDMV/META/DL/` (design part 5.4 step 1, spec 8.1).
/// Def: T22. Use: T23, T24, T25.
pub(crate) fn chain_inputs(job_dir: &Path) -> Result<ChainInputs, String>;

// crates/engine/src/store.rs (added to Task 21's private Store)
pub(crate) fn create_job(&self, id: &str, settings: &Settings) -> io::Result<PathBuf>; // makes jobs/<id>/, writes settings.json (to_vec_pretty, write_atomic)
pub(crate) fn job_log(&self, id: &str, line: &str);                        // appends "<line>\n" to jobs/<id>/job.log
```

testkit (`#[cfg(any(test, feature = "testkit"))]`):

```rust
pub struct MemorySink;          // new() -> (MemorySink, Arc<Mutex<Vec<Notification>>>); deliver pushes a clone
pub struct ManualTimers;        // new(); fire_all(&self) runs and drops every pending timer; pending() -> Vec<u32>
pub enum StepScript { Result(StepResult), Hang }   // Hang returns StepResult { event: None, message: Some("cancelled") } once cancelled
pub struct StepSeen { pub action: String, pub state: String, pub lines: usize } // tag; state.json's state at the call; transitions.jsonl lines
pub struct ScriptedSteps;       // new(script: Vec<(&'static str, StepScript)>); calls() -> Vec<String>; seen() -> Vec<StepSeen>
pub struct Harness;
impl Harness {
    pub fn new_scripted(script: Vec<(&'static str, StepScript)>, settings: serde_json::Value) -> Harness;
    pub fn insert(&self, drive: &str, disc: &str);       // builds <scratch>/volumes/<disc>/ on first use, sets its label, queues the arrival
    pub fn insert_root(&self, drive: &str, root: &Path); // queues an arrival with a root as given
    pub fn remove(&self, drive: &str);                   // queues the removal
    /// Arms the stop hook for the first transition into `state`, hands the queued drive events to the engine, waits
    /// (60 s cap, then panics naming `state` and the states persisted so far) until the owner stopped after persisting
    /// that transition and before running any of its actions, and joins the engine's threads. The hook also logs every
    /// state it sees; when that log, kept since the last `restart` or `run_until`, already holds `state` (possible only
    /// right after `restart`, whose resumed actions start at once), it stops the engine at once instead.
    pub fn run_until(&self, state: &str);
    /// Stops a running engine (`EngineHandle::stop`), drops every pending ManualTimers timer, as a real restart loses
    /// its sleeping threads, and starts a new engine (`start_until`) over the same data folder with the same drive
    /// script, ejector, keep-awake, shares, secrets, MemoryIpc, host, RecordedTransport, ScriptedSteps (used entries
    /// stay used) and MemorySink list.
    pub fn restart(&self);
    pub fn fingerprint(&self, disc: &str) -> String;     // content_hash of the built volume, upper case
    pub fn job_ids(&self) -> Vec<String>;                // the job folders holding a state.json, sorted
    pub fn state_of(&self, id: &str) -> String;          // state.json's state as_str(); "" when no such job
    pub fn record(&self, id: &str) -> JobRecord;         // state.json
    pub fn transitions(&self, id: &str) -> Vec<Transition>;
    pub fn notifications(&self) -> Vec<Notification>;
    pub fn engine_log(&self) -> String;
    pub fn drives_json(&self) -> serde_json::Value;
    pub fn steps(&self) -> &ScriptedSteps;
    pub fn wait(&self, what: &str, done: impl Fn(&Harness) -> bool); // polls every 20 ms; panics with `what` after 10 s
    pub fn settle(&self);   // returns once jobs, notifications, engine.log, ejects and calls are unchanged for 300 ms (10 s cap)
    pub fn data(&self) -> &Path; pub fn staging(&self) -> &Path; pub fn library(&self) -> &Path; pub fn job_dir(&self, id: &str) -> PathBuf;
    pub fn ejector(&self) -> &RecordingEjector; pub fn awake(&self) -> &CountingKeepAwake; pub fn shares(&self) -> &FakeShares;
    pub fn host(&self) -> &FakeHost; pub fn http(&self) -> &RecordedTransport; pub fn timers(&self) -> &ManualTimers;
    pub fn fire_timers(&self);
}
```

Ruling R22.1: the harness's helpers above are the sheet's list; every method takes `&self`, so tests write `let h`. `insert`, `insert_root` and `remove` only queue their drive event in the harness, and every other Harness method first hands the queued events to the `DriveScript`, in order. So `h.insert(..); h.run_until(state)` arms the stop hook before the arrival reaches the engine, and no transition into `state` can pass unseen.

Ruling R22.2: `StepSeen` and `ScriptedSteps::seen` are added. The persistence-order test needs to see the files at the moment of each call.

- [ ] **Step 1: Write the failing tests in `crates/engine/src/owner/tests.rs`**

```rust
//! The job-owner loop over ScriptedSteps (design parts 4.4, 5.4, 7.3; K8, L1a, L2).

use super::*;
use crate::testkit::{Harness, StepScript};
use crate::StepResult;
use lazymkv_core::job::{Action, Event, Failure, FailureClass, NotifyEvent, State};
use lazymkv_core::plan::Mode;
use serde_json::json;

fn ok(event: Event) -> StepScript {
    StepScript::Result(StepResult { event: Some(event), message: None })
}

fn failure(class: FailureClass) -> Failure {
    Failure { class, output: None }
}

/// One title, from RunScan to Ejected.
fn disc_phase() -> Vec<(&'static str, StepScript)> {
    vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", ok(Event::SpaceOk)),
        ("RipTitle", ok(Event::TitleRipped)),
        ("VerifyFile", ok(Event::TitleVerified { last: true })),
        ("Eject", ok(Event::Ejected)),
    ]
}

/// Ends a script that reaches Ejected: from Task 24 on, the file-phase slot runs Probe after Ejected, and this entry
/// fails the job there so that it releases the slot. Before Task 24 it is never used.
fn probe_fails() -> (&'static str, StepScript) {
    ("Probe", ok(Event::ProbeFailed { failure: failure(FailureClass::Unknown) }))
}

/// Whether a transition into `state` was persisted. The tests wait on this rather than on the current state, because
/// from Task 24 on a job moves past Ejected at once.
fn reached(h: &Harness, id: &str, state: &str) -> bool {
    h.transitions(id).iter().any(|line| line.new.as_str() == state)
}

fn id_of(h: &Harness, disc: &str) -> String {
    format!("{}-1", h.fingerprint(disc).to_ascii_lowercase())
}

fn arrival(drive: &str, fingerprint: &str) -> Arrival {
    let meta = DiscMeta {
        volume_label: "X".into(), format: Some("bluray".into()), fingerprint: Some(fingerprint.into()),
        disc_id: None, file_count: 1, total_bytes: 1, captured_unix: 1,
    };
    Arrival { drive: drive.into(), root: PathBuf::from("/x"), number: 1, meta, label: "X".into() }
}

#[test]
fn a_new_disc_persists_detected_then_fingerprinted() {
    let h = Harness::new_scripted(vec![("RunScan", StepScript::Hang)], json!({}));
    h.insert("tok0", "disc-a");
    let id = id_of(&h, "disc-a");
    h.wait("RunScan called", |h| h.steps().calls() == ["RunScan"]);
    let lines = h.transitions(&id);
    assert_eq!(lines.len(), 2);
    assert_eq!((lines[0].event.clone(), lines[0].old.clone()), (None, None));
    assert_eq!((lines[0].new.clone(), lines[0].actions.clone()), (State::Detected, vec![Action::Fingerprint]));
    assert_eq!(lines[1].event, Some(Event::FingerprintDone));
    assert_eq!(lines[1].old, Some(State::Detected));
    assert_eq!((lines[1].new.clone(), lines[1].actions.clone()), (State::Fingerprinted, vec![Action::RunScan]));
    let record = h.record(&id);
    assert_eq!((record.format.as_str(), record.drive.as_str(), record.profile.as_str()), ("lazymkv job v1", "tok0", "job.mmcp.xml"));
    assert_eq!(record.fingerprint, h.fingerprint("disc-a"));
    for file in ["meta.json", "files.json", "nav/BDMV/index.bdmv", "settings.json", "state.json", "transitions.jsonl"] {
        assert!(h.job_dir(&id).join(file).is_file(), "{file}");
    }
}

#[test]
fn already_ripped_sends_already_ripped_and_ejects_outside_the_reducer() {
    let h = Harness::new_scripted(disc_phase(), json!({}));
    h.insert("tok0", "disc-a");
    let id = id_of(&h, "disc-a");
    h.wait("Ejected", |h| reached(h, &id, "Ejected"));
    h.remove("tok0");
    h.settle();
    let lines = h.transitions(&id).len();
    h.insert("tok0", "disc-a");
    h.wait("already_ripped", |h| h.notifications().iter().any(|n| n.event == NotifyEvent::AlreadyRipped));
    h.settle();
    let sent = h.notifications();
    let events: Vec<NotifyEvent> = sent.iter().map(|n| n.event.clone()).collect();
    assert_eq!(events, vec![NotifyEvent::DiscDone, NotifyEvent::AlreadyRipped]);
    let done = &sent[0];
    assert_eq!((done.job_id.as_deref(), done.drive.as_deref()), (Some(id.as_str()), Some("tok0")));
    assert_eq!(done.fingerprint.as_deref(), Some(h.fingerprint("disc-a").as_str()));
    assert_eq!(done.message, "the disc phase is done and the disc was ejected");
    assert_eq!(sent[1].message, format!("the disc was already ripped as job {id} and was ejected"));
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
    assert_eq!(h.transitions(&id).len(), lines);
    assert_eq!(h.job_ids(), vec![id]);
    assert_eq!(h.drives_json()["drives"]["tok0"], json!({"fingerprint": null, "handled": false}));
}

#[test]
fn a_second_drives_disc_queues_and_starts_at_the_first_jobs_ejected() {
    let mut token = DiscToken::default();
    assert!(token.take("a-1"));
    assert!(!token.take("b-1"));
    token.wait(Waiting::Arrival(arrival("tok1", "B")));
    assert!(token.after("a-1", &State::Ripping, &[Action::VerifyFile]).is_none());
    assert!(token.after("a-1", &State::RipVerified { after_copy: false }, &[Action::Eject]).is_none());
    let next = token.after("a-1", &State::Ejected { after_copy: false }, &[Action::Notify { event: NotifyEvent::DiscDone }]);
    match next {
        Some(Waiting::Arrival(found)) => assert_eq!(found.drive, "tok1"),
        None => panic!("the queued arrival was not taken at Ejected"),
    }
    assert_eq!(token.holder(), None);
    assert!(!keeps_token(&State::Ripping, &[Action::WaitForDisc]));
    assert!(!keeps_token(&State::AwaitingSpace { return_to: Box::new(State::Planned { mode: Mode::Direct }) }, &[]));
}

#[test]
fn a_queued_disc_that_is_removed_leaves_the_queue() {
    let h = Harness::new_scripted(vec![("RunScan", StepScript::Hang)], json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("disc-a scanning", |h| h.steps().calls() == ["RunScan"]);
    h.insert("tok1", "disc-b");
    let b = h.fingerprint("disc-b");
    h.wait("disc-b queued", |h| h.drives_json()["drives"]["tok1"]["fingerprint"] == json!(b));
    assert_eq!(h.drives_json()["drives"]["tok1"]["handled"], json!(false));
    h.remove("tok1");
    h.wait("tok1 empty", |h| h.drives_json()["drives"]["tok1"]["fingerprint"] == json!(null));
    h.remove("tok0");
    h.settle();
    assert_eq!(h.job_ids(), vec![a]);
    assert_eq!(h.steps().calls(), ["RunScan"]);
}

#[test]
fn a_disc_pulled_mid_rip_waits_for_its_disc_and_frees_the_token() {
    let mut script = disc_phase();
    script[3] = ("RipTitle", StepScript::Hang);
    script.push(("RunScan", StepScript::Hang));
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    h.remove("tok0");
    h.wait("MediaRemoved", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    let last = h.transitions(&a).pop().unwrap();
    assert_eq!((last.new, last.actions), (State::Ripping, vec![Action::WaitForDisc]));
    h.insert("tok0", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 5);
    assert_eq!(h.steps().calls(), ["RunScan", "Plan", "CheckSpace", "RipTitle", "RunScan"]);
    assert_eq!(h.job_ids().len(), 2);
}

#[test]
fn reinserting_routes_media_arrived_and_rips_again() {
    let mut script = disc_phase();
    script.insert(3, ("RipTitle", StepScript::Hang));
    script.push(probe_fails());
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    h.remove("tok0");
    h.wait("waiting for the disc", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    h.insert("tok0", "disc-a");
    h.wait("Ejected", |h| reached(h, &a, "Ejected"));
    let arrived = h.transitions(&a).into_iter().find(|line| line.event == Some(Event::MediaArrived)).unwrap();
    assert_eq!((arrived.new, arrived.actions), (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle]));
    assert_eq!(
        h.steps().calls()[..8],
        ["RunScan", "Plan", "CheckSpace", "RipTitle", "EmptyTitleDirectory", "RipTitle", "VerifyFile", "Eject"]
    );
    assert_eq!(h.job_ids(), vec![a]);
}

#[test]
fn a_waiting_jobs_disc_is_only_noted_and_takes_no_token() {
    let script = vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", ok(Event::SpaceShort)),
        ("RunScan", StepScript::Hang),
    ];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("AwaitingSpace", |h| h.state_of(&a) == "AwaitingSpace");
    h.remove("tok0");
    h.settle();
    h.insert("tok0", "disc-a");
    h.settle();
    let lines = h.transitions(&a);
    assert!(lines.iter().all(|line| line.event != Some(Event::MediaRemoved) && line.event != Some(Event::MediaArrived)));
    assert_eq!(h.drives_json()["drives"]["tok0"]["handled"], json!(true));
    h.insert("tok1", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 4);
    assert_eq!(h.transitions(&a).len(), lines.len());
}

#[test]
fn a_media_read_error_jobs_reinserted_disc_gets_media_arrived() {
    let script = vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", ok(Event::SpaceOk)),
        ("RipTitle", ok(Event::TitleRipFailed { failure: failure(FailureClass::MediaReadError) })),
        ("RipTitle", StepScript::Hang),
    ];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Failed", |h| h.state_of(&a) == "Failed");
    assert_eq!(h.record(&a).state, State::Failed { class: FailureClass::MediaReadError, from: Box::new(State::Ripping) });
    h.remove("tok0");
    h.insert("tok1", "disc-a");
    h.wait("MediaArrived", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaArrived));
    let last = h.transitions(&a).pop().unwrap();
    assert_eq!(last.new, State::Ripping);
    assert_eq!(
        last.actions,
        vec![Action::EmptyTitleDirectory, Action::RipTitle, Action::Notify { event: NotifyEvent::Resumed }]
    );
    assert_eq!(h.record(&a).drive, "tok1");
}

#[test]
fn drives_json_records_fingerprint_and_handled_at_arrival_removal_and_queue_change() {
    let h = Harness::new_scripted(vec![("RunScan", StepScript::Hang), ("RunScan", StepScript::Hang)], json!({}));
    let (fa, fb) = (h.fingerprint("disc-a"), h.fingerprint("disc-b"));
    h.insert("tok0", "disc-a");
    h.wait("a routed", |h| h.drives_json()["drives"]["tok0"] == json!({"fingerprint": fa, "handled": true}));
    h.insert("tok1", "disc-b");
    h.wait("b queued", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": fb, "handled": false}));
    h.remove("tok0");
    h.wait("b taken", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": fb, "handled": true}));
    let drives = h.drives_json();
    assert_eq!(drives["format"], json!("lazymkv drives v1"));
    assert_eq!(drives["drives"]["tok0"], json!({"fingerprint": null, "handled": false}));
    assert_eq!(drives["drive_incapable"], json!([]));
    h.remove("tok1");
    h.wait("tok1 empty", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": null, "handled": false}));
}

#[test]
fn an_unreadable_volume_sends_failed_with_a_null_job_id() {
    let h = Harness::new_scripted(vec![], json!({}));
    let root = h.data().parent().unwrap().join("volumes/blank");
    std::fs::create_dir_all(root.join("BDMV")).unwrap();
    std::fs::write(root.join("BDMV/index.bdmv"), b"INDX0200").unwrap();
    h.insert_root("tok0", &root);
    h.wait("failed", |h| !h.notifications().is_empty());
    h.settle();
    let sent = h.notifications();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].event, NotifyEvent::Failed { class: FailureClass::Unknown });
    assert_eq!((sent[0].job_id.clone(), sent[0].fingerprint.clone()), (None, None));
    assert_eq!(sent[0].drive.as_deref(), Some("tok0"));
    assert!(h.engine_log().contains(
        "tok0: the disc's fingerprint could not be computed: the volume holds no BDMV/STREAM or VIDEO_TS files"));
    assert_eq!(std::fs::read_dir(h.data().join("jobs")).map(|d| d.count()).unwrap_or(0), 0);
    assert!(h.ejector().calls().is_empty(), "the disc stays in (design part 5.4 step 2)");
}

/// Review Focus 5. A regular file stands for the volume root, so the walk fails as it does when the disc is pulled
/// mid-walk; the removal that follows finds no arrival left to cancel, and nothing else is sent.
#[test]
fn a_disc_removed_during_its_fingerprint_walk_sends_one_failed_with_a_null_job_id_and_makes_no_job() {
    let h = Harness::new_scripted(vec![], json!({}));
    let pulled = h.data().parent().unwrap().join("volumes/pulled");
    std::fs::create_dir_all(pulled.parent().unwrap()).unwrap();
    std::fs::write(&pulled, b"the volume is gone").unwrap();
    h.insert_root("tok0", &pulled);
    h.remove("tok0");
    h.wait("failed", |h| !h.notifications().is_empty());
    h.settle();
    let sent = h.notifications();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].event.clone(), sent[0].job_id.clone()), (NotifyEvent::Failed { class: FailureClass::Unknown }, None));
    assert_eq!(h.engine_log().matches("tok0: the disc's fingerprint could not be computed: ").count(), 1);
    assert_eq!(std::fs::read_dir(h.data().join("jobs")).map(|d| d.count()).unwrap_or(0), 0);
    assert_eq!(h.drives_json()["drives"]["tok0"], json!({"fingerprint": null, "handled": false}));
    assert!(h.ejector().calls().is_empty(), "the disc stays in (design part 5.4 step 2)");
}

#[test]
fn a_volume_that_never_appears_fails_after_drive_ready_timeout_s() {
    let h = Harness::new_scripted(vec![], json!({}));
    let missing = h.data().parent().unwrap().join("volumes/never-mounted");
    h.insert_root("tok0", &missing);
    h.wait("failed", |h| !h.notifications().is_empty());
    h.settle();
    let sent = h.notifications();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].event.clone(), sent[0].job_id.clone()), (NotifyEvent::Failed { class: FailureClass::Unknown }, None));
    assert_eq!(h.engine_log().matches("tok0: the volume did not appear within drive_ready_timeout_s (1 s)").count(), 1);
    assert_eq!(std::fs::read_dir(h.data().join("jobs")).map(|d| d.count()).unwrap_or(0), 0);
    assert!(h.ejector().calls().is_empty());
}

#[test]
fn no_job_starts_while_settings_have_a_problem_or_staging_root_is_unset() {
    let h = Harness::new_scripted(vec![], json!({"staging_root": null}));
    h.insert("tok0", "disc-a");
    h.wait("refused", |h| h.engine_log().contains("tok0: no job starts: staging_root is unset"));
    assert!(h.job_ids().is_empty());
    let h = Harness::new_scripted(vec![], json!({"keep_tuning_evidence": true, "tuning_root": null}));
    h.insert("tok0", "disc-a");
    h.wait("refused", |h| h.engine_log().contains("tok0: no job starts: the settings have a problem: tuning_root"));
    assert!(h.job_ids().is_empty());
    assert_eq!(h.drives_json()["drives"]["tok0"]["handled"], json!(false));
}

#[test]
fn keep_awake_is_held_while_any_job_is_active() {
    let mut script = disc_phase();
    script.insert(3, ("RipTitle", StepScript::Hang));
    script.push(probe_fails());
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    assert_eq!(h.awake().holders(), 1);
    h.remove("tok0");
    h.wait("waiting for the disc", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    h.settle();
    assert_eq!(h.awake().holders(), 0);
    h.insert("tok0", "disc-a");
    h.wait("Ejected", |h| reached(h, &a, "Ejected"));
    h.settle();
    assert_eq!(h.awake().holders(), 0);
}

#[test]
fn each_transition_is_persisted_before_its_actions_run() {
    let mut script = disc_phase();
    script.push(probe_fails());
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Ejected", |h| reached(h, &a, "Ejected"));
    h.settle();
    let lines = h.transitions(&a);
    let seen = h.steps().seen();
    let first: Vec<&str> = seen.iter().take(6).map(|call| call.action.as_str()).collect();
    assert_eq!(first, ["RunScan", "Plan", "CheckSpace", "RipTitle", "VerifyFile", "Eject"]);
    for call in seen {
        let line = &lines[call.lines - 1];
        assert_eq!(call.state, line.new.as_str(), "{}", call.action);
        let tags: Vec<String> = line.actions.iter().map(|a| serde_json::to_value(a).unwrap()["action"].as_str().unwrap().to_string()).collect();
        assert!(tags.contains(&call.action), "{} not in the persisted line's actions {tags:?}", call.action);
    }
}

#[test]
fn posted_edits_and_events_are_taken_once_in_order() {
    let h = Harness::new_scripted(disc_phase(), json!({}));
    h.insert("tok0", "disc-a");
    h.run_until("Planned");
    let mut record = h.record(&id_of(&h, "disc-a"));
    let outbox = Outbox::default();
    outbox.edit("a-1", |record| record.message = Some("one".into()));
    outbox.edit("a-1", |record| record.message = Some(format!("{} then two", record.message.take().unwrap_or_default())));
    let java = Event::ToolMissing { tool: lazymkv_core::job::Tool::Java };
    outbox.feed_first("a-1", java.clone());
    outbox.feed_first("a-1", Event::ShareLost);
    let posted = outbox.take("a-1");
    assert_eq!(posted.first, vec![java, Event::ShareLost]);
    for edit in posted.edits {
        edit(&mut record);
    }
    assert_eq!(record.message.as_deref(), Some("one then two"));
    let again = outbox.take("a-1");
    assert!(again.edits.is_empty() && again.first.is_empty());
}

#[test]
fn run_until_stops_before_the_states_actions_run() {
    let h = Harness::new_scripted(disc_phase(), json!({}));
    h.insert("tok0", "disc-a");
    h.run_until("Planned");
    let a = id_of(&h, "disc-a");
    let last = h.transitions(&a).pop().unwrap();
    assert_eq!((last.new, last.actions), (State::Planned { mode: Mode::Direct }, vec![Action::CheckSpace]));
    assert_eq!(h.record(&a).state, State::Planned { mode: Mode::Direct });
    assert_eq!(h.steps().calls(), ["RunScan", "Plan"]);
    h.settle();
    assert_eq!(h.steps().calls(), ["RunScan", "Plan"], "the stopped engine ran CheckSpace");
}

#[test]
fn restart_keeps_the_data_folder_and_the_drive_script() {
    let mut script = disc_phase();
    script[3] = ("RipTitle", StepScript::Hang);
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    h.run_until("Planned");
    let a = id_of(&h, "disc-a");
    let before = h.transitions(&a);
    h.restart();
    h.settle();
    assert_eq!(h.job_ids(), vec![a.clone()]);
    assert_eq!(h.transitions(&a)[..before.len()], before[..]);
    assert_eq!(h.drives_json()["drives"]["tok0"]["fingerprint"], json!(h.fingerprint("disc-a")));
    assert_eq!(h.steps().calls()[..2], ["RunScan", "Plan"]);
}

#[test]
fn the_engine_logs_its_start_and_stop() {
    let h = Harness::new_scripted(vec![], json!({}));
    let started = format!("engine started (lazymkv {})", env!("CARGO_PKG_VERSION"));
    h.wait("started", |h| h.engine_log().contains(&started));
    h.restart();
    h.wait("started again", |h| h.engine_log().matches(&started).count() == 2);
    assert_eq!(h.engine_log().matches("engine stopped").count(), 1);
}
```

The two harness tests hold before and after Task 25: until Task 25 the restarted engine resumes nothing, and from Task 25 on it resumes Planned and hangs in the scripted rip, which leaves the earlier lines, the drive record and the first calls as they were.

In `crates/engine/src/arrival.rs` `mod tests` (uses `lazymkv_core::testsupport::{Scratch, ScratchExt}`):

```rust
#[test]
fn a_bdmt_name_wins_over_the_volume_label_for_the_search() {
    let job = Scratch::new("bdmt");
    job.write("meta.json", r#"{"volume_label":"SYNTHETIC_MOVIE","format":"bluray","fingerprint":"ABC","disc_id":"DEF","file_count":5,"total_bytes":1,"captured_unix":7}"#);
    job.write("nav/BDMV/META/DL/bdmt_fra.xml", "<di:name>Film Synthétique</di:name>");
    job.write("nav/BDMV/META/DL/bdmt_eng.xml", "<di:name>Synthetic Movie</di:name>");
    let inputs = chain_inputs(&job.0).unwrap();
    assert_eq!((inputs.fingerprint.as_str(), inputs.disc_id.as_deref()), ("ABC", Some("DEF")));
    let name = inputs.name.unwrap();
    assert_eq!((name.raw.as_str(), name.source), ("Synthetic Movie", NameSource::Bdmt));
}

#[test]
fn without_a_bdmt_file_the_volume_label_names_the_disc() {
    let job = Scratch::new("label");
    job.write("meta.json", r#"{"volume_label":"SYNTHETIC_MOVIE","format":"bluray","fingerprint":"ABC","disc_id":null,"file_count":5,"total_bytes":1,"captured_unix":7}"#);
    let inputs = chain_inputs(&job.0).unwrap();
    assert_eq!(inputs.disc_id, None);
    assert_eq!(inputs.name.unwrap().source, NameSource::VolumeLabel);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit`
Expected: compile errors naming `testkit::Harness`, `DiscToken`, `Outbox` and `chain_inputs`, which are not defined yet.

- [ ] **Step 3: Implement**

Files and pinned behaviour:

1. **`owner.rs`: the loop.**
   - One `mpsc` channel carries a private `enum Msg`: `Drive(DriveEvent)`, `Arrival(Result<Arrival, (String, String)>)` (failure: drive and sentence), `StepDone { job: String, generation: u64, result: StepResult }`, `Progress { job: String, title: Option<String>, fraction: f64 }`, `Timer { job: String }`, `Ejected { drive: String }`, `Stop`.
   - The drive thread loops `drives.next(Duration::from_millis(50))` and sends `Msg::Drive` until stopped.
   - At start the owner loads `Store::load_jobs()`. A refused job is logged and left alone. It also reads `Store::drives()`: on Err it logs the error and keeps the drives in memory only, never writing them.
   - Restarts are Task 25's.
2. **Each transition.**
   - Order of work:
     1. `reduce`.
     2. The owner's record actions are applied to the new record: `RecordNote` adds `Noted { note, stream: None, detail: note_detail(note) }`, sorted and unique by note (S41); `ArmRipAgain` sets `rip_again`; `RefreshIdentity` writes a `job.log` line `RefreshIdentity arrives in Task 24` (Task 24 replaces it). The private `fn note_detail(note: Note) -> String` gives `no Java runtime was found, so label rule 8.3.2 does not apply (spec 10.2)` for `Note::LabelRuleUnavailable`, `the whole-disc copy failed twice, so the job switched to direct mode (spec 6.3)` for `Note::DirectModeFallback`, and `note.as_str()` for any other note.
     3. `updated_unix = host.now_unix()`.
     4. `message` is set to the step's message when the new state is `Failed` or a waiting state, else cleared.
     5. `Store::persist(record, &Transition { time_unix, event, old, new, actions })`.
     6. Then `Notify` (through `notify::message`), `StartTimer` (`timers.start`, whose closure sends `Msg::Timer`) and `WaitForDisc` (the job's disc is marked absent). The remaining actions go, in list order, to one worker that runs them one after another and returns the last result whose `event` is Some.
   - `Rejected` writes a `job.log` line `rejected <event> in <state>` and changes nothing.
   - Ruling R22.3: the record actions land in the transition's own `state.json` write. They are pure edits, so the state is still persisted before any action runs (design part 7.3).
3. **Step results.**
   - The owner hands an action with the job's current `generation` and a fresh `CancelToken`.
   - A `StepDone` whose generation is stale is dropped together with its `Outbox` entry (R-A7).
   - Otherwise the owner applies `Outbox::take`'s edits, feeds each `first` event, then feeds `result.event`. A None event writes the `job.log` line `<tag> returned no event` unless the tag is `EmptyTitleDirectory`, `DeletePartialFile` or `ExtractSubtitleEvidence`.
4. **Arrivals.**
   - `DriveEvent::Arrived` bumps the drive's arrival number and calls `spawn_arrival` with `drive_settle_s` and `drive_ready_timeout_s` from `Store::settings()`.
   - A failure is never dropped. It writes the engine.log line `<drive>: <sentence>` and notifies `Failed { Unknown }` with `job_id`, `fingerprint` and `title` None, `drive` Some, and `message` the sentence.
   - A success whose number is no longer the drive's current one, or whose drive is empty, is dropped. It writes the engine.log line `<drive>: the disc was removed before its arrival was handled`.
   - On success `drives.json` gets `{ fingerprint, handled: false }`, then the gate:
     - `staging_root` None logs `<drive>: no job starts: staging_root is unset`.
     - Otherwise the first `validate()` problem logs `<drive>: no job starts: the settings have a problem: <key>: <problem>`.
   - Then routing by `job_lookup` over `(id, state, rip_again)` of the jobs with this fingerprint:
     - `AlreadyRipped(id)`: notify `already_ripped` with that job's fields, mark the drive as ejecting, eject on a worker that sends `Msg::Ejected`, and set `handled: true`. An eject error is only logged.
     - `Resume(id)` of a waiting job: note the disc present and set `handled: true`.
     - `Resume(id)` of a disc-phase job or of `Failed { MediaReadError }`, and `NewJob`: when `token.take` succeeds, feed `MediaArrived` (record.drive becomes this drive), or create the job, and set `handled: true`. Else `token.wait(Waiting::Arrival(..))`.
5. **A new job.**
   - Built from: `job_id(fp, all ids)`, `Store::create_job(id, &settings)`, and a record with `format: JOB_FORMAT`, `disc_id` from the meta, `profile: JOB_PROFILE_FILE`, `created_unix = updated_unix = now` and every list empty.
   - The first transition is `{ event: None, old: None, new: Detected, actions: [Fingerprint] }` (S29). A new job's `Fingerprint` is done by `spawn_recording` and never handed to the runner (design part 5.4 step 3: it "is already done"). Every other `Action::Fingerprint`, which comes only from `EngineRestart` in `Detected` (`resume`, job.rs:732), goes to the `StepRunner` like any other action.
   - `spawn_recording` sends `FingerprintDone`, or `FingerprintFailed { Failure { class: Unknown, output: None } }` with the message `the disc's recording could not be written: <error>`.
6. **Removals.**
   - Remove the drive from the queue, and write `{ fingerprint: null, handled: false }`.
   - A drive marked as ejecting is not fed. Its mark clears when the Eject step returns or `Msg::Ejected` arrives.
   - Else, when the drive holds the disc of the token holder in a disc-phase state: cancel its running step, bump its generation and feed `MediaRemoved`.
   - Any other job whose disc was in that drive is only marked absent.
   - The owner marks a drive as ejecting when it hands `Eject` to the runner.
7. **Token and KeepAwake.**
   - After every transition, `token.after(..)` releases the token and returns the next waiting arrival, which is routed as in item 4.
   - KeepAwake is acquired when "a step runs or the token is held" becomes true, and released when it becomes false. Ruling R22.4: this is "active"; Task 24 adds "or a slot is held".
8. **`start`.** It returns `Err(io::Error::new(io::ErrorKind::Unsupported, "lazymkv-engine: the real step runner is not built yet"))` when `services.steps` is None; Task 23 replaces this.
   - `start` writes the engine.log line `engine started (lazymkv <CARGO_PKG_VERSION>)` before it returns, and `EngineHandle::stop` writes `engine stopped` after the threads have joined (design part 3.5: engine.log holds starts and stops).
   - `start_until` passes `stop_after` to the owner. After each `Store::persist`, the owner calls it with the transition; on true it runs none of the transition's actions and leaves its loop as `Msg::Stop` does.
9. **`notify::message` texts.**
   - `DiscDone`: "the disc phase is done and the disc was ejected".
   - `AlreadyRipped`: "the disc was already ripped as job <id> and was ejected".
   - `Filed`: "filed at <first title's filed path>", or "filed".
   - `NeedsReview`: "held for review: <reasons, snake_case, joined by \", \">".
   - `Waiting`: "waiting: <reason>", plus ": <record.message>" when Some.
   - `Resumed`: "resumed".
   - `Failed`: record.message, else "failed with class <class>".
   - `EjectFailed`: "the eject failed; remove the disc by hand".
   - `title` is `plan.identity.title`, and `time` is `host.now_unix()` (S42).
10. **`chain_inputs`.** It reads meta.json as `serde_json::Value`. A missing `fingerprint` is an Err naming meta.json. The name is `disc_name(bdmt_name(first candidate's text).as_deref(), Some(volume_label))`.
11. **testkit.**
    - Scratch layout: `<scratch>/{data, staging, library/Movies, tuning, volumes}`.
    - Settings: `data/settings.json` is the harness defaults `{"staging_root", "movies_root", "tuning_root"` (the scratch folders), `"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}`, with each given key replacing the default. For `tool_paths`, each key inside it replaces the default (Ruling R22.5: one level deep, so a test can unset one tool).
    - `FakeHost::new("2026-10-23", 1_792_800_000)`.
    - `RecordingEjector::new().with_removals(script)` (Ruling R22.6: a real drive reports the removal its eject causes).
    - `StdProcessGuard`, `FakeShares::new()`, `MemorySecrets::new().with("tmdb_token", "test-token")`, `MemoryIpc::new()`, `RecordedTransport::new()`, `ManualTimers`, `MemorySink`.
    - `EngineConfig { data, resources: None, limits_override: Some(Limits { header: 10 s, scan: 10 s, exit_wait: 10 s, after_kill: 50 ms }), path_var: Some(OsString::new()) }`.
    - Synthetic volume for `<disc>`: `BDMV/index.bdmv` = "INDX0200", `BDMV/PLAYLIST/00800.mpls` = "MPLS0200", `BDMV/STREAM/00001.m2ts` of `1000 + sum of the name's bytes` zero bytes, and `AACS/Unit_Key_RO.inf` = the name's bytes. The label is the name upper-cased.
    - ScriptedSteps rules:
      - Each call records its tag and, by reading `<job_dir>/state.json` and `transitions.jsonl`, a `StepSeen`.
      - The first unused entry of that tag is taken.
      - With no entry, `EmptyTitleDirectory`, `DeletePartialFile` and `ExtractSubtitleEvidence` return `StepResult { event: None, message: None }`, and any other tag returns `message: Some("no scripted result for <tag>")`.
    - The harness holds its engine in a `Mutex<Option<EngineHandle>>` and always starts it with `start_until` and a hook over a shared `Mutex<(Option<String>, Vec<String>)>`: the hook pushes each transition's `new.as_str()` to the log, and returns true, clearing the target, for the first transition whose state equals the target. `restart` and `run_until` clear the log. Dropping the harness stops a running engine.
    - The engine's `Platform.drives` is a private adapter over the harness's `ScriptedDrives` (behind a `Mutex`, `present` answered by the `DriveScript`), so `restart` hands the same drive queue to the new engine. The new engine's sink is a `MemorySink` that pushes to the same list.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit`
Expected: PASS, with the 21 new tests (19 in `owner::tests`, 2 in `arrival::tests`) and Task 21's 9 store tests.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked`
Expected: PASS, the same tests with no feature flag. This shows that the unit tests and the testkit compile under the engine's own `cfg(test)` (Task 1), which Task 30's single workspace count relies on.

- [ ] **Step 5: Commit**

```
git add crates/engine/src
git commit -m "engine: the job-owner loop, arrivals, the disc-phase token and its queue (design parts 4.4, 5.4; K8, L2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>` and no attribution line.

---

### Task 23: Disc-phase steps on stand-ins, the scenarios and the RP21 guard (design parts 5.2, 5.4 to 5.6, 5.8)

**Files:**
- Create:
  - `crates/engine/src/steps.rs` (4 tests in `mod tests`) and `crates/engine/src/space.rs` (3 tests)
  - `crates/fake-tools/tests/engine_disc_phase.rs` (31 tests: the 27 general tests of Step 2, the 3 tests of RP-1, RP-2 and RP-3, and the scan retry test of RP-5)
  - `corpus/scenarios/movie-direct/` and the variant folders listed below
- Modify:
  - `crates/engine/src/owner.rs`: the real runner when `steps` is None; promise release
  - `crates/engine/src/testkit.rs`: `Harness::new`, `TestGuard`, `tools`, `scan_waits_for_lookups`
  - `crates/engine/src/lib.rs`: `mod space; mod steps;`, declared in Step 2 with the new files
  - `crates/dev/tests/clone_values.rs:23-24` (`PUBLIC_FOLDERS`) and `:139` (`sources.retain`): the guard over the scenarios (1 new test), and `.json` files under `crates/` checked beside the `.rs` files

**Interfaces:**

Consumes:
- Task 22's `StepRunner`, `StepResult`, `JobContext`, `Outbox`, `chain_inputs` and `Store`.
- Task 11 to 14's `find_tool`, `ToolCtx`, `Limits::from_settings`, `CancelToken`, `MakemkvFlags`, `list_drives`, `DriveListing`, `scan`, `ScanOutcome`, `rip`, `RipOutcome`, `RipRun`, `StagingHealth`, `ident`, `save_debug_log` and `exe_name`.
- Task 16's `run_round`, `LookupBook`, `SavedFile`.
- Task 4's `ustar_entry`, `DEFAULT_PROFILE_NAME`, `FALLBACK_SELECTION`, `JOB_PROFILE_FILE`, `FALLBACK_PROFILE_FILE`. Task 6's `check_format`, `REMEMBERED_FORMAT`, `TitleRecord`.
- `core`:
  - `profile::{make_job_profile (profile.rs:22), ALL_TRACKS_SELECTION (profile.rs:5)}`
  - `plan::{plan (plan/mod.rs:249), Plan (:171), PlanInputs (:194), staging_bytes_needed (:901), write_bytes_needed (:912), Mode (:35)}`
  - `plan::movie::LibraryFacts` (plan/movie.rs:813)
  - `robot::parse` (robot.rs:82)
  - `disc::{disc (disc.rs:360), same_scan_title (:491), Title::source (:296)}`
  - `verify::{selected_streams (verify.rs:34), verify_rip (:371), failure_reviews (:419)}`
  - `settings::version_triple` (settings.rs:219)
  - `remembered::Remembered` (remembered.rs:114)
  - `job::classify_title_failure` (job.rs:677)

Produces:

```rust
// crates/engine/src/steps.rs
/// The real StepRunner for Fingerprint, RunScan, Plan, CheckSpace in Planned, RipTitle, VerifyFile in Ripping, Eject and
/// EmptyTitleDirectory (design part 5.2); Task 24 adds the file phase. Def: T23. Use: T23 to T29.
pub(crate) struct RealSteps { /* config, platform parts, http, Arc<Store>, Outbox, Arc<Mutex<Promises>>, java_in_use */ }
impl RealSteps { pub(crate) fn new(config: &EngineConfig, platform: &Platform, http: Arc<dyn HttpTransport>, store: Arc<Store>,
    outbox: Outbox, promises: Arc<Mutex<Promises>>, java_in_use: Arc<Mutex<Option<bool>>>) -> RealSteps; }
// `java_in_use` holds the last scan's MSG:3344 verdict (design part 5.4 step 4); the owner keeps the same Arc for
// Task 26's self-test, which reads Java use from the last scan (design part 9.4).
impl StepRunner for RealSteps { /* ... */ }
/// `<makemkvcon's folder>/appdata.tar`'s default profile through make_job_profile, named `lazymkv-<job id>` (S50),
/// written as `<job_dir>/<file>`. Def: T23. Use: T23.
pub(crate) fn write_job_profile(makemkvcon: &Path, job_dir: &Path, job_id: &str, selection: &str, file: &str) -> Result<PathBuf, String>;
/// The N.N.N of a MSG:1005 version: the word after "MakeMKV v", else the whole text, through version_triple. Def: T23. Use: T23, T26.
pub(crate) fn version_of(version: &str) -> Option<(u32, u32, u32)>;
/// `<title dir>/attempt-<n>` with n one more than the highest existing attempt (1 for the first). Def: T23. Use: T23.
pub(crate) fn next_attempt_dir(title_dir: &Path) -> PathBuf;
/// One lookup round on its own thread (design part 8.2): run_round over the locked book, each SavedFile written with
/// write_atomic into the job folder; `done` gets `book.pending()` or chain_inputs' Err. Def: T23. Use: T23, T24.
pub(crate) fn spawn_round(job_dir: PathBuf, lookups: Arc<Mutex<LookupBook>>, http: Arc<dyn HttpTransport>, token: Option<String>,
    today: String, store: Arc<Store>, done: Box<dyn FnOnce(Result<(bool, bool), String>) + Send>);

// crates/engine/src/space.rs
/// Design part 5.8's two steps that promise space. Def: T23. Use: T23, T24, T25.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)] pub(crate) enum PromisedStep { Rip, Remux }
/// Space promised to unfinished steps, by (job, step), each on one volume. Def: T23. Use: T23, T24, T25.
#[derive(Default)] pub(crate) struct Promises { /* BTreeMap<(String, PromisedStep), (String, u64)> */ }
impl Promises {
    pub(crate) fn promise(&mut self, job: &str, step: PromisedStep, volume: &str, bytes: u64); // replaces any earlier one
    pub(crate) fn release(&mut self, job: &str, step: PromisedStep);
    pub(crate) fn promised(&self, volume: &str, except_job: &str) -> u64;
}
/// `shares.volume(root)`, else the root itself (S11). Def: T23. Use: T23, T24.
pub(crate) fn volume_of(shares: &dyn ShareConnector, root: &str) -> String;
/// The Direct rip's check (design part 5.8, S54); SpaceOk promises Rip on staging and Remux on the library. Def: T23. Use: T23.
pub(crate) fn rip_check(rip_bytes: u64, remux_bytes: u64, staging: &str, library: Option<&str>, shares: &dyn ShareConnector,
    promises: &mut Promises, job: &str) -> Event;
/// The Rip promise on staging: `staging_bytes_needed(plan, disc_size)` (plan/mod.rs:901). Def: T23. Use: T23, T25.
pub(crate) fn rip_promise(plan: &Plan, disc_size: u64) -> u64;
/// The rip check's Remux promise on the library: `write_bytes_needed` of the planned titles' sizes (S54). Def: T23. Use: T23.
pub(crate) fn planned_remux_promise(plan: &Plan) -> u64;
/// The remux check's Remux promise: `write_bytes_needed(staged_bytes)`, the staged files' sizes (S54). Def: T23. Use: T24, T25.
pub(crate) fn remux_promise(staged_bytes: u64) -> u64;

// testkit additions
impl Harness {
    pub fn new(fake_tool: &Path, scenarios: &[&str], settings: serde_json::Value) -> Harness;
    pub fn tools(&self) -> &Path;                      // the merged scenario folder, holding the tool copies
    pub fn scan_waits_for_lookups(&self, wait: bool);  // default true
}
```

Ruling R23.1: Plan reads the book with `try_lock`. A book held by a running round counts as an empty book, so every lookup is pending (design part 8.3: planning never waits; `run_round` holds the book for a whole round).

Ruling R23.2: stand-in runs are faster than the recorded lookups' file writes. So `Harness::new` uses a `TestGuard` around `StdProcessGuard`.
- Before spawning a scan (args holding `info` and a `--profile=` argument), it waits until `<the profile's folder>/lookup-status.txt` has one line per distinct URL of the merged `http.json` and no line ends in `000`, for at most 5 s.
- This keeps the engine's "never waits" planning unchanged, while making the whole-job tests deterministic. It relies on Task 16's `run_round` applying each outcome to the book (its step 2.3) before it saves the status file (its step 2.5).

Ruling R23.3: the stand-in's stdout files are named `ident-raw.json` and `partial-ident.json`, not `mkvmerge.json`. That name is the stand-in's own scenario file (Task 10 reads `<stem>.json`). The job folder's recorded names are unchanged.

Ruling R23.4: `rip-summary.json` is written by VerifyFile in Ripping, before it verifies, as design part 7.1's table says ("spec 6.2 step 7"). It takes the run's numbers from `<key>/rip.txt.exit.json` and `rip_index` from the title record that RipTitle edited, so a restart between the rip and the verification loses nothing.

Ruling R23.5: one variant is added, `movie-direct-verify-twice`, because the listed `movie-direct-rerip` cannot fail twice. The below-minimum-version test uses `makemkv_min_version: "2.0.1"` rather than a variant.

Ruling R23.7: `rip-summary.json` has no `started` key in PR 2. Design part 4.6 formats it from `platform`'s local time, as phase 0 did (`yyyy-MM-dd HH:mm:ss.fff`, scripts/phase0/backup-disc.ps1:63), but the PR 2 `Host` has only `local_date`, and design part 7.1 writes only the phase 0 keys the engine measures. PR 3 adds the local time with the Windows implementations and writes `started` (see Not covered).

RP-6 settles this: (a) after a `MSG:1009` on the first scan, the second scan replaces `scan.txt` and its run files, and `job.log` keeps the rejected scan's 1009 text.

RP-1 settles this: (a) a job's later drive listings are each recorded as `drive-list-<n>.txt` with its run files, n counting from 2 as `rescan-<n>.txt` is numbered, and each listing writes the `job.log` line `drive listing <n>: <drive> is index <index>`, n counting from 1.

RP-2 settles this: (a) a drive listing with no `DRV` line for the job's drive gives RunScan `ScanFailed { Unknown }` with the message `MakeMKV's drive listing has no drive <drive>`, so the job fails, and gives RipTitle's listing `TitleRipFailed { Unknown }` with the same message.

RP-3 settles this: (a) a `makemkvcon` with no readable `appdata.tar`, or one without `default.mmcp.xml`, gives `ToolMissing { Makemkvcon }` with `write_job_profile`'s Err as the message, so the job waits in AwaitingTool, as for an incomplete MakeMKV install, and a self-test that finds the tool returns it.

RP-5 settles this: (b) when `scan` fails past its limit, the engine lists the drives again and scans once more (Step 4 item 4), with the test `a_scan_past_its_limit_is_killed_listed_again_and_retried_once`.

- [ ] **Step 1: Write the scenarios**

`corpus/scenarios/movie-direct/`:
- `disc.json`:
```json
{"label":"SYNTHETIC_MOVIE","files":[
 {"path":"BDMV/index.bdmv","text":"INDX0200"},
 {"path":"BDMV/PLAYLIST/00800.mpls","text":"MPLS0200"},
 {"path":"BDMV/META/DL/bdmt_eng.xml","text":"<?xml version=\"1.0\" encoding=\"utf-8\"?><disclib><di:discinfo><di:title><di:name>Synthetic Movie</di:name></di:title></di:discinfo></disclib>"},
 {"path":"AACS/Unit_Key_RO.inf","hex":"00112233445566778899aabbccddeeff"},
 {"path":"BDMV/STREAM/00001.m2ts","size":4096}]}
```
- `makemkvcon64.json`:
```json
{"responses":[
 {"args_contain":["info","disc:9999"],"stdout":"drive-list.txt"},
 {"args_contain":["info","disc:0"],"stdout":"scan.txt","timing":"scan.txt.timing.jsonl"},
 {"args_contain":["mkv","disc:0"],"stdout":"rip.txt","creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"placeholder.mkv"}]}]}
```
- `drive-list.txt`:
```
MSG:1005,0,1,"MakeMKV v2.0.0 win(x64-release) started","%1 started","MakeMKV v2.0.0 win(x64-release)"
DRV:0,2,999,12,"BD-RE SYNTHETIC DRIVE 1.00 SERIAL","SYNTHETIC_MOVIE","tok0"
DRV:1,256,999,0,"","",""
MSG:5010,0,0,"Failed to open disc","Failed to open disc"
```
- `scan.txt`, in this order:
  1. The `MSG:1005` line above.
  2. `MSG:3307,16777216,2,"File 00800.mpls was added as title #0","File %1 was added as title #%2","00800.mpls","0"`
  3. `MSG:5011,0,0,"Operation successfully completed","Operation successfully completed"` and `TCOUNT:1`.
  4. `CINFO:1,6209,"Blu-ray disc"`, `CINFO:2,0,"Synthetic Movie"`, `CINFO:28,0,"eng"`, `CINFO:29,0,"English"`, `CINFO:30,0,"Synthetic Movie"`, `CINFO:32,0,"SYNTHETIC_MOVIE"` and `CINFO:33,0,"0"`.
  5. `TINFO:0,<id>,0,"<value>"` for:

     | Attribute | Value |
     |---|---|
     | 2 | `Synthetic Movie` |
     | 8 | `12` |
     | 9 | `1:59:10` |
     | 10 | `4.0 KB` |
     | 11 | `4096` |
     | 16 | `00800.mpls` |
     | 25 | `1` |
     | 26 | `1` |
     | 27 | `title_t00.mkv` |
     | 28 | `eng` |
     | 29 | `English` |
     | 33 | `0` |

  6. `SINFO:0,0,…` (video). Attribute 1 is `SINFO:0,0,1,6201,"Video"`. The others, written `SINFO:0,0,<id>,0,"<value>"`:

     | Attribute | Value |
     |---|---|
     | 5 | `V_MPEG4/ISO/AVC` |
     | 6 | `Mpeg4` |
     | 7 | `Mpeg4 AVC High@L4.1` |
     | 19 | `1920x1080` |
     | 20 | `16:9` |
     | 21 | `23.976 (24000/1001)` |
     | 22 | `0` |
     | 33 | `0` |

  7. `SINFO:0,1,…` (audio). Attribute 1 is `SINFO:0,1,1,6202,"Audio"`. The others, written `SINFO:0,1,<id>,0,"<value>"`:

     | Attribute | Value |
     |---|---|
     | 2 | `Surround 5.1` |
     | 3 | `eng` |
     | 4 | `English` |
     | 5 | `A_DTS` |
     | 6 | `DTS-HD MA` |
     | 7 | `DTS-HD Master Audio` |
     | 14 | `6` |
     | 17 | `48000` |
     | 22 | `0` |
     | 30 | `DTS-HD MA Surround 5.1 English` |
     | 33 | `90` |
     | 38 | `d` |
     | 39 | `Default` |
     | 40 | `5.1(side)` |

- `scan.txt.timing.jsonl`: one `{"n":<i>,"t_ms":<10*i>}` line per line of `scan.txt`.
- `rip.txt`:
```
MSG:1005,0,1,"MakeMKV v2.0.0 win(x64-release) started","%1 started","MakeMKV v2.0.0 win(x64-release)"
MSG:3307,16777216,2,"File 00800.mpls was added as title #0","File %1 was added as title #%2","00800.mpls","0"
MSG:5015,0,0,"Saving 1 titles into directory","Saving %1 titles into directory %2","1","file://title"
PRGV:0,32768,65536
PRGV:0,65536,65536
MSG:5005,0,1,"1 titles saved","%1 titles saved","1"
MSG:5036,260,1,"Copy complete. 1 titles saved.","Copy complete. %1 titles saved.","1"
```
- `mkvmerge.json`:
```json
{"responses":[
 {"args_contain":["-J","title_t00.mkv"],"stdout":"ident-raw.json"},
 {"args_contain":["--gui-mode","-o"],"stdout":"remux-mkvmerge.txt","creates":[{"arg_after":"-o","copy_from":"placeholder.mkv"}]},
 {"args_contain":["-J",".partial"],"stdout":"partial-ident.json"}]}
```
- `ident-raw.json`:
```json
{"container":{"properties":{"duration":7150000000000}},"chapters":[{"num_entries":12}],"tracks":[
 {"id":0,"type":"video","codec":"AVC/H.264/MPEG-4p10","properties":{"language":"eng"}},
 {"id":1,"type":"audio","codec":"DTS-HD Master Audio","properties":{"language":"eng","audio_channels":6}}]}
```
- `partial-ident.json`:
  - The same container and chapters as `ident-raw.json`, and the same `id`, `type` and `codec` per track.
  - Track 0: `"properties":{"default_track":true}`.
  - Track 1: `"properties":{"language":"eng","audio_channels":6,"track_name":"DTS-HD MA 5.1","default_track":true,"flag_original":true}`.
  - These are the values `verify_final` (verify.rs:489) checks against the layout `tracks::layout` (layout.rs:56) builds for this scan and Task 24's decision:
    - the video track has no name and the default flag (layout.rs:59-69); its language is never compared (verify.rs:443);
    - the audio track's name is `track_name` (tracks/audio.rs:609): the file codec's display `DTS-HD MA` (ffprobe profile `DTS-HD MA`), then `5.1` for 6 channels;
    - it is the first kept audio track, so it is default, and it is original because the TMDb record's `original_language` is `en` and the track is English (tracks/audio.rs:629-636);
    - no track is forced or hearing-impaired.
- `remux-mkvmerge.txt`: `#GUI#progress 45%` then `#GUI#progress 100%`.
- `ffprobe.json`: `{"responses":[{"args_contain":["-show_streams"],"stdout":"ffprobe-streams.json"}]}`.
- `ffprobe-streams.json`:
```json
{"streams":[{"index":0,"codec_type":"video","codec_name":"h264"},
 {"index":1,"codec_type":"audio","codec_name":"dts","profile":"DTS-HD MA","channels":6,"tags":{"language":"eng"}}]}
```
- `placeholder.mkv`: the 16 bytes `lazymkv-test-mkv`, with no newline.
- `http.json` (scenario A, S31):
```json
{"answers":[
 {"url":"https://thediscdb.com/api/dischash/{fingerprint}","status":404,"body":""},
 {"url":"https://thediscdb.com/api/discid/{disc_id}","status":404,"body":""},
 {"url":"{search}","status":200,"body_file":"tmdb-search-answer.json"},
 {"url":"https://api.themoviedb.org/3/movie/900001?append_to_response=external_ids","status":200,"body_file":"tmdb-movie-answer.json"}]}
```
- `tmdb-search-answer.json`: `{"page":1,"results":[{"media_type":"movie","id":900001,"title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03"}],"total_pages":1,"total_results":1}` (the shape `search_json` writes, testsupport/mod.rs:582).
- `tmdb-movie-answer.json`: `{"id":900001,"title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","runtime":119,"external_ids":{"imdb_id":"tt0900001"}}` (the shape `tmdb_movie_json` writes, testsupport/mod.rs:524).

Variant folders. Each holds only the files that differ.

| Folder | Files |
|---|---|
| `movie-direct-rerip` | `mkvmerge.json` with four responses: `-J title_t00.mkv` gives `ident-missing.json`, then `-J title_t00.mkv` gives `ident-raw.json`, then the base's remux and `.partial` responses. `ident-missing.json` is `ident-raw.json` with track 0 only. |
| `movie-direct-verify-twice` | `ident-raw.json` with track 0 only. |
| `movie-direct-drift` | `makemkvcon64.json` with five responses: the listing; scan → `scan.txt`; scan → `scan-drift.txt`; rip → `rip-drift.txt`; rip → `rip-index1.txt` (creates as the base). See the note below the table. |
| `movie-direct-drift-thrice` | `rip.txt` = the base's `rip-drift.txt` content. |
| `movie-direct-fallback` | `makemkvcon64.json` with responses: the listing; `["info","disc:0","job.mmcp.xml"]` → `scan-1009.txt`; `["info","disc:0","fallback.mmcp.xml"]` → `scan.txt`; `["mkv","disc:0","fallback.mmcp.xml"]` → `rip.txt` (creates as the base). `scan-1009.txt` is the `MSG:1005` line then `MSG:1009,0,1,"Failed to load profile","Failed to load profile"`. |
| `movie-direct-fallback-twice` | `makemkvcon64.json` with responses: the listing; `["info","disc:0"]` → `scan-1009.txt`. Also `scan-1009.txt` as above. |
| `movie-direct-copy-plan` | `scan.txt`: the base with a second title 1 (TINFO 9 `1:59:10`, 11 `4096`, 16 `00801.mpls`, 26 `2`, the same two SINFO streams), a second `MSG:3307` for `"00801.mpls","1"`, and `TCOUNT:2`. |
| `movie-direct-licence` | `scan.txt`: the base with `MSG:5050,0,1,"Evaluation period has expired","Evaluation period has expired"` after the `MSG:1005` line. |
| `movie-direct-rip-licence` | `rip.txt`: the `MSG:1005` line, the same `MSG:5050` line, the `MSG:3307` line, and no 5005 or 5036. |
| `movie-direct-hang` | `makemkvcon64.json`: the base with `"hang_after_line":3` on the rip response (used here and by Task 26). |
| `movie-direct-java` | `scan.txt`: the base with `MSG:3344,0,1,"Using Java runtime from /synthetic/java","Using Java runtime from %1","/synthetic/java"` after the `MSG:1005` line (design part 5.4 step 4; used here and by Task 26). |
| `movie-direct-cp1252` | `scan.txt`: the base with its `CINFO:2` line's value `Synthetic Movie Caf<E9>`, where `<E9>` is the single raw byte 0xE9 (Windows-1252 `é`), so the file is not UTF-8 (Review Focus 1). |
| `movie-direct-scan-hang` | `makemkvcon64.json`: the base with a second `["info","disc:0"]` response; the first carries `"hang_after_line":0`, the second is the base's scan (RP-5's retry test). |

`movie-direct-drift`'s files:
- `scan-drift.txt` lists title 0 as `00055.mpls` (TINFO 9 `0:05:40`, 11 `1024`, 16 `00055.mpls`, 26 `2`, the same SINFO streams) and title 1 as the base's title with every `TINFO:0`/`SINFO:0` renumbered to 1. It has `MSG:3307` lines for `"00055.mpls","0"` and `"00800.mpls","1"`, then `TCOUNT:2`.
- `rip-drift.txt` is `rip.txt`'s first three lines, with the 3307 params `"00800.mpls","1"`.
- `rip-index1.txt` is `rip.txt` with the 3307 params `"00800.mpls","1"`.

- [ ] **Step 2: Write the failing tests**

First add `mod space; mod steps;` to `crates/engine/src/lib.rs`, so the new files compile with their tests.

`crates/engine/src/space.rs` `mod tests`:

```rust
use super::*;
use lazymkv_platform::fakes::FakeShares;

#[test]
fn a_rip_check_on_a_volume_shared_with_the_library_counts_its_own_remux() {
    let shares = FakeShares::new();
    shares.same_volume("/s", "/m");
    shares.set_free("/s", 8601);
    shares.set_free("/m", 8601);
    let mut promises = Promises::default();
    assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "a-1"), Event::SpaceShort);
    shares.set_free("/s", 8602);
    shares.set_free("/m", 8602);
    assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "a-1"), Event::SpaceOk);
    assert_eq!(promises.promised(&volume_of(&shares, "/s"), "other"), 8602);
}

#[test]
fn another_jobs_promised_remux_counts_against_this_rip() {
    let shares = FakeShares::new();
    shares.same_volume("/s", "/m");
    let mut promises = Promises::default();
    promises.promise("a-1", PromisedStep::Remux, &volume_of(&shares, "/m"), 4301);
    shares.set_free("/s", 12902);
    shares.set_free("/m", 12902);
    assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "b-1"), Event::SpaceShort);
    shares.set_free("/s", 12903);
    shares.set_free("/m", 12903);
    assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "b-1"), Event::SpaceOk);
}

#[test]
fn a_promise_is_released_when_its_step_finishes() {
    let mut promises = Promises::default();
    promises.promise("a-1", PromisedStep::Rip, "v", 100);
    promises.promise("a-1", PromisedStep::Remux, "v", 50);
    assert_eq!(promises.promised("v", "b-1"), 150);
    assert_eq!(promises.promised("v", "a-1"), 0);
    promises.release("a-1", PromisedStep::Rip);
    assert_eq!(promises.promised("v", "b-1"), 50);
}
```

`crates/engine/src/steps.rs` `mod tests` (uses `lazymkv_core::testsupport::{ustar_build, default_profile_xml, Scratch, ScratchExt}`):

```rust
#[test]
fn version_of_reads_the_banner_or_a_bare_version() {
    assert_eq!(version_of("MakeMKV v2.0.0 win(x64-release)"), Some((2, 0, 0)));
    assert_eq!(version_of("1.17.0"), Some((1, 17, 0)));
    assert_eq!(version_of("MakeMKV v2.0 beta"), None);
}

#[test]
fn the_job_profile_is_written_from_appdata_tar_with_a_job_unique_name() {
    let s = Scratch::new("profile");
    s.write("tools/appdata.tar", ustar_build(&[("data/default.mmcp.xml", default_profile_xml().as_bytes())]));
    let written = write_job_profile(&s.0.join("tools/makemkvcon64"), &s.0.join("job"), "abc-1", ALL_TRACKS_SELECTION, JOB_PROFILE_FILE).unwrap();
    assert_eq!(written, s.0.join("job/job.mmcp.xml"));
    let xml = std::fs::read_to_string(written).unwrap();
    assert!(xml.contains("<name lang=\"eng\">lazymkv-abc-1</name>"));
    assert_eq!(xml.matches("defaultSelection=\"+sel:all,-sel:mvcvideo,=100:all\"").count(), 2);
}

#[test]
fn a_missing_appdata_tar_is_an_error_naming_it() {
    let s = Scratch::new("no-tar");
    let error = write_job_profile(&s.0.join("makemkvcon64"), &s.0.join("job"), "abc-1", ALL_TRACKS_SELECTION, JOB_PROFILE_FILE).unwrap_err();
    assert!(error.contains("appdata.tar"), "{error}");
}

#[test]
fn the_attempt_folder_is_numbered_after_the_last() {
    let s = Scratch::new("attempts");
    assert_eq!(next_attempt_dir(&s.0), s.0.join("attempt-1"));
    s.write("attempt-1/rip.txt", "");
    assert_eq!(next_attempt_dir(&s.0), s.0.join("attempt-2"));
}
```

`crates/fake-tools/tests/engine_disc_phase.rs`:

```rust
//! The disc phase on stand-ins, from arrival to eject (design parts 5.4 to 5.6, 5.8).

use std::fs;
use std::path::Path;

use lazymkv_core::job::{Event, FailureClass, NotifyEvent, State, Tool, WaitReason};
use lazymkv_core::lookup::IdentityStep;
use lazymkv_core::plan::Mode;
use lazymkv_core::review::{Note, ReviewReason};
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Reach;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";

fn harness(variants: &[&str], settings: Value) -> Harness {
    let mut scenarios = vec!["movie-direct"];
    scenarios.extend_from_slice(variants);
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &scenarios, settings)
}

fn run_to(h: &Harness, state: &str) -> String {
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    // A persisted transition into `state`, not the current state: from Task 24 on a job moves past Ejected at once.
    h.wait(state, |h| h.transitions(&id).iter().any(|line| line.new.as_str() == state));
    id
}

fn json_of(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn runs(h: &Harness, stem: &str, response: &str) -> u64 {
    fs::read(h.tools().join(format!("{stem}.calls.json")))
        .map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap()[response].as_u64().unwrap_or(0))
        .unwrap_or(0)
}

fn events(h: &Harness, id: &str) -> Vec<Event> {
    h.transitions(id).into_iter().filter_map(|line| line.event).collect()
}

fn root(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

#[test]
fn a_movie_disc_reaches_ejected_with_its_recording() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    h.wait("lookups answered", |h| fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).map(|t| t.lines().count() == 4).unwrap_or(false));
    for file in ["meta.json", "files.json", "nav/BDMV/index.bdmv", "nav/BDMV/PLAYLIST/00800.mpls", "nav/BDMV/META/DL/bdmt_eng.xml",
        "drive-list.txt", "scan.txt", "job.mmcp.xml", "settings.json", "thediscdb-hash.json", "thediscdb-discid.json",
        "tmdb.json", "tmdb-search.json", "tmdb-record.json", "lookup-status.txt", "t00-00800/rip.txt",
        "t00-00800/mkvmerge.json", "t00-00800/rip-summary.json"] {
        assert!(dir.join(file).is_file(), "{file}");
    }
    assert!(!dir.join("nav/AACS").exists());
    for run in ["drive-list.txt", "scan.txt", "t00-00800/rip.txt", "t00-00800/mkvmerge.json"] {
        for suffix in [".stderr", ".timing.jsonl", ".exit.json"] {
            assert!(dir.join(format!("{run}{suffix}")).is_file(), "{run}{suffix}");
        }
    }
    let summary = json_of(&dir.join(KEY).join("rip-summary.json"));
    assert_eq!(
        (&summary["ok"], &summary["result"], &summary["slug"], &summary["label"], &summary["title"], &summary["profile"], &summary["bytes"]),
        (&json!(true), &json!("ok"), &json!(id), &json!(KEY), &json!(0), &json!("job.mmcp.xml"), &json!(16))
    );
    assert!(summary.get("rip_index").is_none());
    assert_eq!(fs::read(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv")).unwrap(), b"lazymkv-test-mkv");
    let record = h.record(&id);
    assert_eq!(record.plan.unwrap().mode, Mode::Direct);
    let title = &record.titles[0];
    assert_eq!((title.key.as_str(), title.scan_title, title.attempts, title.verified), (KEY, 0, 1, true));
    assert!(title.rip_reviews.is_empty());
}

#[test]
fn job_mmcp_xml_carries_the_all_tracks_selection_and_a_job_unique_name() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let xml = fs::read_to_string(h.job_dir(&id).join("job.mmcp.xml")).unwrap();
    assert!(xml.contains(&format!("<name lang=\"eng\">lazymkv-{id}</name>")));
    assert_eq!(xml.matches("defaultSelection=\"+sel:all,-sel:mvcvideo,=100:all\"").count(), 2);
}

#[test]
fn the_drive_listing_runs_again_before_every_makemkvcon_call() {
    let h = harness(&[], json!({}));
    h.settle();
    let before = runs(&h, "makemkvcon64", "0");
    run_to(&h, "Ejected");
    assert_eq!(runs(&h, "makemkvcon64", "0"), before + 2);
    assert_eq!((runs(&h, "makemkvcon64", "1"), runs(&h, "makemkvcon64", "2")), (1, 1));
}

#[test]
fn a_1009_on_the_first_scan_switches_every_later_call_to_fallback_mmcp_xml() {
    let h = harness(&["movie-direct-fallback"], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    let xml = fs::read_to_string(dir.join("fallback.mmcp.xml")).unwrap();
    assert_eq!(xml.matches("defaultSelection=\"+sel:all,-sel:mvcvideo\"").count(), 2);
    assert!(xml.contains(&format!("<name lang=\"eng\">lazymkv-{id}</name>")));
    assert_eq!(h.record(&id).profile, "fallback.mmcp.xml");
    assert_eq!(json_of(&dir.join(KEY).join("rip-summary.json"))["profile"], json!("fallback.mmcp.xml"));
    assert_eq!((runs(&h, "makemkvcon64", "1"), runs(&h, "makemkvcon64", "2"), runs(&h, "makemkvcon64", "3")), (1, 1, 1));
    assert!(fs::read_to_string(dir.join("job.log")).unwrap().contains("Failed to load profile"));
}

#[test]
fn a_1009_on_both_scans_fails_scan_failed_unknown_quoting_makemkv() {
    let h = harness(&["movie-direct-fallback-twice"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Fingerprinted) });
    assert!(record.message.unwrap().contains("Failed to load profile"));
    assert_eq!(runs(&h, "makemkvcon64", "1"), 2);
    assert!(!h.job_dir(&id).join(KEY).exists());
}

#[test]
fn a_copy_plan_fails_plan_failed_naming_whole_disc_copy_mode_and_keeps_the_disc_in() {
    let h = harness(&["movie-direct-copy-plan"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.plan.as_ref().unwrap().mode, Mode::Copy);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Scanned) });
    assert!(record.message.unwrap().contains("whole-disc copy mode"));
    let failed = h.notifications().into_iter().last().unwrap();
    assert_eq!(failed.event, NotifyEvent::Failed { class: FailureClass::Unknown });
    assert!(failed.message.contains("whole-disc copy mode"));
    assert!(h.ejector().calls().is_empty());
    assert!(!h.staging().join(&id).exists());
}

#[test]
fn a_first_verify_failure_moves_the_attempt_aside_and_rips_again() {
    let h = harness(&["movie-direct-rerip"], json!({}));
    let id = run_to(&h, "Ejected");
    let title = h.job_dir(&id).join(KEY);
    assert!(title.join("attempt-1/rip.txt").is_file());
    assert!(title.join("attempt-1/mkvmerge.json").is_file());
    assert!(title.join("rip.txt").is_file() && !title.join("attempt-2").exists());
    let record = h.record(&id);
    assert_eq!((record.titles[0].attempts, record.titles[0].verified), (2, true));
    assert!(record.titles[0].rip_reviews.is_empty());
    assert!(events(&h, &id).contains(&Event::TitleVerifyFailed { second: false, last: true }));
}

#[test]
fn a_second_verify_failure_goes_on_and_records_verify_failed() {
    let h = harness(&["movie-direct-verify-twice"], json!({}));
    let id = run_to(&h, "Ejected");
    assert_eq!(h.record(&id).titles[0].rip_reviews, vec![ReviewReason::VerifyFailed]);
    let found: Vec<Event> = events(&h, &id).into_iter().filter(|e| matches!(e, Event::TitleVerifyFailed { .. })).collect();
    assert_eq!(found, vec![Event::TitleVerifyFailed { second: false, last: true }, Event::TitleVerifyFailed { second: true, last: true }]);
}

#[test]
fn index_drift_rescans_as_rescan_2_and_rips_the_mapped_index() {
    let h = harness(&["movie-direct-drift"], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    assert!(dir.join("rescan-2.txt").is_file() && !dir.join("rescan-3.txt").exists());
    let summary = json_of(&dir.join(KEY).join("rip-summary.json"));
    assert_eq!((&summary["title"], &summary["rip_index"]), (&json!(0), &json!(1)));
    assert_eq!(h.record(&id).titles[0].rip_index, Some(1));
}

#[test]
fn a_third_mismatch_fails_the_title_as_unknown() {
    let h = harness(&["movie-direct-drift-thrice"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Ripping) });
    assert_eq!(record.message.as_deref(), Some("the index guard found t00-00800 moved three times"));
    let dir = h.job_dir(&id);
    assert!(dir.join("rescan-2.txt").is_file() && dir.join("rescan-3.txt").is_file() && !dir.join("rescan-4.txt").exists());
}

#[test]
fn short_staging_space_waits_in_awaiting_space() {
    let h = harness(&[], json!({}));
    h.shares().set_free(&root(h.staging()), 100);
    let id = run_to(&h, "AwaitingSpace");
    assert_eq!(h.record(&id).state, State::AwaitingSpace { return_to: Box::new(State::Planned { mode: Mode::Direct }) });
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Space }));
}

#[test]
fn an_unreachable_root_at_check_space_feeds_share_lost() {
    let h = harness(&[], json!({}));
    h.shares().set_reach(&root(h.staging()), Reach::Unreachable("gone".into()));
    let id = run_to(&h, "AwaitingShare");
    assert_eq!(h.record(&id).state, State::AwaitingShare { return_to: Box::new(State::Planned { mode: Mode::Direct }) });
    assert!(events(&h, &id).contains(&Event::ShareLost));
}

#[test]
fn no_java_records_label_rule_unavailable() {
    let h = harness(&[], json!({"tool_paths": {"java": null}}));
    let id = run_to(&h, "Ejected");
    let notes = h.record(&id).notes;
    let noted = notes.iter().find(|n| n.note == Note::LabelRuleUnavailable).expect("label_rule_unavailable");
    assert_eq!((noted.stream, noted.detail.as_str()),
        (None, "no Java runtime was found, so label rule 8.3.2 does not apply (spec 10.2)"));
    assert_eq!(events(&h, &id).iter().filter(|e| **e == Event::ToolMissing { tool: Tool::Java }).count(), 1);
}

#[test]
fn the_scan_records_its_version_and_java_use_in_job_log() {
    let h = harness(&["movie-direct-java"], json!({}));
    let id = run_to(&h, "Ejected");
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("scan: MakeMKV 2.0.0; java in use: true"));
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("scan: MakeMKV 2.0.0; java in use: false"));
}

#[test]
fn removing_the_disc_mid_rip_kills_makemkvcon_and_feeds_only_media_removed() {
    let h = harness(&["movie-direct-hang"], json!({}));
    let id = run_to(&h, "Ripping");
    let rip = h.job_dir(&id).join(KEY).join("rip.txt");
    h.wait("the rip runs", |_| rip.is_file());
    h.remove("tok0");
    h.wait("MediaRemoved", |h| events(h, &id).contains(&Event::MediaRemoved));
    let exit = h.job_dir(&id).join(KEY).join("rip.txt.exit.json");
    h.wait("the killed rip's exit file", |_| exit.is_file());
    h.settle();
    assert_ne!(json_of(&exit)["exit_code"], json!(0));
    let found = events(&h, &id);
    assert!(!found.iter().any(|e| matches!(e, Event::TitleRipFailed { .. })), "{found:?}");
    let last = h.transitions(&id).pop().unwrap();
    assert_eq!((last.event, last.new), (Some(Event::MediaRemoved), State::Ripping));
}

#[test]
fn a_disc_name_in_windows_1252_reaches_ejected_with_scan_txt_unchanged() {
    let h = harness(&["movie-direct-cp1252"], json!({}));
    let id = run_to(&h, "Ejected");
    let scenario = fs::read(h.tools().join("scan.txt")).unwrap();
    assert!(String::from_utf8(scenario.clone()).is_err(), "the variant's scan.txt holds the raw byte 0xE9");
    assert_eq!(fs::read(h.job_dir(&id).join("scan.txt")).unwrap(), scenario);
    assert_eq!(h.record(&id).plan.unwrap().mode, Mode::Direct);
    // No engine output carries CINFO 2, so this pins the planner's reading of the recorded bytes, Step 4 item 5's calls.
    let read = lazymkv_core::disc::disc(&lazymkv_core::robot::parse(&String::from_utf8_lossy(&scenario))).unwrap();
    assert_eq!(read.name.as_deref(), Some("Synthetic Movie Caf\u{fffd}"));
}

#[test]
fn no_makemkvcon_waits_in_awaiting_tool() {
    let h = harness(&[], json!({"tool_paths": {"makemkvcon": null}}));
    let id = run_to(&h, "AwaitingTool");
    assert_eq!(h.record(&id).state, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Tool }));
    assert!(!h.job_dir(&id).join("scan.txt").exists());
}

#[test]
fn a_makemkvcon_below_the_minimum_version_waits_in_awaiting_tool() {
    let h = harness(&[], json!({"makemkv_min_version": "2.0.1"}));
    let id = run_to(&h, "AwaitingTool");
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    assert_eq!(record.message.as_deref(), Some("makemkvcon64 is version 2.0.0, below makemkv_min_version 2.0.1"));
    assert!(!h.job_dir(&id).join("scan.txt").exists());
}

#[test]
fn a_scan_licence_message_waits_in_awaiting_makemkv() {
    let h = harness(&["movie-direct-licence"], json!({}));
    let id = run_to(&h, "AwaitingMakeMkv");
    assert_eq!(h.record(&id).state, State::AwaitingMakeMkv { return_to: Box::new(State::Fingerprinted) });
    let waiting = h.notifications().into_iter().last().unwrap();
    assert_eq!(waiting.event, NotifyEvent::Waiting { reason: WaitReason::Makemkv });
    assert!(waiting.message.contains("Evaluation period has expired"));
}

#[test]
fn a_licence_message_during_a_rip_waits_in_awaiting_makemkv() {
    let h = harness(&["movie-direct-rip-licence"], json!({}));
    let id = run_to(&h, "AwaitingMakeMkv");
    assert_eq!(h.record(&id).state, State::AwaitingMakeMkv { return_to: Box::new(State::Ripping) });
    assert!(events(&h, &id).contains(&Event::LicenceMessage));
}

#[test]
fn the_eject_fake_failure_sends_eject_failed_and_ends_the_disc_phase() {
    let h = harness(&[], json!({}));
    h.ejector().fail_next("stuck");
    let id = run_to(&h, "Ejected");
    h.settle();
    assert!(events(&h, &id).contains(&Event::EjectFailed));
    let sent: Vec<NotifyEvent> = h.notifications().into_iter().map(|n| n.event).collect();
    assert!(sent.contains(&NotifyEvent::EjectFailed) && !sent.contains(&NotifyEvent::DiscDone));
    assert_eq!(h.awake().holders(), 0);
}

#[test]
fn the_removal_its_own_eject_causes_is_not_fed() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    h.wait("removal seen", |h| h.drives_json()["drives"]["tok0"]["fingerprint"] == json!(null));
    h.settle();
    assert!(!events(&h, &id).contains(&Event::MediaRemoved));
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
}

#[test]
fn the_scan_does_not_wait_for_a_slow_lookup() {
    let h = harness(&[], json!({}));
    h.scan_waits_for_lookups(false);
    let gate = h.http().gate(&lazymkv_clients::dischash_url(&h.fingerprint("movie-direct")));
    let id = run_to(&h, "Ejected");
    assert!(h.record(&id).plan.unwrap().identity.lookup_pending);
    gate.open();
}

#[test]
fn a_remembered_decision_is_applied_and_copied_as_remembered_json() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    let text = format!(concat!(
        r#"{{"format":"lazymkv remembered v1","fingerprint":"{fp}","tmdb":{{"kind":"movie","id":900001}},"titles":[{{"#,
        r#""source_file":"00800.mpls","dvd_title":null,"segment_map":"1","seconds":7150,"role":{{"role":"feature"}},"#,
        r#""audio":[],"subtitles":[]}}]}}"#), fp = fp);
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), &text).unwrap();
    let id = run_to(&h, "Ejected");
    let plan = h.record(&id).plan.unwrap();
    assert_eq!((plan.identity.step, plan.identity.confidence), (IdentityStep::Remembered, 1.0));
    assert_eq!(fs::read(h.job_dir(&id).join("remembered.json")).unwrap(), text.as_bytes());
}

#[test]
fn a_remembered_file_of_another_major_version_is_ignored_and_reported_in_engine_log() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")),
        format!(r#"{{"format":"lazymkv remembered v2","fingerprint":"{fp}","tmdb":{{"kind":"movie","id":900001}},"titles":[]}}"#)).unwrap();
    let id = run_to(&h, "Ejected");
    assert_ne!(h.record(&id).plan.unwrap().identity.step, IdentityStep::Remembered);
    assert!(!h.job_dir(&id).join("remembered.json").exists());
    assert!(h.engine_log().contains("lazymkv remembered v2"));
}
```

Two tests for `keep_tuning_evidence` (design parts 5.3 and 7.2, S45, S61), the tests of RP-1, RP-2 and RP-3, and the scan retry test of RP-5.

```rust
#[test]
fn with_tuning_evidence_on_every_makemkvcon_call_gets_debug_and_its_log_is_saved() {
    let h = harness(&[], json!({"keep_tuning_evidence": true}));
    let log = h.tools().join("MakeMKV_log.txt");
    fs::write(&log, b"debug KEY-123 line\n").unwrap();
    // FakeHost's clock is fixed, so dating the log 60 s after it makes the log newer than every call (S61).
    let after_start = std::time::UNIX_EPOCH + std::time::Duration::from_secs(h.host().now_unix() + 60);
    fs::File::options().write(true).open(&log).unwrap().set_modified(after_start).unwrap();
    h.host().set_debug_log(&log);
    h.host().set_app_key("KEY-123");
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    let job_log = fs::read_to_string(dir.join("job.log")).unwrap();
    let makemkv_runs: Vec<&str> =
        job_log.lines().filter(|line| line.starts_with("run ") && line.contains("makemkvcon64")).collect();
    // The listing before the scan, the scan, the listing before the rip, the rip.
    assert_eq!(makemkv_runs.len(), 4, "{job_log}");
    assert!(makemkv_runs.iter().all(|line| line.split(' ').any(|arg| arg == "--debug")), "{job_log}");
    for run in ["drive-list.txt", "drive-list-2.txt", "scan.txt", "t00-00800/rip.txt"] {
        assert_eq!(fs::read(dir.join(format!("{run}.makemkv-debug.txt"))).unwrap(), b"debug [app_Key removed] line\n", "{run}");
    }
}

#[test]
fn a_missing_debug_log_only_writes_a_job_log_line() {
    let h = harness(&[], json!({"keep_tuning_evidence": true}));
    let log = h.tools().join("MakeMKV_log.txt");
    h.host().set_debug_log(&log);
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    let job_log = fs::read_to_string(dir.join("job.log")).unwrap();
    let line = format!("scan.txt: the MakeMKV debug log was not saved: {}: no debug log was written", log.display());
    assert!(job_log.contains(&line), "{job_log}");
    assert!(!dir.join("scan.txt.makemkv-debug.txt").exists());
    assert!(events(&h, &id).contains(&Event::Ejected));
}

/// RP-1: every later listing is recorded as drive-list-<n>.txt.
#[test]
fn later_drive_listings_are_recorded_as_drive_list_n() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    for run in ["drive-list.txt", "drive-list-2.txt"] {
        for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
            assert!(dir.join(format!("{run}{suffix}")).is_file(), "{run}{suffix}");
        }
    }
    assert!(!dir.join("drive-list-3.txt").exists());
    let log = fs::read_to_string(dir.join("job.log")).unwrap();
    assert!(log.contains("drive listing 1: tok0 is index 0") && log.contains("drive listing 2: tok0 is index 0"), "{log}");
}

/// RP-2: a listing with no DRV line for the job's drive fails the scan as Unknown.
#[test]
fn a_missing_drive_index_fails_the_scan_naming_the_drive() {
    let h = harness(&[], json!({}));
    h.insert("tok1", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.wait("Failed", |h| h.transitions(&id).iter().any(|line| line.new.as_str() == "Failed"));
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Fingerprinted) });
    assert_eq!(record.message.as_deref(), Some("MakeMKV's drive listing has no drive tok1"));
    assert!(!h.job_dir(&id).join("scan.txt").exists());
}

/// RP-3: a missing appdata.tar waits in AwaitingTool.
#[test]
fn a_missing_appdata_tar_waits_for_makemkvcon() {
    let h = harness(&[], json!({}));
    fs::remove_file(h.tools().join("appdata.tar")).unwrap();
    let id = run_to(&h, "AwaitingTool");
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    let message = record.message.unwrap_or_default();
    assert!(message.contains("appdata.tar"), "{message}");
    assert!(!h.job_dir(&id).join("job.mmcp.xml").exists() && !h.job_dir(&id).join("scan.txt").exists());
}

/// RP-5: the engine, not the driver, retries a scan past its limit, after a fresh drive listing.
#[test]
fn a_scan_past_its_limit_is_killed_listed_again_and_retried_once() {
    let h = harness(&["movie-direct-scan-hang"], json!({}));
    h.settle();
    let before = runs(&h, "makemkvcon64", "0");
    let id = run_to(&h, "Ejected");
    // The hanging scan and the retry, both counted under their first matching response.
    assert_eq!(runs(&h, "makemkvcon64", "1"), 2);
    assert_eq!(runs(&h, "makemkvcon64", "3"), 1, "one rip");
    assert_eq!(runs(&h, "makemkvcon64", "0"), before + 3, "a listing before each scan and before the rip");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert_eq!(log.lines().filter(|line| line.starts_with("run scan.txt:")).count(), 2, "{log}");
}
```

These tests also import `lazymkv_platform::Host` (for `now_unix`). The retry test runs under the harness's 10 s scan limit, so it takes about 10 s.

`crates/dev/tests/clone_values.rs`:
- `PUBLIC_FOLDERS[4]` becomes `"corpus/scenarios/disc-a-scan"`.
- In the existing test, `sources.retain` (`:139`) keeps files whose extension is `rs` or `json`, so the characterisation files Task 17 adds under `crates/dev/src/cmd/decide/characterisation/` are checked too.
- The loop from `let values = clone_values(&evidence);` to the end of the existing test moves into `fn holders(root: &Path, public: &str, values: &BTreeMap<String, (String, CloneValue)>, texts: &[(PathBuf, String, String)]) -> Vec<String>`, used by both tests.
- New test:

```rust
#[test]
fn no_scenario_holds_a_clone_value() {
    let mut ctx = Ctx::for_test(&std::env::current_dir().expect("a current directory"));
    if let Ok(value) = std::env::var("LAZYMKV_EVIDENCE") {
        ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), value);
    }
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => return skip(&refused.line),
    };
    let root = repository();
    let mut public_files: Vec<PathBuf> = PUBLIC_FILES.iter().map(|file| root.join(file)).collect();
    for folder in PUBLIC_FOLDERS {
        files_under(&root.join(folder), &mut public_files);
    }
    let public: String = public_files.iter().filter(|p| p.is_file()).map(|p| plain_digits(&text_of(p))).collect();
    let mut sources = Vec::new();
    files_under(&root.join("corpus/scenarios"), &mut sources);
    sources.retain(|path| !path.starts_with(root.join("corpus/scenarios/disc-a-scan")));
    sources.sort();
    assert!(!sources.is_empty(), "no scenario under corpus/scenarios besides disc-a-scan");
    let texts: Vec<(PathBuf, String, String)> = sources.into_iter().map(|p| (p.clone(), text_of(&p), plain_digits(&text_of(&p)))).collect();
    let found = holders(&root, &public, &clone_values(&evidence), &texts);
    assert!(found.is_empty(), "ruling RP21: a scenario holds values of the private evidence clone:\n{}", found.join("\n"));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase`
Expected: compile error, `no function or associated item named 'new' found for struct 'Harness'`.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit`
Expected: compile errors naming `rip_check`, `Promises`, `write_job_profile`, `version_of` and `next_attempt_dir`.

- [ ] **Step 4: Implement**

1. **owner.rs.**
   - With `services.steps` None, the owner builds `RealSteps::new(..)` with its `Outbox`, an `Arc<Store>`, an `Arc<Mutex<Promises>>` and an `Arc<Mutex<Option<bool>>>` for the last scan's Java use, which it keeps for Task 26. Task 22's Unsupported branch goes away.
   - After each transition, a job whose old state was `Ripping` and whose new state is not `Ripping` gets `release(id, Rip)`.
   - A new `Failed` or waiting state releases both of its promises.
2. **Every tool run in RealSteps.**
   - It uses `ToolCtx { guard: processes, host, limits: config.limits_override or Limits::from_settings(&job.settings), cancel, .. }`, with `on_progress` forwarding to `progress(Some(key), f)` during a rip.
   - Before the run, `job.log` gets `run <name>: <program> <args joined by " ">`. After it, `ended <name>: exit <code or none> after <elapsed_ms> ms`.
   - With `keep_tuning_evidence` on, every `makemkvcon` call is flagged `--debug` (S45), followed by `save_debug_log(host, run.started_unix, &<the run's out path>)`, which itself writes `<out>.makemkv-debug.txt` (Task 14). An Err only writes the `job.log` line `<name>: the MakeMKV debug log was not saved: <err>`.
   - Tools are found with `find_tool(tool, &job.settings, config.resources, host, config.path_var)`.
3. **Fingerprint.** Only an `Action::Fingerprint` from `EngineRestart` in `Detected` reaches the runner; a new job's is done by `spawn_recording` (Task 22, item 5). `tools::snapshot(drive_root, label, job_dir, now)`. The same fingerprint gives `FingerprintDone`; anything else gives `FingerprintFailed { Unknown }`.
4. **RunScan.**
   1. No `makemkvcon` gives `ToolMissing { Makemkvcon }`.
   2. No `java`, and no `LabelRuleUnavailable` note yet, gives `outbox.feed_first(ToolMissing { Java })`.
   3. When `job.lookups` has no states, `spawn_round(.., secrets.get("tmdb_token"), host.local_date(), ..)` with a no-op `done`.
   4. A missing `job.mmcp.xml` is written with `write_job_profile(.., ALL_TRACKS_SELECTION, JOB_PROFILE_FILE)`. An Err gives `ToolMissing { Makemkvcon }` with the Err's text as the message (RP-3).
   5. The drive listing, into `drive-list.txt` for the job's first listing and `drive-list-<n>.txt` (n from 2, one more than the highest existing) for each later one (RP-1). Each listing writes the `job.log` line `drive listing <n>: <drive> is index <index>`. A listing with no `DRV` line for the job's drive gives `ScanFailed { Unknown }` with `MakeMKV's drive listing has no drive <drive>` (RP-2); in RipTitle's loop (item 7) it gives `TitleRipFailed { Unknown }` with the same message.
   6. `version_of(summary.version)`. A version below `version_triple(makemkv_min_version)` gives `ToolMissing { Makemkvcon }` with the message above. No version writes the `job.log` line `the drive listing printed no MakeMKV version`, and the scan goes on.
   7. `scan` into `scan.txt` with `MakemkvFlags { minlength_s, cache_mb, profile: <job_dir>/<record.profile>, debug }`.
   8. Outcomes:
      - `Done` writes the `job.log` line `scan: MakeMKV <summary.version, or "version unknown">; java in use: <summary.java>` (design part 5.4 step 4), stores `summary.java` in `java_in_use`, and gives `ScanDone`.
      - `ProfileRejected { text }` while the profile is `job.mmcp.xml`: write `fallback.mmcp.xml` with `FALLBACK_SELECTION`, `outbox.edit` the profile to `FALLBACK_PROFILE_FILE`, write `job.log` `MakeMKV rejected job.mmcp.xml: <text>`, then list and scan again (RP-6).
      - A second `ProfileRejected { text }` gives `ScanFailed { Unknown }` with `MakeMKV rejected the fallback profile: <text>`.
      - The first `Failed` with Task 13's message `the scan did not finish within scan_timeout_min and was killed` lists the drives again (item 5) and scans once more into the same `scan.txt` (S58, RP-5); a second such `Failed` gives `ScanFailed { Unknown }` with `the scan did not finish within scan_timeout_min and was killed twice`.
      - Any other `Failed { failure, message }` gives `ScanFailed { failure }` with that message.
      - An io error gives `ScanFailed { Unknown }`.
5. **Plan.**
   - `disc(&parse(&lossy(scan.txt)))`.
   - `try_lock` the book (R23.1).
   - `Store::remembered(&fingerprint)`:
     - `Ok(Some((rem, bytes)))` writes `remembered.json` byte for byte.
     - `Err(e)` writes the engine.log line `<id>: the remembered decision is refused: <e>`.
   - `plan(&PlanInputs { disc, fingerprint, disc_id, lookups: book.lookup_inputs(chain_inputs(job_dir)?.name.as_ref()), remembered, overrides: None, sessions: &[], library: &LibraryFacts::default(), settings: &job.settings })` (S30).
   - The plan is persisted through `outbox.edit`, with one `TitleRecord` per planned title: attempts 0, verified false and every list empty.
   - A Copy plan gives `PlanFailed { Unknown }` with `the plan needs whole-disc copy mode, which arrives in phase 3` (P1). Otherwise `PlanDone { mode: Direct }`. `plan`'s Err gives `PlanFailed { Unknown }`.
6. **CheckSpace in Planned.** `rip_check(rip_promise(plan, meta.json total_bytes), planned_remux_promise(plan), ..)`. The three promise functions are one-line calls of core's `staging_bytes_needed` and `write_bytes_needed`, the one copy of the amounts that Tasks 24 and 25 call too.
   - Rule:
     - An unreachable staging or library root gives `ShareLost`.
     - Staging needs `rip + (remux when the library is on the same volume) + promised(staging volume, job)`.
     - A separate library volume needs `remux + promised(its volume, job)`.
     - A `free_bytes` Err gives `ShareLost`, and a short volume gives `SpaceShort`.
     - Otherwise promise Rip and Remux and give `SpaceOk`.
7. **RipTitle.** For the first title not yet verified:
   1. When `<key>/rip.txt` exists, move every file directly under `<key>/` into `next_attempt_dir`.
   2. Empty `<staging>/<id>/rip/<key>/`.
   3. Loop, with at most three mismatches: listing, then `rip(.., index, title, planned title's source(), dir, <key>/rip.txt, &|| StagingHealth { share_healthy: reach is Reachable, out_of_space: free < 1_048_576 })`.
   4. On `IndexMismatch`:
      - Rescan into `rescan-<n>.txt`, n one more than the highest existing (from 2).
      - Map the title to the first rescan title that `same_scan_title` matches with the planned scan title.
      - No match gives `TitleRipFailed { Unknown }` with `the rescan lists no title matching <key>`.
      - The third mismatch gives `TitleRipFailed { Unknown }` with the message above.
   5. Outcomes:
      - `Ripped` edits the title (attempts + 1, rip_index) and gives `TitleRipped`.
      - `Licence` gives `LicenceMessage`.
      - `Failed` gives `TitleRipFailed { failure }`.
   - Ruling R23.6: the out-of-space threshold is 1 MiB of free staging space, because design part 5.6 gives no number.
8. **VerifyFile in Ripping.**
   - First it writes `<key>/rip-summary.json` (R23.4): `{"ok":true,"result":"ok","slug","label","title":<planned scan index>,"profile","bytes","elapsed_s","exit_code","files","dest","cache_mb"}`, plus `"rip_index"` only when the title record's `rip_index` differs from `title`. `elapsed_s` is `elapsed_ms / 1000.0` and `exit_code` is `exit_code`, both from `<key>/rip.txt.exit.json`; `bytes` and `files` are the staged `.mkv`'s size and name; `dest` is the staging title folder; `cache_mb` is the job's.
   - `ident(.., the folder's one .mkv, <key>/mkvmerge.json)`, then `verify_rip(scan title, &ident, &selected_streams(scan, title, Selection::All), Some(rip log), disc.is_dvd)`.
   - `second` is true when this title already failed verification in this engine run. `last` is true when no later title is unverified.
   - Ok edits verified, read_errors and the `failure_reviews(&report, false)` reasons, and gives `TitleVerified { last }`.
   - A mismatch, or an ident error, gives `TitleVerifyFailed { second, last }`. When `second`, rip_reviews gets `failure_reviews(&report, true)`.
9. **Eject.** `ejector.eject(&record.drive)`. Ok gives `Ejected`; Err gives `EjectFailed` with its message. **EmptyTitleDirectory** empties the staging folder of the first unverified title and returns no event.
10. **testkit `Harness::new`.**
    - Copies `corpus/scenarios/<first>` into `<scratch>/tools`, then each later folder over it.
    - Copies `fake_tool` there as `exe_name(tool)` for every `Tool`.
    - Writes `appdata.tar` = `ustar_build(&[("data/default.mmcp.xml", default_profile_xml().as_bytes())])`.
    - Builds `<scratch>/volumes/<first scenario>/` from `disc.json` on `insert` of that name (text, hex-decoded, or `size` zero bytes), and labels it with `disc.json`'s `label`. Any other name gets Task 22's synthetic volume. Its fingerprint and disc ID are not in `http.json`, so its lookups are unrecorded and stay pending, and its scan and rip take the scenario's `makemkvcon64` responses for its drive's index.
    - Sets `tool_paths` to the copies.
    - Loads `http.json` with `RecordedTransport::from_file`, with vars `fingerprint`, `disc_id` (upper-case hex of the built volume) and `search` = `clients::search_url(&disc_name(bdmt_name(<bdmt_eng.xml text>).as_deref(), Some(label)).unwrap().cleaned)`.
    - Uses `TestGuard` (R23.2).
    - Everything else is as `new_scripted`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit`
Expected: PASS, with 7 new tests.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase`
Expected: PASS, 31 tests.

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS, with `no_scenario_holds_a_clone_value` and `no_file_in_crates_holds_a_value_only_the_evidence_clone_holds` both run (not skipped).

- [ ] **Step 6: Commit**

```
git add crates/engine/src crates/fake-tools/tests/engine_disc_phase.rs corpus/scenarios crates/dev/tests/clone_values.rs
git commit -m "engine: the disc phase on stand-ins, from arrival to eject (design parts 5.4 to 5.6, 5.8)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, the noreply identity twice, and no attribution line.

---

### Task 24: The file phase (design parts 5.7, 6.4, 6.5, 8.3, 8.4)

**Files:**
- Create:
  - `crates/engine/src/file_phase.rs` (7 tests in `mod tests`)
  - `crates/fake-tools/tests/engine_file_phase.rs` (20 tests)
  - The scenario variants `corpus/scenarios/movie-direct-two-subs/`, `movie-direct-probe-error/`, `movie-direct-remux-error/`, `movie-direct-partial-long/`, `movie-direct-lookup-late/`, `movie-direct-lookup-later/`, `movie-direct-lookup-down/`, `movie-direct-lookup-partial/`, `movie-direct-lookup-refused/` and `movie-direct-lookup-unparsed/`
- Modify:
  - `crates/engine/src/steps.rs`: Probe, ExtractSubtitleEvidence, Decide, CheckSpace in Decided, Remux, VerifyFile in Remuxing, File, DeletePartialFile
  - `crates/engine/src/space.rs`: `remux_check` (1 test)
  - `crates/engine/src/owner.rs`: the slot, rounds, timers, RefreshIdentity, `filed_unix`, the Remux promise release
  - `crates/engine/src/testkit.rs`: `on_spawn`
  - `crates/engine/src/lib.rs`: `mod file_phase;`, declared in Step 2 with the new file

**Interfaces:**

Consumes:
- Task 19's `read_decide_inputs`, `rec_folder`, `decide_doc`, `no_text_entry`, `DecideInputs`, `FileFacts`, `RipLogText`, `EngineDecideDoc`, and Task 20's `replay_answers`.
- `dev::cmd::plan::{plan_doc (plan.rs:175), PlanOptions (plan.rs:118), PlanSource}`, `dev::evidence::Ctx::from_process` (evidence.rs:33), `dev::pyjson::{dumps, Style::INDENT1_ASCII}`.
- `core`:
  - `plan::refresh_identity` (plan/mod.rs:519), `plan::write_bytes_needed` (:912)
  - `decision::{same_path_reviews (decision.rs:341), FileDecision (:139)}`, `job::decision_done` (job.rs:705)
  - `naming::{NamedPath (naming.rs:196), edition_label (:73)}`, `plan::movie::{LibraryFacts (plan/movie.rs:813), THEATRICAL_CUT (:26)}`, `settings::EditionStyle` (settings.rs:13)
  - `recording::files::MkvmergeIdent` (files.rs:81), to read `<key>/mkvmerge.json` back for the staged report
  - `verify::{verify_rip (:371), verify_final (:489), rip_title (:49), removed_scan_streams (:541), selected_streams (:34)}`
  - `reconcile::{reconcile (reconcile.rs:314), RipLog (:281)}`
  - `tracks::subtitle::step10_runs` (tracks/subtitle.rs:202)
  - `audio::rule::RULE_VERSION` (rule.rs:21), `record::FiledFile`
- Task 14's `probe`, `remux_args(partial: &Path, layout: &Layout, staged: &VerifyReport, raw: &Path) -> Result<Vec<String>, String>`, `remux`, `ident`. Task 9's `rename_no_replace`. Task 23's `spawn_round`, `Promises`, `volume_of` and `remux_promise`.

Produces:

```rust
// crates/engine/src/file_phase.rs
/// The file-phase slot, first in first out (design part 5.7). Def: T24. Use: T24, T25, T26.
pub(crate) struct SlotQueue { /* capacity, holders: Vec<String>, queue: VecDeque<String> */ }
impl SlotQueue {
    pub(crate) fn new(capacity: usize) -> SlotQueue;      // settings.file_phase_concurrency
    pub(crate) fn join(&mut self, job: &str);             // appended; a job already queued or holding keeps its place
    pub(crate) fn leave(&mut self, job: &str);
    pub(crate) fn next(&self) -> Option<String>;          // the head, only while a place is free
    pub(crate) fn grant(&mut self, job: &str);
    pub(crate) fn after(&mut self, job: &str, state: &State) -> bool; // true when a holder reached Filed, NeedsReview, Failed or Discarded and was released
}
/// `<library root>/.lazymkv-partial/<job id>-<title key>.mkv.partial` (design part 5.7 step 13). Def: T24. Use: T24, T25, T26.
pub(crate) fn partial_path(library_root: &str, job_id: &str, key: &str) -> PathBuf;
/// The engine's DecideInputs for one title (K5, design part 5.7 step 11.3). Def: T24. Use: T24, T25.
pub(crate) fn engine_decide_inputs(ctx: &Ctx, job: &JobContext, plan: &Plan, key: &str, probes: &FfprobeStreams,
    library: LibraryFacts) -> Result<DecideInputs, String>;
/// LibraryFacts at the namer's path (design part 6.4): `path_collision` when `path.path` exists, and the edition facts
/// from one listing of the film's folder `<root>/<components[0]>` (Ruling R24.5); a missing folder gives no facts.
/// Def: T24. Use: T24.
pub(crate) fn library_facts(path: &NamedPath, style: EditionStyle) -> LibraryFacts;
/// The waiting message of a pending lookup (design part 8.3): each pending lookup in `Lookup` order as
/// `<word>: <reason>`, joined by "; ", the word being `dischash`, `discid`, `search` or `tmdb record` (Ruling R24.6).
/// Def: T24. Use: T24, T26.
pub(crate) fn waiting_message(book: &LookupBook) -> String;

// crates/engine/src/space.rs
/// The remux's check (design part 5.8, S54): the staged files' sizes; SpaceOk replaces this job's Remux promise.
/// Def: T24. Use: T24.
pub(crate) fn remux_check(remux_bytes: u64, library: &str, shares: &dyn ShareConnector, promises: &mut Promises, job: &str) -> Event;

// testkit
impl Harness { pub fn on_spawn(&self, needles: &[&str], action: impl Fn() + Send + Sync + 'static); } // runs once, before the first spawn whose args hold every needle
```

Ruling R24.1: a lookup round runs only for the head of the slot queue, and the slot is not granted while that round runs. This keeps "oldest first" exact (design part 5.7).

Ruling R24.2: `RefreshIdentity` is applied by the owner to the transition's own record before `state.json` is written, as R22.3 does for the other record actions.

Ruling R24.3: the engine takes each title's `FileDecision` from its rendered `DecideDoc`. So `decide.json` and the decision fed are one value (S24, S26).

Ruling R24.4: six lookup variants are added. Each holds only `http.json`, because the listed variants cannot leave a lookup pending.

Ruling R24.5: the edition facts of design part 6.4 come from one `read_dir` of `<root>/<F>`, where `F = path.components[0]`, the film folder the namer built. A file named `<F> - <R>.mkv` or `<F> - <R> <part>.mkv`, with `R` one of `resolution_label`'s values (`2160p`, `1080p`, `720p`, `576p`, `480p`), is one the namer builds for this film:
- with no part, it is a copy filed without an edition name, the theatrical or main cut (spec 8.4): `earlier_edition_names["theatrical"] = ""`;
- with `edition_style` Jellyfin, a part `E`, and with Combined, a part `E {edition-E}`, is an edition, where `E` is not empty, is not ASCII digits only (that is a copy number, spec 11's keep both) and equals `edition_label(E)`. It sets `folder_has_edition_named_file`, and gives `earlier_edition_names["theatrical"] = E` when `E` is `THEATRICAL_CUT`, else `earlier_edition_names[E.to_lowercase()] = E`;
- with Plex, an edition's files live in their own folder, so no part is read;
- any other name is ignored, and the first name in byte order wins a key.

This is the namer's own pattern (naming.rs:234-264), read back, so no rule beyond design part 6.4 is added.

Ruling R24.6: the reasons of `waiting_message` are `no tmdb_token is set` (NoToken), `TMDb refused the token (HTTP 401)` (TokenRefused), `no answer within 10 s` (Timeout), the error text (Unreachable), `HTTP <n>` (Status) and the parser's sentence (Unparsed). A round that leaves a lookup `Unparsed` also writes the `job.log` line `lookup <word>: <the parser's sentence>` (design part 8.3).

RP-4 settles this: (a) the plan section of an engine `decide.json` is `plan_doc` over the job folder with its `.plan` replaced by the plan the job decided with (the persisted plan, refreshed after a late lookup; design part 8.3, R24.2), so `decide.json` shows the plan behind the decision, and R-B1 (`lazymkv-dev decide` over the finished job folder printing it without its two extra keys) holds while the two plans agree, which Task 29 assertion 7 checks on the CI disc.

- [ ] **Step 1: Write the variants**

All are `http.json` with the base's four answers, except:
- `movie-direct-lookup-late`: dischash is `[{"url":..,"error":"timeout"}, {"url":..,"status":404,"body":""}]`.
- `movie-direct-lookup-later`: dischash is timeout, timeout, then 404.
- `movie-direct-lookup-down`: dischash is `[{"url":..,"error":"timeout"}]`.
- `movie-direct-lookup-partial`: dischash is timeout, timeout, then 404, and the movie record URL is `[{"url":..,"error":"timeout"}]`.
- `movie-direct-lookup-refused`: the search URL is `{"url":"{search}","status":401,"body":"{\"success\":false}"}`, and there is no record answer.
- `movie-direct-lookup-unparsed`: dischash is `{"url":..,"status":200,"body":"{not json"}`.

`movie-direct-two-subs`:
- `scan.txt` is the base plus `SINFO:0,2,…` and `SINFO:0,3,…`, each subtitle stream with:
  - `1,6203,"Subtitles"`
  - `3,0,"eng"`, `4,0,"English"`
  - `5,0,"S_HDMV/PGS"`, `6,0,"PGS"`, `7,0,"HDMV PGS Subtitles"`
  - `22,0,"0"`, `33,0,"90"`
- `ident-raw.json` is the base plus `{"id":2,"type":"subtitles","codec":"HDMV PGS","properties":{"language":"eng"}}` and the same with id 3.

The file phase's own failures:
- `movie-direct-probe-error`: `ffprobe.json` is `{"responses":[{"args_contain":["-show_streams"],"exit_code":1}]}`.
- `movie-direct-remux-error`: `mkvmerge.json` is the base's, with the remux response `{"args_contain":["--gui-mode","-o"],"stdout":"remux-mkvmerge.txt","exit_code":2}` and no `creates`.
- `movie-direct-partial-long`: `partial-ident.json` is the base's with the container duration `7153000000000`, 3 s more than the staged file's 7150 s, beyond `DURATION_MATCH_S` (verify.rs:500-503).

- [ ] **Step 2: Write the failing tests**

First add `mod file_phase;` to `crates/engine/src/lib.rs`, so the new file compiles with its tests.

`crates/engine/src/file_phase.rs` `mod tests`:

```rust
use super::*;
use lazymkv_core::job::State;

#[test]
fn the_slot_is_granted_to_the_oldest_ejected_job() {
    let mut slots = SlotQueue::new(1);
    slots.join("b-1");
    slots.join("a-1");
    slots.join("b-1");
    assert_eq!(slots.next(), Some("b-1".to_string()));
    slots.grant("b-1");
    assert_eq!(slots.next(), None);
    assert!(!slots.after("b-1", &State::Remuxing));
    assert!(slots.after("b-1", &State::Filed));
    assert_eq!(slots.next(), Some("a-1".to_string()));
}

#[test]
fn the_slot_is_held_through_a_wait_entered_after_the_grant() {
    let mut slots = SlotQueue::new(1);
    slots.join("a-1");
    slots.grant("a-1");
    assert!(!slots.after("a-1", &State::AwaitingSpace { return_to: Box::new(State::Decided { approved: None }) }));
    assert!(slots.after("a-1", &State::NeedsReview { held: lazymkv_core::job::Held::DispositionGate }));
}

#[test]
fn a_partial_file_is_named_by_job_and_title_key() {
    assert_eq!(partial_path("/lib/Movies", "abc-1", "t00-00800"), std::path::Path::new("/lib/Movies/.lazymkv-partial/abc-1-t00-00800.mkv.partial"));
}

fn film_path(root: &std::path::Path) -> NamedPath {
    let folder = "Synthetic Movie (2001) {tmdb-900001}".to_string();
    let components = vec![folder.clone(), format!("{folder} - 1080p.mkv")];
    let root = root.to_str().unwrap().to_string();
    NamedPath { path: join(&root, &components), root, components }
}

#[test]
fn an_edition_named_file_in_the_film_folder_sets_folder_has_edition_named_file() {
    let s = Scratch::new("edition-facts");
    let path = film_path(&s.0);
    let folder = path.components[0].clone();
    s.write(&format!("{folder}/{folder} - 2160p Extended Cut.mkv"), "x");
    s.write(&format!("{folder}/{folder} - 1080p 2.mkv"), "x");
    s.write(&format!("{folder}/poster.jpg"), "x");
    assert_eq!(
        library_facts(&path, EditionStyle::Jellyfin),
        LibraryFacts {
            folder_has_edition_named_file: true,
            earlier_edition_names: [("extended cut".to_string(), "Extended Cut".to_string())].into(),
            path_collision: false,
        }
    );
    assert_eq!(library_facts(&path, EditionStyle::Combined), LibraryFacts::default());
}

#[test]
fn an_earlier_copy_without_an_edition_is_the_theatrical_cut_and_a_collision() {
    let s = Scratch::new("theatrical-facts");
    let path = film_path(&s.0);
    let folder = path.components[0].clone();
    s.write(&format!("{folder}/{}", path.components[1]), "x");
    s.write(&format!("{folder}/{folder} - 2160p Theatrical Cut {{edition-Theatrical Cut}}.mkv"), "x");
    let facts = library_facts(&path, EditionStyle::Combined);
    assert!(facts.path_collision && facts.folder_has_edition_named_file);
    assert_eq!(facts.earlier_edition_names, [("theatrical".to_string(), String::new())].into());
}

#[test]
fn a_missing_film_folder_gives_no_facts() {
    let s = Scratch::new("no-film-folder");
    assert_eq!(library_facts(&film_path(&s.0), EditionStyle::Jellyfin), LibraryFacts::default());
}

#[test]
fn the_waiting_message_names_each_pending_lookup_and_its_reason() {
    let mut book = LookupBook::default();
    book.states.insert(Lookup::Dischash, LookupState::Pending { reason: PendingReason::Timeout });
    book.states.insert(Lookup::Discid, LookupState::Answered { status: 404 });
    book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::TokenRefused });
    book.states.insert(Lookup::Record, LookupState::Pending { reason: PendingReason::Status(503) });
    assert_eq!(
        waiting_message(&book),
        "dischash: no answer within 10 s; search: TMDb refused the token (HTTP 401); tmdb record: HTTP 503"
    );
    book.states.insert(Lookup::Dischash, LookupState::Pending { reason: PendingReason::Unparsed("x: bad".to_string()) });
    book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::NoToken });
    book.states.remove(&Lookup::Record);
    assert_eq!(waiting_message(&book), "dischash: x: bad; search: no tmdb_token is set");
}
```

The `file_phase.rs` tests also import `lazymkv_core::naming::{join, NamedPath}`, `lazymkv_core::plan::movie::LibraryFacts`, `lazymkv_core::settings::EditionStyle`, `lazymkv_core::testsupport::{Scratch, ScratchExt}` and `lazymkv_clients::{Lookup, LookupBook, LookupState, PendingReason}`.

`crates/engine/src/space.rs` `mod tests`:

```rust
#[test]
fn the_remux_check_replaces_the_rips_remux_promise_with_the_staged_size() {
    let shares = FakeShares::new();
    let mut promises = Promises::default();
    let library = volume_of(&shares, "/m");
    promises.promise("a-1", PromisedStep::Remux, &library, 4301);
    shares.set_free("/m", 17);
    assert_eq!(remux_check(17, "/m", &shares, &mut promises, "a-1"), Event::SpaceOk);
    assert_eq!(promises.promised(&library, "b-1"), 17);
    shares.set_free("/m", 16);
    assert_eq!(remux_check(17, "/m", &shares, &mut promises, "a-1"), Event::SpaceShort);
}
```

`crates/fake-tools/tests/engine_file_phase.rs` begins with its own helpers, the same as `engine_disc_phase.rs`'s (each integration test file is its own crate):

```rust
//! The file phase on stand-ins, from the slot to the filed file (design parts 5.7, 6.4, 6.5, 8.3, 8.4).

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::job::{Action, Event, FailureClass, Held, NotifyEvent, State, Tool, WaitReason};
use lazymkv_core::lookup::IdentityStep;
use lazymkv_core::review::ReviewReason;
use lazymkv_dev::cmd::decide::{decide_doc, read_decide_inputs, rec_folder};
use lazymkv_dev::cmd::plan::PlanOptions;
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Host;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";

fn harness(variants: &[&str], settings: Value) -> Harness {
    let mut scenarios = vec!["movie-direct"];
    scenarios.extend_from_slice(variants);
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &scenarios, settings)
}

/// Inserts movie-direct in tok0 and waits for a persisted transition into `state` (not the current state).
fn run_to(h: &Harness, state: &str) -> String {
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.wait(state, |h| h.transitions(&id).iter().any(|line| line.new.as_str() == state));
    id
}

fn json_of(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn events(h: &Harness, id: &str) -> Vec<Event> {
    h.transitions(id).into_iter().filter_map(|line| line.event).collect()
}
```

Then the tests:

```rust
fn final_path(h: &Harness) -> PathBuf {
    let folder = "Synthetic Movie (2001) {tmdb-900001}";
    h.library().join(folder).join(format!("{folder} - 1080p.mkv"))
}

#[test]
fn a_lookup_pending_plan_gets_one_round_before_the_slot() {
    let h = harness(&["movie-direct-lookup-late"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "Filed");
    let found = events(&h, &id);
    assert!(!found.contains(&Event::LookupPending));
    assert!(found.contains(&Event::FilePhaseSlot));
    let dischash = lazymkv_clients::dischash_url(&h.fingerprint("movie-direct"));
    assert_eq!(h.http().requests().iter().filter(|(url, _)| *url == dischash).count(), 2);
    assert!(fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap().contains("dischash 404"));
}

#[test]
fn still_pending_after_the_round_waits_in_awaiting_lookup_with_a_60_minute_timer() {
    let h = harness(&["movie-direct-lookup-down"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    assert_eq!(h.record(&id).state, State::AwaitingLookup { return_to: Box::new(State::Ejected { after_copy: false }) });
    assert_eq!(h.timers().pending(), vec![60]);
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Lookup }));
}

#[test]
fn a_timer_expiry_that_answers_returns_with_refresh_identity_first() {
    let h = harness(&["movie-direct-lookup-later"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    h.fire_timers();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let answered = h.transitions(&id).into_iter().find(|l| l.event == Some(Event::LookupAnswered)).unwrap();
    assert_eq!(answered.new, State::Ejected { after_copy: false });
    assert_eq!(answered.actions, vec![Action::RefreshIdentity, Action::Notify { event: NotifyEvent::Resumed }]);
    assert!(!h.record(&id).plan.unwrap().identity.lookup_pending);
}

#[test]
fn an_answer_arriving_during_the_rip_is_saved_and_reaches_decide_without_lookup_answered() {
    let h = harness(&[], json!({}));
    h.scan_waits_for_lookups(false);
    let gate = h.http().gate(&lazymkv_clients::dischash_url(&h.fingerprint("movie-direct")));
    let id = run_to(&h, "Ejected");
    gate.open();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let found = events(&h, &id);
    assert!(!found.contains(&Event::LookupPending) && !found.contains(&Event::LookupAnswered));
    let record = h.record(&id);
    let plan = record.plan.unwrap();
    assert!(!plan.identity.lookup_pending);
    assert_eq!(plan.identity.step, IdentityStep::TmdbOne);
    assert!(record.titles[0].decision.as_ref().unwrap().path.is_some());
}

#[test]
fn a_refused_token_waits_with_a_message_naming_it() {
    let h = harness(&["movie-direct-lookup-refused"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("search: TMDb refused the token (HTTP 401)"), "{message}");
    let waiting = h.notifications().into_iter().last().unwrap();
    assert_eq!(waiting.event, NotifyEvent::Waiting { reason: WaitReason::Lookup });
    assert!(waiting.message.contains("TMDb refused the token (HTTP 401)"), "{}", waiting.message);
}

#[test]
fn an_unparsed_answer_puts_the_parsers_sentence_in_job_log_and_the_waiting_message() {
    let h = harness(&["movie-direct-lookup-unparsed"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    let message = h.record(&id).message.unwrap();
    assert!(message.starts_with("dischash: "), "{message}");
    let sentence = message["dischash: ".len()..].split("; ").next().unwrap().to_string();
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains(&format!("lookup dischash: {sentence}")), "{log}");
}

#[test]
fn a_partial_answer_is_saved_and_the_job_keeps_waiting() {
    let h = harness(&["movie-direct-lookup-partial"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    h.fire_timers();
    h.wait("dischash saved", |h| fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap().contains("dischash 404"));
    h.settle();
    assert_eq!(h.state_of(&id), "AwaitingLookup");
    let last = h.transitions(&id).pop().unwrap();
    assert_eq!((last.event, last.actions), (Some(Event::LookupPending), vec![Action::StartTimer { minutes: 60 }]));
    assert_eq!(h.timers().pending(), vec![60]);
}

#[test]
fn decide_json_is_the_renderers_document_with_two_extra_keys() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    let keys: Vec<&str> = doc.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["slug", "label", "reconcile", "audio", "subtitles", "reviews", "plan", "decision", "audio_rule_version", "lazymkv_version"]);
    assert_eq!((&doc["slug"], &doc["label"]), (&json!(id), &json!(KEY)));
    assert_eq!(doc["audio_rule_version"], json!(lazymkv_core::audio::rule::RULE_VERSION));
    assert_eq!(doc["lazymkv_version"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(doc["audio"], json!([]));
    assert!(doc["subtitles"]["skipped"].as_str().unwrap().starts_with("no OCR text"));
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("step 10a did not run in this version"));
}

#[test]
fn decide_json_uses_the_probe_the_engine_ran() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    assert_eq!(doc["decision"]["facts"]["profile_streams"], json!([1]));
    assert!(h.job_dir(&id).join(KEY).join("ffprobe-streams.json").is_file());
}

#[test]
fn the_engines_decide_inputs_render_as_read_decide_inputs_over_the_finished_job_folder() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let dir = h.job_dir(&id);
    let ctx = Ctx::for_test(h.data());
    let (rec, label) = rec_folder(&ctx, &dir.join(KEY).display().to_string()).unwrap_or_else(|r| panic!("{}", r.line));
    let options = PlanOptions {
        settings: Some(dir.join("settings.json")),
        tmdb_search: Some(dir.join("tmdb-search.json")),
        tmdb_record: Some(dir.join("tmdb-record.json")),
        remembered: None,
    };
    let read = read_decide_inputs(&ctx, &rec, &label, None, None, &options).unwrap_or_else(|r| panic!("{}", r.line));
    let mut written = json_of(&dir.join(KEY).join("decide.json"));
    let object = written.as_object_mut().unwrap();
    object.remove("audio_rule_version");
    object.remove("lazymkv_version");
    assert_eq!(
        dumps(&written, Style::INDENT1_ASCII),
        dumps(&serde_json::to_value(decide_doc(&read)).unwrap(), Style::INDENT1_ASCII)
    );
}

#[test]
fn a_lookup_pending_job_decides_with_the_refreshed_plan() {
    let h = harness(&["movie-direct-lookup-late"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "Filed");
    let record = h.record(&id);
    let persisted = serde_json::to_value(record.plan.as_ref().unwrap()).unwrap();
    assert_eq!(persisted["identity"]["lookup_pending"], json!(false));
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    // RP-4: the plan section carries the persisted refreshed plan.
    assert_eq!(doc["plan"]["plan"], persisted);
}

#[test]
fn two_english_subtitle_streams_fail_the_decision_until_step_10_exists() {
    let h = harness(&["movie-direct-two-subs"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("subtitle evidence (step 10) is not built in this version"));
    assert!(!h.job_dir(&id).join(KEY).join("decide.json").exists());
}

#[test]
fn a_collision_holds_the_job_with_path_collision_and_keeps_the_path() {
    let h = harness(&[], json!({}));
    let target = final_path(&h);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"already here").unwrap();
    let id = run_to(&h, "NeedsReview");
    let record = h.record(&id);
    assert_eq!(record.state, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(record.collisions.iter().map(PathBuf::from).collect::<Vec<_>>(), vec![target.clone()]);
    assert!(record.titles[0].decision.as_ref().unwrap().reviews.contains(&ReviewReason::PathCollision));
    assert_eq!(fs::read(&target).unwrap(), b"already here");
    assert_eq!(fs::read_dir(h.library().join(".lazymkv-partial")).map(|d| d.count()).unwrap_or(0), 0);
}

#[test]
fn an_existing_final_file_fails_file_failed_unknown_and_keeps_the_partial() {
    let h = harness(&[], json!({}));
    let target = final_path(&h);
    let late = target.clone();
    h.on_spawn(&["-J", ".partial"], move || {
        fs::create_dir_all(late.parent().unwrap()).unwrap();
        fs::write(&late, b"late").unwrap();
    });
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Verified) });
    assert!(record.message.unwrap().contains(&target.display().to_string()));
    assert_eq!(fs::read(&target).unwrap(), b"late");
    let partial = h.library().join(".lazymkv-partial").join(format!("{id}-{KEY}.mkv.partial"));
    assert_eq!(fs::read(partial).unwrap(), b"lazymkv-test-mkv");
    assert!(record.collisions.iter().any(|path| PathBuf::from(path) == target));
}

#[test]
fn filed_records_path_size_mtime_and_filed_unix() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let target = final_path(&h);
    assert_eq!(fs::read(&target).unwrap(), b"lazymkv-test-mkv");
    let mtime = fs::metadata(&target).unwrap().modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let record = h.record(&id);
    let filed = record.titles[0].filed.clone().unwrap();
    assert_eq!((PathBuf::from(&filed.path), filed.size, filed.mtime_unix), (target.clone(), 16, mtime));
    assert_eq!(record.filed_unix, Some(h.host().now_unix()));
    let sent: Vec<NotifyEvent> = h.notifications().into_iter().map(|n| n.event).collect();
    assert_eq!(sent, vec![NotifyEvent::DiscDone, NotifyEvent::Filed]);
    assert!(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv").is_file());
}

#[test]
fn the_remux_creates_the_partial_folder_when_the_library_root_has_none() {
    let h = harness(&[], json!({}));
    assert!(h.library().is_dir() && !h.library().join(".lazymkv-partial").exists());
    run_to(&h, "Filed");
    let partial = h.library().join(".lazymkv-partial");
    assert!(partial.is_dir());
    assert_eq!(fs::read_dir(partial).unwrap().count(), 0);
}
```

The file phase's own failure branches (design part 5.7 steps 9, 13 and 14, and spec 10.2's ToolMissing row):

#[test]
fn a_probe_error_fails_probe_failed_unknown() {
    let h = harness(&["movie-direct-probe-error"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("ffprobe exited with code 1"));
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::ProbeFailed { .. })));
}

#[test]
fn a_remux_exit_2_fails_remux_failed_unknown_and_keeps_the_raw() {
    let h = harness(&["movie-direct-remux-error"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert_eq!(record.message.as_deref(), Some("mkvmerge exited with code 2"));
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::RemuxFailed { .. })));
    assert_eq!(fs::read(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv")).unwrap(), b"lazymkv-test-mkv");
    assert!(!final_path(&h).exists());
}

#[test]
fn a_partial_whose_duration_differs_fails_file_verify_failed() {
    let h = harness(&["movie-direct-partial-long"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::FileVerifyFailed { .. })));
    let message = record.message.unwrap_or_default();
    assert!(message.contains("duration: expected 7150 s, file 7153.0 s"), "{message}");
    assert!(!final_path(&h).exists());
}

#[test]
fn no_ffprobe_lets_the_disc_phase_finish_and_waits_in_awaiting_tool() {
    let h = harness(&[], json!({"tool_paths": {"ffprobe": null}}));
    let id = run_to(&h, "AwaitingTool");
    assert_eq!(h.record(&id).state, State::AwaitingTool { return_to: Box::new(State::Inspecting) });
    let found = events(&h, &id);
    assert!(found.contains(&Event::Ejected) && found.contains(&Event::ToolMissing { tool: Tool::Ffprobe }), "{found:?}");
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Tool }));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit`
Expected: compile errors naming `SlotQueue`, `partial_path` and `remux_check`.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase`
Expected: compile error, `no method named 'on_spawn' found for struct 'Harness'`.

- [ ] **Step 4: Implement**

1. **owner.rs: the slot.**
   - The owner keeps `SlotQueue::new(Store::settings().0.file_phase_concurrency as usize)`.
   - A job entering `Ejected { after_copy: false }` joins the queue. Then `grant_next` runs, and runs again whenever `after` releases a holder.
   - `grant_next` rule (R24.1), while `next()` is Some:
     - When the head's persisted plan is lookup-pending and no round has run for its current Ejected entry, start `spawn_round` with `done` sending a private `Msg::RoundDone { job, pending }`, and stop.
     - Otherwise `grant`, feed `FilePhaseSlot`, and stop.
   - `RoundDone` for a job in `Ejected`:
     - Pending is `(true, _)` or `(_, true)`: `leave`, feed `LookupPending` with the message `waiting_message(&book)`, then `grant_next`.
     - Otherwise mark the round done, then `grant_next`.
   - `Msg::Timer` for a job in `AwaitingLookup` starts a round. Its `RoundDone` feeds `LookupPending` with `waiting_message(&book)` while anything is pending, else `LookupAnswered`.
   - `spawn_round` writes the `job.log` line `lookup <word>: <sentence>` for each lookup the round leaves `Pending { Unparsed(sentence) }` (R24.6).
   - KeepAwake's "active" gains "or a slot is held".
   - `RefreshIdentity` (R24.2): with the book locked, `refresh_identity(plan, &book.lookup_inputs(chain_inputs(job_dir)?.name.as_ref()), &job.settings)` replaces the record's plan.
   - Entering `Filed` sets `filed_unix = host.now_unix()`. Leaving `Remuxing` releases the Remux promise.
2. **Probe.**
   - For each planned title in plan order: `probe(.., the staged .mkv, <key>/ffprobe-streams.json)`.
   - Results are kept in RealSteps' in-memory map by (job, key).
   - No `ffprobe` gives `ToolMissing { Ffprobe }`. An Err gives `ProbeFailed { Unknown }` with its message. Otherwise `ProbeDone`.
3. **ExtractSubtitleEvidence.** No event. `job.log` gets `step 10a did not run in this version` (S33).
4. **Decide** (Inspecting or Decided).
   1. The plan is the record's. When `identity.lookup_pending`, refresh it as in item 1 and `outbox.edit` the plan.
   2. For each title:
      1. If `step10_runs(&rip_title(scan title, &selection), &removed_scan_streams(Some(rip log), &selection))`, give `DecisionFailed { Unknown }` with the S32 sentence.
      2. Build `engine_decide_inputs(.., LibraryFacts::default())` and render `decide_doc`.
      3. `DecisionEntry::Skipped { skipped }` gives `DecisionFailed { Unknown }` with `skipped`.
      4. When the decision has a path and `library_facts(path, job.settings.edition_style)` differs from the default, render again with those facts.
   3. `same_path_reviews` over the decisions.
   4. Write each `<key>/decide.json` = `dumps(&to_value(EngineDecideDoc { doc, audio_rule_version: RULE_VERSION, lazymkv_version: env!("CARGO_PKG_VERSION") }), INDENT1_ASCII) + "\n"` with `write_atomic`.
   5. `outbox.edit` each title's decision, and the collisions (the sorted, unique paths whose decision has `path_collision`).
   6. Feed `decision_done(&decisions)`.
   - When the job has no in-memory probe (after a restart), the inputs come from `read_decide_inputs` over the job folder. Task 25 pins the `job.log` line for this.
5. **`engine_decide_inputs`**, built as the skeleton's list says:
   - `(rec, key) = rec_folder(ctx, "<abs job dir>/<key>")`.
   - `slug` = job ID, `label` = key, `title` = the planned scan index, `selection: Selection::All`.
   - `scan` = parsed `scan.txt`, `ident` = `<key>/mkvmerge.json`, `rip_log: Some(RipLogText { shown: rec.tree.display(<key>/rip.txt), text })`.
   - `reconcile` = `reconcile(&scan, title, &ident, Selection::All, Some(RipLog { shown, text }))`, whose Err is returned.
   - `audio: vec![]`, `subtitles: no_text_entry()`.
   - `plan` (RP-4): `plan_doc(ctx, &PlanSource::Recording(&rec), &PlanOptions { settings, tmdb_search, tmdb_record, remembered })`. Each option is the absolute job-folder path, given only when its file exists. Ok becomes `PlanEntry::Results` with `.plan` replaced by the decided plan; Err becomes `PlanEntry::Skipped { skipped: refused.line }`.
   - `answers: replay_answers(ctx, &rec, &options)`, with the same `PlanOptions` as `plan`, so the `--tmdb-record` rule (S49) applies as `read_decide_inputs` applies it. Its Err is held in `DecideInputs.answers`, as the reader holds it.
   - `file: FileFacts { level_md5: Ok(vec![]), probes: Ok(Some(probes.clone())), subs_index: Ok(None), packets: BTreeMap::new() }`.
   - `library` as given, `overrides: None`.
   - The engine's `Ctx` is `Ctx::from_process()`.
6. **CheckSpace in Decided.** `remux_check(remux_promise(sum of the staged .mkv sizes), movies_root, ..)`.
   - An unreachable root or a `free_bytes` Err gives `ShareLost`.
   - `free < bytes + promised(volume, job)` gives `SpaceShort`.
   - Otherwise promise Remux and give `SpaceOk`.
7. **Remux.**
   - For each title in plan order:
     1. `create_dir_all(<root>/.lazymkv-partial)` (Review Focus 3).
     2. The staged report is `verify_rip(scan title, &<key>/mkvmerge.json read as MkvmergeIdent>, &selected_streams(scan, title, Selection::All)?, Some(rip log), disc.is_dvd)`, the call VerifyFile in Ripping made, read from the job folder so that it survives a restart.
     3. `remux(.., &remux_args(&partial_path(..), &decision.layout, &staged, &raw)?, <key>/remux-mkvmerge.txt)`, with progress forwarded as `progress(Some(key), f)`.
   - No `mkvmerge` gives `ToolMissing { Mkvmerge }`. A `remux_args` Err, or a `remux` Err, gives `RemuxFailed { Unknown }` with that text. Otherwise `RemuxDone`.
8. **VerifyFile in Remuxing.**
   - For each title: `ident(partial, <key>/partial-mkvmerge.json)`, then `verify_final(&decision.layout, scan title, &verify_rip(..staged..), &ident)`.
   - Any mismatch gives `FileVerifyFailed { Unknown }`, with the mismatches joined by "; ". Otherwise `FileVerified`.
9. **File.**
   - For each title: `create_dir_all(final's parent)`, then `rename_no_replace(partial, final)`.
     - `AlreadyExists` gives `FileFailed { Unknown }` with `<final> already exists; the partial file stays at <partial>`, and `outbox.edit` adds the collision.
     - Ok records `FiledFile { path, size, mtime_unix }` from the filed file's metadata.
   - All titles filed gives `FileDone`.
   - **DeletePartialFile** removes each title's partial file (NotFound ignored) and returns no event.
10. **testkit `on_spawn`.** `TestGuard` runs a registered action once, before the first spawn whose args contain every needle, then spawns.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit`
Expected: PASS, with 8 new tests (7 in `file_phase::tests`, 1 in `space::tests`). Task 22's owner tests pass unchanged: their scripts end with a failing Probe and they wait on persisted transitions, so the slot does not affect them.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase`
Expected: PASS, 20 tests.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase`
Expected: PASS, 31 tests, unchanged. `run_to` waits on a persisted transition into its state (Task 23), so a job that moves on into the file phase at once still passes.

- [ ] **Step 6: Commit**

```
git add crates/engine/src crates/fake-tools/tests/engine_file_phase.rs corpus/scenarios
git commit -m "engine: the file phase, from the slot to the filed file (design parts 5.7, 6.4, 6.5)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, the noreply identity twice, and no attribution line.

---

### Task 25: Restarts (design part 5.10, L1, L1a)

**Files:**
- Create: `crates/engine/src/restart.rs`, with its unit tests in `mod tests` (12 tests).
- Create: `crates/fake-tools/tests/engine_restart.rs` (13 tests).
- Create: `corpus/scenarios/movie-direct-remux-hang/`, holding only `mkvmerge.json`: the base's three responses, the `["--gui-mode","-o"]` one with `"hang_after_line":0`.
- Modify: `crates/engine/src/owner.rs`, where Task 22 defines `start` and `start_until`, so that `start` runs the restart sequence before it returns, through the owner's stop hook, with the token rebuilt.
- Modify: `crates/engine/src/lib.rs`, adding `mod restart;` in Step 1 beside the new file.
- Modify: `crates/engine/src/steps.rs`, so that Decide reads its inputs back when memory holds none, and File takes an earlier rename as done.
- Modify: `crates/engine/src/file_phase.rs`, so that the slot is rebuilt.
- Modify: `crates/engine/src/space.rs`, so that promises are rebuilt.
- Modify: `crates/engine/src/store.rs`, so that `persist` first writes `\n` when `transitions.jsonl` does not end in one (RP-8).

**Interfaces:**
- Consumes:
  - `pub fn resume(state: &State, disc_present: bool) -> (State, Vec<Action>)` (crates/core/src/job.rs:732). Ripping gives `[EmptyTitleDirectory, RipTitle]`. Remuxing gives `[DeletePartialFile, Remux]`. Verified gives `[File]`. Decided gives `[Decide]`. Inspecting gives `[Probe]`. Ejected gives nothing. AwaitingLookup gives `[StartTimer { minutes: LOOKUP_RETRY_MIN }]`. A disc-phase state with `disc_present` false gives `[WaitForDisc]`.
  - `Event::EngineRestart { disc_present: bool }` (job.rs:576). `reduce` answers it in every state with `resume` (job.rs:791).
  - `pub const LOOKUP_RETRY_MIN: u32 = 60` (job.rs:20). `State::is_disc_phase`, `is_file_phase` and `is_waiting` (job.rs:289 onward).
  - `core::record::{JobRecord, Transition, DrivesFile, DriveRecord, JOB_FORMAT}` (Task 6). `core::profile::JOB_PROFILE_FILE` (Task 4).
  - `clients::{LookupBook, LookupState, Lookup, Answers, PendingReason}` (Task 16).
  - `core::lookup::discdb::{parse_dischash, parse_discid}` (discdb.rs:328, :345) and `core::lookup::tmdb::{parse_search, parse_movie}` (tmdb.rs:131, :278).
  - `lazymkv_dev::cmd::decide::{read_decide_inputs, rec_folder, decide_doc}` and `lazymkv_dev::cmd::plan::PlanOptions` (Tasks 19 and 20).
  - `Promises::promise`, `PromisedStep`, `volume_of`, `rip_promise` and `remux_promise` (Task 23). The file-phase slot of Task 24.
  - `testkit::{Harness, StepScript}` (Tasks 22 and 23), as the sheet defines them: `run_until(&self, state)` stops the engine after it persists the first transition into `state` and before that transition's actions run; `restart(&self)` starts a new engine over the same data folder, fakes, drive script and steps; `new_scripted(Vec<(&'static str, StepScript)>, settings)`; `insert`, `wait`, `settle`, `job_ids`, `state_of`, `job_dir`, `data`, `staging`, `library`, `shares`, `ejector`, `scan_waits_for_lookups` and `fire_timers`.
- Produces (all private to the engine crate):

```rust
/// What the engine does at start (design part 5.10, L1, L1a): which job takes the disc-phase token, the
/// EngineRestart each job is fed, the discs that are arrivals and the slot. Def: T25. Use: T25, T26.
pub(crate) struct RestartPlan {
    pub token: Option<String>,          // the oldest disc-phase job with its disc in a drive (part 5.4 way (c))
    pub feeds: Vec<(String, bool)>,     // (job id, disc_present), oldest job first
    pub arrivals: Vec<String>,          // drive ids whose disc is an arrival (L1), in drive-id order
    pub noted: Vec<String>,             // drive ids whose disc only has its presence noted (the token job's; L1a)
    pub slot_holders: Vec<String>,      // jobs that hold the file-phase slot (design part 5.7)
    pub slot_queue: Vec<String>,        // Ejected { after_copy: false } jobs, oldest first
}
/// `present`: each drive with a disc, and its fingerprint (None when the walk failed). Def: T25. Use: T25.
pub(crate) fn plan_restart(jobs: &[JobRecord], present: &[(String, Option<String>)], drives: &DrivesFile) -> RestartPlan;
/// The promise a job in `state` holds across a restart: Ripping holds Rip, Remuxing holds Remux, every other
/// state none (design parts 5.8, 5.10). Def: T25. Use: T25.
pub(crate) fn held_promise(state: &State) -> Option<PromisedStep>;
/// A job's LookupBook read back from its folder's lookup-status.txt and saved answers (design part 8.3). Def: T25. Use: T25, T26.
pub(crate) fn rebuild_book(job_dir: &Path) -> LookupBook;
/// The job.log line a Decide writes when it read its inputs back (design part 5.10). Def: T25. Use: T25.
pub(crate) const READ_BACK_LINE: &str = "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)";
```

**Rulings** (S70 to S75):
- **Ruling S70 (the restart sequence).** `start` finishes the restart sequence before it returns the handle. The sequence is:
  1. Load the jobs. Refused formats are reported, as in Task 21.
  2. Read the settings and `drives.json`.
  3. Fingerprint each disc that `DriveWatcher::present()` lists, using Task 22's arrival fingerprint code.
  4. Call `plan_restart`.
  5. Persist one `EngineRestart` transition per entry of `feeds`.
  6. Rebuild the books, the promises, the token and the slot.
  7. Hand the resume actions and then the arrivals to the owner thread.

  Why: a test that calls `restart()` can then read the `EngineRestart` lines at once. EngineRestart starts no job, so it is fed even while the settings have a problem. The arrivals then wait as design part 9.4 says.

  Each `EngineRestart` transition goes through `start_until`'s stop hook as the owner's transitions do (Task 22), and a true answer stops the engine before any resume action runs.
- **Ruling S71 (the token job's drive).** The token job's own drive goes in `noted` and not in `arrivals`. Design part 5.4 excepts that disc from the arrivals.
- **Ruling S72 (the file-phase slot).** These jobs hold the slot:
  - a job in Inspecting, Decided, Remuxing or Verified;
  - a job in a waiting state whose `return_to` is one of those four states.

  Ejected jobs queue for the slot. A waiting job whose `return_to` is Ejected holds no slot. Why: design part 5.7 says the slot is "held until Filed, NeedsReview, Failed or Discarded, including any wait entered after the grant".
- **Ruling S73 (the rebuilt book).** `rebuild_book` maps each `lookup-status.txt` line to a lookup by its first word, using S21's words: `dischash`, `discid`, `search`, and `tmdb` for `tmdb movie/<id>`.
  - Last word `200` and the saved file parses: `Answered { status: 200 }`, with the parsed answer in `answers`. The files are `thediscdb-hash.json` (`parse_dischash`), `thediscdb-discid.json` (`parse_discid`), `tmdb-search.json` (`parse_search`) and `tmdb-record.json` (`parse_movie`; PR 2 is movie only, S19).
  - Last word `200` and the file fails to parse: `Pending { Unparsed(<the parser's sentence>) }`.
  - Last word `404`: `Answered { status: 404 }`.
  - `000`: `Pending { Unreachable("no answer before the restart") }`.
  - `unparsed`: `Pending { Unparsed(<the parser's sentence on reading the file again>) }`.
  - Any other number: `Pending { Status(n) }`.
  - No line: `NotAsked`.

  Every job fed EngineRestart gets a rebuilt book. Why: `resume` gives Scanned a Plan and AwaitingLookup a timer, and both read the book.
- **Ruling S74 (reading Decide's inputs back).** A Decide that finds no `DecideInputs` for (job, key) in memory reads them back with `read_decide_inputs`. The call takes:
  - the `Ctx` and `rec_folder("<absolute job dir>/<key>")` that Task 24's Decide uses;
  - `text: None`;
  - `selection: Some(Selection::All)`;
  - the same `PlanOptions` as Task 24's Decide, which carries the four replay paths, each only when its file exists.

  The Decide then writes `<key>: ` + `READ_BACK_LINE` to `job.log` and renders with `decide_doc` as Task 24 does. Why: design part 5.10 asks for dev's readers. In PR 2 there is no staged tuning folder, and the bare-`streamhash.txt` reading is PR 5.
- **Ruling S75 (rebuilt promises).** For each job where `held_promise(&record.state)` is `Some(step)`, the promise is rebuilt with `Promises::promise(job, step, volume, bytes)`, through Task 23's functions (S54): `Rip` is `volume_of(staging_root)` and `rip_promise(plan, meta.json total_bytes)`; `Remux` is `volume_of(movies_root)` and `remux_promise(<the staged .mkv sizes summed>)`.

RP-7 settles this: (a) the verified size that design part 6.5 checks after a restart in Verified ("if the partial file is gone and the final path holds a file of the verified size") lives in `TitleRecord.filed`, which Task 24's VerifyFile in Remuxing sets to `Some(FiledFile { path: <final path>, size: <partial size>, mtime_unix: 0 })` and File then overwrites with the real mtime, so no sheet type changes.

RP-8 settles this: (a) a crash in the middle of an append can leave the last line of `transitions.jsonl` without its newline, so Task 21's `Store::persist` first writes `\n` when the file does not end in one (Step 4), with the test `a_torn_last_line_gets_its_newline_before_the_next_append` beside the lost-whole-line test of Review Focus 4.

- [ ] **Step 1: Write the failing unit tests in `crates/engine/src/restart.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::job::{FailureClass, Held, State};
    use lazymkv_core::plan::Mode;
    use lazymkv_core::profile::JOB_PROFILE_FILE;
    use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, DRIVES_FORMAT, JOB_FORMAT};
    use lazymkv_core::testsupport::{search_json, Scratch, ScratchExt};
    use std::collections::BTreeMap;

    fn record(id: &str, fingerprint: &str, created: u64, state: State) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(), id: id.to_string(), fingerprint: fingerprint.to_string(), disc_id: None,
            drive: "tok0".to_string(), created_unix: created, updated_unix: created, state, message: None,
            rip_again: false, profile: JOB_PROFILE_FILE.to_string(), plan: None, titles: vec![], filed_unix: None,
            collisions: vec![], notes: vec![],
        }
    }

    fn drives(entries: &[(&str, Option<&str>, bool)]) -> DrivesFile {
        DrivesFile {
            format: DRIVES_FORMAT.to_string(),
            drives: entries
                .iter()
                .map(|(id, fp, handled)| (id.to_string(), DriveRecord { fingerprint: fp.map(str::to_string), handled: *handled }))
                .collect::<BTreeMap<_, _>>(),
            drive_incapable: vec![],
        }
    }

    fn present(entries: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        entries.iter().map(|(d, fp)| (d.to_string(), fp.map(str::to_string))).collect()
    }

    fn back(state: State) -> Box<State> { Box::new(state) }

    #[test]
    fn the_oldest_disc_phase_job_with_its_disc_present_takes_the_token() {
        let jobs = [record("bbb-1", "BBB", 20, State::Scanned), record("aaa-1", "AAA", 10, State::Ripping)];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("BBB")), ("tok1", Some("AAA"))]), &drives(&[]));
        assert_eq!(plan.token.as_deref(), Some("aaa-1"));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("bbb-1".to_string(), false)]);
        assert_eq!(plan.noted, vec!["tok1".to_string()]);
        assert_eq!(plan.arrivals, vec!["tok0".to_string()]);
    }

    #[test]
    fn other_disc_phase_jobs_wait_for_their_disc() {
        let jobs = [record("aaa-1", "AAA", 10, State::Ripping), record("ccc-1", "CCC", 30, State::Planned { mode: Mode::Direct })];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("AAA"))]), &drives(&[]));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("ccc-1".to_string(), false)]);
    }

    #[test]
    fn a_disc_phase_job_without_its_disc_leaves_the_token_to_the_next_oldest() {
        let jobs = [record("aaa-1", "AAA", 10, State::Ripping), record("bbb-1", "BBB", 20, State::Fingerprinted)];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("BBB"))]), &drives(&[]));
        assert_eq!(plan.token.as_deref(), Some("bbb-1"));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), false), ("bbb-1".to_string(), true)]);
        assert!(plan.arrivals.is_empty());
    }

    #[test]
    fn filed_discarded_held_and_failed_jobs_are_not_fed() {
        let jobs = [
            record("aaa-1", "AAA", 10, State::Filed),
            record("bbb-1", "BBB", 20, State::Discarded),
            record("ccc-1", "CCC", 30, State::NeedsReview { held: Held::DispositionGate }),
            record("ddd-1", "DDD", 40, State::Failed { class: FailureClass::Unknown, from: back(State::Ripping) }),
        ];
        let plan = plan_restart(&jobs, &present(&[]), &drives(&[]));
        assert!(plan.feeds.is_empty());
        assert_eq!(plan.token, None);
    }

    #[test]
    fn a_waiting_jobs_disc_present_says_whether_a_drive_holds_its_fingerprint() {
        let jobs = [
            record("aaa-1", "AAA", 10, State::AwaitingSpace { return_to: back(State::Planned { mode: Mode::Direct }) }),
            record("bbb-1", "BBB", 20, State::AwaitingLookup { return_to: back(State::Ejected { after_copy: false }) }),
        ];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("AAA"))]), &drives(&[("tok0", Some("AAA"), true)]));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("bbb-1".to_string(), false)]);
        assert_eq!(plan.token, None);
        assert_eq!(plan.noted, vec!["tok0".to_string()]);
    }

    #[test]
    fn a_disc_already_in_a_drive_is_an_arrival_at_start() {
        let plan = plan_restart(&[], &present(&[("tok1", Some("CCC")), ("tok0", None)]), &drives(&[]));
        assert_eq!(plan.arrivals, vec!["tok0".to_string(), "tok1".to_string()]);
    }

    #[test]
    fn a_handled_drive_record_is_not_an_arrival_again() {
        let plan = plan_restart(&[], &present(&[("tok0", Some("CCC"))]), &drives(&[("tok0", Some("CCC"), true)]));
        assert!(plan.arrivals.is_empty());
        assert_eq!(plan.noted, vec!["tok0".to_string()]);
    }

    #[test]
    fn a_queued_disc_or_another_disc_in_a_recorded_drive_is_an_arrival_again() {
        let queued = plan_restart(&[], &present(&[("tok0", Some("CCC"))]), &drives(&[("tok0", Some("CCC"), false)]));
        assert_eq!(queued.arrivals, vec!["tok0".to_string()]);
        let other = plan_restart(&[], &present(&[("tok0", Some("DDD"))]), &drives(&[("tok0", Some("CCC"), true)]));
        assert_eq!(other.arrivals, vec!["tok0".to_string()]);
    }

    #[test]
    fn the_file_phase_slot_is_rebuilt_from_persisted_states() {
        let jobs = [
            record("aaa-1", "AAA", 10, State::Remuxing),
            record("bbb-1", "BBB", 20, State::Ejected { after_copy: false }),
            record("ccc-1", "CCC", 5, State::Ejected { after_copy: false }),
            record("ddd-1", "DDD", 40, State::AwaitingLookup { return_to: back(State::Ejected { after_copy: false }) }),
            record("eee-1", "EEE", 50, State::AwaitingSpace { return_to: back(State::Decided { approved: None }) }),
        ];
        let plan = plan_restart(&jobs, &present(&[]), &drives(&[]));
        assert_eq!(plan.slot_holders, vec!["aaa-1".to_string(), "eee-1".to_string()]);
        assert_eq!(plan.slot_queue, vec!["ccc-1".to_string(), "bbb-1".to_string()]);
    }

    #[test]
    fn space_promises_are_rebuilt_from_persisted_states() {
        assert_eq!(held_promise(&State::Ripping), Some(PromisedStep::Rip));
        assert_eq!(held_promise(&State::Remuxing), Some(PromisedStep::Remux));
        for state in [
            State::Planned { mode: Mode::Direct }, State::RipVerified { after_copy: false }, State::Ejected { after_copy: false },
            State::Verified, State::AwaitingSpace { return_to: back(State::Planned { mode: Mode::Direct }) },
        ] {
            assert_eq!(held_promise(&state), None, "{}", state.as_str());
        }
    }

    #[test]
    fn a_lookup_book_is_rebuilt_from_lookup_status_and_the_saved_files() {
        let s = Scratch::new("book");
        s.write("lookup-status.txt", "dischash 404\ndiscid 000\nsearch 200\ntmdb movie/900001 503\n");
        s.write("thediscdb-hash.json", "");
        s.write("tmdb-search.json", search_json(&[("movie", 900001, "Synthetic Movie", "Synthetic Movie", "2001-02-03")]));
        let book = rebuild_book(&s.0);
        assert!(matches!(book.states[&Lookup::Dischash], LookupState::Answered { status: 404 }));
        assert!(matches!(book.states[&Lookup::Discid], LookupState::Pending { reason: PendingReason::Unreachable(_) }));
        assert!(matches!(book.states[&Lookup::Search], LookupState::Answered { status: 200 }));
        assert!(matches!(book.states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Status(503) }));
        assert_eq!(book.answers.search.as_ref().unwrap().results.len(), 1);
        assert!(book.answers.by_fingerprint.is_none());
    }

    #[test]
    fn a_200_whose_saved_file_does_not_parse_is_pending_and_no_line_is_not_asked() {
        let s = Scratch::new("book-unparsed");
        s.write("lookup-status.txt", "search 200\n");
        s.write("tmdb-search.json", "{not json");
        let book = rebuild_book(&s.0);
        assert!(matches!(book.states[&Lookup::Search], LookupState::Pending { reason: PendingReason::Unparsed(_) }));
        assert!(book.states.get(&Lookup::Dischash).map_or(true, |state| matches!(state, LookupState::NotAsked)));
    }
}
```

- [ ] **Step 2: Write the failing stand-in tests in `crates/fake-tools/tests/engine_restart.rs`**

```rust
//! Restarts over the stand-in tools (design part 5.10, L1, L1a; Ruling S12).

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::job::{Action, Event, FailureClass, Held, State};
use lazymkv_core::record::{JobRecord, Transition};
use lazymkv_engine::testkit::Harness;
use serde_json::json;

/// The disc of the base scenario, as Task 22's Harness::insert names it.
const DISC: &str = "movie-direct";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";

fn settings() -> serde_json::Value {
    json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1})
}

fn harness(scenarios: &[&str], settings: serde_json::Value) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings)
}

fn job_ids(h: &Harness) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(h.data().join("jobs")).map_or(vec![], |dir| {
        dir.map(|e| e.unwrap().file_name().into_string().unwrap()).collect()
    });
    ids.sort();
    ids
}

fn only_job(h: &Harness) -> String {
    let ids = job_ids(h);
    assert_eq!(ids.len(), 1, "{ids:?}");
    ids[0].clone()
}

fn transitions(dir: &Path) -> Vec<Transition> {
    fs::read_to_string(dir.join("transitions.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn state_json(dir: &Path) -> JobRecord {
    serde_json::from_slice(&fs::read(dir.join("state.json")).unwrap()).unwrap()
}

fn final_path(h: &Harness) -> PathBuf { h.library().join(FINAL) }

#[test]
fn a_restart_in_each_state_of_the_path_still_reaches_filed() {
    for state in ["Detected", "Fingerprinted", "Scanned", "Planned", "Ripping", "RipVerified", "Ejected", "Inspecting",
                  "Decided", "Remuxing", "Verified"] {
        let h = harness(&["movie-direct"], settings());
        h.insert("tok0", DISC);
        h.run_until(state);
        h.restart();
        h.run_until("Filed");
        let dir = h.job_dir(&only_job(&h));
        let lines = transitions(&dir);
        assert!(lines.iter().any(|t| matches!(t.event, Some(Event::EngineRestart { .. }))), "{state}");
        assert_eq!(lines.last().unwrap().new, State::Filed, "{state}");
        assert_eq!(fs::read(final_path(&h)).unwrap().len(), 16, "{state}");
        assert!(fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().is_none(), "{state}");
    }
}

#[test]
fn a_restart_after_a_lost_last_transition_line_resumes_from_state_json_and_appends_after_it() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let dir = h.job_dir(&only_job(&h));
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    let kept: Vec<&str> = text.lines().collect::<Vec<_>>()[..text.lines().count() - 1].to_vec();
    let kept_text: String = kept.iter().map(|l| format!("{l}\n")).collect();
    fs::write(dir.join("transitions.jsonl"), &kept_text).unwrap();
    assert_eq!(state_json(&dir).state, State::Ripping);

    h.restart();
    h.run_until("Filed");
    let after = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    assert!(after.starts_with(&kept_text));
    let lines = transitions(&dir);
    let resumed = &lines[kept.len()];
    assert_eq!(resumed.event, Some(Event::EngineRestart { disc_present: true }));
    assert_eq!(resumed.old, Some(State::Ripping));
    assert_eq!(resumed.new, State::Ripping);
    assert_eq!(resumed.actions, vec![Action::EmptyTitleDirectory, Action::RipTitle]);
    assert_eq!(lines.last().unwrap().new, State::Filed);
}

/// RP-8: the next append first restores a torn last line's newline, so every line still parses.
#[test]
fn a_torn_last_line_gets_its_newline_before_the_next_append() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let dir = h.job_dir(&only_job(&h));
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    fs::write(dir.join("transitions.jsonl"), text.strip_suffix('\n').unwrap()).unwrap();

    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("transitions.jsonl")).unwrap().starts_with(&text));
    // `transitions` parses every line, so a joined line fails here.
    assert_eq!(transitions(&dir).last().unwrap().new, State::Filed);
}

#[test]
fn a_held_job_stays_held_after_a_restart() {
    let h = harness(&["movie-direct"], settings());
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::write(final_path(&h), b"owner's").unwrap();
    h.insert("tok0", DISC);
    h.run_until("NeedsReview");
    let dir = h.job_dir(&only_job(&h));
    let before = fs::read(dir.join("transitions.jsonl")).unwrap();
    h.restart();
    assert_eq!(fs::read(dir.join("transitions.jsonl")).unwrap(), before);
    assert_eq!(state_json(&dir).state, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(fs::read(final_path(&h)).unwrap(), b"owner's");
}

#[test]
fn a_disc_left_in_after_a_copy_mode_failure_starts_nothing_at_a_restart() {
    let h = harness(&["movie-direct", "movie-direct-copy-plan"], settings());
    h.insert("tok0", DISC);
    h.run_until("Failed");
    let id = only_job(&h);
    let before = fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap();
    let ejects = h.ejector().calls().len();
    h.restart();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap(), before);
    assert_eq!(h.ejector().calls().len(), ejects);
    let drives: serde_json::Value = serde_json::from_slice(&fs::read(h.data().join("drives.json")).unwrap()).unwrap();
    assert_eq!(drives["drives"]["tok0"]["handled"], json!(true));
}

#[test]
fn a_disc_left_in_after_a_failed_eject_starts_nothing_at_a_restart() {
    let h = harness(&["movie-direct"], settings());
    h.ejector().fail_next("stuck");
    h.insert("tok0", DISC);
    h.run_until("Filed");
    let id = only_job(&h);
    assert!(transitions(&h.job_dir(&id)).iter().any(|t| t.event == Some(Event::EjectFailed)));
    let before = fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap();
    let ejects = h.ejector().calls().len();
    h.restart();
    h.settle();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap(), before);
    assert_eq!(h.ejector().calls().len(), ejects);
    let drives: serde_json::Value = serde_json::from_slice(&fs::read(h.data().join("drives.json")).unwrap()).unwrap();
    assert_eq!(drives["drives"]["tok0"]["handled"], json!(true));
}

#[test]
fn a_restart_after_a_late_answer_decides_with_the_refreshed_identity() {
    let h = harness(&["movie-direct", "movie-direct-lookup-late"], settings());
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC);
    h.run_until("Inspecting");
    let dir = h.job_dir(&only_job(&h));
    assert!(fs::read_to_string(dir.join("lookup-status.txt")).unwrap().contains("dischash 404"));
    assert!(state_json(&dir).plan.unwrap().identity.lookup_pending, "the refresh before Decide has not run yet");
    h.restart();
    h.run_until("Filed");
    let record = state_json(&dir);
    assert!(!record.plan.unwrap().identity.lookup_pending);
    assert_eq!(Path::new(&record.titles[0].filed.as_ref().unwrap().path), final_path(&h).as_path());
}

#[test]
fn a_rebuilt_remux_promise_counts_against_another_jobs_rip_check() {
    // One volume for staging and the library. A's rip check needs 4301 + 4301 (S54) and passes; after the restart in
    // Remuxing, A's rebuilt Remux promise is write_bytes_needed(16) = 17, so B's identical check needs 8619 > 8610.
    let h = harness(&["movie-direct", "movie-direct-remux-hang"], settings());
    let (staging, movies) = (h.staging().to_str().unwrap().to_string(), h.library().to_str().unwrap().to_string());
    h.shares().same_volume(&staging, &movies);
    h.shares().set_free(&staging, 8610);
    h.shares().set_free(&movies, 8610);
    h.insert("tok0", DISC);
    h.run_until("Remuxing");
    let a = only_job(&h);
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", "disc-b");
    h.wait("B waits for space", |h| h.job_ids().iter().any(|id| *id != a && h.state_of(id) == "AwaitingSpace"));
    assert_eq!(h.state_of(&a), "Remuxing");
}

#[test]
fn a_disc_already_in_a_drive_is_an_arrival_at_start() {
    let h = harness(&["movie-direct"], json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                                   "drive_ready_timeout_s": 1, "file_phase_concurrency": 0}));
    h.insert("tok0", DISC);
    h.restart(); // the settings problem keeps the arrival waiting; no job folder exists
    assert!(job_ids(&h).is_empty());
    let path = h.data().join("settings.json");
    let mut stored: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    stored["file_phase_concurrency"] = json!(1);
    fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    h.restart();
    h.run_until("Filed");
    assert_eq!(job_ids(&h).len(), 1);
}

#[test]
fn a_restart_in_verified_with_the_file_already_filed_feeds_file_done() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::rename(&partial, final_path(&h)).unwrap();
    h.restart();
    h.run_until("Filed");
    let lines = transitions(&h.job_dir(&id));
    let n = lines.len();
    assert_eq!(lines[n - 2].event, Some(Event::EngineRestart { disc_present: false }));
    assert_eq!(lines[n - 1].event, Some(Event::FileDone));
    let record = state_json(&h.job_dir(&id));
    let filed = record.titles[0].filed.as_ref().unwrap();
    assert_eq!(Path::new(&filed.path), final_path(&h).as_path());
    assert_eq!(filed.size, 16);
    assert!(record.filed_unix.is_some());
}

#[test]
fn a_restart_in_verified_with_another_size_at_the_final_path_fails_as_unknown() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::remove_file(partial).unwrap();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::write(final_path(&h), b"abc").unwrap();
    h.restart();
    h.run_until("Failed");
    let last = transitions(&h.job_dir(&id)).pop().unwrap();
    assert!(matches!(last.event, Some(Event::FileFailed { failure }) if failure.class == FailureClass::Unknown));
    assert_eq!(fs::read(final_path(&h)).unwrap(), b"abc");
}

#[test]
fn a_decide_after_restart_reads_its_inputs_back_with_devs_readers() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Decided");
    let dir = h.job_dir(&only_job(&h));
    let key = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).find(|p| p.join("decide.json").is_file()).unwrap();
    let first = fs::read(key.join("decide.json")).unwrap();
    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("job.log")).unwrap().contains(
        "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)"));
    assert_eq!(fs::read(key.join("decide.json")).unwrap(), first);
}

#[test]
fn awaiting_lookup_recreates_its_timer() {
    use lazymkv_engine::testkit::StepScript;
    use lazymkv_engine::StepResult;
    let steps = vec![
        ("RunScan", StepScript::Result(StepResult { event: Some(Event::ScanDone), message: None })),
        ("Plan", StepScript::Result(StepResult { event: Some(Event::PlanDone { mode: lazymkv_core::plan::Mode::Direct }), message: None })),
        ("CheckSpace", StepScript::Result(StepResult { event: Some(Event::SpaceOk), message: None })),
        ("RipTitle", StepScript::Result(StepResult { event: Some(Event::TitleRipped), message: None })),
        ("VerifyFile", StepScript::Result(StepResult { event: Some(Event::TitleVerified { last: true }), message: None })),
        ("Eject", StepScript::Result(StepResult { event: Some(Event::Ejected), message: None })),
        ("Probe", StepScript::Result(StepResult { event: Some(Event::ProbeDone), message: None })),
        ("ExtractSubtitleEvidence", StepScript::Result(StepResult { event: None, message: None })),
        ("Decide", StepScript::Result(StepResult { event: Some(Event::LookupPending), message: None })),
    ];
    let h = Harness::new_scripted(steps, settings());
    h.insert("tok0", DISC);
    h.run_until("AwaitingLookup");
    let dir = h.job_dir(&only_job(&h));
    h.restart();
    let restart = transitions(&dir).pop().unwrap();
    assert_eq!(restart.event, Some(Event::EngineRestart { disc_present: false }));
    assert_eq!(restart.actions, vec![Action::StartTimer { minutes: 60 }]);
    let before = transitions(&dir).len();
    h.fire_timers();
    let after = std::time::Instant::now();
    while transitions(&dir).len() == before && after.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::yield_now();
    }
    let fired = transitions(&dir).pop().unwrap();
    assert!(matches!(fired.old, Some(State::AwaitingLookup { .. })));
    assert!(matches!(fired.event, Some(Event::LookupPending | Event::LookupAnswered)));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit restart::`

Expected: compile error, `plan_restart`, `held_promise`, `rebuild_book` and `RestartPlan` not found.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_restart`

Expected: `test result: FAILED. 3 passed; 10 failed`. These 10 fail, each on a wait that times out or with no `EngineRestart` line, because the engine of Task 22 starts nothing at start:
- `a_restart_in_each_state_of_the_path_still_reaches_filed`;
- `a_restart_after_a_lost_last_transition_line_resumes_from_state_json_and_appends_after_it`;
- `a_torn_last_line_gets_its_newline_before_the_next_append`;
- `a_restart_after_a_late_answer_decides_with_the_refreshed_identity`;
- `a_rebuilt_remux_promise_counts_against_another_jobs_rip_check`;
- `a_disc_already_in_a_drive_is_an_arrival_at_start`;
- `a_restart_in_verified_with_the_file_already_filed_feeds_file_done`;
- `a_restart_in_verified_with_another_size_at_the_final_path_fails_as_unknown`;
- `a_decide_after_restart_reads_its_inputs_back_with_devs_readers`;
- `awaiting_lookup_recreates_its_timer`.

These 3 pass already, for the same reason, and pin that Task 25 keeps it so: `a_held_job_stays_held_after_a_restart`, `a_disc_left_in_after_a_copy_mode_failure_starts_nothing_at_a_restart` and `a_disc_left_in_after_a_failed_eject_starts_nothing_at_a_restart`.

- [ ] **Step 4: Implement `crates/engine/src/restart.rs` and wire it in**

- `plan_restart`:
  1. Sort the jobs by (`created_unix`, `id`).
  2. Skip `Filed`, `Discarded`, `NeedsReview` and `Failed`.
  3. The token goes to the first disc-phase job whose `fingerprint` equals a present disc's fingerprint. That job is fed `true`, and every other disc-phase job is fed `false`. Every other fed job gets `true` exactly when some present disc carries its fingerprint.
  4. Sort `present` by drive id. The token job's drive goes to `noted`. A drive whose `drives.json` record has an equal `fingerprint` and `handled: true` also goes to `noted`. Every other drive is an arrival.
  5. Slot: Ruling S72.
- `held_promise`: a `match`, per the doc.
- `rebuild_book`: Ruling S73. It reads files with `std::fs`, because the engine is not core.
- `owner.rs::start` (and `start_until`): Ruling S70. The restart transitions go through Task 21's `Store::persist`, so `state.json` is replaced before each line is appended (design part 7.3).
- `store.rs`, `persist` (RP-8): before it appends, it reads the last byte of a non-empty `transitions.jsonl` and writes `\n` first when that byte is not `\n`. Its `///` doc gains that sentence.
- `steps.rs`, Decide: Ruling S74.
- `steps.rs`, VerifyFile in Remuxing (RP-7): when every title's partial file passes `verify_final`, `outbox.edit` sets each title's `filed` to `Some(FiledFile { path: <its final path>, size: <the partial file's size from fs::metadata>, mtime_unix: 0 })` before the step gives `FileVerified`. Task 24's File overwrites it with the filed file's metadata.
- `steps.rs`, File: when the partial file is missing and the final path holds a file whose size equals the verified size, `record.titles[i].filed`'s `size`, record `FiledFile` from that file's metadata and return `FileDone`. Any other mismatch, or a `filed` of None, keeps Task 24's `FileFailed { Unknown }`.
- `space.rs`: Ruling S75.
- `file_phase.rs`: seed the holders and the FIFO queue from `RestartPlan`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit restart::`

Expected: PASS, 12 tests.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_restart`

Expected: PASS, 13 tests. The earlier stand-in tests are unchanged:
- Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase --test engine_file_phase`
- Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/engine/src crates/fake-tools/tests/engine_restart.rs corpus/scenarios/movie-direct-remux-hang
git commit -m "engine: restarts through resume, with L1 and L1a (design part 5.10)"
```

Check: `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>`.

### Task 26: The IPC server, the notifier sink and the self-test (design parts 3.7, 3.8, 9.1 to 9.4)

**Files:**
- Create: `crates/engine/src/ipc_server.rs`, with unit tests in `mod tests` (6 tests).
- Create: `crates/engine/src/selftest.rs`, with unit tests in `mod tests` (6 tests).
- Create: `crates/fake-tools/tests/engine_ipc.rs` (47 tests).
- Create: `corpus/scenarios/movie-direct-licence-update/`. It holds `makemkvcon64.json`, `drive-list-v1.txt`, `drive-list-v2.txt` and `scan-licence.txt`, all synthetic.
- Modify `corpus/scenarios/movie-direct/` for the version calls of S80 (Step 1): `mkvmerge.json` and `ffprobe.json` gain a response each; `ffmpeg.json`, `java.json`, `mkvmerge-version.txt`, `ffprobe-version.txt`, `ffmpeg-version.txt` and `java-version.txt` are new.
- Modify:
  - `crates/engine/src/lib.rs`: `mod ipc_server; mod selftest;` (declared in Step 2 with the new files), start the IPC server and run the MakeMKV check at start;
  - `crates/engine/src/owner.rs`: requests, `job_update` and `progress` pushes;
  - `crates/engine/src/steps.rs`: each drive listing feeds the MakeMKV state;
  - `crates/engine/src/notify.rs`: the MediaReadError suffix;
  - `crates/engine/src/testkit.rs`: add `Harness::client() -> MemoryConnector`, and the variant `StepScript::After(Duration, StepResult)`, which sleeps for the duration and then returns the result whether or not the step was cancelled. It stands for a CheckSpace or Eject that cancel does not stop (design part 3.8).

**Interfaces:**
- Consumes:
  - `core::ipc`:
    - `parse_request`, `reply_ok`, `reply_error`, `push_line`, `read_reply` and `job_summary`;
    - `Envelope`, `Request`, `Reply`, `Push`, `JobSummary`, `JobUpdate`, `Snapshot`, `SecretState`, `Progress`, `SelfTest` and `SelfTestCheck`;
    - `MAX_LINE_BYTES`, `LINE_TOO_LONG`, `HISTORY_PAGE`, `CANCELLED`, `SECRET_NAMES` and `is_secret_name` (Task 7).
  - `platform`: `IpcTransport::accept() -> io::Result<IpcStream>`, `IpcStream { reader, writer }`, `SecretStore`, `ShareConnector` and `fakes::{MemoryIpc, MemoryConnector}` (Task 8; `MemoryConnector` derives `Clone` there).
  - `platform::{write_atomic, rename_no_replace}` (Task 9).
  - `tools`:
    - `find_tool`, `ToolLocation`, `Found`, `CancelToken`, `run_recorded`, `ToolCtx`, `LineAction` and `with_suffix` (Task 11);
    - `list_drives` and `DriveListing`, with `summary.version` and `summary.licence` (Task 13).
  - `clients::{HttpTransport, authentication_url, Response}` (Task 15).
  - `Settings::from_json` (settings.rs:377) and `Settings::validate` (:262); `remembered::Overrides` (remembered.rs:301, `Default`).
  - Task 23's shared `java_in_use: Arc<Mutex<Option<bool>>>`, and Task 24's `waiting_message`.
  - `testkit` (the sheet's list, Tasks 22 to 24):
    - `Harness::new(fake_tool, scenarios, settings)` and `Harness::new_scripted(script: Vec<(&'static str, StepScript)>, settings)`;
    - `insert`, `remove`, `run_until`, `restart`, `steps`, `notifications`, `data`, `library`, `staging`, `http`, `shares` and `ejector`. `h.steps().calls()` is the call log; each Action tag takes its entries in order.
  - `Harness::insert(drive, disc_name)`. `DISC_A` is the base scenario's disc. `DISC_B` and `DISC_C` are further discs with their own fingerprints, under the names Task 22's queue tests use. On a stand-in harness they get Task 22's synthetic volume, so their lookups are unrecorded and stay pending, and their scans take the scenario's `makemkvcon64` responses for their drive's index (Task 23).
- Produces (private to the engine crate, except `Harness::client`):

```rust
/// One client's queue length; a full queue drops the client (design part 4.4, S52). Def: T26. Use: T26.
pub(crate) const CLIENT_QUEUE: usize = 256;
/// Every connected client's writer thread (design part 9.2). Def: T26. Use: T26.
pub(crate) struct Clients { /* Mutex<BTreeMap<u64, SyncSender<String>>>, next id */ }
impl Clients {
    pub(crate) fn new() -> Clients;
    pub(crate) fn add(&self, writer: Box<dyn Write + Send>) -> u64; // spawns a thread draining sync_channel(CLIENT_QUEUE)
    pub(crate) fn send(&self, client: u64, line: String);            // try_send; Full or Disconnected removes the client
    pub(crate) fn broadcast(&self, line: &str);                      // send to every client
    pub(crate) fn count(&self) -> usize;
}
/// At most one progress push a second per job (design part 9.2). Def: T26. Use: T26.
pub(crate) struct ProgressGate { /* BTreeMap<String, Instant> */ }
impl ProgressGate { pub(crate) fn new() -> ProgressGate; pub(crate) fn admit(&mut self, job: &str, now: Instant) -> bool; }
/// Reads one line of at most `cap` bytes before its newline; Ok(None) at end of stream, Ok(Some(Err(()))) when longer.
/// Def: T26. Use: T26.
pub(crate) fn read_line_capped(reader: &mut dyn BufRead, cap: usize) -> io::Result<Option<Result<String, ()>>>;
/// The host of a webhook URL, without scheme, user, port or path (design part 9.2). Def: T26. Use: T26.
pub(crate) fn secret_host(url: &str) -> Option<String>;
/// The snapshot's jobs: every unfinished job by (created_unix, id), then the 50 most recently finished by
/// (updated_unix, id) descending (design part 9.2). Def: T26. Use: T26.
pub(crate) fn snapshot_jobs(jobs: &[JobRecord]) -> Vec<JobSummary>;
/// The finished jobs after `before` in that descending order, at most HISTORY_PAGE; Err("no job <id>"). Def: T26. Use: T26.
pub(crate) fn history_page(jobs: &[JobRecord], before: Option<&str>) -> Result<Vec<JobSummary>, String>;
/// MakeMKV's last check and the (version, licence code) it recorded (design part 3.7), kept in `file` across restarts
/// (RP-9). Def: T26. Use: T26.
pub(crate) struct MakeMkvState { pub last: Option<SelfTestCheck>, pub recorded: Option<(Option<String>, Option<u32>)>,
    pub file: Option<PathBuf> }
impl MakeMkvState {
    /// `file` is `<data>/selftest/makemkv-check.json`; `recorded` is read from it, `{"version":..,"licence":..}`,
    /// and stays None when it is absent or unreadable.
    pub(crate) fn open(data: &Path) -> MakeMkvState;
    /// True when (version, licence) differs from a recorded one; records it, and rewrites `file` with write_atomic
    /// whenever `recorded` changes.
    pub(crate) fn observe(&mut self, version: Option<String>, licence: Option<u32>) -> bool;
}
/// The version a tool prints (design part 9.2): the word after "mkvmerge v", "ffprobe version " or "ffmpeg version "
/// on stdout's first line, or for java the text between the first two `"` of stderr's first line; None otherwise.
/// Def: T26. Use: T26.
pub(crate) fn version_in(tool: Tool, stdout: &str, stderr: &str) -> Option<String>;
/// Runs `<program> --version` (mkvmerge) or `-version` (ffprobe, ffmpeg, java) under `limits.header`, recorded at
/// `out` (`<data>/selftest/<tool>-version.txt`, design part 7.2), and reads it with `version_in`. Def: T26. Use: T26.
pub(crate) fn tool_version(ctx: &ToolCtx, tool: Tool, program: &Path, out: &Path) -> Option<String>;
/// The write, rename and delete probe of spec 7 in `root` (in `<root>/.lazymkv-partial` when `library`). Def: T26. Use: T26.
pub(crate) fn root_check(name: &str, root: Option<&str>, library: bool) -> SelfTestCheck;
/// GET authentication_url() with the bearer token; ok on 200 (design part 9.4, S53). Def: T26. Use: T26.
pub(crate) fn token_check(http: &dyn HttpTransport, token: Option<&str>) -> SelfTestCheck;
// testkit (feature `testkit`):
impl Harness { pub fn client(&self) -> MemoryConnector; }  // a clone of the harness's MemoryIpc connector
```

The engine's refusal messages are private constants and functions in `ipc_server.rs`. Ruling S76 pins these texts:

| Case | `error.message` |
|---|---|
| unknown job | `no job <id>` |
| `cancel` with no running step | `job <id> has no running step: it is <State::as_str()>` |
| `cancel` during CheckSpace or Eject | `job <id> is running <Action tag>, which cancel does not stop (design part 9.4)` |
| `review_action` approve outside the gate | `job <id> is <State::as_str()>, not held at the disposition gate` |
| other review action, or approve with non-empty overrides | `<action> waits for phase 3's review commands (R-A8)` |
| `discard` | `discard waits for phase 3's review commands (P2, R-A1)` |
| `ui_quit` | `ui_quit waits for phase 5's user interface (R-A8)` |
| `command` | `command needs the engine's run folder, which arrives in pull request 3 (S35)` |
| `shutdown` with steps running | `a step is running for <ids joined ", ">; stop --force cancels it (design part 3.8)` |
| unknown secret name | `<name> is not a secret name (design part 9.4)` |
| `settings_set` with problems | `settings_set refused: <n> problems` (with `problems`) |
| `settings_set` unreadable | `Settings::from_json`'s error text (with empty `problems`) |
| `history` with unknown `before` | `no job <id>` |

**Further rulings** (S77 to S84):
- **Ruling S77 (snapshot problems and secrets).** `snapshot.problems` is `Settings::validate()`. While `staging_root` is unset, it also lists `{"key":"staging_root","problem":"staging_root is unset: the engine starts no job until it is set (design part 9.4)"}`. Design part 9.4 says "the snapshot shows the problem". `snapshot.secrets` holds every `SECRET_NAMES` entry, set or not (`SecretStore::get` is Some), plus each `share:\\server\share` name a `secret_set` of this engine run has set, since `SecretStore` cannot list its names. `host` is `secret_host(value)` for `ha_webhook_url` and `webhook_url` when set, and None for every other name (design part 9.2).
- **Ruling S78 (when replies are sent).** A `cancel` reply `ok` is sent after the failure transition is persisted. `run_selftest` pushes `selftest_result` to every client, then replies `ok` with the `SelfTest` as `result` (design part 9.4). `settings_set` replies `ok` with the new settings serialised. `secret_set`, `retry`, `rip_again`, `review_action` and `shutdown` reply `ok` with `null`.
- **Ruling S79 (the MakeMKV check run file).** Each MakeMKV check outside a job is recorded at `<data>/selftest/drive-list.txt`, with its run files, and is overwritten by the next check. Every check, the job's listings and the check at engine start included, calls `MakeMkvState::observe` with `(summary.version, summary.licence.map(|m| m.code))` on the state `MakeMkvState::open(data)` gave at start. When `observe` returns true, every AwaitingMakeMkv job gets `MakeMkvChanged`, delivered once the token is free (design part 3.7). Because `recorded` is kept in `<data>/selftest/makemkv-check.json` (RP-9), the check at engine start compares against the last check before the restart (design part 5.10).
- **Ruling S80 (self-test checks and versions).** The checks come in this order: `makemkvcon`, `mkvmerge`, `ffprobe`, `ffmpeg`, `java`, `staging_root`, `tuning_root`, `movies_root`, `tmdb_token`. Each tool check carries the tool's path, version and where it was found (design part 9.2):
  - `makemkvcon`'s version is the check's `MSG:1005`. It is `ok` when it is found, its version is at least `makemkv_min_version`, and there is no licence message.
  - The other four tools' versions come from `tool_version`, recorded at `<data>/selftest/<tool>-version.txt` with its run files. Each is `ok` when found; a version is reported, not judged, because spec 13's minimum versions of MKVToolNix and FFmpeg come with layer 2 in PR 5. A tool not found has `path`, `version` and `found` None, and the detail `<tool> was not found`; a found tool whose version cannot be read has the detail `<tool> printed no version`.
  - The `java` check's detail also says `java in use at the last scan: <true|false>` from Task 23's `java_in_use`, or `no scan yet` (design part 9.4: "Java use comes from the last scan").
  - At engine start only the MakeMKV check runs (design part 3.7). Until the first `run_selftest`, the snapshot's other eight checks have `ok` false, `busy` false, every optional field None and the detail `not checked since the engine started`, so a start sends no request and writes no probe file.
  - While a RunScan, RipTitle or listing runs, the MakeMKV check returns `MakeMkvState.last` with `busy: true` and starts no run (design part 3.7).
- **Ruling S81 (what a retry re-checks).** A `retry` in a waiting state re-checks its condition:
  - AwaitingShare: `ShareConnector::reach` of the roots the return state uses gives `ShareBack`.
  - AwaitingSpace: the CheckSpace computation of Task 23, without feeding its event, gives `SpaceFreed`.
  - AwaitingMakeMkv: a fresh MakeMKV check, and `MakeMkvChanged` when `observe` says it changed.
  - AwaitingTool: `find_tool` of `makemkvcon` and `mkvmerge` (disc-phase return) or of `mkvmerge` and `ffprobe` (file-phase return), plus the `makemkvcon` version check. When all pass, it gives `ToolFound { tool: Makemkvcon }` for a disc-phase return and `ToolFound { tool: Mkvmerge }` for a file-phase return. The reducer accepts `ToolFound { .. }` for any tool (job.rs:1086).
  - AwaitingLookup: a lookup round (design part 8.3).
  - A condition still not cleared feeds nothing, and the reply is still `ok`.
  - After `run_selftest`, every AwaitingTool job whose re-check passes gets `ToolFound` (design part 9.4).
- **Ruling S82 (approved reasons).** `approve` feeds `ReviewApproved { overrides: Overrides::default(), approved_reasons }`, where `approved_reasons` is the job's `JobSummary.review_reasons` at the time of approval. That is the held decision's reasons as the snapshot showed them (P25), and PR 2 has no overrides (R-A8).
- **Ruling S83 (forced shutdown).** `shutdown { force: true }` cancels each running step as `cancel` does. It deletes each such job's files `<root>/.lazymkv-partial/<job id>-*.mkv.partial` in every library root, persists, replies `ok`, closes every client, then stops the owner thread. A CheckSpace or Eject in flight is let finish first (design part 3.8).
- **Ruling S84 (job updates and collisions).** After each persisted transition, the engine broadcasts `job_update`. `collisions` is `Some(record.collisions)` only when the job is in NeedsReview and the list is not empty. Otherwise it is `None`.
- **Notifier (spec 12).** A `failed` notification whose class is `MediaReadError` gets `"; try the other drive"` appended to the message Task 22 builds.

Test teardown relies on `EngineHandle::stop` (Task 22) cancelling every running step's `CancelToken`, CheckSpace and Eject included, as its doc says. `stop` is not the `shutdown` request.

RP-9 settles this: (a) because design part 3.7 compares each MakeMKV check, the one at engine start included (design part 5.10), against "the one recorded", the recorded `(version, licence code)` is persisted in `<data>/selftest/makemkv-check.json` with `write_atomic`, so every check compares against the last one, before a restart or after it.

- [ ] **Step 1: Write the scenario variant `corpus/scenarios/movie-direct-licence-update/` and the base's version answers**

`drive-list-v1.txt`. This listing and v2 hold one DRV line for each of two drives:

```
MSG:1005,0,1,"MakeMKV v2.0.0 win(x64-release) started","%1 started","MakeMKV v2.0.0 win(x64-release)"
DRV:0,2,999,12,"BD-RE SYNTHETIC DRIVE 1.00 SERIAL","SYNTHETIC_MOVIE","tok0"
DRV:1,2,999,12,"BD-RE SYNTHETIC DRIVE 1.00 SERIAL2","SYNTHETIC_MOVIE","tok1"
MSG:5010,0,0,"Failed to open disc","Failed to open disc"
```

`drive-list-v2.txt` is the same file with `v2.0.0` replaced by `v2.0.1` in both places.

`scan-licence.txt`:

```
MSG:1005,0,1,"MakeMKV v2.0.0 win(x64-release) started","%1 started","MakeMKV v2.0.0 win(x64-release)"
MSG:5050,0,0,"Synthetic licence message.","Synthetic licence message."
```

`makemkvcon64.json`. Same needles take responses in order, and the last one repeats (Task 10). The calls run in this order:
- the start check takes listing 1;
- the insertion takes listing 2;
- the next check takes listing 3, which is v2.

```json
{"responses": [
  {"args_contain": ["info", "disc:9999"], "stdout": "drive-list-v1.txt"},
  {"args_contain": ["info", "disc:9999"], "stdout": "drive-list-v1.txt"},
  {"args_contain": ["info", "disc:9999"], "stdout": "drive-list-v2.txt"},
  {"args_contain": ["info", "disc:0"], "stdout": "scan-licence.txt"},
  {"args_contain": ["info", "disc:0"], "stdout": "scan.txt", "timing": "scan.txt.timing.jsonl"},
  {"args_contain": ["info", "disc:1"], "stdout": "scan.txt", "timing": "scan.txt.timing.jsonl"},
  {"args_contain": ["mkv", "disc:0"], "stdout": "rip.txt",
   "creates": [{"dir_arg_from_end": 1, "name": "title_t00.mkv", "copy_from": "placeholder.mkv"}]},
  {"args_contain": ["mkv", "disc:1"], "stdout": "rip.txt",
   "creates": [{"dir_arg_from_end": 1, "name": "title_t00.mkv", "copy_from": "placeholder.mkv"}]}
]}
```

`scan.txt`, `rip.txt` and `placeholder.mkv` come from `movie-direct`, which `Harness::new` copies first (S51).

Add the version answers of S80 to `corpus/scenarios/movie-direct/`. Each text file is one line ending in a newline:
- `mkvmerge.json` gains a fourth response, `{"args_contain":["--version"],"stdout":"mkvmerge-version.txt"}`; `mkvmerge-version.txt` is `mkvmerge v88.0 ('Synthetic') 64-bit`.
- `ffprobe.json` gains `{"args_contain":["-version"],"stdout":"ffprobe-version.txt"}`; `ffprobe-version.txt` is `ffprobe version 7.1 Copyright (c) 2007-2024 the FFmpeg developers`.
- `ffmpeg.json` is `{"responses":[{"args_contain":["-version"],"stdout":"ffmpeg-version.txt"}]}`; `ffmpeg-version.txt` is `ffmpeg version 7.1 Copyright (c) 2000-2024 the FFmpeg developers`.
- `java.json` is `{"responses":[{"args_contain":["-version"],"stderr":"java-version.txt"}]}`; `java-version.txt` is `openjdk version "21.0.2" 2024-01-16`.

- [ ] **Step 2: Write the failing unit tests**

First add `mod ipc_server; mod selftest;` to `crates/engine/src/lib.rs`, so the new files compile with their tests.

In `crates/engine/src/ipc_server.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    /// A writer that blocks on its first write until released.
    struct Stuck(Arc<(Mutex<bool>, Condvar)>);
    impl Write for Stuck {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let (lock, cv) = &*self.0;
            let mut open = lock.lock().unwrap();
            while !*open { open = cv.wait(open).unwrap(); }
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    #[test]
    fn a_client_that_falls_behind_is_dropped_without_slowing_the_engine() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let clients = Clients::new();
        clients.add(Box::new(Stuck(gate.clone())));
        clients.add(Box::new(io::sink()));
        let started = Instant::now();
        for n in 0..(CLIENT_QUEUE + 10) { clients.broadcast(&format!("{{\"n\":{n}}}")); }
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(clients.count(), 1);
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
    }

    #[test]
    fn progress_is_pushed_at_most_once_a_second_per_job() {
        let mut gate = ProgressGate::new();
        let t0 = Instant::now();
        assert!(gate.admit("a-1", t0));
        assert!(!gate.admit("a-1", t0 + Duration::from_millis(500)));
        assert!(gate.admit("b-1", t0 + Duration::from_millis(500)));
        assert!(gate.admit("a-1", t0 + Duration::from_millis(1000)));
    }

    #[test]
    fn a_line_longer_than_the_cap_is_reported_without_reading_past_it() {
        let mut long = vec![b'x'; 11];
        long.extend_from_slice(b"\nnext\n");
        let mut reader = io::Cursor::new(long);
        assert_eq!(read_line_capped(&mut reader, 10).unwrap(), Some(Err(())));
        let mut short = io::Cursor::new(b"{\"v\":1}\n".to_vec());
        assert_eq!(read_line_capped(&mut short, 10).unwrap(), Some(Ok("{\"v\":1}".to_string())));
        assert_eq!(read_line_capped(&mut short, 10).unwrap(), None);
    }

    #[test]
    fn a_webhook_secret_shows_only_its_host() {
        assert_eq!(secret_host("http://ha.local:8123/api/webhook/abc123").as_deref(), Some("ha.local"));
        assert_eq!(secret_host("https://user:pw@hooks.example.org/x").as_deref(), Some("hooks.example.org"));
        assert_eq!(secret_host("not a url"), None);
    }

    fn record(n: u64, state: lazymkv_core::job::State) -> lazymkv_core::record::JobRecord {
        let fp = format!("{n:032X}");
        lazymkv_core::record::JobRecord {
            format: lazymkv_core::record::JOB_FORMAT.to_string(), id: format!("{}-1", fp.to_ascii_lowercase()),
            fingerprint: fp, disc_id: None, drive: "tok0".to_string(), created_unix: n, updated_unix: n, state,
            message: None, rip_again: false, profile: "job.mmcp.xml".to_string(), plan: None, titles: vec![],
            filed_unix: None, collisions: vec![], notes: vec![],
        }
    }

    #[test]
    fn the_snapshot_holds_every_unfinished_job_and_the_50_most_recently_finished() {
        use lazymkv_core::job::State;
        let mut jobs: Vec<_> = (1..=60).map(|n| record(n, State::Filed)).collect();
        jobs.push(record(61, State::Ejected { after_copy: false }));
        jobs.push(record(0, State::Ripping));
        let summaries = snapshot_jobs(&jobs);
        assert_eq!(summaries.len(), 52);
        assert_eq!(summaries[0].created_unix, 0);
        assert_eq!(summaries[1].created_unix, 61);
        assert_eq!(summaries[2].updated_unix, 60);
        assert_eq!(summaries[51].updated_unix, 11);
    }

    #[test]
    fn history_pages_at_most_50_before_a_job() {
        use lazymkv_core::job::State;
        let jobs: Vec<_> = (1..=60).map(|n| record(n, State::Filed)).collect();
        let first = history_page(&jobs, None).unwrap();
        assert_eq!(first.len(), 50);
        assert_eq!((first[0].updated_unix, first[49].updated_unix), (60, 11));
        let next = history_page(&jobs, Some(&first[49].id)).unwrap();
        assert_eq!(next.iter().map(|s| s.updated_unix).collect::<Vec<_>>(), (1..=10).rev().collect::<Vec<_>>());
        assert_eq!(history_page(&jobs, Some("nope-1")), Err("no job nope-1".to_string()));
    }
}
```

In `crates/engine/src/selftest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_clients::RecordedTransport;
    use lazymkv_core::testsupport::Scratch;

    #[test]
    fn a_library_root_is_probed_inside_lazymkv_partial_and_left_clean() {
        let s = Scratch::new("probe");
        let check = root_check("movies_root", Some(s.0.to_str().unwrap()), true);
        assert!(check.ok, "{}", check.detail);
        assert_eq!(check.name, "movies_root");
        let partial = s.0.join(".lazymkv-partial");
        assert!(partial.is_dir());
        assert_eq!(std::fs::read_dir(&partial).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&s.0).unwrap().count(), 1);
    }

    #[test]
    fn an_unset_root_fails_its_check() {
        assert!(!root_check("tuning_root", None, false).ok);
    }

    #[test]
    fn the_token_check_is_get_3_authentication() {
        let http = RecordedTransport::new();
        http.answer(&lazymkv_clients::authentication_url(), 200, b"{\"success\":true}");
        let check = token_check(&http, Some("test-token"));
        assert!(check.ok);
        let (url, headers) = http.requests().pop().unwrap();
        assert_eq!(url, "https://api.themoviedb.org/3/authentication");
        assert!(headers.contains(&("Authorization".to_string(), "Bearer test-token".to_string())));
        let refused = RecordedTransport::new();
        refused.answer(&lazymkv_clients::authentication_url(), 401, b"");
        let check = token_check(&refused, Some("test-token"));
        assert!(!check.ok && check.detail.contains("401"));
        assert!(!token_check(&RecordedTransport::new(), None).ok);
    }

    #[test]
    fn version_lines_are_read_per_tool() {
        use lazymkv_core::job::Tool;
        assert_eq!(version_in(Tool::Mkvmerge, "mkvmerge v88.0 ('Synthetic') 64-bit\n", "").as_deref(), Some("88.0"));
        assert_eq!(version_in(Tool::Ffprobe, "ffprobe version 7.1 Copyright\n", "").as_deref(), Some("7.1"));
        assert_eq!(version_in(Tool::Ffmpeg, "ffmpeg version 7.1 Copyright\n", "").as_deref(), Some("7.1"));
        assert_eq!(version_in(Tool::Java, "", "openjdk version \"21.0.2\" 2024-01-16\n").as_deref(), Some("21.0.2"));
        assert_eq!(version_in(Tool::Mkvmerge, "something else\n", ""), None);
    }

    #[test]
    fn the_recorded_check_survives_a_reopen() {
        let s = Scratch::new("makemkv-check");
        let mut state = MakeMkvState::open(&s.0);
        assert!(!state.observe(Some("2.0.0".to_string()), None));
        let reopened = MakeMkvState::open(&s.0);
        assert_eq!(reopened.recorded, Some((Some("2.0.0".to_string()), None)));
        assert!(s.0.join("selftest/makemkv-check.json").is_file());
    }

    #[test]
    fn a_changed_version_or_licence_state_is_reported_once() {
        let mut state = MakeMkvState { last: None, recorded: None, file: None };
        assert!(!state.observe(Some("2.0.0".to_string()), None));
        assert!(!state.observe(Some("2.0.0".to_string()), None));
        assert!(state.observe(Some("2.0.1".to_string()), None));
        assert!(state.observe(Some("2.0.1".to_string()), Some(5050)));
        assert!(!state.observe(Some("2.0.1".to_string()), Some(5050)));
    }
}
```

- [ ] **Step 3: Write the failing stand-in and scripted tests in `crates/fake-tools/tests/engine_ipc.rs`**

```rust
//! The IPC server over the in-memory transport (design parts 3.7, 3.8, 9.1 to 9.4; Ruling S12).

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use lazymkv_core::decision::Disposition;
use lazymkv_core::ipc::{read_reply, Envelope, Reply, CANCELLED, MAX_LINE_BYTES};
use lazymkv_core::job::{Event, Failure, FailureClass, NotifyEvent, State};
use lazymkv_core::plan::Mode;
use lazymkv_core::record::{JobRecord, Transition, JOB_FORMAT};
use lazymkv_core::review::ReviewReason;
use lazymkv_engine::testkit::{Harness, StepScript};
use lazymkv_engine::StepResult;
use lazymkv_platform::{IpcConnector, Reach};
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(20);
/// Task 22's disc names: the base scenario's disc, and two discs with their own fingerprints.
const DISC_A: &str = "movie-direct";
const DISC_B: &str = "disc-b";
const DISC_C: &str = "disc-c";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";

fn settings() -> Value { json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}) }

fn stand_in(scenarios: &[&str], settings: Value) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings)
}

fn step(event: Event) -> StepScript { StepScript::Result(StepResult { event: Some(event), message: None }) }
fn none() -> StepScript { StepScript::Result(StepResult { event: None, message: None }) }
fn unknown() -> Failure { Failure { class: FailureClass::Unknown, output: None } }

/// The main line to Filed, in the order its actions run.
fn main_line() -> Vec<(&'static str, StepScript)> {
    vec![
        ("RunScan", step(Event::ScanDone)), ("Plan", step(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", step(Event::SpaceOk)), ("RipTitle", step(Event::TitleRipped)),
        ("VerifyFile", step(Event::TitleVerified { last: true })), ("Eject", step(Event::Ejected)),
        ("Probe", step(Event::ProbeDone)), ("ExtractSubtitleEvidence", none()),
        ("Decide", step(Event::DecisionDone { disposition: Disposition::AutoFile, reasons: vec![], has_path: true })),
        ("CheckSpace", step(Event::SpaceOk)), ("Remux", step(Event::RemuxDone)),
        ("VerifyFile", step(Event::FileVerified)), ("File", step(Event::FileDone)),
    ]
}

/// The main line up to the `nth` (from 0) entry of `tag`, which hangs; nothing after it.
fn hanging_at(tag: &str, nth: usize) -> Vec<(&'static str, StepScript)> {
    let mut seen = 0;
    let mut out = Vec::new();
    for (t, s) in main_line() {
        if t == tag {
            if seen == nth { out.push((t, StepScript::Hang)); return out; }
            seen += 1;
        }
        out.push((t, s));
    }
    panic!("no {tag} #{nth} in the main line")
}

fn scripted(script: Vec<(&'static str, StepScript)>) -> Harness {
    Harness::new_scripted(script, settings())
}

fn wait_for_call(h: &Harness, tag: &str, count: usize) {
    let start = Instant::now();
    while h.steps().calls().iter().filter(|c| c.as_str() == tag).count() < count {
        assert!(start.elapsed() < WAIT, "{tag} was not called {count} times: {:?}", h.steps().calls());
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn job_ids(h: &Harness) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(h.data().join("jobs")).map_or(vec![], |d| {
        d.map(|e| e.unwrap().file_name().into_string().unwrap()).collect()
    });
    ids.sort();
    ids
}

fn transitions(h: &Harness, id: &str) -> Vec<Transition> {
    fs::read_to_string(h.job_dir(id).join("transitions.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn record(h: &Harness, id: &str) -> JobRecord {
    serde_json::from_slice(&fs::read(h.job_dir(id).join("state.json")).unwrap()).unwrap()
}

struct Client { lines: Receiver<String>, writer: Box<dyn Write + Send>, next: u64, pushes: VecDeque<Envelope> }

impl Client {
    fn open(h: &Harness) -> Client {
        let stream = h.client().connect().unwrap_or_else(|_| panic!("the engine accepts the client"));
        let (send, lines) = channel();
        let mut reader = stream.reader;
        std::thread::spawn(move || loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => if send.send(line).is_err() { break },
            }
        });
        Client { lines, writer: stream.writer, next: 1, pushes: VecDeque::new() }
    }
    /// The next line, or None once the engine closed the connection.
    fn line(&mut self) -> Option<String> {
        match self.lines.recv_timeout(WAIT) {
            Ok(line) => Some(line),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => panic!("no line from the engine within {WAIT:?}"),
        }
    }
    fn raw(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
        self.writer.flush().unwrap();
    }
    fn request(&mut self, kind: &str, payload: Value) -> Reply {
        let id = self.next;
        self.next += 1;
        self.raw(&json!({"v": 1, "type": kind, "id": id, "payload": payload}).to_string());
        loop {
            match read_reply(self.line().expect("a reply").trim_end()).unwrap() {
                Reply::Push(envelope) => self.pushes.push_back(envelope),
                reply @ (Reply::Ok { id: Some(n), .. } | Reply::Error { id: Some(n), .. }) if n == id => return reply,
                other => panic!("unexpected {other:?}"),
            }
        }
    }
    fn ok(&mut self, kind: &str, payload: Value) -> Value {
        match self.request(kind, payload) { Reply::Ok { result, .. } => result, other => panic!("{kind}: {other:?}") }
    }
    fn err(&mut self, kind: &str, payload: Value) -> String {
        match self.request(kind, payload) { Reply::Error { message, .. } => message, other => panic!("{kind}: {other:?}") }
    }
    fn push(&mut self) -> Envelope {
        if let Some(envelope) = self.pushes.pop_front() { return envelope; }
        match read_reply(self.line().expect("a push").trim_end()).unwrap() {
            Reply::Push(envelope) => envelope,
            other => panic!("expected a push, got {other:?}"),
        }
    }
    fn snapshot(&mut self) -> Value {
        let envelope = self.push();
        assert_eq!(envelope.kind, "snapshot");
        envelope.payload
    }
    /// job_update pushes until one whose state is `state` (for `job` when given); returns its payload.
    fn until_state(&mut self, job: Option<&str>, state: &str) -> Value {
        loop {
            let envelope = self.push();
            if envelope.kind == "job_update" && envelope.payload["state"]["state"] == json!(state)
                && job.map_or(true, |id| envelope.payload["id"] == json!(id)) {
                return envelope.payload;
            }
        }
    }
}

// ---- Envelope (design part 9.1) ----

#[test]
fn a_wrong_v_is_answered_and_the_connection_stays_open() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    c.raw(r#"{"v":2,"type":"history","id":7,"payload":{}}"#);
    assert!(matches!(read_reply(c.line().unwrap().trim_end()).unwrap(), Reply::Error { id: Some(7), .. }));
    assert!(c.ok("history", json!({})).as_array().unwrap().is_empty());
}

#[test]
fn a_line_over_4_mib_is_answered_then_the_client_is_dropped() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    c.raw(&"x".repeat(MAX_LINE_BYTES + 1));
    assert!(matches!(read_reply(c.line().unwrap().trim_end()).unwrap(), Reply::Error { id: None, .. }));
    assert_eq!(c.line(), None);
}

#[test]
fn two_clients_each_get_every_push() {
    let h = scripted(main_line());
    let (mut a, mut b) = (Client::open(&h), Client::open(&h));
    a.snapshot();
    b.snapshot();
    h.insert("tok0", DISC_A);
    let states = |c: &mut Client| {
        let mut seen = Vec::new();
        loop {
            let e = c.push();
            if e.kind == "job_update" {
                let state = e.payload["state"]["state"].as_str().unwrap().to_string();
                seen.push(state.clone());
                if state == "Filed" { return seen; }
            }
        }
    };
    let (sa, sb) = (states(&mut a), states(&mut b));
    assert_eq!(sa, sb);
    assert_eq!(sa.first().map(String::as_str), Some("Detected"));
}

#[test]
fn a_reconnect_gets_a_fresh_snapshot() {
    let h = scripted(main_line());
    let mut a = Client::open(&h);
    assert!(a.snapshot()["jobs"].as_array().unwrap().is_empty());
    h.insert("tok0", DISC_A);
    a.until_state(None, "Filed");
    drop(a);
    let mut b = Client::open(&h);
    let jobs = b.snapshot()["jobs"].as_array().unwrap().clone();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["state"], json!({"state": "Filed"}));
}

#[test]
fn a_path_collision_job_update_carries_the_collisions() {
    let h = stand_in(&["movie-direct"], settings());
    let target = h.library().join(FINAL);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"owner's").unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let held = c.until_state(None, "NeedsReview");
    let collisions = held["collisions"].as_array().unwrap();
    assert_eq!(collisions.len(), 1);
    assert!(collisions[0].as_str().unwrap().ends_with("Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv"));
    assert!(held["review_reasons"].as_array().unwrap().contains(&json!("path_collision")));
}

// ---- Snapshot and history (design part 9.2) ----

fn seed(h: &Harness, n: u64, state: State) -> String {
    let fp = format!("{n:032X}");
    let id = format!("{}-1", fp.to_ascii_lowercase());
    let r = JobRecord {
        format: JOB_FORMAT.to_string(), id: id.clone(), fingerprint: fp, disc_id: None, drive: "tok9".to_string(),
        created_unix: n, updated_unix: n, state, message: None, rip_again: false, profile: "job.mmcp.xml".to_string(),
        plan: None, titles: vec![], filed_unix: None, collisions: vec![], notes: vec![],
    };
    let dir = h.data().join("jobs").join(&id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("state.json"), serde_json::to_vec(&r).unwrap()).unwrap();
    id
}

#[test]
fn a_client_gets_a_snapshot_with_unfinished_and_50_recent_jobs() {
    let h = scripted(vec![]);
    for n in 1..=52 { seed(&h, n, State::Filed); }
    let waiting = seed(&h, 60, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    h.restart();
    let mut c = Client::open(&h);
    let snapshot = c.snapshot();
    let jobs = snapshot["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 51);
    assert_eq!(jobs[0]["id"], json!(waiting));
    assert_eq!(jobs[1]["updated_unix"], json!(52));
    assert_eq!(jobs[50]["updated_unix"], json!(3));
    for key in ["jobs", "settings", "problems", "secrets", "selftest"] { assert!(snapshot.get(key).is_some(), "{key}"); }
}

#[test]
fn secrets_show_set_and_only_the_webhook_host() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.ok("secret_set", json!({"name": "ha_webhook_url", "value": "http://ha.local:8123/api/webhook/abc123"})), Value::Null);
    let mut d = Client::open(&h);
    let line = d.line().unwrap();
    assert!(!line.contains("abc123"));
    let secrets: Value = serde_json::from_str::<Value>(&line).unwrap()["payload"]["secrets"].clone();
    assert_eq!(secrets["ha_webhook_url"], json!({"set": true, "host": "ha.local"}));
    assert_eq!(secrets["webhook_url"], json!({"set": false, "host": null}));
}

#[test]
fn history_pages_at_most_50_before_a_job_over_ipc() {
    let h = scripted(vec![]);
    let ids: Vec<String> = (1..=60).map(|n| seed(&h, n, State::Filed)).collect();
    h.restart();
    let mut c = Client::open(&h);
    c.snapshot();
    let first = c.ok("history", json!({}));
    assert_eq!(first.as_array().unwrap().len(), 50);
    let next = c.ok("history", json!({"before": ids[10]}));
    assert_eq!(next.as_array().unwrap().len(), 10);
    assert_eq!(c.err("history", json!({"before": "nope-1"})), "no job nope-1");
}

// ---- Settings and secrets (design part 9.4) ----

#[test]
fn settings_set_with_a_problem_changes_nothing_and_lists_problems() {
    let h = scripted(vec![]);
    let before = fs::read(h.data().join("settings.json")).unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    match c.request("settings_set", json!({"file_phase_concurrency": 0})) {
        Reply::Error { message, problems, .. } => {
            assert_eq!(message, "settings_set refused: 1 problems");
            assert_eq!(problems.len(), 1);
            assert_eq!(problems[0].key, "file_phase_concurrency");
        }
        other => panic!("{other:?}"),
    }
    match c.request("settings_set", json!({"nope": 1})) {
        Reply::Error { problems, .. } => assert!(problems.is_empty()),
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read(h.data().join("settings.json")).unwrap(), before);
}

#[test]
fn settings_set_clearing_the_last_problem_turns_present_discs_into_arrivals() {
    let h = Harness::new_scripted(main_line(), json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                                "drive_ready_timeout_s": 1, "file_phase_concurrency": 0}));
    let mut c = Client::open(&h);
    assert_eq!(c.snapshot()["problems"][0]["key"], json!("file_phase_concurrency"));
    h.insert("tok0", DISC_A);
    let result = c.ok("settings_set", json!({"file_phase_concurrency": 1}));
    assert_eq!(result["file_phase_concurrency"], json!(1));
    c.until_state(None, "Filed");
    assert_eq!(job_ids(&h).len(), 1);
}

#[test]
fn settings_set_does_not_turn_a_handled_disc_into_an_arrival_again() {
    let h = stand_in(&["movie-direct", "movie-direct-copy-plan"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.settle();
    let lines = transitions(&h, &id).len();
    let path = h.data().join("settings.json");
    let mut stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    stored["file_phase_concurrency"] = json!(0);
    fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    assert_eq!(c.ok("settings_set", json!({"file_phase_concurrency": 1}))["file_phase_concurrency"], json!(1));
    h.settle();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(transitions(&h, &id).len(), lines);
    assert!(h.ejector().calls().is_empty());
}

#[test]
fn a_settings_set_mid_job_leaves_the_running_jobs_flags() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    c.ok("settings_set", json!({"cache_mb": 512}));
    c.ok("cancel", json!({"job": id}));
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Ripping");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains("--cache=1024"));
    assert!(!log.contains("--cache=512"));
    let job: Value = serde_json::from_slice(&fs::read(h.job_dir(&id).join("settings.json")).unwrap()).unwrap();
    assert_eq!(job["cache_mb"], json!(1024));
    let data: Value = serde_json::from_slice(&fs::read(h.data().join("settings.json")).unwrap()).unwrap();
    assert_eq!(data["cache_mb"], json!(512));
    c.ok("cancel", json!({"job": id}));
}

#[test]
fn secret_set_refuses_an_unknown_name_and_never_echoes_the_value() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    let message = c.err("secret_set", json!({"name": "tmdb", "value": "s3cret-value"}));
    assert_eq!(message, "tmdb is not a secret name (design part 9.4)");
    assert_eq!(c.ok("secret_set", json!({"name": r"share:\\nas\media", "value": "user\npw"})), Value::Null);
    assert_eq!(c.ok("secret_set", json!({"name": "tmdb_token", "value": "s3cret-value"})), Value::Null);
    let log = fs::read_to_string(h.data().join("engine.log")).unwrap_or_default();
    assert!(!log.contains("s3cret-value"));
    let mut d = Client::open(&h);
    assert!(!d.line().unwrap().contains("s3cret-value"));
}

// ---- Retry (design part 9.4, L3) ----

#[test]
fn retry_on_failed_resumes_when_the_token_is_free() {
    let mut script = vec![("RunScan", step(Event::ScanDone)), ("RunScan", step(Event::ScanDone)),
                          ("Plan", step(Event::PlanDone { mode: Mode::Direct })), ("Plan", step(Event::PlanDone { mode: Mode::Direct })),
                          ("CheckSpace", step(Event::SpaceOk)), ("CheckSpace", step(Event::SpaceOk)),
                          ("RipTitle", step(Event::TitleRipFailed { failure: unknown() })), ("RipTitle", StepScript::Hang),
                          ("RipTitle", step(Event::TitleRipped))];
    script.extend(main_line().into_iter().skip(4).filter(|(t, _)| *t != "RipTitle"));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let a = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "RipTitle", 2);
    let b = job_ids(&h).into_iter().find(|id| *id != a).unwrap();
    c.ok("retry", json!({"job": a}));
    assert!(matches!(record(&h, &a).state, State::Failed { .. }));
    c.ok("cancel", json!({"job": b}));
    c.until_state(Some(&a), "Ripping");
}

#[test]
fn retry_in_awaiting_space_feeds_space_freed_when_space_returns() {
    let h = stand_in(&["movie-direct"], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_free(&staging, 1);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingSpace")["id"].as_str().unwrap().to_string();
    let lines = transitions(&h, &id).len();
    c.ok("retry", json!({"job": id}));
    assert_eq!(transitions(&h, &id).len(), lines);
    h.shares().set_free(&staging, u64::MAX);
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Filed");
    assert!(transitions(&h, &id).iter().any(|t| t.event == Some(Event::SpaceFreed)));
}

#[test]
fn retry_in_each_waiting_state_rechecks_its_condition() {
    // Share: the staging share comes back.
    let h = stand_in(&["movie-direct"], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Unreachable("gone".to_string()));
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingShare")["id"].as_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Reachable);
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Filed");
    assert!(transitions(&h, &id).iter().any(|t| t.event == Some(Event::ShareBack)));
    drop((c, h));

    // MakeMkv: the check after the retry reads v2.0.1 (listing 3 of the variant).
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingMakeMkv")["id"].as_str().unwrap().to_string();
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Fingerprinted");
    assert!(transitions(&h, &id).iter().any(|t| t.event == Some(Event::MakeMkvChanged)));
    drop((c, h));

    // Tool: makemkvcon is moved away before the insertion and back before the second retry.
    let h = stand_in(&["movie-direct"], settings());
    let stored: Value = serde_json::from_slice(&fs::read(h.data().join("settings.json")).unwrap()).unwrap();
    let tool = std::path::PathBuf::from(stored["tool_paths"]["makemkvcon"].as_str().unwrap());
    let aside = tool.with_extension("aside");
    let mut c = Client::open(&h);
    c.snapshot();
    fs::rename(&tool, &aside).unwrap();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingTool")["id"].as_str().unwrap().to_string();
    let lines = transitions(&h, &id).len();
    c.ok("retry", json!({"job": id}));
    assert_eq!(transitions(&h, &id).len(), lines);
    fs::rename(&aside, &tool).unwrap();
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Fingerprinted");
    assert!(transitions(&h, &id).iter().any(|t| matches!(t.event, Some(Event::ToolFound { .. }))));
    drop((c, h));

    // Lookup: a retry runs a round over the recorded transport.
    let mut script = main_line();
    script.truncate(8);
    script.push(("Decide", step(Event::LookupPending)));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingLookup")["id"].as_str().unwrap().to_string();
    let asked = h.http().requests().len();
    c.ok("retry", json!({"job": id}));
    assert!(h.http().requests().len() > asked);
    let last = transitions(&h, &id).pop().unwrap();
    assert!(matches!(last.old, Some(State::AwaitingLookup { .. })));
}

#[test]
fn retry_of_a_file_phase_failure_waits_for_the_slot() {
    let mut script = main_line();
    script.truncate(6); // A's disc phase
    script.extend(main_line().into_iter().take(6)); // B's disc phase
    script.push(("Probe", step(Event::ProbeFailed { failure: unknown() }))); // A
    script.extend([("Probe", step(Event::ProbeDone)), ("ExtractSubtitleEvidence", none()),
                   ("Decide", step(Event::DecisionDone { disposition: Disposition::AutoFile, reasons: vec![], has_path: true })),
                   ("CheckSpace", step(Event::SpaceOk)), ("Remux", StepScript::Hang)]); // B holds the slot
    script.extend([("Probe", step(Event::ProbeDone))]); // A after its retry
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "Remux", 1);
    let ids = job_ids(&h);
    let a = ids.iter().find(|id| matches!(record(&h, id).state, State::Failed { .. })).unwrap().clone();
    let b = ids.iter().find(|id| **id != a).unwrap().clone();
    c.ok("retry", json!({"job": a}));
    assert!(matches!(record(&h, &a).state, State::Failed { .. }));
    c.ok("cancel", json!({"job": b}));
    c.until_state(Some(&a), "Inspecting");
}

#[test]
fn a_retry_with_the_disc_absent_feeds_media_removed_and_frees_the_token() {
    let mut script = main_line();
    script[3] = ("RipTitle", step(Event::TitleRipFailed { failure: unknown() }));
    script.extend(main_line()); // a second disc goes the whole way
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let a = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.remove("tok0");
    c.ok("retry", json!({"job": a}));
    let lines = transitions(&h, &a);
    let n = lines.len();
    assert_eq!(lines[n - 2].event, Some(Event::Retry));
    assert_eq!(lines[n - 1].event, Some(Event::MediaRemoved));
    assert_eq!(lines[n - 1].actions, vec![lazymkv_core::job::Action::WaitForDisc]);
    assert_eq!(h.steps().calls().iter().filter(|c| c.as_str() == "RipTitle").count(), 1);
    h.insert("tok1", DISC_B);
    c.until_state(None, "Ejected");
}

#[test]
fn a_retry_queues_behind_an_earlier_arrival() {
    let mut script = main_line();
    script[3] = ("RipTitle", step(Event::TitleRipFailed { failure: unknown() }));
    script.extend(main_line().into_iter().take(3));
    script.push(("RipTitle", StepScript::Hang)); // B holds the token
    script.extend(main_line()); // C, then A
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let a = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "RipTitle", 2);
    let b = job_ids(&h).into_iter().find(|id| *id != a).unwrap();
    h.insert("tok2", DISC_C);
    c.ok("retry", json!({"job": a}));
    c.ok("cancel", json!({"job": b}));
    let mut order = Vec::new();
    while order.len() < 2 {
        let e = c.push();
        if e.kind != "job_update" { continue; }
        let id = e.payload["id"].as_str().unwrap().to_string();
        if id != b && !order.contains(&id) && e.payload["state"]["state"] != json!("Failed") { order.push(id); }
    }
    assert_ne!(order[0], a);
    assert_eq!(order[1], a);
}

#[test]
fn rip_again_arms_the_job_and_the_next_insertion_starts_a_new_job() {
    let mut script = main_line();
    script.extend(main_line());
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Filed")["id"].as_str().unwrap().to_string();
    assert_eq!(c.ok("rip_again", json!({"job": id})), Value::Null);
    assert!(record(&h, &id).rip_again);
    h.remove("tok0");
    h.insert("tok0", DISC_A);
    let second = c.until_state(None, "Detected")["id"].as_str().unwrap().to_string();
    assert_eq!(second, id.replace("-1", "-2"));
}

// ---- Cancel (design part 9.4, R-A7) ----

fn assert_cancelled(h: &Harness, id: &str, event_tag: &str, from: &str) {
    let last = transitions(h, id).pop().unwrap();
    let event = serde_json::to_value(&last.event).unwrap();
    assert_eq!(event["event"], json!(event_tag), "{from}");
    assert_eq!(event["failure"]["class"], json!("Unknown"), "{from}");
    assert!(matches!(&last.new, State::Failed { class: FailureClass::Unknown, from: f } if f.as_str() == from), "{from}");
    assert_eq!(record(h, id).message.as_deref(), Some(CANCELLED));
    assert!(matches!(h.notifications().last().unwrap().event, NotifyEvent::Failed { class: FailureClass::Unknown }));
}

#[test]
fn each_in_flight_action_cancels_to_its_design_part_9_4_event() {
    for (tag, nth, from, event) in [
        ("RunScan", 0, "Fingerprinted", "ScanFailed"), ("Plan", 0, "Scanned", "PlanFailed"),
        ("RipTitle", 0, "Ripping", "TitleRipFailed"), ("VerifyFile", 0, "Ripping", "TitleRipFailed"),
        ("Probe", 0, "Inspecting", "ProbeFailed"), ("Decide", 0, "Inspecting", "DecisionFailed"),
        ("Remux", 0, "Remuxing", "RemuxFailed"), ("VerifyFile", 1, "Remuxing", "FileVerifyFailed"),
        ("File", 0, "Verified", "FileFailed"),
    ] {
        let h = scripted(hanging_at(tag, nth));
        let mut c = Client::open(&h);
        c.snapshot();
        h.insert("tok0", DISC_A);
        wait_for_call(&h, tag, nth + 1);
        let id = job_ids(&h).pop().unwrap();
        c.ok("cancel", json!({"job": id}));
        assert_cancelled(&h, &id, event, from);
    }
    // Fingerprint: a restart in Detected hands Fingerprint to the runner.
    let h = scripted(vec![("Fingerprint", StepScript::Hang)]);
    h.insert("tok0", DISC_A);
    h.run_until("Detected");
    h.restart();
    wait_for_call(&h, "Fingerprint", 1);
    let id = job_ids(&h).pop().unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("cancel", json!({"job": id}));
    assert_cancelled(&h, &id, "FingerprintFailed", "Detected");
    // Decide in Decided, after an approval.
    let mut script = hanging_at("Decide", 0);
    script.pop();
    script.push(("Decide", step(Event::DecisionDone { disposition: Disposition::NeedsReview,
                                                      reasons: vec![ReviewReason::IdentityUnknown], has_path: true })));
    script.push(("Decide", StepScript::Hang));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "NeedsReview")["id"].as_str().unwrap().to_string();
    c.ok("review_action", json!({"job": id, "action": "approve"}));
    wait_for_call(&h, "Decide", 2);
    c.ok("cancel", json!({"job": id}));
    assert_cancelled(&h, &id, "DecisionFailed", "Decided");
}

#[test]
fn cancel_during_check_space_or_eject_is_refused() {
    for (tag, action) in [("CheckSpace", "CheckSpace"), ("Eject", "Eject")] {
        let h = scripted(hanging_at(tag, 0));
        let mut c = Client::open(&h);
        c.snapshot();
        h.insert("tok0", DISC_A);
        wait_for_call(&h, tag, 1);
        let id = job_ids(&h).pop().unwrap();
        assert_eq!(c.err("cancel", json!({"job": id})),
                   format!("job {id} is running {action}, which cancel does not stop (design part 9.4)"));
    }
}

#[test]
fn a_cancel_with_no_running_step_is_refused_naming_the_state() {
    let mut script = main_line();
    script[2] = ("CheckSpace", step(Event::SpaceShort));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingSpace")["id"].as_str().unwrap().to_string();
    assert_eq!(c.err("cancel", json!({"job": id})), format!("job {id} has no running step: it is AwaitingSpace"));
    assert_eq!(c.err("cancel", json!({"job": "nope-1"})), "no job nope-1");
}

#[test]
fn a_cancelled_disc_phase_job_is_not_ejected() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    c.ok("cancel", json!({"job": id}));
    assert!(h.ejector().calls().is_empty());
}

#[test]
fn cancel_during_a_rip_kills_it_and_fails_with_cancelled_by_the_owner() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    let start = Instant::now();
    c.ok("cancel", json!({"job": id}));
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_cancelled(&h, &id, "TitleRipFailed", "Ripping");
    let exit = fs::read_dir(h.job_dir(&id)).unwrap().map(|e| e.unwrap().path())
        .map(|p| p.join("rip.txt.exit.json")).find(|p| p.is_file()).unwrap();
    let exit: Value = serde_json::from_slice(&fs::read(exit).unwrap()).unwrap();
    assert_ne!(exit["exit_code"], json!(0));
}

// ---- Review actions (design part 9.4, P25, R-A8) ----

fn held_script() -> Vec<(&'static str, StepScript)> {
    let mut script = hanging_at("Decide", 0);
    script.pop();
    script.push(("Decide", step(Event::DecisionDone { disposition: Disposition::NeedsReview,
                                                      reasons: vec![ReviewReason::IdentityUnknown], has_path: true })));
    script
}

#[test]
fn approve_delivers_review_approved_when_the_slot_is_free() {
    let mut script = held_script();
    script.extend(main_line().into_iter().take(6)); // B's disc phase
    script.extend([("Probe", step(Event::ProbeDone)), ("ExtractSubtitleEvidence", none()),
                   ("Decide", step(Event::DecisionDone { disposition: Disposition::AutoFile, reasons: vec![], has_path: true })),
                   ("CheckSpace", step(Event::SpaceOk)), ("Remux", StepScript::Hang)]);
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let held = c.until_state(None, "NeedsReview");
    let a = held["id"].as_str().unwrap().to_string();
    let reasons = held["review_reasons"].clone();
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "Remux", 1);
    let b = job_ids(&h).into_iter().find(|id| *id != a).unwrap();
    c.ok("review_action", json!({"job": a, "action": "approve", "overrides": {}}));
    assert!(matches!(record(&h, &a).state, State::NeedsReview { .. }));
    c.ok("cancel", json!({"job": b}));
    c.until_state(Some(&a), "Decided");
    let approved = transitions(&h, &a).into_iter().find(|t| matches!(t.event, Some(Event::ReviewApproved { .. }))).unwrap();
    let event = serde_json::to_value(&approved.event).unwrap();
    assert_eq!(event["approved_reasons"], reasons);
}

#[test]
fn an_approved_job_decides_again_from_its_kept_inputs_and_files() {
    let h = stand_in(&["movie-direct"], settings());
    let target = h.library().join(FINAL);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"owner's").unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "NeedsReview")["id"].as_str().unwrap().to_string();
    c.ok("review_action", json!({"job": id, "action": "approve"}));
    c.until_state(Some(&id), "NeedsReview"); // path_collision holds it again (blocks_filing)
    fs::remove_file(&target).unwrap();
    c.ok("review_action", json!({"job": id, "action": "approve"}));
    c.until_state(Some(&id), "Filed");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(!log.contains("read back from the job folder"));
    assert_eq!(fs::read(&target).unwrap().len(), 16);
}

#[test]
fn approve_is_refused_outside_the_disposition_gate_naming_the_state() {
    let h = scripted(main_line());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Filed")["id"].as_str().unwrap().to_string();
    assert_eq!(c.err("review_action", json!({"job": id, "action": "approve"})),
               format!("job {id} is Filed, not held at the disposition gate"));
}

#[test]
fn approve_with_overrides_is_refused_naming_phase_3() {
    let h = scripted(held_script());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "NeedsReview")["id"].as_str().unwrap().to_string();
    assert_eq!(c.err("review_action", json!({"job": id, "action": "approve", "overrides": {"season": 2}})),
               "approve waits for phase 3's review commands (R-A8)");
    assert_eq!(c.err("review_action", json!({"job": id, "action": "set_identity"})),
               "set_identity waits for phase 3's review commands (R-A8)");
}

#[test]
fn discard_is_refused_naming_phase_3() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("discard", json!({"job": "abc-1"})), "discard waits for phase 3's review commands (P2, R-A1)");
}

#[test]
fn ui_quit_is_refused_naming_phase_5() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("ui_quit", Value::Null), "ui_quit waits for phase 5's user interface (R-A8)");
}

// ---- Self-test (design parts 3.7, 9.4) ----

fn check<'a>(selftest: &'a Value, name: &str) -> &'a Value {
    selftest["checks"].as_array().unwrap().iter().find(|c| c["name"] == json!(name)).unwrap()
}

#[test]
fn the_engine_runs_the_makemkv_check_at_start_into_the_snapshot() {
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let mut c = Client::open(&h);
    let snapshot = c.snapshot();
    let makemkv = check(&snapshot["selftest"], "makemkvcon");
    assert_eq!(makemkv["ok"], json!(true));
    assert_eq!(makemkv["version"], json!("2.0.0"));
    assert_eq!(makemkv["found"], json!("settings"));
    assert_eq!(makemkv["busy"], json!(false));
    let names: Vec<&str> = snapshot["selftest"]["checks"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["makemkvcon", "mkvmerge", "ffprobe", "ffmpeg", "java", "staging_root", "tuning_root", "movies_root", "tmdb_token"]);
}

#[test]
fn run_selftest_pushes_results_then_delivers_tool_found() {
    let h = stand_in(&["movie-direct"], settings());
    let stored: Value = serde_json::from_slice(&fs::read(h.data().join("settings.json")).unwrap()).unwrap();
    let tool = std::path::PathBuf::from(stored["tool_paths"]["makemkvcon"].as_str().unwrap());
    let aside = tool.with_extension("aside");
    let mut c = Client::open(&h);
    c.snapshot();
    fs::rename(&tool, &aside).unwrap();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "AwaitingTool")["id"].as_str().unwrap().to_string();
    fs::rename(&aside, &tool).unwrap();
    let result = c.ok("run_selftest", Value::Null);
    let pushed = c.pushes.iter().find(|e| e.kind == "selftest_result").expect("selftest_result before the reply");
    assert_eq!(pushed.payload, result);
    c.until_state(Some(&id), "Fingerprinted");
}

#[test]
fn another_discs_listing_with_a_changed_version_returns_a_job_from_awaiting_makemkv_when_the_token_is_free() {
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let a = c.until_state(None, "AwaitingMakeMkv")["id"].as_str().unwrap().to_string();
    h.insert("tok1", DISC_B);
    let mut b_ejected = false;
    loop {
        let e = c.push();
        if e.kind != "job_update" { continue; }
        if e.payload["id"] != json!(a) && e.payload["state"]["state"] == json!("Ejected") { b_ejected = true; }
        if e.payload["id"] == json!(a) && e.payload["state"]["state"] != json!("AwaitingMakeMkv") {
            assert!(b_ejected, "the job left AwaitingMakeMkv while the other disc held the token");
            break;
        }
    }
    assert!(transitions(&h, &a).iter().any(|t| t.event == Some(Event::MakeMkvChanged)));
}

#[test]
fn a_selftest_while_makemkvcon_runs_reports_the_last_check_marked_busy() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    let at_start = check(&c.snapshot()["selftest"], "makemkvcon").clone();
    h.insert("tok0", DISC_A);
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    let result = c.ok("run_selftest", Value::Null);
    let busy = check(&result, "makemkvcon");
    assert_eq!(busy["busy"], json!(true));
    assert_eq!((busy["ok"].clone(), busy["version"].clone()), (at_start["ok"].clone(), at_start["version"].clone()));
    c.ok("cancel", json!({"job": id}));
}

#[test]
fn each_tool_check_reports_the_version_its_stand_in_prints() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    let at_start = c.snapshot();
    assert_eq!(check(&at_start["selftest"], "mkvmerge")["detail"], json!("not checked since the engine started"));
    let result = c.ok("run_selftest", Value::Null);
    for (tool, version) in [("mkvmerge", "88.0"), ("ffprobe", "7.1"), ("ffmpeg", "7.1"), ("java", "21.0.2")] {
        let found = check(&result, tool);
        assert_eq!((&found["ok"], &found["version"], &found["found"]), (&json!(true), &json!(version), &json!("settings")), "{tool}");
        assert!(found["path"].as_str().unwrap().contains(tool), "{tool}");
        for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
            assert!(h.data().join(format!("selftest/{tool}-version.txt{suffix}")).is_file(), "{tool}{suffix}");
        }
    }
}

#[test]
fn the_selftest_java_check_reports_the_last_scans_java_use() {
    let h = stand_in(&["movie-direct", "movie-direct-java"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let before = c.ok("run_selftest", Value::Null);
    assert!(check(&before, "java")["detail"].as_str().unwrap().contains("no scan yet"));
    h.insert("tok0", DISC_A);
    c.until_state(None, "Ejected");
    let after = c.ok("run_selftest", Value::Null);
    assert!(check(&after, "java")["detail"].as_str().unwrap().contains("java in use at the last scan: true"));
}

#[test]
fn a_restart_with_a_changed_makemkv_version_returns_awaiting_makemkv_jobs() {
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let recorded = |h: &Harness| -> Value {
        serde_json::from_slice(&fs::read(h.data().join("selftest/makemkv-check.json")).unwrap()).unwrap()
    };
    h.insert("tok0", DISC_A);
    h.run_until("AwaitingMakeMkv");
    let id = job_ids(&h).pop().unwrap();
    assert_eq!(recorded(&h), json!({"version": "2.0.0", "licence": null}));
    h.restart();
    h.wait("MakeMkvChanged", |h| transitions(h, &id).iter().any(|t| t.event == Some(Event::MakeMkvChanged)));
    let events: Vec<Event> = transitions(&h, &id).into_iter().filter_map(|t| t.event).collect();
    let at = events.iter().position(|e| *e == Event::MakeMkvChanged).unwrap();
    assert!(matches!(events[at - 1], Event::EngineRestart { .. }), "{events:?}");
    assert_eq!(recorded(&h), json!({"version": "2.0.1", "licence": null}));
}

#[test]
fn a_selftest_probes_each_library_root_inside_lazymkv_partial() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let result = c.ok("run_selftest", Value::Null);
    for name in ["staging_root", "tuning_root", "movies_root"] { assert_eq!(check(&result, name)["ok"], json!(true), "{name}"); }
    assert!(h.library().join(".lazymkv-partial").is_dir());
    let left: Vec<_> = fs::read_dir(h.library()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from(".lazymkv-partial")]);
}

#[test]
fn the_token_check_runs_over_the_engines_transport() {
    let h = stand_in(&["movie-direct"], settings());
    h.http().answer(&lazymkv_clients::authentication_url(), 200, b"{\"success\":true}");
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("secret_set", json!({"name": "tmdb_token", "value": "test-token"}));
    let result = c.ok("run_selftest", Value::Null);
    assert_eq!(check(&result, "tmdb_token")["ok"], json!(true));
    assert!(h.http().requests().iter().any(|(url, _)| url == "https://api.themoviedb.org/3/authentication"));
}

// ---- Shutdown, progress, command and notifications ----

#[test]
fn shutdown_is_refused_while_a_step_runs_and_force_cancels_and_deletes_partials() {
    let h = scripted(hanging_at("Remux", 0));
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    wait_for_call(&h, "Remux", 1);
    let id = job_ids(&h).pop().unwrap();
    let partial = h.library().join(".lazymkv-partial");
    fs::create_dir_all(&partial).unwrap();
    fs::write(partial.join(format!("{id}-t00-00800.mkv.partial")), b"x").unwrap();
    fs::write(partial.join("other-1-t00-00800.mkv.partial"), b"y").unwrap();
    let staged = h.staging().join(&id).join("rip").join("t00-00800");
    fs::create_dir_all(&staged).unwrap();
    fs::write(staged.join("title_t00.mkv"), b"raw").unwrap();
    assert_eq!(c.err("shutdown", json!({"force": false})),
               format!("a step is running for {id}; stop --force cancels it (design part 3.8)"));
    assert_eq!(c.ok("shutdown", json!({"force": true})), Value::Null);
    assert_eq!(c.line(), None);
    assert!(!partial.join(format!("{id}-t00-00800.mkv.partial")).exists());
    assert!(partial.join("other-1-t00-00800.mkv.partial").exists());
    assert_eq!(fs::read(staged.join("title_t00.mkv")).unwrap(), b"raw");
    let stopped = record(&h, &id);
    assert_eq!(stopped.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert_eq!(stopped.message.as_deref(), Some(CANCELLED));
}

#[test]
fn shutdown_without_force_succeeds_when_only_waiting_held_failed_or_filed_jobs_exist() {
    // DISC_A is filed, DISC_B fails at its scan, DISC_C waits for space, and disc-d is held for review.
    let mut script = main_line();
    script.push(("RunScan", step(Event::ScanFailed { failure: unknown() })));
    script.extend([("RunScan", step(Event::ScanDone)), ("Plan", step(Event::PlanDone { mode: Mode::Direct })),
                   ("CheckSpace", step(Event::SpaceShort))]);
    let mut held = hanging_at("Decide", 0);
    held.pop();
    held.push(("Decide", step(Event::DecisionDone { disposition: Disposition::NeedsReview,
                                                    reasons: vec![ReviewReason::IdentityUnknown], has_path: true })));
    script.extend(held);
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    c.until_state(None, "Filed");
    h.insert("tok1", DISC_B);
    c.until_state(None, "Failed");
    h.insert("tok2", DISC_C);
    c.until_state(None, "AwaitingSpace");
    h.insert("tok3", "disc-d");
    c.until_state(None, "NeedsReview");
    assert_eq!(c.ok("shutdown", json!({"force": false})), Value::Null);
    assert_eq!(c.line(), None);
    let mut states: Vec<String> = job_ids(&h).iter().map(|id| h.state_of(id)).collect();
    states.sort();
    assert_eq!(states, ["AwaitingSpace", "Failed", "Filed", "NeedsReview"]);
}

#[test]
fn a_forced_shutdown_lets_an_in_flight_eject_finish_then_persists_it() {
    let mut script = hanging_at("Eject", 0);
    script.pop();
    script.push(("Eject", StepScript::After(Duration::from_millis(500), StepResult { event: Some(Event::Ejected), message: None })));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    wait_for_call(&h, "Eject", 1);
    let id = job_ids(&h).pop().unwrap();
    assert_eq!(c.err("shutdown", json!({"force": false})),
               format!("a step is running for {id}; stop --force cancels it (design part 3.8)"));
    assert_eq!(c.ok("shutdown", json!({"force": true})), Value::Null);
    assert_eq!(c.line(), None);
    let last = transitions(&h, &id).pop().unwrap();
    assert_eq!((last.event, last.new), (Some(Event::Ejected), State::Ejected { after_copy: false }));
}

#[test]
fn progress_pushes_name_the_action_and_the_title() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let progress = loop {
        let e = c.push();
        if e.kind == "progress" { break e; }
    };
    assert_eq!(progress.id, None);
    let fraction = progress.payload["fraction"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&fraction));
    assert!(["RipTitle", "Remux"].contains(&progress.payload["step"].as_str().unwrap()));
}

#[test]
fn command_is_refused_until_the_run_folder_exists() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("command", json!({"words": ["plan", "--job", "abc-1"]})),
               "command needs the engine's run folder, which arrives in pull request 3 (S35)");
}

#[test]
fn notifications_reach_the_sink_in_order() {
    let mut script = main_line();
    script[6] = ("Probe", step(Event::ProbeFailed { failure: unknown() }));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    c.until_state(None, "Failed");
    let events: Vec<&str> = h.notifications().iter().map(|n| n.event.as_str()).collect();
    assert_eq!(events, ["disc_done", "failed"]);
}

#[test]
fn a_media_read_error_failure_message_says_try_the_other_drive() {
    let mut script = main_line();
    script[3] = ("RipTitle", step(Event::TitleRipFailed { failure: Failure { class: FailureClass::MediaReadError, output: None } }));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    c.until_state(None, "Failed");
    let failed = h.notifications().pop().unwrap();
    assert_eq!(failed.event, NotifyEvent::Failed { class: FailureClass::MediaReadError });
    assert!(failed.message.ends_with("; try the other drive"), "{}", failed.message);
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit -- ipc_server selftest`

Expected: compile error. `Clients`, `ProgressGate`, `read_line_capped`, `secret_host`, `snapshot_jobs`, `history_page`, `root_check`, `token_check`, `version_in` and `MakeMkvState` are not found.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc`

Expected: compile error, `Harness::client` not found.

- [ ] **Step 5: Implement**

- `ipc_server.rs`:
  - `serve` accepts streams from `IpcTransport::accept` on its own thread. Each accepted stream gets `Clients::add(writer)`, the snapshot sent first (`push_line(&Push::Snapshot(..), None)`), and a reader thread.
  - The reader thread loops over `read_line_capped(.., MAX_LINE_BYTES)`:
    - `Err(())`: send `reply_error(None, LINE_TOO_LONG, &[])`, then drop the client;
    - `Ok(line)`: call `parse_request`. On `Err(line)`, send that line. Otherwise hand `(client, id, request)` to the owner thread over its channel.
  - The owner thread answers each request, using the refusal table and Rulings S76 to S84. It sends replies with `Clients::send` and pushes with `Clients::broadcast`.
  - `settings_set` (design part 9.4): `let mut merged = serde_json::to_value(&current)` with `current` from `Store::settings()`, insert each key of `changes` into `merged`'s object, then `Settings::from_json(&serde_json::to_string(&merged))`. An Err (an unknown key among them, through `deny_unknown_fields`, settings.rs:72) gives `reply_error(id, <that text>, &[])`. Ok with problems gives `reply_error(id, "settings_set refused: <n> problems", &problems)`. Otherwise `Store::write_settings`, `reply_ok(id, <the settings serialised>)`, and then the arrival rule of design part 9.4: every disc in a drive is an arrival, except a disc whose `drives.json` record has its fingerprint and `handled: true` (L1a).
  - `secret_set` of a valid name calls `SecretStore::set`, and a share name joins the in-memory set S77 lists.
  - `progress`: the StepRunner's callback builds `Progress { job, step: <the Action serde tag>, title, fraction }` (S7) and pushes it only when `ProgressGate::admit` returns true.
- `selftest.rs`:
  - Tool checks use `find_tool(tool, &settings, resources, host, path_var)`, then `tool_version` for every tool but `makemkvcon` (S80).
  - `version_in` and `tool_version` follow their docs; `tool_version` reads stderr from `with_suffix(out, ".stderr")` for java.
  - The `makemkvcon` check uses `MakeMkvState` (`open(data)` at start) and `list_drives(.., "", <data>/selftest/drive-list.txt)` (S79).
  - The `java` check's detail reads `java_in_use` (S80).
  - `root_check` writes `lazymkv-selftest.probe`, renames it to `lazymkv-selftest.probe.renamed` and deletes it. It works in `<root>` for staging and tuning, and in `<root>/.lazymkv-partial` (created when missing) for `movies_root`.
  - `token_check` sends `GET authentication_url()` with `("Authorization", "Bearer <token>")` and `("User-Agent", user_agent())`. It is `ok` on 200. The detail names the status, or `no tmdb_token is set`.
- `steps.rs`: each `list_drives` result goes to `MakeMkvState::observe` (S79).
- `notify.rs`: the MediaReadError suffix.
- `testkit.rs`: `client()` returns the stored connector's clone. `ScriptedSteps` runs `StepScript::After(d, result)` by sleeping `d`, then returning `result`.
- `review_action` approve: S82's `ReviewApproved`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --features testkit -- ipc_server selftest`

Expected: PASS, 12 tests.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc`

Expected: PASS, 47 tests.

Run the junction form, because the new scenario folder falls under the RP21 guard of Task 23: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`

Expected: PASS. `no_scenario_holds_a_clone_value` passes.

- [ ] **Step 7: Commit**

```
git add crates/engine/src crates/fake-tools/tests/engine_ipc.rs corpus/scenarios/movie-direct-licence-update corpus/scenarios/movie-direct
git commit -m "engine: the IPC server, requests, self-test and notifier sink (design parts 3.7, 3.8, 9.1 to 9.4)"
```

Check: `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>`.

### Task 27: Local plan, decide and retune

**Files:**
- Modify: `crates/ctl/src/lib.rs` and `crates/ctl/src/main.rs`, both created in Task 1.
- Create: `crates/ctl/src/tests.rs` (8 tests), the sibling of `lib.rs`.
- Modify: `crates/dev/src/lib.rs:84-207` (`Args`), adding `rest` and 2 tests in its `mod tests`.

**Interfaces:**
- Consumes:
  - `pub fn dispatch(ctx: &Ctx, argv: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32` (crates/dev/src/lib.rs:50). It prints `lazymkv-dev: unknown command ...` for unknown names.
  - `pub const COMMANDS: &[(&str, Command, &str)]` (crates/dev/src/lib.rs:23).
  - `pub struct Args` with `new(args: &[String], flags: &[&str])`, `flag`, `switch` and `positionals` (crates/dev/src/lib.rs:86-153). A unique prefix of a declared flag is taken. An ambiguous one is refused as `ambiguous option: --jo could match --job, --jobs`.
  - `lazymkv_dev::evidence::Ctx::{from_process, for_test, abs}` (crates/dev/src/evidence.rs:33, :45, :104) and `evidence::default_spec` (:223).
  - `lazymkv_platform::IpcConnector` (Task 8).
  - `lazymkv_core::testsupport::{Scratch, ScratchExt, scan_text}` (crates/core/src/testsupport/mod.rs:373, :400, :477).
- Produces:

```rust
/// What lazymkvctl reads and writes: the dev context, the engine connector (None in PR 2's binary, S38), and the
/// three streams (design parts 9.5, 9.6). Def: T27. Use: T27, T28.
pub struct Env<'a> { pub ctx: &'a Ctx, pub connector: Option<&'a dyn IpcConnector>, pub stdin: &'a mut dyn Read,
    pub out: &'a mut dyn Write, pub err: &'a mut dyn Write }
/// lazymkvctl's entry: `argv` excludes the program name; returns the exit code of design part 9.6. Def: T27. Use: T27, T28.
pub fn run(argv: &[String], env: Env) -> i32;
/// The commands lazymkvctl runs itself through lazymkv-dev's dispatcher (design part 9.5, P23). Def: T27. Use: T27, T28.
pub const LOCAL_COMMANDS: [&str; 3] = ["plan", "decide", "retune"];
/// The message of exit 4 when this build has no engine transport (design part 4.2, S38). Def: T27. Use: T27, T28.
pub const NO_TRANSPORT: &str = "lazymkvctl: no engine transport exists in this build";
/// Def: T27. Use: T28.
pub const NOT_RUNNING: &str = "lazymkvctl: no engine is running";
/// Def: T27. Use: T28.
pub const REFUSED: &str = "lazymkvctl: the engine refused the connection";
/// Def: T27. Use: T28.
pub const NOT_ELEVATED: &str = "lazymkvctl: run lazymkvctl elevated to reach the engine";
/// retune's eight flags (crates/dev/src/cmd/retune.rs:38) plus "--jobs", for reading a relayed retune's words.
/// Def: T27. Use: T27, T28.
pub(crate) const RETUNE_RELAY_FLAGS: [&str; 9] = ["--evidence", "--set", "--snapshot", "--compare", "--spec",
    "--allow-unmeasured", "--subtitles", "--text", "--jobs"];
/// The spec path of `retune --jobs` (design part 9.6): `--spec`'s value through `ctx.abs`, else
/// `evidence::default_spec(ctx)`; Err is an `Args` error text, or "the following arguments are required: --spec"
/// (retune.rs:83's sentence) when neither gives a path. Def: T27. Use: T27, T28.
pub(crate) fn retune_spec(ctx: &Ctx, words: &[String]) -> Result<PathBuf, String>;
// crates/dev/src/lib.rs
impl Args {
    /// The words left, unchanged and in order ("--" and undeclared flag-like words kept), for a caller that hands
    /// them on to another parser; Err("ambiguous option: ...") for an ambiguous prefix of a declared flag before "--".
    /// Def: T27. Use: T27.
    pub fn rest(self) -> Result<Vec<String>, String>;
}
```

**Rulings** (S85 to S88):
- **Ruling S85 (`Args::rest`).** dev's `Args` gains `rest`. ctl must take out `--job`, `--jobs` and `--title` and pass every other word to dev's own parser. `positionals()` refuses undeclared flags such as `--settings`, so it cannot do this. Keeping one parser keeps argparse's prefix rule in one place.
- **Ruling S86 (word routing).** `run` routes the first word as follows:
  - no word: `USAGE` goes to `err`, exit 2;
  - `--help` or `-h`: `USAGE` goes to `out`, exit 0;
  - a `LOCAL_COMMANDS` name: the local or engine form below;
  - any other `lazymkv_dev::COMMANDS` name: `lazymkvctl: <name> is a lazymkv-dev command, not a lazymkvctl command`, exit 2;
  - anything else: `lazymkvctl: unknown command <name>` and then `USAGE` to `err`, exit 2. Task 28 adds the engine command names before this branch.
- **Ruling S87 (forms and argument errors).** For a local command, ctl calls `Args::new(rest, &["--job", "--jobs", "--title"])`, then `flag("--job")`, `switch("--jobs")`, `flag("--title")` and `rest()`.
  - Each argument error prints `lazymkvctl <name>: error: <message>` to `err` and exits 2 (design part 9.6). The messages are:
    - `argument --title: only with --job`;
    - `argument --title: required with decide --job`;
    - `argument --title: not allowed with <name>`;
    - `argument --job: not allowed with retune; use --jobs`;
    - `argument --jobs: only with retune`;
    - an `Args` error text unchanged.
  - With no `--job` and no `--jobs`, ctl returns `lazymkv_dev::dispatch(ctx, &[name] + words, out, err)`. The documents, the exit codes and dev's stderr are then identical (S37).
  - The engine form, with `--job` or `--jobs`, goes to a private `fn engine_form(name: &str, job: Option<&str>, title: Option<&str>, words: &[String], env: Env) -> i32`. For `retune --jobs` it first calls `retune_spec(ctx, words)`, and an Err is an argument error with that text, exit 2, whatever the engine's state (design part 9.6). Then it writes `NO_TRANSPORT` and returns 4. Task 28 replaces its body with `relay`, which calls the same `retune_spec`, so the spec rule lives in one place.
- **Ruling S88 (`USAGE`).** `USAGE` is a private constant:

```
usage: lazymkvctl <command> [arguments]

commands run here (design part 9.5):
  plan <slug|recording dir|scan file> [lazymkv-dev plan's flags]   or   plan --job ID
  decide (<slug> <label> | --rec DIR) [lazymkv-dev decide's flags]   or   decide --job ID --title KEY
  retune [lazymkv-dev retune's flags]   or   retune --jobs [--spec FILE]
```

`main.rs`:

```rust
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let ctx = lazymkv_dev::evidence::Ctx::from_process();
    let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    let code = lazymkv_ctl::run(&argv, lazymkv_ctl::Env { ctx: &ctx, connector: None, stdin: &mut stdin.lock(),
        out: &mut stdout.lock(), err: &mut stderr.lock() });
    std::process::exit(code);
}
```

- [ ] **Step 1: Write the failing tests for `Args::rest` in `crates/dev/src/lib.rs` `mod tests`**

```rust
#[test]
fn rest_keeps_unknown_flags_and_the_double_dash() {
    let mut args = Args::new(&words(&["x", "--job", "a-1", "--settings", "s.json", "--", "--title"]), &["--job", "--jobs", "--title"]);
    assert_eq!(args.flag("--job"), Ok(Some("a-1".to_string())));
    assert_eq!(args.flag("--title"), Ok(None));
    assert_eq!(args.rest(), Ok(words(&["x", "--settings", "s.json", "--", "--title"])));
}

#[test]
fn rest_refuses_an_ambiguous_prefix_of_a_declared_flag() {
    let args = Args::new(&words(&["--jo", "a-1"]), &["--job", "--jobs"]);
    assert_eq!(args.rest(), Err("ambiguous option: --jo could match --job, --jobs".to_string()));
}
```

- [ ] **Step 2: Write the failing tests in `crates/ctl/src/tests.rs`**

Add `#[cfg(test)] mod tests;` at the end of `lib.rs`.

```rust
use super::*;
use lazymkv_core::testsupport::{scan_text, Scratch, ScratchExt};
use lazymkv_dev::evidence::Ctx;
use serde_json::json;

fn words(text: &[&str]) -> Vec<String> { text.iter().map(|w| w.to_string()).collect() }

/// lazymkvctl over `ctx` with no connector: (exit code, stdout, stderr).
fn ctl(ctx: &Ctx, argv: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut stdin: &[u8] = b"";
    let code = run(&words(argv), Env { ctx, connector: None, stdin: &mut stdin, out: &mut out, err: &mut err });
    (code, out, err)
}

/// lazymkv-dev's dispatcher over the same context.
fn dev(ctx: &Ctx, argv: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(ctx, &words(argv), &mut out, &mut err);
    (code, out, err)
}

/// A made-up recording recs/disc-a with the rip label eng-only, as dev's decide tests build it (decide.rs:810-878).
fn recording(s: &Scratch) {
    s.write("recs/disc-a/scan.txt", concat!(
        "TINFO:0,8,0,\"2\"\nTINFO:0,9,0,\"0:10:00\"\n",
        "SINFO:0,0,1,6201,\"Video\"\nSINFO:0,0,6,0,\"Mpeg4\"\n",
        "SINFO:0,1,1,6202,\"Audio\"\nSINFO:0,1,3,0,\"eng\"\nSINFO:0,1,6,0,\"DTS-HD MA\"\nSINFO:0,1,14,0,\"6\"\n",
        "SINFO:0,2,1,6202,\"Audio\"\nSINFO:0,2,3,0,\"eng\"\nSINFO:0,2,6,0,\"DD\"\nSINFO:0,2,14,0,\"2\"\n",
        "SINFO:0,3,1,6202,\"Audio\"\nSINFO:0,3,3,0,\"spa\"\nSINFO:0,3,6,0,\"DD\"\nSINFO:0,3,14,0,\"2\"\n",
        "SINFO:0,4,1,6203,\"Subtitles\"\nSINFO:0,4,3,0,\"eng\"\nSINFO:0,4,6,0,\"PGS\"\n"));
    s.write("recs/disc-a/eng-only/rip-summary.json",
            json!({"ok": true, "title": 0, "profile": "C:\\made-up\\eng-only.mmcp.xml"}).to_string());
    s.write("recs/disc-a/eng-only/mkvmerge.json", json!({
        "tracks": [
            {"id": 0, "type": "video", "codec": "AVC", "properties": {"language": "und"}},
            {"id": 1, "type": "audio", "codec": "DTS-HD MA", "properties": {"language": "eng", "audio_channels": 6}},
            {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
            {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}
        ],
        "chapters": [{"num_entries": 2}],
        "container": {"properties": {"duration": 600_000_000_000_i64}}
    }).to_string());
    s.write("recs/disc-a/eng-only/rip.txt", "MSG:5011,0,0,\"made-up line\",\"made-up line\"\n");
}

#[test]
fn local_plan_prints_what_lazymkv_dev_plan_prints() {
    let s = Scratch::new("ctl-plan");
    let scan = scan_text(
        &[(2, "SYNTHETIC_MOVIE")],
        &[(0, &[(9, "1:59:10"), (8, "12"), (16, "00800.mpls")][..],
           &[(0, &[(1, "Video")][..]), (1, &[(1, "Audio"), (3, "eng")][..])][..])],
    );
    s.write("scan.txt", scan);
    let ctx = Ctx::for_test(&s.0);
    let local = ctl(&ctx, &["plan", "scan.txt"]);
    assert!(!local.1.is_empty());
    assert_eq!(local, dev(&ctx, &["plan", "scan.txt"]));
}

#[test]
fn local_decide_and_retune_match_lazymkv_dev() {
    let s = Scratch::new("ctl-decide");
    recording(&s);
    let ctx = Ctx::for_test(&s.0);
    let decide = ctl(&ctx, &["decide", "--rec", "recs/disc-a/eng-only"]);
    assert_eq!(decide.0, 0);
    assert!(decide.1.starts_with(b"{"));
    assert_eq!(decide, dev(&ctx, &["decide", "--rec", "recs/disc-a/eng-only"]));
    std::fs::create_dir_all(s.0.join("empty")).unwrap();
    for argv in [&["retune", "--evidence", "empty"][..], &["retune", "--subtitles", "--evidence", "empty"][..]] {
        assert_eq!(ctl(&ctx, argv), dev(&ctx, argv), "{argv:?}");
    }
}

#[test]
fn another_dev_command_name_is_refused_with_exit_2() {
    let s = Scratch::new("ctl-other");
    let ctx = Ctx::for_test(&s.0);
    let (code, out, err) = ctl(&ctx, &["scan", "x"]);
    assert_eq!((code, out), (2, vec![]));
    assert_eq!(String::from_utf8(err).unwrap(), "lazymkvctl: scan is a lazymkv-dev command, not a lazymkvctl command\n");
    let (code, _, err) = ctl(&ctx, &["nonsense"]);
    assert_eq!(code, 2);
    assert!(String::from_utf8(err).unwrap().starts_with("lazymkvctl: unknown command nonsense\nusage: lazymkvctl"));
}

#[test]
fn title_without_job_is_an_argument_error() {
    let s = Scratch::new("ctl-title");
    recording(&s);
    let ctx = Ctx::for_test(&s.0);
    for (argv, message) in [
        (&["decide", "--rec", "recs/disc-a/eng-only", "--title", "t00"][..], "argument --title: only with --job"),
        (&["decide", "--job", "abc-1"][..], "argument --title: required with decide --job"),
        (&["plan", "--job", "abc-1", "--title", "t00"][..], "argument --title: not allowed with plan"),
        (&["retune", "--job", "abc-1"][..], "argument --job: not allowed with retune; use --jobs"),
        (&["plan", "--jobs"][..], "argument --jobs: only with retune"),
        (&["retune", "--jobs"][..], "the following arguments are required: --spec"),
    ] {
        let (code, out, err) = ctl(&ctx, argv);
        assert_eq!((code, out), (2, vec![]), "{argv:?}");
        let name = argv[0];
        assert_eq!(String::from_utf8(err).unwrap(), format!("lazymkvctl {name}: error: {message}\n"), "{argv:?}");
    }
}

#[test]
fn an_ambiguous_prefix_jo_is_refused() {
    let s = Scratch::new("ctl-jo");
    let ctx = Ctx::for_test(&s.0);
    let (code, _, err) = ctl(&ctx, &["plan", "--jo", "abc-1"]);
    assert_eq!(code, 2);
    assert_eq!(String::from_utf8(err).unwrap(), "lazymkvctl plan: error: ambiguous option: --jo could match --job, --jobs\n");
}

#[test]
fn an_engine_form_without_a_connector_exits_4_with_no_transport() {
    let s = Scratch::new("ctl-engine-form");
    let ctx = Ctx::for_test(&s.0);
    for argv in [&["plan", "--job", "abc-1"][..], &["decide", "--job", "abc-1", "--title", "t00-00800"][..],
                 &["retune", "--jobs", "--spec", "spec.md"][..]] {
        let (code, out, err) = ctl(&ctx, argv);
        assert_eq!((code, out), (4, vec![]), "{argv:?}");
        assert_eq!(String::from_utf8(err).unwrap(), format!("{NO_TRANSPORT}\n"));
    }
}

#[test]
fn no_arguments_print_the_usage_and_exit_2_and_help_exits_0() {
    let s = Scratch::new("ctl-usage");
    let ctx = Ctx::for_test(&s.0);
    let (code, out, err) = ctl(&ctx, &[]);
    assert_eq!((code, out), (2, vec![]));
    assert!(String::from_utf8(err).unwrap().starts_with("usage: lazymkvctl <command> [arguments]\n"));
    let (code, out, _) = ctl(&ctx, &["--help"]);
    assert_eq!(code, 0);
    assert!(String::from_utf8(out).unwrap().contains("decide --job ID --title KEY"));
}

#[test]
fn the_local_commands_are_the_three_of_design_part_9_5() {
    assert_eq!(LOCAL_COMMANDS, ["plan", "decide", "retune"]);
    for name in LOCAL_COMMANDS {
        assert!(lazymkv_dev::COMMANDS.iter().any(|(command, _, _)| *command == name), "{name}");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked --lib rest_`

Expected: compile error, no method `rest` on `Args`.

Run: `scripts/box.sh cargo test -p lazymkv-ctl --locked`

Expected: compile error, `run`, `Env` and `LOCAL_COMMANDS` not found.

- [ ] **Step 4: Implement**

- `Args::rest`: walk `self.words`. Before the first `"--"`, a word that `resolve` reads as `Resolved::Ambiguous` returns `Err(format!("ambiguous option: {} could match {}", option, matches.join(", ")))`, the same text `positionals` gives. Otherwise return `Ok(self.words)` unchanged.
- `lib.rs`: implement `Env`, `run`, `LOCAL_COMMANDS`, the four message constants, `RETUNE_RELAY_FLAGS`, `retune_spec`, the private `USAGE` and the private `engine_form` (Rulings S85 to S88). Each item gets the `///` doc shown in Interfaces. `retune_spec` reads `Args::new(words, &RETUNE_RELAY_FLAGS).flag("--spec")`.
- `main.rs`: as written above.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked --lib`

Expected: PASS. Every existing `Args` test is unchanged, and the 2 new tests pass.

Run: `scripts/box.sh cargo test -p lazymkv-ctl --locked`

Expected: PASS, 8 tests.

- [ ] **Step 6: Commit**

```
git add crates/ctl/src/lib.rs crates/ctl/src/main.rs crates/ctl/src/tests.rs crates/dev/src/lib.rs
git commit -m "ctl: lazymkvctl runs plan, decide and retune through dev's dispatcher (design part 9.5)"
```

Check: `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>`.

### Task 28: Engine commands over the in-memory transport (design part 9.6)

**Files:**
- Create: `crates/ctl/src/engine_cmds.rs`, with 13 tests in its `mod tests`.
- Modify: `crates/ctl/src/lib.rs`, created in Task 27. It gains the `mod engine_cmds;` line and two routing branches in `run`.
- Test: create `crates/fake-tools/tests/ctl_engine.rs` with 8 tests against the real engine through `Harness::client()`.

**Interfaces:**
- Consumes:
  - From ctl (Task 27): `pub struct Env<'a> { pub ctx: &'a Ctx, pub connector: Option<&'a dyn IpcConnector>, pub stdin: &'a mut dyn Read, pub out: &'a mut dyn Write, pub err: &'a mut dyn Write }`, `pub fn run(argv: &[String], env: Env) -> i32`, `pub const LOCAL_COMMANDS: [&str; 3]`, `NO_TRANSPORT`, `NOT_RUNNING`, `REFUSED`, `NOT_ELEVATED`, `RETUNE_RELAY_FLAGS` and `retune_spec(ctx: &Ctx, words: &[String]) -> Result<PathBuf, String>`.
  - From platform (Task 8):
    - `pub struct IpcStream { pub reader: Box<dyn BufRead + Send>, pub writer: Box<dyn Write + Send> }`
    - `pub enum ConnectError { NotRunning, Refused, NotElevated, NoTransport }`
    - `pub trait IpcConnector { fn connect(&self) -> Result<IpcStream, ConnectError>; }`
    - `pub trait IpcTransport: Send + Sync { fn accept(&self) -> io::Result<IpcStream>; }`
    - `fakes::MemoryIpc::new() -> (MemoryIpc, MemoryConnector)`
    - `fakes::MemoryConnector::failing(ConnectError) -> MemoryConnector`
  - From core::ipc (Task 7):
    - `PROTOCOL_VERSION`, `Envelope { v, kind (serde "type"), id, payload }` and `read_reply(line: &str) -> Result<Reply, String>`.
    - `Reply::{Ok { id, result }, Error { id, message, problems }, Push(Envelope)}`.
    - `reply_ok`, `reply_error`, `push_line`, `Push::CommandOutput { stream, line }` and `OutputStream::{Stdout, Stderr}`.
    - This task relies on `read_reply` returning `Push(Envelope)` for any envelope whose `type` is not `ok` or `error`, without checking its payload.
  - `core::settings::SettingsProblem { key, problem }` (crates/core/src/settings.rs:238).
  - From dev:
    - `lazymkv_dev::Args` (crates/dev/src/lib.rs:86): `new`, `flag`, `switch` and `positionals`.
    - `lazymkv_dev::evidence::{Ctx, default_spec}` (crates/dev/src/evidence.rs:20, :223) and `Ctx::abs` (evidence.rs:104).
    - `Ctx::for_test` (evidence.rs:45).
    - `lazymkv_dev::pyjson::{dumps, Style}` (crates/dev/src/pyjson.rs:32; `Style::INDENT1_ASCII` at :24).
  - From the engine testkit (Tasks 22, 23 and 26): `Harness::new(fake_tool: &Path, scenarios: &[&str], settings: serde_json::Value)`, plus its `insert`, `run_until`, `restart`, `data` and `client() -> MemoryConnector`.
    - `insert(drive, disc_name)` is read with `disc_name` as the scenario folder whose `disc.json` lays out the volume (Task 23).
    - Every harness method takes `&self` (Task 22).
  - From core testsupport: `Scratch` and `ScratchExt::write_path` (crates/core/src/testsupport/mod.rs:373, :400).
- Produces (crate-private):
  ```rust
  // crates/ctl/src/engine_cmds.rs
  /// Design part 9.6's engine commands as (name, synopsis after "usage: lazymkvctl "), in its table order.
  /// Def: T28. Use: T28.
  pub(crate) const ENGINE_COMMANDS: [(&str, &str); 11] = [
      ("status", "status"), ("watch", "watch"), ("history", "history [--before JOB]"),
      ("settings", "settings get | settings set FILE"), ("secret", "secret set NAME"), ("selftest", "selftest"),
      ("retry", "retry JOB"), ("cancel", "cancel JOB"), ("rip-again", "rip-again JOB"), ("approve", "approve JOB"),
      ("stop", "stop [--force]"),
  ];
  /// Runs one engine command of design part 9.6; argv[0] is an ENGINE_COMMANDS name. Exits 0 on `ok`, 1 on `error`,
  /// 2 on an argument error, 4 when no engine answered (T28-d). Def: T28. Use: T28.
  pub(crate) fn run(argv: &[String], env: Env) -> i32;
  /// Sends the engine form of `plan --job`, `decide --job --title` or `retune --jobs` as `command` (design part 9.5),
  /// prints the relayed lines and exits with the child's code; an `error` reply exits 2 (RP24). Def: T28. Use: T28.
  pub(crate) fn relay(argv: &[String], env: Env) -> i32;
  const CLOSED: &str = "lazymkvctl: the engine closed the connection before replying";
  const NO_SELFTEST: &str = "lazymkvctl: the engine replied without a self-test result";
  const RETUNE_RELAY_USAGE: &str = "retune --jobs [--spec FILE] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE] [--allow-unmeasured] [--subtitles]";
  ```

Rulings settled in this task:
- **Ruling T28-a.** `status`, `settings get`, `history` and `selftest` print their JSON value with `dumps(&value, Style::INDENT1_ASCII)` and a newline. Every other `ok` prints nothing. Why: `lazymkvctl` then has one JSON style, the one `plan` and `decide` print (crates/dev/src/cmd/decide.rs:469).
- **Ruling T28-b.** `watch` prints each line exactly as received, without its line ending, followed by `\n`. It exits 0 when the connection closes. Why: a field that a newer engine adds is not lost.
- **Ruling T28-c.** Each run opens one connection and sends at most one request, with `id` 1. Why: one `lazymkvctl` run is one command.
- **Ruling T28-d.** A connection that closes before the reply, or a line `read_reply` refuses, exits 4. The message is `CLOSED` in the first case and `lazymkvctl: <read_reply's message>` in the second. Why: no engine answered, and exit 1 must stay out of `plan`'s codes (RP24).
- **Ruling T28-e.** Error replies and usage errors use ctl's own program name. Why: these follow argparse's shape. S37 covers only the commands dev runs.
  - An `error` reply prints `lazymkvctl: <message>`, then one line `lazymkvctl: <key>: <problem>` per problem.
  - A usage error prints `usage: lazymkvctl <synopsis>` and `lazymkvctl <name>: error: <message>`, then exits 2.
- **Ruling T28-f.** The `secret set` value is the whole of standard input with one trailing `\n` or `\r\n` removed. An empty value is an argument error. Why: a share credential is two lines (design part 9.4).
- **Ruling T28-g.** `relay` sends argv unchanged as `words`. For `retune`, it then appends `--spec <absolute path>`. Why: the engine builds the child's words (design part 9.5), and `Args::flag` lets the last `--spec` win, so the user's words never need rewriting.
- **Ruling T28-h.** Argument checks, the settings file read and the standard-input read all happen before connecting. Why: an argument error exits 2 whatever the engine's state.
- **Ruling T28-i.** `selftest` prints the payload of the last `selftest_result` push received before the `ok`. Without one it prints `NO_SELFTEST` and exits 1. Why: the engine pushes the result and then replies (design part 9.4).
- **Ruling T28-j.** The `watch` test runs in ctl against a scripted peer, not against `Harness::client()`. Why: only a scripted peer controls when the connection closes.

- [ ] **Step 1: Write the failing ctl tests in `crates/ctl/src/engine_cmds.rs` and add `mod engine_cmds;` to `crates/ctl/src/lib.rs`**

```rust
#[cfg(test)]
mod tests {
    use crate::{run, Env, NOT_ELEVATED, NOT_RUNNING, NO_TRANSPORT, REFUSED};
    use lazymkv_core::ipc::{push_line, reply_error, reply_ok, OutputStream, Push};
    use lazymkv_core::settings::SettingsProblem;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_dev::evidence::Ctx;
    use lazymkv_platform::fakes::{MemoryConnector, MemoryIpc};
    use lazymkv_platform::{ConnectError, IpcConnector, IpcTransport};
    use serde_json::{json, Value};
    use std::io::{BufRead, Write};
    use std::path::Path;
    use std::thread::{self, JoinHandle};

    const SNAPSHOT: &str = r#"{"v":1,"type":"snapshot","id":null,"payload":{"jobs":[]}}"#;

    fn call(argv: &[&str], connector: Option<&dyn IpcConnector>, stdin: &[u8], cwd: &Path) -> (i32, String, String) {
        let ctx = Ctx::for_test(cwd);
        let mut input = stdin;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
        let code = run(&argv, Env { ctx: &ctx, connector, stdin: &mut input, out: &mut out, err: &mut err });
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// Accepts one connection, writes `before`; with `answer` Some, reads one request line and writes the answer;
    /// then closes the connection. Returns the request line, or "" when none was read.
    fn peer(ipc: MemoryIpc, before: Vec<String>, answer: Option<Vec<String>>) -> JoinHandle<String> {
        thread::spawn(move || {
            let mut stream = ipc.accept().unwrap();
            let mut send = |lines: &[String], stream: &mut lazymkv_platform::IpcStream| {
                for line in lines {
                    write!(stream.writer, "{}\n", line.trim_end_matches('\n')).unwrap();
                }
                stream.writer.flush().unwrap();
            };
            send(&before, &mut stream);
            let mut request = String::new();
            if let Some(lines) = answer {
                stream.reader.read_line(&mut request).unwrap();
                send(&lines, &mut stream);
            }
            request
        })
    }

    fn request_of(line: &str) -> Value {
        serde_json::from_str(line.trim_end()).unwrap()
    }

    #[test]
    fn no_connector_exits_4_with_the_no_transport_message() {
        let scratch = Scratch::new("ctl-none");
        for argv in [&["status"][..], &["retry", "a-1"], &["plan", "--job", "a-1"]] {
            let (code, out, err) = call(argv, None, b"", &scratch.0);
            assert_eq!((code, out.as_str(), err), (4, "", format!("{NO_TRANSPORT}\n")), "{argv:?}");
        }
    }

    #[test]
    fn each_connect_error_exits_4_with_its_message() {
        let scratch = Scratch::new("ctl-connect");
        let cases = [
            (ConnectError::NotRunning, NOT_RUNNING),
            (ConnectError::Refused, REFUSED),
            (ConnectError::NotElevated, NOT_ELEVATED),
            (ConnectError::NoTransport, NO_TRANSPORT),
        ];
        for (error, message) in cases {
            let connector = MemoryConnector::failing(error);
            let (code, out, err) = call(&["status"], Some(&connector), b"", &scratch.0);
            assert_eq!((code, out.as_str(), err), (4, "", format!("{message}\n")));
        }
    }

    #[test]
    fn engine_command_argument_errors_exit_2_without_connecting() {
        let scratch = Scratch::new("ctl-args");
        let list = scratch.write_path("list.json", "[1]");
        let missing = scratch.0.join("missing.json");
        let (list, missing) = (list.to_str().unwrap(), missing.to_str().unwrap());
        let connector = MemoryConnector::failing(ConnectError::NotRunning);
        let cases: Vec<(Vec<&str>, &[u8], &str)> = vec![
            (vec!["status", "x"], b"", "status"),
            (vec!["watch", "x"], b"", "watch"),
            (vec!["history", "--before"], b"", "history"),
            (vec!["settings"], b"", "settings"),
            (vec!["settings", "put"], b"", "settings"),
            (vec!["settings", "set"], b"", "settings"),
            (vec!["settings", "set", missing], b"", "settings"),
            (vec!["settings", "set", list], b"", "settings"),
            (vec!["secret", "set"], b"", "secret"),
            (vec!["secret", "set", "tmdb_token"], b"", "secret"),
            (vec!["selftest", "x"], b"", "selftest"),
            (vec!["retry"], b"", "retry"),
            (vec!["cancel", "a-1", "b-1"], b"", "cancel"),
            (vec!["approve"], b"", "approve"),
            (vec!["stop", "--now"], b"", "stop"),
        ];
        for (argv, stdin, name) in cases {
            let (code, out, err) = call(&argv, Some(&connector), stdin, &scratch.0);
            assert_eq!((code, out.as_str()), (2, ""), "{argv:?}");
            assert!(err.starts_with("usage: lazymkvctl "), "{argv:?}: {err}");
            assert!(err.contains(&format!("lazymkvctl {name}: error: ")), "{argv:?}: {err}");
            assert!(!err.contains(NOT_RUNNING), "{argv:?} connected: {err}");
        }
    }

    #[test]
    fn retune_jobs_without_a_spec_exits_2() {
        let scratch = Scratch::new("ctl-retune-nospec");
        let connector = MemoryConnector::failing(ConnectError::NotRunning);
        let (code, out, err) = call(&["retune", "--jobs"], Some(&connector), b"", &scratch.0);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("the following arguments are required: --spec"), "{err}");
        assert!(!err.contains(NOT_RUNNING), "{err}");
    }

    #[test]
    fn status_prints_the_first_snapshots_payload() {
        let scratch = Scratch::new("ctl-status");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![SNAPSHOT.to_string()], None);
        let (code, out, err) = call(&["status"], Some(&connector), b"", &scratch.0);
        server.join().unwrap();
        assert_eq!((code, out.as_str(), err.as_str()), (0, "{\n \"jobs\": []\n}\n", ""));
    }

    #[test]
    fn watch_prints_pushes_until_the_connection_closes() {
        let scratch = Scratch::new("ctl-watch");
        let (ipc, connector) = MemoryIpc::new();
        let lines = vec![
            SNAPSHOT.to_string(),
            r#"{"v":1,"type":"job_update","id":null,"payload":{"id":"a-1"}}"#.to_string(),
            r#"{"v":1,"type":"progress","id":null,"payload":{"job":"a-1","step":"RipTitle","title":"t00-00800","fraction":0.5}}"#
                .to_string(),
        ];
        let server = peer(ipc, lines.clone(), None);
        let (code, out, err) = call(&["watch"], Some(&connector), b"", &scratch.0);
        assert_eq!(server.join().unwrap(), "");
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, lines.join("\n") + "\n");
    }

    #[test]
    fn each_engine_command_sends_its_design_part_9_3_request() {
        let scratch = Scratch::new("ctl-requests");
        let selftest_push = r#"{"v":1,"type":"selftest_result","id":null,"payload":{"checks":[]}}"#.to_string();
        let cases: Vec<(Vec<&str>, &str, Value, &str)> = vec![
            (vec!["history"], "history", json!({"before": null}), "[]\n"),
            (vec!["history", "--before", "a-1"], "history", json!({"before": "a-1"}), "[]\n"),
            (vec!["selftest"], "run_selftest", Value::Null, "{\n \"checks\": []\n}\n"),
            (vec!["retry", "a-1"], "retry", json!({"job": "a-1"}), ""),
            (vec!["cancel", "a-1"], "cancel", json!({"job": "a-1"}), ""),
            (vec!["rip-again", "a-1"], "rip_again", json!({"job": "a-1"}), ""),
            (vec!["approve", "a-1"], "review_action", json!({"job": "a-1", "action": "approve"}), ""),
            (vec!["stop"], "shutdown", json!({"force": false}), ""),
            (vec!["stop", "--force"], "shutdown", json!({"force": true}), ""),
        ];
        for (argv, kind, payload, printed) in cases {
            let (ipc, connector) = MemoryIpc::new();
            let answer = vec![selftest_push.clone(), reply_ok(Some(1), json!([]))];
            let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(answer));
            let (code, out, err) = call(&argv, Some(&connector), b"", &scratch.0);
            let request = request_of(&server.join().unwrap());
            assert_eq!(request, json!({"v": 1, "type": kind, "id": 1, "payload": payload}), "{argv:?}");
            assert_eq!((code, out.as_str(), err.as_str()), (0, printed, ""), "{argv:?}");
        }
    }

    #[test]
    fn secret_set_sends_stdins_text_without_its_last_newline() {
        let scratch = Scratch::new("ctl-secret");
        for (name, stdin, value) in [
            ("tmdb_token", &b"tok-123\r\n"[..], "tok-123"),
            (r"share:\\nas\media", &b"user\npass\n"[..], "user\npass"),
        ] {
            let (ipc, connector) = MemoryIpc::new();
            let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![reply_ok(Some(1), Value::Null)]));
            let (code, out, err) = call(&["secret", "set", name], Some(&connector), stdin, &scratch.0);
            let request = request_of(&server.join().unwrap());
            assert_eq!(request["type"], "secret_set");
            assert_eq!(request["payload"], json!({"name": name, "value": value}));
            assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
        }
    }

    #[test]
    fn an_error_reply_exits_1_with_its_message_and_problems() {
        let scratch = Scratch::new("ctl-error");
        let file = scratch.write_path("change.json", r#"{"cache_mb": "big"}"#);
        let (ipc, connector) = MemoryIpc::new();
        let problem = SettingsProblem { key: "cache_mb".to_string(), problem: "not a number".to_string() };
        let server = peer(
            ipc,
            vec![SNAPSHOT.to_string()],
            Some(vec![reply_error(Some(1), "the settings were refused", &[problem])]),
        );
        let (code, out, err) = call(&["settings", "set", file.to_str().unwrap()], Some(&connector), b"", &scratch.0);
        let request = request_of(&server.join().unwrap());
        assert_eq!(request, json!({"v": 1, "type": "settings_set", "id": 1, "payload": {"cache_mb": "big"}}));
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (1, "", "lazymkvctl: the settings were refused\nlazymkvctl: cache_mb: not a number\n")
        );
    }

    #[test]
    fn a_connection_closed_before_the_reply_exits_4() {
        let scratch = Scratch::new("ctl-closed");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![]));
        let (code, out, err) = call(&["retry", "a-1"], Some(&connector), b"", &scratch.0);
        server.join().unwrap();
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (4, "", "lazymkvctl: the engine closed the connection before replying\n")
        );
    }

    #[test]
    fn a_relayed_command_prints_its_output_lines_and_exits_with_the_childs_code() {
        let scratch = Scratch::new("ctl-relay");
        let (ipc, connector) = MemoryIpc::new();
        let stdout = Push::CommandOutput { stream: OutputStream::Stdout, line: "{".to_string() };
        let stderr = Push::CommandOutput { stream: OutputStream::Stderr, line: "warning: x".to_string() };
        let server = peer(
            ipc,
            vec![SNAPSHOT.to_string()],
            Some(vec![
                push_line(&stdout, Some(1)),
                push_line(&stderr, Some(1)),
                r#"{"v":1,"type":"job_update","id":null,"payload":{"id":"a-1"}}"#.to_string(),
                reply_ok(Some(1), json!({"exit_code": 3})),
            ]),
        );
        let argv = ["decide", "--job", "a-1", "--title", "t00-00800"];
        let (code, out, err) = call(&argv, Some(&connector), b"", &scratch.0);
        let request = request_of(&server.join().unwrap());
        assert_eq!(request, json!({"v": 1, "type": "command", "id": 1, "payload": {"words": argv}}));
        assert_eq!((code, out.as_str(), err.as_str()), (3, "{\n", "warning: x\n"));
    }

    #[test]
    fn a_refused_relayed_command_exits_2() {
        let scratch = Scratch::new("ctl-relay-refused");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(
            ipc,
            vec![SNAPSHOT.to_string()],
            Some(vec![reply_error(Some(1), "a command is already running", &[])]),
        );
        let (code, out, err) = call(&["plan", "--job", "a-1"], Some(&connector), b"", &scratch.0);
        server.join().unwrap();
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", "lazymkvctl: a command is already running\n"));
    }

    #[test]
    fn retune_jobs_sends_the_spec_path_made_absolute() {
        let scratch = Scratch::new("ctl-retune-spec");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![reply_ok(Some(1), json!({"exit_code": 0}))]));
        let (code, _, err) = call(&["retune", "--jobs", "--spec", "spec.md"], Some(&connector), b"", &scratch.0);
        let request = request_of(&server.join().unwrap());
        let absolute = scratch.0.join("spec.md").to_str().unwrap().to_string();
        assert_eq!(request["payload"]["words"], json!(["retune", "--jobs", "--spec", "spec.md", "--spec", absolute]));
        assert_eq!((code, err.as_str()), (0, ""));
    }
}
```

- [ ] **Step 2: Write the failing engine tests in `crates/fake-tools/tests/ctl_engine.rs`**

```rust
//! lazymkvctl's engine commands against the real engine over the in-memory transport (design part 9.6, Task 28).

use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_ctl::{run, Env};
use lazymkv_dev::evidence::Ctx;
use lazymkv_engine::testkit::Harness;
use serde_json::{json, Value};
use std::path::Path;

/// An engine on the movie-direct scenario; with `filed_job`, its disc is taken to Filed and the engine restarted, so
/// that the job is Filed and no step runs. Returns the job's ID when there is one.
fn engine(filed_job: bool) -> (Harness, Option<String>) {
    let harness = Harness::new(
        Path::new(env!("CARGO_BIN_EXE_fake-tool")),
        &["movie-direct"],
        json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}),
    );
    if !filed_job {
        return (harness, None);
    }
    harness.insert("tok0", "movie-direct");
    harness.run_until("Filed");
    harness.restart();
    let id = std::fs::read_dir(harness.data().join("jobs")).unwrap().next().unwrap().unwrap().file_name();
    (harness, Some(id.into_string().unwrap()))
}

fn ctl(harness: &Harness, argv: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let scratch = Scratch::new("ctl-engine");
    let ctx = Ctx::for_test(&scratch.0);
    let connector = harness.client();
    let mut input = stdin;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
    let code = run(&argv, Env { ctx: &ctx, connector: Some(&connector), stdin: &mut input, out: &mut out, err: &mut err });
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn status_prints_the_snapshot() {
    let (harness, id) = engine(true);
    let id = id.unwrap();
    let (code, out, err) = ctl(&harness, &["status"], b"");
    assert_eq!((code, err.as_str()), (0, ""));
    let snapshot = json_of(&out);
    let keys: Vec<&str> = snapshot.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["jobs", "settings", "problems", "secrets", "selftest"]);
    assert_eq!(snapshot["jobs"][0]["id"], id.as_str());
    assert_eq!(snapshot["jobs"][0]["state"], json!({"state": "Filed"}));
    assert!(snapshot["selftest"]["checks"].is_array());
    assert!(out.starts_with("{\n \"jobs\": ["), "{out}");
}

#[test]
fn history_sends_before_and_prints_the_page() {
    let (harness, id) = engine(true);
    let id = id.unwrap();
    let (code, out, _) = ctl(&harness, &["history"], b"");
    let page = json_of(&out);
    assert_eq!((code, page.as_array().unwrap().len()), (0, 1));
    assert_eq!(page[0]["id"], id.as_str());
    let (code, out, _) = ctl(&harness, &["history", "--before", id.as_str()], b"");
    assert_eq!((code, json_of(&out)), (0, json!([])));
}

#[test]
fn settings_get_prints_the_snapshots_settings() {
    let (harness, _) = engine(false);
    let (code, out, err) = ctl(&harness, &["settings", "get"], b"");
    assert_eq!((code, err.as_str()), (0, ""));
    let settings = json_of(&out);
    assert_eq!(settings["keep_tuning_evidence"], false);
    assert_eq!(settings["drive_settle_s"], 0);
    assert_eq!(settings["cache_mb"], 1024);
    assert_eq!(settings, json_of(&ctl(&harness, &["status"], b"").1)["settings"]);
}

#[test]
fn settings_set_reads_a_file() {
    let (harness, _) = engine(false);
    let files = Scratch::new("ctl-settings-set");
    let good = files.write_path("good.json", r#"{"cache_mb": 512}"#);
    let bad = files.write_path("bad.json", r#"{"cache_mb": "big"}"#);
    let (code, out, err) = ctl(&harness, &["settings", "set", good.to_str().unwrap()], b"");
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    assert_eq!(json_of(&ctl(&harness, &["settings", "get"], b"").1)["cache_mb"], 512);
    let saved = json_of(&std::fs::read_to_string(harness.data().join("settings.json")).unwrap());
    assert_eq!(saved["cache_mb"], 512);
    let (code, _, err) = ctl(&harness, &["settings", "set", bad.to_str().unwrap()], b"");
    assert_eq!(code, 1);
    assert!(err.starts_with("lazymkvctl: "), "{err}");
    assert_eq!(json_of(&ctl(&harness, &["settings", "get"], b"").1)["cache_mb"], 512);
}

#[test]
fn secret_set_reads_the_value_from_stdin() {
    let (harness, _) = engine(false);
    let before = json_of(&ctl(&harness, &["status"], b"").1);
    assert_eq!(before["secrets"]["ha_webhook_url"]["set"], false);
    let url = b"http://ha.local:8123/api/webhook/lazymkv-test\n";
    let (code, out, err) = ctl(&harness, &["secret", "set", "ha_webhook_url"], url);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    let (_, status, _) = ctl(&harness, &["status"], b"");
    assert_eq!(json_of(&status)["secrets"]["ha_webhook_url"]["set"], true);
    assert!(!status.contains("lazymkv-test"), "{status}");
    let (code, out, err) = ctl(&harness, &["secret", "set", "nope"], b"hidden-value\n");
    assert_eq!(code, 1);
    assert!(err.starts_with("lazymkvctl: "), "{err}");
    assert!(!out.contains("hidden-value") && !err.contains("hidden-value"));
}

#[test]
fn selftest_sends_run_selftest_and_prints_the_result() {
    let (harness, _) = engine(false);
    let (code, out, err) = ctl(&harness, &["selftest"], b"");
    assert_eq!((code, err.as_str()), (0, ""));
    let result = json_of(&out);
    let checks = result["checks"].as_array().unwrap();
    let names: Vec<&str> = checks.iter().map(|check| check["name"].as_str().unwrap()).collect();
    for tool in ["makemkvcon", "mkvmerge", "ffprobe"] {
        assert!(names.contains(&tool), "{names:?}");
    }
    let makemkvcon = checks.iter().find(|check| check["name"] == "makemkvcon").unwrap();
    assert_eq!(makemkvcon["found"], "settings");
}

#[test]
fn retry_cancel_rip_again_approve_and_stop_map_ok_to_0_and_error_to_1() {
    let (harness, id) = engine(true);
    let id = id.unwrap();
    let cases: [(&[&str], i32, Option<&str>); 6] = [
        (&["rip-again", id.as_str()], 0, None),
        (&["retry", "nosuch-1"], 1, None),
        (&["cancel", id.as_str()], 1, Some("Filed")),
        (&["approve", id.as_str()], 1, Some("Filed")),
        (&["rip-again", "nosuch-1"], 1, None),
        (&["stop"], 0, None),
    ];
    for (argv, expected, names) in cases {
        let (code, out, err) = ctl(&harness, argv, b"");
        assert_eq!((code, out.as_str()), (expected, ""), "{argv:?}: {err}");
        if expected == 0 {
            assert_eq!(err, "", "{argv:?}");
        } else {
            assert!(err.starts_with("lazymkvctl: "), "{argv:?}: {err}");
            if let Some(state) = names {
                assert!(err.contains(state), "{argv:?}: {err}");
            }
        }
    }
}

#[test]
fn plan_job_with_a_refused_command_exits_2() {
    let (harness, _) = engine(false);
    let (code, out, err) = ctl(&harness, &["plan", "--job", "nosuch-1"], b"");
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("lazymkvctl: "), "{err}");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-ctl -p fake-tools --locked`

Expected: compile success, then FAIL. Task 27's `run` refuses `status` with exit 2, so for example `no_connector_exits_4_with_the_no_transport_message` fails with `left: (2, ...` / `right: (4, ...`. Every `ctl_engine.rs` test fails on its exit code.

- [ ] **Step 4: Implement `engine_cmds.rs` and route to it from `run`**

In `crates/ctl/src/lib.rs`, change `run` (Task 27) in two places and leave every other path as Task 27 wrote it:
- When `argv[0]` is a name in `engine_cmds::ENGINE_COMMANDS`, return `engine_cmds::run(argv, env)`.
- When `argv[0]` is in `LOCAL_COMMANDS` and Task 27's parse found `--job` (`plan`, `decide`) or `--jobs` (`retune`), return `engine_cmds::relay(argv, env)`. This replaces Task 27's handling of that form.

In `crates/ctl/src/engine_cmds.rs`, write the module doc naming design parts 9.5 and 9.6, then the items in the Produces block.

`run` reads the words with `Args`, as the table below says. Each request is sent as one line: `serde_json::to_string(&Envelope { v: PROTOCOL_VERSION, kind, id: Some(1), payload })` followed by `\n`, then a flush.

| Command | Checked before connecting (T28-h; a failure is a usage error, exit 2) | Request `type` | `payload` | Printed on `ok` |
|---|---|---|---|---|
| `status` | no words | none: reads the first `snapshot` push | | that push's payload (T28-a) |
| `watch` | no words | none | | every line as received, until EOF (T28-b) |
| `history [--before JOB]` | `--before` has a value; no other words | `history` | `{"before": JOB or null}` | `result` |
| `settings get` | exactly `get` | none: reads the first `snapshot` push | | its `settings` |
| `settings set FILE` | exactly `set FILE`; FILE reads (else `<FILE>: <io error>`) and parses as a JSON object (else `<FILE>: not a JSON object`) | `settings_set` | FILE's object | nothing |
| `secret set NAME` | exactly `set NAME`; the standard-input value (T28-f) is not empty (else `no value on standard input`) | `secret_set` | `{"name", "value"}` | nothing |
| `selftest` | no words | `run_selftest` | `null` | the last `selftest_result` payload (T28-i) |
| `retry JOB`, `cancel JOB`, `rip-again JOB` | exactly one JOB | `retry`, `cancel`, `rip_again` | `{"job"}` | nothing |
| `approve JOB` | exactly one JOB | `review_action` | `{"job", "action": "approve"}` | nothing |
| `stop [--force]` | `--force` is a switch; no other words | `shutdown` | `{"force": bool}` | nothing |

`relay` works like this:
- For `retune`, it calls Task 27's `retune_spec(ctx, &argv[1..])`. An Err is a usage error with `RETUNE_RELAY_USAGE` and that text (`the following arguments are required: --spec` when neither `--spec` nor the repository gives a path).
- The request is `command` with `{"words": argv, plus "--spec", <absolute path> for retune}` (T28-g).

The exchange is one loop over the connection's lines, shared by `run` and `relay`:
1. With no connector, print `NO_TRANSPORT` and return 4. A `connect` error prints the matching constant and returns 4.
2. Read one line at a time.
   - 0 bytes read ends the loop as T28-d says.
   - Each line goes through `read_reply(line.trim_end())`.
3. `Push` lines:
   - A `command_output` push with `id` `Some(1)` writes its `line` to `out` or `err`, chosen by `stream`.
   - A `selftest_result` push is kept.
   - A `snapshot` push ends `status` and `settings get`.
   - Any other push is skipped.
4. `Ok` with `id` `Some(1)`:
   - `relay` returns `result.exit_code`. An `ok` without an integer `exit_code` exits 4, with `lazymkvctl: the engine's reply has no exit_code`.
   - `run` prints as the table says and returns 0.
5. `Error` with `id` `Some(1)` prints as T28-e says, then `run` returns 1 and `relay` returns 2.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-ctl -p fake-tools --locked`

Expected: PASS. The `lazymkv-ctl` unit tests include the 13 new ones, and `tests/ctl_engine.rs` reports `test result: ok. 8 passed; 0 failed`. Task 27's tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ctl/src/engine_cmds.rs crates/ctl/src/lib.rs crates/fake-tools/tests/ctl_engine.rs
git commit -m "ctl: engine commands over the in-memory transport (design part 9.6)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, followed by the message only, with no attribution line.

### Task 29: The whole job on three systems

**Files:**
- Create: `crates/fake-tools/tests/whole_job.rs`, with 2 tests.
- Modify: `.github/workflows/ci.yml:3-6`, the stale comment.

**Interfaces:**
- Consumes:
  - From the engine testkit (Tasks 22, 23 and 26): `Harness::new(fake_tool: &Path, scenarios: &[&str], settings: serde_json::Value)`, `insert(drive, disc_name)` (read with `disc_name` as the scenario folder, as in Task 28), `notifications() -> Vec<Notification>`, `ejector().calls() -> Vec<String>`, `awake().holders() -> usize`, `data()`, `http()` and `host()` (a `FakeHost`, read through `lazymkv_platform::Host::now_unix`, Task 8).
  - `http()` is a `RecordedTransport` with `requests() -> Vec<(String, Vec<(String, String)>)>` and `unrecorded() -> Vec<String>`.
  - From core:
    - `core::ipc::Notification { event: job::NotifyEvent (flattened), job_id, fingerprint, title, drive, message, time }` (Task 7).
    - `core::job::NotifyEvent` (crates/core/src/job.rs:413).
    - `core::lookup::name::disc_name` (crates/core/src/lookup/name.rs:367) and `DiscName.cleaned` (name.rs:347).
    - `core::lookup::tmdb::{TmdbIdentity { kind, id }, TmdbKind}` (crates/core/src/lookup/tmdb.rs:54, :22).
    - `core::audio::rule::RULE_VERSION` (crates/core/src/audio/rule.rs:21).
  - From clients (Task 15): `dischash_url`, `discid_url`, `search_url` and `record_url`.
  - From dev: `lazymkv_dev::dispatch(ctx, argv, out, err) -> i32` (crates/dev/src/lib.rs:50), `Ctx::for_test` (crates/dev/src/evidence.rs:45), and `pyjson::{dumps, Style::INDENT1_ASCII}` (pyjson.rs:32, :24).
  - The main-line rows of `direct_line` (crates/core/src/job.rs:802-842), the title key `t00-00800` (`title_key`, crates/core/src/plan/mod.rs:77), the navigation files of `is_nav_file` (crates/capture/src/disc.rs:57, moved to core in Task 3), and the scenario files of Task 23.
- Produces: no code interface. The test pins design part 12.6 item 2's acceptance.

Rulings settled in this task:
- **Ruling T29-a.** The test waits by polling `notifications()` until one that is not `disc_done` arrives, and then until `awake().holders() == 0`. It does not use `run_until`. Why: `run_until` stops after the transition into the state is persisted, which is before that transition's actions run (Task 22), so the `filed` notification and the keep-awake release would be missed.
- **Ruling T29-b.** The job folder's later drive listings follow RP-1 (a) (Task 23), through the constant `LATER_LISTINGS`, which is `&["drive-list-2.txt"]`, the listing before the rip. Why: assertion 6 lists the folder exactly.
- **Ruling T29-c.** Assertion 8 lists the data folder exactly, `selftest/` included: Task 26 records the MakeMKV check at engine start there (S79), with the recorded state of RP-9. No self-test runs in this test, so no tool's version file is there.

- [ ] **Step 1: Write the tests in `crates/fake-tools/tests/whole_job.rs`**

```rust
//! Design part 12.6 item 2's acceptance: a media-arrival event takes a synthetic movie disc to a filed placeholder
//! file, on stand-in tools, in-memory platform fakes and recorded HTTP answers, with no disc, no network and no
//! Windows API.

use lazymkv_clients::{dischash_url, discid_url, record_url, search_url};
use lazymkv_core::job::NotifyEvent;
use lazymkv_core::lookup::name::disc_name;
use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Host;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

const KEY: &str = "t00-00800";
const FILM: &str = "Synthetic Movie (2001) {tmdb-900001}";
/// The rip's own listing (RP-1, Ruling T29-b).
const LATER_LISTINGS: &[&str] = &["drive-list-2.txt"];

fn scenario_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scenarios/movie-direct")
}

/// Every file under `dir`, relative to it with '/' separators; empty when `dir` does not exist.
fn files_under(dir: &Path) -> BTreeSet<String> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/");
                found.insert(relative);
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(dir, dir, &mut found);
    found
}

/// The names directly inside `dir`; empty when it does not exist.
fn names_in(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .map(|entries| entries.map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect())
        .unwrap_or_default()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Starts the engine on movie-direct, inserts its disc in tok0, waits for the job's last notification and the
/// keep-awake release (Ruling T29-a), and returns the harness with the job folder.
fn filed_job() -> (Harness, PathBuf) {
    let harness = Harness::new(
        Path::new(env!("CARGO_BIN_EXE_fake-tool")),
        &["movie-direct"],
        json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}),
    );
    harness.insert("tok0", "movie-direct");
    let start = Instant::now();
    loop {
        let last = harness.notifications().iter().any(|n| n.event != NotifyEvent::DiscDone);
        if last && harness.awake().holders() == 0 {
            break;
        }
        let sent: Vec<String> = harness.notifications().iter().map(|n| serde_json::to_string(n).unwrap()).collect();
        assert!(start.elapsed() < Duration::from_secs(120), "no last notification; sent: {sent:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let jobs: Vec<PathBuf> =
        fs::read_dir(harness.data().join("jobs")).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = jobs[0].clone();
    (harness, job)
}

#[test]
fn a_media_arrival_takes_a_synthetic_movie_disc_to_a_filed_placeholder() {
    let scenario_before = files_under(&scenario_dir());
    let (harness, job) = filed_job();
    let settings = read_json(&job.join("settings.json"));
    let root_text = |key: &str| settings[key].as_str().unwrap().to_string();
    let (movies_text, staging, tuning) =
        (root_text("movies_root"), PathBuf::from(root_text("staging_root")), PathBuf::from(root_text("tuning_root")));
    let movies = PathBuf::from(&movies_text);
    let meta = read_json(&job.join("meta.json"));
    let fp = meta["fingerprint"].as_str().unwrap().to_string();
    let id = format!("{}-1", fp.to_ascii_lowercase());
    assert_eq!(job.file_name().unwrap().to_str().unwrap(), id);

    // 1. The placeholder is filed at the namer's path (spec 8.9), joined with the root's own separator.
    let placeholder = fs::read(scenario_dir().join("placeholder.mkv")).unwrap();
    let filed = movies.join(FILM).join(format!("{FILM} - 1080p.mkv"));
    assert_eq!(fs::read(&filed).unwrap(), placeholder);
    let sep = if movies_text.contains('\\') { '\\' } else { '/' };
    let filed_text = format!("{movies_text}{sep}{FILM}{sep}{FILM} - 1080p.mkv");

    // 2. state.json.
    let state = read_json(&job.join("state.json"));
    assert_eq!(state["format"], "lazymkv job v1");
    assert_eq!(state["id"], id.as_str());
    assert_eq!(state["state"], json!({"state": "Filed"}));
    assert_eq!(state["filed_unix"], json!(harness.host().now_unix()));
    let mtime = fs::metadata(&filed).unwrap().modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(state["titles"][0]["key"], KEY);
    assert_eq!(
        state["titles"][0]["filed"],
        json!({"path": filed_text, "size": placeholder.len(), "mtime_unix": mtime})
    );

    // 3. transitions.jsonl: the main line of job.rs:802-842, in order.
    let lines: Vec<Value> = fs::read_to_string(job.join("transitions.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let pairs: Vec<(Option<&str>, &str)> =
        lines.iter().map(|line| (line["event"]["event"].as_str(), line["new"]["state"].as_str().unwrap())).collect();
    assert_eq!(
        pairs,
        vec![
            (None, "Detected"),
            (Some("FingerprintDone"), "Fingerprinted"),
            (Some("ScanDone"), "Scanned"),
            (Some("PlanDone"), "Planned"),
            (Some("SpaceOk"), "Ripping"),
            (Some("TitleRipped"), "Ripping"),
            (Some("TitleVerified"), "RipVerified"),
            (Some("Ejected"), "Ejected"),
            (Some("FilePhaseSlot"), "Inspecting"),
            (Some("ProbeDone"), "Inspecting"),
            (Some("DecisionDone"), "Decided"),
            (Some("SpaceOk"), "Remuxing"),
            (Some("RemuxDone"), "Remuxing"),
            (Some("FileVerified"), "Verified"),
            (Some("FileDone"), "Filed"),
        ]
    );
    assert!(lines[0]["old"].is_null() && lines[0]["event"].is_null());
    assert_eq!(lines[3]["event"], json!({"event": "PlanDone", "mode": "direct"}));
    assert_eq!(lines[6]["event"], json!({"event": "TitleVerified", "last": true}));
    assert_eq!((&lines[10]["event"]["disposition"], &lines[10]["event"]["has_path"]), (&json!("auto_file"), &json!(true)));
    for pair in lines.windows(2) {
        assert_eq!(pair[1]["old"], pair[0]["new"]);
    }
    for line in &lines {
        assert_eq!(line["time_unix"], json!(harness.host().now_unix()));
    }
    assert!(lines[14]["actions"].as_array().unwrap().contains(&json!({"action": "Notify", "event": {"event": "filed"}})),
        "{}", lines[14]["actions"]);

    // 4. Exactly disc_done then filed, with spec 12's fields.
    let sent = harness.notifications();
    let events: Vec<&NotifyEvent> = sent.iter().map(|n| &n.event).collect();
    assert_eq!(events, vec![&NotifyEvent::DiscDone, &NotifyEvent::Filed]);
    let payload_events: Vec<Value> = sent.iter().map(|n| serde_json::to_value(n).unwrap()["event"].clone()).collect();
    assert_eq!(payload_events, vec![json!("disc_done"), json!("filed")]);
    for n in &sent {
        assert!(!n.message.is_empty());
        assert_eq!(n.job_id.as_deref(), Some(id.as_str()));
        assert_eq!(n.fingerprint.as_deref(), Some(fp.as_str()));
        assert_eq!(n.drive.as_deref(), Some("tok0"));
        assert_eq!(n.title.as_deref(), Some("Synthetic Movie"));
        assert!(n.time > 0);
    }

    // 5. One eject, keep-awake released.
    assert_eq!(harness.ejector().calls(), vec!["tok0".to_string()]);
    assert_eq!(harness.awake().holders(), 0);

    // 6. The job folder holds exactly design part 7.1's files for this path.
    let mut expected = set(&[
        "meta.json", "files.json", "nav/BDMV/index.bdmv", "nav/BDMV/PLAYLIST/00800.mpls", "nav/BDMV/META/DL/bdmt_eng.xml",
        "thediscdb-hash.json", "thediscdb-discid.json", "tmdb.json", "tmdb-search.json", "tmdb-record.json",
        "lookup-status.txt", "settings.json", "state.json", "transitions.jsonl", "job.log", "job.mmcp.xml",
    ]);
    expected.insert(format!("{KEY}/rip-summary.json"));
    expected.insert(format!("{KEY}/decide.json"));
    let mut runs: Vec<String> = vec!["drive-list.txt".into(), "scan.txt".into()];
    runs.extend(LATER_LISTINGS.iter().map(|name| name.to_string()));
    for name in ["rip.txt", "mkvmerge.json", "ffprobe-streams.json", "remux-mkvmerge.txt", "partial-mkvmerge.json"] {
        runs.push(format!("{KEY}/{name}"));
    }
    for run in &runs {
        for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
            expected.insert(format!("{run}{suffix}"));
        }
    }
    assert_eq!(files_under(&job), expected);

    // 7. R-B1 on stand-ins: lazymkv-dev decide over the job folder prints decide.json without its two extra keys.
    let mut decide = read_json(&job.join(KEY).join("decide.json"));
    let object = decide.as_object_mut().unwrap();
    let last_two: Vec<&str> = object.keys().rev().take(2).map(String::as_str).collect();
    assert_eq!(last_two, ["lazymkv_version", "audio_rule_version"]);
    assert_eq!(object["audio_rule_version"], json!(lazymkv_core::audio::rule::RULE_VERSION));
    // The workspace gives every package one version (Cargo.toml:7), so fake-tools' version is the engine's.
    assert_eq!(object["lazymkv_version"], json!(env!("CARGO_PKG_VERSION")));
    object.shift_remove("audio_rule_version");
    object.shift_remove("lazymkv_version");
    let path = |name: &str| job.join(name).to_str().unwrap().to_string();
    let mut argv: Vec<String> = vec![
        "decide".into(), "--rec".into(), path(KEY), "--settings".into(), path("settings.json"),
        "--tmdb-search".into(), path("tmdb-search.json"), "--tmdb-record".into(), path("tmdb-record.json"),
    ];
    if job.join("remembered.json").exists() {
        argv.extend(["--remembered".to_string(), path("remembered.json")]);
    }
    let ctx = Ctx::for_test(&job);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(&ctx, &argv, &mut out, &mut err);
    assert_eq!((code, String::from_utf8(err).unwrap()), (0, String::new()));
    assert_eq!(String::from_utf8(out).unwrap(), dumps(&decide, Style::INDENT1_ASCII) + "\n");

    // 8. Nothing written outside the scratch roots and the data folder.
    assert_eq!(files_under(&scenario_dir()), scenario_before);
    assert!(files_under(&tuning).is_empty());
    assert_eq!(names_in(harness.data().as_ref()), set(&["drives.json", "engine.log", "jobs", "selftest", "settings.json"]));
    // The MakeMKV check at engine start (S79) and its recorded state (RP-9).
    assert_eq!(
        files_under(&harness.data().join("selftest")),
        set(&["drive-list.txt", "drive-list.txt.stderr", "drive-list.txt.timing.jsonl", "drive-list.txt.exit.json",
              "makemkv-check.json"])
    );
    assert_eq!(names_in(&staging), set(&[id.as_str()]));
    assert_eq!(names_in(&movies), set(&[".lazymkv-partial", FILM]));

    // 9. The staged raw file stays (retention, design part 5.9); no partial file is left.
    assert_eq!(names_in(&staging.join(&id).join("rip").join(KEY)), set(&["title_t00.mkv"]));
    assert!(names_in(&movies.join(".lazymkv-partial")).is_empty());

    // 10. Upper-case fingerprints, no notes, and the drive record after the removal its own eject causes (Task 22 Step 3
    // item 6: every removal writes a null fingerprint and handled false).
    assert!(fp.len() == 32 && fp.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_lowercase()), "{fp}");
    assert_eq!(state["fingerprint"], fp.as_str());
    assert_eq!(state["notes"], json!([]));
    assert_eq!(
        read_json(&harness.data().join("drives.json")),
        json!({"format": "lazymkv drives v1", "drives": {"tok0": {"fingerprint": null, "handled": false}}, "drive_incapable": []})
    );
}

#[test]
fn the_whole_job_runs_with_no_unrecorded_url() {
    let (harness, job) = filed_job();
    let meta = read_json(&job.join("meta.json"));
    let name = disc_name(Some("Synthetic Movie"), Some("SYNTHETIC_MOVIE")).unwrap();
    let urls: Vec<String> = harness.http().requests().into_iter().map(|(url, _)| url).collect();
    assert_eq!(
        urls,
        vec![
            dischash_url(meta["fingerprint"].as_str().unwrap()),
            discid_url(meta["disc_id"].as_str().unwrap()),
            search_url(&name.cleaned),
            record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 900001 }),
        ]
    );
    assert_eq!(harness.http().unrecorded(), Vec::<String>::new());
    assert!(harness.notifications().iter().all(|n| matches!(n.event, NotifyEvent::DiscDone | NotifyEvent::Filed)));
}
```

- [ ] **Step 2: Run the tests**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test whole_job`

Expected: it compiles, and `test result: ok. 2 passed; 0 failed` when Tasks 1 to 28 hold. This is an acceptance test over finished pieces, so it may pass on its first run.
- A failure names its numbered assertion. Fix it in the crate that owns that piece:
  - assertions 1 and 9: Task 24;
  - assertions 2, 3 and 10: Tasks 6 and 22;
  - assertion 6: Tasks 11, 13, 14, 23 and 24;
  - assertion 7: Tasks 19, 20 and 24, and RP-4.
- Add a regression test beside the fix, and commit both in this task.

- [ ] **Step 3: Update the CI comment at `.github/workflows/ci.yml:3-6`**

Replace these four lines:

```
# Every push to main and every pull request builds and tests the workspace on Linux, macOS and Windows with the
# toolchain rust-toolchain.toml pins. The private evidence repository is not cloned here, so the tests that need it
# print their "skipped: ..." reason and the rest run (about 560 of them). ffmpeg is installed for the one test that
# builds its own proxy with lavfi; without it that test skips too.
```

with:

```
# Every push to main and every pull request builds and tests the workspace on Linux, macOS and Windows with the
# toolchain rust-toolchain.toml pins. The private evidence repository is not cloned here, so the tests that need it
# print their "skipped: ..." reason and the rest run; docs/phase2/engine.md has the count. Among them, the stand-in
# tools of crates/fake-tools run a whole engine job, from a disc's arrival to a filed placeholder file, with no disc,
# no network and no Windows API (design part 12.6, item 2). ffmpeg is installed for the one test that builds its own
# proxy with lavfi; without it that test skips too.
```

The CI commands at ci.yml:50 and :53 stay unchanged.

- [ ] **Step 4: Run the whole workspace once on the mini PC**

Run: `scripts/box.sh cargo test --workspace --locked`

Expected: every `test result:` line reads `ok`, with `0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/fake-tools/tests/whole_job.rs .github/workflows/ci.yml
git commit -m "fake-tools: the whole stand-in job on ubuntu, macos and windows (design part 12.6, item 2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>` and the message only. Add any fix and regression test from Step 2 to the `git add` line.

- [ ] **Step 6: Confirm CI on three systems**

CI runs when `phase-2-thin-job` is pushed for its pull request. Push only after the owner has said yes to opening the pull request; until then this step waits. With that yes, run `git push -u origin phase-2-thin-job`, then `gh run watch` on the branch's run.

Expected: `test (ubuntu-latest)`, `test (macos-latest)`, `test (windows-latest)` and `repository guards` all succeed. A failure on one OS is fixed in a further commit of this task, with the same signing check.

### Task 30: Documents and the test count

**Files:**
- Create: `docs/phase2/engine.md`.
- Modify: `README.md:23-24` (the phase 2 sentence), `README.md:33-39` (the `crates/` bullet) and `README.md:46-47` (the pointer to the tests).

**Interfaces:**
- Consumes:
  - the plan's Rulings S1 to S88: S1 to S54 in the skeleton's Rulings section, S55 to S88 in Tasks 13 to 15 and 25 to 27, as that section's index lists them;
  - every other task-level ruling of Tasks 1 to 29, numbered or not: T7-a to T7-i, T8-a to T8-g, T9-a to T9-e, R22.1 to R22.6, R23.1 to R23.7, R24.1 to R24.6, T28-a to T28-j, T29-a to T29-c, and each unnumbered "Ruling:" line;
  - the planner's rulings RP-1 to RP-9 on the open points, and any override of them the owner made before execution;
  - the Task 1 lock delta and the date of the owner's yes;
  - the Not covered list;
  - the phase 1 baseline of 2052 tests (docs/phase1/decision-core.md:701). Pull request #5 added no `#[test]` under `crates/`;
  - the URL of the green CI run of Task 29 Step 6 on ubuntu, macos and windows. Task 30 starts only after that run is green.
- Produces: no code interface.

- [ ] **Step 1: Run the full suite with the evidence clone**

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`

Expected: every `test result:` line reads `ok` with `0 failed`. The dev golden tests print no `skipped:` line, because the clone is present. `LAZYMKV_OCR_TEXT` is not set.

- [ ] **Step 2: Count the tests in one workspace run**

Run: `scripts/box.sh "cargo test --workspace --locked -- --list 2>&1" > "${TMPDIR:-/tmp}/lazymkv-test-list.txt"`, then `tr -d '\r' < "${TMPDIR:-/tmp}/lazymkv-test-list.txt" | grep -c ': test$'` on the Mac.

Expected: one integer, N, the count design part 12.6 asks for. Split N per package from the same file: each block of `: test` lines follows a `Running` header (or a `Doc-tests <crate>` header). A `Running unittests src/...` header names its crate in the executable's file name (`lazymkv_core-<hash>`), and a `Running tests\<file>.rs` header belongs to the package whose `tests/` folder holds that file. That gives a to i, which sum to N. Also run `git rev-parse --short HEAD` for the commit the count was taken on. One workspace run resolves features as CI does, so no per-package feature flag is needed.

- [ ] **Step 3: Write `docs/phase2/engine.md`**

Write it in plain sentences with no em-dashes, expanding acronyms at first use, in the shape of `docs/phase1/decision-core.md`. It has these sections, in this order:

1. `# Phase 2: the engine`. One paragraph:
   - Phase 2 of the design spec builds the direct-mode engine, following `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (design part 12.6).
   - The document grows with each of the six pull requests.
   - This first part records pull request 2, on the branch `phase-2-thin-job` from `main` at `70ada2b`.
2. `## Pull request 2: the thinnest whole job`. What it delivers, one bullet per item of design part 12.6 item 2:
   - the engine's reducer loop and persistence;
   - the `tools` drivers;
   - the `platform` traits with in-memory fakes;
   - the clients behind the recorded transport;
   - the stand-in extensions of design part 12.2;
   - `lazymkvctl`'s local `plan`, `decide` and `retune`, and its engine commands over the in-memory transport;
   - the R-B2 move;
   - the K5 split;
   - lower-case job IDs (K3);
   - the interfaces fixed as serde types in `core`.

   Its acceptance sentence: "A media-arrival event takes a synthetic movie disc to a filed placeholder file on ubuntu, macos and windows (`crates/fake-tools/tests/whole_job.rs`; CI run <the URL of Task 29 Step 6's green run>)."
3. `## How to run it`. Three commands, one sentence each:
   - `scripts/box.sh cargo test -p fake-tools --locked --test whole_job` runs the whole job.
   - `scripts/box.sh cargo run -p lazymkv-ctl --locked --bin lazymkvctl -- plan <recording dir>` and the same with `decide --rec <dir>/<label>` print what `lazymkv-dev` prints, byte for byte, with the same exit codes (design part 9.5).
   - In this version `scripts/box.sh cargo run -p lazymkv-engine --locked -- --data DIR` exits 1 on every OS with its OS's sentence (S28). `lazymkvctl`'s engine commands exit 4 with "lazymkvctl: no engine transport exists in this build" (S38). Pull request 3 brings the Windows implementations and the pipe.
4. `## The crates`. A table with the columns Crate (package, binary), Depends on and Holds:
   - one row each for `lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-engine` (bin `lazymkv-engine`) and `lazymkv-ctl` (bin `lazymkvctl`), with the dependencies of Task 1;
   - one row each for the changes to `lazymkv-capture` (it now calls `tools::snapshot`; R-B2) and `lazymkv-dev` (`read_decide_inputs` and the pure `decide_doc`; K5).

   Then two sentences:
   - "The disc snapshot's pure part is `core::snapshot`, and its file input and output is `tools::snapshot`; `capture disc` prints and writes the same bytes as before, which `crates/capture/tests/disc_snapshot.rs` pins."
   - "The goldens are byte-identical; `crates/dev/src/cmd/decide/characterisation/` pinned `decide`'s output across the split."
5. `## The interfaces fixed in core`. One short paragraph each:
   - `core::record`: the `format` strings `lazymkv job v1`, `lazymkv drives v1` and `lazymkv remembered v1`, the `transitions.jsonl` keys `time_unix`, `event`, `old`, `new` and `actions`, and the `drives.json` shape.
   - `core::ipc`: the envelope, the twelve request `type` strings, the five push types and the notification payload.
   - `core::robot::lines`, `core::ustar`, `core::snapshot` and `core::verify::subtitle_events_of`.

   Cite each by its file path.
6. `## The Cargo.lock delta`. The approval record:
   - "On <date of the owner's yes at Task 1> the owner approved the `Cargo.lock` delta of pull request 2: no external crate; five path packages, `lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-engine` and `lazymkv-ctl`, each version 0.0.0 with no `source` line; `lazymkv-capture` and `fake-tools` gain dependency names only."
   - "`windows-sys` waits for pull request 3, and `ureq` with its TLS stack for pull request 4 (design parts 4.7, 12.6)."
7. `## Rulings`. Every ruling S1 to S88, copied one line each: S1 to S54 under the plan's group headings, then S55 to S88 under the task that settles each. Then every other task-level ruling of Tasks 1 to 29, numbered (`T<n>-<letter>`, `R<n>.<m>`) or not, one line each under its task number. End with the sentence "The owner may overturn any of them."
8. `## Rulings on the open points`. RP-1 to RP-9, recorded as the planner's rulings of 2026-10-24, each with the option chosen and where the code follows it, for example "RP-1: (a); `crates/fake-tools/tests/whole_job.rs` `LATER_LISTINGS`". The owner may have overridden any of them before execution, in which case the record names the override and its date.
9. `## Left for later pull requests`. The plan's "Not covered" list, unchanged in substance, grouped by pull request 3, 4, 5, 6 and "never reached in phase 2".
10. `## Tests`.
    - One sentence: "`cargo test --workspace --locked -- --list` on the mini PC (Windows), on the tree of commit `<sha from Step 2>`, lists N tests: a in `lazymkv-core`, b in `lazymkv-dev`, c in `lazymkv-capture`, d in `fake-tools`, e in `lazymkv-platform`, f in `lazymkv-tools`, g in `lazymkv-clients`, h in `lazymkv-engine` and i in `lazymkv-ctl`."
    - The baseline sentence: "The baseline is the 2052 tests of docs/phase1/decision-core.md:701, which pull request #5 left unchanged (it added no test), so pull request 2 added N - 2052." Write the subtraction's result as a number.
    - The sentence on the lists being the mini PC's, as decision-core.md:702 has it.
    - A table with the columns Where and What it tests:
      - `crates/fake-tools/tests/{tools_record,makemkv_driver,mkvmerge_ffprobe_driver,engine_disc_phase,engine_file_phase,engine_restart,engine_ipc,ctl_engine,whole_job}.rs`;
      - each new crate's unit tests;
      - `crates/dev/tests/clone_values.rs`, now over `corpus/scenarios/` too.
    - One sentence on the guard's reach: "`no_scenario_holds_a_clone_value` needs the evidence clone (`LAZYMKV_EVIDENCE`) and skips without it, so the RP21 guard over `corpus/scenarios/` runs only in the mini PC's junction run, not on CI."

- [ ] **Step 4: Edit `README.md`**

At `README.md:23-24`, replace:
```
The direct-mode engine
comes next (the spec's phase 2), with a developer mode that keeps every rip's evidence for tuning.
```
with:
```
Phase 2 builds the
direct-mode engine ([`docs/superpowers/specs/2026-10-21-phase-2-engine-design.md`](docs/superpowers/specs/2026-10-21-phase-2-engine-design.md)),
with a developer mode that keeps every rip's evidence for tuning. Its second pull request runs a whole job on
stand-in tools on any OS, from a disc's arrival to a filed placeholder file, with no disc, no network and no Windows
API; [`docs/phase2/engine.md`](docs/phase2/engine.md) says how to run it and how many tests there are.
```

In the `crates/` bullet (`README.md:33-39`), make three replacements:
- Replace `the job state reducer, and phase 0's track` with `the job state reducer, the persisted job and drive records and the IPC messages, the `makemkvcon` line reader, and phase 0's track`.
- Replace `the phase 0 readers, the goldens). Tests:` with `the phase 0 readers, the goldens), and phase 2's `platform` (the operating-system traits, with in-memory fakes), `tools` (the `makemkvcon`, `mkvmerge` and `ffprobe` drivers, their recorded runs and the disc snapshot's file input and output), `clients` (TheDiscDB and TMDb behind an HTTP transport, replayed from recorded answers in tests), `engine` (the `lazymkv-engine` job owner, its persistence and its IPC server) and `ctl` (`lazymkvctl`: the local `plan`, `decide` and `retune`, and the engine commands). Tests:`.
- At `README.md:46-47`, replace `are in
  [`docs/phase1/decision-core.md`](docs/phase1/decision-core.md).` with `are in
  [`docs/phase1/decision-core.md`](docs/phase1/decision-core.md), and phase 2's in [`docs/phase2/engine.md`](docs/phase2/engine.md).`.

Rewrap the edited paragraphs at 120 columns, as the file is wrapped.

- [ ] **Step 5: Check the documents**

Run: `grep -n $'\xe2\x80\x94' docs/phase2/engine.md README.md; grep -c '^- S[0-9]' docs/phase2/engine.md`

The first grep searches for the UTF-8 bytes of U+2014, the em-dash, written as escapes so that this plan holds none.

Expected: the first grep prints nothing, so neither file holds an em-dash. The count is at least 88. A ruling line that holds its group heading inline counts once.

- [ ] **Step 6: Commit**

```bash
git add docs/phase2/engine.md README.md
git commit -m "docs: phase 2 pull request 2 notes and test count"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>` and the message only, with no attribution line.

## Not covered (and why)

- **PR 3** (design part 12.6, item 3):
  - the Windows implementations, the named pipe and its client, the mutex and the access lists;
  - the `PATH` writable-folder skip, `engine-task.ps1`, the run folder and the `command` relay;
  - PR 3's `DriveWatcher` must report the drive id in MakeMKV's `"D:"` form;
  - L7's present-but-inaccessible folder (S39);
  - `Host`'s local time and the `started` key of `rip-summary.json` (design part 4.6, Ruling R23.7).
- **PR 4** (S40, S47):
  - `ureq`, the live shape tests, J8's tagged search and merge, and the entry-title search of design part 8.2 item 3;
  - TV seasons;
  - webhook delivery and the webhook self-test check;
  - the stale-copy refetch and K15's exit 3;
  - the cleanup task (P4, R-B4).
- **PR 5:**
  - step 10, step 10a, OCR, the evidence trees, `probe-summary.json` and the `ffmpeg` driver;
  - `Note::SubtitleTextUnread` and the unread streams of `SubtitleFacts` (design part 10.8);
  - the zlib inflater (`miniz_oxide`);
  - dev's `ocr-trial`, `retune`'s repeated `--evidence`, and dev's reader of a bare `streamhash.txt`;
  - `licenses` and `export-models`;
  - layer 2 (real tools, MKVToolNix on CI). CI's commands stay unchanged in PR 2.
- **PR 6:** `compare-job`.
- **Never reached in phase 2** (design part 5.2): `DeleteJobData`, `CopyDisc`, `ExtractTitle`, `NetworkNeeded` and `VersionMessage`.

## Rulings on the open points (RP-1 to RP-9)

These are the planner's rulings of 2026-10-24 on the nine points the plan left open, and the owner may override any of them before execution.

- RP-1 (OPEN 1, later drive listings): (a) record each later listing as `drive-list-<n>.txt` with its run files, numbered from 2. Why: design part 7.2 records every external run; a listing that leaves no file breaks that rule. Cost if wrong: a few small files per job.
- RP-2 (OPEN 2, no DRV line for the drive id): (a) feed `ScanFailed` or `TitleRipFailed` with class `Unknown` and a message naming the drive id. Why: the tool is present; the disc or drive is the problem, and `ToolMissing` would send the owner to reinstall MakeMKV. Cost if wrong: a job fails where a wait would have served.
- RP-3 (OPEN 3, a missing `appdata.tar` or `default.mmcp.xml`): (a) `ToolMissing { tool: makemkvcon }`, which leads to AwaitingTool. Why: the MakeMKV installation is incomplete, and AwaitingTool is the state the design gives an incomplete tool (ARCH-9). Cost if wrong: the owner sees "waiting for a tool" for a file problem; the waiting message names the file.
- RP-4 (OPEN 4, decide's plan section of a lookup-pending job): (a) `PlanEntry::Results` built around the persisted refreshed plan and the job's settings. Why: the decision must follow the plan the engine ripped by (ENG-9, R-B1); Task 29's scenario is not lookup-pending, so CI is unchanged. Cost if wrong: a lookup-pending job's `decide.json` differs from a fresh `plan --job`, which design part 12.7 already accepts and `compare-job` (PR 6) explains.
- RP-5 (OPEN 5, where the one scan retry runs): (b) `scan` fails after the first timeout, and the engine lists the drives and scans once more (Task 23 holds the retry and its test). Why: design part 5.4 step 1 reads the drive index before every call and never caches it; the retry is then a visible engine step with its own records. Cost if wrong: one more drive listing per retried scan.
- RP-6 (OPEN 6, the first scan's records after a `MSG:1009`): (a) the second scan replaces `scan.txt` and its run files, and `job.log` keeps the rejected scan's 1009 text. Why: the design lists no `scan-rejected.txt`, and the recording layout stays phase 0's. Cost if wrong: the rejected scan's lines survive only in `job.log`.
- RP-7 (OPEN 7, where the verified size lives): (a) `VerifyFile` in Remuxing sets `TitleRecord.filed` with the final path and the partial's size, and `File` overwrites it with the real mtime. Why: no new field, no second `mkvmerge -J`. Cost if wrong: a restart in Verified reads a size that `File` would have replaced anyway.
- RP-8 (OPEN 8, a torn last line of `transitions.jsonl`): (a) `Store::persist` first writes `\n` when the file does not end in one. Why: one line of code keeps every later line parseable, and the log is read by people and tools. Cost if wrong: none worth naming.
- RP-9 (OPEN 9, the MakeMKV check after a restart): (a) persist the recorded version and licence code in `<data>/selftest/makemkv-check.json`. Why: the comparison needs a baseline that survives a restart, and each job's own scan output may predate a MakeMKV update. Cost if wrong: one small file in the data folder.

## Rulings

**Records and IPC types**
- S1. `state.json` and `transitions.jsonl` keys are as in the sheet. The design names only `format` and `filed_unix`.
- S2. The record types live in `core::record` and `core::ipc`, so the interfaces are fixed in `core` (design part 12.6).
- S3. `drives.json` `drives` is an object keyed by drive id. Design part 7.4 says "keyed by", and string keys leave RP23 untouched.
- S4. The job summary's `state` is the nested `State`, with `class` and `message` beside it (design part 9.3).
- S7. `progress.step` is the `Action` serde tag, one spelling across the IPC and the reducer.
- S8. One `SelfTest` type serves both `snapshot.selftest` and `selftest_result`.
- S10. The envelope `id` is `Option<u64>`.
- S41. Job notes are `Vec<review::Noted>`, sorted by note. This reuses core's existing note-with-detail type, so no new type is needed.
- S42. `Notification.time` is Unix seconds, because design part 4.6 writes new times as Unix seconds.

**Line reducer and tool runs**
- S5. A saved title is `MSG:5036` or `MSG:5005` with first param `"1"`. Only `5036` has a public line to pin.
- S6. The `PRGV` fraction is total over max, which is the whole run's progress.
- S16. `tools` records no stderr timing, matching capture and design part 7.2.
- S17. `tools` takes over `tee_lines`, `with_suffix` and `Clock`, so there is one recorder.
- S18. PR 2's tool lookup runs settings, resources, the install folder (a `Host` fake) and `PATH`. The writable-folder skip is PR 3.
- S44. Production limits come from each job's settings (`Limits::from_settings`). `EngineConfig.limits_override` exists for tests only.
- S45. The drive listing gets `--debug` while `keep_tuning_evidence` is on, because design parts 5.3 and 7.2 say every `makemkvcon` call does.

**Platform and stand-ins**
- S9. The writer `DiscMeta` stays beside the reader `recording::files::Meta`, because merging them would print `captured_unix` as a float and change capture's output.
- S11. "Same volume" comes from `ShareConnector::volume`, so no drive-letter rule runs on Linux and macOS.
- S12. Tests that run `fake-tool` live in `crates/fake-tools/tests/`, the only crate with `CARGO_BIN_EXE_fake-tool`.
- S13. PR 2's atomic write and no-replace rename use std. PR 3 swaps in `MoveFileExW`.
- S14. All of design part 12.2's code features land in PR 2. The `ffmpeg` stand-in answers only the self-test's `-version` call (Task 26) and its own test until PR 5 adds the step 10a driver.
- S15. The stand-in reads the scenario beside its executable when `LAZYMKV_FAKE_DIR` is unset, which avoids environment races between parallel tests.
- S43. `Spawn` gains `env`, the one seam a test helper child needs. The engine passes none.
- S51. Scenario variants hold only their differing files, and are laid over `movie-direct` at test time.

**Clients and lookups**
- S19. PR 2's clients cover the movie path only, with one round (design part 12.6, item 4).
- S20. The search URL has only `query`, which leaves TMDb's language default alone.
- S21. The `lookup-status.txt` words are `dischash`, `discid`, `search` and `tmdb movie/<id>`. A lookup with no answer writes `000`, and a body that does not parse writes `unparsed`.
- S22. `core::lookup::record_to_fetch` exposes the existing plausibility helpers, keeping one copy of the rule.
- S23. A request writes `000` when it starts, so dev reads an in-flight lookup as pending.
- S31. The CI disc is TMDb-only (confidence 0.90). Its fingerprint and disc ID are computed at test time and substituted into `http.json`.
- S46. TheDiscDB's 404, and a 200 that `parse_dischash` reads as None, are both `Miss`. `Answered` always carries a record.
- S47. The entry-title search of design part 8.2 item 3 goes to PR 4, with S19.

**dev's decide (K5)**
- S24. The reader runs `reconcile`, and `decide_doc` is infallible, as K5 writes it.
- S25. The renderer recomputes the verify report and uses core's `subtitle_events_of`, so there is one copy of each rule.
- S26. Each `decide.json` decides its title alone. `same_path_reviews` applies only to the decisions the engine feeds.
- S27. decide's three replay flags land in PR 2, for Task 29's R-B1 check.
- S48. In the renderer, a `subs_index` of `Ok(None)` gives no text decisions. A `Results` subtitles section implies that `subs_run` read that index, so the case cannot arise from a recording.
- S49. With `--tmdb-record`, decide's decision names the file from that record, as the plan already does (`PlanOptions.tmdb_record` replaces `tmdb.json`).

**Engine**
- S28. The PR 2 engine binary exits 1 on every OS, with its OS's sentence.
- S29. A new job persists `Detected` before `FingerprintDone`, so a restart in `Detected` fingerprints again.
- S30. Plan time uses `LibraryFacts::default()`, because design part 6.4 reads the library only before Decide.
- S32. When `step10_runs` is true, PR 2 feeds `DecisionFailed { Unknown }` with the message `subtitle evidence (step 10) is not built in this version`.
- S33. PR 2 runs no step 10a pass, and `job.log` says so.
- S34. decide's plan section is `plan_doc` over the job folder, with its `.plan` replaced by the plan the job decided with (RP-4 (a); Task 24).
- S35. The `command` relay goes to PR 3, because its child runs from the run folder.
- S36. Notifications go to a `NotifySink`. Delivery moves to PR 4.
- S50. The job profile's unique name is `lazymkv-<job id>`.
- S52. A client's writer channel holds 256 messages. A full channel drops the client (design part 4.4).
- S53. PR 2's self-test checks the tools, the three roots and the TMDb token. The webhook check is PR 4.
- S54. Before the rip, a rip's promise for its own remux is `write_bytes_needed` of the planned titles' sizes. At the remux check it is the staged files' sizes.

**lazymkvctl**
- S37. ctl keeps dev's stderr program name, because design part 9.5 promises identical documents and exit codes, not identical stderr.
- S38. The PR 2 `lazymkvctl` binary has no connector, and exits 4 with `NO_TRANSPORT`.

**Deferrals**
- S39. L7 goes to PR 3, which has the share.
- S40. The cleanup task and the stale refetch go to PR 4 together.

**Task-level rulings.** Their text is in the task that settles them; the numbers run on from S54 and are unique:
- S55 to S59, Task 13: the one-`makemkvcon` check, the wait after a kill, the exit wait, a retried scan's records, a licence message during a rip.
- S60 to S62, Task 14: `remux_args` and its options, the debug-log copy, the debug-log tests.
- S63 to S69, Task 15: the recorded transport, request logging, fetch outcomes, status words, search URL encoding, slim shapes, the clients tests.
- S70 to S75, Task 25: the restart sequence, the token job's drive, the slot, the rebuilt book, reading Decide's inputs back, rebuilt promises.
- S76 to S84, Task 26: refusal texts, snapshot problems and secrets, reply timing, the MakeMKV check's records, self-test checks and versions, retry re-checks, approvals, forced shutdown, job updates.
- S85 to S88, Task 27: `Args::rest`, word routing, forms and argument errors, `USAGE`.

Other task-level rulings keep their task's labels (T7-a, T8-a, T9-a, R22.1, R23.1, R24.1, T28-a, T29-a, and unnumbered "Ruling:" lines). Task 30 copies every one of them under its task.

## Rejected findings

- "Review Focus 2 test name vs its OPEN" (give one test name per option) is rejected. Design part 5.4 says the seventh field "equals" the drive id, so the choice is not open. Review Focus 2 now pins exact equality, as the other finding on the same item asks.
- "Get the owner's answer before Task 13, and write Tasks 13 and 23 for one option only" (OPEN 5) is applied through the planner's ruling RP-5, (b): Tasks 13 and 23 are written for it alone. RP-8 (a) and RP-9 (a) likewise leave Tasks 25 and 26 with one option each. The owner may override any RP ruling before execution.
- "State a concrete passing count for every verification step" is applied where the count follows from the code at `70ada2b` (Task 2's `job::` run, Task 16's `lookup::` run) and rejected elsewhere. The other runs filter by substrings that match tests across modules, or run packages whose unit tests earlier tasks of this plan add, so a pinned number would be a guess. Task 30 measures the count from one workspace run.
- "Drop `--features testkit` from the engine runs" is applied as its alternative: Task 22 Step 4 adds one plain `cargo test -p lazymkv-engine --locked` run, and the later runs keep the flag.
- "Assert that the 0xE9 name was read as U+FFFD downstream" is applied in part. No engine output carries CINFO 2 (the plan's evidence name comes from `bdmt_eng.xml` and the label), so the test pins the planner's reading of the recorded `scan.txt` through the calls of Task 23 Step 4 item 5.
