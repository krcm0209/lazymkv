# Phase 2: the engine

Phase 2 of the design spec (`docs/superpowers/specs/2026-10-04-lazymkv-design.md`, section 16, row "2. Direct-mode
engine") builds the direct-mode engine, following `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md`
(design part 12.6). This document grows with each of the six pull requests of design part 12.6. This first part
records pull request 2, on the branch `phase-2-thin-job` from `main` at `70ada2b`. Its plan is
`docs/superpowers/plans/2026-10-24-phase-2-pr2-thin-job.md`, and it is pull request krcm0209/lazymkv#6, opened as a
draft on 2026-10-25 with the owner's yes. The rulings below shorten "pull request" to PR, as the plan does. They cite
the phase 2 design as "design part" and the design spec as "spec".

## Pull request 2: the thinnest whole job

Pull request 2 delivers design part 12.6 item 2:

- the engine's reducer loop and persistence (`crates/engine`: one owner thread over `core::job::reduce`, the job store, and restarts through `resume`);
- the `tools` drivers (`crates/tools`: `makemkvcon`'s drive listing, scan and rip, `mkvmerge`'s identification and remux, and `ffprobe`, each run recorded as `capture` records one);
- the `platform` traits with in-memory fakes (`crates/platform`);
- the clients behind the recorded transport (`crates/clients`: TheDiscDB and TMDb, The Movie Database, behind an HTTP (Hypertext Transfer Protocol) transport, replayed from recorded answers);
- the stand-in extensions of design part 12.2 (`crates/fake-tools`: stderr replay, output files named by an argument, several responses per run, index drift, hang until killed, exit without a completion message, `MSG:1009` at the scan, and an `ffmpeg` stand-in);
- `lazymkvctl`'s local `plan`, `decide` and `retune`, and its engine commands over the in-memory transport (`crates/ctl`);
- the R-B2 move: `capture`'s disc snapshot, its pure part into `core::snapshot` and its file input and output into `tools::snapshot`;
- the K5 split: dev's `decide` into readers that build a `DecideInputs` value from a recording folder, and the pure renderer `decide_doc(&DecideInputs) -> DecideDoc`, which the engine calls for each `decide.json`;
- lower-case job IDs (K3);
- the interfaces fixed as serde types in `core` (`core::record` and `core::ipc`, below).

The whole job runs with no disc, no network and no Windows API (application programming interface), on stand-in
tools, the platform fakes and recorded HTTP answers. Its acceptance is checked by continuous integration (CI). A
media-arrival event takes a synthetic movie disc to a filed placeholder file on ubuntu, macos and windows
(`crates/fake-tools/tests/whole_job.rs`; CI run https://github.com/krcm0209/lazymkv/actions/runs/40410226751).

## How to run it

Every cargo command runs on the mini PC through `scripts/box.sh`, one at a time (design part 12.5).

- `scripts/box.sh cargo test -p fake-tools --locked --test whole_job` runs the whole job.
- `scripts/box.sh cargo run -p lazymkv-ctl --locked --bin lazymkvctl -- plan <recording dir>` and the same with `decide --rec <dir>/<label>` print what `lazymkv-dev` prints, byte for byte, with the same exit codes (design part 9.5).
- In this version `scripts/box.sh cargo run -p lazymkv-engine --locked -- --data DIR` exits 1 on every operating system (OS) with its OS's sentence (S28). `lazymkvctl`'s engine commands exit 4 with "lazymkvctl: no engine transport exists in this build" (S38). Pull request 3 brings the Windows implementations and the pipe.

## The crates

| Crate (package, binary) | Depends on | Holds |
|---|---|---|
| `lazymkv-platform` | `lazymkv-core` | The operating-system seams of design part 4.2 as traits (`DriveWatcher`, `Ejector`, `ProcessGuard`, `KeepAwake`, `ShareConnector`, `SecretStore`, `IpcTransport`, `IpcConnector`, `Autostart`, `Host`), the std process runner `StdProcessGuard`, `write_atomic` and `rename_no_replace` (std in this version, S13), and the in-memory fakes behind the `fakes` feature |
| `lazymkv-tools` | `lazymkv-core`, `lazymkv-platform`, `serde`, `serde_json` | The recorded run `run_recorded` (capture's recorder, moved here, S17), the tool lookup `find_tool` (design part 3.6, S18), the `makemkvcon` drivers (`list_drives`, `scan`, `rip`, `drive_index` and `save_debug_log`), the `mkvmerge` identification and remux, `ffprobe`, and the disc snapshot's file input and output (`snapshot`, R-B2) |
| `lazymkv-clients` | `lazymkv-core`, `serde`, `serde_json` | TheDiscDB and TMDb behind the `HttpTransport` trait, the `RecordedTransport` that replays `http.json` answers, the lookup book and its rounds (`LookupBook`, `run_round`), `lookup-status.txt` and the slim saved answers (design parts 8.1, 8.3, 8.4) |
| `lazymkv-engine` (bin `lazymkv-engine`) | `lazymkv-core`, `lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-dev`, `serde`, `serde_json`; the `testkit` feature adds `lazymkv-platform/fakes` and `lazymkv-core/testsupport` | The job owner over `core::job::reduce` and its steps (the job profile among them), the job store (`state.json`, `transitions.jsonl`, `drives.json`, `engine.log`, `job.log`), arrivals, the disc-phase token and the file-phase slot, space promises, the restart, the IPC (inter-process communication) server, the self-test, the `NotifySink`, and the test harness `testkit` |
| `lazymkv-ctl` (bin `lazymkvctl`) | `lazymkv-core`, `lazymkv-platform`, `lazymkv-dev`, `serde_json` | `plan`, `decide` and `retune` run locally through `lazymkv_dev::dispatch`, and the engine commands of design part 9.6 over an `IpcConnector` |
| `lazymkv-capture` (bin `lazymkv-capture`), changed | `lazymkv-core`, `serde`, `serde_json`, and now `lazymkv-tools` | Its commands as before; `capture disc` now calls `tools::snapshot`, and `disc.rs` is gone, its items moved to `core::snapshot` and `tools::snapshot` (R-B2) |
| `lazymkv-dev` (bin `lazymkv-dev`), changed | `lazymkv-core`, `serde`, `serde_json` (unchanged) | `decide` split into `read_decide_inputs`, which reads a recording folder into a `DecideInputs` value, and the pure `decide_doc` (K5), with decide's three replay flags (S27); `Args::rest`, which `lazymkvctl` uses beside dev's existing `dispatch` (S85) |

The disc snapshot's pure part is `core::snapshot`, and its file input and output is `tools::snapshot`; `capture disc`
prints and writes the same bytes as before, which `crates/capture/tests/disc_snapshot.rs` pins. The goldens are
byte-identical; `crates/dev/src/cmd/decide/characterisation/` pinned `decide`'s output across the split.

## The interfaces fixed in core

`core::record` (`crates/core/src/record.rs`) holds the persisted records of design parts 7.3 and 7.4. Their `format`
strings are exact: `lazymkv job v1` for a job's `state.json` (`JobRecord`), `lazymkv drives v1` for `drives.json`
(`DrivesFile`) and `lazymkv remembered v1` for a remembered decision. `check_format` also accepts a minor version
(`v1.<minor>`) and refuses anything else, naming both strings. Each `transitions.jsonl` line (`Transition`) has the
keys `time_unix`, `event`, `old`, `new` and `actions`, with `event` and `old` null on a job's first line.
`drives.json` holds `drives`, an object keyed by drive ID whose values carry the drive's last `fingerprint` and whether
its arrival was `handled` (L1a, S3), and `drive_incapable`, the DriveIncapable failures seen so far.

`core::ipc` (`crates/core/src/ipc.rs`) holds the IPC messages of design parts 9.1 and 9.3. Every line of the pipe is
an envelope with the keys `v` (the protocol version, 1), `type`, `id` (`Option<u64>`, S10) and `payload`, at most
4 MiB long. The twelve request `type` strings are `settings_set`, `secret_set`, `retry`, `cancel`, `discard`,
`rip_again`, `run_selftest`, `history`, `shutdown`, `command`, `review_action` and `ui_quit`; a reply is `ok` with a
`result`, or `error` with a `message` and `problems`. The five push types are `snapshot`, `job_update`, `progress`,
`selftest_result` and `command_output`. The notification payload of spec 12 (`Notification`) is the event's `event`
tag and fields flattened beside `job_id`, `fingerprint`, `title`, `drive`, `message` and `time` (Unix seconds, S42);
the events are `disc_done`, `already_ripped`, `filed`, `needs_review`, `waiting`, `resumed`, `failed` and
`eject_failed`.

`core::robot::lines` (`crates/core/src/robot/lines.rs`) is the per-line reader of `makemkvcon` output (`LineReader`),
which tells a driver at once what a line means (progress, the index guard's verdict, a licence message, a rejected
profile) and sums the run into a `RunSummary`. `core::ustar` (`crates/core/src/ustar.rs`) reads `default.mmcp.xml`
out of MakeMKV's `appdata.tar` from bytes (`ustar_entry`). `core::snapshot` (`crates/core/src/snapshot.rs`) is the
disc snapshot's pure part (`is_nav_file`, `unit_key_file`, `disc_meta` and the writer type `DiscMeta`).
`core::verify::subtitle_events_of` (`crates/core/src/verify.rs`) is the one subtitle-event rule over read inputs,
which dev's renderer and `retune --subtitles` share (S25).

## The Cargo.lock delta

On 2026-10-24 the owner approved the `Cargo.lock` delta of pull request 2: no external crate; five path packages,
`lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-engine` and `lazymkv-ctl`, each version 0.0.0 with no
`source` line; `lazymkv-capture` and `fake-tools` gain dependency names only. The lock's package count went from 38 to
43. `windows-sys` waits for pull request 3, and `ureq` with its TLS (Transport Layer Security) stack for pull request 4
(design parts 4.7, 12.6).

## Rulings

The plan of pull request 2 recorded the rulings S1 to S88: S1 to S54 in its Rulings section, under the group headings
below, and S55 to S88 in the tasks that settle them. Each task also recorded rulings of its own, numbered or not. The
pre-flight rulings PF-1 to PF-34 were made after a check of the plan, before execution, and the rulings made during
execution follow them. Each line says what was decided and, where it was recorded, why and the cost if wrong.

### The plan's rulings (S1 to S54)

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

### The task-level rulings S55 to S88

**Task 13** (the `makemkvcon` drivers)
- S55. The "one `makemkvcon` at a time" check runs inside every driver before it spawns. It compares `host.running_processes()` against `makemkvcon64`, `makemkvcon` and `makemkv` (MakeMKV's graphical program), lower-cased and with a trailing `.exe` removed. A match stops the call with the message `another MakeMKV process is running: <name as reported>`. `scan` and `rip` return `Failed { Unknown }` with that message. `list_drives` returns `Err` of kind `io::ErrorKind::ResourceBusy` with the same message, because `DriveListing` has no failure variant. Why: design part 5.3 fails the step as `Unknown`, naming the process.
- S56. A driver that kills its process sleeps `limits.after_kill` before it returns. This covers every kill: a time limit, the exit wait, a `LineAction::Kill` and a cancel. Why: design part 5.3 says no `makemkvcon` call may start within 10 s of a kill, and returning only after the wait guarantees it.
- S57. The exit wait (`limits.exit_wait`) starts at the call's final message, which the driver recognises by its line prefix only to start the clock: a drive listing ends at the first `MSG:5010,`, a scan at the first `TCOUNT:` after a `MSG:5011,`, and a rip at the first of `MSG:5004,`, `MSG:5005,`, `MSG:5036,` or `MSG:5037,`. Every verdict still comes from `RunSummary`. A run killed by the exit wait reports `run.timed_out == true`, and `run.cancelled` reflects only `ctx.cancel`. `scan` and `rip` tell the kills apart by the private `Kill` that `run_makemkv` returns, never by `run.timed_out`, so an exit-wait kill never reports as a scan past its limit, which the engine retries (RP-5). Why: design part 5.3 bounds a run only after its final message, and `run_recorded`'s `limit` bounds the whole run.
- S58. A retried scan writes `out` again. `scan` writes `out` from the start on every call, so the engine's retry (Task 23, RP-5) replaces the killed attempt's files, because design part 7.1 says `scan.txt` is the scan the plan was made from.
- S59. A licence message does not kill a rip. MakeMKV ends the run itself, because design part 5.6 only says the message "is fed as `LicenceMessage`".

**Task 14** (the `mkvmerge` and `ffprobe` drivers)
- S60. `remux_args` maps streams to track IDs through the staged report, and each layout track gets its per-track options in the layout's order. A `Layout` names scan streams, but the options of design part 5.7 step 13 need the raw file's mkvmerge track IDs, and only the staged file's `VerifyReport.pairs` maps one to the other, so `remux_args` takes `staged: &VerifyReport` and returns Err when `!staged.tracks_ok` or when a layout stream has no paired track; a stream's position in the selection would be wrong after any `MSG:4047` removal or a MakeMKV reordering. The options are `--language <id>:<language or und>`, `--track-name <id>:<name or empty>`, `--default-track-flag <id>:yes|no`, `--forced-display-flag <id>:yes|no`, `--hearing-impaired-flag <id>:yes|no` and `--original-flag <id>:yes|no`. After all tracks come `--audio-tracks <ids>` (or `--no-audio` when there are none), `--subtitle-tracks <ids>` (or `--no-subtitles`), `--no-chapters` only when `!layout.chapters_kept`, then `--track-order 0:<id>,...`, then `<raw>`. An empty name and `und` clear the names and languages MakeMKV wrote, because `verify_final` compares the absence of a name and the language through `norm_lang`. Design part 5.7 step 13 lists no `--video-tracks`, so every raw video track is kept.
- S61. `save_debug_log` copies bytes. It replaces each byte occurrence of a non-empty key, and copies the log unchanged when the key is `None`. A log whose modification time in Unix seconds is below `started_unix` is older than the call. Why: design part 7.2 keeps the log "with the registration key removed", and a log need not be UTF-8.
- S62. The debug-log tests use a private `Host` in `makemkv::tests`. Task 1 gives `lazymkv-tools` no dev-dependency on the platform fakes.

**Task 15** (the clients and the recorded transport)
- S63. Answers can be added to a live transport. `answer` and `fail` take `&self`, so a test adds answers to the transport the engine already holds (`Harness::http()` returns `&RecordedTransport`). In `http.json`, `"error":"timeout"` gives `Timeout` and any other error text gives `Unreachable(text)`. Each `{name}` in a URL is replaced from `vars`. An entry with neither `status` nor `error` is refused, naming its index. Why: design part 8.1 only says "answers keyed by URL".
- S64. Every request is logged when it arrives, before its gate is waited on. A test can therefore see a request in flight. Why: Task 23's `the_scan_does_not_wait_for_a_slow_lookup` needs that.
- S65. How a fetch maps to an outcome: `Unrecorded(url)` is `Pending { Unreachable("no recorded answer for <url>"), None }`, because `PendingReason` has no recorded-only variant and design part 8.3 makes every missing answer pending; a 401 is `TokenRefused` only for TMDb, and TheDiscDB's 401 is `Status(401)`, because TheDiscDB needs no credential (design part 8.1); a discid 200 holding `[]` is `Answered` with an empty list, as `parse_discid` reads it; a body that is not UTF-8 is `Unparsed("<url>: the body is not UTF-8")`.
- S66. Status words. `status_word` gives the HTTP status for `Answered` and `Miss`, `unparsed` for `Pending` with `Unparsed`, its status for `Pending` with a fetched answer, and `000` for `Pending` with no fetch, `NoToken` included. Why: S21 makes no answer `000`, and dev reads `000` as failed, so the lookup reads as pending (design part 8.3 makes a missing token pending).
- S67. `search_url` percent-encodes every UTF-8 byte except `A-Z a-z 0-9 - . _ ~`, as `%XX` in upper-case hex. There is no encoding crate (design part 4.6).
- S68. The slim shapes. `slim_tmdb` keeps phase 0's keys in phase 0's order (`id`, `imdb_id`, `title`, `original_title`, `release_date`, `name`, `original_name`, `first_air_date`, `original_language`, each when present and not null), then writes `external_ids` with `imdb_id`, `tvdb_id` and `wikidata_id`, each only when not null, `""`, `0` or `false` (PowerShell's `if`), and only when one remains; last comes `fetched_at` (scripts/phase0/record-disc.ps1:98-106). `slim_record` is `slim_tmdb`'s keys, then `kind`, then `runtime` when present and not null; a tv record then gets `seasons`, each holding `season_number`, `episode_count` when present and `episodes[]{episode_number, runtime}` when the raw season has `episodes`; `fetched_at` comes last. `slim_search` keeps only the `results` whose `media_type` is `movie` or `tv`, each keeping `media_type`, `id` and then the kind's `title`, `original_title` and `release_date` (movie) or `name`, `original_name` and `first_air_date` (tv), each when present and not null; the page then gets `"queries":[query]` and `fetched_at`. Why: design parts 7.1 and 8.4 list these fields, and P29 keeps no full answer.
- S69. The `lazymkv-clients` tests write their bodies inline and use a private scratch folder. Task 1 gives the crate no dev-dependency on `lazymkv-core/testsupport`. The one public TheDiscDB answer is read from `corpus/thediscdb-api`.

**Task 25** (restarts)
- S70 (the restart sequence). `start` finishes the restart sequence before it returns the handle: (1) load the jobs, reporting refused formats as in Task 21; (2) read the settings and `drives.json`; (3) fingerprint each disc that `DriveWatcher::present()` lists, using Task 22's arrival fingerprint code; (4) call `plan_restart`; (5) persist one `EngineRestart` transition per entry of `feeds`; (6) rebuild the books, the promises, the token and the slot; (7) hand the resume actions and then the arrivals to the owner thread. Why: a test that calls `restart()` can then read the `EngineRestart` lines at once. EngineRestart starts no job, so it is fed even while the settings have a problem, and the arrivals then wait as design part 9.4 says. Each `EngineRestart` transition goes through `start_until`'s stop hook as the owner's transitions do (Task 22), and a true answer stops the engine before any resume action runs.
- S71 (the token job's drive). The token job's own drive goes in `noted` and not in `arrivals`. Design part 5.4 excepts that disc from the arrivals.
- S72 (the file-phase slot). A job in Inspecting, Decided, Remuxing or Verified holds the slot, and so does a job in a waiting state whose `return_to` is one of those four states. Ejected jobs queue for the slot, and a waiting job whose `return_to` is Ejected holds no slot. Why: design part 5.7 says the slot is "held until Filed, NeedsReview, Failed or Discarded, including any wait entered after the grant".
- S73 (the rebuilt book). `rebuild_book` maps each `lookup-status.txt` line to a lookup by its first word, using S21's words: `dischash`, `discid`, `search`, and `tmdb` for `tmdb movie/<id>`. Last word `200` and the saved file parses gives `Answered { status: 200 }`, with the parsed answer in `answers` (the files are `thediscdb-hash.json`, `thediscdb-discid.json`, `tmdb-search.json` and `tmdb-record.json`; PR 2 is movie only, S19); `200` and a file that fails to parse gives `Pending { Unparsed(<the parser's sentence>) }`; `404` gives `Answered { status: 404 }`; `000` gives `Pending { Unreachable("no answer before the restart") }`; `unparsed` gives `Pending { Unparsed(<the parser's sentence on reading the file again>) }`; any other number gives `Pending { Status(n) }`; no line gives `NotAsked`. Every job fed EngineRestart gets a rebuilt book. Why: `resume` gives Scanned a Plan and AwaitingLookup a timer, and both read the book.
- S74 (reading Decide's inputs back). A Decide that finds no `DecideInputs` for (job, key) in memory reads them back with `read_decide_inputs`, taking the `Ctx` and `rec_folder("<absolute job dir>/<key>")` that Task 24's Decide uses, `text: None`, `selection: Some(Selection::All)`, and the same `PlanOptions` as Task 24's Decide, which carries the four replay paths, each only when its file exists. The Decide then writes `<key>: ` + `READ_BACK_LINE` to `job.log` and renders with `decide_doc` as Task 24 does. Why: design part 5.10 asks for dev's readers. In PR 2 there is no staged tuning folder, and the bare-`streamhash.txt` reading is PR 5.
- S75 (rebuilt promises). For each job where `held_promise(&record.state)` is `Some(step)`, the promise is rebuilt with `Promises::promise(job, step, volume, bytes)`, through Task 23's functions (S54): `Rip` is `volume_of(staging_root)` and `rip_promise(plan, meta.json total_bytes)`; `Remux` is `volume_of(movies_root)` and `remux_promise(<the staged .mkv sizes summed>)`.

**Task 26** (the IPC server and the self-test)
- S76 (refusal texts). The engine's refusal messages are private constants and functions in `ipc_server.rs`, with these `error.message` texts: an unknown job, `no job <id>`; `cancel` with no running step, `job <id> has no running step: it is <State::as_str()>`; `cancel` during CheckSpace or Eject, `job <id> is running <Action tag>, which cancel does not stop (design part 9.4)`; `review_action` approve outside the gate, `job <id> is <State::as_str()>, not held at the disposition gate`; another review action, or approve with non-empty overrides, `<action> waits for phase 3's review commands (R-A8)`; `discard`, `discard waits for phase 3's review commands (P2, R-A1)`; `ui_quit`, `ui_quit waits for phase 5's user interface (R-A8)`; `command`, `command needs the engine's run folder, which arrives in pull request 3 (S35)`; `shutdown` with steps running, `a step is running for <ids joined ", ">; stop --force cancels it (design part 3.8)`; an unknown secret name, `<name> is not a secret name (design part 9.4)`; `settings_set` with problems, `settings_set refused: <n> problems` (with `problems`); `settings_set` unreadable, `Settings::from_json`'s error text (with empty `problems`); `history` with an unknown `before`, `no job <id>`.
- S77 (snapshot problems and secrets). `snapshot.problems` is `Settings::validate()`. While `staging_root` is unset, it also lists `{"key":"staging_root","problem":"staging_root is unset: the engine starts no job until it is set (design part 9.4)"}`. Design part 9.4 says "the snapshot shows the problem". `snapshot.secrets` holds every `SECRET_NAMES` entry, set or not (`SecretStore::get` is Some), plus each `share:\\server\share` name a `secret_set` of this engine run has set, since `SecretStore` cannot list its names. `host` is `secret_host(value)` for `ha_webhook_url` and `webhook_url` when set, and None for every other name (design part 9.2).
- S78 (when replies are sent). A `cancel` reply `ok` is sent after the failure transition is persisted. `run_selftest` pushes `selftest_result` to every client, then replies `ok` with the `SelfTest` as `result` (design part 9.4). `settings_set` replies `ok` with the new settings serialised. `secret_set`, `retry`, `rip_again`, `review_action` and `shutdown` reply `ok` with `null`.
- S79 (the MakeMKV check run file). Each MakeMKV check outside a job is recorded at `<data>/selftest/drive-list.txt`, with its run files, and is overwritten by the next check. Every check, the job's listings and the check at engine start included, calls `MakeMkvState::observe` with `(summary.version, summary.licence.map(|m| m.code))` on the state `MakeMkvState::open(data)` gave at start. When `observe` returns true, every AwaitingMakeMkv job gets `MakeMkvChanged`, delivered once the token is free (design part 3.7). Because `recorded` is kept in `<data>/selftest/makemkv-check.json` (RP-9), the check at engine start compares against the last check before the restart (design part 5.10).
- S80 (self-test checks and versions). The checks come in this order: `makemkvcon`, `mkvmerge`, `ffprobe`, `ffmpeg`, `java`, `staging_root`, `tuning_root`, `movies_root`, `tmdb_token`. Each tool check carries the tool's path, version and where it was found (design part 9.2). `makemkvcon`'s version is the check's `MSG:1005`, and it is `ok` when it is found, its version is at least `makemkv_min_version`, and there is no licence message. The other four tools' versions come from `tool_version`, recorded at `<data>/selftest/<tool>-version.txt` with its run files; each is `ok` when found, and a version is reported, not judged, because spec 13's minimum versions of MKVToolNix and FFmpeg come with layer 2 in PR 5. A tool not found has `path`, `version` and `found` None, and the detail `<tool> was not found`; a found tool whose version cannot be read has the detail `<tool> printed no version`. The `java` check's detail also says `java in use at the last scan: <true|false>` from Task 23's `java_in_use`, or `no scan yet` (design part 9.4: "Java use comes from the last scan"). At engine start only the MakeMKV check runs (design part 3.7); until the first `run_selftest`, the snapshot's other eight checks have `ok` false, `busy` false, every optional field None and the detail `not checked since the engine started`, so a start sends no request and writes no probe file. While a RunScan, RipTitle or listing runs, the MakeMKV check returns `MakeMkvState.last` with `busy: true` and starts no run (design part 3.7).
- S81 (what a retry re-checks). A `retry` in a waiting state re-checks its condition: in AwaitingShare, `ShareConnector::reach` of the roots the return state uses gives `ShareBack`; in AwaitingSpace, the CheckSpace computation of Task 23, without feeding its event, gives `SpaceFreed`; in AwaitingMakeMkv, a fresh MakeMKV check, and `MakeMkvChanged` when `observe` says it changed; in AwaitingTool, `find_tool` of `makemkvcon` and `mkvmerge` (disc-phase return) or of `mkvmerge` and `ffprobe` (file-phase return), plus the `makemkvcon` version check, and when all pass, `ToolFound { tool: Makemkvcon }` for a disc-phase return and `ToolFound { tool: Mkvmerge }` for a file-phase return (the reducer accepts `ToolFound { .. }` for any tool); in AwaitingLookup, a lookup round (design part 8.3). A condition still not cleared feeds nothing, and the reply is still `ok`. After `run_selftest`, every AwaitingTool job whose re-check passes gets `ToolFound` (design part 9.4).
- S82 (approved reasons). `approve` feeds `ReviewApproved { overrides: Overrides::default(), approved_reasons }`, where `approved_reasons` is the job's `JobSummary.review_reasons` at the time of approval. That is the held decision's reasons as the snapshot showed them (P25), and PR 2 has no overrides (R-A8).
- S83 (forced shutdown). `shutdown { force: true }` cancels each running step as `cancel` does. It deletes each such job's files `<root>/.lazymkv-partial/<job id>-*.mkv.partial` in every library root, persists, replies `ok`, closes every client, then stops the owner thread. A CheckSpace or Eject in flight is let finish first (design part 3.8).
- S84 (job updates and collisions). After each persisted transition, the engine broadcasts `job_update`. `collisions` is `Some(record.collisions)` only when the job is in NeedsReview and the list is not empty. Otherwise it is `None`.

**Task 27** (`lazymkvctl`'s local commands)
- S85 (`Args::rest`). dev's `Args` gains `rest`. ctl must take out `--job`, `--jobs` and `--title` and pass every other word to dev's own parser. `positionals()` refuses undeclared flags such as `--settings`, so it cannot do this. Keeping one parser keeps argparse's prefix rule in one place.
- S86 (word routing). `run` routes the first word as follows: no word, `USAGE` goes to `err`, exit 2; `--help` or `-h`, `USAGE` goes to `out`, exit 0; a `LOCAL_COMMANDS` name, the local or engine form; any other `lazymkv_dev::COMMANDS` name, `lazymkvctl: <name> is a lazymkv-dev command, not a lazymkvctl command`, exit 2; anything else, `lazymkvctl: unknown command <name>` and then `USAGE` to `err`, exit 2. Task 28 adds the engine command names before this branch.
- S87 (forms and argument errors). For a local command, ctl calls `Args::new(rest, &["--job", "--jobs", "--title"])`, then `flag("--job")`, `switch("--jobs")`, `flag("--title")` and `rest()`. Each argument error prints `lazymkvctl <name>: error: <message>` to `err` and exits 2 (design part 9.6); the messages are `argument --title: only with --job`, `argument --title: required with decide --job`, `argument --title: not allowed with <name>`, `argument --job: not allowed with retune; use --jobs`, `argument --jobs: only with retune`, and an `Args` error text unchanged. With no `--job` and no `--jobs`, ctl returns `lazymkv_dev::dispatch(ctx, &[name] + words, out, err)`, so the documents, the exit codes and dev's stderr are identical (S37). The engine form, with `--job` or `--jobs`, goes to a private `engine_form`; for `retune --jobs` it first calls `retune_spec(ctx, words)`, and an Err is an argument error with that text, exit 2, whatever the engine's state (design part 9.6); then it writes `NO_TRANSPORT` and returns 4. Task 28 replaces its body with `relay`, which calls the same `retune_spec`, so the spec rule lives in one place.
- S88 (`USAGE`). `USAGE` is a private constant: `usage: lazymkvctl <command> [arguments]`, then, under "commands run here (design part 9.5):", one line each for `plan <slug|recording dir|scan file> [lazymkv-dev plan's flags]   or   plan --job ID`, `decide (<slug> <label> | --rec DIR) [lazymkv-dev decide's flags]   or   decide --job ID --title KEY` and `retune [lazymkv-dev retune's flags]   or   retune --jobs [--spec FILE]`.

### The other task-level rulings

**Task 1** (the workspace)
- Ruling: lazymkv-engine's dev-dependencies name only packages already in its `[dependencies]`, so the lock delta of Step 4 is unchanged, and every engine unit test compiles without `--features testkit` (Task 22 declares `#[cfg(any(test, feature = "testkit"))] pub mod testkit;`). Task 30's count then needs no feature flag.
- Ruling: until Tasks 21 and 27 replace them, the placeholder binaries exit 1 with a sentence. An empty `main` would exit 0, and a run of it could be mistaken for success.

**Task 2** (lower-case job IDs)
- Ruling: the lower-cased fingerprint is a constant `FP_LOWER`, checked against `FP.to_ascii_lowercase()` by `fp_lower_is_fp_in_lower_case`, rather than a `let` in each test. It reads the same in every `format!`.
- Ruling: the job lookup tests' expected IDs follow `earlier()` into lower case, because they compare against the IDs `earlier()` builds. The skeleton listed only `crates/core/src/job/tests.rs:2271-2316` and `:2409`. `job_lookup` itself does not change.

**Task 3** (the snapshot's pure part)
- Ruling: expected texts are arrays of lines joined with `"\n"`, never multi-line literals. The repository has no `.gitattributes`, so a Windows checkout with `core.autocrlf` could otherwise put `\r\n` into the source.
- Ruling: `core_snapshot_functions_agree_with_captures_output` pins core's three functions against capture's own code while both exist. Task 12 can then switch capture over with the equality already shown. The test is added to the skeleton's list, as is `a_dvd_has_format_dvd_and_no_disc_id_without_a_unit_key`, which covers the `"dvd"` spelling and the absent unit key that the Blu-ray tests do not reach.

**Task 4** (the ustar reader)
- Ruling: one copy of the hand-made profile, used by both profile's tests and the fixture `appdata.tar`.
- Ruling: the prefix field (offset 345) is not read, because the last path part always lies in the name field, so a match never needs the join.
- Ruling: the checksum is not checked, because no test or step depends on it and MakeMKV's own file is the only input.

**Task 5** (the line reducer)
- Ruling: the skeleton's names `5003_or_5004_with_5037_is_a_failed_title` and `2003_and_2019_set_read_and_write_errors` gain a `msg_`/`msgs_` prefix, because a Rust test name cannot start with a digit.
- Ruling: the guard's verdict is fixed at whichever of `5015` or `5014` comes first, as spec 6.2 step 6 names them ("`MSG:5015` with the job profile, else `MSG:5014`"). The reducer cannot know at a `5014` whether a `5015` will follow.
- Ruling: `title_failed` is any of `5003`, `5004` or `5037`, because spec 9.4 requires "none of 5003, 5004 or 5037" for success.
- Ruling: `version` is the bare `N.N.N` after `"MakeMKV v"`, so that `settings::version_triple` reads it against the 2.0.0 minimum.
- Ruling: the guard reads only lines before the verdict, so a `3307` after the saving message counts as missing (spec 6.2 step 6).

**Task 6** (the record types)
- Ruling: notes sort by `Note`'s own order, the declaration order that core's sorted fields already use, and titles sort by key. Sorting at serialise time keeps every writer's output in RP23's sorted form without each caller sorting.
- Ruling: the refusal texts are two sentences, each naming the expected format, so that `engine.log` can quote them (design part 7.4, "reports it").
- Ruling: `JobRecord` and `DrivesFile` have no `deny_unknown_fields`, so that a `v1.<minor>` file's added keys read, as `check_format` accepts minor versions.

**Task 7** (the IPC types)
- T7-a. Every `Option` field serialises as `null` and is never omitted. Spec 12 already writes `job_id` as null, and clients then read one shape.
- T7-b. An `error` reply always carries `problems`, an empty list unless a `settings_set` was refused. This gives one shape for `read_reply`.
- T7-c. A summary's `review_reasons` is `sorted_unique` over three sources: the plan's `reviews`, each title's `rip_reviews`, and each title's `decision.reviews`. `class` and `message` are Some only when `state` is `Failed`, because design part 9.3 gives them "for a failed job".
- T7-d. A summary's `title` is `plan.identity.title`, and None without a plan. It is the only title the record holds before a decision.
- T7-e. A `null` or absent payload reads as `{}`. A request with no payload fields, such as `run_selftest`, then needs nothing, and a missing required field still fails by name.
- T7-f. A share secret name is exactly `share:\\<server>\<share>`, with both parts non-empty and no further backslash, because design part 9.4 names a share and not a folder.
- T7-g. The error texts are the ones in the task's tests. Neither the design nor the spec gives any.
- T7-h. `reply_ok`, `reply_error` and `push_line` return the line without its `\n`, and the IPC writer appends one.
- T7-i. `shutdown`'s `force` is required, because design part 9.3 gives it as "true or false" and ctl always sends it.

**Task 8** (the platform traits and fakes)
- T8-a. `FakeShares` keys free space and reach by the exact root string. Volumes start as `Some(root)`, and `same_volume(a, b)` makes `volume(b)` return `volume(a)` (S11). The engine asks each root by its own string, so no path rule runs on Linux and macOS.
- T8-b. `present()` on the watcher and on the script reflects `arrive` and `remove` at once, sorted by drive id, before `next` hands the event out. The engine reads `present()` at start (L1), before any event is drained.
- T8-c. `with_removals` pushes `Removed` only for a successful eject. A failed eject leaves the disc in.
- T8-d. `KillHandle`'s field is `pub(crate)`, so only `platform` builds one (Task 9).
- T8-e. A `MemoryIpc` write after the other end's reader is gone fails with `ErrorKind::BrokenPipe`. A reader whose writer is gone reads end of file. `accept` returns `Err(BrokenPipe)` once every `MemoryConnector` is dropped. These are the ends a real pipe gives.
- T8-f. `DriveScript::remove` pushes `Removed` even for a drive that is not present. `arrive` on a present drive replaces its root and pushes `Arrived`. A script then says exactly what the test wrote.
- T8-g. `CountingKeepAwake::release` stops at zero.

**Task 9** (the std process runner and file helpers)
- T9-a. The child sits in an `Arc<Mutex<std::process::Child>>`. `wait` polls `try_wait` every 20 ms and releases the lock between polls, so a `KillHandle` used from another thread can always take the lock.
- T9-b. `KillHandle::kill` after the child has exited returns `Ok(())`, with `ErrorKind::InvalidInput` from std mapped to `Ok`. Cancel and timeout races then never surface as errors.
- T9-c. `write_atomic` creates no folders. A missing parent gives `ErrorKind::NotFound`, because each caller owns its folder layout.
- T9-d. `take_stdout` and `take_stderr` return the pipe the first time and `io::empty()` after that.
- T9-e. `StdProcessGuard` spawns with stdin null, stdout and stderr piped, and `env` added to the inherited environment. `below_normal` does nothing in PR 2 (S13, S43).

**Task 10** (the stand-in extensions)
- Ruling: a line with no timing record takes the time of the line before it in its own stream (0 for the first). The streams are merged by time, with stdout first when two times are equal. This makes "interleaved by its own timing" deterministic when one stream has no timing file.
- Ruling: `stop_after_line` and `hang_after_line` count stdout lines only. Stderr lines scheduled after the cut are not printed. A value at or above the line count prints everything, then stops or hangs.
- Ruling: a response that stops never runs its `creates`, because it stands for a run that ended without completing.
- Ruling: one response may not set both `stop_after_line` and `hang_after_line`. The stand-in exits 2 if it does.
- Ruling: with `arg_after` the stand-in does not create the target's folder, and exits 2 if that folder is missing. This makes the stand-in behave like `mkvmerge -o`, so the engine's creation of `.lazymkv-partial` is tested for real. The directory forms keep `create_dir_all`, as before.
- Ruling: the counter file is written on every run that matches, even when only one response matches. One rule is simpler than two, and tests always run in copied folders.

**Task 11** (the recorded run and the tool lookup)
- Ruling: the line given to `on_line` and `on_output` is the line with its trailing `\n` and a `\r` before it removed, decoded with `String::from_utf8_lossy` (PIPE-14). The file on disk keeps every byte.
- Ruling: `<name>.exit.json` writes `-1` when the process has no exit code. This is capture's `unwrap_or(-1)`, so every job folder keeps phase 0's integer shape.
- Ruling: `run_recorded` spawns before it creates any run file. A program that cannot start returns `Err` and leaves no `<name>` files, so a missing tool never looks like a run.
- Ruling: `find_tool` checks `is_file()` on the resources, install-folder and `PATH` candidates, and takes the settings path as given (design part 3.6).
- Ruling: tools has no dev-dependencies (Task 1), so its unit tests use `testutil::temp_dir` and a local `Host` implementation. Tests that need the stand-in or `FakeHost` live in `crates/fake-tools/tests/` (S12).

**Task 12** (capture over `tools::snapshot`)
- Ruling: `disc.rs` is deleted rather than left holding unused types, because every item in it has moved to core or tools. `eject.rs` stays in capture (design part 4.2).

**Task 16** (the lookup chain)
- Ruling: `pending()` treats an absent `Dischash` as "no round has started", so `(true, true)`. A Plan that runs before the lookup thread's first request then reads both lookups as pending, which design part 8.3 requires ("Planning never waits").
- Ruling: a record that answered 404 is asked again in the next round. The book keeps no identity for a miss, so this is the only way a later change of identity reaches its record, and rounds are few (design part 8.3).
- Ruling: the status word is `000` for `InFlight` and for `Pending` with `Timeout`, `Unreachable` or `NoToken`. It is `401` for `TokenRefused`, the code for `Status(code)`, and `unparsed` for `Unparsed`. dev reads every one of these as failed, so dev and the engine both see the lookup as pending (S21, S23).
- Ruling: in PR 2 a TheDiscDB entry with a title but no TMDb ID gets no entry-title search (S47). `record_to_fetch` then reads the name search's page as the entry's search, which is exactly how `identify` reads the same inputs at Plan, so the record and the plan stay consistent.
- Ruling: TheDiscDB bodies are saved as received for every `Fetched`, including a 404's empty body, as phase 0's `curl -o` did. A TMDb answer is saved only when `Answered`, so no error body is kept (P29).
- Ruling: saved JSON is `serde_json::to_vec_pretty` of the slim value plus a final `\n`. dev's readers accept any JSON layout.

**Task 17** (the characterisation of `decide`)
- Ruling: `scripts/box.sh` copies back only `Cargo.lock`, not other files as the skeleton says. Step 3 therefore fetches the generated folder with `scp` before the next `box.sh` call, which wipes `C:\lazymkv\src`.
- Ruling: the stored file's `\r\n` is read as `\n` before comparing. The repository has no `.gitattributes`, and `actions/checkout` on `windows-latest` checks text out with `core.autocrlf` on. decide's stdout never holds a `\r`, because JSON escapes it.
- Ruling: the scratch root is written as `<root>`, in both its plain and its JSON-escaped spelling, before writing or comparing, and each JSON-escaped Windows separator after it as `/`. The files are generated on the mini PC (Windows) only, so this keeps a printed path below the root from differing on ubuntu and macos. `Scratch::new` names a fresh folder per process and per call.
- Ruling: the expected exit code is the one `run` derives from the stored document: 0 when `reconcile.exit_code` is 0, else 1. The files thus pin it without a second file.

**Task 18** (`subtitle_events_of`)
- Ruling: a mapped entry with no `packets` entry gives `"sub-<n>.packets.json: not read"`. `subtitle_events` reads every mapped entry, so the case arises only from a caller that built the map itself, and the text names the file the caller left out.

**Task 19** (the K5 split)
- Ruling (S24): the reader runs `reconcile`, and `decide_doc` cannot fail, as K5 writes it.
- Ruling: `decided_with` keeps its closure type `FnOnce(&mut PlanDoc, &mut DecideDoc)` and calls `decision_section(&inputs, &plan, &doc)` directly. Its twenty closures and `with_text` then stay unchanged.
- Ruling: the reader reads the packets file of every index entry, not only the mapped ones. The verify report is the renderer's (S25), and errors are held per entry, so a broken unmapped file stays silent as before.
- Ruling: `EngineDecideDoc` is `Serialize` only. No reader reads it back as that type.

**Task 20** (decide's replay flags)
- Ruling: the root check runs before the recording is read, so an argument error is reported before any recording error, as argparse's are.
- Ruling: when both roots fail, `movies_root` is named, following spec 14's key order, which `validate` keeps.
- Ruling: `option_text` and `parse_record` become `pub(crate)` rather than being copied, so there is one reading of each file.

**Task 21** (the job store and the engine binary)
- Ruling (S28): the PR 2 binary exits 1 on every OS with its OS's sentence, so the CI whole-job tests drive the library.
- Ruling: `Store::new` takes `now` as a closure and the console as an optional writer, so the store needs no `Host` and its tests need no fake. The engine passes `move || host.now_unix()`.
- Ruling: an engine.log line is `<Unix seconds> <text>`. Design part 4.6 writes new times as Unix seconds, and part 3.5 names no line format.
- Ruling: the refusal is checked at each write by re-reading the existing file's `format`, so a refused file is never overwritten without the store holding any state.
- Ruling: a job folder with no `state.json` is skipped by `load_jobs`. S29 persists `Detected` first, so such a folder holds no job, and its disc's next arrival rewrites it.
- Ruling: an unreadable data-folder `settings.json` gives the defaults with one problem keyed `settings.json`. Any problem stops new jobs (design part 7.4), and the snapshot then names the file.
- Ruling: the store's tests use a temporary folder of their own in `mod tests`, so they run with no feature and no dev-dependency. The binary tests live in `crates/engine/tests/binary.rs`, the only place `CARGO_BIN_EXE_lazymkv-engine` exists, as Task 3 does for capture.

**Task 22** (the job owner and the harness)
- R22.1. The harness's helpers are the interface sheet's list; every method takes `&self`, so tests write `let h`. `insert`, `insert_root` and `remove` only queue their drive event in the harness, and every other Harness method first hands the queued events to the `DriveScript`, in order. So `h.insert(..); h.run_until(state)` arms the stop hook before the arrival reaches the engine, and no transition into `state` can pass unseen.
- R22.2. `StepSeen` and `ScriptedSteps::seen` are added. The persistence-order test needs to see the files at the moment of each call.
- R22.3. The record actions land in the transition's own `state.json` write. They are pure edits, so the state is still persisted before any action runs (design part 7.3).
- R22.4. KeepAwake's "active" is "a step runs or the token is held"; Task 24 adds "or a slot is held".
- R22.5. The harness's `tool_paths` setting is replaced one level deep, each key inside it replacing the default, so a test can unset one tool.
- R22.6. The harness's ejector is `RecordingEjector::new().with_removals(script)`, because a real drive reports the removal its eject causes.

**Task 23** (the disc phase)
- R23.1. Plan reads the book with `try_lock`. A book held by a running round counts as an empty book, so every lookup is pending (design part 8.3: planning never waits; `run_round` holds the book for a whole round).
- R23.2. Stand-in runs are faster than the recorded lookups' file writes, so `Harness::new` uses a `TestGuard` around `StdProcessGuard`: before spawning a scan (arguments holding `info` and a `--profile=` argument), it waits until `<the profile's folder>/lookup-status.txt` has one line per distinct URL of the merged `http.json` and no line ends in `000`, for at most 5 s. This keeps the engine's "never waits" planning unchanged while making the whole-job tests deterministic. It relies on Task 16's `run_round` applying each outcome to the book before it saves the status file.
- R23.3. The stand-in's stdout files are named `ident-raw.json` and `partial-ident.json`, not `mkvmerge.json`. That name is the stand-in's own scenario file (Task 10 reads `<stem>.json`). The job folder's recorded names are unchanged.
- R23.4. `rip-summary.json` is written by VerifyFile in Ripping, before it verifies, as design part 7.1's table says ("spec 6.2 step 7"). It takes the run's numbers from `<key>/rip.txt.exit.json` and `rip_index` from the title record that RipTitle edited, so a restart between the rip and the verification loses nothing.
- R23.5. One variant is added, `movie-direct-verify-twice`, because the listed `movie-direct-rerip` cannot fail twice. The below-minimum-version test uses `makemkv_min_version: "2.0.1"` rather than a variant.
- R23.6. The out-of-space threshold is 1 MiB of free staging space, because design part 5.6 gives no number.
- R23.7. `rip-summary.json` has no `started` key in PR 2. Design part 4.6 formats it from `platform`'s local time, as phase 0 did (`yyyy-MM-dd HH:mm:ss.fff`, scripts/phase0/backup-disc.ps1:63), but the PR 2 `Host` has only `local_date`, and design part 7.1 writes only the phase 0 keys the engine measures. PR 3 adds the local time with the Windows implementations and writes `started`.

**Task 24** (the file phase)
- R24.1. A lookup round runs only for the head of the slot queue, and the slot is not granted while that round runs. This keeps "oldest first" exact (design part 5.7).
- R24.2. `RefreshIdentity` is applied by the owner to the transition's own record before `state.json` is written, as R22.3 does for the other record actions.
- R24.3. The engine takes each title's `FileDecision` from its rendered `DecideDoc`. So `decide.json` and the decision fed are one value (S24, S26).
- R24.4. Six lookup variants are added. Each holds only `http.json`, because the listed variants cannot leave a lookup pending.
- R24.5. The edition facts of design part 6.4 come from one `read_dir` of `<root>/<F>`, where `F = path.components[0]`, the film folder the namer built. A file named `<F> - <R>.mkv` or `<F> - <R> <part>.mkv`, with `R` one of `resolution_label`'s values (`2160p`, `1080p`, `720p`, `576p`, `480p`), is one the namer builds for this film: with no part, it is a copy filed without an edition name, the theatrical or main cut (spec 8.4), giving `earlier_edition_names["theatrical"] = ""`; with `edition_style` Jellyfin a part `E`, and with Combined a part `E {edition-E}`, is an edition, where `E` is not empty, is not ASCII digits only (that is a copy number, spec 11's keep both) and equals `edition_label(E)`, which sets `folder_has_edition_named_file` and gives `earlier_edition_names["theatrical"] = E` when `E` is `THEATRICAL_CUT`, else `earlier_edition_names[E.to_lowercase()] = E`; with Plex, an edition's files live in their own folder, so no part is read; any other name is ignored, and the first name in byte order wins a key. This is the namer's own pattern, read back, so no rule beyond design part 6.4 is added.
- R24.6. The reasons of `waiting_message` are `no tmdb_token is set` (NoToken), `TMDb refused the token (HTTP 401)` (TokenRefused), `no answer within 10 s` (Timeout), the error text (Unreachable), `HTTP <n>` (Status) and the parser's sentence (Unparsed). A round that leaves a lookup `Unparsed` also writes the `job.log` line `lookup <word>: <the parser's sentence>` (design part 8.3).

**Task 26** (the IPC server and the self-test)
- Notifier (spec 12): a `failed` notification whose class is `MediaReadError` gets `"; try the other drive"` appended to the message Task 22 builds.

**Task 28** (`lazymkvctl`'s engine commands)
- T28-a. `status`, `settings get`, `history` and `selftest` print their JSON value with `dumps(&value, Style::INDENT1_ASCII)` and a newline. Every other `ok` prints nothing. Why: `lazymkvctl` then has one JSON style, the one `plan` and `decide` print.
- T28-b. `watch` prints each line exactly as received, without its line ending, followed by `\n`. It exits 0 when the connection closes. Why: a field that a newer engine adds is not lost.
- T28-c. Each run opens one connection and sends at most one request, with `id` 1. Why: one `lazymkvctl` run is one command.
- T28-d. A connection that closes before the reply, or a line `read_reply` refuses, exits 4. The message is `CLOSED` in the first case and `lazymkvctl: <read_reply's message>` in the second. Why: no engine answered, and exit 1 must stay out of `plan`'s codes (RP24).
- T28-e. Error replies and usage errors use ctl's own program name. An `error` reply prints `lazymkvctl: <message>`, then one line `lazymkvctl: <key>: <problem>` per problem; a usage error prints `usage: lazymkvctl <synopsis>` and `lazymkvctl <name>: error: <message>`, then exits 2. Why: these follow argparse's shape. S37 covers only the commands dev runs.
- T28-f. The `secret set` value is the whole of standard input with one trailing `\n` or `\r\n` removed. An empty value is an argument error. Why: a share credential is two lines (design part 9.4).
- T28-g. `relay` sends argv unchanged as `words`. For `retune`, it then appends `--spec <absolute path>`. Why: the engine builds the child's words (design part 9.5), and `Args::flag` lets the last `--spec` win, so the user's words never need rewriting.
- T28-h. Argument checks, the settings file read and the standard-input read all happen before connecting. Why: an argument error exits 2 whatever the engine's state.
- T28-i. `selftest` prints the payload of the last `selftest_result` push received before the `ok`. Without one it prints `NO_SELFTEST` and exits 1. Why: the engine pushes the result and then replies (design part 9.4).
- T28-j. The `watch` test runs in ctl against a scripted peer, not against `Harness::client()`. Why: only a scripted peer controls when the connection closes.

**Task 29** (the whole job)
- T29-a. The test waits by polling `notifications()` until one that is not `disc_done` arrives, and then until `awake().holders() == 0`. It does not use `run_until`. Why: `run_until` stops after the transition into the state is persisted, which is before that transition's actions run (Task 22), so the `filed` notification and the keep-awake release would be missed.
- T29-b. The job folder's later drive listings follow RP-1 (a) (Task 23), through the constant `LATER_LISTINGS`, which is `&["drive-list-2.txt"]`, the listing before the rip. Why: assertion 6 lists the folder exactly.
- T29-c. Assertion 8 lists the data folder exactly, `selftest/` included: Task 26 records the MakeMKV check at engine start there (S79), with the recorded state of RP-9. No self-test runs in this test, so no tool's version file is there.

### The pre-flight rulings (PF-1 to PF-34)

A pre-flight check of the plan's tasks against each other (2026-10-24) found 34 confirmed findings; each ruling below
settles one.

- PF-1 (Tasks 22, 26). Task 26 adds `pub fn deliver(&self)` to `Harness`: it only hands the queued drive events to the `DriveScript`. Every `engine_ipc.rs` test that inserts or removes a disc and then waits only through the IPC `Client` calls `h.deliver()` right after that insert or remove. In `a_retry_with_the_disc_absent_feeds_media_removed_and_frees_the_token`, after `h.remove("tok0"); h.deliver();`, the test waits with `h.wait` until `drives_json()["drives"]["tok0"]["fingerprint"]` is null before sending `retry`. In `a_retry_queues_behind_an_earlier_arrival`, after `h.insert("tok2", DISC_C); h.deliver();`, it waits until that drive's recorded fingerprint equals `h.fingerprint(DISC_C)` with `handled` false before sending `retry`. R22.1 stands. Why: R22.1 queues drive events until the next Harness call, and the IPC Client never touches the harness, so 28 Task 26 tests would time out. Cost if wrong: a few test-only lines; if `deliver` is wrong a test hangs visibly.
- PF-2 (Tasks 10, 22, 23, 25, 28). Task 22's testkit creates the harness root under `std::env::temp_dir()` with a name that contains neither `mkv` nor `lazymkv` (`engine-harness-<pid>-<n>`, removed on drop) and documents why. Task 28's `secret_set_reads_the_value_from_stdin` uses a secret value that cannot occur in a path (for example `hook-secret-7f3a`) and asserts the status lacks that value. Why: the stand-in matches needles by substring, so a root containing `mkv` makes the rip needle match every scan's `--profile=<root>/.../job.mmcp.xml`, and a second scan in one tools folder would replay `rip.txt`. Cost if wrong: one naming rule in test code.
- PF-3 (Tasks 27, 28). Task 28's `relay` checks `retune_spec` before connecting and prints its Err as Task 27's argument error, `lazymkvctl retune: error: <text>`, exit 2, with no usage line. `Args` errors (an ambiguous prefix) take the same form. `RETUNE_RELAY_USAGE` is not defined unless some other relay usage error needs it (none is known). Why: Task 27's `title_without_job_is_an_argument_error` pins the exact stderr, and a leading usage line would break it. Cost if wrong: one stderr form.
- PF-4 (Tasks 22, 25). In Task 25's `awaiting_lookup_recreates_its_timer`, `h.remove("tok0")` is called right before `h.restart()` (it stands for the removal a real eject causes, R22.6), and the `EngineRestart { disc_present: false }` assertion and Step 4's rule stay. Why: on the scripted harness the scripted Eject never calls the ejector, so the disc is still present at restart and the feed would be `true`. Cost if wrong: one test line.
- PF-5 (Tasks 22, 23). In Task 23's `a_scan_past_its_limit_is_killed_listed_again_and_retried_once`, `let id = run_to(&h, "Ejected")` is replaced with `h.insert("tok0", "movie-direct"); h.run_until("Ejected");` (the 60 s cap), and the id is computed as `run_to` does. The note says the test takes more than 10 s. Why: `Harness::wait` panics after 10 s, but the scan's kill alone comes 10 s after the spawn, after the arrival work. Cost if wrong: one test's waiting form.
- PF-6 (Tasks 22, 24, 30). `ExtractSubtitleEvidence` returns `event: None` with no `returned no event` line (design part 5.2: it runs with `Decide` as one unit). The interface sheet's list of no-event actions reads `EmptyTitleDirectory`, `DeletePartialFile` and `ExtractSubtitleEvidence`. Why: Tasks 22 and 24 and the design agree; only the sheet's list was short. Cost if wrong: none.
- PF-7 (Tasks 23, 24). Every Task 23 and 24 test that asserts `h.notifications()` or `h.timers().pending()` after `run_to` calls `h.settle()` first, as `the_eject_fake_failure...` already does: in Task 23 `a_copy_plan_fails...`, `short_staging_space_waits_in_awaiting_space`, `no_makemkvcon_waits_in_awaiting_tool` and `a_scan_licence_message_waits_in_awaiting_makemkv`, and in Task 24 `still_pending_after_the_round...`, `a_refused_token_waits...`, `filed_records_path_size_mtime_and_filed_unix` and `no_ffprobe_lets_the_disc_phase_finish...`. Why: `run_to` returns on the transitions line, which the owner writes before running the transition's Notify and StartTimer actions. Cost if wrong: one call per test.
- PF-8 (Task 26). In the Lookup part of `retry_in_each_waiting_state_rechecks_its_condition`, following S81, the test records the transition count before the retry; after `c.ok("retry", ..)` it waits with `h.wait` until `h.http().requests().len() > asked`, then calls `h.settle()`, then asserts the transition count is unchanged and the state is still AwaitingLookup. The `last.old` assertion is dropped. Why: with an empty RecordedTransport every lookup stays pending and S81 feeds nothing, so the written assertion failed deterministically and the request count read raced. Cost if wrong: one test.
- PF-9 (Task 26). In `a_client_that_falls_behind_is_dropped_without_slowing_the_engine`, `io::sink()` is replaced with a writer that counts the lines it receives (newline bytes, not `write` calls). The test broadcasts `CLIENT_QUEUE` lines, waits with a bounded deadline until that client has taken all of them, then broadcasts 10 more and asserts `count() == 1`, timing only the `broadcast` calls for the under-1-second assertion. The stuck client stays, and its gate is released at the end. Why: as written the sink client could be dropped too when its drain thread lagged, making the count 0 on a busy runner. Cost if wrong: one test.
- PF-10 (Task 17). Task 17 Step 3 runs `mkdir -p crates/dev/src/cmd/decide` before the `scp -r`, still before any other `box.sh` call. Why: without the folder, scp copies the remote folder as `decide/`, and the JSON files land one level too high. Cost if wrong: none.
- PF-11 (Tasks 1, 30). Task 1's implementer runs Steps 1 to 4 and Step 6 (neither `--offline` nor `--locked` can fetch) and stops with nothing committed, writing the full `git diff Cargo.lock` and the Step 4 counts to a file. The controller shows the diff to the owner with Step 5's sentence, waits for an explicit yes in chat, records that yes's date for Task 30, then has the implementer run Step 7. Why: the implementer cannot obtain the owner's yes (P33), and the plan assigned the relay to nobody. Cost if wrong: none; the owner still gates the commit.
- PF-12 (Task 4). The ustar reader checks `offset >= len` (Ok(None)), then `offset + 512 > len` (the truncated error), and only then reads the 512-byte block for the zero-block test. Why: the written order could slice past the end of a truncated archive and panic instead of returning the pinned error. Cost if wrong: none.
- PF-13 (Tasks 5, 11, 27). Docs are completed where the dictated code lacks them: Task 5's `LineReader::new`, `feed` and `finish` name design part 5.3 and end "Def: T5. Use: T13."; Task 11's moved items (`Clock`, `RealClock`, `RealClock::start`, `with_suffix`, `tee_lines`) keep their bodies and gain docs naming design part 7.2, ending "Def: T11. Use: T13, T14, T23."; Task 27's `NOT_RUNNING`, `REFUSED` and `NOT_ELEVATED` name design part 9.6 (exit 4's messages). Why: every `pub` item has a `///` doc naming its design part and ending Def/Use. Cost if wrong: none.
- PF-14 (Task 10). `Response` and `Create` keep `#[serde(default)]` on `exit_code`, `creates`, `stdout` and `timing`, as the existing scenario.rs has them. Lines are split with `split_inclusive(|byte| *byte == b'\n')`. Why: read literally, the brief dropped the defaults, and `split_inclusive(b'\n')` does not compile. Cost if wrong: none.
- PF-15 (Task 10). `a_hanging_run_sleeps_until_killed` reads the first stdout line with a blocking `BufReader::read_line` and asserts it equals `MSG:5015,0,0,"Saving 1 titles"\n`, sleeps about 100 to 200 ms, asserts `try_wait()` is None, kills and waits, then asserts the rest of the pipe is empty. Why: a fixed 500 ms wait before reading is racy on a slow first start of a freshly copied executable (an on-access scan). Cost if wrong: one test.
- PF-16 (Tasks 11, 16, 22, 25). Expected compile-error lists in red steps are examples: the run must fail to compile for the missing items, whichever the compiler names first. Task 16 adds the `pub use chain::{..}` line in Step 7, not Step 5. Task 22 declares its modules in Step 1, with `#[cfg(any(test, feature = "testkit"))] pub mod testkit;` and the owner's `mod tests;`, as Tasks 21, 23, 24 and 25 do. Why: some listed errors named items the named file never imports, and Task 22's compile error could not occur as written. Cost if wrong: none.
- PF-17 (Tasks 15, 16). Task 15 defines `status_word` as `#[cfg(test)] pub(crate) fn status_word<T>(..)`, with a comment saying its callers are its own tests. Task 16's chain writes the status words from `LookupState`, as its ruling says, and never calls `status_word`. Why: otherwise the non-test build warns that `status_word` is never used, from Task 15's commit on. Cost if wrong: none.
- PF-18 (Task 16). In `run_round` item 4, the `tmdb movie/<id>` status line is keyed by `answers.record`'s identity while `Record` is `Answered` with a record, and by `record_to_fetch` over the book otherwise (InFlight, Pending, NotAsked). The tests' final status lines are unchanged. Why: in the late-TheDiscDB scenario the status saved before the refetch would pair the new key with the old fetch's state. Cost if wrong: one status line in one scenario.
- PF-19 (Task 16). In `the_status_line_is_000_while_a_request_is_in_flight`, a drop guard holding a clone of the `Gate` opens it on drop (or the observations are copied out and the gate opened before any assertion, the deadline assertion included); the round thread then always finishes, and a failing assertion fails instead of hanging the scope. Why: `Gate` has no timeout, and `thread::scope` joins the blocked round thread after an unwinding assertion. Cost if wrong: one test.
- PF-20 (Tasks 18, 19). Task 18 writes `subtitle_events`'s doc ending "Def: phase 1, rewritten over `subtitle_events_of` in T18. Use: `subtitle::labels::row14` (retune --subtitles)." Why: Task 19 deletes decide's call, and the function predates T18. Cost if wrong: none.
- PF-21 (Tasks 19, 20, 27, 29). Line citations are located by content, not number: Task 19's reviews build is decide.rs:253-261 and the decision set :277-284; Task 20's `recorded_answers` is plan.rs:280, and HELP closes at decide.rs:85; Task 27's `Args` doc starts at lib.rs:82; Task 29's `is_nav_file` is `lazymkv_core::snapshot::is_nav_file` in crates/core/src/snapshot.rs (Task 12 deletes capture's disc.rs). Why: several citations were off by a few lines, and one named a deleted file. Cost if wrong: none.
- PF-22 (Tasks 22, 24). `Msg` in owner.rs is `pub(crate) enum Msg` (its variants, Task 24's `RoundDone` included, inherit that), and arrival.rs's `spawn_arrival` and `spawn_recording` take `Sender<Msg>`. Why: a Rust-private enum cannot be named from the sibling module. Cost if wrong: none.
- PF-23 (Tasks 22, 26). Task 22's `a_second_drives_disc_queues_and_starts_at_the_first_jobs_ejected` matches with `let Some(Waiting::Arrival(found)) = next else { panic!(..) }; assert_eq!(found.drive, "tok1");`, which keeps compiling when Task 26 adds `Waiting` variants. Why: an exhaustive match over `Waiting` would break (E0004) when Task 26 grows the enum, and Task 26's files omit owner/tests.rs. Cost if wrong: none.
- PF-24 (Task 23). Task 23 Step 4's two "the message above" references mean the tests' texts: `the index guard found <key> moved three times` (4.7.4) and `<tool file stem> is version <found>, below makemkv_min_version <setting>` (4.4.6). Why: the referents were missing from the step text. Cost if wrong: none.
- PF-25 (Task 23). In clone_values.rs only the `for (whence, value) in values.values() { .. }` loop moves into `fn holders(..) -> Vec<String>`; each test keeps its own `clone_values` call, its non-empty assertion and its final assertion with its own wording. Why: the brief named a wider range than must move. Cost if wrong: none.
- PF-26 (Tasks 24, 25). RP-7's VerifyFile-in-Remuxing change (set `TitleRecord.filed` to the final path and the partial's size, mtime 0) is Task 25's, made in its Step 4 and covered by its steps.rs entry. Task 24 does not make it; its File step records `FiledFile` itself. Why: RP-7's text misattributed the change to Task 24. Cost if wrong: none.
- PF-27 (Tasks 24, 25). S74's read-back runs when `RealSteps` holds no in-memory probe results for (job, key), and it applies RP-4: a `PlanEntry::Results` in the read-back inputs has its `.plan` replaced by the job record's persisted plan before `decide_doc`, as `engine_decide_inputs` does; a `Skipped` plan stays as read. Why: Task 24 holds no `DecideInputs` per (job, key), and without RP-4 a Decide after a restart would decide from the folder's fresh plan. Cost if wrong: none.
- PF-28 (Task 26). `start` runs the MakeMKV check (`find_tool`, the drive listing into `<data>/selftest/drive-list.txt`, `MakeMkvState::observe`) to completion before the IPC server accepts its first client. Why: two tests read the check from the first snapshot. Cost if wrong: start takes one drive listing longer.
- PF-29 (Task 26). During a forced shutdown, after the let-finish step's event is fed and persisted (with its Notify actions), the owner starts nothing new: no slot grant or FilePhaseSlot feed, no disc-phase token hand-off, no queued delivery and no new step. It then replies `ok`, closes the clients and stops. Why: otherwise a (FilePhaseSlot, Inspecting) transition breaks the shutdown test's last assertion, and a Probe starts during shutdown. Cost if wrong: none.
- PF-30 (Task 14). Task 14's three debug-log tests use `crate::testutil::temp_dir("debug-key" | "debug-old" | "debug-missing")` and define no `scratch`. Why: Task 11's helper exists for every tools unit test. Cost if wrong: none.
- PF-31 (Task 26). engine_ipc.rs uses `h.job_ids()`, `h.transitions(&id)` and `h.record(&id)` and defines no free `job_ids`, `transitions` or `record`; the then-unused `Transition` import is dropped. Why: Task 22's Harness already offers them and reads the same files, and the free `job_ids` would also list folders without `state.json`. Cost if wrong: none.
- PF-32 (Tasks 27, 30). Abbreviations of `--job`, `--jobs` and `--title` belong to ctl (so `--t` is `--title`, not dev's `--text`); accepted for PR 2 and listed under "Left for later pull requests" below. Why: ctl's parser declares those options, and the divergence is only in abbreviated spellings. Cost if wrong: a user typing `--t` for `--text` sees exit 2.
- PF-33 (Task 28). Task 28 appends a "commands sent to the engine (design part 9.6):" section to Task 27's `USAGE`, one indented line per `ENGINE_COMMANDS` synopsis. Task 27's tests check a prefix and one substring, so they still pass. Why: otherwise `lazymkvctl --help` would list only plan, decide and retune. Cost if wrong: none.
- PF-34 (Task 28). In Task 28's `peer` helper, the closure is written `let send = |..|`, with no `mut`. Why: the closure captures nothing, and `let mut` warns. Cost if wrong: none.

### Rulings made during execution

The controller of the plan's execution made these while the tasks ran, mostly on the reviews of each task.

**Task 4**
- The review's Minor 1 is promoted to one fix round: ustar's tests gain `a_longer_earlier_entry_is_skipped_to_the_next_header`. Why: a dropped 512-byte rounding of a non-empty skipped entry would pass every test of the brief, and the rounding matters in production, where the profile is rarely the first entry. Cost if wrong: one extra test.

**Task 7**
- T7-c1. A `settings_set` whose payload is null or absent is refused with "the settings_set payload is not an object". Why: the brief's step 6 says `settings_set` takes an object as changes, and only every other type reads null as `{}`; `lazymkvctl` always sends an object. Cost if wrong: a one-line change to read null as no changes.
- T7-c2. A payload that is neither null nor an object is refused, on the other nine types that take a payload, with "the <type> payload is not an object", since serde would otherwise accept arrays positionally. Cost if wrong: none.
- T7-c3. A malformed `problems` list in an error reply is an Err from `read_reply`, not silently dropped. Cost if wrong: none.
- T7-c4. `Push` carries `#[allow(clippy::large_enum_variant)]` with a reason comment, since boxing would change the variants the interface sheet fixes. Cost if wrong: none.

**Task 8**
- The review's two documentation Minors (the "sorted" promise of `DriveWatcher::present` belongs to the fake; the `same_volume` doc read as a live link) ride along in the documentation fix round of its two Importants. Why: same files, same kind of edit, one commit. Cost if wrong: none.

**Task 9**
- Hard links on the network-attached storage (NAS) share need no probe in PR 2. Why: PR 2 never writes to the NAS (stand-ins only), and PR 3 replaces the rename with `MoveFileExW` before any share run. Cost if wrong: none in PR 2.

**Task 11**
- The fix round for the review's Important (a panic in `on_line` or `on_output` left `done` unset, so `run_recorded` hung with the tool running) also takes the two robustness Minors of the same function (a file error after the spawn left the child running; a stderr read error, or a failed kill after a stdout tee error, could stall) and corrects the `after_kill` doc: it is S56's pause after any kill, for Task 13 to use. Why: one drop guard that kills the child and sets `done` on any early exit closes all three, in one function and one commit. Cost if wrong: a few lines.

**Task 13**
- T13-c1. `a_scan_past_its_limit_is_killed_and_fails_as_unknown` uses a `limits.scan` of 5 s instead of the brief's 300 ms. Why: a freshly copied stand-in took 0.28 to 1.25 s to start under the parallel Windows run and was killed before it wrote its counter, so the test flaked although the verdict was right. Cost if wrong: one test takes about 5 s.
- T13-c2. The driver also sleeps `after_kill` when `run_recorded` returns Err, because the recorder kills on its error paths without saying so (S56 kept). Cost if wrong: a spawn failure is reported `after_kill` later (10 s in production).
- T13-c3. `Kill::Cancel` means the recorder killed for the cancel; a late cancel gives `Kill::None`, and the summary decides.
- T13-c4. A rip refused by S55 returns an empty `RunResult`.
- T13-c5. `drive_index` keeps the exact match of the drive ID (Review Focus 2).
- Carried to Task 23: it may make `SCAN_PAST_LIMIT` public (a one-line visibility change in tools) instead of copying the string; the rip needles add a rip-only token (the title folder `t00-00800`) on top of PF-2's root naming; after a Done scan it checks its own cancel token.

**Task 16**
- T16-r1. Two divergences between the engine's lookup book and dev's readers are accepted for PR 2, under design part 12.7 and RP-4 (differences in a lookup-pending job are accepted, and `compare-job` in PR 6 explains them): (a) a dischash 200 whose body parses as no disc is `Miss` in the book (S46, which the design binds) and is recorded truthfully as `dischash 200` with its body, although dev's `hit_rate::lookup` reads that as failed or pending; (b) a search left pending writes `search 000`, which dev's plan reader does not read, so dev sees TMDb as not pending while the book says pending. PR 4 aligns dev's lookup readers with S46 and teaches them the `search` word. Why: changing dev's plan reader is outside PR 2's files, and both cases are rare (a 200 body that is not a disc; a job that reaches Plan with the search still pending). Cost if wrong: a dev `plan` over such a job folder shows a different `lookup_pending` than the engine's plan until PR 4.

**Task 21**
- T21-c1. Two extra store tests (RP-8, and the load order of titles and notes) are kept; Task 30 counts from a run.
- T21-c2. RP-8's newline repair in `Store::persist` landed in Task 21, so Task 25's store edit for RP-8 had nothing left to do, and its harness test passed on the first run.
- T21-c3. Format refusals are prefixed with the file's path, like the store's other errors.
- T21-c4. A missing `--data` is reported before leftover words, and a jobs folder that cannot be listed is one Err.
- T21-c5. A private write lock makes `Store` the one writer per path.

**Task 22**
- R22.7 (design over plan). MediaArrived goes only to a disc-phase job with `job.disc == None` (or Failed with `MediaReadError`); a repeat arrival for a job whose disc is present feeds nothing, logs `<drive>: the disc of job <id> is already in <drive>` and records `handled: true`. Why: the brief's item 4 dropped design part 5.4 step 3's "waits for its disc", and could start a second concurrent rip. Cost if wrong: a second copy of a disc in another drive is ignored while the first is ripping.
- R22.8. A same-state transition into Failed or a waiting state whose event brings no message keeps the old message; Tasks 24 to 26 assert against it.
- R22.9. `Harness::restart` hands queued drive events over after the old engine stops (amends R22.1 for restart).
- R22.10. `stop` does not join arrival threads still waiting for a volume; they only read, and a late send hits a closed channel. It is documented as the exception.
- R22.11. A step never posts after its `CancelToken` is raised, because the outbox is keyed by job only.
- The review's Minor 7 (a failed first persist left an unpersisted Detected job in memory with `handled: true`) joins the fix round. Why: the fix touches the same failure path.

**Task 23**
- `ChainInputs` carrying the remembered identity would change the clients interface outside PR 2's remaining files, so it is parked for the final review and PR 4. Why: no PR 2 scenario carries a remembered identity, and P11 holds because dev reads the same files. Cost if wrong: a disc whose remembered identity differs from the search's top result plans without a TMDb record for its own identity until PR 4.

**Task 24**
- R24.c1. `Harness::settle` gains a busy-aware wait: the owner publishes a busy flag (a step or a lookup round outstanding) through the crate-private `EngineHandle::busy()`, and settle needs 300 ms of quiet when idle and waits up to 2 s while busy. Why: starting a freshly copied stand-in on Windows blocks 260 to 480 ms, longer than the old 300 ms quiet window, which made `the_eject_fake_failure...` flaky in the parallel suite. Cost if wrong: settle beside a deliberately hanging step takes up to 2 s.
- R24.c2 (design part 6.4 and spec 7 over the plan's single re-render). After the second render, if its path differs from the first, the library facts are computed again for the new path and the decision rendered once more while they change (it converges at once). Why: the re-render can change the path (the edition name), and a collision measured on the first path only is found only at the rename, after a full remux. Cost if wrong: one extra render.
- R24.c3 (design part 8.3). When a lookup round could not run and the job is in Ejected, the engine decides by the book's own `pending()`, waiting if lookups are pending and proceeding otherwise, instead of always proceeding.
- The review's Minor 3 joins the fix round: `busy` is cleared at the end of `shut_down`, and `EngineHandle::busy`'s doc is corrected.

**Task 25**
- R25.c1 (design part 5.10 over Step 4's L1a wording). In `plan_restart` a drive is an arrival, whatever `drives.json` says, when its fingerprint belongs to a disc-phase job that was fed `disc_present: false`. Why: otherwise a job whose disc sits in a handled drive waits forever. Cost if wrong: one extra MediaArrived that the reducer absorbs.
- R25.c2 (design part 5.8 over `held_promise` and S75). The restart's promise block and `promise_on_return` also rebuild the Remux promise (`planned_remux_promise(plan)` on the plan kind's library volume) for jobs in Ripping, RipVerified, Ejected, Inspecting or Decided. Cost if wrong: a library check undercounts after a restart.
- The fix round also runs the full workspace once (the final tree had only had the engine_restart run) and adds a stop-during-restart test, since the harness can arm the hook before the restart (S70).

**Task 26**
- T26-c1. `cancel_during_a_rip...` waits for `<job>/t00-00800/rip.txt` before cancelling. Why: the Ripping push precedes the rip step's drive listing, so a cancel could land before the rip started. Cost if wrong: one wait line in a test.
- R26.c1 (design part 3.7, "a self-test starts no second makemkvcon", and L2). The MakeMKV check takes the disc-phase token for its listing and holds it until the listing ends; when the token is held, the check reports the last check marked busy (S79) and takes nothing; while the check holds the token, no step that runs `makemkvcon` is handed out. Why: the review found the self-test's own `makemkvcon info disc:9999` was not kept apart from a job's `makemkvcon` start, so S55's check in a concurrent RunScan or RipTitle saw it and failed the step as Unknown. Cost if wrong: a self-test during a rip reports busy instead of a fresh version.
- The fix round also keeps the running check's `CancelToken` in the owner and raises it in `shut_down` and at a forced shutdown, so a listing stuck on a drive cannot hang a stop. It takes Minor 3 (the Java self-test test waits for Filed before its second `run_selftest`, so no two stand-ins of one tool overlap) and Minor 5 (AwaitingShare's re-check requires the roots the return state uses, staging only for a Ripping return, as S81 says).

**Task 27**
- T27-r1. The review's one Important is folded into Task 28's dispatch instead of a separate fix round: ctl's `main.rs` takes dev's lossy `args_os` form; `retune_spec` calls `.rest()?` after `flag("--spec")?`, so an ambiguous retune prefix exits 2 through ctl too (T28-h); `argument_error` gets its Def/Use tail. Why: `std::env::args()` panics on an argument that is not Unicode (exit 101) where dev's main reads it lossily, which breaks P23's identical exit codes on that input, and Task 28 rewrites the same crate. Cost if wrong: none.

**Task 28**
- T28-r1. The review's Minor 3 is promoted to one test-only fix round: the scripted-peer unit tests kept `connector` alive until `server.join()`, so a regression where ctl never connects would hang the test, and CI, instead of failing; `drop(connector)` now comes before each `server.join()`, so `accept` fails with BrokenPipe. Why: a hung CI job costs more than a failing one. Cost if wrong: none.

**Task 29**
- R29.c1. The characterisation test compares the stored document and decide's output as parsed JSON, structurally: every value that is not a number exactly, and numbers within a relative tolerance of 1e-12 (the C library's last-digit differences are about 1e-16); on a difference it names the first differing JSON path. The stored files are not regenerated. Why: the first CI run failed it on ubuntu and macos on `disc-a-eng-only-text.json`, and K5 pins the renderer's structure and values, not the C library's last digit. Cost if wrong: a last-digit float regression in the renderer goes unseen by the characterisation test.
- R29.c1a (amends R29.c1). Under `cfg(windows)` the characterisation keeps the byte comparison `printed == stored`, since the files were generated on Windows and passed byte for byte on windows-latest; elsewhere the structural comparison applies, with an integer-versus-float type difference counted as a difference. Why: the review found that the structural comparison let an integer and a float of the same value pass, and that nothing pinned the made-up fixtures' bytes any more. Cost if wrong: none.
- R29.c2. The testkit's time bounds (`Harness::wait`'s 10 s, `run_until`'s cap and the client's 20 s line wait) scale by 4 when the `CI` environment variable is set, as GitHub sets it; settle's quiet windows stay. Why: windows-latest failed two file-phase tests at the 10 s bound. Cost if wrong: a test that really hangs takes four times longer to fail on CI.
- R29.c2 stays although it was not the cause of those failures, which was that the lookup-timer tests fired the timer before the owner had started it (fixed in `cf01d2e` without changing any assertion). Why: a slow-runner factor is harmless and defensible. `ci_factor` is public because the IPC client lives in another crate.
- R29.c2a. `LOOKUP_WAIT_CAP` scales by `ci_factor` too, and `Harness::fire_timers` waits, scaled, until at least one timer is pending, and panics with a named message if none comes, so the lookup-timer race is loud for every future caller. Why: otherwise a slow runner could turn whole_job's direct line into an AwaitingLookup path.
- R29.c2b (revises the first part of R29.c2a). The `LOOKUP_WAIT_CAP` scaling is reverted, so it stays 5 s; `fire_timers`'s pending wait and the rest of R29.c2 stay. Why: `run_recorded`'s time-limit clock starts before the spawn, so the 20 s lookup wait under CI outlasted the 10 s scan limit and failed `another_discs_listing_with_a_changed_version_returns_a_job_from_awaiting_makemkv_when_the_token_is_free` on all three systems; the scaling was a precaution for a slow-runner path that had not happened. Cost if wrong: on a very slow runner whole_job's direct line could become an AwaitingLookup path, and the ruling is then revisited.
- R29.c3. Every stand-in test helper that executes a freshly copied `fake-tool` retries the spawn on `ErrorKind::ExecutableFileBusy`, up to about 20 times 50 ms apart, in one shared helper where the tests already share one. Why: on Linux a test that copies `fake-tool` and executes it while another thread forks can meet `ETXTBSY` ("text file busy"), because the child briefly holds the copy's write handle; the race is in the tests' copy-then-execute pattern, not in the code under test, a rerun would hide it, and hard links are not available on every CI file system. Cost if wrong: a few lines of test code.
- R29.c4. The `ETXTBSY` retry lives in test support code only: `testkit::BusyRetry`, a `ProcessGuard` wrapper in the engine's testkit that retries `spawn` on `ExecutableFileBusy` up to 20 times 50 ms apart, used by `TestGuard::spawn` and by the three driver test files. `platform::StdProcessGuard` and `tools::run_recorded` stay unchanged, because production copies no executables. Cost if wrong: test-support lines only.
- R29.c5. `wait_for_call`'s 20 s, and any other unscaled wait in the fake-tools engine tests, scales by `ci_factor()` like the harness bounds (R29.c2). Why: two engine_ipc tests failed at that wait on a heavily loaded windows runner. Cost if wrong: a stalled test fails after 80 s instead of 20 s on CI.

The owner may overturn any of them.

## Rulings on the open points

These are the planner's rulings of 2026-10-24 on the nine points the plan left open (OPEN 1 to OPEN 9). The owner made
no override of them before execution, and the code follows each one as written.

- RP-1 (OPEN 1, later drive listings): (a) record each later listing as `drive-list-<n>.txt` with its run files, numbered from 2. Why: design part 7.2 records every external run; a listing that leaves no file breaks that rule. Cost if wrong: a few small files per job. Where: `crates/engine/src/steps.rs` (`listing`); `crates/fake-tools/tests/whole_job.rs` `LATER_LISTINGS`; `crates/fake-tools/tests/engine_disc_phase.rs` `later_drive_listings_are_recorded_as_drive_list_n`.
- RP-2 (OPEN 2, no DRV line for the drive id): (a) feed `ScanFailed` or `TitleRipFailed` with class `Unknown` and a message naming the drive id. Why: the tool is present; the disc or drive is the problem, and `ToolMissing` would send the owner to reinstall MakeMKV. Cost if wrong: a job fails where a wait would have served. Where: `crates/engine/src/steps.rs` (`listing`); `engine_disc_phase.rs` `a_missing_drive_index_fails_the_scan_naming_the_drive`.
- RP-3 (OPEN 3, a missing `appdata.tar` or `default.mmcp.xml`): (a) `ToolMissing { tool: makemkvcon }`, which leads to AwaitingTool. Why: the MakeMKV installation is incomplete, and AwaitingTool is the state the design gives an incomplete tool (ARCH-9). Cost if wrong: the owner sees "waiting for a tool" for a file problem; the waiting message names the file. Where: `crates/engine/src/steps.rs` (`run_scan`, the job profile's write); `engine_disc_phase.rs` `a_missing_appdata_tar_waits_for_makemkvcon`.
- RP-4 (OPEN 4, decide's plan section of a lookup-pending job): (a) `PlanEntry::Results` built around the persisted refreshed plan and the job's settings. Why: the decision must follow the plan the engine ripped by (ENG-9, R-B1); Task 29's scenario is not lookup-pending, so CI is unchanged. Cost if wrong: a lookup-pending job's `decide.json` differs from a fresh `plan --job`, which design part 12.7 already accepts and `compare-job` (PR 6) explains. Where: `crates/engine/src/file_phase.rs` (`engine_decide_inputs`); `crates/fake-tools/tests/engine_file_phase.rs` `a_lookup_pending_job_decides_with_the_refreshed_plan`.
- RP-5 (OPEN 5, where the one scan retry runs): (b) `scan` fails after the first timeout, and the engine lists the drives and scans once more. Why: design part 5.4 step 1 reads the drive index before every call and never caches it; the retry is then a visible engine step with its own records. Cost if wrong: one more drive listing per retried scan. Where: `crates/tools/src/makemkv.rs` (`scan`, `SCAN_PAST_LIMIT`) and `crates/engine/src/steps.rs` (`run_scan`); `crates/fake-tools/tests/makemkv_driver.rs` `a_scan_past_its_limit_is_killed_and_fails_as_unknown` and `engine_disc_phase.rs` `a_scan_past_its_limit_is_killed_listed_again_and_retried_once`.
- RP-6 (OPEN 6, the first scan's records after a `MSG:1009`): (a) the second scan replaces `scan.txt` and its run files, and `job.log` keeps the rejected scan's 1009 text. Why: the design lists no `scan-rejected.txt`, and the recording layout stays phase 0's. Cost if wrong: the rejected scan's lines survive only in `job.log`. Where: `crates/engine/src/steps.rs` (`run_scan`); `engine_disc_phase.rs` `a_1009_on_the_first_scan_switches_every_later_call_to_fallback_mmcp_xml`.
- RP-7 (OPEN 7, where the verified size lives): (a) `VerifyFile` in Remuxing sets `TitleRecord.filed` with the final path and the partial's size, and `File` overwrites it with the real mtime. Why: no new field, no second `mkvmerge -J`. Cost if wrong: a restart in Verified reads a size that `File` would have replaced anyway. Where: `crates/engine/src/steps.rs` (`verify_partial`, `file`; Task 25, PF-26); `crates/fake-tools/tests/engine_restart.rs` `a_restart_in_verified_with_the_file_already_filed_feeds_file_done` and `a_restart_in_verified_with_another_size_at_the_final_path_fails_as_unknown`.
- RP-8 (OPEN 8, a torn last line of `transitions.jsonl`): (a) `Store::persist` first writes `\n` when the file does not end in one. Why: one line of code keeps every later line parseable, and the log is read by people and tools. Cost if wrong: none worth naming. Where: `crates/engine/src/store.rs` (`Store::persist`, landed in Task 21, T21-c2); `engine_restart.rs` `a_torn_last_line_gets_its_newline_before_the_next_append`.
- RP-9 (OPEN 9, the MakeMKV check after a restart): (a) persist the recorded version and licence code in `<data>/selftest/makemkv-check.json`. Why: the comparison needs a baseline that survives a restart, and each job's own scan output may predate a MakeMKV update. Cost if wrong: one small file in the data folder. Where: `crates/engine/src/selftest.rs` (`MakeMkvState::open` and `observe`); `whole_job.rs` assertion 8 (T29-c).

## Left for later pull requests

The plan's "Not covered" list, with the items the reviews of pull request 2 added.

**Pull request 3** (design part 12.6, item 3):
- the Windows implementations, the named pipe and its client, the mutex and the access lists;
- the `PATH` writable-folder skip, `engine-task.ps1`, the run folder and the `command` relay (S35);
- PR 3's `DriveWatcher` must report the drive id in MakeMKV's `"D:"` form;
- L7's present-but-inaccessible folder (S39);
- `Host`'s local time and the `started` key of `rip-summary.json` (design part 4.6, Ruling R23.7);
- `platform::rename_no_replace` is `hard_link` then `remove_file` (S13) and is not atomic; PR 3 swaps in `MoveFileExW`. Hard links on the NAS share were never probed in PR 2, which runs on stand-ins only;
- `StdProcessGuard` kills only the child, so a grandchild that inherits the pipes would block `run_recorded`'s reader after a kill; PR 3's Job Object runner kills the tree;
- `find_tool` checks relative or empty `PATH` entries; PR 3's writable-folder rule should also keep only absolute folders;
- the engine binary has no handle on the IPC waker (`EngineHandle::set_ipc_waker` is crate-private) and no way to learn that a `shutdown` request stopped the owner; the MakeMKV check at start (PF-28) cannot be cancelled, so a real `makemkvcon` hanging at service start would hang `start_until`; one `accept` error ends the serve thread, which should log and continue unless the transport is closed;
- `DriveWatcher`'s doc does not promise that `next()` never reports again the discs `present()` listed at start, which L1a relies on; a present but unreadable disc at a restart gets no readiness wait;
- a settings tool path written without `.exe` counts as missing under the `is_file` filter.

**Pull request 4** (S40, S47):
- `ureq`, the live shape tests, J8's tagged search and merge, and the entry-title search of design part 8.2 item 3;
- TV seasons;
- webhook delivery and the webhook self-test check;
- the stale-copy refetch and K15's exit 3;
- the cleanup task (P4, R-B4);
- the two divergences between the engine's lookup book and dev's readers that T16-r1 accepts: a dischash 200 whose body parses as no disc is `Miss` in the book but failed or pending for dev's `hit_rate::lookup`, and a search left pending writes `search 000`, which dev's plan reader does not read. PR 4 aligns dev's readers with S46 and teaches them the `search` word;
- `ChainInputs` does not carry the remembered identity, so a disc whose remembered identity differs from the search's top result plans without a TMDb record for its own identity;
- the live `ureq` transport must keep request headers, and so the TMDb token, out of error text;
- a `settings set` file over 4 MiB ends with "the engine closed the connection" (exit 4) rather than the engine's own text; a size check against `MAX_LINE_BYTES` before connecting would give exit 2.

**Pull request 5:**
- step 10, step 10a, OCR (optical character recognition), the evidence trees, `probe-summary.json` and the `ffmpeg` driver;
- `Note::SubtitleTextUnread` and the unread streams of `SubtitleFacts` (design part 10.8);
- the zlib inflater (`miniz_oxide`);
- dev's `ocr-trial`, `retune`'s repeated `--evidence`, and dev's reader of a bare `streamhash.txt`;
- `licenses` and `export-models`;
- layer 2 (real tools, MKVToolNix on CI). CI's commands stay unchanged in PR 2.

**Pull request 6:** `compare-job`.

**Never reached in phase 2** (design part 5.2): `DeleteJobData`, `CopyDisc`, `ExtractTitle`, `NetworkNeeded` and
`VersionMessage`.

**Accepted in pull request 2 and left as they are:**
- abbreviations of `--job`, `--jobs` and `--title` belong to `lazymkvctl` (PF-32), so `--t` is `--title`, not dev's `--text`;
- the index guard fails open when no `MSG:5014` or `MSG:5015` appears (Task 13, as the plan requires);
- KeepAwake is held through waits entered after a file-phase slot grant (the review of Task 24; design part 4.2 is to confirm it);
- `release_promises` never releases the rip check's Remux promise on a transition into NeedsReview or Discarded from the checked states; the next restart drops it (the review of Task 25).

## Tests

`cargo test --workspace --locked -- --list` on the mini PC (Windows), on the tree of commit `3a94d84`, lists 2487 tests:
1711 in `lazymkv-core`, 416 in `lazymkv-dev`, 8 in `lazymkv-capture`, 173 in `fake-tools`, 21 in `lazymkv-platform`, 26
in `lazymkv-tools`, 31 in `lazymkv-clients`, 80 in `lazymkv-engine` and 21 in `lazymkv-ctl`. The baseline is the 2052
tests of docs/phase1/decision-core.md:701, which pull request #5 left unchanged (it added no test), so pull request 2
added 435. The lists are the mini PC's: a test compiled only on Windows (such as `lazymkv-capture`'s eject test) is in
them, and a test compiled only on Unix is not. The junction run of that tree on the mini PC passed 2486 tests and
ignored 1 (`lazymkv-platform`'s `process::tests::helper_child`, the child the process tests start); CI run 40410226751
passed 2486 on windows and 2485 on ubuntu and macos, with 1 ignored on each.

| Where | What it tests |
|---|---|
| `crates/fake-tools/tests/tools_record.rs` | `tools::run_recorded` against the stand-in: the stdout, stderr, timing and exit files, bytes kept as received, time limits, cancels and line-action kills, a program that cannot start, a panicking callback, and the busy-spawn retry of R29.c4 (design parts 5.3, 7.2) |
| `crates/fake-tools/tests/makemkv_driver.rs` | The `makemkvcon` drivers: the drive listing; the scan, complete or not, with a licence message, a `MSG:1009` or past its limit; the rip, with the index guard, licence and read errors, an unreachable share and the exit wait; and the one-`makemkvcon` check (design parts 5.3, 5.4, 5.6) |
| `crates/fake-tools/tests/mkvmerge_ffprobe_driver.rs` | The `mkvmerge` identification and remux, with their exit codes, the header limit and progress, and the `ffprobe` driver (design parts 5.3, 5.7, 5.11) |
| `crates/fake-tools/tests/engine_disc_phase.rs` | The disc phase from arrival to eject: the recording and the job profile, the drive listings, the `MSG:1009` fallback, a copy plan, verify failures and index drift, the waiting states, a removal mid-rip, remembered decisions, the debug logs of the tuning evidence, the scan retry and the eject (design parts 5.4 to 5.6, 5.8) |
| `crates/fake-tools/tests/engine_file_phase.rs` | The file phase from the slot to the filed file: lookups left pending or answered late, Decide and `decide.json`, collisions, the remux, the verification and the filing (design parts 5.7, 6.4, 6.5, 8.3, 8.4) |
| `crates/fake-tools/tests/engine_restart.rs` | Restarts in each state of the path, torn and lost transition lines, rebuilt books and promises, and Decide's inputs read back (design part 5.10, L1, L1a) |
| `crates/fake-tools/tests/engine_ipc.rs` | The IPC server over the in-memory transport: the snapshot, every request and its refusals, retries, cancels, the self-test, slow clients and shutdown (design parts 3.7, 3.8, 9.1 to 9.4) |
| `crates/fake-tools/tests/ctl_engine.rs` | `lazymkvctl`'s engine commands against the real engine over the in-memory transport (design part 9.6) |
| `crates/fake-tools/tests/whole_job.rs` | The acceptance of design part 12.6 item 2: a media-arrival event takes a synthetic movie disc to a filed placeholder file; the transitions, the notifications, the job folder and the data folder are checked exactly, `lazymkv-dev decide` over the job folder prints `decide.json` without its two extra keys (R-B1), and no request goes without a recorded answer |
| `crates/fake-tools/tests/replay.rs` | The stand-in extensions of design part 12.2 (stderr replay, files named by `-o`, the response counter, hang and stop, the `ffmpeg` stem) |
| `crates/capture/tests/disc_snapshot.rs` | `capture disc`'s output, byte for byte, across the R-B2 move |
| `crates/engine/tests/binary.rs` | The PR 2 engine binary exits 1 with its OS's sentence and creates nothing (S28) |
| `crates/dev/src/cmd/decide.rs` (`mod tests`, over `crates/dev/src/cmd/decide/characterisation/`) | `decide`'s output across the K5 split against the stored documents, the reader and the pure renderer, and decide's replay flags |
| Each new crate's unit tests (`lazymkv-platform`, `lazymkv-tools`, `lazymkv-clients`, `lazymkv-engine`, `lazymkv-ctl`) and core's new modules (`record`, `ipc`, `robot::lines`, `ustar`, `snapshot`) | The fakes and the std helpers; the recorder, the tool lookup and the drivers' argument builders; the lookups, the recorded transport and the saved answers; the reducer loop, the store, the timers and the self-test over the scripted harness; ctl's routing and its engine commands against a scripted peer; the record and message shapes |
| `crates/dev/tests/clone_values.rs` | Ruling RP21, now over `corpus/scenarios/` too: no scenario holds a value only the evidence clone holds |

`no_scenario_holds_a_clone_value` needs the evidence clone (`LAZYMKV_EVIDENCE`) and skips without it, so the RP21
guard over `corpus/scenarios/` runs only in the mini PC's junction run, not on CI. With the clone, on the mini PC:

```bash
scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"
```

Two notes on how the tests behave on CI. The characterisation test of `decide` compares bytes on Windows and, on ubuntu
and macos, compares the parsed documents structurally, with numbers within a relative tolerance of 1e-12 (R29.c1a),
because the stored documents hold floats computed with the Windows C library's mathematical functions, whose last digit
can differ on the other systems. The engine testkit's time bounds scale by 4 when the `CI` environment variable is set
(R29.c2), so a slow CI runner does not fail a test that is only waiting.
