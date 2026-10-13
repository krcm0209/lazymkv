# Rust port of the phase 0 analysers: design

Date: 2026-10-13. Status: draft for the owner's review. Parent: `2026-10-04-lazymkv-design.md` (the design spec; its decisions D1 to D23 bind this document). Owner decisions taken while brainstorming this document (2026-10-13): the repository becomes Rust plus the mini PC's PowerShell jobs; the port is one pull request; of the fifteen Python modules the six that implement spec rules are ported and the nine one-off experiment readers are deleted; the committed baselines in the evidence repository are the acceptance tests.

## 1. Purpose

Phase 0 measured MakeMKV, mkvmerge and ffmpeg with Python analysers under `scripts/phase0/`. Three of them are the first versions of decision rules the product ships (the audio content rule of spec 8.6, the subtitle text rule of 8.7, the scan-versus-file reconciler of 6.2 step 7), and the tuning harness behind decision D16 only exists in Python. This pull request moves that logic into the `core` crate the design spec names for phase 1, adds the developer command that reads a recording and prints the decisions, and deletes every Python file. When it merges, `cargo test` on a checkout with the evidence clone proves that the Rust rules give the recorded verdicts, and no Python remains.

## 2. Scope

Ported (the module, what it becomes):

| Python | Rust | Rule it implements |
|---|---|---|
| `robot.py` | `core::robot` | spec 9.4, the robot-output parser |
| `compare_tracks.py` | `core::reconcile` | 6.2 step 7, scan streams against file tracks |
| `audio_similarity.py` | `core::audio::{levels, measures, rule, reference}` | 8.6 content rule, EXP-13 section 9, D21 veto |
| `audio_labels.py` | `core::audio::labels` | D16 labels, D23 counting |
| `audio_dataset.py` | `core::audio::dataset` and `lazymkv-dev retune` | spec 15 `retune` |
| `subtitle_text.py` | `core::subtitle::text` | 8.7 text rule, EXP-14 section 3 |
| `subtitle_alignment.py` | `core::subtitle::align` | 8.7 timing bands |
| `proxy_levels.py` | `lazymkv-dev levels-from-proxy` | D16, new measures without the disc |

`proxy_levels.py` is the seventh module: it is a driver around ffmpeg, so it lives in the dev binary, not in `core`.

Deleted without a port, because their answers are in `docs/phase0/REPORT.md` and the experiment reports: `check_encoding.py`, `compare_scans.py`, `exp02_check.py`, `exp11_tracks.py`, `hit_rate.py`, `ocr_compare.py`, `summarize_scan.py`, and every `test_*.py` once its cases are ported. Git history keeps their code.

Out of scope: the OCR engine work (D22; the OCR text keeps coming from `ocr-subs.ps1` and Tesseract on the mini PC), the `tools`, `platform`, `clients` and `engine` crates of phase 2, any change to a PowerShell job beyond the commands it prints, any change to a rule's thresholds or measures. The port reproduces the Python; improving the rules is later work and goes through `retune`.

## 3. Architecture

### 3.1 Crates

