# Engine menu maps: design

Date: 2026-11-24.
Status: the owner answered seven questions in the design conversation of 2026-11-24 (EO-1 to EO-7, part 3) and
approved the design in three parts the same day ("Approve part 1", "Approve part 2", "Approve part 3"). The rest is
the designer's, with every call listed in part 17 for the owner to check. Nothing is built: the implementation plan,
any push and any pull request wait for the owner's word.

"The spec" is `docs/superpowers/specs/2026-10-04-lazymkv-design.md`; "the menu maps design" is
`docs/superpowers/specs/2026-11-17-menu-maps-design.md`; "the format" is `docs/menu-maps/format.md`; "the phase 2
design" is `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md`; "the phase 3 design" is
`docs/superpowers/specs/2026-11-24-phase-3-design.md`; "the notes" are `docs/phase2/engine.md`. EO-1 to EO-7 are the
owner's answers and EM-1 to EM-22 the designer's calls, both recorded here.

## 1. Intent

A menu map says what a disc's own menu calls each choice and which stream the choice selects. Since pull requests #35
and #39 the decision core drops an audio track that a map confirms as a commentary or an audio description (the
format, "The rule"). Only the developer command reads a map (`lazymkv-dev decide --menu`); the engine gives none, so no
real job is changed by a map. This design closes that gap: an engine job finds its disc's map, decides with it, shows
what the menu dropped, and never fails or loses a track because of a bad map.

Why now: the owner chose it on 2026-11-24 as the next menu increment. Asked for "A design first, for your approval,
covering where maps live on the mini PC, how a job finds its disc's map, what happens when there is none, and what the
job shows about a menu drop. Nothing is built until you approve it.", the owner answered "Yes, design it next".

## 2. What exists today

- **The rule is in `core`.** `tracks::audio::decide_audio` takes the menu's confirmations in `AudioFacts.menu` and
  drops them under the guards the owner kept (the format, "The rule"; MR-3, MR-4, MR-5, MR-8, MR-9, MR-13, MR-17,
  MA-2).
- **One renderer.** The engine builds `DecideInputs` and renders each title's `decide.json` through `lazymkv-dev`'s
  pure `decide_doc`, the function `lazymkv-dev decide` prints with (R-B1). Both of the engine's builders set
  `menu: None` (`crates/engine/src/file_phase.rs`, `engine_decide_inputs` and `read_back_inputs`).
- **A job folder already holds what the rule needs from the disc.** It is a recording (phase 2 design, part 7.1):
  `meta.json` carries the disc ID, the value a map's `disc.disc_id_sha1` is checked against (M3-6), and `nav\` holds
  the disc's playlists under `nav\BDMV\PLAYLIST`, which the ties are read from (MR-8).
- **A job keeps what it decided with.** `settings.json`, the TMDb answers and `remembered.json` are copied into the job
  folder, and `lazymkvctl decide --job` replays the decision from those copies, so `compare-job` can compare the two
  documents.
