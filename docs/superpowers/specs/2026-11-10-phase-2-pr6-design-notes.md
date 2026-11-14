# Phase 2 pull request 6: the release, design notes

Written 2026-11-10 from the brainstorm with the owner, on `main` at fd945ae (pull request 5 merged). The binding
design is `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (parts 2.4, 3.6, 4.5, 4.7, 5.3, 11, 12.6 and
15; the owner's decisions P13 to P16, P24, P28, P30 and P33; rulings K1, K4, K10, K11 and R-A2) as pull request 5's
notes amend it (`docs/superpowers/specs/2026-11-06-phase-2-pr5-design-notes.md`: B1 (scope), E1, G1), and, above it,
`docs/superpowers/specs/2026-10-04-lazymkv-design.md` (D17, D22, sections 4.4, 10.4, 13 and 14). These notes record
what the owner decided for this pull request, the amendments those decisions make to the design, the calls the planner
settles, and what is left to the builder. The plan that follows them is written with superpowers:writing-plans and
executed subagent-driven, as pull request 5 was; its workspace rules are pull request 5's (every cargo command on the
mini PC through `scripts/box.sh`, one at a time, `--locked`; signed commits as
`krcm0209 <17315774+krcm0209@users.noreply.github.com>` with no attribution line; no secret, title of the owner's disc,
job id, host, user name or drive serial in the repository, the ledger or the notes; RP21; no subtitle text in any file).

## 1. Intent

lazymkv gets the package a user installs: a zip with a fixed layout that holds the three programs, the tools they
drive (`mkvmerge.exe`, and an `ffmpeg.exe` and `ffprobe.exe` that lazymkv's own continuous integration (CI) builds
from FFmpeg's source), and every licence text, with the sources and the model files beside it (D17, P13, P24). The
release workflow that builds it runs as a dry run only: the zip, the sources and the models stay CI artifacts, no tag
is pushed and nothing is public (P28). The minimal FFmpeg build is checked once on the mini PC against phase 0's
numbers before any release may ship it (P16). Three smaller items ride along: the self-test judges MKVToolNix and
FFmpeg against their minimum versions (S80, T5-R65), the release takes the committed crate notices (E1, T5-R43), and
the optical character recognition (OCR) worker gets a bound on the memory its decoders can take (the review of pull
request 16's robustness thread).

Why now: pull request 5 made the engine's decisions real; what a user would install does not exist yet, the engine's
`resources` lookup has never met a real `resources` folder, and pull request 7 (acceptance) closes phase 2.

## 2. Scope

The "Pull request 6" paragraph of `docs/phase2/engine.md` ("Left for later pull requests"):

- the release layout and its `resources` folder in use (G1);
- the zip;
- the release workflow's dry run on a branch (P28);
- the minimal FFmpeg build, with its once-only re-run of the reference tests and `retune` on the mini PC (P16, P24,
  design part 11);
- the release job's use of the committed crate notices (E1, T5-R43);
- the self-test's check of spec 13's minimum versions of MKVToolNix and FFmpeg (S80, T5-R65);
- the bound on the subtitle decoders' allocation sizes and the zlib inflate, in the form the owner ruled (A1).

Goals: the release workflow's dry run green on the pull request, its zip a CI artifact; the FFmpeg build reproduced
byte for byte by a second run and verified on the mini PC, with the check recorded in `docs/phase2/engine.md`; the
zip's tree run on CI and once on the mini PC, its self-test finding the three bundled tools in `resources`; the
self-test judging the two minimums; the worker started under its memory limit, with its real peak measured;
`scripts/phase0/check-names.sh` printing nothing; the full junction run green on the mini PC with its test count in
the notes.

Non-goals: acceptance, `compare-job` and the second disc (pull request 7); a tag, a public release or a published
draft; code signing (P15; the step stays empty); an installer (phase 5); the two questions design part 15 keeps for
"before the first public release" (items 21 and 22); the desktop task running the release layout (C1); size caps
inside the decoders (A1); holding a job for a tool below its minimum (call 9).

Anti-goals: any edit in the frozen paths (T5-R54), a doc line included; a new crate or any change to `Cargo.toml` or
`Cargo.lock`; a release that can become public without the owner's own act; a release that can ship FFmpeg bytes the
mini PC never verified; a memory limit that changes what the worker reads; a title of the owner's discs, a host name,
a user name or subtitle text in a workflow log, an artifact, the repository or the notes (the workflows' logs and
artifacts are readable by anyone with a GitHub account).

## 3. The owner's decisions (2026-11-10; the git-ignored `.superpowers/pr6/decisions.txt` holds the dated lines)

- **A1. The form of the decoders' bound.** A memory limit on the OCR worker through the Job Object. No frozen file
  changes, so the trial is not reopened. A stream that meets the limit ends unread with `subtitle_text_unread`, as a
  worker abort does today. Design part 5.3's closed list of limits ("these limits and no others") gains this one.
  Size caps inside `crates/core/src/subtitle/bitmap/` and a limit on the inflate wait for the next reopened trial and
  stay listed under "Left for later pull requests".
- **A2. The size of the limit.** A fixed 2 GiB, a constant and not a setting, about a quarter of the mini PC's
  7.8 GB. The real worker's peak is measured on the mini PC and recorded in the notes; a peak above half the limit
  comes back to the owner before the number stands.
- **B1. How the branch reaches GitHub.** A draft pull request from the first task, as pull request 5: one signed
  commit per task, pushed; CI green on the three systems and CodeQL at 0 before the next task; marked ready for review
  only on the owner's word.
- **C1. Where the release tree runs.** The Windows release job extracts the zip into a folder only administrators can
  write, starts the engine and requires the self-test to find `mkvmerge`, `ffprobe` and `ffmpeg` in `resources`. The
  same zip runs once on the mini PC in the FFmpeg check's sitting and is removed afterwards. The desktop task keeps the
  installed tools (P24 and R-A2 stand).
- **D1. The publish path.** Written now. A `v<version>` tag creates a GitHub draft release, which only the owner can
  publish, and the path stops while the workspace version is 0.0.0. It never runs in this pull request and is reviewed
  by reading. Design part 11.5's step 6, "create the release", is amended to "create a draft release".
- **E1. A version the self-test cannot read.** A found tool whose version has no number to compare, such as an FFmpeg
  git build, passes its check, and the detail says the version could not be judged against the minimum; MakeMKV's
  check reads an unreadable version the same way (`below_minimum`).
- **F1. How the plan is written.** Inline with superpowers:writing-plans, with one Opus reviewer.

Design part 12.6 already requires each pull request to list its installs for approval before anything is fetched
(ENG-7), so no question was asked for it: the plan lists every pin of call 1 with its URL, version and SHA-256 (Secure
Hash Algorithm 256), and every action the two new workflows use, for the owner's one yes before the first workflow run.
The installs themselves were approved in kind on 2026-10-21 (P33).

The list as read on 2026-11-10, with no archive downloaded:

- FFmpeg 9.0.2 (2026-09-18, the newest release): `https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz`, SHA-256
  `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e`, the value Homebrew, FreeBSD ports and MSYS2 each
  publish for this file.
- zlib 1.3.2 (2026-02-17): `https://github.com/madler/zlib/releases/download/v1.3.2/zlib-1.3.2.tar.xz`, SHA-256
  `d7a0654783a4da529d1bb793b7ad9c3318020af77667bcae35f95d0e42a792f3`, the release asset's digest. zlib 1.4 is six
  days old and fails the 14-day age rule of pull requests 4 and 5; its notes name a fix in `inflateGetDictionary`,
  which the build's first task confirms FFmpeg's Matroska demuxer does not call, or the pin comes back to the owner.
