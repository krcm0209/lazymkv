# Phase 2 Pull Request 7: Acceptance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove phase 2 on two real discs with the release zip's programs, and compare what the engine decided with what `lazymkvctl` says about each job afterwards.

**Architecture:** `lazymkv-dev compare-job` is a pure comparer of saved JSON files that runs on the Mac. `scripts/engine-task.ps1` learns to install the release zip's tree behind the desktop task. The run itself is a rehearsal on the jobs the mini PC already holds, then one sitting with the owner; its results go into `docs/phase2/engine.md`.

**Tech Stack:** Rust (workspace toolchain, `serde_json` with key order kept), Windows PowerShell 5.1, bash and ssh from the Mac, GitHub Actions (`ci.yml`, `release.yml`).

**Spec:** `docs/superpowers/specs/2026-11-23-phase-2-pr7-design-notes.md` (the notes; "call N" below is its section 6). Binding above it: `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` parts 6.3, 9.5, 12.6 and 12.7.

## Global Constraints

- Rust is built and tested on the Mac and by CI; nothing is built on the mini PC. The Mac's command prefix, written `CARGO` below:
  `env RUSTUP_AUTO_INSTALL=0 PATH="$HOME/.cargo/bin:$PATH" LAZYMKV_OCRS_MODELS="$HOME/.cargo/lazymkv-ocrs-models" cargo`
  The whole run is `CARGO test --workspace --exclude lazymkv-ocr --locked --offline`.
- Windows-only tests (the PowerShell ones in `crates/fake-tools/tests/scripts.rs`) run only in CI's Windows job. A task that adds one is verified by the CI run of its commit.
- Commits are signed, as `krcm0209 <17315774+krcm0209@users.noreply.github.com>`, with no attribution line. The controller makes every commit; an implementer never commits. After each commit: `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G`, that identity and no agent line.
- One commit per task, pushed to the draft pull request; CI green on the three systems and CodeQL at 0 before the next task starts.
- No secret, title of the owner's disc, job ID, host, user name or drive serial in the repository, the ledger or the notes. The mini PC's ssh target is read from the git-ignored `scripts/phase0/box.env` (`LAZYMKV_BOX`), written `$BOX` below. A job ID lives in a shell variable and in folder names under `.superpowers/pr7/jobs/` only.
- No subtitle text in any file. Job recordings are committed nowhere (P22).
- `compare-job`'s rules do not change once the rehearsal (Task 6) has started. A difference is ruled on by the owner.
- Nothing that changes the mini PC's standing state (the task's registration, the settings, a removal) runs before the owner's yes for that step.
- Rulings made while executing are labelled T7-E1, T7-E2 and so on in the ledger `.superpowers/sdd/2026-11-23-phase-2-pr7-acceptance/progress.md` and carried into the notes by Task 8. This plan's own rulings are T7-R1 to T7-R4.

## Owner prerequisites and decisions

| # | What | When |
|---|---|---|
| 1 | The pick among up to three candidate second discs | end of Task 5 |
| 2 | Yes to: stopping the engine, registering the task on the zip's tree, removing the two FFmpeg tool paths from the settings. The owner is logged on at the mini PC's own monitor (a locked session is enough) | start of Task 6 |
| 3 | A ruling on any rehearsal difference that the real run would also meet | end of Task 6 |
| 4 | The sitting: the test library's `Movies` folder emptied; two disc insertions; yes to each approval; yes or no for each film on Jellyfin | Task 7 |
| 5 | Removal of the run's leftovers, or yes to it | end of Task 7 |
| 6 | Marking the pull request ready for review | end of Task 8 |

## This plan's rulings