- **What a job folder does not hold.** `nav\` has the index, the movie object, the playlists and the metadata, not the
  Java programs and clip information a map is made from. A map can therefore be made only for a disc captured with the
  menu work's own capture tooling: 40 discs are captured and 13 are mapped. A disc the engine sees for the first time
  has no map when it is ripped.
- **Undo exists in `core` only.** A Keep override wins over the menu (MR-3), but the engine passes no overrides and
  accepts no review answer until phase 3's review is built (phase 3 design, part 6).
- **`status` and `history` show the job's own notes.** `core::ipc::job_summary` copies `record.notes` and none of the
  notes of the titles' decisions.

## 3. The owner's answers of 2026-11-24 (EO-1 to EO-7)

Each answer was a button chosen from a question that stated a recommendation and what each choice costs.

- **EO-1, where maps live.** Asked "Where should the engine read menu maps from? A new setting names the folder either
  way (unset = no maps, as today); your answer decides how a finished map travels from the Mac.", the owner chose "NAS
  share folder (Recommended)" over "Mini PC protected folder". The chosen option read: "A folder on the media share
  beside staging. A finished map is copied there; nothing is ever written to the mini PC for a map. Cost if wrong:
  anyone who can write to the share could add or alter a map (they can already alter the library itself), and when the
  share does not answer the job waits, as it does for the library."
- **EO-2, what a job shows.** Asked "When a real job drops a track on the menu's word, what should the job show?
  Today's rule (MR-11) records it only inside decide.json: no note, no review.", the owner chose "A note on the job
  (Recommended)" over "Note plus notification" and "Nothing new". The chosen option read: "Each drop adds a note naming
  the track, the role and the menu's wording. It shows in lazymkvctl status and history and, once phase 3's review
  exists, on the review sheet beside the track, where answering 'keep' undoes it. The job is not held and no extra
  notification is sent."
- **EO-3, a title with no programme reference.** Asked "On a title with no programme reference (every kept audio track
  lacks a language code), nothing but the menu's own tie says which track is the film's audio. What may the menu's word
  drop there?", the owner chose "Only if Play's track is known (Recommended)" over "Drop as today" and "Never drop
  there". The chosen option read: "The track plain Play selects stands in for the reference: it is never dropped
  (already so), and when the map does not show which track Play selects, nothing is dropped on that title."
- **EO-4, a doubted reference.** Asked "lazymkv's content check can say of a track: 'this may be the real film audio;
  the reference is in doubt' (review reason audio_reference_unclear). When the menu drops that same track as a
  commentary, the doubt is no longer raised today. What should happen?", the owner chose "Drop, and hold for review
  (Recommended)" over "Drop silently, as today" and "Keep the track and hold".
- **EO-5, the menu against the reference.** Asked "The menu says, with a strong tie, that the track lazymkv picked as
  the programme reference is itself a commentary or audio description. Today that track is kept and nothing is said.
  Should the job be held for review?", the owner chose "Yes, hold for review (Recommended)" over "No, keep silently".
  The chosen option read: "The track is kept as today and the job is held with audio_reference_unclear, with a note
  that the menu calls the reference a commentary. I would treat the sister case the same way: when dropping the
  confirmed tracks would change which track is the reference."
- **EO-6, the approach.** Asked "Which way should the engine get a disc's menu map into a job?", the owner chose "A:
  look up at decide (Recommended)" over "B: import into the engine" and "C: fix at plan time" (part 6).
- **EO-7, publishing.** Asked "Maps are made on this Mac, and the media share is not mounted here right now. How should
  a finished map be copied into the maps folder on the share?", the owner chose "Mac mounts the share (Recommended)"
  over "Through the mini PC" and "I will copy them by hand".

## 4. Goals, non-goals and anti-goals

Goals
1. A real job whose disc has a published map decides with it: the tracks the menu confirms are dropped, under every
   guard of the rule.
2. A job with no map, or with a map that cannot be used, decides exactly as it does today.
3. A person looking at the job sees what the menu dropped, in the menu's own words, without opening a JSON file.
4. A job can be replayed: the map it decided with is in its folder, and `compare-job` finds no difference.
5. Publishing a map is one command on the Mac, and nothing is written to the mini PC for it.

Non-goals (each has a later home in part 18)
- Making or replaying maps on the mini PC, or inside the engine (the menu maps design's non-goal stands: the mini PC
  only reads map files).
- The engine capturing the files a map is made from, so that a disc new to lazymkv could be mapped from its job.
- Subtitles, and any menu role other than commentary and audio description.
- Applying a new or corrected map to a filed job without the owner asking for it.
- More than one map for a disc (a map depends on the player profile it was made with), and a shared database of maps.

Anti-goals (these would make it a failure even if it works)
- A job that fails, is held, or loses a track because a map is bad, stale or meant for another disc.
- A share that does not answer read as "this disc has no map".
- A routine step in which the owner types, photographs or hand-copies anything for a map.
- The engine writing to, renaming in or deleting from the maps folder.
- A disc title, a person's name or a map's text in the public repository. Maps and jobs are private data; every
  example in this document is made up.

## 5. Constraints

- The repository is public: no disc title, person's name, host, share or user name in any committed file. A disc is a
  neutral slug or nothing.
- `core` stays pure (RP19). The engine and `lazymkv-dev decide` keep one renderer (R-B1).
- A job keeps the settings it started with (ENG-19).
- Nothing is fetched at run time except database answers (D17). A map is read from a folder the owner names.
- Over SSH the share is denied; only the engine, in the desktop session, reaches it (phase 2 design, part 6.1).
- No new crate without the owner's yes. Rust builds stay `--locked --offline`.
- Nothing is built on, written to or changed on the mini PC without the owner's explicit yes. Nothing is pushed
  without the owner's word. Commits are signed with the owner's noreply identity and carry no agent attribution.
- `crates/core/src/verify.rs` and `crates/core/src/codec.rs` belong to pull request #37 and are not touched.

## 6. Approaches considered

| Approach | What it is | Trade-off |
|---|---|---|
| **A. Look up at Decide, keep a copy in the job (chosen)** | A setting names a maps folder with one subfolder per disc ID. Every run of the Decide step looks in the job's disc's folder, checks the map, copies it into the job folder and decides from the copy | A map that arrives or is corrected later takes effect the next time the job decides. Decide depends on one more read from a share |
| B. Import into the engine | A new `lazymkvctl` command hands each map to the engine, which checks it and stores it in its protected data folder | A bad map is refused when it is added, and the store is protected like the settings. More to build, a store to manage, and every map is written to the mini PC |
| C. Fix at plan time | The map is looked up once, while the disc is in the drive, like a remembered decision | The simplest replay. A map made after the rip never reaches the job without ripping again, and for a disc new to lazymkv that is the normal case |

The owner chose A (EO-6). One variant of A was dropped: folders named by slug. A slug is readable, but the engine
would have to list the maps folder and open every map to find a disc, and two folders could claim one disc.

## 7. Where maps live

### 7.1 The setting

`menu_maps_root` (spec 14): the folder the engine reads menu maps from, an absolute local or UNC path. Default: unset.
Unset means no map is ever looked up, and every document a job writes is byte for byte what it is today.

`Settings::validate` refuses, with one problem each: a blank value; a path that is not absolute (`is_absolute_root`);
and a maps folder inside `staging_root`, `tuning_root`, `movies_root` or `tv_root`, or any of those inside it
(`path_inside`, both ways). The retention cleanup works under staging, the media servers index the library roots, and
the tuning refresh mirrors under `tuning_root`; the maps folder is kept apart from all three.

The key is written only when it is set. `Settings` writes its other unset roots as `null`, and a plan section, a job's
`settings.json`, the IPC snapshot and the goldens all hold the settings whole; an unset `menu_maps_root` appears in
none of them, which is what keeps today's documents byte-identical (EM-21).

`ShareConnector` treats it as it treats the other roots (phase 2 design, part 6.1): the session's own access first,
lazymkv's stored credential second. The engine only reads there. It never creates the folder, and never writes,
renames or deletes anything in it (EO-1's "the mini PC only reads").

The self-test gains one check, `menu_maps_root`, only while the setting is set: the share answers and the path is a
folder. It makes no write probe, because the engine never writes there (EM-13).

### 7.2 The layout

```
<menu_maps_root>\
  <DISC ID>\                 40 upper-case hex digits
    menu-map.json            the map (the format, v1)
    menu-off.tsv             its off-decisions, when it has any (part 9)