- MKVToolNix 100.0: the portable archive
  `https://mkvtoolnix.download/windows/releases/100.0/mkvtoolnix-64-bit-100.0.7z`, SHA-256
  `061de38bd10e7e28697b897e0b890b78d6f2ec8d668a9c198600ed45c19672ab`, and the source archive
  `https://mkvtoolnix.download/sources/mkvtoolnix-100.0.tar.xz`, SHA-256
  `74480d07a261beeaa8baf898248e668ecc56335e2527bbffa841ef056dc028a1`, each from the checksum file beside it.
- The FFmpeg runner's apt packages, from Ubuntu 24.04's release pocket:
  `gcc-mingw-w64-x86-64-win32=13.2.0-6ubuntu1+26.1`, `binutils-mingw-w64-x86-64=2.41.90.20240122-1ubuntu1+11.4`,
  `mingw-w64-x86-64-dev=11.0.1-3build1`, `mingw-w64-common=11.0.1-3build1`, `nasm=2.16.01-1build1` and
  `make=4.3-4.1build2`.
- The two model files: the URLs and SHA-256 values of `crates/ocr-models/src/pins.rs` (pull request 5's A3).
- Actions: `actions/checkout@v4`, `actions/cache@v4` and `actions/upload-artifact@v4`, the majors `ci.yml` uses, and
  one new one, `actions/download-artifact@v4`. Newer majors exist; moving to them is its own change for all three
  workflows. The release job installs nothing: `windows-latest` already has 7-Zip and PowerShell 7.

## 4. Constraints the plan inherits

- T5-R54's freeze, from `164099d`: `crates/core/src/subtitle/bitmap/`, `crates/ocr/`, `crates/ocr-models/`,
  `crates/tools/src/ocr.rs`, `Cargo.toml` and `Cargo.lock`. This pull request edits none of them. It therefore adds no
  crate and no `windows-sys` feature that changes the lock, and the workspace version stays 0.0.0.
- `run_worker` in the frozen `crates/tools/src/ocr.rs` builds the worker's `Spawn` itself with `..Spawn::default()`
  and its own `ToolCtx` with every field named. A new `Spawn` field compiles without touching that file; a new
  `ToolCtx` field does not. The only way to change the worker's `Spawn` from outside is the `ProcessGuard` the caller
  hands in.
- Rust 1.99.0; every cargo command on the mini PC through `scripts/box.sh`, one at a time, `--locked`; the full run
  is the junction run (`cargo test --workspace --locked --no-fail-fast` with `LAZYMKV_EVIDENCE` and the `corpus\owner`
  junction).
- The mini PC is shared with the disc-menu session: a window is agreed by cross-session message before any use, and
  `scripts/box.sh` never runs while a desktop job or a capture sitting runs. The engine task `lazymkv-engine` has no
  boot trigger; it is stopped for the sitting of call 12 and started again after it.
- Nobody reads `corpus/owner` or the evidence mirror except junction cargo runs and scripts that run on the mini PC
  and print numbers only (RP21).
- The goldens stay byte-identical (K5). No non-ignored test contacts any host but `127.0.0.1`.
- CI is never polled: one bounded `gh run watch` in the background per push, read once.
- Every `pub` item carries its doc ending "Def: T6-<n>. Use: T6-<m>."; plain prose, no em-dashes, acronyms expanded
  at first use.
- Nothing is pushed as a tag, no release or draft is created, and the pull request is marked ready only when the
  owner says so (P28, B1).

## 5. The planner's calls (settled here; the owner may veto any)

1. **The manifest.** `packaging/tools.json` (design part 11.3) holds: the FFmpeg version, its tarball's URL on
   ffmpeg.org and SHA-256; the zlib source archive's URL and SHA-256; each apt package of the FFmpeg runner as
   `package=version`; the MKVToolNix 100.0.0 64-bit portable archive's URL and SHA-256, and its source archive's; the
   two model URLs and SHA-256 values; and the `ffmpeg.exe` and `ffprobe.exe` SHA-256 values with a `verified` field
   (call 4). JSON, because `jq`, PowerShell and `serde_json` all read it and no crate is added. One Rust test, outside
   the frozen paths, fails when the manifest's model pins differ from `crates/ocr-models/src/pins.rs`.
2. **The FFmpeg build.** `packaging/ffmpeg/build.sh` holds design part 11.2's configure line and is the authority for
   it. `.github/workflows/ffmpeg.yml` runs on a push that changes `packaging/`'s FFmpeg inputs or itself, by hand, and
   as a called workflow (`workflow_call`). Design part 11.3 runs it only when the pin or the build script changes; a
   change to the workflow file itself is added, since that file is part of what builds the bytes. It runs on the
   hosted `ubuntu-24.04` image and installs the cross-compiler, binutils and nasm with
   `apt-get install package=version` (P33); a pin apt cannot meet fails the job and goes to the owner. Design part
   11.3's "a pinned Ubuntu runner image" is read as that fixed label, never `ubuntu-latest`. GitHub rebuilds the image
   behind the label, so the image is not an input the manifest can name: if a rebuild ever changes the bytes, release
   step 3 fails on the SHA-256 values (call 4) and the build needs a new check before anything ships. A container
   image pinned by digest was weighed and left out, as a kind of install P33 does not name.
3. **Reproduced before it is kept (K4).** The script builds in one fixed folder that it names and installs zlib at a
   fixed place, so no path that varies reaches the configure arguments, which FFmpeg writes into both programs. It may
   add to design part 11.2's line the flags that point the build at that zlib; part 11.2 already makes the script the
   authority. The workflow builds twice from clean and fails unless both `ffmpeg.exe` and both `ffprobe.exe` are
   byte-identical; only then is the artifact uploaded, with the build settings file of design part 11.3 and the
   licence notice of the mingw-w64 runtime parts the link takes in.
4. **The two SHA-256 values and `verified`.** Design part 11.3 has the manifest record the two values after the mini
   PC check, and release step 3 fail unless the rebuilt files match them. Read strictly, no release dry run can pass
   before the check, and the check's sitting could not include the zip. So the manifest records the two values from
   the first reproduced build, with `"verified": null`; the mini PC check sets `verified` to its date and to a
   SHA-256 over the build's inputs (the FFmpeg, zlib and apt pins and the build script's bytes). Release step 3 always
   fails on a SHA-256 mismatch of either program. The workflows count the build as verified only while `verified`'s
   input value equals the one they compute, so a later change of the FFmpeg pin, the build script or a pinned package
   un-verifies the build by itself (design part 11.3, "Later upgrades"). While the build is not verified, the dry run
   passes with a warning that says so and the publish path refuses. This amends design part 11.3's "the manifest then
   records" and part 11.5's step 3, and keeps their purpose: a release ships only the bytes the check verified.
