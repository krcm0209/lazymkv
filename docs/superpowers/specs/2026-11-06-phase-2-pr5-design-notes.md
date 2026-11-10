# Phase 2 pull request 5: tuning evidence and OCR, design notes

Written 2026-11-06 from the brainstorm with the owner, on `main` at 07ccc48 (pull request 4 merged). The binding
design is `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (parts 2.5, 4.1, 4.5, 4.7, 5.2, 7, 9.5, 9.6,
10, 11.1 to 11.4, 12.1 to 12.5; the owner's decisions P3, P17 to P22, P26, P29, P31, P33; rulings K1, R-A5) and, above
it, `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (sections 6.4 step 10 and 10a, 7, 8.7, 17 row 14). These
notes record what the owner decided for this pull request, the amendments those decisions make to the design, the
calls the planner settles, and what is left to the builder. The plan that follows them is written with
superpowers:writing-plans and executed subagent-driven, as pull request 4 was; its workspace rules are pull request 4's
(every cargo command on the mini PC through `scripts/box.sh`, one at a time, `--locked`; signed commits as
`krcm0209 <17315774+krcm0209@users.noreply.github.com>` with no attribution line; no secret, title of the owner's disc,
job id, host, user name or drive serial in the repository or the notes; RP21; no network on CI beyond what this
document names).

## 1. Intent

A job's file phase gains design part 6.4's steps 10 and 10a: the subtitle bitmaps of every English and no-language
subtitle stream are decoded and read by OCR (optical character recognition) in a worker process, and the audio of
every kept title is decoded into the compact proxy of part 7.6, so the text rule of spec 8.7 and the audio rules of
spec 8.6 decide with evidence instead of holding the job; the evidence lands in the two tuning trees of part 7.5, which
`retune` reads beside the owner's phase 0 clone (P22). The pull request also delivers `lazymkvctl licenses` and
`export-models` (part 9.6, 11.4), the ffmpeg driver and `probe-summary.json`, the zlib inflater, `retune --jobs`
through the relay (held since pull request 3, W25), and layer 2 of the tests (real `mkvmerge`, `ffprobe` and `ffmpeg`
on CI). The OCR engine is chosen by the trial of part 10.9 before the engine's step 10 is wired: ocrs against phase 0's
Tesseract reading, with the speed bar of P17 and P26.

Why now: pull request 4 filed a real disc with identity live, but every disc with two or more English subtitle streams
still holds for review (S32, S33), and every title's audio rules run without their proxy. Pull request 5 is what
makes the decision core's subtitle and audio rules real in the engine; pull request 7 (acceptance) needs it.

## 2. Scope: the owner's split (B1)

The design's part 12.6 item 5 is split in two. Pull request 5 is the engine side: step 10a's decode and the evidence
trees (part 7), the bitmap decoder and OCR with the trial first (part 10, 2.5), `licenses` and `export-models`,
`retune` over several evidence folders (P22, 7.7), and the rest of the notes' pull request 5 list in
`docs/phase2/engine.md` ("Left for later pull requests"): `probe-summary.json` and the `ffmpeg` driver,
`Note::SubtitleTextUnread` and `SubtitleFacts`' unread streams (10.8), the zlib inflater (`miniz_oxide`), dev's
`ocr-trial`, `retune`'s repeated `--evidence`, dev's reader of a bare `streamhash.txt`, the relay's
`--text <tuning_root>\dataset` and `retune --jobs` (W25's `RETUNE_WAITS` refusal removed), `lazymkvctl` making
retune's path flags absolute (V10), `lazymkv-ocr.exe` and its below-normal run with `engine-task.ps1` copying it,
layer 2 (G1), and part 7.5's mirror of the cleanup's deletion with the tuning copy that ends owner item 22's interim.

Pull request 6 is the release: the release layout and its `resources` folder (G1), the zip, the release workflow's dry
run on a branch (P28), and the minimal FFmpeg build with its once-only re-run of the reference tests and `retune` on
the mini PC (P16, P24, part 11). Pull request 7 is acceptance (part 12.7) with `compare-job` (G1). Part 12.6's
numbering is amended by these notes: items 5, 6 and 7 as above.

Goals: a UHD movie job with one English subtitle stream and a job with several run steps 10 and 10a and decide without
holding on S32 or S33; both evidence trees written and read by `retune` beside the clone; the trial's verdict recorded
in `docs/phase2/ocr-trial.md`; the pull request's test count and mini PC runs in `docs/phase2/engine.md` as pull
request 4's are. Non-goals: the release, the FFmpeg build, acceptance's checklist, any UI. Anti-goals: an OCR build in
every program (A2); a worker that writes to the share while it is timed (B1); a job that repeats an hour of decoding
because a lookup was slow (C3); a `retune` that fails for want of labels the owner has not given (D2); any clone value
in the repository (RP21).

## 3. The owner's decisions (2026-11-06; the git-ignored `.superpowers/pr5/decisions.txt` holds the dated lines)

- **A1. Crates.** Two yeses, as pull request 4's A1: at plan time to the list read from crates.io (the variant whose
  releases are all at least 14 days old), then to the written `Cargo.lock` delta before anything is fetched on the
  mini PC. The new crates: `ocrs =0.13.1`, `rten =0.26.0` (ocrs 0.13.1 needs rten 0.26; both pins stand), `png`,
  `sha2`, `ureq` for the `ocr` build script, `miniz_oxide`, and rten's transitive crates (about 15 to 25).