```

`<DISC ID>` is the disc ID of spec 5, the SHA-1 of the disc's unit key file, the value a job's `meta.json` calls
`disc_id` and a map calls `disc.disc_id_sha1`. The folder name is only an address: whether a map belongs to a disc is
still decided by the ID inside the map (M3-6, MR-5).

A job looks in exactly one folder and never lists the maps folder, so "several candidates for one disc" cannot arise
(EM-1). A disc with no usable disc ID is never looked up: a DVD, a disc without AACS, a `meta.json` whose `disc_id` is
missing or is not 40 hex digits. Only a value checked to be 40 ASCII hex digits, upper-cased, reaches the file system
as a folder name, as MR-13 does for playlist names.

The engine reads those two files and nothing else in the folder. The pictures, the key-press scripts and
`replay.json` of a map stay where the map was made (EM-2).

## 8. Publishing a map

A new developer command, run on the Mac where maps are made, with the share mounted there (EO-7):

```
lazymkv-dev menu publish <map> --to <maps root>
```

`<map>` is a map folder or its `menu-map.json`, as `menu show` takes it. The command:

1. reads the map as `menu show` does and refuses it with every problem of the format's rules;
2. refuses a map whose `disc.disc_id_sha1` is not 40 hex digits (`the map has no disc ID, so no job could find it`);
3. reads `menu-off.tsv` beside it and refuses a file it cannot parse, and a line that is stale against this map
   (part 9);
4. refuses a map over 4 MiB or an off file over 1 MiB (the engine's limits, part 10.2);
5. writes the two files into `<maps root>\<DISC ID>\`, each to a temporary name in that folder and then renamed into
   place, and removes an `menu-off.tsv` left there when the map now has none.

It never creates `<maps root>` itself, touches no other disc's folder, and deletes no folder. Its last line is counts
and the ID only (`PUBLISHED <DISC ID> choices=<n> decisions=<m> undecided=<k>`), never a label. Exit 0 when published;
2 for an argument, map or off-decisions problem.

The two files cannot be replaced in one step, so a job could read a new map with an old off file. Part 9's binding
makes that harmless: a decision that does not fit the map it is read with is ignored.

Publishing writes to the owner's share, so it runs at the owner's word. Taking a map back is deleting its folder by
hand.

## 9. Binding the off-decisions to their map

Today a line of `menu-off.tsv` names a choice by id only, so a map made again with other ids would misapply a `role`
line (a limit the format states). A proposal, approved with part 1:

- A line becomes three fields: `<decision><TAB><choice id><TAB><text>`. `<text>` is the choice's text as the role
  reading reads it (`core::menu::role_text`: the innermost heading, `: `, the label, white space collapsed), the
  wording the decision was made from. Made up: `off<TAB>c012<TAB>COMMENTARY: Off`.
- **A decision applies only while it fits.** It is applied to a map only when the map has an audio choice with that id
  whose `role_text` equals the line's text, code point for code point.
- **A stale line.** When the map has an audio choice with that id and its text differs from the line's, the line is
  stale: it is not applied, the choice is read as having no decision (so a text the net catches leaves its stream
  without a role, MR-6), and its id is listed under a new key of the `roles` block, `off_stale`.
- A line with two fields is refused (`line <n>: expected "off<TAB><choice id><TAB><the choice's text>" or "role<TAB>
  ..."`), in `lazymkv-dev decide --menu` and `menu publish` as any other refusal of the file (EM-3). One id given two
  different texts or two different decisions is refused as one id decided both ways is today.
