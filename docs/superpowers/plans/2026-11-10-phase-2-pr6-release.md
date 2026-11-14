# Phase 2 Pull Request 6: The Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** lazymkv gets its release package as a dry run: a zip with the fixed layout `bin\`, `resources\`, `licenses\`, built by a release workflow whose zip, sources and models stay continuous integration (CI) artifacts; a minimal FFmpeg that lazymkv's own CI builds, reproduces and that is checked once on the mini PC; the self-test judging MKVToolNix and FFmpeg against their minimum versions; and the optical character recognition (OCR) worker started under a 2 GiB memory limit.

**Architecture:**
- The memory limit is a new `Spawn` field that the Windows Job Object runner applies; a wrapping guard in `lazymkv-platform` sets it and one function in `lazymkv-tools` hands that guard to the frozen `run_worker`, so no frozen file changes.
- The self-test's minimums are two constants and a pure version reader in `core`; the engine's tool check turns the verdict into `ok` and a detail.
- `packaging/` holds the manifest `tools.json`, the FFmpeg build script and the release scripts. `.github/workflows/ffmpeg.yml` builds FFmpeg twice and compares; `.github/workflows/release.yml` calls it, builds the three programs on Windows, assembles and zips the tree, runs it, and uploads everything as artifacts. A tag path creates a draft release behind a gate that refuses today.
- One sitting on the mini PC checks the FFmpeg build against phase 0's numbers, runs the zip's tree and measures the zip's worker; a commit then marks the build verified.

**Tech Stack:** Rust 1.99.0 (no new crate, `Cargo.toml` and `Cargo.lock` untouched), `windows-sys` Job Objects, GitHub Actions (`ubuntu-24.04`, `windows-latest`), bash with `jq`, PowerShell (5.1 on the mini PC, 7 on the runner), mingw-w64 GCC 13.2.0, FFmpeg 9.0.2, zlib 1.3.2, MKVToolNix 100.0.

**Spec:** `docs/superpowers/specs/2026-11-10-phase-2-pr6-design-notes.md` (the notes; decisions A1 to F1 and calls 1 to 16), over `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (the design; parts 3.6, 5.3, 11, 12.6, 15) and `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (the spec). Executors read the notes and the design parts a task names.

## Global Constraints

- **The freeze (T5-R54).** No task edits `crates/core/src/subtitle/bitmap/`, `crates/ocr/`, `crates/ocr-models/`, `crates/tools/src/ocr.rs`, `Cargo.toml` or `Cargo.lock`, a doc line included. Every commit step first runs `git diff --exit-code 164099d -- crates/core/src/subtitle/bitmap crates/ocr crates/ocr-models crates/tools/src/ocr.rs Cargo.toml Cargo.lock`, which must print nothing. No crate is added; a crate manifest gains no dependency.
- **Toolchain and commands.** The Mac has no Rust toolchain. Every cargo command runs on the mini PC through `scripts/box.sh "<cmd>"`, one at a time, always `--locked`. `BOX '<cmd>'` in this plan is `ssh -o BatchMode=yes "$LAZYMKV_BOX" '<cmd>'` with `LAZYMKV_BOX` from the git-ignored `scripts/phase0/box.env`; it runs no cargo.
- **The shared mini PC.** The protocol agreed with the disc-menu session on 2026-11-10: before each `scripts/box.sh` run the controller sends that session one message naming the run's length, and waits for a reply only when that session has said a sitting is near or under way; it tells that session the time of Task 6's sitting once the owner sets it, since both need the drive and the engine stopped, and the owner decides the order. Before each run it also checks that no desktop job runs: `BOX 'powershell -NoProfile -Command "Get-ScheduledTask -TaskName lazymkv-phase0, lazymkv-spike -ErrorAction SilentlyContinue | ForEach-Object { $_.TaskName + [char]32 + $_.State }"'` prints neither task as `Running` or `Queued`. `scripts/box.sh` never runs while a desktop job or a capture sitting runs. Implementer subagents never touch the mini PC; they hand the controller the command to run.
- **The workspace run.** A code task's Run is its targeted command; before its commit the controller runs `scripts/box.sh cargo test --workspace --locked` once (every `test result:` line `ok`, `0 failed`). Task 0 and Task 7 run the junction form: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked --no-fail-fast 2>&1"`. A golden that changes stops the task and goes to the owner (K5).
- **RP21.** The controller and every subagent never read `corpus/owner` or `C:\lazymkv\evidence\owner`. Evidence is read only inside junction cargo runs and by Task 5's scripts on the mini PC (Task 5 Step 7 and Task 6), which print numbers and equal or different only, with the owner's yes.
- **Names and text.** No secret, webhook, host name, Windows user name, security identifier (SID), drive serial, job id, the owner's first name, or any disc title, slug or label in the repository, the ledger, the plan's records, a commit message, a workflow log or the pull request. Mini PC evidence names the trial's tracks by row number and the film as "phase 0's kept UHD (Ultra HD Blu-ray) all-tracks rip". No subtitle text in any file. `scripts/phase0/check-names.sh` prints nothing before every commit.
- **Docs and prose.** Every new `pub` item has a `///` doc that names its design part, notes decision or ruling and ends "Def: T6-<n>. Use: T6-<m>."; `perl .superpowers/pr6/pub-docs.pl fd945ae` (Task 0) prints `<k> new pub items, 0 without a Def/Use doc` after `git add` and before every commit from Task 1 on. Plain sentences, no em-dashes or en-dashes, acronyms expanded at first use, lines at most 120 characters in new source files, scripts and workflows.
- **Tests.** Named as snake_case sentences, in the file's `mod tests` or beside the tests they extend; tests that run `fake-tool` live in `crates/fake-tools/tests/`; a wait that only waits scales by `ci_factor()`, a bound that proves a kill does not; no non-ignored test contacts any host but `127.0.0.1`. The goldens stay byte-identical.
- **Commits and pushes.** One signed commit per task (Task 3 makes two, and Task 6 one, as they say), author and committer `krcm0209 <17315774+krcm0209@users.noreply.github.com>`, no attribution line anywhere, never `--no-gpg-sign` or `--no-verify`; `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G` and that identity. Each commit is pushed to the draft pull request (B1). CI is never polled: one `gh run watch <id> --exit-status` in the background per pushed run, read once. CI green on ubuntu, macos and windows and CodeQL at 0 open alerts before the next task; a red run is fixed by a new signed commit, subject `<area>: <what> (fix)`, never an amend or a force push.
- **Nothing public.** No tag is pushed, no release or draft release is created, and the pull request is marked ready only on the owner's word (P28, B1, D1).
- **Stop points.** A pin that a runner cannot fetch or whose SHA-256 (Secure Hash Algorithm 256) differs, two FFmpeg builds that differ, any difference in Task 6's checks, a worker peak above 1 GiB, a row 4 time above 600 s, and a changed golden stop the pull request and go to the owner with the numbers. No check is weakened to pass.

## Review Focus

The five failure modes the notes imply that the tasks' main tests would not reach, most likely first. Each names the test that pins it.

1. **A worker that meets its limit.** The stream must end unread and the job go on, with no dialog and nothing hung. Task 1: `a_child_that_allocates_past_its_limit_ends_without_success_and_one_within_it_runs` (Windows) and `a_worker_that_exits_with_a_code_logs_the_memory_limit_in_force`. CI shows no fault dialog in any case, so Task 6 Step 5 runs the first test once in the owner's desktop session, where a dialog could hold the child (T6-R18).
2. **Version strings as tools print them.** `n7.1.5-12-g1fdbca85aa-linux64-gpl-7.1`, `9.0.2-full_build-www.gyan.dev`, `6.1.1-3ubuntu5`, `N-121000-g5d8a9d5e3b`, `2026-10-01-git-0a1b2c3d4e`, `7`, `7.1.1.2`, an empty word. Task 2: `release_numbers_are_read_as_tools_print_them`.
3. **A manifest changed after the check.** A pin or the build script edited after `verified` was set must read as not verified without anyone resetting the field. Task 3: `gate-selftest.sh`'s `stale` fixture.
4. **A tag pushed today.** Version 0.0.0, a tag that is not `v<version>`, or an unverified build must each stop the publish path. Task 4: `gate-selftest.sh`'s `publish` cases.
5. **The zip extracted where its owner can write.** `resources` is then skipped and the self-test finds the tools elsewhere or not at all; the smoke run must fail on that, not pass on a tool found by another route. Task 4: `smoke.ps1` requires `found` to be `resources` for each of the three tools and prints the tree's owner when it is not.

## Owner prerequisites and decisions

Each answer goes to the git-ignored `.superpowers/pr6/decisions.txt` as `<item>, <yyyy-MM-dd>, <answer>`.

1. **The rulings (Task 0).** Whether any of T6-R1 to T6-R18 below is overturned.
2. **The pin list.** Approved 2026-11-10 with the notes.
3. **Chocolatey's FFmpeg on the FFmpeg workflow's Windows smoke job (Task 3).** It only makes layer 2's input files, as `ci.yml`'s Windows job already installs it; the pin list of the notes does not name it, so it gets its own yes with the plan.
4. **The check script's run on itself (Task 5 Step 7).** A yes for the read-only copy of the evidence mirror into a scratch folder under `C:\lazymkv\tmp\pr6`, removed afterwards. The owner is logged on at the mini PC's monitor for about half an hour; the engine keeps running.
5. **The sitting (Task 6).** One yes each for: stopping the engine task; copying the two CI artifacts to the mini PC; the same read-only copy of the evidence mirror; the scratch folders under `C:\lazymkv\tmp\pr6` and `C:\lazymkv\dev\tools\pr6-zip`. The owner is logged on at the mini PC's monitor for its length, about an hour and a half, and the drive stays empty.
6. **The design's amendments (Task 7).** A yes on the wording of the in-place amendments.
7. **Ready for review.** The pull request leaves draft only on the owner's word.
8. **Anything the plan did not foresee** goes back to the owner by name. Nothing is installed on the mini PC or the Mac.

## Rulings this plan makes

Each stands until the owner overturns it; Task 7 records them in `docs/phase2/engine.md` as "not yet ruled on by the owner".

