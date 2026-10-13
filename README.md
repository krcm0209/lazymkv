# lazymkv

Insert a disc, walk away, and find the film or episodes filed on your media server. lazymkv drives
[MakeMKV](https://www.makemkv.com/) to rip Blu-ray, UHD and DVD discs on a small always-on Windows machine, works out
which title and which audio and subtitle tracks are wanted, remuxes once, and files the result under Jellyfin/Plex
naming on a network share. Rust, with a native desktop UI (GPUI Kit) and a headless engine; Linux and macOS are
intended later.

## Status

Design and evidence gathering. The design is in
[`docs/superpowers/specs/2026-10-04-lazymkv-design.md`](docs/superpowers/specs/2026-10-04-lazymkv-design.md); its
section 17 lists phase 0's fourteen experiments against real discs and real MakeMKV output, with what each found (five
were not run, and say why), and the owner's decisions (D1-D23) record what was settled along the way. The phase 0 report is
[`docs/phase0/REPORT.md`](docs/phase0/REPORT.md); each experiment has its own report in
[`docs/phase0/experiments/`](docs/phase0/experiments/). The engine comes next (the spec's phases 1 and 2: the decision core, then the
direct-mode engine), with a developer mode that keeps every rip's evidence for tuning.

Installing (planned; decision D17): one installer carries every tool the app needs except MakeMKV: mkvmerge and
ffmpeg/ffprobe, and the ocrs OCR engine compiled in with its models (decision D22); nothing is downloaded at run time.
MakeMKV cannot be redistributed, so it is never shipped: the app detects it, checks its version and says what to
install.

## Layout

- `crates/` - the Rust workspace: `capture` (records tool output with timing, snapshots a disc, ejects),
  `fake-tools` (a stand-in that replays recorded MakeMKV output for tests), `core`.
- `scripts/phase0/` - the PowerShell jobs that ran on the ripping machine and the Python analyses that read their
  output (track comparison, subtitle text rules, audio content rules, the tuning-dataset harness), with unit tests:
  `python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'`.
- `corpus/` - recordings: pinned public scan logs, TheDiscDB's API answers and replay scenarios. The owner's
  recordings (MakeMKV scans and rip logs of the owner's discs, probe output, TheDiscDB and TMDb answers) are the private
  evidence repository [`lazymkv-evidence`](https://github.com/krcm0209/lazymkv-evidence), cloned at `corpus/owner/`
  and ignored by this repository. No disc's audio, video or subtitle streams are stored in either; subtitle text and
  audio proxies stay in a tuning dataset on the owner's NAS.
  Before a commit the corpus is checked for tokens, unit-key files, machine user names and any path under
  `corpus/owner/` (`scripts/phase0/check-names.sh`), as [`corpus/README.md`](corpus/README.md) describes.
- `docs/` - the design spec, the phase 0 plan, the experiment reports and the environment notes.
- `experiments/ocrs-trial/` - the trial that compared the ocrs OCR engine with Tesseract on subtitle frames.

## Credits

This product uses the TMDB API but is not endorsed or certified by TMDB. Disc identification uses
[TheDiscDB](https://thediscdb.com/). Ripping is done by MakeMKV, which is not part of this repository.

## Licence

MIT; see [`LICENSE`](LICENSE). The ocrs OCR models the app will ship are CC-BY-SA-4.0 and keep their own licence;
MakeMKV is proprietary and is not redistributed.
