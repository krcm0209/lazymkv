# lazymkv

Insert a disc, walk away, and find the film or episodes filed on your media server. lazymkv drives
[MakeMKV](https://www.makemkv.com/) to rip Blu-ray, UHD and DVD discs on a small always-on Windows machine, works out
which title and which audio and subtitle tracks are wanted, remuxes once, and files the result under Jellyfin/Plex
naming on a network share. Rust, with a native desktop UI (GPUI Kit) and a headless engine; Linux and macOS are
intended later.

## Status

Design and evidence gathering. The design is in
[`docs/superpowers/specs/2026-10-04-lazymkv-design.md`](docs/superpowers/specs/2026-10-04-lazymkv-design.md); its
section 17 lists the experiments that phase 0 ran against real discs and real MakeMKV output, and the owner's decisions
(D1-D23) record what was settled along the way. The phase 0 report is
[`docs/phase0/REPORT.md`](docs/phase0/REPORT.md); each experiment has its own report in
[`docs/phase0/experiments/`](docs/phase0/experiments/). Phase 1 (the engine, with a developer mode that keeps every
rip's evidence for tuning) comes next.

## Layout

- `crates/` - the Rust workspace: `capture` (records tool output with timing, snapshots a disc, ejects),
  `fake-tools` (a stand-in that replays recorded MakeMKV output for tests), `core`.
- `scripts/phase0/` - the PowerShell jobs that ran on the ripping machine and the Python analyses that read their
  output (track comparison, subtitle text rules, audio content rules, the tuning-dataset harness), with unit tests:
  `python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'`.
- `corpus/` - recordings: MakeMKV scans and rip logs of the owner's discs, probe output, TheDiscDB and TMDb answers,
  pinned public scan logs, and replay scenarios. No disc content is stored here; subtitle text and audio proxies stay
  in a tuning dataset on the owner's NAS.
- `docs/` - the design spec, the phase 0 plan, the experiment reports and the environment notes.
- `experiments/ocrs-trial/` - the trial that compared the ocrs OCR engine with Tesseract on subtitle frames.

## Credits

This product uses the TMDB API but is not endorsed or certified by TMDB. Disc identification uses
[TheDiscDB](https://thediscdb.com/). Ripping is done by MakeMKV, which is not part of this repository.