- `off_unused` keeps its meaning for the lines that are not stale: a decision whose id names no audio choice of the
  map, or one whose text fits and holds no role word. The two lists never share an id. An unused line is no refusal
  anywhere, as today; it has nothing to act on (EM-22).

Why the wording and not a hash of the map file: a decision is made from the wording alone (menu maps design 5.3), so
the wording is exactly what it must still fit. A map that is saved again with a note added or a level raised keeps its
decisions; a hash of the file would void all of them. And a mismatched pair read in the middle of a publish is safe.

A writer needs each choice's exact text. A small listing is part of the build: for a map, one line for each audio
choice the net catches, in the off file's own three-field form, its first field `off`, `role`, `undecided` or `stale`
(the builder shapes the command; `lazymkv-dev menu off <map>` is the proposal).

The 13 existing off files are private and are rewritten once in the new form, by script, at acceptance.

## 10. How a job gets its map

### 10.1 When

In the Decide step (spec 6.4 step 11; `RealSteps::decide`), at every run of it: the first, one after a restart, a
retry, and every later one that phase 3 adds (an approval with answers, a fix-after). The lookup comes after the
library root's first reachability check and before the first title's document is built, once for the job.

### 10.2 The lookup

After the lookup the job folder holds exactly the pair this run decides with, or neither file. That holds whatever
the outcome below: a copy left by an earlier run is replaced or removed.

1. **The setting.** The job's own `settings.json` has no `menu_maps_root` (unset or blank): no lookup.
2. **The disc ID.** The job's `meta.json` cannot be read, or holds no usable disc ID (part 7.2): no lookup; one line
   in `job.log`.
3. **The share.** `reach(menu_maps_root)`. A share that does not answer fails the step, as the library root does
   (Ruling W22): `DecisionFailed { Unknown }` with `the share holding <root> does not answer (<why>); the menu map was
   not read, and a retry once the share answers decides again`. It is never read as "no map" (EO-1).
4. **The files.** `<root>\<DISC ID>\menu-map.json` and `menu-off.tsv` are read whole. A file over its size limit, 4 MiB
   for the map and 1 MiB for the off file, is not read: the pair is not used (step 6). A missing folder or a missing
   map is "no map"; a missing off file is "no decisions". Any other read error fails the step with its sentence.
5. **The share again.** `reach` is asked once more after the reads, as Decide's second library check does (review
   round 1 of pull request #14, R28), because a missing file and a share that just dropped read alike. Not answering:
   fail as in step 3, and nothing read is used.
6. **The check.** The pair is used only if all of these hold: the map is strict UTF-8 and passes `core::menu::read`
   (the format's rules, the reader's rules, the version rule); the off file passes `parse_off` in part 9's form; the
   map's `disc.disc_id_sha1` matches the job's disc ID (`menu::identity` gives `Matches`). Otherwise the pair is **not
   used**: every problem goes to `job.log`, the job gets the note `menu_map_not_used` (part 12), and the job decides
   as it does with no map. A refused off file makes the whole pair unused, not a map without decisions (EM-4).
7. **The copy.** The bytes as read are written to `<job folder>\menu-map.json` and `<job folder>\menu-off.tsv` with
   the engine's atomic write. A write that fails is a failure of the step, as any write to the job folder is.
8. **The inputs.** Each title's `DecideInputs.menu` is built from the job folder's copy by the function `lazymkv-dev
   decide --menu` uses for a map beside a recording: the map, its off-decisions, the disc ID of the job's `meta.json`,
   and the playlists the map names, read from the job's own `nav\BDMV\PLAYLIST` (MR-8, MR-13). Both of the engine's
   builders use it, the one with the probe in memory and the one that reads back after a restart.

`job.log` gets one line per run saying which of these happened: no maps folder set; no disc ID; no map for this disc;
a map used (with the SHA-256 of the map file); a map not used, followed by its problems.

A missing map is the ordinary case and raises nothing a person has to read: no note, no review, no notification.

### 10.3 The code path

- `crates/dev`: the private wiring of `decide --menu` (`read_map`, `read_off`, `read_playlists`, `disc_id_of`,
  `playlist_names`, `menu_input`) becomes one public function that returns a `MenuInput` for a recording and a map
  file, or the refusal. `run` and the engine both call it, so the two read one way (R-B1).
- `crates/engine`: the lookup of part 10.2 in the Decide step; `engine_decide_inputs` and `read_back_inputs` take the
  job's menu input in place of `None`.
- The relay (`lazymkvctl decide --job JOB --title KEY`) adds `--menu <job folder>\menu-map.json` when the job folder
  has the file, as it adds `--settings` and the other replay files. A `--menu` the caller types replaces the job's, so
  another map can be tried against a job (left to the builder how the two words are reconciled).

### 10.4 What `decide.json` gains

