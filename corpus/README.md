# Corpus

Recorded inputs for lazymkv's tests and analyses. No disc's audio, video or subtitle streams are stored in the corpus,
the evidence repository included: subtitle text and audio proxies stay in the tuning dataset on the owner's NAS
(decision D16).

- `scenarios/` - replay scenarios for the stand-in executables (`crates/fake-tools`): a scenario file per tool
  (`makemkvcon64.json`) and the recorded output it replays with its original timing. `disc-a-scan/` is Disc A's online
  scan, with the machine user name in it replaced by `USER` and the drive's serial number by `SERIAL`. Public, part of this repository.
- `thediscdb/` - real `makemkvcon` scan logs with TheDiscDB's parsed form and human-labelled title types, pinned from
  TheDiscDB's data repository; the `core` tests read them. MIT-licensed: see its `README.md` and `LICENSE`.
- `thediscdb-api/` - TheDiscDB's API answers for those discs, saved on 2026-10-04; its `README.md` gives their source
  and terms.
- `owner/` - the owner's disc recordings from phase 0: the private repository
  [`lazymkv-evidence`](https://github.com/krcm0209/lazymkv-evidence), cloned in place and ignored by this repository
  (`.gitignore`). Its own `README.md` documents the recording layout (spec section 15, "Recordings"). Every path
  written as `corpus/owner/<slug>/...` in this repository's documents is a path inside that clone.

## Why `owner/` is private

The recordings carry:

- verbatim playlist and navigation files copied from commercial discs (each recording's `nav/` folder);
- TMDb API answers (`tmdb.json`), which are TMDb's data under TMDb's API terms of use, not under this repository's
  MIT licence. Each is slimmed to the fields the scripts read, the ids, the work's title and date and its `fetched_at`
  day (`scripts/phase0/record-disc.ps1` does it after the fetch), and is fetched again when it is older than six
  months, the longest TMDb's terms let a copy be kept (spec 8.1);
- the owner's drive and collection details: which discs the owner has, and the drives and machine that read them.

## Working with it

With access, clone it in place: `git clone https://github.com/krcm0209/lazymkv-evidence corpus/owner`. The tools
use it there: `lazymkv-dev retune` (the audio tuning harness; `--evidence DIR` or `LAZYMKV_EVIDENCE` names another
folder) and `lazymkv-dev labels` (the owner's audio labels, given `corpus/owner/<slug>` paths) read and write it,
the other `lazymkv-dev` commands that take a slug and the golden tests of `cargo test --workspace` read it, and
`scripts/phase0/pull-recording.sh <slug>` copies a recording from the mini PC into it. Without the clone, a
`lazymkv-dev` command given a slug says so and exits 3, the golden tests print `skipped: ...` and pass, and
`pull-recording.sh` stops before it touches the mini PC. A pulled recording is committed in the evidence repository
(`git -C corpus/owner add <slug> && git -C corpus/owner commit`), never here.

Contributors without access record their own discs with the developer mode (`keep_tuning_evidence`, decision D16) and
keep those recordings out of this repository too. Nothing under `corpus/owner/` is ever committed here:
`scripts/phase0/check-names.sh` fails on any path under it that is tracked, staged or committed at `HEAD`.

## Checks before a commit

Each of these prints nothing when all is well:

- `scripts/phase0/check-names.sh`: no tracked or staged file names a machine user (the mini PC's Windows user or this
  Mac's), and no path under `corpus/owner/` is part of this repository. It checks this repository's files only.
- `grep -rIlE 'eyJhbGciOi[A-Za-z0-9_-]{20,}' corpus docs/phase0 docs/superpowers/specs`: no TMDb token.
- `find corpus/scenarios -iname '*unit_key*'`: no unit-key file (`AACS/Unit_Key_RO.inf` is never stored anywhere;
  a recording's `meta.json` holds only its SHA-1). Before a commit in the evidence repository, run
  `find corpus/owner -iname '*unit_key*'` there too.
