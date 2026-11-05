# Phase 2 Pull Request 4: Live Lookups and Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A disc identifies live through TheDiscDB and TMDb in design part 8.2's order and timing, its answers are saved slim and fetched again when stale, the cleanup of part 5.9 runs, and every notification reaches the owner's webhooks with part 8.6's retries, so one real movie disc goes from insertion to a filed file in the deployed desktop task with Home Assistant showing its notifications (design part 12.6, item 4; A3), while every item that `docs/phase2/engine.md` leaves for pull request 4 is closed.

**Architecture:**
- `clients` gains the live transport, `UreqTransport` (ureq, rustls on ring, `webpki-roots`, 10 s, `User-Agent`, no redirect, no proxy, an 8 MiB body cap, a fixed error vocabulary that never holds a URL or a header), behind the existing `HttpTransport` trait, which grows `post` and `connect(host, port)`; `RecordedTransport` and `OfflineTransport` implement both.
- `clients::run_round` follows part 8.2 whole: TheDiscDB by fingerprint and disc ID, the speculative name search and J8's tagged second search with the pure `merge_pages`, the record fetch, S47's entry-title search, and for a TV record the season answers merged with `merge_season`. Each new request is its own `Lookup` with its own `lookup-status.txt` word, so the book, the restart's rebuild and dev's readers keep one line per lookup.
- Stale TMDb copies (more than six calendar months old, or deleted by the cleanup) are found by one pure rule in `core` and fetched again by `clients::refetch_record` and `refetch_search`: the engine before a Plan from `Scanned`, before every Decide and before the `command` relay builds its words (with exit 3 when it cannot, K15); `dev` (and so `lazymkvctl`'s local forms) before `plan`, `decide` and `retune` read a folder that is not an engine job folder, with `TMDB_TOKEN`, else exit 3 (the engine owns its job folders' copies, Ruling T4-R10).
- `engine` gains the cleanup task of part 5.9 on a schedule of its own (start, then hourly), the `WebhookSink` notifier of part 8.6 (three attempts, waits of 0 s, 1 s and 4 s, any 2xx, both URLs as secrets read at each delivery), the self-test's two webhook checks (host and port only, no POST), and the Windows binary passes the live transport, the sink and the hourly cleanup in place of `OfflineTransport` and `NoDelivery` (W26 replaced).
- `dev` reads the engine's lookup words (S46, the search words, T16-r1) and gains `lazymkv-dev census` (A9); `ctl` refuses a request over 4 MiB before connecting (exit 2).
- The live shape tests of part 8.5 run only on the mini PC (`#[ignore]`, `TMDB_TOKEN`, records into the evidence mirror); the pull request ends with the owner's token and webhook URL entered by hand and one real movie disc through the desktop task (A3, A6, A7, A8).

**Tech Stack:** Rust 1.99.0 (`rust-toolchain.toml`), edition 2021.
- Workspace crates in use after A2's merges: `serde`, `serde_json` (`preserve_order`, `float_roundtrip`), `md-5` 0.11, `sha1` 0.11, `regex`, `fancy-regex` 0.20, `unicode-normalization`, `windows-sys` (`=0.61.2`, `platform` only).
- **One new crate family, the ureq stack (design parts 4.7 and 8.1, P33, A1):** `ureq` with a caret requirement in `[workspace.dependencies]`, `default-features = false` and only the feature that gives rustls on the ring provider with `webpki-roots` compiled in (expected `features = ["rustls"]` on ureq 3; Task 1 confirms the feature name and that it pulls neither `aws-lc-rs` nor `rustls-platform-verifier` nor `native-tls`), used by `lazymkv-clients` only. The crates expected (the same list Task 1 Step 1 expects; the binding list, each crate with its exact version and manifest licence, is the one Task 0 Step 3 reads from crates.io and the owner approves at plan time): `ureq`, `ureq-proto`, `rustls` (Apache-2.0 OR ISC OR MIT), `ring` (Apache-2.0 AND ISC), `webpki-roots` 1.x (CDLA-Permissive-2.0; 0.25 and 0.26 were MPL-2.0), `rustls-webpki` and `untrusted` (ISC), `rustls-pki-types`, and helpers among `log`, `base64`, `http`, `httparse`, `bytes`, `once_cell`, `percent-encoding`, `utf8-zero`, `zeroize`, `subtle`, `getrandom`, `wasi`, `cc`, `shlex`, `find-msvc-tools` (`cfg-if`, `itoa` and `libc` are already locked; `ring`'s Windows-on-ARM target table may add a second `windows-sys` with `windows-targets` and its eight `windows_*` crates), plus the changed dependency lists of `lazymkv-clients` and `lazymkv-dev` (the new path edge `dev -> clients`, Owner decision 25). The package count goes from the count read at the branch point (45 at `6a862ac`, re-read at Task 0 Step 4 after A2) to the count Task 1 prints. **Task 0 Step 3 reads the real list (each crate's exact version, manifest licence, checksum and features) from crates.io, fetching nothing, for the owner's plan-time yes; Task 1 reads it again, marks any difference from Task 0's list, and prints it with the whole hand-written `Cargo.lock` delta for the owner's second yes before anything is fetched (A1); a licence that differs from design part 4.7's table, a crate needing a new tool (nasm, CMake, perl), or an MSRV above 1.99.0 goes back to the owner.**
- Nothing else new: no tokio, reqwest, clap, `windows` crate, date-time crate, URL crate, percent-encoding crate of our own, TLS server crate or mock HTTP server crate (design part 4.6). `ocr`'s build-script use of ureq is pull request 5's. The toolchain bump to 1.99.1 waits for its own yes (A2).

**Spec:** `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (the design; binding with P1 to P37, R-A, R-B, K1 to K15 and L1 to L15; part 8 is this pull request's heart, with parts 5.9, 7.1, 7.5, 9.4, 9.5, 9.6, 12.1 to 12.7, 14 and 15), `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (the spec: 4.5, 8 with 8.1, 12, 13, 15), `docs/phase2/engine.md` (S1 to S88, PF-1 to PF-34, R22 to R29, I1 to I6, the M items, F1, RR1-1 to RR1-16, W1 to W28, the T3 rulings, PF3, the whole-branch review's F1 and F2, "Review round 1 (the review of pull request #14)" with its V, G and R entries, and the "Pull request 4" list under "Left for later pull requests"), `docs/phase1/decision-core.md` (RP19, RP21, RP23, RP24, J8, H1), and `.superpowers/pr4/decisions.md` (the owner's decisions A1 to A11 of 2026-11-04, which bind this plan).

**Branch:** `phase-2-live-lookups`, from `main` after A2's five Dependabot merges (md-5 0.11, fancy-regex 0.20, sha1 0.11, serde_json 1.0.154, unicode-normalization 0.1.26), made at Task 0. The pull request opens as a draft at Task 1's push (A11).

## Global Constraints

- **Toolchain and commands.** Rust 1.99.0 from `rust-toolchain.toml`. Every cargo command runs on the mini PC through `scripts/box.sh`, one at a time, with `--locked`, and never while a real disc rips or a desktop-session job runs (design part 12.5; each call replaces `C:\lazymkv\src`). The Mac has no toolchain.
  - A task's **Run** is its targeted command; before its commit the task runs `scripts/box.sh cargo test --workspace --locked` once and it must pass.
  - A task that touches dev's `plan`, `decide`, `retune` or their readers (Tasks 10 and 15), touches dev's readers and makes core's `row14` items public (Task 16), or adds a file under `corpus/scenarios/` (Task 18) runs the junction form for its workspace run, since the RP21 guard `no_scenario_holds_a_clone_value` and the goldens run only there: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`. A golden that changes stops the task and goes to the owner (K5).
  - A filter matches test paths (module path and function name). `--test <stem>` names an integration-test file of the crate given with `-p`.
  - Line numbers are as of `891a0e7`; a task that runs after another that edits the same file finds the named item where it then is.
  - `LAZYMKV_OCR_TEXT` is never set. Nobody reads or writes `corpus/owner` except the junction runs' read-only goldens, Task 19's read-only replay of the live records (`saved_live_records_still_parse`), the controller's copy and commit of the live records at Task 20 Step 2, owner item 17, and the owner's own acts. No test reads the clone for the census (A9).
- **Dependencies.** The ureq stack is the only new crate family. Task 1 makes every `Cargo.toml` change of this plan (the ureq requirement, `clients`' dependency, the `dev -> clients` path edge) and fetches only after the owner's second yes on the printed delta; no later task edits a `Cargo.toml` dependency list. After the first fetch the controller checks that the `Cargo.lock` `box.sh` copies back equals the approved hand-written delta.
- **No network on CI.** CI's commands and `.github/workflows/ci.yml` stay unchanged. No non-ignored test contacts TheDiscDB, TMDb, a webhook or any host but `127.0.0.1`: tests use `RecordedTransport`, `OfflineTransport`, or `UreqTransport::with_timeout` against a `std::net::TcpListener` bound on `127.0.0.1:0` (never a name, so no DNS; never a non-loopback address, so no firewall prompt). `UreqTransport::new()` is called only by the Windows binary's production path (`native::run_with` with `NativeOptions::production()`), by `dev`'s `Ctx::with_live_transport` (called only from `crates/dev/src/main.rs` and `crates/ctl/src/main.rs`), by the `#[ignore]` live tests, and by one non-ignored construction-only test (`the_production_timeout_is_ten_seconds`), which sends nothing. `Ctx::from_process` keeps `http: None`, so the engine's Decide (`steps.rs:1569`) never builds a live transport. A test this pull request adds that spawns `lazymkv-dev` or `lazymkvctl` calls `.env_remove("TMDB_TOKEN")` on its `Command` (the existing spawning tests run `scan` and engine commands only, which never fetch), so a binary test on the mini PC, where `TMDB_TOKEN` stays set (A6), can never fetch live or rewrite a copy under `C:\lazymkv\src`. No token or secret is ever stored at GitHub (design part 8.5).
- **No I/O in core (RP19).** The core additions (`entry_search_title`, `search_queries`, `parse_season_answer`, `merge_parsed_season`, `lookup::stale`) are pure.
- **RP21.** No file under `crates/` or `corpus/scenarios/` holds a byte copy of, or a value read only from, `corpus/owner`. New scenarios are synthetic; the census test builds its recordings at test time from files already public (`corpus/scenarios/`, `corpus/thediscdb/`, `corpus/thediscdb-api/`). The live tests' fixed requests use only public IDs from `corpus/thediscdb-api/` and the public name `PIRANHA_3D` (decision-core H1).
- **Unchanged output.** The goldens stay byte-identical (K5), and so do `capture disc`'s output (R-B2) and every phase 0-shaped file form (S68); `hit-rate`'s lines are re-pinned where they change (T4-R17 overturned 2026-11-04). The changed recording files are `tmdb-search.json` (two `queries` when J8's second search ran), `tmdb-record.json` (a TV record's merged seasons) and `lookup-status.txt` (the new words of Ruling T4-R1).
- **Secrets.** The TMDb token, both webhook URLs and the share credential never enter the repository, a test, a recorded file, a log line, a notification or an error text; only the owner types them (A6, A7). Tests use sentinels that cannot occur in a path and never match CI's guard pattern `eyJhbGciOi[A-Za-z0-9_-]{20,}`: `token-sentinel-5d2e`, `token-sentinel-2b9c`, webhook ids `hook-sentinel-7f3a` and `hook-sentinel-1c4b`, userinfo `user-sentinel-7c1e:pass-sentinel-4b9d`. Every failure path's text is asserted to lack them: `TransportError` and `PendingReason` texts, waiting messages, `lookup-status.txt`, `job.log`, `engine.log` (`undelivered_line`), the snapshot (host only), self-test details, `secret_set`'s refusals, exit-3 messages. `UreqTransport` never formats a `ureq::Error`'s `Display` or `Debug` into any text. The engine never reads `TMDB_TOKEN`; `security::CHILD_ENV_NAMES` does not gain it (V1).
- **Persisted files.** Engine-written job-folder files go through `platform::write_atomic`; `state.json` keeps `lazymkv job v1`. Lookup answers stay `serde_json::to_vec_pretty` plus a final `\n` (T16 rulings); TMDb copies are only ever slim (P29); `fetched_at` stays the host's local date `yyyy-MM-dd` (design part 4.6). `dev`'s refetch writes a temporary file beside the copy and renames it over (it has no `platform` edge).
- **Docs and prose.** Every `pub` item has a `///` doc naming its spec section, design part or ruling, ending "Def: T4-<n>. Use: T4-<m>." Prose is plain sentences, no em-dashes, acronyms expanded at first use. Expected multi-line texts in tests are arrays joined with `"\n"`.
- **Tests.**
  - Tests go in the file's `mod tests` or a sibling `tests.rs`, named as snake_case sentences. Tests that run `fake-tool` live in `crates/fake-tools/tests/` (S12); `lazymkv-clients` tests write bodies inline and use no `testsupport` dev-dependency (S69).
  - Waits that only wait scale by `ci_factor()` (R29.c2); `LOOKUP_WAIT_CAP` stays 5 s unscaled (R29.c2b); a bound that proves "does not block" or "gives up in time" does not scale. The notifier's waits are injected (`WebhookSink::with_pause`) and the live transport's timeout (`UreqTransport::with_timeout`), so no test sleeps 5 s for a backoff or 10 s for a timeout.
  - A test whose input is missing prints `skipped: <reason>` and passes; a printed `skipped:` line counts as not run in the notes (Task 21).
  - Live tests carry `#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]`; CI never passes `--ignored` or `--include-ignored`.
- **CodeQL.** The ruleset blocks on any alert, so the pull request's CodeQL run shows none. Watch cleartext logging (no token or URL is ever formatted) and cleartext transmission (the loopback tests post over `http://127.0.0.1`; production http is by design for a LAN Home Assistant, part 8.1). After Tasks 7 and 12 are pushed, the controller reads the draft's CodeQL result and treats an alert as a red CI run.
- **The mini PC and the Mac.** Nothing is installed on either, no scheduled task is registered or re-registered, and no firewall rule is changed without the owner's explicit yes naming it (Owner prerequisites and decisions); the desktop task `lazymkv-engine` is redeployed only by `scripts/engine-task.ps1 -AcceptWritableBuild` after a release build (PR 3 Owner decisions 3 and 13, item 20 below). Every mini PC evidence line goes first to the git-ignored `.superpowers/pr4/` folder, in engine.md's mini PC form, with no key, drive serial, token, account name or SID.
- **TDD cycle per task.** Write the listed tests. Run the task's **Run** command and see the named failure (compile error or FAIL). Implement. Run again for PASS. Run the workspace once. Commit. The engine's existing tests stay green at every task: a task that adds a `Lookup` variant or a trait method updates every match and implementation in the same commit.
- **Commits.** One signed commit per task, no attribution line, author and committer `krcm0209 <17315774+krcm0209@users.noreply.github.com>`; `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G` and that identity. From Task 1's push on, each commit is pushed to the draft pull request; a red CI run on any of the three systems goes back to that task before the next starts.

## Review Focus

The five input classes or failure modes most likely to bite the owner that the tasks' main tests do not reach. Each gets the named tests in its owner task.

1. **TMDb throttles or refuses mid-job.** A 429 (with a `Retry-After` the clients ignore) or a 503 on one search, and a 401 after the owner changes the token in a running engine.
   - Tests: `a_429_leaves_the_search_pending_as_http_429_and_is_asked_once_in_the_round` (clients chain: one request to that URL in the round, status line `search 429`, the waiting text `search: HTTP 429`, the round still asks the tagged search and TheDiscDB; a second round asks it once more). Owner: Task 5. `a_new_token_reaches_the_next_round_without_a_restart` (fake-tools `engine_ipc.rs`: the first round answers 401, the job waits with `search: TMDb refused the token (HTTP 401)`; after `secret_set tmdb_token` with `token-sentinel-2b9c` and `retry`, the next request carries `Authorization: Bearer token-sentinel-2b9c`, `LookupAnswered` is fed, and neither sentinel appears in `job.log`, `engine.log` or `state.json`). Owner: Task 18.
2. **A TheDiscDB record whose TMDb ID is of the other kind** (a `Series` entry naming a movie ID, or an ID TMDb has dropped), so `/tv/{id}` answers 404.
   - Test: `a_discdb_id_of_the_wrong_kind_is_a_record_miss_and_leaves_nothing_pending` (clients chain: status `tmdb tv/<id> 404`, `pending() == (false, false)`, no season request, `lookup_inputs().record` is None, and the next round asks the record once more, E:338). Owner: Task 5.
3. **A webhook URL with userinfo, a port, a path, a query and a fragment** (`http://user-sentinel-7c1e:pass-sentinel-4b9d@ha.local:8123/api/webhook/hook-sentinel-7f3a?x=1#f`).
   - Tests: `a_webhook_url_with_userinfo_path_and_query_targets_only_its_host_and_port` (clients: `webhook_target` gives `ha.local`, 8123, http). Owner: Task 6. `an_undelivered_notification_names_its_channel_and_never_its_url` (engine notify: the reason and the `engine.log` line hold none of `hook-sentinel-7f3a`, `pass-sentinel-4b9d`, `user-sentinel-7c1e`, `x=1`, `/api/webhook`). Owner: Task 12. `the_webhook_check_shows_only_host_and_port` (engine selftest: detail `ha.local:8123 accepted a connection`, `path` None). Owner: Task 13.
4. **A body that is huge, not JSON, not UTF-8, or a redirect** (a captive portal answering 200 HTML or 302 to a login page, a runaway 50 MiB body).
   - Tests: `a_redirect_is_not_followed_and_reads_as_its_status` (loopback: a 302 whose `Location` names a second loopback listener; that listener sees no connection, the bearer sentinel is sent once only, `get` gives `Ok(Response { status: 302, .. })`; the lookups' URLs are fixed to TheDiscDB and TMDb, so the lookup outcome of a 302 is pinned instead by Task 5's `each_http_outcome_has_its_class` as `Pending { Status(302) }`), `a_body_over_8_mib_is_unreachable_and_says_so` (loopback, 8 MiB + 1 byte: `Err(Unreachable(BODY_TOO_LONG))`). Owner: Task 7. `a_200_html_page_is_unparsed_and_its_sentence_quotes_no_markup` (clients: `Pending { Unparsed(s) }`, `s` names the URL and holds no `<html`). Owner: Task 5.
5. **The cleanup and the clock.** A host clock years ahead (every copy stale), a `fetched_at` after today or malformed, and folders the cleanup must never touch: a held, failed or running job's staging, a job folder without `state.json`, a folder under staging not shaped like a job ID, and a link inside a staging folder that points into the library.
   - Tests: `is_stale_reads_calendar_months_clamps_the_day_and_reads_a_future_date_as_fresh` (core). Owner: Task 8. `a_clock_years_ahead_deletes_only_tmdb_copies` (engine cleanup: every other file in every job folder byte-identical, no staging folder of a held, failed or running job removed), `the_cleanup_never_deletes_a_folder_not_shaped_like_a_job_id`, `the_cleanup_never_follows_a_link_out_of_a_staging_folder` (`cfg(unix)` symlink, `cfg(windows)` junction: the link's target and its file survive). Owner: Task 11.

## Owner prerequisites and decisions

Each item names the task that waits on it. Items 1 to 11 are the owner's decisions of 2026-11-04 (`.superpowers/pr4/decisions.md`); items 12 to 25 are what the readers found; item 26 lists the rulings this plan makes where the design leaves the letter open, each of which the owner may overturn. The planner's recommended answer is given where one is needed. Task 0 Step 1 asks, in one message, items 17, 22, 23 and 24, the four rulings that ask the owner's yes (T4-R15, T4-R17, T4-R19, T4-R21), and whether any other ruling is overturned; Task 0 Step 3 asks A1's plan-time yes with item 25; Task 20 Step 0 asks items 12 to 16 and 18 to 21. Each answer goes to `.superpowers/pr4/decisions.txt` as `<item>, <yyyy-MM-dd>, <answer>`.

1. **A1. The new crates.** `ureq` with rustls on the ring provider and `webpki-roots`, default features off except TLS (design part 8.1). The owner approves the exact crate list and the full `Cargo.lock` delta at plan time (the exact list Task 0 Step 3 reads from crates.io after A2's merges, fetching nothing; Tech Stack names the crates expected) and once more, on Task 1's printed list and hand-written delta, before anything is fetched on the mini PC (ENG-7, P33), as PR 3's Task 1 did. Both yeses and their dates go into engine.md's "The Cargo.lock delta". Waits: Task 0 Step 3 (plan time); Task 1 Step 4, before Step 5's fetch.
2. **A2. Dependabot first, toolchain held.** The five open Dependabot bumps merge before `phase-2-live-lookups` branches, so the delta is only the ureq stack's; the rust-toolchain bump to 1.99.1 waits for its own yes (it makes rustup install 1.99.1 on the mini PC). Waits: Task 0 Steps 2 and 4 (before the branch).
3. **A3. The mini PC run.** One real movie disc through the deployed desktop task: it identifies live (TheDiscDB or TMDb), plans, rips to the staging share, files into the test library, and the Home Assistant webhook fires. Design part 12.7's full checklist stays with pull request 6. Recorded in engine.md's mini PC section in its shape. Waits: Task 20.
4. **A4. Channels.** Home Assistant only: `ha_webhook_url` is set for the run; `webhook_url` stays unset, its path covered by tests on the recorded transport. Waits: Task 12 (tests), Task 20 (the run).
5. **A5. TV.** The lookup side only: a TheDiscDB TV record's `/tv/{id}` fetch, the season answers and `merge_season`. TV filing, `tv_root` and season sessions stay phase 4; a TV disc is held with no path (design part 6.6). Waits: Task 4.
6. **A6. The TMDb token.** The owner enters it at the mini PC's elevated console with `lazymkvctl secret set tmdb_token` (standard input); nobody else types or sees it. `TMDB_TOKEN` stays set in the mini PC's environment for the live tests and local dev runs (design part 15 item 8). Waits: Task 20 step 7 (and item 15's way of typing it).
7. **A7. The Home Assistant automation.** The controller drafts the webhook-trigger automation (Task 20 step 6 holds the draft: `mode: queued`, `max: 25`, trigger `webhook` with `allowed_methods: [POST]` and `local_only: true`, a guard condition that `trigger.json` holds `event`, and one `persistent_notification.create` per event carrying `event`, `title`, `job_id`, `reason`/`reasons`/`class`, `message`, `time`), shows it to the owner, and creates it through the session's Home Assistant connector only after the owner's yes. The webhook URL (a secret) reaches the engine by the owner's hand, as A6. Waits: Task 20 step 6 (and item 14).
8. **A8. The disc.** No census now. The owner gives the titles they own; the controller checks each on thediscdb.com and picks a movie TheDiscDB does not know, so the search and the record fetch both run live. Subtitle and Atmos or DTS:X requirements belong to pull requests 5 and 6 (but see item 12). Waits: Task 20 step 9.
9. **A9. The census report.** `lazymkv-dev census`: one line per recorded disc under the evidence clone (title, year, kind, TheDiscDB hit or miss, English subtitle streams, Atmos or DTS:X, UHD, Blu-ray or DVD), read with the existing readers, tested on public files and never on `corpus/owner` (RP21). Waits: Task 16.
10. **A10. Census sessions.** A later census session stops the engine with `lazymkvctl stop` before and restarts it with `Start-ScheduledTask lazymkv-engine` after, both over SSH, each recorded in engine.md. None is scheduled inside PR 4. Waits: Task 21 (the rule is recorded in "How to run it").
11. **A11. Process.** This plan comes from the reader-architect-drafter-critic workflow; the branch is `phase-2-live-lookups` from `main` after A2; the pull request opens as a draft at Task 1's push and is executed subagent-driven, one signed commit per task, no attribution line, every cargo command through `scripts/box.sh`, one at a time. Waits: every task.
12. **The disc and step 10 (S32).** In this version a disc whose ripped main title keeps two or more English or no-language subtitle streams (closed captions and derived forced streams included) fails Decide with `subtitle evidence (step 10) is not built in this version` and never files, which would defeat A3. Recommended: among the owner's titles TheDiscDB does not know, the controller picks one whose disc lists at most one English subtitle track and no closed captions (checked on a public disc listing), and says so when proposing it. Waits: Task 20 step 9.
13. **A hold in the run.** If the job stops in `NeedsReview`, the owner approves it unchanged with `lazymkvctl approve <job id>` so it files, and the reason is recorded as a finding (design part 12.7's pattern). Recommended: yes. Waits: Task 20 step 9.
14. **Who sees the webhook ID (A7).** Creating the trigger through the connector would put the ID in the session. Recommended: the controller creates the automation with the placeholder ID `lazymkv-placeholder-replace-me`; the owner opens it in Home Assistant's editor, deletes the placeholder's webhook trigger and adds a new Webhook trigger (the editor generates a random ID for a new trigger, never for an existing one), POST only and local only, saves, and types the full URL into `lazymkvctl secret set ha_webhook_url`. Waits: Task 20 step 6.
15. **Typing the token at the console.** `secret set` reads standard input to its end, so at an interactive console the token is echoed and the owner ends input with Enter, Ctrl+Z, Enter (a trailing space would make TMDb answer 401). Recommended: in PowerShell at the elevated console, `$env:TMDB_TOKEN | C:\lazymkv\run\lazymkvctl.exe secret set tmdb_token`, which shows nothing; either way is the owner's own act. Waits: Task 20 step 7.
16. **Reading Home Assistant's state for the evidence.** A webhook POST gets 200 from Home Assistant even for an unknown webhook ID, so delivery proves nothing alone; the run's proof is the persistent notifications, read through the connector (never an automation trace, which shows the ID). Recommended: the owner allows that read. Waits: Task 20 step 10.
17. **The live records' place and the hand copy (design part 8.5).** Recommended: `<LAZYMKV_EVIDENCE>\live-lookups\<yyyy-MM-dd>\`, file names as Task 19 gives them; the controller lists names and sizes, and the owner copies the folder into the clone with one `scp -r` and commits it there before any re-mirror. The live tests run once, at Task 20 step 2, and again only if a parser changes after it. The owner's answer of 2026-11-04: the controller copies the live records into the evidence clone with `scp -r` and makes the signed commit there (not the owner); the folder is `<LAZYMKV_EVIDENCE>\live-lookups\<yyyy-MM-dd>\` as recommended; the live tests run once at Task 20 step 2, again only if a parser changes. Waits: Task 0 Step 1 (the answer); Tasks 19 and 20.
18. **What the notes name (A3).** Recommended: no film title or TMDb ID ("a movie disc TheDiscDB does not know"), `<job id>`, `<film folder>`, and the Home Assistant host as `<ha host>`. Waits: Task 21.
19. **The desktop task's settings and the test library.** Recommended: reuse PR 3's `%ProgramData%\lazymkv\settings.json` unchanged (share roots, `tool_paths`, `keep_tuning_evidence` on, `tv_root` unset), checked with `lazymkvctl settings get`; leave experiment 9's clips in the test library (emptying it stays PR 6's prerequisite). Waits: Task 20 step 8.
20. **The redeploy.** PR 3's Owner decisions 3 (one yes for `engine-task.ps1`'s registrations in phase 2) and 13 (`-AcceptWritableBuild`) carry into PR 4, and stopping the PR 3 engine with `lazymkvctl stop` over SSH before the redeploy needs no further yes. Recommended: confirm. Waits: Task 20 steps 4 and 5.
21. **PR 3's several-playlist UHD discs on the mini PC.** Task 18's scenario shows on CI that an identity picks the main title among feature-length playlists (engine.md "Pull request 4", the UHD item). Optionally, after the A3 disc and with its own yes, the owner inserts one of those UHD discs and the controller stops the engine with `lazymkvctl stop --force` once the job is `Ripping` (T3-25-c's pattern). Recommended: optional, only with that separate yes. Waits: Task 20 step 11.
22. **The cleanup before the tuning evidence exists.** Design part 5.9 skips the staging data of a job whose tuning evidence is still pending, and the evidence copy arrives in pull request 5. Recommended: until then, a `Filed` job whose own settings have `keep_tuning_evidence` on counts as evidence-pending, so PR 4's cleanup deletes no staging data of such a job (the A3 run's staging stays until PR 5 or the owner removes it). Waits: Task 0 Step 1; Task 11.
23. **dev's local refetch and the evidence clone.** Design part 8.4 has a local `plan`, `decide` or `retune` fetch a stale copy again with `TMDB_TOKEN` and replace the file. Recommended: it replaces the copy in place in the clone too, as phase 0's refresh did, and the owner commits or reverts it there; the goldens call dev's library (`golden::check`), never a gated command entry, so they never fetch; every test that calls `plan`, `decide` or `retune`'s `run` uses `Ctx::for_test`'s fixed clock (2026-10-13), so no test meets a stale clone copy. An engine job folder is never fetched into by dev (Ruling T4-R10). Waits: Task 0 Step 1; Task 10.
24. **Census and stale copies.** Recommended: `census` is a report, never fetches and never exits 3 for a stale copy; it prints the title it reads. Waits: Task 0 Step 1; Task 16.
25. **The path edge `dev -> clients`.** Design part 8.4 has the `dev` library and `lazymkv-dev` fetch with `TMDB_TOKEN`, and only `clients` holds a transport, so `lazymkv-dev` gains `lazymkv-clients` (design part 4.1's table amended; no cycle: `clients` depends on `core` alone). It is part of A1's lock delta; the table amendment is recorded as Ruling T4-R23 in engine.md (the design file stays unedited, as PR 3 recorded its amendments). Recommended: yes. Waits: Task 0 Step 3 (with A1's plan-time list); Task 1 Step 4.
26. **Rulings this plan makes (the owner may overturn any; an overturn reopens its task).** Recorded in engine.md under "Pull request 4" by Task 21.
    - **T4-R1 (status words, design part 7.1 "one line per lookup").** `lookup-status.txt` keys: `dischash`, `discid`, `search` (the cleaned name), `search-tagged` (J8's second search), `search-entry` (S47), `tmdb <kind>/<id>` (the record), `season-<n>` (each season answer); `job.log`'s `lookup <word>: ...` uses the same words, the record's word staying `tmdb record`. Each key's first word is unique, so dev's `lookup_statuses` (first word, last word) holds. R23.2's test guard (`testkit.rs`) changes its rule from a count to one per kind. Each `url` of the merged `http.json`'s `answers` is read after the same variable substitution `RecordedTransport::from_file(file, &vars)` applies (`testkit.rs:1601-1607`); every distinct GET URL under `lazymkv_clients::DISCDB` or `TMDB` (`lookups.rs:13`, `:16`) then needs a status line of its kind in `lookup-status.txt`, and no line may end in `000`: a dischash URL needs a `dischash` line, a discid URL a `discid` line, a record URL (`/3/movie/<id>` or `/3/tv/<id>` with `append_to_response=external_ids`) its `tmdb <kind>/<id>` line, a season URL (`/3/tv/<id>/season/<n>`) its `season-<n>` line, and the search family (URLs under `/3/search/`, and any URL that still starts with `{` after the substitution, which is a search URL) needs at least as many lines whose key is `search`, `search-tagged` or `search-entry` as there are distinct search URLs. A URL under neither root (a webhook, http or https) needs no line, and `http.json` holds GET answers only (a webhook POST is answered through `h.http().answer`, never recorded there); a URL of none of the kinds above needs none either (no scenario records one). So an entry title equal to the cleaned name gives a `search` and a `search-entry` line for one search URL and passes, and a webhook URL, answered through `h.http().answer`, never in `http.json`, would need no line if it were there. Tasks 2, 18.
    - **T4-R2 (one search file).** `tmdb-search.json` holds the page the plan reads: the entry-title page when the entry search applies (core's `entry_search_title` names a title), else the merged name page; its `queries` say which. The speculative name page is saved there first and replaced when the entry page answers. Once `entry_search_title` names a title, a name search's answer (a tagged search answering in a later round included) updates the book but never writes `tmdb-search.json`. `rebuild_book` puts the file's page in `answers.entry` when the status file holds an answered `search-entry` line, else in `answers.search`. Task 3.
    - **T4-R3 (partial J8).** The two name searches are two lookups; TMDb stays pending until both answer; the file is written with the cleaned page and one query, then rewritten merged with both queries. Task 2.
    - **T4-R4 (seasons).** With a parsed season, that season only, fetched even when the record does not list it (a 404 is a miss); with none, every season the record lists, season 0 included (design part 8.2 item 5's letter); spec 8.1's runtime check still leaves season 0 out (`episode_runtimes(None)`). Each answer's episodes are spliced into `tmdb-record.json`; no season file is saved. Task 4.
    - **T4-R5 (the entry search's query).** The entry's title alone (S20: `query` is the only parameter; core checks the year). Task 3.
    - **T4-R6 (the remembered identity in the chain).** A round's `ChainInputs.remembered` is the job's own `remembered.json` copy's TMDb identity when it exists (V14), else the data folder's `remembered/<FP>.json`, whose inner fingerprint must match (ASCII case ignored, dev's sentence); a refused file gives none, and `round_inputs` writes the same `engine.log` line Plan writes today (`<id>: the remembered decision is refused: <reason>`, `steps.rs:930-933`) through `Store::log`. Task 5.
    - **T4-R7 (stale).** A copy is stale when its `fetched_at` is missing or malformed, or when today is later than `fetched_at` plus six calendar months, the day clamped to the month's last day (2026-08-31 + 6 = 2027-02-28); a `fetched_at` after today is fresh; a malformed `today` makes nothing stale. A missing `fetched_at` makes a copy stale only in an engine job folder (one holding `state.json`); outside one, a copy without `fetched_at` is read as it is (the owner's copy, design part 8.4; dev's retune fixtures write `tmdb.json` without one). A missing copy counts as deleted only in an engine job folder whose status line for it says `200`. `stale_copies(.., job_folder, ..)` applies both. The cleanup deletes only a copy whose `fetched_at` reads as a date more than six months old (design part 5.9's letter); a copy whose `fetched_at` is missing or malformed is left to the refetch, which rewrites it with today's date. Tasks 8, 11.
    - **T4-R8 (restart stays offline).** `rebuild_book` reads a stale or deleted TMDb copy as `Pending { Stale(<file>) }`, so the engine's start sends no request (design part 4.4) and the next round or refetch asks again. Task 9.
    - **T4-R9 (where the engine refetches).** Before every Plan (a job's first finds its copies fetched that day and asks nothing; a running round makes Plan skip it, R23.1), before every Decide, and on the relay's own thread before `child_words`; the TMDb ID from the plan in `state.json`, the queries from the file or, for a deleted page, `core::lookup::search_queries`. The relay's exit 3 is a `command_output` line `{"stream":"stderr","line":"lazymkvctl: <reason>"}` then `ok {"exit_code": 3}`, the only form under which `lazymkvctl` exits 3 (design part 9.5's "the command exits 3"). The relay thread gets `Arc::clone(&job.lookups)`, so its refetch updates the job's book. `retune --jobs` keeps W25's `RETUNE_WAITS` refusal (exit 2) in PR 4; its exit 3 arrives with pull request 5. Task 9.
    - **T4-R10 (dev's fetch).** `Ctx` gains `http`; `Ctx::from_process` leaves it None and `Ctx::with_live_transport` sets `UreqTransport::new()`, called only by the two binaries' `main`; `TMDB_TOKEN` is read from `ctx.env` (`from_process` holds every variable, `for_test` none). The gate sits at the `plan`, `decide` and `retune` command entries only, so the engine's in-process use of dev's library never fetches. A folder holding `state.json` (an engine job folder) is read as it is, without the gate: the engine owns its copies (it fetches them before Plan, Decide and the relay's words, T4-R9; the cleanup deletes them), nothing outside the engine writes in a job folder, and the relayed child (which runs `plan <folder>` or `decide --rec <folder>\<key>`, `relay.rs:78-82`) meets copies the engine fetched with its own date. A `--tmdb-search` or `--tmdb-record` file given by hand is read as it is; only the folder's own copies are gated. Task 10.
    - **T4-R11 (the cleanup's schedule and reach).** `EngineConfig.cleanup_minutes` (None in tests, `Some(60)` in the binary) drives a thread of its own, not `Services.timers` (twenty tests pin the pending timer list); the owner thread builds the pass's job list, the cleanup thread deletes after `ShareConnector::reach`; the job's own settings' `staging_root` (ENG-19); a job whose book a round holds is skipped that pass; only `<32 lower-case hex>-<n>` folder names are deleted; `engine.log` lines `cleanup: <job id>: deleted <file> (fetched_at <date>)`, `cleanup: <job id>: deleted its staging folder`, `cleanup: <job id>: the staging folder was not reached: <reason>`. Task 11.
    - **T4-R12 (transport).** `HttpTransport` gains `post` and `connect(host, port)`; no redirect is followed (a 3xx is `Status(3xx)`, so a bearer token never reaches another host); no proxy is read from the environment; an answer over 8 MiB is `Unreachable(BODY_TOO_LONG)`; errors use a fixed vocabulary; POSTs carry `Content-Type: application/json` (Home Assistant fills `trigger.json` only then). Tasks 6, 7, 12.
    - **T4-R13 (notifier).** Both URLs unset: nothing is logged (part 8.6: "skipped"). A failed channel's reason is exactly one of `<name>: HTTP <n> after 3 attempts`, `<name>: no answer within 10 s after 3 attempts`, `<name>: <transport words> after 3 attempts`, or `<name>: <outcome> after <n> attempt[s] (the engine was stopping)` (`attempt` for 1, `attempts` above), several joined with `"; "`; a 0 s wait calls no pause. At stop the sink skips every wait and gives each channel one attempt per queued notification, and `EngineHandle::stop` waits at most `NOTIFY_DRAIN_S` (15 s) for the notifier (part 8.6: "a restart loses a notification still being retried"). Task 12.
    - **T4-R14 (the self-test's webhook checks).** `ha_webhook_url` then `webhook_url`, after `tmdb_token`; an unset URL is ok (`not set; its notifications are skipped`, part 12.7 makes the generic webhook optional); the check is a TCP connection to the URL's host and port within 10 s, no TLS handshake (part 8.6: "opens a connection to each URL's host"); details show `<host>:<port>` only. Task 13.
    - **T4-R15 (a webhook value's shape).** `secret_set` refuses a webhook value that is not an http or https URL with a host: `<name> could not be stored: the value must be an http or https URL with a host`, never echoing it. **An addition beyond the sources** (S77 already shows `host: null` for such a value; no source asks for the refusal): the owner's yes is asked (Task 0 Step 1); on a no, Task 13 drops the `owner.rs:2591-2604` change and its test. Task 13.
    - **T4-R16 (W26's doubles).** `NoDelivery` and `NO_DELIVERY` are removed; `OfflineTransport` stays as a test double with `OFFLINE = "the offline transport asks nothing (a test double)"`; `Harness::use_binary_services` becomes `use_offline_services` (OfflineTransport and a `WebhookSink` over the harness's secrets). `NoDelivery` stays a test double from Task 12 until Task 17, which removes it with the binary's change (`native.rs:31, :188`, `engine_native.rs:130` use it). Tasks 6, 12, 17.
    - **T4-R17 (dev's readers).** `hit_rate::lookup` stays the Python port (pinned output); `plan`'s reader and `census` read TheDiscDB answers with an S46 reader of their own and read any key `search`, `search-tagged`, `search-entry` or `season-<n>` as TMDb pending when present and not `200` or `404` (as the book's `pending()` does), whether or not `--tmdb-search` is given (SR8 unchanged). **A narrowing of engine.md's "Pull request 4" item** (it names `hit_rate::lookup` as the divergence to align): the owner's yes is asked (Task 0 Step 1); on a no, Task 15 also aligns `hit_rate::lookup` with S46 (its Step 3's last bullet). Overturned 2026-11-04: `hit_rate::lookup` is aligned with S46 too. Task 15.
    - **T4-R18 (4 MiB).** `lazymkvctl` checks the whole serialised request line against `MAX_LINE_BYTES` before connecting, for every command (so `secret set` too), exit 2 with `REQUEST_TOO_LONG`. Task 14.
    - **T4-R19 (live records).** A shape record replaces every scalar with its JSON type name (`"string"`, `"number"`, `"boolean"`, `"null"`), keeps objects' keys, and turns an array into a one-element array of its elements' merged shape (keys united, differing types joined `"number|null"`); the live tests date their records with the UTC date, and the slim records' `fetched_at` is that same UTC date (`clients` has no `platform` edge); the replay of saved records lives in `clients`' tests only and runs core's parsers (design part 8.5's "core and clients tests"). **A narrowing of design part 8.5** ("The core and clients tests replay the saved records"): the owner's yes is asked (Task 0 Step 1); on a no, Task 19 adds the same replay as `crates/core/tests/live_records.rs` over core's parsers, with the same skip lines. Task 19.
    - **T4-R20 (the UHD several-playlist item).** PR 4 closes the identified case on CI: synthetic scenarios with two feature-length playlists and with three of the same length, each with a TheDiscDB record whose `MainMovie` item names the main playlist, rip that one title and file. It does not close the unidentified case: a disc with several same-length playlists that TheDiscDB does not know, identified by TMDb alone, still has no main-title pick and plans copy mode (design part 5.5, phase 3). The third disc of PR 3's run that failed from `Scanned` with the copy-mode message is re-read in item 21's optional run. Task 18.
    - **T4-R21 (the unwritable remembered copy, the second half of engine.md's V14 item).** Accepted as V14 left it, no code: Decide reads only the job's own copy (R-B1: `decide.json` equals `lazymkvctl decide --remembered <job>/remembered.json`), so a copy that cannot be written plans with the remembered identity and titles and decides without the track overrides, and the job's `engine.log` line names the failure. Pinned by Task 5's `an_unwritable_remembered_copy_decides_without_overrides_and_logs_it`. **A narrowing of engine.md's V14 item** (accepted with no code): the owner's yes is asked (Task 0 Step 1); on a no, the owner names the behaviour wanted and Task 5 reopens before it starts. Task 5.
    - **T4-R22 (the census).** The main title is the longest by seconds, the lower number on a tie; an unknown title prints `?`; the English count takes `eng` streams only and leaves out derived-forced-only ones; UHD when `meta.json`'s `format` is `bluray` and the main title's height is 2000 or more (TheDiscDB's `format` is never read); a recording without `scan.txt` is listed with `?` columns and is not an unreadable file (exit 0); exit 2 on an unreadable file (hit-rate's crash rule); `kind` maps row14's `Media::Film` to `Kind::Movie`, `Media::Tv` to `Kind::Tv`, `Media::Unknown` to `?`; `census_row` passes `&Settings::default()` to `row14::media`. The test's recordings are built at test time from public files under `corpus/scenarios/` (`disc-a-scan`, `movie-direct`), `corpus/thediscdb/` and `corpus/thediscdb-api/`, all in RP21's public list (`clone_values.rs`); `corpus/scenarios/` alone holds no whole recording, so A9's "public scenarios under `corpus/scenarios/`" is read as "public files under `corpus/`"; the clone is never read by a test. Task 16.
    - **T4-R23 (design part 4.1's crates table).** The `dev` row's dependencies become `core`, `ocr` (R-A5) and `clients` (item 25), recorded in engine.md's crates table; the design file stays unedited. Task 21.

No install on the mini PC or the Mac and no firewall rule is needed: the live transport makes outbound connections only, and `ring` builds with the MSVC tools already there (Task 1 stops if it asks for nasm, CMake or perl). The only new standing state is the owner's two Credential Manager entries, `lazymkv:tmdb_token` and `lazymkv:ha_webhook_url`, made by the owner's own `secret set` at Task 20 Step 7 (A6, A7; pull request 3 left none) and listed in Task 20 Step 12's leftovers. Any change the plan did not foresee goes back to the owner by name.

## File structure

| Path | Change | Responsibility |
|---|---|---|
| `Cargo.toml:10-21` | Modify | `ureq` in `[workspace.dependencies]` (Task 1) |
| `Cargo.lock` | Modify | The approved ureq stack and the changed dependency lists of `lazymkv-clients` and `lazymkv-dev` (Task 1) |
| `crates/clients/Cargo.toml` | Modify | `ureq = { workspace = true }` (Task 1) |
| `crates/dev/Cargo.toml` | Modify | `lazymkv-clients = { path = "../clients" }` (Task 1) |
| `crates/core/src/lookup/mod.rs:462-482` | Modify | `entry_search_title` (Task 3) after `:482`, `search_queries` beside it (Task 8) |
| `crates/core/src/lookup/tmdb.rs:323-357` | Modify | `parse_season_answer`, `merge_parsed_season`; `merge_season` composed of them (Task 4) |
| `crates/core/src/lookup/stale.rs` | Create | `STALE_MONTHS`, `TMDB_COPIES`, `add_months`, `is_stale`, `fetched_at` (Task 8); `civil_date`, the date arithmetic moved from `dev::evidence::civil_time` (Task 19) |
| `crates/core/src/ipc.rs:31` | Modify | `WEBHOOK_SECRETS` made public here (Task 6) |
| `crates/core/src/subtitle/labels.rs:381`, `crates/core/src/subtitle/labels/row14.rs:261, :430-459` | Modify | `pub mod row14`; `Media`, `media`, `recorded_discdb` made `pub` for the census (Task 16) |
| `crates/platform/src/fakes.rs:408-432` | Modify | `MemorySecrets::forget`, the test seam of a no-token engine (Task 9) |
| `crates/clients/src/lib.rs` | Modify | The lock tripwire test (Task 1); module list and re-exports: `merge_pages` (Task 2), `season_url`, `tmdb_season`, `splice_season`, `seasons_to_fetch` (Task 4), `webhook_target`, `WebhookTarget` (Task 6), `UreqTransport` and its constants (Task 7), `refetch::*`, `reason_text` (Task 8), `shape_of` (Task 19) |
| `crates/clients/src/transport.rs` | Modify | `post`, `connect`, `WebhookTarget`, `webhook_target`, `OFFLINE` reworded (Task 6) |
| `crates/clients/src/recorded.rs` | Modify | Posts and connects recorded (Task 6) |
| `crates/clients/src/live.rs` | Create | `UreqTransport` and its error vocabulary (Task 7) |
| `crates/clients/src/merge.rs` | Create | `merge_pages` (Task 2) |
| `crates/clients/src/lookups.rs` | Modify | `season_url`, `tmdb_season` (Task 4); `reason_text` and `PendingReason::Stale` (Task 8) |
| `crates/clients/src/chain.rs` | Modify | The new `Lookup` variants, `Answers` fields, `LookupBook.record_file`, `ChainInputs.remembered`, the order of part 8.2 (Tasks 2 to 5); `Stale` gives the status word `000` in the production match at `:144-150` (Task 8) |
| `crates/clients/src/saved.rs` | Modify | `slim_search` over pages (Task 2), `splice_season` (Task 4), `shape_of` (Task 19) |
| `crates/clients/src/refetch.rs` | Create | `StaleCopy`, `stale_copies`, `status_record`, `refetch_record`, `refetch_search` (Task 8) |
| `crates/clients/tests/live.rs` | Create | The `#[ignore]` live shape tests and the replay of saved records (Task 19) |
| `crates/engine/src/lib.rs:28-52` | Modify | Re-exports; `EngineConfig.cleanup_minutes`, `CLEANUP_MINUTES` (Tasks 11, 12); `NoDelivery` re-export removed (Task 17) |
| `crates/engine/src/arrival.rs:133-154` | Modify | `round_inputs` (Task 5) |
| `crates/engine/src/store.rs:236-242` | Modify | The remembered file's inner fingerprint (Task 5) |
| `crates/engine/src/steps.rs:436-476, :790-808, :907-935, :1545-1566` | Modify | `spawn_round` over `round_inputs` (Task 5); refetch before Plan and Decide (Task 9) |
| `crates/engine/src/file_phase.rs:345-390` | Modify | `lookup_word` returns `String` (Task 2); `reason_text` moved to clients (Task 8); `refetch_stale` (Task 9) |
| `crates/engine/src/restart.rs:169-250` | Modify | New words, `parse_tv`, `Stale` (Tasks 2 to 4, 9) |
| `crates/engine/src/relay.rs`, `owner.rs:2471-2505` | Modify | The refetch on the relay thread and exit 3 (Task 9) |
| `crates/engine/src/cleanup.rs` | Create | The cleanup of part 5.9 (Task 11) |
| `crates/engine/src/owner.rs:159-202, :216-330, :425-445, :928-990, :1193-1245, :1256-1260, :2591-2604` | Modify | `Msg::Cleanup`, `Msg::CleanupDone`, the schedule and `TestHooks`' `cleanup_done` and `cleanup_period` (Task 11); the notifier's stop flag and drain bound (Task 12); the webhook value's shape (Task 13) |
| `crates/engine/src/notify.rs` | Modify | `WebhookSink`, `ATTEMPT_WAITS_S`, `NOTIFY_DRAIN_S`, `NotifySink::stop_flag` (Task 12); `NoDelivery`, `NO_DELIVERY` and their test removed (Task 17) |
| `crates/engine/src/selftest.rs:162-170, :279-299, :484-496` | Modify | `webhook_check`, the two names in `first_selftest` and `selftest` (Task 13) |
| `crates/engine/src/ipc_server.rs:48, :402-411` | Modify | `WEBHOOK_SECRETS` from core; `secret_host` over `webhook_target` (Task 6) |
| `crates/engine/src/native.rs:21-89, :160-207` | Modify | `cleanup_minutes: None` in the `EngineConfig` literal at `:200-207` (Task 11); `NativeOptions.notify` and `.cleanup_minutes`, the live transport, the sink, `NoDelivery`'s use at `:31, :188` replaced (Task 17) |
| `crates/engine/src/testkit.rs:468, :595-615, :1051-1083, :1191-1203` | Modify | `run_cleanup`, `schedule_cleanup`, `busy` (Task 11); `use_offline_services`, `use_webhook_sink` (Task 12); R23.2's guard checks GET URLs per kind (Task 18); `new_native`'s `notify` and `cleanup_minutes` (Task 17) |
| `crates/dev/src/evidence.rs:20-60, :137-150` | Modify | `Ctx.http`, `Ctx::with_live_transport` (Task 10); `civil_time`'s date part calls core's `civil_date` (Task 19) |
| `crates/dev/src/main.rs:7`, `crates/ctl/src/main.rs:7` | Modify | `Ctx::from_process().with_live_transport()` (Task 10) |
| `crates/ctl/src/tests.rs` | Modify | A local form's exit 3 (Task 10) |
| `crates/dev/src/stale.rs` | Create | `refresh_copies` and the exit-3 texts (Task 10) |
| `crates/dev/src/cmd/{plan,decide,retune}.rs` | Modify | The gate at `run` (Task 10); plan's S46 reader and search and season words (Task 15) |
| `crates/dev/src/cmd/hit_rate.rs` | Modify | `lookup` reads TheDiscDB answers with the S46 reader `parse_dischash` (Task 15) |
| `crates/dev/src/cmd/census.rs`, `crates/dev/src/cmd/mod.rs`, `crates/dev/src/lib.rs:5-9, :24-44` | Create, Modify | `pub mod stale;` (Task 10); `lazymkv-dev census` (Task 16) |
| `crates/ctl/src/engine_cmds.rs:8, :25-34, :36-48, :50-64, :206-220, :248-272, :381-` | Modify | `REQUEST_TOO_LONG` before connecting (Task 14) |
| `crates/fake-tools/tests/{engine_ipc,engine_disc_phase,engine_file_phase,engine_restart,engine_native,whole_job,engine_command}.rs` | Modify | Engine tests over the new lookups, the remembered copy, the sink, the cleanup and the refetch (Tasks 5, 9, 11, 12, 13, 17, 18) |
| `corpus/scenarios/movie-direct-tagged/`, `corpus/scenarios/movie-direct-main-playlist/`, `corpus/scenarios/movie-direct-three-playlists/`, `corpus/scenarios/tv-direct-series/` | Create | Synthetic scenarios: J8's two searches; two feature-length playlists and three of the same length, each named by a TheDiscDB `MainMovie` item; a TheDiscDB `Series` record with its `/tv/{id}` and season answers (Task 18) |
| `docs/superpowers/plans/2026-11-04-phase-2-pr4-live-lookups.md` | Add | This plan, committed with Task 1 |
| `README.md`, `docs/phase2/engine.md` | Modify | Crate list; the PR 4 part, rulings, the crates table's `dev` row (T4-R23), mini PC runs and test count (Task 21) |

## Interface sheet

The items one task produces for another, used exactly as written; each task's Interfaces block fixes its crate-private items. Every item compiles and is tested on all three systems unless marked.

### core

```rust
// core::lookup (mod.rs; spec 8.1, design part 8.2 item 3, S47)
pub fn entry_search_title(inputs: &LookupInputs<'_>) -> Option<String>; // T4-3: the trimmed title of the record
    // discdb_match names, when that record has a title and no TMDb ID; None otherwise
pub fn search_queries(inputs: &LookupInputs<'_>) -> Vec<String>; // T4-8: [entry title] when entry_search_title is Some;
    // else, for a non-generic name, [cleaned] and then tagged when fold_name(tagged) != fold_name(cleaned); else []

// core::lookup::tmdb (design part 8.2 item 5)
pub fn parse_season_answer(text: &str, shown: &str) -> Result<TmdbSeason, String>; // T4-4: a /tv/{id}/season/{n} answer
pub fn merge_parsed_season(record: &mut TmdbRecord, season: TmdbSeason) -> Result<(), String>; // T4-4: merge_season's
    // rule (Err for a movie record; replaces that season's episodes; adds a missing season sorted; absent episode_count
    // takes the merged count); merge_season(record, text, shown) == parse then merge, its tests unchanged

// core::lookup::stale (design part 8.4, R-B4; Ruling T4-R7)
pub const STALE_MONTHS: u32 = 6;
pub const TMDB_COPIES: [&str; 3] = ["tmdb.json", "tmdb-record.json", "tmdb-search.json"];
pub fn add_months(date: &str, months: u32) -> Option<String>; // "yyyy-MM-dd"; day clamped to the month's last day;
    // None when date is not yyyy-MM-dd with a real month and day
pub fn is_stale(fetched_at: Option<&str>, today: &str) -> bool; // false when today is malformed; true when fetched_at is
    // None or malformed; true when today > add_months(fetched_at, STALE_MONTHS) (string compare of yyyy-MM-dd)
pub fn fetched_at(text: &str) -> Option<String>; // the top-level "fetched_at" string of a copy's JSON text, else None
pub fn civil_date(unix_secs: u64) -> (i64, u32, u32); // T4-19: (year, month, day) of seconds since 1970-01-01 UTC,
    // the days-to-civil algorithm moved from dev's civil_time, which calls it; clients' tests/live.rs formats it

// core::ipc (design part 8.6, S77)
pub const WEBHOOK_SECRETS: [&str; 2] = ["ha_webhook_url", "webhook_url"]; // channel order of the notifier and the self-test
```

### clients

```rust
// transport.rs (design parts 8.1, 8.6; Rulings T4-R12, T4-R16)
pub trait HttpTransport: Send + Sync {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError>;
    fn post(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Response, TransportError>; // T4-6
    fn connect(&self, host: &str, port: u16) -> Result<(), TransportError>; // T4-6: a connection only, no request
}   // every status, 3xx included, is Ok(Response); errors are Timeout or Unreachable(<fixed words>) only from live
pub const OFFLINE: &str = "the offline transport asks nothing (a test double)";
#[derive(Clone, Debug, PartialEq, Eq)] pub struct WebhookTarget { pub https: bool, pub host: String, pub port: u16 }
pub fn webhook_target(url: &str) -> Option<WebhookTarget>; // T4-6: scheme http (port 80) or https (443), ASCII case
    // ignored; host as secret_host reads it (userinfo dropped, [v6] unbracketed); explicit port when given; None for any
    // other scheme, no host, whitespace, or a port that is not 1..=65535

// recorded.rs (S63, S64; T4-6)
impl RecordedTransport {
    pub fn posts(&self) -> Vec<(String, Vec<(String, String)>, Vec<u8>)>; // url, headers, body; memory only
    pub fn connect_answer(&self, host: &str, port: u16, outcome: Result<(), TransportError>);
    pub fn connects(&self) -> Vec<(String, u16)>;
}   // a POST is answered from the same per-URL queue as a GET (answer/fail); requests() keeps listing GETs only;
    // an unrecorded connect is Err(Unrecorded("<host>:<port>"))

// live.rs (design part 8.1; Ruling T4-R12; T4-7)
pub struct UreqTransport { /* ureq::Agent */ }
impl UreqTransport {
    pub fn new() -> UreqTransport;                         // with_timeout(Duration::from_secs(TIMEOUT_S))
    pub fn with_timeout(timeout: Duration) -> UreqTransport; // global timeout per request, no redirect, no proxy,
                                                           // status never an error, User-Agent from the caller's headers
    pub fn timeout(&self) -> Duration;                     // the configured timeout (Task 7's production test)
}
impl HttpTransport for UreqTransport {}
pub const MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;
pub const REFUSED: &str = "the connection was refused";
pub const NO_HOST: &str = "the host name could not be resolved";
pub const TLS_FAILED: &str = "the TLS handshake failed";
pub const CONNECTION_FAILED: &str = "the connection failed";
pub const BODY_TOO_LONG: &str = "the answer is longer than 8 MiB";
pub const BAD_ANSWER: &str = "the answer could not be read";

// merge.rs (design part 8.2 item 2, J8; T4-2)
pub fn merge_pages(cleaned: &TmdbSearch, tagged: &TmdbSearch) -> TmdbSearch; // cleaned's results in order, then
    // tagged's results whose identity() is not already among them, in order

// lookups.rs
pub fn season_url(show: i64, season: u32) -> String; // T4-4: "{TMDB}/tv/{show}/season/{season}"
pub fn tmdb_season(t: &dyn HttpTransport, token: Option<&str>, show: i64, season: u32) -> Outcome<TmdbSeason>; // T4-4
pub enum PendingReason { NoToken, TokenRefused, Timeout, Unreachable(String), Status(u16), Unparsed(String),
    Stale(String) } // T4-8: Stale(file) from rebuild_book (T4-R8) and refetch_stale (T4-R9); status word 000
pub fn reason_text(reason: &PendingReason) -> String; // T4-8, moved from engine::file_phase (R24.6): "no tmdb_token is
    // set" | "TMDb refused the token (HTTP 401)" | "no answer within 10 s" | <text> | "HTTP <n>" | <sentence> |
    // "<file> is more than six months old or was deleted, and is fetched again"

// chain.rs (design part 8.2; Rulings T4-R1 to T4-R6)
pub enum Lookup { Dischash, Discid, Search, TaggedSearch, EntrySearch, Record, Season(u32) } // Copy, Ord: chain order
pub struct Answers { pub by_fingerprint: Option<DiscDbRecord>, pub by_disc_id: Vec<DiscDbRecord>,
    pub search: Option<TmdbSearch> /* the name page, merged once the tagged page answered */,
    pub tagged: Option<TmdbSearch>, pub entry: Option<TmdbSearch>,
    pub record: Option<TmdbRecord> /* season answers merged */ }
pub struct LookupBook { pub states: BTreeMap<Lookup, LookupState>, pub answers: Answers,
    pub record_file: Option<Value> /* the tmdb-record.json last saved, for splice_season */ }
pub struct ChainInputs { pub fingerprint: String, pub disc_id: Option<String>, pub name: Option<DiscName>,
    pub remembered: Option<TmdbIdentity> } // T4-5
impl LookupBook {
    pub fn pending(&self) -> (bool, bool); // tmdb adds waiting(TaggedSearch | EntrySearch | Season(_))
    pub fn lookup_inputs<'a>(&'a self, name: Option<&'a DiscName>) -> LookupInputs<'a>; // search = entry page when
        // entry_search_title applies, else the merged name page (T4-R2)
}
pub fn run_round(t: &dyn HttpTransport, token: Option<&str>, book: &mut LookupBook, inputs: &ChainInputs,
                 today: &str, save: &mut dyn FnMut(SavedFile)); // signature unchanged
pub fn seasons_to_fetch(record: &TmdbRecord, name: Option<&DiscName>) -> Vec<u32>; // T4-4: [n] for a parsed season n,
    // else every season the record lists, season 0 included (T4-R4); empty for a movie record

// saved.rs
pub fn slim_search(pages: &[(&Value, &str)], today: &str) -> Value; // T4-2: (raw page, query) in query order; results
    // merged as merge_pages merges them; "queries": [each query]; fetched_at last
pub fn splice_season(record_file: &mut Value, season_raw: &Value); // T4-4: that season's
    // episodes[{episode_number, runtime}] set (episode_count added when absent), a missing season added in number order
pub fn shape_of(value: &Value) -> Value; // T4-19 (Ruling T4-R19)

// refetch.rs (design part 8.4; T4-8)
#[derive(Clone, Debug, PartialEq, Eq)] pub struct StaleCopy { pub file: &'static str, pub fetched_at: Option<String> }
pub fn stale_copies(status_text: &str, read: &dyn Fn(&str) -> Option<Vec<u8>>, job_folder: bool, today: &str)
    -> Vec<StaleCopy>; // in TMDB_COPIES order; Ruling T4-R7's missing-copy rule
pub fn status_record(status_text: &str) -> Option<TmdbIdentity>; // the `tmdb <kind>/<id>` line's identity
pub fn refetch_record(t: &dyn HttpTransport, token: Option<&str>, id: TmdbIdentity, season: Option<u32>, today: &str)
    -> Result<(TmdbRecord, Vec<SavedFile>), PendingReason>; // tmdb.json then tmdb-record.json; a tv record's seasons
    // as Ruling T4-R4; a record 404 is Err(Status(404))
pub fn refetch_search(t: &dyn HttpTransport, token: Option<&str>, queries: &[String], today: &str)
    -> Result<(TmdbSearch, SavedFile), PendingReason>; // each query in order, merged as J8 says;
    // Err(Unparsed("no query to search")) for an empty list
```

### engine

```rust
// lib.rs (design part 5.9; Ruling T4-R11)
pub struct EngineConfig { /* PR 3 fields */ pub cleanup_minutes: Option<u32> } // None: no pass at start, no schedule
pub const CLEANUP_MINUTES: u32 = 60;
pub use notify::{NotifySink, WebhookSink, ATTEMPT_WAITS_S, NOTIFY_DRAIN_S}; // Task 12; NoDelivery, NO_DELIVERY stay
    // re-exported as a test double until Task 17 removes them with the binary's change (Ruling T4-R16)

// notify.rs (design part 8.6, spec 12; Ruling T4-R13)
pub const ATTEMPT_WAITS_S: [u64; 3] = [0, 1, 4]; // the wait before each attempt, counted from the previous failure
pub const NOTIFY_DRAIN_S: u64 = 15;
pub trait NotifySink: Send {
    fn deliver(&mut self, notification: &Notification) -> Result<(), String>;
    fn stop_flag(&self) -> Option<Arc<AtomicBool>> { None } // T4-12: set by EngineHandle::stop
}
pub struct WebhookSink { /* http, secrets, pause, stopping */ }
impl WebhookSink {
    pub fn new(http: Arc<dyn HttpTransport>, secrets: Arc<dyn SecretStore>) -> WebhookSink; // pause: 50 ms steps until the
        // wait has passed or `stopping` is raised (Task 12)
    pub fn with_pause(self, pause: Arc<dyn Fn(Duration) + Send + Sync>) -> WebhookSink;
}
impl NotifySink for WebhookSink {} // POST serde_json::to_vec(notification) with Content-Type: application/json and
    // User-Agent: user_agent() to each set channel in WEBHOOK_SECRETS order; any 2xx delivers

// native.rs (cfg(windows); W26 replaced; T4-17)
pub struct NativeOptions { /* PR 3 fields */ pub notify: Option<Box<dyn NotifySink>>, pub cleanup_minutes: Option<u32> }
    // notify None: WebhookSink over the run's secrets and http; http None: UreqTransport::new();
    // production(): notify None, cleanup_minutes Some(CLEANUP_MINUTES); new_native: a test sink, cleanup_minutes None

// owner.rs (crate-private test seam; T4-11)
pub(crate) struct TestHooks { /* PR 3 fields */ pub(crate) cleanup_done: Option<Sender<()>>,
    pub(crate) cleanup_period: Option<Duration> } // cleanup_period replaces cleanup_minutes' period in tests only

// testkit.rs (T4-11, T4-12, T4-17)
impl Harness {
    pub fn use_offline_services(&self);  // replaces use_binary_services: OfflineTransport + WebhookSink (no-op pause)
    pub fn use_webhook_sink(&self);      // the harness's RecordedTransport + WebhookSink over its MemorySecrets, no-op pause
    pub fn run_cleanup(&self);           // one pass through Msg::Cleanup; returns when TestHooks.cleanup_done fires
                                         // (scaled wait)
    pub fn schedule_cleanup(&self, period: Duration); // from the next start, cleanup_minutes Some(1), period replaced
    pub fn busy(&self) -> bool;          // the engine's busy flag (R24.c1); false when no engine runs
}
```

### dev

```rust
// evidence.rs (design part 8.4; Ruling T4-R10)
pub struct Ctx { /* PR 3 fields */ pub http: Option<Arc<dyn HttpTransport>> } // from_process: None; for_test: None
impl Ctx { pub fn with_live_transport(self) -> Ctx; } // http = Some(UreqTransport::new()); called only by the two main.rs
    // TMDB_TOKEN is read from ctx.env (from_process holds every variable; for_test none)

// stale.rs (K15; T4-10)
pub fn refresh_copies(ctx: &Ctx, folder: &Path, shown: &str) -> Result<Vec<String>, Refused>; // the files fetched
    // again; Refused { code: 3, line } naming the copy; never the token; Ok(vec![]) for a folder holding state.json

// cmd/plan.rs (S46, T16-r1; Ruling T4-R17; T4-15)
pub fn discdb_answer(tree: &dyn Tree, path: &PathKey, status: Option<&str>, discid: bool) -> &'static str;
    // "hit" | "miss" | "failed" | "n/a" by parse_dischash / parse_discid

// cmd/hit_rate.rs (S46; Ruling T4-R17 overturned 2026-11-04; T4-15)
pub fn lookup(tree: &dyn Tree, path: &PathKey, status: Option<&str>, discid: bool) -> &'static str;
    // plan::discdb_answer(tree, path, status, discid); report passes false for the hash file, true for the discid one

// cmd/census.rs (A9; T4-16)
pub const USAGE: &str = "census [--evidence DIR]";
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32;
pub fn report(tree: &dyn Tree, root: &PathKey, out: &mut dyn Write) -> Result<i32, String>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum DiscType { Uhd, BluRay, Dvd } // as_str "UHD" "Blu-ray" "DVD"
pub struct CensusRow { pub slug: String, pub title: Option<String>, pub year: Option<i32>,
    pub kind: Option<Kind> /* Media::Film -> Movie, Media::Tv -> Tv, Media::Unknown -> None */,
    pub discdb: &'static str, pub english_subtitles: Option<usize>, pub object_audio: Vec<ObjectAudio>,
    pub disc_type: Option<DiscType> }
pub fn census_row(tree: &dyn Tree, disc: &PathKey) -> Result<CensusRow, String>;
pub fn main_title(disc: &Disc) -> Option<&Title>; // longest by seconds, the lower number on a tie
```

### ctl

```rust
// engine_cmds.rs (engine.md "Pull request 4"; Ruling T4-R18; T4-14)
pub(crate) const REQUEST_TOO_LONG: &str = "the request is longer than 4 MiB, the most the engine reads in one line";
```

### Files and words

- Job folder files (design part 7.1): `thediscdb-hash.json`, `thediscdb-discid.json`, `tmdb.json`, `tmdb-record.json`, `tmdb-search.json`, `lookup-status.txt`; no season file.
- Status keys (T4-R1): `dischash`, `discid`, `search`, `search-tagged`, `search-entry`, `tmdb <kind>/<id>`, `season-<n>`; words `<HTTP status>`, `000`, `unparsed`.
- Environment: `TMDB_TOKEN` (dev and the live tests only), `LAZYMKV_EVIDENCE`, `CI`.
- Exit codes: 3 for a stale copy that cannot be fetched again (`plan`, `decide`, `retune` local over a folder that is not an engine job folder; `plan --job` and `decide --job` relayed, K15); 2 for `REQUEST_TOO_LONG` and for `retune --jobs` through the relay (W25, until pull request 5).

## Tasks

### Task 0: The plan-time answers, A2's merges and the baseline (A1, A2; owner and controller, manual)

Nothing here changes a tracked file; every record goes to the git-ignored `.superpowers/pr4/` folder. `BOX '<cmd>'` is `ssh -o BatchMode=yes "$LAZYMKV_BOX" '<cmd>'`, with `LAZYMKV_BOX` from `scripts/phase0/box.env` (Task 20 uses the same form).

**Files:** Create (git-ignored, never committed): `.superpowers/pr4/decisions.txt`, `.superpowers/pr4/pr4-lock-delta-plan.txt`, `.superpowers/pr4/baseline-list.txt`, `.superpowers/pr4/pr4-mini-pc-runs.txt`.

**Interfaces:**
- Consumes: Owner prerequisites items 17, 22 to 25 and item 26's rulings; A2's five Dependabot pull requests; Task 1 Step 1's read rules and form.
- Produces: `decisions.txt` holding the plan-time answers (Tasks 1, 5, 10, 11, 13, 15, 16, 19 and 21 read them); the plan-time crate list `pr4-lock-delta-plan.txt` (Task 1 Step 1 compares against it); the baseline count N0, its per-crate counts and `<from>` (Tasks 1 and 21).

- [ ] **Step 1: Owner. The plan-time answers.** The controller asks the owner, in one message, items 17, 22, 23 and 24 with the plan's recommendation for each, the four rulings that ask the owner's yes (T4-R15, T4-R17, T4-R19, T4-R21) with what a no changes, and whether any other ruling of item 26 is overturned. It writes each answer to `.superpowers/pr4/decisions.txt` as `<item>, <yyyy-MM-dd>, <answer>`. An overturn reopens the task its ruling names before that task starts.

- [ ] **Step 2: A2's merges (owner, or the controller on the owner's yes naming each pull request number).** `gh pr list --repo krcm0209/lazymkv --author app/dependabot --state open` lists the open bumps. The controller shows the five crate bumps (md-5 0.11, fancy-regex 0.20, sha1 0.11, serde_json 1.0.154, unicode-normalization 0.1.26) with their numbers and waits for a yes naming them. Then, one at a time: `gh pr merge <n> --repo krcm0209/lazymkv --squash --body "deps: <crate> <version> (A2)"` (a one-line body, so no trailer of the pull request's commits is carried into `main`), each only once that pull request's CI is green on the three systems after Dependabot has rebased it onto the previous merge (all five touch `Cargo.lock`). The rust-toolchain 1.99.1 pull request is left open and untouched (A2).

- [ ] **Step 3: Owner. The plan-time crate list (A1, fetching nothing).** On the Mac, `git switch main && git pull --ff-only`, then the controller runs Task 1 Step 1's crates.io reads against `main`'s `Cargo.lock` (metadata only, Ruling T3-1-a: nothing is downloaded) and writes `.superpowers/pr4/pr4-lock-delta-plan.txt` in Task 1 Step 1's form, with its `DIFFERS FROM THE PLAN` and `NEW LICENCE TERM` marks against Tech Stack's list. It shows the owner the file with this sentence: "Pull request 4's plan-time crate list: `ureq` <v> with rustls on ring and `webpki-roots` <version>, <n> crates added with the licences listed, and the path edge `lazymkv-dev -> lazymkv-clients` (A1, owner item 25)", reads each marked line out by name, waits for an explicit yes in chat, and writes `A1 plan time, <yyyy-MM-dd>, <answer>` and `item 25, <yyyy-MM-dd>, <answer>` to `decisions.txt`. A no stops the plan before the branch.

- [ ] **Step 4: The baseline on the mini PC (SSH).** `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` shows no active job (Global Constraints). Run: `scripts/box.sh cargo test --workspace --locked`. Expected: PASS, with `Downloaded` lines only for A2's crates, which A2's yes covers. Then `scripts/box.sh "cargo test --workspace --locked -- --list 2>&1" > .superpowers/pr4/baseline-list.txt`; `tr -d '\r' < .superpowers/pr4/baseline-list.txt | grep -c ': test$'` gives N0, Task 21 Step 2's `awk` over the same file gives the per-crate counts, and `grep -c '^\[\[package\]\]' Cargo.lock` gives `<from>`.

- [ ] **Step 5: Evidence.** `.superpowers/pr4/pr4-mini-pc-runs.txt` gains `<yyyy-MM-dd>, SSH, the baseline after A2's merges on <short sha>: N0 tests listed, <p> passed, 0 failed; Cargo.lock <from> packages`.

**Run:** as each step says. **Commit:** none.

---

### Task 1: The dependency gate (A1, design parts 4.7 and 8.1)

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`: one `ureq` line after the `windows-sys` line, line 21 at `6a862ac`; A2's merges change the lines above it).
- Modify: `crates/clients/Cargo.toml:10` (`ureq.workspace = true` after `serde_json.workspace = true`).
- Modify: `crates/dev/Cargo.toml:16` (`lazymkv-clients` after `lazymkv-core` in `[dependencies]`).
- Modify: `Cargo.lock` (written by hand on the Mac to the approved delta; Step 5's locked build checks it).
- Modify: `crates/clients/src/lib.rs:16` (a `#[cfg(test)] mod tests` after the re-exports, holding the two tripwires).
- Add: `docs/superpowers/plans/2026-11-04-phase-2-pr4-live-lookups.md` (this plan, untracked at the branch point; committed with this task, as PR 2's and PR 3's plans are tracked).
- Create (git-ignored, never committed): `.superpowers/pr4/pr4-lock-delta.txt`, `.superpowers/pr4/lock-blocks.txt`, `.superpowers/pr4/Cargo.lock.approved`, `.superpowers/pr4/pr-body.md`; `.superpowers/pr4/decisions.txt` gains a line.

**Interfaces:**
- Consumes: Task 0's `decisions.txt` (A1's plan-time yes, item 25), `pr4-lock-delta-plan.txt` and `<from>`.
- Produces: the `ureq` crate inside `lazymkv-clients` (Task 7); the path edge `lazymkv-dev -> lazymkv-clients` (Task 10; Ruling T4-R23, owner item 25). No later task edits a `Cargo.toml` dependency list.

Precondition: Task 0 is done (A2's five merges on `main`, `decisions.txt` holding the plan-time answers and A1's plan-time yes); `git switch -c phase-2-live-lookups` from that `main`; `<from>` is Task 0 Step 4's package count (45 at `6a862ac` before A2), written into the delta file.

Ruling T3-1-a carries over: reading crates.io's metadata downloads no crate, so Step 1 runs before the owner's yes; the first command that downloads anything is Step 5's `--locked` build against the hand-written entries.

- [ ] **Step 1: Read the delta from crates.io, fetching nothing**

On the Mac, run the recorded command of A1's plan-time answer:

```bash
python3 .superpowers/pr4/lock_delta.py --lock Cargo.lock --published-by 2026-10-21 --compare .superpowers/pr4/pr4-lock-delta-plan.txt --out .superpowers/pr4/pr4-lock-delta.txt --blocks .superpowers/pr4/lock-blocks.txt
```

The script reads crates.io's sparse index and API, one second between requests (crates.io's crawler policy), and downloads nothing. It resolves `ureq` with `default-features = false, features = ["rustls"]` as cargo does: an entry of kind `normal` or `build` counts when it is not optional or a feature turned on through each crate's own feature table turns it on; an entry with a `target` counts whatever the target, since `Cargo.lock` holds every target's packages; `dev` entries never count; a reached crate keeps a version the lock already holds when one meets the requirement. The version rule, for `ureq` and every crate reached: the newest non-yanked, non-prerelease release published on or before 2026-10-21, the owner's plan-time cutoff. `<v>` below is 3.5.0. Expected for `ureq` 3.5.0: a feature named `rustls` that turns on rustls with the ring provider and `webpki-roots`, and none of `aws-lc-rs`, `rustls-platform-verifier`, `native-tls`; a differently named feature is reported as the one to use.

Expected (Tech Stack, the plan-time list's names): `ureq`, `ureq-proto`, `rustls` (Apache-2.0 OR ISC OR MIT), `ring` (Apache-2.0 AND ISC), `webpki-roots` 1.x (CDLA-Permissive-2.0), `rustls-webpki`, `untrusted` (ISC), `rustls-pki-types`, and helpers among `log`, `base64`, `http`, `httparse`, `bytes`, `once_cell`, `percent-encoding`, `utf8-zero`, `zeroize`, `subtle`, `getrandom`, `wasi`, `cc`, `shlex`, `find-msvc-tools`; `cfg-if`, `itoa` and `libc` are already locked. `ring`'s target table for Windows on ARM brings a second `windows-sys` line (0.52) with `windows-targets` and its eight `windows_*` crates: they are listed like any other.

The script writes `.superpowers/pr4/pr4-lock-delta.txt` in this form, with the values read, followed by the `--compare` section:

```
Pull request 4's Cargo.lock delta (design parts 4.7 and 8.1, P33, A1), read from crates.io on <yyyy-MM-dd>
direct  ureq <v> <licence> published <date> rust_version <msrv>: lazymkv-clients, default-features = false, features = ["<feature>"]
added   <crate> <version> <licence> published <date> rust_version <msrv or none>  (one line per crate, by name)
changed lazymkv-clients: its dependencies gain "ureq"
changed lazymkv-dev: its dependencies gain "lazymkv-clients" (Ruling T4-R23, owner item 25)
packages: <from> -> <to>
newest ureq on crates.io: <max_stable_version>
```

It also writes `.superpowers/pr4/lock-blocks.txt`: the `[[package]]` blocks Step 3 copies into `Cargo.lock`, under a header that names the `"windows-sys 0.61.2"` change of `lazymkv-platform`'s dependency line.

The script marks a line `DIFFERS FROM THE PLAN` when: a crate of design part 4.7's table has another licence (for `webpki-roots`, any version not 1.x); a crate needs a tool outside cargo and the Microsoft Visual C++ (MSVC) build tools (`cmake`, `nasm`, `perl` named in a build dependency or its documentation); `rust_version` is above 1.99.0; a crate is yanked or published less than 14 days before the read; or any of `aws-lc-rs`, `aws-lc-sys`, `openssl`, `openssl-sys`, `native-tls`, `rustls-platform-verifier`, `cmake`, `tokio` or `reqwest` is reached. A licence expression holding a term other than MIT, Apache-2.0 or ISC (expected: `subtle`'s BSD-3-Clause, `wasi`'s LLVM exception) gets `NEW LICENCE TERM`. Each line is then compared with Task 0's `pr4-lock-delta-plan.txt` (the `--compare` section): a crate, version or licence that differs from it, and a crate missing from it or added to it, also gets `DIFFERS FROM THE PLAN`. Nothing else is edited in this step.

- [ ] **Step 2: Write the failing tripwires**

`crates/clients/src/lib.rs`, after line 16:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Each `[[package]]` of the workspace's Cargo.lock as (name, version).
    fn locked_packages() -> Vec<(String, String)> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut packages = Vec::new();
        let mut name = None;
        for line in text.lines() {
            if line == "[[package]]" {
                name = None;
            } else if let Some(value) = line.strip_prefix("name = ") {
                name = Some(value.trim_matches('"').to_string());
            } else if let Some(value) = line.strip_prefix("version = ") {
                if let Some(name) = name.take() {
                    packages.push((name, value.trim_matches('"').to_string()));
                }
            }
        }
        packages
    }

    #[test]
    fn the_lock_holds_ring_and_no_other_tls_provider() {
        let names: Vec<String> = locked_packages().into_iter().map(|(name, _)| name).collect();
        for wanted in ["ureq", "rustls", "ring", "webpki-roots"] {
            assert!(names.iter().any(|name| name == wanted), "Cargo.lock lacks {wanted}");
        }
        for refused in [
            "aws-lc-rs", "aws-lc-sys", "openssl", "openssl-sys", "native-tls", "rustls-platform-verifier", "cmake",
            "tokio", "reqwest",
        ] {
            assert!(!names.iter().any(|name| name == refused), "Cargo.lock holds {refused} (design part 8.1)");
        }
    }

    #[test]
    fn webpki_roots_is_a_1_x_release() {
        let versions: Vec<String> =
            locked_packages().into_iter().filter(|(name, _)| name == "webpki-roots").map(|(_, version)| version).collect();
        assert!(!versions.is_empty(), "Cargo.lock lacks webpki-roots");
        for version in versions {
            assert!(version.starts_with("1."), "webpki-roots {version} is not 1.x, the CDLA-Permissive-2.0 line (design part 4.7)");
        }
    }
}
```

Run (manifests still unchanged, and A2's crates were fetched at Task 0 Step 4, so nothing is downloaded): `scripts/box.sh cargo test -p lazymkv-clients --locked the_lock_holds`, then `scripts/box.sh cargo test -p lazymkv-clients --locked webpki_roots_is`.
Expected: FAIL, `Cargo.lock lacks ureq` and `Cargo.lock lacks webpki-roots`; `git diff --exit-code Cargo.lock` after each (box.sh copies the lock back) shows no change.

- [ ] **Step 3: Write the manifests and the lock by hand, fetching nothing**

`Cargo.toml`, after the `windows-sys` line:

```toml
# The blocking HTTP client of the lookups and the notifier: rustls on the ring provider with webpki-roots compiled in,
# default features off except TLS; used by lazymkv-clients only (design parts 4.7 and 8.1; P33, A1).
ureq = { version = "<v>", default-features = false, features = ["<feature from Step 1>"] }
```

`crates/clients/Cargo.toml`, after line 10: `ureq.workspace = true`.

`crates/dev/Cargo.toml`, after line 16:

```toml
# TMDb's transport for the local refetch of a stale copy (design part 8.4; Ruling T4-R23, owner item 25).
lazymkv-clients = { path = "../clients" }
```

`Cargo.lock`: `"ureq"` joins `lazymkv-clients`' `dependencies` after `"serde_json"`, and `"lazymkv-clients"` joins `lazymkv-dev`'s before `"lazymkv-core"`. Each Step 1 crate gets its `[[package]]` block, copied from `.superpowers/pr4/lock-blocks.txt`, at its sorted place (by name, then version): `name`, `version`, `source = "registry+https://github.com/rust-lang/crates.io-index"`, the `checksum` read, and a `dependencies` list of the entries Step 1 counted, sorted by name. When the lock holds two versions of one name, every dependency entry naming it is written `"<name> <version>"`, existing entries included (with a second `windows-sys`, `lazymkv-platform`'s `"windows-sys"` becomes `"windows-sys 0.61.2"`, as `lock-blocks.txt`'s header names). Then `grep -c '^\[\[package\]\]' Cargo.lock` must print `<to>`, and `cp Cargo.lock .superpowers/pr4/Cargo.lock.approved`.

- [ ] **Step 4: Owner. Stop for the second yes (A1, owner items 1 and 25)**

The controller shows the owner `.superpowers/pr4/pr4-lock-delta.txt` and `git diff Cargo.toml crates/clients/Cargo.toml crates/dev/Cargo.toml Cargo.lock` with this sentence: "Pull request 4's whole `Cargo.lock` delta: `ureq` <v> with rustls on ring and `webpki-roots` <version>, <n> crates added with the licences listed, <from> to <to> packages, and the path edge `lazymkv-dev -> lazymkv-clients` (design parts 4.7 and 8.1, A1)." Any `DIFFERS FROM THE PLAN` or `NEW LICENCE TERM` line is read out by name. The controller waits for an explicit yes in chat and writes `A1 before fetch, <yyyy-MM-dd>, <answer>` to `.superpowers/pr4/decisions.txt` (Task 21 carries it, with Task 0's plan-time yes, into engine.md's "The Cargo.lock delta"). A no, or a yes naming another version, sends the task back to Step 1 with that version; nothing is fetched before the yes.

- [ ] **Step 5: First fetch, of the approved entries only**

Run: `scripts/box.sh cargo build --workspace --locked`
Expected: at most an `Updating crates.io index` line, one `Downloaded` line per added crate of the delta file that `x86_64-pc-windows-msvc` builds, none for the target-only crates (`windows-sys 0.52.0`, `windows-targets 0.52.6`, the eight `windows_*` 0.52.6, `wasi`), and no `Downloaded` line for a crate outside the delta, then `Finished`. Then on the Mac: `cmp Cargo.lock .superpowers/pr4/Cargo.lock.approved` prints nothing.

`the lock file needs to be updated but --locked was passed` means the hand edit misses or adds an entry: nothing was downloaded; fix the edit against Step 1's list, never drop `--locked`. A build that asks for `nasm`, `cmake` or `perl`, a `Downloaded` line not in the delta, or a `cmp` difference stops the task and goes to the owner; nothing is committed.

- [ ] **Step 6: The tripwires pass, and the workspace**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked the_lock_holds`, then `scripts/box.sh cargo test -p lazymkv-clients --locked webpki_roots_is`.
Expected: PASS, 1 test each.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS, with no `Downloaded` line; the count is Task 0's N0 plus 2. Then `cmp Cargo.lock .superpowers/pr4/Cargo.lock.approved` prints nothing.

- [ ] **Step 7: Commit**

First, `grep -cE 'eyJhbGciOi[A-Za-z0-9_-]{20,}' docs/superpowers/plans/2026-11-04-phase-2-pr4-live-lookups.md` prints `0`: the plan holds only placeholders, sentinels and public IDs.

```bash
git add Cargo.toml Cargo.lock crates/clients/Cargo.toml crates/dev/Cargo.toml crates/clients/src/lib.rs \
  docs/superpowers/plans/2026-11-04-phase-2-pr4-live-lookups.md
git commit -m "deps: the ureq stack for the live lookups, and pull request 4's plan (A1)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, then the subject alone, with no attribution line.

- [ ] **Step 8: Push and open the draft pull request (A11)**

`.superpowers/pr4/pr-body.md` holds exactly this:

```markdown
Phase 2 pull request 4: live lookups and notifications (design part 12.6, item 4).

Draft while the plan's tasks land, one signed commit each. The plan is
docs/superpowers/plans/2026-11-04-phase-2-pr4-live-lookups.md; the owner's decisions A1 to A11 and the rulings
T4-R1 to T4-R23 are recorded in docs/phase2/engine.md by its last task.
```

```bash
git push -u origin phase-2-live-lookups
gh pr create --draft --base main --head phase-2-live-lookups \
  --title "Phase 2 pull request 4: live lookups and notifications" --body-file .superpowers/pr4/pr-body.md
```

Expected: the draft's URL. The controller waits for continuous integration (CI) on the three systems and reads the draft's CodeQL result: a red run or any alert goes back to this task before Task 2 starts.

---

### Task 2: J8's tagged search and the merged page (design part 8.2 item 2)

**Files:**
- Create: `crates/clients/src/merge.rs` (`merge_pages`; 4 tests).
- Modify: `crates/clients/src/saved.rs:119-146` (`slim_search` over pages), `:206-216` (its test in the slice form, same assertion).
- Modify: `crates/clients/src/chain.rs:21-28` (`Lookup::TaggedSearch`), `:43-50` (`Answers.tagged`), `:68-78` (`pending`), `:156-179` (`search-tagged` key), `:204-272` (the tagged step after the cleaned search; the merged page and the file), tests after `:645`.
- Modify: `crates/clients/src/lib.rs:3-16` (`mod merge;`, `pub use merge::merge_pages;`).
- Modify: `crates/engine/src/file_phase.rs:356-365` (`lookup_word` returns `String`), `crates/engine/src/restart.rs:159-250` (the `search-tagged` line, its chain entry, its file), tests after `:487`.

**Interfaces:**
- Consumes: `lazymkv_core::lookup::name::fold_name`; `TmdbSearchResult`'s `media_type` and `id`; `TmdbSearch: Serialize`.
- Produces:

```rust
// crates/clients/src/merge.rs
/// J8's merge of two `/search/multi` first pages (design part 8.2 item 2, designer's call; decision-core J8): every
/// result of `cleaned` in its order, then each result of `tagged`, in its order, whose `(media_type, id)` (its
/// `identity()` for every result `parse_search` keeps) is not among the results kept so far. Pure. Def: T4-2.
/// Use: T4-2 (chain), T4-8 (`refetch_search`).
pub fn merge_pages(cleaned: &TmdbSearch, tagged: &TmdbSearch) -> TmdbSearch;

// crates/clients/src/saved.rs
/// The slim `tmdb-search.json` of one or more pages (design parts 7.1 and 8.4; S68, P29, J8): each page's movie and tv
/// results slimmed as before, in page order, where a result of a later page is left out when a kept result has its
/// `media_type` and `id` (so the results are `merge_pages`'s); then `queries`, each page's query in order; then
/// `fetched_at`. One page gives pull request 2's file byte for byte. Def: T4-2. Use: T4-2, T4-3, T4-8.
pub fn slim_search(pages: &[(&Value, &str)], today: &str) -> Value;

// crates/clients/src/chain.rs
/// One request of the chain of design part 8.2, in chain order. Def: T4-2. Use: T4-3 to T4-5, T4-9, T4-15.
pub enum Lookup { Dischash, Discid, Search, TaggedSearch, Record } // Copy, Ord as before
pub struct Answers { /* as before */
    /// The cleaned name's page, merged with `tagged` once both are in hand (J8). Def: T4-2. Use: T4-2, T4-3, T4-9.
    pub search: Option<TmdbSearch>,
    /// The tagged name's page (design part 8.2 item 2). Def: T4-2. Use: T4-2, T4-3.
    pub tagged: Option<TmdbSearch>, /* by_fingerprint, by_disc_id, record unchanged */ }

// crates/engine/src/file_phase.rs
/// A lookup's word in the waiting message and job.log, the first word of its status line except the record's
/// (Rulings R24.6, T4-R1): `dischash`, `discid`, `search`, `search-tagged`, `tmdb record`. Def: T4-2. Use: T4-3, T4-4.
pub(crate) fn lookup_word(lookup: Lookup) -> String;
```

The chain's rules (T4-R1, T4-R3):
- `TaggedSearch` is part of the chain when the name is searched (not generic) and `fold_name(&name.tagged) != fold_name(&name.cleaned)`; it is asked right after `Search`, with `name.tagged`, through `tmdb_search`.
- The cleaned page's answer sets `answers.search` to it merged with `answers.tagged` when that is in hand; the tagged page's answer sets `answers.tagged` and, when `answers.search` is in hand, `answers.search = merge_pages(search, tagged)`. A 404 clears that page only. Merging an already merged page again changes nothing, which the restart relies on.
- After either name search answers with a page, `tmdb-search.json` is saved as `slim_search` over the pages in hand, `(serde_json::to_value(answers.search), cleaned)` then `(serde_json::to_value(answers.tagged), tagged)`, each only when in hand. The status line's key is `search-tagged`; `pending().1` also holds while `TaggedSearch` is present and not answered.
- `rebuild_book` (Ruling S73 widened): `search-tagged` maps to `TaggedSearch`; its `200` puts `tmdb-search.json`'s page in `answers.tagged` (the `search` line's `200` still puts it in `answers.search`; a file holding the merged page then sits in both, which merges to itself); with no line it is `NotAsked` when `chain_inputs` gives a searched name whose folded forms differ.

- [ ] **Step 1: Write the failing tests**

`crates/clients/src/lib.rs`: `mod merge;` and `pub use merge::merge_pages;` beside the other re-exports, so `merge.rs`'s tests compile into the target.

`crates/clients/src/merge.rs`, `mod tests` (with `use super::*; use crate::saved::slim_search; use lazymkv_core::lookup::tmdb::{parse_search, TmdbSearchResult}; use serde_json::{json, Value};`):

```rust
    const TODAY: &str = "2026-10-23";

    fn result(media_type: &str, id: i64) -> TmdbSearchResult {
        TmdbSearchResult { media_type: media_type.to_string(), id, title: Some(format!("T{id}")), name: None,
            original_title: None, original_name: None, release_date: None, first_air_date: None }
    }

    fn page(results: &[(&str, i64)]) -> TmdbSearch {
        TmdbSearch { results: results.iter().map(|(kind, id)| result(kind, *id)).collect() }
    }

    fn ids(search: &TmdbSearch) -> Vec<(String, i64)> {
        search.results.iter().map(|r| (r.media_type.clone(), r.id)).collect()
    }

    #[test]
    fn merge_keeps_the_cleaned_results_first_then_the_new_tagged_ones() {
        let merged = merge_pages(&page(&[("movie", 1), ("movie", 2)]), &page(&[("movie", 2), ("movie", 3)]));
        assert_eq!(ids(&merged), ids(&page(&[("movie", 1), ("movie", 2), ("movie", 3)])));
    }

    #[test]
    fn merge_compares_by_kind_and_id() {
        let merged = merge_pages(&page(&[("movie", 1)]), &page(&[("tv", 1), ("movie", 1)]));
        assert_eq!(ids(&merged), ids(&page(&[("movie", 1), ("tv", 1)])));
    }

    #[test]
    fn merge_of_an_empty_tagged_page_is_the_cleaned_page() {
        let cleaned = page(&[("movie", 1), ("tv", 2)]);
        assert_eq!(merge_pages(&cleaned, &TmdbSearch::default()), cleaned);
    }

    #[test]
    fn the_slim_merged_page_parses_to_the_merged_pages() {
        let movie = |id: i64, title: &str| json!({"media_type": "movie", "id": id, "title": title, "original_title": title, "release_date": "2010-08-20"});
        let c = json!({"page": 1, "results": [movie(900101, "Piranha"), movie(900102, "Piranha")]});
        let t = json!({"page": 1, "results": [movie(900102, "Piranha"), {"media_type": "person", "id": 7, "name": "Someone"},
            {"media_type": "tv", "id": 900103, "name": "Piranha 3D", "original_name": "Piranha 3D", "first_air_date": "2010-01-01"}]});
        let parse = |value: &Value| parse_search(&value.to_string(), "page").unwrap();
        let slim = slim_search(&[(&c, "PIRANHA"), (&t, "PIRANHA 3D")], TODAY);
        let text = serde_json::to_string(&slim).unwrap();
        assert_eq!(parse_search(&text, "t").unwrap(), merge_pages(&parse(&c), &parse(&t)));
        assert_eq!(slim["queries"], json!(["PIRANHA", "PIRANHA 3D"]));
        assert_eq!(slim.as_object().unwrap().keys().last().map(String::as_str), Some("fetched_at"));
    }
```

`crates/clients/src/saved.rs:213`: `slim_search(&[(&raw, "SYNTHETIC MOVIE")], "2026-10-23")`, the expected text unchanged.

`crates/clients/src/chain.rs`, `mod tests` (add `use lazymkv_core::lookup::name::disc_name;` is already there):

```rust
    /// The name search pages of `PIRANHA_3D` (cleaned `PIRANHA`, tagged `PIRANHA 3D`; decision-core H1). IDs made up.
    const PIRANHA_CLEANED: [(i64, &str, &str); 2] = [(900101, "Piranha", "1978-08-03"), (900102, "Piranha", "2010-08-20")];
    const PIRANHA_TAGGED: [(i64, &str, &str); 2] = [(900102, "Piranha", "2010-08-20"), (900103, "Piranha 3D", "2010-08-20")];

    /// The tmdb-search.json files in `saved`, parsed, in order.
    fn search_files(saved: &[SavedFile]) -> Vec<Value> {
        saved.iter().filter(|f| f.name == "tmdb-search.json").map(|f| serde_json::from_slice(&f.bytes).unwrap()).collect()
    }

    #[test]
    fn a_name_whose_tagged_form_differs_is_searched_twice_and_merged() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("PIRANHA_3D"));
        misses(&t);
        t.answer(&search_url("PIRANHA"), 200, &search_body(&PIRANHA_CLEANED));
        t.answer(&search_url("PIRANHA 3D"), 200, &search_body(&PIRANHA_TAGGED));
        t.answer(&record_url(movie(900103)), 200, &movie_body(900103, "Piranha 3D", "2010-08-20"));
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        let searched = [search_url("PIRANHA"), search_url("PIRANHA 3D")];
        assert_eq!(urls(&t)[2..4], searched);
        assert_eq!(urls(&t).last(), Some(&record_url(movie(900103))));
        let merged: Vec<i64> = book.answers.search.as_ref().unwrap().results.iter().map(|r| r.id).collect();
        assert_eq!(merged, [900101, 900102, 900103]);
        assert_eq!(book.pending(), (false, false));
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200", "search-tagged 200", "tmdb movie/900103 200"]);
        let files = search_files(&saved);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["queries"], json!(["PIRANHA"]));
        assert_eq!(files[1]["queries"], json!(["PIRANHA", "PIRANHA 3D"]));
        assert_eq!(files[1]["results"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn a_name_whose_forms_fold_equal_is_searched_once() {
        let mut inputs = chain(Some(DISC_ID), None);
        inputs.name = disc_name(Some("Synthetic Movie"), Some("SYNTHETIC_MOVIE"));
        let cleaned = inputs.name.as_ref().unwrap().cleaned.clone();
        let t = RecordedTransport::new();
        misses(&t);
        t.answer(&search_url(&cleaned), 200, &search_body(&[(900001, "Synthetic Movie", "2001-02-03")]));
        t.answer(&record_url(movie(900001)), 200, &movie_body(900001, "Synthetic Movie", "2001-02-03"));
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url(&cleaned), record_url(movie(900001))]);
        assert!(!book.states.contains_key(&Lookup::TaggedSearch));
        assert!(!last_status(&saved).iter().any(|line| line.starts_with("search-tagged")));
    }

    #[test]
    fn tmdb_stays_pending_until_the_tagged_search_answers() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("PIRANHA_3D"));
        misses(&t);
        t.answer(&search_url("PIRANHA"), 200, &search_body(&PIRANHA_CLEANED));
        t.answer(&search_url("PIRANHA 3D"), 503, b"");
        t.answer(&search_url("PIRANHA 3D"), 200, &search_body(&PIRANHA_TAGGED));
        t.answer(&record_url(movie(900103)), 200, &movie_body(900103, "Piranha 3D", "2010-08-20"));
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(book.states[&Lookup::TaggedSearch], LookupState::Pending { reason: PendingReason::Status(503) });
        assert!(book.pending().1);
        assert!(last_status(&saved).contains(&"search-tagged 503".to_string()));
        let files = search_files(&saved);
        assert_eq!((files.len(), &files[0]["queries"]), (1, &json!(["PIRANHA"])));
        // Two plausible "Piranha" results: no record yet.
        assert!(!urls(&t).iter().any(|url| url.contains("/movie/")));
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(book.pending(), (false, false));
        assert_eq!(search_files(&saved)[0]["queries"], json!(["PIRANHA", "PIRANHA 3D"]));
        assert_eq!(urls(&t).last(), Some(&record_url(movie(900103))));
    }
```

`crates/engine/src/restart.rs`, `mod tests` (add `use crate::file_phase::waiting_message;`):

```rust
    #[test]
    fn the_rebuilt_book_reads_the_tagged_search_line() {
        let s = Scratch::new("book-tagged");
        let page = search_json(&[("movie", 900101, "Piranha", "Piranha", "1978-08-03"), ("movie", 900103, "Piranha 3D", "Piranha 3D", "2010-08-20")]);
        s.write("tmdb-search.json", &page);
        s.write("lookup-status.txt", "dischash 404\nsearch 200\nsearch-tagged 200\n");
        let book = rebuild_book(&s.0);
        assert!(matches!(book.states[&Lookup::TaggedSearch], LookupState::Answered { status: 200 }));
        assert_eq!(book.answers.search.as_ref().map(|p| p.results.len()), Some(2));
        assert_eq!(book.answers.tagged, book.answers.search);
        s.write("lookup-status.txt", "dischash 404\nsearch 200\nsearch-tagged 503\n");
        let book = rebuild_book(&s.0);
        assert!(book.answers.tagged.is_none());
        assert_eq!(waiting_message(&book), "search-tagged: HTTP 503");
    }
```

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked merge`
Expected: FAIL to compile: `unresolved import` of `merge::merge_pages` / `cannot find function merge_pages`, and `slim_search` takes 3 arguments.

- [ ] **Step 2: Implement**

`merge_pages` and `slim_search` as their docs say (one `Vec` of kept `(media_type, id)` pairs each); `lib.rs`'s two lines are Step 1's. `chain.rs` and `restart.rs` follow the chain's rules above; `status_word`, `settle` and `start` are unchanged; the module doc of `chain.rs` names J8's second search. `lookup_word` returns `String` (`TaggedSearch` gives `search-tagged`); its two callers (`file_phase.rs:384`, `steps.rs:460`) already format it.

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked merge`, then `scripts/box.sh cargo test -p lazymkv-clients --locked chain`, then `scripts/box.sh cargo test -p lazymkv-engine --locked rebuilt_book`.
Expected: PASS (`merge`: the 4 merge tests and the chain's `..._searched_twice_and_merged`; `chain`: every chain test, the 13 of pull request 2 unchanged; `rebuilt_book`: 1).

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS; `whole_job.rs`'s exact URL list is unchanged (its disc's name folds equal).

- [ ] **Step 3: Commit**

```bash
git add crates/clients/src/merge.rs crates/clients/src/saved.rs crates/clients/src/chain.rs crates/clients/src/lib.rs crates/engine/src/file_phase.rs crates/engine/src/restart.rs
git commit -m "clients: J8's tagged second search and the merged page"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 3.

---

### Task 3: The entry-title search (design part 8.2 item 3, S47)

**Files:**
- Modify: `crates/core/src/lookup/mod.rs:482` (`entry_search_title` after `record_to_fetch`), `crates/core/src/lookup/tests.rs:1368` (3 tests at the end).
- Modify: `crates/clients/src/chain.rs` (`Lookup::EntrySearch`, `Answers.entry`, `lookup_inputs`, `pending`, the entry step before the record, the file rule, the `search-entry` key; module doc), tests at its end.
- Modify: `crates/engine/src/file_phase.rs` (`lookup_word`: `search-entry`), `crates/engine/src/restart.rs` (Ruling T4-R2's routing), tests at its end.

**Interfaces:**
- Consumes: Task 2's book, `merge_pages`, `slim_search`; core's private `discdb_match` and `DiscDbRecord::{tmdb_identity, title_and_year}`.
- Produces:

```rust
// crates/core/src/lookup/mod.rs
/// The title of S47's entry-title search (design part 8.2 item 3, spec 8.1; Ruling T4-R5): the trimmed media title of
/// the TheDiscDB record `discdb_match` picks, when that record has a title and no TMDb ID; None when no record is in
/// hand, or the picked one has a TMDb ID or no title. The year is never part of the query (`/search/multi` takes only
/// `query`, S20); `search_by_entry` checks it. Pure. Def: T4-3. Use: T4-3 (chain, restart), T4-8 (`search_queries`).
pub fn entry_search_title(inputs: &LookupInputs<'_>) -> Option<String>;

// crates/clients/src/chain.rs
pub enum Lookup { Dischash, Discid, Search, TaggedSearch, EntrySearch, Record }
pub struct Answers { /* Task 2's fields */
    /// The entry-title search's page (S47; Ruling T4-R2). Def: T4-3. Use: T4-3, T4-9.
    pub entry: Option<TmdbSearch> }
impl LookupBook {
    /// The book as core's `LookupInputs` (Ruling T4-R2): `search` is `answers.entry` when `entry_search_title` over
    /// the book names a title (None while that search is pending), else `answers.search`. Def: T4-3. Use: T4-3, T4-5,
    /// T4-9.
    pub fn lookup_inputs<'a>(&'a self, name: Option<&'a DiscName>) -> LookupInputs<'a>;
}
```

The chain's rules (Rulings T4-R1, T4-R2, T4-R5):
- After the name searches and before the record, the round reads `entry_search_title(&book.lookup_inputs(name))`. Some(title): `EntrySearch` joins the chain; when not answered it is asked with `tmdb_search(t, token, &title)`; its page goes to `answers.entry` and is saved as `tmdb-search.json` = `slim_search(&[(&page, &title)], today)`. An answered entry search is not asked again for another title: the fingerprint's and the disc ID's records of one disc name one work, and the file keeps one page. None while `EntrySearch` is present: `EntrySearch` leaves the book, `answers.entry` is cleared, and `tmdb-search.json` is saved again from the name pages in hand by Task 2's rule (none in hand: no save).
- A name search's answer saves `tmdb-search.json` only while `entry_search_title` names nothing; otherwise it updates the book alone.
- The status key is `search-entry`; `pending().1` also holds while `EntrySearch` is present and not answered.
- `rebuild_book`: `search-entry` maps to `EntrySearch`. When the status text holds `search-entry 200`, the file's page goes to `answers.entry` and the `search` and `search-tagged` lines' `200` set their states without a page; otherwise the file goes where Task 2 put it. After the lines are read, `EntrySearch` is `NotAsked` when it has no line and `entry_search_title(&book.lookup_inputs(None))` names a title.

- [ ] **Step 1: Write the failing tests**

`crates/core/src/lookup/tests.rs`, at the end:

```rust
#[test]
fn the_entry_title_is_named_only_for_a_titled_record_without_a_tmdb_id() {
    let knowing = without_tmdb_id("Knowing", 2009);
    let wick = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let untitled: DiscDbRecord = serde_json::from_str(r#"{"titles": []}"#).unwrap();
    let with = |record| LookupInputs { discdb_by_fingerprint: Some(record), ..LookupInputs::default() };
    assert_eq!(entry_search_title(&with(&knowing)), Some("Knowing".to_string()));
    assert_eq!(entry_search_title(&with(&wick)), None);
    assert_eq!(entry_search_title(&with(&untitled)), None);
    assert_eq!(entry_search_title(&LookupInputs::default()), None);
}

#[test]
fn the_entry_title_follows_the_record_discdb_match_picks() {
    let untitled: DiscDbRecord = serde_json::from_str(r#"{"titles": []}"#).unwrap();
    let knowing = without_tmdb_id("Knowing", 2009);
    let example = without_tmdb_id("Example Film", 1986);
    let only_id = LookupInputs { discdb_by_fingerprint: Some(&untitled), discdb_by_disc_id: Some(&knowing), ..LookupInputs::default() };
    assert_eq!(entry_search_title(&only_id), Some("Knowing".to_string()));
    let both = LookupInputs { discdb_by_fingerprint: Some(&example), discdb_by_disc_id: Some(&knowing), ..LookupInputs::default() };
    assert_eq!(entry_search_title(&both), Some("Example Film".to_string()));
}

#[test]
fn the_entry_title_is_trimmed_and_a_blank_one_is_none() {
    let padded = without_tmdb_id("  Knowing  ", 2009);
    let blank = without_tmdb_id("   ", 2009);
    assert_eq!(entry_search_title(&LookupInputs { discdb_by_fingerprint: Some(&padded), ..LookupInputs::default() }), Some("Knowing".to_string()));
    assert_eq!(entry_search_title(&LookupInputs { discdb_by_fingerprint: Some(&blank), ..LookupInputs::default() }), None);
}
```

`crates/clients/src/chain.rs`, `mod tests`:

```rust
    /// A dischash hit whose media has a title and a year but no TMDb ID (S47's case), with one title.
    fn entry_body(title: &str, year: i32) -> Vec<u8> {
        json!({"media": {"title": title, "full_title": title, "year": year, "type": "Movie", "external_ids": {}},
               "titles": [{"index": 0, "source_file": "00001.mpls", "duration": "2:01:14"}]}).to_string().into_bytes()
    }
    /// Knowing (TMDb 13811, TheDiscDB's) and a made-up namesake.
    const KNOWING_PAGE: [(i64, &str, &str); 2] = [(13811, "Knowing", "2009-03-19"), (900201, "Knowing", "1990-01-01")];

    /// TheDiscDB knows the fingerprint by title alone; the name and entry searches and Knowing's record answer.
    fn knowing(t: &RecordedTransport, name_query: &str) {
        t.answer(&dischash_url(FP), 200, &entry_body("Knowing", 2009));
        t.answer(&discid_url(DISC_ID), 404, b"");
        t.answer(&search_url(name_query), 200, &search_body(&KNOWING_PAGE));
        t.answer(&search_url("Knowing"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&record_url(movie(13811)), 200, &movie_body(13811, "Knowing", "2009-03-19"));
    }

    #[test]
    fn an_entry_without_a_tmdb_id_is_searched_by_its_title_alone() {
        let t = RecordedTransport::new();
        knowing(&t, "KNOWING");
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &chain(Some(DISC_ID), Some("KNOWING")));
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("KNOWING"), search_url("Knowing"), record_url(movie(13811))]);
        assert_eq!(urls(&t)[3], "https://api.themoviedb.org/3/search/multi?query=Knowing");
        assert_eq!(last_status(&saved), ["dischash 200", "discid 404", "search 200", "search-entry 200", "tmdb movie/13811 200"]);
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn the_plan_reads_the_entry_page_not_the_name_page() {
        let record: DiscDbRecord = serde_json::from_slice(&entry_body("Knowing", 2009)).unwrap();
        let name_page = parse_search(&String::from_utf8(search_body(&PIRANHA_CLEANED)).unwrap(), "name").unwrap();
        let entry_page = parse_search(&String::from_utf8(search_body(&KNOWING_PAGE)).unwrap(), "entry").unwrap();
        let mut book = LookupBook {
            states: BTreeMap::from([(Lookup::Dischash, answered(200)), (Lookup::Search, answered(200)), (Lookup::EntrySearch, answered(200))]),
            answers: Answers { by_fingerprint: Some(record), search: Some(name_page), entry: Some(entry_page.clone()), ..Answers::default() },
            ..LookupBook::default() // Task 4 adds record_file
        };
        assert_eq!(book.lookup_inputs(None).search, Some(&entry_page));
        book.states.insert(Lookup::EntrySearch, LookupState::Pending { reason: PendingReason::Status(503) });
        book.answers.entry = None;
        assert_eq!(book.lookup_inputs(None).search, None);
        assert!(book.pending().1);
    }

    #[test]
    fn the_entry_page_replaces_the_name_page_in_tmdb_search_json() {
        let t = RecordedTransport::new();
        t.fail(&dischash_url(FP), TransportError::Timeout);
        knowing(&t, "KNOWING");
        let inputs = chain(Some(DISC_ID), Some("KNOWING"));
        let mut book = LookupBook::default();
        let first = round(&t, &mut book, &inputs);
        assert_eq!(search_files(&first).last().unwrap()["queries"], json!(["KNOWING"]));
        let second = round(&t, &mut book, &inputs);
        assert_eq!(search_files(&second).last().unwrap()["queries"], json!(["Knowing"]));
    }

    #[test]
    fn an_entry_with_neither_id_nor_title_falls_back_to_the_name_search() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.answer(&dischash_url(FP), 200, br#"{"media": {"type": "Movie", "external_ids": {}}, "titles": [{"index": 0}]}"#);
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert!(!book.states.contains_key(&Lookup::EntrySearch));
        assert_eq!(book.lookup_inputs(inputs.name.as_ref()).search, book.answers.search.as_ref());
    }

    #[test]
    fn a_late_name_answer_never_overwrites_the_entry_page() {
        let t = RecordedTransport::new();
        t.answer(&search_url("KNOWING 3D"), 503, b"");
        t.answer(&search_url("KNOWING 3D"), 200, &search_body(&[(900202, "Knowing 3D", "2011-01-01")]));
        knowing(&t, "KNOWING");
        let inputs = chain(Some(DISC_ID), Some("KNOWING_3D"));
        let mut book = LookupBook::default();
        let first = round(&t, &mut book, &inputs);
        assert_eq!(search_files(&first).iter().map(|f| f["queries"].clone()).collect::<Vec<Value>>(), [json!(["Knowing"])]);
        assert!(book.pending().1);
        let second = round(&t, &mut book, &inputs);
        assert!(search_files(&second).is_empty());
        let merged: Vec<i64> = book.answers.search.as_ref().unwrap().results.iter().map(|r| r.id).collect();
        assert_eq!(merged, [13811, 900201, 900202]);
        assert!(last_status(&second).contains(&"search-tagged 200".to_string()));
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn an_entry_search_no_longer_named_leaves_the_book_and_the_name_page_returns() {
        let t = RecordedTransport::new();
        t.fail(&dischash_url(FP), TransportError::Timeout);
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        let disc_id_body = format!("[{}]", String::from_utf8(entry_body("Knowing", 2009)).unwrap());
        t.answer(&discid_url(DISC_ID), 200, disc_id_body.as_bytes());
        t.answer(&search_url("KNOWING"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&search_url("Knowing"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&record_url(movie(13811)), 200, &movie_body(13811, "Knowing", "2009-03-19"));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let inputs = chain(Some(DISC_ID), Some("KNOWING"));
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert!(book.answers.entry.is_some());
        let saved = round(&t, &mut book, &inputs);
        assert!(!book.states.contains_key(&Lookup::EntrySearch));
        assert!(book.answers.entry.is_none());
        assert_eq!(search_files(&saved).last().unwrap()["queries"], json!(["KNOWING"]));
        assert_eq!(urls(&t).last(), Some(&record_url(JOHN_WICK)));
    }
```

(`parse_search` joins the test imports from `lazymkv_core::lookup::tmdb`.)

`crates/engine/src/restart.rs`, `mod tests`:

```rust
    #[test]
    fn a_restart_routes_the_saved_page_by_the_search_entry_line() {
        let s = Scratch::new("book-entry");
        s.write("thediscdb-hash.json", r#"{"media": {"title": "Knowing", "year": 2009, "type": "Movie", "external_ids": {}}, "titles": [{"index": 0}]}"#);
        s.write("tmdb-search.json", &search_json(&[("movie", 13811, "Knowing", "Knowing", "2009-03-19")]));
        s.write("lookup-status.txt", "dischash 200\nsearch 200\nsearch-entry 200\n");
        let book = rebuild_book(&s.0);
        assert!(matches!(book.states[&Lookup::EntrySearch], LookupState::Answered { status: 200 }));
        assert!(matches!(book.states[&Lookup::Search], LookupState::Answered { status: 200 }));
        assert_eq!((book.answers.entry.as_ref().map(|p| p.results.len()), book.answers.search.is_none()), (Some(1), true));
        s.write("lookup-status.txt", "dischash 200\nsearch 200\n");
        let book = rebuild_book(&s.0);
        assert!(book.answers.entry.is_none() && book.answers.search.is_some());
        assert!(matches!(book.states[&Lookup::EntrySearch], LookupState::NotAsked));
        assert!(book.pending().1);
    }
```

Run: `scripts/box.sh cargo test -p lazymkv-core --locked entry_title`
Expected: FAIL to compile: `cannot find function entry_search_title`.

- [ ] **Step 2: Implement**

`entry_search_title`: `discdb_match(inputs, &mut Vec::new())`, then the record's title from `title_and_year()` when `tmdb_identity()` is None. The chain and `rebuild_book` follow the rules above; `lookup_word(EntrySearch)` gives `search-entry`; `chain.rs`'s module doc drops "with no entry-title search (S47)" and names S47's step.

Run: `scripts/box.sh cargo test -p lazymkv-core --locked entry_title`, then `scripts/box.sh cargo test -p lazymkv-clients --locked entry`, then `scripts/box.sh cargo test -p lazymkv-engine --locked search_entry_line`.
Expected: PASS (`entry_title`: the 3 new tests and pull request 1's `thediscdb_by_both_without_a_tmdb_id_searches_the_entry_title_at_0_99`, unchanged; `entry`: the 6 chain tests whose names hold `entry`; `search_entry_line`: 1).

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS, Task 2's chain and restart tests unchanged.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/lookup/mod.rs crates/core/src/lookup/tests.rs crates/clients/src/chain.rs crates/engine/src/file_phase.rs crates/engine/src/restart.rs
git commit -m "clients: the entry-title search of S47"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 4.

---

### Task 4: TV records and their seasons (design part 8.2 item 5, A5)

A5 limits pull request 4 to the lookup side of TV: a TheDiscDB `Series` record's `/tv/{id}` fetch, its season answers and their merge. TV filing, `tv_root` and season sessions stay in phase 4, and a TV disc is held with no path (design part 6.6, Task 18).

**Files:**
- Modify: `crates/core/src/lookup/tmdb.rs:323-357` (`parse_season_answer`, `merge_parsed_season`; `merge_season` becomes their composition; 2 tests in its `mod tests`).
- Modify: `crates/clients/src/lookups.rs:43-52` (`season_url` beside `record_url`), `:173-181` (`tmdb_season` beside `tmdb_record`); 1 test.
- Modify: `crates/clients/src/saved.rs:79-121` (`splice_season` after `slim_record`; 1 test).
- Modify: `crates/clients/src/chain.rs:21-28` (`Lookup::Season(u32)` last), `:52-58` (`LookupBook.record_file`), `:68-78` (`pending`), `:156-179` (`season-<n>` key), `:254-272` (the season step after the record step; `seasons_to_fetch`); the existing `LookupBook { states, answers }` literals of its tests at `:537`, `:608` and `:639` gain `record_file: None`; 4 tests, six test helpers and `const SHOW`.
- Modify: `crates/clients/src/lib.rs:9-16` (re-export `season_url`, `tmdb_season`, `splice_season`, `seasons_to_fetch`).
- Modify: `crates/engine/src/restart.rs:169-250` (`tmdb tv/<id>` read with `parse_tv`, `record_file` loaded, `season-<n>` lines, unasked seasons of an answered TV record added `NotAsked`); 2 tests.
- Modify: `crates/engine/src/file_phase.rs:356-365` (`lookup_word(Lookup::Season(n))` is `season-<n>`); 1 test.

**Interfaces:**
- Consumes: Task 3's book (`Lookup`, `Answers`, the record step), `TmdbRecord::episode_runtimes`, `DiscName.season`, `lookup_word -> String` (Task 2).
- Produces:

```rust
// core::lookup::tmdb
/// A `/tv/{id}/season/{n}` answer (design part 8.2 item 5): `season_number`, the `episodes` array read as
/// `TmdbEpisode`s (runtime 0 reads as None), `episode_count` the number of episodes. Err `<shown>: the season answer
/// has no episodes` when `episodes` is absent or not an array (Ruling T4-4-a: a 200 that is no season is unparsed,
/// never a season with no episodes); serde's sentence prefixed by `<shown>` for malformed JSON or no
/// `season_number`. Def: T4-4. Use: T4-4, T4-8, T4-19.
pub fn parse_season_answer(text: &str, shown: &str) -> Result<TmdbSeason, String>;
/// `merge_season`'s rule over a parsed season (spec 8.1): that season's episodes replaced, a season the record lacks
/// added in number order, an absent `episode_count` taking the season's. Err `a season answer belongs to a TV record,
/// not to movie <id>` for a movie record, the record unchanged. Def: T4-4. Use: T4-4, T4-8.
pub fn merge_parsed_season(record: &mut TmdbRecord, season: TmdbSeason) -> Result<(), String>;
// merge_season(record, text, shown): the movie check first (Err `<shown>: ` + merge_parsed_season's sentence), then
// parse_season_answer, then merge_parsed_season. Its doc's "Use:" becomes "T4-4 (through its parts)".

// clients::lookups
/// TMDb's season call (design parts 8.2 item 5, 8.5): `{TMDB}/tv/{show}/season/{season}`, no query string.
/// Def: T4-4. Use: T4-4, T4-8, T4-19.
pub fn season_url(show: i64, season: u32) -> String;
/// One season of `show`, with the bearer token and the classes of design part 8.3; a 200 is read by
/// `parse_season_answer`, and an answer whose `season_number` is not `season` is `Pending { Unparsed("<url>: the
/// answer is season <m>, not season <n>") }`. Def: T4-4. Use: T4-4, T4-8.
pub fn tmdb_season(t: &dyn HttpTransport, token: Option<&str>, show: i64, season: u32) -> Outcome<TmdbSeason>;

// clients::saved
/// A raw season answer spliced into a slim `tmdb-record.json` (design part 7.1: "for a TV record, its seasons and
/// episodes"; Ruling T4-R4; no season file is saved). The season whose `season_number` matches is rebuilt in place as
/// `season_number`, `episode_count` (kept when present, else the number of episodes), `episodes` (each
/// `episode_number` and `runtime` when present and not null, as `slim_record` keeps them); a season the file lacks is
/// inserted before the first season with a higher number. A file without a `seasons` array, or an answer without a
/// number, is left as it is. `fetched_at` stays last. Def: T4-4. Use: T4-4, T4-8.
pub fn splice_season(record_file: &mut Value, season_raw: &Value);

// clients::chain
pub enum Lookup { Dischash, Discid, Search, TaggedSearch, EntrySearch, Record, Season(u32) } // Season last, by number
pub struct LookupBook { pub states, pub answers,
    /// The `tmdb-record.json` last saved, as JSON, for `splice_season` (Ruling T4-R4). Def: T4-4. Use: T4-4, T4-9.
    pub record_file: Option<Value> }
/// The seasons the chain asks for a TV record (design part 8.2 item 5, Ruling T4-R4): `[n]` for a name with a parsed
/// season `n`, listed or not; else every season the record lists, season 0 included, in the record's order (spec
/// 8.1's runtime check still reads `episode_runtimes(None)`, which leaves season 0 out). Empty for a movie record.
/// Def: T4-4. Use: T4-4, T4-8.
pub fn seasons_to_fetch(record: &TmdbRecord, name: Option<&DiscName>) -> Vec<u32>;
```

The engine's restart uses `seasons_to_fetch` too: a crash between the record's answer and its first season must leave those seasons pending.

- [ ] **Step 1: Write the failing core tests** (in `tmdb.rs`'s `mod tests`, over its `TV`, `SEASON_3`, `the_office`, `pans_labyrinth` fixtures):

```rust
#[test]
fn merge_season_is_parse_then_merge() {
    let mut merged = parse_tv(TV, "tv.json").unwrap();
    merge_season(&mut merged, SEASON_3, "season-3.json").unwrap();
    let season = parse_season_answer(SEASON_3, "season-3.json").unwrap();
    assert_eq!((season.season_number, season.episode_count, season.episodes.len()), (3, Some(4), 4));
    let mut composed = parse_tv(TV, "tv.json").unwrap();
    merge_parsed_season(&mut composed, season.clone()).unwrap();
    assert_eq!(composed, merged);
    let mut movie = pans_labyrinth();
    assert_eq!(merge_parsed_season(&mut movie, season),
        Err("a season answer belongs to a TV record, not to movie 1417".to_string()));
    assert_eq!(movie, pans_labyrinth());
}

#[test]
fn a_season_answer_without_episodes_is_refused_with_a_sentence() {
    assert_eq!(parse_season_answer(r#"{"season_number": 2}"#, "season-2.json"),
        Err("season-2.json: the season answer has no episodes".to_string()));
    assert_eq!(parse_season_answer(r#"{"season_number": 2, "episodes": null}"#, "season-2.json"),
        Err("season-2.json: the season answer has no episodes".to_string()));
    let no_number = parse_season_answer(r#"{"episodes": []}"#, "season.json").unwrap_err();
    assert!(no_number.starts_with("season.json: missing field `season_number`"), "{no_number}");
    let empty = parse_season_answer(r#"{"season_number": 2, "episodes": []}"#, "season-2.json").unwrap();
    assert_eq!((empty.episode_count, empty.episodes.len()), (Some(0), 0));
}
```

- [ ] **Step 2: Write the failing clients tests.** In `lookups.rs`'s `mod tests`:

```rust
#[test]
fn the_season_url_names_the_show_and_season() {
    assert_eq!(season_url(1437, 1), "https://api.themoviedb.org/3/tv/1437/season/1");
    let t = RecordedTransport::new();
    t.answer(&season_url(900002, 2), 200, br#"{"season_number": 3, "episodes": []}"#);
    assert_eq!(tmdb_season(&t, Some("token"), 900002, 2), Outcome::Pending {
        reason: PendingReason::Unparsed(format!("{}: the answer is season 3, not season 2", season_url(900002, 2))),
        fetched: Some(Fetched { url: season_url(900002, 2), status: 200, body: br#"{"season_number": 3, "episodes": []}"#.to_vec() }) });
}
```

In `saved.rs`'s `mod tests`:

```rust
#[test]
fn splice_season_adds_episodes_and_keeps_the_slim_order() {
    let tv = json!({"id": 5, "name": "Show", "original_name": "Show", "first_air_date": "2002-01-01",
        "original_language": "en", "seasons": [{"season_number": 0, "episode_count": 1}, {"season_number": 1},
        {"season_number": 3, "episode_count": 2}]});
    let mut file = slim_record(&tv, TmdbKind::Tv, "2026-10-23");
    splice_season(&mut file, &json!({"season_number": 1, "name": "S1", "episodes": [
        {"episode_number": 1, "name": "Pilot", "runtime": 44, "still_path": "/x.jpg"}, {"episode_number": 2, "runtime": null}]}));
    splice_season(&mut file, &json!({"season_number": 2, "episodes": [{"episode_number": 1, "runtime": 0}]}));
    let text = serde_json::to_string(&file).unwrap();
    assert_eq!(text, r#"{"id":5,"name":"Show","original_name":"Show","first_air_date":"2002-01-01","original_language":"en","kind":"tv","seasons":[{"season_number":0,"episode_count":1},{"season_number":1,"episode_count":2,"episodes":[{"episode_number":1,"runtime":44},{"episode_number":2}]},{"season_number":2,"episode_count":1,"episodes":[{"episode_number":1,"runtime":0}]},{"season_number":3,"episode_count":2}],"fetched_at":"2026-10-23"}"#);
    assert_eq!(parse_tv(&text, "tmdb-record.json").unwrap().episode_runtimes(None), vec![44]);
}
```

In `chain.rs`'s `mod tests` (imports gain `crate::season_url` and `lazymkv_core::lookup::tmdb::parse_tv`), helpers and four tests (`SHOW` is synthetic, never a TMDb ID of the clone; `tv` and `series_dischash` serve Task 5 too):

```rust
const SHOW: i64 = 900002;
fn tv(id: i64) -> TmdbIdentity { TmdbIdentity { kind: TmdbKind::Tv, id } }
/// John Wick's public dischash answer made a `Series` entry naming the TMDb TV ID `show`.
fn series_dischash(show: i64) -> Vec<u8> {
    let mut record: Value = serde_json::from_slice(&corpus(JOHN_WICK_DISCHASH)).unwrap();
    record["media"]["type"] = json!("Series");
    record["media"]["external_ids"]["tmdb"] = json!(show.to_string());
    record.to_string().into_bytes()
}
/// A /tv/{id}?append_to_response=external_ids answer listing (season number, episode count) pairs.
fn tv_body(id: i64, seasons: &[(u32, u32)]) -> Vec<u8> {
    let seasons: Vec<Value> = seasons.iter().map(|(n, count)| json!({"season_number": n, "episode_count": count})).collect();
    json!({"id": id, "name": "Example Show", "original_name": "Example Show", "first_air_date": "2005-03-24",
           "original_language": "en", "seasons": seasons, "external_ids": {"tvdb_id": 70001}}).to_string().into_bytes()
}
/// A /tv/{id}/season/{n} answer whose episodes have `runtimes`, numbered from 1.
fn season_body(n: u32, runtimes: &[u32]) -> Vec<u8> {
    let episodes: Vec<Value> = runtimes.iter().enumerate()
        .map(|(i, runtime)| json!({"episode_number": i + 1, "name": format!("Episode {}", i + 1), "runtime": runtime})).collect();
    json!({"season_number": n, "episodes": episodes}).to_string().into_bytes()
}
/// The disc name's searches answered with an empty page, so only the record and its seasons stay to ask.
fn empty_searches(t: &RecordedTransport, inputs: &ChainInputs) {
    let name = inputs.name.as_ref().unwrap();
    t.answer(&search_url(&name.cleaned), 200, &search_body(&[]));
    t.answer(&search_url(&name.tagged), 200, &search_body(&[]));
}
fn season_urls(t: &RecordedTransport) -> Vec<String> {
    urls(t).into_iter().filter(|url| url.contains("/season/")).collect()
}

#[test]
fn a_tv_record_with_a_parsed_season_fetches_that_season_only() {
    let t = RecordedTransport::new();
    let inputs = chain(None, Some("SHOW_S03"));
    t.answer(&dischash_url(FP), 200, &series_dischash(SHOW));
    empty_searches(&t, &inputs);
    t.answer(&record_url(tv(SHOW)), 200, &tv_body(SHOW, &[(0, 1), (1, 2), (3, 2)]));
    t.answer(&season_url(SHOW, 3), 200, &season_body(3, &[44, 43]));
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(season_urls(&t), [season_url(SHOW, 3)]);
    assert_eq!(book.answers.record.as_ref().unwrap().episode_runtimes(Some(3)), vec![44, 43]);
    assert_eq!(book.states.get(&Lookup::Season(3)), Some(&answered(200)));
    assert!(last_status(&saved).contains(&"season-3 200".to_string()));
    let file = saved.iter().rev().find(|file| file.name == "tmdb-record.json").unwrap();
    let text = String::from_utf8(file.bytes.clone()).unwrap();
    assert_eq!(parse_tv(&text, "tmdb-record.json").unwrap().episode_runtimes(Some(3)), vec![44, 43]);
    assert_eq!(book.pending(), (false, false));
}

#[test]
fn a_tv_record_without_a_parsed_season_fetches_every_listed_season() {
    let t = RecordedTransport::new();
    let inputs = chain(None, None);
    t.answer(&dischash_url(FP), 200, &series_dischash(SHOW));
    t.answer(&record_url(tv(SHOW)), 200, &tv_body(SHOW, &[(0, 1), (1, 2), (2, 2)]));
    t.answer(&season_url(SHOW, 0), 200, &season_body(0, &[30]));
    t.answer(&season_url(SHOW, 1), 200, &season_body(1, &[44, 43]));
    t.answer(&season_url(SHOW, 2), 200, &season_body(2, &[42, 41]));
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(season_urls(&t), [season_url(SHOW, 0), season_url(SHOW, 1), season_url(SHOW, 2)], "T4-R4");
    // Season 0 is merged but left out of spec 8.1's runtime check.
    assert_eq!(book.answers.record.as_ref().unwrap().episode_runtimes(None), vec![44, 43, 42, 41]);
    assert_eq!(book.answers.record.as_ref().unwrap().episode_runtimes(Some(0)), vec![30]);
    // Saved once with the record and again after each season.
    assert_eq!(saved.iter().filter(|file| file.name == "tmdb-record.json").count(), 4);
    assert_eq!(last_status(&saved), ["dischash 200", "season-0 200", "season-1 200", "season-2 200", "tmdb tv/900002 200"]);
}

#[test]
fn a_parsed_season_the_record_does_not_list_is_asked_and_a_404_is_a_miss() {
    let t = RecordedTransport::new();
    let inputs = chain(None, Some("SHOW_S07"));
    t.answer(&dischash_url(FP), 200, &series_dischash(SHOW));
    empty_searches(&t, &inputs);
    t.answer(&record_url(tv(SHOW)), 200, &tv_body(SHOW, &[(1, 2)]));
    t.answer(&season_url(SHOW, 7), 404, b"");
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(season_urls(&t), [season_url(SHOW, 7)]);
    assert_eq!(book.states.get(&Lookup::Season(7)), Some(&answered(404)));
    assert!(last_status(&saved).contains(&"season-7 404".to_string()));
    assert_eq!(book.answers.record.as_ref().unwrap().season(7), None);
    assert!(!book.pending().1);
    let asked = t.requests().len();
    round(&t, &mut book, &inputs);
    assert_eq!(t.requests().len(), asked, "an answered season is not asked again");
}

#[test]
fn a_pending_season_keeps_tmdb_pending() {
    let t = RecordedTransport::new();
    let inputs = chain(None, None);
    t.answer(&dischash_url(FP), 200, &series_dischash(SHOW));
    t.answer(&record_url(tv(SHOW)), 200, &tv_body(SHOW, &[(1, 2)]));
    t.fail(&season_url(SHOW, 1), TransportError::Timeout);
    t.answer(&season_url(SHOW, 1), 200, &season_body(1, &[44, 43]));
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(book.states.get(&Lookup::Season(1)), Some(&LookupState::Pending { reason: PendingReason::Timeout }));
    assert!(last_status(&saved).contains(&"season-1 000".to_string()));
    assert_eq!(book.pending(), (false, true));
    round(&t, &mut book, &inputs);
    assert_eq!(book.pending(), (false, false));
    assert_eq!(urls(&t).iter().filter(|url| **url == record_url(tv(SHOW))).count(), 1, "the record is not asked again");
}
```

- [ ] **Step 3: Write the failing engine tests.** In `restart.rs`'s `mod tests` (`Scratch`, imports `lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind}`; no `meta.json`, so the name is None and every listed season, season 0 included, is wanted):

```rust
const SHOW_RECORD: &str = r#"{"id":900002,"name":"Example Show","original_name":"Example Show","first_air_date":"2005-03-24","original_language":"en","kind":"tv","seasons":[{"season_number":0,"episode_count":1},{"season_number":1,"episode_count":2,"episodes":[{"episode_number":1,"runtime":44},{"episode_number":2,"runtime":43}]},{"season_number":2,"episode_count":2}],"fetched_at":"2026-10-23"}"#;

#[test]
fn a_restart_reads_a_tv_record_with_its_merged_seasons() {
    let job = Scratch::new("tv-record");
    job.write("lookup-status.txt", "dischash 404\nseason-1 200\nseason-2 000\ntmdb tv/900002 200\n");
    job.write("tmdb-record.json", SHOW_RECORD);
    let book = rebuild_book(&job.0);
    let record = book.answers.record.as_ref().unwrap();
    assert_eq!(record.identity, TmdbIdentity { kind: TmdbKind::Tv, id: 900002 });
    assert_eq!(record.episode_runtimes(None), vec![44, 43]);
    assert_eq!(book.states.get(&Lookup::Season(1)), Some(&LookupState::Answered { status: 200 }));
    assert!(matches!(book.states.get(&Lookup::Season(2)), Some(LookupState::Pending { .. })));
    assert_eq!(book.record_file.as_ref().unwrap()["seasons"][1]["episodes"][1]["runtime"], serde_json::json!(43));
    assert_eq!(book.pending(), (false, true));
}

#[test]
fn a_restart_after_a_tv_record_marks_its_unasked_seasons_not_asked() {
    let job = Scratch::new("tv-record-unasked");
    job.write("lookup-status.txt", "dischash 404\ntmdb tv/900002 200\n");
    job.write("tmdb-record.json", SHOW_RECORD);
    let book = rebuild_book(&job.0);
    assert_eq!(book.states.get(&Lookup::Season(0)), Some(&LookupState::NotAsked));
    assert_eq!(book.states.get(&Lookup::Season(1)), Some(&LookupState::NotAsked));
    assert_eq!(book.states.get(&Lookup::Season(2)), Some(&LookupState::NotAsked));
    assert!(book.pending().1);
}
```

In `file_phase.rs`'s `mod tests`:

```rust
#[test]
fn the_waiting_message_names_a_tv_record_season() {
    let mut book = LookupBook::default();
    book.states.insert(Lookup::Season(2), LookupState::Pending { reason: PendingReason::Timeout });
    assert_eq!(lookup_word(Lookup::Season(2)), "season-2");
    assert_eq!(waiting_message(&book), "season-2: no answer within 10 s");
}
```

- [ ] **Step 4: Run them and see them fail.** `scripts/box.sh cargo test -p lazymkv-core --locked season`: compile error, `parse_season_answer` and `merge_parsed_season` not found. The clients and engine runs fail to compile on `season_url`, `Lookup::Season` and `record_file`.

- [ ] **Step 5: Implement core.** `parse_season_answer` reads the text with `parse_json` into a private struct `{ season_number: u32, #[serde(default)] episodes: Option<Vec<TmdbEpisode>> }` (`None` and JSON `null` give the "no episodes" sentence), maps episodes through `known_episode`, and sets `episode_count: Some(count)`. `merge_parsed_season` is the body of today's `merge_season` after its parse; `merge_season` keeps its movie check first, so its three existing tests (`tmdb.rs:694-746`) stay unchanged.

- [ ] **Step 6: Implement clients.** `season_url` and `tmdb_season` as their docs say (`tmdb_season` wraps `parse_season_answer` in `tmdb(..)` and checks the number inside the parse closure). `splice_season` as its doc says. In `chain.rs`:
  1. `pending()`'s TMDb flag also holds when any `Season(_)` state is present and not answered.
  2. `status_file` keys `Season(n)` as `season-<n>`; `file_phase::lookup_word` gives the same word.
  3. The record step: before a fetch whose identity differs from the record in hand, every `Season(_)` state and `record_file` are dropped; an `Answered` record sets `record_file = Some(slim_record(raw, want.kind, today))` (the value already saved as `tmdb-record.json`); a `Miss` sets it to None. The step's two early `return`s become a fall-through, so the season step runs on every round.
  4. The season step runs only while `Record` is `Answered` and `answers.record` is a TV record; otherwise every `Season(_)` state is removed. It inserts `NotAsked` for each of `seasons_to_fetch(record, name)` not present, removes `Season` states not in that list, then for each asked season in number order: `start`, `settle` over `tmdb_season(t, token, record.identity.id, n)`, on `Answered` `merge_parsed_season` into `answers.record` and `splice_season` into `record_file` with the answered JSON (`answered_json`), then `save(json_file("tmdb-record.json", record_file))` when `record_file` is Some; then the status file.
  5. The three `LookupBook { states, answers }` literals of `chain.rs`'s tests (`:537`, `:608`, `:639` at `6a862ac`) gain `record_file: None`.

- [ ] **Step 7: Implement the engine.** `rebuild_book`: a `season-<n>` first word (`strip_prefix("season-")` then `parse::<u32>()`) is `Lookup::Season(n)`; its `200` reads no file (its episodes are in `tmdb-record.json`), and its `unparsed` sentence is `<job>/tmdb-record.json: the answer did not parse before the restart`, as `answer_path` maps `Season(_)` to `tmdb-record.json`. A `tmdb` line whose second word starts `tv/` reads `tmdb-record.json` with `parse_tv`, else with `parse_movie`, and sets `book.record_file` to the file's JSON. After the lines, an answered TV record adds `NotAsked` for each of `seasons_to_fetch(record, chain_inputs(job_dir).ok().and_then(|inputs| inputs.name).as_ref())` with no line.

- [ ] **Step 8: Run and see them pass.** `scripts/box.sh cargo test -p lazymkv-core --locked season`, `scripts/box.sh cargo test -p lazymkv-clients --locked season`, then `scripts/box.sh cargo test -p lazymkv-engine --locked tv_record`: PASS. Then `scripts/box.sh cargo test --workspace --locked`: PASS.

- [ ] **Step 9: Commit.**

```bash
git add crates/core/src/lookup/tmdb.rs crates/clients/src/lookups.rs crates/clients/src/saved.rs crates/clients/src/chain.rs crates/clients/src/lib.rs crates/engine/src/restart.rs crates/engine/src/file_phase.rs
git commit -m "clients: the TV record's season answers merged (A5)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 5.

---

### Task 5: The outcome classes, the remembered identity in the chain, and the remembered file's fingerprint (design part 8.3; engine.md "Pull request 4")

Closes two items of engine.md's "Pull request 4" list (`ChainInputs` carrying the remembered identity; the remembered file's inner fingerprint compared), pins design part 8.3's classes for every lookup, and pins Ruling T4-R21 (the unwritable remembered copy, accepted with no code).

Precondition: `decisions.txt` holds T4-R21's answer (Task 0 Step 1); on a no, this task waits for the behaviour the owner names.

**Files:**
- Modify: `crates/clients/src/chain.rs:60-66` (`ChainInputs.remembered`), `:156-185` (`status_file` and `start` take `&ChainInputs`), `:254-272` (the record step reads `wanted`), `:331-337` (the test helper `chain` sets `remembered: None`); 5 tests.
- Modify: `crates/engine/src/store.rs:236-242` (`remembered_file`; `Store::remembered` over it); 1 test.
- Modify: `crates/engine/src/arrival.rs:129-154` (`chain_inputs` sets `remembered: None`; `round_inputs` after it); 2 tests.
- Modify: `crates/engine/src/steps.rs:436-447` (`spawn_round` calls `round_inputs(&job_dir, &store)` in place of `chain_inputs`; `start_lookups` and every other caller reach it through `spawn_round`).
- Modify: `crates/engine/src/file_phase.rs` (1 test).
- Modify: `crates/fake-tools/tests/engine_disc_phase.rs` (2 tests), `crates/fake-tools/tests/engine_file_phase.rs` (1 test, T4-R21).

**Interfaces:**
- Consumes: Tasks 2 to 4 (the book, `series_dischash`, `tv`), `core::remembered::Remembered`, `read_checked`, `Store::log`.
- Produces:

```rust
// clients::chain
pub struct ChainInputs { pub fingerprint: String, pub disc_id: Option<String>, pub name: Option<DiscName>,
    /// The remembered decision's TMDb identity, which `record_to_fetch` takes before any answer (spec 5, spec 8.1;
    /// engine.md "Pull request 4"; Ruling T4-R6). Def: T4-5. Use: T4-5, T4-9.
    pub remembered: Option<TmdbIdentity> }
// private: fn wanted(book: &LookupBook, inputs: &ChainInputs) -> Option<TmdbIdentity> is record_to_fetch over
// book.lookup_inputs(name) with `remembered` set from inputs; both the record step and the record's status key use it.

// engine::store
/// A remembered-decisions file at `path` through `check_format(REMEMBERED_FORMAT)` (design part 7.4), refused when its
/// inner `fingerprint` is not `fingerprint`, ASCII case ignored (spec 5; engine.md "Pull request 4", the sentence of
/// dev's `--remembered`): Err `<path>: it remembers the disc <inner>, not this disc (<fingerprint>)`. Absent gives
/// Ok(None). Def: T4-5. Use: T4-5.
pub(crate) fn remembered_file(path: &Path, fingerprint: &str) -> Result<Option<(Remembered, Vec<u8>)>, String>;

// engine::arrival
/// `chain_inputs` with the remembered identity (Ruling T4-R6): the job's own `remembered.json` when it reads Ok(Some)
/// (the copy Plan applied, V14), else the data folder's `remembered/<fingerprint>.json`; an Err from either gives None
/// and the engine.log line `<id>: the remembered decision is refused: <reason>` (Plan's line, `steps.rs:930-933`),
/// `<id>` being the job folder's name. Err is chain_inputs' sentence. Def: T4-5. Use: T4-5.
pub(crate) fn round_inputs(job_dir: &Path, store: &Store) -> Result<ChainInputs, String>;
```

- [ ] **Step 1: Write the failing clients tests** (in `chain.rs`'s `mod tests`; `chain` gains `remembered: None`):

```rust
/// Design part 8.3's class of an outcome, as a short text the table compares.
fn class<T>(outcome: &Outcome<T>) -> String {
    match outcome {
        Outcome::Answered { .. } => "answered".to_string(),
        Outcome::Miss { .. } => "miss".to_string(),
        Outcome::Pending { reason, .. } => format!("pending {reason:?}"),
    }
}

#[test]
fn each_http_outcome_has_its_class() {
    let hit = corpus(JOHN_WICK_DISCHASH);
    let page = search_body(&[(245891, "John Wick", "2014-10-22")]);
    let not_utf8: &[u8] = &[0xff, 0xfe, 0x00];
    let unparsed = |url: String| format!("pending Unparsed({:?})", format!("{url}: the body is not UTF-8"));
    // (status, TheDiscDB's body, TMDb's body, TheDiscDB's class, TMDb's class, TMDb's status word)
    let rows: Vec<(u16, &[u8], &[u8], String, String, &str)> = vec![
        (200, hit.as_slice(), page.as_slice(), "answered".into(), "answered".into(), "200"),
        (404, b"", b"", "miss".into(), "miss".into(), "404"),
        (500, b"", b"", "pending Status(500)".into(), "pending Status(500)".into(), "500"),
        (503, b"", b"", "pending Status(503)".into(), "pending Status(503)".into(), "503"),
        (429, b"", b"", "pending Status(429)".into(), "pending Status(429)".into(), "429"),
        (401, b"", b"", "pending Status(401)".into(), "pending TokenRefused".into(), "401"),
        (204, b"", b"", "pending Status(204)".into(), "pending Status(204)".into(), "204"),
        (302, b"", b"", "pending Status(302)".into(), "pending Status(302)".into(), "302"),
        (200, not_utf8, not_utf8, unparsed(dischash_url(FP)), unparsed(search_url("JOHN WICK")), "unparsed"),
    ];
    for (status, discdb_body, tmdb_body, discdb_class, tmdb_class, word) in rows {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), status, discdb_body);
        t.answer(&search_url("JOHN WICK"), status, tmdb_body);
        assert_eq!(class(&discdb_by_fingerprint(&t, FP)), discdb_class, "{status}");
        let outcome = tmdb_search(&t, Some(TOKEN), "JOHN WICK");
        assert_eq!(class(&outcome), tmdb_class, "{status}");
        assert_eq!(crate::saved::status_word(&outcome), word, "{status}");
    }
    let t = RecordedTransport::new();
    t.fail(&search_url("JOHN WICK"), TransportError::Timeout);
    let outcome = tmdb_search(&t, Some(TOKEN), "JOHN WICK");
    assert_eq!((class(&outcome), crate::saved::status_word(&outcome)), ("pending Timeout".to_string(), "000".to_string()));
    // A refused connection (the words Task 7's REFUSED holds, spelled here because Task 7 comes later).
    let t = RecordedTransport::new();
    t.fail(&search_url("JOHN WICK"), TransportError::Unreachable("the connection was refused".to_string()));
    let outcome = tmdb_search(&t, Some(TOKEN), "JOHN WICK");
    assert_eq!((class(&outcome), crate::saved::status_word(&outcome)),
               ("pending Unreachable(\"the connection was refused\")".to_string(), "000".to_string()));
    // No token: no request is made (design part 8.3).
    let t = RecordedTransport::new();
    let outcome = tmdb_search(&t, None, "JOHN WICK");
    assert_eq!((class(&outcome), crate::saved::status_word(&outcome)), ("pending NoToken".to_string(), "000".to_string()));
    assert!(t.requests().is_empty());
}

/// Review Focus 1: TMDb's 429 (its Retry-After ignored) leaves the cleaned search pending, asked once in the round,
/// while the round still asks TheDiscDB and the tagged search; the next round asks it once more.
#[test]
fn a_429_leaves_the_search_pending_as_http_429_and_is_asked_once_in_the_round() {
    let t = RecordedTransport::new();
    let inputs = chain(Some(DISC_ID), Some("PIRANHA_3D"));
    misses(&t);
    t.answer(&search_url("PIRANHA"), 429, br#"{"status_code":25,"status_message":"over the limit"}"#);
    t.answer(&search_url("PIRANHA 3D"), 200, &search_body(&[]));
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("PIRANHA"), search_url("PIRANHA 3D")]);
    assert_eq!(book.states.get(&Lookup::Search), Some(&LookupState::Pending { reason: PendingReason::Status(429) }));
    assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 429", "search-tagged 200"]);
    assert_eq!(book.pending(), (false, true));
    assert!(saved.iter().all(|file| !String::from_utf8_lossy(&file.bytes).contains("status_code")), "P29");
    round(&t, &mut book, &inputs);
    assert_eq!(urls(&t).iter().filter(|url| **url == search_url("PIRANHA")).count(), 2);
    assert_eq!(urls(&t).iter().filter(|url| **url == search_url("PIRANHA 3D")).count(), 1);
}

/// Review Focus 2: a TheDiscDB `Series` entry naming an ID TMDb has only as a movie (or has dropped).
#[test]
fn a_discdb_id_of_the_wrong_kind_is_a_record_miss_and_leaves_nothing_pending() {
    let t = RecordedTransport::new();
    let inputs = chain(None, None);
    t.answer(&dischash_url(FP), 200, &series_dischash(245891));
    t.answer(&record_url(tv(245891)), 404, br#"{"success":false,"status_code":34}"#);
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(urls(&t), [dischash_url(FP), record_url(tv(245891))]);
    assert_eq!(last_status(&saved), ["dischash 200", "tmdb tv/245891 404"]);
    assert_eq!(book.pending(), (false, false));
    assert_eq!(book.lookup_inputs(None).record, None);
    assert!(saved.iter().all(|file| !file.name.starts_with("tmdb")), "no TMDb copy is saved for a miss");
    round(&t, &mut book, &inputs);
    assert_eq!(urls(&t).iter().filter(|url| **url == record_url(tv(245891))).count(), 2, "E:338: asked again");
}

/// Review Focus 4: a captive portal's 200 HTML page.
#[test]
fn a_200_html_page_is_unparsed_and_its_sentence_quotes_no_markup() {
    let t = RecordedTransport::new();
    let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
    misses(&t);
    t.answer(&search_of(&inputs), 200, b"<html><head><title>Sign in</title></head><body>portal</body></html>");
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    let Some(LookupState::Pending { reason: PendingReason::Unparsed(sentence) }) = book.states.get(&Lookup::Search) else {
        panic!("{:?}", book.states)
    };
    assert!(sentence.starts_with(&search_url("JOHN WICK")), "{sentence}");
    assert!(!sentence.contains("<html") && !sentence.contains("portal"), "{sentence}");
    assert!(last_status(&saved).contains(&"search unparsed".to_string()));
    assert!(saved.iter().all(|file| file.name != "tmdb-search.json"));
}

#[test]
fn the_remembered_identity_is_fetched_before_the_search_top_result() {
    let t = RecordedTransport::new();
    let inputs = ChainInputs { remembered: Some(movie(7)), ..chain(Some(DISC_ID), Some("JOHN_WICK")) };
    misses(&t);
    t.answer(&search_of(&inputs), 200, &search_body(&[(9, "John Wick", "2014-10-22")]));
    t.answer(&record_url(movie(7)), 200, &movie_body(7, "Remembered Movie", "2001-02-03"));
    let mut book = LookupBook::default();
    let saved = round(&t, &mut book, &inputs);
    assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(movie(7))]);
    assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(movie(7)));
    assert!(last_status(&saved).contains(&"tmdb movie/7 200".to_string()));
}
```

- [ ] **Step 2: Write the failing engine tests.** In `store.rs`'s `mod tests` (imports `lazymkv_core::testsupport::{Scratch, ScratchExt}`; the module's own `Temp` stays for its other tests):

```rust
#[test]
fn a_remembered_file_for_another_disc_is_refused_ascii_case_ignored() {
    let tmp = Scratch::new("remembered-fp");
    let store = Store::new(tmp.0.clone(), Arc::new(|| 7), None);
    let path = tmp.0.join("remembered").join("ABC.json");
    let text = |inner: &str| format!(r#"{{"format":"lazymkv remembered v1","fingerprint":"{inner}","tmdb":{{"kind":"movie","id":2}},"titles":[]}}"#);
    tmp.write("remembered/ABC.json", &text("FFFF"));
    assert_eq!(store.remembered("ABC").unwrap_err(),
        format!("{}: it remembers the disc FFFF, not this disc (ABC)", path.display()));
    tmp.write("remembered/ABC.json", &text("abc"));
    assert_eq!(store.remembered("ABC").unwrap().unwrap().0.tmdb.id, 2);
}
```

In `arrival.rs`'s `mod tests` (imports `std::fs`, `std::sync::Arc`, `crate::store::Store`, `lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind}`):

```rust
const META: &str = r#"{"volume_label":"SYNTHETIC_MOVIE","format":"bluray","fingerprint":"ABC","disc_id":null,"file_count":5,"total_bytes":1,"captured_unix":7}"#;
fn remembered_text(fingerprint: &str, id: i64) -> String {
    format!(r#"{{"format":"lazymkv remembered v1","fingerprint":"{fingerprint}","tmdb":{{"kind":"movie","id":{id}}},"titles":[]}}"#)
}
fn movie(id: i64) -> Option<TmdbIdentity> { Some(TmdbIdentity { kind: TmdbKind::Movie, id }) }

#[test]
fn a_round_takes_the_jobs_remembered_copy_first_then_the_data_folders() {
    let data = Scratch::new("round-data");
    let store = Store::new(data.0.clone(), Arc::new(|| 7), None);
    let job = Scratch::new("round-job");
    job.write("meta.json", META);
    assert_eq!(round_inputs(&job.0, &store).unwrap().remembered, None);
    data.write("remembered/ABC.json", &remembered_text("ABC", 2));
    assert_eq!(round_inputs(&job.0, &store).unwrap().remembered, movie(2));
    job.write("remembered.json", &remembered_text("ABC", 1));
    let inputs = round_inputs(&job.0, &store).unwrap();
    assert_eq!(inputs.remembered, movie(1));
    let plain = chain_inputs(&job.0).unwrap();
    assert_eq!((inputs.fingerprint, inputs.disc_id, inputs.name), (plain.fingerprint, plain.disc_id, plain.name));
}

#[test]
fn a_remembered_file_for_another_disc_is_refused_and_the_round_has_none() {
    let data = Scratch::new("round-refused-data");
    let store = Store::new(data.0.clone(), Arc::new(|| 7), None);
    let job = Scratch::new("round-refused-job");
    job.write("meta.json", META);
    data.write("remembered/ABC.json", &remembered_text("FFFF", 2));
    assert_eq!(round_inputs(&job.0, &store).unwrap().remembered, None);
    let id = job.0.file_name().unwrap().to_str().unwrap();
    let path = data.0.join("remembered").join("ABC.json");
    let line = format!("{id}: the remembered decision is refused: {}: it remembers the disc FFFF, not this disc (ABC)", path.display());
    let log = fs::read_to_string(data.0.join("engine.log")).unwrap();
    assert!(log.contains(&line), "{log}");
}
```

In `file_phase.rs`'s `mod tests`:

```rust
#[test]
fn a_search_answered_http_429_waits_with_http_429() {
    let mut book = LookupBook::default();
    book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::Status(429) });
    assert_eq!(waiting_message(&book), "search: HTTP 429");
}
```

In `crates/fake-tools/tests/engine_disc_phase.rs` (engine.md "Pull request 4": before this, such a file planned with its identity and then failed Decide with "no plan: ..."):

```rust
#[test]
fn a_remembered_file_for_another_disc_plans_without_it() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), format!(concat!(
        r#"{{"format":"lazymkv remembered v1","fingerprint":"FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF","#,
        r#""tmdb":{{"kind":"movie","id":900001}},"titles":[]}}"#))).unwrap();
    let id = run_to(&h, "Ejected");
    assert_ne!(h.record(&id).plan.unwrap().identity.step, IdentityStep::Remembered);
    assert!(!h.job_dir(&id).join("remembered.json").exists());
    let log = h.engine_log();
    assert!(log.contains(&format!("it remembers the disc FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, not this disc ({fp})")), "{log}");
    assert!(log.contains(&format!("{id}: the remembered decision is refused: ")), "{log}");
}

/// engine.md "Pull request 4": a disc whose remembered identity differs from the search's top result plans with a
/// TMDb record for its own identity (the round's ChainInputs carries it, Ruling T4-R6).
#[test]
fn a_remembered_identity_other_than_the_search_top_result_plans_with_its_own_record() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), format!(concat!(
        r#"{{"format":"lazymkv remembered v1","fingerprint":"{fp}","#,
        r#""tmdb":{{"kind":"movie","id":900002}},"titles":[]}}"#), fp = fp)).unwrap();
    let remembered = TmdbIdentity { kind: TmdbKind::Movie, id: 900002 };
    h.http().answer(&record_url(remembered), 200, concat!(
        r#"{"id":900002,"title":"Synthetic Movie Remembered","original_title":"Synthetic Movie Remembered","#,
        r#""release_date":"2002-03-04","original_language":"en","runtime":119,"external_ids":{"imdb_id":"tt0900002"}}"#)
        .as_bytes());
    let id = run_to(&h, "Ejected");
    let urls: Vec<String> = h.http().requests().into_iter().map(|(url, _)| url).collect();
    assert!(urls.contains(&record_url(remembered)), "{urls:?}");
    assert!(!urls.contains(&record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 900001 })), "{urls:?}");
    let status = fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap();
    assert!(status.lines().any(|line| line == "tmdb movie/900002 200"), "{status}");
    let record: Value = serde_json::from_slice(&fs::read(h.job_dir(&id).join("tmdb-record.json")).unwrap()).unwrap();
    assert_eq!(record["id"], json!(900002));
    let plan = h.record(&id).plan.unwrap();
    assert_eq!((plan.identity.step, plan.identity.tmdb), (IdentityStep::Remembered, Some(remembered)));
}
```

`engine_disc_phase.rs` adds `use lazymkv_clients::record_url;` and `use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};`.

In `crates/fake-tools/tests/engine_file_phase.rs` (Ruling T4-R21; the copy fails as the C7 test of `engine_disc_phase.rs` makes it fail):

```rust
/// Ruling T4-R21, the second half of engine.md's V14 item, accepted with no code: Decide reads only the job's own
/// copy (R-B1), so a copy Plan could not write plans with the remembered identity and titles, decides without the
/// track overrides (audio part 0.95, not the Keep's 1.00), and job.log names the copy.
#[test]
fn an_unwritable_remembered_copy_decides_without_overrides_and_logs_it() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), remembered_keep(&fp)).unwrap();
    h.insert("tok0", "movie-direct");
    h.run_until("Scanned");
    let id = format!("{}-1", fp.to_ascii_lowercase());
    let dir = h.job_dir(&id);
    fs::create_dir_all(dir.join(format!("remembered.json{}", lazymkv_platform::TEMP_SUFFIX))).unwrap();
    h.restart();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    assert_eq!(h.record(&id).plan.unwrap().identity.step, IdentityStep::Remembered);
    assert!(!dir.join("remembered.json").exists());
    let doc = json_of(&dir.join(KEY).join("decide.json"));
    assert_eq!(doc["decision"]["decision"]["audio"]["confidence"], json!(0.95));
    let log = fs::read_to_string(dir.join("job.log")).unwrap();
    assert!(log.contains(&format!("{}: ", dir.join("remembered.json").display())), "{log}");
}
```

- [ ] **Step 3: Run them and see them fail.** `scripts/box.sh cargo test -p lazymkv-clients --locked chain`: compile error, `ChainInputs` has no field `remembered`. `scripts/box.sh cargo test -p lazymkv-engine --locked remembered`: compile error, `round_inputs` not found. `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase another_disc`: FAIL (the plan's step is `Remembered`). `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase own_record`: FAIL (the round asks movie 900001's record, the search's top result, and never 900002's). The T4-R21 test and `a_search_answered_http_429_waits_with_http_429` pass once the target compiles with this step's code, without further changes: they pin behaviour this task must keep.

- [ ] **Step 4: Implement.** `ChainInputs.remembered` and the private `wanted` as the Interfaces say; `status_file(book, inputs)` and `start(book, lookup, inputs, save)` take the inputs so the record's in-flight and pending key name the remembered identity. `remembered_file` is `read_checked` followed by the `eq_ignore_ascii_case` comparison; `Store::remembered(fingerprint)` returns `remembered_file(&<data>/remembered/<fingerprint>.json, fingerprint)`, so Plan's existing `Err` branch (`steps.rs:930-933`) now refuses another disc's file. `round_inputs` calls `chain_inputs`, then `remembered_file(&job_dir.join("remembered.json"), &inputs.fingerprint)`, then on `Ok(None)` `store.remembered(&inputs.fingerprint)`, and sets `remembered` to the file's `tmdb`. No other code changes: Plan's own reading of the remembered decision is untouched.

- [ ] **Step 5: Run and see them pass.** `scripts/box.sh cargo test -p lazymkv-clients --locked chain`, `scripts/box.sh cargo test -p lazymkv-engine --locked remembered`, `scripts/box.sh cargo test -p lazymkv-engine --locked http_429`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase another_disc`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_disc_phase own_record`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase unwritable_remembered`: PASS. Then `scripts/box.sh cargo test --workspace --locked`: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/clients/src/chain.rs crates/engine/src/store.rs crates/engine/src/arrival.rs crates/engine/src/steps.rs crates/engine/src/file_phase.rs crates/fake-tools/tests/engine_disc_phase.rs crates/fake-tools/tests/engine_file_phase.rs
git commit -m "clients: part 8.3's classes and the remembered identity in the chain"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 6.

---

### Task 6: POST and connect on the transport trait (design parts 8.1, 8.6; T4-R12)

The notifier (Task 12) posts each notification and the self-test (Task 13) opens a Transmission Control Protocol (TCP) connection to each webhook's host and port without posting, so the trait both reach the network through gains the two operations. Nothing here opens a socket: the live transport is Task 7's.

**Files:**
- Modify: `crates/clients/src/transport.rs:1-4` (module doc), `:25-30` (the trait), `:41-53` (`OFFLINE` reworded, `OfflineTransport` answers all three), new `WebhookTarget` and `webhook_target` after `user_agent`; `:65-75` (the test `the_offline_transport_answers_every_url_unreachable` replaced); 2 new tests.
- Modify: `crates/clients/src/recorded.rs:57-63` (`State` gains `posts`, `connects`, `connect_answers`), `:155-193` (`posts`, `connect_answer`, `connects`; `get`'s serving moved into a private `serve(url)` that `post` shares); 3 tests.
- Modify: `crates/clients/src/lib.rs:1, :16` (module doc no longer says the live transport "arrives in pull request 4"; re-export `webhook_target`, `WebhookTarget`).
- Modify: `crates/core/src/ipc.rs:30-31` (`WEBHOOK_SECRETS` beside `SECRET_NAMES`; 1 test).
- Modify: `crates/engine/src/ipc_server.rs:47-48` (the private const removed; `:460` reads `lazymkv_core::ipc::WEBHOOK_SECRETS`), `:402-411` (`secret_host` becomes a call of `lazymkv_clients::webhook_target`; its doc says it gives None for a value that is not an http or https URL with a host).
- No edit to `crates/fake-tools/tests/engine_ipc.rs`: the W26 test asserts `dischash: {OFFLINE}` through the constant.

**Interfaces:**
- Consumes: nothing new.
- Produces:

```rust
// clients::transport
/// A blocking HTTP transport shared between the engine's threads (design part 8.1). Every HTTP status, a 3xx
/// included, is `Ok(Response)`; reading it is the caller's job (Ruling T4-R12: no redirect is followed). Errors never
/// hold a request header or, for `post` and `connect`, the URL (design part 8.6: a webhook URL is a secret), except
/// the recorded transport's `Unrecorded`, which the notifier maps to fixed words (Task 12). Def: T15. Use: T16, T4-6,
/// T4-7, T4-12, T4-13.
pub trait HttpTransport: Send + Sync {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError>;
    /// One POST of `body` with `headers` (design part 8.6). Def: T4-6. Use: T4-7, T4-12.
    fn post(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Response, TransportError>;
    /// A TCP connection to `host:port` within the timeout, then closed; no TLS handshake and no request (design part
    /// 8.6, "opens a connection to each URL's host without posting"; Ruling T4-R14). Def: T4-6. Use: T4-7, T4-13.
    fn connect(&self, host: &str, port: u16) -> Result<(), TransportError>;
}
/// The reason `OfflineTransport` gives (Ruling T4-R16). Def: T3-21, T4-6. Use: T4-12, T4-17.
pub const OFFLINE: &str = "the offline transport asks nothing (a test double)";
/// A test double that opens no socket: `get`, `post` and `connect` all give `Unreachable(OFFLINE)` (Ruling T4-R16;
/// the binary passes `UreqTransport` from Task 17). Def: T3-21, T4-6. Use: T4-12, T4-17.
pub struct OfflineTransport;
/// Where a webhook URL connects: its scheme, host and port, nothing else (design part 8.6; Ruling T4-R14).
/// Def: T4-6. Use: T4-12, T4-13.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookTarget { pub https: bool, pub host: String, pub port: u16 }
/// A webhook URL's target (design part 8.6). The scheme is `http` (port 80) or `https` (443), ASCII case ignored;
/// the host is read as the engine's `secret_host` reads it (the authority up to `/`, `?` or `#`, any `user@` dropped,
/// a bracketed IPv6 address unbracketed); an explicit port wins. None for any other scheme, an empty host, whitespace
/// anywhere in the URL, or a port that is empty, not a number or not in 1..=65535. Def: T4-6. Use: T4-12, T4-13.
pub fn webhook_target(url: &str) -> Option<WebhookTarget>;

// clients::recorded (S63, S64)
impl RecordedTransport {
    /// Every POST so far, in arrival order: the URL, the headers and the body, a POST still waiting on its gate
    /// included (S64); memory only. A POST is served from the same per-URL answers as a GET (`answer`, `fail`, `gate`),
    /// and `requests` keeps listing GETs only. Def: T4-6. Use: T4-12, T4-13, T4-18.
    pub fn posts(&self) -> Vec<(String, Vec<(String, String)>, Vec<u8>)>;
    /// Sets the outcome every `connect(host, port)` gets from now on. Def: T4-6. Use: T4-13.
    pub fn connect_answer(&self, host: &str, port: u16, outcome: Result<(), TransportError>);
    /// Every connect so far, in arrival order. An unset pair gives `Err(Unrecorded("<host>:<port>"))` and is not
    /// listed by `unrecorded`, which lists URLs. Def: T4-6. Use: T4-13.
    pub fn connects(&self) -> Vec<(String, u16)>;
}

// core::ipc
/// The two secrets that hold a webhook URL, in the order the notifier delivers and the self-test checks them (design
/// part 8.6, S77). Def: T4-6. Use: T4-6, T4-12, T4-13.
pub const WEBHOOK_SECRETS: [&str; 2] = ["ha_webhook_url", "webhook_url"];
```

- [ ] **Step 1: Write the failing tests.** In `transport.rs`'s `mod tests`:

```rust
#[test]
fn webhook_target_reads_scheme_host_and_port() {
    let target = |https, host: &str, port| Some(WebhookTarget { https, host: host.to_string(), port });
    assert_eq!(webhook_target("http://ha.local:8123/api/webhook/x"), target(false, "ha.local", 8123));
    assert_eq!(webhook_target("https://h/x"), target(true, "h", 443));
    assert_eq!(webhook_target("http://h"), target(false, "h", 80));
    assert_eq!(webhook_target("HTTP://H/"), target(false, "H", 80));
    assert_eq!(webhook_target("http://[::1]:8123/"), target(false, "::1", 8123));
    assert_eq!(webhook_target("https://[fe80::1]/x"), target(true, "fe80::1", 443));
    for refused in ["ftp://h/", "http:///x", "not a url", "h:8123", "", "http://h:0/", "http://h:70000/",
                    "http://h:/", "http://h:80x/", "http://h /x"] {
        assert_eq!(webhook_target(refused), None, "{refused}");
    }
}

/// Review Focus 3: userinfo, a port, a path, a query and a fragment; only the host and port come out.
#[test]
fn a_webhook_url_with_userinfo_path_and_query_targets_only_its_host_and_port() {
    let url = "http://user-sentinel-7c1e:pass-sentinel-4b9d@ha.local:8123/api/webhook/hook-sentinel-7f3a?x=1#f";
    let target = webhook_target(url).unwrap();
    assert_eq!(target, WebhookTarget { https: false, host: "ha.local".to_string(), port: 8123 });
    let shown = format!("{target:?}");
    for secret in ["user-sentinel-7c1e", "pass-sentinel-4b9d", "hook-sentinel-7f3a", "x=1", "/api/webhook"] {
        assert!(!shown.contains(secret), "{shown}");
    }
}

/// Ruling T4-R16: the test double answers every operation unreachable with its one sentence.
#[test]
fn the_offline_transport_answers_get_post_and_connect_with_its_sentence() {
    assert_eq!(OFFLINE, "the offline transport asks nothing (a test double)");
    let offline = Err(TransportError::Unreachable(OFFLINE.to_string()));
    for url in ["https://thediscdb.com/api/dischash/ABC", "https://api.themoviedb.org/3/movie/900001", "http://x", ""] {
        assert_eq!(OfflineTransport.get(url, &[("User-Agent", "lazymkv/test")]), offline, "{url}");
        assert_eq!(OfflineTransport.post(url, &[("Content-Type", "application/json")], b"{}"), offline, "{url}");
    }
    assert_eq!(OfflineTransport.connect("ha.local", 8123), Err(TransportError::Unreachable(OFFLINE.to_string())));
}
```

In `recorded.rs`'s `mod tests`:

```rust
const HOOK: &str = "http://ha.local:8123/api/webhook/hook-sentinel-7f3a";

#[test]
fn a_recorded_post_is_answered_from_its_urls_queue_and_kept_in_memory() {
    let t = RecordedTransport::new();
    t.answer(HOOK, 500, b"");
    t.answer(HOOK, 200, b"");
    let headers = [("Content-Type", "application/json"), ("User-Agent", "lazymkv/test")];
    assert_eq!(t.post(HOOK, &headers, br#"{"event":"filed"}"#), Ok(Response { status: 500, body: Vec::new() }));
    assert_eq!(t.post(HOOK, &headers, b"{}"), Ok(Response { status: 200, body: Vec::new() }));
    let sent: Vec<(String, String)> =
        headers.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect();
    assert_eq!(t.posts(), vec![
        (HOOK.to_string(), sent.clone(), br#"{"event":"filed"}"#.to_vec()),
        (HOOK.to_string(), sent, b"{}".to_vec()),
    ]);
    assert!(t.requests().is_empty(), "requests lists GETs only");
    assert_eq!(t.post("http://other/x", &headers, b""), Err(TransportError::Unrecorded("http://other/x".to_string())));
}

#[test]
fn a_gated_post_waits_until_opened() {
    let t = std::sync::Arc::new(RecordedTransport::new());
    t.answer(HOOK, 200, b"");
    let gate = t.gate(HOOK);
    let (sent, got) = mpsc::channel();
    let posting = t.clone();
    std::thread::spawn(move || sent.send(posting.post(HOOK, &[], b"{}")).unwrap());
    assert!(got.recv_timeout(Duration::from_millis(200)).is_err());
    assert_eq!(t.posts().len(), 1);
    gate.open();
    assert_eq!(got.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(Response { status: 200, body: Vec::new() }));
}

#[test]
fn an_unrecorded_connect_names_only_host_and_port() {
    let t = RecordedTransport::new();
    assert_eq!(t.connect("ha.local", 8123), Err(TransportError::Unrecorded("ha.local:8123".to_string())));
    t.connect_answer("ha.local", 8123, Ok(()));
    t.connect_answer("ha.local", 9000, Err(TransportError::Timeout));
    assert_eq!(t.connect("ha.local", 8123), Ok(()));
    assert_eq!(t.connect("ha.local", 9000), Err(TransportError::Timeout));
    assert_eq!(t.connects(), vec![("ha.local".to_string(), 8123), ("ha.local".to_string(), 8123), ("ha.local".to_string(), 9000)]);
    assert!(t.requests().is_empty() && t.posts().is_empty() && t.unrecorded().is_empty());
}
```

In `crates/core/src/ipc.rs`'s `mod tests`:

```rust
#[test]
fn the_webhook_secrets_are_secret_names_in_channel_order() {
    assert_eq!(WEBHOOK_SECRETS, ["ha_webhook_url", "webhook_url"]);
    assert!(WEBHOOK_SECRETS.iter().all(|name| SECRET_NAMES.contains(name) && is_secret_name(name)));
}
```

- [ ] **Step 2: Run them and see them fail.** `scripts/box.sh cargo test -p lazymkv-clients --locked transport`: compile error, `webhook_target`, `WebhookTarget` and `post` not found. `scripts/box.sh cargo test -p lazymkv-core --locked webhook_secrets`: compile error, `WEBHOOK_SECRETS` not found.

- [ ] **Step 3: Implement.** The trait methods and docs as the Interfaces say; `OfflineTransport` implements all three with `Unreachable(OFFLINE)`. `webhook_target` splits on the first `://` (an absent separator is None), compares the scheme with `eq_ignore_ascii_case`, takes the authority up to the first `/`, `?` or `#`, drops everything up to its last `@`, reads a bracketed host up to `]` and an optional `:port` after it, else splits the host and port text at the first `:`; the port text is parsed as `u16` and refused when it is 0. `RecordedTransport::post` pushes `(url, headers, body)` onto `posts`, waits on the URL's gate as `get` does, then calls the private `serve(url)` (`get`'s slot lookup, which pushes an unanswered URL onto `unrecorded` and gives `Unrecorded(url)`); `connect` pushes `(host, port)` onto `connects` and returns the clone of `connect_answers[(host, port)]`, else `Unrecorded("<host>:<port>")`. `WEBHOOK_SECRETS` moves to `core::ipc` unchanged, and `ipc_server.rs` imports it; `secret_host` (`crates/engine/src/ipc_server.rs:402-411`) keeps no second copy of the authority reading: its body becomes `lazymkv_clients::webhook_target(url).map(|target| target.host)` (engine already depends on clients), and its test `a_webhook_secret_shows_only_its_host` stays. Its doc gains one sentence before `Def:`: "None for a value that is not an http or https URL with a host (`webhook_target`'s reading)."; the rest of the doc stays.

- [ ] **Step 4: Run and see them pass.** `scripts/box.sh cargo test -p lazymkv-clients --locked transport`, `scripts/box.sh cargo test -p lazymkv-clients --locked recorded`, `scripts/box.sh cargo test -p lazymkv-core --locked webhook_secrets`, then `scripts/box.sh cargo test -p lazymkv-engine --locked a_webhook_secret_shows_only_its_host`: PASS. Then `scripts/box.sh cargo test --workspace --locked`: PASS (the W26 test `with_the_offline_transport_a_disc_waits_for_its_lookups` reads the new sentence through `OFFLINE`).

- [ ] **Step 5: Commit.**

```bash
git add crates/clients/src/transport.rs crates/clients/src/recorded.rs crates/clients/src/lib.rs crates/core/src/ipc.rs crates/engine/src/ipc_server.rs
git commit -m "clients: POST and connect on the transport trait"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 7.

---

### Task 7: The live transport (design part 8.1; engine.md "the token kept out of error text")

**Files:**
- Create: `crates/clients/src/live.rs` (`UreqTransport`, `MAX_BODY_BYTES`, the six error words; `mod tests` over loopback listeners only).
- Modify: `crates/clients/src/lib.rs:3-7` (`pub mod live;`), `:16` (re-exports below).

**Interfaces:**
- Consumes: Task 1's `ureq` (rustls on ring, `webpki-roots`); Task 6's `HttpTransport::{get, post, connect}`; `transport::{Response, TransportError, TIMEOUT_S}`.
- Produces (re-exported from `lazymkv_clients`):

```rust
// live.rs (design part 8.1; Ruling T4-R12)
pub const MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;
pub const REFUSED: &str = "the connection was refused";
pub const NO_HOST: &str = "the host name could not be resolved";
pub const TLS_FAILED: &str = "the TLS handshake failed";
pub const CONNECTION_FAILED: &str = "the connection failed";
pub const BODY_TOO_LONG: &str = "the answer is longer than 8 MiB";
pub const BAD_ANSWER: &str = "the answer could not be read";
pub struct UreqTransport { agent: ureq::Agent, timeout: Duration }
impl UreqTransport {
    #[allow(clippy::new_without_default)] // a Default would hide that this builds the live transport
    pub fn new() -> UreqTransport;                            // with_timeout(Duration::from_secs(TIMEOUT_S))
    pub fn with_timeout(timeout: Duration) -> UreqTransport;
    pub fn timeout(&self) -> Duration;
}
impl HttpTransport for UreqTransport { /* get, post, connect */ }
```

Every `pub` item's doc names design part 8.1 or Ruling T4-R12 and ends `Def: T4-7. Use: T4-10, T4-17, T4-19.` (the constants: `Use: T4-7.`).

- [ ] **Step 1: Write the failing tests** in `live.rs`'s `mod tests`. Every URL is `http://127.0.0.1:<port>`: no host name (so no Domain Name System lookup), no other address (so no firewall prompt), no TheDiscDB or TMDb host.

```rust
use super::*;
use crate::transport::user_agent;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const SENTINEL: &str = "token-sentinel-5d2e";
const HOOK: &str = "/api/webhook/hook-sentinel-7f3a";
const QUICK: Duration = Duration::from_millis(500);
/// Windows answers a connection to a closed loopback port only after about 2 s of connection retries.
const REFUSAL: Duration = Duration::from_secs(5);
/// Taken by every test here: `no_proxy_is_read_from_the_environment` sets proxy variables process-wide.
static ENV: Mutex<()> = Mutex::new(());

fn answer(status: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// The request's head, then as many body bytes as its Content-Length names.
fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let (mut bytes, mut byte) = (Vec::new(), [0u8; 1]);
    while !bytes.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 { bytes.push(byte[0]); }
    let head = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    let length = head.lines().find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok()).unwrap_or(0);
    let mut body = vec![0u8; length];
    let _ = stream.read_exact(&mut body);
    bytes.extend(body);
    bytes
}

/// A loopback listener answering `answers` to one connection each, in order; joins to each request's bytes.
fn serve(answers: Vec<Vec<u8>>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = thread::spawn(move || answers.into_iter().map(|answer| {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        let _ = stream.write_all(&answer);
        String::from_utf8_lossy(&request).into_owned()
    }).collect());
    (base, handle)
}

/// A port nothing listens on: bound, then dropped.
fn closed_port() -> u16 { TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() }

/// Whether nothing connected to `listener`.
fn untouched(listener: &TcpListener) -> bool {
    listener.set_nonblocking(true).unwrap();
    matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock)
}

fn bearer() -> String { format!("Bearer {SENTINEL}") }

#[test]
fn a_get_sends_the_headers_it_is_given_and_reads_any_status() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let (base, seen) = serve(vec![answer("404 Not Found", b"gone"), answer("500 Internal Server Error", b"oops")]);
    let t = UreqTransport::with_timeout(QUICK);
    let (ua, auth) = (user_agent(), bearer());
    let headers = [("Authorization", auth.as_str()), ("User-Agent", ua.as_str())];
    assert_eq!(t.get(&format!("{base}/a"), &headers), Ok(Response { status: 404, body: b"gone".to_vec() }));
    assert_eq!(t.get(&format!("{base}/b"), &headers), Ok(Response { status: 500, body: b"oops".to_vec() }));
    for request in seen.join().unwrap() {
        let head = request.to_ascii_lowercase();
        assert!(head.contains(&format!("user-agent: {}", user_agent())), "{head}");
        assert_eq!(head.matches("user-agent:").count(), 1, "ureq adds no User-Agent of its own: {head}");
        assert!(head.contains(&format!("authorization: bearer {SENTINEL}")), "{head}");
    }
}

#[test]
fn a_post_sends_its_body_and_content_type() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let (base, seen) = serve(vec![answer("200 OK", b"")]);
    let ua = user_agent();
    let headers = [("Content-Type", "application/json"), ("User-Agent", ua.as_str())];
    let sent = UreqTransport::with_timeout(QUICK).post(&format!("{base}{HOOK}"), &headers, br#"{"event":"filed"}"#);
    assert_eq!(sent, Ok(Response { status: 200, body: Vec::new() }));
    let request = seen.join().unwrap().remove(0);
    assert!(request.starts_with(&format!("POST {HOOK} HTTP/1.1\r\n")), "{request}");
    assert!(request.to_ascii_lowercase().contains("content-type: application/json"), "{request}");
    assert!(request.ends_with("\r\n\r\n{\"event\":\"filed\"}"), "{request}");
}

#[test]
fn a_refused_port_is_unreachable_and_the_text_holds_no_url_or_header() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let url = format!("http://127.0.0.1:{}{HOOK}", closed_port());
    let t = UreqTransport::with_timeout(REFUSAL);
    let auth = bearer();
    let got = t.get(&url, &[("Authorization", auth.as_str())]);
    let posted = t.post(&url, &[("Authorization", auth.as_str())], b"{}");
    for error in [got, posted] {
        assert_eq!(error, Err(TransportError::Unreachable(REFUSED.to_string())));
        let text = format!("{error:?}");
        for secret in [SENTINEL, HOOK, "hook-sentinel-7f3a", "127.0.0.1"] { assert!(!text.contains(secret), "{text}"); }
    }
}

#[test]
fn a_silent_server_times_out() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}/x", listener.local_addr().unwrap().port());
    let (done, wait) = mpsc::channel::<()>();
    let holder = thread::spawn(move || { let held = listener.accept(); let _ = wait.recv(); drop(held); });
    let started = Instant::now();
    assert_eq!(UreqTransport::with_timeout(QUICK).get(&url, &[]), Err(TransportError::Timeout));
    // A "gives up in time" bound: it does not scale with ci_factor (Global Constraints, Tests).
    assert!(started.elapsed() < QUICK * 4, "{:?}", started.elapsed());
    done.send(()).unwrap();
    holder.join().unwrap();
}

#[test]
fn a_redirect_is_not_followed_and_reads_as_its_status() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let elsewhere = TcpListener::bind("127.0.0.1:0").unwrap();
    let location = format!("http://127.0.0.1:{}/login", elsewhere.local_addr().unwrap().port());
    let moved = format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let (base, seen) = serve(vec![moved.into_bytes()]);
    let auth = bearer();
    let got = UreqTransport::with_timeout(QUICK).get(&format!("{base}/movie/1"), &[("Authorization", auth.as_str())]);
    assert_eq!(got, Ok(Response { status: 302, body: Vec::new() }));
    assert_eq!(seen.join().unwrap().len(), 1, "the bearer token went to the first host once");
    assert!(untouched(&elsewhere), "the redirect's target saw a connection");
}

#[test]
fn a_body_over_8_mib_is_unreachable_and_says_so() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let body = vec![b'x'; (MAX_BODY_BYTES + 1) as usize];
    let (base, seen) = serve(vec![answer("200 OK", &body)]);
    let got = UreqTransport::with_timeout(REFUSAL).get(&format!("{base}/big"), &[]);
    assert_eq!(got, Err(TransportError::Unreachable(BODY_TOO_LONG.to_string())));
    let _ = seen.join();
}

#[test]
fn no_proxy_is_read_from_the_environment() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let named = format!("http://127.0.0.1:{}", proxy.local_addr().unwrap().port());
    let names = ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"];
    for name in names { std::env::set_var(name, &named); }
    let (base, seen) = serve(vec![answer("200 OK", b"direct")]);
    let got = UreqTransport::with_timeout(QUICK).get(&format!("{base}/x"), &[]);
    for name in names { std::env::remove_var(name); }
    assert_eq!(got, Ok(Response { status: 200, body: b"direct".to_vec() }));
    assert_eq!(seen.join().unwrap().len(), 1);
    assert!(untouched(&proxy), "the proxy the environment names saw a connection");
}

#[test]
fn connect_reaches_a_listening_port_and_refuses_a_closed_one() {
    let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let open = TcpListener::bind("127.0.0.1:0").unwrap();
    let t = UreqTransport::with_timeout(REFUSAL);
    assert_eq!(t.connect("127.0.0.1", open.local_addr().unwrap().port()), Ok(()));
    assert_eq!(t.connect("127.0.0.1", closed_port()), Err(TransportError::Unreachable(REFUSED.to_string())));
}

#[test]
fn the_production_timeout_is_ten_seconds() {
    // Construction only: nothing is sent (Global Constraints, No network on CI).
    assert_eq!(UreqTransport::new().timeout(), Duration::from_secs(TIMEOUT_S));
    assert_eq!(UreqTransport::with_timeout(QUICK).timeout(), QUICK);
}
```

- [ ] **Step 2: Run and see it fail.** Run: `scripts/box.sh cargo test -p lazymkv-clients --locked live::`. Expected: compile error, `live` has no `UreqTransport`.

- [ ] **Step 3: Implement** `live.rs` to the signatures above.
  - `with_timeout` builds one `ureq::Agent` with exactly these settings (ureq 3's names; Task 1's approved version is the judge, and a renamed setting takes its equivalent with the tests above unchanged): `timeout_global(Some(timeout))`, `http_status_as_error(false)` (every status is `Ok(Response)`, transport.rs:26-27), `max_redirects(0)` with `max_redirects_will_error(false)` (a 3xx is its own `Response`, so a bearer token never reaches another host, T4-R12), `proxy(None)` (no proxy from the environment), `user_agent(AutoHeaderValue::None)` (the caller's `User-Agent` is the only one), and a Transport Layer Security (TLS) config of `TlsProvider::Rustls` with `RootCerts::WebPki` (the compiled-in roots of design part 8.1).
  - `get` and `post` send the caller's headers unchanged; the body is read with `with_config().limit(MAX_BODY_BYTES).read_to_vec()`.
  - `connect(host, port)`: `(host, port).to_socket_addrs()` (an error or no address is `Unreachable(NO_HOST)`), then `TcpStream::connect_timeout` (a Transmission Control Protocol connection) to each address within what is left of `timeout`; the first that connects is `Ok(())` and the stream is dropped at once (no TLS handshake, no request, T4-R14).
  - One private mapping, by variant only, never by `Display` or `Debug` (engine.md "the token kept out of error text"): ureq's `Timeout` and an I/O `TimedOut` or `WouldBlock` give `Timeout`; an I/O `ConnectionRefused` gives `REFUSED`; `HostNotFound` gives `NO_HOST`; a TLS or rustls error gives `TLS_FAILED`; `ConnectionFailed` and every other I/O error give `CONNECTION_FAILED`; `BodyExceedsLimit` gives `BODY_TOO_LONG`; every other variant gives `BAD_ANSWER`. `connect`'s `io::Error` maps by `ErrorKind` the same way.
  - `lib.rs`: `pub use live::{UreqTransport, BAD_ANSWER, BODY_TOO_LONG, CONNECTION_FAILED, MAX_BODY_BYTES, NO_HOST, REFUSED, TLS_FAILED};`.

- [ ] **Step 4: Run and see it pass.** Run: `scripts/box.sh cargo test -p lazymkv-clients --locked live::`. Expected: 9 passed. Then `scripts/box.sh cargo test --workspace --locked`: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/clients/src/live.rs crates/clients/src/lib.rs
git commit -m "clients: the ureq transport with the token kept out of error text"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems, and the controller reads the draft pull request's CodeQL result (Global Constraints, CodeQL): an alert is a red CI run and goes back to this task before Task 8.

---

### Task 8: Stale copies and the refetch functions (design part 8.4, R-B4, T4-R7)

**Files:**
- Create: `crates/core/src/lookup/stale.rs`; `pub mod stale;` at `crates/core/src/lookup/mod.rs:9` (after `pub mod tmdb;`).
- Modify: `crates/core/src/lookup/mod.rs` (`search_queries` beside Task 3's `entry_search_title`), `crates/core/src/lookup/tests.rs` (one test).
- Create: `crates/clients/src/refetch.rs`.
- Modify: `crates/clients/src/lookups.rs:62-78` (`PendingReason::Stale`; `reason_text` moved here from `crates/engine/src/file_phase.rs:367-377`), `crates/clients/src/chain.rs:144-150` (`Stale` gives `000`), `crates/clients/src/chain.rs:183-189` (`json_file` moves to `saved.rs` as `pub(crate)`, the chain imports it), `crates/clients/src/lib.rs` (`pub mod refetch;`, `pub use refetch::{refetch_record, refetch_search, stale_copies, status_record, StaleCopy};`, `reason_text` added to the `lookups` re-export).
- Modify: `crates/engine/src/file_phase.rs:12, :367-377, :382-388` (the private `reason_text` removed; `waiting_message` calls `lazymkv_clients::reason_text`; `TIMEOUT_S` leaves the import).
- Modify: `crates/core/src/testsupport/mod.rs:524-594` (the test writers `tmdb_movie_json`, `tmdb_tv_json` and `search_json` end their JSON with `"fetched_at": "2026-10-23"`, the fixed test date Task 9 passes, so the pages they write have the real slim form of design part 8.4 and Ruling T4-R7's missing date, stale only in an engine job folder, never meets them; their docs, today "A `/movie/{id}` answer as TMDb documents it" and the like at `:522-523`, `:544-547` and `:579-581`, describe each as the slim saved answer of design part 8.4 that a job folder keeps, with its fields and its `fetched_at`, not as a TMDb API answer), `:716-778` (the three tests that pin those writers' exact bytes, `tmdb_movie_json_has_the_documented_movie_fields`, `tmdb_tv_json_has_the_documented_tv_fields_with_its_seasons_expanded` and `search_json_writes_movie_and_tv_results_in_their_own_fields`, gain `,"fetched_at":"2026-10-23"` before the outer object's final `}`).
- No edit: `crates/clients/src/saved.rs:157-165`'s `#[cfg(test)] status_word` matches on `Outcome`, whose `Pending { fetched: None, .. }` arm already gives `000` for a `Stale` reason.

**Interfaces:**
- Consumes: Task 2's `merge_pages`, `slim_search(pages, today)`; Task 3's `entry_search_title`; Task 4's `tmdb_season`, `splice_season`, `merge_parsed_season`; `core::lookup::name::fold_name`.
- Produces (exact; docs end `Def: T4-8. Use: T4-9, T4-10, T4-11.` unless stated):

```rust
// core::lookup::stale (design part 8.4, R-B4; Ruling T4-R7)
pub const STALE_MONTHS: u32 = 6;
pub const TMDB_COPIES: [&str; 3] = ["tmdb.json", "tmdb-record.json", "tmdb-search.json"];
pub fn add_months(date: &str, months: u32) -> Option<String>;
pub fn is_stale(fetched_at: Option<&str>, today: &str) -> bool;
pub fn fetched_at(text: &str) -> Option<String>;
// core::lookup (mod.rs; design parts 8.2 items 2 and 3, 8.4; J8, S47). Use: T4-9.
pub fn search_queries(inputs: &LookupInputs<'_>) -> Vec<String>;
// clients::lookups (design part 8.3, R24.6). Use: T4-9, T4-10.
pub enum PendingReason { NoToken, TokenRefused, Timeout, Unreachable(String), Status(u16), Unparsed(String), Stale(String) }
pub fn reason_text(reason: &PendingReason) -> String;
// clients::refetch (design part 8.4)
#[derive(Clone, Debug, PartialEq, Eq)] pub struct StaleCopy { pub file: &'static str, pub fetched_at: Option<String> }
pub fn stale_copies(status_text: &str, read: &dyn Fn(&str) -> Option<Vec<u8>>, job_folder: bool, today: &str) -> Vec<StaleCopy>;
pub fn status_record(status_text: &str) -> Option<TmdbIdentity>;
pub fn refetch_record(t: &dyn HttpTransport, token: Option<&str>, id: TmdbIdentity, season: Option<u32>, today: &str)
    -> Result<(TmdbRecord, Vec<SavedFile>), PendingReason>;
pub fn refetch_search(t: &dyn HttpTransport, token: Option<&str>, queries: &[String], today: &str)
    -> Result<(TmdbSearch, SavedFile), PendingReason>;
// clients::saved (crate-private): pub(crate) fn json_file(name: &str, value: &Value) -> SavedFile; // moved, unchanged
```

- [ ] **Step 1: Write the failing core tests.** In `stale.rs`'s `mod tests`:

```rust
#[test]
fn is_stale_reads_calendar_months_clamps_the_day_and_reads_a_future_date_as_fresh() {
    assert!(!is_stale(Some("2026-05-04"), "2026-11-04"));
    assert!(is_stale(Some("2026-05-03"), "2026-11-04"));
    assert!(is_stale(Some("2026-08-31"), "2027-03-01"));
    assert!(!is_stale(Some("2026-08-31"), "2027-02-28"));
    assert!(!is_stale(Some("2027-01-01"), "2026-11-04"), "a date after today is fresh");
    assert!(is_stale(None, "2026-11-04"));
    assert!(is_stale(Some("2026-13-01"), "2026-11-04"));
    for today in ["2026-11-4", "", "2026-02-30", "yesterday"] {
        assert!(!is_stale(None, today), "{today}");
        assert!(!is_stale(Some("2020-01-01"), today), "{today}");
    }
}

#[test]
fn add_months_clamps_to_the_last_day_and_leap_years() {
    assert_eq!(add_months("2027-08-31", 6).as_deref(), Some("2028-02-29"));
    assert_eq!(add_months("2026-08-31", 6).as_deref(), Some("2027-02-28"));
    assert_eq!(add_months("2099-08-31", 6).as_deref(), Some("2100-02-28"));
    assert_eq!(add_months("2026-11-04", 2).as_deref(), Some("2027-01-04"));
    assert_eq!(add_months("2026-01-31", 0).as_deref(), Some("2026-01-31"));
    for bad in ["2026-02-29", "2026-04-31", "2026-00-10", "2026-1-01", "26-01-01", "2026/01/01", "2026-01-01x"] {
        assert_eq!(add_months(bad, 6), None, "{bad}");
    }
    assert_eq!((STALE_MONTHS, TMDB_COPIES), (6, ["tmdb.json", "tmdb-record.json", "tmdb-search.json"]));
}

#[test]
fn fetched_at_reads_the_top_level_date_only() {
    assert_eq!(fetched_at(r#"{"id": 1, "fetched_at": "2026-01-01"}"#).as_deref(), Some("2026-01-01"));
    assert_eq!(fetched_at("\u{feff}{\"fetched_at\": \"2026-01-02\"}").as_deref(), Some("2026-01-02"));
    for text in [r#"{"results": [{"fetched_at": "2026-01-01"}]}"#, r#"{"fetched_at": 20260101}"#, "[]", "not json", ""] {
        assert_eq!(fetched_at(text), None, "{text}");
    }
}
```

In `crates/core/src/lookup/tests.rs` (helpers `label`, `without_tmdb_id` exist):

```rust
#[test]
fn search_queries_follow_the_entry_or_the_two_name_forms() {
    let piranha = label("PIRANHA_3D");
    assert_eq!(search_queries(&LookupInputs { name: Some(&piranha), ..LookupInputs::default() }), ["PIRANHA", "PIRANHA 3D"]);
    let plain = label("SYNTHETIC_MOVIE");
    assert_eq!(search_queries(&LookupInputs { name: Some(&plain), ..LookupInputs::default() }), ["SYNTHETIC MOVIE"]);
    let generic = label("DVD_VIDEO");
    assert!(search_queries(&LookupInputs { name: Some(&generic), ..LookupInputs::default() }).is_empty());
    assert!(search_queries(&LookupInputs::default()).is_empty());
    let knowing = without_tmdb_id("Knowing", 2009);
    let entry = LookupInputs { name: Some(&piranha), discdb_by_fingerprint: Some(&knowing), ..LookupInputs::default() };
    assert_eq!(search_queries(&entry), ["Knowing"], "S47: the entry's title alone (T4-R5)");
}
```

- [ ] **Step 2: Write the failing clients tests.** In `refetch.rs`'s `mod tests` (bodies inline, S69): `TOKEN = "token-sentinel-5d2e"`, `TODAY = "2026-10-23"`, `MOVIE = TmdbIdentity { kind: Movie, id: 900001 }`, `MOVIE_BODY` as `lookups.rs`'s test constant, `SHOW = TmdbIdentity { kind: Tv, id: 1437 }`, `SHOW_BODY` a `/tv/1437` answer listing seasons 0, 1 and 2, `season_body(n)` a `/tv/1437/season/<n>` answer with episodes 1 (`runtime` 44) and 2 (`runtime` 43), `page(ids)` a `/search/multi` page of movies with those IDs, `files(map)` a `read` closure over a `BTreeMap<&str, &str>`.

```rust
#[test]
fn a_copy_without_fetched_at_is_stale_only_in_a_job_folder() {
    let read = files(&[("tmdb.json", r#"{"original_language": "es"}"#)]);
    assert_eq!(stale_copies("", &read, false, TODAY), vec![]);
    assert_eq!(stale_copies("", &read, true, TODAY), vec![StaleCopy { file: "tmdb.json", fetched_at: None }]);
}

#[test]
fn stale_copies_lists_old_copies_and_a_deleted_one_only_in_a_job_folder() {
    let status = "dischash 404\nsearch 200\ntmdb movie/1 200\n";
    let read = files(&[("tmdb.json", r#"{"fetched_at": "2026-01-01"}"#), ("tmdb-search.json", r#"{"fetched_at": "2026-10-01"}"#)]);
    let old = StaleCopy { file: "tmdb.json", fetched_at: Some("2026-01-01".to_string()) };
    assert_eq!(stale_copies(status, &read, false, TODAY), vec![old.clone()]);
    assert_eq!(stale_copies(status, &read, true, TODAY), vec![old.clone(), StaleCopy { file: "tmdb-record.json", fetched_at: None }]);
    assert_eq!(stale_copies("tmdb movie/1 404\nsearch 000\n", &files(&[]), true, TODAY), vec![], "never answered: not deleted");
}

#[test]
fn status_record_reads_the_record_line() {
    assert_eq!(status_record("dischash 404\ntmdb tv/1437 200\n"), Some(SHOW));
    assert_eq!(status_record("tmdb movie/x 200\n"), None);
    assert_eq!(status_record("search 200\n"), None);
}

#[test]
fn refetch_record_writes_both_slim_files_with_todays_date() {
    let t = RecordedTransport::new();
    t.answer(&record_url(MOVIE), 200, MOVIE_BODY);
    let (record, saved) = refetch_record(&t, Some(TOKEN), MOVIE, None, TODAY).unwrap();
    assert_eq!(record.identity, MOVIE);
    let raw: Value = serde_json::from_slice(MOVIE_BODY).unwrap();
    assert_eq!(saved, vec![json_file("tmdb.json", &slim_tmdb(&raw, TODAY)),
                           json_file("tmdb-record.json", &slim_record(&raw, TmdbKind::Movie, TODAY))]);
}

#[test]
fn refetch_record_of_a_tv_show_fetches_its_seasons_again() {
    let t = RecordedTransport::new();
    t.answer(&record_url(SHOW), 200, SHOW_BODY);
    for n in [0, 1, 2] { t.answer(&season_url(1437, n), 200, season_body(n).as_bytes()); }
    let (record, saved) = refetch_record(&t, Some(TOKEN), SHOW, None, TODAY).unwrap();
    let urls: Vec<String> = t.requests().into_iter().map(|(url, _)| url).collect();
    assert_eq!(urls, [record_url(SHOW), season_url(1437, 0), season_url(1437, 1), season_url(1437, 2)], "every listed season (T4-R4)");
    assert_eq!(record.season(2).unwrap().episodes.len(), 2);
    let file: Value = serde_json::from_slice(&saved[1].bytes).unwrap();
    assert_eq!(file["seasons"][2]["episodes"], json!([{"episode_number": 1, "runtime": 44}, {"episode_number": 2, "runtime": 43}]));
    let parsed = RecordedTransport::new();
    parsed.answer(&record_url(SHOW), 200, SHOW_BODY);
    parsed.answer(&season_url(1437, 2), 200, season_body(2).as_bytes());
    refetch_record(&parsed, Some(TOKEN), SHOW, Some(2), TODAY).unwrap();
    assert_eq!(parsed.requests().len(), 2, "a parsed season asks that season only");
}

#[test]
fn refetch_search_runs_each_query_and_merges_them() {
    let t = RecordedTransport::new();
    t.answer(&search_url("PIRANHA"), 200, page(&[1, 2]).as_bytes());
    t.answer(&search_url("PIRANHA 3D"), 200, page(&[2, 3]).as_bytes());
    let queries = ["PIRANHA".to_string(), "PIRANHA 3D".to_string()];
    let (search, saved) = refetch_search(&t, Some(TOKEN), &queries, TODAY).unwrap();
    assert_eq!(search.results.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2, 3]);
    let file: Value = serde_json::from_slice(&saved.bytes).unwrap();
    assert_eq!((saved.name.as_str(), &file["queries"], &file["fetched_at"]), ("tmdb-search.json", &json!(queries), &json!(TODAY)));
    assert_eq!(refetch_search(&t, Some(TOKEN), &[], TODAY).unwrap_err(), PendingReason::Unparsed("no query to search".to_string()));
}

#[test]
fn a_refetch_without_a_token_asks_nothing_and_says_so() {
    let t = RecordedTransport::new();
    assert_eq!(refetch_record(&t, None, MOVIE, None, TODAY).unwrap_err(), PendingReason::NoToken);
    assert_eq!(refetch_search(&t, None, &["X".to_string()], TODAY).unwrap_err(), PendingReason::NoToken);
    assert!(t.requests().is_empty());
}

#[test]
fn a_refetch_answered_404_or_401_is_an_error_whose_text_holds_no_token() {
    let t = RecordedTransport::new();
    t.answer(&record_url(MOVIE), 404, b"");
    assert_eq!(refetch_record(&t, Some(TOKEN), MOVIE, None, TODAY).unwrap_err(), PendingReason::Status(404));
    t.answer(&search_url("X"), 401, b"{}");
    let refused = refetch_search(&t, Some(TOKEN), &["X".to_string()], TODAY).unwrap_err();
    assert_eq!(reason_text(&refused), "TMDb refused the token (HTTP 401)");
}
```

In `lookups.rs`'s tests and `chain.rs`'s tests:

```rust
#[test]
fn reason_text_keeps_r24_6s_texts() {
    let texts = [(PendingReason::NoToken, "no tmdb_token is set"), (PendingReason::TokenRefused, "TMDb refused the token (HTTP 401)"),
        (PendingReason::Timeout, "no answer within 10 s"), (PendingReason::Unreachable("down".into()), "down"),
        (PendingReason::Status(503), "HTTP 503"), (PendingReason::Unparsed("x: bad".into()), "x: bad"),
        (PendingReason::Stale("tmdb-record.json".into()), "tmdb-record.json is more than six months old or was deleted, and is fetched again")];
    for (reason, text) in texts { assert_eq!(reason_text(&reason), text); }
}

#[test]
fn a_stale_lookup_reads_000_in_the_status_file() {
    let mut book = LookupBook::default();
    book.states.insert(Lookup::Dischash, LookupState::Answered { status: 404 });
    book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::Stale("tmdb-search.json".into()) });
    assert_eq!(String::from_utf8(status_file(&book, &chain(None, None)).bytes).unwrap(), "dischash 404\nsearch 000\n");
}
```

- [ ] **Step 3: Run and see them fail.** Run: `scripts/box.sh cargo test -p lazymkv-core --locked stale`. Expected: compile error, no module `stale`. The other three Runs of Step 5 fail the same way (no `search_queries`, no `refetch`, no `reason_text`).

- [ ] **Step 4: Implement.**
  - `add_months`: a strict `yyyy-MM-dd` read (ten characters, digits and two dashes, month 1 to 12, day 1 to the month's last day, Gregorian leap years); month index `year * 12 + month - 1 + months`; the day clamped to the new month's last day; formatted `{:04}-{:02}-{:02}`. No date-time crate (design part 4.6).
  - `is_stale`: false when `add_months(today, 0)` is None; true when `fetched_at` is None or `add_months(fetched_at, STALE_MONTHS)` is None; else `today > limit` as strings.
  - `fetched_at`: the text without a leading byte order mark, parsed as JSON, the top-level object's `fetched_at` when it is a string.
  - `search_queries`: `[entry_search_title(inputs)]` when Some; else, for a name that is not generic, `[cleaned]` then `tagged` when `fold_name(tagged) != fold_name(cleaned)`; else empty.
  - `stale_copies`: for each of `TMDB_COPIES` in order, `read(file)`: present, its `fetched_at` (None when not UTF-8) is listed when `is_stale` holds and either `job_folder` is true or the date is Some; absent, listed with `fetched_at` None only when `job_folder` is true and the status text's line for it ends `200` (`tmdb.json` and `tmdb-record.json`: the `tmdb <kind>/<id>` line; `tmdb-search.json`: any `search`, `search-tagged` or `search-entry` line).
  - `status_record`: the first line whose first word is `tmdb`, its second word read as `<kind>/<id>`.
  - `refetch_record`: `tmdb_record`; `Answered` gives `json_file` of `slim_tmdb` then of `slim_record` (the raw body as JSON), then for a tv record the seasons of Ruling T4-R4 through `tmdb_season`, each answer spliced with `splice_season` and merged with `merge_parsed_season` (a season 404 is a miss and is skipped); a record `Miss` is `Err(Status(404))`; a `Pending` reason (the record's or a season's) is the Err.
  - `refetch_search`: `tmdb_search` per query in order; every query `Answered`, the pages fold through `merge_pages` and the file is `json_file("tmdb-search.json", &slim_search(&pages, today))`; a `Miss` is `Err(Status(404))`; a pending reason is the Err.
  - `reason_text`: R24.6's six arms unchanged, plus `Stale(file)` giving `<file> is more than six months old or was deleted, and is fetched again`.
  - core's test writers (`testsupport/mod.rs`): `tmdb_movie_json`, `tmdb_tv_json` and `search_json` add `"fetched_at": "2026-10-23"` as their object's last key, and the three tests that pin their bytes are adjusted to match (Files above). The existing restart unit tests (`restart.rs:463-485`) and Tasks 2 and 3's tests that write `search_json` pages then keep their meaning once Task 9 gives `rebuild_book` the date `"2026-10-23"`, except `a_200_whose_saved_file_does_not_parse_is_pending_and_no_line_is_not_asked` (`restart.rs:479-486`), which Task 9 renames and re-pins (its saved `"{not json"` has no readable `fetched_at`, so it is stale in a job folder).

- [ ] **Step 5: Run and see them pass.** Run: `scripts/box.sh cargo test -p lazymkv-core --locked stale`, `scripts/box.sh cargo test -p lazymkv-core --locked search_queries`, `scripts/box.sh cargo test -p lazymkv-clients --locked refetch`, `scripts/box.sh cargo test -p lazymkv-clients --locked reason_text`, then `scripts/box.sh cargo test -p lazymkv-clients --locked stale_lookup`. Expected: PASS each. Then `scripts/box.sh cargo test --workspace --locked` (engine's `the_waiting_message_names_each_pending_lookup_and_its_reason` unchanged): PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/core/src/lookup/stale.rs crates/core/src/lookup/mod.rs crates/core/src/lookup/tests.rs crates/core/src/testsupport/mod.rs \
  crates/clients/src/refetch.rs crates/clients/src/lookups.rs crates/clients/src/chain.rs crates/clients/src/saved.rs \
  crates/clients/src/lib.rs crates/engine/src/file_phase.rs
git commit -m "core, clients: the six-month rule and the refetch of a stale copy"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 9.

---

### Task 9: The engine fetches stale copies again (design parts 8.4, 9.5, 5.10; K15; T4-R8, T4-R9)

**Files:**
- Modify: `crates/engine/src/file_phase.rs` (`refetch_stale`, `stale_sentence`; two unit tests).
- Modify: `crates/engine/src/restart.rs:169-250` (`rebuild_book(job_dir, today)`; T4-R8; one unit test; the existing tests' calls gain `"2026-10-23"`; `:479-486`'s `a_200_whose_saved_file_does_not_parse_is_pending_and_no_line_is_not_asked` becomes `a_200_whose_saved_file_does_not_parse_is_stale_and_fetched_again_and_no_line_is_not_asked`, its first assertion re-pinned to `Pending { Stale(..) }`, Step 1), `crates/engine/src/owner.rs:950` (`rebuild_book(&job_dir, &parts.host.local_date())`; then, for each distinct file of the rebuilt book's `Pending { Stale(file) }` states, the engine.log line `<id>: <reason_text(&PendingReason::Stale(file))>`, that is `<id>: <file> is more than six months old or was deleted, and is fetched again`).
- Modify: `crates/engine/src/steps.rs:907-935` (Plan), `:1545-1548` (Decide).
- Modify: `crates/engine/src/relay.rs:107-117` (`RunningCommand.kill` becomes `Option<KillHandle>`), new `command_job`, `CommandRefetch`, `spawn_command_refetch`, `STALE_EXIT`; `crates/engine/src/owner.rs:199` (`Msg::CommandRefetched`), `:1210-1213`, `:1243`, `:2471-2505` (`command`), `:2508-2520` (`kill_command_of`, `kill_command`).
- Modify: `crates/platform/src/fakes.rs:408-432` (`MemorySecrets::forget`, the test seam a no-token engine needs: every harness starts with `tmdb_token`, `testkit.rs:1416`).
- Modify: `crates/fake-tools/tests/engine_restart.rs`, `engine_file_phase.rs`, `engine_command.rs` (new tests; `:124-133`'s `a_missing_replay_file_adds_no_flag` becomes `a_missing_tmdb_copy_is_fetched_again_before_the_words` and `a_missing_replay_file_outside_the_lookups_adds_no_flag`, Step 2).

**Interfaces:**
- Consumes: Task 8's `stale_copies`, `status_record`, `refetch_record`, `refetch_search`, `search_queries`, `reason_text`, `PendingReason::Stale`, `TMDB_COPIES`; Tasks 2 to 4's `Lookup` variants and `LookupBook.record_file`.
- Produces (crate-private; docs end `Def: T4-9. Use: T4-9.`):

```rust
// file_phase.rs (design parts 8.4, 5.10, 9.5; Rulings T4-R8, T4-R9)
pub(crate) fn refetch_stale(job_dir: &Path, plan: Option<&Plan>, lookups: &Mutex<LookupBook>, http: &dyn HttpTransport,
    token: Option<&str>, today: &str) -> Result<Vec<String>, String>; // Ok: the stale copies fetched again (the
    // listed ones only, though a record fetch rewrites both tmdb.json and tmdb-record.json), in TMDB_COPIES order
pub(crate) fn stale_sentence(job_dir: &Path, copy: &StaleCopy, reason: &PendingReason) -> String;
// restart.rs
pub(crate) fn rebuild_book(job_dir: &Path, today: &str) -> LookupBook;
// relay.rs (design part 9.5; Ruling T4-R9)
pub(crate) const STALE_EXIT: i32 = 3;
pub(crate) fn command_job(words: &[String]) -> Option<String>; // read_forms' job for words child_words accepted
pub(crate) struct CommandRefetch { pub(crate) job_dir: PathBuf, pub(crate) plan: Option<Plan>,
    pub(crate) lookups: Arc<Mutex<LookupBook>>, pub(crate) http: Arc<dyn HttpTransport>, pub(crate) token: Option<String>,
    pub(crate) today: String, pub(crate) store: Arc<Store> }
pub(crate) fn spawn_command_refetch(work: CommandRefetch, client: u64, id: Option<u64>, words: Vec<String>,
    owner: Sender<Msg>) -> JoinHandle<()>;
pub(crate) struct RunningCommand { pub(crate) client: u64, pub(crate) id: Option<u64>, pub(crate) kill: Option<KillHandle> }
// owner.rs
Msg::CommandRefetched { client: u64, id: Option<u64>, words: Vec<String>, refetched: Result<(), String> }
// platform fakes.rs (public test double; doc ends "Def: T4-9. Use: T4-9.")
impl MemorySecrets { pub fn forget(&self, name: &str); }
```

The three sentences of `stale_sentence` (exact): `<file>: the TMDb copy is more than six months old (fetched_at <date>) and could not be fetched again: <reason_text>`; with `fetched_at` None and the file present, `<file>: the TMDb copy has no fetched_at date and could not be fetched again: <reason_text>`; with the file absent, `<file>: the TMDb copy was deleted and could not be fetched again: <reason_text>`. `refetch_stale`'s Err joins one per copy not fetched again with `"; "`. Each file fetched again gives the job.log line `the TMDb copy <file> was fetched again`, written by the caller.

- [ ] **Step 1: Write the failing unit tests.** In `file_phase.rs`'s `mod tests`, `stale_folder(copies)` is a `Scratch` job folder holding `lookup-status.txt` `"dischash 404\nsearch 200\ntmdb movie/900001 200\n"`, a slim `tmdb-search.json` dated `2026-10-01`, and each `(file, fetched_at)` of `copies` as a slim copy of movie 900001 with that date; `lock` is `PoisonError::into_inner` over `Mutex::lock`. In `restart.rs`'s, `stale_job_folder()` is the same folder with a `meta.json` (fingerprint, no disc ID, volume label `SYNTHETIC_MOVIE`), a slim `tmdb.json` dated `2026-10-01` (fresh, so the record's first listed stale file is `tmdb-record.json`) and `tmdb-record.json` dated `2026-01-01`. The existing restart unit tests' `rebuild_book` calls gain `"2026-10-23"`; their `search_json` pages carry `"fetched_at": "2026-10-23"` since Task 8 (core's test writers), so they are fresh at that date and keep their meaning (Ruling T4-R7: a missing `fetched_at` is stale only in an engine job folder), with one exception. `a_200_whose_saved_file_does_not_parse_is_pending_and_no_line_is_not_asked` (`restart.rs:479-486`) saves `"{not json"` as `tmdb-search.json` under a `search 200` line; its `fetched_at` cannot be read, so under Ruling T4-R7 `rebuild_book` (a job folder, `stale_copies(.., true, ..)`) reads it as `Pending { Stale("tmdb-search.json") }` and the search is fetched again. The test is renamed `a_200_whose_saved_file_does_not_parse_is_stale_and_fetched_again_and_no_line_is_not_asked` and its first assertion becomes `assert!(matches!(book.states[&Lookup::Search], LookupState::Pending { reason: PendingReason::Stale(_) }));`; its "no line is not asked" assertion on `Lookup::Dischash` stays as it is.

```rust
#[test]
fn refetch_stale_of_fresh_copies_asks_nothing() {
    let s = stale_folder(&[("tmdb.json", "2026-10-01"), ("tmdb-record.json", "2026-10-01")]);
    let t = RecordedTransport::new();
    assert_eq!(refetch_stale(&s.0, None, &Mutex::default(), &t, None, "2026-10-23"), Ok(vec![]));
    assert!(t.requests().is_empty());
}

#[test]
fn refetch_stale_names_each_copy_it_could_not_fetch() {
    let s = stale_folder(&[("tmdb.json", "2026-01-01")]); // tmdb-record.json deleted
    let (t, book) = (RecordedTransport::new(), Mutex::new(LookupBook::default()));
    lock(&book).states.insert(Lookup::Record, LookupState::Answered { status: 200 });
    let error = refetch_stale(&s.0, None, &book, &t, None, "2026-10-23").unwrap_err();
    assert_eq!(error, [
        "tmdb.json: the TMDb copy is more than six months old (fetched_at 2026-01-01) and could not be fetched again: no tmdb_token is set",
        "tmdb-record.json: the TMDb copy was deleted and could not be fetched again: no tmdb_token is set",
    ].join("; "));
    assert_eq!(lock(&book).states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Stale("tmdb.json".into()) });
    assert!(t.requests().is_empty());
}
```

In `restart.rs`'s `mod tests`:

```rust
#[test]
fn a_stale_copy_rebuilds_as_pending_stale_and_a_fresh_one_as_answered() {
    // tmdb-record.json fetched_at 2026-01-01, tmdb-search.json fetched_at 2026-10-01, both slim and parseable.
    let book = rebuild_book(&stale_job_folder().0, "2026-10-23");
    assert_eq!(book.states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Stale("tmdb-record.json".into()) });
    assert_eq!(book.answers.record, None);
    assert_eq!(book.states[&Lookup::Search], LookupState::Answered { status: 200 });
}
```

- [ ] **Step 2: Write the failing engine tests.** `stale(h, id, file)` rewrites `<job>/<file>` with its `fetched_at` set to `2026-01-01` (the harness date is `2026-10-23`); `no_token(h)` is `h.secrets().forget("tmdb_token")`. Each file gains only the helpers and imports it uses: `engine_restart.rs` gains `stale`; `engine_file_phase.rs` gains `stale` and `no_token` and imports `lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind}` and `lazymkv_platform::SecretStore` (beside `Host, Reach`; its `h.secrets().set(..)` is the trait's); `engine_command.rs` gains `stale`, `no_token` and `engine_file_phase.rs`'s `json_of`, and imports `lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind}`. Each message assertion also asserts the message lacks `test-token`.

```rust
// engine_restart.rs
#[test]
fn a_restart_reads_a_stale_copy_as_pending_and_asks_nothing() {
    let h = harness(&[DISC, "movie-direct-probe-error"], settings()); // Failed: no step runs at the restart
    h.insert("tok0", DISC);
    h.run_until("Failed");
    let id = only_job(&h);
    stale(&h, &id, "tmdb-record.json");
    let before = h.http().requests().len();
    h.restart();
    h.settle();
    assert_eq!(h.http().requests().len(), before, "the engine's start sends no request (design part 4.4)");
    let log = h.engine_log();
    assert!(log.contains(&format!("{id}: tmdb-record.json is more than six months old or was deleted, and is fetched again")), "{log}");
}

// engine_file_phase.rs
#[test]
fn a_first_decide_with_a_stale_copy_it_cannot_fetch_waits_for_its_lookup() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    h.restart();
    h.wait("AwaitingLookup", |h| h.state_of(&id) == "AwaitingLookup");
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("tmdb-record.json: the TMDb copy is more than six months old (fetched_at 2026-01-01) and could \
        not be fetched again: no tmdb_token is set"), "{message}");
    assert!(events(&h, &id).contains(&Event::LookupPending));
}

#[test]
fn a_decide_in_decided_after_a_restart_with_a_stale_copy_it_cannot_fetch_fails_unknown() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Decided");
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    h.restart(); // resume(Decided) runs Decide (core job.rs:741)
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::DecisionFailed { failure } if failure.class == FailureClass::Unknown)));
    assert!(h.record(&id).message.unwrap().contains("tmdb-record.json: the TMDb copy is more than six months old"));
}

#[test]
fn a_plan_again_from_scanned_with_a_deleted_copy_fetches_it_and_plans() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Scanned");
    fs::remove_file(h.job_dir(&id).join("tmdb-record.json")).unwrap();
    let movie = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
    let record_asks = |h: &Harness| h.http().requests().iter().filter(|(url, _)| *url == lazymkv_clients::record_url(movie)).count();
    let before = record_asks(&h);
    h.restart();
    h.wait("Planned", |h| h.transitions(&id).iter().any(|l| l.event.as_ref().is_some_and(|e| matches!(e, Event::PlanDone { .. }))));
    assert_eq!(record_asks(&h), before + 1, "the deleted copy is fetched again, once");
    assert_eq!(json_of(&h.job_dir(&id).join("tmdb-record.json"))["fetched_at"], json!("2026-10-23"));
    let plan = h.record(&id).plan.unwrap();
    assert_eq!((plan.identity.tmdb, plan.identity.lookup_pending), (Some(movie), false));
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains("the TMDb copy tmdb-record.json was fetched again"), "{log}");
    assert!(!log.contains("the TMDb copy tmdb.json was fetched again"), "only listed copies are named: {log}");
}

#[test]
fn a_plan_again_from_scanned_with_a_stale_copy_it_cannot_fetch_fails_unknown() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Scanned");
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::PlanFailed { failure } if failure.class == FailureClass::Unknown)));
    assert_eq!(h.record(&id).message.as_deref(), Some("tmdb-record.json: the TMDb copy is more than six months old \
        (fetched_at 2026-01-01) and could not be fetched again: no tmdb_token is set"));
    h.secrets().set("tmdb_token", "test-token").unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("retry", json!({ "job": id }));
    h.wait("Planned", |h| h.transitions(&id).iter().any(|l| l.event.as_ref().is_some_and(|e| matches!(e, Event::PlanDone { .. }))));
}

#[test]
fn a_stale_copy_fetched_again_replaces_the_file_and_the_decision_uses_it() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    let path = h.job_dir(&id).join("tmdb-record.json");
    let mut copy = json_of(&path);
    copy["title"] = json!("Stale Title");
    copy["fetched_at"] = json!("2026-01-01");
    fs::write(&path, serde_json::to_vec_pretty(&copy).unwrap()).unwrap();
    h.restart();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let fresh = json_of(&path);
    assert_eq!((&fresh["title"], &fresh["fetched_at"]), (&json!("Synthetic Movie"), &json!("2026-10-23")));
    assert!(!fs::read_to_string(h.job_dir(&id).join(KEY).join("decide.json")).unwrap().contains("Stale Title"));
    assert!(final_path(&h).is_file());
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("the TMDb copy tmdb-record.json was fetched again"));
}

#[test]
fn the_engine_never_reads_tmdb_token_from_its_environment() {
    let h = harness(&[], json!({}));
    no_token(&h);
    std::env::set_var("TMDB_TOKEN", "token-sentinel-5d2e");
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    std::env::remove_var("TMDB_TOKEN");
    let requests = h.http().requests();
    assert!(requests.iter().all(|(url, _)| !url.starts_with(lazymkv_clients::TMDB)), "{requests:?}");
    assert!(requests.iter().flat_map(|(_, headers)| headers).all(|(_, value)| !value.contains("token-sentinel-5d2e")));
    assert!(h.record(&id).message.unwrap().contains("search: no tmdb_token is set"));
}

// engine_command.rs (filed() as today: a Filed movie-direct job after a restart)
#[test]
fn a_relayed_plan_with_a_stale_copy_it_cannot_fetch_exits_3_and_leaves_the_job_alone() {
    let (h, id) = filed();
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    let mut c = client(&h);
    let (outputs, reply) = c.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(3, 1));
    assert_eq!(outputs, [pair("stderr", "lazymkvctl: tmdb-record.json: the TMDb copy is more than six months old \
        (fetched_at 2026-01-01) and could not be fetched again: no tmdb_token is set")]);
    assert!(ctl_spawns(&h).is_empty());
    assert_eq!(h.state_of(&id), "Filed");
}
```

`a_relayed_decide_with_a_stale_copy_it_cannot_fetch_exits_3_and_leaves_the_job_alone` is the same with `decide --job <id> --title t00-00800`. And:

```rust
#[test]
fn the_relays_refetch_runs_off_the_owner_thread() {
    let (h, id) = filed();
    stale(&h, &id, "tmdb-record.json");
    let url = lazymkv_clients::record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 900001 });
    let record_asks = |h: &Harness| h.http().requests().iter().filter(|(u, _)| *u == url).count();
    let before = record_asks(&h);
    let gate = h.http().gate(&url);
    let mut a = client(&h);
    let asked = a.send("command", json!({ "words": ["plan", "--job", &id] }));
    h.wait("the refetch's request", |h| record_asks(h) == before + 1);
    let mut b = client(&h); // its snapshot arrives while the refetch waits
    b.ok("history", json!({}));
    assert!(ctl_spawns(&h).is_empty(), "the child waits for the refetch");
    gate.open();
    assert_eq!(a.reply_to(asked).1, exit(0, asked));
    assert_eq!(json_of(&h.job_dir(&id).join("tmdb-record.json"))["fetched_at"], json!("2026-10-23"));
}
```

`a_missing_replay_file_adds_no_flag` (`engine_command.rs:124-133`) changes meaning under Ruling T4-R9: a deleted TMDb copy whose status line says `200` is fetched again before the relay's words. It is replaced by two cases:

```rust
#[test]
fn a_missing_tmdb_copy_is_fetched_again_before_the_words() {
    let (h, id) = filed();
    let job = h.job_dir(&id);
    replay_files(&job, &["settings.json", "tmdb-search.json"]); // tmdb-record.json deleted; its status line says 200
    let url = lazymkv_clients::record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 900001 });
    let record_asks = |h: &Harness| h.http().requests().iter().filter(|(u, _)| *u == url).count();
    let before = record_asks(&h);
    let mut c = client(&h);
    let (_, reply) = c.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 1));
    assert!(job.join("tmdb-record.json").is_file(), "the record file is back");
    assert_eq!(record_asks(&h), before + 1);
    let given = ["settings.json", "tmdb-search.json", "tmdb-record.json"];
    assert_eq!(ctl_spawns(&h).pop().unwrap().args, built(&["plan".to_string(), text(job.clone())], &job, &given));
}

#[test]
fn a_missing_replay_file_outside_the_lookups_adds_no_flag() {
    let (h, id) = filed();
    let job = h.job_dir(&id);
    let given = ["settings.json", "tmdb-search.json", "tmdb-record.json"]; // remembered.json deleted: no refetch restores it
    replay_files(&job, &given);
    let mut c = client(&h);
    let (_, reply) = c.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 1));
    assert!(!job.join("remembered.json").exists());
    assert_eq!(ctl_spawns(&h).pop().unwrap().args, built(&["plan".to_string(), text(job.clone())], &job, &given));
}
```

- [ ] **Step 3: Run and see them fail.** Run: `scripts/box.sh cargo test -p lazymkv-engine --locked refetch_stale`. Expected: compile error, no `refetch_stale` (and no `MemorySecrets::forget` for the fake-tools tests).

- [ ] **Step 4: Implement.**
  - `MemorySecrets::forget(&self, name)`: removes `name`; doc: "A test double's removal, as the owner deleting a Credential Manager entry by hand; the engine never removes a secret (design part 4.2)."
  - `refetch_stale` (an algorithm the signature leaves open):
    1. `status` is `lookup-status.txt` (empty when absent); `copies = stale_copies(&status, &|f| fs::read(job_dir.join(f)).ok(), true, today)`; none gives `Ok(vec![])` and no request.
    2. When `tmdb.json` or `tmdb-record.json` is listed: the identity is `plan.and_then(|p| p.identity.tmdb)`, else `status_record(&status)` (none: each such copy fails with `Unparsed("no TMDb ID names the record")`); the season is `chain_inputs(job_dir)`'s `name.season` when `chain_inputs` succeeds, else None (a `Scratch` folder without `meta.json` has no name to parse; the unit tests need no `meta.json`); `refetch_record` once for both files.
    3. When `tmdb-search.json` is listed: the queries are the file's `queries` strings when it holds any, else `search_queries(&book.lookup_inputs(name))` over the locked book (T4-R9); `refetch_search`.
    4. Each fetched file is written with `write_atomic` (a failed write is that copy's failure, `<path>: the TMDb copy could not be written: <error>`). The book is locked only to install an answer, never across a request (the owner thread locks it for `waiting_message`): `answers.record` and `record_file` (the new `tmdb-record.json`), or the page into `answers.entry` when the status holds `search-entry 200`, else `answers.search`; every state of that file's lookups (`Record` and each `Season(_)` for the record's files; `Search`, `TaggedSearch` and `EntrySearch` for the search file) that is `Pending { Stale(_) }` becomes `Answered { status: 200 }`.
    5. Each copy that failed sets its file's lookups present in the book to `Pending { Stale(<first failed file of that group>) }`, so the next round asks them again; the Err is the `stale_sentence`s joined with `"; "`.
  - `rebuild_book(job_dir, today)`: `stale_copies(&text, .., true, today)` is read once; a `200` line whose lookup's file (`tmdb.json` or `tmdb-record.json` for `Record` and `Season(_)`, `tmdb-search.json` for the three searches) is listed becomes `Pending { Stale(<first listed file of it>) }` and its answer is not read (T4-R8). `Owner::load` (`owner.rs:950`) then writes, once per distinct file of those `Stale` reasons, the engine.log line `<id>: <reason_text(&PendingReason::Stale(file))>` (`<id>: <file> is more than six months old or was deleted, and is fetched again`), the observable `a_restart_reads_a_stale_copy_as_pending_and_asks_nothing` reads.
  - Plan (`steps.rs:907`): right after `chain_inputs`, unless `self.job.lookups.try_lock()` is `WouldBlock` (a running round is fetching fresh copies, and Plan then counts the book empty, R23.1), `refetch_stale(&job_dir, record.plan.as_ref(), &job.lookups, http, secrets.get("tmdb_token").as_deref(), &host.local_date())`; Err feeds `PlanFailed { Unknown }` with the reason; each Ok file writes its job.log line. A job's first Plan finds its copies fetched that day, so it asks nothing.
  - Decide (`steps.rs:1547`): right after `job_plan`, the same call with `Some(&plan)`; Err in `Inspecting` gives `with(Event::LookupPending, reason)` (the reducer takes it in Inspecting, core `job.rs:1075-1077`), in `Decided` `unknown(failed, reason)`.
  - The relay: `command` (`owner.rs:2471`) keeps its refusals in Ruling T3-22-b's order and calls `child_words` to check the words; on Ok it sets `self.command = Some(RunningCommand { client, id, kill: None })` and pushes `spawn_command_refetch` over `command_job`'s job (`plan` cloned, `Arc::clone(&job.lookups)`, `parts.http`, `parts.secrets.get("tmdb_token")` as the owned `CommandRefetch.token`, `parts.host.local_date()`, the store) onto `self.threads`; the thread passes `work.token.as_deref()`. The thread runs `refetch_stale`, writes each job.log line through `store.job_log`, and sends `Msg::CommandRefetched`. The owner drops one that no longer matches `self.command` (its client left: `kill_command_of` becomes `&mut self`, and with `kill` None it clears `self.command`, else it kills as today; a stop answered it). On Err it sends the client `push_line(&Push::CommandOutput { stream: Stderr, line: format!("lazymkvctl: {reason}") }, id)`, clears `self.command` and replies `Ok(json!({ "exit_code": STALE_EXIT }))`, with no child and no job transition (K15). On Ok it calls `child_words` again (the flags of design part 9.5 follow the files as they now stand; a refusal then is its reply) and spawns as today, setting `kill`. `quiet_message` drops `CommandRefetched`.

- [ ] **Step 5: Run and see them pass.** Run, one at a time: `scripts/box.sh cargo test -p lazymkv-engine --locked refetch_stale`, `scripts/box.sh cargo test -p lazymkv-engine --locked rebuilds_as_pending_stale`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_restart stale`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase stale`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase deleted_copy`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase tmdb_token`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_command stale`, `scripts/box.sh cargo test -p fake-tools --locked --test engine_command owner_thread`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_command a_missing_`. Expected: PASS each (`a_missing_`: 3, the two cases that replace `a_missing_replay_file_adds_no_flag` and the existing `a_missing_lazymkvctl_is_refused_as_not_started`, `engine_command.rs:284`). Then `scripts/box.sh cargo test --workspace --locked` (the relay's other existing tests, `COMMAND_BUSY` included, unchanged): PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/engine/src/file_phase.rs crates/engine/src/restart.rs crates/engine/src/steps.rs crates/engine/src/relay.rs \
  crates/engine/src/owner.rs crates/platform/src/fakes.rs crates/fake-tools/tests/engine_restart.rs \
  crates/fake-tools/tests/engine_file_phase.rs crates/fake-tools/tests/engine_command.rs
git commit -m "engine: stale TMDb copies fetched again before Plan, Decide and the relay (K15)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 10.

---

### Task 10: dev's local refetch and exit 3 (design part 8.4, K15, T4-R10)

Precondition: `decisions.txt` holds item 23's answer (Task 0 Step 1).

**Files:**
- Modify: `crates/dev/src/evidence.rs:20-52` (`Ctx.http`, None from `from_process` and `for_test`; the field doc of `env` names `TMDB_TOKEN`), after `:52` (`with_live_transport`).
- Modify: `crates/dev/src/main.rs:7`, `crates/ctl/src/main.rs:7` (`Ctx::from_process().with_live_transport()`).
- Create: `crates/dev/src/stale.rs` (`refresh_copies`, the two exit-3 texts; its `mod tests` holds the dev tests below); `crates/dev/src/lib.rs:5-9` (`pub mod stale;`).
- Modify: `crates/dev/src/cmd/plan.rs:1-17, :566-569` (module doc and `HELP`'s exit line name exit 3 for a stale copy), `:607-631` (the gate after `resolve_recording`; none for the scan-file form).
- Modify: `crates/dev/src/cmd/decide.rs:1-14, :108-110` (docs), `:653-675` (the gate after `(rec, label)` is known, before `read_decide_inputs`; `read_decide_inputs` itself, which the engine calls, gets no gate).
- Modify: `crates/dev/src/cmd/retune.rs:1-12` (doc), between `:102-105` (`resolve_evidence`) and `:118-121` (`DiskTree::open`), and the matching place in `run_subtitles` (the gate over each recording folder after `resolve_evidence`, before `DiskTree::open`).
- Modify: `crates/ctl/src/tests.rs` (one test). The fixtures without `fetched_at` (`retune/tests.rs:863-1004`, `tests/retune_subtitles.rs:143`) stay unchanged: outside an engine job folder a copy without `fetched_at` is read as it is (T4-R7).

**Interfaces:**
- Consumes: Task 8's `lazymkv_core::lookup::stale::fetched_at`, `lazymkv_clients::{stale_copies, status_record, refetch_record, refetch_search, reason_text, StaleCopy, OfflineTransport, HttpTransport}`; Task 7's `UreqTransport::new()` (in `with_live_transport` only); Task 1's `dev -> clients` edge.
- Produces:

```rust
// crates/dev/src/evidence.rs
pub struct Ctx {
    // PR 3 fields unchanged; `env`'s doc gains TMDB_TOKEN (design part 8.4)
    /// The transport a local plan, decide or retune fetches a stale TMDb copy with (design part 8.4; Ruling T4-R10):
    /// None in `from_process` and `for_test`, which asks through `OfflineTransport`; `with_live_transport` sets the
    /// live one. The engine's in-process use of dev's library never reaches it. Def: T4-10. Use: T4-10.
    pub http: Option<Arc<dyn HttpTransport>>,
}
impl Ctx {
    /// `self` with `http` set to `UreqTransport::new()` (design parts 8.1, 8.4; Ruling T4-R10). Called only by
    /// lazymkv-dev's and lazymkvctl's `main`. Def: T4-10. Use: T4-10.
    pub fn with_live_transport(self) -> Ctx;
}

// crates/dev/src/stale.rs
/// The variable a local run reads the TMDb token from (design part 8.4, part 15 item 8). Def: T4-10. Use: T4-10.
pub const TMDB_TOKEN_VAR: &str = "TMDB_TOKEN";
/// The file that marks an engine job folder (design part 7.1), which dev reads as it is. Def: T4-10. Use: T4-10.
pub const JOB_STATE_FILE: &str = "state.json";
/// Fetches again each TMDb copy of `folder` that is stale (design part 8.4, R-B4, K15; Rulings T4-R7, T4-R10) and
/// replaces it, giving the names it replaced in `TMDB_COPIES` order; Ok(vec![]) for a folder holding
/// `JOB_STATE_FILE` and for one with nothing stale, asking nothing. Err is `Refused { code: 3, line }` naming
/// `<shown>/<file>`; no text holds the token. Def: T4-10. Use: T4-10.
pub fn refresh_copies(ctx: &Ctx, folder: &Path, shown: &str) -> Result<Vec<String>, Refused>;
/// `<shown>/<file>: the TMDb copy is more than six months old (fetched_at <date>) and TMDB_TOKEN is not set to fetch
/// it again`; `(no fetched_at)` replaces `(fetched_at <date>)` when the copy has none. Def: T4-10. Use: T4-10.
pub fn no_token_line(shown: &str, copy: &StaleCopy) -> String;
/// `<shown>/<file>: the TMDb copy is more than six months old (fetched_at <date>) and could not be fetched again:
/// <why>`, `<why>` being `reason_text` of the fetch's reason or a write's error. Def: T4-10. Use: T4-10.
pub fn failed_line(shown: &str, copy: &StaleCopy, why: &str) -> String;
/// The stderr line of a copy fetched again: `lazymkv-dev <command>: <shown>/<file> was more than six months old and
/// was fetched again`. Def: T4-10. Use: T4-10.
pub fn fetched_line(command: &str, shown: &str, file: &str) -> String;
```

**Tests (in `crates/dev/src/stale.rs`'s `mod tests`, each with `Ctx::for_test`, whose day is 2026-10-13, and a `RecordedTransport` in `ctx.http`; one in `crates/ctl/src/tests.rs`):**

- [ ] **Step 1: Write the failing tests**

```rust
const TOKEN: &str = "token-sentinel-5d2e";
/// Ctx::for_test's day is 2026-10-13: 2026-01-01 plus six months is 2026-07-01, before it; 2026-10-05 is fresh.
const STALE: &str = "2026-01-01";
const FRESH: &str = "2026-10-05";
const STATUS: &str = "\u{feff}dischash 404\r\ndiscid 404\r\ntmdb movie/6537 200\r\n";
const RECORD_ANSWER: &str = r#"{"id": 6537, "title": "Made Up Film", "original_title": "Pelicula Inventada",
 "release_date": "2007-05-20", "original_language": "es", "runtime": 100, "external_ids": {"imdb_id": "tt0006537"}}"#;
const SPEC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/superpowers/specs/2026-10-04-lazymkv-design.md");
/// lookup-status.txt of a disc TheDiscDB knows, as `crates/dev/src/cmd/plan.rs`'s tests with a TheDiscDB hit write it.
const KNOWN_STATUS: &str = "\u{feff}dischash 200\r\ndiscid 404\r\ntmdb movie/6537 200\r\n";
/// TheDiscDB's `dischash` answer naming movie 6537, the minimal form of plan.rs's `DISCHASH`: a Movie whose
/// `media.external_ids.tmdb` gives the plan its TMDb identity (core `lookup/mod.rs:836`), and one MainMovie title.
const DISCHASH: &str = r#"{"content_hash": "0123456789ABCDEF0123456789ABCDEF", "format": "Blu-ray",
 "media": {"title": "Made Up Film", "year": 2007, "type": "Movie", "external_ids": {"tmdb": "6537"}},
 "titles": [{"index": 0, "item": {"title": "Made Up Film", "type": "MainMovie"}}]}"#;

fn copy(fetched: &str) -> String {
    format!(r#"{{"id": 6537, "title": "Made Up Film", "original_language": "es", "fetched_at": "{fetched}"}}"#)
}
fn movie() -> String { record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 6537 }) }
fn line(file: &str, rest: &str) -> String {
    format!("recs/film/{file}: the TMDb copy is more than six months old (fetched_at 2026-01-01) and {rest}")
}

/// recs/film: the scan and the eng-only label files exactly as `recording` in crates/ctl/src/tests.rs:24-46 writes
/// recs/disc-a, plus meta.json `{"volume_label": "FILM", "format": "bluray", "fingerprint":
/// "0123456789ABCDEF0123456789ABCDEF", "disc_id": null, "file_count": 3, "total_bytes": 1000, "captured_unix":
/// 1790000000}`, lookup-status.txt `STATUS` and tmdb.json `tmdb`.
fn film(s: &Scratch, tmdb: &str) { /* as the doc says */ }

/// `film`, then thediscdb-hash.json `DISCHASH` and lookup-status.txt `KNOWN_STATUS`, so the plan has TMDb identity
/// movie 6537 and reads tmdb.json's record for it.
fn known_film(s: &Scratch, tmdb: &str) {
    film(s, tmdb);
    s.write("recs/film/thediscdb-hash.json", DISCHASH);
    s.write("recs/film/lookup-status.txt", KNOWN_STATUS);
}

/// (exit code, stdout, stderr) of lazymkv-dev's dispatcher.
fn run(ctx: &Ctx, argv: &[&str]) -> (i32, String, String) { /* crate::dispatch over two Vec<u8> */ }

fn ctx_with(s: &Scratch, http: &Arc<RecordedTransport>, token: bool) -> Ctx {
    let mut ctx = Ctx::for_test(&s.0);
    ctx.http = Some(Arc::clone(http) as Arc<dyn HttpTransport>);
    if token { ctx.env.insert(TMDB_TOKEN_VAR.to_string(), TOKEN.to_string()); }
    ctx
}

#[test]
fn plan_over_a_folder_with_a_stale_copy_and_no_token_exits_3_naming_it() {
    let s = Scratch::new("stale-plan-no-token");
    film(&s, &copy(STALE));
    let http = Arc::new(RecordedTransport::new());
    let (code, out, err) = run(&ctx_with(&s, &http, false), &["plan", "recs/film"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, format!("lazymkv-dev plan: error: {}\n", line("tmdb.json", "TMDB_TOKEN is not set to fetch it again")));
    assert!(http.requests().is_empty());
    assert_eq!(fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap(), copy(STALE));
}

#[test]
fn plan_with_tmdb_token_fetches_the_stale_copy_again_and_plans() {
    let s = Scratch::new("stale-plan-token");
    known_film(&s, &copy(STALE));
    let http = Arc::new(RecordedTransport::new());
    http.answer(&movie(), 200, RECORD_ANSWER.as_bytes());
    let (code, out, err) = run(&ctx_with(&s, &http, true), &["plan", "recs/film"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(err, "lazymkv-dev plan: recs/film/tmdb.json was more than six months old and was fetched again\n");
    let requests = http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, movie());
    assert!(requests[0].1.contains(&("Authorization".to_string(), format!("Bearer {TOKEN}"))));
    let text = fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap();
    assert_eq!(fetched_at(&text).as_deref(), Some("2026-10-13"));
    assert!(text.ends_with('\n'));
    assert!(!s.0.join("recs/film/tmdb-record.json").exists(), "only the stale copy is replaced");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["plan"]["identity"]["original_language"], json!("es"));
    assert!(!out.contains(TOKEN) && !err.contains(TOKEN));
    assert_eq!(fs::read_dir(s.0.join("recs/film")).unwrap().count(), 6, "no temporary file is left");
}

#[test]
fn decide_and_retune_apply_the_same_gate() {
    let s = Scratch::new("stale-gate");
    film(&s, &copy(STALE));
    let http = Arc::new(RecordedTransport::new());
    let ctx = ctx_with(&s, &http, false);
    let (code, _, err) = run(&ctx, &["decide", "--rec", "recs/film/eng-only"]);
    assert_eq!((code, err), (3, format!("lazymkv-dev decide: error: {}\n",
                                        line("tmdb.json", "TMDB_TOKEN is not set to fetch it again"))));
    let shown = "corpus/owner/film/tmdb.json: the TMDb copy is more than six months old (fetched_at 2026-01-01) and \
                 TMDB_TOKEN is not set to fetch it again\n";
    for argv in [&["retune", "--evidence", "recs", "--spec", SPEC][..],
                 &["retune", "--subtitles", "--evidence", "recs", "--spec", SPEC][..]] {
        assert_eq!(run(&ctx, argv), (3, String::new(), shown.to_string()), "{argv:?}");
    }
    assert!(http.requests().is_empty());
}

#[test]
fn a_fresh_folder_asks_nothing() {
    let s = Scratch::new("stale-fresh");
    film(&s, &copy(FRESH));
    let http = Arc::new(RecordedTransport::new());
    let gated = run(&ctx_with(&s, &http, true), &["plan", "recs/film"]);
    assert_eq!(gated.0, 0);
    assert_eq!(gated, run(&Ctx::for_test(&s.0), &["plan", "recs/film"]), "byte-identical to a run with no transport");
    assert!(http.requests().is_empty());
}

#[test]
fn a_failed_refetch_exits_3_with_the_reason_and_never_the_token() {
    for (fault, why) in [(None, "TMDb refused the token (HTTP 401)"), (Some(TransportError::Timeout), "no answer within 10 s")] {
        let s = Scratch::new("stale-failed");
        film(&s, &copy(STALE));
        let http = Arc::new(RecordedTransport::new());
        match fault { None => http.answer(&movie(), 401, b"{}"), Some(error) => http.fail(&movie(), error) }
        let (code, out, err) = run(&ctx_with(&s, &http, true), &["plan", "recs/film"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert_eq!(err, format!("lazymkv-dev plan: error: {}\n",
                                line("tmdb.json", &format!("could not be fetched again: {why}"))));
        assert!(!err.contains(TOKEN));
        assert_eq!(fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap(), copy(STALE));
    }
}

#[test]
fn an_engine_job_folder_is_read_without_the_gate() {
    let s = Scratch::new("stale-job-folder");
    film(&s, &copy(STALE));
    s.write("recs/film/state.json", "{}");
    let http = Arc::new(RecordedTransport::new());
    let ctx = ctx_with(&s, &http, false);
    assert_eq!(run(&ctx, &["plan", "recs/film"]).0, 0);
    assert_eq!(run(&ctx, &["decide", "--rec", "recs/film/eng-only"]).0, 0);
    assert!(http.requests().is_empty());
    assert_eq!(fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap(), copy(STALE));
    assert_eq!(refresh_copies(&ctx, &s.0.join("recs/film"), "recs/film"), Ok(vec![]));
}

#[test]
fn a_copy_without_fetched_at_outside_a_job_folder_is_read_as_it_is() {
    let s = Scratch::new("stale-no-fetched-at");
    film(&s, r#"{"id": 6537, "title": "Made Up Film", "original_language": "es"}"#); // parse_recorded's id and title, no fetched_at
    let http = Arc::new(RecordedTransport::new());
    let ctx = ctx_with(&s, &http, false);
    assert_eq!(run(&ctx, &["plan", "recs/film"]).0, 0);
    let (code, _, err) = run(&ctx, &["retune", "--evidence", "recs", "--spec", SPEC]);
    assert_ne!(code, 3, "{err}");
    assert!(!err.contains("TMDb copy"), "{err}");
    assert!(http.requests().is_empty());
}
```

In `crates/ctl/src/tests.rs`:

```rust
#[test]
fn ctl_plan_over_a_folder_with_a_stale_copy_and_no_token_exits_3() {
    let s = Scratch::new("ctl-stale");
    recording(&s);
    s.write("recs/disc-a/lookup-status.txt", "tmdb movie/6537 200\n");
    s.write("recs/disc-a/tmdb-record.json",
            r#"{"id": 6537, "title": "Made Up Film", "kind": "movie", "fetched_at": "2026-01-01"}"#);
    let ctx = Ctx::for_test(&s.0);
    let local = ctl(&ctx, &["plan", "recs/disc-a"]);
    assert_eq!(local.0, 3);
    assert_eq!(String::from_utf8(local.2.clone()).unwrap(),
               "lazymkv-dev plan: error: recs/disc-a/tmdb-record.json: the TMDb copy is more than six months old \
                (fetched_at 2026-01-01) and TMDB_TOKEN is not set to fetch it again\n");
    assert_eq!(local, dev(&ctx, &["plan", "recs/disc-a"]));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked stale::`
Expected: compile errors E0432 and E0609 naming `stale`, `refresh_copies`, `TMDB_TOKEN_VAR` and the field `http`.

- [ ] **Step 3: Implement the items of Interfaces**

`Ctx`: the field, `None` in both constructors; `with_live_transport` as its doc says. `main.rs` of both binaries: `Ctx::from_process().with_live_transport()`.

`refresh_copies`, in this order (the algorithm the signature leaves open):
1. `folder.join(JOB_STATE_FILE).is_file()`: `Ok(vec![])`.
2. The status text: `lookup-status.txt` read as UTF-8 with a leading U+FEFF dropped, `""` when absent.
3. `stale_copies(&status, &|name| fs::read(folder.join(name)).ok(), false, &ctx.today())`; empty: `Ok(vec![])`.
4. The token: `ctx.env[TMDB_TOKEN_VAR]` when present and not empty; else `Err(Refused { code: 3, line: no_token_line(shown, &copies[0]) })`. The transport: `ctx.http`, else `OfflineTransport`.
5. `tmdb.json` or `tmdb-record.json` stale: the identity from `status_record(&status)` (no identity: `failed_line(.., "no tmdb line of lookup-status.txt names the record")`); `refetch_record(t, Some(token), id, None, &today)` (dev has no parsed season: every listed season, T4-R4); of its `SavedFile`s only those whose name is a stale copy are written, so a phase 0 folder never gains a `tmdb-record.json`. `Err(reason)`: `failed_line(shown, first stale record copy, &reason_text(&reason))`.
6. `tmdb-search.json` stale: the queries from its top-level `queries` array of strings; `refetch_search`; its file written; `Err` as in 5.
7. Each write: `<file>.lazymkv-tmp` beside the copy, then `fs::rename` over it; an I/O error removes the temporary file and gives `failed_line(.., &<the error's text>)`, code 3.

The gate in each command, after the folder is known and before anything is read (`DiskTree` reads file bytes on first use, `evidence.rs:289-296`, so a replaced copy is the one read): `plan` and `decide` call `refresh_copies(ctx, &rec.tree.path(&rec.dir), &rec.tree.display(&rec.dir))`, print an `Err` as `lazymkv-dev <name>: error: <line>` and return its code, and print `fetched_line` to `err` for each name returned. `retune` (audio and `--subtitles`) calls it for each direct child folder of the evidence root that is not `.git`, in name order, with `shown` `corpus/owner/<name>` (`EVIDENCE_SHOWN`, as retune prints every path), printing an `Err` bare through `fail`. Each module doc and `HELP` exit line gains: "3 when a TMDb copy of the folder is more than six months old and cannot be fetched again (TMDB_TOKEN unset, or the fetch failed; design part 8.4, K15); an engine job folder is read as it is".

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked stale::`
Expected: PASS, 7 tests.

Run: `scripts/box.sh cargo test -p lazymkv-ctl --locked stale`
Expected: PASS, 1 test.

Before the junction run: `BOX 'powershell -NoProfile -Command "Get-ChildItem C:\lazymkv\evidence\owner -Recurse -Filter tmdb*.json | Get-FileHash -Algorithm SHA256 | Sort-Object Path | ForEach-Object Hash"' > "$SCRATCH/pr4-clone-tmdb-before.txt"` (`BOX` as Task 0 defines it; the mirror has no `.git`, design part 8.5, so `git status` cannot be used).

Run (junction form, Global Constraints): `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS; the goldens byte-identical (K5). Then the same `BOX` command into `$SCRATCH/pr4-clone-tmdb-after.txt`, and `cmp "$SCRATCH/pr4-clone-tmdb-before.txt" "$SCRATCH/pr4-clone-tmdb-after.txt"` prints nothing: no test met a stale clone copy (Owner item 23). Only "equal" or "differs" is reported, never a listed hash or name (RP21).

- [ ] **Step 5: Commit**

```bash
git add crates/dev/src/evidence.rs crates/dev/src/main.rs crates/dev/src/lib.rs crates/dev/src/stale.rs crates/dev/src/cmd/plan.rs crates/dev/src/cmd/decide.rs crates/dev/src/cmd/retune.rs crates/ctl/src/main.rs crates/ctl/src/tests.rs
git commit -m "dev: plan, decide and retune fetch a stale copy again or exit 3 (K15)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G krcm0209 <17315774+krcm0209@users.noreply.github.com> krcm0209 <17315774+krcm0209@users.noreply.github.com>`, then the message alone, with no attribution line. CI green on the three systems before Task 11.

---

### Task 11: The cleanup task (design part 5.9, S40, P4, R-B4, T4-R11)

Precondition: `decisions.txt` holds item 22's answer (Task 0 Step 1).

**Files:**
- Create: `crates/engine/src/cleanup.rs` (the pass; its `mod tests` holds the pure tests below); `crates/engine/src/lib.rs:3-19` (`mod cleanup;`).
- Modify: `crates/engine/src/lib.rs:35-53` (`EngineConfig.cleanup_minutes`), after `:57` (`CLEANUP_MINUTES`).
- Modify: `crates/engine/src/owner.rs:159-167` (`TestHooks.cleanup_done`, `.cleanup_period`), `:170-202` (`Msg::Cleanup`, `Msg::CleanupDone`), `:216-333` (the schedule thread after the owner thread starts, when `config.cleanup_minutes` is Some), `:338-367` (`EngineHandle.cleanup_stop`, `.cleanup_schedule`), `:425-445` (`stop` ends the schedule before `Msg::Stop`), `:928-990` (`Owner::load` takes `cleanup_done`; `Owner.cleanup_running`), `:1193-1245` (both message matches), `:1256-1260` (`busy` counts a running pass).
- Modify: `crates/engine/src/testkit.rs:468-469` (fields `cleanup_minutes: Mutex<Option<u32>>`, `cleanup_period: Mutex<Option<Duration>>`, `cleanup_done: Mutex<Option<Receiver<()>>>`), `:614-626` (`cleanup_minutes` in the `EngineConfig` literal), `:647` (the two `TestHooks` fields), `:1428` (their initial values), and the three methods below.
- Modify: `crates/engine/src/native.rs:200-207` (`cleanup_minutes: None` in the `EngineConfig` literal, so the Windows build compiles until Task 17).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:15` (`Held` joins the `lazymkv_core::job` import), after `:174` (`seed_filed`), and three harness tests in a section "The cleanup (design part 5.9; Ruling T4-R11)".

**Interfaces:**
- Consumes: Task 8's `lazymkv_core::lookup::stale::{add_months, is_stale, fetched_at, TMDB_COPIES}`; `ShareConnector::reach`, `Reach`; `Store::log`; the owner's `jobs` with each job's `settings` (ENG-19) and `lookups`; `FakeShares::{set_reach, stall}`, `StallGate::wait_for_caller`.
- Produces:

```rust
// crates/engine/src/lib.rs
pub struct EngineConfig {
    // PR 3 fields unchanged
    /// The cleanup's period in minutes (design part 5.9; Ruling T4-R11): Some runs a pass at start and then each
    /// period on a thread of its own; None runs none (tests). Def: T4-11. Use: T4-11, T4-17.
    pub cleanup_minutes: Option<u32>,
}
/// The binary's cleanup period: hourly (design part 5.9). Def: T4-11. Use: T4-17.
pub const CLEANUP_MINUTES: u32 = 60;

// crates/engine/src/cleanup.rs (design part 5.9, S40, R-B4, L11; Ruling T4-R11)
/// One job as the owner thread hands it to a pass: its record, its own settings' staging root, keep_tuning_evidence and
/// retention_days (ENG-19), and its lookup book, which a round holds for its whole length (R23.1).
pub(crate) struct CleanupJob {
    pub(crate) id: String,
    pub(crate) record: JobRecord,
    pub(crate) staging_root: Option<String>,
    pub(crate) keep_tuning_evidence: bool,
    pub(crate) retention_days: u32,
    pub(crate) lookups: Arc<Mutex<LookupBook>>,
}
pub(crate) fn job_id_shaped(id: &str) -> bool; // 32 lower-case hex digits, '-', a decimal number >= 1 without a
    // leading zero (K3)
pub(crate) fn retention_due(job: &CleanupJob, now_unix: u64) -> bool; // state Filed, filed_unix Some(f),
    // now_unix > f + retention_days * 86_400, keep_tuning_evidence false (Owner item 22), no TuningEvidencePending note
pub(crate) fn run_pass(jobs: Vec<CleanupJob>, data: &Path, shares: &dyn ShareConnector, today: &str, now_unix: u64,
                       log: &dyn Fn(String));

// crates/engine/src/owner.rs
pub(crate) struct TestHooks { /* PR 3 fields */
    pub(crate) cleanup_done: Option<Sender<()>>,     // sent once at the end of each pass
    pub(crate) cleanup_period: Option<Duration> }    // replaces cleanup_minutes' period, tests only
pub(crate) enum Msg { /* PR 3 variants */ Cleanup, CleanupDone }
impl EngineHandle { pub(crate) fn cleanup_now(&self); } // sends Msg::Cleanup

// crates/engine/src/testkit.rs
impl Harness {
    /// One cleanup pass through `Msg::Cleanup`; returns when the pass ends, and panics after 10 s times `ci_factor()`
    /// (design part 5.9). Def: T4-11. Use: T4-11.
    pub fn run_cleanup(&self);
    /// From the next `start`, `cleanup_minutes: Some(1)` with its period replaced by `period` (design part 5.9).
    /// Def: T4-11. Use: T4-11.
    pub fn schedule_cleanup(&self, period: Duration);
    /// The running engine's busy flag (Ruling R24.c1; a running pass counts); false when no engine runs.
    /// Def: T4-11. Use: T4-11.
    pub fn busy(&self) -> bool;
}
```

The `engine.log` lines (T4-R11), exactly: `cleanup: <id>: deleted <file> (fetched_at <date>)`, `cleanup: <id>: <file> could not be read: <error>`, `cleanup: <id>: <file> could not be deleted: <error>`, `cleanup: <id>: deleted its staging folder`, `cleanup: <id>: the staging folder was not reached: <reason>`, `cleanup: <id>: the staging folder could not be deleted: <error>`, `cleanup: <id>: left alone: the name is not shaped like a job ID`.

- [ ] **Step 1: Write the failing tests**

In `cleanup.rs`'s `mod tests`: `const NOW: u64 = 1_792_800_000; const TODAY: &str = "2026-10-23"; const DAY: u64 = 86_400;`; `id(n)` is `format!("{n:032x}-1")`; `job(n, state, filed_unix)` builds a `CleanupJob` whose record is built as `restart.rs`'s test helper `record` builds one, with `staging_root` the scratch `staging` folder, `keep_tuning_evidence` false, `retention_days` 7 and a fresh book; `pass(folders, jobs, shares, today, now)` runs `run_pass` over `<scratch>/data` and gives the lines `log` received; `copy(date)` is `{"id": 1, "fetched_at": "<date>"}`.

```rust
#[test]
fn retention_is_due_only_for_a_filed_job_older_than_its_days_without_pending_evidence() {
    let mut filed = job(1, State::Filed, Some(NOW - 7 * DAY - 1));
    assert!(retention_due(&filed, NOW));
    filed.record.filed_unix = Some(NOW - 7 * DAY);
    assert!(!retention_due(&filed, NOW), "exactly its retention is not older");
    filed.record.filed_unix = Some(NOW - 7 * DAY - 1);
    filed.keep_tuning_evidence = true;
    assert!(!retention_due(&filed, NOW), "Owner item 22");
    filed.keep_tuning_evidence = false;
    filed.record.notes.push(Noted { note: Note::TuningEvidencePending, stream: None, detail: String::new() });
    assert!(!retention_due(&filed, NOW));
    for state in [State::NeedsReview { held: Held::DispositionGate },
                  State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) }, State::Ripping] {
        assert!(!retention_due(&job(2, state, Some(NOW - 30 * DAY)), NOW));
    }
    assert!(!retention_due(&job(3, State::Filed, None), NOW));
}
```

- `a_filed_job_past_retention_has_its_staging_folder_deleted`: `data/jobs/<id>/` holds `state.json` (`x`) and `tmdb.json` (`copy("2026-10-01")`); `staging/<id>/tuning/a.txt` and `staging/<id>/rip/t00/b.mkv`; `job(1, Filed, Some(NOW - 7 * DAY - 1))`, `FakeShares::new()`: `staging/<id>` is gone, `staging` itself is there, both job folder files byte-identical, lines `== [format!("cleanup: {id}: deleted its staging folder")]`.
- `a_filed_job_at_exactly_its_retention_keeps_its_staging_folder`: as above with `Some(NOW - 7 * DAY)`: the folder kept, lines empty.
- `the_cleanup_never_touches_tuning_root`: `tuning/recordings/<id>/tmdb.json` holding `copy("2019-01-01")` and a past-retention filed job: the file is byte-identical after the pass (design part 5.9; part 7.5's mirror is pull request 5's).
- `job_id_shaped_takes_only_a_fingerprint_and_a_number`: true for `id(1)` and `format!("{:032x}-12", 7)`; false for `..`, `abc-1`, the upper-case form of `id(1)`, `format!("{:032x}-0", 1)`, `format!("{:032x}-01", 1)`, `format!("{:032x}", 1)`, `format!("{:031x}-1", 1)`.
- `the_cleanup_deletes_tmdb_copies_older_than_six_months_in_every_job_whatever_its_state`: four jobs, `NeedsReview { DispositionGate }`, `Failed { Unknown, from Remuxing }`, `Filed` (filed `NOW`), `AwaitingLookup { return_to Ejected }`, each folder with `tmdb.json` `copy("2026-04-22")` (stale: plus six months is 2026-10-22), `tmdb-record.json` `copy("2026-04-23")` (fresh: plus six months is today) and `tmdb-search.json` `copy("2026-10-01")`: every `tmdb.json` gone, the others byte-identical, lines in id order `cleanup: <id>: deleted tmdb.json (fetched_at 2026-04-22)`, one per job.
- `a_clock_years_ahead_deletes_only_tmdb_copies` (Review Focus 5): today `"2031-10-23"`, now `NOW + 5 * 365 * DAY`; jobs `Filed` with `keep_tuning_evidence` true, `NeedsReview`, `Failed`, `Ripping`, each folder with `state.json`, `job.log`, `remembered.json`, `t00-00800/decide.json` and the three copies dated `2026-10-01`, each with `staging/<id>/rip/t00/b.mkv`: only the twelve copies are gone, every other file byte-identical (compared by a before and after listing of names and bytes), every staging folder there.
- `the_cleanup_never_deletes_a_folder_not_shaped_like_a_job_id` (Review Focus 5): jobs with ids `..` and the upper-case form of `id(1)`, both `Filed` past retention; `data/tmdb.json` (that is `data/jobs/..`) and `staging/other/x` exist: both survive, `staging` is there, lines are the two `left alone` lines.
- `the_cleanup_never_follows_a_link_out_of_a_staging_folder` (Review Focus 5): `library/film.mkv`; `cfg(unix)`: `std::os::unix::fs::symlink(library, staging/<id>/rip/link)`; `cfg(windows)`: `cmd /c mklink /J <staging>\<id>\rip\link <library>`, as `platform/src/win/folder.rs:405` makes one. After the pass of a past-retention filed job: `staging/<id>` gone, `library/film.mkv` byte-identical.
- `a_copy_without_a_readable_fetched_at_is_left_for_the_refetch` (Ruling T4-R7, design part 5.9's letter): a `NeedsReview` job's folder with `tmdb.json` `{"id": 1}` and `tmdb-record.json` `{"id": 1, "fetched_at": "2026-13-45"}`: both byte-identical after the pass, lines empty.
- `an_unreached_staging_root_deletes_nothing_and_says_so`: `shares.set_reach(staging, Reach::Unreachable("the share did not answer".into()))`: the folder kept, lines `== [format!("cleanup: {id}: the staging folder was not reached: the share did not answer")]`.
- `a_job_whose_book_a_round_holds_is_skipped_this_pass`: a past-retention filed job with a stale `tmdb.json`; the test holds `job.lookups.lock()` (an `Arc::clone` kept before the job moves) across `run_pass`: the copy and the staging folder kept, lines empty.

In `engine_ipc.rs` (`seed_filed(h, n, filed_unix)` is `seed(h, n, State::Filed)` with `filed_unix` set in the written `state.json`):

```rust
#[test]
fn the_harness_never_runs_a_cleanup_unasked() {
    let h = scripted(vec![]);
    let id = seed_filed(&h, 1, 1_792_800_000 - 8 * 86_400);
    fs::write(h.job_dir(&id).join("tmdb.json"), r#"{"id": 1, "fetched_at": "2020-01-01"}"#).unwrap();
    fs::create_dir_all(h.staging().join(&id).join("rip")).unwrap();
    h.restart();
    h.settle();
    assert!(h.job_dir(&id).join("tmdb.json").exists());
    assert!(h.staging().join(&id).is_dir());
    assert_eq!(h.timers().pending(), Vec::<u32>::new());
    assert!(!h.engine_log().contains("cleanup:"), "{}", h.engine_log());
    h.run_cleanup(); // one pass, on demand
    assert!(!h.job_dir(&id).join("tmdb.json").exists());
    assert!(!h.staging().join(&id).exists());
    assert!(h.engine_log().contains(&format!("cleanup: {id}: deleted its staging folder")), "{}", h.engine_log());
}

#[test]
fn the_cleanup_runs_off_the_owner_thread() {
    let h = scripted(vec![]);
    let id = seed_filed(&h, 1, 1_792_800_000 - 8 * 86_400);
    fs::create_dir_all(h.staging().join(&id).join("rip")).unwrap();
    let stall = h.shares().stall(&h.staging().to_string_lossy());
    h.schedule_cleanup(Duration::from_secs(3600));
    h.restart();
    assert!(stall.wait_for_caller(WAIT * ci_factor()), "the pass reached the staging share");
    assert!(snapshot_within(&h, Duration::from_secs(2)).is_some(), "the owner answers while the pass waits");
    assert!(h.busy(), "a running pass counts as busy (R24.c1)");
    stall.release();
    h.wait("the staging folder deleted", |h| !h.staging().join(&id).exists());
    assert!(h.engine_log().contains(&format!("cleanup: {id}: deleted its staging folder")));
}

#[test]
fn a_configured_cleanup_runs_once_at_start_and_again_each_period() {
    let h = scripted(vec![]);
    let id = seed(&h, 1, State::NeedsReview { held: Held::DispositionGate });
    let copy = h.job_dir(&id).join("tmdb.json");
    fs::write(&copy, r#"{"id": 1, "fetched_at": "2020-01-01"}"#).unwrap();
    h.schedule_cleanup(Duration::from_millis(200));
    h.restart();
    h.wait("the first pass", |_| !copy.exists());
    fs::write(&copy, r#"{"id": 1, "fetched_at": "2020-01-02"}"#).unwrap();
    h.wait("the next pass", |_| !copy.exists());
    let log = h.engine_log();
    assert!(log.contains(&format!("cleanup: {id}: deleted tmdb.json (fetched_at 2020-01-01)")), "{log}");
    assert!(log.contains(&format!("cleanup: {id}: deleted tmdb.json (fetched_at 2020-01-02)")), "{log}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked cleanup`
Expected: compile errors E0432 and E0425 naming `cleanup`, `CleanupJob`, `run_pass`, `retention_due` and `job_id_shaped`.

- [ ] **Step 3: Implement the items of Interfaces**

`run_pass`, for each job in the order given (the order and the guards the signature leaves open):
1. `!job_id_shaped(&job.id)`: the `left alone` line; next job.
2. `job.lookups.try_lock()`: `WouldBlock` skips the job silently this pass (a round or a refetch holds it); a poisoned lock is taken as it stands. The guard is held to the job's end.
3. For each file of `TMDB_COPIES` in `<data>/jobs/<id>/`: `NotFound` skips it; another read error gives the `could not be read` line; a copy whose `fetched_at(&String::from_utf8_lossy(&bytes))` is Some(date) with `add_months(date, 0)` Some and `is_stale(Some(&date), today)` true is deleted with `fs::remove_file` and the `deleted <file>` line (T4-R7: a missing or malformed `fetched_at` is left to the refetch; a malformed `today` makes nothing stale). Nothing else under `jobs/` is touched.
4. `retention_due(&job, now_unix)` and `staging_root` Some: `shares.reach(root)`; `Unreachable(why)` gives the `not reached` line; `Reachable`: `<root>/<id>` absent (`symlink_metadata` `NotFound`) does nothing; else `fs::remove_dir_all`, which removes a symlink or junction inside without following it (std since 1.58.1), then the `deleted its staging folder` line, or the `could not be deleted` line.

The owner: `Msg::Cleanup` with `cleanup_running` false builds one `CleanupJob` per job of `self.jobs` (id order, every state) from `job.record`, `job.settings` and `Arc::clone(&job.lookups)`, sets `cleanup_running`, and spawns the pass with `Arc<Store>`, `Arc::clone(&self.parts.shares)`, `self.parts.env.data`, `host.local_date()`, `host.now_unix()`; the thread logs through `Store::log`, sends `Msg::CleanupDone` at its end and is pushed to `self.threads`, so `shut_down` joins it. `Msg::Cleanup` while a pass runs, or during a forced shutdown (`quiet_message`), is dropped. `Msg::CleanupDone` clears `cleanup_running` and sends on `cleanup_done` when Some; `quiet_message` handles `Msg::CleanupDone` the same way as the normal handler (it clears `cleanup_running` and sends on `cleanup_done` when Some), so `busy` never stays set through a forced shutdown. The schedule thread: `send(Msg::Cleanup)`, then `stop.recv_timeout(period)`, repeating on `Timeout` and ending on anything else or a failed send; `period` is `hooks.cleanup_period` else `cleanup_minutes * 60` s. `stop` drops `cleanup_stop` and joins the schedule after the drive thread and before `Msg::Stop`. The harness: `run_cleanup` drains `cleanup_done`, calls `cleanup_now` and waits; `schedule_cleanup` sets the two fields read at the next `start`; `busy` reads `EngineHandle::busy` of the running engine.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked cleanup`
Expected: PASS, 12 tests (`retention_is_due_...` and the eleven listed; the `cfg(unix)` or `cfg(windows)` link test runs on its system).

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc cleanup`
Expected: PASS, 3 tests.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS; no existing test's `timers().pending()` changes.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/cleanup.rs crates/engine/src/lib.rs crates/engine/src/owner.rs crates/engine/src/testkit.rs crates/engine/src/native.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the cleanup task of part 5.9"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 12.

---

### Task 12: The webhook notifier (design part 8.6, spec 12, A4, T4-R13, T4-R16)

**Files:**
- Modify: `crates/engine/src/notify.rs:15-39` (`NotifySink::stop_flag`; the constants; `WebhookSink`; `NoDelivery` and `NO_DELIVERY` kept, their docs reworded "a test double until Task 17 removes it with the binary's change", T4-R16), `:113-180` (the tests below).
- Modify: `crates/engine/src/owner.rs:265-275` (`let notify_stop = notify.stop_flag();` before the sink moves into `Delivered`), `:338-367` (`EngineHandle.notify_stop: Option<Arc<AtomicBool>>`), `:425-445` (`stop` stores true in it first; `notifier_done.recv_timeout(Duration::from_secs(NOTIFY_DRAIN_S))` replaces `recv()`; the doc says so).
- Modify: `crates/engine/src/lib.rs:28` (`pub use notify::{NoDelivery, NotifySink, WebhookSink, ATTEMPT_WAITS_S, NOTIFY_DRAIN_S, NO_DELIVERY};`).
- Modify: `crates/engine/src/testkit.rs:69` (imports `WebhookSink`, drops `NoDelivery`), `:468-469, :1428` (`binary_services: AtomicBool` becomes `services: Mutex<ServicesMode>`, private `enum ServicesMode { Recorded, Offline, Webhook }`), `:600-606` (the three modes), `:1076-1083` (`use_binary_services` becomes `use_offline_services`; `use_webhook_sink` beside it).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:21` (the `NO_DELIVERY` import replaced by `NOTIFY_DRAIN_S`), `:1901-1940` (the two W26 tests) and one drain test.

**Interfaces:**
- Consumes: Task 6's `HttpTransport::post`, `RecordedTransport::{posts, answer, fail, gate}` (a gate holds a POST to its URL as it holds a GET) and `core::ipc::WEBHOOK_SECRETS`; `clients::{user_agent, TIMEOUT_S, TransportError}`; `platform::SecretStore`, `MemorySecrets`.
- Produces:

```rust
// crates/engine/src/notify.rs
pub trait NotifySink: Send {
    fn deliver(&mut self, notification: &Notification) -> Result<(), String>;
    /// The flag `EngineHandle::stop` raises so the sink skips its waits (design part 8.6; Ruling T4-R13); None for
    /// a sink that never waits. Def: T4-12. Use: T4-12.
    fn stop_flag(&self) -> Option<Arc<AtomicBool>> { None }
}
/// The wait before each attempt of one channel, counted from the previous failure (design part 8.6: the second 1 s
/// after the first fails, the third 4 s after the second). Def: T4-12. Use: T4-12.
pub const ATTEMPT_WAITS_S: [u64; 3] = [0, 1, 4];
/// The longest `EngineHandle::stop` waits for the notifier (spec 12: "a restart loses a notification still being
/// retried"; Ruling T4-R13). Def: T4-12. Use: T4-12.
pub const NOTIFY_DRAIN_S: u64 = 15;
/// The engine's notifier (design part 8.6, spec 12; Rulings T4-R12, T4-R13): each notification POSTed as the JSON of
/// `Notification` to `ha_webhook_url`, then `webhook_url`, each read from the secrets at delivery and skipped when
/// unset, with `Content-Type: application/json` and `User-Agent: lazymkv/<version>`; any 2xx delivers; three attempts
/// per channel. A failed channel's reason names the channel, never its URL. Def: T4-12. Use: T4-12, T4-17, T4-18.
pub struct WebhookSink { http: Arc<dyn HttpTransport>, secrets: Arc<dyn SecretStore>,
                         pause: Arc<dyn Fn(Duration) + Send + Sync>, stopping: Arc<AtomicBool> }
impl WebhookSink {
    /// The production pause: sleeps in steps of 50 ms until the wait has passed or `stopping` is raised.
    /// Def: T4-12. Use: T4-17.
    pub fn new(http: Arc<dyn HttpTransport>, secrets: Arc<dyn SecretStore>) -> WebhookSink;
    /// `self` with `pause` in place of the production one (tests: a recorder or a no-op). Def: T4-12. Use: T4-12, T4-18.
    pub fn with_pause(self, pause: Arc<dyn Fn(Duration) + Send + Sync>) -> WebhookSink;
}
impl NotifySink for WebhookSink {} // deliver and stop_flag (Some(Arc::clone(&self.stopping)))

// crates/engine/src/testkit.rs
impl Harness {
    /// From the next `start`: `OfflineTransport` and a `WebhookSink` over the harness's secrets with a no-op pause
    /// (Ruling T4-R16; it replaces `use_binary_services`). Def: T4-12. Use: T4-12.
    pub fn use_offline_services(&self);
    /// From the next `start`: the harness's `RecordedTransport` and a `WebhookSink` over its secrets with a no-op
    /// pause, so `http().posts()` holds the deliveries and `notifications()` stays empty. Def: T4-12. Use: T4-12, T4-18.
    pub fn use_webhook_sink(&self);
}
```

A channel's outcome text: a non-2xx answer `HTTP <n>`; `Timeout` `no answer within 10 s` (from `TIMEOUT_S`); `Unreachable(words)` the words; `Unrecorded(_)` `no recorded answer`. A failed channel's reason: `<name>: <last outcome> after 3 attempts`, or, when the stop flag ended its attempts, `<name>: <last outcome> after <n> attempt (the engine was stopping)` with `attempts` for `n` above 1; several joined with `"; "`.

- [ ] **Step 1: Write the failing tests**

In `notify.rs`'s `mod tests`: `HA` is `http://ha.local:8123/api/webhook/hook-sentinel-7f3a`, `HOOK` is `http://hooks.local/hook-sentinel-1c4b`; `sink(http, secrets)` is `WebhookSink::new(..).with_pause(recorder)` whose recorder pushes each wait into a shared `Vec<Duration>`; `notification(event, job)` as the existing tests build one. The module's `use` lines gain `std::sync::atomic::Ordering`, `std::sync::Mutex`, `std::time::Duration`, `lazymkv_clients::{user_agent, RecordedTransport, TransportError}`, `lazymkv_platform::fakes::MemorySecrets`, `lazymkv_platform::SecretStore` and `super::WebhookSink`.

```rust
#[test]
fn a_notification_is_posted_as_json_to_each_set_channel_in_order() {
    let http = Arc::new(RecordedTransport::new());
    http.answer(HA, 200, b"");
    http.answer(HOOK, 200, b"");
    let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA).with("webhook_url", HOOK));
    let (mut sink, pauses) = sink(&http, &secrets);
    let n = notification(NotifyEvent::Filed, Some("abc-1"));
    assert_eq!(sink.deliver(&n), Ok(()));
    let posts = http.posts();
    assert_eq!(posts.iter().map(|(url, _, _)| url.as_str()).collect::<Vec<_>>(), [HA, HOOK]);
    for (_, headers, body) in &posts {
        assert_eq!(body, &serde_json::to_vec(&n).unwrap());
        assert!(headers.contains(&("Content-Type".to_string(), "application/json".to_string())));
        assert!(headers.contains(&("User-Agent".to_string(), user_agent())));
    }
    assert!(pauses.lock().unwrap().is_empty());
}

#[test]
fn a_failing_channel_is_tried_three_times_with_waits_of_1_and_4_seconds() {
    let http = Arc::new(RecordedTransport::new());
    http.answer(HA, 500, b"");
    let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
    let (mut sink, pauses) = sink(&http, &secrets);
    assert_eq!(sink.deliver(&notification(NotifyEvent::DiscDone, Some("abc-1"))),
               Err("ha_webhook_url: HTTP 500 after 3 attempts".to_string()));
    assert_eq!(http.posts().len(), 3);
    assert_eq!(*pauses.lock().unwrap(), [Duration::from_secs(1), Duration::from_secs(4)]);
}

#[test]
fn an_undelivered_notification_names_its_channel_and_never_its_url() {
    let url = "http://user-sentinel-7c1e:pass-sentinel-4b9d@ha.local:8123/api/webhook/hook-sentinel-7f3a?x=1#f";
    for fault in [None, Some(TransportError::Timeout)] {
        let http = Arc::new(RecordedTransport::new());
        if let Some(error) = fault.clone() { http.fail(url, error); }
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", url));
        let (mut sink, _) = sink(&http, &secrets);
        let n = notification(NotifyEvent::DiscDone, Some("abc-1"));
        let reason = sink.deliver(&n).unwrap_err();
        let expected = if fault.is_some() { "no answer within 10 s" } else { "no recorded answer" };
        assert_eq!(reason, format!("ha_webhook_url: {expected} after 3 attempts"));
        let line = undelivered_line(&n, &reason);
        for needle in ["hook-sentinel-7f3a", "pass-sentinel-4b9d", "user-sentinel-7c1e", "x=1", "/api/webhook"] {
            assert!(!line.contains(needle), "{line}");
        }
    }
}

#[test]
fn a_stop_skips_the_waits_and_gives_each_queued_notification_one_attempt() {
    let http = Arc::new(RecordedTransport::new());
    http.answer(HA, 500, b"");
    let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
    let (mut sink, pauses) = sink(&http, &secrets);
    sink.stop_flag().expect("the sink has a stop flag").store(true, Ordering::SeqCst);
    for job in ["abc-1", "abc-2"] {
        assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some(job))),
                   Err("ha_webhook_url: HTTP 500 after 1 attempt (the engine was stopping)".to_string()));
    }
    assert_eq!(http.posts().len(), 2);
    assert!(pauses.lock().unwrap().is_empty());
}
```

- `any_2xx_delivers_and_a_204_is_not_retried`: `HA` answered 204, then (a second sink) 299: each `Ok(())` after one POST; 300 gives `Err("ha_webhook_url: HTTP 300 after 3 attempts")`.
- `a_channel_whose_url_is_unset_is_skipped_and_both_unset_log_nothing`: no secrets: `Ok(())`, `posts()` empty; through `spawn_notifier` over a `Store` in a `Scratch` with that sink, `engine.log` holds no `was not delivered` line after the sender is dropped and the thread ends.
- `one_failing_channel_does_not_stop_the_other`: `HA` 200, `HOOK` 500: `Err("webhook_url: HTTP 500 after 3 attempts")`, posts `[HA, HOOK, HOOK, HOOK]`.
- `a_url_set_after_start_is_used_by_the_next_notification`: the first `deliver` with no secret posts nothing; after `secrets.set("ha_webhook_url", HA)` the second posts once to `HA`.
- `a_stop_during_the_retries_ends_them_after_the_attempts_made`: a pause that raises the flag when called: one POST, `Err("ha_webhook_url: HTTP 500 after 1 attempt (the engine was stopping)")`.

In `engine_ipc.rs`, replacing the two W26 tests at `:1901-1940` (section heading "The engine's notifier (spec 12; design part 8.6; Ruling T4-R13)"):

```rust
const HA_URL: &str = "http://ha.local:8123/api/webhook/hook-sentinel-7f3a";

#[test]
fn an_undelivered_notification_is_logged_with_its_channel() {
    let h = stand_in(&[DISC_A], settings());
    h.use_webhook_sink();
    h.secrets().set("ha_webhook_url", HA_URL).unwrap();
    h.http().answer(HA_URL, 500, b"");
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC_A);
    let id = id_of(&h, DISC_A);
    let line = format!("notification disc_done for {id} was not delivered: ha_webhook_url: HTTP 500 after 3 attempts");
    h.wait("the undelivered disc_done", |h| h.engine_log().contains(&line));
    assert!(!h.engine_log().contains("hook-sentinel-7f3a"));
    assert!(h.notifications().is_empty(), "{:?}", h.notifications());
}

#[test]
fn with_the_offline_transport_a_disc_waits_for_its_lookups() {
    let h = stand_in(&[DISC_A], settings());
    h.use_offline_services();
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC_A);
    let id = id_of(&h, DISC_A);
    h.wait("AwaitingLookup", |h| h.state_of(&id) == "AwaitingLookup");
    h.settle();
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingLookup { return_to: Box::new(State::Ejected { after_copy: false }) });
    assert!(record.message.unwrap_or_default().contains(&format!("dischash: {OFFLINE}")));
    assert_eq!(h.timers().pending(), vec![60]);
    assert!(h.http().requests().is_empty(), "{:?}", h.http().requests());
    assert!(!h.engine_log().contains("was not delivered"), "no webhook is set: {}", h.engine_log());
}

#[test]
fn stop_returns_within_the_drain_bound_while_a_webhook_hangs() {
    let h = stand_in(&[DISC_A], settings());
    h.use_webhook_sink();
    h.secrets().set("ha_webhook_url", HA_URL).unwrap();
    let gate = h.http().gate(HA_URL);
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC_A);
    h.wait("a held POST", |h| !h.http().posts().is_empty());
    let started = Instant::now();
    h.restart();
    assert!(started.elapsed() < Duration::from_secs(NOTIFY_DRAIN_S + 2), "{:?}", started.elapsed()); // unscaled
    gate.open();
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked notify`
Expected: compile errors E0432 and E0599 naming `WebhookSink`, `ATTEMPT_WAITS_S`, `NOTIFY_DRAIN_S` and `stop_flag`.

- [ ] **Step 3: Implement the items of Interfaces**

`deliver`: the body is `serde_json::to_vec(notification)` (an error gives `the notification is not JSON: <error>`); for each name of `WEBHOOK_SECRETS` whose secret is set and not empty, up to three POSTs with Interfaces' headers, a 2xx ending them; a channel still failing gives its reason in Interfaces' form; `Ok(())` when none failed, else the reasons joined with `"; "`. The one rule the tests leave open, the order of the stop checks: before attempts 2 and 3, when `stopping` is not raised and that attempt's `ATTEMPT_WAITS_S` entry is above 0, call `pause` with it; then, when `stopping` is raised, stop with the attempts made and the stopping form.

`outcome_text` (private) maps as Interfaces says and never formats a URL. `WebhookSink::new`'s pause loops `thread::sleep(min(50 ms, rest))` while time remains and `stopping` is false. `NoDelivery` and `NO_DELIVERY` stay with reworded docs, and `no_delivery_refuses_every_notification_with_its_sentence` stays until Task 17. In the harness, `Recorded` keeps `MemorySink`; `Offline` and `Webhook` build `WebhookSink::new(http, Arc::clone(&self.secrets) as _).with_pause(Arc::new(|_| {}))`. The `NotifySink` doc at `notify.rs:18`, `NoDelivery`'s at `:28-33` and `testkit.rs:468`'s field doc say instead that the binary passes `NoDelivery` until Task 17 (`testkit.rs:1078` changes with `use_binary_services`' rename).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked notify`
Expected: PASS, the three PR 3 tests and the nine new.

Run: `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc undelivered`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc offline_transport`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc drain_bound`
Expected: PASS, one test each; the drain test takes about 15 s.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/notify.rs crates/engine/src/owner.rs crates/engine/src/lib.rs crates/engine/src/testkit.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the webhook notifier of part 8.6"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems, and the controller reads the draft's CodeQL result (Global Constraints: zero alerts; watch cleartext logging and transmission) before Task 13.

---

### Task 13: The self-test's webhook checks and the webhook value's shape (design part 8.6, spec 13, S53, S80, T4-R14, T4-R15)

**Files:**
- Modify: `crates/engine/src/selftest.rs:1-5` (module doc names the webhook checks), `:15` (imports `webhook_target`, `WebhookTarget`, `WEBHOOK_SECRETS`), `:160-166` (`first_selftest`'s list gains `ha_webhook_url`, `webhook_url` from `WEBHOOK_SECRETS`), `:278-299` (`webhook_check` and `host_port` after `token_check`), `:478-497` (`selftest` pushes one `webhook_check` per name of `WEBHOOK_SECRETS` after `token_check`; its doc names T4-R14), `:605-` (`mod tests`).
- Modify: `crates/engine/src/owner.rs:20-21` (imports `webhook_target`, `WEBHOOK_SECRETS`), `:2588-2603` (`secret_set` refuses a webhook value `webhook_target` does not read; only with the owner's yes on T4-R15).
- Modify: `crates/fake-tools/tests/engine_ipc.rs:1566-1578` (the eleven names and no connection at start), `:375-435` (the refused webhook value, T4-R15), the self-test section after `:1578` (one test over the harness).

**Interfaces:**
- Consumes: Task 6's `lazymkv_clients::{webhook_target, WebhookTarget}`, `HttpTransport::connect`, `RecordedTransport::{connect_answer, connects, posts}`; `lazymkv_core::ipc::WEBHOOK_SECRETS`; the private `check` and `transport_text` of `selftest.rs`.
- Produces (crate-private; the detail texts are exact):

```rust
// selftest.rs
/// The detail of a webhook check whose URL is not set: the check passes, because design part 12.7 makes the generic
/// webhook optional and part 8.6 skips an unset channel (Ruling T4-R14). Def: T4-13. Use: T4-13.
pub(crate) const WEBHOOK_NOT_SET: &str = "not set; its notifications are skipped";
/// The detail of a webhook check whose stored value `webhook_target` does not read (Ruling T4-R14). Def: T4-13.
/// Use: T4-13.
pub(crate) const NOT_A_WEBHOOK_URL: &str = "the value is not an http or https URL";
/// `<host>:<port>` of a target, the host in brackets when it holds a `:` (an IPv6 address), so `[::1]:8123`.
fn host_port(target: &WebhookTarget) -> String;
/// The self-test's check of webhook secret `name` (design part 8.6, spec 13, S53; Ruling T4-R14): None passes with
/// WEBHOOK_NOT_SET; a value `webhook_target` does not read fails with NOT_A_WEBHOOK_URL; otherwise one
/// `http.connect(host, port)`, no request and no TLS handshake, so the owner's automation never fires: Ok passes with
/// `<host>:<port> accepted a connection`, Err fails with `<host>:<port> could not be reached: <transport_text>`.
/// `path` is always None and no text holds the URL, its path, its query or its userinfo (the URL is a secret,
/// ENG-17). Def: T4-13. Use: T4-13.
pub(crate) fn webhook_check(http: &dyn HttpTransport, name: &str, url: Option<&str>) -> SelfTestCheck;

// owner.rs (only with the owner's yes on T4-R15)
/// The refusal of a webhook value that is not an http or https URL with a host (Ruling T4-R15), after `<name> could
/// not be stored: `, as SHARE_VALUE_SHAPE is for a share. Def: T4-13. Use: T4-13.
const WEBHOOK_VALUE_SHAPE: &str = "the value must be an http or https URL with a host";
```

- `first_selftest`'s check names after this task, in order: `makemkvcon`, `mkvmerge`, `ffprobe`, `ffmpeg`, `java`, `staging_root`, `tuning_root`, `movies_root`, `tmdb_token`, `ha_webhook_url`, `webhook_url` (S80's nine, then T4-R14's two).

- [ ] **Step 1: The owner's answer on T4-R15 (Owner).** Precondition: `decisions.txt` holds T4-R15's answer (Task 0 Step 1). The controller reads that answer ("refuse a webhook value that is not an http or https URL with a host"). On a no, skip Steps 2's last test, Step 5 and Step 6's owner.rs line; T4-R15 is then recorded in engine.md as declined (Task 21). Nothing else in this task depends on it.

- [ ] **Step 2: Write the failing tests.**

In `crates/engine/src/selftest.rs`'s `mod tests` (the sentinels are the Global Constraints' own):

```rust
const HOOK: &str = "http://ha.local:8123/api/webhook/hook-sentinel-7f3a";

fn webhook_ok(name: &str, detail: &str, ok: bool) -> SelfTestCheck {
    SelfTestCheck { name: name.to_string(), ok, detail: detail.to_string(), path: None, version: None, found: None, busy: false }
}

#[test]
fn the_webhook_check_connects_to_host_and_port_and_posts_nothing() {
    let http = RecordedTransport::new();
    http.connect_answer("ha.local", 8123, Ok(()));
    let check = webhook_check(&http, "ha_webhook_url", Some(HOOK));
    assert_eq!(check, webhook_ok("ha_webhook_url", "ha.local:8123 accepted a connection", true));
    assert_eq!(http.connects(), [("ha.local".to_string(), 8123)]);
    assert!(http.posts().is_empty());
    assert!(http.requests().is_empty());
}

#[test]
fn an_unset_webhook_passes_and_says_its_notifications_are_skipped() {
    let http = RecordedTransport::new();
    assert_eq!(webhook_check(&http, "webhook_url", None), webhook_ok("webhook_url", WEBHOOK_NOT_SET, true));
    assert_eq!(WEBHOOK_NOT_SET, "not set; its notifications are skipped");
    assert!(http.connects().is_empty());
}

/// Review Focus 3.
#[test]
fn the_webhook_check_shows_only_host_and_port() {
    let http = RecordedTransport::new();
    http.connect_answer("ha.local", 8123, Ok(()));
    http.connect_answer("hooks.example.org", 443, Ok(()));
    http.connect_answer("::1", 8123, Ok(()));
    let full = "http://user-sentinel-7c1e:pass-sentinel-4b9d@ha.local:8123/api/webhook/hook-sentinel-7f3a?x=1#f";
    let check = webhook_check(&http, "ha_webhook_url", Some(full));
    assert_eq!(check, webhook_ok("ha_webhook_url", "ha.local:8123 accepted a connection", true));
    let text = serde_json::to_string(&check).unwrap();
    for secret in ["user-sentinel-7c1e", "pass-sentinel-4b9d", "hook-sentinel-7f3a", "x=1", "/api/webhook", "#f"] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
    assert_eq!(webhook_check(&http, "webhook_url", Some("https://hooks.example.org/x")).detail,
               "hooks.example.org:443 accepted a connection");
    assert_eq!(webhook_check(&http, "webhook_url", Some("http://[::1]:8123/")).detail, "[::1]:8123 accepted a connection");
    assert_eq!(http.connects(), [
        ("ha.local".to_string(), 8123), ("hooks.example.org".to_string(), 443), ("::1".to_string(), 8123),
    ]);
}

#[test]
fn an_unreachable_webhook_fails_with_its_host_and_port() {
    let http = RecordedTransport::new();
    http.connect_answer("ha.local", 8123, Err(TransportError::Unreachable("the connection was refused".to_string())));
    let refused = webhook_check(&http, "ha_webhook_url", Some(HOOK));
    assert_eq!(refused, webhook_ok("ha_webhook_url", "ha.local:8123 could not be reached: the connection was refused", false));
    let slow = RecordedTransport::new();
    slow.connect_answer("ha.local", 8123, Err(TransportError::Timeout));
    assert_eq!(webhook_check(&slow, "ha_webhook_url", Some(HOOK)).detail,
               "ha.local:8123 could not be reached: no answer came in time");
    // Task 6: an unrecorded connect names only host and port, so the URL's path cannot reach the detail.
    let unrecorded = webhook_check(&RecordedTransport::new(), "ha_webhook_url", Some(HOOK));
    assert_eq!(unrecorded.detail, "ha.local:8123 could not be reached: no answer is recorded for ha.local:8123");
    assert!(!unrecorded.ok);
}

#[test]
fn a_webhook_value_that_is_not_a_url_fails_without_echoing_it_or_connecting() {
    let http = RecordedTransport::new();
    for value in ["ftp://hook-sentinel-1c4b/x", "hook-sentinel-1c4b", "http:///hook-sentinel-1c4b", "http://h:0/hook-sentinel-1c4b"] {
        let check = webhook_check(&http, "webhook_url", Some(value));
        assert_eq!(check, webhook_ok("webhook_url", NOT_A_WEBHOOK_URL, false), "{value}");
    }
    assert_eq!(NOT_A_WEBHOOK_URL, "the value is not an http or https URL");
    assert!(http.connects().is_empty());
}

#[test]
fn a_start_lists_the_webhook_checks_as_not_checked() {
    let first = first_selftest(not_checked("makemkvcon"));
    let names: Vec<&str> = first.checks.iter().map(|check| check.name.as_str()).collect();
    assert_eq!(names, ["makemkvcon", "mkvmerge", "ffprobe", "ffmpeg", "java", "staging_root", "tuning_root",
                       "movies_root", "tmdb_token", "ha_webhook_url", "webhook_url"]);
    for check in &first.checks[1..] {
        assert_eq!((check.ok, check.detail.as_str()), (false, "not checked since the engine started"), "{}", check.name);
    }
}
```

In `crates/fake-tools/tests/engine_ipc.rs`, `the_engine_runs_the_makemkv_check_at_start_into_the_snapshot` (`:1567`) changes its last assertion and gains one (S80: "a start sends no request"):

```rust
    assert_eq!(names, ["makemkvcon", "mkvmerge", "ffprobe", "ffmpeg", "java", "staging_root", "tuning_root",
                       "movies_root", "tmdb_token", "ha_webhook_url", "webhook_url"]);
    assert!(h.http().connects().is_empty(), "a start opens no connection (S80)");
```

New, in the self-test section:

```rust
/// Design part 8.6, Ruling T4-R14: the self-test connects to the webhook's host and port, posts nothing, and an
/// unset `webhook_url` passes.
#[test]
fn a_selftest_checks_each_webhook_by_host_and_port_and_posts_nothing() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let url = "http://ha.local:8123/api/webhook/hook-sentinel-7f3a";
    c.ok("secret_set", json!({"name": "ha_webhook_url", "value": url}));
    h.http().connect_answer("ha.local", 8123, Ok(()));
    let result = c.ok("run_selftest", Value::Null);
    let ha = check(&result, "ha_webhook_url");
    assert_eq!((&ha["ok"], &ha["detail"], &ha["path"]), (&json!(true), &json!("ha.local:8123 accepted a connection"), &Value::Null));
    let generic = check(&result, "webhook_url");
    assert_eq!((&generic["ok"], &generic["detail"]), (&json!(true), &json!("not set; its notifications are skipped")));
    assert_eq!(h.http().connects(), [("ha.local".to_string(), 8123)]);
    assert!(h.http().posts().is_empty());
    assert!(!result.to_string().contains("hook-sentinel-7f3a"));
    let mut d = Client::open(&h);
    assert!(!d.line().unwrap().contains("hook-sentinel-7f3a"));
    assert!(!h.engine_log().contains("hook-sentinel-7f3a"));
}
```

New, after `a_share_value_of_three_lines_is_refused_without_echoing_it` (only with the owner's yes on T4-R15):

```rust
/// Ruling T4-R15: a webhook value that is not an http or https URL with a host is refused, never echoed, and not
/// stored; a URL is stored.
#[test]
fn a_webhook_value_that_is_not_an_http_url_is_refused_without_echoing_it() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    for (name, value) in [("ha_webhook_url", "ftp://hook-sentinel-1c4b/x"), ("webhook_url", "hook-sentinel-1c4b"),
                          ("webhook_url", "http:///hook-sentinel-1c4b")] {
        let message = c.err("secret_set", json!({"name": name, "value": value}));
        assert_eq!(message, format!("{name} could not be stored: the value must be an http or https URL with a host"));
        assert_eq!(h.secrets().get(name), None, "{name}");
    }
    let mut d = Client::open(&h);
    let line = d.line().unwrap();
    assert!(!line.contains("sentinel"), "{line}");
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["payload"]["secrets"]["webhook_url"], json!({"set": false, "host": null}));
    assert!(!h.engine_log().contains("sentinel"));
    let url = "https://ha.local/api/webhook/hook-sentinel-1c4b";
    assert_eq!(c.ok("secret_set", json!({"name": "webhook_url", "value": url})), Value::Null);
    assert_eq!(h.secrets().get("webhook_url").as_deref(), Some(url));
}
```

- [ ] **Step 3: Run the tests to see them fail.**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked webhook`
Expected: FAIL to compile: `cannot find function 'webhook_check' in this scope` and `cannot find value 'WEBHOOK_NOT_SET' in this scope`.

- [ ] **Step 4: Implement the check.**
  - `pub(crate) fn webhook_check(http: &dyn HttpTransport, name: &str, url: Option<&str>) -> SelfTestCheck` as its doc says: `webhook_target(url)`, then one `http.connect(&target.host, target.port)`, the detail from `host_port(&target)` and `transport_text` (whose `Unrecorded` arm prints what Task 6's connect holds, `<host>:<port>`, never a URL).
  - `fn host_port(target: &WebhookTarget) -> String`: `[<host>]:<port>` when `host` contains `:`, else `<host>:<port>`.
  - `first_selftest`: `others` becomes the eight names followed by `WEBHOOK_SECRETS`; its doc says "the other ten checks" and names T4-R14.
  - `CheckEnv::selftest`: after the `token_check` push, `for name in WEBHOOK_SECRETS { checks.push(webhook_check(self.http.as_ref(), name, self.secrets.get(name).as_deref())); }`; its doc's list ends "the TMDb token and the two webhooks (design part 8.6, Ruling T4-R14)".

- [ ] **Step 5: Implement the value's shape (only with the owner's yes on T4-R15).** In `secret_set`, after the `is_secret_name` refusal and before the store: `if WEBHOOK_SECRETS.contains(&name) && webhook_target(value).is_none() { return Err(format!("{name} could not be stored: {WEBHOOK_VALUE_SHAPE}")); }`. Its doc gains "a webhook name's value only when `webhook_target` reads it (Ruling T4-R15)". `crates/fake-tools/tests/ctl_engine.rs:105`'s `http://ha.local:8123/api/webhook/hook-secret-7f3a` and `engine_ipc.rs:196`'s URL pass unchanged.

- [ ] **Step 6: Run the tests to see them pass.**

Run: `scripts/box.sh cargo test -p lazymkv-engine --locked webhook`, then `scripts/box.sh cargo test -p lazymkv-engine --locked selftest`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc webhook`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc makemkv_check`.
Expected: PASS (`webhook`: the six new unit tests and `ipc_server`'s `a_webhook_secret_shows_only_its_host`; `selftest`: the module's tests; `engine_ipc webhook`: the two new tests, `secrets_show_set_and_only_the_webhook_host` and Task 12's `stop_returns_within_the_drain_bound_while_a_webhook_hangs`, about 15 s; `makemkv_check`: 3). Then `scripts/box.sh cargo test --workspace --locked`: PASS.

Manual on the mini PC: none in this task. The live checks are read at Task 20 step 8 (`ha_webhook_url` "<ha host>:<port> accepted a connection", `webhook_url` "not set; its notifications are skipped"), recorded with the host as `<ha host>`.

- [ ] **Step 7: Commit.**

```bash
git add crates/engine/src/selftest.rs crates/engine/src/owner.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the self-test's webhook checks"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 14.

(On the owner's no to T4-R15, `owner.rs` is left out of `git add`.)

---

### Task 14: lazymkvctl: the 4 MiB check and the token's entry (engine.md "Pull request 4", T4-R18, A6)

**Files:**
- Modify: `crates/ctl/src/engine_cmds.rs:8` (imports `MAX_LINE_BYTES`), `:25-34` (`REQUEST_TOO_LONG` beside `CLOSED`), `:36-48` (`run` passes `exchange_of`'s Ok through `within_line_limit`), `:50-64` (`relay` likewise, its Err through `argument_error` under `argv[0]`), `:206-220` (the doc of `secret`: `tmdb_token` and the two webhook URLs typed on standard input only, A6), `:248-272` (`talk` writes `request_line(kind, &payload)` and `"\n"`), `:381-` (`mod tests`).

**Interfaces:**
- Consumes: `lazymkv_core::ipc::{Envelope, MAX_LINE_BYTES, PROTOCOL_VERSION}` (the engine refuses a line longer than `MAX_LINE_BYTES`, newline excluded, `core::ipc:198`).
- Produces (crate-private):

```rust
/// The argument error of a request whose line, newline excluded, is longer than MAX_LINE_BYTES (engine.md "Pull
/// request 4"; Ruling T4-R18): checked before any connection, so `settings set` and `secret set` exit 2 with this
/// text where the engine would have closed the connection (exit 4). Def: T4-14. Use: T4-14.
pub(crate) const REQUEST_TOO_LONG: &str = "the request is longer than 4 MiB, the most the engine reads in one line";
/// The line `talk` sends for a request, newline excluded: the envelope of `kind` and `payload` with id 1 (T28-c).
fn request_line(kind: &str, payload: &Value) -> String;
/// `exchange` unchanged, or Err(REQUEST_TOO_LONG) when its request's `request_line` is longer than MAX_LINE_BYTES
/// bytes; an exchange that sends nothing passes.
fn within_line_limit(exchange: Exchange) -> Result<Exchange, String>;
```

- Exit codes: 2 for `REQUEST_TOO_LONG`, printed as `usage: lazymkvctl <synopsis>` then `lazymkvctl <name>: error: <REQUEST_TOO_LONG>` for an engine command, and as `lazymkvctl <name>: error: <REQUEST_TOO_LONG>` alone for a relayed form (as its other argument errors). The value of a refused `secret set` is never printed.

- [ ] **Step 1: Write the failing tests** in `engine_cmds.rs`'s `mod tests` (they add `use lazymkv_core::ipc::MAX_LINE_BYTES;` and `use super::REQUEST_TOO_LONG;`):

```rust
#[test]
fn a_settings_file_over_4_mib_is_refused_before_connecting_with_exit_2() {
    let scratch = Scratch::new("ctl-settings-long");
    let file = scratch.write_path("big.json", &format!("{{\"staging_root\": \"{}\"}}", "a".repeat(MAX_LINE_BYTES)));
    let connector = MemoryConnector::failing(ConnectError::NotRunning);
    let (code, out, err) = call(&["settings", "set", file.to_str().unwrap()], Some(&connector), b"", &scratch.0);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, [
        "usage: lazymkvctl settings get | settings set FILE".to_string(),
        format!("lazymkvctl settings: error: {REQUEST_TOO_LONG}"),
        String::new(),
    ].join("\n"));
    assert_eq!(REQUEST_TOO_LONG, "the request is longer than 4 MiB, the most the engine reads in one line");
}

#[test]
fn a_secret_value_over_4_mib_is_refused_without_echoing_it() {
    let scratch = Scratch::new("ctl-secret-long");
    let value = "token-sentinel-5d2e".repeat(MAX_LINE_BYTES / 19 + 1);
    let connector = MemoryConnector::failing(ConnectError::NotRunning);
    let (code, out, err) = call(&["secret", "set", "tmdb_token"], Some(&connector), value.as_bytes(), &scratch.0);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, [
        "usage: lazymkvctl secret set NAME".to_string(),
        format!("lazymkvctl secret: error: {REQUEST_TOO_LONG}"),
        String::new(),
    ].join("\n"));
    assert!(!err.contains("token-sentinel-5d2e"));
}

#[test]
fn a_relayed_request_over_4_mib_is_refused_before_connecting() {
    let scratch = Scratch::new("ctl-relay-long");
    let job = "a".repeat(MAX_LINE_BYTES);
    let connector = MemoryConnector::failing(ConnectError::NotRunning);
    let (code, out, err) = call(&["plan", "--job", &job], Some(&connector), b"", &scratch.0);
    assert_eq!((code, out.as_str(), err), (2, "", format!("lazymkvctl plan: error: {REQUEST_TOO_LONG}\n")));
}

/// The engine counts a line without its newline (`core::ipc`, MAX_LINE_BYTES), so a line of exactly MAX_LINE_BYTES
/// bytes is sent and one byte more is refused.
#[test]
fn a_request_of_exactly_the_limit_is_sent() {
    let scratch = Scratch::new("ctl-limit");
    // preserve_order keeps the envelope's field order and the payload's insertion order.
    let empty = r#"{"v":1,"type":"secret_set","id":1,"payload":{"name":"tmdb_token","value":""}}"#;
    let value = "a".repeat(MAX_LINE_BYTES - empty.len());
    let (ipc, connector) = MemoryIpc::new();
    let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![reply_ok(Some(1), Value::Null)]));
    let (code, out, err) = call(&["secret", "set", "tmdb_token"], Some(&connector), value.as_bytes(), &scratch.0);
    let line = finish(server, connector);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    assert_eq!(line.trim_end_matches('\n').len(), MAX_LINE_BYTES);
    let longer = format!("{value}a");
    let refusing = MemoryConnector::failing(ConnectError::NotRunning);
    let (code, _, err) = call(&["secret", "set", "tmdb_token"], Some(&refusing), longer.as_bytes(), &scratch.0);
    assert_eq!(code, 2);
    assert!(err.ends_with(&format!("lazymkvctl secret: error: {REQUEST_TOO_LONG}\n")), "{err}");
}

/// A6: the owner types the TMDb token on standard input; lazymkvctl sends it once in `secret_set` and prints nothing.
#[test]
fn secret_set_tmdb_token_reads_stdin_and_sends_it_once() {
    let scratch = Scratch::new("ctl-token");
    let (ipc, connector) = MemoryIpc::new();
    let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![reply_ok(Some(1), Value::Null)]));
    let (code, out, err) = call(&["secret", "set", "tmdb_token"], Some(&connector), b"token-sentinel-5d2e\r\n", &scratch.0);
    let line = finish(server, connector);
    assert_eq!(request_of(&line), json!({"v": 1, "type": "secret_set", "id": 1,
                                          "payload": {"name": "tmdb_token", "value": "token-sentinel-5d2e"}}));
    assert_eq!(line.matches("token-sentinel-5d2e").count(), 1);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
}
```

- [ ] **Step 2: Run the tests to see them fail.**

Run: `scripts/box.sh cargo test -p lazymkv-ctl --locked 4_mib`
Expected: FAIL to compile: `unresolved import 'super::REQUEST_TOO_LONG'`. (With the constant alone in place, the three `4_mib` tests fail with exit 4 and the `NOT_RUNNING` message of the failing connector, and the limit test's second half likewise.)

- [ ] **Step 3: Implement.**
  - `REQUEST_TOO_LONG`, `request_line` and `within_line_limit` with the signatures and docs above; `request_line` builds `Envelope { v: PROTOCOL_VERSION, kind: kind.to_string(), id: Some(1), payload: payload.clone() }` and serialises it with `serde_json::to_string`, the call `talk` makes today (`:266-267`), so the measured line is the sent line.
  - `run`: `match exchange_of(name, &argv[1..], env.ctx, &mut *env.stdin).and_then(within_line_limit)`; the Err arm unchanged (usage line, then `argument_error`).
  - `relay`: the exchange it builds goes through `within_line_limit`; Err gives `argument_error(env.err, &argv[0], &message)`.
  - `talk`: `let line = request_line(kind, &payload) + "\n";`.
  - The docs: `exchange_of`'s sentence "Every check, the settings file read and the standard-input read happen here, before any connection (T28-h)" gains "the line's length too (Ruling T4-R18)"; `secret`'s doc becomes: "`secret set NAME` sends NAME and standard input's value (T28-f) as `secret_set`. The owner types `tmdb_token` (the TMDb token the lookups and the self-test's token check send, design part 8.1) and the two webhook URLs this way only, never as an argument, at the elevated console (design part 9.6, A6); the value is never printed, and a refusal never holds it."

- [ ] **Step 4: Run the tests to see them pass.**

Run: `scripts/box.sh cargo test -p lazymkv-ctl --locked 4_mib`, then `scripts/box.sh cargo test -p lazymkv-ctl --locked limit`, then `scripts/box.sh cargo test -p lazymkv-ctl --locked secret_set`.
Expected: PASS (the five new tests; `secret_set_sends_stdins_text_without_its_last_newline` and `engine_command_argument_errors_exit_2_without_connecting` unchanged and passing). Then `scripts/box.sh cargo test --workspace --locked`: PASS (`crates/fake-tools/tests/ctl_engine.rs` unchanged).

Owner, at Task 20 step 7 (A6, Owner prerequisites items 6 and 15): the owner alone types the token into `C:\lazymkv\run\lazymkvctl.exe secret set tmdb_token` at the mini PC's elevated console (recommended: `$env:TMDB_TOKEN | C:\lazymkv\run\lazymkvctl.exe secret set tmdb_token` in PowerShell, which shows nothing), and `secret set ha_webhook_url` the same way. Evidence for engine.md: the exit codes (0 each) and `status`'s `tmdb_token` `"set": true`, `ha_webhook_url` `"set": true, "host": "<ha host>"`; never a value.

- [ ] **Step 5: Commit.**

```bash
git add crates/ctl/src/engine_cmds.rs
git commit -m "ctl: a request over 4 MiB is refused before connecting"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 15.

---

### Task 15: dev's lookup readers aligned with S46 and the search words (T16-r1, T4-R17)

Precondition: `decisions.txt` holds T4-R17's answer (Task 0 Step 1).

**Files:**
- Modify: `crates/dev/src/cmd/plan.rs:37` (imports: `lookup as lookup_answer` dropped), `:144-152` (`Found` gains `other_tmdb_pending`), `:154-173` (`plan_doc`'s doc: S46's reading and the other TMDb words), `:210-242` (the two `tmdb_pending` expressions), `:287-366` (`recording_inputs`: `discdb_answer` for both TheDiscDB files; `other_tmdb_pending`), `:385-395` (`scan_file_inputs`: `other_tmdb_pending: false`), `:504-515` (`discdb_answer` beside `tmdb_answer`), `:648-` (`mod tests`).
- Modify: `crates/dev/src/cmd/hit_rate.rs:32-51` (`lookup` reads TheDiscDB answers with the S46 reader `parse_dischash`, through `plan::discdb_answer`; `Ok(None)` is a miss; T4-R17 overturned 2026-11-04), `:63-64` (`report`'s two calls), `:160-167` (the not-a-disc test renamed, Step 1).

**Interfaces:**
- Consumes: `lazymkv_core::lookup::discdb::{parse_dischash, parse_discid}`; `hit_rate::{statuses, FAILED, HIT, MISS, NOT_MADE}`; in tests, Tasks 2 to 5's `lazymkv_clients::{run_round, ChainInputs, LookupBook, RecordedTransport, SavedFile, dischash_url, discid_url, search_url, record_url}` (the `dev -> clients` edge of Task 1).
- Produces:

```rust
/// TheDiscDB's answer word for `path` as the engine's book reads it (S46; T16-r1; Ruling T4-R17): no status "n/a";
/// 404 "miss"; any status but 200 (`000`, `unparsed`, `429`, `503`) "failed"; 200 with the file unread "failed"; 200
/// read by `parse_discid` when `discid`, else by `parse_dischash`: a record (a non-empty disc ID list) "hit", none
/// (an empty body, `[]`, `{"errors": ..}`, `{"titles": []}`) "miss", a parse error "failed". Def: T4-15. Use: T4-15, T4-16.
pub fn discdb_answer(tree: &dyn Tree, path: &PathKey, status: Option<&str>, discid: bool) -> &'static str;

// crates/dev/src/cmd/hit_rate.rs (its signature gains `discid`; T4-R17 overturned 2026-11-04)
/// `hit`, `miss`, `failed` or `n/a` for the answer file `path` given the lookup's HTTP status, read as S46 reads it
/// (`plan::discdb_answer`; Ruling T4-R17, overturned by the owner on 2026-11-04, so a dischash 200 that is no disc
/// record is a miss, where the Python read it as failed). Def: T4-15. Use: T4-15.
pub fn lookup(tree: &dyn Tree, path: &PathKey, status: Option<&str>, discid: bool) -> &'static str;
```

- `Found.other_tmdb_pending: bool` (private): true when any `lookup-status.txt` key is `search`, `search-tagged`, `search-entry` or starts `season-`, and its word is neither `200` nor `404` (T4-R1's keys; the book's `waiting`). The record's key `tmdb` keeps `tmdb_answer`.
- `plan_doc`'s flag: `tmdb_pending: (found.tmdb_pending && given_record.is_none()) || found.other_tmdb_pending`, and G3's second plan `found.tmdb_pending || found.other_tmdb_pending`. SR8 is unchanged for the record: a given `--tmdb-record` clears the record's failure only, and neither `--tmdb-search` nor `--tmdb-record` clears another TMDb lookup's (a refinement of T4-R17 recorded with it in engine.md by Task 21: the engine's replay gives `tmdb-record.json` while a search or season can still be pending in the book, and the two must agree).

- [ ] **Step 1: Write the failing tests** in `plan.rs`'s `mod tests` (helpers `film_recording`, `film_scan`, `bdmt_xml`, `planned`, `doc_of`, `found`, and `META`, `DISCHASH`, `FINGERPRINT`, `DISC_ID` exist at `:664-800`; the module's imports gain `use lazymkv_clients::{dischash_url, discid_url, record_url, run_round, search_url, ChainInputs, LookupBook, RecordedTransport, SavedFile, TransportError};`):

```rust
const PENDING_TMDB: &str = "the TMDb lookup is pending";

fn rec_key(tmp: &Scratch, file: &str) -> (Recording, PathKey) {
    let rec = resolve_recording(&Ctx::for_test(&tmp.0), None, "rec").unwrap();
    let key = rec.dir.join(file);
    (rec, key)
}

#[test]
fn a_dischash_200_that_is_no_disc_is_a_miss_for_plan() {
    let tmp = Scratch::new("plan-s46-dischash");
    film_recording(&tmp, "rec");
    let cases: [(&str, Option<&str>, &str); 9] = [
        (r#"{"errors": "x"}"#, Some("200"), "miss"), (r#"{"titles": []}"#, Some("200"), "miss"), ("", Some("200"), "miss"),
        (DISCHASH, Some("200"), "hit"), ("not json", Some("200"), "failed"), (DISCHASH, Some("404"), "miss"),
        (DISCHASH, None, "n/a"), (DISCHASH, Some("000"), "failed"), (DISCHASH, Some("unparsed"), "failed"),
    ];
    for (body, status, word) in cases {
        tmp.write("rec/thediscdb-hash.json", body);
        let (rec, key) = rec_key(&tmp, "thediscdb-hash.json");
        assert_eq!(discdb_answer(&rec.tree, &key, status, false), word, "{body:?} {status:?}");
    }
    let (rec, absent) = rec_key(&tmp, "absent.json");
    assert_eq!(discdb_answer(&rec.tree, &absent, Some("200"), false), "failed");
    // Through plan: S46's miss leaves TheDiscDB answered, not pending.
    tmp.write("rec/thediscdb-hash.json", r#"{"errors": "x"}"#);
    let doc = planned(&tmp, &PlanOptions::default());
    assert_eq!(doc.inputs.discdb_hash, found("rec/thediscdb-hash.json", "miss"));
    assert!(!doc.plan.identity.lookup_pending, "{:?}", doc.plan.identity.evidence);
}

#[test]
fn an_empty_discid_200_is_a_miss_for_plan() {
    let tmp = Scratch::new("plan-s46-discid");
    film_recording(&tmp, "rec");
    for (body, word) in [("", "miss"), ("[]", "miss"), (r#"{"x": 1}"#, "failed")] {
        tmp.write("rec/thediscdb-discid.json", body);
        let (rec, key) = rec_key(&tmp, "thediscdb-discid.json");
        assert_eq!(discdb_answer(&rec.tree, &key, Some("200"), true), word, "{body:?}");
    }
    tmp.write("rec/thediscdb-discid.json", &format!("[{DISCHASH}]"));
    let (rec, key) = rec_key(&tmp, "thediscdb-discid.json");
    assert_eq!(discdb_answer(&rec.tree, &key, Some("200"), true), "hit");
    tmp.write("rec/thediscdb-discid.json", "");
    tmp.write("rec/lookup-status.txt", "dischash 200\ndiscid 200\ntmdb movie/6537 200\n");
    let doc = planned(&tmp, &PlanOptions::default());
    assert_eq!(doc.inputs.discdb_discid, found("rec/thediscdb-discid.json", "miss"));
    assert!(!doc.plan.identity.lookup_pending, "{:?}", doc.plan.identity.evidence);
}

#[test]
fn a_pending_search_word_makes_tmdb_pending_for_plan() {
    let tmp = Scratch::new("plan-search-words");
    film_recording(&tmp, "rec");
    for (line, pending) in [("search 000", true), ("search-tagged 503", true), ("search-entry unparsed", true),
                            ("search 429", true), ("search 404", false), ("search 200", false)] {
        tmp.write("rec/lookup-status.txt", &format!("dischash 200\ndiscid 404\ntmdb movie/6537 200\n{line}\n"));
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.inputs.tmdb, found("rec/tmdb.json", "hit"), "{line}");
        assert_eq!(doc.plan.identity.lookup_pending, pending, "{line}");
        assert_eq!(doc.plan.identity.evidence.iter().any(|s| s == PENDING_TMDB), pending, "{line}");
    }
}

#[test]
fn a_pending_season_word_makes_tmdb_pending_for_plan() {
    let tmp = Scratch::new("plan-season-words");
    film_recording(&tmp, "rec");
    for (line, pending) in [("season-2 000", true), ("season-1 500", true), ("season-7 404", false), ("season-1 200", false)] {
        tmp.write("rec/lookup-status.txt", &format!("dischash 200\ndiscid 404\ntmdb movie/6537 200\n{line}\n"));
        assert_eq!(planned(&tmp, &PlanOptions::default()).plan.identity.lookup_pending, pending, "{line}");
    }
}

/// SR8 unchanged: a given search page or record answers its own lookup only, never another TMDb lookup's.
#[test]
fn a_given_tmdb_search_file_does_not_clear_a_pending_search() {
    let tmp = Scratch::new("plan-search-given");
    film_recording(&tmp, "rec");
    tmp.write("rec/lookup-status.txt", "dischash 200\ndiscid 404\ntmdb movie/6537 200\nsearch 000\n");
    tmp.write("search.json", search_json(&[("movie", 6537, "Made Up Film", "Pelicula Inventada", "2007-05-20")]));
    tmp.write("movie.json", tmdb_movie_json(6537, "Made Up Film", "Pelicula Inventada", "2007-05-20", "es", Some(100)));
    let search = PlanOptions { tmdb_search: Some(PathBuf::from("search.json")), ..PlanOptions::default() };
    assert!(planned(&tmp, &search).plan.identity.lookup_pending);
    let both = PlanOptions { tmdb_record: Some(PathBuf::from("movie.json")), ..search };
    let doc = planned(&tmp, &both);
    assert!(doc.plan.identity.lookup_pending);
    assert!(doc.plan.identity.evidence.iter().any(|s| s == PENDING_TMDB), "{:?}", doc.plan.identity.evidence);
}
```

The equivalence test. Each case answers the chain's URLs on a `RecordedTransport` (`Fail` uses `fail`), runs one `run_round` with the token `Some("token-sentinel-5d2e")` and today `2026-10-13` (`Ctx::for_test`'s date), saves every `SavedFile` into `rec/`, then compares dev's flags and plan with the book:

```rust
enum Ans { Status(u16, String), Fail(TransportError) }

/// Design part 7.1, T16-r1: dev reads a folder the chain wrote as the book reads itself.
#[test]
fn plan_over_an_engine_job_folder_flags_lookups_as_the_book_does() {
    let name = disc_name(Some("Made Up Film"), Some("MADE_UP_FILM")).unwrap();
    let record = tmdb_movie_json(6537, "Made Up Film", "Pelicula Inventada", "2007-05-20", "es", Some(100));
    let page = search_json(&[("movie", 6537, "Made Up Film", "Pelicula Inventada", "2007-05-20")]);
    let empty = search_json(&[]);
    let ok = |body: &str| Ans::Status(200, body.to_string());
    // (case, dischash, search, record, the book's pending)
    let cases = [
        ("all answered", ok(DISCHASH), ok(&page), ok(&record), (false, false)),
        ("S46 miss", ok(r#"{"errors": "x"}"#), ok(&empty), ok(&record), (false, false)),
        ("TheDiscDB 503", Ans::Status(503, String::new()), ok(&empty), ok(&record), (true, false)),
        ("search timeout", ok(DISCHASH), Ans::Fail(TransportError::Timeout), ok(&record), (false, true)),
        ("search 429", ok(DISCHASH), Ans::Status(429, String::new()), ok(&record), (false, true)),
        ("record 500", ok(DISCHASH), ok(&page), Ans::Status(500, String::new()), (false, true)),
    ];
    for (case, hash, search, rec_answer, expected) in cases {
        let tmp = Scratch::new("plan-book-equivalence");
        tmp.write("rec/scan.txt", film_scan(FILM_STREAMS));
        tmp.write("rec/meta.json", META);
        tmp.write(&format!("rec/{BDMT_DIR}/bdmt_eng.xml"), bdmt_xml("Made Up Film"));
        let http = RecordedTransport::new();
        let identity = TmdbIdentity { kind: TmdbKind::Movie, id: 6537 };
        for (url, answer) in [(dischash_url(FINGERPRINT), hash), (search_url(&name.cleaned), search), (record_url(identity), rec_answer)] {
            match answer { Ans::Status(code, body) => http.answer(&url, code, body.as_bytes()), Ans::Fail(error) => http.fail(&url, error) }
        }
        http.answer(&discid_url(DISC_ID), 404, b"");
        let mut book = LookupBook::default();
        let inputs = ChainInputs { fingerprint: FINGERPRINT.to_string(), disc_id: Some(DISC_ID.to_string()),
                                   name: Some(name.clone()), remembered: None };
        let dir = tmp.0.join("rec");
        run_round(&http, Some("token-sentinel-5d2e"), &mut book, &inputs, "2026-10-13",
                  &mut |file: SavedFile| fs::write(dir.join(&file.name), &file.bytes).unwrap());
        assert_eq!(book.pending(), expected, "{case}: the case's own premise");
        let (rec, _) = rec_key(&tmp, "scan.txt");
        let found = recording_inputs(&rec).unwrap_or_else(|refused| panic!("{case}: {}", refused.line));
        assert_eq!((found.discdb_pending, found.tmdb_pending || found.other_tmdb_pending), expected, "{case}");
        // The engine's replay (file_phase::replay_options): the folder's tmdb-search.json and tmdb-record.json given.
        let given = |file: &str| Some(dir.join(file)).filter(|path| path.is_file());
        let replay = PlanOptions { tmdb_search: given("tmdb-search.json"), tmdb_record: given("tmdb-record.json"), ..PlanOptions::default() };
        let doc = doc_of(&tmp, "rec", &replay).unwrap_or_else(|refused| panic!("{case}: {}", refused.line));
        assert_eq!(doc.plan.identity.lookup_pending, expected.0 || expected.1, "{case}");
    }
}
```

In `hit_rate.rs`'s `mod tests`, `hit_rate_a_200_answer_that_is_not_a_disc_record_is_a_failure` (`hit_rate.rs:160-167`) is renamed and reads S46's miss (T4-R17 overturned 2026-11-04; a lone `hash=miss` with no disc ID line is answered, so the exit code is 0, `hit_rate.rs:84`):

```rust
#[test]
fn hit_rate_a_200_answer_that_is_not_a_disc_record_is_a_miss() {
    let tmp = TempDir::new("not-a-disc");
    disc(&tmp, "a", "dischash 200\n", Some(r#"{"errors": "x"}"#), None);
    let (code, out, _) = quiet(&tmp.0);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("hash=miss"), "{out}");
}
```

The other bodies of `hit_rate.rs`'s tests are read by the S46 readers as those tests mean them, so they stay unchanged: `parse_dischash` accepts `{"titles": [{"index": 0}]}` as a record (`hit`), `parse_discid` reads `[{"titles": []}]` as a non-empty list (`hit`) and `[]` as `miss`, and `not json` stays `failed` (disc `b` of `hit_rate_a_failed_lookup_is_not_a_miss`).

- [ ] **Step 2: Run the tests to see them fail.**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked plan::`
Expected: FAIL to compile: `cannot find function 'discdb_answer' in this scope` and `no field 'other_tmdb_pending' on type 'Found'`.

- [ ] **Step 3: Implement.**
  - `discdb_answer` with the signature and doc above, beside `tmdb_answer`: the status match of `tmdb_answer`'s shape, then `read_file_text` of `path` (an unread file is FAILED), then `parse_discid` or `parse_dischash` per `discid`, each `Err` FAILED.
  - `recording_inputs`: `discdb_hash.answer` from `discdb_answer(tree, &hash_key, status("dischash"), false)`, `discdb_discid.answer` from `discdb_answer(tree, &discid_key, status("discid"), true)`; `discdb_pending` keeps its expression over the two words; `other_tmdb_pending = lookup_status.iter().any(|(key, word)| (matches!(key.as_str(), "search" | "search-tagged" | "search-entry") || key.starts_with("season-")) && word != "200" && word != "404")`.
  - `plan_doc`: the two flag expressions of the Interfaces block; the SR8 comment names T4-R17.
  - `plan_doc`'s doc: "lookup-status.txt (`hit_rate::statuses`), TheDiscDB's two files read by `discdb_answer` (S46: a 200 that names no disc is a miss), and the other TMDb lookups' words (`search`, `search-tagged`, `search-entry`, `season-<n>`), each pending when present and neither 200 nor 404, whatever file is given (Ruling T4-R17)".
  - `hit_rate::lookup` (T4-R17 overturned 2026-11-04): its signature gains `discid: bool` and its body becomes `crate::cmd::plan::discdb_answer(tree, path, status, discid)`, so it reads TheDiscDB answers with the S46 readers (`parse_dischash` Ok(None) gives `miss`, an empty discid 200 gives `miss`, a parse error stays `failed`); `report` passes `false` for `thediscdb-hash.json` and `true` for `thediscdb-discid.json`. Its doc: "`hit`, `miss`, `failed` or `n/a` for the answer file `path` given the lookup's HTTP status, read as S46 reads it (`plan::discdb_answer`; Ruling T4-R17, overturned by the owner on 2026-11-04, so a dischash 200 that is no disc record is a miss, where the Python read it as failed). Def: T4-15. Use: T4-15." (the old doc's `Py: hit_rate.lookup.` ending is dropped; the doc and signature are the Produces block's). A changed `hit-rate` line in the junction run goes to the owner by test name only (K5) and is re-pinned with the owner's yes.

- [ ] **Step 4: Run the tests to see them pass.**

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked plan::`
Expected: PASS (the six new tests, and the existing `a_failed_lookup_is_planned_as_pending`, `an_error_body_of_a_failed_or_missed_lookup_is_not_parsed`, `the_tmdb_answer_word_follows_the_lookup_status`).

Run: `scripts/box.sh cargo test -p lazymkv-dev --locked hit_rate`
Expected: PASS (`hit_rate_a_200_answer_that_is_not_a_disc_record_is_a_miss` and the module's other tests unchanged).

Then the junction workspace run (Global Constraints):
`scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`
Expected: PASS, `hit-rate`'s pinned lines (`crates/dev/tests/readers.rs`) re-pinned where they change (T4-R17 overturned 2026-11-04), each change reported to the owner by test name only (K5) before it is re-pinned. Manual on the mini PC (controller): if a `plan` golden changes because a recording in the clone holds a dischash 200 that is no disc (its word goes `failed` to `miss`), the task stops and the controller reports to the owner the golden's name and the changed key paths only (K5), never a value from the clone (RP21); nothing is regenerated without the owner's yes.

- [ ] **Step 5: Commit.**

```bash
git add crates/dev/src/cmd/plan.rs crates/dev/src/cmd/hit_rate.rs
git commit -m "dev: plan reads TheDiscDB as S46 does and the search words"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 16.

---

### Task 16: `lazymkv-dev census` (A9)

Precondition: `decisions.txt` holds item 24's answer (Task 0 Step 1).

**Files:**
- Create: `crates/dev/src/cmd/census.rs` (the command, its pure row reader and its `mod tests`).
- Modify: `crates/dev/src/cmd/mod.rs` (`pub mod census;` after `pub mod plan;`).
- Modify: `crates/dev/src/lib.rs:24-44` (`COMMANDS` gains `("census", cmd::census::run as Command, "[--evidence DIR]")` as its last row, after `golden`, so the six readers' run that `the_six_readers_are_registered_in_spec_order` pins (`crates/dev/tests/readers.rs:101-119`) is untouched).
- Modify: `crates/core/src/subtitle/labels.rs:381` (`mod row14;` becomes `pub mod row14;`).
- Modify: `crates/core/src/subtitle/labels/row14.rs:261` (`enum Media` becomes `pub enum Media`), `:430` (`fn media` becomes `pub fn media`), `:454` (`fn recorded_discdb` becomes `pub fn recorded_discdb`). No behaviour change; each doc keeps its text and ends "Def: <its existing spec citation, e.g. Spec 17 row 14 (a), 8.2>; T4-16 makes it public. Use: T4-16."

**Interfaces:**
- Consumes: `evidence::{resolve_evidence, DiskTree, EVIDENCE_SHOWN, Refused}`, `crate::{Args, usage_error}`, `hit_rate::statuses`, Task 15's `cmd::plan::discdb_answer`, `lookup::tmdb::parse_recorded`, `DiscDbRecord::title_and_year`, `row14::{media, recorded_discdb, Media}`, `disc::disc`, `robot::parse`, `recording::{read_text, text_mode, read_json, Encoding, Errors}`, `recording::files::Meta`, `Title::video_height`, `Flags::derived_forced_only`, `Codec::object` (`codec.rs:140`), `ObjectAudio::as_str`, `lookup::Kind`, `Settings::default()`.
- Produces (Ruling T4-R22):

```rust
// crates/core/src/subtitle/labels/row14.rs (made public; no behaviour change)
// each doc keeps its text and ends "Def: <its existing spec citation, e.g. Spec 17 row 14 (a), 8.2>; T4-16 makes it public. Use: T4-16."
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Media { Film, Tv, Unknown }
pub fn media(evidence: &dyn Tree, dir: &PathKey, typed: Option<&Disc>, settings: &Settings) -> (Media, &'static str);
pub fn recorded_discdb(evidence: &dyn Tree, dir: &PathKey) -> Option<DiscDbRecord>;

// crates/dev/src/cmd/census.rs (decisions.md A9; Ruling T4-R22)
pub const USAGE: &str = "census [--evidence DIR]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum DiscType { Uhd, BluRay, Dvd }
impl DiscType { pub fn as_str(self) -> &'static str; } // "UHD", "Blu-ray", "DVD"
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CensusRow {
    pub slug: String,                    // the folder's name, as hit-rate prints it
    pub title: Option<String>,           // tmdb.json's title, else TheDiscDB's media title
    pub year: Option<i32>,
    pub kind: Option<Kind>,              // Media::Film -> Movie, Media::Tv -> Tv, Media::Unknown -> None
    pub discdb: &'static str,            // "hit" | "miss" | "failed" | "n/a"
    pub english_subtitles: Option<usize>,// None when the recording has no scan.txt
    pub object_audio: Vec<ObjectAudio>,  // Atmos before DTS:X, each once
    pub disc_type: Option<DiscType>,
}
pub fn main_title(disc: &Disc) -> Option<&Title>;      // longest by seconds, the lower number on a tie
pub fn census_row(tree: &dyn Tree, disc: &PathKey) -> Result<CensusRow, String>;
pub fn line(row: &CensusRow) -> String;                  // the printed line, without its newline
pub fn report(tree: &dyn Tree, root: &PathKey, out: &mut dyn Write) -> Result<i32, String>;
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32;
```

Each `pub` item of `census.rs` carries a doc naming decisions.md A9 and Ruling T4-R22 and ends "Def: T4-16. Use: T4-16." (`run` ends "Use: T4-16, T4-20.", since Task 20 step 11a may run it).

The rules the signatures do not fix (Ruling T4-R22):
- Recordings: every entry of `tree.list(root)` holding `meta.json`, in `PathKey` order (hit-rate's rule, `hit_rate.rs:57-60`).
- `discdb`: `discdb_answer(tree, thediscdb-hash.json, status["dischash"], false)` and `discdb_answer(tree, thediscdb-discid.json, status["discid"], true)`; `hit` when either is `hit`; else `miss` when the first is `miss` and the second `miss` or `n/a`; else `failed` when either is `failed`; else `n/a`.
- `title`, `year`: `tmdb.json` through `parse_recorded` when the file exists and the status line keyed `tmdb` is `200` or absent; else `recorded_discdb(..).and_then(|r| r.title_and_year())`; else None.
- `kind`: `media(tree, disc, typed.as_ref(), &Settings::default())`.
- Scan: `read_text(tree, scan.txt, Encoding::Utf8, Errors::Replace)`, then `disc(&parse(&text_mode(&text)))` as `plan.rs:518-520` does; no `scan.txt`: `english_subtitles` None, `object_audio` empty, `kind` from `tmdb.json` only.
- English count: in `main_title`, streams of `StreamKind::Subtitles` with `language == Some("eng")` and `!flags.derived_forced_only()`.
- `object_audio`: read from `main_title`'s audio streams (`StreamKind::Audio`): the distinct `codec.object` values (`Codec::object`, `codec.rs:140`) of its audio tracks, Atmos before DTS:X, each once; empty when none has one (the `line` rule prints `-`) and empty without a scan (printed `?`).
- `disc_type`: `meta.json`'s `format` `dvd` gives `Dvd`; `bluray` gives `Uhd` when `main_title`'s `video_height()` is 2000 or more, else `BluRay`, and None without a scan; any other format None. TheDiscDB's `format` is never read.
- `line`: `format!("{:<40} {:<7} {:<5} discdb={:<6} eng-subs={:<2} object={:<11} {}", slug, type, kind, discdb, subs, object, title)` with `type` `as_str()` or `?`; `kind` `movie`, `tv` or `?`; `subs` the count or `?`; `object` `?` when there is no scan, `-` when empty, else the names joined with `+` (`Atmos+DTS:X`); `title` `<title> (<year>)`, `<title>`, or `?`.
- Errors (exit 2, hit-rate's crash rule): `meta.json` that does not parse, `lookup-status.txt` that is not UTF-8, a `scan.txt` that `disc` refuses, a read `tmdb.json` that `parse_recorded` refuses; each `Err` text names the file through `tree.display`. `run` prints it on `err` and returns 2 after the lines already written. No summary line follows the rows. `census` never fetches and never exits 3 for a stale copy (Owner decision 24); it exits 3 only through `resolve_evidence`.

- [ ] **Step 1: Write the failing tests** in `crates/dev/src/cmd/census.rs`'s `mod tests`. The recordings are built at test time from public files (T4-R22); no test reads `corpus/owner` (A9, RP21).

```rust
use super::{census_row, main_title, run};
use crate::evidence::{Ctx, EVIDENCE_URL};
use crate::testutil::{call, printed, Scratch, ScratchExt};
use lazymkv_core::disc::disc;
use lazymkv_core::recording::{text_mode, PathKey};
use lazymkv_core::robot::parse;
use std::path::Path;

const SMILE_SCAN: &str = "corpus/thediscdb/movie/Smile (2022)/2022-4k/disc01.txt";
const KNOWING_SCAN: &str = "corpus/thediscdb/movie/Knowing (2009)/2010-dvd/disc01.txt";

/// A public file of the repository as text (RP21's public list, crates/dev/tests/clone_values.rs).
fn public(relative: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative)).unwrap()
}

/// The five recordings of Ruling T4-R22, built from public files.
fn public_recordings(tmp: &Scratch) {
    tmp.write("disc-a/meta.json", r#"{"format": "bluray"}"#);
    tmp.write("disc-a/lookup-status.txt", "dischash 404\n");
    tmp.write("disc-a/thediscdb-hash.json", "");
    tmp.write("disc-a/scan.txt", &public("corpus/scenarios/disc-a-scan/scan.txt"));
    tmp.write("smile/meta.json", r#"{"format": "bluray"}"#);
    tmp.write("smile/lookup-status.txt", "dischash 200\n");
    tmp.write("smile/thediscdb-hash.json", &public("corpus/thediscdb-api/dischash-E826F0C64B04079EA83094AC046E8709.json"));
    tmp.write("smile/scan.txt", &public(SMILE_SCAN));
    tmp.write("knowing/meta.json", r#"{"format": "dvd"}"#);
    tmp.write("knowing/lookup-status.txt", "dischash 200\n");
    tmp.write("knowing/thediscdb-hash.json", &public("corpus/thediscdb-api/dischash-631BD77872C157D7C436E6F48F06A478.json"));
    tmp.write("knowing/scan.txt", &public(KNOWING_SCAN));
    tmp.write("synthetic-movie/meta.json", r#"{"format": "bluray"}"#);
    tmp.write("synthetic-movie/lookup-status.txt", "dischash 404\ndiscid 404\ntmdb movie/900001 200\n");
    tmp.write("synthetic-movie/tmdb.json", r#"{"id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie", "release_date": "2001-02-03", "original_language": "en", "external_ids": {"imdb_id": "tt0900001"}, "fetched_at": "2026-10-01"}"#);
    tmp.write("synthetic-movie/scan.txt", &public("corpus/scenarios/movie-direct/scan.txt"));
    tmp.write("not-a-disc/meta.json", r#"{"format": "dvd"}"#);
    tmp.write("not-a-disc/lookup-status.txt", "dischash 200\n");
    tmp.write("not-a-disc/thediscdb-hash.json", r#"{"errors": "x"}"#);
    tmp.write("no-meta/scan.txt", &public("corpus/scenarios/movie-direct/scan.txt"));
    tmp.write("a-file.txt", "not a recording");
}

fn census(dir: &Path) -> (i32, String, String) {
    call(run, &Ctx::for_test(dir), &["--evidence", &dir.to_string_lossy()])
}

#[test]
fn census_lists_each_public_recording_with_its_columns() {
    let tmp = Scratch::new("census-public");
    public_recordings(&tmp);
    let (code, out, err) = census(&tmp.0);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, printed(&[
        "disc-a                                   UHD     movie discdb=miss   eng-subs=1  object=-           ?",
        "knowing                                  DVD     movie discdb=hit    eng-subs=2  object=-           Knowing (2009)",
        "not-a-disc                               DVD     ?     discdb=miss   eng-subs=?  object=?           ?",
        "smile                                    UHD     movie discdb=hit    eng-subs=3  object=Atmos       Smile (2022)",
        "synthetic-movie                          Blu-ray movie discdb=miss   eng-subs=0  object=-           Synthetic Movie (2001)",
    ]));
}

#[test]
fn census_reads_a_dischash_200_that_is_no_disc_as_a_miss() {
    let tmp = Scratch::new("census-s46");
    for (name, body) in [("empty", ""), ("errors", r#"{"errors": "x"}"#), ("no-titles", r#"{"titles": []}"#)] {
        tmp.write(&format!("{name}/meta.json"), r#"{"format": "dvd"}"#);
        tmp.write(&format!("{name}/lookup-status.txt"), "dischash 200\n");
        tmp.write(&format!("{name}/thediscdb-hash.json"), body);
    }
    tmp.write("broken/meta.json", r#"{"format": "dvd"}"#);
    tmp.write("broken/lookup-status.txt", "dischash 200\n");
    tmp.write("broken/thediscdb-hash.json", "not json");
    let tree = crate::evidence::DiskTree::open(&tmp.0, "evidence").unwrap();
    let word = |name: &str| census_row(&tree, &PathKey::root().join(name)).unwrap().discdb;
    assert_eq!([word("empty"), word("errors"), word("no-titles"), word("broken")], ["miss", "miss", "miss", "failed"]);
}

#[test]
fn census_takes_the_longest_title_and_the_lower_number_on_a_tie() {
    let smile = disc(&parse(&text_mode(&public(SMILE_SCAN)))).unwrap();
    assert_eq!(main_title(&smile).map(|title| title.number), Some(14)); // 14 and 122 both run 1:55:37
    let knowing = disc(&parse(&text_mode(&public(KNOWING_SCAN)))).unwrap();
    assert_eq!(main_title(&knowing).map(|title| title.number), Some(0));
}

#[test]
fn census_counts_english_subtitles_without_derived_forced_only_streams() {
    let tmp = Scratch::new("census-subs");
    public_recordings(&tmp);
    let tree = crate::evidence::DiskTree::open(&tmp.0, "evidence").unwrap();
    let subs = |name: &str| census_row(&tree, &PathKey::root().join(name)).unwrap().english_subtitles;
    assert_eq!(subs("disc-a"), Some(1)); // eng flags 0 counted, eng flags 6144 (4096 and 2048) left out
    assert_eq!(subs("knowing"), Some(2)); // eng flags 0 and eng flags 1 (a commentary flag) both counted
}

#[test]
fn census_lists_a_recording_without_scan_txt_with_question_marks_and_exits_0() {
    let tmp = Scratch::new("census-noscan");
    tmp.write("not-a-disc/meta.json", r#"{"format": "bluray"}"#);
    let (code, out, err) = census(&tmp.0);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, printed(&[
        "not-a-disc                               ?       ?     discdb=n/a    eng-subs=?  object=?           ?",
    ]));
}

#[test]
fn census_exits_2_naming_a_meta_json_that_does_not_parse() {
    let tmp = Scratch::new("census-broken");
    public_recordings(&tmp);
    tmp.write("zz-broken/meta.json", "{");
    let (code, out, err) = census(&tmp.0);
    assert_eq!(code, 2);
    assert_eq!(out.lines().count(), 5, "{out}"); // the rows before the broken one are kept
    assert!(err.contains("zz-broken/meta.json"), "{err}");
}

#[test]
fn census_without_the_evidence_clone_exits_3() {
    let tmp = Scratch::new("census-absent");
    let absent = tmp.0.join("absent").to_string_lossy().into_owned();
    let (code, out, err) = call(run, &Ctx::for_test(&tmp.0), &["--evidence", &absent]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, printed(&[&format!(
        "{absent}: no such folder; the evidence repository lazymkv-evidence is not cloned there (git clone {EVIDENCE_URL} {absent})"
    )]));
}

#[test]
fn census_is_reached_through_its_row_of_the_command_table() {
    let row = crate::COMMANDS.iter().find(|(name, _, _)| *name == "census").map(|(_, _, synopsis)| *synopsis);
    assert_eq!(row, Some("[--evidence DIR]"));
    assert_eq!(crate::COMMANDS.last().map(|(name, _, _)| *name), Some("census"));
    let tmp = Scratch::new("census-dispatch");
    public_recordings(&tmp);
    let argv: Vec<String> = ["census", "--evidence", &tmp.0.to_string_lossy()].iter().map(|w| w.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(crate::dispatch(&Ctx::for_test(&tmp.0), &argv, &mut out, &mut err), 0);
    assert_eq!(String::from_utf8(out).unwrap(), census(&tmp.0).1);
}
```

- [ ] **Step 2: Run them and see them fail.** Run: `scripts/box.sh cargo test -p lazymkv-dev --locked census`. Expected: compile error (`cmd::census` does not exist; `row14::media` is private).

- [ ] **Step 3: Make `row14`'s three items public** (`labels.rs:381` `pub mod row14;`; `row14.rs:261, :430, :454` `pub`), each newly public item keeping its doc text and ending "Def: <its existing spec citation, e.g. Spec 17 row 14 (a), 8.2>; T4-16 makes it public. Use: T4-16."

- [ ] **Step 4: Write `census.rs`** with the signatures above and the rules listed under Interfaces. `run` parses `Args::new(args, &["--evidence"])` as `titles.rs:18-32` does (a positional is `usage_error(err, "census", USAGE, "unrecognized arguments: ...")`, exit 2), calls `resolve_evidence(ctx, flag)` (the flag, else `LAZYMKV_EVIDENCE`; a `Refused` prints its line and returns its code), opens `DiskTree::open(&dir, EVIDENCE_SHOWN)` (as `retune` does, retune.rs:102-121), then `report(&tree, &PathKey::root(), out)`; `Err(text)` prints `text` on `err` and returns 2. Add the `pub mod census;` line and the `COMMANDS` row.

- [ ] **Step 5: Run again.** Run: `scripts/box.sh cargo test -p lazymkv-dev --locked census`. Expected: PASS (8 tests). Then `scripts/box.sh cargo test -p lazymkv-dev --locked --test readers the_six_readers`: PASS (the readers' run unchanged).

- [ ] **Step 6: The workspace run in the junction form** (Global Constraints: this task touches dev's readers and core's `row14`): `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`. Expected: PASS, the goldens byte-identical (a changed golden stops the task, K5), `no_scenario_holds_a_clone_value` passing.

- [ ] **Step 7: Commit.**

```bash
git add crates/dev/src/cmd/census.rs crates/dev/src/cmd/mod.rs crates/dev/src/lib.rs crates/core/src/subtitle/labels.rs crates/core/src/subtitle/labels/row14.rs
git commit -m "dev: the census report (A9)"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 17.

---

### Task 17: The Windows binary: live transport, webhook sink and cleanup (W26 replaced)

**Files:**
- Modify: `crates/engine/src/native.rs:21` (`use lazymkv_clients::{HttpTransport, UreqTransport};`), `:31` (`use crate::{.., CLEANUP_MINUTES, NotifySink, WebhookSink, ..}`, `NoDelivery` dropped), `:44-90` (`NativeOptions` gains `notify` and `cleanup_minutes`; `production()` sets them; the `http` doc says `UreqTransport::new()`), `:118-130` (the destructuring takes the two fields), `:171-188` (the order: `secrets`, then `http`, then `notify`, then `shares` and `Platform`, so `secrets` is cloned before `Platform` takes it), `:200-207` (`cleanup_minutes: options' cleanup_minutes`, which Task 11 left `None`), `:294-329` (`mod tests` gains one test).
- Modify: `crates/engine/src/testkit.rs:1172-1213` (`new_native` passes `notify: Some(MemorySink)` and `cleanup_minutes: None`; a new `new_native_with_webhook_sink`; both through one private `native_with(.., webhook_sink: bool)`), `:69` (`NoDelivery` out of the `use` list if Task 12 left it).
- Modify: `crates/fake-tools/tests/engine_native.rs:19` (`use lazymkv_engine::testkit::{ci_factor, Harness, MemorySink};`), `:23` (`use lazymkv_platform::{Host, SecretStore, StdProcessGuard};`, for `h.secrets().set(..)`), `:62-76` (`options` gains `notify: Some(Box::new(MemorySink::new().0))`, `cleanup_minutes: None`), `:117-138` (the undelivered assertion replaced), and two new tests.
- Modify: `crates/engine/src/notify.rs:18-39, :123, :147-154` (`NoDelivery`, `NO_DELIVERY` and `no_delivery_refuses_every_notification_with_its_sentence` removed; the `NotifySink` doc reworded: the binary passes `WebhookSink`, design part 8.6), `crates/engine/src/lib.rs:28` (`NoDelivery`, `NO_DELIVERY` out of the re-export, Ruling T4-R16).

**Interfaces:**
- Consumes: Task 7's `UreqTransport::new`; Task 11's `EngineConfig.cleanup_minutes`, `CLEANUP_MINUTES`; Task 12's `WebhookSink::new`, `NotifySink`; `MemorySink`, `MemorySecrets`, `RecordedTransport::{answer, posts}` (Task 6).
- Produces:

```rust
// native.rs (cfg(windows); design parts 4.2, 5.9, 8.1, 8.6; Rulings W26 replaced, T4-R11, T4-R16)
pub struct NativeOptions {
    /* PR 3 fields */
    /// None: `UreqTransport::new()` (design part 8.1).
    pub http: Option<Arc<dyn HttpTransport>>,
    /// None: `WebhookSink::new(Arc::clone(&http), Arc::clone(&secrets))` over this run's transport and store (design
    /// part 8.6).
    pub notify: Option<Box<dyn NotifySink>>,
    /// The cleanup's period in minutes (design part 5.9; Ruling T4-R11); None runs no cleanup.
    pub cleanup_minutes: Option<u32>,
}
// NativeOptions doc ends "Def: T3-23; T4-17 adds notify and cleanup_minutes. Use: T3-23, T4-17."
impl NativeOptions { pub fn production() -> NativeOptions; } // http None, notify None, cleanup_minutes Some(CLEANUP_MINUTES)

// testkit.rs (cfg(windows))
impl Harness {
    /// `new_native`, except that `NativeOptions.notify` is None, so the binary builds its own `WebhookSink` over the
    /// harness's `MemorySecrets` and recorded transport (design part 8.6; W26 replaced); its waits are real, so a test
    /// answers the webhook 2xx. Def: T4-17. Use: T4-17.
    pub fn new_native_with_webhook_sink(fake_tool: &Path, scenarios: &[&str], settings: Value, pipe: &str) -> Harness;
}
```

`new_native`'s doc gains "`notify`: a `MemorySink` over the harness's list, so `notifications()` reads it; `cleanup_minutes`: None" and ends "Def: T3-23; T4-17. Use: T3-23, T4-17."

- [ ] **Step 1: Write the failing tests.**

In `crates/engine/src/native.rs`'s `mod tests` (the module is `cfg(windows)`, so this runs on the mini PC and Windows CI only):

```rust
#[test]
fn production_options_leave_http_and_notify_to_the_binary() {
    let options = NativeOptions::production();
    assert!(options.http.is_none() && options.notify.is_none());
    assert_eq!(options.cleanup_minutes, Some(CLEANUP_MINUTES));
    assert_eq!(CLEANUP_MINUTES, 60);
}
```

In `crates/fake-tools/tests/engine_native.rs` (the file is `#![cfg(windows)]`), `a_native_engine_files_a_stand_in_disc_through_the_real_pipe` loses the `NO_DELIVERY` sentence and gains, after its wait for Filed, a wait for the `filed` notification as `whole_job.rs`'s `filed_job` does (a transition is persisted before its actions run, `testkit.rs:1047-1049`), then the list:

```rust
    wait_for(&h, "the filed notification", JOB_CAP, |h| h.notifications().iter().any(|n| n.event.as_str() == "filed"));
    let log = h.engine_log();
    assert!(!log.contains("was not delivered"), "{log}");
    let events: Vec<&str> = h.notifications().iter().map(|n| n.event.as_str()).collect();
    assert_eq!(events, ["disc_done", "filed"]);
```

and two new tests:

```rust
const HOOK: &str = "http://127.0.0.1:9/api/webhook/hook-sentinel-7f3a"; // answered by the recorded transport only

#[test]
fn a_native_engine_with_no_webhook_set_logs_no_undelivered_line() {
    if makemkv_runs_here() {
        return;
    }
    let pipe = format!(r"\\.\pipe\{}", test_name("native-nohook"));
    let h = Harness::new_native_with_webhook_sink(fake_tool(), &["movie-direct"], settings(), &pipe);
    h.insert("tok0", "movie-direct");
    wait_for(&h, "the job to be Filed", JOB_CAP, |h| h.job_ids().first().is_some_and(|id| h.state_of(id) == "Filed"));
    assert!(h.http().posts().is_empty(), "{:?}", h.http().posts());
    assert!(!h.engine_log().contains("was not delivered"), "{}", h.engine_log());
    assert_eq!(ctl(&h, &["stop"]), (0, String::new(), String::new()));
    assert_eq!(h.native_exit(), 0);
}

#[test]
fn a_native_engine_posts_its_notifications_to_the_set_webhook() {
    if makemkv_runs_here() {
        return;
    }
    let pipe = format!(r"\\.\pipe\{}", test_name("native-hook"));
    let h = Harness::new_native_with_webhook_sink(fake_tool(), &["movie-direct"], settings(), &pipe);
    h.secrets().set("ha_webhook_url", HOOK).unwrap();
    h.http().answer(HOOK, 200, b"");
    h.insert("tok0", "movie-direct");
    wait_for(&h, "two webhook posts", JOB_CAP, |h| h.http().posts().len() == 2);
    let posts = h.http().posts();
    let events: Vec<String> = posts.iter()
        .map(|(_, _, body)| serde_json::from_slice::<Value>(body).unwrap()["event"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(events, ["disc_done", "filed"]);
    assert!(posts.iter().all(|(url, headers, _)| url == HOOK
        && headers.contains(&("Content-Type".to_string(), "application/json".to_string()))));
    let log = h.engine_log();
    assert!(!log.contains("was not delivered") && !log.contains("hook-sentinel-7f3a"), "{log}");
    assert_eq!(ctl(&h, &["stop"]), (0, String::new(), String::new()));
    assert_eq!(h.native_exit(), 0);
}
```

- [ ] **Step 2: Run them and see them fail.** Run: `scripts/box.sh cargo test -p lazymkv-engine --locked production_options`. Expected: compile error (`NativeOptions` has no field `notify`). Then `scripts/box.sh cargo test -p fake-tools --locked --test engine_native`: compile error (`new_native_with_webhook_sink` is not found).

- [ ] **Step 3: Implement.** `NativeOptions` and `production()` as the Interfaces block gives. In `run`, after `let secrets: Arc<dyn SecretStore> = ..`:
  `let http: Arc<dyn HttpTransport> = http.unwrap_or_else(|| Arc::new(UreqTransport::new()));`
  `let notify: Box<dyn NotifySink> = notify.unwrap_or_else(|| Box::new(WebhookSink::new(Arc::clone(&http), Arc::clone(&secrets))));`
  then `share_connector`, `Platform` and `Services { platform, http, notify, timers: Arc::new(StdTimers), steps: None }`, and `EngineConfig { .., cleanup_minutes }`. The file's module doc names W26 replaced by design parts 8.1 and 8.6. `UreqTransport::new()` stays reachable only from `production()`'s None (Global Constraints: no non-ignored test reaches it, since every test passes `http: Some(..)`). Remove `NoDelivery`, `NO_DELIVERY`, their re-export and their unit test. In `testkit.rs`, `new_native` and `new_native_with_webhook_sink` call `native_with(fake_tool, scenarios, settings, pipe, webhook_sink)`, whose `NativeOptions` literal has `notify: (!webhook_sink).then(|| Box::new(MemorySink { list: Arc::clone(&harness.sent) }) as Box<dyn NotifySink>)` and `cleanup_minutes: None`.

- [ ] **Step 4: Check nothing names the removed double.** Run: `git grep -n -e NoDelivery -e NO_DELIVERY -e use_binary_services -- crates`. Expected: no line. A line left in `crates/fake-tools/tests/engine_ipc.rs` (its `use lazymkv_engine::NO_DELIVERY;` at `:21`, if Task 12 kept it) is removed in this task.

- [ ] **Step 5: Run again.** Run: `scripts/box.sh cargo test -p lazymkv-engine --locked production_options`, then `scripts/box.sh cargo test -p fake-tools --locked --test engine_native`. Expected: PASS (the native whole jobs run one at a time through `NativeSlot`; a `skipped: a MakeMKV process is running on this machine (S55)` line counts as not run, and the controller then runs it again with MakeMKV closed).

- [ ] **Step 6: The workspace run.** `scripts/box.sh cargo test --workspace --locked`. Expected: PASS. After Step 7's push, the controller reads the draft pull request's CodeQL result: zero alerts (cleartext transmission: `UreqTransport` is now reachable from `main`; a new alert stops the task as a red CI run would).

- [ ] **Step 7: Commit.**

```bash
git add crates/engine/src/native.rs crates/engine/src/notify.rs crates/engine/src/lib.rs crates/engine/src/testkit.rs crates/fake-tools/tests/engine_native.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "engine: the Windows binary passes the live transport, the notifier and the cleanup"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

(`engine_ipc.rs` is added only when Step 4 changed it.) Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems and CodeQL at zero alerts before Task 18.

---

### Task 18: Engine tests over the new lookups (design part 8.2; the UHD item; A5's held TV disc; T4-R1, T4-R20)

**Files:**
- Create: `corpus/scenarios/movie-direct-tagged/` (overlay on `movie-direct`): `disc.json` (movie-direct's with `"label":"SYNTHETIC_MOVIE_3D"` and the `bdmt_eng.xml` name `Synthetic Movie 3D`, so `cleaned` is `Synthetic Movie` and `tagged` is `Synthetic Movie 3D`), `tmdb-search-tagged-answer.json` (`{"page":1,"results":[],"total_pages":1,"total_results":0}`), `http.json`:

```json
{"answers":[
 {"url":"https://thediscdb.com/api/dischash/{fingerprint}","status":404,"body":""},
 {"url":"https://thediscdb.com/api/discid/{disc_id}","status":404,"body":""},
 {"url":"{search}","status":200,"body_file":"tmdb-search-answer.json"},
 {"url":"{search_tagged}","status":200,"body_file":"tmdb-search-tagged-answer.json"},
 {"url":"https://api.themoviedb.org/3/movie/900001?append_to_response=external_ids","status":200,"body_file":"tmdb-movie-answer.json"}]}
```

- Create: `corpus/scenarios/movie-direct-main-playlist/` (overlay on `movie-direct`): `scan.txt` (a byte copy of `movie-direct-copy-plan/scan.txt`: titles 0 `00800.mpls` and 1 `00801.mpls`, both 1:59:10); `thediscdb-main-playlist.json`:

```json
{"format":"Blu-ray","media":{"title":"Synthetic Movie","full_title":"Synthetic Movie","year":2001,"type":"Movie","external_ids":{"tmdb":"900001","imdb":"tt0900001"}},"titles":[{"index":1,"source_file":"00801.mpls","duration":"1:59:10","size":4096,"item":{"title":"Synthetic Movie","type":"MainMovie"}}]}
```

  `http.json` (movie-direct's four answers with the dischash answer `"status":200,"body_file":"thediscdb-main-playlist.json"`); `makemkvcon64.json` (movie-direct's with the rip response `{"args_contain":["mkv","disc:0","t01-00801"],"stdout":"rip-01.txt","creates":[{"dir_arg_from_end":1,"name":"title_t01.mkv","copy_from":"placeholder.mkv"}]}` in place of the `t00-00800` one); `rip-01.txt` (movie-direct's `rip.txt` with `00800.mpls` and title `#0`/`"0"` written `00801.mpls` and `#1`/`"1"`); `mkvmerge.json` (movie-direct's with `-J title_t01.mkv`).
- Create: `corpus/scenarios/movie-direct-three-playlists/` (overlay on `movie-direct`): as the main-playlist folder with a `scan.txt` of three titles (`00800`, `00801`, `00802`, each 1:59:10, title 2's TINFO and SINFO lines a copy of title 1's with `1` written `2` and `00801` written `00802`, and its `MSG:3307` line; `TCOUNT:2` written `TCOUNT:3`), `thediscdb-three-playlists.json` whose one title is `{"index":2,"source_file":"00802.mpls",..}`, and the rip, its file and `-J` keyed `t02-00802`, `rip-02.txt`, `title_t02.mkv`.
- Create: `corpus/scenarios/tv-direct-series/` (overlay on `movie-direct`; one title, so movie-direct's rip stand-ins serve it): `thediscdb-series.json` (`{"format":"Blu-ray","media":{"title":"Synthetic Show","full_title":"Synthetic Show","year":2003,"type":"Series","external_ids":{"tmdb":"900101"}},"titles":[{"index":0,"source_file":"00800.mpls","duration":"1:59:10","size":4096,"item":{"title":"Pilot","type":"Episode","season":"1","episode":"1"}}]}`), `tmdb-tv-answer.json` (`{"id":900101,"name":"Synthetic Show","original_name":"Synthetic Show","first_air_date":"2003-04-05","original_language":"en","seasons":[{"season_number":1,"episode_count":1}],"external_ids":{"imdb_id":"tt0900101"}}`), `tmdb-season-answer.json` (`{"season_number":1,"episodes":[{"episode_number":1,"name":"Pilot","runtime":119}]}`), `tmdb-search-empty.json` (the empty page above), and `http.json` with five answers: dischash 200 `thediscdb-series.json`, discid 404, `{search}` 200 `tmdb-search-empty.json`, `https://api.themoviedb.org/3/tv/900101?append_to_response=external_ids` 200 `tmdb-tv-answer.json`, `https://api.themoviedb.org/3/tv/900101/season/1` 200 `tmdb-season-answer.json`.
- Modify: `crates/engine/src/testkit.rs:1523-1529` (`answered(status: &Path, urls: &LookupUrls) -> bool`, the per-kind rule of Ruling T4-R1), `:1486-1489` (`TestGuard.urls` becomes `LookupUrls`, its doc "the lookup URLs of the merged `http.json` by kind; empty waits for nothing") and `:1512` (`self.urls > 0` becomes `!self.urls.is_empty()`), `:553-557` (the two branches without a disc give `LookupUrls::default()`), `:1580-1610` (`recorded_http` returns `(RecordedTransport, LookupUrls)`; its vars gain `("search_tagged", search_url(&name.tagged))`; its doc and `Harness::new`'s doc at `:509-510` name the var), `:1476-1480` (the guard's doc: the rule of Ruling T4-R1, stated as that ruling states it), `:1604-1607` (the count moves into a private `lookup_urls_by_kind(document: &Value, vars: &[(&str, &str)]) -> LookupUrls` over a private `#[derive(Debug, Default, PartialEq)] struct LookupUrls { keys: BTreeSet<String>, searches: usize }`), and a new `#[cfg(test)] mod tests` at the file's end.
- Modify: `crates/fake-tools/tests/whole_job.rs`, `crates/fake-tools/tests/engine_file_phase.rs`, `crates/fake-tools/tests/engine_ipc.rs` (the tests below).

**Interfaces:**
- Consumes: Tasks 2 to 5 (`search-tagged`, `season-<n>` words, `tmdb-search.json`'s `queries`, the spliced `tmdb-record.json`), Task 6 (`posts`), Task 12 (`use_webhook_sink`), `clients::{search_url, record_url, season_url}`.
- Produces: no `pub` item. The `http.json` var `{search_tagged}` (the search URL of `DiscName.tagged`) for every later scenario. All values are synthetic (RP21): IDs 900001 and 900101, `tt0900001`, `tt0900101`, the names `Synthetic Movie`, `Synthetic Movie 3D`, `Synthetic Show`, `Pilot`.

- [ ] **Step 1: Write the failing tests.**

`crates/engine/src/testkit.rs`, new `mod tests` (the guard's rule, Ruling T4-R1; the live webhook half is pinned by `a_webhook_receives_disc_done_and_filed_for_a_whole_job`, whose POSTs are never in `http.json`):

```rust
#[cfg(test)]
mod tests {
    use super::{answered, lookup_urls_by_kind};
    use lazymkv_core::testsupport::Scratch;

    #[test]
    fn the_guard_wants_a_line_of_each_lookup_urls_kind_and_ignores_webhooks() {
        let http = serde_json::json!({"answers": [
            {"url": "https://thediscdb.com/api/dischash/{fingerprint}", "status": 404, "body": ""},
            {"url": "https://thediscdb.com/api/discid/{disc_id}", "status": 404, "body": ""},
            {"url": "{search}", "status": 200, "body": ""},
            {"url": "{search}", "status": 200, "body": ""},
            {"url": "{search_tagged}", "status": 200, "body": ""},
            {"url": "https://api.themoviedb.org/3/movie/900001?append_to_response=external_ids",
             "status": 200, "body": ""},
            {"url": "http://127.0.0.1:9/api/webhook/hook-sentinel-7f3a", "status": 200, "body": ""}]});
        // `{search}` is substituted to a TMDb search URL; `{search_tagged}` is given no var, so it still starts
        // with `{` and is a search URL too. One distinct `{search}` URL plus the tagged one: two search URLs.
        let search = lazymkv_clients::search_url("Synthetic Movie");
        let vars = [("fingerprint", "AB12"), ("disc_id", "CD34"), ("search", search.as_str())];
        let urls = lookup_urls_by_kind(&http, &vars);
        assert_eq!(urls.searches, 2, "{urls:?}");
        assert_eq!(
            urls.keys.iter().map(String::as_str).collect::<Vec<_>>(),
            ["dischash", "discid", "tmdb movie/900001"],
            "the webhook URL needs no line"
        );
        let scratch = Scratch::new("guard");
        let status = scratch.0.join("lookup-status.txt");
        let case = |text: &str| {
            std::fs::write(&status, text).unwrap();
            answered(&status, &urls)
        };
        assert!(case("dischash 404\ndiscid 404\nsearch 200\nsearch-tagged 200\ntmdb movie/900001 200\n"));
        // An entry title equal to the cleaned name: one more search line than search URLs passes.
        assert!(case(
            "dischash 404\ndiscid 404\nsearch 200\nsearch-tagged 200\nsearch-entry 200\ntmdb movie/900001 200\n"
        ));
        // The record URL needs its own line: neither the search lines nor another record's line stands in for it.
        assert!(!case("dischash 404\ndiscid 404\nsearch 200\nsearch-tagged 200\nsearch-entry 200\n"));
        assert!(!case("dischash 404\ndiscid 404\nsearch 200\nsearch-tagged 200\ntmdb movie/900002 200\n"));
        // Two search URLs want two search lines.
        assert!(!case("dischash 404\ndiscid 404\nsearch 200\ntmdb movie/900001 200\n"));
        assert!(!case("dischash 404\ndiscid 404\nsearch 200\nsearch-tagged 000\ntmdb movie/900001 200\n"));
        // A season URL wants its season line.
        let tv = serde_json::json!({"answers": [
            {"url": "https://api.themoviedb.org/3/tv/900101?append_to_response=external_ids", "status": 200, "body": ""},
            {"url": "https://api.themoviedb.org/3/tv/900101/season/1", "status": 200, "body": ""}]});
        let tv_urls = lookup_urls_by_kind(&tv, &[]);
        assert_eq!(tv_urls.keys.iter().map(String::as_str).collect::<Vec<_>>(), ["season-1", "tmdb tv/900101"]);
        std::fs::write(&status, "tmdb tv/900101 200\n").unwrap();
        assert!(!answered(&status, &tv_urls));
        std::fs::write(&status, "season-1 200\ntmdb tv/900101 200\n").unwrap();
        assert!(answered(&status, &tv_urls));
    }
}
```

`crates/fake-tools/tests/whole_job.rs`:

```rust
/// Waits (120 s, 480 s under CI) for the one job of `h` to reach `state`; its folder.
fn job_in(h: &Harness, state: &str) -> PathBuf { /* polls h.job_ids() and h.state_of every 20 ms */ }

#[test]
fn a_tagged_name_files_with_both_searches_saved() {
    let h = Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &["movie-direct", "movie-direct-tagged"],
        json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}));
    h.insert("tok0", "movie-direct");
    let job = job_in(&h, "Filed");
    let name = disc_name(Some("Synthetic Movie 3D"), Some("SYNTHETIC_MOVIE_3D")).unwrap();
    assert_eq!((name.cleaned.as_str(), name.tagged.as_str()), ("Synthetic Movie", "Synthetic Movie 3D"));
    let urls: Vec<String> = h.http().requests().into_iter().map(|(url, _)| url).collect();
    let at = |url: &str| urls.iter().position(|asked| asked == url).unwrap_or_else(|| panic!("{url} not asked: {urls:?}"));
    assert!(at(&search_url(&name.cleaned)) < at(&search_url(&name.tagged)));
    assert_eq!(urls.len(), 5, "{urls:?}"); // each of the five recorded URLs once
    assert_eq!(h.http().unrecorded(), Vec::<String>::new());
    let status = fs::read_to_string(job.join("lookup-status.txt")).unwrap();
    assert!(status.lines().any(|l| l == "search 200") && status.lines().any(|l| l == "search-tagged 200"), "{status}");
    assert_eq!(read_json(&job.join("tmdb-search.json"))["queries"], json!(["Synthetic Movie", "Synthetic Movie 3D"]));
    assert_eq!(names_in(&h.library().join(FILM)).len(), 1);
}

#[test]
fn a_webhook_receives_disc_done_and_filed_for_a_whole_job() {
    const HOOK: &str = "http://127.0.0.1:9/api/webhook/hook-sentinel-7f3a"; // the recorded transport answers it
    let h = Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &["movie-direct"],
        json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}));
    h.use_webhook_sink();
    h.restart();
    h.secrets().set("ha_webhook_url", HOOK).unwrap();
    h.http().answer(HOOK, 200, b"");
    h.insert("tok0", "movie-direct");
    let job = job_in(&h, "Filed");
    h.wait("two posts", |h| h.http().posts().len() == 2);
    let events: Vec<String> = h.http().posts().iter()
        .map(|(_, _, body)| serde_json::from_slice::<Value>(body).unwrap()["event"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(events, ["disc_done", "filed"]);
    for text in [h.engine_log(), fs::read_to_string(job.join("job.log")).unwrap()] {
        assert!(!text.contains("hook-sentinel-7f3a") && !text.contains("was not delivered"), "{text}");
    }
}
```

`the_whole_job_runs_with_no_unrecorded_url` stays unchanged: `Synthetic Movie`'s two forms fold equal, so its four URLs stand.

`crates/fake-tools/tests/engine_file_phase.rs` (its `harness` puts `movie-direct` first; `run_to` inserts it):

```rust
/// The rips the job asked of makemkvcon: each spawn whose arguments hold "mkv".
fn rips(h: &Harness) -> Vec<String> {
    h.spawns().into_iter().filter(|s| s.args.iter().any(|a| a == "mkv")).map(|s| s.args.join(" ")).collect()
}

#[test]
fn an_identified_disc_with_two_feature_length_playlists_rips_only_its_main_title() {
    let h = harness(&["movie-direct-main-playlist"], json!({}));
    let id = run_to(&h, "Filed");
    let rips = rips(&h);
    assert_eq!(rips.len(), 1, "{rips:?}");
    assert!(rips[0].contains("t01-00801") && !rips[0].contains("t00-00800"), "{rips:?}");
    assert!(fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap().contains("dischash 200"));
    assert!(!h.record(&id).message.unwrap_or_default().contains("copy mode"));
}

#[test]
fn an_identified_disc_with_three_same_length_playlists_rips_the_one_its_record_names() {
    let h = harness(&["movie-direct-three-playlists"], json!({}));
    let id = run_to(&h, "Filed");
    let rips = rips(&h);
    assert_eq!(rips.len(), 1, "{rips:?}");
    assert!(rips[0].contains("t02-00802"), "{rips:?}");
    assert!(!h.record(&id).message.unwrap_or_default().contains("copy mode"));
}

#[test]
fn a_tv_disc_identified_through_its_seasons_is_held_with_no_path() {
    let h = harness(&["tv-direct-series"], json!({})); // the harness leaves tv_root unset
    let id = run_to(&h, "NeedsReview");
    let status = fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap();
    assert!(status.contains("tmdb tv/900101 200") && status.contains("season-1 200"), "{status}");
    let record = json_of(&h.job_dir(&id).join("tmdb-record.json"));
    assert_eq!(record["seasons"][0]["episodes"], json!([{"episode_number": 1, "runtime": 119}]));
    assert!(h.record(&id).titles.iter().all(|t| t.filed.is_none()));
    assert!(files_under(h.library()).is_empty(), "nothing is written under movies_root (A5, design part 6.6)");
}
```

(`files_under` is copied from `whole_job.rs:29-46` into `engine_file_phase.rs` when absent; `TitleRecord.filed` is `core::record`'s `Option<FiledFile>`. `whole_job.rs` already imports `disc_name`, `search_url` and `Value`; its `use lazymkv_platform::Host;` becomes `use lazymkv_platform::{Host, SecretStore};` for `h.secrets().set(..)`.)

`crates/fake-tools/tests/engine_ipc.rs` (Review Focus 1; the file's `use` lines gain `lazymkv_clients::{record_url, search_url}`, `lazymkv_core::lookup::name::disc_name` and `lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind}` when absent):

```rust
#[test]
fn a_new_token_reaches_the_next_round_without_a_restart() {
    let h = stand_in(&["movie-direct", "movie-direct-lookup-refused"], settings()); // search answers 401
    h.secrets().set("tmdb_token", "token-sentinel-5d2e").unwrap();
    h.scan_waits_for_lookups(false);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    let id = id_of(&h, DISC_A);
    h.wait("AwaitingLookup", |h| h.state_of(&id) == "AwaitingLookup");
    h.settle();
    assert!(h.record(&id).message.unwrap().contains("search: TMDb refused the token (HTTP 401)"));
    let name = disc_name(Some("Synthetic Movie"), Some("SYNTHETIC_MOVIE")).unwrap();
    let scenario = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scenarios/movie-direct");
    h.http().answer(&search_url(&name.cleaned), 200, &fs::read(scenario.join("tmdb-search-answer.json")).unwrap());
    h.http().answer(&record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 900001 }), 200,
                    &fs::read(scenario.join("tmdb-movie-answer.json")).unwrap());
    assert_eq!(c.ok("secret_set", json!({"name": "tmdb_token", "value": "token-sentinel-2b9c"})), Value::Null);
    c.ok("retry", json!({"job": id}));
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let bearers: Vec<String> = h.http().requests().into_iter()
        .filter(|(url, _)| *url == search_url(&name.cleaned))
        .filter_map(|(_, headers)| headers.into_iter().find(|(k, _)| k == "Authorization").map(|(_, v)| v))
        .collect();
    // The refused search is asked with the first token in every round before AwaitingLookup (at arrival and before
    // the file-phase slot), then once with the new token after the retry.
    assert!(bearers.len() >= 2, "{bearers:?}");
    assert_eq!(bearers.last().map(String::as_str), Some("Bearer token-sentinel-2b9c"), "{bearers:?}");
    assert!(bearers[..bearers.len() - 1].iter().all(|b| b == "Bearer token-sentinel-5d2e"), "{bearers:?}");
    for path in [h.data().join("engine.log"), h.job_dir(&id).join("job.log"), h.job_dir(&id).join("state.json")] {
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("token-sentinel-5d2e") && !text.contains("token-sentinel-2b9c"), "{}", path.display());
    }
}
```

- [ ] **Step 2: Run them and see them fail.** Run, one at a time: `scripts/box.sh cargo test -p lazymkv-engine --locked guard` (compile error, no `lookup_urls_by_kind` and no `LookupUrls`); `scripts/box.sh cargo test -p fake-tools --locked --test whole_job` (FAIL: the scenario folder `movie-direct-tagged` does not exist, then `{search_tagged}` is not substituted and the tagged search is unrecorded); `scripts/box.sh cargo test -p fake-tools --locked --test engine_file_phase playlists` and `... held_with_no_path` (FAIL: the scenario folders do not exist); `scripts/box.sh cargo test -p fake-tools --locked --test engine_ipc new_token` (the bearers are at least two, every one before the last `token-sentinel-5d2e`, the last `token-sentinel-2b9c`; FAIL only if Tasks 2 to 12 left a gap; a PASS here is recorded as "already held").

- [ ] **Step 3: Implement.** Write the four scenario folders as Files gives them (UTF-8, `\n` line ends as the existing scenarios; the scan copies byte-for-byte apart from the lines named). In `testkit.rs`, the guard applies Ruling T4-R1's per-kind rule, which this step does not restate. `lookup_urls_by_kind` substitutes each `url` with `vars` as `from_file` does (the same `fold` of `replace("{name}", value)`), keeps the distinct results, and sorts each into `LookupUrls`: a URL that starts with `{` or with `format!("{TMDB}/search/")` adds one to `searches`; one under `DISCDB` adds `dischash` or `discid` to `keys` by its path's first segment; one under `TMDB` adds `tmdb <kind>/<id>` for `/movie/<id>` or `/tv/<id>` with the query `append_to_response=external_ids`, and `season-<n>` for `/tv/<id>/season/<n>`; any other URL adds nothing. `LookupUrls::is_empty` is `keys.is_empty() && searches == 0`. `answered` reads the status file's non-empty lines, false when any ends in `000`, takes each line's key as the text before its last space, and is true when every key of `urls.keys` is a line's key and the lines whose key is `search`, `search-tagged` or `search-entry` number at least `urls.searches`. `recorded_http` adds the `search_tagged` var and passes its `vars` to `lookup_urls_by_kind` over the parsed document. No change to `crates/core`, `crates/clients` or `crates/engine/src` beyond `testkit.rs`: if a playlist scenario plans copy mode or holds instead of filing, or the TV disc does not reach `NeedsReview`, the task stops and the controller reports the plan's reasons to the owner as a finding against Ruling T4-R20 or A5 (no rule is changed here).

- [ ] **Step 4: Run again.** The five Run commands of Step 2, then `scripts/box.sh cargo test -p fake-tools --locked --test whole_job the_whole_job_runs_with_no_unrecorded_url`. Expected: PASS each.

- [ ] **Step 5: The workspace run in the junction form** (this task adds files under `corpus/scenarios/`): `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`. Expected: PASS, `no_scenario_holds_a_clone_value` passing over the four new folders, the goldens byte-identical.

- [ ] **Step 6: Commit.**

```bash
git add corpus/scenarios/movie-direct-tagged corpus/scenarios/movie-direct-main-playlist corpus/scenarios/movie-direct-three-playlists corpus/scenarios/tv-direct-series crates/engine/src/testkit.rs crates/fake-tools/tests/whole_job.rs crates/fake-tools/tests/engine_file_phase.rs crates/fake-tools/tests/engine_ipc.rs
git commit -m "fake-tools: whole jobs over J8's two searches, identified multi-playlist discs and a held TV disc"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```

Expected: `G` and the identity of Task 1, no attribution line. CI green on the three systems before Task 19.

---

### Task 19: The live shape tests (design part 8.5, T4-R19; mini PC only)

The `#[ignore]` tests ask TheDiscDB and TMDb (The Movie Database) a fixed list of public requests, run each answer through the parser the engine uses, and write dated records into the evidence mirror; they run once, at Task 20 Step 2 (Owner decision 17). The non-ignored tests pin the shape record, the date, the public IDs and the replay of saved records. Line anchors are at `6a862ac` plus Tasks 1 to 18.

Precondition: `decisions.txt` holds item 17's and T4-R19's answers (Task 0 Step 1).

**Files:**
- Modify: `crates/clients/src/saved.rs` (`shape_of` after `status_text`, `:157`; one test in `mod tests`, `:170`).
- Modify: `crates/clients/src/lib.rs:15` (`shape_of` joins the `saved` re-export).
- Create: `crates/clients/tests/live.rs` (4 ignored tests, 3 not ignored; no new dependency: `lazymkv-core`, `serde_json` and the crate itself).
- Modify: `crates/core/src/lookup/stale.rs` (`civil_date` beside Task 8's `add_months`: the date arithmetic moved here, so no copy of it is made).
- Modify: `crates/dev/src/evidence.rs:137-150` (`civil_time` calls `lazymkv_core::lookup::stale::civil_date` for its date part; its test `civil_time_counts_days_as_the_gregorian_calendar` unchanged).

**Interfaces:**
- Consumes: Task 7 `UreqTransport::new()`; Task 2 `merge_pages`, `slim_search(pages, today)`; Task 4 `tmdb_season`, `splice_season`, `core::lookup::tmdb::merge_parsed_season`; Task 8 `reason_text`, `core::lookup::stale::{is_stale, fetched_at}`; existing `discdb_by_fingerprint`, `discdb_by_disc_id`, `tmdb_search`, `tmdb_record`, `slim_record`, `Outcome`, `core::lookup::discdb::{parse_dischash, parse_discid}`, `core::lookup::tmdb::{parse_search, parse_movie, parse_tv, TmdbIdentity, TmdbKind}`, `core::lookup::name::{clean, tagged, fold_name, plausible}`.
- Produces:

```rust
// core::lookup::stale (design part 4.6: no date-time crate)
/// The proleptic Gregorian (year, month, day) of a count of seconds since 1970-01-01 (UTC), by the days-to-civil
/// algorithm. Pure. Def: T4-19. Use: T4-19, dev's `civil_time`.
pub fn civil_date(unix_secs: u64) -> (i64, u32, u32);

// saved.rs
/// The shape record of design part 8.5 (P29, Ruling T4-R19): every scalar becomes its JSON type name ("string",
/// "number", "boolean", "null"); an object keeps its keys, in order, with their values' shapes; an array becomes a
/// one-element array of its elements' merged shape (`[]` when empty). Merging unites two objects' keys (first seen
/// first) key by key, merges two arrays' element shapes, and otherwise joins the type names with `|` in the order
/// object, array, string, number, boolean, null (`"number|null"`); an object or array meeting a scalar keeps only its
/// type name (`"object|null"`). No value of the answer survives. Def: T4-19. Use: T4-19.
pub fn shape_of(value: &Value) -> Value;
```

  Records written (Owner decision 17), under `<LAZYMKV_EVIDENCE>\live-lookups\<UTC yyyy-MM-dd>\`, ten files: `thediscdb-dischash-76E63EBB5B66F73B0C37BFAB59B8ABA5.json` and `thediscdb-discid-8D0355F530C31D44912831F8637FF65ACBCD6886.json` (bytes as received); `tmdb-search-PIRANHA_3D`, `tmdb-movie-245891`, `tmdb-tv-1437` and `tmdb-tv-1437-season-1`, each `.slim.json` and `.shape.json`. JSON records are `serde_json::to_vec_pretty` plus `\n`; the slim records' `fetched_at` is the same UTC date (T4-R19).

- [ ] **Step 1: Write the failing tests**

In `crates/clients/src/saved.rs`'s `mod tests` (synthetic values only):

```rust
#[test]
fn shape_records_hold_names_and_types_only() {
    let raw = json!({"id": 900001, "title": "Synthetic Movie", "adult": false, "imdb_id": null,
        "genres": [{"id": 28, "name": "Action"}, {"id": 53, "name": null, "extra": true}], "videos": [],
        "external_ids": {"imdb_id": "tt0900001", "tvdb_id": 7}, "mixed": [1, null, "x"],
        "maybe": [{"a": 1}, null], "nested": [[1], [2.5, "y"]]});
    let shape = shape_of(&raw);
    assert_eq!(shape, json!({"id": "number", "title": "string", "adult": "boolean", "imdb_id": "null",
        "genres": [{"id": "number", "name": "string|null", "extra": "boolean"}], "videos": [],
        "external_ids": {"imdb_id": "string", "tvdb_id": "number"}, "mixed": ["string|number|null"],
        "maybe": ["object|null"], "nested": [["string|number"]]}));
    let keys: Vec<&str> = shape.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["id", "title", "adult", "imdb_id", "genres", "videos", "external_ids", "mixed", "maybe", "nested"]);
    let genre: Vec<&str> = shape["genres"][0].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(genre, ["id", "name", "extra"]);
    let text = serde_json::to_string(&shape).unwrap();
    assert_eq!(text.find(|c: char| c.is_ascii_digit()), None, "{text}");
    for value in ["Synthetic Movie", "Action", "tt0900001", "\"x\"", "\"y\""] {
        assert!(!text.contains(value), "{value} in {text}");
    }
    assert_eq!(shape_of(&json!("x")), json!("string"));
}
```

`crates/clients/tests/live.rs`: its header, constants and helpers (helpers as signatures, each body the one line its doc says), then the seven tests:

```rust
//! The live shape checks of design part 8.5 (ENG-20, Ruling T4-R19). The `#[ignore]` tests run only on the mini PC:
//! `cargo test -p lazymkv-clients --locked --test live -- --ignored --test-threads=1`, with `TMDB_TOKEN` and
//! `LAZYMKV_EVIDENCE` naming the evidence mirror. The others run everywhere; the replay skips without the clone.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lazymkv_clients::{discdb_by_disc_id, discdb_by_fingerprint, merge_pages, reason_text, shape_of, slim_record,
    slim_search, splice_season, tmdb_record, tmdb_search, tmdb_season, Outcome, UreqTransport};
use lazymkv_core::lookup::discdb::{parse_discid, parse_dischash};
use lazymkv_core::lookup::name::{clean, fold_name, plausible, tagged};
use lazymkv_core::lookup::stale::{civil_date, fetched_at, is_stale};
use lazymkv_core::lookup::tmdb::{merge_parsed_season, parse_movie, parse_search, parse_tv, TmdbIdentity, TmdbKind};
use serde_json::Value;

/// John Wick's 4K disc, from `corpus/thediscdb-api` (public, RP21).
const FINGERPRINT: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
const DISC_ID: &str = "8D0355F530C31D44912831F8637FF65ACBCD6886";
/// A Firefly disc ID of `corpus/thediscdb-api`, whose answer names the show.
const FIREFLY_DISC_ID: &str = "66C36172D7981A2845308045DFBD9DE1";
const MOVIE: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 245891 };
const SHOW: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Tv, id: 1437 };
const SEASON: u32 = 1;
/// Decision-core H1's public disc name, whose tagged and cleaned forms differ (J8).
const NAME: &str = "PIRANHA_3D";

/// Writes `skipped: <reason>` and a newline straight to std::io::stderr() (past the harness's capture), as
/// `lazymkv_dev::evidence::skip` does; `clients` has no `dev` edge.
fn skip(reason: &str);
/// `yyyy-MM-dd` (UTC) of a count of seconds since 1970-01-01: `civil_date(seconds)` formatted `{:04}-{:02}-{:02}`.
fn utc_date(seconds: u64) -> String;
/// `utc_date` of `SystemTime::now()` (Ruling T4-R19: the records and their `fetched_at` carry the UTC date).
fn today() -> String;
/// `<LAZYMKV_EVIDENCE>/live-lookups/<today>/`, created with `create_dir_all`. None after
/// `skip("LAZYMKV_EVIDENCE is not set")` when the variable is unset or empty, and after
/// `skip("<root> is not the evidence mirror (no folder, or it holds .git; design part 8.5)")` when it names no folder
/// or a folder holding `.git`: the records go to the mirror only, never to a clone or this repository.
fn records_dir(today: &str) -> Option<PathBuf>;
/// `TMDB_TOKEN` when set and not empty; else None after `skip("TMDB_TOKEN is not set")`.
fn token() -> Option<String>;
/// The value and the body of `Outcome::Answered`; a `Miss` panics `<what>: no such record (HTTP <status>)`, a
/// `Pending` panics `<what>: <reason_text>`, words that never hold the token (Task 7).
fn answered<T>(outcome: Outcome<T>, what: &str) -> (T, Vec<u8>);
/// `serde_json::from_slice(body)`, panicking `a parsed answer is JSON`.
fn json_of(body: &[u8]) -> Value;
/// Panics `<what>: no <pointer>` when `value.pointer(pointer)` is None, and `<what>: <pointer> is <kind>, not
/// <types>` when its kind ("object", "array", or `shape_of`'s scalar name) is not in `types`.
fn field(value: &Value, pointer: &str, types: &[&str], what: &str);
/// Asserts the bytes hold no window equal to the token (`<name> would hold the token`, the token itself never
/// printed), then `std::fs::write(dir.join(name), bytes)`.
fn save(dir: &Path, name: &str, bytes: &[u8], token: Option<&str>);
/// `serde_json::to_vec_pretty(value)` and a final `\n`.
fn pretty(value: &Value) -> Vec<u8>;
```

The body of `civil_date` in `crates/core/src/lookup/stale.rs`, moved from `dev::evidence::civil_time` (`evidence.rs:137-150`), whose date part then calls it (`let (year, month, day) = civil_date(seconds);` before its `format!`, the time of day as today):

```rust
pub fn civil_date(unix_secs: u64) -> (i64, u32, u32) {
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}
```
Then the seven tests, in this order:

```rust
#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_dischash_and_discid_parse_and_are_saved_as_received() {
    let Some(dir) = records_dir(&today()) else { return };
    let t = UreqTransport::new();
    let (record, body) = answered(discdb_by_fingerprint(&t, FINGERPRINT), "dischash");
    assert_eq!(record.tmdb_identity(), Some(MOVIE));
    save(&dir, &format!("thediscdb-dischash-{FINGERPRINT}.json"), &body, None);
    let (records, body) = answered(discdb_by_disc_id(&t, DISC_ID), "discid");
    assert!(records.iter().any(|record| record.tmdb_identity() == Some(MOVIE)), "discid names John Wick");
    save(&dir, &format!("thediscdb-discid-{DISC_ID}.json"), &body, None);
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_search_runs_both_j8_queries_and_merges_them() {
    let today = today();
    let Some(dir) = records_dir(&today) else { return };
    let Some(token) = token() else { return };
    let (cleaned, tagged) = (clean(NAME), tagged(NAME));
    assert_eq!((cleaned.as_str(), tagged.as_str()), ("PIRANHA", "PIRANHA 3D"));
    assert_ne!(fold_name(&cleaned), fold_name(&tagged), "J8 asks twice only when the forms differ");
    let t = UreqTransport::new();
    let (cleaned_page, cleaned_body) = answered(tmdb_search(&t, Some(&token), &cleaned), "the cleaned search");
    let (tagged_page, tagged_body) = answered(tmdb_search(&t, Some(&token), &tagged), "the tagged search");
    let raws = [json_of(&cleaned_body), json_of(&tagged_body)];
    for (page, what) in raws.iter().zip(["the cleaned search", "the tagged search"]) {
        for (at, result) in page["results"].as_array().expect("results").iter().enumerate() {
            let fields = match result["media_type"].as_str() {
                Some("movie") => ["title", "original_title", "release_date"],
                Some("tv") => ["name", "original_name", "first_air_date"],
                _ => continue,
            };
            field(result, "/id", &["number"], what);
            for name in fields {
                field(result, &format!("/{name}"), &["string"], &format!("{what}, result {at}"));
            }
        }
    }
    let merged = merge_pages(&cleaned_page, &tagged_page);
    assert!(merged.results.iter().any(|result| plausible(result, &tagged)), "H1: the tagged search finds Piranha 3D");
    let slim = slim_search(&[(&raws[0], cleaned.as_str()), (&raws[1], tagged.as_str())], &today);
    assert_eq!(slim["queries"], serde_json::json!([cleaned, tagged]));
    assert_eq!(parse_search(&slim.to_string(), "the slim page").unwrap(), merged);
    save(&dir, "tmdb-search-PIRANHA_3D.slim.json", &pretty(&slim), Some(&token));
    let shape = shape_of(&Value::Array(raws.to_vec()))[0].clone(); // both pages' merged shape
    save(&dir, "tmdb-search-PIRANHA_3D.shape.json", &pretty(&shape), Some(&token));
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_movie_record_has_every_field_core_reads() {
    let today = today();
    let Some(dir) = records_dir(&today) else { return };
    let Some(token) = token() else { return };
    let (record, body) = answered(tmdb_record(&UreqTransport::new(), Some(&token), MOVIE), "the movie record");
    let raw = json_of(&body);
    for pointer in ["/title", "/original_title", "/release_date", "/original_language", "/external_ids/imdb_id"] {
        field(&raw, pointer, &["string"], "the movie record");
    }
    field(&raw, "/id", &["number"], "the movie record");
    field(&raw, "/runtime", &["number"], "the movie record");
    assert_eq!((record.identity, record.year), (MOVIE, Some(2014)));
    assert!(record.runtime.is_some() && record.imdb_id.is_some() && record.original_language.is_some());
    let slim = slim_record(&raw, TmdbKind::Movie, &today);
    assert_eq!(parse_movie(&slim.to_string(), "the slim record").unwrap(), record, "the slim record keeps what core reads");
    save(&dir, "tmdb-movie-245891.slim.json", &pretty(&slim), Some(&token));
    save(&dir, "tmdb-movie-245891.shape.json", &pretty(&shape_of(&raw)), Some(&token));
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_tv_record_and_season_have_every_field_core_reads() {
    let today = today();
    let Some(dir) = records_dir(&today) else { return };
    let Some(token) = token() else { return };
    let t = UreqTransport::new();
    let (mut record, body) = answered(tmdb_record(&t, Some(&token), SHOW), "the tv record");
    let raw = json_of(&body);
    for pointer in ["/name", "/original_name", "/first_air_date", "/original_language"] {
        field(&raw, pointer, &["string"], "the tv record");
    }
    field(&raw, "/id", &["number"], "the tv record");
    field(&raw, "/external_ids/tvdb_id", &["number", "null"], "the tv record");
    for (at, season) in raw["seasons"].as_array().expect("seasons").iter().enumerate() {
        field(season, "/season_number", &["number"], &format!("the tv record, season {at}"));
        field(season, "/episode_count", &["number"], &format!("the tv record, season {at}"));
    }
    assert_eq!((record.identity, record.year), (SHOW, Some(2002)));
    let slim = slim_record(&raw, TmdbKind::Tv, &today);
    assert_eq!(parse_tv(&slim.to_string(), "the slim record").unwrap(), record);
    let (season, season_body) = answered(tmdb_season(&t, Some(&token), SHOW.id, SEASON), "season 1");
    let season_raw = json_of(&season_body);
    field(&season_raw, "/season_number", &["number"], "season 1");
    for (at, episode) in season_raw["episodes"].as_array().expect("episodes").iter().enumerate() {
        field(episode, "/episode_number", &["number"], &format!("season 1, episode {at}"));
        field(episode, "/runtime", &["number", "null"], &format!("season 1, episode {at}"));
    }
    merge_parsed_season(&mut record, season).unwrap();
    let runtimes = record.episode_runtimes(Some(SEASON));
    assert!(!runtimes.is_empty(), "season 1 has episode runtimes");
    let mut spliced = slim.clone();
    splice_season(&mut spliced, &season_raw);
    assert_eq!(parse_tv(&spliced.to_string(), "the spliced record").unwrap().episode_runtimes(Some(SEASON)), runtimes);
    save(&dir, "tmdb-tv-1437.slim.json", &pretty(&slim), Some(&token));
    save(&dir, "tmdb-tv-1437.shape.json", &pretty(&shape_of(&raw)), Some(&token));
    save(&dir, "tmdb-tv-1437-season-1.slim.json", &pretty(&spliced), Some(&token));
    save(&dir, "tmdb-tv-1437-season-1.shape.json", &pretty(&shape_of(&season_raw)), Some(&token));
}

/// Reads the files to check the IDs are listed there; no body is replayed, S69 untouched.
#[test]
fn the_live_requests_use_only_public_ids() {
    let public = |name: String| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb-api").join(&name);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
    };
    let hash = parse_dischash(&public(format!("dischash-{FINGERPRINT}.json")), "dischash").unwrap();
    assert_eq!(hash.and_then(|record| record.tmdb_identity()), Some(MOVIE));
    for (disc_id, identity) in [(DISC_ID, MOVIE), (FIREFLY_DISC_ID, SHOW)] {
        let records = parse_discid(&public(format!("discid-{disc_id}.json")), "discid").unwrap();
        assert!(records.iter().any(|record| record.tmdb_identity() == Some(identity)), "{disc_id}");
    }
}

#[test]
fn the_utc_date_counts_days_as_the_gregorian_calendar() {
    for (seconds, date) in [(0, "1970-01-01"), (86_399, "1970-01-01"), (86_400, "1970-01-02"),
                            (951_782_400, "2000-02-29"), (1_793_750_400, "2026-11-04"), (1_793_836_799, "2026-11-04")] {
        assert_eq!(utc_date(seconds), date, "{seconds}");
    }
}

#[test]
fn saved_live_records_still_parse() {
    let root = match std::env::var_os("LAZYMKV_EVIDENCE").filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/owner"),
    };
    if !root.is_dir() {
        return skip(&format!("no evidence clone at {}", root.display()));
    }
    let live = root.join("live-lookups");
    if !live.is_dir() {
        return skip("no live-lookups folder in the evidence clone; run the live tests on the mini PC (design part 8.5)");
    }
    let (today, mut read) = (today(), 0);
    for date in sorted(&live) {
        for file in sorted(&date) {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            let shown = format!("{}/{name}", date.file_name().unwrap().to_string_lossy());
            let text = std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{shown}: {error}"));
            if name.starts_with("tmdb-") && name.ends_with(".slim.json") && is_stale(fetched_at(&text).as_deref(), &today) {
                skip(&format!("{shown} is more than six months old; run the live tests again"));
                continue;
            }
            replay(&name, &text, &shown);
            read += 1;
        }
    }
    if read == 0 {
        skip("the live-lookups folder holds no record younger than six months");
    }
}
```

Below them, the replay's two helpers and `sorted`:

```rust
/// The folder's entries, sorted by path.
fn sorted(dir: &Path) -> Vec<PathBuf>;
/// Each record through core's parser by its name (Task 19's ten names): a `.shape.json` passes `shape_only`;
/// `thediscdb-dischash-*` gives Some by `parse_dischash`, `thediscdb-discid-*` a non-empty list by `parse_discid`;
/// `tmdb-search-*.slim.json` a non-empty page by `parse_search`; `tmdb-movie-*.slim.json` `parse_movie` Ok;
/// `tmdb-tv-<id>-season-<n>.slim.json` `parse_tv` with `episode_runtimes(Some(n))` non-empty; `tmdb-tv-<id>.slim.json`
/// `parse_tv` Ok; any other name fails with `<shown>: not a live record of design part 8.5`.
fn replay(name: &str, text: &str, shown: &str);
/// Fails unless every leaf of the shape is a string of shape_of's type names joined by `|` and every array holds at
/// most one element: no value of an answer reached the record.
fn shape_only(shape: &Value, shown: &str);
```

Each `#[ignore]` reason is the Global Constraints' text exactly.

- [ ] **Step 2: Run them to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked shape_records`
Expected: FAIL to compile, `error[E0425]: cannot find function `shape_of` in this scope` (saved.rs) and `error[E0432]: unresolved import `lazymkv_clients::shape_of`` (tests/live.rs).

- [ ] **Step 3: Implement `shape_of`**

Signature as above, in `saved.rs` after `status_text`, with `pub use saved::{shape_of, ...}` in `lib.rs:15`. `shape_of` itself is the match its doc and `shape_records_hold_names_and_types_only` fix (an array folds its elements' shapes through `merge_shapes`). Its merge, which the signature and the test do not fully fix:

```rust
/// The order a shape joins type names in (Ruling T4-R19).
const SHAPE_TYPES: [&str; 6] = ["object", "array", "string", "number", "boolean", "null"];

fn merge_shapes(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut a), Value::Object(b)) => {
            for (key, shape) in b {
                match a.get_mut(&key) {
                    // get_mut and take, never remove and insert: serde_json's preserve_order map would reorder keys.
                    Some(slot) => *slot = merge_shapes(std::mem::take(slot), shape),
                    None => drop(a.insert(key, shape)),
                }
            }
            Value::Object(a)
        }
        (Value::Array(a), Value::Array(b)) => Value::Array(a.into_iter().chain(b).reduce(merge_shapes).into_iter().collect()),
        (a, b) => {
            let names = |shape: &Value| -> Vec<String> {
                match shape {
                    Value::Object(_) => vec!["object".to_string()],
                    Value::Array(_) => vec!["array".to_string()],
                    other => other.as_str().unwrap_or_default().split('|').map(str::to_string).collect(),
                }
            };
            let (a, b) = (names(&a), names(&b));
            let joined: Vec<&str> = SHAPE_TYPES.into_iter().filter(|name| a.iter().chain(&b).any(|kept| kept == name)).collect();
            Value::from(joined.join("|"))
        }
    }
}
```

- [ ] **Step 4: Run them to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked shape_records`
Expected: PASS, `1 passed`.

Run: `scripts/box.sh cargo test -p lazymkv-clients --locked --test live`
Expected: PASS, `3 passed; 0 failed; 4 ignored`, and from `saved_live_records_still_parse` either `skipped: no evidence clone at C:\lazymkv\src\crates\clients\../../corpus/owner` or, when the mini PC's environment sets `LAZYMKV_EVIDENCE`, `skipped: no live-lookups folder in the evidence clone; run the live tests on the mini PC (design part 8.5)`. Nothing is asked of any service: the four live tests are listed `ignored`.

Run: `scripts/box.sh cargo test --workspace --locked`
Expected: PASS. CI runs the same three tests on the three systems and ignores the four (Global Constraints: CI never passes `--ignored`).

- [ ] **Step 5: Commit**

```bash
git add crates/clients/src/saved.rs crates/clients/src/lib.rs crates/clients/tests/live.rs crates/core/src/lookup/stale.rs crates/dev/src/evidence.rs
git commit -m "clients: the live shape tests of part 8.5"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, no attribution line; the draft's CI green on the three systems with 4 more ignored tests on each.

---

### Task 20: The mini PC end (A3, A6, A7, A8; manual)

Everything here runs on the mini PC or in Home Assistant; nothing is committed in this repository (Task 21 records it); Step 2's item 5 commits the live records in the evidence clone (owner item 17). Each step writes its evidence line to `.superpowers/pr4/pr4-mini-pc-runs.txt` (git-ignored) in engine.md's form `<yyyy-MM-dd>, <SSH | desktop task | owner>, <what ran>: <result>`, writing `<job id>`, `<film folder>`, `<ha host>` and `<id>` for the TMDb ID (Owner decision 18), and never a token, webhook ID or URL path, drive serial, account name, security identifier (SID) or MakeMKV key. A step that prints `skipped:` is recorded as not run. `BOX '<cmd>'` is `ssh -o BatchMode=yes "$LAZYMKV_BOX" '<cmd>'`, with `LAZYMKV_BOX` from `scripts/phase0/box.env`; `$SCRATCH` is the session's scratchpad. No `scripts/box.sh cargo ...` call runs from Step 4 until the job of Step 9 ends (each sync replaces `C:\lazymkv\src`; design part 12.5); Step 5's `box.sh` runs only `engine-task.ps1`, with the engine stopped and no job.

**Files:** `.superpowers/pr4/pr4-mini-pc-runs.txt`, `.superpowers/pr4/decisions.txt` (git-ignored). No code.

**Interfaces:**
- Consumes: Task 19's live tests and records; Task 14's `secret set`; Task 13's self-test details (`TMDb answered 200 to the token`, `<host>:<port> accepted a connection`, `not set; its notifications are skipped`); Task 12's `undelivered_line`; Task 11's `cleanup:` lines; Task 17's binary; PR 3's `scripts/engine-task.ps1 -AcceptWritableBuild` and its last line `ENGINE-TASK-DONE status=ok|error`.
- Produces: the evidence lines Task 21 copies, and the owner's answers to Owner decisions 12 to 16 and 18 to 21 in `decisions.txt` (`<item>, <yyyy-MM-dd>, <answer>`).

- [ ] **Step 0: Preconditions.** Tasks 1 to 19 pushed; `gh run list --repo krcm0209/lazymkv --branch phase-2-live-lookups --workflow CI --limit 1 --json conclusion,headSha` shows `success` on `git rev-parse HEAD`; `gh api "repos/krcm0209/lazymkv/code-scanning/alerts?ref=refs/pull/<n>/merge&state=open" --jq length` prints `0`; `decisions.txt` holds the owner's answers to items 12 to 16 and 18 to 21 (asked once, together, before this step; item 17 was answered at Task 0 Step 1). `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` exits 0 and its snapshot lists no job.

- [ ] **Step 1: The workspace, with the clone (SSH).**
  Run: `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked 2>&1" > .superpowers/pr4/pr4-test-run.txt`, then `scripts/box.sh "cargo test --workspace --locked -- --list 2>&1" > .superpowers/pr4/pr4-test-list.txt` (both in the git-ignored `.superpowers/pr4/`, where Task 21 reads them).
  Expected: every `test result:` line `ok` with `0 failed`; ignored 13 (PR 3's 9 and Task 19's 4); `skipped:` lines are PR 3's 39 plus `skipped: no live-lookups folder in the evidence clone; ...`. Evidence: `<date>, SSH, the workspace with the evidence junction on <short sha>: <p> passed, 0 failed, 13 ignored, <k> skipped: lines`. Task 21 reads both files.

- [ ] **Step 2: The live shape tests (SSH; Owner decision 17).**
  1. Run: `scripts/box.sh "if defined TMDB_TOKEN (echo TMDB_TOKEN set) else (echo TMDB_TOKEN NOT SET)"`. Expected: `TMDB_TOKEN set`. `NOT SET` stops here and goes to the owner (A6); the value is never printed.
  2. Run: `scripts/box.sh "set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test -p lazymkv-clients --locked --test live -- --ignored --test-threads=1"`. Expected: `4 passed; 0 failed`, no `skipped:` line. A failure names the request and the field; it goes to the owner as a finding against design part 8.5 (a parser that refuses a live answer reopens Task 2, 4 or 19).
  3. Run: `scripts/box.sh "set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test -p lazymkv-clients --locked --test live saved_live_records"`. Expected: `1 passed`, no `skipped:` line: today's ten records replay.
  4. `BOX 'findstr /s /m /c:"eyJhbGciOi" C:\lazymkv\evidence\owner\live-lookups\*'` prints nothing (exit 1): no token in a record. `BOX 'dir C:\lazymkv\evidence\owner\live-lookups\<yyyy-MM-dd>'` lists the ten names of Task 19 with sizes; the controller passes names and sizes to the owner.
  5. **Controller** (Owner decision 17, the owner's answer of 2026-11-04), on the Mac, before any re-mirror: `mkdir -p corpus/owner/live-lookups && scp -q -r -o BatchMode=yes "$LAZYMKV_BOX:C:/lazymkv/evidence/owner/live-lookups/<yyyy-MM-dd>" corpus/owner/live-lookups/`, then `git -C corpus/owner add live-lookups/<yyyy-MM-dd>` and `git -C corpus/owner commit -m "live-lookups: the shape records of <yyyy-MM-dd> (lazymkv pull request 4, T4-19)"`: one signed commit as `krcm0209 <17315774+krcm0209@users.noreply.github.com>`, no attribution line, touching only the new folder. `git -C corpus/owner log -1 --name-only --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G`, that identity, the subject alone and only paths under `live-lookups/<yyyy-MM-dd>/`. The controller does this itself; no subagent reads or writes the clone. The controller tells the owner in chat what was committed (the folder, the ten names and the commit's short sha).
  Evidence: `<date>, SSH, the live shape tests: 4 passed; requests dischash/76E63EBB5B66F73B0C37BFAB59B8ABA5, discid/8D0355F530C31D44912831F8637FF65ACBCD6886, search/multi PIRANHA and PIRANHA 3D, movie/245891, tv/1437, tv/1437/season/1; ten records under live-lookups\<date>; no token pattern; committed in the clone by the controller on <date>`.

- [ ] **Step 3: The release build (SSH).** Run: `scripts/box.sh cargo build --workspace --locked --release`. Expected: `Finished`. The last cargo command until Step 9's job ends. Evidence: the commit's short sha.

- [ ] **Step 4: Stop the running engine (SSH; Owner decision 20).** `BOX 'C:\lazymkv\run\lazymkvctl.exe stop'` exits 0; then `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` exits 4 with `lazymkvctl: no engine is running`.

- [ ] **Step 5: Redeploy (desktop task; Owner decision 20, PR 3's Owner decisions 3 and 13).** The owner is logged on at the mini PC's monitor (locked is enough). Run: `scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1 -AcceptWritableBuild'`. Expected: six `accepted:` lines (`C:\lazymkv`, `C:\lazymkv\src`, `C:\lazymkv\target`, `C:\lazymkv\target\release`, `lazymkv-engine.exe`, `lazymkvctl.exe`), then `ENGINE-TASK-DONE status=ok`, exit 0. Then `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` exits 0 with no job, and `BOX 'findstr /c:"cleanup:" "%ProgramData%\lazymkv\engine.log"'` prints nothing: the start's cleanup pass (T4-R11) found nothing to delete.

- [ ] **Step 6: The Home Assistant automation (Owner; A7, Owner decisions 14 and 16).**
  1. The controller reads the Home Assistant connector's best-practices skill (its server instructions require it), then shows the owner this YAML, word for word, and asks for a yes naming it:

```yaml
alias: lazymkv notifications
description: >-
  Shows each lazymkv engine notification (lazymkv spec 12, design part 8.6) as a persistent notification. The engine
  POSTs one JSON body per event; the webhook ID is a secret held only here and in lazymkv's SecretStore
  (ha_webhook_url).
mode: queued
max: 25
triggers:
  - trigger: webhook
    webhook_id: lazymkv-placeholder-replace-me
    allowed_methods:
      - POST
    local_only: true
conditions:
  - condition: template
    value_template: "{{ trigger.json is defined and trigger.json.get('event') is not none }}"
actions:
  - action: persistent_notification.create
    data:
      title: >-
        lazymkv: {{ trigger.json.event }}{% if trigger.json.get('title') %}: {{ trigger.json.title }}{% endif %}
      message: >-
        {{ trigger.json.get('message') or '' }}

        event: {{ trigger.json.event }}
        job: {{ trigger.json.get('job_id') or 'no job' }}
        {%- if trigger.json.get('reason') %}
        reason: {{ trigger.json.reason }}
        {%- endif %}
        {%- if trigger.json.get('reasons') %}
        reasons: {{ trigger.json.reasons | join(', ') }}
        {%- endif %}
        {%- if trigger.json.get('class') %}
        class: {{ trigger.json['class'] }}
        {%- endif %}
        time: {{ trigger.json.time | timestamp_local }}
```

  2. After the yes, the controller creates it with the connector's automation tool (`ha_config_set_automation`) and reads it back once (`ha_config_get_automation`): alias, mode and the placeholder ID as above. No `notification_id` is set, so each event is its own card.
  3. **Owner:** in Home Assistant's automation editor, opens `lazymkv notifications`, deletes the placeholder's webhook trigger and adds a new Webhook trigger (the editor generates a random ID for a new trigger), ticks POST only and local only, saves, and copies the full webhook URL. Until then the guessable placeholder ID is live, on the LAN only (`local_only: true`). It must be `http://<ha host>:<port>/api/webhook/<id>` on the LAN, or https with a publicly trusted certificate (a self-signed one is refused, design part 8.1). From here the controller never reads that automation's configuration or traces, which show the ID.
  Evidence: `<date>, owner, the Home Assistant automation lazymkv notifications created through the connector with the placeholder; the owner set its webhook ID in the editor`.

- [ ] **Step 7: The secrets (Owner; A6, A7, Owner decision 15).** At the mini PC's elevated PowerShell console, the owner runs `$env:TMDB_TOKEN | C:\lazymkv\run\lazymkvctl.exe secret set tmdb_token`, then enters the webhook URL with `C:\lazymkv\run\lazymkvctl.exe secret set ha_webhook_url` (paste, Enter, Ctrl+Z, Enter; or `Get-Clipboard | ...` when the URL was copied on the mini PC, then `Set-Clipboard -Value $null`). Each exits 0 and prints nothing. Nobody else types or sees either value; the controller asks for neither. `webhook_url` stays unset (A4).

- [ ] **Step 8: Status and self-test (SSH; Owner decision 19).**
  - `BOX 'C:\lazymkv\run\lazymkvctl.exe status'`: the snapshot's `secrets` shows `tmdb_token` set, `ha_webhook_url` set with host `<ha host>`, `webhook_url` not set.
  - `BOX 'C:\lazymkv\run\lazymkvctl.exe settings get'`: PR 3's settings unchanged (the share roots, `tool_paths`, `keep_tuning_evidence` true, `tv_root` unset), and `tmdb_only_autofile` true and `auto_threshold` 0.85 (the defaults, `crates/core/src/settings.rs:144-146`; with `tmdb_only_autofile` off a TMDb-only identity is capped at 0.84 and holds). A different value goes to the owner before Step 9.
  - `BOX 'C:\lazymkv\run\lazymkvctl.exe selftest'` exits 0 with eleven checks, all ok: `makemkvcon`, `mkvmerge`, `ffprobe`, `ffmpeg`, `java` with versions and places as PR 3's last self-test; the three roots; `tmdb_token` `TMDb answered 200 to the token`; `ha_webhook_url` `<ha host>:<port> accepted a connection`; `webhook_url` `not set; its notifications are skipped`. A `tmdb_token` detail `TMDb answered 401 to the token` goes back to the owner for Step 7 again (a trailing space, Owner decision 15).

- [ ] **Step 9: The disc (Owner; A3, A8, Owner decisions 12 and 13).**
  1. **Owner:** gives the titles they own (a photo or a list).
  2. The controller searches each on thediscdb.com (read only, no account) and keeps those it has no release of. For each kept title it reads a public disc listing (the retailer's or blu-ray.com's specifications) and keeps a Blu-ray (so both TheDiscDB lookups run) with at most one English subtitle track and no closed captions or SDH (subtitles for the deaf and hard of hearing) (S32: two or more fail Decide). It also searches each kept title on themoviedb.org (read only, no account) and keeps titles whose movie search shows exactly one result with that title and year (spec 8.1's plausibility rule), preferring a release whose disc title is the film's name, not a generic label. It proposes one, naming the three checks; the owner's yes names it.
  3. **Owner:** inserts it at the monitor and gives no command. The controller reads `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` every 2 minutes until the job is `Filed`, `NeedsReview` or `Failed`, and asks the owner to take the disc when it is ejected.
  4. `NeedsReview`: the reasons are recorded as a finding; on the owner's yes in chat, `BOX 'C:\lazymkv\run\lazymkvctl.exe approve <job id>'`, unchanged, and the reading goes on to `Filed`. `Failed`: the class and message are recorded, the run stops and goes to the owner. A job held without a final path, or one whose plan has no identity, is recorded as a finding; with the owner's yes, Step 9 runs again with another title.

- [ ] **Step 10: The run's evidence (SSH and the connector).** With `J` for `%ProgramData%\lazymkv\jobs\<job id>`:
  - `BOX 'type "J\transitions.jsonl"'`: the `new` states in order, ending `Filed` (after `NeedsReview` when held).
  - `BOX 'type "J\lookup-status.txt"'`: `dischash` and `discid` each `404` or `200` read as a miss (S46); `search 200`; `search-tagged 200` only when the name's forms differ; `tmdb movie/<id> 200`; no `search-entry` line and no `000`.
  - `BOX 'powershell -NoProfile -Command "(Get-Content -Raw J\state.json | ConvertFrom-Json).plan.identity.step"'` prints `tmdb_one` (spec 8.1 step 4).
  - `BOX 'findstr /c:"step 10a did not run in this version" /c:"share access for" "J\job.log"'`: the step 10a line and the share access lines.
  - Home Assistant: the persistent notifications whose titles start `lazymkv: disc_done` and `lazymkv: filed` (and `lazymkv: needs_review` if held), read through the connector if it lists them, else read by the owner in Home Assistant's notification panel and told in chat; each carries `event`, `job`, `time` and the message; the `filed` message names the filed path, recorded as `\\unas-pro\media\lazymkv-test-library\Movies\<film folder>\<file>`.
  - `BOX 'findstr /c:"was not delivered" /c:"cleanup:" "%ProgramData%\lazymkv\engine.log"'` prints nothing.
  - `BOX 'findstr /s /m /c:"eyJhbGciOi" "%ProgramData%\lazymkv\*"'` and `BOX 'findstr /s /m /c:"/api/webhook/" "%ProgramData%\lazymkv\*"'` print nothing (exit 1).
  - `BOX 'findstr /i /c:"unas-pro" "J\state.json" "J\job.log"'`: every path printed lies under `\\unas-pro\media\lazymkv-staging\engine\<job id>\`, `\\unas-pro\media\lazymkv-tuning\engine\` or the test library's `Movies` (design part 12.7's paths check).
  Evidence: one line per bullet, in shape.

- [ ] **Step 11 (optional, only with its own yes; Owner decision 21):** one of PR 3's several-playlist UHD discs; the controller stops the engine with `BOX 'C:\lazymkv\run\lazymkvctl.exe stop --force'` once the job is `Ripping`, records that it planned one main title (T4-R20), and restarts the task with `BOX 'powershell -NoProfile -Command "Start-ScheduledTask -TaskName lazymkv-engine"'`. Otherwise the line reads `not run: <answer>`.

- [ ] **Step 11a (optional, only with its own yes; A9):** after Step 9's job ends (and Step 11's, if it ran), `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` shows no active job; then `scripts/box.sh "set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo run -p lazymkv-dev --locked -- census"`; its lines are shown in the session only and never written to a file; the evidence line holds the exit code and the line count (RP21).

- [ ] **Step 12: The leftovers (each removed only by the owner).** Written to `pr4-mini-pc-runs.txt` as a list: the redeployed task `lazymkv-engine`; the Credential Manager entries `lazymkv:tmdb_token` and `lazymkv:ha_webhook_url` (`cmdkey /delete:lazymkv:<name>`; `secret unset` is phase 3's); the job folder `%ProgramData%\lazymkv\jobs\<job id>`; its staging data under `\\unas-pro\media\lazymkv-staging\engine\<job id>\` (kept by Owner decision 22 until pull request 5); the tuning data under `\\unas-pro\media\lazymkv-tuning\engine\`; the filed file in the test library; the Home Assistant automation and its notifications; the live records in the mirror until the next re-mirror (the clone keeps the owner's copy); the dated `live-lookups/<yyyy-MM-dd>` commit in the owner's clone (kept); `TMDB_TOKEN` in the mini PC's environment (A6, kept for the live tests); Step 11's job folder and its partial staging data, if Step 11 ran.

**Run:** as each step says. **Commit:** none.

---

### Task 21: The docs and the test count

**Files:**
- Modify: `docs/phase2/engine.md` (anchors at `6a862ac`): `:1-11` (opening paragraph), a new `## Pull request 4: live lookups and notifications` after `:50`, `:52-62` (How to run it), `:70`, `:71`, `:74` (the crates table), after `:129` (The Cargo.lock delta), `:131-138` (the Rulings introduction), four subsections after `:861`, `:906-918` and the groups after it (Left for later pull requests), a new `## Pull request 4 on the mini PC` after `:1027`, `:1028-1046` and the table (Tests).
- Modify: `README.md:23-31` (the phase 2 sentence), `:46-48` (the `dev` list), `:50-51` (the `clients` and `dev` clauses of the `crates/` bullet).

**Interfaces:**
- Consumes: `.superpowers/pr4/pr4-lock-delta-plan.txt`, `pr4-lock-delta.txt` and `decisions.txt` (A1's two yeses, Owner decisions 12 to 25, the answers on T4-R15, T4-R17, T4-R19 and T4-R21, any overturn of T4-R1 to T4-R23, each dated); each task's `Ruling T4-<n>-<letter>` and the controller's rulings made during execution; `pr4-mini-pc-runs.txt`; Task 20's `.superpowers/pr4/pr4-test-run.txt` and `.superpowers/pr4/pr4-test-list.txt`; Task 0 Step 4's `.superpowers/pr4/baseline-list.txt` (the branch point's `--list` count N0 and its per-crate counts, after A2); the draft pull request's number `<n>`; the plan's "Not covered".
- Produces: no code interface.

- [ ] **Step 1: Check the inputs.** `gh run list --repo krcm0209/lazymkv --branch phase-2-live-lookups --workflow CI --limit 1 --json databaseId,headSha,conclusion,url`: `success` on `git rev-parse HEAD`; `gh api "repos/krcm0209/lazymkv/code-scanning/alerts?ref=refs/pull/<n>/merge&state=open" --jq length` prints `0`; `pr4-mini-pc-runs.txt` holds a line for each of Task 20's Steps 0 to 12 (an optional step `not run: <answer>`).

- [ ] **Step 2: Count.** From `pr4-test-list.txt`: `tr -d '\r' < .superpowers/pr4/pr4-test-list.txt | grep -c ': test$'` gives N; `tr -d '\r' < .superpowers/pr4/pr4-test-list.txt | awk '/Running |Doc-tests /{h = $0} /: test$/{n[h]++} END {for (k in n) print n[k], k}'` gives per binary counts, summed per package into nine. From `pr4-test-run.txt`: `tr -d '\r' < .superpowers/pr4/pr4-test-run.txt | grep -E '^test result:' | awk '{p += $4; i += $8} END {print p, "passed", i, "ignored"}'` and `grep '^skipped:'`. From CI: `gh run view <id> --repo krcm0209/lazymkv --log | grep 'test result:' | sed -E 's/^test \(([a-z]+)-latest\).*test result: [A-Za-z]+\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored.*/\1 \2 \3 \4/' | awk '{p[$1] += $2; f[$1] += $3; i[$1] += $4} END {for (k in p) print k, p[k], "passed", f[k], "failed", i[k], "ignored"}'`. Expected: 0 failed everywhere; ignored windows 13, ubuntu 5, macos 5 (PR 3's 9, 1, 1 plus Task 19's four live tests on each).

- [ ] **Step 3: Write `docs/phase2/engine.md`.** Plain sentences, no em-dashes; acronyms new to the file expanded at first use (UTC, Coordinated Universal Time; HTTPS, HTTP over TLS; SDH, subtitles for the deaf and hard of hearing); this plan's tasks named `T4-<n>`; no secret, webhook ID, film title, TMDb ID of the owner's disc, drive serial, account name or SID. In file order:
  1. **Opening (`:1-11`).** After the PR 3 sentence: "Pull request 4, on the branch `phase-2-live-lookups` from `main` at `<branch point sha>`, adds its own sections, each named for it; its plan is `docs/superpowers/plans/2026-11-04-phase-2-pr4-live-lookups.md`, and it is pull request krcm0209/lazymkv#<n>."
  2. **`## Pull request 4: live lookups and notifications`** after `:50`. "Pull request 4 delivers design part 12.6 item 4:" then one bullet each: the live transport `UreqTransport` (ureq, rustls on ring, `webpki-roots`, 10 s, no redirect, no proxy, an 8 MiB cap, error words that never hold a URL or a header; T4-6, T4-7); design part 8.2's order whole: TheDiscDB by fingerprint and disc ID, J8's tagged second search and merge, the entry-title search (S47), the record with `external_ids`, a TV record's seasons (T4-2 to T4-5); part 8.3's outcome classes and the remembered identity in the chain (T4-5); stale TMDb copies fetched again before Plan, Decide and the relay's words, and K15's exit 3 in the relay and in `plan`, `decide` and `retune` (T4-8 to T4-10); the cleanup of part 5.9 at start and hourly (T4-11); the webhook notifier of part 8.6 and the self-test's webhook checks (T4-12, T4-13); `lazymkvctl`'s 4 MiB check and `secret set tmdb_token` (T4-14); dev's readers aligned with S46 and the search words, and `lazymkv-dev census` (T4-15, T4-16, A9); the Windows binary passing the live transport, the notifier and the cleanup (W26 replaced, T4-17); the live shape tests of part 8.5 (T4-19). Then one sentence from `pr4-mini-pc-runs.txt`'s Step 9 and Step 10 lines as they ran: the identity step, whether the job was held and approved (Owner decision 13), and the notifications Home Assistant showed (A3; Pull request 4 on the mini PC, below; CI run <url>).
  3. **How to run it (`:52-62`).** Add after `:61`: "The live shape tests (design part 8.5) run only on the mini PC: `scripts/box.sh "set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test -p lazymkv-clients --locked --test live -- --ignored --test-threads=1"`, with `TMDB_TOKEN` set there; they write dated records into the mirror's `live-lookups` folder, which the controller copies into the clone and commits there, owner item 17." / "The owner enters the TMDb token and the webhook URLs at the mini PC's elevated console: `$env:TMDB_TOKEN | C:\lazymkv\run\lazymkvctl.exe secret set tmdb_token`, and `lazymkvctl secret set ha_webhook_url` or `webhook_url` on standard input (A6)." / "`plan`, `decide` and `retune` over a recording folder fetch a TMDb copy more than six months old again with `TMDB_TOKEN`, and exit 3 naming it without the token (K15); an engine job folder is read as it is (T4-R10)." / "`lazymkv-dev census [--evidence DIR]` prints one line per recording of the clone; its output names the owner's discs and is never committed (A9)." / "A census session stops the engine with `lazymkvctl stop` over SSH before it and starts the task with `Start-ScheduledTask lazymkv-engine` after, each recorded here (A10)."
  4. **The crates table.** `:70` `lazymkv-clients`: "Depends on" gains "`ureq` (rustls on ring, `webpki-roots`)"; "Holds" drops "which the Windows binary passes until pull request 4 (W26)" and gains "; since pull request 4, the live `UreqTransport`, the merged searches, seasons and refetch (`refetch`), `shape_of`, `OfflineTransport` kept as a test double". `:71` `lazymkv-engine`: the cell's end "the `command` relay `relay` (S35) and `NoDelivery` |" becomes "the `command` relay `relay` (S35); since pull request 4, the `WebhookSink` notifier, the cleanup task `cleanup` and the refetch before Plan, Decide and the relay (`NoDelivery` removed) |". `:74` `lazymkv-dev`: "Depends on" becomes "`lazymkv-core`, `lazymkv-clients`, `serde`, `serde_json`" and "Holds" gains "; since pull request 4, the stale-copy gate `stale` and `census` (design part 4.1's `dev` row amended to `core`, `ocr` (R-A5) and `clients`, T4-R23)".
  5. **The Cargo.lock delta**, after `:129`: "On <date of A1's plan-time yes> the owner approved pull request 4's crate list, read from crates.io at Task 0, and on <date of A1's second yes> the `Cargo.lock` delta, read again on <date of Task 1's read>: <each crate, version and licence of `pr4-lock-delta.txt`>, and the path edge `lazymkv-dev -> lazymkv-clients` (Owner decision 25). The lock's package count went from <from> to <to>. Only `lazymkv-clients` depends on `ureq`, with default features off and rustls on the ring provider (design parts 4.7, 8.1)."
  6. **Rulings introduction (`:131-138`).** Add: "Pull request 4's plan recorded the planner's rulings T4-R1 to T4-R23 and, in its tasks, rulings numbered T4-<task>-<letter>."
  7. **Four subsections after `:861`:** `### Pull request 4: the planner's rulings (T4-R1 to T4-R23)` (one line each from the plan's Owner prerequisites item 26, T4-R15, T4-R17, T4-R19 and T4-R21 each with the owner's answer, each overturn with its date); `### Pull request 4: the task-level rulings` (a bold heading per task, `**Task T4-<n>** (<title>)`, one line per ruling); `### Pull request 4: the owner's decisions` (A1 to A11 of 2026-11-04 from `.superpowers/pr4/decisions.md`, then items 12 to 25, `- <n> (<subject>): <answer>, <date>`); `### Pull request 4: rulings made during execution` (one line each, or "None."), ending "The owner may overturn any of them."
  8. **Left for later (`:906-918`).** "**Pull request 4** (S40, S47): done. Each item and where it closed:" then, in order: `ureq` T4-1, T4-7; the live shape tests T4-19; J8 T4-2; the entry-title search T4-3; TV seasons T4-4 (filing stays phase 4, A5); webhook delivery and its self-test check T4-12, T4-13, T4-17; the refetch and K15's exit 3 T4-8 to T4-10 (`retune --jobs` keeps W25's refusal); the cleanup T4-11 (Owner decision 22's interim); dev's readers T4-15 (`hit-rate` aligned with S46 too, T4-R17 overturned by the owner on 2026-11-04); `ChainInputs` and the remembered identity T4-5 (T4-R6); the inner fingerprint T4-5, the unwritable copy accepted as T4-R21; the token out of error text T4-7; the 4 MiB check T4-14 (T4-R18); delivery and `secret set tmdb_token` T4-12, T4-14, T4-17, entered by the owner at T4-20; the several-playlist UHD discs T4-18 for an identified disc (T4-R20), the unidentified case open below. "Pull request 5" gains part 7.5's mirror of the cleanup's deletion into `<tuning_root>\recordings\<job id>\` and the end of Owner decision 22's interim. "Phase 3" gains a disc with several same-length playlists that TheDiscDB does not know (T4-R20) and the reviewed identity in `ChainInputs`. A new "**Accepted in pull request 4 and left as they are:**" group copies the plan's "Deliberately unchanged in PR 4" items, one line each.
  9. **`## Pull request 4 on the mini PC`** after `:1027`: "These runs were on the mini PC, each in the session it names; no secret, webhook ID, account name, user SID or drive serial is written, the owner's disc is named only as "a movie disc TheDiscDB does not know", and a run that printed `skipped:` counts as not run." Then one bullet per line of `pr4-mini-pc-runs.txt`, in order, ending with the leftovers list (Task 20 Step 12).
  10. **Tests (`:1028`).** A new first paragraph in PR 3's form: "`cargo test --workspace --locked -- --list` on the mini PC (Windows), on the tree of commit `<sha>`, lists N tests: <nine counts>. The baseline is N0 at `<branch point sha>` (Task 0 Step 4), so pull request 4 added <N minus baseline>: <per crate, net of the tests it replaced or removed, among them `no_delivery_refuses_every_notification_with_its_sentence` and `the_offline_transport_answers_every_url_unreachable`>. The junction run passed <p> and ignored <i>, the ignored list gaining `lazymkv-clients`' four live tests; on CI run <id> windows passed <p> and ignored 13, ubuntu passed <p> and macos <p>, each ignoring 5; CodeQL's analysis of `<sha>` found 0 results, and no alert is open." Then the `skipped:` lines beyond PR 3's 39, each with what ran instead (the replay's `no live-lookups folder` line: it ran at Task 20 Step 2.3). Table rows: `crates/clients/tests/live.rs` "The live shape checks of design part 8.5: `#[ignore]` requests to TheDiscDB and TMDb on the mini PC, the shape record, the public IDs, and the replay of the clone's saved records (T4-19)"; `crates/clients/src/live.rs` "The live transport over loopback listeners only: headers, status, timeout, no redirect, no proxy, the 8 MiB cap and error words that hold no URL or header (T4-7)"; `crates/engine/src/cleanup.rs` "The cleanup of design part 5.9 (T4-11)"; `crates/dev/src/cmd/census.rs` "The census over recordings built at test time from public files (A9, T4-16)"; and the clauses "; since pull request 4, <topics with the task>" on the rows of `engine_ipc.rs`, `engine_file_phase.rs`, `engine_restart.rs`, `engine_command.rs`, `engine_native.rs` and `whole_job.rs`.

- [ ] **Step 4: Edit `README.md`.** At `:28-31`, after "pipe and the Job Object runner", replace "; [`docs/phase2/engine.md`](docs/phase2/engine.md) says how to run them and how many tests there are." with: ". Its fourth asks TheDiscDB and TMDb live over its own HTTPS transport, keeps their answers slim and fetches a TMDb copy again after six months, cleans old copies and staging data, and posts every notification to the owner's Home Assistant webhook; [`docs/phase2/engine.md`](docs/phase2/engine.md) says how to run them and how many tests there are." At `:50-51`, "`clients` (TheDiscDB and TMDb behind an HTTP transport, replayed from recorded answers in tests)" becomes "`clients` (TheDiscDB and TMDb behind an HTTP transport: live over ureq and rustls, replayed from recorded answers in tests, with the `#[ignore]` live shape checks)", and the `dev` list "(`plan` and `decide`, ..." gains "the `census` report," after "the goldens".

- [ ] **Step 5: Run.** `BOX 'C:\lazymkv\run\lazymkvctl.exe status'` shows no active job (a `box.sh` sync must not meet a running job). Run: `scripts/box.sh cargo test --workspace --locked -- --list 2>&1 | tr -d '\r' | grep -c ': test$'`. Expected: N of Step 2 (this task adds no test). Then `scripts/box.sh cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 6: Commit and push.**

```bash
git add docs/phase2/engine.md README.md
git commit -m "docs: pull request 4's notes, rulings and test count"
git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'
git push
```
Expected: `G`, `krcm0209 <17315774+krcm0209@users.noreply.github.com>` as author and committer, no attribution line; CI green on the three systems and CodeQL at 0 for this push. The pull request stays a draft; marking it ready is the owner's call.

---

## Not covered

- **Pull request 5:** OCR and `ocr`'s build-script use of ureq; the evidence trees and part 7.5's tuning refresh (its mirror of the cleanup's deletion into `<tuning_root>\recordings\<job id>\`); the tuning evidence copy, after which Owner decision 22's interim ends; `retune --jobs` through the relay (W25's `RETUNE_WAITS` stays); `lazymkvctl licenses` with TMDb's attribution (part 9.6); the release layout and FFmpeg; MKVToolNix on the CI runners.
- **Pull request 6:** the acceptance checklist of part 12.7 (both acceptance discs, `commentary_detected`, two or more English subtitle streams, the Atmos or DTS:X profile against OR7, Jellyfin playback), `compare-job`, the empty test library.
- **Phase 3:** copy mode, and with it a disc with several same-length playlists that TheDiscDB does not know (T4-R20); `secret unset`; the review commands, and the reviewed identity (spec 11) carried in `ChainInputs`; deletion when space is short (AwaitingSpace's resolver).
- **Phase 4:** TV filing, `tv_root`, season sessions; the slim season keeping episode names (it keeps `episode_number` and `runtime` only).
- **Deliberately unchanged in PR 4:** dev commands other than `plan`, `decide` and `retune` that read `tmdb.json` (`titles`, `levels-args`, `labels`, `golden`, `census`) never fetch a stale copy; a `--tmdb-search` or `--tmdb-record` file given by hand, and any engine job folder, is read by dev as it is (T4-R10); dev reads a folder without `lookup-status.txt` as nothing pending where the engine's book would be pending; the replay of saved live records runs in `clients`' tests only (T4-R19, unless the owner's no at Task 0 Step 1 added core's), though design part 8.5 names `core` and `clients`; https webhook delivery and the self-signed refusal of design part 8.1 are exercised only live on the mini PC, CI posting http over loopback only; `retune --jobs` keeps W25's refusal (exit 2).
- **Deferred elsewhere:** libdvdread's DVD disc ID (LIB-6); a TLS handshake in the webhook check (T4-R14); `Retry-After` (a 429 waits for the next round); the rust-toolchain 1.99.1 bump (A2); any census session (A10).