- **T7-R1 (the second disc's lookups).** No command makes TheDiscDB or TMDb lookups for a scan on the Mac, and the TMDb token is only on the mini PC. The shortlist of Task 5 therefore rests on the scans and the menu spike's decide documents, and the lookups of call 15 are the engine's own in the first minutes of the job. A plan that is not step 4 of spec 8.1 in Direct mode is cancelled and the next candidate goes in.
- **T7-R2 (the mark that step 10 ran).** `<job folder>/<title key>/subs-ocr/summary.json` is a file. The relay reads it; no new field enters `state.json`.
- **T7-R3 (no helper script for the sitting).** The saved outputs come straight over ssh into files on the Mac (`ssh "$BOX" '<command>' > file`), which carries bytes and needs no file on the mini PC. The commands are in Tasks 6 and 7; no script is committed for them.
- **T7-R4 (the tree's place).** `C:\lazymkv\run\release\<the zip's one folder>\`, so the run folder's protected list covers it. The run folder may hold the three loose programs or `release`, never both.

## Review Focus

1. A saved output that is not UTF-8 JSON (UTF-16 from Windows PowerShell's `>`, an empty file, the relay's refusal line): `compare-job` exits 2 and names the file; it never reports "differences" or panics. Test in Task 2.
2. A job that never reached Decide (cancelled, failed, or held before it): `state.json` has no plan, or a title has no `decide.json`. `compare-job` exits 2 for a missing plan, and prints `not compared: title <key> has no decide.json` for each such title. Tests in Tasks 1 and 2.
3. `--decide` given twice for one key, or for a key the job does not have: an argument error, not the last one winning. Test in Task 2.
4. A job with two titles of which only one had its subtitles read: the relay gives `--text` for that one alone. Test in Task 3.
5. An interrupted earlier install left a partial `release` folder, or the loose programs of a build are still in the run folder: the script removes what it put there and installs cleanly. Test in Task 4.

---

### Task 0: Start (controller)

**Files:**
- Create: `.superpowers/sdd/2026-11-23-phase-2-pr7-acceptance/progress.md` (git-ignored ledger)
- Commit: `docs/superpowers/specs/2026-11-23-phase-2-pr7-design-notes.md`, `docs/superpowers/plans/2026-11-23-phase-2-pr7-acceptance.md`

- [ ] **Step 1: Baseline.** Run `CARGO test --workspace --exclude lazymkv-ocr --locked --offline` on branch `phase-2-acceptance`. Expected: 0 failed. Write the three numbers (passed, failed, ignored) to the ledger as N0.
- [ ] **Step 2: Commit the notes and this plan** as one signed commit, `docs: the design notes and plan for pull request 7 (acceptance)`. Run the signature check of Global Constraints.
- [ ] **Step 3: Push and open the draft pull request** against `main` (call 26). The body names the notes and the plan and carries no attribution line. Bind it with the app's pull request tools and read its CI. Expected: CI green on three systems, CodeQL 0.

### Task 1: The comparison rules

**Files:**
- Create: `crates/dev/src/cmd/compare_job.rs`
- Modify: `crates/dev/src/cmd/mod.rs` (add `pub mod compare_job;`)
- Modify: `crates/core/src/plan/mod.rs` (the constant, used by `refresh_identity`)

**Interfaces:**
- Consumes: `crate::pyjson::{structural_diff, Tolerance}` (`crates/dev/src/pyjson.rs`).
- Produces, in `lazymkv_core::plan`:
  `pub const REFRESHED_IDENTITY: &str = "the identity was made again with the answered lookups (8.1): ";`
  `refresh_identity` builds its sentence from it; the sentence's text does not change (the test at `plan/mod.rs` that pins "…(8.1): step discdb_one, confidence 0.95" still passes).
- Produces, in `lazymkv_dev::cmd::compare_job`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Whole, PlanAndAudio }

/// The output lines in order, and how many of them are differences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report { pub lines: Vec<String>, pub differences: usize }

/// `state`: state.json. `own`: (title key, decide.json) for every title that has one, in key order.
/// `saved_plan`: lazymkvctl plan's output. `saved`: (title key, lazymkvctl decide's output).
/// Err is an argument-level problem's text (exit 2); Ok is the report (exit 0 or 1).
pub fn compare(state: &Value, own: &[(String, Value)], saved_plan: &Value, saved: &[(String, Value)], mode: Mode)
    -> Result<Report, String>
```

The rules `compare` implements (calls 3, 4 and 5):

- Tolerance is exact: `Tolerance { abs: 0.0, loose: 0.0, loose_keys: &[], ignore: Vec::new() }`. The engine's side is `expected`.
- **Errors.** `state["plan"]` null or absent: `the job has no plan`. `saved_plan` without a `plan` key: `the saved plan output has no plan key`. An own document without `audio_rule_version` or `lazymkv_version`: `<key>/decide.json lacks <name>`. In `Whole`: an own key with no saved pair: `no --decide for <key>`. In either mode: a saved key with no own document: `the job has no decide.json for <key>`.
- **Whole.** The plan line set is `structural_diff(state["plan"], saved_plan["plan"])`, each line prefixed `plan: `. For each key, the own document without its two extra keys against the saved one, each line prefixed `decide <key>: `.
- **Lookup pending** (both modes): any string of `state["plan"]["evidence"]["sentences"]` starts with `REFRESHED_IDENTITY`. The first output line is then `the job was planned with a lookup pending: only the plan's identity is compared (design part 12.7)`. The plan comparison covers `/identity` alone (pointers printed as `/identity/...`). In `Whole`, each decide comparison replaces the whole `/plan` section on both sides by `{"plan": {"identity": <its identity>}}`, so the section's slug, inputs and settings are not compared either (call 4).
- **PlanAndAudio.** The plan as above. For each saved pair, `/audio` and `/decision/decision/audio` alone, pointers printed in full. Own titles without a pair are not an error. After the differences comes one line: `not compared: slug, label, reconcile, subtitles, reviews, the plan section's slug, inputs and settings, and the decision section apart from decision.audio (design notes, call 5)`, then one `not compared: title <key>` line per own title without a pair.
- **Titles without a document** (both modes): for each key of `state["titles"]` (an array of objects with a `key`) that has no own document, one line `not compared: title <key> has no decide.json`, after the differences and before the last line.
- **Last line.** `compare-job: no difference` or `compare-job: <n> difference(s)`. `differences` counts the prefixed diff lines only.

- [ ] **Step 1: Write the failing tests** in `compare_job.rs`'s `#[cfg(test)] mod tests`, on documents made with `json!`. A helper `engine_doc(doc: Value) -> Value` appends the two extra keys. Each test asserts `lines` whole and `differences`:

```rust
#[test] fn equal_documents_give_no_difference()            // Whole; lines == ["compare-job: no difference"]
#[test] fn a_plan_value_that_differs_is_one_line()          // "plan: /mode: \"direct\" != \"copy\"", then "compare-job: 1 difference(s)"
#[test] fn a_decide_value_that_differs_names_its_title()    // "decide t00-00800: /label: ..."
#[test] fn the_two_extra_keys_are_dropped_and_required()    // equal with them; Err("t00-00800/decide.json lacks lazymkv_version") without
#[test] fn key_order_and_number_kind_count_as_the_diff_helper_reads_them() // {"a":1,"b":2} vs {"b":2,"a":1} equal; 1 vs 1.0 a difference
#[test] fn a_refreshed_plan_compares_its_identity_only()    // titles differ, identity equal: first line is the pending sentence, 0 differences;
                                                            // identity differs: "plan: /identity/step: ..."; decide's /plan/slug, /plan/inputs and
                                                            // /plan/plan/titles ignored, its /plan/plan/identity compared
#[test] fn plan_and_audio_compares_the_plan_and_the_two_audio_parts_only() // slug, label, subtitles differ: 0 differences and the
                                                            // "not compared" lines; /audio/0/x differs: 1; /decision/decision/audio/y differs: 1
#[test] fn plan_and_audio_names_a_title_it_was_given_no_pair_for()
#[test] fn a_job_without_a_plan_and_a_pair_without_a_document_are_errors() // the Err texts above, each
#[test] fn whole_requires_a_pair_for_every_title_that_has_a_document()
#[test] fn a_title_of_state_json_without_a_document_gets_its_not_compared_line_in_both_modes()
```

- [ ] **Step 2: Run them.** `CARGO test -p lazymkv-dev --locked --offline compare_job`. Expected: they do not compile (`compare` is not defined).
- [ ] **Step 3: Add `REFRESHED_IDENTITY` to `core` and implement `compare`.** No file or argument handling in this task.
- [ ] **Step 4: Run them again**, then `CARGO test -p lazymkv-core --locked --offline plan`. Expected: all pass.
- [ ] **Step 5: Commit** (controller): `dev: compare-job's rules (pull request 7, calls 3 to 5)`.

### Task 2: The command

**Files:**
- Modify: `crates/dev/src/cmd/compare_job.rs` (`run`, with its tests)
- Modify: `crates/dev/src/lib.rs` (one row in `COMMANDS`, after `menu`)
- Modify: `crates/ctl/src/tests.rs` (one test)

**Interfaces:**
- Consumes: `compare`, `Mode`, `Report` of Task 1; `crate::evidence::Ctx` (`ctx.abs`), `crate::{usage_error, Args}` as `crates/dev/src/cmd/menu.rs` uses them.
- Produces: `pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32`, and the row
  `("compare-job", cmd::compare_job::run as Command, "<job folder> --plan FILE [--decide KEY=FILE ...] [--plan-and-audio]")`.

Behaviour (calls 2 and 7):

- One positional, the job folder. `--plan FILE` exactly once (read with `Args::flags`, since `Args::flag` keeps only the last value). `--decide KEY=FILE` any number of times, split at the first `=`; an empty key or file, or a key given twice, is a usage error. `--plan-and-audio` picks `Mode::PlanAndAudio`. `-h` or `--help` prints the usage and returns 0. Any other flag is a usage error. Usage errors go through `usage_error` and return 2.
- It reads `<job folder>/state.json`, then `<job folder>/<key>/decide.json` for every `key` of `state["titles"]` whose file exists, then the saved files. A file that cannot be read or parsed returns 2 with `compare-job: <path as given>: <reason>` on `err`. When the bytes start with `FF FE` or `FE FF`, or hold a NUL, the reason is `not UTF-8 JSON (it looks like UTF-16; save the output as bytes, not through PowerShell's >)`. An empty file's reason is `empty`.
- An `Err` from `compare` returns 2 with `compare-job: <text>`. Otherwise the report's lines go to `out`, and the exit code is 0 for no difference and 1 for any.

- [ ] **Step 1: Write the failing tests**, through `crate::dispatch` on a `Scratch` folder (`lazymkv_core::testsupport::Scratch`, `Ctx::for_test`), with a helper that writes a small job folder (`state.json` with one or two titles, their `decide.json`) and the saved files:

```rust
#[test] fn an_equal_job_exits_0_and_prints_no_difference()
#[test] fn a_difference_exits_1_with_its_line_on_stdout()
#[test] fn plan_and_audio_is_chosen_by_its_flag()
#[test] fn help_prints_the_usage_and_exits_0()
#[test] fn each_argument_error_exits_2()   // no folder; two folders; no --plan; --plan twice; --decide without '=';
                                           // --decide '=f'; --decide 'k='; one key twice; an unknown flag
#[test] fn decide_splits_at_the_first_equals_sign()          // "t00-00800=dir/a=b.json" reads dir/a=b.json
#[test] fn an_unreadable_or_non_json_file_exits_2_and_names_it() // missing state.json; saved plan "no job x-1\n";
                                                             // an empty file; UTF-16LE bytes with a BOM, with the hint
#[test] fn a_job_without_a_plan_exits_2()                    // "compare-job: the job has no plan"
#[test] fn a_title_without_decide_json_is_left_out_and_a_pair_for_it_is_an_error()
```

  And in `crates/ctl/src/tests.rs`, following its existing tests for a developer command:

```rust
#[test] fn compare_job_is_a_lazymkv_dev_command_only()
// lazymkvctl compare-job x exits 2 with "lazymkvctl: compare-job is a lazymkv-dev command, not a lazymkvctl command"
```

- [ ] **Step 2: Run them.** `CARGO test -p lazymkv-dev --locked --offline compare_job` and `CARGO test -p lazymkv-ctl --locked --offline compare_job`. Expected: FAIL (unknown command `compare-job`).
- [ ] **Step 3: Implement `run` and add the `COMMANDS` row.** If a test pins the help table or a command count, update it in this task.
- [ ] **Step 4: Run the whole workspace**, `CARGO test --workspace --exclude lazymkv-ocr --locked --offline`. Expected: 0 failed.
- [ ] **Step 5: Commit** (controller): `dev: the compare-job command (pull request 7, calls 1, 2 and 7)`.

### Task 3: Stand-in jobs through the relay's words, and the relayed decide (calls 8 and 9)

**Files:**
- Create: `crates/fake-tools/tests/compare_job.rs`
- Modify: `crates/engine/src/relay.rs` (`CommandJob`, `child_words`, their tests)
- Modify: `crates/engine/src/owner.rs` (where `CommandJob` is built, near the line that reads `tuning_copy`)

**Interfaces:**
- Consumes: the `compare-job` command of Task 2 through `lazymkv_dev::dispatch`; the harness as `crates/fake-tools/tests/engine_command.rs` uses it (`RELAY`, `client`, `c.command`, `ctl_spawns`), the tuning helpers of `engine_tuning.rs` (`on()`, `wait_copy_state`) and the two-stream set-up of `engine_file_phase.rs` (`two_subs`, the worker stand-in).
- Produces: `CommandJob` gains `pub(crate) text_keys: Vec<String>`, the titles whose step 10 ran (T7-R2). `child_words` adds `--text <tuning_root>/dataset` to a `decide --job --title KEY` only when the dataset copy is done **and** `KEY` is in `text_keys`.

The test file's helpers: `filed_with_copy(variants: &[&str]) -> (Harness, String)` (scenarios `movie-direct`, the variants and `command-relay`; settings `{"keep_tuning_evidence": true, "drive_settle_s": 0, "drive_ready_timeout_s": 1}`; runs until the job is `Filed`, approving it first if it stops at `NeedsReview` as the tests near the end of `engine_tuning.rs` do for the two-stream job; waits for `TuningCopy::Done`; restarts) and `saved(h, id, words) -> PathBuf` (sends `words` as a `command`, takes the stand-in `lazymkvctl`'s spawned arguments, runs them through the real `lazymkv_dev::dispatch`, requires exit 0, writes the output to a scratch file).

- [ ] **Step 1: Write the tests.**

```rust
/// One English subtitle stream: step 10 does not run (Pan's Labyrinth's case).
#[test] fn a_single_stream_job_compares_equal_through_the_relays_words() {
    // saved plan and saved decide for KEY, then:
    // dispatch(["compare-job", job, "--plan", plan, "--decide", "t00-00800=<decide>"]) == 0
    // stdout == "compare-job: no difference\n"; the decide words hold no "--text"
}
/// Two English subtitle streams: step 10 runs, and the words hold --text <tuning>/dataset.
#[test] fn a_two_stream_job_compares_equal_through_the_relays_words()   // variant "movie-direct-two-subs"; same assertions, "--text" present
#[test] fn a_changed_saved_value_is_a_difference()   // the saved plan's /plan/title_confidence set to 0.0: exit 1,
                                                     // one line starting "plan: /title_confidence: "
```

  And in `relay.rs`'s tests, beside `one_job`:

```rust
#[test] fn decide_job_gets_text_only_for_a_title_whose_step_10_ran()
// keys ["t00-00800", "t01-00801"], text_keys ["t01-00801"], dataset_done true, root "tuning-root":
// the words for t01-00801 end with ["--text", "<tuning-root>/dataset"]; the words for t00-00800 hold no "--text";
// with dataset_done false neither does.
```

- [ ] **Step 2: Run them.** `CARGO test -p fake-tools --locked --offline --test compare_job` and `CARGO test -p lazymkv-engine --locked --offline relay`. Expected: the relay test does not compile (`text_keys`); with that test left out, `a_single_stream_job_compares_equal_through_the_relays_words` FAILS with a line starting `decide t00-00800: /subtitles`, and the other two pass. If the two-stream test or the changed-value test fails here, stop, write what it printed to the ledger and bring it to the controller.
  **If the single-stream test passes here**, the notes' reading was wrong: stop, write that to the ledger as T7-E-next, leave `relay.rs` and `owner.rs` alone, keep the three stand-in tests, and tell the owner call 9 lapsed.
- [ ] **Step 3: Implement.** `text_keys` in `CommandJob`; the condition in `child_words`; in `owner.rs` fill it with each key of the job whose `<job folder>/<key>/subs-ocr/summary.json` is a file, with the expression `crates/engine/src/steps.rs` already uses for "step 10 ran" (`job_dir.join(key).join(READING).join("summary.json").is_file()`). Update `child_words`' and `CommandJob`'s doc comments, citing "pull request 7's notes, call 9", and the three other places that build a `CommandJob` in tests (`text_keys: Vec::new()`, or the key where the test's `dataset_done` case expects `--text`).
- [ ] **Step 4: Run** the two commands of Step 2, then the whole workspace. Expected: 0 failed. Confirm in the output of the single-stream test's job that `whole_job.rs`'s file set for `movie-direct` holds no `subs-ocr/summary.json` (T7-R2's premise); if it does, stop and bring it to the controller.
- [ ] **Step 5: Commit** (controller): `engine: a relayed decide gets --text only for a title whose subtitles were read (pull request 7, call 9)`.

### Task 4: The zip behind the desktop task (call 10)

**Files:**
- Modify: `scripts/engine-task.ps1`
- Modify: `crates/fake-tools/tests/scripts.rs`
- Modify: `docs/phase2/engine.md` (the paragraph that documents `engine-task.ps1`, near "scripts/engine-task.ps1")

**Interfaces:**
- Produces two parameters, `[string]$Zip = ''` and `[string]$Sha256 = ''`, and three functions that the tests call alone through `define_and_call`:

```powershell
# '' when $zip is a file whose SHA-256 is $sha256 (64 hex digits, either case); else the refusal's text.
function Get-ZipProblem([string]$zip, [string]$sha256)
# Extracts $zip into the fresh folder $into (an earlier one is removed first) and returns the tree's path.
# Throws unless the zip holds exactly one top-level folder whose name starts with "lazymkv-".
function Expand-ReleaseTree([string]$zip, [string]$into)
# '' when $tree holds bin\lazymkv-engine.exe, bin\lazymkvctl.exe, bin\lazymkv-ocr.exe, resources\mkvmerge.exe,
# resources\ffprobe.exe and resources\ffmpeg.exe; else "the release tree lacks <the first one missing, in this order>".
function Get-TreeProblem([string]$tree)
```

The script's flow with `-Zip` (T7-R4):

1. `-Zip` without `-Sha256`, or the reverse, is refused first, with `-Zip and -Sha256 go together: the hash is the zip's line in the release workflow's SHA256SUMS`.
2. The elevation, task-state and `-Data` refusals run as today. The build folder's checks (the three programs in `C:\lazymkv\target\release`; the writers of `C:\lazymkv\src`, `C:\lazymkv\target`, the release folder and its programs) are skipped. The writers check of the run folder's parent stays, under `-AcceptWritableBuild`.
3. The run folder is created, checked for a link and protected as today. The planted-file check accepts the three programs, `release` and `release.zip`, and nothing else.
4. The three loose programs and an earlier `release` and `release.zip` are removed. The zip is copied to `C:\lazymkv\run\release.zip`, and `Get-ZipProblem` is asked about **that copy**, so nothing can change it between the check and the extraction. A problem removes the copy and is thrown.
5. `Expand-ReleaseTree` extracts the copy into `C:\lazymkv\run\release`; the copy is removed. Every extracted item's owner is set to Administrators, as `packaging/release/smoke.ps1` does (`icacls <release folder> /setowner *S-1-5-32-544 /T /C /Q`, a non-zero exit thrown), since any other owner counts as a writer. Then `Get-TreeProblem`, and `Get-Writers` on the tree's `bin`, `resources` and the six files, must each come back empty.
6. The task is registered at `<tree>\bin\lazymkv-engine.exe` with `<tree>\bin` as its working folder; `lazymkvctl status` is asked of `<tree>\bin\lazymkvctl.exe`. Everything after that is as today, the last line included.

Without `-Zip` the script behaves as today, except that it removes an earlier `release` and `release.zip` before it copies the programs.

- [ ] **Step 1: Write the failing tests** in `scripts.rs`. On every system, on the script's text:

```rust
#[test] fn the_task_script_takes_a_zip_only_with_its_hash()
// the param block declares [string]$Zip and [string]$Sha256; the refusal text of flow step 1 is in the script;
// the script never calls Get-FileHash on $Zip itself, only on the copy in the run folder
```

  On Windows, through `define_and_call`, with fixtures made inside the PowerShell call (`Compress-Archive` into `unique(...)` temp folders, removed afterwards):

```rust
#[cfg(windows)] #[test] fn the_zip_check_refuses_a_missing_file_a_malformed_hash_and_a_wrong_hash_and_passes_the_right_one()
#[cfg(windows)] #[test] fn the_extraction_returns_the_one_lazymkv_folder_and_replaces_an_earlier_tree()
// an earlier $into holding a stray file: gone afterwards; a zip with two top-level entries, or one not named lazymkv-*: throws
#[cfg(windows)] #[test] fn the_tree_check_names_the_first_file_a_tree_lacks()
// a full tree: ''; without resources\ffprobe.exe: "the release tree lacks resources\ffprobe.exe"
```

- [ ] **Step 2: Run** `CARGO test -p fake-tools --locked --offline --test scripts`. Expected: the all-systems test FAILS; the Windows ones are not built on the Mac.
- [ ] **Step 3: Implement** the parameters, the three functions and the flow. Update the script's header comment (the new usage line, where the hash comes from) and the paragraph in `docs/phase2/engine.md`. Update any existing test in `scripts.rs` whose assertion the new lines change.
- [ ] **Step 4: Run** Step 2's command, then parse the script with no Windows machine: on the Mac nothing parses PowerShell 5.1, so the parse test `the_task_script_parses_without_errors` and the three Windows tests are verified by CI. Expected on the Mac: 0 failed.
- [ ] **Step 5: Commit** (controller): `scripts: engine-task.ps1 installs a release zip's tree (pull request 7, call 10)`. Push. Expected in CI: the Windows job runs the three new Windows tests and passes; the `Release` workflow runs on this head (the crates changed in Tasks 1 to 3) and uploads `lazymkv-release-dry-run`.

### Task 5: The second disc's shortlist (controller; the Mac only; private)

**Files:**
- Create (git-ignored): `.superpowers/pr7/shortlist/<slug>.plan.json`, `.superpowers/pr7/shortlist/table.tsv`

- [ ] **Step 1: Build the developer program.** `CARGO build -p lazymkv-dev --locked --offline`.
- [ ] **Step 2: Plan every captured scan.** For each folder of `~/lazymkv-menu-spike/captures/` that holds a `scan.txt`: `target/debug/lazymkv-dev plan <folder>/scan.txt > .superpowers/pr7/shortlist/<slug>.plan.json`. A scan that exits non-zero is listed with its error line, not dropped.
- [ ] **Step 3: Tabulate**, one row per slug: `plan.kind`, `plan.mode`, `plan.title_confidence`, `plan.reviews`, the number of rivals in `plan.evidence.rivals`; from the scan, the main title's English subtitle streams (forced-only twins not counted) and whether an audio stream is Atmos or DTS:X; from `~/lazymkv-menu-spike/out/decide-8f30673/<slug>--*.json` where one exists, whether any audio verdict is a commentary or an audio description and what the subtitles section says.
- [ ] **Step 4: Filter by call 14.** Keep a row when: the kind is a movie; the mode is `direct` with no title review; the main title has two or more English subtitle streams; and no decide document shows a commentary or audio description verdict. Rank object audio first, then discs the spike already ripped (their decide documents are evidence), then fewer English audio tracks. Leave out the four discs that are evidence recordings.
- [ ] **Step 5: Ask the owner** to pick, showing up to three candidates by title (titles from `~/lazymkv-menu-spike/private/`, in the session only), each with one line of why, and the order they would be tried in. If nothing passes the filter, say so and ask for a scanning sitting on further discs of the collection list.
- [ ] **Step 6: Ledger.** The picked slugs in order, never titles. No commit.

### Task 6: Install the zip and rehearse (controller; no disc)

**Files:**
- Create (git-ignored): `.superpowers/pr7/zip/`, `.superpowers/pr7/settings-before.json`, `.superpowers/pr7/jobs/<job id>/` with `saved/`

Shell variables used below: `BOX` (from `scripts/phase0/box.env`); `CTL='C:\lazymkv\run\release\<tree>\bin\lazymkvctl.exe'` once Step 5 has run; `ID` a job ID; `KEY` a title key. In zsh, take `$?` before any pipe. `$CTL` runs on the mini PC: every `$CTL …` below is `ssh "$BOX" "$CTL …"`. `status` shows a job's state, review reasons and notes only; a job's `rip_again`, `tuning_copy`, `plan.identity.step` and `plan.mode` are read from its record, `ssh "$BOX" "type C:\ProgramData\lazymkv\jobs\$ID\state.json"`, written `STATE` below.

- [ ] **Step 1: Fetch the zip.** `gh run list --workflow release.yml --branch phase-2-acceptance --limit 5` gives the run on the head that holds Tasks 1 to 4. `gh run download <run> -n lazymkv-release-dry-run -D .superpowers/pr7/zip/<short sha>`. Read the zip's line in `SHA256SUMS`, and check it with `shasum -a 256`. Expected: equal. Ledger: the run's number, the commit, the hash.
- [ ] **Step 2: Read the mini PC, changing nothing.** Free space on `C:`; the state of the task `lazymkv-engine`; `lazymkvctl status` from the present run folder (expected: two filed jobs, nothing active); `lazymkvctl settings get > .superpowers/pr7/settings-before.json`; whether `C:\lazymkv\evidence\owner\pans-labyrinth-2006-uhd` exists and one of its files hashes as the Mac's `corpus/owner` copy does. Ask any other session that uses the mini PC whether it is free.
- [ ] **Step 3: Ask the owner** (prerequisite 2), listing the standing changes: the engine is stopped; the run folder's three programs are replaced by the zip's tree; the task is registered again at the tree; `tool_paths.ffprobe` and `tool_paths.ffmpeg` leave the settings. Proceed only on a yes.
- [ ] **Step 4: Copy.** Create `C:\lazymkv\tmp\pr7` on the mini PC, then `scp` the zip into it. `scripts/box.sh` with no command syncs the tree, which puts `scripts\engine-task.ps1` at `C:\lazymkv\src\scripts\`.
- [ ] **Step 5: Install.** `lazymkvctl stop` from the present run folder; the task reads `Ready`. Then
  `ssh "$BOX" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\engine-task.ps1 -Zip C:\lazymkv\tmp\pr7\<zip> -Sha256 <hash> -AcceptWritableBuild'`.
  Expected last line: `ENGINE-TASK-DONE status=ok`. Any other outcome: stop, read the refusal, and do not retry with changed flags without a ruling.
- [ ] **Step 6: Settings and self-test.** `settings set` lays its file over the settings in force and merges `tool_paths` key by key, so a key that is left out stays. Write `{"tool_paths":{"ffprobe":null,"ffmpeg":null}}` to a file on the mini PC and run `$CTL settings set <file>`; `settings get` shows both as `null`. `lazymkvctl selftest`. Expected: `mkvmerge`, `ffprobe` and `ffmpeg` each ok and found in `resources`, at the versions of `packaging/tools.json`; MakeMKV ok with a valid licence; the roots, the TMDb token and the webhooks ok. This also answers X1 for a start: the statically linked programs run (call 22).
- [ ] **Step 7: Save the outputs of both filed jobs.** For each job, into `.superpowers/pr7/jobs/$ID/saved/`:
  `ssh "$BOX" "$CTL plan --job $ID" > plan.json`, and for each of the job's title keys that has a `decide.json`, `ssh "$BOX" "$CTL decide --job $ID --title $KEY" > decide-$KEY.json`. Each exit code is 0. Then pull the job folder: `scp -r "$BOX:C:/ProgramData/lazymkv/jobs/$ID" .superpowers/pr7/jobs/$ID/job`.
- [ ] **Step 8: The whole comparison**, for each job: `target/debug/lazymkv-dev compare-job .superpowers/pr7/jobs/$ID/job --plan .../saved/plan.json --decide $KEY=.../saved/decide-$KEY.json`. Record the exit code and each difference's pointer (never its values) in the ledger.
- [ ] **Step 9: The plan-and-audio comparison** for the Pan's Labyrinth job, against phase 0's recording on the mini PC (from Step 2; copy that one recording there first if it is missing or behind):
  `ssh "$BOX" "$CTL plan C:\lazymkv\evidence\owner\pans-labyrinth-2006-uhd --settings C:\ProgramData\lazymkv\jobs\$ID\settings.json" > p0-plan.json`, and `... "$CTL decide --rec C:\lazymkv\evidence\owner\pans-labyrinth-2006-uhd\<phase 0's label of the feature> --settings <the same>" > p0-decide.json` (no `--text`). Then `compare-job <job> --plan p0-plan.json --decide $KEY=p0-decide.json --plan-and-audio`.
- [ ] **Step 10: Read the results.** In the plan-and-audio comparison `/audio` holds each level folder's name and its measures as exact numbers, and phase 0 took them with another FFmpeg build and its own folder names, so a difference there is likely and is the kind this step exists to find. Older builds made these jobs, so sort each difference: one that only an older build explains (say which commit changed the value) is a finding for the notes; one the real run would also meet goes to the owner now (prerequisite 3), with the pointer, what each side holds in kind, and a proposed ruling. A fault in lazymkv becomes a fix task with its test first, a new zip (call 12) and this task again from Step 1.
- [ ] **Step 11: Leave the engine up**, the task `Ready` or `Running` as it was found, and tell any session asked in Step 2 that the mini PC is free. No commit.

### Task 7: The sitting (controller and the owner)

The checklist is design part 12.7's, with the notes' amendments. Each item's result goes into the ledger as it is read. The owner's steps are marked **Owner**. The shell variables, and the `$CTL` and `STATE` forms, are Task 6's.

- [ ] **Step 1: Before the first disc.** **Owner:** the test library's `Movies` folder is empty; say so. Controller: `settings get` shows part 12.7's roots, `tv_root` unset and `keep_tuning_evidence` on (set them with `settings set` on the owner's yes if not; a root to unset is sent as `null`); `selftest` passes as in Task 6 Step 6; the zip's SHA-256 is the ledger's; no job is active.
- [ ] **Step 2: Arm the repeat.** `"$CTL" rip-again <the earlier filed Pan's Labyrinth job>` (call 13). Expected: exit 0, and `STATE` of that job shows `"rip_again": true`.
- [ ] **Step 3: Pan's Labyrinth.** **Owner:** insert the disc and give no command. Controller watches `status` from a background wait that ends when the job's state changes. Expected: the job reaches `NeedsReview` with `commentary_detected` only; `disc_done` arrived and the disc was ejected; `needs_review` arrived. **Owner:** yes to approve. Controller: `"$CTL" approve $ID`. Expected: `Filed`, and `filed` arrived. Notifications are read through the Home Assistant connector.
- [ ] **Step 4: Its comparisons.** When `STATE` shows `"tuning_copy": "done"` (call 6; the job's pending note stays after the copy and is no sign of it): Task 6's Steps 7 and 8 for this job, expected exit 0; then Step 9 against phase 0's recording, expected exit 0 or only what the owner ruled on after the rehearsal. Step 10 must not have run on this disc: the pulled job folder holds no `<key>/subs-ocr/summary.json`.
- [ ] **Step 5: The second disc.** **Owner:** insert the first candidate. Within minutes the job's plan is in `STATE`. Expected: `plan.identity.step` is `tmdb_one` (spec 8.1 step 4) and `plan.mode` is `direct`. Otherwise (T7-R1): `"$CTL" cancel $ID`, the disc comes out, the ledger gets the reason by slug, and the next candidate goes in. The job then ends `Filed`, or `NeedsReview` with `subtitle_ambiguous` only, approved on the owner's yes. A hold for any other reason is a finding, and the next candidate goes in (design part 12.7). As for the first disc: `disc_done` arrived and the disc was ejected; `needs_review` arrived if the job was held; `filed` arrived.
- [ ] **Step 6: Its checks.** The whole comparison as in Step 4, expected exit 0. In the pulled job folder: `<key>/subs-ocr/summary.json` exists, and for each English bitmap stream its entry there (`outcome`, `exit`, `wall_ms`) shows a read within P19's cap, or the job carries the note `subtitle_text_unread`. The per-stream `sub-<n>.run.json` files are on the share, not in the job folder. If the disc has Atmos or DTS:X: the profile string in the job's `ffprobe` output for that stream, compared with OR7's spelling in `crates/core/src/codec.rs`; a difference is a fix task (spec section 17 row 6 and `core`), then call 21.
- [ ] **Step 7: Jellyfin.** For each film the controller reads from the job's `decide.json` the filed path's title and year, the default audio track and the default subtitle track, and states them. **Owner:** the film shows once under that title, plays, and starts with those tracks: yes or no for each.
- [ ] **Step 8: The paths.** From each pulled job folder (`state.json`, `transitions.jsonl`, `job.log`): every path written is under the engine's staging or tuning subfolder or the test library. Record yes or the path's kind.
- [ ] **Step 9: Close the sitting.** The engine is up with nothing active. List for the owner what the run left (the two films in the test library, the staging and tuning data, the job folders, the earlier settings in the ledger) and ask what to remove (prerequisite 5). Ledger: every checklist item with its result. No commit.

**When a step fails:** stop the sitting at that step. A fault in lazymkv: a fix task with its test first, the new head's zip installed by Task 6 Steps 1, 4 and 5, then the disc again with `rip-again` and a fresh `movies_root` (design part 6.3; call 21). A fault in a tool or the disc: a finding, brought to the owner.

### Task 8: The notes, and closing phase 2

**Files:**
- Modify: `docs/phase2/engine.md` (new "Pull request 7" sections; RP-4 and T16-r1; the "Left for later pull requests" entry for pull request 7; the tests table)
- Modify: `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (part 12.7; the header's amendment sentence)
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (section 16 row 2; section 17 row 6)
- Modify: `README.md` (phase 2's state; `compare-job` beside the other developer commands, if they are listed)

- [ ] **Step 1: `docs/phase2/engine.md`, "Pull request 7".** In the form of the "Pull request 6" sections: the owner's decision A1 and the approval of the notes' calls 3, 9, 22, 23 and 24; this plan's rulings T7-R1 to T7-R4 and the ledger's T7-E rulings; the rehearsal's findings; the checklist with each item's result, by slug; the zip's run number, commit and SHA-256; the findings; the second disc's `ffprobe` profile string against OR7; the deferral of the DVD disc ID [LIB-6] (design part 12.6 item 7); what phase 2 leaves to phase 3; the counts of CI's three jobs on the final head (call 24). RP-4 and T16-r1 name pull request 7, not 6.
- [ ] **Step 2: The design's amendments, in place and marked** as pull request 6's were: part 12.7's "It ignores exactly…" sentence replaced by call 3's rule; its phase 0 sentence given call 5's two parts; "The built binaries are in the run folder" as the notes' section 3 words it; the self-test line of call 11 added to "Before the first disc"; part 9.5's `decide --job` row saying `--text` is given for a title whose subtitles were read (call 9).
- [ ] **Step 3: The spec.** Section 16 row 2 gains its result in one sentence. Section 17 row 6 gets the profile string's answer, or stays as it is if the second disc had neither format.
- [ ] **Step 4: Check the documents.** No title, job ID, host, user name or drive serial: `git diff main -- docs README.md | grep -n -i -E '<the private words, read from the ledger>'` prints nothing. `CARGO test --workspace --exclude lazymkv-ocr --locked --offline` passes (tests read the script and some documents).
- [ ] **Step 5: Commit and push** (controller): `docs: phase 2's acceptance, results and notes (pull request 7)`. Read CI's three counts from the run of this head and, if they differ from Step 1's, correct them in one further docs commit.
- [ ] **Step 6: The whole-branch review** (one Opus reviewer over `main..HEAD`, told the notes' anti-goals), its fixes as their own commits, then the pull request's final body. Ask the owner whether to mark it ready for review (prerequisite 6).
