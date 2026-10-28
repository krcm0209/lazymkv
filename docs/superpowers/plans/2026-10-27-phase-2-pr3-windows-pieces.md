# Phase 2 Pull Request 3: Windows Pieces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The engine runs on Windows behind its mutex and its protected named pipe, with the Windows implementations of every `platform` trait but `Autostart`, `lazymkvctl`'s real pipe client, the share connection of P7 and the desktop task script of P5, and a stand-in disc goes from arrival to a filed file through the real pipe, the Job Object runner and the Windows file moves on `windows-latest` and the mini PC (design part 12.6, item 3), while every item that `docs/phase2/engine.md` leaves for pull request 3 is closed.

**Architecture:**
- `platform` gains the only Windows code. Every Win32 call sits in `platform::win` (`#[cfg(windows)]`) and wraps its `unsafe`. The pure parts it needs (security descriptor strings, token verdicts, the share and volume rules, the drive diff, the local-time formatter, the eject report) sit in modules that compile and are tested on all three systems.
- The traits keep their PR 2 shapes, with four additions: `IpcTransport::close`, `ShareConnector::access` (a default method), and the `Host` methods `now_ms`, `local_time` and `user_writable`. `NotifySink::deliver` returns a `Result`.
- `engine` gains a Windows entry (`engine::native`) that runs the start-up sequence of design parts 3.2, 3.3 and 3.5 (elevation, mutex, data folder, first pipe instance), builds the native `Platform`, mirrors `engine.log` to standard error, maps the console's stop events to a forced shutdown, and waits for the owner to stop. The non-Windows binary keeps its S28 sentence and exit 1.
- `ctl`'s Windows binary passes `platform::win::PipeConnector`; Linux and macOS keep `connector: None` and `NO_TRANSPORT` (S38).
- The engine-logic leftovers of PR 2 (share work off the owner thread, bounded MakeMKV checks, L7, the fingerprint's temporary folder, unpersisted transitions, the `command` relay) are fixed on the fakes and stand-ins, so all three CI systems test them.
- `scripts/engine-task.ps1` deploys and starts the desktop task; it and every mini PC run that changes the machine wait for the owner's yes.

**Tech Stack:** Rust 1.99.0 (`rust-toolchain.toml`), edition 2021.
- Workspace crates in use: `serde`, `serde_json` (`preserve_order`, `float_roundtrip`), `md-5`, `sha1`, `regex`, `fancy-regex`, `unicode-normalization` (Cargo.toml:10-19).
- **One new external crate:** `windows-sys`, pinned exactly in `[workspace.dependencies]` (`windows-sys = "=0.61.2"`, the candidate Task 1 confirms against crates.io), used by `lazymkv-platform` only, as `[target.'cfg(windows)'.dependencies] windows-sys = { workspace = true, features = [...] }` with exactly these features: `Win32_Foundation`, `Win32_Security`, `Win32_Security_Authorization`, `Win32_Security_Credentials`, `Win32_Storage_FileSystem`, `Win32_System_Console`, `Win32_System_Diagnostics_Debug`, `Win32_System_Diagnostics_ToolHelp`, `Win32_System_IO`, `Win32_System_Ioctl`, `Win32_System_JobObjects`, `Win32_System_LibraryLoader`, `Win32_System_Pipes`, `Win32_System_Power`, `Win32_System_Registry`, `Win32_System_SystemInformation`, `Win32_System_Threading`, `Win32_System_Time`, `Win32_NetworkManagement_WNet`, `Win32_UI_WindowsAndMessaging`.
- Expected lock delta: 43 to 45 packages, `windows-sys 0.61.2` and `windows-link 0.2.1`, both `MIT OR Apache-2.0`. Task 1 reads the real list from crates.io and prints it for the owner's yes before anything is fetched; a different list or licence goes back to the owner.
- Nothing else new: no tokio, reqwest, clap, `windows` crate, date-time, Matroska or tar crate (design part 4.6); `ureq` waits for PR 4. No new path edge between workspace crates.

**Spec:** `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (the design; binding with P1 to P37, R-A, R-B, K1 to K15 and L1 to L15), `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (the spec), and `docs/phase2/engine.md` (S1 to S88, PF-1 to PF-34, R22 to R29, I1 to I6, the M items, F1, RR1-1 to RR1-16, and the "Pull request 3" list at engine.md:572-594). `docs/phase1/decision-core.md` holds the phase 1 rulings that still bind (RP19, RP21, RP23, RP24).

**Branch:** `phase-2-windows-pieces`, from `main` at `dcd5924` (the merge of pull request #6).

## Global Constraints

- **Toolchain and commands.** Rust 1.99.0 from `rust-toolchain.toml`. Every cargo command runs on the mini PC through `scripts/box.sh`, one at a time, and never while a real disc rips (design part 12.5). The Mac has no toolchain.
  - A task's **Run** is its targeted command; before its commit the task runs `scripts/box.sh cargo test --workspace --locked` once and it must pass.
  - A task that touches dev's `decide`, `plan` or `retune`, the RP21 guard, or decide's inputs (Task 18), or adds a file under `corpus/scenarios/` (Task 22), runs the junction form for its workspace run, since the RP21 guard `no_scenario_holds_a_clone_value` runs only there: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`.
  - `LAZYMKV_OCR_TEXT` is never set. Nobody reads or writes `corpus/owner`.
  - `-p lazymkv-platform` builds the crate without its `fakes` feature (`crates/platform/Cargo.toml:7-10`). Platform's own tests outside `fakes.rs` never use `crate::fakes` (they use `testutil::Secrets`); a command that runs `fakes.rs`'s own tests adds `--features lazymkv-platform/fakes`.
  - A filter matches test paths (module path and function name), not file names: a command that runs one integration-test file names it with `--test <file stem>`, and `--test` is given only with `-p fake-tools`.
- **Dependencies.** `windows-sys` is the only new crate. Task 1 fetches only after the owner's yes on its printed delta, and every cargo command of this plan, Task 1's fetch included, uses `--locked`. No later task edits a `Cargo.toml` dependency list.
- **Windows code placement.** Only `platform` calls `windows-sys`, inside `platform::win`, and every `unsafe` block there has a `// SAFETY:` line. `capture`'s hand-declared eject stays as it is (design part 4.2). Production names (`PIPE_NAME`, `MUTEX_NAME`, the `lazymkv:` credential prefix, `%ProgramData%\lazymkv`) are used only by the binaries, and by `win::shares::arranged`, which reads the entry the owner stored (Owner decision 7); every test uses a name of its own made from `security::test_name(tag)` (`lazymkv-test-<tag>-<pid>-<counter>`; public and all OS, so the tests of `ctl`, `engine` and `fake-tools` reach it), composed as `format!(r"\\.\pipe\{}", test_name(t))`, `format!(r"Global\{}", test_name(t))` and `format!("{}:", test_name(t))` (a credential prefix), so tests never meet a running engine or each other.
- **No I/O in core (RP19).** The one core addition, `ipc::share_credential`, is a pure string function.
- **RP21.** No file under `crates/` or `corpus/scenarios/` holds a byte copy of, or a value read only from, `corpus/owner`. New scenarios are synthetic.
- **Unchanged output.** The goldens stay byte-identical (K5) and so does `capture disc`'s output (R-B2). The only changed engine documents are `rip-summary.json` (its new `started` key, Task 9) and `job.log` (the share access line, which fakes never produce, Task 17).
- **Docs and prose.** Every `pub` item has a `///` doc naming its spec section, design part or ruling, ending "Def: T<n>. Use: T<m>.", with T numbers of this plan written "T3-<n>" where they could be confused with PR 2's. Prose is plain sentences, no em-dashes, acronyms expanded at first use.
- **Tests.**
  - Tests go in the file's `mod tests` or a sibling `tests.rs`, named as snake_case sentences. Tests that run `fake-tool` live in `crates/fake-tools/tests/` (S12).
  - Windows tests compile only under `#[cfg(windows)]` and run on `windows-latest` and on the mini PC over SSH; traits, fakes and pure helpers compile and are tested everywhere (design part 12.4).
  - Tests that need the drive, the owner's desktop session or the share carry `#[ignore = "<reason>"]` and read their inputs from `LAZYMKV_TEST_DRIVE` (a drive id such as `D:`) and `LAZYMKV_TEST_SHARE_ROOT` (a UNC folder under `lazymkv-staging`), printing `skipped: <reason>` when unset (design part 12.1's convention). A printed `skipped:` line counts as not run in the notes (Task 26). Where one file holds ignored tests for different sessions, they sit in test modules named for the session (`win::shares`' `desktop`, `over_ssh` and `arranged`), so one filter selects one session's tests. A test that waits for the owner writes its cue with `testutil::cue` straight to standard error, so the harness's capture never holds it back.
  - A wait that only waits scales by `ci_factor()` (R29.c2); a bound that proves "does not block" (K12, the share stalls) is set far above normal latency and does not scale. `platform` gets its own `testutil::ci_factor`, `testutil::skip` and `testutil::cue` (`CUE: <text>\n` written with `std::io::stderr().write_all` and flushed, as `lazymkv_dev::evidence::skip` writes), copies of the engine's and dev's, since `platform` sits below both, and `testutil::Secrets`, a `SecretStore` over a `Mutex<BTreeMap<String, String>>` that compares names exactly, for its own tests.
  - No test prints or asserts a MakeMKV key, an account name, a drive serial or a credential; credentials in tests are sentinel values that cannot occur in a path (`user-sentinel-7c1e`, `pass-sentinel-4b9d`), asserted absent from every output.
- **Persisted files.** Every persisted write still goes through `platform::write_atomic` with its `format` (`lazymkv job v1`, `lazymkv drives v1`, `lazymkv remembered v1`); `TitleRecord`'s new optional field is skipped when absent, so files written before it read and write back unchanged (design part 7.4). Times stay Unix seconds, except `rip_started_ms` (Unix milliseconds) and phase 0's `started` text.
- **Secrets.** The share credential and the TMDb token never enter the repository, a test, a recorded file, a log line, a notification or an error text. Only the owner types them, on `lazymkvctl secret set`'s standard input. `Reach::Unreachable`, free-space errors, listing errors and `SecretStore` errors never carry a value or a user name (V15).
- **CodeQL.** The ruleset requires CodeQL's Rust results and every alert blocks the merge, so the pull request's CodeQL run shows no alert. The secret paths (`CredentialStore::set`, `secret_set`, the share fallback) never format a value into a log line, an error or a file.
- **The mini PC and the Mac.** Nothing is installed on either, no scheduled task is registered, no access list is set outside a test's own temporary folder, and no firewall rule is changed without the owner's explicit yes naming it (Owner prerequisites and decisions). Tests set access lists only on folders they created under `std::env::temp_dir()` and remove them.
- **TDD cycle per task.** Write the listed tests. Run the task's **Run** command and see the named failure (compile error or FAIL). Implement. Run again for PASS. Run the workspace once. Commit.
- **Commits.** One signed commit per task, with no attribution line. From Task 2 Step 6's yes on, each task's commit is pushed to the draft pull request, whose CI runs that task's `cfg(windows)` tests on `windows-latest`; a red run goes back to that task before the next starts. `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` must show `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>`.

## Review Focus

The five input classes or failure modes most likely to bite the owner that no task's listed tests reach. Each gets the named test in its owner task.

1. **A UNC root spelled differently from the stored share name.** Settings accept `//unas-pro/media/...` and any case (`crates/core/src/settings.rs:207-215`), while the owner types `share:\\unas-pro\media`. The connector must find the secret for the share whatever the slashes, and the volume rule must treat both spellings as one volume.
   - Tests: `a_forward_slash_root_finds_the_backslash_secret_and_a_root_in_another_case_shares_its_volume` (`shares`, all OS: the secret is stored with backslashes in the root's own case, since the test store compares exactly; `connect` gets that secret's user and password for `//unas-pro/media/x`, and `volume_key("//UNAS-PRO/Media/y") == volume_key(r"\\unas-pro\media\x")`). Owner: Task 14. And `a_secret_is_found_by_its_name_in_another_case` (`win::creds`: written as `<test prefix>share:\\server\share`, read as `<test prefix>share:\\SERVER\Share`; only Credential Manager ignores case). Owner: Task 13.
2. **The last queued line before the pipe closes.** `shutdown`'s `ok`, and the `LINE_TOO_LONG` error before a drop, are written and then the writer is dropped; the client must read them before end of stream, or `lazymkvctl stop` exits 4 with `CLOSED`.
   - Test: `the_last_queued_line_reaches_the_client_before_the_pipe_closes` (write one line, drop the server writer at once, the client reads that line and then end of stream). Owner: Task 11.
3. **A grandchild started as the child's first act.** A Job Object assigned after the child runs misses it; the child must be in its job before its first instruction.
   - Test: `a_grandchild_started_at_once_is_killed_with_its_parent` (the helper's first act starts the grandchild; the test reads the lines `child <pid>` and `grandchild <pid>` before `kill`; after `kill`, both process ids have exited and the stdout reader reaches end of file within 10 s). Owner: Task 5.
4. **A credential typed in a Windows console.** `lazymkvctl secret set share:\\server\share` at an elevated desktop terminal sends `user\r\npassword`; the split must drop the `\r`, and a third line or an empty half must be refused at `secret_set` without echoing anything.
   - Test: `a_credential_typed_in_a_windows_console_splits_into_user_and_password` (core) and `a_share_value_of_three_lines_is_refused_without_echoing_it` (fake-tools `engine_ipc.rs`). Owner: Task 13.
5. **A wrong stored password and the NAS's lockout.** Every `reach` of a denied share would try the stored credential again: two per title in File, plus CheckSpace, retries and self-tests, enough to lock the owner's NAS account.
   - Test: `a_refused_stored_credential_is_not_tried_again_until_it_changes` (scripted `ShareApi`: the second `reach` makes no `connect` call while the stored value is unchanged, and makes one after `testutil::Secrets` holds a new value). Owner: Task 14.

## Owner prerequisites and decisions

Each item names the task that waits on it. Items 1 to 3 are needed before execution reaches that task; the rest during it. The planner's recommended answer is given where one exists.

1. **The `Cargo.lock` delta (P33, ENG-7, PF-11).** `windows-sys` at its exact version and every crate the lock gains with it, versions and manifest licences read from crates.io, before any fetch. The owner's yes and its date go into engine.md. Waits: Task 1.
2. **The mini PC's console runs (design part 3.1).** Creating `C:\lazymkv\dev\data`, `C:\lazymkv\dev\staging`, `C:\lazymkv\dev\tuning`, `C:\lazymkv\dev\library\Movies`, `C:\lazymkv\dev\bin` (release copies of `lazymkv-engine.exe` and `lazymkvctl.exe`, so no later build meets a locked file) and `C:\lazymkv\dev\tools` (copies of `fake-tool.exe` named as the four tools, with the synthetic `movie-direct` scenario's files beside them); creating `C:\lazymkv\dev\data` from the elevated SSH session, so its owner is Administrators, and writing its `settings.json` (the three local roots, `tool_paths` naming the stand-ins) before the first start, since without it the start check would find and run the real `makemkvcon64.exe`; and the engine setting the protected access list on `C:\lazymkv\dev\data` at each start (design part 3.5); if that folder's owner is not Administrators, `icacls C:\lazymkv\dev\data /setowner *S-1-5-32-544` (Ruling T3-25-f); and owner Administrators with a protected list (SYSTEM and Administrators full control, Users read and execute) on `C:\lazymkv\dev\bin` and `C:\lazymkv\dev\tools` before the copies, since the elevated SSH session runs those programs from folders that would otherwise let any authenticated user write them (Ruling T3-25-i). No disc goes into `D:` while the console engine runs (design part 3.1). If `C:\lazymkv\dev\data` already exists owned by the owner's account, the engine refuses it, so the owner chooses: remove it, or let the run use a fresh folder. Recommended: remove it and create it again from the elevated session. Waits: Task 25 step 5 (the console run).
3. **The scheduled task `lazymkv-engine` (P5, design part 15 item 6).** Registering it (no trigger, the owner's account, interactive, highest run level), creating `C:\lazymkv\run` with its protected access list, and whether one yes covers `engine-task.ps1` re-registering it on every later deploy, as `run-interactive.ps1` does with `-Force`. The task's engine runs the installed `makemkvcon64.exe` for its start check and self-test, with no disc in `D:`. Recommended: one yes for the script's registrations in phase 2. Waits: Task 24 (writing the script needs no yes; running it does, in Task 25).
4. **The task's settings, which the design leaves unset.** Task Scheduler's defaults stop a task after 72 hours and run it at task priority 7 (below normal, lowered I/O and memory priority), which `makemkvcon` inherits; phase 0's recordings were made at priority 7. Recommended: `-ExecutionTimeLimit ([TimeSpan]::Zero)`, priority 7 kept (rip timing comparable with phase 0), battery conditions off. Also whether the engine's console window shows on the desktop (closing it is a forced shutdown) or the task hides it. Recommended: shown. Waits: Task 24.
5. **The desktop task's data folder.** Design part 3.1 ties `--data C:\lazymkv\dev\data` to console runs and part 15 item 6 protects `%ProgramData%\lazymkv`, while part 12.5 says development runs give the override. Recommended: the task runs without `--data`, so `%ProgramData%\lazymkv` is created and protected at its first start; the script takes `-Data DIR` for the other choice. Either way, a yes to the engine creating `%ProgramData%\lazymkv` and setting its protected access list at every start (design part 15 item 6). Waits: Task 24.
6. **The desktop-session test runs (design part 12.4).** The `DriveWatcher` and real-share `#[ignore]` tests run through phase 0's `job.sh`, which re-registers `lazymkv-phase0` with `-Force` and runs only scripts under `scripts\phase0\`. A yes for the new `scripts/phase0/platform-ignored.ps1` and for that re-registration at every `platform-ignored.ps1` job of Task 25 (Steps 4 and 8); the owner logged on at the mini PC's monitor (locked is enough) and inserting and removing a disc on each printed cue, relayed in chat from the log. No `scripts/box.sh` call runs while a desktop job runs, since each call replaces `C:\lazymkv\src`. Waits: Task 25 steps 4 and 8.
7. **P7's stored-credential fallback on the real NAS.** In the desktop session the saved Windows credential succeeds first (design part 6.1, part 15 item 11), and over SSH the vault is out of reach, so the fallback's success branch cannot fire as things stand. Recommended: the fallback's order is tested through the `ShareApi` seam on all three systems (Task 14), and the SSH failure branch (denied, no reachable credential) runs once over SSH; the success branch stays an `#[ignore]` test in `win::shares::arranged` that runs, as its own step, only if the owner arranges a share the session cannot reach and types its credential himself. The run leaves the entry `lazymkv:share:\\<server>\<share>` in Credential Manager and a connection to that share until logoff; the owner removes them with `cmdkey /delete:lazymkv:share:\\<server>\<share>` and `net use \\<server>\<share> /delete`, and the evidence writes such a share as `\\SERVER\SHARE`. Waits: Task 25 (steps 3 and 8).
8. **Credential Manager entries.** Lazymkv's entries are generic credentials, target `lazymkv:<secret name>`, persistence `CRED_PERSIST_LOCAL_MACHINE`, the value as UTF-8 bytes in the blob (Ruling W13). Confirm the persistence. Also whether the round-trip test may write and delete a `lazymkv-test-...` entry in the owner's vault in the desktop session (a run without `LAZYMKV_TEST_VAULT`, as over SSH, skips before writing anything; `platform-ignored.ps1` sets it for the `win::creds::` job only). Recommended: yes, desktop session only. Waits: Task 13 (the persistence), Task 25 step 4 (the desktop run, `-Filter win::creds::`).
9. **First writes to the NAS.** Whether PR 3's desktop runs write under `\\unas-pro\media\lazymkv-staging\engine` and `\\unas-pro\media\lazymkv-tuning\engine` (the engine creates the `engine` folders, design part 6.2) and file into `\\unas-pro\media\lazymkv-test-library\Movies` (which holds experiment 9's clips), or stay on the console's local folders. Recommended: the real-share tests write only under `lazymkv-staging`, and a real-disc job (item 10) uses the three share roots. The share tests create `\\unas-pro\media\lazymkv-staging\pr3-tests` and leave it empty (their own folder below it is removed however a test ends), and the self-test of Task 25 step 7 writes and removes a probe in the test library's `.lazymkv-partial` folder. Waits: Task 25.
10. **A real disc in the desktop task.** Whether PR 3 ends with one movie disc that is neither acceptance disc in the desktop task (Task 25 step 7). Planning never waits for a lookup (design part 8.3), so with the offline transport (W26) the job plans with its identity pending and starts ripping its first planned title to `\\unas-pro\media\lazymkv-staging\engine\<job id>\`; once its state is `Ripping`, `lazymkvctl plan --job` runs through the relay and `lazymkvctl stop --force` ends the engine, which deletes the partial rip (Ruling T3-25-c). The job folder under the data folder and the job's staging folder stay for the owner to remove. The self-test first writes and removes a probe under the test library's `.lazymkv-partial`. It exercises `DriveWatcher`, the MakeMKV listing, the scan, a rip onto the share, the `command` relay and the pipe from SSH. Recommended: yes, one disc, the owner inserting and removing it. Waits: Task 25.
11. **Rulings that change earlier rulings or the design's letter** (the owner may overturn any; engine.md:526): W3 (elevation is read before the mutex), W8 (the window's message loop runs on the watcher's own thread, not the engine's drive thread), W10 (design part 9.2 amended: `command_output` goes only to the client that sent the `command`; S52 amended: the relay waits for room in that client's queue instead of dropping it), W20 (S57 amended: listings outside a job are bounded by `limits.header`), W21 (S81 amended: AwaitingShare's re-check also reads free space and whether the staging root could be made), W25 (the relay refuses `retune --jobs` until PR 5), W17 (design part 3.6's filtered token: with no linked token a restricted token stands in), W22 (decide's inputs, R-B1-sensitive per engine.md:591: a library read error fails Decide), W24 (design part 9.5's two refusals gain `COMMAND_FORMS`, `COMMAND_WRITABLE` and `no_title`; S76's `command` refusal is retired), Ruling T3-24-a (design part 3.1's order: the run folder's list is set before the copy). Waits: Task 23 (W3), Task 16 (W8), Tasks 10 and 22 (W10), Task 19 (W20), Task 17 (W21), Task 22 (W25), Task 7 (W17), Task 18 (W22), Task 22 (W24), Task 24 (T3-24-a); none blocks its task, but an overturn reopens it.
12. **Account names on the console and in engine.log.** Design part 3.3 reports a mutex squatter "by name". Ruling W4 prints the account name on standard error only and logs no account name in `engine.log` (a refused pipe client is named by its process id). Confirm. Waits: Task 2 (`MutexRefusal::Squatter`) and Task 23 (the start-up texts); neither blocks.
13. **Residual risk in the build trees.** `engine-task.ps1` copies the elevated task's programs from `C:\lazymkv\target\release`, built from `C:\lazymkv\src`, folders under `C:\` that authenticated users can probably write. The script checks both folders and refuses when either lets a non-administrator write (Task 24). The owner chooses: (a) accept, so Task 25 step 6 passes `-AcceptWritableBuild` on this development machine; or (b) protect the build, which `icacls C:\lazymkv\src` cannot do, since `scripts/box.sh` removes and recreates that folder at every sync, so (b) means a separate protected build folder this plan does not provide, and defers the desktop task (Task 25 steps 6 to 8) to a later pull request. Recommended: (a). Waits: Task 24 (the check) and Task 25 step 6 (the answer).
14. **The desktop task's settings (design parts 3.6, 6.2, 9.4).** The engine starts no job while `staging_root` is unset, and the WinGet `Links` folder is skipped, so the desktop task needs part 6.2's share roots and `tool_paths.ffprobe` and `tool_paths.ffmpeg`. The owner writes a settings file outside the repository and applies it with `lazymkvctl settings set FILE` over SSH after Task 25 step 6. The file is never recorded, and the notes write any such path as `C:\Users\USER\...`. Waits: Task 25 step 7.
15. **If `windows-latest` is not elevated, or its session has no credential vault (design part 12.4).** Task 2's token test asserts elevation when `CI` is set, so the first CI run (Task 2 Step 6, the draft pull request) shows the runner's facts. If the runner's test processes are not elevated, that assertion becomes a printed fact on the runner, and the pipe, mutex, folder and native tests (Tasks 2, 3, 11, 12 and 23) skip there with `skipped: windows-latest is not elevated (design part 12.4)` and run only on the mini PC. If the `win::creds` round-trip tests fail on the runner with os error 1312 (`CredWriteW` with no vault), they skip on the runner with `skipped: no credential vault on windows-latest (design part 12.4)` and run in the desktop session (Task 25 step 4); `stored` gains that branch only after the owner takes this fallback. Recommended: take each fallback only if the run shows it is needed. Waits: Task 2 Step 6's CI run (elevation) and Task 13's first CI run (the vault).
16. **The TMDb token.** It is not entered in PR 3; it waits for PR 4's live lookups (design part 15 item 8). Recommended. Waits: nothing in PR 3.
17. **The eject over SSH (design part 12.4).** The owner inserts a disc in `D:` on request; `win::eject::tests::the_tray_opens_on_eject` ejects it over SSH, as experiment 7 did, and the disc stays out for Step 4. Recommended: yes. Waits: Task 25 step 2.
18. **Ctrl+C at a real console (optional).** The owner starts the console-run engine of Task 25 step 5 at an elevated desktop prompt with `D:` empty and stops it with Ctrl+C; nothing is installed or registered. Recommended: yes, if the owner is at the mini PC anyway. Waits: Task 25 step 5a.

No install on the mini PC or the Mac and no firewall rule is needed: the pipe refuses remote clients itself, WNet, Credential Manager, Job Objects and `MoveFileExW` are part of Windows, and SMB to the NAS is already in use. Any change the plan did not foresee goes back to the owner by name.

## File structure

| Path | Change | Responsibility |
|---|---|---|
| `Cargo.toml:10-19` | Modify | `windows-sys = "=0.61.2"` (or the approved version) in `[workspace.dependencies]` (Task 1) |
| `Cargo.lock` | Modify | `windows-sys`, `windows-link` (Task 1) |
| `crates/platform/Cargo.toml` | Modify | `[target.'cfg(windows)'.dependencies]` with the feature list (Task 1) |
| `crates/platform/src/lib.rs:1, :12-16, :37-45, :107-149, :179-197, :199-218, :220-256, :257-272` | Modify | Crate doc; the module list (`#[cfg(windows)] pub mod win;` Task 1, `pub mod security;` and `#[cfg(test)] mod testutil;` Task 2, `pub mod localtime;` Task 7, `pub mod shares;` Task 13, `pub mod eject;` Task 15, `pub mod drives;` Task 16); `DriveWatcher` doc (L1a, W8); `ShareConnector::access`; `IpcTransport::close` and its accept contract; `Host` additions; `native_guard`; the file moves; the inline `testutil` moved out |
| `crates/platform/src/security.rs` | Create | Pure: `TokenFacts`, `token_verdict`, the SDDL strings, `test_name` (Task 2); `connect_error` (Task 12) |
| `crates/platform/src/localtime.rs` | Create | Pure: `format_local_time` |
| `crates/platform/src/shares.rs` | Create | Pure: `share_of`, `volume_key`, `credential_target`, `ShareApi`, `Shares<A>` (reach, fallback, texts) |
| `crates/platform/src/drives.rs` | Create | Pure: `drive_id`, `ready_drives`, `DriveQueue` |
| `crates/platform/src/eject.rs` | Create | Pure: `drive_letter`, `eject_report` |
| `crates/platform/src/testutil.rs` | Create (from the `testutil` in `lib.rs`) | `temp_dir`, `ci_factor`, `skip`, `cue`, `Secrets` (tests only) |
| `crates/platform/src/win/mod.rs` | Create (Task 1), Modify (Tasks 2 to 6, 11 to 16, 23) | Module doc; each task that creates a `win/<name>.rs` adds `mod <name>;` and `pub use <name>::{<its sheet items>};` |
| `crates/platform/src/win/{token,mutex,folder,files,job,awake,host,pipe,client,creds,shares,eject,drives,console}.rs` | Create | The Windows implementations (`#[cfg(windows)]`) |
| `crates/platform/src/process.rs:1-4, :93-146` | Modify | Doc; helper modes `spawn-grandchild` and `print-pid-sleep` |
| `crates/platform/src/fakes.rs:200-262, :298-402, :416-478` | Modify | `FakeShares::{stall, set_access, set_free_error}`; `MemoryIpc::close`; `FakeHost` additions |
| `crates/core/src/ipc.rs:31-43` | Modify | `share_credential`, `SHARE_VALUE_SHAPE` |
| `crates/core/src/record.rs:104-130` | Modify | `TitleRecord::rip_started_ms` |
| `crates/tools/src/lookup.rs:49-108` | Modify | Absolute `PATH` folders only, the writable skip, `.exe` on a settings path; `TestHost` |
| `crates/tools/src/makemkv.rs:99-110, :627-635` | Modify | `list_drives` takes `limit`; `KeyHost` |
| `crates/clients/src/transport.rs` | Modify | `OfflineTransport`, `OFFLINE` |
| `crates/dev/src/forms.rs`, `crates/dev/src/lib.rs` | Create, Modify | `read_forms` moved from ctl, with `title` |
| `crates/ctl/src/lib.rs:11-17, :100-143`, `crates/ctl/src/main.rs`, `crates/ctl/src/engine_cmds.rs` | Modify | The `Env.connector` doc (Task 12); call `lazymkv_dev::forms` (Task 22); the Windows connector (Task 12); the Windows test over a test pipe in `engine_cmds.rs`'s `mod tests` (Task 12) |
| `crates/engine/src/lib.rs:25-110` | Modify | `EngineConfig` fields; `--data` optional through `data_arg`; `main_with` routes to `native::run_with` on Windows |
| `crates/engine/src/main.rs` | Modify | Doc (Task 23) |
| `crates/engine/src/native.rs` | Create | The Windows start-up sequence and `run_with` (`#[cfg(windows)]`) |
| `crates/engine/src/owner.rs:198-412, :615-628, :1370, :1520-1521, :1733-1760, :1959, :2109, :2212-2310, :2641-2715` | Modify | Stop API, `close`, drain, console writer; undelivered notifications; `feed`'s result; promises; the retry timer in `on_timer`; relay; secret shape; shutdown |
| `crates/engine/src/ipc_server.rs:27-48, :186-221, :364-376` | Modify | Accept-error rule; drain; `Clients::send_waiting`; constants of the relay |
| `crates/engine/src/relay.rs` | Create | The `command` relay (S35, design part 9.5) |
| `crates/engine/src/notify.rs:17-20, :67-79` | Modify | `deliver` returns `Result`; `NoDelivery`; undelivered lines to the owner |
| `crates/engine/src/space.rs:85-115, :193-245`, `selftest.rs:225-249, :345-417, :521-530` | Modify | Promises off the owner thread; root folders; reach in `root_check`; bounded checks; S81 re-check |
| `crates/engine/src/steps.rs:164-180, :569-646, :868-893, :916-991, :1035-1082, :1368-1430, :1459-1470, :1556-1690` | Modify | The hard-link half state `cfg(not(windows))` (Task 4); the job's listing (Task 19) and the fingerprint's temporary folder (Task 18); CheckSpace's job.log access line (Task 17); the rip step's `rip_started_ms` and `started` (Task 9); Decide asks `reach` (Task 18); Remux asks `reach` (Task 17); L7 (Task 18) and the hard-link branches (Task 4) |
| `crates/engine/src/file_phase.rs:282-310` | Modify | Library read errors |
| `crates/engine/src/testkit.rs:121-137, :340-360, :386-430, :433-492, :1029-1081` | Modify | `MemorySink` returns Ok (Task 21); the ctl stand-in and `spawns` (Task 22); the `EngineConfig` literal, the console buffer and `close` instead of the waker (Task 10); `TestGuard` over `native_guard()` (Task 5); `new_native`, `pipe_client`, `console_stop_event` (Task 23) |
| `crates/engine/tests/binary.rs` | Modify | The Windows sentence test deleted; the not-Unicode test split by OS; a Windows usage-error test (Task 23) |
| `crates/fake-tools/tests/{engine_command,engine_native,scripts}.rs` | Create | Relay on a stand-in (Task 22); the native whole job (`cfg(windows)`, Task 23); the task script's parse check (Task 24) |
| `crates/fake-tools/tests/{engine_ipc,engine_file_phase,engine_restart,engine_disc_phase,makemkv_driver}.rs` | Modify | Replaced refusal test; new engine-logic tests; `started`; `list_drives`'s new parameter at makemkv_driver.rs:140, :145, :450 (Task 19). `whole_job.rs` is unchanged |
| `corpus/scenarios/command-relay/` | Create | `lazymkvctl.json` and its line files |
| `scripts/engine-task.ps1` | Create | The desktop task (P5, design part 3.1) |
| `scripts/phase0/platform-ignored.ps1` | Create | The desktop-session ignored runs through `job.sh` |
| `.github/workflows/ci.yml:3-8`, `README.md:23-46`, `docs/phase2/engine.md` | Modify | Comment; crate list; the PR 3 part, count and notes |

## Interface sheet

The items one task produces for another, used exactly as written; each task's Interfaces block fixes its own crate-private items. "All OS" items compile on ubuntu, macos and windows; `win::` items only under `cfg(windows)`.

### core

```rust
// core::ipc (design parts 6.1, 9.4; Ruling W12)
pub const SHARE_VALUE_SHAPE: &str = "the value must be a user name and a password on two lines";
pub fn share_credential(value: &str) -> Result<(&str, &str), String>; // split at the first '\n', one '\r' before it
    // dropped; Err(SHARE_VALUE_SHAPE) when there is no '\n', a second '\n', or an empty half. The Err never holds the value.

// core::record (R23.7, Ruling W16)
pub struct TitleRecord { /* PR 2 fields unchanged */
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rip_started_ms: Option<u64> } // the last rip's start, Unix ms
```

### platform, all OS

```rust
// lib.rs: trait changes
pub trait IpcTransport: Send + Sync {
    fn accept(&self) -> io::Result<IpcStream>; // Err kinds: BrokenPipe only after close (the serve thread ends);
        // PermissionDenied = a refused client, text "process <pid>: <verdict>"; ConnectionAborted = a client gone before
        // its check; any other = one failed accept (logged, paused ACCEPT_PAUSE, retried). An OS error whose kind is
        // BrokenPipe before close is returned as ErrorKind::Other, so it never reads as closed (engine.md:581 (d)).
    fn close(&self);                           // a pending and every later accept returns BrokenPipe
}
pub trait ShareConnector: Send + Sync { /* reach, free_bytes, volume unchanged */
    fn access(&self, root: &str) -> Option<&'static str> { None } } // "local folder" | "the session's access" | "lazymkv's stored credential"
pub trait Host: Send + Sync { /* PR 2 methods unchanged */
    fn now_ms(&self) -> u64;                     // Unix milliseconds
    fn local_time(&self, unix_ms: u64) -> String; // "yyyy-MM-dd HH:mm:ss.fff", local time of that instant (R23.7)
    fn user_writable(&self, path: &Path) -> bool; // a non-elevated process of the user could write it (design part 3.6); true when unknown
}
pub fn native_guard() -> Arc<dyn ProcessGuard>;  // win::JobProcessGuard on Windows, StdProcessGuard elsewhere

// security.rs (design parts 3.3 to 3.5)
pub const SYSTEM_SID: &str = "S-1-5-18";
pub const ADMINISTRATORS_SID: &str = "S-1-5-32-544";
pub const HIGH_INTEGRITY_SID: &str = "S-1-16-12288";
pub const MUTEX_SDDL: &str = "O:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)";
pub fn pipe_sddl(owner_sid: &str) -> String;   // "D:P(A;;GRGW;;;<sid>)(A;;GRGW;;;SY)S:(ML;;NWNR;;;HI)"
pub fn folder_sddl(reader_sid: &str) -> String; // "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;<sid>)"
pub fn owner_allowed(owner_sid: &str) -> bool;  // ADMINISTRATORS_SID or SYSTEM_SID
#[derive(Clone, Debug, PartialEq, Eq)] pub enum ElevationType { Default, Full, Limited }
#[derive(Clone, Debug, PartialEq, Eq)] pub struct TokenFacts { pub user_sid: String, pub elevated: bool,
    pub elevation_type: ElevationType, pub integrity_sid: String, pub linked: bool }
pub fn token_verdict(peer: &TokenFacts, own_user_sid: &str) -> Result<(), &'static str>;
    // Err("its user differs from this process's user") checked first, then Err("its token is not elevated")
pub const PIPE_NAME: &str = r"\\.\pipe\lazymkv";
pub const MUTEX_NAME: &str = r"Global\lazymkv-engine";
pub fn test_name(tag: &str) -> String; // "lazymkv-test-<tag>-<pid>-<counter>", the counter a process-wide AtomicU32;
    // all OS, for the tests of every crate (Global Constraints)
pub(crate) fn connect_error(raw_os_error: i32, own_elevated: bool) -> ConnectError; // the pipe client's open error
    // (W11): 2 NotRunning; 5 NotElevated when !own_elevated, else Refused; every other code Refused (231 after the one
    // wait among them)

// localtime.rs (design part 4.6, R23.7)
pub fn format_local_time(unix_ms: u64, offset_minutes: i32) -> String; // "yyyy-MM-dd HH:mm:ss.fff"; civil-from-days, no crate

// shares.rs (design part 6.1, P7, S11, V15)
pub fn share_of(root: &str) -> Option<String>;      // r"\\server\share" with '/' read as '\', case kept; None for a local
                                                    // root and for "\\?\" and "\\.\" paths
pub fn volume_key(root: &str) -> Option<String>;    // share_of lower-cased, or the upper-case "X:" of a drive-letter root
pub fn share_secret_name(share: &str) -> String;    // "share:" + share
pub const CREDENTIAL_PREFIX: &str = "lazymkv:";
pub fn credential_target(prefix: &str, name: &str) -> String; // prefix + name
pub const SESSION_CONFLICT: &str = "ERROR_SESSION_CREDENTIAL_CONFLICT";
pub(crate) trait ShareApi: Send + Sync {
    fn probe(&self, path: &str) -> Result<(), u32>;                              // the share root or the drive root answers
    fn connect(&self, share: &str, user: &str, password: &str) -> Result<(), u32>; // deviceless, no CONNECT_UPDATE_PROFILE
    fn free(&self, path: &str) -> Result<u64, u32>; }                           // bytes available to the caller
pub(crate) struct Shares<A: ShareApi> { /* api, secrets: Arc<dyn SecretStore>, refused: Mutex<BTreeMap<String, (u64, u32)>>,
    access: Mutex<BTreeMap<String, &'static str>> */ }
impl<A: ShareApi> ShareConnector for Shares<A>;
// Unreachable texts (Ruling W14), `<share>` the share as share_of gives it, `<error>` std::io::Error::from_raw_os_error(code):
//   "<share> does not answer: <error>"
//   "<share> refused this session's access (<error>), and this session can read no lazymkv credential for it"
//   "<share> refused this session's access (<error>), and Windows refused lazymkv's stored credential: <error>; it is not tried again until it changes"
//   "<share>: ERROR_SESSION_CREDENTIAL_CONFLICT: this session already reaches the server as another user, so lazymkv's stored credential was not used"
//   "<share> refused access again after connecting with lazymkv's stored credential: <error>"
//   "lazymkv's stored credential for <share> is not a user name and a password on two lines"
//   "<root> is neither a drive folder nor a share"
//   local root: "<X:\> does not answer: <error>"

// drives.rs (design parts 4.2, 5.4; L1, L1a; Ruling W8)
pub fn drive_id(letter: char) -> String;            // "D:" for 'd' or 'D'
pub(crate) fn ready_drives(mask: u32, kind: impl Fn(char) -> u32, answers: impl Fn(char) -> bool)
    -> BTreeMap<String, PathBuf>; // GetLogicalDrives' letters whose kind is DRIVE_CDROM (5) and whose volume answers
    // (GetVolumeInformationW), as drive_id -> "X:\" (W8)
pub struct DriveQueue;                              // the watcher's state and its undelivered events
impl DriveQueue { pub fn new(now: BTreeMap<String, PathBuf>) -> DriveQueue;
    pub fn observe(&mut self, now: BTreeMap<String, PathBuf>);  // queues Removed (by id) then Arrived (by id) for each change
    pub fn present(&mut self) -> Vec<(String, PathBuf)>;      // the current discs; drops every queued event
    pub fn pop(&mut self) -> Option<DriveEvent>; }
pub const DRIVE_POLL_EVERY: Duration = Duration::from_secs(2);

// eject.rs (design part 4.2; capture/src/eject.rs:34-106)
pub fn drive_letter(drive: &str) -> Result<char, String>; // "D:" only (ASCII upper-case letter, colon); Err "not a drive id: <drive>"
pub fn eject_report(steps: &[(&str, Result<(), u32>)]) -> String; // "lock=ok, dismount=error 5, allow-removal=ok, eject=ok"

// fakes.rs (feature "fakes")
impl FakeShares { pub fn stall(&self, root: &str) -> StallGate; pub fn set_access(&self, root: &str, access: &'static str);
    pub fn set_free_error(&self, root: &str, message: &str); } // free_bytes(root) is Err(io::Error::other(message)); reach unchanged
pub struct StallGate;               // reach and free_bytes of that root block until StallGate::release(&self) or drop
impl StallGate { pub fn release(&self); pub fn wait_for_caller(&self, timeout: Duration) -> bool; }
impl ShareConnector for FakeShares { fn access(&self, root: &str) -> Option<&'static str>; } // set_access's value
impl MemoryIpc { /* IpcTransport::close: accept returns BrokenPipe after close, which wakes a waiting accept within
    MEMORY_ACCEPT_POLL, and, as in PR 2, once every connector is dropped */ }
impl FakeHost { pub fn set_writable(&self, path: &Path); } // now_ms = now_unix * 1000; local_time = format_local_time(ms, 0)

// testutil.rs (#[cfg(test)] only; Global Constraints)
pub(crate) fn temp_dir(tag: &str) -> PathBuf;   // moved from lib.rs unchanged
pub(crate) fn ci_factor() -> u32;               // a copy of engine::testkit::ci_factor (R29.c2)
pub(crate) fn skip(reason: &str);               // a copy of lazymkv_dev::evidence::skip
pub(crate) fn cue(text: &str);                  // "CUE: <text>\n" to standard error with write_all, then flush
pub(crate) struct Secrets;                      // impl SecretStore over Mutex<BTreeMap<String, String>>, exact names;
    // Secrets::new(), Secrets::with(name, value) as MemorySecrets has
pub(crate) fn within<T: Send + 'static>(bound: Duration, work: impl FnOnce() -> T + Send + 'static) -> T; // Task 11
pub(crate) fn line(reader: &mut dyn BufRead) -> String; // Task 11; "" at the end of the stream
```

### platform::win (cfg(windows))

```rust
// lib.rs: `#[cfg(windows)] pub mod win;`; win/mod.rs: `mod <file>;` and `pub use <file>::{..};` for each public item below
pub(crate) fn wide(text: impl AsRef<OsStr>) -> Vec<u16>;           // NUL-terminated UTF-16 (Task 2)
pub(crate) fn token_of_process(pid: u32) -> io::Result<TokenFacts>; // the pipe's peer (Task 2; used by Tasks 11, 12)
pub(crate) struct Descriptor;  // from_sddl, as_ptr, attributes, dacl; Send + Sync; LocalFree on drop (Task 2)
pub(crate) const MAXIMUM_ALLOWED: u32 = 0x0200_0000;               // winnt.h (Task 7; SystemServices is not enabled)
pub(crate) const SE_GROUP_INTEGRITY: u32 = 0x20;                    // winnt.h (Task 7; used by Task 11's test)
pub fn own_token() -> io::Result<TokenFacts>;                       // the process token (design part 3.2)
pub struct EngineMutex;                                             // Drop releases and closes it (design part 3.8)
pub enum MutexRefusal { EngineRunning, Squatter { account: String }, Os(io::Error) }
impl EngineMutex { pub fn take(name: &str) -> Result<EngineMutex, MutexRefusal>; } // created from MUTEX_SDDL (design part 3.3)
pub fn protect_data_folder(path: &Path, reader_sid: &str) -> Result<(), String>;    // design part 3.5
    // Err "<path> is owned by <owner SID>, neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)"
pub struct JobProcessGuard;                                         // impl ProcessGuard (design part 4.2)
pub struct WinKeepAwake;                                            // impl KeepAwake; new() -> io::Result<Self>; holders(&self) -> usize
pub struct WinHost;                                                 // impl Host; new() -> WinHost
pub struct PipeServer;                                              // impl IpcTransport
impl PipeServer { pub fn bind(name: &str) -> io::Result<PipeServer>; // ErrorKind::AddrInUse "another process holds the pipe <name>"
    #[cfg(test)] pub(crate) fn bind_as(name: &str, own_user_sid: &str) -> io::Result<PipeServer>; } // checks clients against that SID
pub struct PipeConnector;                                           // impl IpcConnector
impl PipeConnector { pub fn new(name: &str) -> PipeConnector;
    #[cfg(test)] pub(crate) fn new_as(name: &str, own_user_sid: &str) -> PipeConnector; } // checks the server against that SID
pub struct CredentialStore;                                         // impl SecretStore
impl CredentialStore { pub fn new() -> CredentialStore; pub(crate) fn with_prefix(prefix: &str) -> CredentialStore;
    #[cfg(test)] pub(crate) fn remove(&self, name: &str) -> Result<(), String>; }
pub fn share_connector(secrets: Arc<dyn SecretStore>) -> Arc<dyn ShareConnector>; // Shares<Win32Shares>
pub struct WinEjector;                                              // impl Ejector
pub struct WinDrives;                                               // impl DriveWatcher; new() -> io::Result<Self>
pub fn on_console_stop(handler: Box<dyn Fn() + Send + Sync>) -> io::Result<()>; // Ctrl+C, Ctrl+Break, close, logoff, shutdown
pub fn program_data() -> Option<PathBuf>;                           // %ProgramData% from the environment
pub(crate) const PIPE_MODE: u32 = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS; // win::pipe
pub(crate) const PIPE_BUFFER: u32 = 65536;                            // win::pipe (K12)
pub(crate) fn pipe_stream(handle: OwnedHandle) -> io::Result<IpcStream>; // win::pipe; either end, overlapped (Task 11)
pub(crate) fn accept_error(code: u32) -> io::Error; pub(crate) fn not_closed(error: io::Error) -> io::Error; // win::pipe
pub(crate) const FLUSH_BOUND: Duration = Duration::from_secs(2); // win::pipe; added only on Task 11's fallback (W10)
```

Windows values the code uses (each from the design part named, numbers Windows' own):
- Pipe open mode `PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED`, plus `FILE_FLAG_FIRST_PIPE_INSTANCE` on the first; pipe mode `PIPE_MODE`, passed by every `CreateNamedPipeW` call; `PIPE_UNLIMITED_INSTANCES`; buffers 65536 each way (parts 3.3, 3.4; K12). The writer half's `flush` is a no-op: `FlushFileBuffers`, which on a pipe waits for the client to read, is never called per line (K12).
- Client open `GENERIC_READ | GENERIC_WRITE`, `OPEN_EXISTING`, `FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`; `ERROR_PIPE_BUSY` (231) waits once with `WaitNamedPipeW(name, 5000)`. Every open error goes through `security::connect_error` (W11).
- Accept errors (W10; engine.md:581 (d)): `ConnectNamedPipe`'s `ERROR_PIPE_CONNECTED` (535) is a connection; `ERROR_NO_DATA` (232), `ERROR_BROKEN_PIPE` (109) and `ERROR_PIPE_NOT_CONNECTED` (233) met before the token check, and a failed `GetNamedPipeClientProcessId`, are `io::Error::new(ErrorKind::ConnectionAborted, ..)`; any other error keeps its kind, except that kind `BrokenPipe` becomes `ErrorKind::Other`; `BrokenPipe` comes only after `close`. `pipe_stream`'s reader half returns `Ok(0)` on 109 (T8-e).
- Job Object `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; creation flags `CREATE_SUSPENDED`, plus `BELOW_NORMAL_PRIORITY_CLASS` when `below_normal`; tree kill `TerminateJobObject(job, 1)` (part 4.2).
- `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)` and `ES_CONTINUOUS` (part 4.2).
- `MoveFileExW(temp, path, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` after `File::sync_all` (`FlushFileBuffers`); `MoveFileExW(from, to, 0)` (parts 6.5, 7.4; S13).
- Credential `CRED_TYPE_GENERIC`, `CRED_PERSIST_LOCAL_MACHINE`, user name `lazymkv`, blob at most 2560 bytes; `ERROR_NOT_FOUND` (1168) is None (part 6.1; Ruling W13).
- Share fallback on `ERROR_ACCESS_DENIED` (5) and `ERROR_LOGON_FAILURE` (1326); `ERROR_SESSION_CREDENTIAL_CONFLICT` (1219) by name; `WNetAddConnection2W` with `RESOURCETYPE_DISK`, null local name, null provider, flags 0 (part 6.1).
- Eject `FSCTL_LOCK_VOLUME` 0x0009_0018, `FSCTL_DISMOUNT_VOLUME` 0x0009_0020, `IOCTL_STORAGE_MEDIA_REMOVAL` 0x002D_4804 with byte 0, `IOCTL_STORAGE_EJECT_MEDIA` 0x002D_4808, `DRIVE_CDROM` 5 (part 4.2).
- Writable test masks: a folder `FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE | WRITE_DAC | WRITE_OWNER`; a file `FILE_WRITE_DATA | FILE_APPEND_DATA | DELETE | WRITE_DAC | WRITE_OWNER` (part 3.6; Ruling W17).
- MakeMKV key `HKEY_CURRENT_USER\Software\MakeMKV`, value `app_Key`; debug log `%USERPROFILE%\MakeMKV_log.txt`; defaults `%ProgramFiles(x86)%\MakeMKV\makemkvcon64.exe`, `%ProgramFiles%\MKVToolNix\mkvmerge.exe` (parts 3.6, 7.2).

### tools

```rust
pub fn find_tool(tool: Tool, settings: &Settings, resources: Option<&Path>, host: &dyn Host,
    path_var: Option<&OsStr>) -> Option<ToolLocation>; // unchanged signature; W18's rules
pub fn list_drives(ctx: &ToolCtx, program: &Path, debug: bool, drive: &str, out: &Path,
    limit: Option<Duration>) -> io::Result<DriveListing>; // changed: a job passes None (S57), the checks Some(limits.header) (W20)
```

### clients

```rust
pub const OFFLINE: &str = "no live transport in this build; it arrives in pull request 4";
pub struct OfflineTransport;   // impl HttpTransport: every get is Err(TransportError::Unreachable(OFFLINE.to_string()))
```

### dev

```rust
// dev::forms (moved from crates/ctl/src/lib.rs:109-143, Ruling S87; W24)
pub struct Forms { pub job: Option<String>, pub jobs: bool, pub title: Option<String>, pub words: Vec<String> }
pub fn read_forms(name: &str, rest: &[String]) -> Result<Forms, String>; // ctl's texts and checks unchanged
```

### engine

```rust
pub struct EngineConfig { pub data: PathBuf, pub resources: Option<PathBuf>, pub limits_override: Option<Limits>,
    pub path_var: Option<OsString>,
    pub ctl: Option<PathBuf>,   // the relay's child (design part 9.5); None refuses `command` with COMMAND_NO_CTL
    pub console: Option<Box<dyn Write + Send>> } // engine.log lines also to this writer, passed to Store::new (design
                                // part 3.5); None: no mirror
pub trait NotifySink: Send { fn deliver(&mut self, notification: &Notification) -> Result<(), String>; }
    // Err: the notifier thread writes undelivered_line, "notification <event> for <job id or "no job"> was not
    // delivered: <reason>", to engine.log through the Store (spec 12)
pub const NO_DELIVERY: &str = "no notification delivery in this build; it arrives in pull request 4";
pub struct NoDelivery;           // impl NotifySink: Err(NO_DELIVERY)
impl EngineHandle {
    pub fn shutdown_handle(&self) -> ShutdownHandle;
    pub fn wait_for_exit(&self, timeout: Option<Duration>) -> bool; // true once the owner left its loop
    pub fn stop(self) -> io::Result<()>; } // closes the transport, drains each client up to CLIENT_DRAIN, joins
    // set_ipc_waker is removed (engine.md:581 (a))
#[derive(Clone)] pub struct ShutdownHandle; impl ShutdownHandle { pub fn shutdown(&self, force: bool); } // as a client-less shutdown request
pub(crate) const CLIENT_DRAIN: Duration = Duration::from_secs(2);
pub(crate) const ACCEPT_PAUSE: Duration = Duration::from_millis(100);
pub const USAGE: &str = "usage: lazymkv-engine [--data DIR]";
pub(crate) fn data_arg(args: &[String]) -> Result<Option<String>, String>; // None without --data (W1); Err the usage message
pub fn main_with(args: &[String], err: &mut dyn Write) -> i32; // non-Windows: S28 sentence, 1;
    // Windows: native::run_with(args, err, NativeOptions::production())
// engine::native (cfg(windows); design parts 3.1 to 3.8)
pub struct NativeOptions { pub mutex: String, pub pipe: String, pub drives: Option<Box<dyn DriveWatcher>>,
    pub ejector: Option<Arc<dyn Ejector>>, pub processes: Option<Arc<dyn ProcessGuard>>, // None: native_guard() (T3-23-a)
    pub secrets: Option<Arc<dyn SecretStore>>, pub http: Option<Arc<dyn HttpTransport>>,
    pub console: Option<Box<dyn Write + Send>>,  // None: standard error (W26)
    pub console_stop: Option<Box<dyn FnOnce(Box<dyn Fn() + Send + Sync>) -> io::Result<()> + Send>>, // None: win::on_console_stop
    pub token: Option<TokenFacts> }              // None: win::own_token() (design part 3.2)
impl NativeOptions { pub fn production() -> NativeOptions; } // MUTEX_NAME, PIPE_NAME, every override None
pub fn run_with(args: &[String], err: &mut dyn Write, options: NativeOptions) -> i32;
// Start-up texts (exit 1 each; Ruling W3, W4):
pub const NOT_ELEVATED: &str = "lazymkv-engine: run lazymkv-engine elevated (design part 3.2)";
pub const ENGINE_RUNNING: &str = "lazymkv-engine: an engine is already running (design part 3.3)";
pub const STOPPED_STARTING: &str = "lazymkv-engine: stopped by the console while starting (design part 3.8)";
//   squatter: "lazymkv-engine: <mutex name> is held by <account>, which is not an engine (design part 3.3)"
//   mutex error: "lazymkv-engine: <mutex name> could not be taken: <error>"
//   no ProgramData: "lazymkv-engine: %ProgramData% is not set; give --data DIR"
//   folder: "lazymkv-engine: " + protect_data_folder's Err
//   pipe: "lazymkv-engine: another process holds the pipe <pipe name> (design part 3.3)"
//   start: "lazymkv-engine: the engine could not start: <error>"
// engine.log lines (Ruling W4, W10): "refused a pipe client: process <pid>: <verdict>";
//   "the pipe could not accept a client: <error>"
// job.log line (Task 17): "share access for <root>: <access>"
// The relay (design part 9.5; Ruling W24, W25)
pub(crate) const COMMAND_BUSY: &str = "a command is already running; one runs at a time (design part 9.5)";
pub(crate) const COMMAND_NO_CTL: &str = "command needs lazymkvctl beside the engine, and this engine has none";
pub(crate) const RETUNE_WAITS: &str = "retune --jobs needs the evidence trees of pull request 5 (design part 9.5)";
pub(crate) const NO_EXIT_CODE: &str = "lazymkvctl ended without an exit code";
// a command a stop ends is answered with ipc_server's existing STOPPING (Ruling T3-22-d)
pub(crate) const COMMAND_FORMS: &str = "command needs plan --job, decide --job --title or retune --jobs (design part 9.5)";
pub(crate) const COMMAND_WRITABLE: &str =
    "lazymkvctl's folder can be written by a non-elevated process, so the engine will not run it (design part 3.6)";
pub(crate) const COMMAND_HEADROOM: usize = 64;
impl Clients { pub(crate) fn send_waiting(&self, client: u64, line: String) -> bool; } // command_output only: waits,
    // outside the lock, until that client's queue holds fewer than CLIENT_QUEUE - COMMAND_HEADROOM (192) lines; false
    // once the client is gone (S52 amended, W10; Ruling T3-22-a)
//   unknown job: no_job(id) = "no job <id>" (S76); unknown title: "job <id> has no title <key>"
// testkit
impl Harness { pub fn spawns(&self) -> Vec<Spawn>;   // every Spawn TestGuard saw, in order (Task 22)
    pub fn console_text(&self) -> String;            // the console buffer EngineConfig.console writes to (Task 10)
    pub fn shutdown_handle(&self) -> ShutdownHandle; pub fn wait_for_exit(&self, timeout: Duration) -> bool; } // Task 10
#[cfg(windows)] impl Harness {
    pub fn new_native(fake_tool: &Path, scenarios: &[&str], settings: Value, pipe: &str) -> Harness;
        // stand-ins, scenarios and settings as `new` builds them, then native::run_with(["--data", <harness data>],
        // <buffer>, NativeOptions { mutex: Global\ and the pipe's name after \\.\pipe\, pipe, drives: SharedDrives,
        // ejector: RecordingEjector with removals, processes: its TestGuard, secrets: MemorySecrets, http: the recorded
        // transport, console: the harness's console buffer, console_stop: a closure that captures the handler, token:
        // None }) on a thread; the host is the real WinHost. Its drop sends a forced `shutdown` through the pipe, joins
        // the thread and asserts 0; `restart` panics (Ruling T3-23-c).
    pub fn pipe_client(&self) -> PipeConnector;
    pub fn console_stop_event(&self);    // calls the captured console handler, as Ctrl+C would (Task 23)
    pub fn native_exit(&self) -> i32; }  // waits for run_with to return and gives its exit code (Task 23)
```

### ctl

`crates/ctl/src/main.rs` passes `connector: Some(&PipeConnector::new(PIPE_NAME))` under `cfg(windows)` and `None` elsewhere. The constants `NO_TRANSPORT`, `NOT_RUNNING`, `REFUSED`, `NOT_ELEVATED` and `CLOSED` are unchanged. The pipe client maps `ERROR_FILE_NOT_FOUND` (2) to `NotRunning`; `ERROR_ACCESS_DENIED` (5) to `NotElevated` when its own token is not elevated, else `Refused`; a pipe still busy after its one wait, a failed server check and any other error to `Refused` (Ruling W11), through `security::connect_error`.

### Files, messages and scripts

- `rip-summary.json` gains `"started": "<yyyy-MM-dd HH:mm:ss.fff>"` between `bytes` and `elapsed_s` (design part 7.1, R23.7).
- Wire, unchanged shapes: `command` → `command_output` pushes with the command's `id`, sent only to the requesting client (W10, design part 9.2 amended), then `ok` `{"exit_code": <i32>}` (design parts 9.1, 9.3); `secret_set` → `ok` `null`, or `<name> could not be stored: <reason>`.
- `scripts/engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]`: task `lazymkv-engine`; run folder `C:\lazymkv\run`; copies from `C:\lazymkv\target\release` `lazymkv-engine.exe`, `lazymkvctl.exe`, and `lazymkv-ocr.exe` when it exists; waits 60 s, polling `lazymkvctl status` every 2 s; prints `ENGINE-TASK-DONE status=ok|error`.
- `scripts/phase0/platform-ignored.ps1 -Filter <filter> [-Drive <id>] [-ShareRoot <UNC>]`: sets `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"` and `CARGO_TARGET_DIR=C:\lazymkv\target` as `box.sh` does, sets `LAZYMKV_TEST_DRIVE` and `LAZYMKV_TEST_SHARE_ROOT` from its parameters and `LAZYMKV_TEST_VAULT` to `1` for a `win::creds::` filter only (Owner decision 8), and runs `cmd /c "cargo test -p lazymkv-platform --locked -- --include-ignored --test-threads=1 --nocapture <filter> 2>&1"` in `C:\lazymkv\src`, so each `CUE:` line reaches the log when it is written and PowerShell 5.1 never wraps the native standard error as error records. `--include-ignored` also runs the selected module's tests that are not ignored, such as `win::creds`' round trip. `job.sh` passes the parameters unquoted inside `run-interactive.ps1`'s `& '<script>' <ArgLine>`, so PowerShell binds them by name and no value may hold a space or a quote.

## Rulings on the readers' open questions

The planner's rulings of 2026-10-27 where the design decides or allows a builder's call. The owner may overturn any (Owner decisions item 11 lists those that change an earlier ruling).

- **W1 (`--data`).** Optional, as design part 3.1 says: `usage: lazymkv-engine [--data DIR]`; on Windows the default is `%ProgramData%\lazymkv`, read from the `ProgramData` environment variable. A missing value, or words left over, stay usage errors with exit 2. Outside Windows the arguments are read the same way and the S28 sentence follows.
- **W2 (exit codes).** Every start refusal exits 1 (S28's precedent), a usage error 2, a clean stop 0.
- **W3 (start order).** Arguments, then the elevation check, then the mutex, then the data folder, then the first pipe instance, then `start`. The elevation check only reads the process token, so the mutex still comes before anything with an effect (design part 3.3); a non-elevated engine thus says it is not elevated rather than meeting the mutex's access error.
- **W4 (names).** A squatter's account is named on standard error only (design part 3.3). `engine.log` names no account: a refused client is its process id and the verdict, a foreign data-folder owner its SID.
- **W5 (the mutex owner).** The engine sets `O:BA` itself (`MUTEX_SDDL`), so "owned by Administrators" does not depend on the token's default owner. An owner that cannot be read is reported as a squatter named `an account lazymkv cannot read`.
- **W6 (the data folder).** "Read" for the owner's account is `FR`. The engine sets owner `BA` on a folder it creates. The list is set at every start with `SetNamedSecurityInfoW` and propagates below; its cost grows with the jobs tree, which phase 2's job count keeps small.
- **W7 (the Job Object runner).** `std::process::Command` with `CREATE_SUSPENDED` (and `BELOW_NORMAL_PRIORITY_CLASS`), `AssignProcessToJobObject`, then `ResumeThread` on the child's one thread, found through Toolhelp (T3-5-b); std's spawn keeps its handle discipline. One job per child. T9-a to T9-e hold unchanged. On Windows the testkit's `TestGuard` wraps `native_guard()`, so every Windows whole fake job runs the Job Object runner; the driver tests in `crates/fake-tools/tests/` keep `StdProcessGuard`.
- **W8 (the drive watcher's window).** `WinDrives` owns a thread that creates a hidden top-level window (message-only windows do not receive volume broadcasts), registers for `GUID_DEVINTERFACE_CDROM` notifications, re-scans on every `WM_DEVICECHANGE` and every `DRIVE_POLL_EVERY` (2 s), and feeds a `DriveQueue`; the engine's drive thread drains it through `next`. This departs from design part 4.4's "the drive thread runs the window's message loop" only in which thread owns the loop. A scan is `GetLogicalDrives`, `GetDriveTypeW == DRIVE_CDROM` and a successful `GetVolumeInformationW`; the debounce is the state diff (one `Arrived` per absent-to-present change) plus the engine's `drive_settle_s`. `new` scans once before returning, so `present()` is the state at start and `next()` never reports those discs again (L1a). No handle notification is registered, so nothing needs closing before an eject (design part 5.6 step 8). The drive id is the letter (`D:`); `DriveEvent` keeps no separate letter. A disc whose volume does not answer is therefore not in `present()`: it arrives through `next()` once its volume answers and gets `drive_settle_s` and the readiness wait like any arrival, while a disc `present()` lists is read at once (design part 5.4). This closes engine.md:582's second half. The scan's filter is the pure `drives::ready_drives`.
- **W9 (KeepAwake).** `WinKeepAwake` owns a thread; `acquire` and `release` send to it, and it calls `SetThreadExecutionState` when the count leaves or reaches zero, so the state never depends on the caller's thread. KeepAwake held through waits after a file-phase slot grant (engine.md:630) is confirmed by spec 4.3's "while any job is active".
- **W10 (the pipe server).** The engine's transport hands a connection out only after creating the next instance (the name is never unheld) and after the token check. `accept` returns `BrokenPipe` only after `close`; `ERROR_PIPE_CONNECTED` (535) is a connection; a client gone before its check (232, 109 or 233 before the check, or a failed `GetNamedPipeClientProcessId`) is `ConnectionAborted`; a refused one `PermissionDenied`; any other OS error keeps its kind, and kind `BrokenPipe` becomes `Other`, so a per-client error never ends the serve thread (engine.md:581 (d)). `close` sets a stop event the accept waits on. The writer half's `flush` is a no-op, so a silent client never blocks a push (K12). The server never calls `DisconnectNamedPipe`: dropping the writer cancels the reader's pending read (which then reads end of file), and the handle closes when both halves are gone, so bytes already written stay readable by the client (Review Focus 2 verifies this; if Windows discards them, the writer's drop first runs `FlushFileBuffers` on a helper thread bounded by `win::pipe`'s `FLUSH_BOUND`, 2 s). `command_output` goes only to the requesting client (design part 9.2 amended: every `lazymkvctl` run uses `id` 1, so a broadcast would print one client's output in another's wait), and the relay waits for room in that client's queue through `Clients::send_waiting` instead of dropping it (S52 amended for `command_output` only). Owner decision 11 lists both amendments.
- **W11 (the pipe client).** `ConnectError` keeps its four variants; a server that fails the check is `Refused` (design part 9.6 names three exit 4 messages). The client opens with `SECURITY_IDENTIFICATION`, so a squatting server cannot impersonate it.
- **W12 (the share credential's value).** Two lines, both non-empty, split by `core::ipc::share_credential`. `secret_set` refuses another shape with `<name> could not be stored: the value must be a user name and a password on two lines`.
- **W13 (Credential Manager).** Every secret is a generic credential, target `lazymkv:` + the secret name (so `lazymkv:share:\\server\share` as design part 6.1 fixes, and `lazymkv:tmdb_token`), user name `lazymkv`, the value's UTF-8 bytes as the blob, `CRED_PERSIST_LOCAL_MACHINE`. `get` is None on any error, so over SSH a stored secret reads as unset (design part 6.1 says the vault is written from the desktop session); `set`'s error is `Credential Manager refused the entry: <error>`. S77 stays: no listing.
- **W14 (the share connector).** The first attempt is a probe of the share root `\\server\share\` (or the drive root `X:\`), not of the configured folder, so a missing `engine` folder is not an unreachable share; `free_bytes` reads `GetDiskFreeSpaceExW`'s bytes available to the caller on the same path. The stored secret for a root is looked up under `share:` + `share_of(root)` as written, and Windows' lookup ignores case (the all-OS tests store it in the root's own case, since their stores compare exactly; `win::creds` tests the case rule). A credential Windows refused is remembered with a hash of its value and not tried again until the value changes (Review Focus 5). `access` reports how the last `reach` of that share succeeded.
- **W15 (engine folders).** Before CheckSpace's first write and in the self-test, the engine creates a missing `staging_root` or `tuning_root` with `fs::create_dir` when its parent exists, never deeper, and is never given a library root. The self-test's `root_check` asks `reach` before its probe (spec 13: "each root reachable").
- **W16 (`started`).** The rip step records `host.now_ms()` as `TitleRecord::rip_started_ms` in its posted record edits before `tools::rip` runs; VerifyFile writes `started` from it with `host.local_time`, so a restart between the rip and VerifyFile loses nothing (R23.4). A record without the field writes no `started`.
- **W17 (the writable test).** `WinHost::user_writable` runs `AccessCheck` with the filtered token: the linked token when the process token's elevation type is Full, else `CreateRestrictedToken(LUA_TOKEN)` at medium integrity. Any failure answers true.
- **W18 (the tool lookup).** `find_tool` drops relative and empty `PATH` entries, skips a `PATH` folder or program file `host.user_writable` calls writable and a writable `resources` folder, and on a settings path that is not a file tries the path with `EXE_SUFFIX` appended. The default install folders come from `%ProgramFiles(x86)%` and `%ProgramFiles%`, not the registry.
- **W19 (the start check and Ctrl+C).** The console handler is installed before `start`; a stop event during `start` ends the process at once, and the Job Objects end its listing. After `start` returns it sends a forced shutdown.
- **W20 (bounded listings, amends S57 for checks).** The MakeMKV check at start and the self-test's listing pass `Some(limits.header)` (120 s); a listing killed by that limit reports `makemkvcon did not finish the drive listing within the header limit and was killed`, and one killed by the exit wait reports `EXIT_WAIT_PASSED` (M2), both not ok. A job's listing keeps no limit.
- **W21 (AwaitingShare, amends S81).** The re-check asks `reach`, then `free_bytes`, of the same roots, and for `staging_root` `space::root_ready`; a free-space error, or a staging root that is not a folder and whose parent is not one, keeps the job waiting, so a retry never cycles through ShareBack (engine.md:589).
- **W22 (library reads, R-B1).** `library_facts` reads `NotFound` as absent and turns any other error into DecisionFailed { Unknown } with `the library folder <path> could not be read: <error>`; error-free inputs are unchanged, so decide's output is unchanged. Decide asks `reach` of the library root first, as File does (V7).
- **W23 (unpersisted transitions, V1, RR1-16).** `feed` returns `Fed::{Rejected, Persisted, NotPersisted}`. `step_done` stops feeding a report's later events after `NotPersisted`. An event whose transition was not persisted from a state with no failure row (Planned, RipVerified, an Ejected job's FilePhaseSlot) is kept and fed again on a one-minute timer until it persists.
- **W24 (the relay).** The engine reads `words` with `lazymkv_dev::forms::read_forms`, the parser ctl uses. Built words come first and the client's remaining words after, so `Args`' last-wins rule lets an explicit flag win. The child runs `EngineConfig.ctl` with `Spawn { env: vec![], below_normal: true }` in the engine's working folder. A disconnected client kills its child; a `shutdown` kills it and replies `STOPPING`; no exit code is `NO_EXIT_CODE`. All are error replies, so ctl exits 2. The relay runs on a thread of its own and reports its end to the owner (design part 4.4). Words `read_forms` accepts without `--job` or `--jobs` are refused with `COMMAND_FORMS`. The child's run is not recorded capture-style (design part 7.2 names four tools). The relay refuses with `COMMAND_WRITABLE` when `host.user_writable(<lazymkvctl's folder>)`, as design part 3.6 skips a writable `resources` folder beside the engine; the task's `C:\lazymkv\run` is protected by the script, and a console run from a writable folder gets the refusal.
- **W25 (what PR 3 relays).** `plan --job` and `decide --job --title`; `--text` is never added, since no dataset copy exists before PR 5; `retune --jobs` is refused with `RETUNE_WAITS`. The stale-copy refetch and K15's exit 3 stay PR 4's: the child reads the copies the job folder holds.
- **W26 (the binary's services).** `http` is `OfflineTransport` (lookups stay pending), `notify` is `NoDelivery` (each notification logged undelivered), `timers` `StdTimers`, `steps` None, `ctl` the folder of `current_exe()` joined with `lazymkvctl` and `EXE_SUFFIX`, `resources` `<exe folder>\..\resources`, `path_var` the process `PATH`, `console` `Some(Box::new(std::io::stderr()))`.
- **W27 (PR 3's running whole job).** The CI `whole_job` on three systems, unchanged, plus Task 23's native whole job on `windows-latest` and the mini PC, which runs the real start-up sequence, the real pipe and `lazymkvctl`'s real client, the Job Object runner, the Windows file moves, the real `Host`, `KeepAwake` and a local-folder `ShareConnector`, with scripted drives, a `RecordingEjector` with removals, a recorded transport and memory secrets. S55 stops every stand-in `makemkvcon` call while a MakeMKV process runs, and the MakeMKV interface may be open on the mini PC, so that test first asks `WinHost::new().running_processes()` and, when one of S55's names is listed, prints `skipped: a MakeMKV process is running on this machine (S55)` and passes; with `CI` set it never skips.
- **W28 (L7's test, design part 6.5).** After `create_dir_all` of the final path's folder returns Ok or `AlreadyExists`, File opens that folder with `fs::read_dir` before the rename; an error there gives L7's message and FileFailed { Unknown }, and the rename runs only after the open succeeds. The folder opened is the title's own, never the library root, so LIB-7 stands.

## Tasks

| Task | Deliverable | Needs the owner |
|---|---|---|
| 1 | `windows-sys` and the lock delta | yes on the delta (decision 1) |
| 2 | Token facts and the engine mutex | decision 12 (non-blocking); the owner's yes to open the draft (Step 6), then decision 15 |
| 3 | The protected data folder | no |
| 4 | The file moves on `MoveFileExW` | no |
| 5 | The Job Object runner | no |
| 6 | `KeepAwake` | no |
| 7 | `Host` additions and `WinHost` | decision 11 (W17, non-blocking) |
| 8 | The tool lookup rules | no |
| 9 | `rip-summary.json`'s `started` | no |
| 10 | Transport close, the accept rule, the stop API | decision 11 (W10, non-blocking) |
| 11 | The named pipe server | no |
| 12 | `lazymkvctl`'s pipe client | no |
| 13 | Secrets on Credential Manager | decision 8 (the persistence; the desktop run in 25) |
| 14 | The share connector | no (real share in 25) |
| 15 | The `Ejector` | no (hardware in 25, decision 17) |
| 16 | The `DriveWatcher` | decision 11 (W8, non-blocking; hardware in 25) |
| 17 | Share work off the owner thread, share checks | decision 11 (W21, non-blocking) |
| 18 | Library reads, L7, the fingerprint's temporary folder | decision 11 (W22, non-blocking) |
| 19 | Bounded MakeMKV checks | decision 11 (W20, non-blocking) |
| 20 | Unpersisted transitions | no |
| 21 | Undelivered notifications, the offline transport | no |
| 22 | The `command` relay | decision 11 (W10, W24, W25, non-blocking) |
| 23 | The engine binary on Windows, the native whole job | decisions 11 (W3) and 12, non-blocking |
| 24 | `scripts/engine-task.ps1` | decisions 4, 5, 13; 11 (T3-24-a, non-blocking) |
| 25 | The mini PC runs | decisions 2 to 10, 13, 14, 17 and 18 |
| 26 | Documents, notes and the count | no |

### Task 1: `windows-sys` and the `Cargo.lock` delta

**Files:**
- Modify: `Cargo.toml:19` (a `windows-sys` line after `unicode-normalization`, in `[workspace.dependencies]`).
- Modify: `crates/platform/Cargo.toml:13` (a `[target.'cfg(windows)'.dependencies]` table after `[dependencies]`).
- Modify: `Cargo.lock` (edited by hand on the Mac to the approved delta; Step 4's locked build checks it).
- Create: `crates/platform/src/win/mod.rs` (the module doc only).
- Modify: `crates/platform/src/lib.rs:12-13` (the `win` module, after `pub mod fakes;`).
- Tests: none. The checks are the crates.io read, the lock diff and the locked build.

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes: nothing.
- Produces: `windows_sys` inside `lazymkv-platform`, on Windows only, with exactly the Tech Stack's 20 features. No other crate depends on it, and no later task edits a `Cargo.toml` dependency list.

```rust
// crates/platform/src/lib.rs, after `pub mod fakes;`
/// The Windows implementations of the platform traits and helpers; every `windows-sys` call of the workspace sits
/// here (design parts 4.2, 4.3). Compiled only on Windows. Def: T3-1. Use: T3-2 to T3-16, T3-23.
#[cfg(windows)]
pub mod win;
```

Ruling settled in this task:
- **Ruling T3-1-a:** reading crates.io's application programming interface (API) for a version's manifest licence, publication date, checksum, features and dependencies downloads no crate and changes nothing on the Mac or the mini PC, so Step 1 runs before the owner's yes. The first command that downloads anything is Step 4's build, which is `--locked` against the hand-written approved entries, so it can fetch nothing else (PF-11, ENG-7).

- [ ] **Step 1: Read the delta from crates.io, fetching nothing**

On the Mac, read each of these with `curl -sf -A 'lazymkv-lock-check' https://crates.io/api/v1/crates/<path>`:
- `windows-sys`: `crate.max_stable_version`. A line newer than 0.61 is reported beside the delta; the pin stays the version the owner approves.
- `windows-sys/0.61.2`: `version.license`, `version.created_at`, `version.yanked`, `version.checksum`, and the keys of `version.features`, which must hold all 20 names of the Tech Stack.
- `windows-sys/0.61.2/dependencies`: the entries of kind `normal` or `build`. Expected: exactly one, `windows-link`, with its `req`.
- `windows-link/versions`: the newest version that is not yanked and meets that `req`, which is the version cargo resolves, with its `license`, `created_at` and `checksum`. Then `windows-link/<that version>/dependencies`, expected to hold no entry of kind `normal` or `build`.

Expected: `windows-sys` 0.61.2 and `windows-link` 0.2.1, both `MIT OR Apache-2.0`, neither yanked, each `created_at` at least 14 days before the read, and all 20 features present.

Write the delta to `.superpowers/pr3/pr3-lock-delta.txt` in the working tree (git-ignored as plan-execution scratch, so `scripts/box.sh` never copies it and it outlives the session; every evidence file of this plan goes in that folder), in this form, with the values read:

```
Pull request 3's Cargo.lock delta (design part 4.7, P33, ENG-7), read from crates.io on <yyyy-MM-dd>
added   windows-sys  0.61.2  MIT OR Apache-2.0  published <yyyy-MM-dd>  direct: lazymkv-platform, Windows only, "=0.61.2"
added   windows-link 0.2.1   MIT OR Apache-2.0  published <yyyy-MM-dd>  through windows-sys (req "<req>")
changed lazymkv-platform: its dependencies gain "windows-sys"
packages: 43 -> 45
windows-sys 0.61.2 declares all 20 features of the plan's Tech Stack
newest windows-sys on crates.io: <max_stable_version>
```

A version, licence, dependency or feature that differs from the expected values goes into the file with the words `DIFFERS FROM THE PLAN`. Then stop: nothing is edited, committed or fetched.

- [ ] **Step 2: Stop for the owner's yes (Owner decision 1)**

The controller shows the owner the file with this sentence: "Pull request 3's whole `Cargo.lock` delta: `windows-sys` 0.61.2 and `windows-link` 0.2.1, both MIT OR Apache-2.0, 43 to 45 packages (design part 4.7, P33)." It waits for an explicit yes in chat and records it as a line of `.superpowers/pr3/decisions.txt` (`<decision number>, <yyyy-MM-dd>, <answer>`; every owner answer of this plan goes there, for Task 26). A different version in the yes replaces 0.61.2 in Step 3 and in Step 4's expected diff. Task 2 does not start before the yes.

- [ ] **Step 3: Write the manifests and the module**

`Cargo.toml`, after line 19:

```toml
# The Windows API declarations, used by lazymkv-platform only and pinned exactly (design parts 4.3, 4.5, 4.7; P33).
windows-sys = "=0.61.2"
```

`crates/platform/Cargo.toml`, after line 13:

```toml

[target.'cfg(windows)'.dependencies]
# Every Win32 call of platform::win (design parts 4.2, 4.3); the features are the API areas design part 4.3 names.
windows-sys = { workspace = true, features = [
    "Win32_Foundation",
    "Win32_NetworkManagement_WNet",
    "Win32_Security",
    "Win32_Security_Authorization",
    "Win32_Security_Credentials",
    "Win32_Storage_FileSystem",
    "Win32_System_Console",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_Diagnostics_ToolHelp",
    "Win32_System_IO",
    "Win32_System_Ioctl",
    "Win32_System_JobObjects",
    "Win32_System_LibraryLoader",
    "Win32_System_Pipes",
    "Win32_System_Power",
    "Win32_System_Registry",
    "Win32_System_SystemInformation",
    "Win32_System_Threading",
    "Win32_System_Time",
    "Win32_UI_WindowsAndMessaging",
] }
```

`crates/platform/src/win/mod.rs`:

```rust
//! The Windows implementations of lazymkv-platform's traits and helpers (design parts 4.2, 4.3). Every `windows-sys`
//! call of the workspace sits in this module, and each `unsafe` block carries a `// SAFETY:` line saying why the call
//! is sound. The rules these implementations apply (descriptor strings, the token verdict, the share and volume rules,
//! the drive diff, the local-time text, the eject report) sit in modules that compile and are tested on every system.
//! Compiled only on Windows.
```

`crates/platform/src/lib.rs`: the `win` declaration of Interfaces, after line 13 (`pub mod fakes;`).

- [ ] **Step 4: Write the approved lock entries, then fetch only them**

On the Mac, edit `Cargo.lock` by hand: add the line ` "windows-sys",` after ` "lazymkv-core",` in `lazymkv-platform`'s `dependencies`, and add the two blocks below at their alphabetical places (after the last package whose name sorts before `windows-link`), each `checksum` the value Step 1 read:

```toml
[[package]]
name = "windows-link"
version = "0.2.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "<windows-link 0.2.1's checksum from Step 1>"

[[package]]
name = "windows-sys"
version = "0.61.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "<windows-sys 0.61.2's checksum from Step 1>"
dependencies = [
 "windows-link",
]
```

Run: `scripts/box.sh cargo build --workspace --locked`
Expected: at most an `Updating crates.io index` line (index metadata, no crate), exactly two `Downloaded` lines, `windows-link v0.2.1` and `windows-sys v0.61.2`, then `Finished`. Then on the Mac: `git diff Cargo.lock` is the hand edit and nothing else (`scripts/box.sh` copies the lock back), and `grep -c '^\[\[package\]\]' Cargo.lock` prints 45.

An error `the lock file needs to be updated but --locked was passed` means the edit or the manifests differ from the approved delta: nothing was downloaded. Fix the edit; never drop `--locked`. A delta that cannot be written as approved goes back to the owner, and nothing is committed.

- [ ] **Step 5: Build and test with the locked file**

Run: `scripts/box.sh cargo build --workspace --locked`
Expected: `Finished`, with no `Downloaded` line.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS, with no test added (pull request 2's count, 2573 on the mini PC, is unchanged).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/platform/Cargo.toml crates/platform/src/lib.rs crates/platform/src/win/mod.rs
git commit -m "platform: windows-sys, pinned, on Windows only (design part 4.7, P33)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, then the message alone, with no attribution line.

---

### Task 2: Token facts and the engine mutex (design parts 3.2, 3.3)

**Files:**
- Create: `crates/platform/src/security.rs` (all systems; 8 tests in its `mod tests`).
- Create: `crates/platform/src/testutil.rs` (`cfg(test)`; `temp_dir` moved from `lib.rs:257-271`, and `ci_factor`, `skip`, `cue`, `Secrets`; 2 tests).
- Create: `crates/platform/src/win/token.rs` (2 tests) and `crates/platform/src/win/mutex.rs` (6 tests), compiled only under `cfg(windows)` through Task 1's `#[cfg(windows)] pub mod win;`.
- Modify: `crates/platform/src/lib.rs:15-16` (after `pub use process::StdProcessGuard;`: `pub mod security;` and `pub use security::{MUTEX_NAME, PIPE_NAME};`) and `:257-271` (the inline `testutil` module replaced by a file module).
- Modify: `crates/platform/src/win/mod.rs` (`mod token; mod mutex;`, the re-exports below, and `wide`).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes: Task 1's `windows_sys` (features `Win32_Foundation`, `Win32_Security`, `Win32_Security_Authorization`, `Win32_System_Threading`); `SecretStore` (`crates/platform/src/lib.rs:127-133`).
- Produces (`crates/platform/src/security.rs`, all systems):

```rust
//! The pure security rules of design parts 3.2 to 3.5 and P6: the security descriptors of the mutex, the pipe and the
//! data folder, the verdict on the token at the other end of the pipe, the production names and the tests' own names.
//! Compiled and tested on every system; `platform::win` applies them.

/// The SID (security identifier) of the LocalSystem account (design part 3.5). Def: T3-2. Use: T3-2, T3-3.
pub const SYSTEM_SID: &str = "S-1-5-18";
/// The SID of the built-in Administrators group (design parts 3.3, 3.5). Def: T3-2. Use: T3-2, T3-3.
pub const ADMINISTRATORS_SID: &str = "S-1-5-32-544";
/// The SID of the high mandatory integrity level, an elevated token's level (design part 3.4). Def: T3-2.
/// Use: T3-2, T3-11.
pub const HIGH_INTEGRITY_SID: &str = "S-1-16-12288";
/// The engine mutex's security descriptor in SDDL (Security Descriptor Definition Language): owner Administrators,
/// full access for SYSTEM and Administrators only (design part 3.3, Ruling W5). Def: T3-2. Use: T3-2.
pub const MUTEX_SDDL: &str = "O:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)";
/// The pipe's descriptor: read and write for `owner_sid` and SYSTEM only, under a high mandatory label that refuses
/// reads and writes from a lower level (design part 3.4, P6). Def: T3-2. Use: T3-11.
pub fn pipe_sddl(owner_sid: &str) -> String; // "D:P(A;;GRGW;;;<owner_sid>)(A;;GRGW;;;SY)S:(ML;;NWNR;;;HI)"
/// The data folder's access list: protected from the parent's, inherited by everything below, full control for
/// SYSTEM and Administrators, read for `reader_sid` (design part 3.5, Ruling W6). Def: T3-2. Use: T3-3.
pub fn folder_sddl(reader_sid: &str) -> String; // "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;<reader_sid>)"
/// Whether `owner_sid` may own the data folder: Administrators or SYSTEM, compared exactly (design part 3.5).
/// Def: T3-2. Use: T3-3.
pub fn owner_allowed(owner_sid: &str) -> bool;
/// A token's elevation type (design part 3.2): `Default` has no linked token (UAC, User Account Control, off, or a
/// service or Secure Shell (SSH) logon), `Full` is the elevated half of a split token, `Limited` the filtered half.
/// Def: T3-2. Use: T3-2, T3-7, T3-23.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElevationType { Default, Full, Limited }
/// What lazymkv reads from a token (design parts 3.2, 3.4): the user's SID string, whether the token is elevated, its
/// elevation type, its integrity level's SID string, and whether it has a linked token (Ruling T3-2-c).
/// Def: T3-2. Use: T3-2, T3-7, T3-11, T3-12, T3-23.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenFacts { pub user_sid: String, pub elevated: bool, pub elevation_type: ElevationType,
    pub integrity_sid: String, pub linked: bool }
/// The verdict on the process at the other end of the pipe (design parts 3.3, 3.4; P6): another user is refused
/// first, then a token that is not elevated. Only `user_sid` and `elevated` are read. Def: T3-2. Use: T3-11, T3-12.
pub fn token_verdict(peer: &TokenFacts, own_user_sid: &str) -> Result<(), &'static str>;
    // Err("its user differs from this process's user"), then Err("its token is not elevated")
/// The engine's pipe (design part 3.3, ARCH-16). Only the binaries use it. Def: T3-2. Use: T3-12, T3-23.
pub const PIPE_NAME: &str = r"\\.\pipe\lazymkv";
/// The engine's mutex, in the namespace every logon session shares (design part 3.3, ARCH-16). Only the binaries use
/// it. Def: T3-2. Use: T3-23.
pub const MUTEX_NAME: &str = r"Global\lazymkv-engine";
/// A name for one test's pipe, mutex or credential prefix that no other test and no running engine uses (Global
/// Constraints): `lazymkv-test-<tag>-<process id>-<n>`, `n` from a process-wide `AtomicU32` that starts at 0. `tag`
/// is a short lower-case word with no backslash. Def: T3-2. Use: T3-2, T3-11, T3-12, T3-13, T3-23.
pub fn test_name(tag: &str) -> String;
```

Each variant of `ElevationType` and each field of `TokenFacts` gets a one-line `///` saying what it holds.

- Produces (`crates/platform/src/win`, `cfg(windows)`):

```rust
// win/mod.rs (Task 1's doc stays)
mod mutex;
mod token;
pub use mutex::{EngineMutex, MutexRefusal};
pub use token::own_token;
pub(crate) use token::{sid_string, token_facts, token_of_process, Descriptor};
#[cfg(test)]
pub(crate) use token::{read_descriptor, read_handle, read_path, AclEntry, ReadBack};
/// `text` as UTF-16 code units with a terminating NUL, for the wide-string arguments of Win32 calls (design part
/// 4.3). Def: T3-2. Use: T3-2 to T3-16.
pub(crate) fn wide(text: impl AsRef<OsStr>) -> Vec<u16>;

// win/token.rs
/// The facts of this process's token (design part 3.2): the engine's start check, and the confirmation that the mini
/// PC's SSH session is elevated. Def: T3-2. Use: T3-3, T3-11, T3-12, T3-23.
pub fn own_token() -> io::Result<TokenFacts>;
/// The facts of `token`, opened with `TOKEN_QUERY` (design parts 3.2, 3.4). Def: T3-2. Use: T3-2, T3-7, T3-11.
pub(crate) fn token_facts(token: HANDLE) -> io::Result<TokenFacts>;
/// The facts of process `pid`'s token: the pipe's client (T3-11) and its server (T3-12) (design parts 3.3, 3.4).
/// Def: T3-2. Use: T3-11, T3-12.
pub(crate) fn token_of_process(pid: u32) -> io::Result<TokenFacts>;
/// `sid` in its string form, `S-1-5-...` (design parts 3.3, 3.5). Def: T3-2. Use: T3-2, T3-3.
pub(crate) fn sid_string(sid: PSID) -> io::Result<String>;
/// A security descriptor built from an SDDL string, freed with `LocalFree` on drop (design parts 3.3 to 3.5).
/// Def: T3-2. Use: T3-2, T3-3, T3-11, T3-12.
pub(crate) struct Descriptor(PSECURITY_DESCRIPTOR);
impl Descriptor {
    pub(crate) fn from_sddl(sddl: &str) -> io::Result<Descriptor>;
    pub(crate) fn as_ptr(&self) -> PSECURITY_DESCRIPTOR;
    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES; // points at this descriptor, which outlives the call it is passed to
    pub(crate) fn dacl(&self) -> *mut ACL;                   // null when the descriptor has no access list
}
// SAFETY: the descriptor is written only by the conversion that made it and is read-only after; LocalFree runs once,
// in Drop. PipeServer (an IpcTransport: Send + Sync) holds one (T3-11).
unsafe impl Send for Descriptor {}
unsafe impl Sync for Descriptor {}
// cfg(test) only: what a test reads back from an object (design parts 3.3 to 3.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AclEntry { pub(crate) kind: u8, pub(crate) flags: u8, pub(crate) mask: u32, pub(crate) sid: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReadBack { pub(crate) owner: String, pub(crate) protected: bool, pub(crate) entries: Vec<AclEntry> }
pub(crate) fn read_descriptor(descriptor: PSECURITY_DESCRIPTOR) -> ReadBack; // an absent owner reads ""
pub(crate) fn read_handle(handle: HANDLE, object: SE_OBJECT_TYPE) -> io::Result<ReadBack>;
pub(crate) fn read_path(path: &Path) -> io::Result<ReadBack>;

// win/mutex.rs
/// The engine's named mutex, held for the engine's life (design parts 3.3, 3.8). Its existence is the lock (Ruling
/// T3-2-a); `Drop` closes the handle, which releases the name once no process holds one. Def: T3-2. Use: T3-23.
pub struct EngineMutex { handle: HANDLE }
/// Why the mutex was not taken (design part 3.3; Rulings W4, W5, T3-2-b). Def: T3-2. Use: T3-23.
#[derive(Debug)]
pub enum MutexRefusal {
    /// The mutex exists and Administrators own it: another elevated engine made it.
    EngineRunning,
    /// Something else holds the name; `account` names its owner, for standard error only (Ruling W4).
    Squatter { account: String },
    /// Windows refused the creation for another reason.
    Os(io::Error),
}
impl EngineMutex {
    /// Creates the mutex `name` from MUTEX_SDDL, or says who holds it (design part 3.3; Rulings W5, T3-2-b).
    /// Def: T3-2. Use: T3-23.
    pub fn take(name: &str) -> Result<EngineMutex, MutexRefusal>;
}
/// A squatter's account when its owner cannot be read (Ruling W5). Def: T3-2. Use: T3-2.
pub(crate) const UNREADABLE_ACCOUNT: &str = "an account lazymkv cannot read";
```

- Produces (`crates/platform/src/testutil.rs`, `cfg(test)`; the Global Constraints' helpers for every platform test):

```rust
pub(crate) fn temp_dir(tag: &str) -> PathBuf; // moved unchanged from lib.rs:262-270
pub(crate) fn ci_factor() -> u32;             // a copy of crates/engine/src/testkit.rs:71-77 (R29.c2): 4 when CI is set, else 1
pub(crate) fn skip(reason: &str);             // "skipped: <reason>\n" with std::io::stderr().write_all, as crates/dev/src/evidence.rs:513-521
pub(crate) fn cue(text: &str);                // "CUE: <text>\n" with std::io::stderr().write_all, then flush
pub(crate) struct Secrets { secrets: Mutex<BTreeMap<String, String>> } // impl SecretStore; names compared exactly
impl Secrets { pub(crate) fn new() -> Secrets; pub(crate) fn with(self, name: &str, value: &str) -> Secrets; } // as MemorySecrets, fakes.rs:264-296
```

Rulings settled in this task:
- **Ruling T3-2-a:** the mutex is created with `bInitialOwner` false and never waited on. Its existence is the lock, as design part 3.3 decides by the owner of an existing mutex. A wait would be wrong twice: a mutex's owning thread takes it again at once, so a second take in one process would pass, and an owned mutex must be released by the thread that took it.
- **Ruling T3-2-b:** an existing mutex owned by Administrators is `EngineRunning`. Any other owner that can be read is `Squatter`, named `<domain>\<name>` by `LookupAccountSidW` (`<name>` when the domain is empty, the SID string when the SID has no name). An owner that cannot be read is `Squatter { account: UNREADABLE_ACCOUNT }`: a mutex whose list refuses the engine and its read of the owner, and a name another kind of object holds (`ERROR_INVALID_HANDLE`, 6).
- **Ruling T3-2-c:** `TokenFacts.linked` is true when `GetTokenInformation(TokenLinkedToken)` returns a token, which is closed at once. A `Default` token has none.
- **Ruling T3-2-d:** the token test writes its facts line with `std::io::stderr().write_all`, past the test harness's capture, so a plain `cargo test` on `windows-latest` shows it in the log Task 26 step 4 reads, and Task 25 step 1 shows it over SSH.

Owner decisions: 12 (a squatter's account goes to standard error only, Ruling W4; non-blocking) and 15 (after Step 6's continuous integration (CI) run). Creating an object owned by Administrators needs an elevated token (`ERROR_INVALID_OWNER`, 1307, otherwise), so if Owner decision 15's fallback is taken, the `win::mutex` tests skip on the runner with the same sentence as the pipe tests.

- [ ] **Step 1: Write the failing tests**

In `crates/platform/src/lib.rs`, after `pub use process::StdProcessGuard;`:

```rust
/// The pure security rules of design parts 3.2 to 3.5: the descriptor strings, the token verdict and the names.
/// Def: T3-2. Use: T3-2, T3-3, T3-11, T3-12, T3-23.
pub mod security;
pub use security::{MUTEX_NAME, PIPE_NAME};
```

and, in place of `lib.rs:257-271`:

```rust
#[cfg(test)]
#[allow(dead_code)] // ci_factor, skip and cue serve the Windows tests, so the other systems never call them
mod testutil;
```

`crates/platform/src/win/mod.rs` gains the `mod` and `use` lines of Interfaces.

`crates/platform/src/security.rs`, holding only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "S-1-5-21-1000-2000-3000-1001";
    const OTHER: &str = "S-1-5-21-1000-2000-3000-1002";
    const OTHER_USER: Result<(), &str> = Err("its user differs from this process's user");

    fn peer(user_sid: &str, elevated: bool, elevation_type: ElevationType) -> TokenFacts {
        TokenFacts {
            user_sid: user_sid.to_string(),
            elevated,
            elevation_type,
            // S-1-16-8192 is the medium level a non-elevated process runs at.
            integrity_sid: if elevated { HIGH_INTEGRITY_SID } else { "S-1-16-8192" }.to_string(),
            linked: true,
        }
    }

    #[test]
    fn the_pipe_descriptor_grants_the_owner_and_system_read_and_write_with_a_high_label() {
        assert_eq!(pipe_sddl(OWN), "D:P(A;;GRGW;;;S-1-5-21-1000-2000-3000-1001)(A;;GRGW;;;SY)S:(ML;;NWNR;;;HI)");
    }

    #[test]
    fn the_folder_descriptor_is_protected_and_inherited() {
        assert_eq!(
            folder_sddl(OWN),
            "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;S-1-5-21-1000-2000-3000-1001)"
        );
    }

    #[test]
    fn only_administrators_and_system_may_own_the_data_folder() {
        assert!(owner_allowed("S-1-5-32-544"));
        assert!(owner_allowed("S-1-5-18"));
        assert!(!owner_allowed(OWN));
        assert!(!owner_allowed("S-1-5-32-545")); // the Users group
        assert!(!owner_allowed(""));
    }

    #[test]
    fn a_peer_of_another_user_is_refused_before_its_elevation_is_read() {
        assert_eq!(token_verdict(&peer(OTHER, false, ElevationType::Limited), OWN), OTHER_USER);
        assert_eq!(token_verdict(&peer(OTHER, true, ElevationType::Full), OWN), OTHER_USER);
        assert_eq!(token_verdict(&peer(SYSTEM_SID, true, ElevationType::Default), OWN), OTHER_USER);
    }

    #[test]
    fn a_peer_of_the_same_user_without_elevation_is_refused() {
        assert_eq!(token_verdict(&peer(OWN, false, ElevationType::Limited), OWN), Err("its token is not elevated"));
    }

    #[test]
    fn an_elevated_peer_of_the_same_user_passes() {
        assert_eq!(token_verdict(&peer(OWN, true, ElevationType::Full), OWN), Ok(()));
        // An SSH session's elevated token has no linked token.
        assert_eq!(token_verdict(&peer(OWN, true, ElevationType::Default), OWN), Ok(()));
    }

    #[test]
    fn test_names_differ_within_a_process_and_carry_the_tag_and_pid() {
        let (first, second) = (test_name("pipe"), test_name("pipe"));
        let prefix = format!("lazymkv-test-pipe-{}-", std::process::id());
        assert_ne!(first, second);
        assert!(first.starts_with(&prefix), "{first}");
        assert!(second.starts_with(&prefix), "{second}");
        assert!(first[prefix.len()..].parse::<u32>().is_ok(), "{first}");
    }

    #[test]
    fn the_production_names_are_the_designs() {
        assert_eq!(PIPE_NAME, r"\\.\pipe\lazymkv");
        assert_eq!(MUTEX_NAME, r"Global\lazymkv-engine");
        assert_eq!(MUTEX_SDDL, "O:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)");
        assert_eq!((SYSTEM_SID, ADMINISTRATORS_SID, HIGH_INTEGRITY_SID), ("S-1-5-18", "S-1-5-32-544", "S-1-16-12288"));
    }
}
```

`crates/platform/src/testutil.rs`, holding `temp_dir` moved from `lib.rs:259-270` (its `use std::path::PathBuf;` and the function, made `pub(crate)`) and these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretStore;

    #[test]
    fn the_test_secret_store_compares_names_exactly() {
        let secrets = Secrets::new().with(r"share:\\server\share", "user-sentinel-7c1e\npass-sentinel-4b9d");
        assert_eq!(secrets.get(r"share:\\server\share").as_deref(), Some("user-sentinel-7c1e\npass-sentinel-4b9d"));
        assert_eq!(secrets.get(r"share:\\SERVER\Share"), None);
        secrets.set("tmdb_token", "token-sentinel-2f6a").unwrap();
        assert_eq!(secrets.get("TMDB_TOKEN"), None);
    }

    #[test]
    fn the_skip_and_cue_lines_end_in_a_newline_after_their_prefix() {
        assert_eq!(skip_line("no drive in LAZYMKV_TEST_DRIVE"), "skipped: no drive in LAZYMKV_TEST_DRIVE\n");
        assert_eq!(cue_line("insert a disc in D:"), "CUE: insert a disc in D:\n");
    }
}
```

`crates/platform/src/win/token.rs`, holding only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn the_process_token_is_read_and_printed() {
        let facts = own_token().unwrap();
        let line = format!(
            "token facts: elevated={} type={:?} integrity={} linked={}\n",
            facts.elevated, facts.elevation_type, facts.integrity_sid, facts.linked
        );
        std::io::stderr().write_all(line.as_bytes()).unwrap(); // Ruling T3-2-d; the user SID is not printed
        assert!(facts.user_sid.starts_with("S-1-5-"), "the user SID is not an account SID");
        assert!(facts.integrity_sid.starts_with("S-1-16-"), "{}", facts.integrity_sid);
        if std::env::var_os("CI").is_some() || std::env::var_os("SSH_CONNECTION").is_some() {
            assert!(facts.elevated, "design part 3.2: the runner and the mini PC's SSH session run elevated");
        }
    }

    #[test]
    fn the_token_of_this_process_read_by_its_id_equals_the_process_token() {
        assert!(token_of_process(std::process::id()).unwrap() == own_token().unwrap());
    }
}
```

`crates/platform/src/win/mutex.rs`, holding only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use std::io;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::Authorization::SE_KERNEL_OBJECT;
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW};

    use super::*;
    use crate::security::{test_name, ADMINISTRATORS_SID, SYSTEM_SID};
    use crate::win::{own_token, read_handle, wide, Descriptor};

    const MUTEX_ALL_ACCESS: u32 = 0x001F_0001;
    const GENERIC_ALL: u32 = 0x1000_0000;

    /// A handle another process would hold on a name, closed on drop.
    struct Held(HANDLE);

    impl Drop for Held {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateMutexW or CreateEventW in this test and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Creates the mutex `name` with `sddl`, as a process other than the engine would.
    fn hold_mutex(name: &str, sddl: &str) -> Held {
        let descriptor = Descriptor::from_sddl(sddl).unwrap();
        let attributes = descriptor.attributes();
        let name = wide(name);
        // SAFETY: the attributes point at `descriptor`, which outlives the call; the name is NUL-terminated.
        let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
        assert!(!handle.is_null(), "CreateMutexW: {}", io::Error::last_os_error());
        Held(handle)
    }

    /// The result's variant, for messages that never print a squatter's account (Ruling W4).
    fn kind(result: &Result<EngineMutex, MutexRefusal>) -> &'static str {
        match result {
            Ok(_) => "Ok",
            Err(MutexRefusal::EngineRunning) => "EngineRunning",
            Err(MutexRefusal::Squatter { .. }) => "Squatter",
            Err(MutexRefusal::Os(_)) => "Os",
        }
    }

    /// The mask as the kernel holds it once GA is mapped to the mutex's own rights.
    fn mapped(mask: u32) -> u32 {
        if mask == GENERIC_ALL { MUTEX_ALL_ACCESS } else { mask }
    }

    #[test]
    fn a_second_take_of_a_held_mutex_says_an_engine_is_running() {
        let name = format!(r"Global\{}", test_name("mutex"));
        let first = EngineMutex::take(&name);
        assert_eq!(kind(&first), "Ok");
        assert_eq!(kind(&EngineMutex::take(&name)), "EngineRunning");
    }

    #[test]
    fn the_mutex_is_free_again_once_dropped() {
        let name = format!(r"Global\{}", test_name("mutex-free"));
        let first = EngineMutex::take(&name);
        assert_eq!(kind(&first), "Ok");
        drop(first);
        assert_eq!(kind(&EngineMutex::take(&name)), "Ok");
    }

    #[test]
    fn a_mutex_owned_by_the_user_is_a_squatter_named_by_account() {
        let user = own_token().unwrap().user_sid;
        let name = format!(r"Global\{}", test_name("mutex-user"));
        let _held = hold_mutex(&name, &format!("O:{user}D:P(A;;GA;;;{user})(A;;GA;;;SY)(A;;GA;;;BA)"));
        match EngineMutex::take(&name) {
            Err(MutexRefusal::Squatter { account }) => {
                assert!(!account.is_empty() && account != UNREADABLE_ACCOUNT, "the squatter's account is not named")
            }
            other => panic!("expected Squatter, got {}", kind(&other)),
        }
    }

    #[test]
    fn a_mutex_whose_owner_cannot_be_read_is_a_squatter_lazymkv_cannot_read() {
        // OW is OWNER RIGHTS: denying it takes the owner's implicit right to read the descriptor away too.
        let user = own_token().unwrap().user_sid;
        let name = format!(r"Global\{}", test_name("mutex-dark"));
        let _held = hold_mutex(&name, &format!("O:{user}D:P(D;;GA;;;OW)"));
        match EngineMutex::take(&name) {
            Err(MutexRefusal::Squatter { account }) => {
                assert!(account == UNREADABLE_ACCOUNT, "the account is not UNREADABLE_ACCOUNT")
            }
            other => panic!("expected Squatter, got {}", kind(&other)),
        }
    }

    #[test]
    fn a_name_held_by_another_kind_of_object_is_a_squatter_lazymkv_cannot_read() {
        let name = format!(r"Global\{}", test_name("mutex-event"));
        let wide_name = wide(&name);
        // SAFETY: default security, a manual-reset event not signalled; the name is NUL-terminated.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide_name.as_ptr()) };
        assert!(!event.is_null(), "CreateEventW: {}", io::Error::last_os_error());
        let _held = Held(event);
        match EngineMutex::take(&name) {
            Err(MutexRefusal::Squatter { account }) => {
                assert!(account == UNREADABLE_ACCOUNT, "the account is not UNREADABLE_ACCOUNT")
            }
            other => panic!("expected Squatter, got {}", kind(&other)),
        }
    }

    #[test]
    fn the_created_mutex_is_owned_by_administrators_and_grants_only_system_and_administrators() {
        let name = format!(r"Global\{}", test_name("mutex-list"));
        let taken = EngineMutex::take(&name);
        assert_eq!(kind(&taken), "Ok");
        let mutex = taken.ok().unwrap();
        let read = read_handle(mutex.handle, SE_KERNEL_OBJECT).unwrap();
        assert_eq!(read.owner, ADMINISTRATORS_SID);
        // Type 0 is an allow entry; the flags are 0, since a mutex has nothing below it to inherit.
        let entries: Vec<(u8, u8, u32, &str)> =
            read.entries.iter().map(|e| (e.kind, e.flags, mapped(e.mask), e.sid.as_str())).collect();
        assert_eq!(entries, vec![(0, 0, MUTEX_ALL_ACCESS, SYSTEM_SID), (0, 0, MUTEX_ALL_ACCESS, ADMINISTRATORS_SID)]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: compile errors E0425, E0412 and E0432 naming `pipe_sddl`, `folder_sddl`, `owner_allowed`, `token_verdict`, `TokenFacts`, `ElevationType`, `test_name`, `Secrets`, `skip_line`, `cue_line`, `own_token`, `token_of_process`, `EngineMutex`, `MutexRefusal`, `UNREADABLE_ACCOUNT`, `Descriptor`, `read_handle` and `wide`.

- [ ] **Step 3: Implement the items of Interfaces**

`security.rs`: the items of Interfaces, as their docs and the tests fix them.

`testutil.rs`:
- A module doc: the test helpers of Global Constraints, compiled under `cfg(test)` only.
- `ci_factor` copies `crates/engine/src/testkit.rs:71-77`.
- `skip` and `cue` write the private `skip_line(reason)` and `cue_line(text)` (the test pins both texts); `cue` then flushes.
- Both write with `std::io::stderr().write_all`, so the harness's capture never holds them back, and ignore a write error.
- `Secrets` copies `MemorySecrets` (`fakes.rs:264-296`) under its own name, with `fakes.rs:18-20`'s poison-tolerant lock.

`win/token.rs`. Every `unsafe` block carries a `// SAFETY:` line.
- `own_token`: `OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY)`, then `token_facts`, then `CloseHandle`.
- `token_of_process`: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid)`, `OpenProcessToken(TOKEN_QUERY)`, then `token_facts`. Both handles are closed on every path.
- `token_facts` reads, with `GetTokenInformation`:
  - `TokenUser`, the SID through `sid_string`;
  - `TokenElevation`, its `TokenIsElevated != 0`;
  - `TokenElevationType`: 1 is `Default`, 2 `Full`, 3 `Limited`, and any other value is `ErrorKind::InvalidData`;
  - `TokenIntegrityLevel`, the label's SID through `sid_string`;
  - `TokenLinkedToken`, as Ruling T3-2-c says.
  The variable-size classes are read with two calls, the first for the size. Any other failure is `io::Error::last_os_error()`.
- `sid_string`: `ConvertSidToStringSidW`, copied into a `String`, then the buffer freed with `LocalFree`.
- `Descriptor::from_sddl`: `ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl, SDDL_REVISION_1, ..)`. `Drop` calls `LocalFree`.
- `Descriptor::attributes` is `SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: self.0, bInheritHandle: 0 }`.
- `Descriptor::dacl`: `GetSecurityDescriptorDacl`, or null when it reports no list.
- `read_descriptor`, from the descriptor:
  - the owner through `GetSecurityDescriptorOwner` and `sid_string`;
  - `protected` from `GetSecurityDescriptorControl`'s `SE_DACL_PROTECTED` (0x1000);
  - each entry through `GetAce` over the list's `AceCount`, read as `ACCESS_ALLOWED_ACE`, whose layout the deny entry shares. Any other entry type panics, since no lazymkv list holds one.
- `read_handle`: `GetSecurityInfo(handle, object, OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION, ..)`, then `read_descriptor`, then `LocalFree`.
- `read_path`: the same through `GetNamedSecurityInfoW(path, SE_FILE_OBJECT, ..)`.

`win/mutex.rs`. `EngineMutex::take(name)`, Rulings T3-2-a and T3-2-b, in this order:
1. Call `CreateMutexW(&Descriptor::from_sddl(MUTEX_SDDL)?.attributes(), FALSE, name)`. A descriptor error is `Os`.
2. A handle, with `GetLastError()` read at once and not `ERROR_ALREADY_EXISTS` (183), is ours: `Ok(EngineMutex { handle })`.
3. A handle with 183 is an existing mutex. Read its owner through that handle with `GetSecurityInfo(SE_KERNEL_OBJECT, OWNER_SECURITY_INFORMATION)`.
4. A null handle with `ERROR_ACCESS_DENIED` (5) or `ERROR_INVALID_HANDLE` (6): try `OpenMutexW(READ_CONTROL, FALSE, name)`. A handle from it is read as in 3; a failure is `Squatter { account: UNREADABLE_ACCOUNT }`.
5. Any other null-handle error is `Os(io::Error::from_raw_os_error(code))`.
6. The owner read in 3 or 4 decides:
   - `ADMINISTRATORS_SID` is `EngineRunning`;
   - any other SID is `Squatter`, named by `LookupAccountSidW` as Ruling T3-2-b says;
   - a failed read is `Squatter { account: UNREADABLE_ACCOUNT }`.
   The handle is closed in every case.

`Drop for EngineMutex` calls `CloseHandle(self.handle)`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: PASS, `test result: ok. 25 passed; 0 failed; 1 ignored` (PR 2's 7 and 1 ignored `process::tests::helper_child`, and 18 new). Standard error shows one line `token facts: elevated=true type=<Default|Full|Limited> integrity=S-1-16-<n> linked=<true|false>`. On ubuntu and macos the same command gives `17 passed`.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/platform/src/lib.rs crates/platform/src/security.rs crates/platform/src/testutil.rs crates/platform/src/win/mod.rs crates/platform/src/win/token.rs crates/platform/src/win/mutex.rs
git commit -m "platform: token facts and the engine mutex (design parts 3.2, 3.3)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, then the message alone, with no attribution line.

- [ ] **Step 6: Open the draft pull request and read `windows-latest`'s facts (Owner decision 15)**

CI runs only on a pull request and on a push to `main` (`.github/workflows/ci.yml:10-13`), so the runner's facts need the draft open now. Ask the owner in chat: "May I push `phase-2-windows-pieces` (Tasks 1 and 2) and open pull request 3 as a draft, then push each later task's commit to it, so CI shows `windows-latest`'s token facts and runs each task's Windows tests?" Only on his yes (a line in `decisions.txt`):

```bash
git push -u origin phase-2-windows-pieces
gh pr create --draft --repo krcm0209/lazymkv --base main --head phase-2-windows-pieces \
  --title "Phase 2 pull request 3: the Windows pieces" --body-file .superpowers/pr3/pr3-body.md
gh run watch
gh run view <run id> --log | grep 'token facts:'
```

`pr3-body.md` holds the Goal paragraph and "Draft: tasks land one commit at a time", with no attribution line. Expected: CI green on the three systems, and one `token facts: elevated=<bool> ...` line from `windows-latest`. `elevated=false` puts Owner decision 15 to the owner before Task 3 starts; record his answer. Without the yes, Tasks 3 to 25 go on and Task 26 Step 1 opens the draft.

---

### Task 3: The protected data folder (design part 3.5)

**Files:**
- Create: `crates/platform/src/win/folder.rs` (`cfg(windows)`; 7 tests in its `mod tests`).
- Modify: `crates/platform/src/win/mod.rs` (`mod folder; pub use folder::{program_data, protect_data_folder};`).

No test of this task touches `%ProgramData%\lazymkv` or `C:\lazymkv\dev\data`. Each works in a folder it makes under `std::env::temp_dir()` and removes, even when an assertion fails (Global Constraints). The engine's first protection of a real folder waits for Owner decisions 2 and 5 in Task 25.

**Interfaces:**
- Consumes: `security::{folder_sddl, owner_allowed, ADMINISTRATORS_SID}`; `win::{own_token, sid_string, wide, Descriptor}` and, in tests, `win::{read_descriptor, read_path, AclEntry}` (Task 2); `testutil::temp_dir`; Task 1's features `Win32_Security`, `Win32_Security_Authorization`, `Win32_Storage_FileSystem`.
- Produces:

```rust
// crates/platform/src/win/folder.rs
/// Makes `path` the engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c). An absent
/// folder is created owned by Administrators. A link, a junction, a file, or a folder owned by any account but
/// Administrators or SYSTEM is refused. Then the protected access list `folder_sddl(reader_sid)` is set, and everything
/// below inherits it. `reader_sid` is the owner's account, the engine's own token user. Err is a sentence for standard
/// error. Def: T3-3. Use: T3-23.
pub fn protect_data_folder(path: &Path, reader_sid: &str) -> Result<(), String>;
/// `%ProgramData%` from the environment (design part 3.1, Ruling W1), or None when it is unset, empty or not an
/// absolute path. The engine's default data folder is this joined with `lazymkv`. Def: T3-3. Use: T3-23.
pub fn program_data() -> Option<PathBuf>;
```

Its error texts (`<path>` is `path.display()`; `<error>` is `std::io::Error::from_raw_os_error(<code>)` or the `io::Error` met):
- `<path> is owned by <owner SID>, neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)` (the sheet's text)
- `<path> is a link, a junction or a file, not a folder; the engine will not use it (design part 3.5)`
- `<path> could not be created: <error>`
- `<path> could not be read: <error>`
- `<path>'s owner could not be read: <error>`
- `<path>'s access list could not be set: <error>`

Rulings settled in this task:
- **Ruling T3-3-a (creation):** an absent folder is made by `CreateDirectoryW` with security attributes from `Descriptor::from_sddl(&format!("O:BA{}", folder_sddl(reader_sid)))`. It is therefore owned by Administrators (Ruling W6) and protected from its first moment, with no window in which the parent's list lets another account write it. Only the last component is created. A missing parent is `could not be created`, as Ruling W15 never creates deeper.
- **Ruling T3-3-b (links):** a path whose own attributes carry `FILE_ATTRIBUTE_REPARSE_POINT` (0x400), such as a symbolic link or a junction, or a path that is not a folder, is refused before any change. `SetNamedSecurityInfoW` follows a link, so an elevated engine would otherwise rewrite the list of whatever folder another process pointed it at.
- **Ruling T3-3-c (order):** the owner is checked before the list is set, so a refused folder keeps its list.

Owner decision 15: if its fallback is taken, the `win::folder` tests that make a folder owned by Administrators skip on the runner with the same sentence as the pipe tests, since that needs an elevated token.

- [ ] **Step 1: Write the failing tests**

`crates/platform/src/win/mod.rs` gains `mod folder; pub use folder::{program_data, protect_data_folder};`. `crates/platform/src/win/folder.rs`, holding only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, SetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID, UNPROTECTED_DACL_SECURITY_INFORMATION,
    };

    use super::*;
    use crate::security::{folder_sddl, ADMINISTRATORS_SID};
    use crate::testutil::temp_dir;
    use crate::win::{own_token, read_descriptor, read_path, wide, AclEntry, Descriptor};

    /// Removes the test's folder when the test ends, failed or not (Global Constraints).
    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The entries `folder_sddl(sid)` holds, read with the reader the folder is read with.
    fn folder_entries(sid: &str) -> Vec<AclEntry> {
        read_descriptor(Descriptor::from_sddl(&folder_sddl(sid)).unwrap().as_ptr()).entries
    }

    /// The entries a file below gets from the folder's: the same rights, flagged only INHERITED_ACE (0x10).
    fn inherited(folder: &[AclEntry]) -> Vec<AclEntry> {
        folder.iter().map(|entry| AclEntry { flags: 0x10, ..entry.clone() }).collect()
    }

    /// Sets `path`'s owner to `sid`, as another process or an earlier run would have left it.
    fn set_owner(path: &Path, sid: &str) {
        let (sid_text, name) = (wide(sid), wide(path));
        let mut owner: PSID = std::ptr::null_mut();
        // SAFETY: the SID text is NUL-terminated; `owner` receives a SID that LocalFree frees below.
        assert_ne!(unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut owner) }, 0, "ConvertStringSidToSidW");
        // SAFETY: the name is NUL-terminated, `owner` is a valid SID, and no other part is passed.
        let code = unsafe {
            SetNamedSecurityInfoW(name.as_ptr(), SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION, owner,
                std::ptr::null_mut(), std::ptr::null(), std::ptr::null())
        };
        // SAFETY: `owner` came from ConvertStringSidToSidW and is freed once.
        unsafe { LocalFree(owner) };
        assert_eq!(code, 0, "SetNamedSecurityInfoW(owner): {}", std::io::Error::from_raw_os_error(code as i32));
    }

    /// Replaces `path`'s list with `sddl`'s, not protected, as an earlier tool might have left it.
    fn set_open_list(path: &Path, sddl: &str) {
        let (descriptor, name) = (Descriptor::from_sddl(sddl).unwrap(), wide(path));
        // SAFETY: the name is NUL-terminated and the list belongs to `descriptor`, which outlives the call.
        let code = unsafe {
            SetNamedSecurityInfoW(name.as_ptr(), SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | UNPROTECTED_DACL_SECURITY_INFORMATION, std::ptr::null_mut(),
                std::ptr::null_mut(), descriptor.dacl(), std::ptr::null())
        };
        assert_eq!(code, 0, "SetNamedSecurityInfoW(list): {}", std::io::Error::from_raw_os_error(code as i32));
    }

    #[test]
    fn an_absent_folder_is_created_owned_by_administrators_with_the_protected_list() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-new"));
        let path = dir.0.join("data");
        protect_data_folder(&path, &own.user_sid).unwrap();
        let read = read_path(&path).unwrap();
        assert_eq!(read.owner, ADMINISTRATORS_SID);
        assert!(read.protected, "the folder still inherits from its parent");
        assert_eq!(read.entries, folder_entries(&own.user_sid));
    }

    #[test]
    fn an_existing_folder_owned_by_administrators_gets_the_list_again() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-again"));
        let path = dir.0.join("data");
        fs::create_dir(&path).unwrap();
        set_owner(&path, ADMINISTRATORS_SID);
        set_open_list(&path, "D:AI(A;OICI;FA;;;WD)"); // WD is Everyone
        fs::write(path.join("settings.json"), "{}").unwrap();
        protect_data_folder(&path, &own.user_sid).unwrap();
        let read = read_path(&path).unwrap();
        assert_eq!(read.owner, ADMINISTRATORS_SID);
        assert!(read.protected);
        assert_eq!(read.entries, folder_entries(&own.user_sid));
        let file = read_path(&path.join("settings.json")).unwrap();
        assert_eq!(file.entries, inherited(&folder_entries(&own.user_sid)), "the new list reached the file below");
    }

    #[test]
    fn a_folder_owned_by_the_user_is_refused_by_sid() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-user"));
        let path = dir.0.join("data");
        fs::create_dir(&path).unwrap();
        set_owner(&path, &own.user_sid);
        let before = read_path(&path).unwrap();
        assert_eq!(
            protect_data_folder(&path, &own.user_sid),
            Err(format!(
                "{} is owned by {}, neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)",
                path.display(),
                own.user_sid
            ))
        );
        assert!(read_path(&path).unwrap() == before, "a refused folder keeps its owner and list");
    }

    #[test]
    fn a_file_created_below_inherits_the_list() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-below"));
        let path = dir.0.join("data");
        protect_data_folder(&path, &own.user_sid).unwrap();
        fs::write(path.join("engine.log"), "").unwrap();
        let file = read_path(&path.join("engine.log")).unwrap();
        assert!(!file.protected);
        assert_eq!(file.entries, inherited(&folder_entries(&own.user_sid)));
    }

    #[test]
    fn a_junction_or_a_file_in_place_of_the_folder_is_refused() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-link"));
        let target = dir.0.join("target");
        fs::create_dir(&target).unwrap();
        set_owner(&target, ADMINISTRATORS_SID); // so only Ruling T3-3-b can refuse it
        let before = read_path(&target).unwrap();
        let link = dir.0.join("data");
        let made = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(&link).arg(&target)
            .output().unwrap();
        assert!(made.status.success(), "mklink /J failed");
        let refusal = |path: &Path| {
            format!("{} is a link, a junction or a file, not a folder; the engine will not use it (design part 3.5)",
                path.display())
        };
        assert_eq!(protect_data_folder(&link, &own.user_sid), Err(refusal(&link)));
        assert!(read_path(&target).unwrap() == before, "the junction's target kept its list");
        let file = dir.0.join("file");
        fs::write(&file, "").unwrap();
        assert_eq!(protect_data_folder(&file, &own.user_sid), Err(refusal(&file)));
    }

    #[test]
    fn a_missing_parent_is_not_created() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-parent"));
        let path = dir.0.join("absent").join("data");
        let error = protect_data_folder(&path, &own.user_sid).unwrap_err();
        assert!(error.starts_with(&format!("{} could not be created: ", path.display())), "{error}");
        assert!(!dir.0.join("absent").exists());
    }

    #[test]
    fn program_data_is_read_from_the_environment() {
        let set = PathBuf::from(std::env::var_os("ProgramData").expect("Windows sets ProgramData"));
        assert!(set.is_absolute());
        assert_eq!(program_data(), Some(set));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::folder`
Expected: compile errors E0425 and E0432 naming `protect_data_folder` and `program_data`.

- [ ] **Step 3: Implement `protect_data_folder` and `program_data`**

`protect_data_folder(path, reader_sid)`, in this order (Rulings T3-3-a to T3-3-c). Every `unsafe` block carries a `// SAFETY:` line.
1. `fs::symlink_metadata(path)`:
   - `NotFound`: create the folder as Ruling T3-3-a says. A `CreateDirectoryW` failure is `could not be created`, except `ERROR_ALREADY_EXISTS` (183), which means another process made it meanwhile; that path is read again and goes on as an existing one.
   - Any other error: `could not be read`.
2. An existing path whose `MetadataExt::file_attributes()` carries 0x400, or that is not a folder, gets the link refusal.
3. `GetNamedSecurityInfoW(path, SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION, ..)` and `sid_string`:
   - a failure is `'s owner could not be read`;
   - an owner for which `owner_allowed` is false gets the owner refusal, with that SID.
4. `SetNamedSecurityInfoW(path, SE_FILE_OBJECT, DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, null, null, Descriptor::from_sddl(&folder_sddl(reader_sid))?.dacl(), null)`. Windows propagates the list below. A failure, or a descriptor error, is `'s access list could not be set`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::folder`
Expected: PASS, `test result: ok. 7 passed; 0 failed; 0 ignored`, with the crate's other tests filtered out.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/platform/src/win/folder.rs crates/platform/src/win/mod.rs
git commit -m "platform: the protected data folder (design part 3.5, MISS-ARCH-18)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, then the message alone, with no attribution line.

---

### Task 4: The file moves on `MoveFileExW` (S13; design parts 6.5, 7.4)

**Files:**
- Create: `crates/platform/src/win/files.rs` (compiled only under `cfg(windows)`, through `lib.rs`'s `#[cfg(windows)] pub mod win;`; 6 tests in its `mod tests`).
- Modify: `crates/platform/src/win/mod.rs` (`mod files; pub(crate) use files::{move_no_replace, move_replacing};`).
- Modify: `crates/platform/src/lib.rs:220-256` (the docs of `write_atomic` and `rename_no_replace`; on Windows `write_atomic`'s last line calls `win::move_replacing` and `rename_no_replace` calls `win::move_no_replace`; the PR 2 bodies stay under `cfg(not(windows))`).
- Modify: `crates/engine/src/steps.rs:164-179` (`one_file` and `half_done` under `#[cfg(not(windows))]`), `:1556-1564` (`file`'s doc), `:1652-1659` and `:1668-1671` (the two match arms that call `one_file`, under `#[cfg(not(windows))]`).
- Modify: `crates/engine/src/store.rs:291` (the sentence of `replace_state`'s doc that names pull request 3).
- Modify: `crates/fake-tools/tests/engine_restart.rs:263-266` (`a_restart_after_a_rename_stopped_between_link_and_remove_finishes_it` under `#[cfg(not(windows))]`, its doc saying why).

**Interfaces:**
- Consumes: Task 1's `windows_sys` (`Win32_Foundation`, `Win32_Storage_FileSystem`); Task 2's `testutil::{temp_dir, skip}` and `security::test_name`.
- Produces: the two PR 2 functions, unchanged in signature, with Windows bodies; on Windows `write_atomic` returns `io::Error::from_raw_os_error` of the failed call unchanged (RR1-2 reads `raw_os_error()`), and `rename_no_replace` returns `ErrorKind::AlreadyExists` (os error 80 or 183) on an existing target.

```rust
// crates/platform/src/lib.rs (all OS; signatures unchanged)
/// Writes `bytes` to `path` so that a reader sees the old file or the new one whole, never a part (design part 7.4,
/// S13). It writes `<file name>.lazymkv-tmp` beside `path`, after removing one a crash left there, flushes it with
/// `sync_all` (`FlushFileBuffers` on Windows), and puts it in place of `path`. On Windows that is
/// `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`, tried again while the target is held open
/// (os errors 5, 32 and 33; 10 tries, 10 ms apart, Ruling T3-4-a), and the last error comes back with its raw code,
/// which the store's state.json retry reads (RR1-2); elsewhere it is `std::fs::rename`. It creates no folders: a
/// missing parent gives `ErrorKind::NotFound`, because each caller owns its folder layout (Ruling T9-c). A path with no
/// file name gives `ErrorKind::InvalidInput`. Def: T9; T3-4 swaps in `MoveFileExW` on Windows. Use: T21 to T26.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()>;
/// Moves `from` to `to` and fails with `ErrorKind::AlreadyExists` when `to` is there, so a finished file is never
/// overwritten (design part 6.5, LIB-8, S13). On Windows it is one `MoveFileExW(from, to, 0)`: without
/// `MOVEFILE_REPLACE_EXISTING` an existing `to` fails with os error 80 or 183, and without `MOVEFILE_COPY_ALLOWED` a
/// move to another volume fails with os error 17 instead of copying, so within a share it is the server's rename.
/// Elsewhere, where only the Linux and macOS CI runs use it, it is `hard_link` then `remove_file`, which is not atomic:
/// a stop between the two leaves both names on one file, which File then finishes (design part 6.5).
/// Def: T9; T3-4 swaps in `MoveFileExW` on Windows. Use: T24, T3-18.
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()>;

// crates/platform/src/win/files.rs (cfg(windows))
//! The file moves on `MoveFileExW` (S13; design parts 6.5, 7.4) that `write_atomic` and `rename_no_replace` make on
//! Windows.
/// How many times `move_replacing` tries a replace that meets a held target, and the pause after each failed try:
/// at most 90 ms in all (Ruling T3-4-a).
const HELD_TRIES: u32 = 10;
const HELD_PAUSE: Duration = Duration::from_millis(10);
/// `path` as the NUL-terminated wide (UTF-16, 16-bit Unicode) string Windows takes. Err `InvalidInput` with the text
/// `the path holds a NUL character` when the path holds one, since Windows would read the name only up to it. Named
/// apart from `win::wide` (T3-2), which takes any text and checks nothing.
fn wide_path(path: &Path) -> io::Result<Vec<u16>>;
/// Puts `from` in place of `to` with `MoveFileExW(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`
/// (design part 7.4, S13). A try that fails with os error 5, 32 or 33 is made again, up to `HELD_TRIES` tries
/// `HELD_PAUSE` apart (Ruling T3-4-a); any other error, and the last try's, is `io::Error::last_os_error()` read at once
/// after the failed call, so its raw code is the call's (RR1-2). Def: T3-4. Use: T3-4 (`write_atomic`).
pub(crate) fn move_replacing(from: &Path, to: &Path) -> io::Result<()>;
/// Moves `from` to `to` with one `MoveFileExW(from, to, 0)` (design part 6.5, LIB-8, S13); a failure is
/// `io::Error::last_os_error()` read at once after the call, so an existing `to` is `ErrorKind::AlreadyExists`
/// (80 or 183) and another volume is os error 17. Def: T3-4. Use: T3-4 (`rename_no_replace`).
pub(crate) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()>;
```

Settled in this task:
- **Ruling T3-4-a (a held target).** `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` refuses a target that any handle holds open, even one opened with delete sharing (os error 5), and the test harness opens `drives.json` and every `state.json` every 20 ms while an engine runs (`Harness::drives_json` at testkit.rs:731 and `quiet_marks`' `fs::metadata` at testkit.rs:783-803), as a virus scanner or an indexer may in production. Only `state.json` has the store's retry (RR1-2, store.rs:283-322), so a `drives.json` write that met a poll would fail and log `drives.json could not be written` (owner.rs:1165-1166). `move_replacing` therefore tries again on os errors 5, 32 and 33, 10 tries 10 ms apart, which covers a reader's open of a few microseconds; the store's retry stays around it for longer holders. The cost is at most 90 ms per write against a target held for good.
- **Ruling T3-4-b (File's half state).** `one_file`, `half_done` and the two arms of `file` that use them exist only for the hard-link body, so they compile only under `cfg(not(windows))`, and the restart test that builds the half state with `fs::hard_link` runs only there. On Windows a hard link at the final path is a collision like any other file. Both branches stay for the Linux and macOS body, since `windows-sys` is the only new crate and std has no rename that refuses an existing target there.
- `rename_no_replace`'s flags are 0, as the sheet says: design part 6.5 names only the two flags it leaves out.

No test in this task needs the owner's desktop session: the `win::files` tests run over Secure Shell (SSH) through `scripts/box.sh` and on `windows-latest`.

- [ ] **Step 1: Write the failing tests**

Add `mod files;` to `win/mod.rs`, so the crate compiles the tests. In `crates/platform/src/win/files.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Component, Path, Prefix};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    use super::{move_no_replace, move_replacing};
    use crate::security::test_name;
    use crate::testutil::{skip, temp_dir};
    use crate::{rename_no_replace, write_atomic};

    /// The upper-case drive letter `path` starts with, or None for a path without one.
    fn drive_of(path: &Path) -> Option<u8> {
        match path.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter.to_ascii_uppercase()),
                _ => None,
            },
            _ => None,
        }
    }

    #[test]
    fn the_no_replace_move_refuses_an_existing_target_and_keeps_both_files() {
        let dir = temp_dir("files-taken");
        let (from, to) = (dir.join("a.mkv.partial"), dir.join("a.mkv"));
        fs::write(&from, "new").unwrap();
        fs::write(&to, "old").unwrap();
        let error = move_no_replace(&from, &to).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(matches!(error.raw_os_error(), Some(80 | 183)), "{error:?}");
        assert_eq!(fs::read_to_string(&from).unwrap(), "new");
        assert_eq!(fs::read_to_string(&to).unwrap(), "old");
    }

    /// Design part 6.5: without `MOVEFILE_COPY_ALLOWED` a move to another volume is refused, never done as a copy. The
    /// other volume is the one this test binary sits on, when it is not the temporary folder's (on `windows-latest`
    /// the target folder is on D: and the temporary folder on C:; on the mini PC both are on C:, so it skips there).
    #[test]
    fn the_no_replace_move_across_volumes_fails_rather_than_copies() {
        let dir = temp_dir("files-xvol");
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        match (drive_of(&dir), drive_of(&exe_dir)) {
            (Some(temp), Some(exe)) if temp != exe => {}
            _ => {
                skip("the temporary folder and this test binary are on one volume, so no move across volumes can be tried");
                return;
            }
        }
        let other = exe_dir.join(test_name("xvol"));
        fs::create_dir(&other).unwrap();
        let from = dir.join("a.mkv.partial");
        fs::write(&from, "placeholder").unwrap();
        let error = move_no_replace(&from, &other.join("a.mkv")).unwrap_err();
        let kept = (from.exists(), other.join("a.mkv").exists());
        fs::remove_dir_all(&other).unwrap();
        assert_eq!(error.raw_os_error(), Some(17), "{error:?}");
        assert_eq!(kept, (true, false));
    }

    #[test]
    fn a_path_holding_nul_is_invalid_input_and_nothing_moves() {
        let dir = temp_dir("files-nul");
        fs::write(dir.join("a"), "kept").unwrap();
        let error = move_no_replace(&dir.join("a\0b"), &dir.join("c")).unwrap_err();
        assert_eq!((error.kind(), error.to_string()), (io::ErrorKind::InvalidInput, "the path holds a NUL character".to_string()));
        let error = move_replacing(&dir.join("a"), &dir.join("c\0d")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(dir.join("a").exists() && !dir.join("c").exists());
    }

    #[test]
    fn a_name_outside_ascii_survives_both_moves() {
        let dir = temp_dir("files-names");
        write_atomic(&dir.join("Café Ōsaka 千 (2001).json"), b"{}").unwrap();
        let partial = dir.join("Café Ōsaka 千 (2001).mkv.partial");
        fs::write(&partial, "placeholder").unwrap();
        rename_no_replace(&partial, &dir.join("Café Ōsaka 千 (2001).mkv")).unwrap();
        let mut names: Vec<String> =
            fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["Café Ōsaka 千 (2001).json", "Café Ōsaka 千 (2001).mkv"]);
    }

    /// RR1-2: a target held open for the whole call fails the replace with a code the store's retry knows, the old
    /// bytes stay, and the next write, once the holder is gone, replaces it and removes the temporary file.
    #[test]
    fn a_replace_over_a_target_held_open_returns_the_raw_os_error_the_store_retries() {
        for share in [FILE_SHARE_READ | FILE_SHARE_WRITE, 0] {
            let dir = temp_dir("files-held");
            let path = dir.join("state.json");
            fs::write(&path, b"old").unwrap();
            let held = fs::OpenOptions::new().read(true).share_mode(share).open(&path).unwrap();
            let error = write_atomic(&path, b"new").unwrap_err();
            println!("share {share:#x}: {error}");
            assert!(matches!(error.raw_os_error(), Some(5 | 32 | 33)), "share {share:#x}: {error:?}");
            drop(held);
            assert_eq!(fs::read(&path).unwrap(), b"old", "share {share:#x}");
            write_atomic(&path, b"new").unwrap();
            assert_eq!(fs::read(&path).unwrap(), b"new", "share {share:#x}");
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "share {share:#x}: a temporary file was left");
        }
    }

    /// Ruling T3-4-a: a reader that opens the target again and again, as the harness's polls do, never fails a write.
    #[test]
    fn a_target_another_thread_keeps_opening_is_replaced_every_time() {
        let dir = temp_dir("files-polled");
        let path = dir.join("drives.json");
        write_atomic(&path, b"0").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let poller = {
            let (path, stop) = (path.clone(), Arc::clone(&stop));
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    drop(fs::File::open(&path));
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        };
        for n in 1..=200u32 {
            write_atomic(&path, n.to_string().as_bytes()).unwrap_or_else(|error| panic!("write {n}: {error}"));
        }
        stop.store(true, Ordering::SeqCst);
        poller.join().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "200");
    }
}
```

The five PR 2 tests at `lib.rs:281-327` stay as they are and now run the Windows bodies on Windows; `write_atomic_replaces_the_file_whole` (its folder holds one file after two writes) is the "no temporary file after success" check. The engine's `persist_retries_a_state_json_another_process_holds_open` (store.rs:735) stays as it is and must pass on the new body.

In `crates/fake-tools/tests/engine_restart.rs`, above `a_restart_after_a_rename_stopped_between_link_and_remove_finishes_it` (:263-266), the doc gains a first sentence and the test its attribute:

```rust
/// Off Windows only (S13): there `rename_no_replace` is a hard link then a remove, and on Windows one `MoveFileExW`,
/// so the half state cannot arise there and a hard link at the final path is a collision (Ruling T3-4-b).
/// Design part 6.5 with `rename_no_replace`'s two steps (hard link, then remove): ...
#[cfg(not(windows))]
#[test]
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: a compile error, ``unresolved imports `super::move_no_replace`, `super::move_replacing` ``.

- [ ] **Step 3: Implement**

- `win/files.rs`: the items of the Interfaces block, as their docs say, each `unsafe` call with a `// SAFETY:` line; both moves convert their paths with `wide_path` before any call.
- `win/mod.rs`: `pub(crate) use files::{move_no_replace, move_replacing};`.
- `lib.rs`: `write_atomic` keeps every line up to `drop(file)`; its last line is `win::move_replacing(&temp, path)` under `cfg(windows)` and `std::fs::rename(&temp, path)` under `cfg(not(windows))`. `file.sync_all()` stays: std implements it with `FlushFileBuffers` on Windows, so the flush of design part 7.4 needs no `windows-sys` call. `rename_no_replace` is `win::move_no_replace(from, to)` under `cfg(windows)` and PR 2's `hard_link` then `remove_file` under `cfg(not(windows))`. Both docs are the Interfaces block's.
- `steps.rs`: `#[cfg(not(windows))]` on `one_file` (:164-174), `half_done` (:176-179) and the arms at :1652-1659 and :1668-1671. In `file`'s doc, the text from "An earlier rename is taken as done" (:1559) to the end of the first paragraph (:1564) becomes: "An earlier rename is taken as done, as design part 6.5 says after a restart in Verified, when the partial file is gone and the final path holds a file of the verified size (the title's `filed` size, RP-7); a partial file that is gone otherwise gives FileFailed { Unknown }. Off Windows, where `rename_no_replace` is a hard link then a remove (S13), a rename stopped between the two leaves both names on one file (`one_file`): its partial name is then removed and the rename taken as done, and a remove that fails after the link gives FileFailed { Unknown } with a message saying the final path holds the file. On Windows `MoveFileExW` renames in one step, so that half state cannot arise and those branches are not compiled (Ruling T3-4-b)."
- `store.rs:291`: "The retry stays when pull request 3 swaps in `MoveFileExW`." becomes "The retry stays now that `write_atomic` replaces with `MoveFileExW`, whose own tries cover only a reader's brief open (10 tries 10 ms apart, Ruling T3-4-a)."

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: PASS, with the six `win::files::tests` and the five PR 2 `tests` among them. On the mini PC `the_no_replace_move_across_volumes_fails_rather_than_copies` prints its `skipped:` line; it runs on `windows-latest`.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib store::`
Expected: PASS, `store::tests::persist_retries_a_state_json_another_process_holds_open` among them.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase --test engine_restart --test whole_job`
Expected: PASS; every filing and every persisted write of these harness jobs now runs the Windows moves, and the hard-link restart test is not compiled on Windows.

Then the workspace: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 5: Commit**

```
git add crates/platform/src/win/files.rs crates/platform/src/win/mod.rs crates/platform/src/lib.rs crates/engine/src/steps.rs crates/engine/src/store.rs crates/fake-tools/tests/engine_restart.rs
git commit -m "platform: write_atomic and rename_no_replace on MoveFileExW (S13; design parts 6.5, 7.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 5: The Job Object runner (design part 4.2; engine.md:579)

**Files:**
- Create: `crates/platform/src/win/job.rs` (`JobProcessGuard`; 10 tests in its `mod tests`).
- Modify: `crates/platform/src/process.rs:1-4` (the module doc), `:41-47` (`handed_over` made `pub(crate)`), `:84-146` (`#[cfg(test)] pub(crate) mod tests`: `HELPER_ARGS`, `helper` made `pub(crate)`, the helper modes `spawn-grandchild` and `print-pid-sleep`, and the two PR 2 test bodies moved into `check_kills_its_child` and `check_pipes_both_streams_and_adds_env`, which the PR 2 tests call with `&StdProcessGuard`).
- Modify: `crates/platform/src/lib.rs:15-16` (`native_guard` after `pub use process::StdProcessGuard;`), `crates/platform/src/win/mod.rs` (`mod job; pub use job::JobProcessGuard;`).
- Modify: `crates/engine/src/testkit.rs:38-40` (the import: `native_guard` in, `StdProcessGuard` out), `:417` (`TestGuard { .., inner: native_guard() }`), `:1029-1033` (`Runner::scripted` sets `processes: native_guard()`, its doc saying so), `:1043-1058` (`TestGuard` gains `inner: Arc<dyn ProcessGuard>`; its doc) and `:1080` (the last line becomes `retry_busy(|| self.inner.spawn(spawn))`). `BusyRetry` is unchanged, since `Arc<dyn ProcessGuard>` does not implement `ProcessGuard`.
- Modify: `crates/fake-tools/tests/tools_record.rs` (one `#[cfg(windows)]` test of engine.md:579 through `run_recorded`). The driver tests keep `StdProcessGuard` (Ruling W7).

**Interfaces:**
- Consumes: `Spawn`, `ProcessGuard`, `GuardedChild` and `KillHandle` with its `pub(crate)` field (lib.rs:53-96, Ruling T8-d); Task 1's `windows_sys` (`Win32_Foundation`, `Win32_System_JobObjects`, `Win32_System_Threading`, and `Win32_System_Diagnostics_ToolHelp` for the fallback of step 3); Task 2's `testutil::{ci_factor, temp_dir}`.
- Produces:

```rust
// crates/platform/src/lib.rs (all OS)
/// The process runner the engine uses on this system (spec 4.3, design part 4.2; Ruling W7): `win::JobProcessGuard`
/// on Windows, `StdProcessGuard` elsewhere. Def: T3-5. Use: T3-5 (testkit), T3-23.
pub fn native_guard() -> Arc<dyn ProcessGuard>;

// crates/platform/src/win/job.rs (cfg(windows))
/// The Windows `ProcessGuard` (spec 4.3, design part 4.2; Ruling W7): each child in a Job Object of its own with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, started suspended and assigned before its first instruction, at
/// `BELOW_NORMAL_PRIORITY_CLASS` when `Spawn::below_normal`. A kill is `TerminateJobObject(job, 1)` and ends the
/// child's whole tree; so does the engine's end, since closing a job's last handle kills what it holds (design part
/// 3.8). Standard input null, output and error piped, `Spawn::env` added (Rulings T9-a to T9-e hold).
/// Def: T3-5. Use: T3-23.
pub struct JobProcessGuard;
impl ProcessGuard for JobProcessGuard { fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>>; }
/// One Job Object; dropping the last `Arc` of it closes the handle and so kills what the job still holds.
struct Job(OwnedHandle);
impl Job {
    fn new() -> io::Result<Job>;                          // CreateJobObjectW, then the kill-on-close limit
    fn assign(&self, child: &Child) -> io::Result<()>;    // AssignProcessToJobObject
    fn terminate(&self) -> io::Result<()>;                // TerminateJobObject(job, 1)
}
/// The suspended child's main thread resumed.
fn resume(child: &Child) -> io::Result<()>;
/// One child of `JobProcessGuard`; its `KillHandle`s hold the same `Arc<Job>` (Ruling T3-5-a).
struct JobChild { child: Child, job: Arc<Job>, stdout: Option<ChildStdout>, stderr: Option<ChildStderr> } // impl GuardedChild

// crates/platform/src/process.rs
pub(crate) fn handed_over<R: Read + Send + 'static>(pipe: Option<R>) -> Box<dyn Read + Send>; // was private (T9-d)
#[cfg(test)]
pub(crate) mod tests {
    /// The arguments that make this test binary run only `helper_child`.
    pub(crate) const HELPER_ARGS: [&str; 4] = ["--exact", "process::tests::helper_child", "--ignored", "--nocapture"];
    pub(crate) fn helper(mode: &str) -> Spawn;                        // unchanged but for HELPER_ARGS
    pub(crate) fn check_kills_its_child(guard: &dyn ProcessGuard);     // PR 2's the_runner_kills_its_child body
    pub(crate) fn check_pipes_both_streams_and_adds_env(guard: &dyn ProcessGuard); // PR 2's pipes test body
}
```

Settled in this task:
- **Ruling T3-5-a (the job's life).** The job handle is shared by the child and every `KillHandle` made from it, so a kill works after the child is dropped, and the tree is killed at the latest when the last of them is dropped. `run_recorded` holds both only for the run (record.rs:253-254), so a tool's leftover descendants end with the run.
- **Ruling T3-5-b (resuming the child).** `resume` finds the suspended child's one thread through Toolhelp, with features Task 1 lists: `CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)`, the `THREADENTRY32` whose `th32OwnerProcessID` is `child.id()` (a process created suspended has exactly one thread), `OpenThread(THREAD_SUSPEND_RESUME, FALSE, th32ThreadID)` into an `OwnedHandle`, then `ResumeThread`. `ChildExt::main_thread_handle` is not used: it is unstable (rust-lang/rust#96723), and the workspace pins a stable toolchain.
- **Ruling T3-5-c (the helper's lines).** The helper modes write `\nchild <pid>\n` and `\ngrandchild <pid>\n`, flushed: the leading newline keeps the line apart from libtest's `test ... ` prefix, which shares the line when the child's test harness runs single-threaded.

No test in this task needs the owner's desktop session: they run over SSH through `scripts/box.sh` and on `windows-latest`.

- [ ] **Step 1: Write the failing tests**

In `crates/platform/src/process.rs`, `mod tests` becomes `#[cfg(test)] pub(crate) mod tests` and gains `use std::process::{Command, Stdio};`, `helper` uses `HELPER_ARGS`, the bodies of `the_runner_kills_its_child` and `the_runner_pipes_the_childs_output_and_passes_its_environment` move unchanged (with `guard.spawn(..)` for `StdProcessGuard.spawn(..)`) into `check_kills_its_child` and `check_pipes_both_streams_and_adds_env`, those two tests call them with `&StdProcessGuard`, and `helper_child` gains two arms (its doc lists the four modes):

```rust
            Ok("spawn-grandchild") => {
                // The first act: the grandchild starts before this child writes anything (Review Focus 3).
                let _grandchild = Command::new(std::env::current_exe().unwrap())
                    .args(HELPER_ARGS)
                    .env("LAZYMKV_TEST_HELPER", "print-pid-sleep")
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap();
                say(&format!("child {}", std::process::id()));
                std::thread::sleep(Duration::from_secs(60));
            }
            Ok("print-pid-sleep") => {
                say(&format!("grandchild {}", std::process::id()));
                std::thread::sleep(Duration::from_secs(60));
            }
```

with, in the same module:

```rust
    /// Writes `line` on a line of its own to standard output and flushes it (Ruling T3-5-c).
    fn say(line: &str) {
        let mut out = std::io::stdout();
        out.write_all(format!("\n{line}\n").as_bytes()).unwrap();
        out.flush().unwrap();
    }
```

The grandchild's standard output and error are the child's, inherited, so it holds the child's pipes as a real tool's helper would.

Add `mod job;` to `win/mod.rs`, so the crate compiles the tests. In `crates/platform/src/win/job.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, BufReader, Read};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetPriorityClass, OpenProcess, WaitForSingleObject, BELOW_NORMAL_PRIORITY_CLASS,
        IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

    use super::JobProcessGuard;
    use crate::process::tests::{check_kills_its_child, check_pipes_both_streams_and_adds_env, helper};
    use crate::testutil::{ci_factor, temp_dir};
    use crate::{GuardedChild, ProcessGuard, Spawn};

    /// How long a killed tree may take to end. A hang is the failure it catches, so it does not scale (Global
    /// Constraints).
    const ENDS_WITHIN: Duration = Duration::from_secs(10);

    /// A handle to the running process `pid`, opened while it runs, so a reused id cannot answer for it.
    fn open_process(pid: u32) -> OwnedHandle {
        // SAFETY: OpenProcess takes no pointer; a null result is checked before the handle is owned.
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        assert!(!raw.is_null(), "process {pid} could not be opened: {}", io::Error::last_os_error());
        // SAFETY: raw is an open handle that nothing else owns.
        unsafe { OwnedHandle::from_raw_handle(raw) }
    }

    /// Whether `process` ended within `bound`.
    fn ended_within(process: &OwnedHandle, bound: Duration) -> bool {
        // SAFETY: the handle is open for the whole call.
        unsafe { WaitForSingleObject(process.as_raw_handle(), bound.as_millis() as u32) == WAIT_OBJECT_0 }
    }

    /// `process`'s priority class.
    fn priority(process: &OwnedHandle) -> u32 {
        // SAFETY: the handle is open and has PROCESS_QUERY_LIMITED_INFORMATION.
        unsafe { GetPriorityClass(process.as_raw_handle()) }
    }

    /// `child`'s standard output read on a thread: each line, then None at the end of file.
    fn lines_of(child: &mut dyn GuardedChild) -> mpsc::Receiver<Option<String>> {
        let reader = BufReader::new(child.take_stdout());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(Some(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(None);
        });
        rx
    }

    /// The process ids the `spawn-grandchild` helper writes, as (child, grandchild), within 10 s (scaled).
    fn pids(lines: &mpsc::Receiver<Option<String>>) -> (u32, u32) {
        let deadline = Instant::now() + Duration::from_secs(10) * ci_factor();
        let (mut child, mut grandchild) = (None, None);
        while child.is_none() || grandchild.is_none() {
            match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Some(line)) => {
                    if let Some(pid) = line.strip_prefix("child ") {
                        child = pid.trim().parse().ok();
                    }
                    if let Some(pid) = line.strip_prefix("grandchild ") {
                        grandchild = pid.trim().parse().ok();
                    }
                }
                other => panic!("the helper's process ids did not come: {other:?}"),
            }
        }
        (child.unwrap(), grandchild.unwrap())
    }

    /// Whether the reader of `lines_of` reached the end of file within `bound`.
    fn end_of_file_within(lines: &mpsc::Receiver<Option<String>>, bound: Duration) -> bool {
        let deadline = Instant::now() + bound;
        loop {
            match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => return true,
                Ok(Some(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => return false,
            }
        }
    }

    /// `spawn` under the job runner, its output read on a thread, and handles to the child and the grandchild the
    /// `spawn-grandchild` helper names, opened while both run.
    fn start_tree(spawn: &Spawn) -> (Box<dyn GuardedChild>, mpsc::Receiver<Option<String>>, [OwnedHandle; 2]) {
        let mut child = JobProcessGuard.spawn(spawn).unwrap();
        let lines = lines_of(&mut *child);
        let (parent, grandchild) = pids(&lines);
        (child, lines, [open_process(parent), open_process(grandchild)])
    }

    #[test]
    fn the_job_runner_kills_its_child() {
        check_kills_its_child(&JobProcessGuard);
    }

    #[test]
    fn the_job_runner_pipes_both_streams_and_adds_env() {
        check_pipes_both_streams_and_adds_env(&JobProcessGuard);
    }

    /// Review Focus 3: the helper's first act starts the grandchild, so a job assigned after the child ran would miss it.
    #[test]
    fn a_grandchild_started_at_once_is_killed_with_its_parent() {
        let (mut child, lines, [parent, grandchild]) = start_tree(&helper("spawn-grandchild"));
        child.killer().kill().unwrap();
        assert!(ended_within(&parent, ENDS_WITHIN), "the child outlived the kill");
        assert!(ended_within(&grandchild, ENDS_WITHIN), "the grandchild outlived the kill");
        assert!(end_of_file_within(&lines, ENDS_WITHIN), "standard output stayed open after the kill");
        assert_ne!(child.wait().unwrap(), Some(0));
    }

    /// Ruling T9-b, and Ruling T3-5-a's kill handle that outlives its child.
    #[test]
    fn kill_after_exit_is_ok() {
        let mut child = JobProcessGuard.spawn(&helper("echo")).unwrap();
        let killer = child.killer();
        io::copy(&mut child.take_stdout(), &mut io::sink()).unwrap();
        assert_eq!(child.wait().unwrap(), Some(0));
        assert!(killer.kill().is_ok() && killer.kill().is_ok(), "a kill after the exit is no error");
        drop(child);
        assert!(killer.kill().is_ok(), "nor is a kill from a handle that outlived its child");
    }

    /// Ruling T9-a: `wait` holds no lock a kill needs.
    #[test]
    fn a_kill_from_another_thread_gets_through_while_wait_runs() {
        let mut child = JobProcessGuard.spawn(&helper("sleep")).unwrap();
        let killer = child.killer();
        let (tx, rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || tx.send(child.wait().map_err(|error| error.kind())).unwrap());
        std::thread::sleep(Duration::from_millis(300));
        killer.kill().unwrap();
        let status = rx.recv_timeout(ENDS_WITHIN).expect("wait did not return after the kill");
        assert_ne!(status, Ok(Some(0)));
        waiter.join().unwrap();
    }

    /// Ruling T9-d, for both streams.
    #[test]
    fn the_streams_are_given_once_then_empty() {
        let mut child = JobProcessGuard.spawn(&helper("echo")).unwrap();
        let (mut out, mut err) = (String::new(), String::new());
        child.take_stdout().read_to_string(&mut out).unwrap();
        child.take_stderr().read_to_string(&mut err).unwrap();
        assert_eq!(child.wait().unwrap(), Some(0));
        assert!(out.contains("helper-out\n") && err.contains("helper-err\n"), "{out:?} {err:?}");
        let (mut out, mut err) = (String::new(), String::new());
        child.take_stdout().read_to_string(&mut out).unwrap();
        child.take_stderr().read_to_string(&mut err).unwrap();
        assert_eq!((out.as_str(), err.as_str()), ("", ""));
    }

    /// Spec 4.3 and design part 5.3: `mkvmerge`, `ffprobe`, `ffmpeg` and the optical character recognition (OCR) worker
    /// run below normal, and what they start inherits it. Without the flag the child gets the class Windows gives any
    /// child of this process: normal, unless this process itself runs idle or below normal.
    #[test]
    fn below_normal_is_the_childs_priority_class() {
        // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing; GetPriorityClass only reads it.
        let own = unsafe { GetPriorityClass(GetCurrentProcess()) };
        let inherited =
            if own == IDLE_PRIORITY_CLASS || own == BELOW_NORMAL_PRIORITY_CLASS { own } else { NORMAL_PRIORITY_CLASS };
        for (below_normal, expected) in [(true, BELOW_NORMAL_PRIORITY_CLASS), (false, inherited)] {
            let (mut child, _lines, [parent, grandchild]) =
                start_tree(&Spawn { below_normal, ..helper("spawn-grandchild") });
            assert_eq!(priority(&parent), expected, "the child, below_normal {below_normal}");
            if below_normal {
                assert_eq!(priority(&grandchild), BELOW_NORMAL_PRIORITY_CLASS, "a below-normal child's grandchild");
            }
            child.killer().kill().unwrap();
            child.wait().unwrap();
        }
    }

    /// The engine runs several tools at once and the harness tests run in parallel in one process.
    #[test]
    fn killing_one_childs_tree_leaves_another_guarded_child_running() {
        let (mut first, _first_lines, first_tree) = start_tree(&helper("spawn-grandchild"));
        let (mut second, _second_lines, second_tree) = start_tree(&helper("spawn-grandchild"));
        first.killer().kill().unwrap();
        assert!(first_tree.iter().all(|process| ended_within(process, ENDS_WITHIN)), "the first tree outlived its kill");
        assert!(
            second_tree.iter().all(|process| !ended_within(process, Duration::ZERO)),
            "the second tree ended with the first"
        );
        second.killer().kill().unwrap();
        assert!(second_tree.iter().all(|process| ended_within(process, ENDS_WITHIN)));
        first.wait().unwrap();
        second.wait().unwrap();
    }

    /// Kill-on-close: the guarded child dropped, with no kill handle taken, closes the job, as the engine's end does.
    #[test]
    fn dropping_the_guarded_child_ends_its_tree() {
        let (child, lines, tree) = start_tree(&helper("spawn-grandchild"));
        drop(child);
        assert!(tree.iter().all(|process| ended_within(process, ENDS_WITHIN)), "kill-on-close did not end the tree");
        assert!(end_of_file_within(&lines, ENDS_WITHIN));
    }

    #[test]
    fn a_program_that_cannot_start_is_the_os_error() {
        let program = temp_dir("job-missing").join("absent.exe");
        let spawn = Spawn { program, args: Vec::new(), env: Vec::new(), below_normal: false };
        let error = JobProcessGuard.spawn(&spawn).err().expect("an absent program does not start");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
```

In `crates/fake-tools/tests/tools_record.rs`, after `a_program_that_cannot_start_is_an_error_and_writes_no_files`:

```rust
/// engine.md:579 (design part 4.2): on Windows the engine's runner kills a tool's whole tree, so a grandchild that
/// inherited the tool's standard output cannot hold `run_recorded`'s reader open after the time limit's kill. The tool
/// is `cmd.exe` and the grandchild the `ping` it starts, which would hold the pipe for about 60 s. The 15 s bound
/// catches a hang, so it does not scale.
#[cfg(windows)]
#[test]
fn a_time_limit_ends_a_tool_whose_grandchild_holds_its_output() {
    let dir = Scratch::new("rec-tree");
    let cmd = PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot is set")).join(r"System32\cmd.exe");
    let out = dir.0.join("tree.txt");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let guard = lazymkv_platform::native_guard();
        let host = FakeHost::new("2026-10-23", NOW);
        let limits = Limits::from_settings(&Settings::default());
        let ctx = ToolCtx {
            guard: &*guard,
            host: &host,
            limits: &limits,
            cancel: CancelToken::default(),
            on_output: &|_: OutputStream, _: &str| {},
            on_progress: &|_: f64| {},
        };
        let args = ["/c", "ping", "-n", "60", "127.0.0.1"].map(String::from);
        let run = run_recorded(&ctx, &cmd, &args, &out, Some(Duration::from_secs(1)), false, &mut |_| LineAction::Continue);
        tx.send(run.map(|run| run.timed_out).map_err(|error| error.to_string())).unwrap();
    });
    let timed_out = rx.recv_timeout(Duration::from_secs(15)).expect("run_recorded did not return within 15 s of a 1 s limit");
    assert_eq!(timed_out, Ok(true));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: a compile error, ``unresolved import `super::JobProcessGuard` ``.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test tools_record`
Expected: a compile error, ``cannot find function `native_guard` in crate `lazymkv_platform` ``.

- [ ] **Step 3: Implement**

- `JobProcessGuard::spawn`, in this order, which is what keeps a grandchild inside the job (W7). Every `unsafe` call has a `// SAFETY:` line, and every failure of a call is `io::Error::last_os_error()` read at once.
  1. `Job::new()`: `CreateJobObjectW(null, null)`, then `SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info, size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())` with `info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and every other field zero. Nothing has started if this fails.
  2. `Command::new(&spawn.program)` with `spawn.args`, `spawn.env`, standard input null, output and error piped (T9-e), and `creation_flags(CREATE_SUSPENDED | BELOW_NORMAL_PRIORITY_CLASS)` when `below_normal`, else `creation_flags(CREATE_SUSPENDED)`; `spawn()?`, its error returned unchanged (a missing program is `NotFound`) while the empty job closes. std's own spawn keeps its handle discipline, so a concurrent spawn never inherits this child's pipe ends.
  3. `job.assign(&child)`; on failure, `child.kill()` and `child.wait()`, then the error.
  4. `resume(&child)` (Ruling T3-5-b); a failed call, or a `ResumeThread` return of `u32::MAX`, is a failure: `job.terminate()` and `child.wait()`, then the error.
  5. The two pipes taken out of the `Child`; `Ok(Box::new(JobChild { child, job: Arc::new(job), stdout, stderr }))`.
- `JobChild`: `take_stdout` and `take_stderr` are `handed_over` (T9-d); `killer` is `KillHandle(Arc::new(move || job.terminate()))` over a clone of the `Arc<Job>`; `wait` is `self.child.wait()` mapped to `ExitStatus::code()`, a blocking wait with no lock, since a kill goes through the job and never touches the `Child` (T9-a). `TerminateJobObject` on a job whose processes have all ended succeeds, so a kill after the exit is `Ok(())` (T9-b); a killed tree's exit code is 1, as std's kill gives.
- `win/mod.rs`: `pub use job::JobProcessGuard;`.
- `lib.rs`: `native_guard` as in the Interfaces block: `Arc::new(win::JobProcessGuard)` under `cfg(windows)`, `Arc::new(StdProcessGuard)` under `cfg(not(windows))`.
- `process.rs`'s module doc becomes: "The process runner on `std::process` (design part 4.2): the runner on Linux and macOS, and on Windows the one the driver tests in `crates/fake-tools/tests` keep (Ruling W7). It kills only the child it started, not the child's own children, and ignores `Spawn::below_normal` (S43). The engine on Windows takes `win::JobProcessGuard` through `native_guard`, which kills the tree." `handed_over` becomes `pub(crate)`.
- `testkit.rs`: the import, `TestGuard`'s `inner` field and its last line, and `Runner::scripted`, as the Files list says. `TestGuard`'s doc ends: "Every spawn then goes to `native_guard()` through `retry_busy` (Ruling R29.c4): the Job Object runner on Windows, so every Windows whole fake job runs it (Ruling W7), and `StdProcessGuard` elsewhere." `Runner::scripted`'s doc: "`new_scripted`'s: the script, `native_guard()`, a transport with no answers and no scenario."

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked`
Expected: PASS, with the ten `win::job::tests` and the two `process::tests` among them; `process::tests::helper_child` stays ignored.

Run: `scripts/box.sh cargo test -p fake-tools --locked`
Expected: PASS, with `a_time_limit_ends_a_tool_whose_grandchild_holds_its_output` among them; every Windows harness job now runs on the Job Object runner.

Then the workspace: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 5: Commit**

```
git add crates/platform/src/win/job.rs crates/platform/src/win/mod.rs crates/platform/src/process.rs crates/platform/src/lib.rs crates/engine/src/testkit.rs crates/fake-tools/tests/tools_record.rs
git commit -m "platform: the Job Object runner kills the tree; Windows harness jobs use it (design part 4.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 6: `KeepAwake` on the thread execution state (design part 4.2; W9)

**Files:**
- Create: `crates/platform/src/win/awake.rs` (`WinKeepAwake`; 4 tests in its `mod tests`).
- Modify: `crates/platform/src/win/mod.rs` (`mod awake; pub use awake::WinKeepAwake;`).

**Interfaces:**
- Consumes: `KeepAwake` (lib.rs:98-105); Task 1's `windows_sys` (`Win32_System_Power`, `Win32_Foundation`); Task 2's `testutil::skip`.
- Produces:

```rust
// crates/platform/src/win/awake.rs (cfg(windows))
/// The Windows `KeepAwake` (spec 4.3, design part 4.2; Ruling W9). A thread of its own, `lazymkv-keep-awake`, sets the
/// thread execution state: `ES_CONTINUOUS | ES_SYSTEM_REQUIRED` when the holders go from 0 to 1, `ES_CONTINUOUS` when
/// they go back to 0. The state belongs to the thread that set it, so it never depends on which thread calls `acquire`
/// or `release`, or on that thread living on. Calls nest; a release with no holder stays at zero (Ruling T8-g).
/// `acquire` and `release` return once the thread has applied a change (Ruling T3-6-a). Dropping it lets the state go
/// and ends the thread. Def: T3-6. Use: T3-23.
pub struct WinKeepAwake { holders: Mutex<Holders>, thread: Option<JoinHandle<()>> }
/// The count and the way to the thread, under one lock, so changes reach the thread in the order they were made.
struct Holders { count: usize, to_thread: Option<mpsc::Sender<(bool, mpsc::Sender<()>)>> } // (hold, done)
impl WinKeepAwake {
    /// Starts the keeper's thread. Err when the thread cannot start (design part 4.2). Def: T3-6. Use: T3-23.
    pub fn new() -> io::Result<WinKeepAwake>;
    /// How many `acquire` calls have no `release` yet (design part 4.2). Def: T3-6. Use: T3-6.
    pub fn holders(&self) -> usize;
}
impl KeepAwake for WinKeepAwake { fn acquire(&self); fn release(&self); }
impl Drop for WinKeepAwake { fn drop(&mut self); }
```

Settled in this task:
- **Ruling T3-6-a (a change is applied before the call returns).** The count lives under the caller's lock, so `holders()` is exact at once, and a 0-to-1 or 1-to-0 change waits for the thread's reply on `done`, holding the lock, so the system state matches the count when `acquire` or `release` returns. The wait is one `SetThreadExecutionState` call on the other thread; the thread never takes the lock, so it cannot deadlock. A send or a reply that fails, the thread being gone, is ignored: the trait gives `acquire` and `release` no error to return.

No test in this task needs the owner's desktop session: the execution state is per thread, not per session, so the tests run over SSH through `scripts/box.sh` and on `windows-latest`. A runner never sleeps, so the tests read the state back with `CallNtPowerInformation(SystemExecutionState)`; they do not observe sleep.

- [ ] **Step 1: Write the failing tests**

Add `mod awake;` to `win/mod.rs`, so the crate compiles the tests. In `crates/platform/src/win/awake.rs`:

```rust
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

    #[test]
    fn the_system_state_is_required_while_a_holder_remains() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let keeper = WinKeepAwake::new().unwrap();
        keeper.acquire();
        keeper.acquire();
        keeper.release();
        assert_eq!(keeper.holders(), 1);
        assert!(system_required());
        keeper.release();
        assert_eq!(keeper.holders(), 0);
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::awake`
Expected: a compile error, ``unresolved import `super::WinKeepAwake` ``.

- [ ] **Step 3: Implement**

- `WinKeepAwake::new`: an `mpsc::channel::<(bool, mpsc::Sender<()>)>()`; `std::thread::Builder::new().name("lazymkv-keep-awake".into()).spawn(..)?` of a loop that, for each `(hold, done)` received, calls `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)` when `hold` and `SetThreadExecutionState(ES_CONTINUOUS)` otherwise, then sends on `done`; when the channel closes it calls `SetThreadExecutionState(ES_CONTINUOUS)` and returns. The call's result (the previous state, or 0 on failure) is not read: design part 4.2 names no report and the trait no error.
- `acquire`: under the lock, `count += 1`; when it became 1, send `(true, done)` and wait on `done`'s receiver, still under the lock (Ruling T3-6-a).
- `release`: under the lock, nothing when `count` is 0 (T8-g); else `count -= 1`, and when it became 0, send `(false, done)` and wait as `acquire` does.
- `holders`: `count` under the lock.
- `Drop`: take `to_thread` out of the lock and drop it, so the thread sets `ES_CONTINUOUS` and returns, then join the thread.
- Every lock is taken with `unwrap_or_else(PoisonError::into_inner)`, as the fakes do, so a test's panic does not poison the next call. The two `SetThreadExecutionState` calls have `// SAFETY:` lines (they take flags only).
- `win/mod.rs`: `pub use awake::WinKeepAwake;`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::awake`
Expected: PASS, `4 passed`. On the mini PC, `dropping_the_keeper_ends_its_thread` prints its `skipped:` line for the read-back half only while another process holds the state.

Then the workspace: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 5: Commit**

```
git add crates/platform/src/win/awake.rs crates/platform/src/win/mod.rs
git commit -m "platform: KeepAwake on the thread execution state, from one thread (design part 4.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 7: `Host` additions and `WinHost` (design parts 3.6, 4.2, 4.6, 7.2)

**Files:**
- Create: `crates/platform/src/localtime.rs` (all OS; 5 tests in its `mod tests`).
- Create: `crates/platform/src/win/host.rs` (compiled only under `cfg(windows)`, through `lib.rs`'s `#[cfg(windows)] pub mod win;`; 10 tests in its `mod tests`).
- Modify: `crates/platform/src/lib.rs:12-16` (`pub mod localtime;` beside the module lines of Tasks 1 and 2) and `:179-197` (the `Host` doc and its three new methods).
- Modify: `crates/platform/src/win/mod.rs` (`mod host; pub use host::WinHost;` and the constants `MAXIMUM_ALLOWED` and `SE_GROUP_INTEGRITY`, Step 3).
- Modify: `crates/platform/src/fakes.rs:5` (`BTreeSet` imported), `:404-478` (`HostState.writable`, the `FakeHost` doc, `set_writable`, the three methods) and `:641-646` (two tests after `the_fake_host_gives_a_fixed_local_date`).
- Modify: `crates/tools/src/lookup.rs:98-108` (`TestHost`) and `crates/tools/src/makemkv.rs:625-635` (`KeyHost`): the three methods, so the tools crate's tests compile. No other `Host` implementation exists in the workspace.

**Interfaces:**
- Consumes: `Host` (lib.rs:182-197); `testutil::temp_dir` (Task 2); Task 1's `windows_sys` features `Win32_Foundation`, `Win32_Security`, `Win32_Security_Authorization`, `Win32_Storage_FileSystem`, `Win32_System_Diagnostics_ToolHelp`, `Win32_System_Registry`, `Win32_System_SystemInformation`, `Win32_System_Threading`, `Win32_System_Time`.
- Produces:

```rust
// crates/platform/src/lib.rs. The seven PR 2 methods are unchanged.
/// What the engine asks of the machine it runs on (spec 4.3, design part 4.2): the clock, the calendar date and the
/// local time, MakeMKV's registration key and debug log, the running programs, a disc's label, where each tool is
/// installed by default, and whether a non-elevated process of the user could write a path.
/// Def: T8; T3-7 adds now_ms, local_time and user_writable. Use: T11, T22, T3-8, T3-9, T3-22, T3-23.
pub trait Host: Send + Sync {
    /// The current time in milliseconds since the Unix epoch (R23.7, Ruling W16).
    fn now_ms(&self) -> u64;
    /// The instant `unix_ms` as local time, `yyyy-MM-dd HH:mm:ss.fff`, under that instant's daylight-saving rule: the
    /// form phase 0 wrote `started` in (design part 4.6, R23.7; scripts/phase0/rip-title.ps1:103).
    fn local_time(&self, unix_ms: u64) -> String;
    /// Whether a non-elevated process of the user could write `path` (design part 3.6, Ruling W17): for a folder, add a
    /// file or a folder, delete a child, delete it, or change its access list or owner; for a file, write, append,
    /// delete it, or change its access list or owner. True when it cannot be told.
    fn user_writable(&self, path: &Path) -> bool;
}

// crates/platform/src/localtime.rs (all OS)
//! Phase 0's local-time text (design part 4.6, R23.7), from Unix milliseconds and an offset from Coordinated
//! Universal Time (UTC), with no date-time crate.
/// `unix_ms` as `yyyy-MM-dd HH:mm:ss.fff` at `offset_minutes` ahead of UTC (negative: behind), in the proleptic
/// Gregorian calendar (design part 4.6, R23.7). Def: T3-7. Use: T3-7.
pub fn format_local_time(unix_ms: u64, offset_minutes: i32) -> String;

// crates/platform/src/fakes.rs. FakeHost's doc gains: "Its clock in milliseconds is `now_unix` times 1000, its local
// time is that clock read as UTC, and no path is user-writable until a test sets one."
impl FakeHost {
    /// Makes `user_writable` answer true for exactly `path` (design part 3.6). Def: T3-7. Use: T3-22.
    pub fn set_writable(&self, path: &Path);
}
// impl Host for FakeHost: now_ms = now_unix * 1000; local_time(ms) = format_local_time(ms, 0);
// user_writable(path) = a path set_writable named, compared exactly.

// crates/platform/src/win/host.rs (cfg(windows))
/// The engine's `Host` on Windows (design part 4.2's helpers outside the traits; parts 3.6, 4.6, 7.2; Rulings W17,
/// W18): the system clock, the local time from the time-zone rules, MakeMKV's key from the registry, a Toolhelp
/// process snapshot, the volume label, the Program Files install paths, and the writable test with the user's
/// filtered token. It holds no handle between calls. Def: T3-7. Use: T3-23.
pub struct WinHost;
impl WinHost {
    /// A host over this machine (design part 4.2). Def: T3-7. Use: T3-23.
    pub fn new() -> WinHost;
}
impl Host for WinHost { /* the ten methods */ }
```

- [ ] **Step 1: Write the failing tests**

`crates/platform/src/localtime.rs`, holding only its tests for now, with `pub mod localtime;` added to `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::format_local_time;

    #[test]
    fn the_epoch_formats_with_three_digit_milliseconds() {
        assert_eq!(format_local_time(0, 0), "1970-01-01 00:00:00.000");
    }

    #[test]
    fn a_leap_day_and_a_negative_offset_roll_back_a_day() {
        // 2024-03-01 00:59:59.999 UTC, read one hour behind UTC.
        assert_eq!(format_local_time(1_709_254_799_999, -60), "2024-02-29 23:59:59.999");
    }

    #[test]
    fn milliseconds_are_padded() {
        assert_eq!(format_local_time(7, 0), "1970-01-01 00:00:00.007");
    }

    #[test]
    fn the_century_rules_make_2000_a_leap_year_and_2100_not() {
        assert_eq!(format_local_time(951_782_400_000, 0), "2000-02-29 00:00:00.000");
        assert_eq!(format_local_time(4_107_542_399_999, 0), "2100-02-28 23:59:59.999");
        assert_eq!(format_local_time(4_107_542_400_000, 0), "2100-03-01 00:00:00.000");
    }

    #[test]
    fn a_negative_offset_at_the_epoch_gives_1969() {
        assert_eq!(format_local_time(0, -60), "1969-12-31 23:00:00.000");
    }
}
```

In `crates/platform/src/fakes.rs`'s `mod tests`, after `the_fake_host_gives_a_fixed_local_date`:

```rust
    #[test]
    fn the_fake_hosts_local_time_is_utc_of_its_clock() {
        let host = FakeHost::new("2026-10-23", 1_792_800_000);
        assert_eq!(host.now_ms(), 1_792_800_000_000);
        assert_eq!(host.local_time(host.now_ms()), "2026-10-24 00:00:00.000");
        assert_eq!(host.local_time(1_792_800_000_123), "2026-10-24 00:00:00.123");
        // The date is a setting of the fake, so it differs from its clock's UTC date on purpose.
        assert_eq!(host.local_date(), "2026-10-23");
    }

    #[test]
    fn a_path_is_writable_only_when_set() {
        let host = FakeHost::new("2026-10-23", 1_800_000_000);
        assert!(!host.user_writable(Path::new("/tools")));
        host.set_writable(Path::new("/tools"));
        assert!(host.user_writable(Path::new("/tools")));
        assert!(!host.user_writable(Path::new("/tools/mkvmerge")));
        assert!(!host.user_writable(Path::new("/other")));
    }
```

`crates/platform/src/win/host.rs`, holding only its tests for now, with `mod host;` added to `win/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use lazymkv_core::job::Tool;
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    use windows_sys::Win32::System::Time::SystemTimeToFileTime;

    use super::WinHost;
    use crate::testutil::temp_dir;
    use crate::Host;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    /// `local_time`'s text back into its fields.
    fn fields(text: &str) -> SYSTEMTIME {
        let at = |from: usize, to: usize| text[from..to].parse::<u16>().unwrap();
        SYSTEMTIME { wYear: at(0, 4), wMonth: at(5, 7), wDayOfWeek: 0, wDay: at(8, 10), wHour: at(11, 13),
            wMinute: at(14, 16), wSecond: at(17, 19), wMilliseconds: at(20, 23) }
    }

    /// A local SYSTEMTIME as 100-nanosecond ticks, so two local readings can be subtracted.
    fn ticks(time: &SYSTEMTIME) -> i64 {
        let mut file = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        // SAFETY: both pointers are to live locals of the declared types.
        assert_ne!(unsafe { SystemTimeToFileTime(time, &mut file) }, 0);
        (i64::from(file.dwHighDateTime) << 32) | i64::from(file.dwLowDateTime)
    }

    fn local_now() -> SYSTEMTIME {
        let mut time = fields("1601-01-01 00:00:00.000");
        // SAFETY: GetLocalTime writes the struct it is given.
        unsafe { GetLocalTime(&mut time) };
        time
    }

    /// The byte count of `HKCU\Software\MakeMKV\app_Key`, read without its data.
    fn app_key_bytes() -> Option<u32> {
        let (key, value) = (wide(r"Software\MakeMKV"), wide("app_Key"));
        let mut bytes = 0u32;
        // SAFETY: a size query: no type or data pointer; `bytes` receives the count.
        let status = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(),
                std::ptr::null_mut(), &mut bytes)
        };
        (status == 0).then_some(bytes)
    }

    #[test]
    fn local_time_matches_get_local_time_within_a_second() {
        let host = WinHost::new();
        let ours = host.local_time(host.now_ms());
        let windows = local_now();
        assert_eq!(ours.len(), 23, "{ours}");
        // GetLocalTime moves at the timer tick and now_ms at the precise clock, so they are compared by distance.
        let apart_ms = (ticks(&fields(&ours)) - ticks(&windows)).abs() / 10_000;
        assert!(apart_ms < 1_000, "{ours} is {apart_ms} ms from GetLocalTime");
    }

    #[test]
    fn local_date_is_the_first_ten_characters_of_local_time() {
        let host = WinHost::new();
        let (first, time, last) = (host.local_date(), host.local_time(host.now_ms()), host.local_date());
        assert_eq!(first.len(), 10, "{first}");
        // Equal dates around the read mean no midnight passed between them.
        if first == last {
            assert_eq!(&time[..10], first);
        }
    }

    #[test]
    fn the_process_list_holds_this_test_binary() {
        let exe = std::env::current_exe().unwrap();
        let own = exe.file_stem().unwrap().to_string_lossy().to_lowercase();
        let names: Vec<String> = WinHost::new()
            .running_processes()
            .iter()
            .map(|name| name.to_lowercase().trim_end_matches(".exe").to_string())
            .collect();
        // Only the count is printed: the list is the machine's.
        assert!(names.contains(&own), "{own} is not among {} names", names.len());
    }

    #[test]
    fn the_system_drive_has_a_label_or_none_without_error() {
        let host = WinHost::new();
        let drive = std::env::var("SystemDrive").unwrap();
        // Some: the volume answered (its label may be empty). None: the path is not a volume root.
        assert!(host.volume_label(&PathBuf::from(format!(r"{drive}\"))).is_some());
        assert!(host.volume_label(&PathBuf::from(&drive)).is_some());
        assert_eq!(host.volume_label(&temp_dir("host-label").join("absent")), None);
    }

    #[test]
    fn the_debug_log_is_makemkv_log_txt_in_the_profile() {
        let profile = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        // assert!, not assert_eq!: a failure would print the profile path, which holds the account name.
        assert!(
            WinHost::new().makemkv_debug_log() == Some(profile.join("MakeMKV_log.txt")),
            "the debug log is not MakeMKV_log.txt in the profile"
        );
    }

    #[test]
    fn the_app_key_is_read_without_being_printed() {
        // More than the terminating NUL means a key is set. Only the two booleans meet the assertion, so a failure
        // prints no key (S61, design part 7.2).
        let set = app_key_bytes().is_some_and(|bytes| bytes > 2);
        assert_eq!(WinHost::new().makemkv_app_key().is_some(), set);
    }

    #[test]
    fn default_install_paths_are_under_program_files_and_none_for_ffprobe() {
        let host = WinHost::new();
        let x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
        let plain = std::env::var_os("ProgramFiles").map(PathBuf::from);
        assert_eq!(host.default_install_path(Tool::Makemkvcon), x86.map(|p| p.join(r"MakeMKV\makemkvcon64.exe")));
        assert_eq!(host.default_install_path(Tool::Mkvmerge), plain.map(|p| p.join(r"MKVToolNix\mkvmerge.exe")));
        for tool in [Tool::Ffprobe, Tool::Ffmpeg, Tool::Java] {
            assert_eq!(host.default_install_path(tool), None, "{tool:?}");
        }
    }

    #[test]
    fn a_fresh_temp_folder_is_user_writable() {
        let dir = temp_dir("host-writable");
        let file = dir.join("tool.exe");
        fs::write(&file, b"x").unwrap();
        let host = WinHost::new();
        assert!(host.user_writable(&dir));
        assert!(host.user_writable(&file));
    }

    #[test]
    fn the_windows_folder_is_not_user_writable() {
        let system32 = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
        let host = WinHost::new();
        assert!(!host.user_writable(&system32));
        assert!(!host.user_writable(&system32.join("cmd.exe")));
    }

    #[test]
    fn a_path_that_cannot_be_read_counts_as_writable() {
        // Ruling W17: any failure answers true, so a lookup never trusts what it could not judge.
        assert!(WinHost::new().user_writable(&temp_dir("host-absent").join("absent")));
    }
}
```

No test here needs the owner's desktop session: the `win::host` tests run over Secure Shell (SSH) through `scripts/box.sh` and on `windows-latest`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform -p lazymkv-tools --features lazymkv-platform/fakes --locked`
Expected: compile errors: ``unresolved import `super::format_local_time` `` (E0432), ``no method named `now_ms` found for struct `FakeHost` `` (also `local_time`, `user_writable`, `set_writable`) and ``unresolved import `super::WinHost` ``.

- [ ] **Step 3: Implement**

- `Host` gains the three methods of the Interfaces block, with their docs; the trait's doc is replaced by the one above.
- `format_local_time`: the local instant's day by `div_euclid` and its time of day by `rem_euclid`; the date by civil-from-days over 400-year eras (Howard Hinnant's algorithm), no crate.
- `FakeHost`: `HostState` gains `writable: BTreeSet<PathBuf>`; `set_writable` inserts `path.to_path_buf()`; the three methods as the Interfaces block's comment says.
- `TestHost` and `KeyHost`: `fn now_ms(&self) -> u64 { 0 }`, `fn local_time(&self, _: u64) -> String { String::new() }`, `fn user_writable(&self, _: &Path) -> bool { false }`.
- `win/mod.rs`: `mod host; pub use host::WinHost;`.
- `WinHost`, one approach per method. Every handle sits in `std::os::windows::io::OwnedHandle`, every security descriptor and security identifier (SID) that Windows allocates is freed with `LocalFree`, and every `unsafe` block has a `// SAFETY:` line.
  - `now_ms`: `SystemTime::now().duration_since(UNIX_EPOCH)` in milliseconds; `now_unix` is `now_ms() / 1000`.
  - `local_time(ms)`: the instant as `FILETIME` ticks `(ms + 11_644_473_600_000) * 10_000`, `FileTimeToSystemTime`, `SystemTimeToTzSpecificLocalTime(null, ..)` (the zone's rule for that instant), `SystemTimeToFileTime` of the result; the offset is the tick difference divided by 600_000_000, and the text is `format_local_time(ms, offset)`, so one formatter serves every host. A failed call formats with offset 0.
  - `local_date`: the first ten characters of `local_time(now_ms())`.
  - `makemkv_app_key`: `RegGetValueW(HKEY_CURRENT_USER, "Software\MakeMKV", "app_Key", RRF_RT_REG_SZ, ..)` once for the byte count and once into a `Vec<u16>` of that size; the text up to its first NUL; None on any error or an empty value. The value is never formatted into a log line, an error or a test message (S61, design part 7.2).
  - `makemkv_debug_log`: `%USERPROFILE%\MakeMKV_log.txt`; None when `USERPROFILE` is unset.
  - `running_processes`: `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)`, then `Process32FirstW` and `Process32NextW` with `dwSize` set, each `szExeFile` up to its NUL; an empty list when the snapshot fails, since the trait has no error (S55's check then finds no MakeMKV process).
  - `volume_label(root)`: `GetVolumeInformationW` on `root` with a `\` appended when it has none, a 261-unit name buffer, and null serial, component-length and flag pointers, so no drive serial is read; Some(the label, possibly empty) on success, None when the call fails.
  - `default_install_path`: `Makemkvcon` is `%ProgramFiles(x86)%\MakeMKV\makemkvcon64.exe` and `Mkvmerge` `%ProgramFiles%\MKVToolNix\mkvmerge.exe`, each None when its variable is unset (design part 3.6; W18 reads the folders, not the registry); `Ffprobe`, `Ffmpeg` and `Java` are None.
  - `user_writable(path)` (Ruling W17), true at the first failure of any call:
    1. `GetNamedSecurityInfoW(path, SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION, ..)`; `AccessCheck` needs the owner and the group, and the label makes it apply the no-write-up rule.
    2. The filtered token: `OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE)` and `GetTokenInformation(TokenElevationType)`. When it is `TokenElevationTypeFull`, `GetTokenInformation(TokenLinkedToken)`'s `LinkedToken`, which is already an identification-level impersonation token at medium integrity. Otherwise `CreateRestrictedToken(token, LUA_TOKEN, ..)` (LUA: limited user account; the Administrators group becomes deny-only), `DuplicateTokenEx(.., TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_IMPERSONATE, null, SecurityIdentification, TokenImpersonation)`, and `SetTokenInformation(TokenIntegrityLevel)` with a `TOKEN_MANDATORY_LABEL` of the SID `S-1-16-8192` (medium, from `ConvertStringSidToSidW`) and `SE_GROUP_INTEGRITY`.
    3. `AccessCheck(descriptor, token, MAXIMUM_ALLOWED, &GENERIC_MAPPING { GenericRead: FILE_GENERIC_READ, GenericWrite: FILE_GENERIC_WRITE, GenericExecute: FILE_GENERIC_EXECUTE, GenericAll: FILE_ALL_ACCESS }, ..)` with a 256-byte privilege-set buffer. `MAXIMUM_ALLOWED` returns every right granted, so the answer is "any write right", not "every write right".
    4. Writable when the access status is true and the granted mask meets `FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE | WRITE_DAC | WRITE_OWNER` for a folder (`path.is_dir()`) or `FILE_WRITE_DATA | FILE_APPEND_DATA | DELETE | WRITE_DAC | WRITE_OWNER` for a file.
  - `windows-sys` 0.61.2 declares `MAXIMUM_ALLOWED` and `SE_GROUP_INTEGRITY` only in `Win32::System::SystemServices`, which Task 1's list does not enable, so `win/mod.rs` declares `pub(crate) const MAXIMUM_ALLOWED: u32 = 0x0200_0000;` and `pub(crate) const SE_GROUP_INTEGRITY: u32 = 0x20;` (winnt.h's values; `SID_AND_ATTRIBUTES.Attributes` is a `u32`), each with a one-line doc naming winnt.h; no `Cargo.toml` changes (Global Constraints). Task 11's test uses `SE_GROUP_INTEGRITY` too.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform -p lazymkv-tools --features lazymkv-platform/fakes --locked`
Expected: PASS, with the five `localtime::tests`, the two new `fakes::tests` and the ten `win::host::tests` among them.

Then the workspace: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 5: Commit**

```
git add crates/platform/src/localtime.rs crates/platform/src/win/host.rs crates/platform/src/lib.rs crates/platform/src/win/mod.rs crates/platform/src/fakes.rs crates/tools/src/lookup.rs crates/tools/src/makemkv.rs
git commit -m "platform: Host gains local time and the writable test; WinHost (design parts 3.6, 4.6, 7.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 8: The tool lookup rules (S18; design part 3.6; engine.md:574, :580, :583; W18)

**Files:**
- Modify: `crates/tools/src/lookup.rs:1-2` (the module doc), `:49-89` (`find_tool`'s doc and body; the private `path_folders`), `:98-108` (`TestHost` gains its writable list) and its `mod tests` (6 new tests; `the_lookup_order_is_settings_resources_install_folder_path` renamed and extended; every `TestHost(x)` becomes `TestHost(x, vec![])`).

**Interfaces:**
- Consumes: `Host::user_writable` (Task 7); `std::env::consts::EXE_SUFFIX`.
- Produces: `find_tool` with W18's rules, signature unchanged; the private helper `path_folders`.

```rust
//! Finds where each tool is installed (design part 3.6): the settings, the resources folder, the tool's default
//! install folder, then `PATH`, skipping what a non-elevated process of the user could write (S18, Ruling W18).

/// Finds `tool` by the order of design part 3.6 and returns the first hit (S18; Ruling W18):
/// 1. the path in `settings.tool_paths`, taken as given even when a non-elevated process could write it; when that
///    path is not a file and `EXE_SUFFIX` is not empty, the path with `EXE_SUFFIX` appended, when that is a file
///    (engine.md:583);
/// 2. the tool's file name in `resources`, when that is a file and `host.user_writable(resources)` is false;
/// 3. `host.default_install_path(tool)`, when that is a file; it is not judged, since Windows' own access lists
///    protect the Program Files install folders (design part 3.6);
/// 4. the tool's file name in the first absolute folder of `path_var` that holds it as a file, where
///    `host.user_writable` calls neither the folder nor the file writable; relative and empty entries are dropped
///    (engine.md:580).
///
/// None when no source has it. Def: T11, T3-8. Use: T23, T26.
pub fn find_tool(tool: Tool, settings: &Settings, resources: Option<&Path>, host: &dyn Host,
    path_var: Option<&OsStr>) -> Option<ToolLocation>;

/// The absolute folders of `path_var`, in order; relative and empty entries are dropped (engine.md:580).
fn path_folders(path_var: Option<&OsStr>) -> Vec<PathBuf>;
```

- [ ] **Step 1: Write the failing tests**

In `crates/tools/src/lookup.rs`'s `mod tests`, `TestHost` and a helper become:

```rust
    /// A host with an optional default install path and the paths it calls user-writable, compared exactly.
    struct TestHost(Option<PathBuf>, Vec<PathBuf>);

    impl Host for TestHost {
        /* the seven PR 2 methods and Task 7's now_ms and local_time unchanged */
        fn user_writable(&self, path: &Path) -> bool { self.1.iter().any(|writable| writable == path) }
    }

    /// Writes mkvmerge's file name into `root/<folder>` and returns (folder, file).
    fn place(root: &Path, folder: &str) -> (PathBuf, PathBuf) {
        let dir = root.join(folder);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(exe_name(Tool::Mkvmerge));
        fs::write(&file, b"x").unwrap();
        (dir, file)
    }
```

`the_settings_path_is_taken_as_given` and `a_missing_tool_is_none` change only in their `TestHost(.., vec![])`. The order test is renamed and gains the unjudged install folder; the rest are new:

```rust
    /// Design part 3.6's order holds under W18. The default install folder is used even when the host calls it
    /// writable: the rule judges only `PATH` and `resources`, and Windows protects Program Files.
    #[test]
    fn the_order_stays_settings_resources_install_folder_path() {
        let root = temp_dir("lookup-order");
        let (resources, in_resources) = place(&root, "resources");
        let (install_folder, install) = place(&root, "install");
        let (on_path, in_path) = place(&root, "path");
        let path_var = std::env::join_paths([root.join("empty"), on_path]).unwrap();
        let find = |settings: &Settings, resources: Option<&Path>, host: &TestHost| {
            find_tool(Tool::Mkvmerge, settings, resources, host, Some(path_var.as_os_str())).map(|l| (l.path, l.found))
        };
        let host = TestHost(Some(install.clone()), vec![]);
        let default = Settings::default();
        assert_eq!(find(&named("given.exe"), Some(&resources), &host), Some((PathBuf::from("given.exe"), Found::Settings)));
        assert_eq!(find(&default, Some(&resources), &host), Some((in_resources, Found::Resources)));
        assert_eq!(find(&default, None, &host), Some((install.clone(), Found::InstallFolder)));
        let judged = TestHost(Some(install.clone()), vec![install_folder, install.clone()]);
        assert_eq!(find(&default, None, &judged), Some((install, Found::InstallFolder)));
        assert_eq!(find(&default, None, &TestHost(None, vec![])), Some((in_path, Found::Path)));
    }

    #[test]
    fn a_relative_or_empty_path_entry_is_skipped() {
        let root = temp_dir("lookup-relative");
        let (on_path, file) = place(&root, "path");
        let path_var = std::env::join_paths([PathBuf::new(), PathBuf::from("relative"), on_path.clone()]).unwrap();
        assert_eq!(path_folders(Some(path_var.as_os_str())), vec![on_path]);
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), None, &TestHost(None, vec![]), Some(path_var.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
    }

    #[test]
    fn a_writable_path_folder_is_skipped_for_the_next_one() {
        let root = temp_dir("lookup-writable-folder");
        let (first, _) = place(&root, "first");
        let (second, file) = place(&root, "second");
        let host = TestHost(None, vec![first.clone()]);
        let both = std::env::join_paths([&first, &second]).unwrap();
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), None, &host, Some(both.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
        let only_first = std::env::join_paths([&first]).unwrap();
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), None, &host, Some(only_first.as_os_str())), None);
    }

    #[test]
    fn a_writable_program_file_in_a_protected_folder_is_skipped() {
        let root = temp_dir("lookup-writable-file");
        let (first, first_file) = place(&root, "first");
        let (second, file) = place(&root, "second");
        let host = TestHost(None, vec![first_file]);
        let both = std::env::join_paths([&first, &second]).unwrap();
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), None, &host, Some(both.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
    }

    #[test]
    fn a_writable_resources_folder_is_skipped() {
        let root = temp_dir("lookup-writable-resources");
        let (resources, _) = place(&root, "resources");
        let (on_path, file) = place(&root, "path");
        let host = TestHost(None, vec![resources.clone()]);
        let path_var = std::env::join_paths([&on_path]).unwrap();
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), Some(&resources), &host, Some(path_var.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), Some(&resources), &host, None), None);
    }

    #[test]
    fn a_settings_path_is_taken_as_given_even_when_writable() {
        let root = temp_dir("lookup-writable-settings");
        let (folder, file) = place(&root, "named");
        let host = TestHost(None, vec![folder, file.clone()]);
        let found = find_tool(Tool::Mkvmerge, &named(file.to_str().unwrap()), None, &host, None);
        assert_eq!(found, Some(ToolLocation { tool: Tool::Mkvmerge, path: file, found: Found::Settings }));
    }

    #[test]
    fn a_settings_path_without_its_suffix_finds_the_file_with_it() {
        let root = temp_dir("lookup-suffix");
        let (folder, file) = place(&root, "named");
        let host = TestHost(None, vec![]);
        // Windows: `<folder>\mkvmerge` finds `mkvmerge.exe`. Elsewhere EXE_SUFFIX is empty and the given path is the file.
        let given = folder.join("mkvmerge");
        let found = find_tool(Tool::Mkvmerge, &named(given.to_str().unwrap()), None, &host, None);
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Settings)));
        // Neither the path nor the path with the suffix is a file: the given path, which the callers' is_file filter
        // then counts as missing (design part 3.7).
        let absent = folder.join("absent");
        let found = find_tool(Tool::Mkvmerge, &named(absent.to_str().unwrap()), None, &host, None);
        assert_eq!(found.map(|l| l.path), Some(absent));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-tools --locked lookup`
Expected: compile error, ``cannot find function `path_folders` in this scope``. With that helper alone in place, the writable tests and, on Windows, the suffix test FAIL; on Linux and macOS the suffix test passes before the change, since `EXE_SUFFIX` is empty there.

- [ ] **Step 3: Implement**

- The module doc and `find_tool`'s doc as in the Interfaces block.
- The settings branch: when `PathBuf::from(path)` is not a file and `EXE_SUFFIX` is not empty, the path's `OsString` with `EXE_SUFFIX` pushed is returned when that is a file (pushed, not `with_extension`, which would replace the last dotted part of a name such as `mkvmerge.v95`); otherwise the given path, as before.
- The resources branch adds `!host.user_writable(folder)` after the `is_file` check.
- The install-folder branch is unchanged.
- The `PATH` branch walks `path_folders(path_var)` and takes the first `folder.join(&name)` that is a file and where neither `host.user_writable(&folder)` nor `host.user_writable(&file)` is true; `user_writable` runs only for folders that hold the tool.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-tools --locked lookup`
Expected: PASS, the 11 tests of `lookup::tests`.

Then the workspace: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS. The engine harness names every stand-in in `tool_paths` with its suffix and passes an empty `PATH` (testkit.rs:461), so no engine test changes.

- [ ] **Step 5: Commit**

```
git add crates/tools/src/lookup.rs
git commit -m "tools: find_tool keeps absolute PATH folders a non-elevated process cannot write (S18, design part 3.6)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 9: `rip-summary.json`'s `started` (R23.7; design parts 4.6, 7.1; W16)

**Files:**
- Modify: `crates/core/src/record.rs:102-129` (`TitleRecord::rip_started_ms`, its last field), `:234-247` (the tests' `title` helper) and its `mod tests` (2 tests after `a_title_record_without_verify_failures_reads_as_zero`, :270-277).
- Modify: the three other `TitleRecord` literals gain `rip_started_ms: None`: `crates/core/src/ipc.rs:559-562`, `crates/engine/src/store.rs:539-542`, `crates/engine/src/steps.rs:807-821`.
- Modify: `crates/engine/src/steps.rs:916-988` (`rip_title` posts the start after the drive listing and before `self.started`), `:1033-1068` (`write_rip_summary` writes `started` after `bytes`, through the new `rip_started_text`) and its `mod tests` (1 test).
- Modify: `crates/fake-tools/tests/engine_disc_phase.rs` (2 tests after `a_movie_disc_reaches_ejected_with_its_recording`, :48-76).
- Unchanged: `crates/fake-tools/tests/whole_job.rs`, which checks only that `rip-summary.json` is present (whole_job.rs:189).

**Interfaces:**
- Consumes: `Host::{now_ms, local_time}` (Task 7); `Step::post_title` (steps.rs:536-543), whose edits go with the first transition the step's report feeds (RR1-16); the harness's `FakeHost::new("2026-10-23", 1_792_800_000)` (testkit.rs:1003); `Harness::{insert, run_until_event, restart, run_until, record, job_dir, fingerprint}`.
- Produces:

```rust
// crates/core/src/record.rs, after `filed`
    /// When the title's last rip started, in Unix milliseconds: RipTitle posts it before `makemkvcon` runs, and
    /// VerifyFile writes rip-summary.json's `started` from it, so the start lives in state.json, not in memory (R23.4,
    /// R23.7; Ruling W16). A record written before the key existed reads as None and writes back unchanged (design
    /// part 7.4). Def: T3-9. Use: T3-9.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rip_started_ms: Option<u64>,
```

```rust
// crates/engine/src/steps.rs
/// rip-summary.json's `started`: `host.local_time` of the title record's `rip_started_ms`, None when the record has
/// none (R23.7; Ruling W16). Def: T3-9. Use: T3-9 (`write_rip_summary`).
pub(crate) fn rip_started_text(title: &TitleRecord, host: &dyn Host) -> Option<String>;
```

- `rip-summary.json` gains `"started": "<yyyy-MM-dd HH:mm:ss.fff>"` between `bytes` and `elapsed_s`, as design part 7.1 lists phase 0's keys; a title record without `rip_started_ms` writes no `started`. `FakeHost`'s clock is fixed, so only `rip_started_text`'s own test tells the record's start from the clock.

- [ ] **Step 1: Write the failing core tests**

In `crates/core/src/record.rs`'s `mod tests`, the `title` helper gains `rip_started_ms: None`, and:

```rust
    /// Ruling W16 (design part 7.4): a title written before `rip_started_ms` existed reads as None and writes back
    /// byte for byte, so the format stays `lazymkv job v1`.
    #[test]
    fn a_title_record_without_rip_started_reads_and_writes_back_unchanged() {
        let text = serde_json::to_string(&title("t00-00800")).unwrap();
        assert!(!text.contains("rip_started_ms"), "{text}");
        let read: TitleRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(read.rip_started_ms, None);
        assert_eq!(serde_json::to_string(&read).unwrap(), text);
    }

    #[test]
    fn rip_started_ms_round_trips() {
        let mut record = title("t00-00800");
        record.rip_started_ms = Some(1_792_800_000_123);
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains(r#""rip_started_ms":1792800000123"#), "{text}");
        assert_eq!(serde_json::from_str::<TitleRecord>(&text).unwrap(), record);
    }
```

- [ ] **Step 2: Run the core tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked record`
Expected: compile errors, ``struct `TitleRecord` has no field named `rip_started_ms` `` and ``no field `rip_started_ms` on type `TitleRecord` ``.

- [ ] **Step 3: Add the field**

`TitleRecord` gains the field of the Interfaces block as its last field; `ipc.rs:560`, `store.rs:539` and `steps.rs:810` add `rip_started_ms: None` to their literals.

- [ ] **Step 4: Run the core tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core --locked record`
Expected: PASS, the two new tests among them.

- [ ] **Step 5: Write the failing engine tests**

In `crates/fake-tools/tests/engine_disc_phase.rs`, after `a_movie_disc_reaches_ejected_with_its_recording`:

```rust
/// R23.7 with Ruling W16 (design parts 4.6, 7.1): `started` is the rip's recorded start in phase 0's local-time form,
/// between `bytes` and `elapsed_s`. FakeHost's clock, 1_792_800_000 s, read as UTC is 2026-10-24 00:00:00.000; the
/// harness's `local_date` stays 2026-10-23, since the fake's date is a setting and `started` is its clock read as UTC.
#[test]
fn the_rip_summary_has_started_from_the_rip_start_between_bytes_and_elapsed_s() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let summary = json_of(&h.job_dir(&id).join(KEY).join("rip-summary.json"));
    let keys: Vec<&str> = summary.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["ok", "result", "slug", "label", "title", "profile", "bytes", "started", "elapsed_s", "exit_code",
        "files", "dest", "cache_mb"]);
    assert_eq!(summary["started"], json!("2026-10-24 00:00:00.000"));
    assert_eq!(h.record(&id).titles[0].rip_started_ms, Some(1_792_800_000_000));
}

/// R23.4 with Ruling W16: the rip's start is persisted with TitleRipped, before VerifyFile writes rip-summary.json, so
/// a restart there finds it in state.json. A restart in Ripping rips the title again (`resume`, core/src/job.rs:742),
/// and the summary takes the start that rip posted; the first rip, moved aside, never got a summary.
#[test]
fn a_restart_between_rip_and_verify_keeps_started() {
    let h = harness(&[], json!({}));
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.run_until_event("TitleRipped", |event| matches!(event, Event::TitleRipped));
    let key_dir = h.job_dir(&id).join(KEY);
    assert!(!key_dir.join("rip-summary.json").exists());
    assert_eq!(h.record(&id).titles[0].rip_started_ms, Some(1_792_800_000_000));
    h.restart();
    h.run_until("Ejected");
    assert_eq!(json_of(&key_dir.join("rip-summary.json"))["started"], json!("2026-10-24 00:00:00.000"));
    assert!(!key_dir.join("attempt-1").join("rip-summary.json").exists());
    let record = h.record(&id);
    assert_eq!((record.titles[0].attempts, record.titles[0].rip_started_ms), (2, Some(1_792_800_000_000)));
}
```

In `crates/engine/src/steps.rs`'s `mod tests`:

```rust
    /// Ruling W16: `started` is the record's start, not the clock at VerifyFile.
    #[test]
    fn started_is_the_records_rip_start_not_the_clock() {
        let host = lazymkv_platform::fakes::FakeHost::new("2026-10-23", 1_792_800_000);
        let mut title = TitleRecord {
            key: "t00-00800".to_string(), scan_title: 0, rip_index: None, attempts: 1, verified: false,
            verify_failures: 0, read_errors: Vec::new(), rip_reviews: Vec::new(), decision: None, filed: None,
            rip_started_ms: Some(1_700_000_000_123),
        };
        assert_eq!(rip_started_text(&title, &host).as_deref(), Some("2023-11-14 22:13:20.123"));
        title.rip_started_ms = None;
        assert_eq!(rip_started_text(&title, &host), None);
    }
```

- [ ] **Step 6: Run the engine tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase --test whole_job`
Expected: FAIL in `the_rip_summary_has_started_from_the_rip_start_between_bytes_and_elapsed_s` (the keys lack `started`) and in `a_restart_between_rip_and_verify_keeps_started` (`left: None`, `right: Some(1792800000000)`); every other test passes.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib steps::`
Expected: a compile error, ``cannot find function `rip_started_text` ``.

- [ ] **Step 7: Implement**

- `rip_title`, inside its loop, after `let (drive, _) = self.listing(&makemkvcon, failed)?;` and before `self.started(..)`, as phase 0 took its start after the drive listing (scripts/phase0/rip-title.ps1:101-103); each pass of the index guard's loop posts again, so the record holds the last rip's start:

```rust
            let started_ms = self.steps.host.now_ms();
            if !self.post_title(key, move |title| title.rip_started_ms = Some(started_ms)) {
                return Err(cancelled());
            }
```

- `rip_started_text` as its doc says. `write_rip_summary`: its doc gains "`started` is `rip_started_text`, after `bytes`; a record without a start writes no `started` (R23.7, Ruling W16)". The `json!` literal ends at `bytes`; `started` is inserted when `rip_started_text` is Some; then `elapsed_s`, `exit_code`, `files`, `dest` and `cache_mb` are set by index assignment, which appends in order under `preserve_order`; `rip_index` stays last, as today.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase --test whole_job`
Expected: PASS, `whole_job` included.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib` (`steps.rs`'s and `store.rs`'s own tests).
Expected: PASS.

Then the workspace: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 9: Commit**

```
git add crates/core/src/record.rs crates/core/src/ipc.rs crates/engine/src/store.rs crates/engine/src/steps.rs crates/fake-tools/tests/engine_disc_phase.rs
git commit -m "engine: rip-summary.json writes started from the recorded rip start (R23.7)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 10: Transport close, the accept rule and the stop API (engine.md:581 (a), (b), (d); W10)

**Files:**
- Modify: `crates/platform/src/lib.rs:144-149` (`IpcTransport` gains `close` and Ruling W10's accept contract).
- Modify: `crates/platform/src/fakes.rs:354-387` (`MemoryIpc` gains `closed`, `close` and an `accept` that wakes on it) and its `mod tests`, one test after `memory_ipc_is_a_duplex_line_stream` (:598-617).
- Modify: `crates/engine/src/lib.rs:17-40` (`EngineConfig.ctl` and `EngineConfig.console`; `ShutdownHandle` in the re-export at :25).
- Modify: `crates/engine/src/owner.rs:161-191` (`Msg::Log`), `:205-298` (`start_until`: the console writer goes to `Store::new` at :213; the clients and the transport are kept for the handle), `:301-397` (`EngineHandle`: `waker` and `set_ipc_waker` removed; `shutdown_handle`, `wait_for_exit`; `stop` closes and drains; `ShutdownHandle`), `:1064-1105` (`on_message` and `quiet_message` write `Msg::Log`).
- Modify: `crates/engine/src/ipc_server.rs:26-27` (`CLIENT_DRAIN`, `ACCEPT_PAUSE`, `NO_CLIENT` after `CLIENT_QUEUE`), `:163-222` (`Clients` counts its writer threads; `drain`), `:361-377` (`spawn_serve`'s rule), and its `mod tests` (:407-548).
- Modify: `crates/engine/src/testkit.rs:340-343` (the fields `ipc` and `connector` become one `ipc: Mutex<(Arc<MemoryIpc>, MemoryConnector)>`; a new `console: Arc<Mutex<Vec<u8>>>`), `:433-492` (`start` makes a fresh `MemoryIpc`, passes `ctl: None` and the console writer; the waker block at :484-490 goes), `:653-657` (`restart`'s doc), `:901-906` (`client`), `:975-1005` (`Folders::harness`), and three new methods after `client`.
- Modify: `crates/fake-tools/tests/engine_ipc.rs` (three tests in the section "Shutdown, progress, command and notifications", after `a_pulled_disc_reinserted_after_a_forced_shutdown_resumes_its_job`, :1899-1908).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes (pull request 2): `MemoryIpc`, `MemoryConnector` (fakes.rs:354-401); `EngineHandle` (owner.rs:301-397); `Store::new(data, now, console)` (store.rs:85-104) and `Store::log` (store.rs:269-280); `Clients` (ipc_server.rs:163-222); `spawn_serve` (ipc_server.rs:361-377); `Request::Shutdown { force }` (`crates/core/src/ipc.rs`); `Harness::{client, record, engine_log, wait}` (testkit.rs).
- Produces:

```rust
// crates/platform/src/lib.rs
/// The engine's end of the pipe: hands out one stream per client that connects (spec 4.3, design parts 4.2, 4.4;
/// Ruling W10). Def: T8, T3-10. Use: T26, T3-10, T3-11, T3-23.
pub trait IpcTransport: Send + Sync {
    /// Waits for the next client (engine.md:581 (d); Ruling W10). Err by kind: `BrokenPipe` only once `close` ran, and
    /// it ends the serve thread; `PermissionDenied` a refused client, its text `process <pid>: <verdict>`;
    /// `ConnectionAborted` a client gone before its check; any other kind one failed accept, which the serve thread
    /// logs and retries after `ACCEPT_PAUSE`. An implementation turns an operating-system error of kind `BrokenPipe`
    /// met before `close` into `ErrorKind::Other`, so it never reads as closed.
    fn accept(&self) -> io::Result<IpcStream>;
    /// Closes the transport: an `accept` waiting now and every later one return `BrokenPipe` (design part 3.8: on exit
    /// the engine closes the pipe). Streams already handed out are not touched.
    fn close(&self);
}

// crates/platform/src/fakes.rs
pub struct MemoryIpc { incoming: Mutex<Receiver<IpcStream>>, closed: AtomicBool }
impl IpcTransport for MemoryIpc {
    /// Waits for a client. Fails with BrokenPipe after `close`, which wakes a waiting accept within
    /// `MEMORY_ACCEPT_POLL`, and once every connector is dropped, which leaves no way in (Ruling T8-e; ctl's scripted
    /// peer relies on it, `crates/ctl/src/engine_cmds.rs:428-432`).
    fn accept(&self) -> io::Result<IpcStream>;
    /// Sets `closed` (Ruling W10).
    fn close(&self);
}
/// How often a waiting `MemoryIpc::accept` looks at `closed`.
const MEMORY_ACCEPT_POLL: Duration = Duration::from_millis(10);

// crates/engine/src/lib.rs
pub use owner::{start, EngineHandle, JobContext, Services, ShutdownHandle, StepResult, StepRunner};
pub struct EngineConfig { /* data, resources, limits_override, path_var unchanged */
    /// The `lazymkvctl` the `command` relay runs as its child (design part 9.5); None refuses `command` with
    /// `COMMAND_NO_CTL`. Def: T3-10. Use: T3-22, T3-23.
    pub ctl: Option<PathBuf>,
    /// Where every engine.log line is also written: standard error in a console run (design part 3.5), the harness's
    /// buffer in tests; None writes engine.log only. Def: T3-10. Use: T3-10, T3-23.
    pub console: Option<Box<dyn Write + Send>>,
}

// crates/engine/src/owner.rs
pub(crate) enum Msg { /* pull request 2's variants unchanged */
    /// A line for engine.log from a thread that holds no store: the serve thread's refusals and failed accepts (design
    /// part 3.5; Ruling W10).
    Log(String),
}
/// Asks the owner for a shutdown as a `shutdown` request with no client does (design part 3.8: Ctrl+C in a console
/// run is a forced shutdown). It sends `Msg::Request` with `client: NO_CLIENT`, `id: None` and
/// `request: Request::Shutdown { force }`, so the reply goes nowhere; a send to an owner that has stopped does nothing.
/// Def: T3-10. Use: T3-10, T3-23.
#[derive(Clone)]
pub struct ShutdownHandle { sender: Sender<Msg> }
impl ShutdownHandle {
    /// See the struct. Def: T3-10. Use: T3-10, T3-23.
    pub fn shutdown(&self, force: bool);
}
pub struct EngineHandle { /* pull request 2's fields but `waker`, plus: */
    /// The transport the serve thread accepts from; `stop` closes it.
    ipc: Arc<dyn IpcTransport>,
    /// The owner's clients; `stop` drains their writers.
    clients: Arc<Clients>,
}
impl EngineHandle {
    /// A handle that asks this engine's owner for a shutdown from any thread (design part 3.8; engine.md:581 (a)).
    /// Def: T3-10. Use: T3-10, T3-23.
    pub fn shutdown_handle(&self) -> ShutdownHandle;
    /// True once the owner thread has left its loop, after a `shutdown` request or a stop; false when `timeout` passed
    /// first; None waits without a limit. It polls `JoinHandle::is_finished` every `WAIT_POLL` (engine.md:581 (b)).
    /// Def: T3-10. Use: T3-10, T3-23.
    pub fn wait_for_exit(&self, timeout: Option<Duration>) -> bool;
    /// Pull request 2's stop, which now closes the transport and drains the clients (Step 3 gives the order).
    /// Def: T22, T3-10. Use: T22 to T29, T3-23.
    pub fn stop(self) -> io::Result<()>;
    // removed: pub(crate) fn set_ipc_waker (engine.md:581 (a))
}

// crates/engine/src/ipc_server.rs
/// How long `EngineHandle::stop` waits for the clients' writers to write what was queued, such as `shutdown`'s `ok`,
/// before it returns anyway (design parts 3.8, 9.4; Ruling W10). Def: T3-10. Use: T3-10.
pub(crate) const CLIENT_DRAIN: Duration = Duration::from_secs(2);
/// The serve thread's pause after a failed accept, before it tries again (engine.md:581 (d); Ruling W10).
/// Def: T3-10. Use: T3-10.
pub(crate) const ACCEPT_PAUSE: Duration = Duration::from_millis(100);
/// The client number of a request no connection sent; `Clients` numbers from 1, so its replies go nowhere (design part
/// 3.8). Def: T3-10. Use: T3-10.
pub(crate) const NO_CLIENT: u64 = 0;
impl Clients {
    /// Waits until every writer thread `add` started has ended and dropped its writer, at most `bound`; true when all
    /// have (design parts 3.8, 9.1). Def: T3-10. Use: T3-10.
    pub(crate) fn drain(&self, bound: Duration) -> bool;
}
pub(crate) fn spawn_serve(transport: Arc<dyn IpcTransport>, stop: Arc<AtomicBool>, owner: Sender<Msg>) -> JoinHandle<()>;
    // signature unchanged; Ruling W10's rule (Step 3)

// crates/engine/src/testkit.rs
impl Harness {
    /// Everything the engines of this harness wrote to their console writer (`EngineConfig.console`), as text (design
    /// part 3.5). Def: T3-10. Use: T3-10, T3-23.
    pub fn console_text(&self) -> String;
    /// The running engine's `EngineHandle::shutdown_handle`. Def: T3-10. Use: T3-10.
    pub fn shutdown_handle(&self) -> ShutdownHandle;
    /// The running engine's `EngineHandle::wait_for_exit(Some(timeout))`; true when no engine runs. Def: T3-10.
    /// Use: T3-10.
    pub fn wait_for_exit(&self, timeout: Duration) -> bool;
}
```

The engine.log lines the serve thread sends (Rulings W4, W10):
- `refused a pipe client: <error>` for `PermissionDenied`; with the named pipe's text (Task 11) it reads `refused a pipe client: process <pid>: <verdict>`.
- `the pipe could not accept a client: <error>` for any kind but `BrokenPipe`, `PermissionDenied` and `ConnectionAborted`.
- No line for `ConnectionAborted`: a client that left before its check was not refused.

Why the drain is tested on `Clients`: `MemoryIpc`'s writer never blocks, so no test through the harness can tell a drained stop from an undrained one. The two `drain` tests below use a writer that is slow and one that never returns. The order "`ok` before the end" over the fake is already pinned by `shutdown_is_refused_while_a_step_runs_and_force_cancels_and_deletes_partials` (engine_ipc.rs:1756-1757); over the real pipe it is Task 11's `the_last_queued_line_reaches_the_client_before_the_pipe_closes` and Task 23's native `stop`.

Owner decision 11 lists W10; it does not block this task.

- [ ] **Step 1: Write the failing tests**

In `crates/platform/src/fakes.rs`'s `mod tests`, after `memory_ipc_is_a_duplex_line_stream` (which stays as it is: the dropped connector still closes the fake):

```rust
    #[test]
    fn a_closed_memory_ipc_wakes_a_waiting_accept_with_broken_pipe() {
        let (ipc, _connector) = MemoryIpc::new();
        let ipc = Arc::new(ipc);
        let waiting = Arc::clone(&ipc);
        let (done, result) = channel();
        std::thread::spawn(move || {
            let _ = done.send(waiting.accept().err().map(|error| error.kind()));
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(result.try_recv().is_err(), "accept returned while a connector lived and nothing closed the fake");
        ipc.close();
        assert_eq!(result.recv_timeout(Duration::from_secs(5)).unwrap(), Some(io::ErrorKind::BrokenPipe));
        assert_eq!(ipc.accept().err().map(|error| error.kind()), Some(io::ErrorKind::BrokenPipe));
    }
```

In `crates/engine/src/ipc_server.rs`'s `mod tests`, beside `Stuck` and `Counting`:

```rust
    use crate::owner::Msg;
    use lazymkv_platform::fakes::MemoryIpc;
    use lazymkv_platform::{IpcStream, IpcTransport};
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc::channel;

    /// A writer that takes 100 ms per write and counts the lines it took.
    struct Slow(Arc<AtomicUsize>);
    impl Write for Slow {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            std::thread::sleep(Duration::from_millis(100));
            self.0.fetch_add(buf.iter().filter(|&&byte| byte == b'\n').count(), Ordering::SeqCst);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    /// A transport that returns its scripted results in order, then BrokenPipe as a closed transport does, and notes
    /// when each accept ran.
    struct Scripted { results: Mutex<VecDeque<io::Result<IpcStream>>>, calls: Mutex<Vec<Instant>> }
    impl IpcTransport for Scripted {
        fn accept(&self) -> io::Result<IpcStream> {
            self.calls.lock().unwrap().push(Instant::now());
            let next = self.results.lock().unwrap().pop_front();
            next.unwrap_or_else(|| Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed")))
        }
        fn close(&self) {}
    }

    fn stream() -> io::Result<IpcStream> {
        Ok(IpcStream { reader: Box::new(io::Cursor::new(Vec::new())), writer: Box::new(io::sink()) })
    }

    /// Runs the serve thread over `results` to its end; returns what it sent the owner and when each accept ran.
    fn serve(results: Vec<io::Result<IpcStream>>) -> (Vec<Msg>, Vec<Instant>) {
        let transport = Arc::new(Scripted { results: Mutex::new(results.into()), calls: Mutex::new(Vec::new()) });
        let (owner, inbox) = channel();
        spawn_serve(Arc::clone(&transport) as _, Arc::new(AtomicBool::new(false)), owner).join().unwrap();
        let calls = transport.calls.lock().unwrap().clone();
        (inbox.try_iter().collect(), calls)
    }

    #[test]
    fn a_refused_client_is_logged_and_the_serve_thread_goes_on() {
        let refused = io::Error::new(io::ErrorKind::PermissionDenied, "process 4242: its token is not elevated");
        let (sent, _) = serve(vec![Err(refused), stream()]);
        let expected = "refused a pipe client: process 4242: its token is not elevated";
        assert!(matches!(&sent[..], [Msg::Log(line), Msg::Connect(_)] if line == expected));
    }

    #[test]
    fn a_failed_accept_is_logged_paused_and_retried() {
        let (sent, calls) = serve(vec![Err(io::Error::other("the pipe broke")), stream()]);
        let expected = "the pipe could not accept a client: the pipe broke";
        assert!(matches!(&sent[..], [Msg::Log(line), Msg::Connect(_)] if line == expected));
        assert!(calls[1] - calls[0] >= ACCEPT_PAUSE);
    }

    #[test]
    fn a_client_gone_before_its_check_is_passed_over_without_a_line() {
        let gone = io::Error::new(io::ErrorKind::ConnectionAborted, "the client left before its check");
        let (sent, calls) = serve(vec![Err(gone), stream()]);
        assert!(matches!(&sent[..], [Msg::Connect(_)]));
        assert!(calls[1] - calls[0] < ACCEPT_PAUSE);
    }

    #[test]
    fn a_closed_transport_ends_the_serve_thread() {
        let (ipc, _connector) = MemoryIpc::new();
        let ipc = Arc::new(ipc);
        let (owner, _inbox) = channel();
        let serve = spawn_serve(Arc::clone(&ipc) as _, Arc::new(AtomicBool::new(false)), owner);
        std::thread::sleep(Duration::from_millis(50));
        ipc.close();
        let (done, ended) = channel();
        std::thread::spawn(move || {
            let _ = done.send(serve.join().is_ok());
        });
        assert_eq!(ended.recv_timeout(Duration::from_secs(5)), Ok(true));
    }

    #[test]
    fn drain_returns_once_every_closed_clients_queued_lines_are_written() {
        let taken = Arc::new(AtomicUsize::new(0));
        let clients = Clients::new();
        let client = clients.add(Box::new(Slow(taken.clone())));
        for n in 0..3 { clients.send(client, format!("{{\"n\":{n}}}")); }
        clients.close_all();
        assert!(clients.drain(Duration::from_secs(5)));
        assert_eq!(taken.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drain_gives_up_at_its_bound_for_a_client_that_never_reads() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let clients = Clients::new();
        let client = clients.add(Box::new(Stuck(gate.clone())));
        clients.send(client, "{}".to_string());
        clients.close_all();
        let started = Instant::now();
        assert!(!clients.drain(Duration::from_millis(300)));
        let spent = started.elapsed();
        assert!(spent >= Duration::from_millis(300) && spent < Duration::from_secs(2), "{spent:?}");
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
    }
```

In `crates/fake-tools/tests/engine_ipc.rs`, after `a_pulled_disc_reinserted_after_a_forced_shutdown_resumes_its_job`:

```rust
#[test]
fn wait_for_exit_returns_after_a_shutdown_request() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert!(!h.wait_for_exit(Duration::from_millis(300)), "the owner left its loop before any shutdown");
    assert_eq!(c.ok("shutdown", json!({"force": false})), Value::Null);
    assert!(h.wait_for_exit(WAIT * ci_factor()));
    assert_eq!(c.line(), None);
}

#[test]
fn a_forced_shutdown_from_the_handle_needs_no_client() {
    let h = scripted(hanging_at("Remux", 0));
    h.insert("tok0", DISC_A);
    wait_for_call(&h, "Remux", 1);
    let id = h.job_ids().pop().unwrap();
    h.shutdown_handle().shutdown(false);
    assert!(!h.wait_for_exit(Duration::from_millis(300)), "a shutdown without force stopped a running step");
    h.shutdown_handle().shutdown(true);
    assert!(h.wait_for_exit(WAIT * ci_factor()));
    let stopped = h.record(&id);
    assert_eq!(stopped.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert_eq!(stopped.message.as_deref(), Some(CANCELLED));
}

#[test]
fn a_console_engine_mirrors_its_log_lines_to_its_console_writer() {
    let h = scripted(vec![]);
    h.restart();
    h.wait("the console writer to hold every engine.log line", |h| h.console_text() == h.engine_log());
    let log = h.engine_log();
    assert!(log.contains(" engine started (lazymkv ") && log.contains(" engine stopped\n"), "{log}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform -p lazymkv-engine -p lazymkv-ctl --features lazymkv-platform/fakes --locked`
Expected: a compile error: no method named `close` found for `MemoryIpc`, no variant `Log` in `Msg`, no method named `drain` found for `Clients`, `ACCEPT_PAUSE` not found.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc --test ctl_engine`
Expected: a compile error: no method named `wait_for_exit`, `shutdown_handle` or `console_text` found for `Harness`.

- [ ] **Step 3: Implement**

- `platform::IpcTransport`: the two methods and docs of the Produces block.
- `fakes::MemoryIpc`: `new` sets `closed: AtomicBool::new(false)`. `accept` holds the `incoming` lock and loops: `closed` set gives `Err(BrokenPipe, "the transport is closed")`; `recv_timeout(MEMORY_ACCEPT_POLL)` gives the stream on Ok, loops on Timeout, and gives `Err(BrokenPipe, "every connector to the pipe is gone")` on Disconnected. `close` stores true. `closed` is read before each wait, so a stream queued after `close` is dropped and its client reads the end.
- `engine::EngineConfig`: the two fields of the Produces block; its doc adds design part 9.5.
- `engine::owner`:
  - `start_until(mut config, ..)`: `let console = config.console.take();` is passed to `Store::new` in place of `None` (owner.rs:213); `let clients = Arc::new(Clients::new());` goes into `Parts` and the handle; `Arc::clone(&ipc)` is kept before `ipc` moves into `spawn_serve`.
  - `Msg::Log(line)`: `on_message` and `quiet_message` call `self.store.log(&line)`.
  - `ShutdownHandle::shutdown` and `EngineHandle::shutdown_handle` as their docs say. `wait_for_exit`: polls `self.owner.is_finished()` every `WAIT_POLL` until true, or until the deadline when `timeout` is Some.
  - `EngineHandle::stop`, in this order: raise `serve_stop`, `ipc.close()`, join the serve thread (it always ends now, so the old `None => true` branch goes); raise `drive_stop` and join the drive thread; send `Msg::Stop`, drop the sender and join the owner, whose `shut_down` closes every client; wait for `notifier_done`; `clients.drain(CLIENT_DRAIN)`; log `engine stopped`. Err as before when the serve, drive or owner thread panicked. The doc replaces "woken by the IPC waker" with the transport's close and adds the drain (design parts 3.8, 9.4).
  - `set_ipc_waker` and the `waker` field are deleted.
- `engine::ipc_server`:
  - `Clients` gains `writers: Arc<(Mutex<usize>, Condvar)>`. `add` increments it before spawning; the thread's first local is a guard whose `Drop` decrements and calls `notify_all`, declared before `writer`, so the count falls only after the writer is dropped (locals drop in reverse order). `drain` is `Condvar::wait_timeout_while(count > 0)` with `bound` and returns `count == 0`.
  - `spawn_serve` loops on `transport.accept()`:
    - `Ok(stream)`: as before, `stop` raised or a failed `Msg::Connect` send ends the loop;
    - `BrokenPipe`: ends the loop;
    - `ConnectionAborted`: the next accept at once, no line;
    - `PermissionDenied`: `Msg::Log(format!("refused a pipe client: {error}"))`, then the next accept;
    - any other kind: `Msg::Log(format!("the pipe could not accept a client: {error}"))`, then `thread::sleep(ACCEPT_PAUSE)`, then the next accept.

    A failed `Msg::Log` send (the owner is gone) ends the loop too. The doc names engine.md:581 (d) and Ruling W10, and drops "wakes the waiting accept with a connection of its own".
- `engine::testkit`:
  - `struct ConsoleBuffer(Arc<Mutex<Vec<u8>>>)` implements `Write` by extending the vector (`flush` does nothing). `Folders::harness` sets `console: Arc::default()` and `ipc: Mutex::new(<the MemoryIpc::new() pair it builds today, ipc in an Arc>)`.
  - `start` first replaces `self.ipc` with a fresh `MemoryIpc::new()` pair, so a restarted engine never meets the transport its predecessor's `stop` closed. It builds `Platform.ipc` from the new pair and adds `ctl: None` and `console: Some(Box::new(ConsoleBuffer(Arc::clone(&self.console))))` to the `EngineConfig` literal. The `set_ipc_waker` block and its comment go.
  - `client` clones the current pair's connector. `restart`'s doc says "a new MemoryIpc" in place of "MemoryIpc" in its list of what stays.
  - `console_text`: `String::from_utf8_lossy` of the buffer. `shutdown_handle`: the running engine's, with `expect("an engine runs")`. `wait_for_exit`: true when `self.engine` holds none.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform -p lazymkv-engine -p lazymkv-ctl --features lazymkv-platform/fakes --locked`
Expected: PASS, `ctl`'s scripted-peer tests in `engine_cmds.rs` among them (their `finish` still ends `accept` by dropping the connector).

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc --test ctl_engine`
Expected: PASS: every pull request 2 test of both files passes without the waker, and the three new tests pass.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/lib.rs crates/platform/src/fakes.rs crates/engine/src/lib.rs crates/engine/src/owner.rs crates/engine/src/ipc_server.rs crates/engine/src/testkit.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the transport closes, the serve thread survives a failed accept, and the binary can wait for a shutdown (engine.md:581)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 11: The named pipe server (design parts 3.3, 3.4, 4.4; K12; P6; W10)

**Files:**
- Create: `crates/platform/src/win/pipe.rs` (`#[cfg(windows)]` through `win`: `PipeServer`, the overlapped stream halves shared with Task 12, `PIPE_MODE`, `PIPE_BUFFER`, `accept_error`, `not_closed`; 15 tests).
- Modify: `crates/platform/src/win/mod.rs` (`mod pipe; pub use pipe::PipeServer;`).
- Modify: `crates/platform/src/testutil.rs` (Task 2): `within` and `line`, for this task's tests and Task 12's.

**Interfaces:**
- Consumes: `security::{pipe_sddl, token_verdict, TokenFacts, test_name, SYSTEM_SID, HIGH_INTEGRITY_SID}` and `win::{own_token, token_of_process, wide, Descriptor}` (Task 2); `win::SE_GROUP_INTEGRITY` (Task 7); `testutil::ci_factor` (Task 2); `IpcTransport::{accept, close}` and its error contract (Task 10); `IpcStream` (lib.rs:137-143).
- Produces:

```rust
// crates/platform/src/win/pipe.rs (cfg(windows))
/// The engine's end of the named pipe (design parts 3.3, 3.4, 4.4; K12, P6; Ruling W10). Every instance is created
/// overlapped, from `pipe_sddl` of this process's user, with `PIPE_MODE`, unlimited instances and `PIPE_BUFFER` each
/// way; the first with `FILE_FLAG_FIRST_PIPE_INSTANCE`. A client's token is checked, and the next instance created,
/// before its stream is handed out. Def: T3-11. Use: T3-12, T3-23.
pub struct PipeServer {
    name: Vec<u16>,                        // wide(name)
    shown: String,                         // the name as given, for texts
    descriptor: Descriptor,                // win::Descriptor::from_sddl(&pipe_sddl(<own user SID>)), made once in bind_with
    own_user_sid: String,                  // the security identifier (SID) token_verdict compares a client with
    stop: OwnedHandle,                     // a manual-reset event `close` sets
    closed: AtomicBool,
    listening: Mutex<Option<OwnedHandle>>, // the instance the next client connects to
}
impl PipeServer {
    /// Creates the first instance of `name` (design part 3.3). Err `ErrorKind::AddrInUse` with the text
    /// `another process holds the pipe <name>` when `CreateNamedPipeW` refuses `FILE_FLAG_FIRST_PIPE_INSTANCE` with
    /// `ERROR_ACCESS_DENIED` (5); any other failure is that operating-system error. Def: T3-11. Use: T3-12, T3-23.
    pub fn bind(name: &str) -> io::Result<PipeServer>;
    /// `bind`, but clients are checked against `own_user_sid` instead of this process's user; the descriptor still
    /// grants this process's user, so a test client opens the pipe and meets the check. Def: T3-11. Use: T3-11.
    #[cfg(test)]
    pub(crate) fn bind_as(name: &str, own_user_sid: &str) -> io::Result<PipeServer>;
}
impl IpcTransport for PipeServer {
    fn accept(&self) -> io::Result<IpcStream>; // Step 3's order and Ruling W10's error kinds
    fn close(&self);                           // sets `closed` and `stop`, drops the listening instance
}
/// The pipe mode every `CreateNamedPipeW` call passes (design parts 3.3, 3.4). Def: T3-11. Use: T3-11, T3-12.
pub(crate) const PIPE_MODE: u32 = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
/// Each instance's buffer, each way, in bytes (K12). Def: T3-11. Use: T3-11, T3-12.
pub(crate) const PIPE_BUFFER: u32 = 65536;
/// One connected pipe handle, server or client end, opened overlapped, as an `IpcStream` (K12; Rulings T8-e, W10).
/// The reader half and the writer half each wait on an event of their own, so the writer never waits behind the
/// reader's pending read. The writer's `flush` does nothing: `FlushFileBuffers`, which on a pipe waits until the peer
/// has read, is never called per line. Dropping the writer sets the stream's closing event, which ends the reader's
/// pending or next read as end of file; the handle closes when both halves are gone, so bytes written before stay
/// readable by the peer. A read returns `Ok(0)` on `ERROR_BROKEN_PIPE` (109); a write after the peer is gone fails
/// with kind `BrokenPipe`. Def: T3-11. Use: T3-11, T3-12.
pub(crate) fn pipe_stream(handle: OwnedHandle) -> io::Result<IpcStream>;
/// The error `accept` returns for a failed connect or client check with OS code `code` (Ruling W10):
/// `ERROR_BROKEN_PIPE` (109), `ERROR_NO_DATA` (232) and `ERROR_PIPE_NOT_CONNECTED` (233) are `ConnectionAborted`;
/// any other code is `not_closed(io::Error::from_raw_os_error(code))`. Def: T3-11. Use: T3-11.
pub(crate) fn accept_error(code: u32) -> io::Error;
/// `error`, unless its kind is `BrokenPipe`, which becomes `io::Error::new(ErrorKind::Other, error)`, so only `close`
/// ever gives `BrokenPipe` (engine.md:581 (d)). Def: T3-11. Use: T3-11.
pub(crate) fn not_closed(error: io::Error) -> io::Error;

// crates/platform/src/testutil.rs (cfg(test))
/// `work` on a thread of its own; the test fails when it has not ended within `bound`.
pub(crate) fn within<T: Send + 'static>(bound: Duration, work: impl FnOnce() -> T + Send + 'static) -> T;
/// One line from `reader` with its newline, "" at the end of the stream.
pub(crate) fn line(reader: &mut dyn BufRead) -> String;
```

The client-check texts (`<pid>` the client's process id from `GetNamedPipeClientProcessId`; Rulings W4, W10):
- `GetNamedPipeClientProcessId` fails: `ConnectionAborted`, `the client left before its check: <error>`.
- `token_of_process(pid)` fails: `PermissionDenied`, `process <pid>: its token could not be read: <error>`.
- `token_verdict` refuses: `PermissionDenied`, `process <pid>: <verdict>`, the verdict being `its user differs from this process's user` or `its token is not elevated`.

Owner decision 11 lists W10; it does not block this task. Owner decision 15: the tests of this module need an elevated test process, since the pipe's high integrity label refuses anything lower. If Task 2 Step 6's continuous integration (CI) run shows that `windows-latest` does not run its tests elevated, each test here begins with `if !own_token().unwrap().elevated { crate::testutil::skip("windows-latest is not elevated (design part 12.4)"); return; }`; otherwise no test skips.

- [ ] **Step 1: Write the failing tests**

Create `crates/platform/src/win/pipe.rs` holding its module doc and the test module only, and add `mod pipe;` to `win/mod.rs`, so the crate builds without the tests. Add `within` and `line` to `testutil.rs`; they are test helpers, not under test.

In `crates/platform/src/win/pipe.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{test_name, HIGH_INTEGRITY_SID, SYSTEM_SID};
    use crate::testutil::{ci_factor, line, within};
    use crate::win::own_token;
    use crate::{IpcStream, IpcTransport};
    use std::io::{self, BufRead, Write};
    use std::os::windows::io::{AsRawHandle, OwnedHandle};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{FILE_GENERIC_READ, FILE_GENERIC_WRITE};
    use windows_sys::Win32::System::Pipes::{GetNamedPipeInfo, PIPE_SERVER_END, PIPE_TYPE_MESSAGE};

    /// The bound of a "does not block" proof (K12): far above a local pipe's latency, never scaled (Global
    /// Constraints).
    const NO_BLOCK: Duration = Duration::from_secs(5);
    /// A SID that is not the test's own user.
    const OTHER_SID: &str = "S-1-5-21-1-2-3-1001";
    /// winnt.h's SYSTEM_MANDATORY_LABEL_NO_WRITE_UP and _NO_READ_UP, which windows-sys declares only under a feature
    /// Task 1 does not enable.
    const NO_WRITE_UP: u32 = 0x1;
    const NO_READ_UP: u32 = 0x2;

    fn pipe_name() -> String {
        format!(r"\\.\pipe\{}", test_name("pipe"))
    }

    /// A client end of `name` as a test opens it: `CreateFileW(GENERIC_READ | GENERIC_WRITE, share 0, OPEN_EXISTING,
    /// FILE_FLAG_OVERLAPPED)`, then `pipe_stream`. Err is the open's raw OS error.
    fn open_client(name: &str) -> Result<IpcStream, i32>;

    /// What an instance's descriptor holds, read back (named apart from Task 2's `win::ReadBack`).
    struct PipeSecurity {
        protected: bool,
        allowed: Vec<(String, u32)>,
        labels: Vec<(String, u32)>,
    }

    /// `GetSecurityInfo(handle, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION)`;
    /// `protected` is `SE_DACL_PROTECTED` from `GetSecurityDescriptorControl`. Each access-allowed access control entry
    /// (ACE) of the discretionary access control list (DACL), and each mandatory-label ACE of the system list, is
    /// (its SID as `ConvertSidToStringSidW` writes it, its mask), in order. An allowed mask first goes through
    /// `MapGenericMask` with the file mapping (`FILE_GENERIC_READ`, `FILE_GENERIC_WRITE`, `FILE_GENERIC_EXECUTE`,
    /// `FILE_ALL_ACCESS`), so a mask Windows kept generic and one it mapped at creation compare the same. The
    /// descriptor is freed with `LocalFree`.
    fn security_of(handle: &OwnedHandle) -> PipeSecurity;

    /// Runs `work` on the calling thread while it impersonates a copy of the process token lowered to medium integrity:
    /// `OpenProcessToken(TOKEN_DUPLICATE | TOKEN_QUERY)`, `DuplicateTokenEx(TOKEN_ALL_ACCESS, SecurityImpersonation,
    /// TokenImpersonation)`, `SetTokenInformation(TokenIntegrityLevel)` with the SID `S-1-16-8192` and
    /// `crate::win::SE_GROUP_INTEGRITY`, `SetThreadToken(null, copy)`. `RevertToSelf` runs in a drop guard, so a failing `work`
    /// leaves no impersonation behind.
    fn as_medium_integrity<T>(work: impl FnOnce() -> T) -> T;

    #[test]
    fn binding_a_held_name_fails_with_addr_in_use_naming_the_pipe() {
        let name = pipe_name();
        let _held = PipeServer::bind(&name).unwrap();
        let Err(error) = PipeServer::bind(&name) else { panic!("a second bind of {name} succeeded") };
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        assert_eq!(error.to_string(), format!("another process holds the pipe {name}"));
    }

    #[test]
    fn the_instance_carries_the_owner_and_system_list_and_the_high_label() {
        let server = PipeServer::bind(&pipe_name()).unwrap();
        let own = own_token().unwrap().user_sid;
        let listening = server.listening.lock().unwrap();
        let read_back = security_of(listening.as_ref().unwrap());
        let read_write = FILE_GENERIC_READ | FILE_GENERIC_WRITE;
        assert!(read_back.protected);
        assert_eq!(read_back.allowed, vec![(own, read_write), (SYSTEM_SID.to_string(), read_write)]);
        let no_up = NO_WRITE_UP | NO_READ_UP;
        assert_eq!(read_back.labels, vec![(HIGH_INTEGRITY_SID.to_string(), no_up)]);
    }

    #[test]
    fn the_pipe_rejects_remote_clients_as_written() {
        // Design part 12.4's "as written": no client from another machine can be made here, so the test reads the
        // flags the code passes and what Windows reports of the instance; neither GetNamedPipeInfo nor
        // GetNamedPipeHandleStateW reports the reject flag itself.
        assert_ne!(PIPE_MODE & PIPE_REJECT_REMOTE_CLIENTS, 0);
        let server = PipeServer::bind(&pipe_name()).unwrap();
        let listening = server.listening.lock().unwrap();
        let handle = listening.as_ref().unwrap().as_raw_handle() as HANDLE;
        let (mut flags, mut out_size, mut in_size, mut instances) = (0u32, 0u32, 0u32, 0u32);
        // SAFETY: `handle` is the server's open instance, kept open by the guard for the call; the four out pointers
        // are locals.
        let read = unsafe { GetNamedPipeInfo(handle, &mut flags, &mut out_size, &mut in_size, &mut instances) };
        assert_ne!(read, 0, "{}", io::Error::last_os_error());
        assert_ne!(flags & PIPE_SERVER_END, 0);
        assert_eq!(flags & PIPE_TYPE_MESSAGE, 0, "the byte type (PIPE_TYPE_BYTE is 0)");
        assert_eq!(instances, PIPE_UNLIMITED_INSTANCES);
    }

    #[test]
    fn a_lowered_token_cannot_open_the_pipe() {
        let name = pipe_name();
        let _server = PipeServer::bind(&name).unwrap();
        let opened = thread::spawn(move || as_medium_integrity(|| open_client(&name).err())).join().unwrap();
        // ERROR_ACCESS_DENIED: the high label refuses a medium-integrity token of the same user (P6). This tests the
        // label, not the engine's own token check.
        assert_eq!(opened, Some(5));
    }

    #[test]
    fn a_client_of_another_user_is_refused_with_permission_denied_naming_its_process() {
        let name = pipe_name();
        let server = PipeServer::bind_as(&name, OTHER_SID).unwrap();
        let refusal = format!("process {}: its user differs from this process's user", std::process::id());
        // The second client reaches the next accept and is refused the same way: the server serves on.
        for _ in 0..2 {
            let mut client = open_client(&name).unwrap();
            let Err(error) = server.accept() else { panic!("a client of another user was handed out") };
            assert_eq!((error.kind(), error.to_string()), (io::ErrorKind::PermissionDenied, refusal.clone()));
            assert_eq!(line(&mut *client.reader), "");
        }
    }

    #[test]
    fn a_push_to_a_silent_client_does_not_block() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut client = open_client(&name).unwrap();
        let IpcStream { reader, mut writer } = server.accept().unwrap();
        // The server's reader waits for a line the silent client never sends (K12).
        let pending = thread::spawn(move || {
            let mut reader = reader;
            line(&mut *reader)
        });
        thread::sleep(Duration::from_millis(100));
        // As spawn_writer writes each line (ipc_server.rs:189): write_all, then flush.
        let writer = within(NO_BLOCK, move || {
            for n in 0..10 {
                writer.write_all(format!("{{\"n\":{n}}}\n").as_bytes()).unwrap();
                writer.flush().unwrap();
            }
            writer
        });
        for n in 0..10 {
            assert_eq!(line(&mut *client.reader), format!("{{\"n\":{n}}}\n"));
        }
        drop(client);
        assert_eq!(within(NO_BLOCK, move || pending.join().unwrap()), "");
        drop(writer);
    }

    #[test]
    fn two_clients_at_once_each_read_their_own_lines() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut first = open_client(&name).unwrap();
        let mut to_first = server.accept().unwrap();
        let mut second = open_client(&name).unwrap();
        let mut to_second = server.accept().unwrap();
        to_second.writer.write_all(b"second\n").unwrap();
        to_first.writer.write_all(b"first\n").unwrap();
        assert_eq!(line(&mut *first.reader), "first\n");
        assert_eq!(line(&mut *second.reader), "second\n");
    }

    #[test]
    fn the_name_stays_held_while_a_connection_is_handed_out() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let _first = open_client(&name).unwrap();
        let _handed_out = server.accept().unwrap();
        // The next instance exists before accept returns, so a client neither finds the pipe busy (231) nor finds no
        // pipe (2), and needs no wait.
        assert_eq!(open_client(&name).err(), None);
    }

    #[test]
    fn the_last_queued_line_reaches_the_client_before_the_pipe_closes() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut client = open_client(&name).unwrap();
        let IpcStream { reader, mut writer } = server.accept().unwrap();
        let reading = thread::spawn(move || {
            let mut reader = reader;
            line(&mut *reader)
        });
        let ok = "{\"v\":1,\"type\":\"ok\",\"id\":1,\"payload\":{\"result\":null}}\n";
        writer.write_all(ok.as_bytes()).unwrap();
        drop(writer);
        // Dropping the writer ends the server's own pending read, so its reader half goes and the handle closes,
        // with the line still unread by the client (Review Focus 2).
        assert_eq!(within(NO_BLOCK, move || reading.join().unwrap()), "");
        assert_eq!(line(&mut *client.reader), ok);
        assert_eq!(line(&mut *client.reader), "");
    }

    #[test]
    fn a_write_after_the_client_closed_is_broken_pipe_and_the_reader_reads_end_of_file() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let client = open_client(&name).unwrap();
        let IpcStream { reader, mut writer } = server.accept().unwrap();
        drop(client);
        let ended = within(NO_BLOCK, move || {
            let mut reader = reader;
            line(&mut *reader)
        });
        assert_eq!(ended, "");
        assert_eq!(writer.write_all(b"late\n").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_client_that_opens_and_closes_before_accept_is_connection_aborted_and_the_next_is_served() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        drop(open_client(&name).unwrap());
        let Err(gone) = server.accept() else { panic!("a client that had closed was handed out") };
        assert_eq!(gone.kind(), io::ErrorKind::ConnectionAborted);
        let mut client = open_client(&name).unwrap();
        let mut stream = server.accept().unwrap();
        stream.writer.write_all(b"served\n").unwrap();
        assert_eq!(line(&mut *client.reader), "served\n");
    }

    #[test]
    fn a_client_connected_before_accept_is_handed_out() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut client = open_client(&name).unwrap();
        // ConnectNamedPipe meets ERROR_PIPE_CONNECTED (535).
        let mut stream = server.accept().unwrap();
        client.writer.write_all(b"{\"v\":1}\n").unwrap();
        assert_eq!(line(&mut *stream.reader), "{\"v\":1}\n");
    }

    #[test]
    fn close_wakes_a_waiting_accept_with_broken_pipe() {
        let name = pipe_name();
        let server = Arc::new(PipeServer::bind(&name).unwrap());
        let waiting = Arc::clone(&server);
        let (done, result) = channel();
        thread::spawn(move || {
            let _ = done.send(waiting.accept().err().map(|error| error.kind()));
        });
        thread::sleep(Duration::from_millis(100));
        assert!(result.try_recv().is_err(), "accept returned before close");
        server.close();
        assert_eq!(result.recv_timeout(NO_BLOCK).unwrap(), Some(io::ErrorKind::BrokenPipe));
        assert_eq!(server.accept().err().map(|error| error.kind()), Some(io::ErrorKind::BrokenPipe));
        // ERROR_FILE_NOT_FOUND: no instance holds the name any more, so lazymkvctl would say no engine is running.
        assert_eq!(open_client(&name).err(), Some(2));
    }

    #[test]
    fn a_line_of_four_mebibytes_and_one_byte_passes_the_transport_intact() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let IpcStream { reader: _kept, writer: mut client_writer } = open_client(&name).unwrap();
        let mut stream = server.accept().unwrap();
        let mut big = vec![b'x'; 4 * 1024 * 1024 + 1];
        big.push(b'\n');
        let sent = big.clone();
        let writing = thread::spawn(move || client_writer.write_all(&sent).map(|()| client_writer));
        let got = within(Duration::from_secs(20) * ci_factor(), move || {
            let mut got = Vec::new();
            stream.reader.read_until(b'\n', &mut got).unwrap();
            got
        });
        assert_eq!(got.len(), big.len());
        assert!(got == big, "the line came through changed");
        assert!(writing.join().unwrap().is_ok());
    }

    #[test]
    fn errors_of_a_client_gone_before_its_check_are_connection_aborted_and_none_reads_as_closed() {
        for code in [109, 232, 233] {
            assert_eq!(accept_error(code).kind(), io::ErrorKind::ConnectionAborted, "{code}");
        }
        for code in [5, 6, 87, 995] {
            assert_eq!(accept_error(code).raw_os_error(), Some(code as i32), "{code}");
        }
        assert_eq!(not_closed(io::Error::from_raw_os_error(109)).kind(), io::ErrorKind::Other);
        assert_eq!(not_closed(io::Error::from_raw_os_error(5)).raw_os_error(), Some(5));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::pipe`
Expected: a compile error: `PipeServer`, `pipe_stream`, `PIPE_MODE`, `accept_error` and `not_closed` are not found.

- [ ] **Step 3: Implement**

Every `unsafe` block has a `// SAFETY:` line; every handle is an `OwnedHandle` (`std::os::windows::io`), passed to Win32 as `as_raw_handle() as HANDLE`; every `OVERLAPPED` is a local of the call that waits for its operation's end, cancellation included, so none outlives its operation.

- `bind(name)` is `bind_with(name, own_token()?.user_sid)`, and `bind_as` passes its SID to the same private `bind_with`, which reads `own_token()` for the descriptor (`Descriptor::from_sddl(&pipe_sddl(..))`, Task 2) in both cases. It makes the manual-reset `stop` event and the first instance through the private `fn create_instance(&self, first: bool) -> io::Result<OwnedHandle>`: `CreateNamedPipeW(name, PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | (FILE_FLAG_FIRST_PIPE_INSTANCE when first), PIPE_MODE, PIPE_UNLIMITED_INSTANCES, PIPE_BUFFER, PIPE_BUFFER, 0, &self.descriptor.attributes())`. For the first instance, `ERROR_ACCESS_DENIED` becomes `AddrInUse` with the bind text.
- `accept`, in this order (Ruling W10):
  1. `closed` set: `Err(BrokenPipe, "the pipe server is closed")`.
  2. Take the instance from `listening`, or create one (`create_instance(false)`, its error through `not_closed`) when an earlier accept left none. If `closed` is now set, drop it and return `BrokenPipe`.
  3. `ConnectNamedPipe` with a fresh manual-reset event in a local `OVERLAPPED`. A nonzero return, or `ERROR_PIPE_CONNECTED` (535), is a connection. `ERROR_IO_PENDING` (997) waits with `WaitForMultipleObjects([connect event, stop], FALSE, INFINITE)`:
     - `stop`: `CancelIoEx(instance, &overlapped)`, then `GetOverlappedResult(.., TRUE)` to wait for the cancellation, then drop the instance and return `BrokenPipe`;
     - the connect event: `GetOverlappedResult(.., FALSE)`.

     Any failure code is kept as `accept_error(code)`.
  4. Create the next instance and store it in `listening`, under the `listening` lock and only while `closed` is unset; this happens before the connected instance is handed out or dropped, so the name is never unheld. If the creation fails, drop the connected instance and return `not_closed(<that error>)`.
  5. A failure kept in step 3: drop the instance and return it.
  6. The client check: `GetNamedPipeClientProcessId`, `token_of_process`, then `token_verdict(&facts, &self.own_user_sid)`, with the texts above. On a refusal, drop the instance (no `DisconnectNamedPipe`) and return the error.
  7. Return `pipe_stream(instance)`, an error of it through `not_closed`.
- `close`: under the `listening` lock, store `closed` and take the listening instance, then `SetEvent(stop)`; the dropped instance closes. `stop` stays set, so an accept that starts its wait after `close` returns at once.
- `pipe_stream(handle)`: one `Arc` holds the handle and a manual-reset closing event. Each half makes its own manual-reset event.
  - The reader half is wrapped in `BufReader::with_capacity(PIPE_BUFFER as usize, ..)`. Its `read`: `Ok(0)` when the closing event is already set; otherwise `ReadFile` with its event. On `ERROR_IO_PENDING` it waits with `WaitForMultipleObjects([read event, closing], FALSE, INFINITE)`, so a completed read wins over the closing event. On closing it runs `CancelIoEx(handle, &overlapped)` and `GetOverlappedResult(.., TRUE)` and returns the bytes that completed, which is `Ok(0)` when none did. `ERROR_BROKEN_PIPE` (109) and `ERROR_OPERATION_ABORTED` (995) after closing are `Ok(0)`; any other code is `io::Error::from_raw_os_error`.
  - The writer half's `write`: `WriteFile` with its own event, waiting on that event alone, and returns the bytes written. A failure is `io::Error::from_raw_os_error`, so `ERROR_NO_DATA` (232) and 109 read as `BrokenPipe` (Ruling T8-e). `flush` returns `Ok(())`. `Drop` calls `SetEvent(closing)` and nothing that waits.
- `accept_error` and `not_closed` as their docs say.
- `win/mod.rs`: `pub use pipe::PipeServer;`.

Only if `the_last_queued_line_reaches_the_client_before_the_pipe_closes` fails because the client reads the end before the line (Windows discarding unread bytes when the server's handle closes): add `pub(crate) const FLUSH_BOUND: Duration = Duration::from_secs(2);` with the doc "the longest a writer's drop waits for the peer to read what was written (Ruling W10's fallback). Def: T3-11. Use: T3-11." The writer's `Drop` then starts a helper thread, which holds the shared `Arc`, runs `FlushFileBuffers(handle)` and signals a channel; the drop waits on that channel at most `FLUSH_BOUND` before it sets the closing event. Record the outcome in the phase 2 notes (Task 26). Without that failure no flush is added, so no constant goes unused.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::pipe`
Expected: PASS, the 15 tests of `win::pipe::tests`, none printing `skipped:`. The mini PC's Secure Shell (SSH) session is elevated (design part 3.2), and every test uses a pipe of its own, so a desktop engine on `\\.\pipe\lazymkv` is never met.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/win/pipe.rs crates/platform/src/win/mod.rs crates/platform/src/testutil.rs
git commit -m "platform: the named pipe server, overlapped, with its access list and token check (design parts 3.3, 3.4; K12, P6)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 12: `lazymkvctl`'s pipe client (design parts 3.3, 9.6; S38; W11)

**Files:**
- Create: `crates/platform/src/win/client.rs` (`PipeConnector`, `BUSY_WAIT_MS`; 5 tests).
- Modify: `crates/platform/src/security.rs` (Task 2): `connect_error` and two tests.
- Modify: `crates/platform/src/win/mod.rs` (`mod client; pub use client::PipeConnector;`).
- Modify: `crates/platform/src/lib.rs:156-157` (`ConnectError::Refused`'s doc names the server check, Ruling W11).
- Modify: `crates/ctl/src/main.rs:1-10` (the connector under `cfg(windows)`), `crates/ctl/src/lib.rs:11-17` (the `Env` and `Env.connector` docs).
- Modify: `crates/ctl/src/engine_cmds.rs` (two `#[cfg(windows)]` tests at the end of `mod tests`, after `retune_jobs_sends_the_spec_path_made_absolute`, :646-656).

**Interfaces:**
- Consumes: `pipe_stream`, `PIPE_MODE`, `PIPE_BUFFER`, `PipeServer` and `testutil::{within, line}` (Task 11); `token_verdict`, `test_name`, `PIPE_NAME` (re-exported from `lib.rs`), `own_token`, `token_of_process` and `win::wide` (Task 2); `ConnectError`, `IpcConnector`, `IpcStream` (lib.rs:137-168); `NOT_RUNNING` (ctl/src/lib.rs:33) and the test helpers `call` and `SNAPSHOT` (engine_cmds.rs:391-399).
- Produces:

```rust
// crates/platform/src/security.rs (all systems)
/// The `ConnectError` of the pipe client's failed open with OS code `raw_os_error` (design part 9.6; Ruling W11):
/// `ERROR_FILE_NOT_FOUND` (2) is NotRunning; `ERROR_ACCESS_DENIED` (5) is NotElevated when this process is not
/// elevated, else Refused; any other code, `ERROR_PIPE_BUSY` (231) after the one wait among them, is Refused.
/// Def: T3-12. Use: T3-12.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn connect_error(raw_os_error: i32, own_elevated: bool) -> ConnectError;

// crates/platform/src/win/client.rs (cfg(windows))
/// lazymkvctl's end of the named pipe (design parts 3.3, 9.6; S38; Ruling W11). It opens `name` with
/// `GENERIC_READ | GENERIC_WRITE`, `OPEN_EXISTING` and `FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT |
/// SECURITY_IDENTIFICATION`, so a server can at most identify the client, never impersonate it. It waits once for a
/// busy pipe, then refuses a server that is not an elevated process of its own user before it sends anything.
/// Def: T3-12. Use: T3-12, T3-23.
pub struct PipeConnector {
    name: Vec<u16>,               // wide(name)
    own_user_sid: Option<String>, // None: this process's user, read at each connect
}
impl PipeConnector {
    /// The connector for `name` (`PIPE_NAME` in the binary, a `test_name` pipe in tests). Def: T3-12. Use: T3-12, T3-23.
    pub fn new(name: &str) -> PipeConnector;
    /// `new`, but the server is checked against `own_user_sid`. Def: T3-12. Use: T3-12.
    #[cfg(test)]
    pub(crate) fn new_as(name: &str, own_user_sid: &str) -> PipeConnector;
}
impl IpcConnector for PipeConnector {
    fn connect(&self) -> Result<IpcStream, ConnectError>; // Step 3's order
}
/// How long the client waits, once, for a busy pipe, in milliseconds (Ruling W11). Def: T3-12. Use: T3-12.
pub(crate) const BUSY_WAIT_MS: u32 = 5000;
```

`crates/ctl/src/main.rs`, whole:

```rust
//! lazymkvctl's binary (design parts 4.1, 9.5, 9.6). It hands its arguments, context and streams to `lazymkv_ctl::run`.
//! On Windows the engine commands reach the engine through the named pipe's client (design part 3.3); on Linux and
//! macOS there is no transport, and they exit 4 with NO_TRANSPORT (S38).
fn main() {
    // As lazymkv-dev's main reads them, so a non-Unicode argument cannot panic and the exit codes stay identical (P23).
    let argv: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let ctx = lazymkv_dev::evidence::Ctx::from_process();
    #[cfg(windows)]
    let pipe = lazymkv_platform::win::PipeConnector::new(lazymkv_platform::PIPE_NAME);
    #[cfg(windows)]
    let connector: Option<&dyn lazymkv_platform::IpcConnector> = Some(&pipe);
    #[cfg(not(windows))]
    let connector: Option<&dyn lazymkv_platform::IpcConnector> = None;
    let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    let code = lazymkv_ctl::run(&argv, lazymkv_ctl::Env { ctx: &ctx, connector, stdin: &mut stdin.lock(),
        out: &mut stdout.lock(), err: &mut stderr.lock() });
    std::process::exit(code);
}
```

The docs in `crates/ctl/src/lib.rs:11-17`:
- `Env`: `What lazymkvctl reads and writes: the dev context, the engine connector (the named pipe's client on Windows, design part 3.3; None on Linux and macOS, S38), and the three streams (design parts 9.5, 9.6). Def: T27, T3-12. Use: T27, T28, T3-12.`
- `Env.connector`: `The way to the engine; None when the build has no transport, as on Linux and macOS (S38).`

`ConnectError::Refused`'s doc (lib.rs:156): `Something listens and turned the client away, or it is not an elevated process of the client's user and the client turned it away (design part 3.3; Ruling W11).`

Pinned by `ctl`'s existing tests and left unchanged: the four exit 4 messages and their mapping in `open` (`crates/ctl/src/engine_cmds.rs:232-241`), and `no_connector_exits_4_with_the_no_transport_message` for `connector: None`. No test runs the `lazymkvctl` binary against the production pipe: on the mini PC it would reach the desktop task's engine. Task 25's console run exercises `main.rs`'s connector.

Owner decision 15 applies to `win::client`'s tests and the two `ctl` tests exactly as to Task 11's.

- [ ] **Step 1: Write the failing tests**

Create `crates/platform/src/win/client.rs` holding its module doc and the test module only, and add `mod client;` to `win/mod.rs`.

In `crates/platform/src/security.rs`'s `mod tests`:

```rust
    #[test]
    fn access_denied_is_not_elevated_only_when_this_process_is_not() {
        assert_eq!(connect_error(5, false), ConnectError::NotElevated);
        assert_eq!(connect_error(5, true), ConnectError::Refused);
    }

    #[test]
    fn file_not_found_is_not_running_and_any_other_error_is_refused() {
        assert_eq!(connect_error(2, true), ConnectError::NotRunning);
        assert_eq!(connect_error(2, false), ConnectError::NotRunning);
        for code in [231, 109, 87, 1326] {
            assert_eq!(connect_error(code, true), ConnectError::Refused, "{code}");
            assert_eq!(connect_error(code, false), ConnectError::Refused, "{code}");
        }
    }
```

In `crates/platform/src/win/client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::test_name;
    use crate::testutil::{line, within};
    use crate::win::pipe::{PIPE_BUFFER, PIPE_MODE};
    use crate::win::{wide, PipeServer};
    use crate::{ConnectError, IpcConnector, IpcStream, IpcTransport};
    use std::io::{Read, Write};
    use std::os::windows::io::{AsRawHandle, OwnedHandle};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe};

    /// The bound of a "does not block" proof (K12), never scaled.
    const NO_BLOCK: Duration = Duration::from_secs(5);
    /// A SID that is not the test's own user.
    const OTHER_SID: &str = "S-1-5-21-1-2-3-1001";
    const SNAPSHOT: &str = r#"{"v":1,"type":"snapshot","id":null,"payload":{"jobs":[]}}"#;

    fn pipe_name() -> String {
        format!(r"\\.\pipe\{}", test_name("client"))
    }

    /// A synchronous instance of `name` with room for one instance and Windows' default security, as a busy engine or
    /// a squatter holds a pipe: `CreateNamedPipeW(PIPE_ACCESS_DUPLEX, PIPE_MODE, 1, PIPE_BUFFER, PIPE_BUFFER, 0, null)`.
    fn single_instance(name: &str) -> OwnedHandle;

    #[test]
    fn no_server_is_not_running() {
        assert_eq!(PipeConnector::new(&pipe_name()).connect().err(), Some(ConnectError::NotRunning));
    }

    #[test]
    fn a_server_of_this_user_elevated_is_reached_and_its_snapshot_line_read() {
        let name = pipe_name();
        let server = Arc::new(PipeServer::bind(&name).unwrap());
        let serving = Arc::clone(&server);
        let peer = thread::spawn(move || {
            let mut stream = serving.accept().unwrap();
            stream.writer.write_all(format!("{SNAPSHOT}\n").as_bytes()).unwrap();
            stream
        });
        let mut client = PipeConnector::new(&name).connect().unwrap();
        assert_eq!(line(&mut *client.reader), format!("{SNAPSHOT}\n"));
        drop(peer.join().unwrap());
    }

    #[test]
    fn a_server_of_another_user_is_refused_before_anything_is_sent() {
        let name = pipe_name();
        let server = Arc::new(PipeServer::bind(&name).unwrap());
        let serving = Arc::clone(&server);
        let peer = thread::spawn(move || match serving.accept() {
            Ok(mut stream) => {
                let mut seen = Vec::new();
                stream.reader.read_to_end(&mut seen).unwrap();
                seen
            }
            // The client refused the server and left before this accept met it (232, ConnectionAborted): it sent
            // nothing either way.
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionAborted => Vec::new(),
            Err(error) => panic!("{error}"),
        });
        assert_eq!(PipeConnector::new_as(&name, OTHER_SID).connect().err(), Some(ConnectError::Refused));
        assert_eq!(within(NO_BLOCK, move || peer.join().unwrap()), Vec::<u8>::new());
    }

    #[test]
    fn a_busy_pipe_is_waited_for_once() {
        // Freed during the wait: the second client connects once the instance listens again.
        let name = pipe_name();
        let instance = single_instance(&name);
        let first = PipeConnector::new(&name).connect().unwrap();
        let freer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            let handle = instance.as_raw_handle() as HANDLE;
            // SAFETY: `handle` is the instance this thread owns, open for both calls; ConnectNamedPipe on this
            // synchronous instance takes no OVERLAPPED and returns once a client connects.
            unsafe {
                DisconnectNamedPipe(handle);
                ConnectNamedPipe(handle, std::ptr::null_mut());
            }
            instance
        });
        let started = Instant::now();
        let second = PipeConnector::new(&name).connect();
        assert!(second.is_ok(), "{:?}", second.as_ref().err());
        assert!(started.elapsed() >= Duration::from_millis(400), "the pipe was not busy");
        drop((first, second, freer.join().unwrap()));
        // Still busy after the one wait: Refused, once BUSY_WAIT_MS has passed.
        let name = pipe_name();
        let _instance = single_instance(&name);
        let _occupant = PipeConnector::new(&name).connect().unwrap();
        let started = Instant::now();
        assert_eq!(PipeConnector::new(&name).connect().err(), Some(ConnectError::Refused));
        assert!(started.elapsed() >= Duration::from_millis(u64::from(BUSY_WAIT_MS) - 500));
    }

    #[test]
    fn the_client_and_the_server_each_write_while_the_other_reads() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let IpcStream { reader: client_reader, writer: mut client_writer } = PipeConnector::new(&name).connect().unwrap();
        let IpcStream { reader: server_reader, writer: mut server_writer } = server.accept().unwrap();
        let request = "{\"v\":1,\"type\":\"history\",\"id\":1,\"payload\":{}}\n";
        let reply = "{\"v\":1,\"type\":\"ok\",\"id\":1,\"payload\":{\"result\":[]}}\n";
        // Both readers wait first, so each end's writer writes while its own reader's read is pending (K12).
        let client_read = thread::spawn(move || {
            let mut reader = client_reader;
            line(&mut *reader)
        });
        let server_read = thread::spawn(move || {
            let mut reader = server_reader;
            line(&mut *reader)
        });
        thread::sleep(Duration::from_millis(100));
        let writers = within(NO_BLOCK, move || {
            client_writer.write_all(request.as_bytes()).unwrap();
            server_writer.write_all(reply.as_bytes()).unwrap();
            (client_writer, server_writer)
        });
        assert_eq!(within(NO_BLOCK, move || server_read.join().unwrap()), request);
        assert_eq!(within(NO_BLOCK, move || client_read.join().unwrap()), reply);
        drop(writers);
    }
}
```

In `crates/ctl/src/engine_cmds.rs`'s `mod tests`, after `retune_jobs_sends_the_spec_path_made_absolute`:

```rust
    #[cfg(windows)]
    #[test]
    fn ctl_status_over_a_test_pipe_prints_the_engines_reply() {
        use lazymkv_platform::security::test_name;
        use lazymkv_platform::win::{PipeConnector, PipeServer};
        use std::io::Read;
        let scratch = Scratch::new("ctl-pipe");
        let name = format!(r"\\.\pipe\{}", test_name("ctl"));
        let server = PipeServer::bind(&name).unwrap();
        let peer = thread::spawn(move || {
            let mut stream = server.accept().unwrap();
            writeln!(stream.writer, "{SNAPSHOT}").unwrap();
            let mut sent = Vec::new();
            stream.reader.read_to_end(&mut sent).unwrap();
            sent
        });
        let connector = PipeConnector::new(&name);
        let (code, out, err) = call(&["status"], Some(&connector), b"", &scratch.0);
        assert_eq!((code, out.as_str(), err.as_str()), (0, "{\n \"jobs\": []\n}\n", ""));
        // status sends nothing (T28-c): the peer reads the end at once.
        assert_eq!(peer.join().unwrap(), Vec::<u8>::new());
    }

    #[cfg(windows)]
    #[test]
    fn ctl_status_with_no_engine_on_a_test_pipe_exits_4_saying_none_is_running() {
        use lazymkv_platform::security::test_name;
        use lazymkv_platform::win::PipeConnector;
        let scratch = Scratch::new("ctl-pipe-none");
        let connector = PipeConnector::new(&format!(r"\\.\pipe\{}", test_name("ctl-none")));
        let (code, out, err) = call(&["status"], Some(&connector), b"", &scratch.0);
        assert_eq!((code, out.as_str(), err), (4, "", format!("{NOT_RUNNING}\n")));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform -p lazymkv-ctl --locked`
Expected: a compile error: `connect_error`, `PipeConnector` and `BUSY_WAIT_MS` are not found, and `lazymkv_platform::win::PipeConnector` is unresolved in `ctl`'s tests.

- [ ] **Step 3: Implement**

- `security::connect_error`: the match of its doc. It is all-system and pure, so `ctl`'s and the client's mapping is tested on every system; outside Windows only the tests call it, hence the `cfg_attr`.
- `PipeConnector::new` stores `wide(name)` and None; `new_as` stores the SID. `connect`, every `unsafe` block with a `// SAFETY:` line:
  1. `own_token()`; an error is `Refused`.
  2. `CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, null, OPEN_EXISTING, FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, null)`. On `ERROR_PIPE_BUSY` (231): `WaitNamedPipeW(name, BUSY_WAIT_MS)`, whatever it returns, then `CreateFileW` once more. A failed open is `Err(connect_error(code, own.elevated))`.
  3. `GetNamedPipeServerProcessId(handle, &mut pid)`; a failure is `Refused`.
  4. `token_of_process(pid)`, then `token_verdict(&facts, sid)`, `sid` being the `new_as` SID or `own.user_sid`; an error of either is `Refused`. The handle is dropped on each refusal, before anything is written.
  5. `pipe_stream(handle)`; an error is `Refused`.
- `win/mod.rs`: `pub use client::PipeConnector;`.
- `crates/ctl/src/main.rs`, the `Env` docs and `ConnectError::Refused`'s doc: the text above.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform -p lazymkv-ctl --locked`
Expected: PASS, `win::client::tests` and the two `cfg(windows)` `ctl` tests among them, none printing `skipped:`. `a_busy_pipe_is_waited_for_once` takes about 5.5 s, most of it the one wait of `BUSY_WAIT_MS`.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS; on the mini PC every PR 2 `ctl` test passes, `no_connector_exits_4_with_the_no_transport_message` among them.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/security.rs crates/platform/src/win/client.rs crates/platform/src/win/mod.rs crates/platform/src/lib.rs crates/ctl/src/main.rs crates/ctl/src/lib.rs crates/ctl/src/engine_cmds.rs
git commit -m "platform, ctl: lazymkvctl's pipe client checks the server's user and elevation (design parts 3.3, 9.6)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 13: Secrets on Credential Manager and the share credential's shape (design parts 6.1, 9.4; W12, W13)

**Files:**
- Modify: `crates/core/src/ipc.rs:30-43` (`SHARE_VALUE_SHAPE` and `share_credential` after `is_secret_name`) and its `mod tests` after `share_secret_names_are_accepted` (:761-770).
- Create: `crates/platform/src/shares.rs` (the module doc, `CREDENTIAL_PREFIX`, `credential_target` and one test; Task 14 adds the rest).
- Create: `crates/platform/src/win/creds.rs` (`CredentialStore` and four tests).
- Modify: `crates/platform/src/lib.rs:12-16` (`pub mod shares;` in the module list, after Task 7's `pub mod localtime;`), `crates/platform/src/win/mod.rs` (`mod creds; pub use creds::CredentialStore;`).
- Modify: `crates/engine/src/owner.rs:21-24` (import `share_credential`) and `:2298-2309` (`secret_set` checks a share value's shape before `set`).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:446-459` (two tests after `secret_set_refuses_an_unknown_name_and_never_echoes_the_value`).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes: `SecretStore` (`crates/platform/src/lib.rs:127-133`); `is_secret_name` and `SECRET_NAMES` (`crates/core/src/ipc.rs:31-43`); `security::test_name` and `testutil::{skip, Secrets}` (Task 2); `scripted`, `Client` and `Harness::engine_log` in `engine_ipc.rs` and `testkit.rs` (pull request 2).
- Produces:

```rust
// crates/core/src/ipc.rs
/// The text `secret_set` refuses a share credential's value with when it is not two lines (design parts 6.1, 9.4;
/// Ruling W12). It never holds the value. Def: T3-13. Use: T3-13.
pub const SHARE_VALUE_SHAPE: &str = "the value must be a user name and a password on two lines";
/// A share credential's value as (user name, password) (design parts 6.1, 9.4; Ruling W12): the text before the first
/// `\n`, with one `\r` before that `\n` dropped (a Windows console's line end, Review Focus 4), and the text after it.
/// Err(SHARE_VALUE_SHAPE) when there is no `\n`, a second `\n`, or an empty half. Pure (RP19). Def: T3-13.
/// Use: T3-13, T3-14.
pub fn share_credential(value: &str) -> Result<(&str, &str), String>;

// crates/platform/src/shares.rs (all systems)
/// The prefix of lazymkv's Credential Manager entries (design part 6.1; Ruling W13): the secret
/// `share:\\server\share` is the entry `lazymkv:share:\\server\share`, and `tmdb_token` the entry
/// `lazymkv:tmdb_token`. Only `CredentialStore::new` uses it; tests give a prefix of their own (Global Constraints).
/// Def: T3-13. Use: T3-13.
pub const CREDENTIAL_PREFIX: &str = "lazymkv:";
/// The Credential Manager target of the secret `name` under `prefix`: the two joined (design part 6.1; Ruling W13).
/// Def: T3-13. Use: T3-13.
pub fn credential_target(prefix: &str, name: &str) -> String;

// crates/platform/src/win/creds.rs (cfg(windows))
/// Credential Manager as the `SecretStore` (spec 4.3; design parts 4.2, 6.1; Ruling W13; Owner decision 8): each
/// secret is a generic credential with the target `credential_target(prefix, name)`, the user name `lazymkv`, the
/// value's UTF-8 bytes as its blob (2560 bytes at most) and the persistence `CRED_PERSIST_LOCAL_MACHINE`. `get` is None
/// on every error, so in a logon session with no vault (a key-based Secure Shell (SSH) logon, design part 6.1) a
/// stored secret reads as unset. `set`'s Err never holds the value. It lists nothing (S77 stays).
/// Def: T3-13. Use: T3-14, T3-23.
pub struct CredentialStore { prefix: String }
impl CredentialStore {
    /// The engine's store, under CREDENTIAL_PREFIX (design part 6.1; Ruling W13). Def: T3-13. Use: T3-14, T3-23.
    pub fn new() -> CredentialStore;
    pub(crate) fn with_prefix(prefix: &str) -> CredentialStore;              // tests: format!("{}:", test_name(tag))
    #[cfg(test)] pub(crate) fn remove(&self, name: &str) -> Result<(), String>; // Ok also when nothing is stored
}
impl SecretStore for CredentialStore { /* get, set */ }
```

Texts `CredentialStore` writes (Ruling W13; `<error>` is `std::io::Error::from_raw_os_error(<code>)`):
- `set` of a value over 2560 bytes, before any Windows call: `the value is longer than 2560 bytes, the most a Credential Manager entry holds` (a private `TOO_LONG`).
- `set` refused by `CredWriteW`: `Credential Manager refused the entry: <error>`.
- `remove` refused by `CredDeleteW` with any code but `ERROR_NOT_FOUND` (1168): `Credential Manager could not remove the entry: <error>`.
- The engine wraps `set`'s two texts as `<name> could not be stored: <text>` (`owner.rs:2304`), and a malformed share value as `<name> could not be stored: the value must be a user name and a password on two lines`.

- [ ] **Step 1: Write the failing tests**

Create `crates/platform/src/shares.rs` and `crates/platform/src/win/creds.rs` holding only their test modules, and add `pub mod shares;` to `lib.rs` and `mod creds;` to `win/mod.rs`, so the crate builds without the tests.

In `crates/core/src/ipc.rs`'s `mod tests`, after `share_secret_names_are_accepted`:

```rust
    #[test]
    fn a_credential_typed_in_a_windows_console_splits_into_user_and_password() {
        let parts = Ok(("user-sentinel-7c1e", "pass-sentinel-4b9d"));
        assert_eq!(share_credential("user-sentinel-7c1e\r\npass-sentinel-4b9d"), parts);
        assert_eq!(share_credential("user-sentinel-7c1e\npass-sentinel-4b9d"), parts);
    }

    #[test]
    fn one_line_three_lines_and_an_empty_half_are_refused_without_the_value() {
        for value in [
            "",
            "user-sentinel-7c1e",
            "user-sentinel-7c1e\npass-sentinel-4b9d\nthird",
            "user-sentinel-7c1e\r\npass-sentinel-4b9d\r\n",
            "\npass-sentinel-4b9d",
            "\r\npass-sentinel-4b9d",
            "user-sentinel-7c1e\n",
            "user-sentinel-7c1e\r\n",
        ] {
            let refused = share_credential(value).unwrap_err();
            assert_eq!(refused, SHARE_VALUE_SHAPE, "{value:?}");
            assert!(!refused.contains("sentinel"), "{value:?}");
        }
    }
```

In `crates/platform/src/shares.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_targets_are_the_prefix_and_the_secret_name() {
        assert_eq!(credential_target(CREDENTIAL_PREFIX, r"share:\\server\share"), r"lazymkv:share:\\server\share");
        assert_eq!(credential_target(CREDENTIAL_PREFIX, "tmdb_token"), "lazymkv:tmdb_token");
    }
}
```

In `crates/platform/src/win/creds.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::test_name;
    use crate::testutil::skip;
    use crate::SecretStore;

    /// A store under the test's own prefix, `lazymkv-test-<tag>-<pid>-<n>:` (Global Constraints).
    fn store(tag: &str) -> CredentialStore {
        CredentialStore::with_prefix(&format!("{}:", test_name(tag)))
    }

    /// Removes the test's entry however the test ends.
    struct Removal<'a> {
        store: &'a CredentialStore,
        name: &'a str,
    }

    impl Drop for Removal<'_> {
        fn drop(&mut self) {
            let _ = self.store.remove(self.name);
        }
    }

    /// Stores `value` under `name` and returns false. Returns true after printing a skip, writing nothing, when `CI`
    /// and `LAZYMKV_TEST_VAULT` are both unset, so no run writes the owner's vault before Owner decision 8's yes
    /// (`platform-ignored.ps1` sets the variable for `win::creds::` only); and when this logon session has no vault
    /// (ERROR_NO_SUCH_LOGON_SESSION, 1312; design part 6.1) and `CI` is unset. On continuous integration (CI) it never
    /// skips, unless Owner decision 15's vault fallback is taken.
    fn stored(store: &CredentialStore, name: &str, value: &str) -> bool {
        if std::env::var_os("CI").is_none() && std::env::var_os("LAZYMKV_TEST_VAULT").is_none() {
            skip("LAZYMKV_TEST_VAULT is not set; the vault tests run on CI and, after Owner decision 8, in the desktop session (design part 6.1)");
            return true;
        }
        let Err(error) = store.set(name, value) else { return false };
        assert!(error.starts_with("Credential Manager refused the entry: "), "{error}");
        assert!(!error.contains("sentinel"), "{error}");
        if error.contains("(os error 1312)") && std::env::var_os("CI").is_none() {
            skip("no credential vault in this logon session (design part 6.1)");
            return true;
        }
        panic!("{error}");
    }

    #[test]
    fn a_test_secret_round_trips_and_is_removed() {
        let store = store("creds");
        let _removal = Removal { store: &store, name: "tmdb_token" };
        if stored(&store, "tmdb_token", "pass-sentinel-4b9d") {
            return;
        }
        assert_eq!(store.get("tmdb_token").as_deref(), Some("pass-sentinel-4b9d"));
        assert!(!stored(&store, "tmdb_token", "pass-sentinel-4b9d-2"));
        assert_eq!(store.get("tmdb_token").as_deref(), Some("pass-sentinel-4b9d-2"));
        store.remove("tmdb_token").unwrap();
        assert_eq!(store.get("tmdb_token"), None);
        store.remove("tmdb_token").unwrap();
    }

    #[test]
    fn an_absent_secret_is_none() {
        assert_eq!(store("creds-absent").get(r"share:\\server\share"), None);
    }

    #[test]
    fn a_value_over_2560_bytes_is_refused_without_the_value() {
        let store = store("creds-long");
        let value = "pass-sentinel-4b9d".repeat(143); // 2574 bytes
        let refused = store.set("tmdb_token", &value).unwrap_err();
        assert_eq!(refused, "the value is longer than 2560 bytes, the most a Credential Manager entry holds");
        assert_eq!(store.get("tmdb_token"), None);
    }

    /// Review Focus 1: only Credential Manager ignores case, so this is the one test of the case rule.
    #[test]
    fn a_secret_is_found_by_its_name_in_another_case() {
        let store = store("creds-case");
        let _removal = Removal { store: &store, name: r"share:\\server\share" };
        if stored(&store, r"share:\\server\share", "user-sentinel-7c1e\npass-sentinel-4b9d") {
            return;
        }
        assert_eq!(store.get(r"share:\\SERVER\Share").as_deref(), Some("user-sentinel-7c1e\npass-sentinel-4b9d"));
    }
}
```

In `crates/fake-tools/tests/engine_ipc.rs`, after `secret_set_refuses_an_unknown_name_and_never_echoes_the_value`:

```rust
#[test]
fn a_share_value_of_three_lines_is_refused_without_echoing_it() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    let name = r"share:\\nas\media";
    let value = "user-sentinel-7c1e\r\npass-sentinel-4b9d\r\nthird";
    let message = c.err("secret_set", json!({"name": name, "value": value}));
    assert_eq!(message, format!("{name} could not be stored: the value must be a user name and a password on two lines"));
    let mut d = Client::open(&h);
    let line = d.line().unwrap();
    assert!(!line.contains("sentinel"), "{line}");
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["payload"]["secrets"].get(name), None);
    assert!(!h.engine_log().contains("sentinel"));
}

#[test]
fn a_two_line_share_value_is_stored() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    let name = r"share:\\nas\media";
    let value = "user-sentinel-7c1e\r\npass-sentinel-4b9d";
    assert_eq!(c.ok("secret_set", json!({"name": name, "value": value})), Value::Null);
    let mut d = Client::open(&h);
    let line = d.line().unwrap();
    assert!(!line.contains("sentinel"), "{line}");
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["payload"]["secrets"][name], json!({"set": true, "host": null}));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core -p lazymkv-platform --locked`
Expected: a compile error: `share_credential` and `SHARE_VALUE_SHAPE` are not found in `ipc`, and `credential_target`, `CREDENTIAL_PREFIX` and `CredentialStore` are not found.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc`
Expected: FAIL in `a_share_value_of_three_lines_is_refused_without_echoing_it` (the panic `secret_set: Ok { .. }`, since the engine stores any value). `a_two_line_share_value_is_stored` passes already; it pins the accepted shape.

- [ ] **Step 3: Implement**

- `core::ipc::share_credential` and `platform::shares::credential_target` as their docs and tests fix them. The `shares` module doc says it holds the parts of design part 6.1 that need no Windows call, compiled and tested on every system (design part 4.2).
- `platform::win::creds`, every `unsafe` block with a `// SAFETY:` line, every wide string ending in one 0:
  - `get`: `CredReadW(<wide target>, CRED_TYPE_GENERIC, 0, &mut credential)`; on success the blob's `CredentialBlobSize` bytes read as UTF-8 (a size of 0 is the empty string, and the pointer is not read; bytes that are not UTF-8 give None), then `CredFree`. Any failure gives None (W13).
  - `set`: a value over 2560 bytes gives `TOO_LONG` before any call. Otherwise `CredWriteW(&CREDENTIALW { Type: CRED_TYPE_GENERIC, TargetName: <wide target>, CredentialBlobSize: <len>, CredentialBlob: <the value's bytes>, Persist: CRED_PERSIST_LOCAL_MACHINE, UserName: <wide "lazymkv">, every other field 0 or null }, 0)`; a zero return gives `Credential Manager refused the entry: <error>` from `GetLastError()`. The persistence is Owner decision 8's; an overturn changes this one constant.
  - `remove` (`#[cfg(test)]`): `CredDeleteW(<wide target>, CRED_TYPE_GENERIC, 0)`, Ok on success and on `ERROR_NOT_FOUND`, else the remove text.
- `engine::owner::Owner::secret_set`: after the `is_secret_name` check and before `set`, a name outside `SECRET_NAMES` runs `share_credential(value)`, and its Err is returned as `format!("{name} could not be stored: {shape}")`. The method's doc adds Ruling W12.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core -p lazymkv-platform --locked`
Expected: PASS. Over SSH on the mini PC, `a_test_secret_round_trips_and_is_removed` and `a_secret_is_found_by_its_name_in_another_case` print `skipped: LAZYMKV_TEST_VAULT is not set; ...` and pass, writing nothing; `an_absent_secret_is_none` and `a_value_over_2560_bytes_is_refused_without_the_value` run. On `windows-latest` the two vault tests write and remove `lazymkv-test-...` entries; if they fail there with os error 1312 (no vault in the runner's session), Owner decision 15's vault fallback goes to the owner, and only on his yes does `stored` skip on CI with `skipped: no credential vault on windows-latest (design part 12.4)`.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc`
Expected: PASS, every pull request 2 test of the file among them.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/core/src/ipc.rs crates/platform/src/shares.rs crates/platform/src/win/creds.rs crates/platform/src/win/mod.rs crates/platform/src/lib.rs crates/engine/src/owner.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "platform: secrets in Credential Manager; a share credential is two lines (design parts 6.1, 9.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

The desktop-session run of `win::creds::` is Task 25 step 4, after Owner decision 8; its evidence is that step's.

---

### Task 14: The share connector (P7; design parts 6.1, 6.2; S11; V15; W14)

**Files:**
- Modify: `crates/platform/src/shares.rs` (`share_of`, `volume_key`, `share_secret_name`, `SESSION_CONFLICT`, `ShareApi`, `Shares<A>` and their tests).
- Modify: `crates/platform/src/lib.rs:116-125` (`ShareConnector::access`, a default method).
- Create: `crates/platform/src/win/shares.rs` (`Win32Shares`, `share_connector`, the test module `tests`, and the ignored tests in its test modules `desktop`, `over_ssh` and `arranged`).
- Modify: `crates/platform/src/win/mod.rs` (`mod shares; pub use shares::share_connector;`).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes: `Reach`, `ShareConnector`, `SecretStore` (`crates/platform/src/lib.rs:107-133`); `core::ipc::share_credential` (Task 13); `CredentialStore::{new, with_prefix}` (Task 13); `rename_no_replace` on `MoveFileExW` (Task 4); `security::test_name`, `testutil::{skip, temp_dir, Secrets}` (Task 2).
- Produces:

```rust
// crates/platform/src/lib.rs, in ShareConnector
    /// How the last successful `reach` of the share holding `root` got in: "local folder", "the session's access" or
    /// "lazymkv's stored credential"; None before any, after a failed one, and for a connector that does not say.
    /// For job.log's share access line (design parts 6.1, 7.1). Def: T3-14. Use: T3-17.
    fn access(&self, _root: &str) -> Option<&'static str> { None }

// crates/platform/src/shares.rs (all systems)
/// The `\\server\share` of a Universal Naming Convention (UNC) root (design part 6.1): '/' is read as '\', the first
/// two components after the leading `\\` are kept in their own case, and both must be non-empty. None for a local
/// root, for a root with fewer than two components, and for the `\\?\` and `\\.\` forms. Def: T3-14. Use: T3-14.
pub fn share_of(root: &str) -> Option<String>;
/// The volume of a root, equal for two roots on one volume (design part 6.1, S11): `share_of(root)` in ASCII lower
/// case, or `X:` in upper case for a drive-letter root (`X:\` or `X:/`); None for any other root. Lexical: no Windows
/// call, so the owner thread's `volume` calls never wait on a share. Def: T3-14. Use: T3-14.
pub fn volume_key(root: &str) -> Option<String>;
/// The secret name of a share's credential: "share:" and the share (design parts 6.1, 9.4; Ruling T7-f).
/// Def: T3-14. Use: T3-14.
pub fn share_secret_name(share: &str) -> String;
/// The Windows error a second credential for an already reached server gives, named in the connector's text (design
/// part 6.1). Def: T3-14. Use: T3-14.
pub const SESSION_CONFLICT: &str = "ERROR_SESSION_CREDENTIAL_CONFLICT";
/// The three Windows calls of the share connector (design part 6.1; Owner decision 7), behind a seam so that the
/// fallback's order is tested on every system: `probe` answers whether a share root `\\server\share\` or a drive root
/// `X:\` opens as a folder, `connect` makes a deviceless connection with a user name and a password, and `free` reads
/// the bytes available to the caller on such a root. Each Err is a Windows error code. Def: T3-14. Use: T3-14.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) trait ShareApi: Send + Sync {
    fn probe(&self, path: &str) -> Result<(), u32>;
    fn connect(&self, share: &str, user: &str, password: &str) -> Result<(), u32>;
    fn free(&self, path: &str) -> Result<u64, u32>;
}
/// The `ShareConnector` of design part 6.1 over a `ShareApi` (P7, S11, V15; Ruling W14): the session's access first,
/// lazymkv's stored credential second, a refused credential not tried again until its value changes (Review Focus 5),
/// and texts that hold no user name or password. Def: T3-14. Use: T3-14.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct Shares<A: ShareApi> {
    api: A,
    secrets: Arc<dyn SecretStore>,
    refused: Mutex<BTreeMap<String, (u64, u32)>>,   // volume_key -> (the refused value's hash, Windows' code)
    access: Mutex<BTreeMap<String, &'static str>>,  // volume_key -> how the last reach got in
}
#[cfg_attr(not(windows), allow(dead_code))]
impl<A: ShareApi> Shares<A> { pub(crate) fn new(api: A, secrets: Arc<dyn SecretStore>) -> Shares<A>; }
#[cfg_attr(not(windows), allow(dead_code))]
impl<A: ShareApi> ShareConnector for Shares<A> { /* reach, free_bytes, volume, access */ }

// crates/platform/src/win/shares.rs (cfg(windows))
pub(crate) struct Win32Shares;                 // impl ShareApi
/// The Windows `ShareConnector` (spec 4.3; design parts 4.2, 6.1; P7): `Shares` over the Windows calls, reading
/// lazymkv's credential from `secrets`. Def: T3-14. Use: T3-23.
pub fn share_connector(secrets: Arc<dyn SecretStore>) -> Arc<dyn ShareConnector>;
```

`ShareApi`, `Shares` and its two `impl` blocks carry `#[cfg_attr(not(windows), allow(dead_code))]`, because outside Windows only the tests build them; the private helpers they call are then live too.

- [ ] **Step 1: Write the failing tests**

In `crates/platform/src/shares.rs`'s `mod tests` (Task 13's test stays):

```rust
    use std::collections::VecDeque;
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::{Arc, Mutex};

    use crate::testutil::Secrets;
    use crate::{Reach, SecretStore, ShareConnector};

    const ROOT: &str = r"\\nas\media\lazymkv-staging\engine";
    const NAME: &str = r"share:\\nas\media";
    const CREDENTIAL: &str = "user-sentinel-7c1e\npass-sentinel-4b9d";

    /// The text of a Windows error code, as the connector writes it.
    fn e(code: u32) -> String {
        io::Error::from_raw_os_error(code as i32).to_string()
    }

    /// A scripted ShareApi: `probe` answers from `answers` in order and repeats the last one, `connect` and `free` give
    /// their fields, and every call is recorded.
    struct Api {
        answers: Mutex<VecDeque<Result<(), u32>>>,
        connect: Result<(), u32>,
        free: Result<u64, u32>,
        probes: Mutex<Vec<String>>,
        connects: Mutex<Vec<(String, String, String)>>,
        frees: Mutex<Vec<String>>,
    }

    fn api_free(answers: &[Result<(), u32>], connect: Result<(), u32>, free: Result<u64, u32>) -> Arc<Api> {
        Arc::new(Api {
            answers: Mutex::new(answers.iter().copied().collect()),
            connect,
            free,
            probes: Mutex::default(),
            connects: Mutex::default(),
            frees: Mutex::default(),
        })
    }

    fn api(answers: &[Result<(), u32>], connect: Result<(), u32>) -> Arc<Api> {
        api_free(answers, connect, Ok(123_456))
    }

    impl ShareApi for Arc<Api> {
        fn probe(&self, path: &str) -> Result<(), u32> {
            self.probes.lock().unwrap().push(path.to_string());
            let mut answers = self.answers.lock().unwrap();
            if answers.len() > 1 { answers.pop_front().unwrap() } else { answers.front().copied().unwrap_or(Ok(())) }
        }
        // `secret_half`, not `password`: a name CodeQL's cleartext-logging query keys on (Global Constraints).
        fn connect(&self, share: &str, user: &str, secret_half: &str) -> Result<(), u32> {
            self.connects.lock().unwrap().push((share.to_string(), user.to_string(), secret_half.to_string()));
            self.connect
        }
        fn free(&self, path: &str) -> Result<u64, u32> {
            self.frees.lock().unwrap().push(path.to_string());
            self.free
        }
    }

    fn probes(api: &Api) -> Vec<String> {
        api.probes.lock().unwrap().clone()
    }

    fn connects(api: &Api) -> Vec<(String, String, String)> {
        api.connects.lock().unwrap().clone()
    }

    fn sentinels(share: &str) -> (String, String, String) {
        (share.to_string(), "user-sentinel-7c1e".to_string(), "pass-sentinel-4b9d".to_string())
    }

    /// testutil::Secrets that counts its reads.
    struct Counted {
        inner: Secrets,
        gets: AtomicUsize,
    }

    impl SecretStore for Counted {
        fn get(&self, name: &str) -> Option<String> {
            self.gets.fetch_add(1, SeqCst);
            self.inner.get(name)
        }
        fn set(&self, name: &str, value: &str) -> Result<(), String> {
            self.inner.set(name, value)
        }
    }

    fn counted(entries: &[(&str, &str)]) -> Arc<Counted> {
        let inner = entries.iter().fold(Secrets::new(), |secrets, (name, value)| secrets.with(name, value));
        Arc::new(Counted { inner, gets: AtomicUsize::new(0) })
    }

    fn connector(api: &Arc<Api>, secrets: &Arc<Counted>) -> Shares<Arc<Api>> {
        Shares::new(Arc::clone(api), Arc::clone(secrets) as Arc<dyn SecretStore>)
    }

    #[test]
    fn the_session_access_is_tried_first_and_the_secret_store_not_read() {
        let (api, secrets) = (api(&[Ok(())], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Reachable);
        assert_eq!(probes(&api), [r"\\nas\media\"]);
        assert!(connects(&api).is_empty());
        assert_eq!(secrets.gets.load(SeqCst), 0);
    }

    #[test]
    fn access_denied_and_logon_failure_fall_back_to_the_stored_credential_once() {
        for code in [5, 1326] {
            let (api, secrets) = (api(&[Err(code), Ok(())], Ok(())), counted(&[(NAME, CREDENTIAL)]));
            assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Reachable, "{code}");
            // assert!, not assert_eq!: a failure prints no recorded half of the credential.
            assert!(connects(&api) == [sentinels(r"\\nas\media")], "{code}: connect was not called with the stored credential's two halves");
            assert_eq!(probes(&api), [r"\\nas\media\", r"\\nas\media\"], "{code}");
        }
    }

    #[test]
    fn any_other_first_error_is_unreachable_with_no_fallback() {
        for code in [53, 67, 121] {
            let (api, secrets) = (api(&[Err(code)], Ok(())), counted(&[(NAME, CREDENTIAL)]));
            let reach = connector(&api, &secrets).reach(ROOT);
            assert_eq!(reach, Reach::Unreachable(format!(r"\\nas\media does not answer: {}", e(code))));
            assert!(connects(&api).is_empty());
            assert_eq!(secrets.gets.load(SeqCst), 0);
        }
    }

    #[test]
    fn denied_with_no_readable_credential_says_so() {
        let (api, secrets) = (api(&[Err(5)], Ok(())), counted(&[]));
        let reach = connector(&api, &secrets).reach(ROOT);
        let text = format!(r"\\nas\media refused this session's access ({}), and this session can read no lazymkv credential for it", e(5));
        assert_eq!(reach, Reach::Unreachable(text));
        assert!(connects(&api).is_empty());
    }

    #[test]
    fn a_refused_connection_says_windows_refused_the_stored_credential() {
        let (api, secrets) = (api(&[Err(5)], Err(86)), counted(&[(NAME, CREDENTIAL)]));
        let reach = connector(&api, &secrets).reach(ROOT);
        let text = format!(r"\\nas\media refused this session's access ({}), and Windows refused lazymkv's stored credential: {}; it is not tried again until it changes", e(5), e(86));
        assert_eq!(reach, Reach::Unreachable(text));
        assert_eq!(probes(&api).len(), 1);
    }

    #[test]
    fn a_session_credential_conflict_is_named_and_not_retried() {
        let (api, secrets) = (api(&[Err(5)], Err(1219)), counted(&[(NAME, CREDENTIAL)]));
        let text = r"\\nas\media: ERROR_SESSION_CREDENTIAL_CONFLICT: this session already reaches the server as another user, so lazymkv's stored credential was not used";
        assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Unreachable(text.to_string()));
        assert_eq!((probes(&api).len(), connects(&api).len()), (1, 1));
    }

    #[test]
    fn a_retried_access_that_fails_again_says_so() {
        let (api, secrets) = (api(&[Err(5), Err(5)], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        let reach = connector(&api, &secrets).reach(ROOT);
        let text = format!(r"\\nas\media refused access again after connecting with lazymkv's stored credential: {}", e(5));
        assert_eq!(reach, Reach::Unreachable(text));
        assert_eq!(probes(&api).len(), 2);
    }

    #[test]
    fn a_malformed_stored_value_is_named_without_its_text() {
        for value in ["user-sentinel-7c1e", "user-sentinel-7c1e\npass-sentinel-4b9d\nthird", "\npass-sentinel-4b9d"] {
            let (api, secrets) = (api(&[Err(5)], Ok(())), counted(&[(NAME, value)]));
            let text = r"lazymkv's stored credential for \\nas\media is not a user name and a password on two lines";
            assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Unreachable(text.to_string()), "{value:?}");
            assert!(connects(&api).is_empty());
        }
    }

    #[test]
    fn no_text_holds_the_sentinel_user_or_password() {
        let cases: [(&[Result<(), u32>], Result<(), u32>, &str); 7] = [
            (&[Err(53)], Ok(()), CREDENTIAL),
            (&[Err(5)], Err(1326), CREDENTIAL),
            (&[Err(5)], Err(1219), CREDENTIAL),
            (&[Err(5)], Err(53), CREDENTIAL),
            (&[Err(5), Err(5)], Ok(()), CREDENTIAL),
            (&[Err(5)], Ok(()), "user-sentinel-7c1e"),
            (&[Err(5)], Ok(()), "pass-sentinel-4b9d\n"),
        ];
        for (answers, connect, value) in cases {
            let (api, secrets) = (api_free(answers, connect, Err(5)), counted(&[(NAME, value)]));
            let shares = connector(&api, &secrets);
            for _ in 0..2 {
                let Reach::Unreachable(text) = shares.reach(ROOT) else { panic!("{answers:?} {connect:?} reached") };
                let free = shares.free_bytes(ROOT).unwrap_err().to_string();
                for text in [text, free] {
                    assert!(!text.contains("sentinel"), "{text}");
                }
            }
        }
    }

    /// Review Focus 5.
    #[test]
    fn a_refused_stored_credential_is_not_tried_again_until_it_changes() {
        let (api, secrets) = (api(&[Err(5)], Err(1326)), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        let first = shares.reach(ROOT);
        assert_eq!(shares.reach(ROOT), first);
        assert_eq!(connects(&api).len(), 1);
        secrets.set(NAME, "user-sentinel-7c1e\npass-sentinel-4b9d-new").unwrap();
        shares.reach(ROOT);
        assert_eq!(connects(&api).len(), 2);
    }

    #[test]
    fn a_connection_the_server_does_not_answer_is_tried_again() {
        let (api, secrets) = (api(&[Err(5)], Err(53)), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.reach(ROOT), Reach::Unreachable(format!(r"\\nas\media does not answer: {}", e(53))));
        shares.reach(ROOT);
        assert_eq!(connects(&api).len(), 2);
    }

    /// Review Focus 1: the secret is stored with backslashes in the root's own case, since this store compares exactly.
    #[test]
    fn a_forward_slash_root_finds_the_backslash_secret_and_a_root_in_another_case_shares_its_volume() {
        let secrets = counted(&[(r"share:\\unas-pro\media", "user-sentinel-7c1e\r\npass-sentinel-4b9d")]);
        let api = api(&[Err(5), Ok(())], Ok(()));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.reach("//unas-pro/media/x"), Reach::Reachable);
        assert_eq!(probes(&api), [r"\\unas-pro\media\", r"\\unas-pro\media\"]);
        assert!(connects(&api) == [sentinels(r"\\unas-pro\media")], "connect was not called with the stored credential's two halves");
        assert_eq!(share_of(r"\\UNAS-PRO\Media\y").as_deref(), Some(r"\\UNAS-PRO\Media"));
        assert_eq!(shares.volume("//UNAS-PRO/Media/y"), shares.volume(r"\\unas-pro\media\x"));
        assert_eq!(shares.volume("//UNAS-PRO/Media/y").as_deref(), Some(r"\\unas-pro\media"));
    }

    #[test]
    fn two_local_roots_on_one_letter_are_one_volume() {
        let shares = connector(&api(&[Ok(())], Ok(())), &counted(&[]));
        assert_eq!(shares.volume(r"C:\lazymkv\dev\staging").as_deref(), Some("C:"));
        assert_eq!(shares.volume("c:/lazymkv/dev/library/Movies").as_deref(), Some("C:"));
        assert_eq!(shares.volume(r"D:\x").as_deref(), Some("D:"));
    }

    #[test]
    fn a_question_mark_path_is_not_a_share() {
        for root in [r"\\?\C:\x", r"\\?\UNC\nas\media\x", r"\\.\pipe\lazymkv", r"\\nas", r"\\nas\", "Movies", "/srv/media", "C:relative"] {
            assert_eq!((share_of(root), volume_key(root)), (None, None), "{root}");
            let (api, secrets) = (api(&[Ok(())], Ok(())), counted(&[]));
            let shares = connector(&api, &secrets);
            let text = format!("{root} is neither a drive folder nor a share");
            assert_eq!(shares.reach(root), Reach::Unreachable(text.clone()));
            assert_eq!(shares.free_bytes(root).unwrap_err().to_string(), text);
            assert!(probes(&api).is_empty());
        }
    }

    #[test]
    fn a_local_root_never_connects_and_reports_its_drive_free_space() {
        let (api, secrets) = (api(&[Ok(())], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.reach(r"C:\lazymkv\dev\staging"), Reach::Reachable);
        assert_eq!(shares.free_bytes("c:/lazymkv/dev/staging").unwrap(), 123_456);
        assert_eq!(probes(&api), [r"C:\"]);
        assert_eq!(*api.frees.lock().unwrap(), [r"C:\"]);
        let denied = self::api(&[Err(5)], Ok(()));
        let reach = connector(&denied, &secrets).reach(r"Q:\x");
        assert_eq!(reach, Reach::Unreachable(format!(r"Q:\ does not answer: {}", e(5))));
        assert!(connects(&api).is_empty() && connects(&denied).is_empty());
        assert_eq!(secrets.gets.load(SeqCst), 0);
    }

    #[test]
    fn access_reports_how_the_last_reach_succeeded() {
        let (api, secrets) = (api(&[Ok(()), Err(5), Ok(()), Err(53)], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.access(ROOT), None);
        shares.reach(ROOT);
        assert_eq!(shares.access(ROOT), Some("the session's access"));
        shares.reach(ROOT);
        assert_eq!(shares.access(r"\\NAS\Media\other"), Some("lazymkv's stored credential"));
        shares.reach(ROOT);
        assert_eq!(shares.access(ROOT), None);
        let local = connector(&self::api(&[Ok(())], Ok(())), &secrets);
        local.reach(r"C:\x");
        assert_eq!(local.access(r"c:\y"), Some("local folder"));
    }
```

`self::api` names the helper where a local `api` binding shadows it.

Create `crates/platform/src/win/shares.rs` with these test modules, and `mod shares;` in `win/mod.rs`:

```rust
/// LAZYMKV_TEST_SHARE_ROOT, or None after printing the skip (design part 12.1's convention).
#[cfg(test)]
fn test_share_root() -> Option<String> {
    let root = std::env::var("LAZYMKV_TEST_SHARE_ROOT").ok().filter(|root| !root.is_empty());
    if root.is_none() {
        crate::testutil::skip("LAZYMKV_TEST_SHARE_ROOT is not set (design part 12.4)");
    }
    root
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;

    use super::share_connector;
    use crate::testutil::{skip, temp_dir, Secrets};
    use crate::{Reach, ShareConnector};

    #[test]
    fn a_local_temp_folder_is_reachable_with_free_space() {
        let dir = temp_dir("share-local");
        let root = dir.to_str().unwrap();
        let shares = share_connector(Arc::new(Secrets::new()));
        assert_eq!(shares.reach(root), Reach::Reachable);
        assert!(shares.free_bytes(root).unwrap() > 0);
        assert_eq!(shares.access(root), Some("local folder"));
        assert_eq!(shares.volume(root), Some(root[..2].to_ascii_uppercase()));
    }

    #[test]
    fn a_missing_drive_letter_is_unreachable_with_its_os_error() {
        // SAFETY: GetLogicalDrives takes no argument and returns a bit mask.
        let mask = unsafe { GetLogicalDrives() };
        let Some(letter) = ('D'..='Z').rev().find(|letter| mask & (1 << (*letter as u32 - 'A' as u32)) == 0) else {
            return skip("every drive letter from D to Z is in use");
        };
        let root = format!(r"{letter}:\lazymkv");
        let shares = share_connector(Arc::new(Secrets::new()));
        let Reach::Unreachable(text) = shares.reach(&root) else { panic!("{root} reads as reachable") };
        assert!(text.starts_with(&format!(r"{letter}:\ does not answer: ")), "{text}");
        assert!(text.contains("(os error "), "{text}");
        assert!(shares.free_bytes(&root).unwrap_err().raw_os_error().is_some());
        assert_eq!(shares.access(&root), None);
    }
}

#[cfg(test)]
mod desktop {
    use std::sync::Arc;
    use std::{fs, io};

    use super::{share_connector, test_share_root};
    use crate::security::test_name;
    use crate::testutil::Secrets;
    use crate::{rename_no_replace, Reach, ShareConnector};

    #[test]
    #[ignore = "desktop session, LAZYMKV_TEST_SHARE_ROOT under lazymkv-staging (design part 12.4)"]
    fn a_real_share_root_is_reachable_with_the_session_access() {
        let Some(root) = test_share_root() else { return };
        let shares = share_connector(Arc::new(Secrets::new()));
        assert_eq!(shares.reach(&root), Reach::Reachable);
        assert_eq!(shares.access(&root), Some("the session's access"));
    }

    #[test]
    #[ignore = "desktop session, LAZYMKV_TEST_SHARE_ROOT under lazymkv-staging (design part 12.4)"]
    fn a_real_share_reports_free_space_and_refuses_a_move_over_a_file() {
        let Some(root) = test_share_root() else { return };
        assert!(root.to_ascii_lowercase().contains(r"\lazymkv-staging\"), "{root} is not under lazymkv-staging (Owner decision 9)");
        let shares = share_connector(Arc::new(Secrets::new()));
        assert!(shares.free_bytes(&root).unwrap() > 0);
        let dir = std::path::Path::new(&root).join(test_name("share-move"));
        fs::create_dir_all(&dir).unwrap();
        // Removes the folder however the test ends, so a failure leaves nothing on the NAS (Owner decision 9).
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let (from, taken, free) = (dir.join("a.mkv.partial"), dir.join("b.mkv"), dir.join("c.mkv"));
        fs::write(&from, "new").unwrap();
        fs::write(&taken, "old").unwrap();
        assert_eq!(rename_no_replace(&from, &taken).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!((fs::read_to_string(&from).unwrap(), fs::read_to_string(&taken).unwrap()), ("new".to_string(), "old".to_string()));
        rename_no_replace(&from, &free).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read_to_string(&free).unwrap(), "new");
    }
}

#[cfg(test)]
mod over_ssh {
    use std::sync::Arc;

    use super::{share_connector, test_share_root};
    use crate::security::test_name;
    use crate::shares::share_of;
    use crate::win::CredentialStore;
    use crate::{Reach, ShareConnector};

    #[test]
    #[ignore = "over SSH, LAZYMKV_TEST_SHARE_ROOT"]
    fn over_ssh_a_denied_share_without_a_reachable_credential_says_so() {
        let Some(root) = test_share_root() else { return };
        let share = share_of(&root).expect("LAZYMKV_TEST_SHARE_ROOT names a share");
        let secrets = CredentialStore::with_prefix(&format!("{}:", test_name("ssh")));
        let Reach::Unreachable(text) = share_connector(Arc::new(secrets)).reach(&root) else {
            panic!("{root} answered over SSH, where design part 6.1 expects access denied");
        };
        assert!(text.starts_with(&format!("{share} refused this session's access (")), "{text}");
        assert!(text.ends_with("), and this session can read no lazymkv credential for it"), "{text}");
    }
}

#[cfg(test)]
mod arranged {
    use std::sync::Arc;

    use super::{share_connector, test_share_root};
    use crate::win::CredentialStore;
    use crate::{Reach, ShareConnector};

    /// The owner stored this share's credential with `lazymkvctl secret set`, so it sits under the engine's own prefix;
    /// the test reads it and writes nothing (Owner decision 7).
    #[test]
    #[ignore = "desktop session, only when the owner arranges a share the session cannot reach and stores its credential (Owner decision 7)"]
    fn the_stored_credential_reaches_a_share_the_session_cannot() {
        let Some(root) = test_share_root() else { return };
        let shares = share_connector(Arc::new(CredentialStore::new()));
        assert_eq!(shares.reach(&root), Reach::Reachable);
        assert_eq!(shares.access(&root), Some("lazymkv's stored credential"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked shares`
Expected: a compile error: `ShareApi`, `Shares`, `share_of`, `volume_key` and `share_connector` are not found, and `access` is no method of `ShareConnector`.

- [ ] **Step 3: Implement**

`ShareConnector::access` is the default method of the Produces block. `share_of`, `volume_key` and `share_secret_name` follow their docs. `Shares::new` starts both maps empty.

`Shares::reach(root)` (Ruling W14). `<e c>` is `io::Error::from_raw_os_error(c as i32)`, the key is `volume_key(root)`, and no lock is held across an `api` call:
1. The target: `<share>\` when `share_of(root)` is Some; else `X:\`, upper case, for a drive-letter root; else `Unreachable("<root> is neither a drive folder nor a share")`.
2. `api.probe(target)` Ok: the access becomes `local folder` (a drive root) or `the session's access` (a share); Reachable.
3. A drive root's Err(c): `<X:\> does not answer: <e c>`, so a local root never connects. A share's Err(c) other than 5 (`ERROR_ACCESS_DENIED`) and 1326 (`ERROR_LOGON_FAILURE`): `<share> does not answer: <e c>`.
4. A share's 5 or 1326, called `first`: `secrets.get(share_secret_name(share))`. None: `<share> refused this session's access (<e first>), and this session can read no lazymkv credential for it`. A value `share_credential` refuses: `lazymkv's stored credential for <share> is not a user name and a password on two lines`.
5. `refused[key]` holds this value's hash (`std::hash::DefaultHasher` over the value, kept in memory only): `<share> refused this session's access (<e first>), and Windows refused lazymkv's stored credential: <e remembered code>; it is not tried again until it changes`, with no `connect`.
6. `api.connect(share, user, password)`:
   - Err(1219): `<share>: ERROR_SESSION_CREDENTIAL_CONFLICT: this session already reaches the server as another user, so lazymkv's stored credential was not used`, built from `SESSION_CONFLICT`, with no second probe.
   - Err(c) with c in the private `CREDENTIAL_REFUSED` = [5 `ERROR_ACCESS_DENIED`, 86 `ERROR_INVALID_PASSWORD`, 1326 `ERROR_LOGON_FAILURE`, 1327 `ERROR_ACCOUNT_RESTRICTION`, 1330 `ERROR_PASSWORD_EXPIRED`, 1331 `ERROR_ACCOUNT_DISABLED`, 1909 `ERROR_ACCOUNT_LOCKED_OUT`]: `refused[key] = (hash, c)`, then the text of rule 5 with `c`.
   - Any other Err(c): `<share> does not answer: <e c>`, nothing remembered, so a server that did not answer is tried again at the next reach.
   - Ok: `api.probe(target)` once more. Ok: `refused[key]` removed, the access becomes `lazymkv's stored credential`, Reachable. Err(c): `<share> refused access again after connecting with lazymkv's stored credential: <e c>`.
7. Every Unreachable removes `access[key]`. Two threads that meet one value at the same moment may each try it once; the memory lasts for the engine's run.

`free_bytes(root)`: rule 1's target, then `api.free(target)`, its Err(c) as `io::Error::from_raw_os_error(c as i32)`; with no target, `io::Error::new(ErrorKind::InvalidInput, "<root> is neither a drive folder nor a share")`. `volume(root)` is `volume_key(root)`. `access(root)` reads `access[volume_key(root)]`.

`win::shares` (every `unsafe` block with a `// SAFETY:` line, every wide string ending in one 0):
- `probe`: `GetFileAttributesW(path)`; `INVALID_FILE_ATTRIBUTES` gives Err(`GetLastError()`), and attributes without `FILE_ATTRIBUTE_DIRECTORY` give Err(267, `ERROR_DIRECTORY`).
- `connect`: `WNetAddConnection2W(&NETRESOURCEW { dwType: RESOURCETYPE_DISK, lpLocalName: null, lpRemoteName: <wide share>, lpProvider: null, every other field 0 or null }, <wide password>, <wide user>, 0)`: no local name, flags 0, so no `CONNECT_UPDATE_PROFILE` and no `CONNECT_INTERACTIVE` prompt (design part 6.1). A return of 0 is Ok, any other is Err(code). The wide copy of the password is filled with zeros after the call.
- `free`: `GetDiskFreeSpaceExW(path, &mut available, null, null)`, the bytes available to the caller (W14); a zero return gives Err(`GetLastError()`).
- `share_connector(secrets)` is `Arc::new(Shares::new(Win32Shares, secrets))`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked shares`
Expected: PASS: the 17 `shares::tests` tests (Task 13's among them) and the 2 `win::shares::tests` tests; the 4 tests of `win::shares::{desktop, over_ssh, arranged}` are listed as ignored.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/shares.rs crates/platform/src/lib.rs crates/platform/src/win/shares.rs crates/platform/src/win/mod.rs
git commit -m "platform: the share connector, session access first and lazymkv's credential second (P7, design part 6.1)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

The ignored share tests run on the mini PC in Task 25 steps 3, 4 and 8, after Owner decisions 6, 7 and 9; their evidence is those steps'.

---

### Task 15: The `Ejector` (design parts 4.2, 5.6; PIPE-16)

**Files:**
- Create: `crates/platform/src/eject.rs` (all systems: `drive_letter`, `eject_report`, two tests).
- Create: `crates/platform/src/win/eject.rs` (`WinEjector`, the step seam `run_steps`, three tests).
- Modify: `crates/platform/src/lib.rs:12-16` (`pub mod eject;` in the module list, after Task 13's `pub mod shares;`), `crates/platform/src/win/mod.rs` (`mod eject; pub use eject::WinEjector;`).

`crates/capture/src/eject.rs` is not touched (design part 4.2); its values are copied from :34-43.

**Interfaces:**
- Consumes: `Ejector` (`crates/platform/src/lib.rs:47-51`); `testutil::{ci_factor, skip}` (Task 2).
- Produces:

```rust
// crates/platform/src/eject.rs (all systems)
/// The letter of a drive id in MakeMKV's `D:` form, the form the `DriveWatcher` reports and the job record keeps: an
/// ASCII upper-case letter and a colon, nothing else (design part 5.4; Ruling W8). Err `not a drive id: <drive>` for
/// any other text. Def: T3-15. Use: T3-15.
pub fn drive_letter(drive: &str) -> Result<char, String>;
/// Phase 0's eject report (`crates/capture/src/eject.rs`, experiment 7): each step's name and `ok` or `error <code>`,
/// joined by ", ", in the order given (design part 5.6 step 8). Def: T3-15. Use: T3-15.
pub fn eject_report(steps: &[(&str, Result<(), u32>)]) -> String;

// crates/platform/src/win/eject.rs (cfg(windows))
/// The `Ejector` on Windows (spec 4.3; design parts 4.2 and 5.6 step 8; PIPE-16): phase 0's four steps, lock, dismount,
/// allow removal and eject, rewritten on `windows-sys` from `capture`'s eject, which stays as it is. Every step runs
/// even when an earlier one failed; Ok when the eject step succeeded, else Err with phase 0's report. An id that is
/// not of the `D:` form and a drive that is not optical are refused before anything is opened, and nothing is written
/// to standard output. Def: T3-15. Use: T3-23.
pub struct WinEjector;
impl Ejector for WinEjector { fn eject(&self, drive: &str) -> Result<(), String>; }

// private, with the values of crates/capture/src/eject.rs:39-43 (experiment 7 ran them on both drives)
const DRIVE_CDROM: u32 = 5;
const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
const FSCTL_DISMOUNT_VOLUME: u32 = 0x0009_0020;
const IOCTL_STORAGE_MEDIA_REMOVAL: u32 = 0x002D_4804;
const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D_4808;
/// The four steps in order: the report's name, the control code and its input (a one-byte 0 for allow removal).
const STEPS: [(&str, u32, &[u8]); 4] = [
    ("lock", FSCTL_LOCK_VOLUME, &[]),
    ("dismount", FSCTL_DISMOUNT_VOLUME, &[]),
    ("allow-removal", IOCTL_STORAGE_MEDIA_REMOVAL, &[0]),
    ("eject", IOCTL_STORAGE_EJECT_MEDIA, &[]),
];
/// Runs every step of STEPS through `control`, each even when an earlier one failed; Ok when the eject step
/// succeeded, else Err(eject_report(..)) (design part 5.6 step 8).
fn run_steps(control: impl FnMut(u32, &[u8]) -> Result<(), u32>) -> Result<(), String>;
```

Texts `WinEjector` gives (the last three are `capture`'s, so the engine's notes read as phase 0's did):
- `not a drive id: <drive>` (from `drive_letter`).
- `<drive> is not an optical drive`, when `GetDriveTypeW("<letter>:\")` is not `DRIVE_CDROM`.
- `cannot open drive <drive> (error <code>)`, when `CreateFileW(r"\\.\<letter>:")` fails.
- The report, such as `lock=ok, dismount=ok, allow-removal=ok, eject=error 1`, when the eject step fails.

- [ ] **Step 1: Write the failing tests**

Create `crates/platform/src/eject.rs` and `crates/platform/src/win/eject.rs` holding only their test modules, and add `pub mod eject;` to `lib.rs` and `mod eject;` to `win/mod.rs`.

In `crates/platform/src/eject.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_upper_case_letter_and_a_colon_is_a_drive_id() {
        assert_eq!(drive_letter("D:"), Ok('D'));
        assert_eq!(drive_letter("Z:"), Ok('Z'));
        for drive in ["d:", r"D:\", "DD:", "D", "", "1:", ":D", "tok0", "É:"] {
            assert_eq!(drive_letter(drive), Err(format!("not a drive id: {drive}")), "{drive}");
        }
    }

    #[test]
    fn the_report_names_each_step_in_order_with_its_error_code() {
        let steps = [("lock", Ok(())), ("dismount", Err(5)), ("allow-removal", Ok(())), ("eject", Ok(()))];
        assert_eq!(eject_report(&steps), "lock=ok, dismount=error 5, allow-removal=ok, eject=ok");
        assert_eq!(eject_report(&[("lock", Err(21)), ("eject", Err(1))]), "lock=error 21, eject=error 1");
    }
}
```

In `crates/platform/src/win/eject.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::testutil::{ci_factor, skip};
    use crate::Ejector;

    #[test]
    fn a_fixed_drive_is_refused_as_not_optical() {
        let system = std::env::var("SystemDrive").expect("SystemDrive is set on Windows").to_ascii_uppercase();
        assert_eq!(WinEjector.eject(&system), Err(format!("{system} is not an optical drive")));
    }

    #[test]
    fn every_step_runs_after_an_earlier_one_fails() {
        let mut seen = Vec::new();
        let result = run_steps(|code, input| {
            seen.push((code, input.to_vec()));
            match code {
                0x0009_0018 => Err(5),
                0x0009_0020 => Err(6),
                _ => Ok(()),
            }
        });
        assert_eq!(result, Ok(()));
        assert_eq!(seen, [(0x0009_0018, vec![]), (0x0009_0020, vec![]), (0x002D_4804, vec![0]), (0x002D_4808, vec![])]);
        let failed = run_steps(|code, _| if code == 0x002D_4808 { Err(1) } else { Ok(()) });
        assert_eq!(failed, Err("lock=ok, dismount=ok, allow-removal=ok, eject=error 1".to_string()));
    }

    #[test]
    #[ignore = "needs a disc in LAZYMKV_TEST_DRIVE; over SSH, as experiment 7 (design part 12.4)"]
    fn the_tray_opens_on_eject() {
        let Ok(drive) = std::env::var("LAZYMKV_TEST_DRIVE") else {
            return skip("LAZYMKV_TEST_DRIVE is not set (design part 12.4)");
        };
        let root = format!(r"{drive}\");
        std::fs::read_dir(&root)
            .map(drop)
            .unwrap_or_else(|error| panic!("{root} must hold a readable disc before the eject: {error}"));
        assert_eq!(WinEjector.eject(&drive), Ok(()));
        let (start, wait) = (Instant::now(), Duration::from_secs(10) * ci_factor());
        while std::fs::read_dir(&root).is_ok() {
            assert!(start.elapsed() < wait, "{root} still reads {wait:?} after the eject");
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked eject`
Expected: a compile error: `drive_letter`, `eject_report`, `WinEjector` and `run_steps` are not found.

- [ ] **Step 3: Implement**

- `eject.rs`'s module doc names design parts 4.2 and 5.6 step 8 and says that `win::eject` makes the Windows calls. `drive_letter`, `eject_report` and `run_steps` as their docs and tests fix them.
- `WinEjector::eject(drive)`, every `unsafe` block with a `// SAFETY:` line:
  1. `drive_letter(drive)?`.
  2. `GetDriveTypeW(<wide "<letter>:\">)` other than `DRIVE_CDROM`: `<drive> is not an optical drive`.
  3. `CreateFileW(<wide r"\\.\<letter>:">, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, null, OPEN_EXISTING, 0, null)`; `INVALID_HANDLE_VALUE` gives `cannot open drive <drive> (error <GetLastError()>)`. The handle is closed by a guard's `Drop`, so every path below closes it once.
  4. `run_steps(|code, input| ...)` where each step is `DeviceIoControl(handle, code, <input pointer, or null when the input is empty>, input.len(), null, 0, &mut returned, null)`, and a zero return gives `Err(GetLastError())`.

  No handle notification needs closing first: the `DriveWatcher` registers none (Ruling W8; design part 5.6 step 8).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked eject`
Expected: PASS: `eject::tests`' 2 tests and `win::eject::tests`' 2 tests; `win::eject::tests::the_tray_opens_on_eject` is listed as ignored.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/eject.rs crates/platform/src/win/eject.rs crates/platform/src/lib.rs crates/platform/src/win/mod.rs
git commit -m "platform: the Ejector, phase 0's four steps on windows-sys (design part 4.2)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

The ignored eject test runs on the mini PC in Task 25 step 2, after Owner decision 17; its evidence is that step's.

---

### Task 16: The `DriveWatcher` (design parts 4.2, 5.4; L1a; engine.md:575, :582; W8)

**Files:**
- Create: `crates/platform/src/drives.rs` (all systems: `drive_id`, `ready_drives`, `volume_answers`, `DriveQueue`, `DRIVE_POLL_EVERY`, and 9 tests in its `mod tests`).
- Create: `crates/platform/src/win/drives.rs` (`#[cfg(windows)]` through `win`: `WinDrives`, its window thread, 2 tests and 1 ignored test).
- Modify: `crates/platform/src/lib.rs:12-16` (`pub mod drives;` in the module list, after Task 15's `pub mod eject;`) and `:37-45` (the `DriveWatcher` doc, given whole in Step 3).
- Modify: `crates/platform/src/win/mod.rs` (`mod drives; pub use drives::WinDrives;`).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes: `DriveEvent` and `DriveWatcher` (`crates/platform/src/lib.rs:18-45`, pull request 2); `win::wide` (Task 2); `testutil::{ci_factor, skip, cue}` (Task 2); Task 1's features `Win32_Foundation`, `Win32_Storage_FileSystem`, `Win32_System_IO`, `Win32_System_Ioctl`, `Win32_System_LibraryLoader`, `Win32_UI_WindowsAndMessaging` and `Win32_System_Diagnostics_Debug` (`SetThreadErrorMode`).
- Produces:

```rust
// crates/platform/src/drives.rs (all systems)
/// The drive id of `letter` in MakeMKV's `D:` form: the letter upper-cased, then a colon (design part 5.4 step 1;
/// engine.md:575). The same id keys `drives.json` and is what `Ejector::eject` takes. Def: T3-16. Use: T3-16.
pub fn drive_id(letter: char) -> String;

/// How often the watcher scans the drives with no notification (Ruling W8). Def: T3-16. Use: T3-16.
pub const DRIVE_POLL_EVERY: Duration = Duration::from_secs(2);

/// The scan's filter (Ruling W8): each letter of `mask` (`GetLogicalDrives`' bits, bit 0 `A` to bit 25 `Z`, higher bits
/// ignored) whose `kind` is `DRIVE_CDROM` (5) and whose volume `answers`, as `drive_id` -> `X:\`. `answers` is asked
/// only for an optical drive, so the volume of a fixed or network drive is never touched. Def: T3-16. Use: T3-16.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn ready_drives(mask: u32, kind: impl Fn(char) -> u32, answers: impl Fn(char) -> bool)
    -> BTreeMap<String, PathBuf>;

/// Whether a volume answers after `GetVolumeInformationW` gave `read` (Ruling W8): Ok answers; ERROR_ACCESS_DENIED (5)
/// and ERROR_SHARING_VIOLATION (32), which a volume lock held by the `Ejector` or another program gives, keep the last
/// scan's answer `before`, so a lock is never reported as a removal; every other code, ERROR_NOT_READY (21) for an
/// empty tray and ERROR_UNRECOGNIZED_VOLUME (1005) among them, does not answer. Def: T3-16. Use: T3-16.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn volume_answers(read: Result<(), u32>, before: bool) -> bool;

/// The watcher's state (design part 5.4, L1a, Ruling W8): the discs of the last scan and the events not yet handed
/// out. Def: T3-16. Use: T3-16.
pub struct DriveQueue { /* now: BTreeMap<String, PathBuf>, events: VecDeque<DriveEvent> */ }
impl DriveQueue {
    /// A queue whose current discs are `now`, with no event: the discs at start are `present`'s, never `next`'s (L1a).
    /// Def: T3-16. Use: T3-16.
    pub fn new(now: BTreeMap<String, PathBuf>) -> DriveQueue;
    /// Takes one scan: queues `Removed` for each id of the last scan missing from `now`, in id order, then `Arrived`
    /// with its root for each id of `now` missing from the last scan, in id order; an id in both queues nothing, which
    /// is the debounce (W8). Def: T3-16. Use: T3-16.
    pub fn observe(&mut self, now: BTreeMap<String, PathBuf>);
    /// The current discs as (id, root) in id order, and every queued event dropped, since the list holds their outcome
    /// (L1a). Def: T3-16. Use: T3-16.
    pub fn present(&mut self) -> Vec<(String, PathBuf)>;
    /// The oldest queued event. Def: T3-16. Use: T3-16.
    pub fn pop(&mut self) -> Option<DriveEvent>;
}

// crates/platform/src/win/drives.rs (cfg(windows))
/// The Windows `DriveWatcher` (spec 4.3; design parts 4.2, 5.4; L1a; Ruling W8): a thread of its own owns a hidden
/// top-level window, which receives `WM_DEVICECHANGE` broadcasts and CD-ROM device interface notifications (message-only
/// windows receive no broadcasts), and scans the drives on each of them and every `DRIVE_POLL_EVERY`, feeding one
/// `DriveQueue` that `next` and `present` read. The message loop runs on this thread, not on the engine's drive thread
/// (W8, Owner decision 11). It holds no handle on any volume, so the `Ejector`'s lock is never refused because of it
/// (design part 5.6 step 8). Def: T3-16. Use: T3-23.
pub struct WinDrives { /* queue: Arc<(Mutex<DriveQueue>, Condvar)>, window: usize (the HWND), thread: Option<JoinHandle<()>> */ }
impl WinDrives {
    /// Starts the window thread and returns once the window is registered for notifications and its first scan is the
    /// queue's state, so `present` is the state at start and no change after that scan is lost (L1a). Err is Windows'
    /// error for the window class, the window or the timer. Def: T3-16. Use: T3-23.
    pub fn new() -> io::Result<WinDrives>;
}
impl DriveWatcher for WinDrives { /* next: waits on the Condvar up to `timeout` while the queue is empty, then pops;
                                     present: DriveQueue::present under the lock */ }
impl Drop for WinDrives { /* PostMessageW(window, WM_CLOSE, 0, 0), then joins the thread */ }
```

Windows values (Ruling W8; numbers Windows' own): `WM_DEVICECHANGE` 0x0219, `WM_TIMER` 0x0113, `WM_CLOSE` 0x0010, `WM_DESTROY` 0x0002; `DBT_DEVTYP_DEVICEINTERFACE` 5; `DEVICE_NOTIFY_WINDOW_HANDLE` 0; `GUID_DEVINTERFACE_CDROM`, the globally unique identifier (GUID) `{53F56308-B6BF-11D0-94F2-00A0C91EFB8B}`; `DRIVE_CDROM` 5 (a private `#[cfg_attr(not(windows), allow(dead_code))] const DRIVE_CDROM: u32 = 5;` in `drives.rs`, which compiles on every system; as with Task 14's items, outside Windows only the tests use these three); `ERROR_CLASS_ALREADY_EXISTS` 1410; `SEM_FAILCRITICALERRORS` 0x0001; timer id 1; window class `lazymkv-drive-watcher`; in the ignored test, `FSCTL_LOCK_VOLUME` 0x0009_0018 and `FSCTL_UNLOCK_VOLUME` 0x0009_001C.

- [ ] **Step 1: Write the failing tests**

`crates/platform/src/drives.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::DriveEvent;

    fn discs(ids: &[&str]) -> BTreeMap<String, PathBuf> {
        ids.iter().map(|id| (id.to_string(), PathBuf::from(format!(r"{id}\")))).collect()
    }

    fn arrived(id: &str) -> Option<DriveEvent> {
        Some(DriveEvent::Arrived { drive: id.to_string(), root: PathBuf::from(format!(r"{id}\")) })
    }

    fn removed(id: &str) -> Option<DriveEvent> {
        Some(DriveEvent::Removed { drive: id.to_string() })
    }

    #[test]
    fn a_lower_case_letter_is_reported_upper_case_with_a_colon() {
        assert_eq!(drive_id('d'), "D:");
        assert_eq!(drive_id('D'), "D:");
    }

    #[test]
    fn an_arrival_and_a_removal_are_queued_once_each() {
        let mut queue = DriveQueue::new(discs(&[]));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.pop(), arrived("D:"));
        assert_eq!(queue.pop(), None);
        queue.observe(discs(&[]));
        assert_eq!(queue.pop(), removed("D:"));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn a_disc_seen_twice_is_reported_once() {
        let mut queue = DriveQueue::new(discs(&[]));
        queue.observe(discs(&["D:"]));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.pop(), arrived("D:"));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn removals_come_before_arrivals_in_letter_order() {
        let mut queue = DriveQueue::new(discs(&["D:", "F:"]));
        queue.observe(discs(&["E:", "G:"]));
        let order: Vec<Option<DriveEvent>> = (0..5).map(|_| queue.pop()).collect();
        assert_eq!(order, vec![removed("D:"), removed("F:"), arrived("E:"), arrived("G:"), None]);
    }

    /// L1a: what `present` lists, `next` never reports again.
    #[test]
    fn present_drops_events_queued_before_it() {
        let mut queue = DriveQueue::new(discs(&["E:"]));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.present(), vec![("D:".to_string(), PathBuf::from(r"D:\"))]);
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn an_event_after_present_is_reported() {
        let mut queue = DriveQueue::new(discs(&["D:"]));
        assert_eq!(queue.present().len(), 1);
        queue.observe(discs(&[]));
        assert_eq!(queue.pop(), removed("D:"));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.pop(), arrived("D:"));
    }

    /// engine.md:582, Ruling W8: a disc whose volume does not answer is not listed, so it arrives through `next` once
    /// it answers and gets the readiness wait of any arrival.
    #[test]
    fn a_cd_drive_whose_volume_does_not_answer_is_not_listed() {
        // C: is fixed (3); D: and E: are optical (5); only D:'s volume answers.
        let mask = (1 << 2) | (1 << 3) | (1 << 4);
        let kind = |letter: char| if letter == 'C' { 3 } else { 5 };
        assert_eq!(ready_drives(mask, kind, |letter| letter == 'D'), discs(&["D:"]));
    }

    #[test]
    fn only_cd_drives_are_asked_whether_their_volume_answers() {
        let asked = RefCell::new(Vec::new());
        // A:, C:, D: and a bit past Z:.
        let mask = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 30);
        let kind = |letter: char| if letter == 'D' { 5 } else { 3 };
        let found = ready_drives(mask, kind, |letter| {
            asked.borrow_mut().push(letter);
            true
        });
        assert_eq!(found, discs(&["D:"]));
        assert_eq!(*asked.borrow(), vec!['D']);
    }

    #[test]
    fn a_locked_volume_keeps_its_last_answer() {
        assert!(volume_answers(Ok(()), false));
        assert!(!volume_answers(Err(21), true));
        assert!(!volume_answers(Err(1005), true));
        assert!(volume_answers(Err(5), true));
        assert!(!volume_answers(Err(5), false));
        assert!(volume_answers(Err(32), true));
        assert!(!volume_answers(Err(32), false));
    }
}
```

`crates/platform/src/win/drives.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::drives::DRIVE_POLL_EVERY;
    use crate::testutil::{ci_factor, cue, skip};
    use crate::{DriveEvent, DriveWatcher};

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

    #[test]
    fn present_lists_only_ids_of_the_d_colon_form_with_their_roots() {
        let watcher = WinDrives::new().unwrap();
        for (id, root) in watcher.present() {
            let bytes = id.as_bytes();
            assert!(bytes.len() == 2 && bytes[0].is_ascii_uppercase() && bytes[1] == b':', "{id}");
            assert_eq!(root, PathBuf::from(format!(r"{id}\")));
            println!("present: {id}");
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
    struct VolumeLock(HANDLE); // windows_sys::Win32::Foundation::HANDLE
    fn lock_volume(drive: &str) -> Result<VolumeLock, u32>;

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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked drives`
Expected: a compile error: `drive_id`, `ready_drives`, `volume_answers` and `DriveQueue` are not found in `drives`, and `WinDrives` and `volume_information` are not found in `win::drives`.

- [ ] **Step 3: Implement**

`crates/platform/src/lib.rs:37-45`, the trait's doc and its two method docs, whole (the signatures are unchanged):

```rust
/// Delivers disc arrivals and removals to the engine's drive thread (spec 4.3, design parts 4.2, 5.4). Drive ids are
/// MakeMKV's `D:` form, the letter in upper case and a colon (engine.md:575). A disc counts as present once its volume
/// answers: a disc whose volume does not answer is not listed and arrives through `next` once it answers, so it gets
/// `drive_settle_s` and the readiness wait like any arrival, while a disc `present` lists is read at once (Ruling W8,
/// engine.md:582). `ScriptedDrives` replays what a test scripts (Rulings T8-b, T8-f): an event a test queued before
/// the engine started is reported although `present` lists its outcome. Def: T8, its promises T3-16. Use: T22, T25,
/// T3-23.
pub trait DriveWatcher: Send {
    /// Waits up to `timeout` for the next event and returns it, or None when none came in that time. It never reports
    /// as `Arrived` a disc the last `present` listed until that drive's `Removed` (L1a).
    fn next(&mut self, timeout: Duration) -> Option<DriveEvent>;
    /// The drives holding a disc at the moment of the call, as (drive, root). Every event queued before the call is
    /// dropped, since the list holds its outcome (L1a). The engine reads this at start (design part 4.2, L1), before it
    /// drains any event, and does not rely on the order.
    fn present(&self) -> Vec<(String, PathBuf)>;
}
```

`drives.rs` (module doc: the pure half of design part 4.2's watcher, Ruling W8, compiled and tested on every system): its items as their docs and the nine tests fix them.

`win/drives.rs`, every `unsafe` block with a `// SAFETY:` line, every wide string from `win::wide`:
- `fn volume_information(letter: char) -> Result<(), u32>`: `GetVolumeInformationW(wide("<X>:\"), null, 0, null, null, null, null, 0)`, Err of `GetLastError()` on a zero return.
- `fn scan(last: &BTreeMap<String, PathBuf>) -> BTreeMap<String, PathBuf>`: `ready_drives(GetLogicalDrives(), |l| GetDriveTypeW(wide("<l>:\")), |l| volume_answers(volume_information(l), last.contains_key(&drive_id(l))))`.
- The window thread (named `lazymkv-drive-watcher`) runs, in this order, so that nothing between the first scan and the first notification is lost:
  1. `SetThreadErrorMode(SEM_FAILCRITICALERRORS, null_mut())`, so a scan of an empty drive never raises Windows' "no disk" dialog on the owner's desktop.
  2. `RegisterClassW` of `lazymkv-drive-watcher` with the module's `GetModuleHandleW(null)` and the window procedure below (`WNDCLASSW` written field by field: its `Default` needs `Win32_Graphics_Gdi`, which Task 1 does not enable); a zero return with `ERROR_CLASS_ALREADY_EXISTS` counts as registered, since the class is per process and parallel tests make several watchers.
  3. `CreateWindowExW(0, class, "lazymkv drive watcher", 0, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, null parent, null menu, module, null)`: a top-level window that is never shown.
  4. `RegisterDeviceNotificationW(window, &DEV_BROADCAST_DEVICEINTERFACE_W { dbcc_size, dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE, dbcc_classguid: GUID_DEVINTERFACE_CDROM, .. }, DEVICE_NOTIFY_WINDOW_HANDLE)`; a refusal is not an error, since the poll covers every change.
  5. `SetTimer(window, 1, 2000, None)`.
  6. The first scan, against an empty map, becomes both the queue (`DriveQueue::new`) and the thread's last scan, then `new` gets `Ok(window)` over a channel. Any Err of steps 2, 3 or 5 is sent instead (`io::Error::last_os_error()`), and the thread ends.
  7. `GetMessageW` and `DispatchMessageW` until `GetMessageW` returns 0 or -1.
- The window procedure reaches the queue through a `thread_local!` the thread sets before step 2, since each window lives on its own thread. `WM_DEVICECHANGE` (any `wParam`) and `WM_TIMER` scan against the thread's copy of the last scan (kept in the same `thread_local!`), `observe` a clone of the result under the lock, keep the result as the last scan and `notify_all` the `Condvar`; `WM_DEVICECHANGE` returns 1 (TRUE), so a query to remove a device is never denied. `WM_CLOSE` runs `DestroyWindow`; `WM_DESTROY` runs `UnregisterDeviceNotification` (when step 4 succeeded), `KillTimer(window, 1)` and `PostQuitMessage(0)`. Every other message goes to `DefWindowProcW`.
- `win/mod.rs`: `mod drives; pub use drives::WinDrives;`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked drives`
Expected: PASS: the 9 tests of `drives::tests`, and on Windows the 2 of `win::drives::tests` with 1 ignored (`an_inserted_and_removed_disc_is_reported_once_each`). On `windows-latest` the workspace run covers the same tests.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/drives.rs crates/platform/src/win/drives.rs crates/platform/src/win/mod.rs crates/platform/src/lib.rs
git commit -m "platform: the DriveWatcher, a hidden window and a polling fallback over one drive queue (design part 4.2, L1a)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

The ignored drive test runs on the mini PC in Task 25 step 4, after Owner decision 6; its evidence is that step's.

---

### Task 17: Share work off the owner thread and the share checks (engine.md:584, :585, :589, :590; W15, W21)

**Files:**
- Modify: `crates/platform/src/lib.rs:121-124` (the `ShareConnector::volume` doc).
- Modify: `crates/platform/src/fakes.rs:193-262` (`SharesState`, `FakeShares::{stall, set_access, set_free_error}`, `StallGate`, `access`, `set_free` clearing a free-space error) and its `mod tests` after `fake_shares_report_free_space_and_switchable_reach` (:560); 3 tests.
- Modify: `crates/engine/src/space.rs:84-117` (`rip_check`), `:155-180` (`staged_remux_check`), `:182-235` (`rebuild_promises`, `rebuild_promise`), `:255-283` (`remux_check`); new `make_root`, `root_ready`, `released_steps`, `refine_remux_promise`; `mod tests` (:285-497): every `rip_check`, `remux_check` and `staged_remux_check` call takes `&Mutex<Promises>`, `an_unreadable_staging_folder_rebuilds_the_remux_promise_from_the_plan` (:473) is replaced, and 5 tests are added.
- Modify: `crates/engine/src/selftest.rs:225-249` (`root_check`), `:289-313` (`CheckEnv.shares`), `:443-452` (`selftest`), `:514-530` (`share_back`), `:532-560` (`space_freed`), and `mod tests` (:563-): the `root_check` calls gain `&FakeShares::new()`, the `waiting` closure of `a_share_recheck_reaches_the_roots_its_return_uses` (:635) becomes a module helper, 2 tests.
- Modify: `crates/engine/src/owner.rs:226-238` (the `CheckEnv` literal), `:892` and `:962-971` (the restart's doc and promise block: doc only), `:1731-1758` (`release_promises`, `promise_on_return`).
- Modify: `crates/engine/src/steps.rs:63` (imports), `:868-893` (`check_space`), `:1431-1453` (`remux_space`), `:1461-1497` (`remux`), `:1698-1712` (`delete_partial_file`).
- Modify: `crates/fake-tools/tests/engine_ipc.rs` (2 helpers and 7 tests), `crates/fake-tools/tests/engine_disc_phase.rs` (1 helper and 3 tests), `crates/fake-tools/tests/engine_restart.rs` (1 test; the doc of `a_rebuilt_remux_promise_counts_against_another_jobs_rip_check` at :188).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes: `ShareConnector::access` (Task 14); `Reach`, `Promises`, `PromisedStep`, `staged_bytes`, `volume_of` (pull request 2); `CancelToken` (`crates/tools/src/record.rs:121-135`, re-exported as `lazymkv_tools::CancelToken`); `Transition` (`lazymkv_core::record`).
- Produces:

```rust
// crates/platform/src/lib.rs, ShareConnector::volume's doc, whole:
/// An identifier of the volume holding `root`, equal for two roots on one volume (S11), or None when unknown. It answers
/// from the text of `root` alone, with no input or output, so the owner thread may call it (design part 6.1's rule,
/// engine.md:585).

// crates/platform/src/fakes.rs (feature "fakes")
impl FakeShares {
    /// Makes `reach` and `free_bytes` of `root` block until the returned gate is released or dropped (engine.md:584).
    /// No lock of the fake is held while a call waits, so other roots answer. A later `stall` of the same root puts a
    /// new gate in place. Def: T3-17. Use: T3-17.
    pub fn stall(&self, root: &str) -> StallGate;
    /// Sets what `access(root)` reports (design part 7.1's job.log line). Def: T3-17. Use: T3-17.
    pub fn set_access(&self, root: &str, access: &'static str);
    /// Makes `free_bytes(root)` Err(io::Error::other(message)) while `reach` still answers (Ruling W21); a later
    /// `set_free(root, ..)` clears it. Def: T3-17. Use: T3-17.
    pub fn set_free_error(&self, root: &str, message: &str);
}
/// A stall `FakeShares::stall` made (engine.md:584). Dropping it releases it. Def: T3-17. Use: T3-17.
pub struct StallGate { /* gate: Arc<(Mutex<GateState { open: bool, waiting: usize }>, Condvar)> */ }
impl StallGate {
    /// Lets every waiting and later call through. Def: T3-17. Use: T3-17.
    pub fn release(&self);
    /// Whether a call waits on the gate within `timeout`, so a test knows the step has reached the share.
    /// Def: T3-17. Use: T3-17.
    pub fn wait_for_caller(&self, timeout: Duration) -> bool;
}
impl ShareConnector for FakeShares { fn access(&self, root: &str) -> Option<&'static str>; } // set_access's value, else None

// crates/engine/src/space.rs
/// Design part 6.2, Ruling W15: a missing `staging_root` or `tuning_root` is created with `fs::create_dir` when its
/// parent is a folder, never deeper; a library root is never passed. Ok when `root` is a folder or was made
/// (`AlreadyExists` counts as made). Err: `<root> does not exist and could not be created: <parent> is not a folder`;
/// `<root> does not exist and could not be created: <error>`; `<root> is not a folder` for a root with no parent.
/// It calls `root_ready` before `fs::create_dir`. Def: T3-17. Use: T3-17.
pub(crate) fn make_root(root: &str) -> Result<(), String>;
/// Whether `make_root` could succeed, creating nothing: Ok when `root` is a folder or its parent is; Err `make_root`'s
/// text otherwise (engine.md:589, Ruling W21: AwaitingShare's re-check asks it of `staging_root`, so a missing parent
/// keeps the job waiting instead of cycling). Def: T3-17. Use: T3-17.
pub(crate) fn root_ready(root: &str) -> Result<(), String>;
/// Design part 5.8's releases of `transition`, from it alone, so the owner locks the promises only when there is
/// something to release (engine.md:584): Rip when it leaves Ripping, Remux when it leaves Remuxing, both when it enters
/// a waiting state or Failed; each at most once, Rip first. Def: T3-17. Use: T3-17.
pub(crate) fn released_steps(transition: &Transition) -> Vec<PromisedStep>;
/// Changed: the three checks take the promises' Mutex and read `reach`, `volume` and `free_bytes` before they lock it
/// (engine.md:584); the lock is held only to add the promised amounts, compare and promise.
pub(crate) fn rip_check(rip_bytes: u64, remux_bytes: u64, staging: &str, library: Option<&str>,
    shares: &dyn ShareConnector, promises: &Mutex<Promises>, job: &str) -> (Event, Option<String>);
pub(crate) fn remux_check(remux_bytes: u64, library: &str, shares: &dyn ShareConnector, promises: &Mutex<Promises>,
    job: &str) -> (Event, Option<String>);
pub(crate) fn staged_remux_check(staged: Result<u64, String>, plan: &Plan, staging: &str, library: &str,
    shares: &dyn ShareConnector, promises: &Mutex<Promises>, job: &str) -> (Event, Option<String>);
/// engine.md:585 (Ruling S75): the Remux step's own promise once its staged sizes are read with no lock held:
/// `remux_promise(bytes)` on `volume_of(library)`, written under the lock unless `cancel` is raised (the owner cancels a
/// step before it releases that job's promises, so a release is never undone). Err leaves the estimate in place.
/// Def: T3-17. Use: T3-17.
pub(crate) fn refine_remux_promise(staged: Result<u64, String>, library: &str, shares: &dyn ShareConnector,
    promises: &Mutex<Promises>, job: &str, cancel: &CancelToken);
// rebuild_promises keeps its signature; a Remuxing job's Remux promise is now planned_remux_promise(plan), so it reads no
// staged file (engine.md:585), and its doc says the Remux step replaces it.

// crates/engine/src/selftest.rs
pub(crate) struct CheckEnv { /* PR 2 fields */ pub(crate) shares: Arc<dyn ShareConnector> } // the roots' reach (W15)
/// Changed: `reach` first (spec 13, "each root reachable"; Ruling W15), then `make_root` for staging and tuning, then
/// the probe.
pub(crate) fn root_check(name: &str, root: Option<&str>, library: bool, shares: &dyn ShareConnector) -> SelfTestCheck;
```

Texts (each a `FileFailed`, `RemuxFailed` or check sentence as named; `<why>` is `Reach::Unreachable`'s text):
- job.log, `check_space`, for staging and the plan kind's library root when `access` reports one: `share access for <root>: <access>`.
- `RemuxFailed { Unknown }` when the title's library root does not answer: `<key>: the library share holding <root> does not answer (<why>); the remux was not started, and a retry once the share answers runs it`.
- job.log, `delete_partial_file`, when the title's library root does not answer: `<key>: the library share holding <root> does not answer (<why>); the partial file <partial> was not deleted`.
- The self-test, a root that does not answer: `<root> is unreachable: <why>` (`space.rs`' wording).

- [ ] **Step 1: Write the failing tests**

`crates/platform/src/fakes.rs`, `mod tests` (`ci_factor` from `crate::testutil`):

```rust
    #[test]
    fn a_stalled_root_blocks_reach_and_free_bytes_until_released() {
        let shares = Arc::new(FakeShares::new());
        let gate = shares.stall("/s");
        let (sent, got) = std::sync::mpsc::channel();
        let caller = Arc::clone(&shares);
        std::thread::spawn(move || sent.send((caller.reach("/s"), caller.free_bytes("/s").ok())).unwrap());
        assert!(gate.wait_for_caller(Duration::from_secs(5) * crate::testutil::ci_factor()));
        assert!(got.recv_timeout(Duration::from_millis(100)).is_err(), "reach went through a closed gate");
        assert_eq!(shares.reach("/m"), Reach::Reachable);
        gate.release();
        let answered = got.recv_timeout(Duration::from_secs(5) * crate::testutil::ci_factor()).unwrap();
        assert_eq!(answered, (Reach::Reachable, Some(u64::MAX)));
        drop(shares.stall("/s"));
        assert_eq!(shares.reach("/s"), Reach::Reachable);
    }

    #[test]
    fn a_free_space_error_leaves_reach_and_set_free_clears_it() {
        let shares = FakeShares::new();
        shares.set_free_error("/s", "access is denied");
        assert_eq!(shares.reach("/s"), Reach::Reachable);
        assert_eq!(shares.free_bytes("/s").unwrap_err().to_string(), "access is denied");
        shares.set_free("/s", 7);
        assert_eq!(shares.free_bytes("/s").unwrap(), 7);
    }

    #[test]
    fn the_fake_reports_the_access_it_was_given() {
        let shares = FakeShares::new();
        assert_eq!(shares.access("/s"), None);
        shares.set_access("/s", "the session's access");
        assert_eq!(shares.access("/s"), Some("the session's access"));
    }
```

`crates/engine/src/space.rs`, `mod tests`. The seven existing tests that call `rip_check`, `remux_check` or `staged_remux_check` change only in their promises: `let promises = Mutex::new(Promises::default());`, the checks get `&promises`, and every `promises.promised(..)` or `promises.promise(..)` goes through `promises.lock().unwrap()`. Added (with `use std::sync::{Arc, Mutex};`, `use lazymkv_core::job::FailureClass;`, `use lazymkv_core::plan::Mode;`, `use lazymkv_core::record::Transition;`, `use lazymkv_tools::CancelToken;`):

```rust
    /// Shares that fail the test when a read runs while the promises are locked (engine.md:584).
    struct LockProbe(Arc<Mutex<Promises>>);

    impl ShareConnector for LockProbe {
        fn reach(&self, root: &str) -> Reach {
            assert!(self.0.try_lock().is_ok(), "reach({root}) ran under the promises lock");
            Reach::Reachable
        }
        fn free_bytes(&self, root: &str) -> io::Result<u64> {
            assert!(self.0.try_lock().is_ok(), "free_bytes({root}) ran under the promises lock");
            Ok(u64::MAX)
        }
        fn volume(&self, root: &str) -> Option<String> {
            Some(root.to_string())
        }
    }

    #[test]
    fn the_checks_read_the_shares_before_they_take_the_promises_lock() {
        let promises = Arc::new(Mutex::new(Promises::default()));
        let probe = LockProbe(Arc::clone(&promises));
        assert_eq!(rip_check(1, 1, "/s", Some("/m"), &probe, &promises, "a-1"), (Event::SpaceOk, None));
        assert_eq!(remux_check(1, "/m", &probe, &promises, "a-1"), (Event::SpaceOk, None));
        let unread = Err("/s/a-1/rip/t00-00800: not found".to_string());
        let (found, _) = staged_remux_check(unread, &movie_plan(), "/s", "/m", &probe, &promises, "a-1");
        assert_eq!(found, Event::SpaceOk);
    }

    #[test]
    fn the_steps_a_transition_releases_are_computed_from_it_alone() {
        let released =
            |old: Option<State>, new: State| released_steps(&Transition { time_unix: 1, event: None, old, new, actions: vec![] });
        let planned = || State::Planned { mode: Mode::Direct };
        let failed = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Ripping) };
        let both = vec![PromisedStep::Rip, PromisedStep::Remux];
        assert_eq!(released(Some(State::Ripping), State::RipVerified { after_copy: false }), vec![PromisedStep::Rip]);
        assert_eq!(released(Some(State::Remuxing), State::Verified), vec![PromisedStep::Remux]);
        assert_eq!(released(Some(planned()), State::AwaitingSpace { return_to: Box::new(planned()) }), both);
        assert_eq!(released(Some(State::Ripping), failed), both);
        assert_eq!(released(None, State::Fingerprinted), Vec::<PromisedStep>::new());
        assert_eq!(released(Some(planned()), planned()), Vec::<PromisedStep>::new());
        assert_eq!(released(Some(State::Decided { approved: None }), State::Remuxing), Vec::<PromisedStep>::new());
    }

    #[test]
    fn a_missing_root_under_a_folder_is_made_and_nothing_deeper() {
        let s = Scratch::new("make-root");
        let made = s.0.join("engine");
        assert_eq!(make_root(made.to_str().unwrap()), Ok(()));
        assert!(made.is_dir());
        assert_eq!(make_root(made.to_str().unwrap()), Ok(()));
        let parent = s.0.join("absent");
        let deeper = parent.join("engine");
        let refused = make_root(deeper.to_str().unwrap()).unwrap_err();
        let expected =
            format!("{} does not exist and could not be created: {} is not a folder", deeper.display(), parent.display());
        assert_eq!(refused, expected);
        assert!(!parent.exists());
    }

    /// engine.md:585: a Remuxing job's rebuilt Remux promise is the plan's estimate on the library's volume whatever
    /// staging holds, so the owner thread reads no staged file; its Remux step replaces it (`refine_remux_promise`).
    /// Replaces `an_unreadable_staging_folder_rebuilds_the_remux_promise_from_the_plan`, with its record and settings.
    #[test]
    fn a_rebuilt_remux_promise_is_the_plans_estimate_and_reads_no_staged_file() {
        let plan = movie_plan();
        let s = Scratch::new("rebuild-estimate");
        let staging = s.0.join("staging");
        let settings = Settings {
            staging_root: Some(staging.to_str().unwrap().to_string()),
            movies_root: Some("/m".to_string()),
            ..Settings::default()
        };
        let record = JobRecord {
            format: JOB_FORMAT.to_string(), id: "a-1".to_string(), fingerprint: "AAA".to_string(), disc_id: None,
            drive: "tok0".to_string(), created_unix: 1, updated_unix: 1, state: State::Remuxing, message: None,
            rip_again: false, profile: "job.mmcp.xml".to_string(), plan: Some(plan.clone()),
            titles: Vec::<TitleRecord>::new(), filed_unix: None, collisions: vec![], notes: vec![],
        };
        let shares = FakeShares::new();
        for staged in [false, true] {
            if staged {
                s.write("staging/a-1/rip/t00-00800/title_t00.mkv", [0u8; 16]);
            }
            let mut promises = Promises::default();
            rebuild_promises(&mut promises, &shares, &record, &settings, &s.0);
            assert_eq!(promises.promised(&volume_of(&shares, "/m"), "b-1"), planned_remux_promise(&plan), "{staged}");
        }
    }

    #[test]
    fn the_remux_step_replaces_the_estimate_with_the_staged_sizes() {
        let shares = FakeShares::new();
        let library = volume_of(&shares, "/m");
        let promises = Mutex::new(Promises::default());
        promises.lock().unwrap().promise("a-1", PromisedStep::Remux, &library, 4301);
        let live = CancelToken::default();
        refine_remux_promise(Err("/s/a-1/rip/t00-00800: not found".to_string()), "/m", &shares, &promises, "a-1", &live);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 4301);
        refine_remux_promise(Ok(16), "/m", &shares, &promises, "a-1", &live);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), remux_promise(16));
        promises.lock().unwrap().release("a-1", PromisedStep::Remux);
        let cancelled = CancelToken::default();
        cancelled.cancel();
        refine_remux_promise(Ok(16), "/m", &shares, &promises, "a-1", &cancelled);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 0);
    }
```

`crates/engine/src/selftest.rs`, `mod tests`: `a_missing_library_root_fails_its_check_and_is_not_created` and `an_unset_root_fails_its_check` pass `&FakeShares::new()` as `root_check`'s fourth argument (as does the probe test above them); the `waiting` closure of `a_share_recheck_reaches_the_roots_its_return_uses` becomes `fn waiting_for_share(return_to: State) -> JobRecord`, unchanged in body; added:

```rust
    /// Ruling W21 (S81 amended): AwaitingShare's re-check also reads free space, and an error there keeps the job
    /// waiting.
    #[test]
    fn a_share_recheck_waits_while_the_free_space_cannot_be_read() {
        use lazymkv_core::job::State;
        use lazymkv_platform::fakes::FakeShares;
        let settings = Settings { staging_root: Some("/staging".to_string()), ..Settings::default() };
        let shares = FakeShares::new();
        shares.set_free_error("/staging", "access is denied");
        assert_eq!(share_back(&waiting_for_share(State::Ripping), &settings, &shares), None);
        shares.set_free("/staging", 1);
        assert_eq!(share_back(&waiting_for_share(State::Ripping), &settings, &shares), Some(Event::ShareBack));
    }

    /// engine.md:589, Ruling W21: a staging root `make_root` could not make keeps the job waiting, and the re-check
    /// creates nothing.
    #[test]
    fn a_share_recheck_waits_while_the_staging_root_cannot_be_made() {
        use lazymkv_core::job::State;
        use lazymkv_platform::fakes::FakeShares;
        let s = Scratch::new("recheck-root");
        let engine = s.0.join("absent").join("engine");
        let settings = Settings { staging_root: Some(engine.to_str().unwrap().to_string()), ..Settings::default() };
        let shares = FakeShares::new();
        assert_eq!(share_back(&waiting_for_share(State::Ripping), &settings, &shares), None);
        std::fs::create_dir(s.0.join("absent")).unwrap();
        assert_eq!(share_back(&waiting_for_share(State::Ripping), &settings, &shares), Some(Event::ShareBack));
        assert!(!engine.exists(), "the re-check created the root");
    }
```

`crates/fake-tools/tests/engine_ipc.rs` (imports gain `lazymkv_platform::IpcConnector` where not already there):

```rust
/// A new client's snapshot, or None when none came within `limit`. The limit proves "does not block" and is not
/// scaled (Global Constraints).
fn snapshot_within(h: &Harness, limit: Duration) -> Option<Value> {
    let stream = h.client().connect().unwrap_or_else(|_| panic!("the engine accepts the client"));
    let (send, lines) = channel();
    let mut reader = stream.reader;
    std::thread::spawn(move || {
        let mut line = String::new();
        if reader.read_line(&mut line).is_ok() {
            let _ = send.send(line);
        }
    });
    let line = lines.recv_timeout(limit).ok();
    drop(stream.writer);
    match read_reply(line?.trim_end()).unwrap() {
        Reply::Push(envelope) if envelope.kind == "snapshot" => Some(envelope.payload),
        other => panic!("expected the snapshot, got {other:?}"),
    }
}

/// Whether a `job_update` of `job` reaches `c` within `limit` (unscaled, as `snapshot_within`); other pushes are kept.
fn job_update_within(c: &mut Client, job: &str, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(line) = c.lines.recv_timeout(left) else {
            return false;
        };
        if let Reply::Push(envelope) = read_reply(line.trim_end()).unwrap() {
            if envelope.kind == "job_update" && envelope.payload["id"] == json!(job) {
                return true;
            }
            c.pushes.push_back(envelope);
        }
    }
    false
}

/// engine.md:584 (design part 4.4): CheckSpace reads the shares before it locks the promises, so while the staging
/// share stalls inside CheckSpace the owner still persists the removal's MediaRemoved and answers a new client.
#[test]
fn a_stalled_staging_share_does_not_stop_the_engine_answering_status() {
    let h = stand_in(&[DISC_A], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    let gate = h.shares().stall(&staging);
    h.insert("tok0", DISC_A);
    h.deliver();
    assert!(gate.wait_for_caller(WAIT * ci_factor()), "CheckSpace never reached the staging share");
    h.remove("tok0");
    h.deliver();
    h.wait("tok0 emptied", |h| h.drives_json()["drives"]["tok0"]["fingerprint"].is_null());
    let snapshot = snapshot_within(&h, Duration::from_secs(5)).expect("a snapshot within 5 s while the share stalls");
    assert_eq!(snapshot["jobs"].as_array().unwrap().len(), 1);
    gate.release();
}

/// engine.md:584, :585: a `retry` of an AwaitingSpace job re-checks space on its own thread and reads the shares
/// before it locks the promises, so while that read stalls another disc's arrival is fed and its job_update arrives.
#[test]
fn a_return_from_a_wait_with_a_stalled_share_leaves_drive_events_flowing() {
    let h = stand_in(&[DISC_A], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_free(&staging, 1);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let a = c.until_state(None, "AwaitingSpace")["id"].as_str().unwrap().to_string();
    let gate = h.shares().stall(&staging);
    c.raw(&json!({"v": 1, "type": "retry", "id": 99, "payload": {"job": a}}).to_string());
    assert!(gate.wait_for_caller(WAIT * ci_factor()), "the re-check never reached the staging share");
    h.insert("tok1", DISC_B);
    h.deliver();
    let b = format!("{}-1", h.fingerprint(DISC_B).to_ascii_lowercase());
    assert!(job_update_within(&mut c, &b, Duration::from_secs(5)), "no job_update of {b} while the share stalls");
    gate.release();
}

/// engine.md:585: the restart rebuilds the promises with no share read, and the resumed CheckSpace reads the shares
/// before it locks them, so a client is answered while the staging share stalls.
#[test]
fn a_restart_with_a_stalled_share_answers_a_client_before_the_share_does() {
    let h = stand_in(&[DISC_A], settings());
    h.insert("tok0", DISC_A);
    h.run_until("Planned");
    let staging = h.staging().to_str().unwrap().to_string();
    let gate = h.shares().stall(&staging);
    h.restart();
    assert!(gate.wait_for_caller(WAIT * ci_factor()), "the resumed CheckSpace never reached the staging share");
    h.remove("tok0");
    h.deliver();
    h.wait("tok0 emptied", |h| h.drives_json()["drives"]["tok0"]["fingerprint"].is_null());
    assert!(snapshot_within(&h, Duration::from_secs(5)).is_some(), "no snapshot within 5 s while the share stalls");
    gate.release();
}

/// Ruling W21 (S81 amended): AwaitingShare's re-check asks `reach` and then `free_bytes`, so a share that answers but
/// whose free space cannot be read keeps the job waiting, with no ShareBack to cycle on.
#[test]
fn awaiting_share_stays_while_the_free_space_cannot_be_read() {
    let h = stand_in(&[DISC_A], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Unreachable("gone".to_string()));
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingShare")["id"].as_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Reachable);
    h.shares().set_free_error(&staging, "access is denied");
    c.ok("retry", json!({"job": id}));
    h.settle();
    assert_eq!(h.state_of(&id), "AwaitingShare");
    assert!(!h.transitions(&id).iter().any(|t| t.event == Some(Event::ShareBack)));
    h.shares().set_free(&staging, u64::MAX);
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Filed");
}

/// engine.md:589 with Ruling W15: a staging root whose parent is missing sends the job to AwaitingShare, and a retry
/// feeds nothing until the parent exists, so it never cycles through ShareBack.
#[test]
fn a_retry_while_the_staging_parent_is_missing_feeds_nothing() {
    let h = stand_in(&[DISC_A], settings());
    let parent = h.staging().join("absent");
    let engine = parent.join("engine");
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("settings_set", json!({"staging_root": engine.to_str().unwrap()}));
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingShare")["id"].as_str().unwrap().to_string();
    c.ok("retry", json!({"job": id}));
    h.settle();
    assert_eq!(h.state_of(&id), "AwaitingShare");
    assert!(!h.transitions(&id).iter().any(|t| t.event == Some(Event::ShareBack)));
    std::fs::create_dir(&parent).unwrap();
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Filed");
}

/// Spec 13 ("each root reachable"), Ruling W15: the self-test asks `reach` of each root before its probe.
#[test]
fn the_self_test_reports_an_unreachable_root_from_reach_before_its_probe() {
    let h = stand_in(&[DISC_A], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Unreachable("cable out".to_string()));
    let mut c = Client::open(&h);
    c.snapshot();
    let result = c.ok("run_selftest", Value::Null);
    let found = check(&result, "staging_root");
    assert_eq!(found["ok"], json!(false));
    assert_eq!(found["detail"], json!(format!("{staging} is unreachable: cable out")));
    assert_eq!(check(&result, "movies_root")["ok"], json!(true));
}

/// Design part 6.2, Ruling W15: the self-test creates a missing tuning_root whose parent is a folder.
#[test]
fn the_self_test_creates_a_missing_tuning_root_under_an_existing_parent() {
    let h = stand_in(&[DISC_A], settings());
    let tuning = h.data().parent().unwrap().join("tuning").join("engine");
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("settings_set", json!({"tuning_root": tuning.to_str().unwrap()}));
    let result = c.ok("run_selftest", Value::Null);
    assert_eq!(check(&result, "tuning_root")["ok"], json!(true), "{result}");
    assert!(tuning.is_dir());
}
```

`crates/fake-tools/tests/engine_disc_phase.rs`:

```rust
/// Writes `root` as `key` into data/settings.json and restarts, so the next job starts with it (design part 6.2).
fn with_root(h: &Harness, key: &str, root: &Path) {
    let path = h.data().join("settings.json");
    let mut stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    stored[key] = json!(root.to_str().unwrap());
    fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    h.restart();
}

/// Design part 6.2, Ruling W15: CheckSpace creates a missing staging_root whose parent is a folder before the rip
/// writes into it.
#[test]
fn a_missing_staging_root_under_an_existing_parent_is_created_at_check_space() {
    let h = harness(&[], json!({}));
    let engine = h.staging().join("engine");
    with_root(&h, "staging_root", &engine);
    let id = run_to(&h, "Ejected");
    assert!(engine.join(&id).join("rip").join(KEY).join("title_t00.mkv").is_file());
}

/// Ruling W15: only the last folder is ever created; a staging root whose parent is missing is ShareLost naming both.
#[test]
fn a_missing_parent_is_not_created() {
    let h = harness(&[], json!({}));
    let parent = h.staging().join("absent");
    let engine = parent.join("engine");
    with_root(&h, "staging_root", &engine);
    let id = run_to(&h, "AwaitingShare");
    h.settle();
    let why = format!("{} does not exist and could not be created: {} is not a folder", engine.display(), parent.display());
    assert_eq!(h.record(&id).message.as_deref(), Some(why.as_str()));
    assert!(!parent.exists());
}

/// Design part 7.1: job.log names the share access CheckSpace used, for each root the connector reports one for, and
/// nothing with the default fake, so `whole_job`'s job.log is unchanged.
#[test]
fn the_share_access_line_is_written_when_the_connector_reports_one() {
    let h = harness(&[], json!({}));
    let (staging, library) = (root(h.staging()), root(h.library()));
    h.shares().set_access(&staging, "the session's access");
    h.shares().set_access(&library, "lazymkv's stored credential");
    let id = run_to(&h, "Ripping");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains(&format!("share access for {staging}: the session's access")), "{log}");
    assert!(log.contains(&format!("share access for {library}: lazymkv's stored credential")), "{log}");
    let plain = harness(&[], json!({}));
    let id = run_to(&plain, "Ripping");
    let log = fs::read_to_string(plain.job_dir(&id).join("job.log")).unwrap();
    assert!(!log.contains("share access for"), "{log}");
}
```

`crates/fake-tools/tests/engine_restart.rs` (the doc of `a_rebuilt_remux_promise_counts_against_another_jobs_rip_check` now reads: "A's Remux step, resumed at the restart, replaces the rebuilt estimate with write_bytes_needed(16) = 17 (`refine_remux_promise`, engine.md:585); B's identical check needs more than 8610 with either figure, 8619 with 17"):

```rust
/// engine.md:590 (V7): a restart into Remuxing runs DeletePartialFile and Remux, and each asks `reach` of the title's
/// library root before it touches the share, so with the share not answering nothing there is deleted or written and
/// Remux fails naming the share and the way out.
#[test]
fn a_restart_into_remuxing_with_the_library_share_unreachable_fails_remux_without_writing() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Remuxing");
    let id = only_job(&h);
    let library = root(h.library());
    let partial = h.library().join(".lazymkv-partial").join(format!("{id}-t00-00800.mkv.partial"));
    fs::create_dir_all(partial.parent().unwrap()).unwrap();
    fs::write(&partial, b"sentinel").unwrap();
    h.shares().set_reach(&library, Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    let record = state_json(&h.job_dir(&id));
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    let why = format!(
        "t00-00800: the library share holding {library} does not answer (cable out); the remux was not started, and a \
         retry once the share answers runs it"
    );
    assert_eq!(record.message.as_deref(), Some(why.as_str()));
    assert_eq!(fs::read(&partial).unwrap(), b"sentinel");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    let kept = format!(
        "t00-00800: the library share holding {library} does not answer (cable out); the partial file {} was not deleted",
        partial.display()
    );
    assert!(log.contains(&kept), "{log}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-platform --features lazymkv-platform/fakes --locked fakes::`
Expected: a compile error: no methods `stall`, `set_free_error`, `set_access` or `access` on `FakeShares`.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: a compile error: `make_root`, `root_ready`, `released_steps` and `refine_remux_promise` are not found, and `rip_check` and `root_check` are called with the wrong arguments.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase --test engine_file_phase --test engine_restart --test engine_ipc`
Expected: a compile error: no methods `stall`, `set_access` or `set_free_error` on `FakeShares`. Once the fakes are in, the new tests fail on the old engine: the two stall tests on the owner thread with no snapshot within 5 s, `a_return_from_a_wait_with_a_stalled_share_leaves_drive_events_flowing` with no job_update, and the rest on their texts or states.

- [ ] **Step 3: Implement**

- `FakeShares`: `SharesState` gains `stalls: BTreeMap<String, Arc<(Mutex<GateState>, Condvar)>>` (private `struct GateState { open: bool, waiting: usize }`, the same `Arc` the root's `StallGate` holds), `access: BTreeMap<String, &'static str>` and `free_errors: BTreeMap<String, String>`. `reach` and `free_bytes` clone the root's gate under the state lock, drop the lock, then wait on the gate (`waiting` counted up while they wait) before they answer as before; `free_bytes` gives the free error before the free figure; `set_free` removes the root's free error. `StallGate::release` and its drop set `open` and `notify_all`.
- `space.rs`, each function as its doc says:
  - `rip_check`: 1. `reach` of staging, then of the library, an unreachable one ShareLost as in pull request 2; 2. the volumes and whether they are shared; 3. `free_bytes` of staging, and of the library when not shared, all before the lock; 4. under the lock, in pull request 2's order: a staging free-space error is ShareLost, staging short is SpaceShort, a library free-space error is ShareLost, the library short is SpaceShort, else both promises and SpaceOk.
  - `remux_check`: `reach`, `volume`, `free_bytes`, then the lock for the comparison and the promise.
  - `staged_remux_check` passes the Mutex on; `rebuild_promise`'s Remux arm uses `planned_remux_promise(plan)` and calls no `staged_bytes`; the module doc and `rebuild_promises`' doc say so.
  - `make_root`, `root_ready`, `released_steps` and `refine_remux_promise` as their docs say; the lock is `promises.lock().unwrap_or_else(PoisonError::into_inner)`.
- `selftest.rs`: `CheckEnv.shares`; `root_check`'s order is unset, then `reach` (Unreachable gives `ok: false` with `<root> is unreachable: <why>` and `path: Some(root)`), then the library folder rule as before or `make_root` for staging and tuning, then the probe; `selftest` passes `self.shares.as_ref()`; `share_back` asks each root `reach` and then `free_bytes`, and `staging_root` `root_ready`, and gives ShareBack only when every check passes; `space_freed` takes no lock of its own.
- `owner.rs`: the `CheckEnv` literal gains `shares: Arc::clone(&platform.shares)`; `release_promises` computes `released_steps(transition)` first and locks only when it is not empty; `promise_on_return`, the restart's promise block and step 4 of the restart's doc say that `rebuild_promises` reads no share.
- `steps.rs`:
  - `check_space`: `reach(staging)` and, when it answers, `make_root(staging)`, whose Err goes through `space_result` as `(Event::ShareLost, Some(sentence))`; then `rip_check` with `&self.steps.promises` (its own `reach` names an unreachable root); then, for staging and the library root, the access line when `access` reports one.
  - `remux_space`: `staged_remux_check` with `&self.steps.promises`.
  - `remux`: after the tool check, `refine_remux_promise(staged_bytes(staging, id, plan), library, ..., &self.cancel)` when both roots are set; then for each title, right after `decided(key)` and before its partial folder, `reach(&path.root)`, a refusal its `RemuxFailed { Unknown }` text above; pull request 2's `create_dir_all` of `.lazymkv-partial` stays.
  - `delete_partial_file`: each title asks `reach(&path.root)` first; when it does not answer, the job.log line above and that title is skipped.
  - The doc of each changed step names engine.md:584, :585 or :590 and Ruling W15.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-platform --features lazymkv-platform/fakes --locked fakes::`
Expected: PASS.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: PASS.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase --test engine_file_phase --test engine_restart --test engine_ipc`
Expected: PASS, every pull request 2 test of the four files among them (`the_remux_creates_the_partial_folder_when_the_library_root_has_none` and the unreadable-title-folder tests keep their messages and outcomes).

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS; `whole_job` is unchanged, since the default fake reports no access.

- [ ] **Step 6: Commit**

```
git add crates/platform/src/lib.rs crates/platform/src/fakes.rs crates/engine/src/space.rs crates/engine/src/selftest.rs crates/engine/src/owner.rs crates/engine/src/steps.rs crates/fake-tools/tests/engine_ipc.rs crates/fake-tools/tests/engine_disc_phase.rs crates/fake-tools/tests/engine_restart.rs
git commit -m "engine: share work leaves the owner thread; Remux and the self-test ask reach; staging and tuning roots are created (engine.md:584-590)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 18: Library reads, L7 and the fingerprint's temporary folder (engine.md:576, :587, :591; W22, W28)

**Files:**
- Modify: `crates/engine/src/file_phase.rs:277-310` (`library_facts` returns a `Result`) and its `mod tests` (:395-436): the three `library_facts` tests `.unwrap()` its result, and 1 test is added.
- Modify: `crates/engine/src/steps.rs:620-646` (`fingerprint` through `snapshot_aside`; `FRESH_SNAPSHOT` and `snapshot_aside` after it), `:1316-1350` (`title_doc` passes `library_facts`' error on), `:1362-1376` (`decide` asks `reach` of the library root first), `:1641-1648` (L7 in `file`, between the folder and the rename), and `mod tests` (:1717-): 4 tests.
- Modify: `crates/fake-tools/tests/engine_file_phase.rs` (the helpers `Closed` and `retry`, and 6 tests; imports gain `std::io::{BufRead, Write}`, `std::sync::atomic::{AtomicBool, Ordering}`, `std::sync::mpsc::channel`, `std::sync::Arc`, `std::time::Duration`, `lazymkv_core::ipc::{read_reply, Reply}`, `lazymkv_core::job::FailureClass` where missing, `lazymkv_engine::testkit::ci_factor`, `lazymkv_platform::{IpcConnector, Reach}`).

Line anchors are at `dcd5924`. Task 4's `cfg(not(windows))` hard-link branches in `file` stay as Task 4 left them.

**Interfaces:**
- Consumes: `ShareConnector::reach`; `lazymkv_tools::snapshot` (`crates/tools/src/snapshot.rs:56`, unchanged: it writes `files.json`, `nav/` and `meta.json` under its `out`); `NamedPath`; the `Step` helpers `job_plan`, `log` and `unknown` (pull request 2).
- Produces:

```rust
// crates/engine/src/file_phase.rs
/// Changed (Ruling W22, R-B1; engine.md:591): every read error but NotFound is Err(`the library folder <folder> could not
/// be read: <error>`), `<folder>` being the film's folder `<root>/<components[0]>` (or `path.path` without components),
/// for the final path's metadata, the folder's listing, an entry of it and an entry's metadata. NotFound reads as
/// absent, as before, so inputs that read without error give the same facts and decide's output is unchanged.
/// Def: T24, its errors T3-18. Use: T24.
pub(crate) fn library_facts(path: &NamedPath, style: EditionStyle) -> Result<LibraryFacts, String>;

// crates/engine/src/steps.rs
/// The folder of the job folder the Fingerprint step snapshots into before it compares fingerprints (engine.md:587,
/// review round 1's V10). Def: T3-18. Use: T3-18.
pub(crate) const FRESH_SNAPSHOT: &str = "fingerprint.lazymkv-new";
/// The Fingerprint step's recording (design part 5.4 step 2; engine.md:587): `tools::snapshot` of the disc at `root`
/// into `<job_dir>/FRESH_SNAPSHOT`, which a crash may have left and is removed first; on the job's `fingerprint`,
/// `files.json`, `nav` and `meta.json` replace the job folder's own, and `FRESH_SNAPSHOT` is removed. On any other
/// outcome the job folder's snapshot is untouched and `FRESH_SNAPSHOT` is removed. Err is the step's message:
/// `the disc's recording could not be written: <error>` (the snapshot, or a leftover that cannot be removed),
/// `the disc's fingerprint <found or "none"> differs from the job's <fingerprint>`, or `the disc's recording could not
/// be put in place: <error>` (the swap). Def: T3-18. Use: T3-18.
pub(crate) fn snapshot_aside(root: &Path, label: &str, job_dir: &Path, captured_unix: u64, fingerprint: &str)
    -> Result<(), String>;
```

Texts (Rulings W22, W28; `<why>` is `Reach::Unreachable`'s text):
- `DecisionFailed { Unknown }` when the plan kind's library root (`movies_root` or `tv_root`) does not answer: `the library share holding <root> does not answer (<why>); the library was not read, and a retry once the share answers decides again`. An unset root asks nothing, as the namer then gives no path.
- `DecisionFailed { Unknown }` from `library_facts`: its Err, unchanged.
- `FileFailed { Unknown }`, L7: `<folder> is present but cannot be opened (<error>); the partial file stays at <partial>, and a retry tries again`, `<folder>` the final path's folder and `<error>` `fs::read_dir`'s. No collision is recorded.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/file_phase.rs`'s `mod tests`, the four calls of `library_facts` at :410, :417, :427 and :435 take `.unwrap()`; then add:

```rust
    /// Ruling W22: a film folder that cannot be listed is an error naming it, not a folder holding nothing. A regular
    /// file stands where the folder goes, so the read fails on every system with an error other than NotFound.
    #[test]
    fn a_film_folder_that_cannot_be_listed_is_an_error_naming_it() {
        let s = Scratch::new("unlistable-film");
        let path = film_path(&s.0);
        let folder = s.0.join(&path.components[0]);
        std::fs::write(&folder, b"").unwrap();
        let error = library_facts(&path, EditionStyle::Jellyfin).unwrap_err();
        assert!(error.starts_with(&format!("the library folder {} could not be read: ", folder.display())), "{error}");
    }
```

`crates/engine/src/steps.rs`, `mod tests` (`snapshot` and `FRESH_SNAPSHOT` through `use super::*;`):

```rust
    /// A synthetic disc under `s` as the harness builds one: an index, a playlist and a stream of `stream` zero bytes.
    fn disc_volume(s: &Scratch, name: &str, stream: usize) -> PathBuf {
        s.write(&format!("{name}/BDMV/index.bdmv"), "INDX0200");
        s.write(&format!("{name}/BDMV/PLAYLIST/00800.mpls"), "MPLS0200");
        s.write(&format!("{name}/BDMV/STREAM/00001.m2ts"), vec![0u8; stream]);
        s.0.join(name)
    }

    /// The job folder's snapshot, read whole.
    fn snapshot_files(job: &Path) -> Vec<Vec<u8>> {
        ["files.json", "meta.json", "nav/BDMV/index.bdmv", "nav/BDMV/PLAYLIST/00800.mpls"]
            .iter()
            .map(|file| fs::read(job.join(file)).unwrap())
            .collect()
    }

    /// engine.md:587 (V10): a different disc in the job's drive leaves the job's nav/, files.json and meta.json as they
    /// were, and no fresh snapshot behind.
    #[test]
    fn a_different_disc_leaves_the_jobs_snapshot_unchanged() {
        let s = Scratch::new("aside-differs");
        let job = s.0.join("job");
        let fingerprint = snapshot(&disc_volume(&s, "a", 1000), "A", &job, 1).unwrap().fingerprint.unwrap();
        let before = snapshot_files(&job);
        let error = snapshot_aside(&disc_volume(&s, "b", 2000), "B", &job, 2, &fingerprint).unwrap_err();
        let tail = format!(" differs from the job's {fingerprint}");
        assert!(error.starts_with("the disc's fingerprint ") && error.ends_with(&tail), "{error}");
        assert_eq!(snapshot_files(&job), before);
        assert!(!job.join(FRESH_SNAPSHOT).exists());
    }

    #[test]
    fn a_matching_disc_swaps_in_the_new_snapshot() {
        let s = Scratch::new("aside-matches");
        let job = s.0.join("job");
        let disc = disc_volume(&s, "a", 1000);
        let fingerprint = snapshot(&disc, "A", &job, 1).unwrap().fingerprint.unwrap();
        fs::write(job.join("nav/BDMV/stale.bdmv"), b"an older snapshot's file").unwrap();
        snapshot_aside(&disc, "A", &job, 2, &fingerprint).unwrap();
        let meta: Value = serde_json::from_slice(&fs::read(job.join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["captured_unix"], json!(2));
        assert!(job.join("nav/BDMV/index.bdmv").is_file());
        assert!(!job.join("nav/BDMV/stale.bdmv").exists());
        assert!(!job.join(FRESH_SNAPSHOT).exists());
    }

    #[test]
    fn a_fresh_snapshot_a_crash_left_is_replaced() {
        let s = Scratch::new("aside-leftover");
        let job = s.0.join("job");
        let disc = disc_volume(&s, "a", 1000);
        let fingerprint = snapshot(&disc, "A", &job, 1).unwrap().fingerprint.unwrap();
        s.write(&format!("job/{FRESH_SNAPSHOT}/nav/BDMV/left.bdmv"), "left by a crash");
        snapshot_aside(&disc, "A", &job, 2, &fingerprint).unwrap();
        assert!(!job.join(FRESH_SNAPSHOT).exists());
        assert!(!job.join("nav/BDMV/left.bdmv").exists());
    }

    #[test]
    fn a_disc_that_cannot_be_walked_leaves_the_jobs_snapshot_unchanged() {
        let s = Scratch::new("aside-unwalked");
        let job = s.0.join("job");
        let fingerprint = snapshot(&disc_volume(&s, "a", 1000), "A", &job, 1).unwrap().fingerprint.unwrap();
        let before = snapshot_files(&job);
        let error = snapshot_aside(&s.0.join("absent"), "", &job, 2, &fingerprint).unwrap_err();
        assert!(error.starts_with("the disc's recording could not be written: "), "{error}");
        assert_eq!(snapshot_files(&job), before);
        assert!(!job.join(FRESH_SNAPSHOT).exists());
    }
```

`crates/fake-tools/tests/engine_file_phase.rs`:

```rust
/// A folder this process cannot open (L7), closed once by `close` (which creates it first) and opened again by `open`
/// or a drop: Unix mode 0o000 and back to 0o755; Windows `icacls <folder> /deny *S-1-1-0:(OI)(CI)(RD,WD,AD,X)` (a deny
/// entry for Everyone, security identifier (SID) S-1-1-0: RD stops the listing, WD the rename into it, and X is
/// listed although traverse checking is bypassed by default) and `icacls <folder> /remove:d *S-1-1-0`, each asserting
/// that icacls succeeded.
struct Closed {
    folder: PathBuf,
    once: AtomicBool, // `close` has acted
    shut: AtomicBool, // closed now
}

impl Closed {
    fn new(folder: PathBuf) -> Closed;
    fn close(&self);           // acts on its first call only
    fn holds(&self) -> bool;   // closed, and fs::read_dir(folder) fails (false for root on Unix)
    fn open(&self);            // acts only while closed
}

impl Drop for Closed {
    fn drop(&mut self) {
        self.open();
    }
}

/// Sends `retry` for `id` through a new client and waits for its `ok` (20 s, times `ci_factor`; design part 9.4).
fn retry(h: &Harness, id: &str) {
    let stream = h.client().connect().unwrap_or_else(|_| panic!("the engine accepts the client"));
    let mut writer = stream.writer;
    let mut reader = stream.reader;
    let (send, lines) = channel();
    std::thread::spawn(move || loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => if send.send(line).is_err() { break },
        }
    });
    let request = json!({"v": 1, "type": "retry", "id": 1, "payload": {"job": id}});
    writer.write_all(format!("{request}\n").as_bytes()).unwrap();
    writer.flush().unwrap();
    loop {
        let line = lines.recv_timeout(Duration::from_secs(20) * ci_factor()).expect("the retry's reply");
        match read_reply(line.trim_end()).unwrap() {
            Reply::Ok { id: Some(1), .. } => return,
            Reply::Error { id: Some(1), message, .. } => panic!("retry refused: {message}"),
            _ => {}
        }
    }
}

/// Runs movie-direct to Failed with its film folder closed just before File (the hook fires on VerifyFile's
/// `mkvmerge -J <partial>`, after Decide read the library). Returns the job, the guard and the final path.
fn run_with_the_film_folder_closed(h: &Harness) -> (String, Arc<Closed>, PathBuf) {
    let target = final_path(h);
    let closed = Arc::new(Closed::new(target.parent().unwrap().to_path_buf()));
    let later = Arc::downgrade(&closed);
    h.on_spawn(&["-J", ".partial"], move || {
        if let Some(closed) = later.upgrade() {
            closed.close();
        }
    });
    let id = run_to(h, "Failed");
    (id, closed, target)
}

/// Ruling W22 (R-B1; engine.md:591): a library folder that cannot be read fails Decide as Unknown naming it, instead of
/// reading as a folder holding nothing.
#[test]
fn a_library_folder_that_cannot_be_read_fails_decide_rather_than_reading_as_empty() {
    let h = harness(&[], json!({}));
    let film = final_path(&h).parent().unwrap().to_path_buf();
    fs::create_dir_all(film.parent().unwrap()).unwrap();
    fs::write(&film, b"").unwrap();
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let message = record.message.unwrap_or_default();
    assert!(message.starts_with(&format!("the library folder {} could not be read: ", film.display())), "{message}");
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::DecisionFailed { .. })));
}

/// Ruling W22: a film folder that does not exist still reads as no file, so the job files as before.
#[test]
fn an_absent_library_folder_still_reads_as_no_file() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let decision = h.record(&id).titles[0].decision.clone().unwrap();
    assert!(!decision.reviews.contains(&ReviewReason::PathCollision), "{:?}", decision.reviews);
    assert_eq!(decision.path.map(|path| PathBuf::from(path.path)), Some(final_path(&h)));
}

/// Ruling W22 (V7, design part 6.1): Decide asks `reach` of the library root before it reads the library, so a share
/// that does not answer fails Decide naming it instead of reading as an empty folder.
#[test]
fn decide_asks_the_library_share_before_reading_it() {
    let h = harness(&[], json!({}));
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    let id = h.job_ids().pop().unwrap();
    let library = h.library().to_str().unwrap().to_string();
    h.shares().set_reach(&library, Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let why = format!(
        "the library share holding {library} does not answer (cable out); the library was not read, and a retry once \
         the share answers decides again"
    );
    assert_eq!(record.message.as_deref(), Some(why.as_str()));
}

/// L7 (design parts 2.10, 6.5; Ruling W28): the title's folder is present but cannot be opened, so File fails as
/// Unknown naming that folder (never the library root, so LIB-7 stands), the partial file stays in
/// `.lazymkv-partial`, and nothing is renamed.
#[test]
fn a_present_but_inaccessible_folder_fails_filing_and_keeps_the_partial() {
    let h = harness(&[], json!({}));
    let (id, closed, target) = run_with_the_film_folder_closed(&h);
    if !closed.holds() {
        return; // root on Unix opens a folder of mode 0o000
    }
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Verified) });
    let partial = h.library().join(".lazymkv-partial").join(format!("{id}-{KEY}.mkv.partial"));
    let film = target.parent().unwrap();
    let message = record.message.unwrap_or_default();
    let head = format!("{} is present but cannot be opened (", film.display());
    let tail = format!("); the partial file stays at {}, and a retry tries again", partial.display());
    assert!(message.starts_with(&head) && message.ends_with(&tail), "{message}");
    assert_eq!(fs::read(&partial).unwrap(), b"lazymkv-test-mkv");
    assert!(record.collisions.is_empty(), "{:?}", record.collisions);
    closed.open();
    assert!(!target.exists());
}

/// L7 (design part 6.5): a `retry` once the folder opens files the title.
#[test]
fn a_retry_after_the_folder_opens_files_the_title() {
    let h = harness(&[], json!({}));
    let (id, closed, target) = run_with_the_film_folder_closed(&h);
    if !closed.holds() {
        return; // root on Unix opens a folder of mode 0o000
    }
    closed.open();
    retry(&h, &id);
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    assert_eq!(fs::read(&target).unwrap(), b"lazymkv-test-mkv");
}
```

`crates/fake-tools/tests/engine_restart.rs`' `a_restart_in_detected_after_a_finished_recording_fingerprints_again` is unchanged and now runs the matching disc's swap end to end. A different disc never reaches the Fingerprint step through the harness, since the restart matches fingerprints before any step runs, so the differing case is the unit test above.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: a compile error: `snapshot_aside` and `FRESH_SNAPSHOT` are not found, and `unwrap` and `unwrap_err` are no methods of `LibraryFacts`.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase`
Expected: FAIL in `a_library_folder_that_cannot_be_read_fails_decide_rather_than_reading_as_empty` (the job fails from Verified, at the folder File cannot make), `decide_asks_the_library_share_before_reading_it` (the job waits in AwaitingShare from the remux check) and `a_present_but_inaccessible_folder_fails_filing_and_keeps_the_partial` (the message is the rename's `could not be renamed`). `an_absent_library_folder_still_reads_as_no_file` and `a_retry_after_the_folder_opens_files_the_title` pass already; they pin what must not change.

- [ ] **Step 3: Implement**

- `library_facts`: `fs::metadata(&path.path)` for `path_collision` (Ok true, NotFound false, else Err); `fs::read_dir(<folder>)` (NotFound gives the facts so far, else Err); each entry's error is Err; an entry is a folder when `fs::metadata(entry.path())` says so (NotFound counts as not a folder, as `is_dir` read it; else Err). The rest is pull request 2's.
- `title_doc`: `path.map(|path| library_facts(&path, style)).transpose()?.unwrap_or_default()`.
- `decide`: after `job_plan` and before anything else, `reach` of the plan kind's library root when set, its refusal the `DecisionFailed` text above. The doc names Ruling W22 and V7.
- `snapshot_aside`, in this order (its doc fixes the outcomes):
  1. `fs::remove_dir_all(<job_dir>/FRESH_SNAPSHOT)`, NotFound ignored.
  2. `snapshot(root, label, &fresh, captured_unix)`; an Err, or a fingerprint other than `fingerprint`, removes `fresh` and returns the step's message.
  3. For `files.json`, `nav` and `meta.json`, in that order: the job folder's own removed (`remove_file` or `remove_dir_all`, NotFound ignored), then `fs::rename(fresh/<name>, job_dir/<name>)` when `fresh` holds it. `fs::rename` and not `rename_no_replace`, because the target is gone by then and `rename_no_replace` cannot move a folder on Linux and macOS, where it is a hard link (Task 4).
  4. `fs::remove_dir(fresh)`.
- `fingerprint`: the label as before, then `snapshot_aside(root, &label, &self.job.job_dir, host.now_unix(), &self.job.record.fingerprint)`: Ok is FingerprintDone, Err is `FingerprintFailed { Unknown }` with it. The doc names engine.md:587.
- `file` (Ruling W28): `fs::create_dir_all(folder)` that returns Ok or an `AlreadyExists` error goes on to `fs::read_dir(folder)`; a `read_dir` error gives the L7 text above as `FileFailed { Unknown }`; any other `create_dir_all` error keeps pull request 2's `<folder>: <error>`; the rename runs only after the open succeeded. The folder opened is the title's own, never the library root (LIB-7). The doc names L7, Ruling W28 and design part 6.5.
- `Closed` and `retry` as their docs say.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: PASS.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase --test engine_restart`
Expected: PASS, every pull request 2 test of both files among them.

- [ ] **Step 5: Run the workspace (the junction form)**

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS: the goldens and `lazymkv-dev`'s decide tests are byte-identical (R-B1), and the RP21 guard `no_scenario_holds_a_clone_value` runs.

- [ ] **Step 6: Commit**

```
git add crates/engine/src/file_phase.rs crates/engine/src/steps.rs crates/fake-tools/tests/engine_file_phase.rs
git commit -m "engine: library read errors fail Decide, L7's inaccessible folder fails File, and the fingerprint snapshots aside (engine.md:576, :587, :591)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 19: Bounded MakeMKV checks (engine.md:581 (c), :586, :594; M2; W20)

**Files:**
- Modify: `crates/tools/src/makemkv.rs:87-105` (`DriveListing` gains `past_limit`; `list_drives` takes `limit` and its doc says when each kill comes) and `:341-349` (`LISTING_PAST_LIMIT` between `SCAN_PAST_LIMIT` and `EXIT_WAIT_PASSED`).
- Modify: `crates/tools/src/record.rs:96-97` (the doc of `Limits.header` names its second use).
- Modify: `crates/engine/src/steps.rs:563-580` (the job's `listing` passes `None`; its doc says a job's listing has no limit, S57).
- Modify: `crates/engine/src/selftest.rs:20-23` (the import of `LISTING_PAST_LIMIT` and `EXIT_WAIT_PASSED` from `lazymkv_tools::makemkv`), `:310-312` (the doc of `CheckEnv.cancel`), `:346-399` (`makemkv_check`'s doc, its `list_drives` call and its `Ok(listing)` arms).
- Modify: `crates/fake-tools/tests/makemkv_driver.rs:140, :145, :450` (the three PR 2 calls pass `None`) and two new tests after `a_drive_listing_finds_the_index_and_ignores_msg_5010` (:133-147).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:16-19` (imports) and three new tests after `stop_cancels_a_makemkv_check_whose_listing_hangs` (:1677-1694).

**Interfaces:**
- Consumes: `Limits.header` (record.rs:95-118, 120 s from `Limits::from_settings`, 10 s in the harness's `limits_override`, testkit.rs:455-460); `run_makemkv`'s private `Kill` (makemkv.rs:363-376), which already stops the limit at the call's final message (M1); `MakeMkv::observe` (selftest.rs:120-124); `EXIT_WAIT_PASSED` (makemkv.rs:349).
- Produces:

```rust
// crates/tools/src/makemkv.rs
/// One drive listing: the index for the drive id, the reducer's summary, the run and whether its limit killed it
/// (design parts 3.7, 5.4; Ruling W20). Def: T13; T3-19 adds `past_limit`. Use: T23, T26, T3-19.
#[derive(Clone, Debug)]
pub struct DriveListing {
    /* index, summary, run unchanged */
    /// Whether `list_drives`' `limit` killed the run before its final `MSG:5010` (Ruling W20). `run.timed_out` without
    /// it is the exit wait's kill (S57).
    pub past_limit: bool,
}

/// Runs and records the drive listing into `out` (design part 5.4 step 1). Another MakeMKV process running gives `Err`
/// of kind `ResourceBusy` naming it (S55). With `limit` None, as a job's listing passes (S57), the run is killed only
/// by the exit wait after its `MSG:5010` (`run.timed_out`) or by the cancel (`run.cancelled`). With `Some(limit)`, as
/// the MakeMKV check at engine start and the self-test pass `limits.header` (Ruling W20, amending S57 for the listings
/// outside a job), it is also killed once `limit` has passed since the call started with no `MSG:5010` yet, which
/// sets `past_limit` and `run.timed_out`. Def: T13; T3-19 adds `limit`. Use: T23, T26, T3-19.
pub fn list_drives(ctx: &ToolCtx, program: &Path, debug: bool, drive: &str, out: &Path, limit: Option<Duration>)
    -> io::Result<DriveListing>;

/// The detail of a MakeMKV check whose drive listing `limits.header` killed before its final message (Ruling W20;
/// engine.md:581 (c), :586). Def: T3-19. Use: T3-19.
pub const LISTING_PAST_LIMIT: &str =
    "makemkvcon did not finish the drive listing within the header limit and was killed";
```

- [ ] **Step 1: Write the failing tests**

In `crates/fake-tools/tests/makemkv_driver.rs`, the calls at :140, :145 and :450 gain a last argument `None` (for example `list_drives(ctx, &program, false, "tok0", &out, None)`), and after `a_drive_listing_finds_the_index_and_ignores_msg_5010`:

```rust
/// Ruling W20 (S57 amended for the listings outside a job): a listing given a limit is killed once the limit passes
/// with no final `MSG:5010`, and `past_limit` tells that kill from the exit wait's. The limit is 5 s, long enough for a
/// freshly copied stand-in to start while the other tests run, as in `a_scan_past_its_limit_is_killed_and_fails_as_unknown`.
#[test]
fn a_listing_with_a_limit_is_killed_when_it_hangs() {
    let scratch = Scratch::new("drive-list-limit");
    let env = Env::new();
    env.watchdog();
    let program = stand_in(&scratch,
        r#"{"responses":[{"args_contain":["info","disc:9999"],"stdout":"drive-list.txt","hang_after_line":1}]}"#,
        &[("drive-list.txt", DRIVES.as_bytes())]);
    let out = scratch.0.join("selftest/drive-list.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let started = Instant::now();
    let listing =
        env.run(|ctx| list_drives(ctx, &program, false, "tok0", &out, Some(Duration::from_secs(5)))).unwrap();
    assert!(started.elapsed() >= Duration::from_secs(5), "{:?}", started.elapsed());
    assert!(started.elapsed() < Duration::from_secs(20), "{:?}", started.elapsed());
    assert!(listing.past_limit);
    assert!(listing.run.timed_out);
    assert!(!listing.run.cancelled);
    assert_eq!(listing.index, Some(0));
    assert_eq!(fs::read_to_string(&out).unwrap(), format!("{}\n", DRIVES.lines().next().unwrap()));
}

/// S57 for a job's listing: with no limit, a listing that hangs before its final message is killed by no limit, the
/// header limit included; here only the cancel, raised after 3 s, ends it.
#[test]
fn a_listing_without_a_limit_still_waits_for_its_final_message() {
    let scratch = Scratch::new("drive-list-no-limit");
    let mut env = Env::new();
    env.limits.header = Duration::from_secs(1);
    let program = stand_in(&scratch,
        r#"{"responses":[{"args_contain":["info","disc:9999"],"stdout":"drive-list.txt","hang_after_line":1}]}"#,
        &[("drive-list.txt", DRIVES.as_bytes())]);
    let out = scratch.0.join("job/drive-list.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let cancel = env.cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        cancel.cancel();
    });
    let started = Instant::now();
    let listing = env.run(|ctx| list_drives(ctx, &program, false, "tok0", &out, None)).unwrap();
    assert!(started.elapsed() >= Duration::from_secs(3), "{:?}", started.elapsed());
    assert!(started.elapsed() < Duration::from_secs(20), "{:?}", started.elapsed());
    assert!(listing.run.cancelled);
    assert!(!listing.run.timed_out && !listing.past_limit);
}
```

In `crates/fake-tools/tests/engine_ipc.rs`, the imports gain `use lazymkv_tools::makemkv::{EXIT_WAIT_PASSED, LISTING_PAST_LIMIT};`, and after `stop_cancels_a_makemkv_check_whose_listing_hangs`:

```rust
// ---- Bounded MakeMKV checks (Ruling W20; engine.md:581 (c), :586, :594) ----

/// A MakeMKV version line newer than the scenario's 2.0.0, so a listing that was observed would change
/// `makemkv-check.json`.
const NEWER_VERSION: &str =
    "MSG:1005,0,1,\"MakeMKV v2.0.1 win(x64-release) started\",\"%1 started\",\"MakeMKV v2.0.1 win(x64-release)\"\n";

/// The check at start lists normally, the self-test's listing hangs after its first line (no `MSG:5010`), and the
/// job's listing, scan and rip are movie-direct's. The stand-in's counter already holds the check at start's run.
const SELF_TEST_LISTING_HANGS: &str = r#"{"responses":[
 {"args_contain":["info","disc:9999"],"stdout":"drive-list.txt"},
 {"args_contain":["info","disc:9999"],"stdout":"drive-list.txt","hang_after_line":1},
 {"args_contain":["info","disc:9999"],"stdout":"drive-list.txt"},
 {"args_contain":["info","disc:0"],"stdout":"scan.txt","timing":"scan.txt.timing.jsonl"},
 {"args_contain":["mkv","disc:0","t00-00800"],"stdout":"rip.txt","creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"placeholder.mkv"}]}]}"#;

/// The check at start lists normally; the self-test's listing prints all four lines, its `MSG:5010` among them, and
/// then does not exit.
const SELF_TEST_LISTING_DOES_NOT_EXIT: &str = r#"{"responses":[
 {"args_contain":["info","disc:9999"],"stdout":"drive-list.txt"},
 {"args_contain":["info","disc:9999"],"stdout":"drive-list.txt","hang_after_line":100}]}"#;

/// Ruling W20 (engine.md:581 (c)): the MakeMKV check at engine start passes `limits.header`, the harness's 10 s, so a
/// `makemkvcon` that hangs at service start ends there and the engine then serves its clients. A listing the limit
/// killed is partial, so its version is neither reported nor observed. This test takes more than 10 s.
#[test]
fn a_hanging_start_check_ends_at_the_header_limit_and_the_engine_serves() {
    let h = stand_in(&[DISC_A], settings());
    let recorded = |h: &Harness| -> Value {
        serde_json::from_slice(&fs::read(h.data().join("selftest/makemkv-check.json")).unwrap()).unwrap()
    };
    assert_eq!(recorded(&h), json!({"version": "2.0.0", "licence": null}));
    fs::write(h.tools().join("drive-list-newer.txt"), NEWER_VERSION).unwrap();
    fs::write(h.tools().join("makemkvcon64.json"),
        r#"{"responses":[{"args_contain":["info","disc:9999"],"stdout":"drive-list-newer.txt","hang_after_line":1}]}"#)
        .unwrap();
    let started = Instant::now();
    h.restart();
    let took = started.elapsed();
    assert!(took >= Duration::from_secs(10), "the check at start ended before the header limit: {took:?}");
    assert!(took < Duration::from_secs(10) + WAIT * ci_factor(), "{took:?}");
    let mut c = Client::open(&h);
    let makemkv = check(&c.snapshot()["selftest"], "makemkvcon").clone();
    assert_eq!((&makemkv["ok"], &makemkv["busy"], &makemkv["version"]), (&json!(false), &json!(false), &Value::Null));
    assert_eq!(makemkv["detail"], json!(LISTING_PAST_LIMIT));
    assert_eq!(LISTING_PAST_LIMIT, "makemkvcon did not finish the drive listing within the header limit and was killed");
    assert_eq!(recorded(&h), json!({"version": "2.0.0", "licence": null}), "a listing the limit killed is not observed");
    assert!(c.ok("history", json!({})).as_array().unwrap().is_empty());
}

/// Ruling W20 (engine.md:586): the self-test's listing holds the disc-phase token (R26.c1) only until the header limit,
/// so a disc that arrived while it hung gets the token once the limit kills it. The tools beside the job's are unset
/// as in `a_makemkv_check_holds_the_disc_phase_token_until_its_listing_ends`. This test takes more than 10 s.
#[test]
fn a_hanging_self_test_listing_frees_the_disc_phase_token_at_the_limit() {
    let unset = json!({"ffprobe": null, "ffmpeg": null, "java": null});
    let h = stand_in(&[DISC_A], json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                       "drive_ready_timeout_s": 1, "tool_paths": unset}));
    fs::write(h.tools().join("makemkvcon64.json"), SELF_TEST_LISTING_HANGS).unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    let out = h.data().join("selftest").join("drive-list.txt");
    let at_start = fs::read(&out).unwrap();
    let started = Instant::now();
    c.raw(&json!({"v": 1, "type": "run_selftest", "id": 99, "payload": null}).to_string());
    h.wait("the self-test's listing to hang", |_| fs::read(&out).is_ok_and(|now| !now.is_empty() && now != at_start));
    h.insert("tok0", DISC_A);
    h.deliver();
    h.wait("the arrival to be walked", |h| h.drives_json()["drives"]["tok0"]["fingerprint"].is_string());
    h.settle();
    assert!(h.job_ids().is_empty(), "a job started while the check's listing hung: {:?}", h.job_ids());
    let selftest = loop {
        match read_reply(c.line().expect("a line").trim_end()).unwrap() {
            Reply::Ok { id: Some(99), result } => break result,
            Reply::Push(envelope) => c.pushes.push_back(envelope),
            other => panic!("unexpected {other:?}"),
        }
    };
    assert!(started.elapsed() >= Duration::from_secs(10), "{:?}", started.elapsed());
    let makemkv = check(&selftest, "makemkvcon");
    assert_eq!((&makemkv["ok"], &makemkv["busy"], &makemkv["detail"]),
               (&json!(false), &json!(false), &json!(LISTING_PAST_LIMIT)));
    let id = c.until_state(None, "Scanned")["id"].as_str().unwrap().to_string();
    assert!(h.job_dir(&id).join("drive-list.txt").is_file());
}

/// M2's half that pull request 2 left optional (engine.md:594; Ruling W20): a check's listing that printed its final
/// `MSG:5010` and then did not exit is killed by the exit wait, the harness's 10 s, and the check is not ok and says so.
/// Its listing was whole, so its version is reported. This test takes more than 10 s.
#[test]
fn a_check_killed_by_the_exit_wait_is_not_ok() {
    let h = stand_in(&[DISC_A], settings());
    fs::write(h.tools().join("makemkvcon64.json"), SELF_TEST_LISTING_DOES_NOT_EXIT).unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    let started = Instant::now();
    let result = c.ok("run_selftest", Value::Null);
    assert!(started.elapsed() >= Duration::from_secs(10), "{:?}", started.elapsed());
    let makemkv = check(&result, "makemkvcon");
    assert_eq!((&makemkv["ok"], &makemkv["busy"], &makemkv["detail"]),
               (&json!(false), &json!(false), &json!(EXIT_WAIT_PASSED)));
    assert_eq!(makemkv["version"], json!("2.0.0"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test makemkv_driver --test engine_ipc`
Expected: compile errors: `this function takes 5 arguments but 6 arguments were supplied` (E0061) at every `list_drives` call of `makemkv_driver.rs`, `no field past_limit on type DriveListing` (E0609), and `unresolved import lazymkv_tools::makemkv::LISTING_PAST_LIMIT` (E0432) in `engine_ipc.rs`.

- [ ] **Step 3: Implement**

- `tools::makemkv`: `LISTING_PAST_LIMIT` and the two docs of the Interfaces block. `list_drives` passes `limit` to `run_makemkv` (which already kills at a limit only before the final message, M1) and builds `DriveListing { index, summary, run, past_limit: kill == Kill::Limit }` from the `Kill` it returns, as `scan` and `rip` read it (S57).
- `tools::record`: the doc of `Limits.header` becomes "The longest a header call, such as `mkvmerge -J` or `ffprobe`, may take, and the longest the drive listing of a MakeMKV check outside a job may run before its final message (Ruling W20)."
- `engine::steps::listing`: `list_drives(ctx, program, self.debug(), drive, &out, None)`; its doc gains "A job's listing has no time limit (S57; Ruling W20 bounds only the checks)." The `run.timed_out` branch stays, since with no limit only the exit wait sets it.
- `engine::selftest`:
  - `CheckEnv.cancel`'s doc: "Stops the check's tool runs: the owner gives each check a token of its own and raises it at a stop or a forced shutdown (design parts 3.8, 5.3). The drive listing is also bounded by `limits.header` (Ruling W20)."
  - `makemkv_check` calls `list_drives(ctx, &location.path, debug, "", &out, Some(ctx.limits.header))` inside `with_ctx`, so the start check (`start_until`, owner.rs:240), `selftest`, AwaitingMakeMkv's and AwaitingTool's re-checks are all bounded.
  - Its match on the listing, in this order: `Err` arms unchanged; `Ok(listing) if listing.run.cancelled` unchanged; `Ok(listing) if listing.past_limit` gives `(false, LISTING_PAST_LIMIT.to_string(), None)` with no `observe` call, since the listing is partial and may have stopped before a licence message; any other `Ok(listing)` calls `observe` as in PR 2 and then gives `(false, EXIT_WAIT_PASSED.to_string(), summary.version)` when `listing.run.timed_out` (M2: the listing printed its final message, so its summary is whole, as a job's listing reads it at steps.rs:589-594), else PR 2's match over version and licence. Every arm but the cancelled one goes on to `set_last` as before.
  - Its doc gains: "The listing runs under `limits.header` (Ruling W20): one that limit killed is not ok with `LISTING_PAST_LIMIT`, reports no version and is not observed; one the exit wait killed is not ok with `EXIT_WAIT_PASSED` (M2), with its version reported and observed (S79)."

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-tools --locked`
Expected: PASS (the crate builds with the new signature; its own tests do not call `list_drives`).

Run: `scripts/box.sh cargo test -p fake-tools --locked --test makemkv_driver --test engine_ipc`
Expected: PASS, with the two driver tests and the three `engine_ipc.rs` tests above among them. The PR 2 tests that hang a check's listing stay green: `stop_cancels_a_makemkv_check_whose_listing_hangs` still ends by the stop's cancel within 10 s, and `a_makemkv_check_holds_the_disc_phase_token_until_its_listing_ends` holds its spawn gate for about 3 s (the walk and one `settle`), inside the 10 s header limit, whose clock starts before the spawn (R29.c2b).

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/tools/src/makemkv.rs crates/tools/src/record.rs crates/engine/src/steps.rs crates/engine/src/selftest.rs crates/fake-tools/tests/makemkv_driver.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the MakeMKV checks are bounded and read a killed listing (engine.md:581, :586, :594)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 20: Unpersisted transitions (V1, RR1-16; engine.md:592, :593; W23)

**Files:**
- Modify: `crates/engine/src/owner.rs`:
  - `:60-79` (the private constant `REFEED_MINUTES` beside the owner's other constants);
  - `:746-800` (`Owner` gains `refeed` after `kept`, :792-795) and `:876` (its empty value beside `kept: BTreeSet::new()`);
  - `:1368-1382` (`feed` returns `Fed`; the enum `Fed` just before it);
  - `:1480-1496` (`persist_transition` passes the event's message to `unpersisted` at :1491 and removes the job's `refeed` entry beside `self.kept.remove(id)` at :1496);
  - `:1511-1557` (`unpersisted`'s doc, the doc lines :1520-1521 included, its new parameter and its branch for a state with no failure row, :1539-1547);
  - `:1608-1617` and `:1637-1643` (`grant_next` compares with `Fed::Persisted`);
  - `:1945-1993` (`step_done`'s doc and its loop over the posted events; the cancel's `persisted` at :1966);
  - `:2104-2118` (`on_timer` takes a kept event first; the private `feed_again` after it).
- Modify: `crates/fake-tools/tests/engine_restart.rs` (a helper and four tests at the end of the file, after `awaiting_lookup_recreates_its_timer`, :529-565).

**Interfaces:**
- Consumes: `Timers` through `Owner::start_timer` (owner.rs:1839-1848); `keeps_token` (owner.rs:526-528); `Owner::deliver` (owner.rs:2345-2378), which waits for the disc-phase token (L2); `grant_next` (owner.rs:1586-1645); the testkit's V1 persist faults `Harness::{refuse_persists, allow_persists}` and `ManualTimers` (`timers().pending()`, `fire_timers`).
- Produces:

```rust
// crates/engine/src/owner.rs
/// What `feed` did with an event (review round 1, V1, RR1-16; Ruling W23). Def: T3-20. Use: T3-20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fed {
    /// The reducer refused it in the job's state, or there is no such job: nothing changed, and a refusal wrote its
    /// job.log line `rejected <event> in <state>`.
    Rejected,
    /// Its transition was persisted, the stop hook's case included (`commit`).
    Persisted,
    /// The reducer took it but its transition could not be persisted: the job keeps its persisted state
    /// (`unpersisted`).
    NotPersisted,
}

/// The minutes after which an event `unpersisted` kept is fed again (Ruling W23).
const REFEED_MINUTES: u32 = 1;

// Owner gains:
    /// The event, with its message, that each job in Planned or RipVerified, or an Ejected job's FilePhaseSlot, was fed
    /// when its transition could not be persisted, fed again at its `REFEED_MINUTES` timer until it persists (review
    /// round 1, V1; Ruling W23). A persisted transition of the job takes its entry out.
    refeed: BTreeMap<String, (Event, Option<String>)>,

impl Owner {
    fn feed(&mut self, id: &str, event: Event, message: Option<String>) -> Fed;
    fn unpersisted(&mut self, id: &str, transition: &Transition, message: Option<String>, error: &io::Error);
    fn feed_again(&mut self, id: &str, event: Event, message: Option<String>);
}
```

Engine.log line of a kept event (exact): `<id>: keeps its persisted state <state>; <event tag> is fed again in 1 minute`, with `<state>` from `State::as_str` and `<event tag>` from `event_tag`, for example `abc-1: keeps its persisted state Planned; SpaceOk is fed again in 1 minute`.

- [ ] **Step 1: Write the failing tests**

At the end of `crates/fake-tools/tests/engine_restart.rs`:

```rust
// ---- Unpersisted transitions (review round 1, V1, RR1-16; Ruling W23) ----

/// The ID of the first job of the base scenario's disc.
fn first_job(h: &Harness) -> String {
    format!("{}-1", h.fingerprint(DISC).to_ascii_lowercase())
}

/// RR1-16 (engine.md:593; Ruling W23): with no Java and a profile MakeMKV rejects, RunScan posts ToolMissing { Java }
/// and the fallback's `record.profile`, then reports ScanDone. When ToolMissing's transition and the failure fed in its
/// place both cannot be persisted, the report stops there: ScanDone is not fed, so no Scanned is persisted with the
/// job profile while scan.txt came from the fallback.
#[test]
fn a_lost_tool_missing_java_stops_the_report_before_scan_done() {
    use lazymkv_core::job::Tool;
    let h = harness(&["movie-direct", "movie-direct-fallback"], json!({"keep_tuning_evidence": false,
        "drive_settle_s": 0, "drive_ready_timeout_s": 1, "tool_paths": {"java": null}}));
    h.refuse_persists(|_, transition| {
        transition.event == Some(Event::ToolMissing { tool: Tool::Java }) || matches!(transition.new, State::Failed { .. })
    });
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state Fingerprinted until a restart resumes it");
    h.wait("the lost ToolMissing's failure kept", |h| h.engine_log().contains(&kept));
    h.settle();
    let lines = h.transitions(&id);
    assert!(lines.iter().all(|line| line.event != Some(Event::ScanDone)), "ScanDone was fed: {lines:?}");
    let record = h.record(&id);
    assert_eq!(record.state, State::Fingerprinted);
    assert_eq!(record.profile, "job.mmcp.xml");
    assert!(h.job_dir(&id).join("fallback.mmcp.xml").is_file(), "the fallback scan ran");
}

/// V1 (engine.md:592; Ruling W23): CheckSpace's SpaceOk in Planned, a state with no failure row, whose transition
/// cannot be persisted is kept and fed again at a one-minute timer, again after each refusal, until it persists; the
/// job then rips and files as usual.
#[test]
fn a_planned_job_whose_next_transition_failed_to_persist_moves_on_at_the_retry_timer() {
    let h = harness(&["movie-direct"], settings());
    h.refuse_persists(|_, transition| transition.event == Some(Event::SpaceOk));
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state Planned; SpaceOk is fed again in 1 minute");
    h.wait("SpaceOk kept", |h| h.engine_log().contains(&kept));
    assert_eq!(h.state_of(&id), "Planned");
    assert_eq!(h.timers().pending(), vec![1]);
    h.fire_timers();
    h.wait("SpaceOk kept again", |h| h.engine_log().matches(&kept).count() == 2);
    assert_eq!(h.state_of(&id), "Planned");
    assert_eq!(h.timers().pending(), vec![1]);
    h.allow_persists();
    h.fire_timers();
    h.wait("SpaceOk persisted", |h| h.transitions(&id).iter().any(|line| line.event == Some(Event::SpaceOk)));
    let fed = h.transitions(&id).into_iter().find(|line| line.event == Some(Event::SpaceOk)).unwrap();
    let planned = State::Planned { mode: lazymkv_core::plan::Mode::Direct };
    assert_eq!((fed.old, fed.new), (Some(planned), State::Ripping));
    h.run_until("Filed");
}

/// V1 (engine.md:592; Ruling W23): Eject's Ejected in RipVerified, a state with no failure row, whose transition cannot
/// be persisted is kept and fed again at the one-minute timer; the disc is not ejected a second time.
#[test]
fn a_rip_verified_job_moves_on_at_the_retry_timer() {
    let h = harness(&["movie-direct"], settings());
    h.refuse_persists(|_, transition| transition.event == Some(Event::Ejected));
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state RipVerified; Ejected is fed again in 1 minute");
    h.wait("Ejected kept", |h| h.engine_log().contains(&kept));
    assert_eq!(h.state_of(&id), "RipVerified");
    assert_eq!(h.timers().pending(), vec![1]);
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
    h.allow_persists();
    h.fire_timers();
    h.run_until("Filed");
    let fed = h.transitions(&id).into_iter().find(|line| line.event == Some(Event::Ejected)).unwrap();
    assert_eq!((fed.old, fed.new), (Some(State::RipVerified { after_copy: false }), State::Ejected { after_copy: false }));
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
}

/// V1 (engine.md:592; Ruling W23): an Ejected job's FilePhaseSlot whose transition cannot be persisted goes back into
/// the slot's queue (review round 1) and is granted again at the one-minute timer, with no other job's transition,
/// lookup round, `settings_set` or restart needed.
#[test]
fn an_ejected_jobs_slot_grant_is_persisted_at_the_retry_timer() {
    let h = harness(&["movie-direct"], settings());
    h.refuse_persists(|_, transition| transition.event == Some(Event::FilePhaseSlot));
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state Ejected; FilePhaseSlot is fed again in 1 minute");
    h.wait("FilePhaseSlot kept", |h| h.engine_log().contains(&kept));
    h.settle();
    assert_eq!(h.state_of(&id), "Ejected");
    assert_eq!(h.timers().pending(), vec![1]);
    h.allow_persists();
    h.fire_timers();
    h.run_until("Filed");
    let granted = h.transitions(&id).into_iter().find(|line| line.event == Some(Event::FilePhaseSlot)).unwrap();
    assert_eq!((granted.old, granted.new), (Some(State::Ejected { after_copy: false }), State::Inspecting));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_restart`
Expected: the file compiles (the tests use only PR 2's harness). FAIL in `a_lost_tool_missing_java_stops_the_report_before_scan_done` (`ScanDone was fed`, since `step_done` feeds the result after the lost ToolMissing), and in the other three with `waited 10s for SpaceOk kept`, `waited 10s for Ejected kept` and `waited 10s for FilePhaseSlot kept`. Every PR 2 test of the file passes.

- [ ] **Step 3: Implement**

- `Fed` beside `feed`, with its doc from the Interfaces block. `feed` returns `Fed::Rejected` when there is no such job and on a refusal (its job.log line unchanged), and maps `commit`'s result: `true` to `Fed::Persisted`, `false` to `Fed::NotPersisted`. `feed`'s doc: "Feeds `event` to `id`'s reducer and says what came of it (`Fed`): a refused event writes the job.log line `rejected <event> in <state>` and changes nothing."
- The three readers of `feed`'s result compare with `Fed::Persisted`: the cancel's `persisted` in `step_done`, and `delivered` and `granted` in `grant_next`. The other callers ignore it, as before.
- `step_done`: in the loop over `posted.first`, `let fed = self.feed(id, event, None);` and `if self.stopping || fed == Fed::NotPersisted { self.drop_edits(id); return; }`, so neither a later posted event nor the result's event is fed after a transition that was accepted but not persisted (RR1-16). Its doc's sentence "Otherwise each posted event is fed, then the result's event." gains: "; a posted event whose transition was accepted but could not be persisted ends the report there, and neither a later posted event nor the result's event is fed (RR1-16; Ruling W23)".
- `Owner.refeed` with its doc from the Interfaces block, `BTreeMap::new()` in `Owner::load`'s literal, and `self.refeed.remove(id)` beside `self.kept.remove(id)` in `persist_transition`'s success path.
- `persist_transition` keeps a clone of the `message` it was given before the record copy takes it, and passes it at its `unpersisted` call.
- `unpersisted(id, transition, message, error)`: the guards (forced shutdown, stop, a running step of the job's current generation) and the branch for a state with a failure row are unchanged. In the branch for a transition not into Failed whose state has no failure row: when `transition.event` is `Some(event)` and the job's state is `State::Planned { .. }` or `State::RipVerified { .. }`, or `event` is `Event::FilePhaseSlot` (which `grant_next` feeds only to an `Ejected { after_copy: false }` job), it inserts `(event, message)` into `refeed`, calls `self.start_timer(id, REFEED_MINUTES)` when the job had no entry before, and then writes the engine.log line of the Interfaces block, the timer before the line, so a test that sees the line finds the timer. Any other state with no failure row gets nothing, as in PR 2. Its doc's sentence "A state with no failure row gets nothing: Planned and RipVerified wait for a restart (listed for pull request 3), a waiting, failed or held job can be retried or approved again, and an Ejected job's grant is undone in `grant_next`." becomes: "In a state with no failure row, Planned and RipVerified keep the event in `refeed`, and so does an Ejected job its FilePhaseSlot, whose grant `grant_next` also undoes; each is fed again by `on_timer` after `REFEED_MINUTES` until it persists (Ruling W23). A waiting, failed or held job gets nothing: it can be retried or approved again."
- `on_timer` first does `if let Some((event, message)) = self.refeed.remove(id) { return self.feed_again(id, event, message); }`; StartTimer's timers never belong to Planned, RipVerified or Ejected, so the two kinds of timer never meet in one job. Its doc gains: "A job with a kept event (`refeed`) has it fed again instead (Ruling W23)."
- `feed_again`, whose choice between the token's queue and a direct feed the tests do not fix:

```rust
    /// A kept event's timer ran out (Ruling W23). A FilePhaseSlot asks for the slot again, so the queue's order holds;
    /// an event whose transition keeps the disc-phase token, SpaceOk into Ripping or Copying, goes through `deliver`,
    /// which waits for the token (L2) and carries no message, as SpaceOk has none; any other event is fed with its
    /// message. A refusal again keeps it for another timer through `unpersisted`.
    fn feed_again(&mut self, id: &str, event: Event, message: Option<String>) {
        if event == Event::FilePhaseSlot {
            return self.grant_next();
        }
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        match reduce(&job.record.state, &event) {
            Ok((new, actions)) if keeps_token(&new, &actions) => self.deliver(id, event),
            _ => {
                self.feed(id, event, message);
            }
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_restart --test engine_ipc`
Expected: PASS, the four tests above among them, and every V1 test of `engine_ipc.rs`: `a_file_phase_slot_grant_that_cannot_be_persisted_lets_the_next_job_have_the_slot` fires no timer, so its job is granted the slot when the other job's cancel frees it, as before.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: PASS (`owner/tests.rs`'s V1 tests among them).

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/engine/src/owner.rs crates/fake-tools/tests/engine_restart.rs
git commit -m "engine: an unpersisted transition is fed again and a report stops at it (V1, RR1-16)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 21: Undelivered notifications and the offline transport (spec 12; design part 3.5; W26)

**Files:**
- Modify: `crates/engine/src/notify.rs:1-2` (module doc), `:15-20` (`NotifySink`), new items after it (`NO_DELIVERY`, `NoDelivery`, `undelivered_line`), `:67-79` (`spawn_notifier` takes the store and logs a refusal), and a new `mod tests` at the end.
- Modify: `crates/engine/src/lib.rs:24` (`pub use notify::{NoDelivery, NotifySink, NO_DELIVERY};`).
- Modify: `crates/engine/src/owner.rs:248` (`spawn_notifier` gets `Arc::clone(&store)`) and `:618-628` (`Delivered` returns its sink's result).
- Modify: `crates/engine/src/testkit.rs:25` (imports `HttpTransport` and `OfflineTransport` from `lazymkv_clients`), `:46` (imports `NoDelivery`), `:121-137` (`MemorySink` returns Ok), `:353` (the field `binary_services` after `engine`), `start`'s `Services` literal (:445-449 at `dcd5924`), `use_binary_services` after `scan_waits_for_lookups` (:895-899), and `Folders::harness`'s literal (:1010).
- Modify: `crates/clients/src/transport.rs` (`OFFLINE`, `OfflineTransport` after `user_agent`; one test in its `mod tests`) and `crates/clients/src/lib.rs:16` (re-exports `OfflineTransport` and `OFFLINE`).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:16-19` (imports) and two tests after the self-test section (after `the_token_check_runs_over_the_engines_transport`, :1726).

**Interfaces:**
- Consumes: `Store::log` (store.rs:269-280), which takes one lock per line and also writes the console in a console run; `EngineHandle::stop`, which waits for the notifier thread before it writes `engine stopped` (owner.rs:386-387); `HttpTransport`, `TransportError` (clients/src/transport.rs:15-29); `Harness::{restart, scan_waits_for_lookups, engine_log, notifications, http, timers, record}`.
- Produces:

```rust
// crates/engine/src/notify.rs
/// Where notifications go (design part 8.6, S36; spec 12): pull request 3's binary passes `NoDelivery` (Ruling W26),
/// and pull request 4 adds webhook delivery. Def: T22; T3-21 makes `deliver` return a Result. Use: T22, T26, T29,
/// T3-21, T3-23.
pub trait NotifySink: Send {
    /// Delivers one notification, or gives why it could not. The notifier then writes `undelivered_line` to engine.log
    /// and goes on, so a notification never blocks a job (spec 12, design part 3.5). The reason never holds a secret,
    /// such as a webhook URL (spec 12).
    fn deliver(&mut self, notification: &Notification) -> Result<(), String>;
}

/// The reason `NoDelivery` gives for every notification (Ruling W26). Def: T3-21. Use: T3-21.
pub const NO_DELIVERY: &str = "no notification delivery in this build; it arrives in pull request 4";

/// The sink of pull request 3's engine binary (Ruling W26): it delivers nothing and refuses every notification with
/// `NO_DELIVERY`, so each one is logged undelivered in engine.log (spec 12, design part 3.5) until pull request 4's
/// webhooks. Def: T3-21. Use: T3-21, T3-23.
pub struct NoDelivery;
impl NotifySink for NoDelivery { /* deliver: Err(NO_DELIVERY.to_string()) */ }

/// engine.log's line for a notification the sink refused (spec 12, design part 3.5):
/// `notification <event> for <job id> was not delivered: <reason>`, with `no job` for a notification of no job and
/// `<event>` from `NotifyEvent::as_str`. It holds neither the notification's message nor any other field.
/// Def: T3-21. Use: T3-21.
pub(crate) fn undelivered_line(notification: &Notification, reason: &str) -> String;

/// The notifier thread (design part 4.4): it drains the channel in order into the sink, writes `undelivered_line` of
/// each notification the sink refuses through `store`, the one writer of engine.log (design part 3.5), and ends once
/// every sender is gone. Def: T22; T3-21 adds `store`. Use: T22, T3-21.
pub(crate) fn spawn_notifier(sink: Box<dyn NotifySink>, store: Arc<Store>) -> Sender<Notification>;

// crates/clients/src/transport.rs
/// The reason `OfflineTransport` gives for every request (Ruling W26). Def: T3-21. Use: T3-21, T3-23.
pub const OFFLINE: &str = "no live transport in this build; it arrives in pull request 4";

/// The transport of pull request 3's engine binary (Ruling W26): it opens no socket and answers every URL
/// `Unreachable(OFFLINE)`, so every lookup stays pending and a job waits in AwaitingLookup until pull request 4's
/// `ureq` transport (design parts 8.1, 8.3, 12.6 item 4). Def: T3-21. Use: T3-21, T3-23.
pub struct OfflineTransport;
impl HttpTransport for OfflineTransport { /* get: Err(TransportError::Unreachable(OFFLINE.to_string())) */ }

// crates/engine/src/testkit.rs
impl Harness {
    /// From the next `start`, that is the next `restart`, the engine gets pull request 3's binary transport and sink
    /// (Ruling W26): `OfflineTransport` as `Services.http` and `NoDelivery` as `Services.notify`, so `http()` records
    /// no request and `notifications()` stays empty. Def: T3-21. Use: T3-21.
    pub fn use_binary_services(&self);
}
```

The undelivered line is written by the notifier thread, through the engine's `Store`, not by the owner thread: `EngineHandle::stop` joins the owner before it waits for the notifier (owner.rs:386-387), so a refusal sent back to the owner's channel during a stop, a forced shutdown's failure notifications among them, would be lost unlogged.

- [ ] **Step 1: Write the failing tests**

In `crates/clients/src/transport.rs`'s `mod tests`:

```rust
    /// Ruling W26: the binary's transport answers every request unreachable with one sentence, whatever its URL or
    /// headers, so a lookup reads it as pending (design part 8.3).
    #[test]
    fn the_offline_transport_answers_every_url_unreachable() {
        assert_eq!(OFFLINE, "no live transport in this build; it arrives in pull request 4");
        let urls = ["https://thediscdb.com/api/dischash/ABC", "https://api.themoviedb.org/3/movie/900001", "http://x", ""];
        for url in urls {
            let answer = OfflineTransport.get(url, &[("User-Agent", "lazymkv/test")]);
            assert_eq!(answer, Err(TransportError::Unreachable(OFFLINE.to_string())), "{url}");
        }
    }
```

At the end of `crates/engine/src/notify.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc::{channel, Sender};
    use std::sync::Arc;

    use lazymkv_core::ipc::Notification;
    use lazymkv_core::job::{FailureClass, NotifyEvent};
    use lazymkv_core::testsupport::Scratch;

    use super::{spawn_notifier, undelivered_line, NoDelivery, NotifySink, NO_DELIVERY};
    use crate::store::Store;

    fn notification(event: NotifyEvent, job: Option<&str>) -> Notification {
        Notification { event, job_id: job.map(str::to_string), fingerprint: None, title: None,
            drive: Some("tok0".to_string()), message: "m".to_string(), time: 7 }
    }

    /// Delivers the first notification and refuses every later one; its channel closes when the notifier drops it.
    struct FirstOnly {
        delivered: bool,
        _done: Sender<()>,
    }

    impl NotifySink for FirstOnly {
        fn deliver(&mut self, _: &Notification) -> Result<(), String> {
            if std::mem::replace(&mut self.delivered, true) {
                Err("refused by the test sink".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn no_delivery_refuses_every_notification_with_its_sentence() {
        assert_eq!(NO_DELIVERY, "no notification delivery in this build; it arrives in pull request 4");
        let mut sink = NoDelivery;
        for event in [NotifyEvent::DiscDone, NotifyEvent::Filed, NotifyEvent::EjectFailed] {
            assert_eq!(sink.deliver(&notification(event, Some("abc-1"))), Err(NO_DELIVERY.to_string()));
        }
    }

    #[test]
    fn the_undelivered_line_names_the_event_and_the_job_or_no_job() {
        assert_eq!(undelivered_line(&notification(NotifyEvent::DiscDone, Some("abc-1")), "why"),
                   "notification disc_done for abc-1 was not delivered: why");
        let failed = notification(NotifyEvent::Failed { class: FailureClass::Unknown }, None);
        assert_eq!(undelivered_line(&failed, "why"), "notification failed for no job was not delivered: why");
    }

    /// Spec 12: each refused notification is one engine.log line, in order, and a delivered one writes none.
    #[test]
    fn the_notifier_logs_each_refused_notification_and_nothing_for_a_delivered_one() {
        let scratch = Scratch::new("notifier");
        let store = Arc::new(Store::new(scratch.0.clone(), Arc::new(|| 7), None));
        let (done, ended) = channel::<()>();
        let sender = spawn_notifier(Box::new(FirstOnly { delivered: false, _done: done }), Arc::clone(&store));
        sender.send(notification(NotifyEvent::DiscDone, Some("abc-1"))).unwrap();
        sender.send(notification(NotifyEvent::Filed, Some("abc-1"))).unwrap();
        sender.send(notification(NotifyEvent::Failed { class: FailureClass::Unknown }, None)).unwrap();
        drop(sender);
        let _ = ended.recv(); // Err once the notifier has dropped the sink, after its last line
        assert_eq!(fs::read_to_string(scratch.0.join("engine.log")).unwrap(),
                   "7 notification filed for abc-1 was not delivered: refused by the test sink\n\
                    7 notification failed for no job was not delivered: refused by the test sink\n");
    }
}
```

In `crates/fake-tools/tests/engine_ipc.rs`, the imports gain `use lazymkv_clients::OFFLINE;` and `use lazymkv_engine::NO_DELIVERY;`, and after `the_token_check_runs_over_the_engines_transport`:

```rust
// ---- The binary's transport and sink (spec 12; design part 3.5; Ruling W26) ----

/// Spec 12 and design part 3.5: a notification the sink cannot deliver is logged in engine.log with its event, its job
/// and the sink's reason, and never blocks the job. Under pull request 3's binary sink, `NoDelivery`, every
/// notification is one (Ruling W26).
#[test]
fn an_undelivered_notification_is_logged_with_its_reason() {
    let h = stand_in(&[DISC_A], settings());
    h.use_binary_services();
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC_A);
    let id = id_of(&h, DISC_A);
    let line = format!("notification disc_done for {id} was not delivered: {NO_DELIVERY}");
    h.wait("the undelivered disc_done", |h| h.engine_log().contains(&line));
    assert_eq!(NO_DELIVERY, "no notification delivery in this build; it arrives in pull request 4");
    assert!(h.notifications().is_empty(), "{:?}", h.notifications());
}

/// Ruling W26: with pull request 3's binary transport, `OfflineTransport`, every lookup is unreachable, so a disc goes
/// through its disc phase and then waits in AwaitingLookup with its hourly timer, its waiting message naming the
/// transport's reason; the recorded transport is never asked.
#[test]
fn with_the_offline_transport_a_disc_waits_for_its_lookups() {
    let h = stand_in(&[DISC_A], settings());
    h.use_binary_services();
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC_A);
    let id = id_of(&h, DISC_A);
    h.wait("AwaitingLookup", |h| h.state_of(&id) == "AwaitingLookup");
    h.settle();
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingLookup { return_to: Box::new(State::Ejected { after_copy: false }) });
    let message = record.message.unwrap_or_default();
    assert!(message.contains(&format!("dischash: {OFFLINE}")), "{message}");
    assert_eq!(h.timers().pending(), vec![60]);
    assert!(h.http().requests().is_empty(), "{:?}", h.http().requests());
    let waiting = format!("notification waiting for {id} was not delivered: {NO_DELIVERY}");
    assert!(h.engine_log().contains(&waiting), "{}", h.engine_log());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked`
Expected: compile errors: ``cannot find value `OFFLINE` `` and ``cannot find value `OfflineTransport` `` in `transport.rs`'s tests.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: compile errors in `notify.rs`'s tests: ``unresolved imports `super::undelivered_line`, `super::NoDelivery`, `super::NO_DELIVERY` ``, `method deliver has an incompatible type for trait` for `FirstOnly`, and `this function takes 1 argument but 2 arguments were supplied` for `spawn_notifier`.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc`
Expected: compile errors: ``unresolved import `lazymkv_clients::OFFLINE` ``, ``unresolved import `lazymkv_engine::NO_DELIVERY` `` and ``no method named `use_binary_services` found for struct `Harness` ``.

- [ ] **Step 3: Implement**

- `clients::transport`: `OFFLINE` and `OfflineTransport` as the Interfaces block and the test fix them. `clients/src/lib.rs:16` re-exports both. The module doc's "the live transport of pull request 4 (`ureq`)" gains "and pull request 3's `OfflineTransport`".
- `engine::notify`: the module doc gains "and engine.log's line for a notification the sink refuses (spec 12)". `NotifySink`, `NO_DELIVERY`, `NoDelivery` and `undelivered_line` as the Interfaces block and the tests fix them. `spawn_notifier(sink, store)`: in its loop, `if let Err(reason) = sink.deliver(&notification) { store.log(&undelivered_line(&notification, &reason)); }`.
- `engine/src/lib.rs:24`: `pub use notify::{NoDelivery, NotifySink, NO_DELIVERY};`.
- `engine::owner`: `Delivered::deliver` returns `self.sink.deliver(notification)`; `start_until` passes `Arc::clone(&store)` to `spawn_notifier` (:248).
- `engine::testkit`:
  - `MemorySink::deliver` pushes the clone as before and returns `Ok(())`; the doc of `MemorySink` ends "Def: T22; T3-21 returns Ok. Use: T22 to T29, T3-21."
  - `Harness` gains `binary_services: AtomicBool` ("whether `start` passes pull request 3's binary transport and sink, `use_binary_services`"), `AtomicBool::new(false)` in `Folders::harness`'s literal.
  - `start` reads it once: when set, `http: Arc::new(OfflineTransport) as Arc<dyn HttpTransport>` and `notify: Box::new(NoDelivery)`; otherwise the recorded transport and the `MemorySink` of PR 2.
  - `use_binary_services` stores true, with the doc of the Interfaces block.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked`
Expected: PASS, with `the_offline_transport_answers_every_url_unreachable`.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc`
Expected: PASS, with the two tests above; every PR 2 test that reads `notifications()` is unchanged, since the harness keeps `MemorySink` unless a test calls `use_binary_services`.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: PASS, with the three `notify::tests` tests and `owner.rs`'s own tests.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add crates/engine/src/notify.rs crates/engine/src/lib.rs crates/engine/src/owner.rs crates/engine/src/testkit.rs crates/clients/src/transport.rs crates/clients/src/lib.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine, clients: undelivered notifications go to engine.log; the offline transport (spec 12)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 22: The `command` relay (S35, S76; design parts 9.1, 9.3, 9.5; W10, W24, W25)

**Files:**
- Create: `crates/dev/src/forms.rs` (`Forms` and `read_forms`, moved from `crates/ctl/src/lib.rs:107-142`, with `title` kept; 1 test).
- Create: `crates/engine/src/relay.rs` (the relay's texts, `child_words`, `command_answer`, `RunningCommand`, `spawn_relay`; 3 tests).
- Create: `corpus/scenarios/command-relay/` holding `lazymkvctl.json`, `relay-plan-out.txt`, `relay-plan-err.txt`, `relay-decide-out.txt`, `relay-hang.txt` and `relay-burst.txt` (1000 lines), all synthetic (RP21).
- Create: `crates/fake-tools/tests/engine_command.rs` (20 tests).
- Modify: `crates/dev/src/lib.rs:5-10` (`pub mod forms;` after `pub mod evidence;`).
- Modify: `crates/ctl/src/lib.rs:4` (`use lazymkv_dev::forms::read_forms;` beside the `Args` import, which `retune_spec` still uses), `:107-142` (the private `Forms` and `read_forms` deleted; `local` at :147-159 is unchanged).
- Modify: `crates/engine/src/lib.rs:3-15` (`mod relay;`).
- Modify: `crates/engine/src/file_phase.rs:135-146` (`replay_options` becomes `pub(crate)`, so the relay adds the same four replay files `decide`'s read-back uses).
- Modify: `crates/engine/src/ipc_server.rs:39-40` (`COMMAND_REFUSED` deleted), `:159-222` (`Clients` counts each client's queued lines; `send_waiting`; `COMMAND_HEADROOM`, `SEND_POLL`), `:378-405` (`spawn_reader` sends `Msg::Gone` after it closes its client), and its `mod tests` (3 tests and the writer `Held`).
- Modify: `crates/engine/src/owner.rs:42-46` (imports: `COMMAND_REFUSED` out; `relay`'s items in), `:161-186` (`Msg::CommandDone`, `Msg::Gone`), `:255-269` (`Parts.ctl` from `config.ctl`), `:631-656` (`Parts.ctl`), `:746-801` and `:845-875` (`Owner.command`), `:1064-1105` (the two new messages in `on_message` and `quiet_message`), `:1124-1155` (`shut_down` stops a running command first), `:2212-2229` (`Request::Command` goes to `command`), `:2641-2715` (`shutdown` and `close_and_stop` stop a running command).
- Modify: `crates/engine/src/testkit.rs:317-352` (`Harness.ctl`, `Harness.spawns`), `:386-430` (`new` copies the stand-in `lazymkvctl`), `:433-492` (`start` passes `ctl: self.ctl.clone()` where Task 10 put `ctl: None`), `:1007-1035` (`Runner.ctl`, `Runner.spawns`), `:1037-1081` (`TestGuard.spawns`), and `spawns` after `on_spawn` (:911-916).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:1928-1935` (`command_is_refused_until_the_run_folder_exists` replaced by `command_without_lazymkvctl_beside_the_engine_is_refused`).

Line anchors are at `dcd5924`; Tasks 5, 7 and 10 change some of these files first, so an anchor names the item it points at.

**Interfaces:**
- Consumes: Task 10's `EngineConfig.ctl` and `Clients` with its writer count (`drain`); Task 7's `Host::user_writable` and `FakeHost::set_writable`; Task 5's `TestGuard.inner` (`native_guard()`); pull request 2's `ProcessGuard`, `Spawn`, `GuardedChild`, `KillHandle` (platform lib.rs:53-96), `Push::CommandOutput`, `OutputStream`, `push_line` (core ipc.rs:265-311), `Request::Command { words }`, `no_job` and `STOPPING` (ipc_server.rs:43-48), `file_phase::replay_options`, dev's `Args`.
- Produces:

```rust
// crates/dev/src/forms.rs
//! The engine forms of plan, decide and retune (design part 9.5; Ruling S87), read one way by lazymkvctl and by the
//! engine's `command` relay (Ruling W24).

/// What `--job`, `--jobs` and `--title` took out of a command's words, and the words left, unchanged and in order
/// (Rulings S87, T28-g). Def: T27 (in ctl), moved by T3-22. Use: T27, T3-22.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forms {
    /// `--job`'s value.
    pub job: Option<String>,
    /// Whether `--jobs` was given.
    pub jobs: bool,
    /// `--title`'s value.
    pub title: Option<String>,
    /// The words left, for dev's parser.
    pub words: Vec<String>,
}
/// Takes `--job`, `--jobs` and `--title` out of `rest`, the words after `name`, through
/// `Args::new(rest, &["--job", "--jobs", "--title"])`, `flag`, `switch`, `flag` and `rest()`, and checks which of them
/// `name` allows, with ctl's texts unchanged (Ruling S87). Err is the argument error's text. Def: T27 (in ctl), moved
/// by T3-22. Use: T27, T3-22.
pub fn read_forms(name: &str, rest: &[String]) -> Result<Forms, String>;

// crates/engine/src/relay.rs
//! The `command` relay (S35; design parts 4.4, 9.1, 9.3, 9.5; Rulings W10, W24, W25): the child's words, the thread
//! that streams its lines to the client that asked, and the reply.

/// A second `command` while one runs (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_BUSY: &str = "a command is already running; one runs at a time (design part 9.5)";
/// An engine whose `EngineConfig.ctl` is None (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_NO_CTL: &str = "command needs lazymkvctl beside the engine, and this engine has none";
/// `retune --jobs` before pull request 5 (Ruling W25). Def: T3-22. Use: T3-22.
pub(crate) const RETUNE_WAITS: &str = "retune --jobs needs the evidence trees of pull request 5 (design part 9.5)";
/// A child that ended without an exit code (design part 9.3). Def: T3-22. Use: T3-22.
pub(crate) const NO_EXIT_CODE: &str = "lazymkvctl ended without an exit code";
/// Words that are none of design part 9.5's three engine forms (Ruling W24). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_FORMS: &str = "command needs plan --job, decide --job --title or retune --jobs (design part 9.5)";
/// A `lazymkvctl` whose folder a non-elevated process could write (design part 3.6; Ruling W24). Def: T3-22.
/// Use: T3-22.
pub(crate) const COMMAND_WRITABLE: &str =
    "lazymkvctl's folder can be written by a non-elevated process, so the engine will not run it (design part 3.6)";
/// `job <id> has no title <key>`, for a `decide --job --title` whose key is not one of the job record's titles, so no
/// key leads out of the job folder (Ruling W24). Def: T3-22. Use: T3-22.
pub(crate) fn no_title(id: &str, key: &str) -> String;
/// `lazymkvctl could not be started: <error>` (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) fn not_started(error: &io::Error) -> String;
/// The child's words for a `command`'s `words` (design part 9.5's table; Rulings W24, W25), or the refusal's text.
/// `job` gives a job's folder and its title keys, None for an unknown job. Built words come first and the client's
/// remaining words after them, unchanged, so `Args`' last-wins rule lets a flag the client gave win. Def: T3-22.
/// Use: T3-22.
pub(crate) fn child_words(words: &[String], job: &dyn Fn(&str) -> Option<(PathBuf, Vec<String>)>)
    -> Result<Vec<String>, String>;
/// The reply to a `command` once its child ended (design parts 9.3, 9.5): `{"exit_code": <code>}`, `NO_EXIT_CODE`
/// for none, or `lazymkvctl could not be waited for: <error>`. Def: T3-22. Use: T3-22.
pub(crate) fn command_answer(exit: io::Result<Option<i32>>) -> Result<Value, String>;
/// The command that runs (design part 9.5: one at a time): the client and request it answers and its child's kill
/// handle. Def: T3-22. Use: T3-22.
pub(crate) struct RunningCommand { pub(crate) client: u64, pub(crate) id: Option<u64>, pub(crate) kill: KillHandle }
/// Streams `child`'s lines to `client` and reports its end, on a thread of its own (design parts 4.4, 9.1; Rulings
/// W10, W24): each line of either stream goes out as a `command_output` push carrying `id`, through
/// `Clients::send_waiting`; once both streams end, the child's `wait` goes to the owner as `Msg::CommandDone`.
/// Def: T3-22. Use: T3-22.
pub(crate) fn spawn_relay(child: Box<dyn GuardedChild>, client: u64, id: Option<u64>, clients: Arc<Clients>,
    owner: Sender<Msg>) -> JoinHandle<()>;

// crates/engine/src/ipc_server.rs
/// The places of a client's queue that `send_waiting` leaves free, so that a push, or the command's own reply, that
/// comes while a command's lines wait for the client still finds room (S52 as amended by W10; Ruling T3-22-a).
/// Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_HEADROOM: usize = 64;
/// How often a waiting `send_waiting` looks again.
const SEND_POLL: Duration = Duration::from_millis(10);
impl Clients {
    /// Queues `line` for `client` once its queue holds fewer than `CLIENT_QUEUE - COMMAND_HEADROOM` lines, waiting
    /// for that outside the lock; false once the client is gone (closed, dropped by S52, or its writer ended). Only
    /// the relay's `command_output` lines take this path; every other line keeps S52's drop (Rulings W10, T3-22-a).
    /// Def: T3-22. Use: T3-22.
    pub(crate) fn send_waiting(&self, client: u64, line: String) -> bool;
}

// crates/engine/src/owner.rs (crate-private)
pub(crate) enum Msg { /* earlier variants unchanged */
    /// The relay's child ended (design part 9.5): its `wait`, for the `command` request `id` of `client`.
    CommandDone { client: u64, id: Option<u64>, exit: io::Result<Option<i32>> },
    /// A client's reader thread saw its connection end (design part 9.2), so a command it runs is killed (Ruling W24).
    Gone { client: u64 },
}

// crates/engine/src/testkit.rs
impl Harness {
    /// Every `Spawn` the harness's `TestGuard` saw, in order (design part 12.2): the stand-in tools' and the relay's
    /// `lazymkvctl`. Empty for `new_scripted`, whose guard is not a `TestGuard`. Def: T3-22. Use: T3-22.
    pub fn spawns(&self) -> Vec<Spawn>;
}
```

Settled in this task:
- **Ruling T3-22-a (headroom).** `send_waiting` waits until the client's queue holds fewer than `CLIENT_QUEUE - COMMAND_HEADROOM` (192) lines. Waiting at a full queue (256), as the sheet's words would allow, keeps the queue full while a slow `lazymkvctl` reads a burst, so the next `progress` or `job_update` broadcast meets a full queue and S52 drops the client; the command's reply then goes nowhere and `lazymkvctl` exits 4 with `CLOSED`. Over the real pipe (Task 11's 65536-byte buffers) a client slower than its child is the normal case. `Clients` therefore counts each client's queued lines: one is added before every `try_send` and taken back when it fails, the writer thread takes one after each write and stores 0 when it ends.
- **Ruling T3-22-b (the order of refusals).** A command already running (`COMMAND_BUSY`); `EngineConfig.ctl` None (`COMMAND_NO_CTL`); `host.user_writable(<ctl's folder>)` (`COMMAND_WRITABLE`); then `child_words`: a first word that is not `plan`, `decide` or `retune` (`COMMAND_FORMS`), `read_forms`' error text, `--jobs` (`RETUNE_WAITS`), no `--job` (`COMMAND_FORMS`), an unknown job (`no job <id>`, S76), an unknown title (`no_title`); last, a spawn error (`not_started`). Every refusal is an error reply, so `lazymkvctl` exits 2 (RP24).
- **Ruling T3-22-c (a client that leaves).** The reader thread of a client sends `Msg::Gone` after it closes the client, and the owner kills a command that client runs; a `send_waiting` that finds its client gone kills the child too. A child that prints nothing is thus killed when its client leaves, not when it next prints.
- **Ruling T3-22-d (a stop).** A `shutdown` that is not refused (no step runs, or `force`), and the owner's `shut_down`, kill the running command's child, reply to its `command` with ipc_server's existing `STOPPING` (`the engine is stopping (design part 3.8)`, the text a request during a forced shutdown already gets), and close that client, so a relay thread waiting on a client that never reads cannot hold the stop. The sheet's second `STOPPING` without the part number is not added.
- **Ruling T3-22-e (the spawn).** The owner thread calls `ProcessGuard::spawn`, which only creates the process, so it holds the kill handle from the start; the relay thread reads, waits and reports (design part 4.4: the owner never runs a tool or writes to a client).
- The stand-in's responses are chosen by marker words the tests pass through to the child (`--relay-burst`, `--relay-hang`, `--relay-exit-3`), since the relay passes every client word on; `select` hands a group's first run the first matching response (crates/fake-tools/src/scenario.rs:98-124), so the marker responses come first in `lazymkvctl.json`.

Owner decision 11 lists W10 and W25; neither blocks this task.

- [ ] **Step 1: Write the failing tests and the scenario**

`corpus/scenarios/command-relay/lazymkvctl.json`:

```json
{"responses":[
 {"args_contain":["--relay-burst"],"stdout":"relay-burst.txt"},
 {"args_contain":["--relay-hang"],"stdout":"relay-hang.txt","hang_after_line":1},
 {"args_contain":["--relay-exit-3"],"stdout":"relay-plan-out.txt","stderr":"relay-plan-err.txt","exit_code":3},
 {"args_contain":["decide","--rec"],"stdout":"relay-decide-out.txt"},
 {"args_contain":["plan"],"stdout":"relay-plan-out.txt","stderr":"relay-plan-err.txt"}]}
```

Beside it, each file ending in a newline: `relay-plan-out.txt` holds `relay plan line 1` and `relay plan line 2`; `relay-plan-err.txt` holds `relay plan warning`; `relay-decide-out.txt` holds `relay decide line 1`; `relay-hang.txt` holds `relay hang line`; and `relay-burst.txt` is written with `seq -f 'relay burst line %04g' 1 1000 > corpus/scenarios/command-relay/relay-burst.txt` (`relay burst line 0001` to `relay burst line 1000`).

In `crates/dev/src/forms.rs`, which holds only this test until Step 3, with `pub mod forms;` added to `crates/dev/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn read_forms_keeps_ctls_errors_and_returns_the_title() {
        let forms = |job: Option<&str>, jobs: bool, title: Option<&str>, rest: &[&str]| Forms {
            job: job.map(str::to_string), jobs, title: title.map(str::to_string), words: words(rest),
        };
        assert_eq!(read_forms("decide", &words(&["--job", "a-1", "--t", "t00-00800", "--settings", "s.json"])),
                   Ok(forms(Some("a-1"), false, Some("t00-00800"), &["--settings", "s.json"])));
        assert_eq!(read_forms("retune", &words(&["--jobs", "--spec", "spec.md"])),
                   Ok(forms(None, true, None, &["--spec", "spec.md"])));
        assert_eq!(read_forms("plan", &words(&["slug", "--evidence", "e"])), Ok(forms(None, false, None, &["slug", "--evidence", "e"])));
        for (name, given, message) in [
            ("plan", vec!["--title", "k"], "argument --title: only with --job"),
            ("decide", vec!["--job", "a-1"], "argument --title: required with decide --job"),
            ("plan", vec!["--job", "a-1", "--title", "k"], "argument --title: not allowed with plan"),
            ("retune", vec!["--job", "a-1"], "argument --job: not allowed with retune; use --jobs"),
            ("plan", vec!["--jobs"], "argument --jobs: only with retune"),
            ("plan", vec!["--jo", "a-1"], "ambiguous option: --jo could match --job, --jobs"),
            ("plan", vec!["--job"], "argument --job: expected one argument"),
        ] {
            assert_eq!(read_forms(name, &words(&given)), Err(message.to_string()), "{name} {given:?}");
        }
    }
}
```

In `crates/engine/src/relay.rs`, which holds only these tests until Step 3, with `mod relay;` added to `crates/engine/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use serde_json::json;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    fn text(path: PathBuf) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn the_child_words_are_the_job_folders_then_the_clients_own() {
        let s = Scratch::new("relay-words");
        s.write("a-1/settings.json", "{}");
        s.write("a-1/tmdb-search.json", "{}");
        let folder = s.0.join("a-1");
        let job = |id: &str| (id == "a-1").then(|| (s.0.join(id), vec!["t00-00800".to_string()]));
        let flags = vec!["--settings".to_string(), text(folder.join("settings.json")), "--tmdb-search".to_string(),
                         text(folder.join("tmdb-search.json"))];
        let mut plan = vec!["plan".to_string(), text(folder.clone())];
        plan.extend(flags.clone());
        plan.extend(words(&["--evidence", "e", "--settings", "mine.json"]));
        assert_eq!(child_words(&words(&["plan", "--job", "a-1", "--evidence", "e", "--settings", "mine.json"]), &job),
                   Ok(plan));
        let mut decide = vec!["decide".to_string(), "--rec".to_string(), text(folder.join("t00-00800"))];
        decide.extend(flags);
        assert_eq!(child_words(&words(&["decide", "--job", "a-1", "--t", "t00-00800"]), &job), Ok(decide));
    }

    #[test]
    fn every_other_form_is_refused_with_its_text() {
        let job = |id: &str| (id == "a-1").then(|| (PathBuf::from("jobs").join(id), vec!["t00-00800".to_string()]));
        let cases: Vec<(Vec<&str>, String)> = vec![
            (vec![], COMMAND_FORMS.to_string()),
            (vec!["status"], COMMAND_FORMS.to_string()),
            (vec!["plan", "some-slug"], COMMAND_FORMS.to_string()),
            (vec!["retune", "--set", "x=1"], COMMAND_FORMS.to_string()),
            (vec!["retune", "--jobs", "--spec", "spec.md"], RETUNE_WAITS.to_string()),
            (vec!["plan", "--jobs"], "argument --jobs: only with retune".to_string()),
            (vec!["plan", "--job", "b-1"], "no job b-1".to_string()),
            (vec!["decide", "--job", "a-1", "--title", "t99-00000"], "job a-1 has no title t99-00000".to_string()),
            (vec!["decide", "--job", "a-1", "--title", ".."], "job a-1 has no title ..".to_string()),
        ];
        for (given, refusal) in cases {
            assert_eq!(child_words(&words(&given), &job), Err(refusal), "{given:?}");
        }
    }

    #[test]
    fn the_answer_is_the_exit_code_or_why_there_is_none() {
        assert_eq!(command_answer(Ok(Some(3))), Ok(json!({"exit_code": 3})));
        assert_eq!(command_answer(Ok(None)), Err(NO_EXIT_CODE.to_string()));
        assert_eq!(command_answer(Err(io::Error::other("gone"))),
                   Err("lazymkvctl could not be waited for: gone".to_string()));
    }
}
```

In `crates/engine/src/ipc_server.rs`'s `mod tests`, after `a_client_that_falls_behind_is_dropped_without_slowing_the_engine` (Ruling T3-22-a; `Stuck` cannot show what reached the writer, so `Held` keeps it):

```rust
    /// A writer that blocks until its gate opens, then keeps every byte.
    struct Held(Arc<(Mutex<bool>, Condvar)>, Arc<Mutex<Vec<u8>>>);
    impl Write for Held {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let (lock, cv) = &*self.0;
            let mut open = lock.lock().unwrap();
            while !*open { open = cv.wait(open).unwrap(); }
            self.1.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    fn open(gate: &Arc<(Mutex<bool>, Condvar)>) {
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
    }

    /// The first `count` lines `kept` holds once it holds them (10 s cap).
    fn lines_of(kept: &Arc<Mutex<Vec<u8>>>, count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = String::from_utf8(kept.lock().unwrap().clone()).unwrap();
            if text.lines().count() >= count { return text.lines().take(count).map(str::to_string).collect(); }
            assert!(Instant::now() < deadline, "the writer holds {} lines", text.lines().count());
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A client whose writer is held, and a thread sending it 300 numbered lines through `send_waiting`; the thread
    /// gives true when every send did.
    fn burst() -> (Arc<Clients>, u64, Arc<(Mutex<bool>, Condvar)>, Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<bool>) {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let kept = Arc::new(Mutex::new(Vec::new()));
        let clients = Arc::new(Clients::new());
        let client = clients.add(Box::new(Held(gate.clone(), kept.clone())));
        let sender = {
            let clients = Arc::clone(&clients);
            std::thread::spawn(move || (0..300).all(|n| clients.send_waiting(client, format!("{{\"n\":{n}}}"))))
        };
        std::thread::sleep(Duration::from_millis(200));
        (clients, client, gate, kept, sender)
    }

    #[test]
    fn send_waiting_waits_for_room_and_keeps_every_line_in_order() {
        let (clients, _, gate, kept, sender) = burst();
        assert!(!sender.is_finished(), "send_waiting returned while the client's queue had no room");
        assert_eq!(clients.count(), 1);
        open(&gate);
        assert!(sender.join().unwrap());
        assert_eq!(lines_of(&kept, 300), (0..300).map(|n| format!("{{\"n\":{n}}}")).collect::<Vec<_>>());
    }

    #[test]
    fn a_broadcast_during_a_command_burst_keeps_the_requesting_client() {
        let (clients, _, gate, kept, sender) = burst();
        clients.broadcast("{\"push\":1}");
        assert_eq!(clients.count(), 1, "the broadcast dropped the client whose command waits");
        open(&gate);
        assert!(sender.join().unwrap());
        assert!(lines_of(&kept, 301).contains(&"{\"push\":1}".to_string()));
    }

    #[test]
    fn send_waiting_gives_false_once_the_client_is_closed() {
        let (clients, client, gate, _, sender) = burst();
        let closed = Instant::now();
        clients.close(client);
        assert!(!sender.join().unwrap());
        assert!(closed.elapsed() < Duration::from_secs(5));
        open(&gate);
    }
```

In `crates/fake-tools/tests/engine_ipc.rs`, `command_is_refused_until_the_run_folder_exists` (:1928-1935) becomes:

```rust
#[test]
fn command_without_lazymkvctl_beside_the_engine_is_refused() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("command", json!({"words": ["plan", "--job", "abc-1"]})),
               "command needs lazymkvctl beside the engine, and this engine has none");
}
```

`crates/fake-tools/tests/engine_command.rs`: its module doc, imports, constants and helpers in full, then its 20 tests by name, input and expected value.

```rust
//! The `command` relay on a stand-in lazymkvctl (S35, S76; design parts 9.1, 9.3, 9.5; Rulings W10, W24, W25,
//! T3-22-a to T3-22-d). `Harness::new` copies `fake-tool` as `<tools>/lazymkvctl` when a scenario holds
//! `lazymkvctl.json`; the tests choose its responses with marker words the relay passes through (`--relay-...`).

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use lazymkv_core::ipc::{read_reply, Envelope, Reply};
use lazymkv_core::testsupport::Scratch;
use lazymkv_ctl::{run, Env};
use lazymkv_dev::evidence::Ctx;
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::{IpcConnector, Spawn};
use serde_json::{json, Value};

/// The tests' waits: 20 s, times `ci_factor` (Rulings R29.c2, R29.c5).
const WAIT: Duration = Duration::from_secs(20);
const KEY: &str = "t00-00800";
/// movie-direct's disc and the stand-in lazymkvctl.
const RELAY: &[&str] = &["movie-direct", "command-relay"];
const REPLAY: [&str; 4] = ["settings.json", "tmdb-search.json", "tmdb-record.json", "remembered.json"];
// The relay's texts (Rulings W24, W25, T3-22-d), as the engine's crate-private constants hold them.
const BUSY: &str = "a command is already running; one runs at a time (design part 9.5)";
const NO_CTL: &str = "command needs lazymkvctl beside the engine, and this engine has none";
const RETUNE: &str = "retune --jobs needs the evidence trees of pull request 5 (design part 9.5)";
const FORMS: &str = "command needs plan --job, decide --job --title or retune --jobs (design part 9.5)";
const WRITABLE: &str =
    "lazymkvctl's folder can be written by a non-elevated process, so the engine will not run it (design part 3.6)";
const STOPPING: &str = "the engine is stopping (design part 3.8)";

fn settings() -> Value { json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}) }

fn stand_in(scenarios: &[&str]) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings())
}

/// A harness on RELAY whose movie-direct job is Filed and whose engine was restarted, so no step runs, with the job's
/// id (as ctl_engine.rs's `engine(true)`).
fn filed() -> (Harness, String) {
    let h = stand_in(RELAY);
    h.insert("tok0", "movie-direct");
    h.run_until("Filed");
    h.restart();
    let id = h.job_ids().pop().expect("the filed job");
    (h, id)
}

fn text(path: PathBuf) -> String { path.to_string_lossy().into_owned() }

/// Leaves exactly the replay files `given` of REPLAY in the job folder `job`; one it adds holds `{}`.
fn replay_files(job: &Path, given: &[&str]) {
    for name in REPLAY {
        let path = job.join(name);
        if !given.contains(&name) {
            let _ = fs::remove_file(&path);
        } else if !path.is_file() {
            fs::write(&path, "{}").unwrap();
        }
    }
}

/// `head`, then the flag and path of each replay file of `given`, in design part 9.5's order.
fn built(head: &[String], job: &Path, given: &[&str]) -> Vec<String> {
    let mut words = head.to_vec();
    for (flag, name) in ["--settings", "--tmdb-search", "--tmdb-record", "--remembered"].into_iter().zip(REPLAY) {
        if given.contains(&name) { words.extend([flag.to_string(), text(job.join(name))]); }
    }
    words
}

/// Every spawn of the stand-in lazymkvctl, in order.
fn ctl_spawns(h: &Harness) -> Vec<Spawn> {
    h.spawns().into_iter().filter(|spawn| spawn.program.file_stem().is_some_and(|stem| stem == "lazymkvctl")).collect()
}

/// The lines of `stream` among a command's outputs, in order.
fn of(outputs: &[(String, String)], stream: &str) -> Vec<String> {
    outputs.iter().filter(|(of, _)| of == stream).map(|(_, line)| line.clone()).collect()
}

fn exit(code: i32, id: u64) -> Reply { Reply::Ok { id: Some(id), result: json!({"exit_code": code}) } }

fn pair(stream: &str, line: &str) -> (String, String) { (stream.to_string(), line.to_string()) }

/// One client over the harness's in-memory pipe: engine_ipc.rs's `Client` (:79-150), whose reader thread, `read`
/// (a line within `WAIT * ci_factor()`, else a panic naming the timeout or the closed connection), `snapshot`, `ok`
/// and `err` it copies, with the relay's readers below.
struct Client { lines: Receiver<String>, writer: Box<dyn Write + Send>, next: u64, pushes: VecDeque<Envelope> }

impl Client {
    fn open(h: &Harness) -> Client;
    fn read(&mut self) -> Reply;
    fn snapshot(&mut self);
    /// Sends `kind` with the next id (`next`, then incremented); returns that id.
    fn send(&mut self, kind: &str, payload: Value) -> u64;
    /// Each `command_output` of request `id` as (stream, line), in order, and the reply to `id`. A `command_output`
    /// carrying another id fails the test (`assert_eq!(e.id, Some(id))`); other pushes go to `pushes`.
    fn reply_to(&mut self, id: u64) -> (Vec<(String, String)>, Reply);
    /// `send("command", {"words": words})`, then `reply_to` its id.
    fn command(&mut self, words: &[&str]) -> (Vec<(String, String)>, Reply);
    fn ok(&mut self, kind: &str, payload: Value) -> Value;    // the Ok result of `reply_to`, else a panic
    fn err(&mut self, kind: &str, payload: Value) -> String;  // the Error message of `reply_to`, else a panic
    /// The next `command_output` as (stream, line); other pushes go to `pushes`.
    fn output(&mut self) -> (String, String);
}
```

Each test below is `#[test] fn <name>()`. "`filed`" is `let (h, id) = filed();` with `job = h.job_dir(&id)`; "a client" is `Client::open(&h)` then `snapshot()`; "no spawn" is `assert!(ctl_spawns(&h).is_empty())`; "the args" are `ctl_spawns(&h).pop().unwrap().args`.

| Test | Setup and input | Expected |
|---|---|---|
| `plan_job_runs_the_child_with_the_job_folder_and_its_existing_replay_flags` | `filed`; `replay_files(&job, &REPLAY)`; a client; `command(&["plan", "--job", &id])` | reply `exit(0, 1)`; the args `== built(&["plan", text(job)], &job, &REPLAY)`; `of(.., "stdout") == ["relay plan line 1", "relay plan line 2"]`; `of(.., "stderr") == ["relay plan warning"]` |
| `decide_job_title_runs_decide_on_the_title_folder` | as above; `command(&["decide", "--job", &id, "--title", KEY])` | `exit(0, 1)`; the args `== built(&["decide", "--rec", text(job.join(KEY))], &job, &REPLAY)`; outputs `== [pair("stdout", "relay decide line 1")]` |
| `a_missing_replay_file_adds_no_flag` | `filed`; `replay_files(&job, &["settings.json", "tmdb-search.json"])`; `command` plan | `exit(0, 1)`; the args `== built(&["plan", text(job)], &job, &<those two>)` |
| `the_clients_own_words_come_after_the_built_ones` | `filed`, all replay files; `command(&["plan", "--job", &id, "--settings", "mine.json", "--evidence", "e"])` | `exit(0, 1)`; the args `== built(..REPLAY)` followed by `--settings mine.json --evidence e` |
| `each_child_line_is_a_command_output_with_the_commands_id_to_that_client_only` | `filed`; clients `a` and `b`; `a.next = 7`; `a.command` plan | `exit(0, 7)` and 3 outputs, each carrying id 7; then `b.ok("history", json!({}))` returns, with no `command_output` before it (B's queue is in order) |
| `a_watch_client_sees_no_command_output` | `filed`; clients `watcher` and `a`; `a.command` plan; `a.ok("run_selftest", Value::Null)` | `exit(0, 1)`; `watcher` reads pushes up to `selftest_result`, none of kind `command_output` |
| `a_burst_of_child_lines_larger_than_the_client_queue_all_reach_the_client` | `filed`; `command` plan with `--relay-burst` | `exit(0, 1)`; stdout `relay burst line 0001` to `relay burst line 1000`, in order |
| `the_reply_carries_the_childs_exit_code` | `filed`; `command` plan with `--relay-exit-3` | `exit(3, 1)` |
| `a_second_command_while_one_runs_is_refused` | `filed`; client `a` sends plan with `--relay-hang`; `a.output()`; client `b` | `a.output() == pair("stdout", "relay hang line")`; `b.err("command", plan)` is `BUSY`; one ctl spawn |
| `an_unknown_job_is_refused_with_no_job` | `stand_in(RELAY)`, no job; a client | `plan --job nosuch-1` and `decide --job nosuch-1 --title t00-00800` each refused `no job nosuch-1`; no spawn |
| `an_unknown_title_is_refused` | `filed`; a client; `decide --job <id> --title <key>` for `t99-00000` and `..` | `job <id> has no title <key>` each; no spawn |
| `words_without_job_or_jobs_are_refused` | `stand_in(RELAY)`; a client; words `[]`, `[plan, some-slug]`, `[status]`, `[retune, --set, x=1]`, then `[plan, --jobs]` | `FORMS` for the four; `argument --jobs: only with retune` for the last; no spawn |
| `retune_jobs_is_refused_until_pull_request_5` | `stand_in(RELAY)`; `retune --jobs --spec spec.md` | `RETUNE`; no spawn |
| `an_engine_without_ctl_refuses_command` | `stand_in(&["movie-direct"])`, whose tools hold no `lazymkvctl<EXE_SUFFIX>`; `plan --job abc-1` | `NO_CTL` |
| `a_ctl_in_a_writable_folder_is_refused` | `stand_in(RELAY)`; `h.host().set_writable(h.tools())`; `plan --job abc-1` | `WRITABLE`; no spawn |
| `a_missing_lazymkvctl_is_refused_as_not_started` | `filed`; `fs::remove_file(h.tools().join(format!("lazymkvctl{}", EXE_SUFFIX)))`; `plan --job <id>` | the refusal starts with `lazymkvctl could not be started: ` (Ruling T3-22-b's last refusal) |
| `the_child_runs_below_normal` | `filed`; `command` plan | one ctl spawn, `program == h.tools().join("lazymkvctl<EXE_SUFFIX>")`, `below_normal`, `env` empty (S43) |
| `a_client_that_disconnects_kills_its_child` | `filed`; `a` sends plan with `--relay-hang`, reads `relay hang line`, `drop(a)`; client `b` sends `command` plan every 50 ms while the refusal is `BUSY`, within `WAIT * ci_factor()` | `b` gets `{"exit_code": 0}` before the deadline (the stand-in sleeps until killed, so the relay ends only once the kill came) |
| `a_shutdown_kills_the_running_command_and_answers_it_stopping` | `filed`; `a` sends plan with `--relay-hang` and reads its line; client `b`: `b.ok("shutdown", json!({"force": false}))` | `Value::Null`; `a.reply_to(<that command's id>)` is an Error `STOPPING` |
| `ctl_plan_job_prints_the_childs_lines_and_exits_with_its_code` | `filed`; `run(&["plan", "--job", &id, "--relay-exit-3"], Env { ctx: &Ctx::for_test(&Scratch::new("ctl-relay").0), connector: Some(&h.client()), stdin: &mut &b""[..], out, err })` | `(3, "relay plan line 1\nrelay plan line 2\n", "relay plan warning\n")` |

`crates/fake-tools/tests/ctl_engine.rs`'s `plan_job_with_a_refused_command_exits_2` (:158-164) stays as it is: its engine has no `lazymkvctl.json`, so the refusal is now `COMMAND_NO_CTL`, still exit 2.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked forms::`
Expected: a compile error, ``cannot find function `read_forms` in this scope`` (and `Forms`).

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: compile errors: ``cannot find function `child_words` `` in `relay.rs`'s tests and ``no method named `send_waiting` found for struct `Clients` `` in `ipc_server.rs`'s.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_command --test engine_ipc`
Expected: a compile error, ``no method named `spawns` found for struct `Harness` ``.

- [ ] **Step 3: Implement**

- `dev::forms`: ctl's `Forms` and `read_forms` (crates/ctl/src/lib.rs:107-142), moved unchanged but `pub`, with the docs of the Interfaces block and `title: title` kept in the returned `Forms`. `dev/src/lib.rs` gains `pub mod forms;`. ctl deletes its copies and imports `lazymkv_dev::forms::read_forms`; `local` reads `forms.job`, `forms.jobs` and `forms.words` as before.
- `file_phase::replay_options`: `pub(crate)`, body unchanged.
- `relay.rs`, the Interfaces block's items:
  - `no_title`, `not_started` and `command_answer` as their docs and tests fix them.
  - `child_words`, in this order: the first word must be `plan`, `decide` or `retune`, else `COMMAND_FORMS` (no words included); `read_forms(name, rest)`'s Err is the refusal; `jobs` gives `RETUNE_WAITS`; no `job` gives `COMMAND_FORMS`; `job(id)` None gives `no_job(id)`; for `decide`, a `title` not among the keys gives `no_title`. The words are then `plan <folder>` or `decide --rec <folder joined with the key>`, then `--settings`, `--tmdb-search`, `--tmdb-record` and `--remembered` with each path `replay_options(&folder)` gives, in that order, then `forms.words`; paths as `to_string_lossy`. No `--text` is added (Ruling W25).
  - `spawn_relay`: one thread that takes both streams and `child.killer()`, and runs one forwarding thread per stream. A forwarding thread reads with `BufRead::read_until(b'\n')` to the end; each line loses its `\n` and one `\r` before it and is read with `String::from_utf8_lossy` (PIPE-14's convention), then goes out as `push_line(&Push::CommandOutput { stream, line }, id)` through `send_waiting`. The first `false` kills the child, and the lines after it are read and dropped, so the child never blocks on a full pipe; a read error ends that stream. The relay thread joins both, calls `child.wait()` and sends `Msg::CommandDone { client, id, exit }`, ignoring a send to an owner that is gone.
- `ipc_server.rs`:
  - `COMMAND_REFUSED` is deleted.
  - `Clients`' map value becomes the client's `SyncSender<String>` and an `Arc<AtomicUsize>` of its queued lines (Ruling T3-22-a): `send`, `broadcast` and `send_waiting` add one before each `try_send` and take it back when the send fails; `add`'s writer thread takes one after each write and stores 0 when it ends, alongside Task 10's writer count.
  - `send_waiting` loops: under the lock, a client not in the map gives false; with fewer than `CLIENT_QUEUE - COMMAND_HEADROOM` lines queued it adds one and calls `try_send`: Ok gives true, `Full` takes the one back and waits, `Disconnected` takes it back, removes the client and gives false; with the queue at that mark it waits. A wait is `thread::sleep(SEND_POLL)` with the lock released.
  - `spawn_reader`: after `clients.close(client)` it sends `Msg::Gone { client }`, ignoring a failed send.
- `owner.rs`:
  - `Parts` gains `ctl: Option<PathBuf>`, set from `config.ctl.clone()` in `start_until`; `Owner` gains `command: Option<RunningCommand>`, None in `Owner::load`.
  - `on_request`: `Request::Command { words } => return self.command(client, id, words)`.
  - `command(&mut self, client: u64, id: Option<u64>, words: Vec<String>)`: Ruling T3-22-b's order, with `user_writable` asked of `ctl.parent()`; `child_words` gets a closure giving `(self.store.job_dir(id), <the record's title keys>)` for a job in `self.jobs`. Then `self.parts.env.processes.spawn(&Spawn { program: ctl, args, env: vec![], below_normal: true })`; Err replies `not_started`; Ok stores `RunningCommand { client, id, kill: child.killer() }` and pushes `spawn_relay(child, client, id, Arc::clone(&self.parts.clients), self.sender.clone())` onto `self.threads`. No reply is sent here.
  - `on_message`: `Msg::CommandDone { client, id, exit }` sets `command` to None and replies `command_answer(exit)`; `Msg::Gone { client }` kills the running command's child when that client runs it (Ruling T3-22-c). `quiet_message`: `CommandDone` does nothing (the stop already answered it), `Gone` kills as `on_message` does.
  - `fn stop_command(&mut self)`: takes `command`; when there was one, kills its child, replies `Err(STOPPING)` to it and closes its client (Ruling T3-22-d). `shutdown` calls it once the request is not refused (before `close_and_stop`, or before cancelling the steps of a forced one); `close_and_stop` and `shut_down` call it first.
- `testkit.rs`:
  - `Harness` and `Runner` gain `ctl: Option<PathBuf>` and `spawns: Arc<Mutex<Vec<Spawn>>>`; `TestGuard` gains `spawns` and pushes `spawn.clone()` before anything else in `spawn`.
  - `Harness::new`, after the scenarios are merged into `tools`: when `tools.join("lazymkvctl.json")` is a file, it copies `fake_tool` to `tools.join(format!("lazymkvctl{}", std::env::consts::EXE_SUFFIX))` and keeps that path as `ctl`, else None. `Runner::scripted` has `ctl: None` and an empty list. `start` passes `ctl: self.ctl.clone()`.
  - `spawns`: a clone of the list. The module doc's paragraph on `Harness::new` adds the `lazymkvctl` copy.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-dev -p lazymkv-ctl --locked`
Expected: PASS, `forms::tests::read_forms_keeps_ctls_errors_and_returns_the_title` and every pull request 2 ctl test among them.

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked --lib`
Expected: PASS, with the 3 `relay::tests` and the 3 new `ipc_server::tests`.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_command --test engine_ipc --test ctl_engine`
Expected: PASS: the 20 `engine_command.rs` tests, `command_without_lazymkvctl_beside_the_engine_is_refused` and every other test of the two files.

- [ ] **Step 5: Run the workspace, with the junction**

The task adds `corpus/scenarios/command-relay/`, so the RP21 guard `no_scenario_holds_a_clone_value` must read it (Global Constraints):
Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS, the guard among the tests run, and the goldens unchanged (K5).

- [ ] **Step 6: Commit**

```
git add crates/dev/src/forms.rs crates/dev/src/lib.rs crates/ctl/src/lib.rs crates/engine/src/lib.rs crates/engine/src/relay.rs crates/engine/src/file_phase.rs crates/engine/src/ipc_server.rs crates/engine/src/owner.rs crates/engine/src/testkit.rs corpus/scenarios/command-relay crates/fake-tools/tests/engine_command.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the command relay runs lazymkvctl beside the engine (S35, design part 9.5)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

---

### Task 23: The engine binary on Windows and the native whole job (design parts 3.1 to 3.8, 4.2; S28; W1 to W5, W19, W26, W27)

**Files:**
- Create: `crates/engine/src/native.rs` (`#[cfg(windows)]`: the start-up texts, `NativeOptions`, `run_with`, `mutex_refusal_text`, `binary_paths`; 2 tests in its `mod tests`).
- Create: `crates/platform/src/win/console.rs` (`on_console_stop`; 2 tests).
- Create: `crates/fake-tools/tests/engine_native.rs` (`#![cfg(windows)]`; 7 tests).
- Modify: `crates/engine/src/lib.rs:1-15` (`#[cfg(windows)] pub mod native;`), `:42-74` (`USAGE` made `pub` with `[--data DIR]`; `main_with` routes Windows to `native::run_with`; `data_arg` made `pub(crate)` and returning an `Option`), `:76-101` (its tests).
- Modify: `crates/engine/src/main.rs:1-2` (the doc).
- Modify: `crates/engine/tests/binary.rs` (the header doc; `run_engine` and `absent_data` under `cfg(not(windows))`; `the_binary_has_no_windows_implementations_in_this_build` deleted; `an_argument_that_is_not_unicode_is_read_lossily` split by OS; a Windows `an_unknown_argument_is_a_usage_error_exit_2`).
- Modify: `crates/engine/src/testkit.rs:317-352` (`#[cfg(windows)]` fields `native`, `pipe`, `console_handler`), `:386-430` (`new`'s body moved into a private `prepared`, which `new` and `new_native` share), `:495-500` (`stop` also stops a native run), `:658-669` (`restart` panics on a native harness), `:918-934` (`Drop` stops a native run), and a `#[cfg(windows)] impl Harness` block after `on_spawn` (`new_native`, `pipe_client`, `console_stop_event`, `native_exit`).
- Modify: `crates/platform/src/lib.rs:1` (the crate doc), `:198-200` (`Platform`'s doc), `crates/platform/src/win/mod.rs` (`mod console; pub use console::on_console_stop;`).

**Interfaces:**
- Consumes: Task 2's `own_token`, `TokenFacts`, `EngineMutex`, `MutexRefusal`, `MUTEX_NAME`, `PIPE_NAME`, `test_name`; Task 3's `protect_data_folder`, `program_data`; Task 5's `native_guard`; Task 6's `WinKeepAwake`; Task 7's `WinHost`; Task 10's `ShutdownHandle`, `EngineHandle::{shutdown_handle, wait_for_exit, stop}`, `EngineConfig::{ctl, console}`, `Harness::console_text` and its console buffer; Task 11's `PipeServer::bind`; Task 12's `PipeConnector`; Task 13's `CredentialStore`; Task 14's `share_connector`; Task 15's `WinEjector`; Task 16's `WinDrives`; Task 21's `OfflineTransport` (`lazymkv_clients::transport`) and `NoDelivery`; pull request 2's `start`, `Services`, `StdTimers`, `Store`'s console mirror.
- Produces:

```rust
// crates/engine/src/lib.rs
/// The engine on Windows (design parts 3.1 to 3.8). Def: T3-23. Use: T3-23.
#[cfg(windows)]
pub mod native;
/// The binary's usage line; `--data` is the one optional argument (design part 3.1; Ruling W1). Def: T21, T3-23.
/// Use: T21, T3-23.
pub const USAGE: &str = "usage: lazymkv-engine [--data DIR]";
/// The lazymkv-engine binary's run (design parts 3.1, 4.2, 4.6; S28; Rulings W1, W2). On Windows it is
/// `native::run_with(args, err, NativeOptions::production())`. Elsewhere it reads the arguments with `data_arg`: a
/// usage error prints `USAGE` and `lazymkv-engine: error: <message>` to `err` and returns 2; otherwise it prints
/// `lazymkv-engine runs only on Windows in this version` and returns 1, creating nothing. Def: T21, T3-23.
/// Use: T21, T3-23.
pub fn main_with(args: &[String], err: &mut dyn Write) -> i32;
/// `--data`'s value, None without it (design part 3.1; Ruling W1), or the usage error's message in argparse's words:
/// a missing value, or `unrecognized arguments: <the words left, joined by " ">`. Def: T21, T3-23. Use: T21, T3-23.
pub(crate) fn data_arg(args: &[String]) -> Result<Option<String>, String>;

// crates/engine/src/native.rs (cfg(windows))
//! The engine binary on Windows (design parts 3.1 to 3.8, 4.2; S28; Rulings W1 to W5, W19, W26, W27): the start-up
//! sequence, the native `Platform`, and the wait for a stop.

/// A start whose token is not elevated (design part 3.2; Ruling W3). Def: T3-23. Use: T3-23.
pub const NOT_ELEVATED: &str = "lazymkv-engine: run lazymkv-engine elevated (design part 3.2)";
/// A start that met another engine's mutex (design part 3.3). Def: T3-23. Use: T3-23.
pub const ENGINE_RUNNING: &str = "lazymkv-engine: an engine is already running (design part 3.3)";
/// The console's stop event before `start` returned (design part 3.8; Ruling W19). Def: T3-23. Use: T3-23.
pub const STOPPED_STARTING: &str = "lazymkv-engine: stopped by the console while starting (design part 3.8)";
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
}
impl NativeOptions {
    /// `MUTEX_NAME`, `PIPE_NAME`, and every other field None (design parts 3.3, 4.2). Def: T3-23. Use: T3-23.
    pub fn production() -> NativeOptions;
}
/// Runs the engine on Windows until a `shutdown` request or the console's stop event stops it (design parts 3.1 to
/// 3.8; Rulings W1 to W5, W19, W26): the arguments, the elevation check, the mutex, the data folder, the first pipe
/// instance, the console handler, then `start`. Returns 0 after a clean stop, 1 for a refusal or a failed start, and
/// 2 for a usage error; every refusal's text goes to `err` (Step 3 lists them). Def: T3-23. Use: T3-23.
pub fn run_with(args: &[String], err: &mut dyn Write, options: NativeOptions) -> i32;
/// The start-up text of a mutex refusal (design part 3.3; Rulings W4, W5): `EngineRunning` is `ENGINE_RUNNING`;
/// `Squatter` is `lazymkv-engine: <mutex> is held by <account>, which is not an engine (design part 3.3)`; `Os` is
/// `lazymkv-engine: <mutex> could not be taken: <error>`. Def: T3-23. Use: T3-23 (`run_with` step 3).
pub(crate) fn mutex_refusal_text(mutex: &str, refusal: &MutexRefusal) -> String;
/// The binary's `resources` folder and its sibling `lazymkvctl` from the path of the running executable (design parts
/// 9.5, 11.5; Ruling W26): `(<exe folder>\..\resources, <exe folder>\lazymkvctl<EXE_SUFFIX>)`. Def: T3-23.
/// Use: T3-23 (`run_with` step 7).
pub(crate) fn binary_paths(exe: &Path) -> (PathBuf, PathBuf);

// crates/platform/src/win/console.rs
/// Calls `handler` on the console's stop events through `SetConsoleCtrlHandler` (design part 3.8; Ruling W19):
/// Ctrl+C and Ctrl+Break, after which the handler routine returns at once, and the console window's close, logoff
/// and shutdown, after which it waits for the process to end, since Windows ends the process when the routine
/// returns. A second call replaces the handler. Err is `SetConsoleCtrlHandler`'s. Def: T3-23. Use: T3-23.
pub fn on_console_stop(handler: Box<dyn Fn() + Send + Sync>) -> io::Result<()>;
// private: `#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum StopKind { Return, WaitForExit }`,
// `fn stop_kind(code: u32) -> Option<StopKind>`,
// `extern "system" fn routine(code: u32) -> BOOL`, `static HANDLER: Mutex<Option<Box<dyn Fn() + Send + Sync>>>`,
// `fn set_handler(handler: Option<Box<dyn Fn() + Send + Sync>>)`.

// crates/engine/src/testkit.rs (cfg(windows))
impl Harness {
    /// A harness whose engine is the Windows binary's own start, `native::run_with`, on a thread (design parts 3.1
    /// to 3.8; Ruling W27). The stand-ins, scenarios and settings are `new`'s; the data folder is made again through
    /// `protect_data_folder` with the same settings.json (Ruling T3-23-b). `run_with` gets `["--data", <data>]`, a
    /// buffer of its own for standard error, and `NativeOptions { mutex: Global\<the pipe's name after
    /// \\.\pipe\>, pipe, drives: the harness's SharedDrives, ejector: its RecordingEjector with removals, processes:
    /// its TestGuard, secrets: its MemorySecrets, http: its recorded transport, console: its console buffer,
    /// console_stop: a closure that keeps the handler, token: None }`. The real WinHost, WinKeepAwake and share
    /// connector run, so `host()`, `awake()` and `shares()` are not this engine's. It returns once a client through
    /// `pipe_client()` has read its snapshot line (60 s cap, 240 s under continuous integration, CI), and panics with
    /// `run_with`'s standard error when `run_with` returned first. `restart`, `run_until` and `run_until_event` are not
    /// for it. Def: T3-23. Use: T3-23.
    pub fn new_native(fake_tool: &Path, scenarios: &[&str], settings: Value, pipe: &str) -> Harness;
    /// The real pipe client of this harness's pipe (design part 3.3). Def: T3-23. Use: T3-23.
    pub fn pipe_client(&self) -> PipeConnector;
    /// Calls the console handler `run_with` installed, as Ctrl+C would (design part 3.8; Ruling W19); panics when
    /// none was installed. Def: T3-23. Use: T3-23.
    pub fn console_stop_event(&self);
    /// Waits for `run_with` to return (60 s cap, 240 s under CI) and gives its exit code; the drop then has nothing
    /// to stop (Ruling T3-23-c). Def: T3-23. Use: T3-23.
    pub fn native_exit(&self) -> i32;
}
```

Settled in this task:
- **Ruling T3-23-a (`NativeOptions.processes`).** The sheet's options have no process guard, so a native whole job would spawn through `native_guard()` directly and lose the `TestGuard`'s wait for the recorded lookups (R23.2), which makes the stand-in job deterministic. `NativeOptions` gains `processes`; production passes None (`native_guard()`), and `new_native` passes the harness's `TestGuard`, whose inner guard is `native_guard()` (Task 5), so every child of the native job still runs under the Job Object runner (W27).
- The native job runs on the real `WinHost`, which reads no label from the scenario's volume folder as the harness's `FakeHost::set_label` gives it; `bdmt_eng.xml` names movie-direct's disc ahead of any volume label (`disc_name`, crates/core/src/lookup/name.rs:367-372), so the recorded lookups still match and the job files as `whole_job`'s does.
- **Ruling T3-23-b (the native harness's data folder).** A folder the test process makes need not be owned by Administrators, and `protect_data_folder` refuses any other owner (design part 3.5). `new_native` therefore removes the data folder `Folders` made, has `protect_data_folder(<data>, <its own user security identifier (SID)>)` create it, owned by Administrators with the protected list, and writes the settings.json bytes back.
- **Ruling T3-23-c (the native harness's names and exit).** Its mutex is `Global\` and the pipe's name after `\\.\pipe\`, so a test that made the pipe name with `test_name` knows the mutex to take afterwards; `native_exit` gives `run_with`'s exit code to a test that stopped the engine itself. Both are additions to the sheet. The harness's drop stops a run still going with a forced `shutdown` through the pipe (a connect that finds no engine skips it), joins the thread and asserts 0 unless the test is already panicking.
- **Ruling T3-23-d (start failures).** A token that cannot be read, a drive watcher, keep-awake or console handler that cannot start, a pipe error other than a held name, and an Err from `start` all print `lazymkv-engine: the engine could not start: <error>` and exit 1 (Ruling W2). A stop event before `start` returned writes `STOPPED_STARTING` to the process's standard error and exits 1 at once (Ruling W19); the Job Objects end the MakeMKV check's listing.
- If Owner decision 15's fallback is taken, each test here but `a_native_engine_that_is_not_elevated_says_so_before_the_mutex` starts with `if !own_token().unwrap().elevated { skip("windows-latest is not elevated (design part 12.4)"); return; }`.

Owner decisions 11 (W3) and 12 (account names on standard error only) do not block this task.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/lib.rs`, the tests module becomes:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn a_missing_data_value_is_a_usage_error_exit_2() {
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["--data"]), &mut err), 2);
        assert_eq!(String::from_utf8(err).unwrap(),
                   "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: argument --data: expected one argument\n");
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["--data", "x", "extra"]), &mut err), 2);
        assert_eq!(String::from_utf8(err).unwrap(),
                   "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: extra\n");
    }

    /// Ruling W1. It never calls `main_with` with valid arguments, which would start an engine on Windows.
    #[test]
    fn data_is_optional() {
        assert_eq!(data_arg(&[]), Ok(None));
        assert_eq!(data_arg(&words(&["--data", "x"])), Ok(Some("x".to_string())));
    }

    #[test]
    fn words_left_over_are_a_usage_error() {
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["extra"]), &mut err), 2);
        assert_eq!(String::from_utf8(err).unwrap(),
                   "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: extra\n");
    }
}
```

`crates/engine/tests/binary.rs` becomes:

```rust
//! The engine binary (design parts 3.1, 4.2; S28; Rulings W1, W2, M7). Outside Windows it creates nothing and exits 1
//! with S28's sentence. On Windows only its usage errors run here: a start would take the engine's mutex and pipe, so
//! Task T3-23 tests the start through `native::run_with` with names of its own (`engine_native.rs`).

use std::ffi::OsString;
use std::process::Command;

#[cfg(not(windows))]
fn run_engine(data: &std::path::Path) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(data).output().unwrap();
    (output.status.code(), String::from_utf8(output.stderr).unwrap())
}

#[cfg(not(windows))]
fn absent_data() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("lazymkv-engine-binary-{}", std::process::id()))
}

#[cfg(not(windows))]
#[test]
fn the_binary_refuses_to_run_outside_windows_in_this_version() {
    let data = absent_data();
    assert_eq!(run_engine(&data), (Some(1), "lazymkv-engine runs only on Windows in this version\n".to_string()));
    assert!(!data.exists());
}

/// A word that is not Unicode: a byte that is not UTF-8 on Unix, an unpaired surrogate on Windows.
#[cfg(unix)]
fn not_unicode() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(b"lazymkv-engine-\xff".to_vec())
}

#[cfg(windows)]
fn not_unicode() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    let mut wide: Vec<u16> = "lazymkv-engine-".encode_utf16().collect();
    wide.push(0xD800);
    OsString::from_wide(&wide)
}

/// The binary reads its arguments lossily, as lazymkv-dev's and lazymkvctl's mains do, so a word that is not Unicode
/// reaches `main_with` instead of panicking with exit 101 (M7): outside Windows as a data folder, exit 1.
#[cfg(not(windows))]
#[test]
fn an_argument_that_is_not_unicode_is_read_lossily() {
    let data = std::env::temp_dir().join(not_unicode());
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(&data).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!data.exists());
}

/// M7 on Windows: the word reaches `run_with` as a word left over, a usage error that starts nothing.
#[cfg(windows)]
#[test]
fn an_argument_that_is_not_unicode_is_read_lossily() {
    let word = not_unicode();
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg(&word).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8(output.stderr).unwrap(),
               "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: lazymkv-engine-\u{FFFD}\n");
    assert!(!std::env::temp_dir().join(&word).exists());
}

#[cfg(windows)]
#[test]
fn an_unknown_argument_is_a_usage_error_exit_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("extra").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8(output.stderr).unwrap(),
               "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: extra\n");
}
```

In `crates/platform/src/win/console.rs`, which holds only these tests until Step 3, with `mod console;` added to `crates/platform/src/win/mod.rs` (the routine is called directly and nothing is registered with Windows, so a test run's own Ctrl+C keeps working):

```rust
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
```

In `crates/engine/src/native.rs`, which holds only these tests until Step 3, with `#[cfg(windows)] pub mod native;` added to `lib.rs` (fake-tools cannot make a user-owned mutex, so the squatter's text is pinned here):

```rust
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
    fn the_run_folder_gives_resources_and_lazymkvctl() {
        assert_eq!(binary_paths(Path::new(r"C:\lazymkv\run\lazymkv-engine.exe")),
                   (PathBuf::from(r"C:\lazymkv\run\..\resources"), PathBuf::from(r"C:\lazymkv\run\lazymkvctl.exe")));
    }
}
```

`crates/fake-tools/tests/engine_native.rs`:

```rust
//! The engine binary's Windows start and the native whole job (design parts 3.1 to 3.8, 4.2; S28; Rulings W1 to W5,
//! W19, W26, W27, T3-23-a to T3-23-d). Every engine here uses names of its own (`test_name`), so none meets the desktop
//! task's engine or another test.
#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lazymkv_clients::OfflineTransport;
use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::testsupport::Scratch;
use lazymkv_ctl::{run, Env};
use lazymkv_dev::evidence::{skip, Ctx};
use lazymkv_engine::native::{run_with, NativeOptions, ENGINE_RUNNING, NOT_ELEVATED};
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::security::{test_name, TokenFacts};
use lazymkv_platform::fakes::{MemorySecrets, RecordingEjector, ScriptedDrives};
use lazymkv_platform::win::{own_token, EngineMutex, PipeConnector, PipeServer, WinHost};
use lazymkv_platform::{ConnectError, Host, IpcConnector, StdProcessGuard};
use serde_json::{json, Value};

const KEY: &str = "t00-00800";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";
/// S55's names (crates/tools/src/makemkv.rs:353).
const MAKEMKV: [&str; 3] = ["makemkvcon64", "makemkvcon", "makemkv"];
/// A whole stand-in job's cap: 120 s, 480 s under CI (Ruling R29.c5).
const JOB_CAP: Duration = Duration::from_secs(120);

fn fake_tool() -> &'static Path { Path::new(env!("CARGO_BIN_EXE_fake-tool")) }

fn settings() -> Value { json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}) }

/// Ruling W27: S55 stops every stand-in `makemkvcon` call while a MakeMKV process runs, and the MakeMKV interface may
/// be open on the mini PC. True, after printing the skip line, when one runs and `CI` is unset; never true on CI.
fn makemkv_runs_here() -> bool {
    if std::env::var_os("CI").is_some() {
        return false;
    }
    let running = WinHost::new().running_processes().into_iter().any(|name| {
        let lower = name.to_lowercase();
        MAKEMKV.contains(&lower.strip_suffix(".exe").unwrap_or(&lower))
    });
    if running {
        skip("a MakeMKV process is running on this machine (S55)");
    }
    running
}

/// A test's own mutex, pipe and data folder; the folder is not created.
fn names(tag: &str) -> (String, String, PathBuf) {
    let name = test_name(tag);
    (format!(r"Global\{name}"), format!(r"\\.\pipe\{name}"), std::env::temp_dir().join(format!("{name}-data")))
}

/// Test parts for every service, so a start a refusal wrongly let through meets no drive, vault, console or real
/// `makemkvcon64.exe` (Global Constraints; design part 3.1). Only the token stays None: the real token is what the
/// refusal tests read.
fn options(mutex: &str, pipe: &str) -> NativeOptions {
    NativeOptions {
        mutex: mutex.to_string(),
        pipe: pipe.to_string(),
        drives: Some(Box::new(ScriptedDrives::new().0)),
        ejector: Some(Arc::new(RecordingEjector::new())),
        processes: Some(Arc::new(StdProcessGuard)),
        secrets: Some(Arc::new(MemorySecrets::new())),
        http: Some(Arc::new(OfflineTransport)),
        console: Some(Box::new(std::io::sink())),
        console_stop: Some(Box::new(|_| Ok(()))),
        token: None,
    }
}

/// `run_with(["--data", data], .., options)` on a thread: its exit code and standard error. Fails the test when it
/// has not returned within 30 s (120 s under CI), since it then started an engine instead of refusing.
fn refused(data: &Path, options: NativeOptions) -> (i32, String) {
    let args = vec!["--data".to_string(), data.display().to_string()];
    let (send, got) = channel();
    std::thread::spawn(move || {
        let mut err = Vec::new();
        let code = run_with(&args, &mut err, options);
        let _ = send.send((code, String::from_utf8(err).unwrap()));
    });
    let cap = Duration::from_secs(30) * ci_factor();
    got.recv_timeout(cap).unwrap_or_else(|_| panic!("run_with did not return within {cap:?}: it started an engine"))
}

/// Polls every 20 ms until `done` holds; fails naming `what` after `cap` times `ci_factor`.
fn wait_for(h: &Harness, what: &str, cap: Duration, done: impl Fn(&Harness) -> bool) {
    let deadline = Instant::now() + cap * ci_factor();
    while !done(h) {
        assert!(Instant::now() < deadline, "waited {:?} for {what}", cap * ci_factor());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// lazymkvctl through the native harness's real pipe client.
fn ctl(h: &Harness, argv: &[&str]) -> (i32, String, String) {
    let scratch = Scratch::new("native-ctl");
    let ctx = Ctx::for_test(&scratch.0);
    let connector = h.pipe_client();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
    let code = run(&argv, Env { ctx: &ctx, connector: Some(&connector), stdin: &mut &b""[..], out: &mut out, err: &mut err });
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn a_native_engine_files_a_stand_in_disc_through_the_real_pipe() {
    if makemkv_runs_here() {
        return;
    }
    let pipe = format!(r"\\.\pipe\{}", test_name("native-job"));
    let h = Harness::new_native(fake_tool(), &["movie-direct"], settings(), &pipe);
    let (code, out, err) = ctl(&h, &["status"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["jobs"], json!([]));
    h.insert("tok0", "movie-direct");
    wait_for(&h, "the job to be Filed", JOB_CAP, |h| h.job_ids().first().is_some_and(|id| h.state_of(id) == "Filed"));
    assert!(h.library().join(FINAL).is_file());
    let log = h.engine_log();
    assert!(log.contains("was not delivered: no notification delivery in this build; it arrives in pull request 4"), "{log}");
    assert!(!log.contains("refused a pipe client"), "{log}");
    assert_eq!(ctl(&h, &["stop"]), (0, String::new(), String::new()));
    assert_eq!(h.native_exit(), 0);
    assert!(h.engine_log().ends_with(" engine stopped\n"));
    let mutex = format!(r"Global\{}", pipe.strip_prefix(r"\\.\pipe\").unwrap());
    assert!(EngineMutex::take(&mutex).is_ok(), "the stopped engine released its mutex");
}

#[test]
fn a_second_native_engine_says_an_engine_is_running() {
    let (mutex, pipe, data) = names("native-second");
    let first = EngineMutex::take(&mutex).expect("the first engine's mutex");
    let (code, err) = refused(&data, options(&mutex, &pipe));
    drop(first);
    assert_eq!((code, err), (1, format!("{ENGINE_RUNNING}\n")));
    assert!(!data.exists(), "the mutex comes before the data folder (W3)");
}

#[test]
fn a_native_engine_refuses_a_data_folder_owned_by_the_user() {
    let (mutex, pipe, data) = names("native-owner");
    fs::create_dir_all(&data).unwrap();
    let sid = own_token().unwrap().user_sid;
    let set = Command::new("icacls").arg(&data).args(["/setowner", &format!("*{sid}")]).output().unwrap();
    assert!(set.status.success(), "icacls /setowner: {}", String::from_utf8_lossy(&set.stdout));
    let (code, err) = refused(&data, options(&mutex, &pipe));
    let _ = fs::remove_dir_all(&data);
    assert_eq!(code, 1, "{err}");
    assert!(err.starts_with(&format!("lazymkv-engine: {} is owned by S-1-5-", data.display())), "{err}");
    assert!(err.ends_with(", neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)\n"), "{err}");
    assert!(EngineMutex::take(&mutex).is_ok(), "the refusal released the mutex");
    assert_eq!(PipeConnector::new(&pipe).connect().err(), Some(ConnectError::NotRunning), "the pipe comes after the folder");
}

#[test]
fn a_native_engine_refuses_a_held_pipe_name() {
    let (mutex, pipe, data) = names("native-pipe");
    let held = PipeServer::bind(&pipe).expect("another process's pipe");
    let (code, err) = refused(&data, options(&mutex, &pipe));
    drop(held);
    let _ = fs::remove_dir_all(&data);
    assert_eq!((code, err), (1, format!("lazymkv-engine: another process holds the pipe {pipe} (design part 3.3)\n")));
    assert!(EngineMutex::take(&mutex).is_ok(), "the refusal released the mutex");
}

#[test]
fn a_native_engine_that_is_not_elevated_says_so_before_the_mutex() {
    let (mutex, pipe, data) = names("native-lua");
    let token = TokenFacts { elevated: false, ..own_token().unwrap() };
    let (code, err) = refused(&data, NativeOptions { token: Some(token), ..options(&mutex, &pipe) });
    assert_eq!((code, err), (1, format!("{NOT_ELEVATED}\n")));
    assert!(EngineMutex::take(&mutex).is_ok());
    assert!(!data.exists(), "no data folder before the elevation check (W3)");
}

#[test]
fn the_native_engines_log_lines_reach_its_console_writer() {
    let pipe = format!(r"\\.\pipe\{}", test_name("native-log"));
    let h = Harness::new_native(fake_tool(), &["movie-direct"], settings(), &pipe);
    h.wait("the console writer to hold every engine.log line", |h| {
        h.engine_log().contains(" engine started (lazymkv ") && h.console_text() == h.engine_log()
    });
}

#[test]
fn a_forced_shutdown_from_the_console_handler_stops_running_steps() {
    if makemkv_runs_here() {
        return;
    }
    let pipe = format!(r"\\.\pipe\{}", test_name("native-stop"));
    let h = Harness::new_native(fake_tool(), &["movie-direct", "movie-direct-hang"], settings(), &pipe);
    h.insert("tok0", "movie-direct");
    wait_for(&h, "the rip to run", JOB_CAP, |h| {
        h.job_ids().first().is_some_and(|id| h.job_dir(id).join(KEY).join("rip.txt").is_file())
    });
    let id = h.job_ids().pop().unwrap();
    h.console_stop_event();
    assert_eq!(h.native_exit(), 0);
    assert_eq!(h.state_of(&id), "Failed");
    assert_eq!(h.record(&id).message.as_deref(), Some(CANCELLED));
    assert!(h.job_dir(&id).join(KEY).join("rip.txt.exit.json").is_file(), "the killed rip ended before the engine did");
    assert!(h.engine_log().ends_with(" engine stopped\n"));
}
```

`whole_job.rs` is unchanged and stays green on the three systems.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked`
Expected: compile errors in `lib.rs`'s tests (`data_arg(&[])` is `Result<String, String>`, not `Result<Option<String>, String>`) and in `native.rs`'s (`mutex_refusal_text`, `binary_paths`, `ENGINE_RUNNING` not found); with those fixed by hand, the usage lines would still read `usage: lazymkv-engine --data DIR`.

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::console`
Expected: compile errors, ``cannot find function `stop_kind` `` and ``cannot find function `set_handler` ``.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_native`
Expected: a compile error, ``unresolved import `lazymkv_engine::native` ``.

- [ ] **Step 3: Implement**

- `lib.rs`: `#[cfg(windows)] pub mod native;`; `USAGE` as the Interfaces block. `data_arg`: `Args::new(args, &["--data"])`, `flag("--data")?`, `positionals()?`; words left give `unrecognized arguments: <joined>`; Ok(the flag). `main_with`: `#[cfg(windows)]` returns `native::run_with(args, err, native::NativeOptions::production())`; `#[cfg(not(windows))]` keeps the usage error and S28's sentence.
- `main.rs`: the doc reads "lazymkv-engine's binary (design parts 3.1 to 3.8, 4.1, 4.2; S28): `[--data DIR]` through `lazymkv_engine::main_with`, which runs the engine on Windows (`native::run_with`) and elsewhere exits 1 with S28's sentence." The body is unchanged.
- `native.rs`: `NativeOptions::production()` as its doc says. `run_with`, in this order, each refusal one line on `err` and exit 1 unless said:
  1. `data_arg(args)`: Err prints `USAGE` and `lazymkv-engine: error: <message>`, exit 2.
  2. The token: `options.token`, else `own_token()`. Not elevated: `NOT_ELEVATED`.
  3. `EngineMutex::take(&options.mutex)`: a refusal prints `mutex_refusal_text(&options.mutex, &refusal)`. The mutex is held to the end of `run_with` (design part 3.8: it is released on exit).
  4. The data folder: `--data`'s value, else `program_data()` joined with `lazymkv`, else `lazymkv-engine: %ProgramData% is not set; give --data DIR`. Then `protect_data_folder(&data, &token.user_sid)`; its Err text follows `lazymkv-engine: `.
  5. `PipeServer::bind(&options.pipe)`: kind `AddrInUse` gives `lazymkv-engine: another process holds the pipe <pipe> (design part 3.3)`.
  6. The console handler (Ruling W19): an `Arc<Mutex<Option<ShutdownHandle>>>` starts None; the handler calls `shutdown(true)` on the handle once there is one, and before that writes `STOPPED_STARTING` and a newline to `std::io::stderr()` and calls `std::process::exit(1)`. It is installed through `options.console_stop`, else `on_console_stop`.
  7. The `Platform`: `drives` from the options or `WinDrives::new()`, `ejector` or `WinEjector`, `processes` or `native_guard()`, `awake` `WinKeepAwake::new()`, `secrets` or `CredentialStore::new()`, `shares` `share_connector(Arc::clone(&secrets))`, `ipc` the bound `PipeServer`, `host` `WinHost::new()`. `Services`: `http` or `OfflineTransport`, `notify` `NoDelivery`, `timers` `StdTimers`, `steps` None. `EngineConfig` (Ruling W26): `data`; `resources` and `ctl` from `binary_paths(&current_exe())`; `limits_override` None; `path_var` `std::env::var_os("PATH")`; `console` `Some(options.console, else Box::new(std::io::stderr()))`. When `current_exe()` fails, `resources` and `ctl` are None.
  8. `start(config, services)`. Steps 2, 5 to 8 meet other errors with Ruling T3-23-d's text.
  9. The handle slot gets `engine.shutdown_handle()`; then `engine.wait_for_exit(None)` and `engine.stop()`: Err writes the error's own text and gives 1; Ok gives 0.
- `win/console.rs`: `stop_kind` as its test fixes it. `routine` (a safe `extern "system" fn`, which coerces to `SetConsoleCtrlHandler`'s routine type): with a kind and a handler under the lock, it calls the handler, releases the lock, and for `WaitForExit` sleeps in a loop until Windows or `main`'s `exit` ends the process; it returns 1 (TRUE) after `Return`, and 0 (FALSE) with no kind or no handler. `on_console_stop` sets the handler and calls `SetConsoleCtrlHandler(Some(routine), 1)` the first time only (a static `AtomicBool`); a zero return is `io::Error::last_os_error()`. Each `unsafe` call has its `// SAFETY:` line.
- `testkit.rs`:
  - `Harness` gains, under `cfg(windows)`, `native: Mutex<Option<JoinHandle<i32>>>`, `pipe: Option<String>` and `console_handler: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>`, set empty by `Folders::harness`.
  - `new`'s body up to `harness.start()` becomes `fn prepared(fake_tool, scenarios, settings) -> Harness`; `new` is `prepared` then `start`.
  - `new_native`: `prepared`, then Ruling T3-23-b's data folder, then `run_with` on a thread with the options of its doc (the err buffer is a `ConsoleBuffer` over a vector of its own), then the readiness wait of its doc: a helper thread connects through `pipe_client()` (sleeping 20 ms after each `NotRunning`) and sends the first line it reads; the snapshot's `"type":"snapshot"` ends the wait.
  - `pipe_client`: `PipeConnector::new(<pipe>)`. `console_stop_event`: calls the kept handler. `native_exit`: takes the handle, polls `is_finished` every 20 ms up to `RUN_UNTIL_CAP * ci_factor()`, then `join` (a panic in `run_with` is resumed).
  - `stop` and `Drop`: on a native harness with a run still going, Ruling T3-23-c's forced `shutdown`, the join and the exit-code assertion; `restart` panics with `restart is not for a native harness`.
- `platform/src/lib.rs`: the crate doc reads "lazymkv-platform: the operating-system seams of spec 4.3 as traits (design parts 4.1, 4.2), their Windows implementations in `win` on `windows-sys` (`cfg(windows)`), the pure helpers they use, which every system tests, and in-memory fakes behind the `fakes` feature." `Platform`'s doc: "On Windows `engine::native` fills it from `win`; the fakes fill it in tests." `win/mod.rs` gains the console line.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked`
Expected: PASS: the three `lib.rs` tests, the two `native::tests`, `binary.rs`'s two Windows tests, and every pull request 2 engine test.

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::console`
Expected: PASS, 2 tests.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_native --test whole_job`
Expected: PASS, the 7 native tests and `whole_job`. On the mini PC, the two job tests print `skipped: a MakeMKV process is running on this machine (S55)` while the MakeMKV interface is open; a printed skip is recorded as not run (Task 26), and the run is repeated with MakeMKV closed. On `windows-latest` they never skip.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS. On the CI runs of Linux and macOS, `engine_native.rs` compiles to no test and `binary.rs` keeps S28's two tests.

- [ ] **Step 6: Commit**

```
git add crates/engine/src/lib.rs crates/engine/src/main.rs crates/engine/src/native.rs crates/engine/src/testkit.rs crates/engine/tests/binary.rs crates/platform/src/lib.rs crates/platform/src/win/mod.rs crates/platform/src/win/console.rs crates/fake-tools/tests/engine_native.rs
git commit -m "engine: the Windows binary runs the engine behind its mutex and pipe; the native whole job (design parts 3.1 to 3.8)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

Ctrl+C at a real console runs in Task 25 Step 5a, after Owner decision 18; the refusals at a desktop prompt that is not elevated are Task 25 Step 6's optional item 5.

---

### Task 24: The desktop task script (P5; design parts 3.1, 3.5)

**Files:**
- Create: `scripts/engine-task.ps1` (the whole script is in Step 3; design part 3.1 fixes its steps, and Owner decisions 4, 5 and 13 its settings and checks).
- Create: `crates/fake-tools/tests/scripts.rs` (3 tests: two text checks on every system, a PowerShell parse check under `cfg(windows)`).

**Interfaces:**
- Consumes: the release build in `C:\lazymkv\target\release` (`scripts/box.sh cargo build --workspace --locked --release`); `lazymkvctl status`'s exit codes (0 when an engine answered, 4 when none was reached; design part 9.6, Task 12); `security::folder_sddl` (Task 2), which the test compares with the script.
- Produces: `scripts/engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]`. Its last line is `ENGINE-TASK-DONE status=ok` or `ENGINE-TASK-DONE status=error`, after `ERROR: <reason>` on an error; its exit code is 0 or 1. It writes no account name: `whoami` goes only to Task Scheduler, and `lazymkvctl status`'s snapshot, whose tool paths may name the owner's profile, goes to `nul`. The order:
  1. refuse unless elevated;
  2. refuse while the task `lazymkv-engine` is `Running` or `Queued` (design part 3.1);
  3. refuse when `C:\lazymkv\src` or `C:\lazymkv\target` is owned by, or lets write, a security identifier (SID) other than SYSTEM (`S-1-5-18`), Administrators (`S-1-5-32-544`) and CREATOR OWNER (`S-1-3-0`), unless `-AcceptWritableBuild` (Owner decision 13), naming the SIDs;
  4. refuse when `lazymkv-engine.exe` or `lazymkvctl.exe` is missing from `C:\lazymkv\target\release`;
  5. create `C:\lazymkv\run` and set its owner (Administrators) and protected access list, `folder_sddl` of the owner's SID, then read it back (design part 3.5);
  6. refuse when `C:\lazymkv\run` holds a file other than the three programs;
  7. copy `lazymkv-engine.exe`, `lazymkvctl.exe`, and `lazymkv-ocr.exe` when it exists (pull request 5), into `C:\lazymkv\run`;
  8. refuse when `lazymkvctl status` already answers (a console run holds the pipe);
  9. register the task: no trigger; principal `whoami`, `-LogonType Interactive -RunLevel Highest`; action `C:\lazymkv\run\lazymkv-engine.exe`, with `--data DIR` when `-Data` is given, working folder `C:\lazymkv\run`; Owner decision 4's settings;
  10. start it and ask `C:\lazymkv\run\lazymkvctl.exe status` every 2 s, at most 30 times (60 s); with no answer, the error names the task's last result and the data folder's `engine.log`.

Settled in this task:
- **Ruling T3-24-a (the access list).** The list is set before the copy, so no non-administrator can write the folder while the programs land in it, and a folder already holding a file the script did not put there is refused, since an elevated program loads a library planted beside it. Design part 3.1 lists the copy first; the order changes no result. The list is set with `DirectorySecurity.SetSecurityDescriptorSddlForm` from `O:BA` and `folder_sddl`'s exact string, rather than `icacls` (the sheet's word), so the script and `protect_data_folder` share one string, which the test checks; the read-back accepts `D:PAI` and `D:P`, since Windows may or may not report the inheritance flag.
- **Ruling T3-24-b (the task's working folder).** `C:\lazymkv\run`, so the engine, and the `command` relay's child, which runs in the engine's working folder (W24), never start in `C:\Windows\System32`.
- **Owner decision 4's values.** `-ExecutionTimeLimit ([TimeSpan]::Zero)`, `-Priority 7`, `-AllowStartIfOnBatteries -DontStopIfGoingOnBatteries`, `-MultipleInstances IgnoreNew`, and the console window shown (the action is the engine itself, not a hidden PowerShell). Another answer from the owner changes that one line.
- **Owner decision 5.** Without `-Data` the task's engine uses `%ProgramData%\lazymkv`; `DIR` may hold no space or quote, as `run-interactive.ps1`'s ArgLine may not.
- **The command line.** From the Mac, the script runs as `scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1'`; the single quotes keep the Mac's shell from reading the backslash, which Task 25 step 6's unquoted form would lose.

Nothing in this task runs the script: it registers a task and sets an access list, so its first run is Task 25 step 6, after Owner decisions 3, 4, 5 and 13.

- [ ] **Step 1: Write the failing tests**

`crates/fake-tools/tests/scripts.rs`:

```rust
//! `scripts/engine-task.ps1`, the desktop task's script (P5; design parts 3.1, 3.5; Task T3-24), read and parsed here,
//! never run: running it registers a task, which needs the owner's yes (Owner decision 3).

use std::fs;
use std::path::PathBuf;

use lazymkv_platform::security::folder_sddl;

fn script() -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("scripts").join("engine-task.ps1");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    (path, text)
}

#[test]
fn the_task_script_names_the_task_the_run_folder_and_no_trigger() {
    let (_, text) = script();
    for needle in [
        "$name = 'lazymkv-engine'",
        r"$run = 'C:\lazymkv\run'",
        r"$release = 'C:\lazymkv\target\release'",
        "-LogonType Interactive -RunLevel Highest",
        "-ExecutionTimeLimit ([TimeSpan]::Zero)",
        "-WorkingDirectory $run",
        "ENGINE-TASK-DONE status=",
    ] {
        assert!(text.contains(needle), "the script lacks {needle}");
    }
    for absent in ["New-ScheduledTaskTrigger", "-Trigger"] {
        assert!(!text.contains(absent), "the script holds {absent}");
    }
}

/// Design part 3.5: the run folder gets the data folder's protected list, one string with `protect_data_folder`'s.
#[test]
fn the_run_folders_list_is_the_data_folders() {
    let (_, text) = script();
    assert!(text.contains(&folder_sddl("$sid")), "the script's list is not folder_sddl's");
}

#[cfg(windows)]
#[test]
fn the_task_script_parses_without_errors() {
    let (path, _) = script();
    let check = format!(
        "$t = $null; $e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile('{}', [ref]$t, [ref]$e); \
         $e | ForEach-Object {{ $_.ToString() }}; exit $e.Count",
        path.display()
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &check])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test scripts`
Expected: FAIL, each test panicking with `<path>\scripts\engine-task.ps1: The system cannot find the file specified. (os error 2)`.

- [ ] **Step 3: Write the script**

`scripts/engine-task.ps1`, exactly:

```powershell
# Deploys and starts the engine's desktop task, lazymkv-engine (P5; design parts 3.1, 3.5). Run it elevated over SSH
# through scripts/box.sh, after a release build:
#   scripts/box.sh cargo build --workspace --locked --release
#   scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]'
# It registers a task and sets an access list, which are standing changes: run it only after the owner's yes. DIR may
# hold no space or quote. The last line is "ENGINE-TASK-DONE status=ok|error" and the exit code 0 or 1. It writes no
# account name.
param(
    [string]$Data = '',
    [switch]$AcceptWritableBuild
)
$ErrorActionPreference = 'Stop'
$name = 'lazymkv-engine'
$run = 'C:\lazymkv\run'
$release = 'C:\lazymkv\target\release'
# The trees the programs are built from and copied out of (Owner decision 13).
$trees = 'C:\lazymkv\src', 'C:\lazymkv\target'
# lazymkv-ocr.exe arrives with pull request 5 and is copied once it exists.
$required = 'lazymkv-engine.exe', 'lazymkvctl.exe'
$optional = 'lazymkv-ocr.exe'
# SYSTEM, Administrators, and CREATOR OWNER, which grants only to whoever creates an object below.
$trusted = 'S-1-5-18', 'S-1-5-32-544', 'S-1-3-0'
# Ruling W17's write rights (add a file or write data, add a folder or append, delete a child, delete, change the
# access list, take ownership), with GENERIC_WRITE and GENERIC_ALL, which entries for what is created below carry.
$writeMask = 0x500D0046

# The SIDs other than $trusted that own $folder, or that its access list lets write it or anything in it.
function Get-Writers([string]$folder) {
    $acl = Get-Acl -LiteralPath $folder
    $sidType = [System.Security.Principal.SecurityIdentifier]
    $writers = @()
    $owner = $acl.GetOwner($sidType).Value
    if ($trusted -notcontains $owner) { $writers += $owner }
    foreach ($rule in $acl.GetAccessRules($true, $true, $sidType)) {
        $who = $rule.IdentityReference.Value
        $writes = ([int]$rule.FileSystemRights -band $writeMask) -ne 0
        if ($rule.AccessControlType -eq 'Allow' -and $writes -and $trusted -notcontains $who) { $writers += $who }
    }
    $writers | Sort-Object -Unique
}

$status = 'error'
try {
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $administrator = [System.Security.Principal.WindowsBuiltInRole]::Administrator
    if (-not (New-Object System.Security.Principal.WindowsPrincipal($identity)).IsInRole($administrator)) {
        throw 'run engine-task.ps1 elevated: it registers a task at the highest run level and protects C:\lazymkv\run (design parts 3.1, 3.5)'
    }
    $task = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
    if ($task -and $task.State -in 'Running', 'Queued') {
        throw "the task $name is $($task.State); stop its engine with lazymkvctl stop before deploying again (design part 3.1)"
    }
    foreach ($tree in $trees) {
        $writers = @(Get-Writers $tree)
        if ($writers.Count -eq 0) { continue }
        $list = $writers -join ', '
        if (-not $AcceptWritableBuild) {
            throw "$tree lets $list write, and none of them is SYSTEM or Administrators, so a non-elevated process could change a program before it is copied (Owner decision 13); give -AcceptWritableBuild to accept that on this development machine (scripts/box.sh recreates C:\lazymkv\src at every sync, so a list set on it does not last)"
        }
        "accepted: $tree lets $list write (-AcceptWritableBuild, Owner decision 13)"
    }
    foreach ($program in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $release $program) -PathType Leaf)) {
            throw "$release\$program is missing; build it first with scripts/box.sh cargo build --workspace --locked --release"
        }
    }
    # Design part 3.5's list, folder_sddl's string, set before anything is copied (Ruling T3-24-a).
    $sid = $identity.User.Value
    $dacl = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;$sid)"
    New-Item -ItemType Directory -Force -Path $run | Out-Null
    $protected = New-Object System.Security.AccessControl.DirectorySecurity
    $protected.SetSecurityDescriptorSddlForm("O:BA$dacl", [System.Security.AccessControl.AccessControlSections]'Owner, Access')
    [System.IO.Directory]::SetAccessControl($run, $protected)
    $aces = $dacl.Substring(5)
    $read = (Get-Acl -LiteralPath $run).Sddl
    if (-not ($read.StartsWith('O:BA') -and ($read.EndsWith("D:PAI$aces") -or $read.EndsWith("D:P$aces")))) {
        throw "$run's access list did not read back as the protected list of design part 3.5"
    }
    $known = $required + $optional
    $planted = @(Get-ChildItem -LiteralPath $run -Force | Where-Object { $known -notcontains $_.Name })
    if ($planted.Count -gt 0) {
        throw "$run holds $($planted.Name -join ', '), which this script did not put there; remove it by hand and run the script again (design part 3.5)"
    }
    foreach ($program in $known) {
        $from = Join-Path $release $program
        if (Test-Path -LiteralPath $from -PathType Leaf) { Copy-Item -LiteralPath $from -Destination $run -Force }
    }
    $ctl = Join-Path $run 'lazymkvctl.exe'
    cmd /c "$ctl status >nul 2>&1"
    if ($LASTEXITCODE -eq 0) {
        throw 'an engine already answers on the pipe, such as a console run; stop it with lazymkvctl stop first (design part 3.3)'
    }
    $engine = Join-Path $run 'lazymkv-engine.exe'
    if ($Data) {
        $action = New-ScheduledTaskAction -Execute $engine -Argument "--data $Data" -WorkingDirectory $run
    } else {
        $action = New-ScheduledTaskAction -Execute $engine -WorkingDirectory $run
    }
    # Owner decision 4: no time limit, task priority 7 as phase 0's recordings had, no battery conditions. The engine's
    # console window shows; closing it is a forced shutdown (design part 3.8).
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -Priority 7 -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
    $principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
    Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings -Force -ErrorAction Stop | Out-Null
    Start-ScheduledTask -TaskName $name -ErrorAction Stop
    $answered = $false
    for ($try = 1; $try -le 30 -and -not $answered; $try++) {
        Start-Sleep -Seconds 2
        cmd /c "$ctl status >nul 2>&1"
        $answered = $LASTEXITCODE -eq 0
    }
    if (-not $answered) {
        $folder = if ($Data) { $Data } else { Join-Path $env:ProgramData 'lazymkv' }
        $result = (Get-ScheduledTaskInfo -TaskName $name).LastTaskResult
        throw "the task started, but lazymkvctl status did not answer within 60 s; the task's last result is $result; read $folder\engine.log"
    }
    $status = 'ok'
} catch {
    "ERROR: $($_.Exception.Message)"
} finally {
    "ENGINE-TASK-DONE status=$status"
}
if ($status -ne 'ok') { exit 1 }
```

Why each choice: `cmd /c ... >nul 2>&1` keeps `lazymkvctl`'s standard error out of PowerShell 5.1, which would turn each native standard-error line into an error record and, under `$ErrorActionPreference = 'Stop'`, end the script; `$LASTEXITCODE` is then `lazymkvctl`'s exit code. `Get-Writers` counts entries meant only for what is created below (inherit-only), since those reach the programs in `target\release`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test scripts`
Expected: PASS, 3 tests (on Linux and macOS CI, the 2 text checks). The script is not run.

- [ ] **Step 5: Run the workspace**

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 6: Commit**

```
git add scripts/engine-task.ps1 crates/fake-tools/tests/scripts.rs
git commit -m "scripts: engine-task.ps1 deploys and starts the desktop task (P5, design part 3.1)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line.

The script runs on the mini PC in Task 25 step 6, after Owner decisions 3, 4, 5 and 13; its evidence is that step's.

---

### Task 25: The mini PC runs (design parts 3.1, 12.4, 12.5)

This task writes the one script the desktop-session runs need, then runs on the mini PC everything that continuous integration (CI) cannot: the token check of the Secure Shell (SSH) session, the hardware and share tests that carry `#[ignore]`, a console run of the engine on stand-ins, the desktop task, and one real disc. Each run step waits for the owner decision it names and records its evidence for Task 26. No secret, account name, user security identifier (SID) or drive serial is written anywhere.

**Files:**
- Create: `scripts/phase0/platform-ignored.ps1` (the sheet's script, every character of it in ASCII, the American Standard Code for Information Interchange).
- Modify: `crates/fake-tools/tests/scripts.rs` (one `cfg(windows)` test, appended after Task 24's two).
- Working files in `.superpowers/pr3/` (git-ignored, so never committed and never copied by `scripts/box.sh`; they outlive the session): `pr3-mini-pc-runs.txt` (one evidence line per run, read by Task 26), `console-settings.json` (Step 5), `console-engine.txt` (Step 5).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes:
  - Task 2: the `win::token` tests and their facts line `token facts: elevated=<bool> type=<Default|Full|Limited> integrity=<SID> linked=<bool>`, written past the test harness's capture (Ruling T3-2-d).
  - Task 13: the four `win::creds::tests` tests; without `LAZYMKV_TEST_VAULT` (over SSH) two of them print `skipped: LAZYMKV_TEST_VAULT is not set; ...` and write nothing.
  - Task 14: the test modules `win::shares::desktop` (2 tests), `win::shares::over_ssh` (1) and `win::shares::arranged` (1), which read `LAZYMKV_TEST_SHARE_ROOT`.
  - Task 15: `win::eject::tests::the_tray_opens_on_eject`, which reads `LAZYMKV_TEST_DRIVE`.
  - Task 16: the three `win::drives` tests, among them `an_inserted_and_removed_disc_is_reported_once_each`, which reads `LAZYMKV_TEST_DRIVE`, writes `CUE: insert a disc in <drive>` and `CUE: remove the disc from <drive>` with `testutil::cue`, and waits up to 5 minutes for each.
  - Task 17: the `job.log` line `share access for <root>: <access>`.
  - Task 22: the `command` relay (`lazymkvctl plan --job ID` through the engine).
  - Task 23: the Windows binary, `usage: lazymkv-engine [--data DIR]`, `ENGINE_RUNNING` (`lazymkv-engine: an engine is already running (design part 3.3)`), `NOT_ELEVATED` (`lazymkv-engine: run lazymkv-engine elevated (design part 3.2)`), and the `engine.log` lines `<Unix seconds> engine started (lazymkv 0.0.0)` and `<Unix seconds> engine stopped` (`crates/engine/src/owner.rs:221`, `:388`), mirrored to standard error (W26).
  - Task 24: `scripts/engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]` and its last line `ENGINE-TASK-DONE status=ok|error`.
  - Pull request 2 and phase 0: `scripts/phase0/job.sh`, `run-interactive.ps1` (task `lazymkv-phase0`, re-registered with `-Force` at each start, log `C:\lazymkv\phase0\logs\<log>.txt` ending `PHASE0-DONE status=ok|error exit=N`), `wait.sh` (exit 0 ok, 1 the script threw, 3 not done yet, 4 not running, 5 unreachable); `lazymkvctl`'s exit-4 texts `NOT_RUNNING` (`lazymkvctl: no engine is running`) and `NOT_ELEVATED` (`lazymkvctl: run lazymkvctl elevated to reach the engine`) (`crates/ctl/src/lib.rs:29-39`).
- Produces (no `pub` item):

```powershell
# scripts/phase0/platform-ignored.ps1, run only through job.sh (design part 12.4)
param([Parameter(Mandatory)][string]$Filter, [string]$Drive = '', [string]$ShareRoot = '')
```

```rust
// crates/fake-tools/tests/scripts.rs
#[cfg(windows)] #[test] fn the_platform_ignored_script_parses_and_runs_one_session_of_platform_tests();
```

  and one line per run in `pr3-mini-pc-runs.txt`, of the form `<yyyy-MM-dd>, <SSH | desktop session | console run | desktop task>, <what ran>: <result>`.

Rulings settled in this task:
- **Ruling T3-25-a:** the controller relays a cue by reading the job's log every 10 s over plain ssh (Step 4), not through `wait.sh`, whose first report of a running job comes after about 520 s, past a cue's 5-minute wait. The owner stays at the mini PC for the whole drive job.
- **Ruling T3-25-b:** the share folder of Steps 3 and 4 is `\\unas-pro\media\lazymkv-staging\pr3-tests`, under phase 0's write area (Owner decision 9). Task 14's move test makes and removes one folder below it; `pr3-tests` itself stays, empty.
- **Ruling T3-25-c:** the real disc of Step 7 is stopped with `stop --force` once its first title rips. Planning never waits for a lookup (design part 8.3), so with the offline transport (W26) the job plans with its identity pending, rips every planned title to the share, and reaches AwaitingLookup only after its eject; Owner decision 10 describes this run. The disc is a movie disc that is neither acceptance disc of design part 12.7, so its job never meets acceptance's job lookup.
- **Ruling T3-25-d:** the arranged share run is Step 8, after Step 6, because only the desktop task's engine writes the owner's vault (design part 6.1). Steps 1 to 7 keep the numbers Tasks 2, 13, 14, 15 and the Owner decisions cite.
- **Ruling T3-25-e:** the console run names five stand-ins, `java` among them, as the harness copies one for every `Tool` (`crates/engine/src/testkit.rs:395-399`), so no real program on the elevated engine's `PATH` runs. Its `settings.json` is copied with `scp` from `.superpowers/pr3/`, so no quoting crosses ssh and cmd.
- **Ruling T3-25-f:** the console run's data folder is created from the elevated SSH session before the first start, because `settings.json` must name the stand-ins before the start check runs; if its owner is not Administrators, the controller sets it with `icacls C:\lazymkv\dev\data /setowner *S-1-5-32-544` (Owner decision 2 covers the folder) and reads it again.
- **Ruling T3-25-g:** Step 6 runs `engine-task.ps1` first without `-AcceptWritableBuild`, so Owner decision 13's check meets the real build folders; the switch is passed only after the owner's yes to that run.
- **Ruling T3-25-i:** the console run copies into `C:\lazymkv\dev\bin` and `C:\lazymkv\dev\tools`, not `C:\lazymkv\run`, which belongs to the desktop task and is protected only by `engine-task.ps1` after Owner decision 3; design part 12.5's sentence applies to the desktop task. Since the elevated SSH session runs those programs, and folders made under `C:\` let authenticated users write (design part 3.5), Step 5 sets their owner to Administrators and a protected list (full control for SYSTEM and Administrators, read and execute for Users, `S-1-5-32-545`, so no SID is printed) before the copies; Owner decision 2's yes covers these lists.
- **Ruling T3-25-h:** the File structure lists no new test for this task. One parse test is added, as Task 24 has for its script, because a parse error in a hidden desktop job would show only after a wait and a disc cue.

The commands below use `BOX '<command>'` for `ssh -o BatchMode=yes "$LAZYMKV_BOX" '<command>'`, with `LAZYMKV_BOX` read as `scripts/box.sh` reads it (`. scripts/phase0/box.env`). Single quotes keep the controller's shell from reading backslashes and `&&`. When ssh times out, the Mac's Tailscale is not connected, and the controller asks the owner.

- [ ] **Step A: Write the failing test**

Append to `crates/fake-tools/tests/scripts.rs` (full paths, so the test needs no import of Task 24's):

```rust
#[cfg(windows)]
#[test]
fn the_platform_ignored_script_parses_and_runs_one_session_of_platform_tests() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\..\scripts\phase0\platform-ignored.ps1");
    let text = std::fs::read_to_string(&script).unwrap_or_else(|error| panic!("{}: {error}", script.display()));
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    let parse = format!(
        "$errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile('{}', [ref]$null, [ref]$errors); $errors.Count",
        script.display()
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &parse])
        .output()
        .expect("powershell starts");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "0", "{}", String::from_utf8_lossy(&out.stderr));
    for line in [
        "[Parameter(Mandatory)][string]$Filter",
        r#"$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path""#,
        r"$env:CARGO_TARGET_DIR = 'C:\lazymkv\target'",
        "$env:LAZYMKV_TEST_DRIVE = $Drive",
        "$env:LAZYMKV_TEST_SHARE_ROOT = $ShareRoot",
        "$env:LAZYMKV_TEST_VAULT = if ($Filter -like 'win::creds::*') { '1' } else { '' }",
        r"Set-Location 'C:\lazymkv\src'",
        r#"cmd /c "cargo test -p lazymkv-platform --locked -- --include-ignored --test-threads=1 --nocapture $Filter 2>&1""#,
    ] {
        assert!(text.contains(line), "platform-ignored.ps1 lacks: {line}");
    }
    assert!(!text.contains("ScheduledTask"), "the script registers nothing; run-interactive.ps1 does");
}
```

- [ ] **Step B: Run it to verify it fails**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test scripts platform_ignored`
Expected: FAIL, the panic `C:\lazymkv\src\crates\fake-tools\..\..\scripts\phase0\platform-ignored.ps1: The system cannot find the file specified. (os error 2)`.

- [ ] **Step C: Write the script**

`scripts/phase0/platform-ignored.ps1`, exactly (the sheet fixes its command):

```powershell
# Runs one session's lazymkv-platform tests in the owner's desktop session on the mini PC (design part 12.4): the
# DriveWatcher, real-share and Credential Manager tests that SSH cannot run. It is started from the Mac through
# job.sh, so it lives under scripts\phase0\, the only folder run-interactive.ps1 runs, and its output is the log
# C:\lazymkv\phase0\logs\<log name>.txt. Build the test binary first over SSH
# (scripts/box.sh cargo test -p lazymkv-platform --locked --no-run), so this run only starts it, and run no
# scripts/box.sh until the job ends: each sync replaces C:\lazymkv\src.
# Usage: scripts/phase0/job.sh platform-ignored.ps1 <log name> -Filter <module path> [-Drive <id>] [-ShareRoot <UNC folder>]
# job.sh passes the arguments unquoted, so no value may hold a space or a quote.
param(
    [Parameter(Mandatory)][string]$Filter,
    [string]$Drive = '',
    [string]$ShareRoot = ''
)
$ErrorActionPreference = 'Stop'
# As scripts/box.sh sets them for a command over SSH.
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:CARGO_TARGET_DIR = 'C:\lazymkv\target'
# The ignored tests read these. An empty value removes the variable, and a test without its input prints
# "skipped: <reason>" and passes (design part 12.1's convention); the notes record that as not run.
$env:LAZYMKV_TEST_DRIVE = $Drive
$env:LAZYMKV_TEST_SHARE_ROOT = $ShareRoot
# The vault tests write the owner's Credential Manager only in their own job (Owner decision 8).
$env:LAZYMKV_TEST_VAULT = if ($Filter -like 'win::creds::*') { '1' } else { '' }
"platform-ignored: filter $Filter, drive '$Drive', share root '$ShareRoot'"
Set-Location 'C:\lazymkv\src'
# cmd joins the test binary's standard error to its standard output, so each "CUE:" line reaches the log when it is
# written, and Windows PowerShell 5.1 never turns a native standard-error line into an error record.
# --include-ignored also runs the module's tests that are not ignored; --test-threads=1 gives one cue at a time.
cmd /c "cargo test -p lazymkv-platform --locked -- --include-ignored --test-threads=1 --nocapture $Filter 2>&1"
if ($LASTEXITCODE -ne 0) { throw "cargo test exited $LASTEXITCODE" }
```

- [ ] **Step D: Run it to verify it passes, then the workspace**

Run: `scripts/box.sh cargo test -p fake-tools --locked --test scripts platform_ignored`
Expected: PASS, `1 passed`.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step E: Commit**

```bash
git add scripts/phase0/platform-ignored.ps1 crates/fake-tools/tests/scripts.rs
git commit -m "scripts: the desktop-session run of the platform's ignored tests (design part 12.4)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and no attribution line. A fix that a later step of this task needs goes into a further commit of this task, with the same check.

Rules for Steps 1 to 8: one cargo command at a time on the mini PC; no build while a real disc rips (design part 12.5); no `scripts/box.sh` call while a desktop job of Step 4 or the console engine of Step 5 runs, since each sync replaces `C:\lazymkv\src`. A run that prints `skipped:` is recorded as not run. Evidence never quotes a `DRV` line (it carries the drive's serial), the snapshot's `settings` (its tool paths name the owner's profile) or a SID other than the well-known ones; the owner's SID is written `<user SID>` and a path under his profile `C:\Users\USER\...`. Nor does it quote the real disc's job ID or fingerprint, disc name, volume label or title sizes, or any line of its plan, `transitions.jsonl` or `job.log` beyond the state names and the share access line (RP21: values read only from the owner's disc stay out of the public repository); a folder of a share the owner arranges is written `\\SERVER\SHARE\<rest>` unless it is under `\\unas-pro\media`.

- [ ] **Step 1: The SSH session's token (design part 3.2)**

Run: `scripts/box.sh cargo test -p lazymkv-platform --locked win::token -- --nocapture`
Expected: PASS, every `win::token` test, and on standard error one line `token facts: elevated=true type=<Default|Full|Limited> integrity=S-1-16-12288 linked=<true|false>`. `elevated=true` confirms design part 3.2's premise that the SSH session is elevated; a failure of the elevation assertion stops the task and goes to the owner.
Evidence: `<date>, SSH, win::token: <the facts line as printed>`.

- [ ] **Step 2: The `Ejector` over SSH (Manual on the mini PC; Owner decision 17; the owner present with a disc)**

1. The controller asks the owner in chat to insert a disc in `D:`. Then `scripts/box.sh 'tasklist | findstr /i makemkv'` prints nothing (its exit 1 with no output is the pass): no MakeMKV program holds the drive (S55).
2. Run: `scripts/box.sh 'set LAZYMKV_TEST_DRIVE=D:&& cargo test -p lazymkv-platform --locked -- --ignored --test-threads=1 win::eject::'`
   Expected: `test win::eject::tests::the_tray_opens_on_eject ... ok`, `1 passed`.
3. The owner says in chat whether the disc came out, and keeps it out: Step 4's drive test starts with `D:` empty.

Evidence: `<date>, SSH, the_tray_opens_on_eject on drive D: ok; the owner saw the disc come out`, or the test's message, whose report names each step's error code.

- [ ] **Step 3: The share over SSH (Owner decision 7)**

Run: `scripts/box.sh 'set LAZYMKV_TEST_SHARE_ROOT=\\unas-pro\media\lazymkv-staging\pr3-tests&& cargo test -p lazymkv-platform --locked -- --ignored --test-threads=1 win::shares::over_ssh::'`
Expected: `test win::shares::over_ssh::over_ssh_a_denied_share_without_a_reachable_credential_says_so ... ok`, `1 passed`. This is P7's failure branch on the real network-attached storage (NAS): the session is denied, the test's own credential prefix reads nothing, no connection is tried and nothing is written. A panic saying the share answered over SSH is recorded as found, since design part 6.1 expects a denial there.
Evidence: `<date>, SSH, over_ssh_a_denied_share_without_a_reachable_credential_says_so with \\unas-pro\media\lazymkv-staging\pr3-tests: ok`.

- [ ] **Step 4: The desktop-session tests (Manual on the mini PC; Owner decisions 6, 8 and 9)**

The owner is logged on at the mini PC's monitor (locked is enough) and, for the first job, at the mini PC with a disc in hand. Decision 6's yes covers `run-interactive.ps1` re-registering `lazymkv-phase0` at each job.

1. Run: `scripts/box.sh cargo test -p lazymkv-platform --locked --no-run`
   Expected: `Finished` and an `Executable unittests src\lib.rs (C:\lazymkv\target\debug\deps\lazymkv_platform-<hash>.exe)` line. From here until the last job of this step ends, no `scripts/box.sh` call runs.
2. `BOX 'vol D:'` prints `The device is not ready.`: the drive is empty.
3. The drive job. Run: `scripts/phase0/job.sh platform-ignored.ps1 pr3-drives -Filter win::drives:: -Drive D:`
   Expected: `started platform-ignored.ps1; log: C:\lazymkv\phase0\logs\pr3-drives.txt`. The controller starts `scripts/phase0/wait.sh pr3-drives` in the background and, until it returns, reads the cues every 10 s with `BOX 'findstr /b CUE: C:\lazymkv\phase0\logs\pr3-drives.txt'`, posting each new line to the owner in chat at once, word for word (Ruling T3-25-a). Expected cues, in order: `CUE: insert a disc in D:`, then `CUE: remove the disc from D:` (a disc already present at start gives one `CUE: remove the disc from D:` first). If 60 s after `started` the log holds no `running` line (`BOX 'findstr /c:"running " C:\lazymkv\phase0\logs\pr3-drives.txt'` prints nothing), the log is not written as the job runs, so the controller tells the owner to act by the clock instead: with `D:` empty, insert the disc 30 s after `started`, wait at least 90 s more (the volume's readiness and the test's three 6-second quiet checks come between), then remove it; the test waits up to 5 minutes for each event and checks only their order, so the timetable meets it. The run is recorded with `timetable, not cues`. On each exit 3 of `wait.sh` the controller starts it again, until it exits 0 or 1.
   Expected log: the three `win::drives` tests `ok`, `an_inserted_and_removed_disc_is_reported_once_each` among them, and `PHASE0-DONE status=ok exit=0`.
4. The share job (Owner decision 9: writes only under `lazymkv-staging`). Run: `scripts/phase0/job.sh platform-ignored.ps1 pr3-shares -Filter win::shares::desktop:: -ShareRoot '\\unas-pro\media\lazymkv-staging\pr3-tests'`, then `scripts/phase0/wait.sh pr3-shares` until it exits 0 or 1.
   Expected: both `win::shares::desktop` tests `ok` (the session's own access reaches the share; free space read; `MoveFileExW` without replace refuses an existing file on the NAS).
5. The Credential Manager job, only after Owner decision 8's yes. Run: `scripts/phase0/job.sh platform-ignored.ps1 pr3-creds -Filter win::creds::`, then `scripts/phase0/wait.sh pr3-creds`.
   Expected: the four `win::creds::tests` tests `ok` and no `skipped:` line: the round trip and the case rule against the owner's own vault, each test entry removed by its drop guard.

Evidence, one line per job: `<date>, desktop session, win::drives: 3 passed; the owner inserted and removed a disc in D: on cue`; `<date>, desktop session, win::shares::desktop with \\unas-pro\media\lazymkv-staging\pr3-tests: 2 passed`; `<date>, desktop session, win::creds: 4 passed, no skip`; or the failing test's name and message, or `not run: <the decision's answer>`.

- [ ] **Step 5: The console run (Owner decision 2)**

The engine runs in the foreground of an SSH session with local folders and stand-in tools, and is never used with a disc (design part 3.1).

1. Preconditions; each command prints what follows it:
   - `BOX 'if exist C:\lazymkv\dev (dir /b C:\lazymkv\dev) else (echo absent)'` prints `absent`. Anything else goes to the owner with the listing, and decision 2's answer (remove it, or a fresh folder) is applied first.
   - `BOX 'tasklist | findstr /i makemkv'` prints nothing, since S55 stops every stand-in `makemkvcon` call while a MakeMKV process runs.
   - `BOX 'vol D:'` prints `The device is not ready.`, since the real `WinDrives` watches `D:`.
2. Run: `scripts/box.sh cargo build --workspace --locked --release`. Expected: `Finished`.
3. The folders, the program folders' lists (Ruling T3-25-i), then the copies (cmd's `mkdir` makes `library\Movies`'s parent too):
   `BOX 'mkdir C:\lazymkv\dev\data C:\lazymkv\dev\staging C:\lazymkv\dev\tuning C:\lazymkv\dev\library\Movies C:\lazymkv\dev\bin C:\lazymkv\dev\tools'`
   `BOX 'icacls C:\lazymkv\dev\bin /setowner *S-1-5-32-544 && icacls C:\lazymkv\dev\bin /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F *S-1-5-32-545:(OI)(CI)RX && icacls C:\lazymkv\dev\tools /setowner *S-1-5-32-544 && icacls C:\lazymkv\dev\tools /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F *S-1-5-32-545:(OI)(CI)RX'` (no `for` block, whose parentheses the `(OI)(CI)` flags would end). Expected: four `Successfully processed 1 files; Failed processing 0 files` lines.
   `BOX 'copy /y C:\lazymkv\target\release\lazymkv-engine.exe C:\lazymkv\dev\bin\ && copy /y C:\lazymkv\target\release\lazymkvctl.exe C:\lazymkv\dev\bin\ && xcopy /e /i /q /y C:\lazymkv\src\corpus\scenarios\movie-direct C:\lazymkv\dev\tools && for %t in (makemkvcon64 mkvmerge ffprobe ffmpeg java) do @copy /y C:\lazymkv\target\release\fake-tool.exe C:\lazymkv\dev\tools\%t.exe'`
   Expected: each copy reports `1 file(s) copied.`, and `xcopy` the scenario's file count.
4. `BOX 'powershell -NoProfile -Command "(Get-Acl C:\lazymkv\dev\data).Sddl.Substring(0,4)"'` prints `O:BA`. Otherwise Ruling T3-25-f's `icacls` call, and this check again.
5. The controller writes `.superpowers/pr3/console-settings.json`, exactly:

   ```json
   {
     "staging_root": "C:\\lazymkv\\dev\\staging",
     "tuning_root": "C:\\lazymkv\\dev\\tuning",
     "movies_root": "C:\\lazymkv\\dev\\library\\Movies",
     "tool_paths": {
       "makemkvcon": "C:\\lazymkv\\dev\\tools\\makemkvcon64.exe",
       "mkvmerge": "C:\\lazymkv\\dev\\tools\\mkvmerge.exe",
       "ffprobe": "C:\\lazymkv\\dev\\tools\\ffprobe.exe",
       "ffmpeg": "C:\\lazymkv\\dev\\tools\\ffmpeg.exe",
       "java": "C:\\lazymkv\\dev\\tools\\java.exe"
     }
   }
   ```

   and copies it: `scp -q -o BatchMode=yes .superpowers/pr3/console-settings.json "$LAZYMKV_BOX:C:/lazymkv/dev/data/settings.json"`.
6. Start, in the background, with its output kept: `ssh -o BatchMode=yes "$LAZYMKV_BOX" 'C:\lazymkv\dev\bin\lazymkv-engine.exe --data C:\lazymkv\dev\data' > .superpowers/pr3/console-engine.txt 2>&1`. Expected within 30 s: `console-engine.txt` holds `<Unix seconds> engine started (lazymkv 0.0.0)`.
7. From a second SSH session:
   - `BOX 'C:\lazymkv\dev\bin\lazymkvctl.exe status'`: exit 0 and the snapshot, with `jobs` empty and `problems` empty.
   - `BOX 'C:\lazymkv\dev\bin\lazymkvctl.exe selftest'`: exit 0; checks `makemkvcon` (version 2.0.0 from the stand-in's `MSG:1005`), `mkvmerge`, `ffprobe`, `ffmpeg`, `java`, `staging_root`, `tuning_root` and `movies_root` ok, and `tmdb_token` not ok with `no tmdb_token is set` (Owner decision 16).
   - `BOX 'powershell -NoProfile -Command "(Get-Acl C:\lazymkv\dev\data).Sddl"'`: owner `O:BA` and the list `D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;<user SID>)`, set by the engine at its start (design part 3.5).
   - `ssh -o BatchMode=yes "$LAZYMKV_BOX" 'C:\lazymkv\dev\bin\lazymkv-engine.exe --data C:\lazymkv\dev\data'; echo "exit=$?"`: `lazymkv-engine: an engine is already running (design part 3.3)` and `exit=1`.
   - `BOX 'C:\lazymkv\dev\bin\lazymkvctl.exe stop'`: exit 0. The background call then ends with status 0, `console-engine.txt` ends with `<Unix seconds> engine stopped`, and `BOX 'type C:\lazymkv\dev\data\engine.log'` holds the same lines as `console-engine.txt` (design part 3.5).
   - `BOX 'C:\lazymkv\dev\bin\lazymkvctl.exe status'`: exit 4 and `lazymkvctl: no engine is running`: the mutex and the pipe are gone with the engine (design part 3.8).

`C:\lazymkv\dev` stays for later console runs.
Evidence: `<date>, console run, C:\lazymkv\dev\bin and tools owned by Administrators with SYSTEM and Administrators full and Users read and execute; C:\lazymkv\dev\data owned by Administrators with D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;<user SID>); status exit 0; selftest exit 0, every check ok but tmdb_token (no tmdb_token is set); a second engine exit 1 with ENGINE_RUNNING; stop exit 0, engine exit 0, engine.log mirrored on standard error; status afterwards exit 4 with NOT_RUNNING`.

- [ ] **Step 5a: Ctrl+C at the desktop (Owner decision 18; optional)**

1. Preconditions: `BOX 'vol D:'` prints `The device is not ready.`; `BOX 'C:\lazymkv\dev\bin\lazymkvctl.exe status'` exits 4 with `lazymkvctl: no engine is running`.
2. The owner, at an elevated Command Prompt at the mini PC, runs `C:\lazymkv\dev\bin\lazymkv-engine.exe --data C:\lazymkv\dev\data`, inserts no disc, waits for the line ending `engine started (lazymkv <version>)`, presses Ctrl+C and runs `echo %ERRORLEVEL%`. Expected: a last line ending `engine stopped`, then `0`. The owner reports both in chat.

Evidence: `<date>, desktop session, console engine stopped by Ctrl+C: exit 0`, or `not run: Owner decision 18 <answer>`.

- [ ] **Step 6: The desktop task (Owner decisions 3, 4, 5 and 13)**

The owner is logged on at the mini PC's monitor. Decision 4's recommended answer shows the engine's console window on the desktop, and closing it is a forced shutdown.

1. Preconditions: `BOX 'tasklist | findstr /i makemkv'` prints nothing; `BOX 'vol D:'` prints `The device is not ready.`; `BOX 'C:\lazymkv\dev\bin\lazymkvctl.exe status'` exits 4 with `lazymkvctl: no engine is running`.
2. Run: `scripts/box.sh cargo build --workspace --locked --release`. Expected: `Finished`.
3. Run: `scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1'`, with ` -Data C:\lazymkv\dev\task-data` appended inside the quotes when decision 5 chose the override (a folder absent before, so the engine creates it owned by Administrators; never the console run's folder, whose settings name stand-ins).
   Expected: the last line `ENGINE-TASK-DONE status=ok`; or, as folders under `C:\` inherit write for authenticated users, a refusal naming `C:\lazymkv\src` or `C:\lazymkv\target` and `ENGINE-TASK-DONE status=error`, with nothing copied or registered. On that refusal the controller records it and asks the owner whether to accept the writable build folders on this machine; only on his yes it runs the same command with ` -AcceptWritableBuild` (Ruling T3-25-g).
4. Checks from SSH:
   - `BOX 'C:\lazymkv\run\lazymkvctl.exe status'`: exit 0 and the snapshot. This is the cross-session pipe: an elevated client in the SSH session reaches the elevated engine in the desktop session, and each end's check accepts the other (design parts 3.3, 3.4).
   - `BOX 'C:\lazymkv\run\lazymkvctl.exe selftest'`: exit 0. With no `settings.json`: `makemkvcon` found in its install folder `C:\Program Files (x86)\MakeMKV`, with the version MakeMKV printed, and ok unless MakeMKV printed a licence or key message (design part 3.7), which the evidence then quotes; `mkvmerge` ok from `C:\Program Files\MKVToolNix`; `ffprobe` and `ffmpeg` not ok, `ffprobe was not found` and `ffmpeg was not found`, since the WinGet `Links` folder is writable by the owner and skipped (design part 3.6, W18); `java` as the machine has it; `staging_root is unset`, `tuning_root is unset`, `movies_root is unset`; `no tmdb_token is set`.
   - `BOX 'powershell -NoProfile -Command "(Get-Acl C:\lazymkv\run).Sddl; (Get-Acl $env:ProgramData\lazymkv).Sddl"'` (with `-Data`, that folder in place of `$env:ProgramData\lazymkv`): each list protected (`D:P`), with exactly three entries inherited below (`OICI`): full access for `SY` and `BA` and read for `<user SID>`; the data folder's owner `O:BA` (design part 3.5).
   - `BOX 'powershell -NoProfile -Command "$t = Get-ScheduledTask -TaskName lazymkv-engine; $t.State; $t.Triggers.Count; $t.Principal.LogonType; $t.Principal.RunLevel; $t.Settings.ExecutionTimeLimit; $t.Settings.Priority"'`: `Running`, `0`, `Interactive`, `Highest`, then decision 4's values (recommended `PT0S` and `7`). The principal's account is not printed.
5. Manual on the mini PC, optional (the owner, at a PowerShell window at the desktop that is not elevated): `C:\lazymkv\target\release\lazymkvctl.exe status; $LASTEXITCODE` shows `lazymkvctl: run lazymkvctl elevated to reach the engine` and `4` (the pipe's high integrity label, design part 3.4); `C:\lazymkv\target\release\lazymkv-engine.exe --data C:\lazymkv\dev\data; $LASTEXITCODE` shows `lazymkv-engine: run lazymkv-engine elevated (design part 3.2)` and `1`, before the mutex (W3). The owner reports both in chat.

The engine keeps running for Step 7, or the controller stops it with `BOX 'C:\lazymkv\run\lazymkvctl.exe stop'` when Step 7 waits for its decisions.
Evidence: `<date>, desktop task, engine-task.ps1: <the first run's result>; ENGINE-TASK-DONE status=ok; task Running, 0 triggers, Interactive, Highest, <time limit>, priority <n>; C:\lazymkv\run and <data folder> protected (<their D: parts, the owner's SID as <user SID>>); status from SSH exit 0; selftest: <each check's name and ok, makemkvcon's version, how each tool was found>`, and, when the owner ran it, `<date>, desktop session, not elevated: lazymkvctl status exit 4 with NOT_ELEVATED; lazymkv-engine exit 1 with NOT_ELEVATED`.

- [ ] **Step 7: A real disc in the desktop task (Manual on the mini PC; Owner decisions 9, 10 and 14)**

1. The owner writes his settings file on the mini PC as `%USERPROFILE%\lazymkv-pr3-settings.json`, outside the repository: `staging_root` `\\unas-pro\media\lazymkv-staging\engine`, `tuning_root` `\\unas-pro\media\lazymkv-tuning\engine`, `movies_root` `\\unas-pro\media\lazymkv-test-library\Movies` (design part 6.2), and `tool_paths.ffprobe` and `tool_paths.ffmpeg` naming his WinGet copies (design part 3.6). The file is never copied or recorded.
2. With the engine of Step 6 running (else Step 6's command 3 again, with any switch the owner approved there): `BOX 'C:\lazymkv\run\lazymkvctl.exe settings set %USERPROFILE%\lazymkv-pr3-settings.json'`. Expected: exit 0.
3. `BOX 'C:\lazymkv\run\lazymkvctl.exe selftest'`: exit 0; `staging_root`, `tuning_root` and `movies_root` ok (the engine created the two `engine` folders, W15; the library probe ran inside `.lazymkv-partial`), `ffprobe` and `ffmpeg` ok from settings.
4. The owner inserts one movie disc that is neither acceptance disc (Ruling T3-25-c). The controller reads `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` every 30 s and follows the job in `jobs`.
5. Once the job's state is `Ripping`:
   - `BOX 'C:\lazymkv\run\lazymkvctl.exe plan --job <id>'`: exit 0 and the plan document, printed by the relay's child `C:\lazymkv\run\lazymkvctl.exe` (S35, W24, W25).
   - `BOX 'C:\lazymkv\run\lazymkvctl.exe stop --force'`: exit 0. The engine cancels the rip as `cancel` does, persists the job and exits (design part 3.8).
6. Afterwards:
   - `BOX 'tasklist | findstr /i makemkv'` prints nothing: the Job Object ended `makemkvcon64` and anything it started (design part 4.2).
   - `BOX 'powershell -NoProfile -Command "(Get-ScheduledTask -TaskName lazymkv-engine).State"'` prints `Ready`.
   - `BOX 'type "%ProgramData%\lazymkv\jobs\<id>\transitions.jsonl"'` (or the `-Data` folder): the `new` states `Detected`, `Fingerprinted`, `Scanned`, `Planned`, `Ripping`, then the cancel's.
   - `BOX 'findstr /c:"share access for" "%ProgramData%\lazymkv\jobs\<id>\job.log"'` prints `<Unix seconds> share access for \\unas-pro\media\lazymkv-staging\engine: the session's access` (Task 17).
   - The owner removes the disc. The job folder and `\\unas-pro\media\lazymkv-staging\engine\<id>` stay: nothing in pull request 3 removes them, so the controller gives the owner the job ID in chat, and he removes `\\unas-pro\media\lazymkv-staging\engine\<id>` and the job folder when he wants them gone.

The scan ran on the drive the real `DriveWatcher` reported, so `drive-list.txt`'s line for that drive carried `D:` as its seventh field (engine.md:575); the evidence says so without quoting the line.
Evidence (the job ID written literally as `<job id>`, Rules above): `<date>, desktop task, a movie disc that is not an acceptance disc: settings set exit 0; selftest roots ok; job <job id>: Detected, Fingerprinted, Scanned, Planned, Ripping, <the cancel's state>; job.log: share access for \\unas-pro\media\lazymkv-staging\engine: the session's access; plan --job exit 0 through the relay; stop --force exit 0; no MakeMKV process left; task Ready`.

- [ ] **Step 8: The arranged share (Manual on the mini PC; only after Owner decision 7's yes)**

1. With the desktop task's engine running (else Step 6's command 3 again, with any switch the owner approved there), the owner arranges a share his session cannot reach and, at an elevated terminal at the desktop, runs `C:\lazymkv\run\lazymkvctl.exe secret set share:\\<server>\<share>`, typing the user name, Enter, the password, Enter, then Ctrl+Z and Enter. The console shows what he types; nothing is recorded, and no phase 2 command removes the entry (`lazymkv:share:\\<server>\<share>` in Credential Manager). Expected: exit 0.
2. Run: `scripts/phase0/job.sh platform-ignored.ps1 pr3-arranged -Filter win::shares::arranged:: -ShareRoot '<the arranged share's folder>'` (no space or quote in it), then `scripts/phase0/wait.sh pr3-arranged`.
   Expected: `the_stored_credential_reaches_a_share_the_session_cannot ... ok`.
3. `BOX 'C:\lazymkv\run\lazymkvctl.exe stop'`: exit 0.
4. The run leaves the entry `lazymkv:share:\\<server>\<share>` in Credential Manager and a connection to that share until logoff. The owner removes them, if he wants them gone, at the desktop with `cmdkey /delete:lazymkv:share:\\<server>\<share>` and `net use \\<server>\<share> /delete`.

Evidence: `<date>, desktop session, the_stored_credential_reaches_a_share_the_session_cannot with \\SERVER\SHARE\<rest>: ok` (the folder as the Rules above write it), or `not run: Owner decision 7 <answer>`.

**Run:** as each step says. **Commit:** Step E's.

---

### Task 26: Documents, notes and the test count

**Files:**
- Modify: `docs/phase2/engine.md`: `:1-9` (the opening paragraph), a new `## Pull request 3: the Windows pieces` after `:29`, `:31-37` (How to run it), `:43-47` and `:49` (the crates table's rows), `:57-83` (one paragraph at the end), `:85-91` (one paragraph at the end), `:93-99` (the Rulings introduction) and four new subsections after `:552`, `:568-631` (Left for later pull requests), a new `## Pull request 3 on the mini PC` before `:633`, and `:633-683` (Tests).
- Modify: `README.md:26-28` (the phase 2 sentence), `:45-49` (the `crates/` bullet), `:59-62` (the `scripts/` bullet).
- Modify: `.github/workflows/ci.yml:3-8` (the comment).

Line anchors are at `dcd5924`.

**Interfaces:**
- Consumes:
  - Task 1's `.superpowers/pr3/pr3-lock-delta.txt` and the date of the owner's yes;
  - the plan's rulings W1 to W28, each task's `Ruling T3-<n>-<letter>`, and any ruling the controller made during execution;
  - the owner's answers to Owner decisions 1 to 18, each with its date, from `.superpowers/pr3/decisions.txt`;
  - Task 25's `.superpowers/pr3/pr3-mini-pc-runs.txt`;
  - the plan's "Not covered" list;
  - pull request 2's count, 2573 tests (`docs/phase2/engine.md:635-651`);
  - the open pull request's number and its latest CI run on the commit before this task's.
- Produces: no code interface.

- [ ] **Step 1: Check the inputs**

The pull request is open as a draft since Task 2 Step 6. If the owner declined it there, the controller asks his yes to open it now, as pull request #6 was opened on 2026-10-25, pushes, and waits for its CI. Then, with `<n>` its number:
- `gh run list --repo krcm0209/lazymkv --branch phase-2-windows-pieces --workflow CI --limit 1 --json databaseId,headSha,conclusion,url`: `conclusion` `success` and `headSha` equal to `git rev-parse HEAD`. Another result waits; a failure goes back to the task that owns the failing test.
- `gh api "repos/krcm0209/lazymkv/code-scanning/alerts?ref=refs/pull/<n>/merge&state=open" --jq length` prints `0`, and `gh pr checks <n> --repo krcm0209/lazymkv` lists every check passing, CodeQL's among them.
- `pr3-mini-pc-runs.txt` holds a line for each of Task 25's Steps 1 to 8 and 5a, a step not run naming its decision's answer.

`$SCRATCH` below is the session's scratchpad directory.

- [ ] **Step 2: Run the full suite with the evidence clone**

Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked 2>&1" > "$SCRATCH/lazymkv-test-run.txt"`, then `tr -d '\r' < "$SCRATCH/lazymkv-test-run.txt" | grep -E '^test result:' | awk '{p += $4; i += $8} END {print p, "passed", i, "ignored"}'` and `tr -d '\r' < "$SCRATCH/lazymkv-test-run.txt" | grep '^skipped:'`.

Expected: every `test result:` line `ok` with `0 failed`; ignored 7, all in `lazymkv-platform`: `process::tests::helper_child`, `win::eject::tests::the_tray_opens_on_eject`, `an_inserted_and_removed_disc_is_reported_once_each`, the two `win::shares::desktop` tests, the `win::shares::over_ssh` test and the `win::shares::arranged` test. The `skipped:` lines include `skipped: LAZYMKV_TEST_VAULT is not set; the vault tests run on CI and, after Owner decision 8, in the desktop session (design part 6.1)` twice (`win::creds`, which ran in the desktop session instead, Task 25 Step 4) and no clone skip, since the clone is present; every `skipped:` line is recorded with its reason. `LAZYMKV_OCR_TEXT` is not set.

- [ ] **Step 3: Count the tests**

Run: `scripts/box.sh "cargo test --workspace --locked -- --list 2>&1" > "$SCRATCH/lazymkv-test-list.txt"`, then `tr -d '\r' < "$SCRATCH/lazymkv-test-list.txt" | grep -c ': test$'`, then `tr -d '\r' < "$SCRATCH/lazymkv-test-list.txt" | awk '/Running |Doc-tests /{h = $0} /: test$/{n[h]++} END {for (k in n) print n[k], k}'`, and `git rev-parse --short HEAD`.

Expected: one integer N, above 2573. The second command gives a count per test binary: a `Running unittests src\lib.rs (...\deps\<crate>-<hash>.exe)` header names its crate, and a `Running tests\<file>.rs` header belongs to the package whose `tests/` folder holds the file. Summed per package they give the nine counts, which add up to N.

- [ ] **Step 4: Read CI's facts**

With `<id>` the run of Step 1:
- `gh run view <id> --repo krcm0209/lazymkv --log | grep 'token facts:'`: one line, from `test (windows-latest)`, holding `windows-latest`'s token facts (Task 2's test). `elevated=true` means Owner decision 15's fallback was not needed; `elevated=false` names the fallback the run took.
- `gh run view <id> --repo krcm0209/lazymkv --log | grep 'test result:' | sed -E 's/^test \(([a-z]+)-latest\).*test result: [A-Za-z]+\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored.*/\1 \2 \3 \4/' | awk '{p[$1] += $2; f[$1] += $3; i[$1] += $4} END {for (k in p) print k, p[k], "passed", f[k], "failed", i[k], "ignored"}'`: per system, 0 failed. Windows passes more than ubuntu and macos by the `cfg(windows)` tests; Windows ignores 7, and ubuntu and macos 1 each (`helper_child`).

- [ ] **Step 5: Write `docs/phase2/engine.md`**

Plain sentences, no em-dashes. Expand at first use in the file each acronym new to it: SSH (Secure Shell), SID (security identifier), UNC (Universal Naming Convention), COM (Component Object Model). Name this plan's tasks `T3-<n>` wherever pull request 2's `Task <n>` could be meant. Write no secret, account name, user SID, drive serial or `DRV` line; a path under the owner's profile is `C:\Users\USER\...`. The changes, in file order:

1. **Opening paragraph (`:1-9`).** After "They cite the phase 2 design as "design part" and the design spec as "spec".", add: "Pull request 3, on the branch `phase-2-windows-pieces` from `main` at `dcd5924`, adds its own sections, each named for it; its plan is `docs/superpowers/plans/2026-10-27-phase-2-pr3-windows-pieces.md`, and it is pull request krcm0209/lazymkv#<n>." Replace "This first part records pull request 2" with "The first part records pull request 2".
2. **`## Pull request 3: the Windows pieces`**, after `:29`. "Pull request 3 delivers design part 12.6 item 3:" and one bullet each:
   - the Windows implementations of every `platform` trait but `Autostart`, in `platform::win` on `windows-sys`: `WinDrives`, `WinEjector`, `JobProcessGuard`, `WinKeepAwake`, the share connector, `CredentialStore`, `PipeServer`, `PipeConnector` and `WinHost`, with `write_atomic` and `rename_no_replace` on `MoveFileExW` (S13);
   - the named pipe `\\.\pipe\lazymkv` and its access list (P6), with `lazymkvctl`'s real pipe client;
   - the mutex `Global\lazymkv-engine` and the protected data folder (design parts 3.3, 3.5);
   - the engine binary on Windows (`engine::native`): elevation, mutex, data folder and first pipe instance in that order (W3), the console mirror of `engine.log`, and the console's stop events as a forced shutdown;
   - the desktop task script `scripts/engine-task.ps1` (P5) and the run folder `C:\lazymkv\run`;
   - the share connection (P7): the session's access first, lazymkv's stored credential second;
   - the `command` relay (S35, design part 9.5);
   - every item of pull request 2's list for pull request 3 (Left for later pull requests, below);
   - the platform tests on `windows-latest` and the mini PC (design part 12.4), with `scripts/phase0/platform-ignored.ps1` for the desktop session.

   The acceptance sentence: "The whole job of pull request 2 stays green on ubuntu, macos and windows, and on `windows-latest` and the mini PC a stand-in disc goes from arrival to a filed file through the real start-up sequence, the real pipe and `lazymkvctl`'s real client, the Job Object runner and the Windows file moves (W27; `crates/fake-tools/tests/engine_native.rs`; CI run <the url of Step 1>)."
3. **How to run it (`:37`).** The third bullet becomes these five:
   - "On Windows, `lazymkv-engine.exe [--data DIR]` runs the engine elevated behind the mutex `Global\lazymkv-engine` and the pipe `\\.\pipe\lazymkv`, its data folder `%ProgramData%\lazymkv` unless `--data` moves it (design parts 3.1 to 3.5). Outside Windows it still exits 1 with "lazymkv-engine runs only on Windows in this version" (S28)."
   - "On Windows, `lazymkvctl`'s engine commands reach that engine through the pipe from any elevated process of the same account, an SSH session included; a process that is not elevated exits 4 with "lazymkvctl: run lazymkvctl elevated to reach the engine". On Linux and macOS they still exit 4 with "lazymkvctl: no engine transport exists in this build" (S38)."
   - "A console run on the mini PC copies `lazymkv-engine.exe` and `lazymkvctl.exe` out of `C:\lazymkv\target\release`, gives `--data C:\lazymkv\dev\data` with a `settings.json` naming local roots and stand-in tools, starts the engine over SSH and stops it with `lazymkvctl stop` from a second SSH session; its `engine.log` lines also go to standard error (design parts 3.1, 3.5). It is never used with a disc."
   - "The desktop task: `scripts/box.sh cargo build --workspace --locked --release`, then `scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1'` copies the programs into `C:\lazymkv\run`, registers and starts the task `lazymkv-engine`, and waits for `lazymkvctl status` (P5, design part 3.1). The owner must be logged on at the mini PC's monitor."
   - "The platform's hardware and desktop-session tests carry `#[ignore]`. Over SSH: `scripts/box.sh 'set LAZYMKV_TEST_DRIVE=D:&& cargo test -p lazymkv-platform --locked -- --ignored --test-threads=1 win::eject::'`. In the desktop session: `scripts/phase0/job.sh platform-ignored.ps1 <log name> -Filter <module path> [-Drive D:] [-ShareRoot <UNC folder>]`, then `scripts/phase0/wait.sh <log name>` (design part 12.4)."
4. **The crates table (`:43-47`, `:49`).** In place:
   - `lazymkv-platform`: "Depends on" gains "; on Windows, `windows-sys`"; in "Holds", "`write_atomic` and `rename_no_replace` (std in this version, S13)" becomes "`native_guard`, `write_atomic` and `rename_no_replace` (std on Linux and macOS, `MoveFileExW` on Windows, S13), the rules the Windows code applies, compiled everywhere (`security`, `localtime`, `shares`, `drives`, `eject`), the Windows implementations in `win` (design part 4.2)".
   - `lazymkv-tools`: "; since pull request 3, `find_tool` keeps only absolute `PATH` folders a non-elevated process cannot write (W18), and `list_drives` takes a time limit (W20)".
   - `lazymkv-clients`: "; since pull request 3, `OfflineTransport`, which the Windows binary passes until pull request 4 (W26)".
   - `lazymkv-engine`: "; since pull request 3, the Windows start-up `native` (design parts 3.1 to 3.8), the `command` relay `relay` (S35) and `NoDelivery`".
   - `lazymkv-ctl`: "; on Windows over `platform::win::PipeConnector` (Linux and macOS keep no transport, S38)".
   - `lazymkv-dev`: "; since pull request 3, `forms::read_forms`, moved from `lazymkv-ctl` so that the engine's relay reads words as `lazymkvctl` does (W24)".
5. **The interfaces fixed in core (end of `:57-83`).** One paragraph: "Pull request 3 adds `core::ipc::share_credential` and `SHARE_VALUE_SHAPE`, the two-line shape of a share credential's value (W12), and `core::record::TitleRecord::rip_started_ms`, the last rip's start in Unix milliseconds, left out when absent so that earlier `state.json` files read and write back unchanged (W16, design part 7.4). `rip-summary.json` gains `started`, local time as `yyyy-MM-dd HH:mm:ss.fff`, between `bytes` and `elapsed_s` (R23.7)."
6. **The Cargo.lock delta (end of `:85-91`).** "On <date of the yes> the owner approved the `Cargo.lock` delta of pull request 3, read from crates.io on <date of Task 1's read>: `windows-sys` <version> and `windows-link` <version>, both MIT OR Apache-2.0. The lock's package count went from 43 to 45. Only `lazymkv-platform` depends on `windows-sys`, on Windows only, pinned `=<version>` (design parts 4.3, 4.5, 4.7). `ureq` with its TLS stack waits for pull request 4." With the values of `pr3-lock-delta.txt`.
7. **The Rulings introduction (`:93-99`).** Add: "Pull request 3's plan recorded the planner's rulings W1 to W28 and, in its tasks, rulings numbered T3-<task>-<letter>. They follow pull request 2's, with the owner's decisions of pull request 3 and its rulings made during execution."
8. **Four subsections after `:552`**, before `## Rulings on the open points`:
   - `### Pull request 3: the planner's rulings (W1 to W28)`: one line each, `- W<n> (<title>). <the decision, in at most two sentences>`, citing the design part or ruling it applies. W3, W8, W10, W17, W20, W21, W22, W24 and W25 end with "(amends <what it amends>; Owner decision 11: <the owner's answer and its date>)"; W24's amends S76: `command`'s refusal is retired, and the relay's are `COMMAND_BUSY`, `COMMAND_NO_CTL`, `COMMAND_WRITABLE`, `COMMAND_FORMS`, `RETUNE_WAITS`, `no_title`, `not_started` and `STOPPING`. Ruling T3-24-a's line under its task ends the same way.
   - `### Pull request 3: the task-level rulings`: under a bold heading per task, `**Task T3-<n>** (<the task's title>)`, one line per `Ruling T3-<n>-<letter>`.
   - `### Pull request 3: the owner's decisions`: one line per Owner decision 1 to 18, `- <n> (<subject>): <the answer>, <date>`.
   - `### Pull request 3: rulings made during execution`: one line each under its task's bold heading, or the one sentence "None." The subsection ends with "The owner may overturn any of them."
9. **Left for later pull requests (`:568-631`).**
   - The "Pull request 3" list (`:572-594`) becomes "**Pull request 3** (design part 12.6, item 3): done. Each item and where it closed:" with one line per bullet, in its order: (1) T3-2 to T3-7, T3-11 to T3-16; (2) T3-8, T3-24, T3-22 (W24); (3) T3-16 (W8); (4) T3-18 (W28); (5) T3-7, T3-9 (W16); (6) T3-4; (7) T3-5 (W7); (8) T3-8 (W18); (9) the waker and `shutdown` T3-10, the start check T3-19 (W20), the accept error T3-10 (W10); (10) T3-16, whose W8 also closes the bullet's second half; (11) T3-8 (W18); (12), (13) T3-17; (14) T3-19; (15) T3-18; (16) T3-14 (`no_text_holds_the_sentinel_user_or_password`); (17) T3-17 (W21); (18) T3-17; (19) T3-18 (W22); (20), (21) T3-20 (W23); (22) T3-19. An item a task re-deferred during execution moves to its later list by name instead.
   - "Pull request 4" gains: "notification delivery (`NoDelivery` logs each notification undelivered until then, W26) and entering the TMDb token with `lazymkvctl secret set tmdb_token` (design part 15 item 8; Owner decision 16)".
   - "Pull request 5" gains: "the relay's `--text <tuning_root>\dataset` and `retune --jobs`, refused with `RETUNE_WAITS` until then (W25)"; "`lazymkv-ocr.exe` and its below-normal run (`engine-task.ps1` copies it once it exists)"; "the release layout's `resources` folder in use".
   - New groups after "Pull request 6": "**Phase 3** (from pull request 3):" a share lost during File, Remux or Decide sends the job to AwaitingShare instead of failing with class `Unknown` (V7 option b); the `path_collision` review that takes over L7; a `secret unset` command; listing stored share names (S77 stays); `WNetCancelConnection2W` for lazymkv's deviceless connections, which live until logoff; a time bound on a share call (pull request 3 only moves the calls off the owner thread). "**Phase 5:** `Autostart` and the logon trigger, through Task Scheduler's COM interface (design part 3.9)." "**Not planned:** spec 4.4's registry-entry form of the default install lookup; spec 4.4 allows the folder or the entry, and W18 reads the folders." "**Not measured in pull request 3:**" whether a window in an SSH session receives drive notifications (design part 3.1); P7's fallback success branch, unless Task 25 Step 8 ran; Ctrl+C at a real console, unless Task 25 Step 5a ran (the native test calls the handler's closure); an engine and `lazymkvctl` that are not elevated at a desktop terminal, unless the owner's check of Task 25 Step 6 ran. A line whose measurement `pr3-mini-pc-runs.txt` records is left out.
   - In "Accepted in pull request 2 and left as they are", the KeepAwake line ends "; confirmed in pull request 3 by W9, from spec 4.3's "while any job is active"" in place of "design part 4.2 is to confirm it".
10. **`## Pull request 3 on the mini PC`**, before `## Tests`. One sentence: "These runs were on the mini PC, each in the session it names; no secret, account name, user SID or drive serial is written, and a run that printed `skipped:` counts as not run." Then a bullet with `windows-latest`'s facts line (Step 4) beside the SSH session's (Task 25 Step 1) and Owner decision 15's outcome, then one bullet per line of `pr3-mini-pc-runs.txt`, in its order. Then one bullet, "What pull request 3 leaves on the mini PC and the NAS, each removed only by the owner:", with one line each: the task `lazymkv-engine` (`Unregister-ScheduledTask -TaskName lazymkv-engine -Confirm:$false`); `C:\lazymkv\run` and `C:\lazymkv\dev` (`rmdir /s /q <folder>`); the desktop task's data folder, `%ProgramData%\lazymkv` or Owner decision 5's (`rmdir /s /q`); `\\unas-pro\media\lazymkv-staging\pr3-tests`; the real disc's job folder and its staging folder under `\\unas-pro\media\lazymkv-staging\engine` (Task 25 Step 7); the two `engine` folders under `lazymkv-staging` and `lazymkv-tuning`; and, after Task 25 Step 8, the Credential Manager entry and the connection (`cmdkey /delete:...`, `net use ... /delete`).
11. **Tests (`:633-683`).**
   - A new first paragraph: "`cargo test --workspace --locked -- --list` on the mini PC (Windows), on the tree of commit `<sha from Step 3>`, lists N tests: <one count per package, as in pull request 2's sentence>. The baseline is pull request 2's 2573, below, so pull request 3 added <N - 2573, written as a number>." Then: "The list is the mini PC's: the `cfg(windows)` tests of pull request 3 are in it, and on CI run <id> windows passed <p> and ignored 7, ubuntu passed <p> and macos <p>, each ignoring 1. The junction run of that tree passed <p> and ignored 7 (each named), and its RP21 guard read the clone and passed over `corpus/scenarios/`, `command-relay` included." Then the `skipped:` lines of Step 2, each with what ran instead. Pull request 2's paragraph follows, its first words "`cargo test --workspace --locked -- --list` on the mini PC" preceded by "For pull request 2,".
   - New rows in the table:
     - `crates/fake-tools/tests/engine_command.rs` | "The `command` relay on a stand-in `lazymkvctl` (S35, design part 9.5): the child's words and replay flags, its lines as `command_output` to the requesting client only, its exit code, one command at a time, the refusals (unknown job or title, other words, `retune --jobs`, no `lazymkvctl`, a writable folder), below-normal priority, and a client that disconnects (T3-22)" |
     - `crates/fake-tools/tests/engine_native.rs` | "Windows only: the native whole job through the real start-up sequence, pipe and client (W27), the start-up refusals (a second engine, a data folder the user owns, a held pipe name, a token that is not elevated, before the mutex), the console mirror, and a forced shutdown from the console handler (T3-23)" |
     - `crates/fake-tools/tests/scripts.rs` | "Windows only: `engine-task.ps1` and `platform-ignored.ps1` parse in Windows PowerShell 5.1 and name their task, folders and command (T3-24, T3-25)" |
     - `crates/platform/src/win/` (unit tests, Windows only) | "Token facts, the mutex, the data folder, the file moves, the Job Object runner, `KeepAwake`, `WinHost`, the pipe server and client, Credential Manager, the share connector, the `Ejector` and the `DriveWatcher` (design part 12.4); the `#[ignore]` ones ran as "Pull request 3 on the mini PC" records" |
     - `crates/platform/src/{security,localtime,shares,drives,eject}.rs` | "All systems: the descriptor strings and the token verdict, the local-time text, the share and volume rules with the credential fallback through a scripted `ShareApi`, the drive diff, and the eject report" |
   - The `binary.rs` row becomes: "Outside Windows the engine binary exits 1 with S28's sentence and creates nothing; on every system an argument error exits 2 under `usage: lazymkv-engine [--data DIR]`; no binary test starts an engine (T3-23)".
   - Each existing `crates/fake-tools/tests/` row whose file gained tests in pull request 3 (`engine_ipc.rs`, `engine_disc_phase.rs`, `engine_file_phase.rs`, `engine_restart.rs`, `makemkv_driver.rs`) gains "; since pull request 3, <the topics of the tests each task added to that file, with the task>", as review round 1's clauses are written.
   - The closing paragraph gains: "`lazymkv-platform`'s tests scale a wait that only waits by their own copy of `ci_factor`, and a bound that proves a call does not block (K12, the share stalls) does not scale."

- [ ] **Step 6: Edit `README.md` and the CI comment**

At `README.md:26-28`, replace:
```
with a developer mode that keeps every rip's evidence for tuning. Its second pull request runs a whole job on stand-in
tools on any OS, from a disc's arrival to a filed placeholder file, with no disc, no network and no Windows API;
[`docs/phase2/engine.md`](docs/phase2/engine.md) says how to run it and how many tests there are.
```
with:
```
with a developer mode that keeps every rip's evidence for tuning. Its second pull request runs a whole job on stand-in
tools on any OS, from a disc's arrival to a filed placeholder file, with no disc and no network. Its third adds the
Windows pieces: the engine binary behind its mutex and protected named pipe, `lazymkvctl`'s pipe client, the share
connection, Credential Manager secrets and the desktop task script, so on Windows the same job runs through the real
pipe and the Job Object runner; [`docs/phase2/engine.md`](docs/phase2/engine.md) says how to run them and how many
tests there are.
```

In the `crates/` bullet (`README.md:45-49`), make three replacements:
- `(the operating-system traits, with in-memory fakes)`, across lines 45 and 46, becomes `(the operating-system traits, their Windows implementations and in-memory fakes)`;
- `` `engine` (the `lazymkv-engine` job owner, its persistence and its IPC server) `` becomes `` `engine` (the `lazymkv-engine` job owner, its persistence, its IPC server, its Windows start-up and the `command` relay) ``;
- `` the engine commands) `` becomes `` the engine commands, over the named pipe on Windows) ``.

At `README.md:59-60`, replace `` - `scripts/phase0/` - the PowerShell jobs that run on the ripping machine, and the Mac's shell helpers that sync, `` and the next line's `start and wait for them and pull their recordings.` with `` - `scripts/` - `box.sh`, which mirrors the working tree to the mini PC and runs one command there; `engine-task.ps1`, which copies the engine into `C:\lazymkv\run` and starts it as the desktop task `lazymkv-engine`; and `phase0/`, the PowerShell jobs that run on the ripping machine (`platform-ignored.ps1` among them, the desktop-session run of the platform's ignored tests) and the Mac's shell helpers that sync, start and wait for them and pull their recordings. ``

Rewrap each edited paragraph at 120 columns, as the file is wrapped.

At `.github/workflows/ci.yml:3-8`, replace the six comment lines with:
```
# Every push to main and every pull request builds and tests the workspace on Linux, macOS and Windows with the
# toolchain rust-toolchain.toml pins. The private evidence repository is not cloned here, so the tests that need it
# print their "skipped: ..." reason and the rest run; docs/phase2/engine.md has the count. Among them, the stand-in
# tools of crates/fake-tools run a whole engine job, from a disc's arrival to a filed placeholder file, on every
# system (design part 12.6, item 2), and on Windows the platform's Windows implementations run too: the named pipe,
# the mutex, the Job Object runner, Credential Manager under a test name, and a native whole job through the real
# pipe (design parts 12.4 and 12.6, item 3). Tests that need the drive, the share or the owner's desktop session are
# ignored here and run on the mini PC. ffmpeg is installed for the one test that builds its own proxy with lavfi;
# without it that test skips too.
```
The CI commands stay unchanged.

- [ ] **Step 7: Check the documents**

Run: `grep -n $'\xe2\x80\x94' docs/phase2/engine.md README.md .github/workflows/ci.yml; grep -c '^- W[0-9]' docs/phase2/engine.md; grep -nE 'S-1-5-21-|DRV:|eyJhbGciOi' docs/phase2/engine.md README.md scripts/engine-task.ps1 scripts/phase0/platform-ignored.ps1; grep -n 'C:\\Users\\' docs/phase2/engine.md README.md | grep -v 'C:\\Users\\USER\\'; scripts/phase0/check-names.sh`

The first grep searches for the UTF-8 bytes of U+2014, the em-dash, written as escapes so that this plan holds none.
Expected: the first grep prints nothing; the count is 28; the third and fourth print nothing (no user SID, no `DRV` line, no token, no path under a profile but `C:\Users\USER\`); `check-names.sh` prints nothing and exits 0.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS (this task changes no code; the run is the Global Constraints' check before a commit).

- [ ] **Step 8: Commit**

```bash
git add docs/phase2/engine.md README.md .github/workflows/ci.yml
git commit -m "docs: pull request 3's notes, rulings and test count"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```
Expected: `G` with `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, and the message only, with no attribution line.

- [ ] **Step 9: Push and confirm CI**

Run: `git push`, then `gh run watch` on the branch's new CI run.
Expected: `test (ubuntu-latest)`, `test (macos-latest)`, `test (windows-latest)` and `repository guards` succeed, and the pull request's checks show no CodeQL alert. A failure is fixed in a further commit of this task, with the same signing check. Marking the pull request ready for review (`gh pr ready <n>`) waits for the owner's yes in chat.

**Run:** `scripts/box.sh cargo test --workspace --locked`

**Commit:** `docs: pull request 3's notes, rulings and test count`

## Not covered (and why)

- **PR 4** (design part 12.6 item 4; engine.md "Pull request 4"): `ureq` and the live lookups, so in PR 3 a real disc's job plans with its lookups pending, rips, and waits in AwaitingLookup after its eject; notification delivery and the webhook self-test check (`NoDelivery` until then); entering the TMDb token (`lazymkvctl secret set tmdb_token`, part 15 item 8), which needs the live transport to matter; the stale-copy refetch before the relay's words and K15's exit 3; the cleanup task.
- **PR 5:** the relay's `--text <tuning_root>\dataset` and `retune --jobs` (refused with `RETUNE_WAITS`), `retune`'s repeated `--evidence`, `lazymkv-ocr.exe` and its below-normal run (the task script copies it once it exists), the release layout's `resources` folder in use, layer 2 (real tools, MKVToolNix on CI).
- **PR 6:** acceptance (part 12.7), with `compare-job`.
- **Phase 3:** a share lost during File, Remux or Decide sending the job to AwaitingShare instead of failing with class Unknown (V7 option b); the `path_collision` review that takes over L7; a `secret unset` command; listing stored share names (S77 stays); `WNetCancelConnection2W` for lazymkv's deviceless connections, which live until logoff; a time bound on a share call (Task 17 only moves the calls off the owner thread).
- **Phase 5:** `Autostart` and the logon trigger, Task Scheduler over COM.
- **Not planned:** spec 4.4's registry-entry form of the default install lookup; spec 4.4 allows the folder or the entry, and W18 reads the folders.
- **Not measured in PR 3:** whether a window in an SSH session receives drive notifications (design part 3.1); console runs never use a disc; P7's fallback success branch, unless the owner arranges it (Owner decision 7); Ctrl+C at a real console, unless Task 25 Step 5a ran (Task 23 calls the handler's closure); a non-elevated engine and `lazymkvctl` at a desktop terminal, unless Task 25 Step 6's optional check ran.