5. **The smoke job (design part 11.3).** A Windows job runs `-version` for both programs and
   `ffmpeg -hide_banner -layouts`, then the whole of layer 2 (`crates/fake-tools/tests/layer2.rs`) with the three
   programs that ship as the tools under test: the minimal `ffmpeg.exe` and `ffprobe.exe`, and the `mkvmerge.exe` of
   the manifest's portable archive, checked and extracted as release step 2 does. The runner's full FFmpeg only makes
   the lavfi input files, through a second, test-only variable. So the smoke job runs the engine's own command lines,
   not retyped shapes, and layer 2's zlib test shows `--enable-zlib` working before the SHA-256 values are recorded.
   A layer 2 test that cannot run on the minimal build for a reason other than a missing listed component stops the
   task and is reported. The job needs the model cache, since the test program links `lazymkvctl`'s crate.
6. **The release workflow.** `.github/workflows/release.yml` runs as a dry run on a pull request that changes
   `packaging/` or either new workflow, and by hand; a `v<version>` tag runs the same jobs and then the publish job
   (D1). It calls `ffmpeg.yml` for a fresh build and follows design part 11.5's six steps on `windows-latest`. Step 1
   is CI's first Windows build of `lazymkv-ocr` (call 23 of pull request 5 kept it on ubuntu and the mini PC), so the
   workflow's first task plans for a Windows-only break there and restores the model cache as `ci.yml` does. Only
   the publish job has `contents: write`. The publish job refuses unless the tag equals `v<workspace version>`, the
   version is not 0.0.0 and `verified` is set; it then creates a draft release with the assets of design part 11.4.
