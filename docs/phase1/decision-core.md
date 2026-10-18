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
| `review` | 8.8, and the notes of 8.6, 8.7, 8.9 and 7 | The closed sets of review reasons and notes, with the spec's spellings |
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
| Expendables 3, `disc01.txt` | discdb_one 0.95, movie 138103 | movie | database: title 41 (00756.mpls), edition `Unrated Cut` (from the Java label table, `FPL_MainFeature_UR`; see Choices the owner should confirm) | 0.98, Direct | none |
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
  read or watched, sure and given before the verdict, and counts the UK-style SDH track from `style`.
- RP26. VC-1 video (`V_MS/VFW/WVC1`) is a codec family of its own, matched on attribute 5 alone, with the display
  and mkvmerge name `VC-1`, and no tier.
- RP27. Four note names the design spec describes only in words keep the plan's spellings until the owner confirms
  them: `possible_audio_description` (the port's ruling RP7), `lossy_before_lossless` (decision D9),
  `direct_mode_fallback` (6.3 step 6a) and `label_rule_unavailable` (8.3 rule 2 with 10.2).

### Rulings made during execution

Task 5: under 8.3 rule 1, a TheDiscDB-marked main movie whose Java label has an unrecognised suffix keeps 0.98 with no `unnamed_cut` (the label table belongs to rule 2). Task 14 (amended by F12 below): in an approved `Decided`, a decision whose reasons include `path_collision` or a reason the owner did not approve, or whose `DecisionDone.has_path` is false, holds the job again at the disposition gate, while a low confidence alone never holds an approved job again (design spec 7, 6.4 step 12 and 11 under Approve). Task 18: the row 14 harness of `retune --subtitles` reads `subs/index.json` through `verify::subs_index`, the one reader of that file (the one-copy rule).

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
  TheDiscDB does not know; ruling R1 then settled it.
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

## Choices the owner should confirm

The plan settled these where the spec leaves a choice open; each is pinned by a test, and each can be changed by
changing that test and its rule. The first four are open questions the plan puts to the owner; until the owner rules,
the plan keeps the reading given.

- Ruling R1 (2026-10-17): 8.3 rule 3.1 counts a play-all title only when every part is episode-length (15 to 120
  minutes); a play-all of extras (Smile's deleted-scenes playlist, title 117) is an ordinary group, so a movie disc
  with one stays a movie without TheDiscDB. 8.5 still drops any concatenation title from a TV disc (R15).
- Open question: two label cuts with the same edition name. 8.3 rule 2's table names both the plain `FPL_MainFeature`
  row and the `_R` row "Theatrical Cut". A disc carrying both is planned at 0.60 with `title_ambiguous` ("two cuts are
  both named Theatrical Cut") rather than 0.90; two TheDiscDB main movies that would be filed under one name get the
  same 0.60 rather than 0.98. The owner may rule that they are one cut, or that 0.90 stands.
- Open question: the default subtitle when the default audio is not English and no regular English track is kept.
  8.10 names only the regular track; the plan makes the SDH track the default. The owner may rule that no subtitle is
  default.
- Open question: leading periods in a path component. 8.9 trims trailing spaces and periods only, so a title that
  begins with periods (`...And Justice for All`) would give a folder and a file hidden on the share. The plan drops
  leading periods as well as leading spaces (`And Justice for All (1979) {tmdb-ID}`). The owner may confirm it or rule
  another treatment.
- A TMDb record without a runtime (or a season without episode runtimes) fails 8.1's runtime check, so step 4 cannot
  pass and step 5 applies (0.50, `identity_ambiguous`).
- Identity step 2 (TheDiscDB by both disc ID and fingerprint, 0.99) needs both records and, when both carry a TMDb ID,
  the same one; with different IDs the fingerprint's record wins at step 3 (0.95) and the disagreement is written to
  the identity's evidence.
- 8.3 rule 1 ("reconciles with the scan") means every `MainMovie` title of TheDiscDB's record matches a scan title;
  if one matches none, the record falls to rule 2. A `MainMovie`'s edition comes from its item title, else from the
  Java label table (Expendables 3's `FPL_MainFeature_UR` gives `Unrated Cut`).
- 8.3 rule 2 runs before identification with English only, and again with the original language once TMDb gives it;
  only the variant chosen within a cut can change between the two passes.
- On a DVD, TheDiscDB's `source_file` is the DVD title number (attribute 24), compared as a number; DVD title groups
  are keyed by attribute 24 and the segment map, because a DVD's segment numbers restart in every title.
- The closed-caption stream of a DVD is recognised by its codec (`S_CC608/DVD`), not by attribute 41 alone.
- Three TheDiscDB description kinds are provisional, because TheDiscDB's wording for them is not yet known: isolated
  score, music and effects, and a plain subtitle description (one containing "subtitle" that does not also name
  commentary, or one equal to the track's own language name).
- The object-based audio extension is read from ffprobe's profile ("Atmos", "DTS:X"), else from attribute 7
  ("TrueHD Atmos"); both spellings are provisional until experiment 6 runs.
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
- The DVD disc ID, the decoding of bitmap subtitles for OCR (decision D22), the app's own recording format and the
  form of the tuning evidence, D17's bundling table and code signing (RP10, RP13).
- Performing the reducer's `RefreshIdentity` (persisting the plan `plan::refresh_identity` gives and passing it to
  Decide), and giving `ReviewApproved` the reasons the review screen showed and `ReFinalise` the filed decision's.

## Tests

`cargo test --workspace -- --list` on the mini PC (Windows), on the tree of commit `0388aae`, lists 1932 tests: 1527 in `lazymkv-core`, 394 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
At `666fc16` (the merge of the port, where the decision core's branch starts) the same command listed 603, so the decision core added 1329.
After the fixes of the branch's final review the same command lists 1965: 1557 in `lazymkv-core`, 397 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
After the fixes of the review of pull request #3 (F1 to F15) the same command lists 1986: 1578 in `lazymkv-core`, 397 in `lazymkv-dev`, 8 in `lazymkv-capture` and 3 in `fake-tools`.
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