- **A2. rten out of every program.** `dev`'s link to the `ocr` crate sits behind a cargo feature `ocr-trial`, off by
  default and on only for the `lazymkv-dev` binary's trial builds; the engine reaches OCR only through the
  `lazymkv-ocr.exe` process (K1). CI builds the feature once so it does not rot. R-A5 is kept by this; design 4.1's
  sentence "dev depends on ocr for ocr-trial, which pulls rten into every dev build" is amended.
- **A3. Model pins.** The two SHA-256 values are computed on the mini PC from `C:\lazymkv\phase0\models` after their
  SHA-1 values match phase 0's; the pinned S3 URLs are downloaded once and must give the same bytes; a difference stops
  and goes to the owner (P20, 10.11).
- **B1. The worker's folder.** The worker reads and writes in a local work folder under the engine's data folder; the
  engine moves the finished output to the staged tuning folder (`<staging>\<job id>\tuning\<title key>\subs-ocr\`,
  10.2 step 3) after the run ends; a killed run's folder is deleted. The time cap (10.8) therefore counts no share
  write. About 150 MB of local disk per stream while it runs.
- **B2. The trial's inputs.** A track missing its Tesseract reading, `evidence.mks` or packet list stops the trial
  and goes to the owner (10.9 step 1); a track missing only phase 0's frame set lets the trial go on, with arm R'
  reported as not run for that track.
- **B3. The trial's rule.** Nothing is decided in advance: any fail (parity or speed) stops the pull request and goes to
  the owner with the numbers; the Tesseract route of 10.10 is then the owner's call.
- **C1. The clipped proxy.** As designed (7.6, spec 7): a title whose proxy would clip at -18 dB is made again from
  the staged file at -30 dB; nothing else changes.
- **C2. Owner item 22's interim.** One rule: a `Filed` job's staging data is held only while its tuning copy is
  pending; a job with no staged tuning folder has nothing pending. Pull request 4's one real job therefore goes to the
  cleanup once its retention has passed; the version-free rule replaces the interim.
- **C3. Re-running the unit.** Steps 10, 10a and 11 run as one unit (5.2). On a re-run (a retry after a Decide that
  could not fetch a stale TMDb copy, a restart), a title whose staged tuning folder is complete, that is
  `audio-proxy.json` present (written last) or, with `keep_tuning_evidence` off, `streamhash.txt`, and
  `subs-ocr\summary.json` present where step 10 runs, skips steps 10 and 10a; an incomplete folder is deleted and the
  steps run again. Spec 6.4 step 10a's "a retry after `tuning_evidence_pending` is a copy, not a second decode" is
  extended to every re-run with complete evidence.
- **D1. `retune` over several folders.** One report section per folder, in the order the `--evidence` flags were
  given; the exit code is the worst over the folders (3 above 1 above 0); `--snapshot` and `--compare` are allowed
  only with a single `--evidence` (exit 2 otherwise), so snapshots stay a clone tool.
- **D2. Unlabelled folders.** A folder with no `audio-labels.json` or `subtitle-labels.json` is reported as "no
  labelled track" and does not change the exit code while one folder of the run has labels; a run in which no folder
  has labels exits 3 as today.
- **E1. Crate notices.** A notices file generated with `cargo-about` (installed on one CI job, under P33) is committed
  and embedded in `lazymkvctl`; a ubuntu CI check regenerates it from `Cargo.lock` and fails when the file differs, so
  a dependency bump that changes a licence fails CI until the file is refreshed. `licenses` prints it with the rest of
  builder call 18.
- **E2. TMDb's attribution.** `licenses` prints TMDb's required sentence, "This product uses the TMDB API but is not
  endorsed or certified by TMDB.", with TMDb's address; the logo waits for the UI.
- **E3. The models' carrier.** As designed (4.1, 11.5): `lazymkvctl` embeds the two model files (12.2 MB) for
  `export-models` and `licenses`; it links the `ocr` crate's model bytes only, not rten.
- **F1. CI and OCR.** The decoder, preparation and line-finder tests in `core` run on all three systems; on ubuntu only,
  one run of `lazymkv-ocr.exe` over a hand-built Matroska file with made-up bitmaps (block shapes, no font) checks the
  records, `run.json`, the crops and the time-cap kill, never the text; one rten build and one model download cached
  by the models' SHA-256 values.
- **F2. The mini PC within pull request 5.** The decoder goldens on phase 0's six tracks (10.4), the OCR trial (10.9),
  and phase 0's kept UHD (Ultra HD Blu-ray) all-tracks rip replayed through the desktop task with step 10a on and `tuning_root` set, which writes both
  evidence trees and runs `retune --jobs` over them (12.1, K14); step 10's engine path meets real data only through
  the trial's arm R. No new disc in this pull request (P27's second disc is pull request 7's).
- **F3. The owner's labels.** Report only: after the trial, `retune --subtitles` runs over arm R's text against the
  phase 0 subtitle labels and its counts sit beside the verdict in `docs/phase2/ocr-trial.md`; no new gate.
- **G1. Homes for the split's leftovers.** Layer 2 (real `mkvmerge`, `ffprobe` and `ffmpeg` on lavfi files;
  MKVToolNix installed on the three CI runners, under P33) is pull request 5's; the release layout's `resources`
  folder is pull request 6's; `compare-job` is pull request 7's.
- **G2. `retune --jobs` and `--evidence`.** `--evidence` accumulates through the relay: `lazymkvctl retune --jobs
  --evidence <clone>` reads the engine's recordings and the clone in one run (P22, 7.7). Ruling W24's "last flag wins"
  stays for every single-valued flag and is amended for `--evidence` in the notes.
- **G3. Stale TMDb copies in the tuning recordings.** A relayed `retune` over `<tuning_root>\recordings` fetches a
  missing or stale `tmdb.json` into the job folder first (as `--job` already does, 9.5) and the tuning refresh copies
  it across; without a token it exits 3 naming the copy. Part 7.7's "fetches a stale copy again first, as every reader
  does" is read this way, and the tuning tree stays a mirror.

## 4. Constraints the plan inherits

- Rust 1.99.0 (`rust-toolchain.toml`; Dependabot #8's 1.99.1 waits for its own yes). Exact pins for ocrs and rten
  (D5, 4.5); everything else caret with `--locked`; `Cargo.lock` committed.
- The mini PC (Intel J4125, 4 cores, 7.8 GB, 119 GB SSD) is the only Windows build host; phase 0 built the ocrs trial
  crate there in release mode. The engine task `lazymkv-engine` runs pull request 4's build with both secrets set;
  it is stopped during the trial (10.9 step 3) and redeployed by `scripts/engine-task.ps1 -AcceptWritableBuild`
  after the release build, as pull request 4 did (the owner's decision 20).
- The engine starts every child with an empty environment (V1): the worker's gets `RTEN_NUM_THREADS=3` explicitly.
- Nobody reads or writes `corpus/owner` except the junction runs' read-only goldens, the live-record replay, and the
  owner; tests of several folders build their folders from made-up values (RP21); clone-reading tests skip without
  `LAZYMKV_EVIDENCE`.
- No non-ignored test contacts any host but `127.0.0.1`; the `ocr` build script's download runs on CI once per cache
  miss, from the pinned S3 URLs, and nowhere else.
- The goldens stay byte-identical (K5); `decide`'s output for every recording without an unread stream is unchanged.
- Every `pub` item carries its doc ending "Def: T5-<n>. Use: T5-<m>."; plain prose, no em-dashes, acronyms expanded
  at first use; one signed commit per task, pushed to the draft pull request; CI green on three systems and CodeQL at
  0 before the next task.

## 5. The planner's calls (settled here; the owner may veto any)

1. Pins: `ocrs = "=0.13.1"` with `default-features = false` and only the feature ocrs needs for the ONNX models;
   `rten = "=0.26.0"` with default features off; the rest caret.
2. The `ocr` build script reads `LAZYMKV_OCRS_MODELS` first, else downloads from the pinned S3 URLs over `ureq`; a
   SHA-256 mismatch or a missing network fails the build naming the variable; `scripts/box.sh` sets the variable to
   `C:\lazymkv\phase0\models` for the mini PC.
3. CI caches the two models with `actions/cache` keyed by their SHA-256 values and passes the folder as
   `LAZYMKV_OCRS_MODELS`.
4. If a release build of rten runs the mini PC short of memory, the build is retried with `CARGO_BUILD_JOBS=2` and
   the peak is recorded in the notes.
5. The worker's command line: `lazymkv-ocr <evidence.mks> --stream N --clear-max BYTES --out DIR`; exit 0 read, 2
   argument error, 3 the stream refused (codec, encoding, lacing, decode or ocrs error) with the reason on stderr.
6. The cap per stream is computed before the run from the event count of `subs\sub-<n>.packets.json` (packets above
   `sub_clear_max_bytes`): `max(60, 2.03 x events)` seconds (P17, P26); the note's "{k} of {m}" takes k from the
   records written.
7. The decoder goldens compare dark-pixel boxes exactly and grey values within 2 levels (10.4) and print numbers only.
8. During the trial the engine task is stopped with `lazymkvctl stop` and no cargo or `makemkvcon` runs; the trial's
   worker writes to C: and its output is copied to `subs-ocr-engine` after timing.
9. Step 10a's `ffmpeg` graph is ported from `scripts/phase0/audio-levels.ps1` unchanged; on the replay's heavy run the
   engine's level series must equal phase 0's byte for byte with the same FFmpeg 9.0.2; any difference goes to the
   owner.
10. The tuning copy runs after the unit's Decide as an engine-side step outside the reducer, retried at engine start
    and when `tuning_root` answers; only a pending dataset copy holds the staging data (C2); `TuningEvidencePending`
    stays in `state.json` with the copy's state and is marked done when the copy lands.
11. The recordings refresh copies only files whose size or modification time changed, never writes
    `audio-labels.json` or `subtitle-labels.json`, and deletes only TMDb copies it wrote earlier.
12. `--evidence DIR=TEXTDIR` splits at the first `=`; a single `--evidence` with `--text` keeps today's meaning; the
    phase 0 baseline check of `audio-dataset.json` runs for a folder that has one; the engine folder has none.
13. `lazymkvctl` makes retune's path flags absolute before relaying (V10) now that `--evidence`'s form is settled, and
    W25's `RETUNE_WAITS` refusal goes.
14. The relay adds `--text <tuning_root>\dataset` to `decide --job` once the job's dataset copy exists (9.5).
15. `licenses` prints, in this order: lazymkv's MIT text, the models' CC-BY-SA-4.0 text with design 11.4's
    attribution, TMDb's sentence (E2), the embedded crate notices (E1), and the bundled tools' licences only when a
    `licenses\` folder sits beside `bin\` (tested with a made-up folder).
16. `export-models DIR` refuses to overwrite an existing file and writes both files byte-identical to the embedded
    ones (a test checks the SHA-256 values).
17. `Note::SubtitleTextUnread` and `SubtitleFacts`' unread streams land in `core` first, with `decide`'s goldens
    unchanged for every recording without such a stream.
18. `engine-task.ps1` copies `lazymkv-ocr.exe` into the run folder beside the other two programs.
19. The trial's verdict and figures go into `docs/phase2/ocr-trial.md`, numbers only, and the ocrs row of spec 4.4
    gets the measured figures (10.9 step 5); F3's counts sit beside them.
20. Layer 2 runs on CI with MKVToolNix installed by the workflow (apt on ubuntu, brew on macos, winget or the
    MKVToolNix installer on windows, each pinned to a version the plan names) and FFmpeg at or above K11's 7.0 on every
    runner (ubuntu's apt FFmpeg is 6.1, so the workflow installs a static 7.x build there, pinned by URL and SHA-256).
21. Task order in the plan: crates and the model pins first (the owner's two yeses), then `core`'s decoders and
    preparation, the `ocr` crate and worker, the trial (a stop point), then step 10 and 10a in the engine, the evidence
    trees and the tuning copy, the cleanup's mirror, `retune` over several folders and the relay, `licenses` and
    `export-models`, layer 2, the mini PC runs, the docs.

22. A title whose proxy clips at -30 dB too (7.6: the dataset gets no proxy and `job.log` names the stream) still
    writes `audio-proxy.json`, recording the two gains tried and that no proxy was kept, so the staged folder reads
    complete (C3), the dataset copy counts as done (C2) and no re-run decodes it again; `retune` reports the title as
    unmeasured, as phase 0 does for a recording without a proxy.
23. The `ocr` crate is two packages: `lazymkv-ocr-models` (the two model files embedded through the build script of
    call 2, no rten; what `lazymkvctl` links for E3) and `lazymkv-ocr` (rten, ocrs and the worker binary). CI builds
    and runs the worker on ubuntu only (F1); the macos and windows jobs build and test the workspace with
    `--workspace --exclude lazymkv-ocr`; the Windows worker is built on the mini PC, where a Windows-only compile
    break of the worker is found at its first build there, before the trial.
24. The size cross-check of 10.3 (ffprobe's packet sizes against the reader's, once the encoding is undone) is
    confirmed at the plan's first golden check on the mini PC with FFmpeg 9.0.2 and on CI with each runner's FFmpeg; a
    mismatch stops the task and goes to the owner with the numbers, as A3 and B3 do; the builder never weakens the
    check to count and order alone.
25. G3 for a recordings folder whose engine job folder no longer exists (an orphan, such as a job the owner deleted):
    the relayed run fetches the stale or missing copy into the recordings folder itself, which is then the copy's
    only home, and the refresh leaves it alone; the relayed run completes every refetch before the child starts, so
    the child never reads a copy mid-write; an orphan whose fetch fails is reported as unmeasured, never an exit 3 for
    the whole run.
26. The notices file is generated with a `cargo-about` template that lists each crate's name, licence and the licence
    text, with no versions, so only a change in the crate set or a licence alters it; the ubuntu CI check prints the
    difference and uploads the regenerated file as a workflow artifact, and the refresh is one commit of that artifact
    (no `cargo-about` install on the Mac or the mini PC).

## 6. Left to the builder

The internal shapes of the decoders and the line finder (10.3 to 10.7 give the rules and thresholds); the exact
`SubtitleFacts` fields for unread streams within 10.8's words; the test fixtures' bitmaps; the layout of the worker's
`run.json` within 10.12 and K1/L-rulings; the retune report's per-folder header text; the notices file's name and
format as `cargo-about` emits it; the CI workflow steps for the caches and installs; the `docs/phase2/engine.md`
section form, which follows pull request 4's.