7. **The zip's name** is `lazymkv-0.0.0-windows-x64.zip`: the workspace version, which the freeze holds at 0.0.0. The
   first real version edits the frozen `Cargo.toml`; that is a ruling for the pull request that makes the first
   release, not this one.
8. **The licences folder.** The texts for `mkvmerge`, FFmpeg and zlib are taken at release time from the pinned,
   checked source archives, so each is the upstream's own file; the models' licence text and attribution, lazymkv's
   MIT text and the crate notices are the repository's files; the list of source assets by name and SHA-256 is
   written by the job. The model files attached as assets are written by the built `lazymkvctl export-models`, so
   they are the embedded bytes by construction (P20). The job fails unless `lazymkvctl licenses`, run from the
   assembled tree, prints the committed `crates/ctl/crate-notices.txt` and each file of `licenses\` (E1, T5-R43).
9. **The self-test's minimums.** Two constants in `core`, MKVToolNix 65.0 (K10) and FFmpeg 7.0 (K11), not settings.
   `mkvmerge` is judged against the first, `ffprobe` and `ffmpeg` each against the second; `java` is not judged. A
   version is read as an optional leading `n`, then one to three runs of digits joined by dots, ending at the end of
   the word or at a `-`, `+` or `~`; missing parts count as 0. Below the minimum, the check is not `ok` and its detail
   is `<tool> is version <found>, below the minimum <minimum>`; unreadable, it is `ok` with a detail that says so
   (E1). A found tool that prints no version at all stays `ok`, as S80 has it today, and its detail gains the same
   clause: it could not be judged against the minimum. (MakeMKV's check differs there and is unchanged: no version is
   not `ok`, and a version it cannot read is not below the minimum.) A tool below its minimum fails the self-test
   only: no job is held for it, since design part 11.5 has the self-test report it; holding a job is left for phase
   3, with the other watchdogs.
10. **The memory limit's plumbing (A1, A2).** `Spawn` gains `memory_limit: Option<u64>`, in bytes, None by default.
    The Windows runner sets it on the run's Job Object as the job's committed-memory limit, which covers the worker's
    whole tree; elsewhere it does nothing, as `below_normal` does where the platform has no such class. A guard that
    wraps another and sets the field lives in `lazymkv-platform`; step 10 hands `run_worker` that guard, so the frozen
    driver is untouched, and `lazymkv-dev ocr-trial read` hands it the same guard, so a later trial measures what
    ships. No other tool gets a limit. The constant is `2 * 1024 * 1024 * 1024`. job.log's line for a worker that
    exited with a code names the limit in force.
11. **The limit, the shipped worker and the trial.** The limit is outside the frozen paths and changes nothing the
    worker reads unless it is met. The zip's `lazymkv-ocr.exe` is built on a CI runner, while the trial read with the
    mini PC's own build of the same frozen source. One run in the sitting answers for both: the zip's worker, its
    SHA-256 checked against the zip's, is started through `lazymkv-dev ocr-trial read` under the limit over the
    trial's six tracks. Its peak committed memory per track is sampled and recorded, and the largest must be under
    1 GiB (A2). Each track's records are compared with the trial's kept arm R output and printed as equal or
    different. Row 4's wall time is reported against P26's 600 s. A difference, a peak above 1 GiB or a row 4 time
    above 600 s stops and goes to the owner with the numbers; none reopens the trial by itself. If the trial's arm R
    output is no longer in place, the comparison is reported as not run and the other two rules decide.
12. **One sitting on the mini PC**, in the desktop session with the owner logged on and the engine task stopped, after
    the release dry run is green with `verified` null, each part with the owner's yes: (a) design part 11.3's one-off
    check on the FFmpeg workflow's artifact, whose two SHA-256 values must equal the manifest's before anything runs:
    `lazymkv-dev golden check`, step 10a's measures over phase 0's kept UHD (Ultra HD Blu-ray) all-tracks rip with
    `audio-levels.ps1` against phase 0's level files, stream hashes and `ffprobe` listings, `retune` over the phase 0
    corpus with the new level files, and the step 10 packet listing against the full build's; (b) the dry run's zip,
    extracted into a folder whose whole tree is owned by Administrators and carries the protected access list of
    design part 3.5, run as a console engine with a scratch data folder: its self-test must find `mkvmerge`,
    `ffprobe` and `ffmpeg` in `resources`; (c) call 11's run of the zip's worker. The scripts print numbers and equal
    or different only. Everything copied for the sitting is removed after it, and the engine task is started again.
    Any difference in (a) stops the check and goes to the owner with the differing values (design part 11.3). After a
    pass, one commit sets `verified`, and the next dry run must be green without the warning.
13. **The CI smoke run of the zip (C1).** The release job extracts the zip into a new folder, makes Administrators
    the owner of the whole tree and gives it an access list that lets only SYSTEM and Administrators write (an owner
    other than those would count as able to write, W17), starts `bin\lazymkv-engine.exe --data <scratch>`, runs
    `bin\lazymkvctl selftest`, and requires the three tool checks to be `ok`, found in `resources`, at mkvmerge 100.0
    and FFmpeg 9.0.2; then `lazymkvctl stop`. MakeMKV is not on the runner, so the self-test as a whole is not
    required to pass. If the engine program cannot start on a hosted runner, the plan's first task on the release
    workflow finds that out and the step falls back to the tool lookup's own test over the extracted tree, with the
    mini PC run of call 12 (b) as the engine's run.
14. **Artifacts** are kept 14 days. The FFmpeg artifact the mini PC check verified need not outlive that: a rebuild
    reproduces it, and the manifest holds its SHA-256 values.
15. **Task order in the plan:** the memory limit; the self-test's minimums; the manifest, the build script and
    `ffmpeg.yml` with its first reproduced artifact (the owner's yes to the pin list first); `release.yml` and the dry
    run with the CI smoke run; the sitting (call 12); `verified`; the design's amendments, the notes and the test
    counts; the final junction run.
16. **The record.** `docs/phase2/engine.md` gets a "Pull request 6" section, the planner's rulings as T6-R1 onward,
    each marked "not yet ruled on by the owner" until the owner answers, "Pull request 6: the owner's decisions",
    "Pull request 6 on the mini PC" and the measured test counts, as pull request 5's. The design is amended in place,
    each change marked with these notes' id: part 5.3's list of limits (A1); part 11.3's runner image and its "when
    it runs" (call 2), its manifest sentence (call 4) and its smoke test (call 5); part 11.5's steps 3 and 6 (call 4,
    D1); and S80's sentence in the notes (T5-R65 closed).

## 6. Left to the builder

The manifest's key names; the build script's layout within design part 11.2's configure line; the workflows' step
lists, caches and job names; the names of the files in `licenses\`; the wording of the self-test's details within call
9's sentences; the name of the wrapping guard and of the test-only variable of call 5; the sitting's script names and
their output form within "numbers and equal or different only"; the sampler that reads the worker's peak; the
`docs/phase2/engine.md` section form, which follows pull request 5's.
