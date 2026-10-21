# Phase 1: the decision core

Phase 1 of the design spec (`docs/superpowers/specs/2026-10-04-lazymkv-design.md`, section 16, row "1. Decision
core") delivers `core`, with a command that reads a recording and prints the plan, decisions, confidences and review
reasons as JSON, tested on any OS. It was built in two parts. The first was the Rust port of phase 0's Python
analysers (pull request krcm0209/lazymkv#2; its design is `docs/superpowers/specs/2026-10-13-rust-port-design.md`, and
[`rust-port-tests.md`](rust-port-tests.md) maps its tests). The second, on the branch `phase-1-decision-core`, built
the rest of `core` from the design spec, and the command: `lazymkv-dev plan` for the disc phase, and two new sections
of `lazymkv-dev decide` for the file phase.

Nothing the second part added runs an external program, reads a disc, touches a share or uses the network. Every new
rule is a pure function in `crates/core` over typed inputs: the parsed scan, the recorded or synthetic TheDiscDB and TMDb (The
Movie Database) answers, and explicit input structs for the facts the engine will measure in phase 2 (packet MD5
sums, ffprobe profiles, subtitle events, the content and text verdicts, library facts, the owner's review decisions).
`crates/dev` reads the recordings and fills those structs.

## What `core` holds

| Module | Design spec | What it decides |
|---|---|---|
| `settings` | 14, 7 | Every setting with its default; a settings file names only the keys it changes; the checks of the roots (a staging root inside a library root, the tuning root) and of the values the rules divide by or compare against |
| `review` | 8.8, and the notes of 8.6, 8.7, 8.9, 7, 6.3, 10.2 and 8.3 | The closed sets of review reasons and notes, with the spec's spellings |
| `codec` | 8.6 "Codecs", 8.10 | MakeMKV's codec names (attributes 5, 6 and 7) to a family, a fidelity tier and a display name; attribute 41, MakeMKV's output codec short name, to the codec the file holds; the bitrate of attribute 13; channel layouts; the object-based extension (Atmos, DTS:X); mkvmerge's names for the verification |
| `lang` (extended) | 8.6 "Languages", R4 | The wanted languages: English and the original language |
| `disc` | 9.4, 9.5, 5, 6.1 step 2 | The typed disc model over the parsed scan: titles, streams, flags, the MakeMKV version, Java use, the `bdmt` disc name, title matching by source file, segment map and duration |
| `lookup` (`discdb`, `tmdb`, `name`) | 8.1 | TheDiscDB and TMDb answers, the disc name and its cleaning, plausibility, the runtime checks, the identity and its confidence |
| `plan` (`kind`, `movie`, `tv`) | 6.1 step 5, 8.2 to 8.5, 5, 7 | The disc kind, the feature or the cuts (by TheDiscDB, by the Java playlist labels, by structure), editions, TV episodes and season sessions, title keys, the mode (direct or whole-disc copy), the space needed |
| `remembered` | 5, 11 | Remembered decisions and the owner's review decisions (overrides), matched to a new scan |
| `tracks` (`audio`, `subtitle`, `layout`) | 8.6, 8.7, 8.10 | The audio tracks kept, dropped and ordered; the English subtitle roles, the forced track and the subtitle confidence; the names, flags and order of the tracks in the final file |
| `naming` | 8.9, 8.4, 11 | Library paths for films, editions and episodes; the compatibility tag; sanitisation; the copy label of a second copy |
| `verify` | 6.2 step 7, 6.4 step 14 | A ripped or remuxed file against the scan or the decision (a remuxed track's codec and channels against its source stream, the chapters against the staged file's); read errors |
| `decision` | 6.4 step 11, 8.8, 11 | One title's file decision: tracks, path, the three confidences (identity, title, track), review reasons, notes and the disposition (auto-file or review); the job-level step that holds every title of a job whose final path another title also builds (`same_path_reviews`, `path_collision`); Remember, which keeps each title's track decisions with that title |
| `job` | 10.1, 10.2, 5, 12 | The job state reducer: states, events, actions, the waiting states and the state each returns to, failure classes, resume rules, the job ID, and the job's one decision event combined from its titles' decisions (`decision_done`) |
| `subtitle::labels` | 15, 17 row 14 | The owner's subtitle labels file and the subtitle harness that `retune --subtitles` runs |

The port's modules keep their behaviour: `core::reconcile` and its goldens are byte-identical (it is the Python's
rule; `verify` is the product's rule beside it), and so are `decide`'s six existing sections and their goldens.
`fingerprint` gained `hashed_files` (the `HSH:` lines of a TheDiscDB scan log, moved there from a test so `plan` can
fingerprint a pinned log) and `recording::files::Tmdb` gained the optional fields of the recorded `tmdb.json`.

## Running it

The commands are in the `lazymkv-dev` binary (`crates/dev`). On a machine with the Rust toolchain:

```bash
cargo run -q -p lazymkv-dev -- --help
```

On the Mac, which has no toolchain, every command runs on the mini PC through `scripts/box.sh`, which mirrors the
tree there first:

```bash
scripts/box.sh cargo run -q -p lazymkv-dev -- --help
```

A slug (`pans-labyrinth-2006-uhd`) is read under the evidence clone: `corpus/owner/`, or `--evidence DIR`, or the
`LAZYMKV_EVIDENCE` environment variable; without it a slug exits 3 with a line naming the evidence repository. The
mini PC keeps a mirror of the clone at `C:\lazymkv\evidence\owner`, reached through a junction:

```bash
scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo run -q -p lazymkv-dev -- plan pans-labyrinth-2006-uhd"
```

`plan` and `decide` print their documents as JSON with an indent of 1 and ASCII escapes, as `decide` printed it before
phase 1; `labels` and `retune` print text lines.

### `lazymkv-dev plan`

```
lazymkv-dev plan <slug|recording dir|scan file> [--evidence DIR] [--settings FILE] [--tmdb-search FILE] [--tmdb-record FILE] [--remembered FILE]
```

The disc phase of design spec 6.1 step 5, over what a recording holds. The argument is one of:

- a slug or a recording directory: `scan.txt`, `meta.json` (the fingerprint and the disc ID), the disc name in
  `nav/BDMV/META/DL/bdmt_*.xml`, `thediscdb-hash.json`, `thediscdb-discid.json`, `tmdb.json` and `lookup-status.txt`.
  A lookup file that is absent means that lookup was not made, and is not an error; an answer file that
  `lookup-status.txt` records as 200 (or that has no status line) and does not parse is (exit 2); an answer recorded
  with another status (a 404, a failed lookup) is not parsed. A directory is always read as a recording.
- a bare scan log, such as one of the pinned public logs `corpus/thediscdb/<kind>/<work>/<release>/discNN.txt`: its
  fingerprint comes from the log's `HSH` lines, TheDiscDB's answer from `corpus/thediscdb-api/dischash-<fingerprint>.json`
  when that file exists (it carries the TMDb ID and the kind), else from the `discNN.json` beside the log (which does
  not), and there is no TMDb answer. TheDiscDB's disc-ID lookup is not made, because TheDiscDB's `global_disc_id` is
  not the disc ID of design spec 5.

The options:

- `--settings FILE`: a JSON object of design spec 14's keys. Absent keys keep their defaults; an unknown key or a
  wrong type refuses the file (exit 2, `<file>: <key>: <problem>`). The problems of the checks of the roots and
  values are printed in `settings_problems`, and the plan is made with the settings anyway. The defaults themselves raise exactly one
  problem, keyed `tuning_root`: section 14 turns `keep_tuning_evidence` on and leaves `tuning_root` unset, which the
  engine will refuse.
- `--tmdb-search FILE` and `--tmdb-record FILE`: a TMDb search answer (`/search/multi`) and a TMDb record (`/movie/{id}`,
  or `/tv/{id}` with its seasons and the season's episodes). No search answer and no full record is recorded anywhere
  in the evidence (the recorded `tmdb.json` is a slim form without runtimes or seasons), so these files are written by
  hand in TMDb's documented shapes; phase 2's client checks the shapes against the live service.
- `--remembered FILE`: a remembered-decisions file (`core::remembered::Remembered` as JSON), as the engine will keep
  one per fingerprint (design spec 5 and 11).

The document has the keys `slug`, `inputs` (which files were found and what each lookup answered), `settings`,
`settings_problems` and `plan`. The plan's keys are `identity` (the step of 8.1 that decided, the confidence, the TMDb
identity, title, year, original language, the step's review reason, the plausible results, the runtime check,
whether a lookup is pending, and one sentence per piece of evidence), `kind` and `kind_source`, `titles` (each with its key, title number, role,
duration, size and source file, and whether it was suggested or remembered), `mode`, `identity_confidence`,
`title_confidence`, `reviews`, `notes`, `wanted` (the wanted languages), `session` (the TV season session) and
`evidence` (what the planner saw).

Exit codes: 0 when the document was printed, with or without review reasons or settings problems; 2 for an
argument error, a scan that cannot be read, typed or planned, a lookup file (answered 200 or without a status line), a
settings file, a TMDb file (`--tmdb-search`, `--tmdb-record`) or a remembered file that does not parse, or a remembered
file for another fingerprint; 3 when a slug is given and the evidence clone is absent; never 1: nothing `plan` checks is
a verdict.

For example, over John Wick's pinned ultra-high-definition (UHD) log (run on any machine, the log is in this repository):

```bash
cargo run -q -p lazymkv-dev -- plan "corpus/thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt"
```

TheDiscDB knows the log's fingerprint, so the identity is step `discdb_one` at 0.95 (TMDb movie 245891); the
database rule picks title 0 (`00001.mpls`), whose key is `t00-00001`; the title confidence is 0.98, the mode is
`direct` and there is no review reason.

The plans the tests pin, one per pinned log and per recording of the evidence clone (`crates/dev/tests/plan.rs`; the
recordings' rows skip without the clone):

| Log or recording | Identity | Kind | Title rule and titles (in `Plan.titles` order) | Title confidence, mode | Reviews |
|---|---|---|---|---|---|
| John Wick UHD, `disc01.txt` | discdb_one 0.95, movie 245891 | movie | database: title 0 (00001.mpls; title 8 has the same segment map), key `t00-00001` | 0.98, Direct | none |
| John Wick Blu-ray, `disc02.txt` | discdb_one 0.95, movie 245891 | movie | database: title 0 (00001.mpls), key `t00-00001` | 0.98, Direct | none |
| Expendables 3, `disc01.txt` | discdb_one 0.95, movie 138103 | movie | database: title 41 (00756.mpls), edition `Unrated Cut` (from the Java label table, `FPL_MainFeature_UR`; see Choices the owner should confirm), then title 54 (00820.mpls, `FPL_MainFeature_R`) as an unnamed cut (OR5) | 0.98, Direct | `unnamed_cut`; note `unlisted_label_cut` |
| Smile, `disc01.txt` | discdb_one 0.95, movie 882598 | movie | database: title 14 (00040.mpls) | 0.98, Direct | none |
| Knowing DVD, `disc01.txt` | discdb_one 0.95, movie 13811 | movie | database: title 0 (attribute 24 `"01"`), key `t00-title01` | 0.98, Direct | none |
| Chernobyl disc 1, `disc01.txt` | discdb_one 0.95, tv 87108 | tv | mapped: title 1 S01E01, title 8 S01E02, title 7 S01E03 | 0.98, Direct | none; name `CHERNOBYL_D1`, disc 1, no season |
| Chernobyl disc 2, `disc02.txt` | discdb_one 0.95, tv 87108 | tv | mapped: title 6 S01E04, title 8 S01E05 | 0.98, Direct | none |
| Firefly disc 1, `disc01.txt` | discdb_one 0.95, tv 1437 | tv | mapped (attribute 24): title 0 S01E01, title 1 S01E02, title 2 S01E03 | 0.98, Direct | none |
| Firefly disc 2, `disc02.txt` | discdb_one 0.95, tv 1437 | tv | mapped (attribute 24): titles 0, 1, 2, 5 as S01E04 to S01E07 | 0.98, Direct | none |
| `pans-labyrinth-2006-uhd` (clone) | discdb_one 0.95 (discid answered 404), movie 1417, original `es`, wanted eng and spa | movie | database: title 0 (00800.mpls); name from `bdmt_eng.xml` | 0.98, Direct | none |
| `the-office-2005-s3d3-bluray` (clone) | discdb_one 0.95, tv 2316 | tv | mapped: title 3 S03E12-E13, title 2 S03E14, title 1 S03E15, title 0 S03E16, title 6 S03E17, title 5 S03E18 (ordered by episode); season 3 and disc 3 from its volume label | 0.98, Direct | none |
| `short-circuit-1986-dvd` (clone) | nothing 0 (no answers; no search recorded) | movie | structure: one rival, title 0 | 0.85, Direct | identity_unknown |
| `short-circuit-2-1988-dvd` (clone) | nothing 0 (`DVD_VIDEO` is generic) | movie | structure: title 0 (1:50:06) | 0.85, Direct | identity_unknown |

All nine pinned logs have their TheDiscDB answer in `corpus/thediscdb-api`: the fingerprint of each log's `HSH` lines
is the `ContentHash` of its `discNN.json` and names one of the nine `dischash-*.json` answers there, so every pinned
row's identity and kind are TheDiscDB's.

### `lazymkv-dev decide`

```
lazymkv-dev decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE] [--evidence DIR]
```

The port's command, for one rip label of a recording. Its six sections, `slug`, `label`, `reconcile`, `audio`,
`subtitles` and `reviews`, and their goldens are unchanged. Phase 1 appends two, and adds `--settings FILE`: a JSON
file of design spec 14's keys, read as `plan --settings FILE` reads it, that the `plan` and `decision` sections both
use in place of the defaults.

- `plan`: the `plan` command's document for the recording, with the default settings or those `--settings FILE`
  names, or `{"skipped": "<reason>"}` when it refuses.
- `decision`: the file phase (design spec 6.4 step 11) for the label's title, or `{"skipped": "<reason>"}` when the
  plan does not include that title or an input cannot be read. It holds `title_key`; `verify`, the report that maps
  the file's tracks onto the scan's streams (6.2 step 7); `facts`, which scan streams each measured fact covered (the
  packet MD5 sums from the level folders' manifests, the ffprobe profiles, the subtitle events of `subs/`, the content
  verdicts of the `audio` section and their reference, the text decisions of the `subtitles` section that the decision
  used, the streams the rip removed); and `decision`: the audio and subtitle tracks kept and dropped, their order, flags and names
  (`layout`), the resolution, the edition, the final path, the three confidences, the review reasons, the notes, the
  read errors and the disposition (`auto_file` or `needs_review`). With the default settings no library root is set,
  so `decide` builds no path (`path` is null) and every decision is held for review; the tests make a second decision
  with the owner's roots to pin the paths. When the `plan` section is skipped, the `decision` section is skipped with
  `no plan: ` and the same line.

The text decisions are there only when OCR (optical character recognition) text is found (`--text DIR`, or `LAZYMKV_OCR_TEXT`), as for the port's
`subtitles` section, and the decision uses them only where design spec 6.4 step 10 runs: when the file holds two or more
English or no-language subtitle streams (a derived forced-only stream and the closed captions count; a stream MakeMKV
removed does not). A title with one English subtitle track keeps it as "English" without the text, as `retune
--subtitles`' 8.7 column does; `core::tracks::subtitle::step10_runs` is the one copy of that rule. The top-level `reviews` stays the port's union of the audio verdicts' and the subtitle
decisions' reasons; the decision's own `reviews` are the product's (8.8). The exit codes are the port's: 0; 1 when a
reconcile verdict is no; 2 for an argument error, an unreadable input or a folder that is not a rip label; 3 for a
slug without the evidence clone. A skipped section never changes the exit code.

### `lazymkv-dev labels --subtitles`

The owner's subtitle labels live beside the audio labels in a recording: `<slug>/subtitle-labels.json`, whose format
line is `lazymkv subtitle labels v1`. Each entry names a track by its rip label and stream and says:

- `kind`: one of `regular`, `sdh`, `forced`, `commentary`, `trivia`, `lyrics`, `translation`, `dub-transcript`,
  `closed-captions`, `other`;
- `how` the owner knows it: `read` or `watched` (the owner read or watched the track's text; only these two count,
  decision D23), or one of the observations `packaging`, `menu`, `flag` (a flag in MakeMKV's scan), `player`, `thediscdb`;
- `sure`, `date` (`YYYY-MM-DD`), `saw_verdict` (a label given after seeing the verdict never counts) and `note`;
- `style`: `uk` for a UK-style SDH track (speakers by colour, sounds without brackets; ruling RP25), else null.

`lazymkv-dev labels --subtitles kinds` lists the kinds and sources; `set`, `unset` and `check` add or replace one
entry, remove one (recording why and when, as the audio labels' `unset` does) and check a file. The problems are the audio labels' sentences with
"subtitle" for "audio", and the validation is shared with them. The exit codes are `labels`' own: 0, or 2 for an
argument error, a file that does not validate or a change that would add a problem. `lazymkv-dev --help` prints the
synopsis.

### `lazymkv-dev retune --subtitles`

```
lazymkv-dev retune [--evidence DIR] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE] [--spec FILE] [--allow-unmeasured] [--subtitles] [--text DIR]
```

`--subtitles` takes only `--evidence`, `--spec` and `--text`; with `--set`, `--snapshot`, `--compare` or
`--allow-unmeasured` it is an argument error (exit 2). The subtitle thresholds are design spec 14's defaults
(`Settings::default().text_rule()`).

With `--subtitles`, the subtitle harness of design spec 15 (ruling RP15 below). For every labelled English or
no-language subtitle track of every rip label in the evidence, it verifies the rip against the scan and judges the
track by the subtitle text rule of 8.7 "Text" over the OCR text found under `--text DIR` (or `LAZYMKV_OCR_TEXT`). For
a stream the text rule sets aside (the DVD closed captions), it judges the classifier of 8.7 instead. Beside each
judged track it shows what the classifier of 8.7 decides from the `subs/` events. A track without OCR text is listed
as not judged. It then reports:

- the count per kind of the labels made by reading or watching (D23) that the owner is sure of and gave before seeing the verdict (`saw_verdict` false; ruling RP25), against the kinds section 17 row 14 requires (a recording is a feature film or a TV episode by its `tmdb.json`, else by its recorded TheDiscDB answer, else by 8.2's structural rule, and the report names each such recording's kind and where it was read), and row 14's UK-style SDH track, counted from the counted `sdh` labels whose `style` is `uk`,
  read from the spec text as the audio harness reads row 13 (`--spec`, default the design spec under the repository
  root); a label on one of the two discs row 14 names as fitting discs is reported but never counted;
- the costly verdicts as row 14 defines them, each without a review: a dialogue track dropped
  (`dialogue_dropped`), SDH (subtitles for the deaf and hard of hearing) placed as regular (`sdh_as_regular`) or regular as SDH (`regular_as_sdh`), a track
  forced wrongly (`forced_wrongly`), a forced track placed as dialogue (`forced_as_dialogue`), and a commentary, trivia
  or lyrics track kept (`unwanted_kept`);
- how many labelled tracks the rules placed without a review.

It exits 1 on any costly verdict; 2 for an argument error, a spec problem or a subtitle labels file that does not
validate; 3 when there is no label, or when any labelled English or no-language track could not be judged (its rip,
scan or OCR text could not be read; for example without the evidence clone or without `--text`).

## Rulings of the phase 1 plan

The plan recorded these rulings (RP1 to RP9 are the port's, in its design). The owner can overturn any of them.

- RP10. Phase 1 is design spec section 16's decision core and nothing of phase 2. The items the spec marks "to be
  designed in phase 1" that concern the engine (D17's bundling table in 4.4 and 19, the app's own recording format,
  the form of the tuning evidence, the decoding of bitmap subtitles for OCR, code signing) are phase 2 design work.
- RP11. The verification rule of 6.2 step 7 and 6.4 step 14 (closed captions first, the DVD duration band from 2 s
  shorter to 1% longer, the codec through attributes 41 and 42, `MSG:4047` mapped through the selection, MakeMKV-only
  flags carried onto the output tracks) is a new module, `core::verify`. `core::reconcile` and its goldens stay
  byte-identical, and `lazymkv-dev reconcile` keeps it.
- RP12. The job state reducer of 10.1 is in `core`: every state, event and action of 10.1, the transition table, the
  waiting states' return states and the resume rules, with a test per transition and per resume rule. The engine
  performs the actions in phase 2.
- RP13. Identification (8.1) is pure over recorded or synthetic answers. The DVD disc ID (libdvdread's) stays
  unimplemented, as design spec 5 says, since nothing could check it; a DVD is looked up by its fingerprint alone.
- RP14. The command is `lazymkv-dev plan` (the disc phase) and `decide`'s two new sections (the file phase), with
  `decide`'s existing sections and goldens unchanged. Phase 2 folds both into `lazymkvctl`.
- RP15. Design spec 15's "the subtitle text rule's thresholds (8.7) are brought under `retune` in phase 1" is
  `subtitle-labels.json`, `lazymkv-dev labels --subtitles` and `lazymkv-dev retune --subtitles` (amended by RP25).
- RP16. Remembered decisions are a `core` type with pure functions: a remembered title matches a scan title by source
  file, segment map and duration within 2 s, and each title's audio and subtitle overrides, kept with it, are keyed by
  the stream's signature (type, language, codec, channels, position among identical streams; per title since F10
  below). Keeping them is phase 2's.
- RP17. Season sessions (8.5) are a `core` record with pure functions (`find`, `suggest`, `assign`, `close`); keeping
  them is phase 2's.
- RP18. Edition naming (8.4) takes "the library folder already holds an edition-named file" and "an earlier filed copy
  of that cut" as inputs (`LibraryFacts`); reading the library is phase 2's.
- RP19. `core` stays free of I/O: every new function takes parsed values, text or a `Tree` (the dev crate's tree over
  the disk, or an in-memory one in tests). No new crate.
- RP20. The new modules are written from the spec, not from any Python. New JSON documents use the struct's field
  order and the spec's spellings, and keyed collections are sorted maps (in a type written as JSON, sorted lists of
  named entries: RP23).
- RP21. No test in `crates/` holds a byte copy of, or a value read only from, `corpus/owner`; dev tests over the clone
  read it at run time, and core's tests use made-up values of the same shape. `crates/dev/tests/clone_values.rs` pins
  it over the clone: no file under `crates/` quotes a disc name or volume label of a recording's scan, or holds one of
  its title sizes or TheDiscDB item titles, unless the public spec, `docs/phase0` or the pinned fixtures publish it.
- RP22. `Settings::from_json` is the one reader of a settings file. It refuses, in one sentence naming the key, only
  text that is not one JSON object or a key it cannot read (a key the settings lack, or a value of the wrong type);
  otherwise it returns the settings with the problems the checks of design spec 14 and section 7 find, and
  `lazymkv-dev plan` prints those problems and plans anyway.
- RP23. A type written as JSON (the plan, the decision, the remembered decisions, the season session, the job's
  events and the commands' documents) holds no map keyed by a number, since serde cannot read one back through an
  untagged or flattened type; it holds a list of named entries sorted by their key (`title`, `stream`, or both).
  Types never written as JSON keep their maps.
- RP24. `lazymkv-dev plan` exits 0 (planned, with or without review reasons or settings problems), 2 (an argument
  error, an unreadable scan, or a lookup, settings, TMDb or remembered file that does not parse) and 3 (a slug given
  and the evidence clone absent), never 1; `decide` keeps 0, 1, 2, 3 with the port spec's meanings.
- RP25. Amends RP15: besides `label`, `stream`, `kind`, `how`, `sure`, `date` and `note`, a subtitle label has the
  required `saw_verdict` (a label given after seeing the rule's verdict never counts, as with the audio labels) and
  `style` (null, or `uk` for row 14's UK-style SDH track), and `how` gains the observation `flag`.
  `labels --subtitles set` takes `--saw-verdict yes|no` and `--style uk`; `retune --subtitles` counts only labels
  read or watched, sure and given before the verdict, and counts the UK-style SDH track from `style`
  (confirmed by ruling OR9, 2026-10-17).
- RP26. VC-1 video (`V_MS/VFW/WVC1`) is a codec family of its own, matched on attribute 5 alone, with the display
  and mkvmerge name `VC-1`, and no tier.
- RP27. The four note names the design spec described only in words are confirmed (ruling OR8, 2026-10-17) and the
  spec now names them: `possible_audio_description` (the port's ruling RP7), `lossy_before_lossless` (decision D9),
  `direct_mode_fallback` (6.3 step 6a) and `label_rule_unavailable` (8.3 rule 2 with 10.2).

### Rulings made during execution

Task 5: under 8.3 rule 1, a TheDiscDB-marked main movie whose Java label has an unrecognised suffix keeps 0.98 with no `unnamed_cut` (the label table belongs to rule 2). Ruling OR5 (2026-10-17) adds: a Java-labelled cut none of whose titles is a main movie's is planned as an unnamed cut with unnamed_cut and the note unlisted_label_cut; the confidence stays 0.98. Task 14 (amended by F12 below): in an approved `Decided`, a decision whose reasons include `path_collision` or a reason the owner did not approve, or whose `DecisionDone.has_path` is false, holds the job again at the disposition gate, while a low confidence alone never holds an approved job again (design spec 7, 6.4 step 12 and 11 under Approve). Task 18: the row 14 harness of `retune --subtitles` reads `subs/index.json` through `verify::subs_index`, the one reader of that file (the one-copy rule).

The final review of the branch added these, which the owner may also overturn:

- A search identity whose kind contradicts the planned kind adds `identity_ambiguous` and, unless assigned titles cover
  the titles, holds the title confidence at 0.60 with `title_ambiguous`, so the disc is copied whole and review never
  needs it again (6.3, 8.8 "Limits of D1"). This is a provisional rule: the spec leaves the case to the owner.
- Two titles of one job that build the same final path both get `path_collision` and are held
  (`decision::same_path_reviews`, which phase 2 runs over the job's decisions before the disposition gate), whatever
  Resolve collision says; the owner names the cuts with Assign titles.
- Remember stored an audio or subtitle decision for a stream signature only when every title of the job decided it the
  same way. F10 below replaces this: each title keeps its own track decisions.
- The text rule's decisions are used only where 6.4 step 10 runs (two or more English or no-language subtitle streams
  in the file), in one gate inside `decide_subtitles` that `decide` and `retune --subtitles` share.
- The job's `DecisionDone` is built by `job::decision_done` over every title's decision: it needs review when any title
  does, and has a path only when every title has one; a job with no final path is never filed, whether or not the
  owner approved it.
- Discard is accepted from a failed job as well as a held one (10.1's table lists only NeedsReview), so the owner can
  free a failed job's staging data, which section 7 never deletes automatically.

### The review of pull request #3

The review of pull request krcm0209/lazymkv#3 found fifteen defects, and the owner ruled on each fix (F1 to F15). The
owner may still overturn any of them.

- F1. The codec table knows DTS Express (`A_DTS` with attribute 6 or 7 `DTS-HD LBR` or `DTS Express`: other lossy,
  displayed and named by mkvmerge "DTS Express"), and a DTS stream accepts mkvmerge's "DTS-ES" and "DTS 96/24". A codec
  the table does not know is not compared: its type, language and channels are, and the report's `unchecked` says the
  codec was not compared, so a correct rip is neither ripped again nor held with `verify_failed`.
- F2. `decide` takes the wanted languages from the filed identity's TMDb record whenever the record is given, so a
  record fetched after planning (a pending TMDb lookup) gives its original language.
- F3. An Episode item on a play-all-shaped title is kept unless every episode it names is mapped by another kept
  Episode item: a multi-clip episode playlist whose clips carry no item is the episode, not a play-all title.
- F4. On a disc TheDiscDB states is a movie, rule 3.1 is skipped and a play-all-shaped group is an ordinary rule 3
  group (`movie::by_structure_as_movie`); the open question on extras play-all titles below now concerns only discs
  TheDiscDB does not know; ruling OR1 then settled it.
- F5. Resolve collision answers a path: `Overrides.collision` lists the plain final paths the owner was shown, each
  with its action, and Keep both or Replace resolves only a decision whose own plain path is one of them (compared
  whole and case-insensitively); any other collision raises `path_collision` again.
- F6. When a pending lookup answers and the wait returns to a file-phase state (a disc-phase step re-plans with the
  answers itself), the reducer's return from AwaitingLookup starts with `RefreshIdentity`:
  `plan::refresh_identity` identifies again with the answered lookups and the plan's runtime facts
  (`PlanEvidence.runtime`), keeps the ripped titles, their title confidence and the mode, and refreshes the identity,
  its confidence and review reasons, the wanted languages and the evidence. Phase 2 passes the refreshed plan to Decide.
- F7. A disc-name search with several plausible results, all of the other kind than the planned kind, holds the titles
  at 0.60 with `title_ambiguous`, as one such result does; a mixed-kind list does not, rule 4 remaining the guard.
- F8. Rule 3.4's covering test counts streams per signature, so a title with one more stream of a signature (an SDH
  track beside the regular one) covers the other wherever it comes in the group.
- F9. A dialogue group of three or more tracks keeps one SDH track (one described as SDH if any, else the one with the
  most events) and, as regular, the remaining track with the fewest events, an undescribed one before one described as
  SDH; `subtitle_ambiguous` is still raised.
- F10. A remembered decision keeps each title's audio and subtitle overrides with that title, keyed by the stream's
  signature within it, and applies them to the scan title the remembered title matches (`remembered::matched_titles`);
  design spec 5's sentence says so. Two titles that decide streams of one signature differently each keep their own
  decision, so a remembered job reproduces every title's reviewed decision.
- F11. A planned feature carries the planner's theatrical flag (`PlannedRole::Feature.theatrical`), so a lone unnamed
  cut keeps no edition and Remember refuses it until the owner names it.
- F12. Approval records the reasons shown at the gate (`ReviewApproved.approved_reasons`), and a re-finalise approves
  the filed decision's (`ReFinalise.approved_reasons`); an approved job whose decision made again carries a reason the
  owner did not approve (Set identity to a Japanese film that raises `original_language_missing`) is held again, as
  one with `path_collision` or no final path is.
- F13. `verify_final` takes the title and the staged file's `verify_rip` report: it compares each remuxed track's codec
  and channels with the scan stream the layout names, and the chapter count with the staged file's when the layout
  keeps chapters.
- F14. A blank library root is a root not set, so no path is built under it, and `Settings` refuses a blank or relative
  `movies_root` or `tv_root` (a UNC, drive-letter or POSIX path is absolute).
- F15. A stream's file codec (attribute 41 through the output codec table when present, else the scan codec) is read
  once, into `disc::Stream.file_codec`; the verification and the audio names use it, so an LPCM stream the rip writes
  as FLAC is named "FLAC 5.1", and an attribute 41 value the table does not know expects its own text.

### The owner's rulings of 2026-10-17

After the merge the owner ruled on the open questions and the follow-ups (OR1 to OR16); the controller's sub-rulings SR1
to SR9 settled what the readers found while planning. The owner may overturn any of them. The plan of this work is
`docs/superpowers/plans/2026-10-17-phase-1-rulings.md`, called the rulings plan below; a task named without it, such
as Task 5 in OR5, is the phase 1 plan's.

- OR1. 8.3 rule 3.1 counts a play-all title only when every part is episode-length (15 to 120 minutes, 8.2). OR15 limits
  this to rule 3.1: 8.5's dropping of play-all titles from a TV disc keeps the literal shape (any concatenation of two
  or more other groups' maps), so an extras play-all is never suggested as an episode. The whole group's own length is
  unconstrained (SR1).
- OR2. Two label cuts both named "Theatrical Cut" (the `none` and `_R` rows) keep the 0.60 `title_ambiguous` hold; two
  TheDiscDB main movies that would share one name likewise. No code change; the spec says so (the rulings plan's
  Task 4).
- OR3. When the default audio is not English and no regular English track is kept, the SDH track is the default
  subtitle. No code change; the spec says so (the rulings plan's Task 4).
- OR4. Leading periods and spaces are dropped from a path component as well as trailing ones. No code change; the spec
  says so (the rulings plan's Task 4).
- OR5. C1 (the final review's parked finding: a labelled second cut TheDiscDB does not list): rule 1 still wins
  (TheDiscDB's main movies at 0.98, Direct), but a Java-labelled cut (rule 2's grouping) none of whose titles is a main
  movie's scan title is planned too, as an unnamed cut (edition none, not theatrical), with the review reason
  `unnamed_cut` and, per OR16, the note `unlisted_label_cut` naming the title and its label, so the owner names it with
  Assign titles. The main movie's own label suffix, its dub variants and a Narrative variant name no second cut (the
  Task 5 ruling stands).
- OR6. Name cleaning: `S3D` and `3D` are format tags (never a season marker or a disc number); a region tag that is the
  whole label is kept (`US` is the film Us). SR2 settles the scope: the whole-label rule applies to a single-token label
  that is a region tag (`US`, `UK`, `EU`, `WW`, `DOM`, `INT`, `R1` to `R6`), with its brackets stripped (`(US)` gives
  `US`); a format tag that is the whole label (`UHD`, `DVD`, `BLU-RAY`, `3D`) still cleans to nothing and stays generic;
  a label of two or more tag tokens cleans to nothing (superseded by G8 and G8a: `US_UHD` is the film Us). `S1M0NE`,
  `BLADE_RUNNER_2049` and `D2_THE_MIGHTY_DUCKS` stay as they are.
- OR7. The object-audio spellings (`Atmos`, `DTS:X`) stay provisional until experiment 6 runs. No change.
- OR8. RP27's four note names are confirmed: `possible_audio_description`, `lossy_before_lossless`,
  `direct_mode_fallback`, `label_rule_unavailable`; the spec names them (the rulings plan's Task 4).
- OR9. RP25's subtitle-label vocabulary is confirmed. No change.
- OR10. The final review's six provisional rules are kept. No change.
- OR11. RP10 is confirmed; the spec's three sentences still saying "phase 1" for phase 2 work (lines 226, 363, 743) are
  edited now (the rulings plan's Task 4).
- OR12. Of the 95 deferred minors only the test gaps are pinned (the rulings plan's Tasks 5 to 8); where a gap exposed
  a defect, the defect is fixed with the test (SR4, SR5, SR6, SR8).
- OR13. The four phase 2 leftovers (lookup answers outside AwaitingLookup, restart between answer and refresh, collision
  answer by path, a relative root in `decide`) stay for phase 2.
- OR14. The pull request runs subagent-driven.
- OR15. (Scope of OR1.) The episode-length restriction applies to rule 3.1 only; 8.5 keeps dropping any concatenation
  title.
- OR16. (Form of OR5's note.) A new note `unlisted_label_cut`, the thirteenth of the closed set, with the title and
  label in its detail, carried into `Plan.notes` and every `FileDecision.notes`; plus an evidence sentence. H3 words
  the detail as the fact only: `title {n}'s label {comment} names a cut TheDiscDB does not list as a main movie`.

The controller made these sub-rulings while writing the rulings plan from the readers' findings; the owner may
overturn any of them.

- SR1. Rule 3.1's parts are each checked against the 8.2 band; the whole group's length and the parts' mutual spread are
  not checked.
- SR2. The whole-label rule's scope, as written under OR6. Superseded by G8 (the owner, 2026-10-19) and G8a: when
  exactly one token of the name is a region tag, nothing kept holds a letter and nothing kept comes before it, that tag
  is the name, so `US_UHD`, `US_2019` and `US_D1` give `US`.
- SR3. `by_database` takes `wanted: &Wanted` (the languages `movie_plan` already holds) so the unlisted cut's variant is
  chosen as rule 2 chooses it; no second pass with the original language for rule 1 plans; so a rule 1 plan's unlisted
  cut keeps the variant chosen with English only, and for a non-English film its original-language variant may need
  the disc again (8.8 "Limits of D1").
- SR4. `plan::movie::coverage` uses `lang::is_english` (phase 1 plan decision 6: one English test); `Wanted.english`
  stays, serialised as before.
- SR5. `codec::dts_family` reads attribute 6, then attribute 7, as its doc says (a conflicting made-up pair follows
  attribute 6); the DTS:X pre-check stays first.
- SR6. `disc::character_reference` refuses a numeric reference outside XML 1.0's Char production (U+0000 to U+0008,
  U+000B, U+000C, U+000E to U+001F, U+FFFE, U+FFFF, and the surrogates and values above U+10FFFF it already refuses),
  leaving it as written like a refused entity.
- SR7. Pinned as they are, no change: a tuning root inside the staging root is refused whatever `keep_tuning_evidence`
  says; owner-assigned episodes join or create the season session under an unconfirmed search identity (the owner
  assigned them); ffprobe's object extension wins over attribute 7's; a DTS:X track is named `DTS-HD MA DTS:X 7.1`
  (display name, extension, layout, as `TrueHD Atmos 7.1`).
- SR8. `lazymkv-dev plan`: a `--tmdb-record` file clears the pending flag a failed TMDb lookup set (the option replaces
  the recording's record); `--tmdb-search` does not (a different lookup); a failed TheDiscDB lookup keeps the job
  pending. Narrowed by G3 (the owner, 2026-10-19): only a record of the planned identity clears it.
- SR9. `TEXT_HINT` has one definition, `pub const` in `core::subtitle::labels` (its harness text already carries it);
  the dev crate uses it.

The controller made these rulings while executing the rulings plan.

- ER1. The owner's rulings are cited as OR1 to OR16 in every document and comment, because the design spec's R1 to R8
  are its requirements (the rulings plan's Task 1 first commit subject keeps "(R1, R15)"; history is not rewritten).
- ER2. The second-label-pass test's disc gives each cut's two variants one shared segment map (two variants of one cut
  play the same segments), since four distinct equal-length groups read as TV under 8.2.
- ER3. The pending-flag test asserts on `plan.identity.evidence`, where `identify` writes the pending sentence; the
  plan's `evidence.sentences` assertion was vacuous and was deleted.
- ER4. The rulings plan's Task 8 test T18a
  (`the_text_rule_judges_the_events_sub_clear_max_bytes_gives_when_ocr_text_is_given`) pins `sub_clear_max_bytes`
  only: `subs_tolerance_ms` does not move the text rule's words on the harness fixtures, so the call site passing
  `Settings::text_rule()` to the text run stays unpinned (a follow-up).
- Closed by H1 (the owner, 2026-10-19): under OR6 a title containing `3D` (`PIRANHA_3D`, `STEP_UP_3D`) cleans to the
  bare title, so TMDb's "Piranha 3D" was not a plausible result and identity rested on the runtime check against the
  same-named film; plausibility now also accepts the name with its region and format tags kept, and such a match is the
  only plausible one.

### The review of pull request #4

The review of pull request krcm0209/lazymkv#4 posted fifteen comments; the owner ruled on G1, G3, G7 and G8 and the
controller applied the rest as written (G1 to G15). The owner may overturn any of them.

- G1 (the owner). On a disc TheDiscDB does not know as a movie, rule 3's one feature that is itself a play-all of other
  titles (8.5's reading: a concatenation of two or more other groups' maps, any parts) is held at 0.60 with
  `title_ambiguous`, so the disc is copied whole: a play-all of short parts, or an extras play-all longer than the film,
  is no longer ripped as the feature at 0.85 in Direct mode. F4 keeps a TheDiscDB movie's play-all-shaped feature an
  ordinary rival. Left to the owner: two episodes more than 10% apart beside an extras play-all still rip the longer one
  at 0.85 in Direct mode (Chernobyl disc 2 is held only because its two episodes are rivals); the owner accepts it (H6).
  The hold's evidence sentence reads `8.3 rule 3.5: the only feature, title {n}, is a play-all of other titles (8.5),
  so the plan is held at 0.60 and the disc is copied whole` (J13).
- G2. `kind::structure_says_tv` compares in integers (`longest * 100 <= shortest * 115`), so a pair exactly 15% apart
  (1300 s and 1495 s) is TV; `TV_SPREAD` became `TV_SPREAD_PERCENT` (15).
- G3 (the owner). `lazymkv-dev plan` clears the pending flag of a failed TMDb lookup for a `--tmdb-record` only when the
  record's identity is the planned one; otherwise it plans a second time with the flag the lookups set. Without
  TheDiscDB's answer the plan has no TMDb identity, so a given record then leaves the TMDb lookup pending too.
- G4. Rule 1 does not plan an unlisted labelled cut whose chosen title and every sibling fail the 8.2 candidate filter
  (a menu loop, a title shorter than `minlength_s` or without a duration); `movie::by_database` takes the settings.
  J3, J5 and J10 replace the check: `movie::by_labels` drops such a title before grouping, and `by_database` no longer
  takes the settings.
- G5. `movie::parse_label` reads a trailing `Narrative` token after another suffix (`FPL_MainFeature_UR_Narrative` is
  the UR cut with `Label.narrative`), and rule 2's grouping drops it as it drops the bare `_Narrative` suffix, so it
  names no unlisted cut.
- G6. `SHOW_S3D_1` keeps OR6's reading: `S3D` is the format tag, so the name has no season marker or disc number and
  cleans to `SHOW 1`, while `SHOW_S2D_1` is season 2, disc 1. Pinned as it is and listed for the owner.
- G7 (the owner). Spec 10.2 widens back: the planner records `label_rule_unavailable` when the movie planner reached
  rule 2 (not on a DVD), and the job reducer records it on a missing-Java event whatever the plan (10.1). No code
  change.
- G8 (the owner). The whole-label rule becomes general and supersedes SR2: when exactly one token of the raw name is a
  region tag, nothing kept holds a letter and nothing kept comes before the tag, the name is that tag without its
  brackets (`US_UHD`, `US_2019`, `US_D1` and `(US)` give `US`; `Us (2019)` gives `Us`). `US UK` and `UHD` still clean to
  nothing, and `THE UK FILES` gives `THE FILES`. The `US_2019` owner question is answered. The condition "nothing kept
  before the tag" is the controller's G8a (2026-10-19): without it a title of digits was renamed (`1917_US` gave `US`,
  and Us's 116 minutes sit within 3% of 1917's 119), so `1917_US` gives `1917` and `2012_UK` gives `2012`, which stay
  generic as before.
- G9. `Feature.theatrical`'s doc limits the Other suffix to rule 2 and says that a rule 1 main movie whose own label
  suffix is Other has no edition and stays theatrical unless it is a later main movie (the Task 5 ruling).
- G10. The `AVATAR 3D 2` assertion names the guard that holds: the D of the 3D tag starts no marker, even beside a
  following number.
- G11. `kind::is_episode_length` and `kind::play_all_of_episodes` name the rulings plan's Task 1 (OR1) as their
  definition.
- G12. `movie::by_database` drops its `unlisted` flag (each unlisted cut pushes `unnamed_cut` beside its note) and
  expects the chosen title's label instead of defaulting to an empty one.
- G13. The two tests of a tuning root inside the staging root are one,
  `a_tuning_root_inside_the_staging_root_is_refused_whatever_keep_tuning_evidence_says`.
- G14. The two play-all tests of `plan::tv` share one fixture, `two_play_all_groups`.
- G15. `READING` (`subs-ocr`) has one definition, `pub const` in `core::subtitle::text`, beside the text rule that reads
  it (`subtitle::labels` already depends on `subtitle::text`, so the constant sits below it), re-exported from
  `subtitle::labels` beside `TEXT_HINT`; `decide`, `golden`, `subs` and `subtitle::text::load_title` use it.

### The owner's answers of 2026-10-19 (H1 to H8)

The owner answered the behaviour questions pull request #4 left open (H1 to H6), had the final review's five parked
wording residuals applied (H7) and settled the bookkeeping (H8). The owner may overturn any of them.

- H1. Plausibility also accepts the name with its tags kept. `DiscName` gains `tagged`, after `cleaned`: the raw name
  with underscores read as spaces, its markers and the year cleaning removes gone, but its region and format tags kept
  as written and no whole-label rule (`name::tagged`). When it differs from the cleaned name and some search result
  matches it, only those results are plausible (the more specific name wins); otherwise the results matching the
  cleaned name are, as before. `PIRANHA_3D` is Piranha 3D (0.90 with a passing runtime check), not Piranha; `The Last
  of Us` is the show, not "The Last"; `SMILE` is unchanged. A tag is kept as written, so `EXAMPLE_S3D` with its tags
  kept is `EXAMPLE S3D`, which "Example 3D" does not match. Spec 8.1 says so.
- H2. Beside a cut TheDiscDB does not list, a main movie whose edition is None because its own label suffix is Other or
  it carries no label is not theatrical, so `edition_names` proposes no "Theatrical Cut" and the owner names both cuts.
  A main movie the label table names "Theatrical Cut" (the bare `FPL_MainFeature` and `_R` rows) stays theatrical, and
  without an unlisted cut the Task 5 ruling stands. Spec 8.3 rule 1 says so.
- H3. The `unlisted_label_cut` note's detail states only the fact, `title {n}'s label {comment} names a cut TheDiscDB
  does not list as a main movie`; the "name it with Assign titles" clause went.
- H4. Rule 1 counts a labelled cut as listed when any of its titles, chosen or sibling, is a main movie's scan title or
  shares its 8.2 group (the same segment map); `movie::by_database` takes the groups `movie_plan` holds. Spec 8.3 rule
  1 says so.
- H5. `US 1917` and `US_300` read as the film Us under G8a (accepted), pinned beside the G8a tests.
- H6. Two episodes more than 10% apart beside an extras play-all, without TheDiscDB, labels or a season marker, plan the
  longer one alone at 0.85 in Direct mode (accepted as OR1's consequence; the identity guards hold the job), pinned by
  `two_unequal_episodes_beside_an_extras_play_all_plan_the_longer_one_alone_h6`.
- H7. The final review's five parked wording residuals: the `BLADE_RUNNER_2049` test comment (the year still goes,
  OR6), the spec's rulings paragraph after D23 (SR, ER, G and H), 8.5's unmapped bullet (any concatenation), 8.6 step
  6's `possible_commentary` bullet (the `note-commentary` trigger before its colon) and 8.4's clause on a labelled cut
  TheDiscDB does not list (on a disc TheDiscDB knows).
- H8. This block and the test count below. G6 stays pinned as it is (`SHOW_S3D_1` has no season marker and cleans to
  `SHOW 1`), and the 3D plausibility question left open under the OR rulings is closed by H1.

### The second review of pull request #4 (J1 to J15)

A second review posted fifteen comments at 68b5d59; the controller applied them as J1 to J15 (J2 and J5 are behaviour
choices made on spec 8.4 and 8.2, reported to the owner). The owner may overturn any of them.

- J1. `movie::by_database` plans the unlisted cuts and applies H2 before the same-name check, which then reads the main
  movies only. With TheDiscDB's main movies "Film" and "Film (Theatrical Cut)" beside an unlisted `_UR` cut, the first
  main movie is left unnamed and no longer clashes with the second, so the plan stays 0.98 with `unnamed_cut` alone;
  the confidence is 0.60 when a later main movie has no edition or two main movies share a name, else 0.98 (an
  unlisted cut alone keeps 0.98, OR5). Two main movies named alike with no unlisted cut stay 0.60 `title_ambiguous`.
- J2 (a behaviour choice on spec 8.4). A labelled title in a main movie's 8.2 group (H4) whose label the table names
  (`_UR`, `_EXT`, or the bare and `_R` rows' "Theatrical Cut") names the main movie's edition when TheDiscDB's item
  title and the main movie's own label name none, with the evidence sentence `edition '{name}' from the label
  {comment} of title {n}, the same content as main movie {m}`, where {m} is TheDiscDB's main movie index; a suffix the
  table does not name gives nothing, as on the main movie's own label. A main movie without a label whose same-map
  playlist carries `FPL_MainFeature_UR` is now the Unrated Cut rather than an unnamed theatrical cut, and H4's main
  movie beside a bare `FPL_MainFeature` duplicate is now "Theatrical Cut". Spec 8.3 rule 1 says so.
- J3 and J5 (J5 a behaviour choice on spec 8.2). `movie::by_labels` takes the 8.2 candidates and drops every labelled
  title outside them before grouping, so a cut whose titles all fail the filter is not planned and a cut's variant is
  chosen among its candidate titles. Without TheDiscDB, G4's disc (title 1 `_UR`, title 2 `_R` with no audio stream)
  plans the unrated cut alone at 0.90 rather than filing the menu loop as the theatrical cut; with TheDiscDB, an
  unlisted `_R` cut whose only candidate is its `_R_JPN` dub is planned by the dub, with no sibling, rather than by
  the menu loop. G4's separate check in `by_database` is gone. The labelled features of the pinned logs (The
  Expendables 3's titles 41 and 54, Smile's title 14) remain candidates. Spec 8.3 rules 1 and 2 say so.
- J4. A truncated 8.2 group (a segment map ending in `...`) lists no cut and names no main movie's edition, since its
  members may differ after the known prefix (`kind::play_all` already refuses truncated groups); the main titles
  themselves stay listed.
- J6. `name::tagged` tests a token's joined text, so a token whose only part left is empty keeps nothing: `SHOW D1- X`
  gives `SHOW X`, as `clean` does.
- J7. `movie::parse_label` takes the language code and the Narrative token off the end in either order, each at most
  once: `FPL_MainFeature_JPN_Narrative` is the dub suffix jpn with `narrative`, and `_UR_JPN_Narrative` the UR cut's
  Japanese narrative variant, as `_UR_Narrative_JPN` was. The label table row says so.
- J8. Documents only: `LookupInputs.search`, `name::disc_name` and spec 8.1 state the client contract. When the disc
  name's `tagged` differs from its `cleaned` once folded, the caller searches TMDb with both and passes the two answer
  pages merged in order, the cleaned name's results first and then the tagged name's results not already present, so
  the tagged match can be found and `PIRANHA_3D` keeps its fallback to Piranha. Listed under "Left for phase 2".
- J9. `MoviePlan::hold_ambiguous` holds a plan (confidence at most 0.60, `title_ambiguous`, an evidence sentence);
  `runtime_sanity` and the G1 hold in `plan::plan`, now one condition, use it, and the docs of
  `ReviewReason::TitleAmbiguous` and `movie::by_structure_as_movie` name rule 3.5's hold.
- J10. `movie::by_database` drops its `settings`: `movie_plan` reads the candidates once from the groups (exactly the
  candidates, as `plan::plan` builds the groups from them) and gives them to rules 1 and 2; a main title that fails the
  filter is in no group, so `listed` still chains the main titles in.
- J11. The H2 condition is stated with "edition is None" everywhere (spec 8.3 rule 1: a main movie "whose edition
  TheDiscDB's item title does not name"); the list of unnamed cuts lives only in `movie::Feature.theatrical`'s doc,
  and the docs of `PlannedRole::Feature` and `decision::job_features` point at it.
- J12. "(c1:F1)" became "(the Task 5 ruling)" in `movie::Feature.theatrical`'s doc and the G9 bullet (the copies in
  `PlannedRole::Feature` and `job_features` went with J11), and the Chernobyl disc 2 test says the longer of two
  unequal episodes alone at 0.85 was accepted by the owner (H6).
- J13. The G1 evidence sentence cites the spec clause instead of a review id, as the G1 bullet quotes it.
- J14. The G3 test asserts that the planned identity stays TheDiscDB's movie 6537 and that the other record lends it
  no original language, and `PlanOptions.tmdb_record`'s doc says the record clears the pending flag only when it is
  the planned identity's (G3).
- J15. `by_database`'s `nameless` list became `first_nameless`: only the first main movie can be left unnamed by H2,
  since a later one with no edition is an unnamed cut already.

## Choices the owner should confirm

The first four were the open questions, ruled on 2026-10-17 (OR1 to OR4 above) and now stated in the spec; the phase 1
plan settled the rest where the spec leaves a choice open. Each is pinned by a test, and each can be changed by
changing that test and its rule.

- Ruling OR1 (2026-10-17): 8.3 rule 3.1 counts a play-all title only when every part is episode-length (15 to 120
  minutes); a play-all of extras (Smile's deleted-scenes playlist, title 117) is an ordinary group, so a movie disc
  with one stays a movie without TheDiscDB. 8.5 still drops any concatenation title from a TV disc (OR15), except a
  play-all-shaped title whose TheDiscDB Episode item names an episode no other kept title maps (F3).
- Ruling OR2 (2026-10-17): two cuts named alike keep 0.60 with `title_ambiguous`. 8.3 rule 2's table names both the
  plain `FPL_MainFeature` row and the `_R` row "Theatrical Cut", so a disc carrying both is planned at 0.60 ("two cuts
  are both named Theatrical Cut") rather than 0.90, and two TheDiscDB main movies that would be filed under one name
  get the same 0.60 rather than 0.98.
- Ruling OR3 (2026-10-17): when the default audio is not English and no regular English track is kept, the SDH track is
  the default subtitle, and the forced track is not. 8.10 named only the regular track; the spec now names the SDH
  fallback too.
- Ruling OR4 (2026-10-17): leading periods and spaces are dropped from a path component. 8.9 trimmed trailing spaces
  and periods only, so a title that begins with periods (`...And Justice for All`) would give a folder and a file
  hidden on the share; it is now filed as `And Justice for All (1979) {tmdb-ID}`, and the spec now says so.
- A TMDb record without a runtime (or a season without episode runtimes) fails 8.1's runtime check, so step 4 cannot
  pass and step 5 applies (0.50, `identity_ambiguous`).
- Identity step 2 (TheDiscDB by both disc ID and fingerprint, 0.99) needs both records and, when both carry a TMDb ID,
  the same one; with different IDs the fingerprint's record wins at step 3 (0.95) and the disagreement is written to
  the identity's evidence.
- 8.3 rule 1 ("reconciles with the scan") means every `MainMovie` title of TheDiscDB's record matches a scan title; if
  one matches none, the record falls to rule 2. A `MainMovie`'s edition comes from its item title, else from the Java
  label table (Expendables 3's `FPL_MainFeature_UR` gives `Unrated Cut`). Ruling OR5 (2026-10-17): a Java-labelled cut
  that no `MainMovie`'s scan title belongs to or shares an 8.2 group with (H4) is not dropped; rule 1 plans it beside
  the main movies as an unnamed cut with `unnamed_cut` and the note `unlisted_label_cut`, so the owner names it with
  Assign titles, and the title confidence stays 0.98.
- 8.3 rule 2 runs before identification with English only, and again with the original language once TMDb gives it;
  only the variant chosen within a cut can change between the two passes.
- On a DVD, TheDiscDB's `source_file` is the DVD title number (attribute 24), compared as a number; DVD title groups
  are keyed by attribute 24 and the segment map, because a DVD's segment numbers restart in every title.
- The closed-caption stream of a DVD is recognised by its codec (`S_CC608/DVD`), not by attribute 41 alone.
- Three TheDiscDB description kinds are provisional, because TheDiscDB's wording for them is not yet known: isolated
  score, music and effects, and a plain subtitle description (one containing "subtitle" that does not also name
  commentary, or one equal to the track's own language name).
- The object-based audio extension is read from ffprobe's profile ("Atmos", "DTS:X"), else from attribute 7
  ("TrueHD Atmos"); both spellings are provisional until experiment 6 runs (ruling OR7: kept provisional).
- In 8.6's ranking, a lossy track without bitrate text comes after every lossy track with one, and guard G1 treats it
  as at or below 256 kb/s.
- A title with exactly one English subtitle stream and no subtitle events keeps it as regular "English" at 0.90.
- The text rule's disagreement (8.7 "Text") compares roles only, and one review note names every disagreeing stream.
- Library paths use the root's own separator (a backslash when the root holds one or is a UNC (universal naming convention) path, else a slash);
  `core` never asks the host. The 255-byte cap cuts on a character boundary after Unicode normalisation form C (NFC).
- `Settings` also refuses an `auto_threshold` outside 0 to 1, `subs_diff_max` above `subs_same_min`, a zero
  `minlength_s`, `feature_min_minutes` or `file_phase_concurrency`, and a blank or relative library root (F14).
- The verification needs every expected track to pair in order and tolerates no extra track in the file; a codec the
  table does not know is not compared, and the report says so (F1); read errors are kept by offset with their count and
  the error text only, never the drive's name or serial number.
- Verification lowers no confidence: a failed or read-error rip holds the job through its review reason
  (`verify_failed`, `read_errors`).
- The owner's review decisions each cover one confidence and set it to 1.00; a re-plan and the file decision both
  apply them, so a job approved at the disposition gate needs no re-plan.
- 10.1's two transitions to `Ripping` from `Copying` pass through the direct plan's space check, and a failed eject
  ends the disc phase like an eject and sends `eject_failed` instead of `disc_done`.
- VC-1 video (`V_MS/VFW/WVC1`) is a codec family of its own (ruling RP26), matched on attribute 5 alone because no
  recording shows its attributes 6 and 7, and its rip verifies against mkvmerge's documented name `VC-1`, a spelling
  not yet seen either.
- 8.3 rule 4 lowers a rule 3 plan only when every feature is more than 10% from TMDb's runtime.
- TheDiscDB maps a TV disc only when every Episode item matches a scan title; otherwise the disc is unmapped (0.60,
  `tv_unmapped`, whole-disc copy). A play-all-shaped title TheDiscDB labels an Episode is dropped only when the other
  kept Episode items map every episode it names (F3).
- A decision without a path (no library root for its role, a blank one included, no TMDb identity, or a feature without a video height) is
  held for review with no review reason, since 8.8's closed set has none for it. An episode without a season or
  episode number has no path either, and raises `tv_unmapped`.

## Left for phase 2

- The `platform`, `tools`, `clients` and `engine` crates and `lazymkvctl` (design spec 4.2 and 16, row 2), which
  perform the job reducer's actions and fold `plan`, `decide` and `retune` into `lazymkvctl`.
- Keeping remembered decisions, season sessions and jobs; reading the library for `LibraryFacts` and path collisions.
- The TheDiscDB and TMDb clients, and the live check of the TMDb shapes that phase 1's synthetic answers follow.
- The TMDb client's query rule (J8): when the disc name's `tagged` differs from its `cleaned` once folded, search with
  both and pass `plan::plan` the two first pages merged in order, the cleaned name's results first and then the tagged
  name's results not already among them.
- The DVD disc ID, the decoding of bitmap subtitles for OCR (decision D22), the app's own recording format and the
  form of the tuning evidence, D17's bundling table and code signing (RP10, RP13).
- Performing the reducer's `RefreshIdentity` (persisting the plan `plan::refresh_identity` gives and passing it to
  Decide), and giving `ReviewApproved` the reasons the review screen showed and `ReFinalise` the filed decision's.
- A lookup answer that arrives while the job is not in AwaitingLookup (OR13).
- A restart between a lookup's answer and the identity refresh, so that RefreshIdentity still runs (OR13).
- The collision answer by path: F5 already matches an answer to a decision's plain path; the engine still has to show
  the owner the colliding paths and keep the answers with the job (OR13).
- A relative root given to `decide` (OR13).

## Tests

`cargo test --workspace -- --list` on the mini PC (Windows), on the tree of commit `0388aae`, lists 1932 tests: 1527 in `lazymkv-core`, 394 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
At `666fc16` (the merge of the port, where the decision core's branch starts) the same command listed 603, so the decision core added 1329.
After the fixes of the branch's final review the same command lists 1965: 1557 in `lazymkv-core`, 397 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
After the fixes of the review of pull request #3 (F1 to F15) the same command lists 1986: 1578 in `lazymkv-core`, 397 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
After the rulings pull request the same command lists 2030: 1617 in `lazymkv-core`, 402 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
After the fixes of the review of pull request #4 (G1 to G15 and G8a) the same command lists 2038: 1624 in `lazymkv-core`, 403 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
After the owner's answers of 2026-10-19 (H1 to H8) the same command lists 2046: 1632 in `lazymkv-core`, 403 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
After the fixes of the second review of pull request #4 (J1 to J15) the same command lists 2051: 1637 in `lazymkv-core`, 403 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
The lists are the mini PC's: a test compiled only on Windows (such as `lazymkv-capture`'s eject test) is in them, and a test compiled only on Unix is not.

Design spec 15 asks for a test of every rule of section 8 for its normal case and for each guard, review reason and
confidence row, and a test per transition and per resume rule of the state reducer; the plan's tasks hold to that.
The tests are:

- unit tests at the bottom of each module (or in a sibling `tests.rs` for `tracks::audio`, `tracks::subtitle`, `job`
  and `lookup`), over synthetic scans and answers and over the pinned TheDiscDB logs (`corpus/thediscdb`) and API
  answers (`corpus/thediscdb-api`); these always run;
- `crates/core/tests/thediscdb_corpus.rs`, the pinned logs as a corpus;
- the `lazymkv-dev` integration tests: `lookup_answers.rs` (the recorded TheDiscDB and TMDb answers of the evidence
  clone), `disc_model.rs` (the disc model over every recording's `scan.txt`), `clone_values.rs` (ruling RP21 over the
  clone: no value only the clone holds is written under `crates/`),
  `verify.rs` (the recordings' rips against their scans), `plan.rs` (the command over the nine pinned logs, which
  always run, and the four recordings), `decide.rs` (the port's golden comparisons, unchanged, and the new `plan`
  and `decision` sections, including the Office's twin tracks dropped as duplicates by their packet MD5 sums),
  `labels_subtitles.rs` and `retune_subtitles.rs` (the subtitle labels and harness, over a synthetic recording and
  over the evidence clone).

The tests that read the evidence clone print `skipped: <reason>` and pass without it, never silently and never
failing for a missing input. Continuous integration (`.github/workflows/ci.yml`) builds and tests the workspace on Linux, macOS and
Windows without the clone. With it, on the mini PC:

```bash
scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace"
```