Exactly what `lazymkv-dev decide --menu` prints today (the format, "How decide shows a map"), with part 13's and part
12's changes: the `menu` key with the title's block and its `roles`, `decision.facts.menu_streams`,
`decision.decision.audio.menu` with each confirmation's outcome, and the dropped streams with `{"why": "menu"}`. In
the engine's file the `menu` key sits before the two version keys the engine appends. A job with no map has no `menu`
key and its documents are unchanged.

The copies are part of the job's recording: the tuning refresh copies them with the rest of the job folder to
`<tuning_root>\recordings\<job id>\`. A recording still holds no subtitle text; a map holds the disc's menu wording,
which is private data like the rest of a recording.

## 11. When something is wrong

A bad map never fails a job and never drops a track. Only a share that does not answer, or a file that cannot be
read or written, fails the step, as it already does for the library and the job folder.

| Case | The job | What is recorded |
|---|---|---|
| `menu_maps_root` unset | decides as today | nothing |
| No usable disc ID | decides as today | a `job.log` line |
| No folder, or no `menu-map.json`, for this disc | decides as today | a `job.log` line |
| The map is over its size limit, is not UTF-8 or JSON, breaks the format's rules, or is another format version | decides without it | the note `menu_map_not_used`; every problem in `job.log` |
| The map inside names another disc, or no disc | decides without it | the same |
| `menu-off.tsv` is over its limit or cannot be parsed | the pair is not used | the same |
| A stale off line | the line is ignored; its choice has no decision | `off_stale` in the title's `roles` block |
| A title's own playlist is missing, cannot be parsed, or its play items differ | no stream of that title is tied, so nothing is confirmed (MR-8) | `roles.acts` false with `roles.why` |
| A playlist entry disagrees with the scan in language, codec or count | the tie is `tie-conflict`, which confirms nothing (MR-9) | the row's `why` |
| The share does not answer, or a read fails with another error | Decide fails and is retried; phase 3 makes it wait for the share (its part 9.3) | the failure's message |
| The copy cannot be written into the job folder | Decide fails | the failure's message |

A share that does not answer, in today's engine and in phase 3's:
- **Today.** A share lost during Decide fails the job as `Unknown` (V7), and `retry` decides again. The maps folder's
  share does the same. The job is not in `AwaitingShare`, so nothing cycles.
- **Once phase 3's failures pull request is built** (phase 3 design, parts 9.3 and 9.4), a step that finds a root it
  needs unreachable waits in `AwaitingShare` and returns by itself. The maps folder is then one of the roots a
  file-phase return needs, in the step's own check and in the waiting job's re-check (`share_back` reads staging and
  the library root today). That design's words cover it; its code must name the root. Whichever of the two is built
  second adds it, and its plan says so.

## 12. What a job shows

**The note `menu_dropped` (EO-2).** `decide_audio` raises one for each stream it drops on the menu's word. It names
the stream and holds one plain sentence with the role and the wording of the choices that label the stream, in map
order. Made up:

```
the disc's menu calls stream 4 a commentary ("SETUP > AUDIO: Commentary by Person A"); dropped on the menu's word
```

For this the confirmation the decision is given (`MenuConfirmation`) carries the choices' shown labels beside their
ids. The note is part of the decision, so it is written in `decide.json` and kept with the title's decision in the
job's record. This replaces MR-11's "a menu drop raises no note"; "no review" stands, so a job with a menu drop and
nothing else to hold it files. `lazymkv-dev decide --menu` prints the note too, since it is the same rule.

**The note `menu_reference_conflict` (EO-5)** goes with the review reason of part 13.3 and says what the menu says of
the reference, or that a drop would have moved it.

**The note `menu_map_not_used`** is the job's own (in `state.json`, like `tuning_evidence_pending`): a map was found
for the disc and was not used, and `job.log` says why. It reflects the last run of Decide: a later run that uses a map
or finds none removes it.

**`status` and `history`.** The job summary lists the notes of the titles' decisions after the job's own, in the
plan's order of titles (EM-8). Without this the new note would not be seen there. It also brings the existing notes of
a decision (`possible_commentary` and the others) into both commands, which spec 13 already asks of history ("with
their decisions and notes").

**Notifications** do not change (EO-2).

**Phase 3's review sheet.** Its document lists every audio track with what the decision did to it and why, and has an
open-ended evidence list per item (phase 3 design, part 6.2). Once both this design and that document are built, a
track the menu dropped reads "dropped: the disc's menu calls it a commentary", with the wording, and the menu's
wording is listed as evidence for every audio track a menu choice labels. Whichever of the two is built second adds
the sentence. This takes up the item the phase 3 design leaves for later ("menu-map labels as evidence on the sheet").

## 13. Rule changes

These three change `core::tracks::audio::decide_audio`, so the engine and `lazymkv-dev decide --menu` behave alike.
With no confirmation the decision is, as before, the one without a map.

### 13.1 A title with no programme reference (EO-3)

Today a title whose kept audio streams all lack a language code has no reference, and the menu's confirmations are
dropped as long as one track remains (MA-2). The owner's answer: the stream plain Play selects stands in for the
reference.

- The `roles` block gains a key, `play`: the scan stream plain Play itself selects, placed as MR-17 places it, or null
  when it is not known. The decision is given it beside the confirmations.
- When the first arrangement has no reference and there is a candidate to drop: the candidates are dropped only if
  Play's stream is known and is among the kept tracks. Otherwise none is dropped, and each candidate gets the new
  outcome `no_reference` (EM-10 reads "known" as "known and kept").
- Play's own stream is never a candidate (MR-17), so it stays.
- The outcome is tested before `would_leave_none`. The outcomes are then seven: `dropped`, `not_kept`, `overridden`,
  `reference`, `no_reference`, `reference_would_change`, `would_leave_none`.

### 13.2 A doubted reference (EO-4)

Today a dropped stream's own verdict raises nothing, so a `reference-unclear` verdict on a stream the menu drops no
longer raises `audio_reference_unclear`. The owner's answer: the drop stands and the job is held.

The rule: the menu's drop never removes `audio_reference_unclear`. When the arrangement before the drop raised it, the
decision carries it after the drop. The title-level cases (verdicts measured against another reference, D21's veto)
already survive a drop; this adds the per-stream case. `commentary_detected` raised only by a stream the menu drops
still goes with the stream, as the owner ruled on 2026-11-24 ("Yes, drop it").

### 13.3 The menu against the reference (EO-5)

Today a confirmation on the programme reference is kept with the outcome `reference`, and candidates whose drop would
move the reference are kept with `reference_would_change`; neither says anything. The owner's answer covers both: the
tracks are kept as today, and the decision adds the review reason `audio_reference_unclear` and the note
`menu_reference_conflict`. Made-up details:

```
the disc's menu calls stream 1, the programme reference, a commentary ("AUDIO: Commentary by Person A"); kept
dropping the tracks the menu confirms (streams 3, 4) would change the programme reference from 1 to 5; none dropped
```

Other candidates of a title whose reference the menu confirms are still dropped, as today; the job is held before it
files, so the owner sees both. No new review reason is added to spec 8.8's table: `audio_reference_unclear` already
means "the reference is in doubt", and phase 3's audio point answers it.

### 13.4 What does not change

The guards (MR-3, MR-4, MR-5, MR-8, MR-9, MR-13, MR-17, MA-2), step 1, D13's duplicates, the audio confidence (MR-14,
EM-14), and subtitles. An outcome that drops nothing raises no note unless part 13.3 names it (EM-9).

## 14. Undo, and switching maps off

- **A Keep override wins** over the menu (MR-3), from a remembered decision or a review answer; the confirmation's
  outcome is `overridden`.
- **With phase 3's review** (its pull requests 2 and 3): `lazymkvctl review JOB` on the filed job, the answer "keep"
  on the dropped track, and the job files again while its rip is kept. The answer is remembered (phase 3 design, Z6),
  so a later job for the same disc keeps the track too.
- **Before phase 3's review is built** the engine has no undo. The way back is to delete or correct the disc's folder
  in the maps folder and rip the disc again (EM-12).
- **A proposal for the owner:** leave `menu_maps_root` unset in the settings that file into the real library until
  phase 3's fix-after is built. The test library can use maps at once.
- **Switching off:** one disc by deleting its folder, which the next Decide of any job for that disc sees; every new
  job by unsetting the setting.
- **A map that arrives after a job filed** takes effect when that job decides again (a fix-after). That reaches only
  jobs started while the setting was set, because a job keeps the settings it started with (EM-6).

## 15. Trust

- **Who may write a map:** whoever can write to the share that holds the maps folder. The owner accepted this with
  EO-1; the same people can already change the library.
- **What a map can do:** make lazymkv leave an audio track out of a file it files. It can never drop the programme
  reference, the stream plain Play selects, or a title's last audio track, and a Keep override beats it.
- **What a map cannot do.** It cannot fail a job (part 11). It cannot run anything: it is data, read by the JSON
  library and the map's validator, and the playlist reader refuses any input without a panic. It cannot make the
  elevated engine open a path of its choosing: the only names that reach the file system are a checked disc ID as a
  folder name, two fixed file names, and five-digit playlist names inside the job's own `nav` folder (MR-13). It
  cannot fill the mini PC's disk: the size limits bound the copy kept in each job folder (EM-7).
- **The level.** Unchanged: only a choice at level `replayed` has a tie at all, so a map's unconfirmed choices already
  act on nothing.
- **The maker.** `made.by` and `made.checked_by` are free text that nobody can verify, so no rule reads them (EM-11).
- **Disc text.** A menu's wording now enters job folders, `decide.json`, notes, the IPC job summary and the recording
  copies under `tuning_root`. All of these are the owner's private data. Notifications carry none of it.

## 16. The build, its tests and acceptance

### 16.1 What changes, by crate

- `core`: `Settings.menu_maps_root` and its checks; the three notes; `MenuOutcome::NoReference`; the decision's inputs
  gain Play's stream and the confirmations' labels; `RolesBlock` gains `play` and `off_stale`; `parse_off` and the role
  reading take part 9's third field; the three rules of part 13; `job_summary` lists the decisions' notes.
- `dev`: the shared menu-input function (part 10.3); `menu publish` (part 8); the off listing (part 9).
- `engine`: the lookup (part 10.2); the job's note; the relay's `--menu`; the self-test's check.
- `ctl`: the two new `lazymkv-dev menu` forms are not relayed; nothing else.

Proposed as one implementation plan in two pull requests. The first holds `core` and `dev`: it changes only what
`decide --menu` prints, and ends with acceptance stage A. The second holds the engine, and ends with stage B. No new
crate is expected: the SHA-256 of `job.log`'s line comes from a crate the workspace already has.

Neither pull request needs phase 3 to be built first, but both designs touch the spec, `Note`, the job summary and
how Decide meets a lost share. When each is built is the owner's choice at plan time. Three things follow from the
order: which branch carries the spec's amendments second and merges them; which adds the maps folder to the waiting
job's roots (part 11); and which adds the menu's sentence to the review sheet (part 12).

### 16.2 Documents amended when it is built

| Where | Change |
|---|---|
| The spec, 5 | a row for `<menu_maps_root>\<disc ID>\`; the job folder gains `menu-map.json` and `menu-off.tsv` |
| The spec, 7 | the maps folder is a root the engine only reads, kept apart from the other roots |
| The spec, 8.6 step 1 | "The engine gives no map yet" becomes how the engine gives one; the no-reference rule |
| The spec, 8.6 step 6 and "Content"; 8.8 | the two decision notes; `audio_reference_unclear` is also raised by part 13.2 and 13.3 |
| The spec, 13 and 14 | the self-test's check; the setting |
| The format | the off file's third field, `off_stale`, `roles.play`, the seventh outcome, the notes, MR-11 replaced, the sentences saying the engine gives no map, the rulings EO-1 to EO-7 and EM-1 to EM-22 |
| The menu maps design | its status line and a pointer to this document; its non-goal stands |
| `docs/phase1/decision-core.md` | the decide section's `--menu` paragraph |
| The notes | a section for this increment: rulings, tests, the mini PC record |

### 16.3 Tests

Every test uses made-up maps, playlists, scans and jobs. No test reads the owner's data.

- **`core`:** each rule of part 13 with its outcome, its review reason and its note, and the cases beside it (Play's
  stream known but not kept; a doubted reference at title level; a confirmation on the reference with other
  candidates); the off file's three-field form, a stale line, the refusals; the setting's checks; the summary's notes.
- **`dev`:** `decide --menu` documents with the notes and the new keys; `menu publish` on temporary folders (a good
  pair, each refusal, an old off file removed, another disc's folder untouched); the off listing.
- **Whole jobs on the stand-in tools,** on all three systems. The stand-in disc gains a unit key file, and a made-up
  playlist that agrees with its scan. Scenarios:
  - a map present: the confirmed track is not in the filed file's plan, the note is in the record and the summary, the
    two copies are in the job folder, and `compare-job` finds no difference;
  - no maps folder set, and no map for the disc: every document byte-identical to a run of today's code;
  - a map that breaks the format's rules, a map for another disc, an off file that cannot be parsed: the job files as
    without a map, with `menu_map_not_used` and no copy in the job folder;
  - a share that does not answer before the reads, and one that drops between them: Decide fails, and a retry decides;
  - a map replaced between two runs of Decide: the second run uses the new one and the copy is replaced;
  - a restart between Probe and Decide: the read-back path gives the same document.

### 16.4 Acceptance on the owner's discs

**Stage A, on the Mac only.** Nothing touches the mini PC or the share.
- The 13 private off files are rewritten in the three-field form. The 13 maps are published into a local folder that
  stands in for the maps folder; each lands under its disc ID.
- `lazymkv-dev decide` without and with `--menu` over the copies of the 13 recordings used for the last record (the
  format, "Menu alone"). Expected, from that record: 30 confirmations, 30 dropped on 11 discs, the programme reference
  the same before and after on each, 30 `menu_dropped` notes, and every key but `decision` and `menu` byte-equal. The
  review reasons with a map are expected to be that record's: against the run without a map, `commentary_detected`
  goes from 4 discs and no other reason changes. None of part 13's three cases occurred in that record, so part 13
  should add no hold.
- Whatever the counts are, they go to the owner with slugs and numbers only. A hold that part 13 raises on a real disc
  is reported with its reason, and no rule is changed to make it go away.
- Publishing the 13 maps to the share follows only at the owner's word, with the share mounted on the Mac.

**Stage B, on the mini PC, only with the owner's explicit yes** for the build and the run, from a release zip, filing
into the test library, as pull request 7 did.
- `menu_maps_root` is set. One of the 11 discs on which the menu drops a track goes in. The job must file without the
  confirmed tracks, show the notes in `status` and `history`, hold the two copies, and pass `compare-job`. The maps
  folder must be as it was: the engine wrote nothing there.
- The owner plays the filed file once and checks that the film's audio is there and the commentary is not.
- One disc with no map goes in and must behave as it did before.
- If phase 3's fix-after is built by then: one drop is undone by a "keep" answer and the job files again.

## 17. The designer's calls, for the owner to check

Each is a call made here, with what it costs if it is wrong. The owner saw EM-1 to EM-14 with part 3 and approved the
part; EM-15 to EM-22 came up while this document was written and checked.

- **EM-1.** A disc's folder is named by its disc ID, not by a slug. Cost: a person cannot tell a disc from its folder
  name; `menu show` tells.
- **EM-2.** Only the two files are published. Cost: a later screen that wants the label pictures needs the maps
  published again.
- **EM-3.** An off line needs all three fields; a two-field line is refused. Cost: the 13 files are rewritten once.
- **EM-4.** A refused off file makes the whole pair unused. Cost: fewer drops until the pair is published again.
- **EM-5.** Every run of Decide looks up afresh, and a map removed from the share stops applying. Cost: a job decided
  twice can differ if the maps folder changed in between; the job's copy always says which map a run used.
- **EM-6.** A job keeps the `menu_maps_root` it started with (ENG-19). Cost: a job started before the setting was set
  never gets a map without ripping again.
- **EM-7.** Size limits of 4 MiB for a map and 1 MiB for an off file; the owner's largest are 56 KB and 287 bytes.
  Cost: a larger real map would be refused, and say so.
- **EM-8.** The job summary lists the decisions' notes. Cost: `status` and `history` grow for jobs that carry other
  notes.
- **EM-9.** An outcome that drops nothing raises no note, except the two of part 13.3. Cost: an override, a
  `no_reference` keep and the like are seen only in `decide.json`.
- **EM-10.** "Play's track is known" also requires that track to be among the kept ones. Cost: on an odd title a
  commentary stays.
- **EM-11.** No rule reads `made.by`. Cost: a map from a weaker maker acts like any other.
- **EM-12.** No undo command before phase 3's review. Cost: until then, undoing a drop means ripping again.
- **EM-13.** The self-test checks only that the maps folder answers and is a folder. Cost: a folder the engine cannot
  read shows at the first job, not at setup.
- **EM-14.** The audio confidence does not change with a menu drop (MR-14). Cost: a job with a menu drop files at the
  same confidence as one without.
- **EM-15.** A read error other than "not found" fails the step, as a share that does not answer does. Cost: a map
  file the engine may not open stops that disc's jobs at Decide until it is fixed or removed, where "decide without
  it" would have let them through; the failure names the file.
- **EM-16.** The job's copy is removed when a run finds no map or refuses one. Cost: the map an earlier run used is
  then only in the tuning copy of the recording, if that was made in between.
- **EM-17.** A missing map raises no note. Cost: which discs have no map is read from `job.log` or from the absence of
  the `menu` key, not from `status`.
- **EM-18.** When the menu confirms the reference, the title's other candidates are still dropped, and the job is
  held. Cost: if the map is wrong about the reference it may be wrong about them too; the owner sees them on the held
  job before it files.
- **EM-19.** `menu publish` refuses a stale off line, while the engine only ignores one. Cost: none known; a pair
  placed by hand is still read safely.
- **EM-20.** Two pull requests, `core` and `dev` first. Cost: `decide --menu` changes one pull request before any job
  does.
- **EM-21.** `menu_maps_root` is written into a settings object only when it is set, unlike the other roots. Cost: one
  key that behaves differently from its neighbours; an older lazymkv refuses a settings file that names it, as it
  refuses any key it does not know.
- **EM-22.** An off line whose id names no audio choice of the map is unused, not stale, and is refused nowhere.
  Cost: a leftover line from an older map is carried along until someone removes it; it acts on nothing.

## 18. Left to the builder, and left for later

Left to the builder: function, type and field names; the exact sentences of the notes and of `job.log`, within the
examples here; the order of the new keys in the `roles` block; the shape of the off listing; how a typed `--menu`
replaces the job's in the relay; how the stand-in disc gains its unit key and playlist; whether the summary's notes
say which title they are of.

Left for later, each its own design:

- **The engine captures what a map is made from.** Proposed as the step after this one: the job's snapshot would also
  copy the disc's Java programs, clip information and certificates, so a disc new to lazymkv could be mapped from its
  job folder with one insertion, and fix-after would then apply the map.
- Maps made without an agent (the menu maps design, 5.5), and non-Java discs (5.6).
- A list of jobs whose disc has no map, for a mapping batch.
- Subtitles and other roles, when the owner rules on them.
- More than one map for a disc, and any sharing of maps.
- The app's review screen showing the label pictures.