`crates/core` (exists: `fingerprint`, `profile`) gains the modules of section 2 plus `lang` (ISO 639 bibliographic and terminology codes, the no-language codes `"", -, und, zxx, mul, mis`, TMDb's `xx`) and `recording` (the data model of a recording and the parsers of its files, from text and bytes handed in). `core` keeps the design spec's rule: no I/O, no OS-specific code, every function takes parsed inputs and returns values or typed errors. New dependency: `regex` (the subtitle text rule's patterns); `serde`, `serde_json`, `md-5` and `sha1` are already there.

`crates/dev` is new and builds the binary `lazymkv-dev`. It owns the I/O: walking a recording directory, reading files, running ffmpeg for `levels-from-proxy`, printing reports and JSON, exit codes. It is the first form of decision D16's developer and tuning mode; phase 2 folds `retune` and `decide` into `lazymkvctl` and this crate keeps whatever the engine does not take over.

`crates/capture` and `crates/fake-tools` are unchanged.

### 3.2 Data flow

```
recording dir --(dev::evidence reads files)--> core::recording::Recording
   scan.txt ----------------> core::robot::Scan
   mkvmerge.json, ffprobe-streams.json, rip.txt --> core::reconcile::Report
   audio-levels-20ms-*/ -----> core::audio::levels::Folder --> measures --> rule verdicts
   audio-labels.json, audio-fitted.json, tmdb.json --> core::audio::labels, reference
   subs/, subs-ocr/, TEXT/*.ocr.jsonl --> core::subtitle::text::Report, align::Report
all of it --> lazymkv-dev decide --> JSON on stdout
all titles of the evidence clone --> lazymkv-dev retune --> the harness report, audio-dataset.json
```

### 3.3 Module contracts

- `core::robot`: `parse(&str) -> Result<Scan, RobotError>`; `Scan { drives, titles: Vec<Title>, messages }` with every attribute id of spec 9.4 as a typed accessor and the raw map kept; `RobotError { line: usize, kind }`, never a panic on recorded input; `duration_seconds(&str) -> Option<f64>`; a splitter for the quoted CSV fields that matches `robot.split_fields` on every line of every recorded scan.
- `core::reconcile`: `expected_rows(scan, title, selection)`, `removed_streams(rip_text)` (MSG:4047 indices mapped through the selection, as phase 0 found), `file_rows(mkvmerge_json)`, `checks(want, got, title, ident) -> Report { count, order, duration, rows }` with the caption-first exception (a text-converted subtitle track before the bitmap ones is in order) and the duration tolerance: 2 s on a Blu-ray; on a DVD 2 s shorter to 1 % longer (provisional, spec section 17 row 1).
- `core::audio::levels`: readers of `manifest.txt` (first line `# lazymkv audio-levels v2`), `main-mix.txt`, `main-fc.txt`, `cand-<n>.txt` (one dB level per 20 ms frame, floor -100), `streams.json`, `audio-proxy.json`; a folder of the first measurement form is refused with the message the Python prints.
- `core::audio::measures`: `analyse(main, cand, dialogue, reverse) -> Measures` with the constants of `audio_similarity.CONSTANTS` as a `Constants` struct (defaults equal to the Python's), DC, AG, tenths, SMI (the 64-point modulation spectrum with precomputed twiddles, computed in the Python's order), LS, ADL, lags, `swapped`, `reverse_measures`.
- `core::audio::rule`: `Rule` (the thresholds of `audio_similarity.RULE`), `Verdict` (`duplicate, commentary, commentary-cross-language, reference-unclear, ad-pattern, note-commentary, note-ad, keep, not-judged`), `verdict(measures, rule)`, the reverse and peer runs, `cross_language`, `MEANING` texts, the position veto (row 0, D21: `first_of_language`, `position_veto`).
- `core::audio::reference`: `is_lossless`, `rank`, `placed_first` (8.6 step 3 with guards G1 and G2 as far as ffprobe's facts go), `programme_reference` (the original language from `tmdb.json`, else English, as spec 8.6 now states), `wanted`.
- `core::audio::labels`: the labels file `lazymkv audio labels v1` and the fitted file `lazymkv audio fitted v1` as serde types with the same validation as `audio_labels.check`; `KINDS`, `HOW`, `COUNTING_HOW = (listened)`, `OBSERVATION_HOW`, `NOT_COUNTED`; `not_counted_because`; the edits `set`, `unset`, `title`, `auto`, `fitted` as pure functions from a file's content to its new content (the dev binary writes the file in one step, only when the change adds no problem).
- `core::audio::dataset`: the harness: every level folder of every title, the join with labels, the counting against the totals of spec section 17 row 13 (read from the spec file the caller hands in, as the Python does, so the numbers have one source), the FAIL rules (a programme track with a drop verdict or the cross-language pattern, a folder measured against another reference than the rule's own choice, an unwanted reference, a swapped reference), the not-judged exclusions (no language, a language that is neither English nor the original, a folder without `streams.json`), the missing-title list, the snapshot `lazymkv audio dataset snapshot v2` with its cache. The cache key uses the rule version instead of a script hash: `RULE_VERSION`, a constant string bumped by hand whenever a measure or threshold changes, plus a hash of the constants in force. The report text and exit codes (0; 1 for a FAIL; 3 when evidence could not be read) are the Python's.
- `core::subtitle::text`: `Rule` (the thresholds of `subtitle_text.RULE`), the ASCII folding map and OCR corrections, the marker regexes (`CUE`, `LABEL`, `>>`, note lines, wrapped lines), `measure(stream_text, packets, frames) -> Measures` (events, readability, language shares, markers CUE/LABEL/M/SO/CS, capital style), the pair measures (time-free bigram containment, timing alignment), `decide(measures, rule) -> Vec<Decision>` with rules T0 to T7, the closed-caption set-aside, the "before a yes" mode that only adds reviews; the report prints numbers only, never a line of subtitle text.
- `core::subtitle::align`: `events(packets, clear_max_bytes)`, `aligned(a, b, tolerance_s)`, `buckets`, `is_text(codec)`, the index of a `subs/` folder.
- `core::recording`: the types for `meta.json`, `files.json`, `rip-summary.json`, `extract-summary.json`, `probe-summary.json`, `mkvmerge.json` (the parts used), `ffprobe-streams.json`, `subs/index.json`, `sub-<n>.packets.json`, `subs-ocr/summary.json`, `sub-<n>.frames.json`, `tmdb.json` (slimmed form, `original_language` and `fetched_at`), and `Missing(path)` as the one error for an absent file. Every text file is read as UTF-8 with an optional byte-order mark, because PowerShell wrote them.

### 3.4 The dev binary

`lazymkv-dev <command>`; every command takes `--evidence DIR` (default `corpus/owner`, or the environment variable `LAZYMKV_EVIDENCE`) and fails with exit 3 and one line naming the evidence repository when the directory is not a clone of it.

| Command | Replaces | Output |
|---|---|---|
| `scan <slug>` | `summarize_scan.py` (the useful part) | the title table of `scan.txt` as JSON |
| `decide <slug> <label>` | spec phase 1's command | reconciler report, audio verdicts, subtitle verdicts, reviews and measures, as JSON |
| `retune [--set K=V ...] [--snapshot FILE] [--compare FILE] [--spec FILE] [--allow-unmeasured]` | `audio_dataset.py` | the harness report; writes `audio-dataset.json` as the Python did |
| `levels-args <slug> <label>` | `audio_dataset.py args` | the `audio-levels.ps1` job lines |
| `labels kinds\|check\|set\|unset\|title\|auto\|fitted ...` | `audio_labels.py` | the same arguments and messages |
| `subs <slug> <label> --text DIR [--orig-lang CODE]` | `subtitle_text.py` | the numbers-only report; exit 2 when input cannot be read |
| `align --subs-dir DIR \| name=path ... [--clear-max-bytes N] [--tolerance S]` | `subtitle_alignment.py` | the alignment table |
| `levels-from-proxy <audio-proxy.mka> <out> --main N [--cand N,...]` | `proxy_levels.py` | a level folder; needs `ffmpeg` and `ffprobe` on PATH |
| `golden check` | new | runs every golden comparison of section 5; exit 1 on any difference |

Exit codes follow the Python command each replaces. A command never writes outside the evidence clone, the output path it is given, or the temporary directory.

## 4. Exactness

The port reproduces the Python, not an improvement of it:

- Verdicts, markers, classes, counts, orders, review reasons, exit codes and report lines that carry a decision are identical.
- Floating-point measures agree within 1e-9 absolute, except SMI within 1e-6 (its spectrum sums 64 complex products). The Rust code computes in `f64` in the Python's order of operations; where that is impossible, the test documents the deviation and the golden records the Rust value with the Python value beside it.
- The random-free parts are byte-identical where a file is written (`manifest.txt`, level files, `streams.json`, the labels file after an edit, `audio-dataset.json` apart from `created` and the rule-version key).
- The review's fixes of 2026-10-13 (T2 over a snapshot, the T6 guards, unjudged tracks, the veto without `streams.json`, DVD segment numbers) are part of the Python the port reproduces.

## 5. Acceptance: goldens and ported tests

**Goldens.** Before any Python is deleted, a one-off script `scripts/phase0/write_goldens.py` (added and deleted in this pull request) runs every Python analyser over the evidence clone and writes, per recording, into `corpus/owner/<slug>/golden/`:

- `scan.json`: the parsed title table (`robot.parse`) and `duration_seconds` of every title;
- `<label>/reconcile.json`: `compare_tracks` rows and verdicts for every rip label;
- `<label>/audio-<folder>.json`: for every level folder, every candidate's measures (forward, reverse, swapped, peer where it exists), verdict, veto line;
- `<label>/subs.json`: per stream the measures, markers, class, language and decision of `subtitle_text.py`, the pair measures, the exit code; written only where the OCR text is present on the Mac (the text itself is never written);
- `<label>/align.json`: `subtitle_alignment.py` over the `subs/` folder;
- `labels.json`: `audio_labels.check` output and `not_counted_because` for every labelled track.

`audio-dataset.json` stays the baseline of `retune`. The goldens are committed in the evidence repository (they hold numbers and verdicts, no subtitle text, no serials, no user names); the Python that wrote them is deleted in the same pull request.

**Rust tests.** Every Python test case becomes a Rust test in the module it exercises (synthetic fixtures become Rust helpers; the count is reported in the pull request). The golden comparisons are `#[test]`s in `crates/dev` that read the evidence clone when it is present and otherwise print `skipped: no evidence clone at <path>` and pass, so a public CI without the clone stays green and honest. The parser also runs over the pinned public scan logs under `corpus/thediscdb` (title count and duration of every title, against the counts in their parsed forms), which does run in CI. `cargo test --workspace` on the Mac with the clone, and on the mini PC, must pass before the pull request is marked ready.

**Harness round trip.** `lazymkv-dev retune` on the evidence clone must end `RESULT ok`, reproduce every verdict of `audio-dataset.json`, and write a snapshot that differs from the committed one only in `created` and the rule-version key; that snapshot is committed in the evidence repository with the pull request.

## 6. Deletion and hand-over

In the same pull request, after the goldens and tests pass:

- `scripts/phase0/*.py` and `scripts/phase0/test_*.py` are deleted; `__pycache__/` leaves `.gitignore`.
- Every PowerShell script that prints or documents a `python3 scripts/phase0/...` follow-up prints the `lazymkv-dev` equivalent; the plan's D16 routine (Task 11 Step 11, Task 12 Steps 15 to 20) is rewritten to the new commands; `README.md`, `corpus/README.md`, `corpus/owner/README.md` (in the evidence repository), `docs/phase0/REPORT.md` and the experiment designs say where each rule now lives.
- The design spec's section 15 names `lazymkv-dev retune` as the form `retune` takes until `lazymkvctl` exists, and section 16's phase 1 row names this pull request as its first step.

## 7. Risks

- **Regex semantics.** Python `re` and Rust `regex` differ (no look-around in `regex`; the OCR patterns use none, which the port must confirm pattern by pattern; Unicode classes differ, which the ASCII folding sidesteps).
- **Float order.** Python's `sum` and Rust iterators differ in accumulation order; the measures use explicit loops in the Python's order. The 1e-9 bound catches a slip.
- **The spec as data.** `retune` reads row 13's totals from the design spec, so an edit of that row changes the harness; the Python already does this, and the golden check pins the numbers read.
- **Evidence drift.** A new recording pulled before the port lands changes nothing here: goldens are per recording and the harness ignores unlabelled tracks for its FAIL rules.
- **ffmpeg on the Mac.** `levels-from-proxy` and its test need `ffmpeg` on PATH; the test skips with a message when it is absent.

## 8. Decisions recorded by this document

- RP1. The port is one pull request, executed as one plan (owner, 2026-10-13).
- RP2. Six rule modules are ported, nine one-off readers are deleted; `proxy_levels.py` is ported as a dev command (owner, 2026-10-13, on the recommendation).
- RP3. Acceptance is by goldens written by the Python once plus the Python tests ported one for one (approach A of the brainstorm; owner: "the design holds").
- RP4. `core` stays free of I/O: directory walking and file reading live in `crates/dev`.
- RP5. The rule version replaces the script hash in the harness's cache key.