- **T6-R1 (A1, call 10).** `Spawn::memory_limit` is applied as `JOB_OBJECT_LIMIT_JOB_MEMORY` with `JobMemoryLimit`, the job-wide committed-memory limit, since each child has a job of its own. `StdProcessGuard` ignores it.
- **T6-R2 (call 10).** The engine and `lazymkv-dev ocr-trial read` start the worker through `lazymkv_tools::run_worker_limited`, which wraps the caller's guard in `lazymkv_platform::MemoryLimited` and calls the frozen `run_worker`. `WORKER_MEMORY_LIMIT` is `2 * 1024 * 1024 * 1024`.
- **T6-R3 (call 10).** After a worker run that ended as `WorkerEnd::Exited` (a code that is none of the worker's own, or no code), job.log gets `lazymkv-ocr s<n> was started with a memory limit of 2147483648 bytes (design part 5.3)`. The line says what the engine asked for, which is true on every system; only the Windows runner enforces it. The note's reason stays T5-R8's.
- **T6-R4 (call 9, E1).** A release number is an optional leading `n`, then one to three runs of ASCII digits joined by `.`, ending at the end of the word or at `-`, `+` or `~`; missing parts are 0; a first number above 999 (a dated git build), a fourth part, or anything else is unreadable.
- **T6-R5 (call 9).** The four details: `<tool> <version>` when at or above the minimum; `<tool> is version <version>, below the minimum <minimum>` with `ok` false; `<tool> <version>; the version could not be judged against the minimum <minimum>` with `ok` true; `<tool> printed no version; it could not be judged against the minimum <minimum>` with `ok` true. `java` keeps its two forms.
- **T6-R6 (calls 1, 4).** The manifest's shape is Task 3's. The inputs' digest is the SHA-256 of `jq -cS '[.ffmpeg.version, .ffmpeg.source, .ffmpeg.zlib, .ffmpeg.runner, .ffmpeg.apt]'` of the manifest followed by the bytes of `packaging/ffmpeg/build.sh`; `.gitattributes` keeps `packaging/**` at LF on every checkout, and both scripts pass every `jq` output through `tr -d '\r'`, since `jq` on Windows writes CRLF line ends; so the digest is the same on Linux and Windows, and the release job proves it by comparing its own digest with the `inputs-sha256.txt` the Linux build wrote.
- **T6-R7 (call 3).** The build folder is `/tmp/lazymkv-ffmpeg` and zlib is installed under `/tmp/lazymkv-ffmpeg/zlib`; `SOURCE_DATE_EPOCH` is the modification time of the tarball's `RELEASE` file. The build script installs the apt pins itself, so the workflow file is not an input.
- **T6-R8 (call 5).** Layer 2 makes its lavfi input files with the FFmpeg `LAZYMKV_TEST_FFMPEG_FULL` names when it is set, else with the tool under test, as today.
- **T6-R9 (call 8).** The files of `licenses\` are Task 4's list. `lazymkvctl licenses` does not print a file of `licenses\` whose text equals a text it already printed from its own bytes (the MIT text, the models' part, the crate notices), so a release tree prints each text once.
- **T6-R10 (call 6).** `release.yml`'s dry run runs on `pull_request` for the paths `packaging/**` and the two workflow files, and on `workflow_dispatch`. Because a pull request's path filter reads its whole diff, every push to this pull request runs it once `packaging/` exists.
- **T6-R11 (D1, call 6).** The publish job runs `gh release create <tag> --draft --verify-tag` only after `gate.sh publish` passes: the tag is `v<workspace version>`, the version is not `0.0.0`, and the build is verified.
- **T6-R12 (call 12 (a)).** `scripts/phase0/audio-levels.ps1` gains two optional parameters, `-RecordingsRoot` and `-TuningRoot`, that replace `$Recordings` and `$TuningData` for one run; without them it behaves as before. The check writes only under them.
- **T6-R13 (call 12 (a), design part 11.3).** The check's rule. Byte for byte equal: every level series file, `streamhash.txt` and `graph.txt` of the new level folder against phase 0's; `manifest.txt` except its run-time line; the packet hashes of the new proxy against phase 0's; each trial row's packet listing against the recording's. The minimal `ffprobe`'s stream listing of the kept rip is compared with the recording's `ffprobe-streams.json` key by key: no key of an audio stream may differ, and every differing key of another stream is reported by stream kind and key name. `lazymkv-dev golden check` and `lazymkv-dev retune` over a scratch copy of the whole evidence mirror must print the same bytes before and after the film's new listing and level folder are put into the copy. Every listing and both commands' output are captured as bytes with `lazymkv-capture record --out`, never through a PowerShell pipeline, which would re-encode them. A differing audio key, a differing byte, or changed `golden check` or `retune` output stops the check; differing keys of other streams with unchanged `golden check` and `retune` output are reported to the owner in the evidence and do not stop it.
- **T6-R14 (call 12).** The check script first runs on the installed FFmpeg 9.0.2 (`-Self`), at the end of Task 5 and outside the sitting; every part must then print equal with no differing key. Anything else is a fault of the script and is fixed before the sitting is booked.
- **T6-R15 (call 11).** The worker check places the zip's `lazymkv-ocr.exe` beside a copy of the mini PC's `lazymkv-dev.exe` built with `ocr-trial` from the pull request's head, so `ocr-trial read` starts the zip's worker through `run_worker_limited`. It checks that worker's SHA-256 against the value the controller reads from the zip. The records are compared line by line after each line's last member, `"ms"`, the run's own timings, is removed. It writes nothing into the trial's kept output.
- **T6-R16 (call 13).** If `lazymkv-engine.exe` cannot start on a hosted runner, Task 4 adds a Windows test in `lazymkv-tools` that runs `find_tool` with `WinHost` over the folder `LAZYMKV_TEST_RESOURCES` names and requires `Found::Resources` for the three tools, and the smoke step runs that test instead of the engine.
- **T6-R17 (call 12 (b)).** The zip's tree runs on the mini PC as a console engine started over SSH (Secure Shell), as pull request 3's console runs do, not through the desktop task: nothing in it reads a share.
- **T6-R18 (A1).** Whether a child that dies of its limit can be held by a fault dialog is checked once in the owner's desktop session: `a_child_that_allocates_past_its_limit_ends_without_success_and_one_within_it_runs` run through `platform-ignored.ps1`, whose 10 s bound fails if a dialog holds the child. A failure there goes to the owner; the worker's time cap bounds the wait in any case.

## File structure

| File | Task | Responsibility |
|---|---|---|
| `crates/platform/src/lib.rs` | 1 | `Spawn::memory_limit`, `MemoryLimited` |
| `crates/platform/src/win/job.rs` | 1 | the job's memory limit |
| `crates/platform/src/process.rs` | 1 | the `allocate` helper mode |
| `crates/tools/src/worker_limit.rs` (new) | 1 | `WORKER_MEMORY_LIMIT`, `run_worker_limited` |
| `crates/engine/src/step10.rs`, `crates/dev/src/cmd/ocr_trial.rs` | 1 | call `run_worker_limited` |
| `crates/core/src/toolver.rs` (new) | 2 | the minimums and the release-number reader |
| `crates/engine/src/selftest.rs` | 2 | the tool check's verdict |
| `corpus/scenarios/movie-direct-old-tools/` (new) | 2 | stand-in version lines |
| `packaging/tools.json` (new) | 3 | the manifest |
| `packaging/ffmpeg/build.sh`, `packaging/ffmpeg/inputs-digest.sh` (new) | 3 | the build and its inputs' digest |
| `packaging/release/gate.sh`, `gate-selftest.sh`, `fixtures/` (new) | 3, 4 | verified or not; may it publish |
| `.github/workflows/ffmpeg.yml` (new) | 3 | build twice, compare, smoke |
| `crates/fake-tools/tests/layer2.rs` | 3 | `LAZYMKV_TEST_FFMPEG_FULL` |
| `crates/ctl/src/tests.rs` | 3, 4 | the manifest test; the licences' once-only test |
| `crates/ctl/src/licenses.rs` | 4 | no text printed twice |
| `packaging/release/assemble.ps1`, `smoke.ps1` (new) | 4 | the tree, the zip, the run |
| `.github/workflows/release.yml` (new) | 4 | the dry run and the gated publish job |
| `scripts/phase0/ffmpeg-check.ps1`, `worker-check.ps1` (new), `audio-levels.ps1` | 5 | the sitting's checks |
| `crates/fake-tools/tests/scripts.rs` | 4, 5 | the scripts parse and print no name |
| `docs/phase2/engine.md`, the design, `.gitattributes` | 3, 7 | the record and the amendments |

---

## Tasks

### Task 0: The window, the baseline, the branch and the draft pull request (controller, manual)

**Files:** Create (git-ignored): `.superpowers/pr6/pub-docs.pl`, `.superpowers/pr6/baseline-list.txt`, `.superpowers/pr6/baseline-junction-run.txt`, `.superpowers/pr6/pr6-mini-pc-runs.txt`, `.superpowers/sdd/2026-11-10-phase-2-pr6-release/progress.md`. Commit: the notes and this plan.

**Interfaces:** Produces the baseline count N0 with its per-crate counts and the junction run's passed, ignored and `skipped:` counts (Task 7 compares); the branch `phase-2-release`; the draft pull request's number `<pr>`; `.superpowers/pr6/pub-docs.pl`.

- [ ] **Step 1: The owner's rulings.** Send the owner: "Pull request 6's plan makes rulings T6-R1 to T6-R18 (the plan's section 'Rulings this plan makes'). Is any overturned?" Record `rulings T6-R1 to T6-R18, <date>, none overturned` or one `T6-R<n>, <date>, overturned: <words>` line each. An overturn reopens its task's text before that task starts. The other steps do not wait for the answer; Task 1 does.
- [ ] **Step 2: The window.** Done on 2026-11-10: the disc-menu session answered that no sitting of its own is running or planned that day, that it does not use `C:\lazymkv\src` or `C:\lazymkv\target`, that its desktop jobs run as the task `lazymkv-spike`, and that it will say before any sitting starts and when it ends (Global Constraints, the shared mini PC). Send it the message for Step 4's run.
- [ ] **Step 3: The Def/Use check.** Copy the main checkout's git-ignored `.superpowers/pr5/pub-docs.pl` to `.superpowers/pr6/pub-docs.pl` and change its three `T5-` patterns to `T6-`. `perl -c` prints `syntax OK`; `perl .superpowers/pr6/pub-docs.pl fd945ae` prints `0 new pub items, 0 without a Def/Use doc`.
- [ ] **Step 4: The baseline on fd945ae.** `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` exits 0 with no active job. Run `scripts/box.sh "cargo test --workspace --locked -- --list 2>&1" > .superpowers/pr6/baseline-list.txt`, then the junction run into `.superpowers/pr6/baseline-junction-run.txt`. Read N0 with `tr -d '\r' < .superpowers/pr6/baseline-list.txt | grep -c ': test$'`, and the passed, failed and ignored sums and the `skipped:` count as pull request 5's Task 0 Step 7 did. Expected: 3459 listed, 0 failed, 13 ignored, 46 `skipped:` lines; another count is recorded, not a stop; a failure is a stop. `git diff --exit-code Cargo.lock` prints nothing.
- [ ] **Step 5: The branch and the first commit.** `git branch -m phase-2-release`; `git add docs/superpowers/specs/2026-11-10-phase-2-pr6-design-notes.md docs/superpowers/plans/2026-11-10-phase-2-pr6-release.md`; `scripts/phase0/check-names.sh` prints nothing; commit `docs: pull request 6's design notes and plan`; check the signature line.
- [ ] **Step 6: The draft pull request (B1).** `git push -u origin phase-2-release`; `gh pr create --draft --base main --title "Phase 2 pull request 6: the release" --body-file <a body of three sentences naming the notes and the plan, with no attribution line>`. Bind it with the `ccd_pr` tools. Watch the CI run once in the background; green before Task 1.
- [ ] **Step 7: Evidence.** `.superpowers/pr6/pr6-mini-pc-runs.txt` gets its header and two lines: the tools' versions from `lazymkvctl selftest`, and `<date>, SSH, the baseline on fd945ae: N0 <n> tests listed (<per-crate counts>); the junction run <p> passed, 0 failed, <i> ignored, <k> skipped: lines; Cargo.lock unchanged`.

---

### Task 1: The worker's memory limit (A1, A2; call 10; T6-R1 to T6-R3)

**Files:**
- Modify: `crates/platform/src/lib.rs` (`Spawn`, new `MemoryLimited`), `crates/platform/src/win/job.rs` (`Job::new`, `JobProcessGuard::spawn`, the literal at line 654), `crates/platform/src/process.rs` (`helper`, `helper_child`), `crates/tools/src/lib.rs`, `crates/tools/src/ffmpeg.rs:45` and `crates/engine/src/owner.rs:3103` (their `Spawn` literals), `crates/engine/src/step10.rs:338`, `crates/dev/src/cmd/ocr_trial.rs:432`
- Create: `crates/tools/src/worker_limit.rs`
- Test: `crates/platform/src/lib.rs`, `crates/platform/src/win/job.rs`, `crates/fake-tools/tests/ocr_driver.rs`, `crates/fake-tools/tests/engine_file_phase.rs`, `crates/dev/src/cmd/ocr_trial.rs`

**Interfaces:**
- Consumes: `lazymkv_tools::ocr::run_worker(ctx: &ToolCtx, program: &Path, args: &[String], capture: &Path, cap: Duration) -> io::Result<WorkerEnd>` (frozen; it builds its `Spawn` with `..Spawn::default()` and its own `ToolCtx` from `ctx`'s six fields).
- Produces:
  - `pub memory_limit: Option<u64>` on `lazymkv_platform::Spawn`, bytes, `None` by default.
  - `pub struct MemoryLimited<'a>` in `lazymkv_platform` with `pub fn new(inner: &'a dyn ProcessGuard, limit: u64) -> MemoryLimited<'a>`, implementing `ProcessGuard`: its `spawn` calls `inner.spawn` with a clone of the `Spawn` whose `memory_limit` is `Some(limit)`.
  - `pub const WORKER_MEMORY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;` and `pub fn run_worker_limited(ctx: &ToolCtx, program: &Path, args: &[String], capture: &Path, cap: Duration) -> io::Result<WorkerEnd>` in `lazymkv_tools::worker_limit`, both re-exported from `lazymkv_tools`.

- [ ] **Step 1: Write the failing tests.**

In `crates/platform/src/lib.rs`'s tests, with a test-local guard `Seen(Mutex<Vec<Spawn>>)` whose `spawn` records a clone and returns `Err(io::ErrorKind::Unsupported.into())`:

```rust
#[test]
fn a_memory_limited_guard_sets_the_limit_and_changes_nothing_else() {
    let seen = Seen(Mutex::new(Vec::new()));
    let spawn = Spawn { program: "p".into(), args: vec!["a".into()], env: vec![("K".into(), "V".into())],
        below_normal: true, cwd: Some("d".into()), memory_limit: None };
    assert!(MemoryLimited::new(&seen, 4096).spawn(&spawn).is_err());
    let got = seen.0.lock().unwrap().remove(0);
    assert_eq!(got.memory_limit, Some(4096));
    assert_eq!((got.program, got.args, got.env, got.below_normal, got.cwd),
               (spawn.program, spawn.args, spawn.env, spawn.below_normal, spawn.cwd));
    assert_eq!(Spawn::default().memory_limit, None);
}
```

In `crates/platform/src/win/job.rs`'s tests (Windows only, as the module is). `MIB` is `1024 * 1024`:

```rust
#[test]
fn a_job_with_a_memory_limit_has_the_job_memory_flag_and_value() {
    // Job::new(Some(256 * MIB)), read back with QueryInformationJobObject as the test beside it does:
    // LimitFlags == KILL_ON_JOB_CLOSE | DIE_ON_UNHANDLED_EXCEPTION | JOB_OBJECT_LIMIT_JOB_MEMORY
    // and JobMemoryLimit == 256 * MIB as usize.
}

#[test]
fn a_child_that_allocates_past_its_limit_ends_without_success_and_one_within_it_runs() {
    // helper("allocate") with env LAZYMKV_TEST_ALLOCATE_MIB and memory_limit Some(256 * MIB).
    // 1024: wait() within ENDS_WITHIN is Ok(Some(code)) with code != 0, and stdout has no "allocated" line.
    // 16: stdout has the line "allocated 16" and wait() is Ok(Some(0)).
}
```

`a_job_kills_on_close_and_lets_no_fault_dialog_hold_its_processes` becomes `Job::new(None)` and keeps its two-flag assertion.

In `crates/fake-tools/tests/ocr_driver.rs`, beside `the_worker_gets_rten_num_threads_3_and_below_normal_priority`, with its recording guard:

```rust
#[test]
fn run_worker_limited_adds_the_2_gib_limit_and_nothing_else() {
    // run_worker_limited over the same stand-in worker and arguments as the test beside it.
    assert_eq!(spawns[0].memory_limit, Some(2_147_483_648));
    assert_eq!(WORKER_MEMORY_LIMIT, 2_147_483_648);
    assert_eq!(spawns[0].env, [("RTEN_NUM_THREADS".to_string(), "3".to_string())]);
    assert!(spawns[0].below_normal);
}
```

In `crates/fake-tools/tests/engine_file_phase.rs`: extend `the_worker_runs_once_per_english_or_no_language_stream_one_at_a_time_below_normal_with_rten_num_threads_3` with `assert_eq!(spawn.memory_limit, Some(WORKER_MEMORY_LIMIT));`, and add:

```rust
#[test]
fn only_the_worker_runs_under_a_memory_limit() {
    // A two-stream job to Filed: of every recorded spawn, memory_limit is Some(WORKER_MEMORY_LIMIT) exactly where
    // the program's file stem is "lazymkv-ocr", and None for mkvmerge, ffprobe, ffmpeg and makemkvcon64.
}

#[test]
fn a_worker_that_exits_with_a_code_logs_the_memory_limit_in_force() {
    // The scenario of a_worker_that_panics_leaves_the_stream_unread_with_its_exit_code_and_no_stderr_text.
    // job.log holds "lazymkv-ocr s<n> was started with a memory limit of 2147483648 bytes (design part 5.3)" once for the
    // stream that exited 101, after its "ended lazymkv-ocr s<n>: exit 101" line, and no such line for a stream read.
}
```

In `crates/dev/src/cmd/ocr_trial.rs`, extend `read_starts_the_sibling_worker_below_normal_with_rten_num_threads_3_and_the_fixed_cap` with `assert_eq!(spawned[0].memory_limit, Some(lazymkv_tools::WORKER_MEMORY_LIMIT));`.

- [ ] **Step 2: Run to see them fail.** Controller: `scripts/box.sh cargo test -p lazymkv-platform --locked memory`. Expected: a compile error, `no field memory_limit`.
- [ ] **Step 3: `Spawn::memory_limit` and `MemoryLimited`** in `crates/platform/src/lib.rs`, with the three exhaustive `Spawn` literals (`process.rs` `helper`, `job.rs:654`, `owner.rs:3103`) and `ffmpeg.rs:45` given `memory_limit: None`. `Spawn`'s doc says the engine sets it for the OCR worker only and that it is a no-op where the platform has no such limit.
- [ ] **Step 4: The job's limit.** `Job::new(memory_limit: Option<u64>)`: with `Some(n)`, `LimitFlags` gains `JOB_OBJECT_LIMIT_JOB_MEMORY` and `info.JobMemoryLimit = n as usize`. `JobProcessGuard::spawn` passes `spawn.memory_limit`. `StdProcessGuard`'s doc says it ignores the field (T6-R1).
- [ ] **Step 5: The `allocate` helper mode** in `process.rs`'s `helper_child`: read `LAZYMKV_TEST_ALLOCATE_MIB`, allocate that many MiB in one `Vec<u8>`, write one byte per 4096, pass it through `std::hint::black_box`, then `say("allocated <mib>")`.
- [ ] **Step 6: `crates/tools/src/worker_limit.rs`.** `run_worker_limited` builds `MemoryLimited::new(ctx.guard, WORKER_MEMORY_LIMIT)`, a `ToolCtx` with that guard and `ctx`'s other five fields (`cancel` cloned), and returns `ocr::run_worker` of it.
- [ ] **Step 7: The two callers.** `step10.rs:338` and `ocr_trial.rs:432` call `run_worker_limited`. In `step10.rs`, after the `ended lazymkv-ocr` line, a `WorkerEnd::Exited(_)` end logs T6-R3's line.
- [ ] **Step 8: Run for PASS.** Controller: `scripts/box.sh cargo test -p lazymkv-platform --locked`, then `-p fake-tools --test ocr_driver`, then `-p fake-tools --test engine_file_phase`, then `-p lazymkv-dev --features ocr-trial ocr_trial`. Then the workspace run.
- [ ] **Step 9: Commit and push.** The freeze check, `check-names.sh`, `pub-docs.pl`; commit `platform, tools: the OCR worker runs under a 2 GiB memory limit`; push; one watch. Windows CI must show the two new `win::job` tests passed.

---

### Task 2: The self-test's minimum versions (S80, T5-R65; E1; call 9; T6-R4, T6-R5)

**Files:**
- Create: `crates/core/src/toolver.rs`, `corpus/scenarios/movie-direct-old-tools/mkvmerge-version.txt`, `ffprobe-version.txt`, `ffmpeg-version.txt`
- Modify: `crates/core/src/lib.rs` (`pub mod toolver;`), `crates/engine/src/selftest.rs` (`tool_check`)
- Test: `crates/core/src/toolver.rs`, `crates/engine/src/selftest.rs`, `crates/fake-tools/tests/engine_ipc.rs`

**Interfaces:**
- Consumes: `lazymkv_core::job::Tool`; `selftest.rs`'s `tool_check`, `check` and `version_in`.
- Produces, in `lazymkv_core::toolver`:
  - `pub const MKVTOOLNIX_MIN: &str = "65.0";` (K10) and `pub const FFMPEG_MIN: &str = "7.0";` (K11)
  - `pub fn minimum(tool: Tool) -> Option<&'static str>`: `Mkvmerge` gives `MKVTOOLNIX_MIN`, `Ffprobe` and `Ffmpeg` give `FFMPEG_MIN`, the others `None`.
  - `pub fn release_number(version: &str) -> Option<(u32, u32, u32)>` (T6-R4)
  - `pub enum Standing { AtLeast, Below, Unreadable }` and `pub fn standing(version: &str, minimum: &str) -> Standing`
- Produces, in `selftest.rs`: `pub(crate) fn judged(name: &str, version: Option<&str>, minimum: Option<&str>) -> (bool, String)` (T6-R5).

- [ ] **Step 1: Write the failing tests.**

`crates/core/src/toolver.rs`:

```rust
#[test]
fn release_numbers_are_read_as_tools_print_them() {
    for (text, want) in [
        ("100.0", Some((100, 0, 0))), ("9.0.2", Some((9, 0, 2))), ("9.0.2-full_build-www.gyan.dev", Some((9, 0, 2))),
        ("n7.1.5-12-g1fdbca85aa-linux64-gpl-7.1", Some((7, 1, 5))), ("6.1.1-3ubuntu5", Some((6, 1, 1))),
        ("7.1", Some((7, 1, 0))), ("7", Some((7, 0, 0))), ("8.0+git", Some((8, 0, 0))), ("7.0~rc1", Some((7, 0, 0))),
        ("999.0", Some((999, 0, 0))), ("N-121000-g5d8a9d5e3b", None), ("2026-10-01-git-0a1b2c3d4e", None),
        ("1000.0", None), ("7.1.1.2", None), ("7.x", None), ("7.", None), ("v7.1", None), ("n", None), ("", None),
        ("4294967296.0", None),
    ] {
        assert_eq!(release_number(text), want, "{text}");
    }
}

#[test]
fn a_version_stands_at_least_below_or_unreadable_against_its_minimum() {
    assert_eq!(standing("65.0", MKVTOOLNIX_MIN), Standing::AtLeast);
    assert_eq!(standing("64.0", MKVTOOLNIX_MIN), Standing::Below);
    assert_eq!(standing("7.0", FFMPEG_MIN), Standing::AtLeast);
    assert_eq!(standing("6.1.1-3ubuntu5", FFMPEG_MIN), Standing::Below);
    assert_eq!(standing("N-121000-g5d8a9d5e3b", FFMPEG_MIN), Standing::Unreadable);
}

#[test]
fn only_mkvmerge_ffprobe_and_ffmpeg_have_a_minimum() {
    assert_eq!(minimum(Tool::Mkvmerge), Some("65.0"));
    assert_eq!((minimum(Tool::Ffprobe), minimum(Tool::Ffmpeg)), (Some("7.0"), Some("7.0")));
    assert_eq!((minimum(Tool::Java), minimum(Tool::Makemkvcon)), (None, None));
}
```

`crates/engine/src/selftest.rs`:

```rust
#[test]
fn a_found_tool_is_judged_against_its_minimum() {
    let judged = |v: Option<&str>, m: Option<&str>| judged("ffmpeg", v, m);
    assert_eq!(judged(Some("9.0.2"), Some("7.0")), (true, "ffmpeg 9.0.2".to_string()));
    assert_eq!(judged(Some("6.1.1-3ubuntu5"), Some("7.0")),
               (false, "ffmpeg is version 6.1.1-3ubuntu5, below the minimum 7.0".to_string()));
    assert_eq!(judged(Some("N-121000-g5d8a9d5e3b"), Some("7.0")),
               (true, "ffmpeg N-121000-g5d8a9d5e3b; the version could not be judged against the minimum 7.0".to_string()));
    assert_eq!(judged(None, Some("7.0")),
               (true, "ffmpeg printed no version; it could not be judged against the minimum 7.0".to_string()));
    assert_eq!(judged(Some("21.0.2"), None), (true, "ffmpeg 21.0.2".to_string()));
    assert_eq!(judged(None, None), (true, "ffmpeg printed no version".to_string()));
}
```

`crates/fake-tools/tests/engine_ipc.rs`, beside `each_tool_check_reports_the_version_its_stand_in_prints`. The overlay's three files hold `mkvmerge v64.0 ('made-up') 64-bit`, `ffprobe version N-121000-g0a1b2c3d4e Copyright (c) the FFmpeg developers` and `ffmpeg version 6.1.1-3ubuntu5 Copyright (c) the FFmpeg developers`, in the form of `corpus/scenarios/movie-direct`'s:

```rust
#[test]
fn a_tool_below_its_minimum_fails_its_check_and_an_unreadable_version_passes_with_a_note() {
    let h = stand_in(&["movie-direct", "movie-direct-old-tools"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let result = c.ok("run_selftest", Value::Null);
    let seen = |tool: &str| { let f = check(&result, tool); (f["ok"].clone(), f["detail"].clone(), f["version"].clone()) };
    assert_eq!(seen("mkvmerge"), (json!(false), json!("mkvmerge is version 64.0, below the minimum 65.0"), json!("64.0")));
    assert_eq!(seen("ffprobe"), (json!(true),
        json!("ffprobe N-121000-g0a1b2c3d4e; the version could not be judged against the minimum 7.0"),
        json!("N-121000-g0a1b2c3d4e")));
    assert_eq!(seen("ffmpeg"), (json!(false),
        json!("ffmpeg is version 6.1.1-3ubuntu5, below the minimum 7.0"), json!("6.1.1-3ubuntu5")));
}

#[test]
fn no_job_is_held_for_a_tool_below_its_minimum() {
    // The same two scenarios; a disc is inserted and delivered; the job reaches Filed (call 9).
}
```

- [ ] **Step 2: Run to see them fail.** `scripts/box.sh cargo test -p lazymkv-core --locked toolver`: a compile error, the module is missing.
- [ ] **Step 3: `crates/core/src/toolver.rs`**, pure, by T6-R4. `standing` reads both sides with `release_number`; a minimum that does not read is a bug and gives `Unreadable`.
- [ ] **Step 4: `judged` and `tool_check`.** `tool_check` takes `ok` and the detail from `judged(name, version.as_deref(), minimum(tool))`; the java suffix, the cancelled check and the not-found check are unchanged. `selftest.rs`'s module doc and `tool_check`'s doc name S80 as amended here. `tool_found` is not changed (call 9).
- [ ] **Step 5: The scenario overlay** `corpus/scenarios/movie-direct-old-tools/` with the three version files.
- [ ] **Step 6: Run for PASS.** `-p lazymkv-core toolver`, `-p lazymkv-engine selftest`, `-p fake-tools --test engine_ipc`; then the workspace run.
- [ ] **Step 7: Commit and push.** `core, engine: the self-test judges MKVToolNix and FFmpeg against their minimum versions`.

---

### Task 3: The manifest, the FFmpeg build and its workflow (P16, K1, K4; calls 1 to 5; T6-R6 to T6-R8)

**Files:**
- Create: `packaging/tools.json`, `packaging/ffmpeg/build.sh`, `packaging/ffmpeg/inputs-digest.sh`, `packaging/release/gate.sh`, `packaging/release/gate-selftest.sh`, `packaging/release/fixtures/*.json`, `.github/workflows/ffmpeg.yml`
- Modify: `.gitattributes`, `crates/fake-tools/tests/layer2.rs`, `crates/ctl/src/tests.rs`

**Interfaces:**
- Produces `packaging/tools.json`, exactly this shape, with the notes' section 3 values:

```json
{
  "ffmpeg": {
    "version": "9.0.2",
    "source": { "url": "https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz",
                "sha256": "8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e" },
    "zlib": { "version": "1.3.2",
              "url": "https://github.com/madler/zlib/releases/download/v1.3.2/zlib-1.3.2.tar.xz",
              "sha256": "d7a0654783a4da529d1bb793b7ad9c3318020af77667bcae35f95d0e42a792f3" },
    "runner": "ubuntu-24.04",
    "apt": ["gcc-mingw-w64-x86-64-win32=13.2.0-6ubuntu1+26.1", "binutils-mingw-w64-x86-64=2.41.90.20240122-1ubuntu1+11.4",
            "mingw-w64-x86-64-dev=11.0.1-3build1", "mingw-w64-common=11.0.1-3build1", "nasm=2.16.01-1build1",
            "make=4.3-4.1build2"],
    "built": { "ffmpeg_exe_sha256": null, "ffprobe_exe_sha256": null, "verified": null }
  },
  "mkvtoolnix": {
    "version": "100.0",
    "portable": { "url": "https://mkvtoolnix.download/windows/releases/100.0/mkvtoolnix-64-bit-100.0.7z",
                  "sha256": "061de38bd10e7e28697b897e0b890b78d6f2ec8d668a9c198600ed45c19672ab" },
    "source": { "url": "https://mkvtoolnix.download/sources/mkvtoolnix-100.0.tar.xz",
                "sha256": "74480d07a261beeaa8baf898248e668ecc56335e2527bbffa841ef056dc028a1" }
  },
  "models": {
    "text-detection.onnx": { "url": "<DETECTION_URL>", "sha256": "<DETECTION_SHA256>" },
    "text-recognition.onnx": { "url": "<RECOGNITION_URL>", "sha256": "<RECOGNITION_SHA256>" }
  }
}
```

  `verified`, once set by Task 6, is `{ "date": "<yyyy-MM-dd>", "inputs_sha256": "<64 hex digits>" }`.
- Produces `packaging/ffmpeg/inputs-digest.sh`: prints T6-R6's digest as 64 hex digits and a newline; works with `sha256sum` or `shasum -a 256`; every `jq` output goes through `tr -d '\r'`, here and in `gate.sh`.
- Produces `packaging/ffmpeg/build.sh <out dir>`: installs the apt pins, fetches and checks the two archives, builds zlib then FFmpeg in `/tmp/lazymkv-ffmpeg` (removed first), and writes into `<out dir>`: `ffmpeg.exe`, `ffprobe.exe`, `ffmpeg-build-settings.txt` (the script's text, the configure line as run, the versions of GCC, binutils, nasm and zlib, and FFmpeg's configure summary), `mingw-w64-runtime-copyright.txt` (a copy of `/usr/share/doc/mingw-w64-common/copyright`), `COPYING.LGPLv2.1` and `LICENSE.md` from the FFmpeg tarball, zlib's `LICENSE`, and `inputs-sha256.txt`. The configure line is design part 11.2's, plus only the flags that point at the zlib of `/tmp/lazymkv-ffmpeg/zlib`.
- Produces `packaging/release/gate.sh`, with `MANIFEST` (default `packaging/tools.json`) and `INPUTS_SHA256` (default: `inputs-digest.sh`'s output) read from the environment:
  - `gate.sh status` prints `verified` and exits 0 when `built.verified.inputs_sha256` equals `INPUTS_SHA256` and both `built` hashes are 64 hex digits; else prints `unverified: <reason>` and exits 0. The reasons: `no build is recorded`, `the build was never verified on the target machine`, `the build's inputs changed after it was verified`.
  - `gate.sh hashes <ffmpeg.exe> <ffprobe.exe>` exits 0 when both files' SHA-256 values equal the manifest's, or when the manifest's are null (it then prints the two values and `not recorded yet`); else prints both pairs and exits 1.
  - `gate.sh publish <ref> <version>` exits 0 only when `<ref>` is `refs/tags/v<version>`, `<version>` is not `0.0.0` and `status` is `verified`; else prints `refused: <reason>` and exits 1 (T6-R11).
- Produces the artifact `ffmpeg-windows-x64`: `build.sh`'s output files and `SHA256SUMS`.
- Produces `LAZYMKV_TEST_FFMPEG_FULL` in layer 2 (T6-R8).

- [ ] **Step 1: `.gitattributes`** gains `packaging/** text eol=lf` with a one-line comment naming T6-R6.
- [ ] **Step 2: The manifest and its Rust test.** Write `packaging/tools.json`. In `crates/ctl/src/tests.rs`:

```rust
#[test]
fn the_manifests_model_pins_are_the_embedded_models_pins() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../../../packaging/tools.json")).unwrap();
    for (name, url, sha256) in [(DETECTION_NAME, DETECTION_URL, DETECTION_SHA256),
                                (RECOGNITION_NAME, RECOGNITION_URL, RECOGNITION_SHA256)] {
        assert_eq!(manifest["models"][name], json!({ "url": url, "sha256": sha256 }), "{name}");
    }
}

#[test]
fn every_sha256_of_the_manifest_is_64_lower_case_hex_digits_or_null() {
    // Walk the whole value: every string under a key that is "sha256" or ends "_sha256" matches ^[0-9a-f]{64}$;
    // such a key may hold null only under ffmpeg.built.
}
```

  Run: `scripts/box.sh cargo test -p lazymkv-ctl --locked manifest`. Expected: FAIL before the file exists, PASS after.
- [ ] **Step 3: `inputs-digest.sh`, `gate.sh` and the fixtures.** `packaging/release/fixtures/` holds five manifests in the real shape with made-up hashes: `none.json` (built hashes null), `unverified.json` (hashes set, `verified` null), `verified.json` (`verified.inputs_sha256` is `aa…a`, 64 times), `stale.json` (the same, `bb…b`), `half.json` (one built hash null). `gate-selftest.sh` runs `gate.sh` over them with `INPUTS_SHA256` set to `aa…a` and fails on the first wrong answer:

| Call | Expected output | Exit |
|---|---|---|
| `status`, `none.json` | `unverified: no build is recorded` | 0 |
| `status`, `half.json` | `unverified: no build is recorded` | 0 |
| `status`, `unverified.json` | `unverified: the build was never verified on the target machine` | 0 |
| `status`, `stale.json` | `unverified: the build's inputs changed after it was verified` | 0 |
| `status`, `verified.json` | `verified` | 0 |
| `publish refs/tags/v1.2.3 1.2.3`, `verified.json` | (nothing) | 0 |
| `publish refs/tags/v0.0.0 0.0.0`, `verified.json` | `refused: the workspace version is 0.0.0` | 1 |
| `publish refs/tags/v1.2.4 1.2.3`, `verified.json` | `refused: the tag is not v1.2.3` | 1 |
| `publish refs/heads/main 1.2.3`, `verified.json` | `refused: the tag is not v1.2.3` | 1 |
| `publish refs/tags/v1.2.3 1.2.3`, `unverified.json` | `refused: unverified: the build was never verified on the target machine` | 1 |
| `publish refs/tags/v1.2.3 1.2.3`, `stale.json` | `refused: unverified: the build's inputs changed after it was verified` | 1 |
| `hashes` with two made-up files whose SHA-256 values are `unverified.json`'s | (nothing) | 0 |
| `hashes` with the same files, `verified.json` holding other values | both pairs | 1 |

  Run on the Mac: `bash packaging/release/gate-selftest.sh` prints `gate self-test: 13 passed` and exits 0. It also runs `inputs-digest.sh` twice and requires the same 64 hex digits both times.
- [ ] **Step 4: `build.sh`.** `set -euo pipefail`; every download with `curl --fail --location`, each SHA-256 checked before unpacking; `SOURCE_DATE_EPOCH` by T6-R7; zlib built with the cross-compiler as a static library and installed under `/tmp/lazymkv-ffmpeg/zlib`; FFmpeg configured in tree and built with `make -j"$(nproc)"`. Before the configure, the script greps the unpacked `libavformat/matroskadec.c` for `inflateGetDictionary` and fails if it is there (the notes' zlib pin). `bash -n packaging/ffmpeg/build.sh` passes on the Mac; nothing is built there.
- [ ] **Step 5: Layer 2's generator (T6-R8).** In `layer2.rs`, the three callers of `lavfi_mkv` pass `generator(ffmpeg)`: the path `LAZYMKV_TEST_FFMPEG_FULL` names when it is set and not empty (it must be a file, and FFmpeg 7.0 or later by `below_7_0`, else `skip_or_fail`), else the tool under test. The file's header doc gains one sentence. Run: `scripts/box.sh cargo test -p fake-tools --locked --test layer2`: the count is unchanged and nothing new is skipped.
- [ ] **Step 6: `.github/workflows/ffmpeg.yml`.** Header comment in `ci.yml`'s style. `permissions: contents: read`. Triggers:

```yaml
on:
  push:
    paths: ['packaging/tools.json', 'packaging/ffmpeg/**', '.github/workflows/ffmpeg.yml']
  workflow_dispatch:
  workflow_call:
```

  Job `build` (`runs-on: ubuntu-24.04`, `timeout-minutes: 90`): checkout; `bash packaging/release/gate-selftest.sh`; `bash packaging/ffmpeg/build.sh "$RUNNER_TEMP/out1"`; the same into `out2`; `cmp` of both programs between the two folders, failing with `the two builds differ` (K4); `bash packaging/release/gate.sh hashes "$RUNNER_TEMP/out1/ffmpeg.exe" "$RUNNER_TEMP/out1/ffprobe.exe"`; write `SHA256SUMS` in `out1`; upload `out1` as `ffmpeg-windows-x64` with `retention-days: 14`.
  Job `smoke` (`runs-on: windows-latest`, `needs: build`, `timeout-minutes: 90`): checkout; the pinned toolchain; restore the model cache as `ci.yml`'s test job does, without `fail-on-cache-miss`, setting `LAZYMKV_OCRS_MODELS` only on a hit; download the artifact; `-version` of both programs and `ffmpeg -hide_banner -layouts`, each first line printed; `choco install ffmpeg -y --no-progress` for the generator (owner item 3); the manifest's portable archive fetched, checked and `mkvmerge.exe` alone extracted with `7z e`; then `cargo test -p fake-tools --locked --test layer2 -- --nocapture` with `LAZYMKV_TEST_FFMPEG` and `LAZYMKV_TEST_FFPROBE` naming the minimal programs, `LAZYMKV_TEST_MKVMERGE` the extracted one and `LAZYMKV_TEST_FFMPEG_FULL` Chocolatey's. Expected: `5 passed; 0 failed` and no `skipped:` line. No step writes a `${{ github.event.* }}` value into a script.
- [ ] **Step 7: Commit and push (first commit).** The workspace run first. Commit `packaging: the tool manifest, the minimal FFmpeg build and its workflow`; push. The push starts `ffmpeg.yml`; watch it and CI once each. A red `smoke` on a layer 2 test is read against call 5: a missing listed component is fixed in `build.sh`; any other cause stops the task and is reported.
- [ ] **Step 8: Record the hashes (second commit).** From the green run's log, the two SHA-256 values `gate.sh hashes` printed go into `built.ffmpeg_exe_sha256` and `built.ffprobe_exe_sha256`; `verified` stays null. `bash packaging/release/gate.sh status` prints `unverified: the build was never verified on the target machine`. Commit `packaging: record the first reproduced FFmpeg build's hashes`; push. The push runs `ffmpeg.yml` again: a third and fourth build must reproduce the recorded values, which is K4 across runs. A mismatch is a stop.

---

### Task 4: The release scripts, the workflow and the dry run (P24, P28; D1; calls 6 to 8, 13, 14; T6-R9 to T6-R11, T6-R16)

**Files:**
- Create: `packaging/release/assemble.ps1`, `packaging/release/smoke.ps1`, `.github/workflows/release.yml`
- Modify: `crates/ctl/src/licenses.rs` (`bundled`), `crates/ctl/src/tests.rs`, `crates/fake-tools/tests/scripts.rs`

**Interfaces:**
- Consumes: Task 3's manifest, `gate.sh` and the artifact `ffmpeg-windows-x64`; `licenses_text(exe: Option<&Path>) -> Result<String, String>`; `lazymkvctl selftest`, which prints the `selftest_result` payload as JSON, `{ "checks": [ { "name", "ok", "detail", "path", "version", "found", "busy" } ] }`, and exits 0.
- Produces the tree, where `<v>` is the workspace version read with `cargo metadata --no-deps --format-version 1`:

```
lazymkv-<v>\
  bin\        lazymkv-engine.exe  lazymkv-ocr.exe  lazymkvctl.exe
  resources\  ffmpeg.exe  ffprobe.exe  mkvmerge.exe
  licenses\   lazymkv-LICENSE.txt             the repository's LICENSE
              ocrs-models.txt                 the models' part as lazymkvctl licenses prints it: attribution, then text
              crate-notices.txt               crates/ctl/crate-notices.txt
              mkvtoolnix-COPYING.txt          COPYING of the MKVToolNix source archive
              ffmpeg-COPYING.LGPLv2.1.txt     from the FFmpeg tarball
              ffmpeg-LICENSE.md               from the FFmpeg tarball
              ffmpeg-build-settings.txt
              zlib-LICENSE.txt
              mingw-w64-runtime-copyright.txt
              SOURCES.txt                     one "<sha256>  <asset name>" line per source asset
```

- Produces the dry run's artifact `lazymkv-release-dry-run` (`retention-days: 14`): `lazymkv-<v>-windows-x64.zip`, `mkvtoolnix-100.0.tar.xz`, `ffmpeg-9.0.2.tar.xz`, `zlib-1.3.2.tar.xz`, `ffmpeg-build-settings.txt`, `text-detection.onnx`, `text-recognition.onnx`, `ocrs-models.txt`, and `SHA256SUMS` over all of them.
- Produces `packaging/release/smoke.ps1 -Zip <file> -Root <folder that does not exist> -Data <folder that does not exist> [-Mkvmerge 100.0] [-Ffmpeg 9.0.2]`, for Windows PowerShell 5.1 and PowerShell 7, used by the release job and by Task 6. Its last line is `SMOKE-DONE status=ok` or `SMOKE-DONE status=error`, exit 0 or 1.

- [ ] **Step 1: Write the failing tests.** In `crates/ctl/src/tests.rs`:

```rust
#[test]
fn a_release_tree_prints_each_text_once() {
    // release/licenses/ holds lazymkv-LICENSE.txt = MIT_TEXT, crate-notices.txt = CRATE_NOTICES,
    // ocrs-models.txt = format!("{ATTRIBUTION}\n{LICENCE_TEXT}"), and zlib-LICENSE.txt = "made-up zlib licence\n".
    // licenses_text(Some(release/bin/lazymkvctl.exe)) equals the four parts of every licenses output followed by
    // exactly one bundled part, "Bundled tool licence: zlib-LICENSE.txt".
}

#[test]
fn a_licence_file_that_differs_from_the_embedded_text_by_one_byte_is_still_printed() {
    // lazymkv-LICENSE.txt = MIT_TEXT with one letter changed: its bundled part is printed.
}
```

  In `crates/fake-tools/tests/scripts.rs`, with `parse_errors`: `the_release_scripts_parse_without_errors` (both `.ps1` files; Windows only, as its neighbours).
- [ ] **Step 2: Run to see them fail.** `scripts/box.sh cargo test -p lazymkv-ctl --locked release_tree`: FAIL, three texts printed twice.
- [ ] **Step 3: `bundled` skips what was printed (T6-R9).** `bundled(exe, printed: &[&str])` leaves out a file whose text, with a final newline added when it has none, equals one of `printed` under the same rule; `licenses_text` passes the bodies of its MIT part, models part and crate notices. Run for PASS; the existing licences tests stay green.
- [ ] **Step 4: `assemble.ps1 -Bin <dir> -Ffmpeg <dir> -Out <dir>`** (PowerShell 7, the runner only), design part 11.5's steps 2 to 6 without the release itself. It calls no `bash`: from PowerShell on a Windows runner that name can resolve to the Windows Subsystem for Linux, so the gate runs in the workflow's own `shell: bash` steps (Step 6).
  1. Fetch the MKVToolNix portable archive and the three source archives with `curl.exe --fail --location`, check each SHA-256 against the manifest, fail on a mismatch.
  2. Extract `mkvmerge.exe` alone with `7z e` into `resources\`; run `resources\mkvmerge.exe --version`; its first line must start `mkvmerge v100.0 `.
  3. Copy the two FFmpeg programs into `resources\`; the workflow has already checked their hashes (Step 6).
  4. Copy the three programs into `bin\`. Write `licenses\`. The two repository files are copied as bytes. `ocrs-models.txt` is cut as bytes: `cmd /c "<Bin>\lazymkvctl.exe licenses > <file>"`, then the bytes between the first `The ocrs models: CC-BY-SA-4.0\n\n` and the next `\nThe Movie Database (TMDB)\n`, written with `[IO.File]::WriteAllBytes`; a PowerShell pipeline would re-encode the text and change its line ends. MKVToolNix's `COPYING` comes out of its checked source archive with 7-Zip, the `.xz` layer and then the tar, since Windows' own `tar` reads no xz. FFmpeg's two files, zlib's `LICENSE`, the build settings and the mingw-w64 notice come from the FFmpeg artifact. Then `SOURCES.txt`.
  5. The check of call 8, on bytes read as UTF-8: `cmd /c "bin\lazymkvctl.exe licenses > <file>"` from the tree exits 0; the file holds `crates/ctl/crate-notices.txt`'s text exactly once, and one `Bundled tool licence: <name>` heading for each of the seven files that are not the three repository texts, and none for those three.
  6. The signing step: print `signing: the release is unsigned (P15)`.
  7. `7z a -tzip` the tree into `lazymkv-<v>-windows-x64.zip`; `bin\lazymkvctl.exe export-models` into the output folder, each model's SHA-256 checked against the manifest; copy the assets; write `SHA256SUMS`.
- [ ] **Step 5: `smoke.ps1`** (call 13, Review Focus 5). Refuse an existing `-Root` or `-Data`. `Expand-Archive` into `-Root`; set the owner of the whole tree to Administrators (`S-1-5-32-544`) and the protected list `D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)` on `-Root`, children inheriting; start `bin\lazymkv-engine.exe --data <Data>` hidden with its standard error to a file; poll `bin\lazymkvctl.exe status` for up to 60 s; run `bin\lazymkvctl.exe selftest` and parse its JSON. For each of `mkvmerge`, `ffprobe`, `ffmpeg`: `ok` true, `found` `resources`, `version` the expected one; a miss prints the check's `ok`, `found`, `version` and `detail`, and the tree's owner and access list. Then `bin\lazymkvctl.exe stop`, wait up to 30 s for the engine to exit, and kill it if it has not. `finally`: the engine is not left running; `-Data` is removed; `-Root` is left for the caller. It prints no account name: the access list is printed in SDDL form with any `S-1-5-21-…` written `<sid>`.
- [ ] **Step 6: `.github/workflows/release.yml`.** `permissions: contents: read` at the top. Triggers by T6-R10, plus `push: tags: ['v*']`. `concurrency` by ref, cancelling in progress.
  Job `ffmpeg`: `uses: ./.github/workflows/ffmpeg.yml`.
  Job `package` (`windows-latest`, `needs: ffmpeg`, `timeout-minutes: 120`): checkout; the pinned toolchain; the model cache as in Task 3; `cargo build --locked --release -p lazymkv-engine -p lazymkv-ocr -p lazymkv-ctl`; download `ffmpeg-windows-x64`; then three `shell: bash` steps: `bash packaging/release/gate-selftest.sh`; the digest step, which fails unless `bash packaging/ffmpeg/inputs-digest.sh` prints the artifact's `inputs-sha256.txt` (T6-R6); and the gate step, in which `gate.sh hashes` on the artifact's two programs must exit 0 (step 3 of design part 11.5) and `gate.sh status`, when it is not `verified`, is written as `::warning::the FFmpeg build is not verified on the target machine: <reason>`; then `assemble.ps1`; `smoke.ps1` with `-Root` and `-Data` under `$env:RUNNER_TEMP`; upload `lazymkv-release-dry-run`.
  Job `publish` (`ubuntu-24.04`, `needs: package`, `if: startsWith(github.ref, 'refs/tags/v')`, `permissions: contents: write`, `timeout-minutes: 15`, `env: GH_TOKEN: ${{ github.token }}`): checkout; download the artifact; `bash packaging/release/gate.sh publish "$GITHUB_REF" "<v>"`, the version read from `Cargo.toml`'s `[workspace.package]`; then `gh release create "$GITHUB_REF_NAME" --draft --verify-tag --title "lazymkv <v>" --notes "This release is unsigned (P15)."` with every file of the artifact.
- [ ] **Step 7: The workspace run, commit and push.** Commit `packaging: the release layout, its zip and the release workflow's dry run`; push. The pull request now runs `release.yml`. Watch it and CI once each.
- [ ] **Step 8: Read the dry run.** Expected: `ffmpeg / build` and `ffmpeg / smoke` green; `package` green with the warning `the FFmpeg build is not verified on the target machine: the build was never verified on the target machine`; `smoke.ps1`'s last line `SMOKE-DONE status=ok`; `publish` skipped; the artifact lists the nine files. If `lazymkv-ocr` does not build on the runner, the error goes to the owner before anything changes (it would be in a frozen path). If the engine cannot start on the runner, T6-R16's test replaces the engine in the smoke step, in a `(fix)` commit, and Task 6's run of `smoke.ps1` on the mini PC is the engine's run.
- [ ] **Step 9: Evidence.** `.superpowers/pr6/pr6-mini-pc-runs.txt` gets `<date>, controller, the release dry run <run id> on <short sha>: the FFmpeg build reproduced <ffmpeg sha256 first 12> and <ffprobe sha256 first 12>; the zip <bytes> bytes; smoke ok (mkvmerge 100.0, ffprobe 9.0.2 and ffmpeg 9.0.2 found in resources); unverified warning shown; publish skipped`.

---

### Task 5: The sitting's scripts, and the check script's run on itself (calls 11, 12; T6-R12 to T6-R15)

**Files:**
- Create: `scripts/phase0/ffmpeg-check.ps1`, `scripts/phase0/worker-check.ps1`
- Modify: `scripts/phase0/audio-levels.ps1` (two parameters), `crates/fake-tools/tests/scripts.rs`

**Interfaces:**
- Consumes: `scripts/phase0/env.ps1` (`$P0`, `$Staging`, `$TuningData`, `$Recordings`, `$CAP`, `Resolve-Tool`), `job.sh` and `wait.sh`, `engine-ocr-trial.ps1`'s `Get-TrialRows`, `Copy-Checked`, `Assert-Quiet` and its `inventory` step (copied, not shared, since each script runs alone), `C:\lazymkv\phase0\trial-tracks.txt`, the trial's kept output `$TuningData\<slug>\<label>\subs-ocr-engine\sub-<n>.ocr.jsonl`, `lazymkv_tools::probe_args` (`-v error -show_streams -show_format -of json <file>`) and `packets_args` (`-v error -select_streams s:<n> -show_entries packet=pts_time,duration_time,size -of json <file>`), and the two commands `C:\lazymkv\target\release\lazymkv-dev.exe golden check --evidence <dir>` and `... retune --evidence <dir> --spec C:\lazymkv\src\docs\superpowers\specs\2026-10-04-lazymkv-design.md`.
- `job.sh` passes its arguments unquoted and `run-interactive.ps1`'s argument line allows no quote, so neither script takes a path with a space, and every path has a default. On the Mac a Windows path argument is written in single quotes, since bash would otherwise eat its backslashes.
- Produces `audio-levels.ps1 ... [-RecordingsRoot <dir>] [-TuningRoot <dir>]` (T6-R12).
- Produces `ffmpeg-check.ps1`, run through `job.sh`, in one of two forms:
  - `-Self`: the candidate is the FFmpeg `env.ps1` resolves, the installed build; `PATH` is not changed; the `hashes` line reads `hashes: not checked (the installed build)`.
  - `-FfmpegSha256 <hex> -FfprobeSha256 <hex>`: the candidate is the folder `-Candidate` (default `C:\lazymkv\tmp\pr6\ffmpeg`), whose two programs must have those hashes; the script puts that folder first on `PATH` before it dot-sources `env.ps1` and fails unless `$FFMPEG` and `$FFPROBE` then resolve inside it.
  - Both forms take `-Evidence` (default `C:\lazymkv\evidence\owner`) and `-Work` (default `C:\lazymkv\tmp\pr6\check`, which must not exist; the script creates and removes it and writes nowhere else).
  - It finds the film itself: the one recording under `$Recordings` whose name ends `-uhd` and whose `all-tracks` folder holds a level folder `audio-levels-20ms-<main>[-<cand>...]` with `audio-proxy.json`; none or several is a failure. `<main>` and the candidates are read from that folder's name.
  - It prints one line per part, in this order, then `RESULT ok` or `RESULT FAILED: <reason>`:

```
hashes: equal | different | not checked (the installed build)
levels: <n> series files, <e> equal, <d> different; streamhash equal|different; graph equal|different; manifest equal|different
proxy: <t> tracks, packet hashes equal|different
listing: <a> audio streams, <k> differing keys; other streams: <k2> differing keys<: kind key, kind key, ...>
packets row <r>: equal|different            (six lines)
corpus copy: <files> files, <bytes> bytes
golden check: same output | different output; exit <before> then <after>
retune: same output | different output; exit <before> then <after>
```

- Produces `worker-check.ps1 -Row <1..6> -WorkerSha256 <hex>`, run through `job.sh` once per row with the engine stopped, with `-Bin` (default `C:\lazymkv\tmp\pr6\bin`, holding `lazymkv-dev.exe` and the zip's `lazymkv-ocr.exe`) and `-Work` (default `C:\lazymkv\tmp\pr6\worker-<row>`, created and removed by the script). Its lines, then `RESULT ok` or `RESULT FAILED: <reason>`:

```
row <r>: worker sha256 equal | different
row <r>: events <n>, wall <s> s, end <the read line's end word>
row <r>: peak committed <MiB> MiB
row <r>: records equal | different (<lines> lines here, <lines> kept, <d> lines differ) | not compared (no kept output)
```

- [ ] **Step 1: Write the failing tests** in `crates/fake-tools/tests/scripts.rs`, in the style of `the_engine_ocr_trial_script_parses_sets_rten_num_threads_for_rows_and_prints_numbers_only`:

```rust
#[test]
fn the_ffmpeg_check_script_parses_and_prints_no_name() {
    // parse_errors is empty (Windows). The text holds the stems of the eight kinds of line above. No output line
    // interpolates the film's folder name: the variable that holds it appears only inside path expressions and
    // comparisons. Its one call of audio-levels.ps1 passes -RecordingsRoot and -TuningRoot and no -Force.
}

#[test]
fn the_worker_check_script_parses_names_rows_only_and_never_writes_the_trials_kept_output() {
    // parse_errors is empty (Windows). Every output line starts "row ${Row}:" or "RESULT". The text holds no
    // Copy-Item, Copy-Checked, Set-Content, Out-File or WriteAll call whose destination names subs-ocr-engine.
}

#[test]
fn audio_levels_takes_its_two_roots_only_when_given() {
    // The param block holds [string]$RecordingsRoot = '' and [string]$TuningRoot = ''; after the dot-source of
    // env.ps1 each replaces its variable only when not empty.
}
```

- [ ] **Step 2: Run to see them fail.** `scripts/box.sh cargo test -p fake-tools --locked --test scripts`: FAIL, the files are missing.
- [ ] **Step 3: `audio-levels.ps1`'s two parameters**, with one header line naming T6-R12.
- [ ] **Step 4: `ffmpeg-check.ps1`** by T6-R13.
  - The level pass is `audio-levels.ps1 -Slug <film> -Label all-tracks -Main <main> -Cand <cands> -RecordingsRoot <Work>\recordings -TuningRoot <Work>\dataset`.
  - The proxy's packet hashes come from the installed full `ffmpeg -v error -i <proxy> -map 0 -c copy -f streamhash -hash md5 -` over the new proxy and over phase 0's, each captured with `& $CAP record --out`.
  - The listing is the candidate `ffprobe` with `probe_args`' words over the kept rip, captured with `& $CAP record --out`, and compared with `$Recordings\<film>\all-tracks\ffprobe-streams.json` after both are parsed. A key is named by its path inside its stream entry; values are never printed.
  - The packet listings use `packets_args`' words over each row's `evidence.mks`, found as `engine-ocr-trial.ps1`'s `inventory` step finds it, captured with `& $CAP record --out`, and compared as bytes with `$Recordings\<slug>\<label>\subs\sub-<n>.packets.json`.
  - For `golden check` and `retune`: copy the whole of `-Evidence` to `<Work>\evidence`, refusing above 4 GB; run both commands over the copy through `& $CAP record --out` and keep the two captures; put the film's new listing and new level folder into the copy; run both again; compare each pair of captures as bytes.
- [ ] **Step 5: `worker-check.ps1`** by call 11 and T6-R15. `Assert-Quiet` first. The evidence file is copied to `-Work` before the run and deleted after it. A background job samples `PeakPagedMemorySize64` of the process named `lazymkv-ocr` every 500 ms and keeps the largest value seen. `lazymkv-dev ocr-trial read` runs from `-Bin` as `engine-ocr-trial.ps1`'s `read` step runs it, into `-Work`. The records are read as UTF-8 lines; each line's last member, matched by `,"ms":\{[^{}]*\}\}$`, becomes `}`; the lines are then compared in order. `finally` removes `-Work`.
- [ ] **Step 6: Run for PASS**, the workspace run, then commit and push: `scripts: the FFmpeg check and the worker check for the mini PC`. CI green.
- [ ] **Step 7: The check script on itself (T6-R14; controller, with owner item 4's yes).** Message the disc-menu session. `scripts/box.sh "cargo build --release --locked -p lazymkv-dev -p lazymkv-capture"`, which also places the scripts in `C:\lazymkv\src`. `BOX 'if not exist C:\lazymkv\tmp\pr6 mkdir C:\lazymkv\tmp\pr6'`. Then `scripts/phase0/job.sh ffmpeg-check.ps1 pr6-check-self -Self` and `scripts/phase0/wait.sh pr6-check-self` until it exits 0 or 1; no `scripts/box.sh` runs meanwhile. Expected: `levels` with 0 different and the three `equal`; `proxy` equal; `listing` with `0 differing keys` twice; six `packets row <r>: equal`; `golden check: same output`; `retune: same output`; `RESULT ok`. Anything else is a fault of the script: a `(fix)` commit, pushed, CI green, and this step again. Evidence: `<date>, desktop task, ffmpeg-check.ps1 on the installed FFmpeg 9.0.2: <the lines' numbers>; the corpus copy <files> files, <bytes> bytes, removed`.

---

### Task 6: The sitting on the mini PC, and `verified` (calls 4, 11, 12; T6-R17, T6-R18; controller and owner, manual)

**Files:** Modify (git-ignored, append only): `.superpowers/pr6/pr6-mini-pc-runs.txt`, `.superpowers/pr6/decisions.txt`. Modify and commit: `packaging/tools.json` (`built.verified`). On the mini PC only, all removed in Step 10: `C:\lazymkv\tmp\pr6\`, `C:\lazymkv\dev\tools\pr6-zip\`.

**Interfaces:** Consumes Task 4's green dry run `<run>` and its two artifacts, Task 5's scripts and its green run on the installed build, the manifest's two hashes. Produces the evidence lines Task 7 copies into `docs/phase2/engine.md`, and the manifest's `verified`.

Every value that names the film, a track, a share or the account stays in the controller's session. Evidence lines use `<slug>`, `<share>` and row numbers.

- [ ] **Step 0: Preconditions.** Tasks 0 to 5 are pushed, Task 5 Step 7 printed `RESULT ok`; CI, `ffmpeg.yml` and `release.yml` are green on `git rev-parse HEAD`; CodeQL shows 0 open alerts on the pull request; `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` shows no active job.
- [ ] **Step 1: The owner's yeses and the time.** One message: what the sitting does (the four items of "Owner prerequisites" 5), that it takes about an hour and a half with the owner logged on at the monitor and the engine stopped, and that the drive stays empty. Wait for an explicit yes to each; record `T6 sitting <item>, <date>, <answer>`. A no to an item records the parts that need it as `not run: <answer>` and leaves `verified` null. Tell the disc-menu session the time.
- [ ] **Step 2: The artifacts.** On the Mac: `gh run download <run> -n ffmpeg-windows-x64 -D "$SCRATCH/ffmpeg"` and `-n lazymkv-release-dry-run -D "$SCRATCH/release"`; `shasum -a 256 -c SHA256SUMS` passes in each folder; the two programs' values equal the manifest's; `unzip -p "$SCRATCH/release/<zip>" lazymkv-0.0.0/bin/lazymkv-ocr.exe | shasum -a 256` gives `<worker sha256>`. `BOX 'mkdir C:\lazymkv\tmp\pr6\ffmpeg'` (its parent is Task 5's). Copy with `scp`: the two programs into `C:\lazymkv\tmp\pr6\ffmpeg\`, the zip into `C:\lazymkv\tmp\pr6\`. `BOX` `Get-FileHash` of the three files prints the same values.
- [ ] **Step 3: The builds.** `scripts/box.sh "cargo build --release --locked -p lazymkv-ocr -p lazymkv-dev -p lazymkv-capture --features lazymkv-dev/ocr-trial"`, then `scripts/box.sh cargo test -p lazymkv-platform --locked --no-run`. These are the last `scripts/box.sh` calls of the sitting.
- [ ] **Step 4: Stop the engine.** `BOX 'C:\lazymkv\run\lazymkvctl.exe stop'` exits 0; `status` then exits 4.
- [ ] **Step 5: The limit in the desktop session (T6-R18).** `scripts/phase0/job.sh platform-ignored.ps1 pr6-limit -Filter win::job::tests::a_child_that_allocates`, then `wait.sh pr6-limit`. Expected: `test result: ok. 1 passed`. A failure of the 10 s bound goes to the owner as a finding against A1; the sitting goes on.
- [ ] **Step 6: The FFmpeg check (design part 11.3).** `scripts/phase0/job.sh ffmpeg-check.ps1 pr6-check -FfmpegSha256 <hex> -FfprobeSha256 <hex>` with the manifest's two values, then `wait.sh pr6-check` until it exits 0 or 1. Expected by T6-R13: `hashes: equal`; `levels` with 0 different, `streamhash equal`, `graph equal`, `manifest equal`; `proxy` equal; `listing` with 0 differing keys on the audio streams; six `packets row <r>: equal`; `golden check: same output`; `retune: same output`. A stop by T6-R13 goes to the owner with the numbers and the key names, `verified` stays null, and Steps 7 and 8 still run.
- [ ] **Step 7: The zip's tree (call 12 (b), T6-R17).** Over SSH: `BOX 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\packaging\release\smoke.ps1 -Zip C:\lazymkv\tmp\pr6\lazymkv-0.0.0-windows-x64.zip -Root C:\lazymkv\dev\tools\pr6-zip -Data C:\lazymkv\tmp\pr6\data'`. Expected: `SMOKE-DONE status=ok`.
- [ ] **Step 8: The zip's worker (call 11).** `BOX`: make `C:\lazymkv\tmp\pr6\bin\` with a copy of `C:\lazymkv\target\release\lazymkv-dev.exe` and of `C:\lazymkv\dev\tools\pr6-zip\lazymkv-0.0.0\bin\lazymkv-ocr.exe`. For each row 1 to 6: `scripts/phase0/job.sh worker-check.ps1 pr6-worker-<r> -Row <r> -WorkerSha256 <worker sha256>`, then `wait.sh pr6-worker-<r>`. Expected for every row: `worker sha256 equal`, `end Read`, a peak under 1024 MiB, `records equal`; row 4's wall under 600 s. A peak above 1024 MiB, a `different`, or row 4 above 600 s goes to the owner with the numbers (call 11).
- [ ] **Step 9: Restart.** `BOX 'powershell -NoProfile -Command "Start-ScheduledTask -TaskName lazymkv-engine"'`; `status` exits 0 within 60 s.
- [ ] **Step 10: Clean up.** `BOX 'rmdir /s /q C:\lazymkv\tmp\pr6'` and `BOX 'rmdir /s /q C:\lazymkv\dev\tools\pr6-zip'`; a `dir` of each then finds nothing. Tell the disc-menu session the sitting is over.
- [ ] **Step 11: Evidence.** One line per step in `pr6-mini-pc-runs.txt`, in engine.md's form `<date>, <SSH | desktop task | owner | controller>, <what ran>: <result>`, with the numbers the scripts printed.
- [ ] **Step 12: `verified` (only after Step 6 passed with nothing stopped).** Set `built.verified` to `{ "date": "<today>", "inputs_sha256": "<bash packaging/ffmpeg/inputs-digest.sh>" }`; `bash packaging/release/gate.sh status` prints `verified`. Commit `packaging: the FFmpeg build is verified on the mini PC`; push; watch `release.yml` once. Expected: `package` green with no warning annotation; `publish` skipped.

---

### Task 7: The record, the design's amendments and the counts (call 16)

**Files:** Modify: `docs/phase2/engine.md`, `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md`. Create (git-ignored): `.superpowers/pr6/pr6-test-list.txt`, `.superpowers/pr6/pr6-junction-run.txt`.

**Interfaces:** Consumes Task 0's baseline, every task's commits, `pr6-mini-pc-runs.txt`, `decisions.txt`.

- [ ] **Step 1: The final runs.** The agreed window; `scripts/box.sh "cargo test --workspace --locked -- --list 2>&1" > .superpowers/pr6/pr6-test-list.txt`; the junction run into `.superpowers/pr6/pr6-junction-run.txt`. Read the listed count per crate, the passed, failed and ignored sums and the `skipped:` lines. Expected: 0 failed, 13 ignored, the baseline's `skipped:` lines and no new one; `git diff --exit-code Cargo.lock` prints nothing.
- [ ] **Step 2: `docs/phase2/engine.md`.** In pull request 5's form:
  - a "Pull request 6: the release" section after "Pull request 5", listing what was delivered with each item's notes decision and task;
  - "How to run it" gains lines for `ffmpeg.yml`, `release.yml`'s dry run, `smoke.ps1`, the two check scripts and the worker's limit;
  - "Pull request 6: the planner's rulings (T6-R1 to T6-R18)", each marked "not yet ruled on by the owner" unless `decisions.txt` holds the owner's answer;
  - "Pull request 6: the owner's decisions" (A1 to F1, the notes' approval, the pin list, the sitting's yeses), and "Pull request 6: rulings made during execution" for anything the tasks added;
  - S80's sentence amended: the self-test judges the two minimums since this pull request (T5-R65 closed);
  - "Left for later pull requests": the "Pull request 6" paragraph becomes a done list with where each item closed, as pull request 5's; the decoders' size caps and the inflate limit stay listed for the next reopened trial; new entries: holding a job for a tool below its minimum (phase 3), the first real version's edit of the frozen `Cargo.toml`, moving the workflows to the actions' newer majors, zlib 1.4;
  - "Pull request 6 on the mini PC" from `pr6-mini-pc-runs.txt`;
  - "Tests": the listed count, the per-crate counts, what this pull request added against Task 0's baseline, the junction run's numbers, and the CI run's per-system numbers.
- [ ] **Step 3: The design's amendments, each marked "(amended by pull request 6's notes, <id>)".** Part 5.3's "Hard limits" gains the worker's memory limit and keeps "and no others" true (A1); part 11.3's runner sentence, "when it runs", manifest sentence and smoke test (calls 2, 4, 5); part 11.5's steps 3 and 6 (call 4, D1). Show the owner the exact new sentences and wait for the yes (owner item 6).
- [ ] **Step 4: The checks.** The freeze check; `scripts/phase0/check-names.sh` prints nothing; a search of the changed docs for an em-dash or an en-dash prints nothing.
- [ ] **Step 5: Commit and push.** `docs: pull request 6 in the phase 2 notes, with the design's amendments`. CI green; CodeQL 0.
- [ ] **Step 6: The whole-branch review.** superpowers:subagent-driven-development's final review over `fd945ae..HEAD`, with one fix wave; its confirmed-and-left items go into engine.md's "Accepted in pull request 6 and left as they are" in a last docs commit, with the counts run again if a test was added.
- [ ] **Step 7: Hand over.** Tell the owner the pull request is ready to leave draft, with the dry run's link, the mini PC evidence and the open rulings. It leaves draft only on the owner's word.

---

## Requirement map

| Scope item (the notes' section 2) | Tasks |
|---|---|
| The release layout and its `resources` folder in use | 4, 6 Step 7 |
| The zip | 4 |
| The release workflow's dry run | 4, 6 Step 12 |
| The minimal FFmpeg build and its once-only re-run | 3, 5, 6 |
| The release job's use of the committed crate notices | 4 Steps 3 and 4 |
| The self-test's minimum versions | 2 |
| The bound on the decoders' memory | 1, 6 Steps 5 and 8 |
| The record and the test counts | 0, 7 |

## Not covered

Pull request 7 (acceptance, `compare-job`, the second disc); signing; an installer; a tag or a published release; size caps inside the decoders; holding a job for a tool below its minimum; the desktop task running the release layout.
