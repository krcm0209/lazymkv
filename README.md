# lazymkv

Insert a disc, walk away, and find the film or episodes filed on your media server. lazymkv drives
[MakeMKV](https://www.makemkv.com/) to rip Blu-ray, UHD and DVD discs on a small always-on Windows machine, works out
which title and which audio and subtitle tracks are wanted, remuxes once, and files the result under Jellyfin/Plex
naming on a network share. Rust, with a native desktop UI (GPUI Kit) and a headless engine; Linux and macOS are
intended later.

## Status

Phase 1, the decision core, is built. The design is in
[`docs/superpowers/specs/2026-10-04-lazymkv-design.md`](docs/superpowers/specs/2026-10-04-lazymkv-design.md); its
section 17 lists phase 0's fourteen experiments against real discs and real MakeMKV output, with what each found (five
were not run, and say why), and the owner's decisions (D1-D23) record what was settled along the way. The phase 0 report is
[`docs/phase0/REPORT.md`](docs/phase0/REPORT.md); each experiment has its own report in
[`docs/phase0/experiments/`](docs/phase0/experiments/). Phase 1 first ported phase 0's Python analysers to Rust
([`docs/superpowers/specs/2026-10-13-rust-port-design.md`](docs/superpowers/specs/2026-10-13-rust-port-design.md)),
then built the rest of the decision core from the design spec: the disc model, identification from TheDiscDB and TMDb
answers, the film and TV title rules, the audio and subtitle track rules, naming, the check of a rip against its scan,
the confidences and the disposition, and the job state machine. `lazymkv-dev plan` prints a disc's plan (from a
recording or a pinned public scan log) and `lazymkv-dev decide` a rip's decisions, as JSON, on any OS and without a
disc or a network; [`docs/phase1/decision-core.md`](docs/phase1/decision-core.md) says how to run them, the owner's
rulings and what may still be overturned, what is left for phase 2 and how many tests there are. Phase 2 builds the
direct-mode engine
([`docs/superpowers/specs/2026-10-21-phase-2-engine-design.md`](docs/superpowers/specs/2026-10-21-phase-2-engine-design.md)),
with a developer mode that keeps every rip's evidence for tuning. Its second pull request runs a whole job on stand-in
tools on any OS, from a disc's arrival to a filed placeholder file, with no disc and no network. Its third adds the
Windows pieces: the engine binary behind its mutex and protected named pipe, `lazymkvctl`'s pipe client, the share
connection, Credential Manager secrets and the desktop task script, so on Windows the same job runs through the real
pipe and the Job Object runner; [`docs/phase2/engine.md`](docs/phase2/engine.md) says how to run them and how many
tests there are.

Installing (planned; decision D17): one installer carries every tool the app needs except MakeMKV: mkvmerge and
ffmpeg/ffprobe, and the ocrs OCR engine compiled in with its models (decision D22); nothing is downloaded at run time.
MakeMKV cannot be redistributed, so it is never shipped: the app detects it, checks its version and says what to
install.

## Layout

- `crates/` - the Rust workspace: `capture` (records tool output with timing, snapshots a disc, ejects), `fake-tools`
  (the stand-in for `makemkvcon`, `mkvmerge`, `ffprobe`, `ffmpeg` and `java`, chosen by its file name, which replays a
  scenario's responses, and every test that runs it), `core` (the decision rules, with no I/O: the robot-output parser
  and the typed disc model, identification, the title planner, the audio and subtitle track rules, the namer, the
  verification of a rip, confidence and disposition, the job state reducer, the persisted job and drive records and the
  IPC messages, the `makemkvcon` line reader, and phase 0's track reconciler, audio content rule and subtitle text rule)
  and `dev`, the `lazymkv-dev` binary that runs them over recordings (`plan` and `decide`, scan summaries, track
  comparison, the tuning-dataset harness `retune` for audio and, with `--subtitles`, for subtitles, the audio and
  subtitle labels, the phase 0 readers, the goldens), and phase 2's `platform` (the operating-system traits, their
  Windows implementations and in-memory fakes), `tools` (the `makemkvcon`, `mkvmerge` and `ffprobe` drivers, their
  recorded runs and the disc snapshot's file input and output), `clients` (TheDiscDB and TMDb behind an HTTP transport,
  replayed from recorded answers in tests), `engine` (the `lazymkv-engine` job owner, its persistence, its IPC server,
  its Windows start-up and the `command` relay) and `ctl` (`lazymkvctl`: the local `plan`, `decide` and `retune`, and
  the engine commands, over the named pipe on Windows). Tests: `cargo test --workspace`; with the evidence clone at
  `corpus/owner/` they also compare the Rust with the goldens phase 0's Python wrote, and without it those comparisons
  print `skipped: ...` and pass. The port was built and tested on the mini PC (Windows) during the port; the CI workflow
  (`.github/workflows/ci.yml`) then built and tested it on Linux, macOS and Windows runners, all green (run 38910414789
  on cec66c1), so the cfg(unix) code (runner.rs's signal exit status, audio_report.rs's symlink test, the `date` clock
  path, the Unix-only assertions in align.rs, ocr_compare.rs and labels/tests.rs) has compiled and run. The decision
  core's tests, where each part is tested and how many tests there are, are in
  [`docs/phase1/decision-core.md`](docs/phase1/decision-core.md), and phase 2's in
  [`docs/phase2/engine.md`](docs/phase2/engine.md).
- `scripts/` - `box.sh`, which mirrors the working tree to the mini PC and runs one command there; `engine-task.ps1`,
  which copies the engine into `C:\lazymkv\run` and starts it as the desktop task `lazymkv-engine`; and `phase0/`, the
  PowerShell jobs that run on the ripping machine (`platform-ignored.ps1` among them, the desktop-session run of the
  platform's ignored tests) and the Mac's shell helpers that sync, start and wait for them and pull their recordings.
  Phase 0's Python analyses are now `crates/core` and `lazymkv-dev`;
  [`docs/phase1/rust-port-tests.md`](docs/phase1/rust-port-tests.md) maps each of their tests to its Rust port.
- `corpus/` - recordings: pinned public scan logs, TheDiscDB's API answers and replay scenarios. The owner's
  recordings (MakeMKV scans and rip logs of the owner's discs, probe output, TheDiscDB and TMDb answers) are the private
  evidence repository [`lazymkv-evidence`](https://github.com/krcm0209/lazymkv-evidence), cloned at `corpus/owner/`
  and ignored by this repository. No disc's audio, video or subtitle streams are stored in either; subtitle text and
  audio proxies stay in a tuning dataset on the owner's NAS.
  Before a commit the corpus is checked for tokens, unit-key files, machine user names and any path under
  `corpus/owner/` (`scripts/phase0/check-names.sh`), as [`corpus/README.md`](corpus/README.md) describes.
- `docs/` - the design spec and the port's design, the implementation plans, the experiment reports, the environment
  notes, the phase 1 notes (`docs/phase1/`: the port's test mapping and the decision core) and the phase 2 notes
  (`docs/phase2/`: the engine).
- `experiments/ocrs-trial/` - the trial that compared the ocrs OCR engine with Tesseract on subtitle frames.

## Credits

This product uses the TMDB API but is not endorsed or certified by TMDB. Disc identification uses
[TheDiscDB](https://thediscdb.com/). Ripping is done by MakeMKV, which is not part of this repository.

## Licence

MIT; see [`LICENSE`](LICENSE). The ocrs OCR models the app will ship are CC-BY-SA-4.0 and keep their own licence;
MakeMKV is proprietary and is not redistributed.
